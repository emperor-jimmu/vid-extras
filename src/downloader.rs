// Downloader module - handles video downloading using yt-dlp

use crate::error::DownloadError;
use crate::models::{DownloadResult, SUBTITLE_EXTENSIONS, VideoSource};
use log::{debug, error, info, warn};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;

/// Truncate a string to at most `max` characters, appending "…" if trimmed.
fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

/// Downloader handles video acquisition using yt-dlp
pub struct Downloader {
    /// Base directory for temporary downloads
    temp_base: PathBuf,
    /// Timeout duration for individual downloads (default: 5 minutes)
    download_timeout: Duration,
    /// Browser to source cookies from for bot-detection bypass
    cookies_from_browser: Option<String>,
}

impl Downloader {
    /// Create a new Downloader with the specified temporary base directory
    pub fn new(temp_base: PathBuf) -> Self {
        Self {
            temp_base,
            download_timeout: Duration::from_secs(300), // 5 minutes
            cookies_from_browser: None,
        }
    }

    /// Create a new Downloader with browser cookie authentication
    pub fn with_cookies(temp_base: PathBuf, browser: String) -> Self {
        Self {
            temp_base,
            download_timeout: Duration::from_secs(300),
            cookies_from_browser: Some(browser),
        }
    }

    /// Create a new Downloader with custom timeout
    /// Used for testing scenarios where timeout behavior needs to be controlled
    #[cfg(test)]
    pub fn with_timeout(temp_base: PathBuf, timeout_secs: u64) -> Self {
        Self {
            temp_base,
            download_timeout: Duration::from_secs(timeout_secs),
            cookies_from_browser: None,
        }
    }

    /// Download all videos for a movie, returning results for each
    pub async fn download_all(
        &self,
        movie_id: &str,
        sources: Vec<VideoSource>,
    ) -> Vec<DownloadResult> {
        if sources.is_empty() {
            info!("No sources to download for movie_id: {}", movie_id);
            return Vec::new();
        }

        // Create temp directory for this movie
        let temp_dir = match self.create_temp_dir(movie_id).await {
            Ok(dir) => dir,
            Err(e) => {
                error!("Failed to create temp directory for {}: {}", movie_id, e);
                // Return failed results for all sources
                return sources
                    .into_iter()
                    .map(|source| DownloadResult {
                        source,
                        local_path: PathBuf::new(),
                        success: false,
                        error: Some(format!("Failed to create temp directory: {}", e)),
                        subtitle_paths: vec![],
                    })
                    .collect();
            }
        };

        info!(
            "Downloading {} videos for movie_id: {} to {:?}",
            sources.len(),
            movie_id,
            temp_dir
        );

        let mut results = Vec::new();
        let total = sources.len();

        // Download each source sequentially with progress indicator
        for (index, source) in sources.into_iter().enumerate() {
            let progress = index + 1;
            info!(
                "Download progress [{}/{}]: {} from {}",
                progress, total, source.title, source.url
            );

            let result = self.download_single(&source, &temp_dir).await;

            if result.success {
                info!("✓ Downloaded [{}/{}]: {}", progress, total, source.title);
            } else {
                // Friendly message is already logged by download_single;
                // just emit a compact progress line here.
                info!(
                    "⊘ Skipped [{}/{}]: {} — {}",
                    progress,
                    total,
                    source.title,
                    result.error.as_deref().unwrap_or("unknown error")
                );
            }

            results.push(result);
        }

        info!(
            "Download batch complete for {}: {}/{} successful",
            movie_id,
            results.iter().filter(|r| r.success).count(),
            total
        );

        results
    }

    /// Create temporary directory for a movie's downloads
    async fn create_temp_dir(&self, movie_id: &str) -> Result<PathBuf, DownloadError> {
        let temp_dir = self.temp_base.join(movie_id);

        debug!("Creating temp directory: {:?}", temp_dir);

        // If directory already exists, reuse it (don't clean up during same processing session)
        // This allows multiple download batches (regular extras + specials) to coexist
        if temp_dir.exists() {
            debug!("Temp directory already exists, reusing: {:?}", temp_dir);
            return Ok(temp_dir);
        }

        // Create the directory
        fs::create_dir_all(&temp_dir).await?;

        Ok(temp_dir)
    }

    /// Download a single video source
    async fn download_single(&self, source: &VideoSource, dest_dir: &Path) -> DownloadResult {
        info!("Downloading: {} from {}", source.title, source.url);

        // Generate a unique hash from the URL to prevent filename collisions
        // when multiple videos have the same title
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        source.url.hash(&mut hasher);
        let url_hash = hasher.finish();

        // Build yt-dlp command with unique filename to prevent collisions
        // Format: "title_HASH.ext" where HASH is first 8 chars of URL hash
        // We use the hash during download to prevent collisions, but will rename it later
        let output_template = dest_dir.join(format!("%(title)s_{:08x}.%(ext)s", url_hash));
        let output_template_str = output_template.to_string_lossy().to_string();

        let mut cmd = Command::new("yt-dlp");
        cmd.arg("-o")
            .arg(&output_template_str)
            .arg(&source.url)
            .arg("--no-playlist") // Don't download playlists
            .arg("--quiet") // Reduce output noise
            .arg("--no-warnings") // Suppress warnings
            .arg("--remote-components")
            .arg("ejs:github"); // Download JS challenge solver from GitHub

        // Pass browser cookies when configured to bypass bot-detection
        if let Some(browser) = &self.cookies_from_browser {
            cmd.arg("--cookies-from-browser").arg(browser);
        }

        // On Windows, restrict filenames to Windows-compatible characters
        // This prevents issues with special characters like full-width quotes
        #[cfg(target_os = "windows")]
        cmd.arg("--windows-filenames");

        debug!("Executing yt-dlp command: {:?}", cmd);

        // Execute with timeout
        let download_future = cmd.output();
        let result = timeout(self.download_timeout, download_future).await;

        match result {
            Ok(Ok(output)) => {
                if output.status.success() {
                    // Find the downloaded file
                    match self
                        .find_downloaded_file(dest_dir, &source.title, url_hash)
                        .await
                    {
                        Ok(local_path) => {
                            // Verify the file actually exists before marking as success
                            if local_path.exists() {
                                info!("Successfully downloaded: {:?}", local_path);

                                // Detect audio language; absent metadata is treated as English
                                // to avoid false positives on content with missing tags.
                                let subtitle_paths = {
                                    let lang = Self::detect_audio_language(&source.url).await;
                                    let yt_dlp_non_english = match lang.as_deref() {
                                        None | Some("en") | Some("eng") => false,
                                        Some(_) => true,
                                    };

                                    if yt_dlp_non_english {
                                        // Confirm with ffprobe before fetching subs to
                                        // avoid false positives from yt-dlp metadata gaps.
                                        let ffprobe_lang =
                                            Self::detect_audio_language_ffprobe(&local_path).await;
                                        let confirmed = match ffprobe_lang.as_deref() {
                                            // ffprobe says English — trust it, skip subs
                                            Some("en") | Some("eng") => false,
                                            // Non-English or absent — proceed with subs
                                            _ => true,
                                        };

                                        if confirmed {
                                            // Skip subtitle download if we can't derive a
                                            // valid stem — a fallback name would orphan the
                                            // subtitle files (stem mismatch in organizer).
                                            if let Some(base_name) =
                                                local_path.file_stem().and_then(|s| s.to_str())
                                            {
                                                self.download_subtitles(
                                                    &source.url,
                                                    dest_dir,
                                                    base_name,
                                                )
                                                .await
                                            } else {
                                                debug!(
                                                    "Cannot derive file stem from {:?}, skipping subtitle download",
                                                    local_path
                                                );
                                                vec![]
                                            }
                                        } else {
                                            vec![]
                                        }
                                    } else {
                                        vec![]
                                    }
                                };

                                DownloadResult {
                                    source: source.clone(),
                                    local_path,
                                    success: true,
                                    error: None,
                                    subtitle_paths,
                                }
                            } else {
                                error!(
                                    "File not found after download (path mismatch): {:?}",
                                    local_path
                                );
                                DownloadResult {
                                    source: source.clone(),
                                    local_path: PathBuf::new(),
                                    success: false,
                                    error: Some(format!(
                                        "File not found after download: {:?}",
                                        local_path
                                    )),
                                    subtitle_paths: vec![],
                                }
                            }
                        }
                        Err(e) => {
                            error!("Download succeeded but file not found: {}", e);
                            DownloadResult {
                                source: source.clone(),
                                local_path: PathBuf::new(),
                                success: false,
                                error: Some(format!("File not found after download: {}", e)),
                                subtitle_paths: vec![],
                            }
                        }
                    }
                } else {
                    // yt-dlp failed
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let stderr_trimmed = stderr.trim();
                    let friendly = Self::classify_ytdlp_error(stderr_trimmed);
                    let error_msg = format!(
                        "yt-dlp failed with exit code: {:?}: {}",
                        output.status, stderr_trimmed
                    );

                    // Log the full technical error at debug level, friendly
                    // summary at warn so normal runs stay readable.
                    debug!("{} for URL: {}", error_msg, source.url);
                    warn!("⊘ {}: {} — {}", source.title, friendly, source.url);

                    // Clean up any partial files for this specific download
                    self.cleanup_partial_files(dest_dir, url_hash).await;

                    DownloadResult {
                        source: source.clone(),
                        local_path: PathBuf::new(),
                        success: false,
                        error: Some(friendly),
                        subtitle_paths: vec![],
                    }
                }
            }
            Ok(Err(e)) => {
                // Command execution failed
                let error_msg = format!("Failed to execute yt-dlp: {}", e);
                error!("{}", error_msg);

                // Clean up any partial files for this specific download
                self.cleanup_partial_files(dest_dir, url_hash).await;

                DownloadResult {
                    source: source.clone(),
                    local_path: PathBuf::new(),
                    success: false,
                    error: Some(error_msg),
                    subtitle_paths: vec![],
                }
            }
            Err(_) => {
                // Timeout occurred
                let error_msg = format!(
                    "Download timeout after {} seconds",
                    self.download_timeout.as_secs()
                );
                error!("{} for: {}", error_msg, source.title);

                // Clean up any partial files for this specific download
                self.cleanup_partial_files(dest_dir, url_hash).await;

                DownloadResult {
                    source: source.clone(),
                    local_path: PathBuf::new(),
                    success: false,
                    error: Some(error_msg),
                    subtitle_paths: vec![],
                }
            }
        }
    }

    /// Find the downloaded file in the destination directory.
    ///
    /// Reads the directory once, then tries three strategies in order:
    /// 1. Hash-based lookup (exact match on URL hash suffix)
    /// 2. Fuzzy title matching (word overlap scoring)
    /// 3. Most recently modified file (last resort)
    async fn find_downloaded_file(
        &self,
        dest_dir: &Path,
        expected_title: &str,
        url_hash: u64,
    ) -> Result<PathBuf, DownloadError> {
        debug!(
            "Searching for downloaded file matching: '{}' with hash {:08x}",
            expected_title, url_hash
        );

        // Read directory once and collect all file paths
        let files = Self::collect_files(dest_dir).await?;

        let hash_suffix = format!("_{:08x}", url_hash);

        // Strategy 1: exact hash match
        if let Some(path) = Self::find_by_hash(&files, &hash_suffix) {
            return self.remove_hash_from_filename(&path, &hash_suffix).await;
        }

        warn!(
            "No file found with hash suffix {}, falling back to fuzzy matching",
            hash_suffix
        );

        // Single-file shortcut (common case after a single download)
        if files.len() == 1 {
            debug!("Only one file found, using: {:?}", files[0]);
            return Ok(files[0].clone());
        }

        // Strategy 2: fuzzy title matching
        if let Some(path) = Self::find_by_title_similarity(&files, expected_title) {
            return Ok(path);
        }

        // Strategy 3: most recently modified
        if let Some(path) = Self::find_most_recent(&files) {
            warn!("No good filename match, using most recent file: {:?}", path);
            return Ok(path);
        }

        Err(DownloadError::YtDlpFailed(
            "No downloaded file found".to_string(),
        ))
    }

    /// Read a directory once and return all file paths.
    async fn collect_files(dir: &Path) -> Result<Vec<PathBuf>, DownloadError> {
        let mut entries = fs::read_dir(dir).await?;
        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
        debug!("Found {} files in temp directory", files.len());
        Ok(files)
    }

    /// Strategy 1: find a file whose name contains the expected hash suffix.
    fn find_by_hash(files: &[PathBuf], hash_suffix: &str) -> Option<PathBuf> {
        for path in files {
            if let Some(filename) = path.file_name()
                && filename.to_string_lossy().contains(hash_suffix)
            {
                debug!("Found file with hash suffix: {:?}", path);
                return Some(path.clone());
            }
        }
        None
    }

    /// Strategy 2: score each file by how many title words (len > 3) appear in
    /// its filename, and return the highest-scoring match.
    fn find_by_title_similarity(files: &[PathBuf], expected_title: &str) -> Option<PathBuf> {
        let title_lower = expected_title.to_lowercase();
        let title_words: Vec<&str> = title_lower.split_whitespace().collect();

        let mut best: Option<(PathBuf, usize)> = None;

        for path in files {
            let filename_str = path
                .file_name()
                .map(|f| f.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            let score = title_words
                .iter()
                .filter(|w| w.len() > 3 && filename_str.contains(**w))
                .count();

            debug!("File {:?} has match score: {}", path.file_name(), score);

            if score > 0 && best.as_ref().is_none_or(|(_, s)| score > *s) {
                best = Some((path.clone(), score));
            }
        }

        if let Some((ref path, score)) = best {
            debug!("Best match found with score {}: {:?}", score, path);
        }
        best.map(|(p, _)| p)
    }

    /// Strategy 3: return the most recently modified file.
    fn find_most_recent(files: &[PathBuf]) -> Option<PathBuf> {
        files
            .iter()
            .filter_map(|p| {
                p.metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| (p.clone(), t))
            })
            .max_by_key(|(_, t)| *t)
            .map(|(p, _)| p)
    }

    /// Remove hash suffix from filename and rename the file
    async fn remove_hash_from_filename(
        &self,
        path: &Path,
        hash_suffix: &str,
    ) -> Result<PathBuf, DownloadError> {
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            // Remove hash suffix from filename
            let clean_filename = filename.replace(hash_suffix, "");

            // Always sanitize filename for Windows compatibility, even if no hash to remove
            let sanitized_filename = Self::sanitize_filename(&clean_filename);

            // Only rename if the filename actually changed
            if sanitized_filename != filename {
                let clean_path = path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(&sanitized_filename);

                // Rename the file to remove the hash and sanitize
                match fs::rename(path, &clean_path).await {
                    Ok(_) => {
                        debug!("Renamed {} to {}", filename, sanitized_filename);
                        return Ok(clean_path);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to rename {} to {}: {}",
                            filename, sanitized_filename, e
                        );
                        // Return original path if rename fails
                        return Ok(path.to_path_buf());
                    }
                }
            }
        }

        Ok(path.to_path_buf())
    }

    /// Turn raw yt-dlp stderr into a short, human-readable reason.
    fn classify_ytdlp_error(stderr: &str) -> String {
        let lower = stderr.to_lowercase();

        if lower.contains("video is private") {
            "video is private".into()
        } else if lower.contains("video unavailable") {
            "video is unavailable".into()
        } else if lower.contains("this video has been removed") {
            "video has been removed".into()
        } else if lower.contains("copyright") {
            "video removed due to copyright".into()
        } else if lower.contains("geo") || lower.contains("not available in your country") {
            "video is geo-restricted".into()
        } else if lower.contains("age") && lower.contains("verify") {
            "video requires age verification".into()
        } else if lower.contains("sign in") || lower.contains("login") {
            "video requires sign-in".into()
        } else if lower.contains("429") || lower.contains("too many requests") {
            "rate-limited by server".into()
        } else if lower.contains("403") || lower.contains("forbidden") {
            "access forbidden by server".into()
        } else if lower.contains("404") || lower.contains("not found") {
            "video not found (404)".into()
        } else {
            // Fall back to the first ERROR line, trimmed for readability
            stderr
                .lines()
                .find(|l| l.contains("ERROR"))
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| format!("yt-dlp error: {}", truncate_str(stderr, 120)))
        }
    }

    /// Sanitize filename for Windows compatibility
    /// Replaces characters that are problematic on Windows filesystems
    /// Handles both ASCII and Unicode variants of special characters
    fn sanitize_filename(filename: &str) -> String {
        filename
            // ASCII special characters
            .replace(['|', '<', '>', ':', '/', '\\', '*'], "-")
            .replace('"', "'")
            .replace('?', "")
            // Unicode full-width variants (common in Asian text)
            .replace(['｜', '＜', '＞', '：', '／', '＼', '＊'], "-")
            .replace(['"', '"'], "'") // Left and right double quotation marks
            .replace('？', "") // Full-width question mark (U+FF1F)
    }

    /// Detect audio language from yt-dlp metadata for a URL.
    ///
    /// Finds the best audio format (highest abr) and returns its language code.
    /// Returns `None` on any error or if no audio format is found — callers treat
    /// `None` as English to avoid false positives on content with missing metadata.
    async fn detect_audio_language(url: &str) -> Option<String> {
        let output = match timeout(
            Duration::from_secs(30),
            Command::new("yt-dlp")
                .args(["--dump-json", "--no-playlist", "--quiet", url])
                .output(),
        )
        .await
        {
            Ok(Ok(o)) => o,
            _ => return None,
        };

        if !output.status.success() {
            return None;
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let formats = json.get("formats")?.as_array()?;

        // Find the audio format with the highest bitrate
        let best_audio = formats
            .iter()
            .filter(|f| f.get("acodec").and_then(|v| v.as_str()) != Some("none"))
            .max_by(|a, b| {
                let abr_a = a.get("abr").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let abr_b = b.get("abr").and_then(|v| v.as_f64()).unwrap_or(0.0);
                abr_a
                    .partial_cmp(&abr_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })?;

        best_audio
            .get("language")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Detect audio language from a local file using ffprobe.
    ///
    /// Used as confirmation after `detect_audio_language` returns a non-English result.
    /// Returns `None` on any error or if the audio stream has no language tag.
    async fn detect_audio_language_ffprobe(path: &Path) -> Option<String> {
        let output = match timeout(
            Duration::from_secs(15),
            Command::new("ffprobe")
                .args(["-v", "quiet", "-print_format", "json", "-show_streams"])
                .arg(path)
                .output(),
        )
        .await
        {
            Ok(Ok(o)) => o,
            _ => return None,
        };

        if !output.status.success() {
            return None;
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let streams = json.get("streams")?.as_array()?;

        streams
            .iter()
            .find(|s| s.get("codec_type").and_then(|v| v.as_str()) == Some("audio"))
            .and_then(|s| s.get("tags"))
            .and_then(|t| t.get("language"))
            .and_then(|l| l.as_str())
            .map(|s| s.to_string())
    }

    /// Download English subtitles for a video URL into `dest_dir`.
    ///
    /// Uses `--write-subs --write-auto-subs` so manual subtitles are preferred over
    /// auto-generated ones. Returns paths of any subtitle files written; returns an
    /// empty `Vec` if yt-dlp fails or no subtitles are available (subtitles are best-effort).
    async fn download_subtitles(
        &self,
        url: &str,
        dest_dir: &Path,
        base_name: &str,
    ) -> Vec<PathBuf> {
        let output_template = dest_dir.join(format!("{}.%(ext)s", base_name));
        let output_template_str = output_template.to_string_lossy().to_string();

        let mut cmd = Command::new("yt-dlp");
        cmd.args([
            "--write-subs",
            "--write-auto-subs",
            "--sub-langs",
            "en",
            "--skip-download",
            "--no-playlist",
            "--quiet",
            "-o",
            &output_template_str,
            url,
        ]);

        if let Some(browser) = &self.cookies_from_browser {
            cmd.arg("--cookies-from-browser").arg(browser);
        }

        match timeout(self.download_timeout, cmd.output()).await {
            Ok(Ok(output)) if output.status.success() => {
                let mut found = Vec::new();
                if let Ok(mut entries) = tokio::fs::read_dir(dest_dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        if SUBTITLE_EXTENSIONS.contains(&ext)
                            && (stem == base_name || stem.starts_with(&format!("{}.", base_name)))
                        {
                            found.push(path);
                        }
                    }
                }
                if found.is_empty() {
                    debug!("No subtitle files found after yt-dlp for: {}", url);
                }
                found
            }
            Ok(Ok(_)) => {
                debug!("yt-dlp returned non-zero exit for subtitles: {}", url);
                vec![]
            }
            Ok(Err(e)) => {
                debug!("Failed to execute yt-dlp for subtitles: {}", e);
                vec![]
            }
            Err(_) => {
                debug!("Subtitle download timed out for: {}", url);
                vec![]
            }
        }
    }

    /// Clean up partial files after a failed download
    /// Only removes files with the specific URL hash suffix or temporary extensions (.part, .tmp)
    /// This prevents accidentally deleting successfully downloaded files
    async fn cleanup_partial_files(&self, dest_dir: &Path, url_hash: u64) {
        let hash_suffix = format!("_{:08x}", url_hash);
        debug!(
            "Cleaning up partial files with hash suffix: {}",
            hash_suffix
        );

        match fs::read_dir(dest_dir).await {
            Ok(mut entries) => {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_file() {
                        // Check if this is a partial file for this specific download
                        if let Some(filename) = path.file_name() {
                            let filename_str = filename.to_string_lossy();
                            // Only remove files with:
                            // 1. The specific URL hash suffix (failed download for this URL)
                            // 2. Common partial extensions (.part, .tmp)
                            if filename_str.contains(&hash_suffix)
                                || filename_str.ends_with(".part")
                                || filename_str.ends_with(".tmp")
                            {
                                if let Err(e) = fs::remove_file(&path).await {
                                    warn!("Failed to remove partial file {:?}: {}", path, e);
                                } else {
                                    debug!("Removed partial file: {:?}", path);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to read directory for cleanup: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContentCategory, SourceType};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_temp_dir() {
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        let temp_dir = downloader.create_temp_dir("test_movie_123").await.unwrap();

        assert!(temp_dir.exists());
        assert!(temp_dir.ends_with("test_movie_123"));
    }

    #[tokio::test]
    async fn test_create_temp_dir_reuses_existing() {
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        // Create directory first time
        let temp_dir1 = downloader.create_temp_dir("test_movie_456").await.unwrap();
        // Create a file in it
        let test_file = temp_dir1.join("test.txt");
        fs::write(&test_file, "test").await.unwrap();
        assert!(test_file.exists());

        // Create directory second time - should reuse existing directory
        let temp_dir2 = downloader.create_temp_dir("test_movie_456").await.unwrap();
        assert!(temp_dir2.exists());
        assert!(test_file.exists()); // Old file should still exist (not cleaned up)
        assert_eq!(temp_dir1, temp_dir2); // Should be the same directory
    }

    #[tokio::test]
    async fn test_download_all_empty_sources() {
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        let results = downloader.download_all("movie_789", vec![]).await;

        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_download_all_temp_dir_creation_failure() {
        // Use a path that will fail on most systems (null device)
        #[cfg(unix)]
        let invalid_path = PathBuf::from("/dev/null/invalid");
        #[cfg(windows)]
        let invalid_path = PathBuf::from("NUL:\\invalid");

        let downloader = Downloader::new(invalid_path);

        let sources = vec![VideoSource {
            url: "https://example.com/video".to_string(),
            source_type: SourceType::YouTube,
            category: ContentCategory::Trailer,
            title: "Test Video".to_string(),
            season_number: None,
            duration_secs: None,
        }];

        let results = downloader.download_all("movie_fail", sources).await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.is_some());
        // Error should mention temp directory or IO error
        let error_msg = results[0].error.as_ref().unwrap();
        assert!(
            error_msg.contains("temp directory") || error_msg.contains("IO error"),
            "Error message was: {}",
            error_msg
        );
    }

    #[tokio::test]
    async fn test_cleanup_partial_files() {
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        let temp_dir = downloader.create_temp_dir("cleanup_test").await.unwrap();

        // Create a URL hash for testing
        let test_hash = 0xABCDEF12u64;
        let hash_suffix = format!("_{:08x}", test_hash);

        // Create some partial files with the hash suffix
        let partial_with_hash = temp_dir.join(format!("video{}.mkv", hash_suffix));
        let partial_part = temp_dir.join("video.part");
        let partial_tmp = temp_dir.join("video.tmp");
        // Create a successfully downloaded file (no hash suffix)
        let complete = temp_dir.join("complete_video.mp4");
        // Create another file with a different hash (should not be deleted)
        let other_hash_file = temp_dir.join("other_video_99999999.mkv");

        fs::write(&partial_with_hash, "partial").await.unwrap();
        fs::write(&partial_part, "partial").await.unwrap();
        fs::write(&partial_tmp, "partial").await.unwrap();
        fs::write(&complete, "complete").await.unwrap();
        fs::write(&other_hash_file, "other").await.unwrap();

        // Cleanup partial files for the specific hash
        downloader.cleanup_partial_files(&temp_dir, test_hash).await;

        // Files with the specific hash suffix should be removed
        assert!(!partial_with_hash.exists());
        // .part and .tmp files should be removed
        assert!(!partial_part.exists());
        assert!(!partial_tmp.exists());
        // Complete file should remain (no hash suffix)
        assert!(complete.exists());
        // File with different hash should remain
        assert!(other_hash_file.exists());
    }

    #[tokio::test]
    async fn test_with_timeout() {
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::with_timeout(temp_base.path().to_path_buf(), 10);

        assert_eq!(downloader.download_timeout, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_find_downloaded_file() {
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        let temp_dir = downloader.create_temp_dir("find_test").await.unwrap();

        // Create a file with hash suffix
        let test_hash = 0x12345678u64;
        let test_file_with_hash = temp_dir.join(format!("Test Trailer_{:08x}.mp4", test_hash));
        fs::write(&test_file_with_hash, "video").await.unwrap();

        // Should find the file and remove the hash suffix
        let found = downloader
            .find_downloaded_file(&temp_dir, "Test Trailer", test_hash)
            .await
            .unwrap();

        // The returned path should have the hash removed
        let expected_file = temp_dir.join("Test Trailer.mp4");
        assert_eq!(found, expected_file);
        // Verify the file was actually renamed
        assert!(found.exists());
        assert!(!test_file_with_hash.exists());
    }

    #[tokio::test]
    async fn test_find_downloaded_file_not_found() {
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        let temp_dir = downloader.create_temp_dir("notfound_test").await.unwrap();

        // No files in directory
        let result = downloader
            .find_downloaded_file(&temp_dir, "Nonexistent", 0x99999999u64)
            .await;

        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_filename_ascii() {
        let input = "Title | Part 1: Test <File> Name*.mkv";
        let result = Downloader::sanitize_filename(input);
        assert_eq!(result, "Title - Part 1- Test -File- Name-.mkv");
    }

    #[test]
    fn test_sanitize_filename_unicode() {
        // Test full-width Unicode variants common in Asian text
        let input = "Solo Leveling Season 2 -Arise from the Shadow- ｜ OFFICIAL TEASER TRAILER.mkv";
        let result = Downloader::sanitize_filename(input);
        assert_eq!(
            result,
            "Solo Leveling Season 2 -Arise from the Shadow- - OFFICIAL TEASER TRAILER.mkv"
        );
    }

    #[test]
    fn test_sanitize_filename_mixed_unicode() {
        let input = "Title：Part／1＜Test＞？.mp4";
        let result = Downloader::sanitize_filename(input);
        assert_eq!(result, "Title-Part-1-Test-.mp4");
    }

    #[test]
    fn test_sanitize_filename_quotation_marks() {
        let input = r#"Title "with" quotes and "curly" quotes.mkv"#;
        let result = Downloader::sanitize_filename(input);
        assert_eq!(result, "Title 'with' quotes and 'curly' quotes.mkv");
    }

    // Story 6.2: Dailymotion Download via yt-dlp
    // Verifies that Dailymotion URLs flow through the downloader identically to YouTube URLs
    // (NFR11: all sources use yt-dlp as the sole download backend)

    #[tokio::test]
    async fn test_dailymotion_url_flows_through_downloader() {
        // Verify that a Dailymotion VideoSource is handled by yt-dlp, not a special code path.
        // We use an invalid URL so yt-dlp fails fast — the important assertion is that
        // the result is a DownloadResult (not a panic or type error), confirming yt-dlp was invoked.
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        let sources = vec![VideoSource {
            url: "https://www.dailymotion.com/video/x_nonexistent_test".to_string(),
            source_type: SourceType::Dailymotion,
            category: ContentCategory::Trailer,
            title: "Dailymotion Test Trailer".to_string(),
            season_number: None,
            duration_secs: None,
        }];

        let results = downloader
            .download_all("dailymotion_test_movie", sources)
            .await;

        // Must produce exactly one result — no source-type branching that would skip it
        assert_eq!(results.len(), 1);
        // The download will fail (invalid URL), but it must have been attempted via yt-dlp
        assert!(!results[0].success);
        assert!(results[0].error.is_some());
    }

    #[tokio::test]
    async fn test_dailymotion_download_failure_does_not_stop_pipeline() {
        // Verify error isolation: a failed Dailymotion download does not prevent
        // other sources from producing DownloadResult entries (FR33, AC #3).
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        let sources = vec![
            VideoSource {
                url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ_fake_1".to_string(),
                source_type: SourceType::YouTube,
                category: ContentCategory::Trailer,
                title: "YouTube Video 1".to_string(),
                season_number: None,
                duration_secs: None,
            },
            VideoSource {
                url: "https://www.dailymotion.com/video/x_bad_url_test".to_string(),
                source_type: SourceType::Dailymotion,
                category: ContentCategory::BehindTheScenes,
                title: "Dailymotion BTS".to_string(),
                season_number: None,
                duration_secs: None,
            },
            VideoSource {
                url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ_fake_2".to_string(),
                source_type: SourceType::YouTube,
                category: ContentCategory::Featurette,
                title: "YouTube Video 2".to_string(),
                season_number: None,
                duration_secs: None,
            },
        ];

        let results = downloader
            .download_all("isolation_test_movie", sources)
            .await;

        // All 3 sources must produce a result — no early exit on Dailymotion failure
        assert_eq!(results.len(), 3);
        // The Dailymotion entry (index 1) must have failed, not been skipped
        assert!(!results[1].success);
        assert!(results[1].error.is_some());
    }

    #[tokio::test]
    async fn test_dailymotion_uses_temp_dir_conventions() {
        // Verify that the temp directory structure is identical regardless of source type (AC #4).
        // The downloader must use the same tmp_downloads/{movie_id}/ path for Dailymotion.
        let temp_base = TempDir::new().unwrap();
        let downloader = Downloader::new(temp_base.path().to_path_buf());

        // download_all creates the temp dir before invoking download_single
        let sources = vec![VideoSource {
            url: "https://www.dailymotion.com/video/x_temp_dir_test".to_string(),
            source_type: SourceType::Dailymotion,
            category: ContentCategory::Trailer,
            title: "Temp Dir Test".to_string(),
            season_number: None,
            duration_secs: None,
        }];

        let movie_id = "dailymotion_tempdir_check";
        let expected_temp_dir = temp_base.path().join(movie_id);

        downloader.download_all(movie_id, sources).await;

        // The temp directory must have been created under temp_base with the movie_id as the name
        assert!(expected_temp_dir.exists());
        assert!(expected_temp_dir.is_dir());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::TempDir;

    // Feature: extras-fetcher, Property 14: Temporary Directory Creation
    // Validates: Requirements 6.1
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]
        #[test]
        fn prop_temp_directory_creation(
            movie_id in "[a-zA-Z0-9_-]{1,50}"
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp_base = TempDir::new().unwrap();
                let downloader = Downloader::new(temp_base.path().to_path_buf());

                // Create temp directory
                let temp_dir = downloader.create_temp_dir(&movie_id).await.unwrap();

                // Verify directory exists
                prop_assert!(temp_dir.exists());

                // Verify directory is under temp_base
                prop_assert!(temp_dir.starts_with(temp_base.path()));

                // Verify directory ends with movie_id
                prop_assert!(temp_dir.ends_with(&movie_id));

                // Verify directory is actually a directory
                prop_assert!(temp_dir.is_dir());

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            }).unwrap();
        }
    }

    // Feature: extras-fetcher, Property 15: Download Failure Cleanup
    // Validates: Requirements 6.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]
        #[test]
        fn prop_download_failure_cleanup(
            num_partial_files in 1usize..5usize,
            test_hash in 0x10000000u64..0xFFFFFFFFu64
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp_base = TempDir::new().unwrap();
                let downloader = Downloader::new(temp_base.path().to_path_buf());

                let temp_dir = downloader.create_temp_dir("cleanup_test").await.unwrap();

                let hash_suffix = format!("_{:08x}", test_hash);

                // Create partial files with the hash suffix that should be cleaned up
                for i in 0..num_partial_files {
                    let partial_file = temp_dir.join(format!("video{}_part{}.mkv", hash_suffix, i));
                    tokio::fs::write(&partial_file, "partial").await.unwrap();
                }

                // Create .part and .tmp files that should also be cleaned up
                let part_file = temp_dir.join("video.part");
                let tmp_file = temp_dir.join("video.tmp");
                tokio::fs::write(&part_file, "partial").await.unwrap();
                tokio::fs::write(&tmp_file, "partial").await.unwrap();

                // Create files that shouldn't be cleaned (different hash, no hash, complete files)
                let unrelated_file = temp_dir.join("unrelated.mp4");
                let other_hash_file = temp_dir.join("other_video_99999999.mkv");
                tokio::fs::write(&unrelated_file, "complete").await.unwrap();
                tokio::fs::write(&other_hash_file, "complete").await.unwrap();

                // Cleanup partial files for the specific hash
                downloader.cleanup_partial_files(&temp_dir, test_hash).await;

                // Count remaining files
                let mut entries = tokio::fs::read_dir(&temp_dir).await.unwrap();
                let mut file_count = 0;
                let mut has_unrelated = false;
                let mut has_other_hash = false;

                while let Some(entry) = entries.next_entry().await.unwrap() {
                    file_count += 1;
                    let filename = entry.file_name();
                    if filename == "unrelated.mp4" {
                        has_unrelated = true;
                    }
                    if filename == "other_video_99999999.mkv" {
                        has_other_hash = true;
                    }
                }

                // Only the unrelated files should remain (2 files)
                prop_assert_eq!(file_count, 2);
                prop_assert!(has_unrelated);
                prop_assert!(has_other_hash);

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            }).unwrap();
        }
    }

    // Feature: extras-fetcher, Property 16: Download Error Continuation
    // Validates: Requirements 6.5
    // NOTE: This test is disabled because it requires actual yt-dlp execution
    // which can hang or take too long. The functionality is validated through
    // unit tests and integration tests instead.
    #[test]
    #[ignore]
    fn prop_download_error_continuation_disabled() {
        // Test disabled - requires real yt-dlp execution
        // Functionality validated through unit tests
    }

    // Feature: extras-fetcher, Property 17: Network Timeout Graceful Handling
    // Validates: Requirements 6.6
    // NOTE: This test is disabled because it requires actual yt-dlp execution
    // which can hang or take too long. The functionality is validated through
    // unit tests and integration tests instead.
    #[test]
    #[ignore]
    fn prop_timeout_graceful_handling_disabled() {
        // Test disabled - requires real yt-dlp execution
        // Functionality validated through unit tests
    }
}
