// Organizer module - moves converted files to Jellyfin subdirectories and creates done markers

use crate::error::OrganizerError;
use crate::models::{
    ContentCategory, ConversionResult, DoneMarker, SUBTITLE_EXTENSIONS, SpecialEpisode,
};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Check if a filename is opaque numeric (stem consists entirely of digits)
fn is_opaque_numeric_filename(path: &Path) -> bool {
    path.file_stem()
        .and_then(|s| s.to_str())
        .is_some_and(|stem| !stem.is_empty() && stem.chars().all(|c| c.is_ascii_digit()))
}

/// Determine the final filename for a file being organized.
/// Opaque numeric filenames (e.g., `10032.mp4`) are normalized to `{Category} #{N}.{ext}`.
/// Descriptive filenames are preserved as-is. All results are sanitized for Windows compatibility.
fn normalize_filename(path: &Path, category: ContentCategory, counter: usize) -> String {
    let raw = if is_opaque_numeric_filename(path) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp4")
            .to_lowercase();
        format!("{} #{}.{}", category, counter, ext)
    } else {
        // Preserve the original filename. If the path has no filename component
        // (e.g., a bare directory path), log a warning and skip — returning an
        // empty string here would cause move_file to target the directory itself.
        match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => {
                warn!(
                    "normalize_filename: path {:?} has no filename component, skipping",
                    path
                );
                return String::new();
            }
        }
    };
    sanitize_filename(&raw)
}

/// Sanitize filename for Windows compatibility.
/// Handles both ASCII and Unicode variants of special characters.
fn sanitize_filename(filename: &str) -> String {
    filename
        .replace(['|', '<', '>', ':', '/', '\\', '*'], "-")
        .replace('"', "'")
        .replace('?', "")
        .replace(['｜', '＜', '＞', '：', '／', '＼', '＊'], "-")
        .replace(['\u{201c}', '\u{201d}'], "'")
        .replace('？', "")
}

/// Move subtitle sidecar files that share the same stem as `video_path` into `dest_dir`.
///
/// yt-dlp writes subtitles as `{stem}.{lang}.{format}` (e.g., `My Trailer.en.vtt`).
/// Subtitle files are never renamed — they keep their yt-dlp-assigned names.
/// Failures are logged at `warn!` level and do not abort the organize operation.
async fn move_sibling_subtitles(video_path: &Path, dest_dir: &Path) {
    let stem = match video_path.file_stem().and_then(|s| s.to_str()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return,
    };

    let parent = video_path.parent().unwrap_or(Path::new("."));

    let mut entries = match tokio::fs::read_dir(parent).await {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to read dir for subtitle scan {:?}: {}", parent, e);
            return;
        }
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let entry_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        if SUBTITLE_EXTENSIONS.contains(&ext)
            && (entry_stem == stem || entry_stem.starts_with(&format!("{}.", stem)))
            && let Some(filename) = path.file_name()
        {
            let dest = dest_dir.join(filename);
            match tokio::fs::rename(&path, &dest).await {
                Ok(_) => {
                    debug!("Moved subtitle {:?} -> {:?}", path, dest);
                }
                Err(e) if e.raw_os_error() == Some(17) => {
                    // Cross-drive move on Windows — fall back to copy+delete
                    if let Err(e) = tokio::fs::copy(&path, &dest).await {
                        warn!("Failed to copy subtitle {:?} to {:?}: {}", path, dest, e);
                    } else if let Err(e) = tokio::fs::remove_file(&path).await {
                        warn!("Failed to delete subtitle source {:?}: {}", path, e);
                    } else {
                        debug!("Copied subtitle {:?} -> {:?}", path, dest);
                    }
                }
                Err(e) => {
                    warn!("Failed to move subtitle {:?} to {:?}: {}", path, dest, e);
                }
            }
        }
    }
}

/// Organizer handles file organization into Jellyfin-compatible directory structure
pub struct Organizer {
    /// Path to the movie folder
    movie_path: PathBuf,
}

impl Organizer {
    /// Create a new Organizer for a specific movie folder
    pub fn new(movie_path: PathBuf) -> Self {
        Self { movie_path }
    }

    /// Organize converted files into appropriate subdirectories and create done marker
    ///
    /// This method:
    /// 1. Creates subdirectories for each content category if they don't exist
    /// 2. Moves converted files to their appropriate subdirectories
    /// 3. Cleans up the temporary download folder
    /// 4. Creates a done marker file to indicate completion
    pub async fn organize(
        &self,
        conversions: Vec<ConversionResult>,
        temp_dir: &Path,
    ) -> Result<(), OrganizerError> {
        info!(
            "Organizing {} files for movie at {:?}",
            conversions.len(),
            self.movie_path
        );

        // Group conversions by category
        let mut files_by_category: std::collections::HashMap<ContentCategory, Vec<PathBuf>> =
            std::collections::HashMap::new();

        for conversion in conversions {
            if !conversion.success {
                warn!("Skipping failed conversion: {:?}", conversion.input_path);
                continue;
            }

            // Verify the output file actually exists before trying to move it
            if !conversion.output_path.exists() {
                warn!(
                    "Skipping conversion with missing output file: {:?}",
                    conversion.output_path
                );
                continue;
            }

            // Use the category from the conversion result
            files_by_category
                .entry(conversion.category)
                .or_default()
                .push(conversion.output_path);
        }

        // Move files to their subdirectories
        let mut category_counters: HashMap<ContentCategory, usize> = HashMap::new();
        for (category, files) in files_by_category {
            let subdir = self.ensure_subdirectory(category).await?;

            for file_path in files {
                let counter = category_counters.entry(category).or_insert(0);
                *counter += 1;
                let n = *counter;
                let dest_filename = normalize_filename(&file_path, category, n);
                if dest_filename.is_empty() {
                    warn!(
                        "Skipping file with no valid filename component: {:?}",
                        file_path
                    );
                    continue;
                }
                self.move_file(&file_path, &subdir, &dest_filename).await?;
                move_sibling_subtitles(&file_path, &subdir).await;
            }
        }

        // Clean up temp directory
        self.cleanup_temp_dir(temp_dir).await?;

        // Create done marker
        self.create_done_marker().await?;

        info!("Organization complete for {:?}", self.movie_path);
        Ok(())
    }

    /// Ensure a subdirectory exists for the given content category
    async fn ensure_subdirectory(
        &self,
        category: ContentCategory,
    ) -> Result<PathBuf, OrganizerError> {
        let subdir_name = category.subdirectory();
        let subdir_path = self.movie_path.join(subdir_name);

        debug!("Ensuring subdirectory exists: {:?}", subdir_path);

        if !subdir_path.exists() {
            fs::create_dir_all(&subdir_path).await.map_err(|e| {
                OrganizerError::SubdirectoryCreation(format!(
                    "Failed to create {:?}: {}",
                    subdir_path, e
                ))
            })?;
            info!("Created subdirectory: {:?}", subdir_path);
        }

        Ok(subdir_path)
    }

    /// Move a file to the target subdirectory with the given destination filename
    async fn move_file(
        &self,
        source: &Path,
        dest_dir: &Path,
        dest_filename: &str,
    ) -> Result<(), OrganizerError> {
        let dest_path = dest_dir.join(dest_filename);

        debug!("Moving file: {:?} -> {:?}", source, dest_path);

        // Try rename first (fast, atomic), but fall back to copy+delete for cross-drive moves
        match fs::rename(source, &dest_path).await {
            Ok(_) => {
                info!("Moved file to: {:?}", dest_path);
                Ok(())
            }
            Err(e) if e.raw_os_error() == Some(17) => {
                // Error 17 on Windows: "The system cannot move the file to a different disk drive"
                // Fall back to copy + delete
                debug!(
                    "Cross-drive move detected, using copy+delete: {:?} -> {:?}",
                    source, dest_path
                );

                fs::copy(source, &dest_path).await.map_err(|e| {
                    OrganizerError::FileMove(format!(
                        "Failed to copy {:?} to {:?}: {}",
                        source, dest_path, e
                    ))
                })?;

                fs::remove_file(source).await.map_err(|e| {
                    OrganizerError::FileMove(format!(
                        "Failed to delete source file {:?} after copy: {}",
                        source, e
                    ))
                })?;

                info!("Copied and deleted file to: {:?}", dest_path);
                Ok(())
            }
            Err(e) => Err(OrganizerError::FileMove(format!(
                "Failed to move {:?} to {:?}: {}",
                source, dest_path, e
            ))),
        }
    }

    /// Clean up the temporary download directory
    async fn cleanup_temp_dir(&self, temp_dir: &Path) -> Result<(), OrganizerError> {
        if !temp_dir.exists() {
            debug!(
                "Temp directory does not exist, skipping cleanup: {:?}",
                temp_dir
            );
            return Ok(());
        }

        debug!("Cleaning up temp directory: {:?}", temp_dir);

        fs::remove_dir_all(temp_dir).await.map_err(|e| {
            // Log as warning but don't fail the operation
            warn!("Failed to cleanup temp directory {:?}: {}", temp_dir, e);
            OrganizerError::Io(e)
        })?;

        info!("Cleaned up temp directory: {:?}", temp_dir);
        Ok(())
    }

    /// Create a done marker file indicating successful completion
    async fn create_done_marker(&self) -> Result<(), OrganizerError> {
        let marker_path = self.movie_path.join("done.ext");

        debug!("Creating done marker: {:?}", marker_path);

        let marker = DoneMarker {
            finished_at: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let json = serde_json::to_string_pretty(&marker)
            .map_err(|e| OrganizerError::Io(std::io::Error::other(e)))?;

        fs::write(&marker_path, json).await?;

        info!("Created done marker: {:?}", marker_path);
        Ok(())
    }
}

/// SeriesOrganizer handles file organization for TV series into Jellyfin-compatible structure
pub struct SeriesOrganizer {
    /// Path to the series folder
    series_path: PathBuf,
    /// Available seasons on disk (for validation)
    available_seasons: Vec<u8>,
}

impl SeriesOrganizer {
    /// Create a new SeriesOrganizer for a specific series folder
    pub fn new(series_path: PathBuf, available_seasons: Vec<u8>) -> Self {
        Self {
            series_path,
            available_seasons,
        }
    }

    /// Organize series extras into appropriate subdirectories
    ///
    /// This method:
    /// 1. Creates subdirectories for each content category if they don't exist
    /// 2. Moves converted files to their appropriate subdirectories
    /// 3. Supports both series-level extras (no season) and season-specific extras
    pub async fn organize_extras(
        &self,
        conversions: Vec<ConversionResult>,
        season: Option<u8>,
    ) -> Result<(), OrganizerError> {
        // Validate that the season exists on disk if specified
        if let Some(s) = season
            && !self.available_seasons.contains(&s)
        {
            warn!(
                "Skipping organization for non-existent season {} at {:?}",
                s, self.series_path
            );
            return Ok(());
        }

        info!(
            "Organizing {} files for series at {:?}, season: {:?}",
            conversions.len(),
            self.series_path,
            season
        );

        // Group conversions by category
        let mut files_by_category: std::collections::HashMap<ContentCategory, Vec<PathBuf>> =
            std::collections::HashMap::new();

        for conversion in conversions {
            if !conversion.success {
                warn!("Skipping failed conversion: {:?}", conversion.input_path);
                continue;
            }

            // Verify the output file actually exists before trying to move it
            if !conversion.output_path.exists() {
                warn!(
                    "Skipping conversion with missing output file: {:?}",
                    conversion.output_path
                );
                continue;
            }

            files_by_category
                .entry(conversion.category)
                .or_default()
                .push(conversion.output_path);
        }

        // Move files to their subdirectories
        let mut category_counters: HashMap<ContentCategory, usize> = HashMap::new();
        for (category, files) in files_by_category {
            let subdir = self.ensure_subdirectory(category, season).await?;

            for file_path in files {
                let counter = category_counters.entry(category).or_insert(0);
                *counter += 1;
                let n = *counter;
                let dest_filename = normalize_filename(&file_path, category, n);
                if dest_filename.is_empty() {
                    warn!(
                        "Skipping file with no valid filename component: {:?}",
                        file_path
                    );
                    continue;
                }
                self.move_file(&file_path, &subdir, &dest_filename).await?;
                move_sibling_subtitles(&file_path, &subdir).await;
            }
        }

        info!(
            "Organization complete for series at {:?}, season: {:?}",
            self.series_path, season
        );
        Ok(())
    }

    /// Organize Season 0 special episodes with Sonarr-compatible naming
    ///
    /// This method:
    /// 1. Creates a specials folder (default: "Season 00", configurable via folder_name)
    /// 2. Formats filenames as "{Series Name} - S00E{num} - {title}.mkv" (Sonarr pattern)
    /// 3. Zero-pads episode numbers using aired_episode_number from TVDB
    /// 4. Sanitizes filenames to remove Windows-invalid characters
    /// 5. Skips files when target already exists
    ///
    /// # Arguments
    /// * `series_name` - Name of the series for filename formatting
    /// * `specials` - List of special episodes to organize
    /// * `folder_name` - Name of the folder for specials (e.g., "Specials", "Season 00")
    ///
    /// # Requirements
    /// Validates: 7.1, 7.2, 7.3, 7.5
    pub async fn organize_specials(
        &self,
        series_name: &str,
        specials: Vec<SpecialEpisode>,
        folder_name: &str,
    ) -> Result<(), OrganizerError> {
        if specials.is_empty() {
            debug!("No specials to organize");
            return Ok(());
        }

        info!(
            "Organizing {} special episodes for series at {:?} into folder '{}'",
            specials.len(),
            self.series_path,
            folder_name
        );

        let specials_dir = self.series_path.join(folder_name);
        tokio::fs::create_dir_all(&specials_dir)
            .await
            .map_err(|e| {
                OrganizerError::SubdirectoryCreation(format!(
                    "Failed to create {} folder: {}",
                    folder_name, e
                ))
            })?;

        for special in specials {
            if let Some(local_path) = special.local_path {
                // Sanitize both series name and episode title for filename safety
                let sanitized_series = Self::sanitize_filename(series_name);
                let sanitized_title = Self::sanitize_filename(&special.title);

                // Sonarr-compatible naming: "{series_title} - S00E{episode_number:02} - {sanitized_title}.mkv"
                // Use aired_episode_number from TVDB (stored in episode_number field)
                let filename = format!(
                    "{} - S00E{:02} - {}.mkv",
                    sanitized_series, special.episode_number, sanitized_title
                );

                let target_path = specials_dir.join(&filename);

                // Skip if target already exists (Requirement 7.5)
                if target_path.exists() {
                    info!(
                        "Skipping special episode, target already exists: {:?}",
                        target_path
                    );
                    continue;
                }

                debug!(
                    "Moving special episode: {:?} -> {:?}",
                    local_path, target_path
                );

                // Try rename first (fast, atomic), but fall back to copy+delete for cross-drive moves
                match tokio::fs::rename(&local_path, &target_path).await {
                    Ok(_) => {
                        info!("Moved special episode to: {:?}", target_path);
                    }
                    Err(e) if e.raw_os_error() == Some(17) => {
                        // Error 17 on Windows: cross-drive move
                        debug!(
                            "Cross-drive move detected, using copy+delete: {:?} -> {:?}",
                            local_path, target_path
                        );

                        tokio::fs::copy(&local_path, &target_path)
                            .await
                            .map_err(|e| {
                                OrganizerError::FileMove(format!(
                                    "Failed to copy {:?} to {:?}: {}",
                                    local_path, target_path, e
                                ))
                            })?;

                        tokio::fs::remove_file(&local_path).await.map_err(|e| {
                            OrganizerError::FileMove(format!(
                                "Failed to delete source file {:?} after copy: {}",
                                local_path, e
                            ))
                        })?;

                        info!("Copied and deleted special episode to: {:?}", target_path);
                    }
                    Err(e) => {
                        return Err(OrganizerError::FileMove(format!(
                            "Failed to move {:?} to {:?}: {}",
                            local_path, target_path, e
                        )));
                    }
                }
            }
        }

        info!("Special episodes organization complete");
        Ok(())
    }

    /// Ensure a subdirectory exists for the given content category
    /// Resolve the on-disk season folder for a given season number.
    ///
    /// Checks common naming variants (e.g. "Season 2", "Season 02") and returns
    /// the first match. Falls back to zero-padded format if none exists yet.
    fn resolve_season_folder(&self, season: u8) -> PathBuf {
        let candidates = [
            format!("Season {}", season),
            format!("Season {:02}", season),
        ];

        for candidate in &candidates {
            let path = self.series_path.join(candidate);
            if path.is_dir() {
                return path;
            }
        }

        // No existing folder found — use zero-padded format as default
        self.series_path.join(format!("Season {:02}", season))
    }

    async fn ensure_subdirectory(
        &self,
        category: ContentCategory,
        season: Option<u8>,
    ) -> Result<PathBuf, OrganizerError> {
        let subdir_name = category.subdirectory();

        let subdir_path = if let Some(s) = season {
            // Use the existing season folder on disk if present
            self.resolve_season_folder(s).join(subdir_name)
        } else {
            // Series-level subdirectory
            self.series_path.join(subdir_name)
        };

        debug!("Ensuring subdirectory exists: {:?}", subdir_path);

        if !subdir_path.exists() {
            fs::create_dir_all(&subdir_path).await.map_err(|e| {
                OrganizerError::SubdirectoryCreation(format!(
                    "Failed to create {:?}: {}",
                    subdir_path, e
                ))
            })?;
            info!("Created subdirectory: {:?}", subdir_path);
        }

        Ok(subdir_path)
    }

    /// Move a file to the target subdirectory with the given destination filename
    async fn move_file(
        &self,
        source: &Path,
        dest_dir: &Path,
        dest_filename: &str,
    ) -> Result<(), OrganizerError> {
        let dest_path = dest_dir.join(dest_filename);

        debug!("Moving file: {:?} -> {:?}", source, dest_path);

        // Try rename first (fast, atomic), but fall back to copy+delete for cross-drive moves
        match fs::rename(source, &dest_path).await {
            Ok(_) => {
                info!("Moved file to: {:?}", dest_path);
                Ok(())
            }
            Err(e) if e.raw_os_error() == Some(17) => {
                // Error 17 on Windows: "The system cannot move the file to a different disk drive"
                // Fall back to copy + delete
                debug!(
                    "Cross-drive move detected, using copy+delete: {:?} -> {:?}",
                    source, dest_path
                );

                fs::copy(source, &dest_path).await.map_err(|e| {
                    OrganizerError::FileMove(format!(
                        "Failed to copy {:?} to {:?}: {}",
                        source, dest_path, e
                    ))
                })?;

                fs::remove_file(source).await.map_err(|e| {
                    OrganizerError::FileMove(format!(
                        "Failed to delete source file {:?} after copy: {}",
                        source, e
                    ))
                })?;

                info!("Copied and deleted file to: {:?}", dest_path);
                Ok(())
            }
            Err(e) => Err(OrganizerError::FileMove(format!(
                "Failed to move {:?} to {:?}: {}",
                source, dest_path, e
            ))),
        }
    }

    /// Sanitize filename by removing invalid characters
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ensure_subdirectory_creates_missing_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        let subdir = organizer
            .ensure_subdirectory(ContentCategory::Trailer)
            .await
            .unwrap();

        assert!(subdir.exists());
        assert_eq!(subdir, movie_path.join("trailers"));
    }

    #[tokio::test]
    async fn test_ensure_subdirectory_handles_existing_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let trailers_dir = movie_path.join("trailers");
        fs::create_dir(&trailers_dir).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        let subdir = organizer
            .ensure_subdirectory(ContentCategory::Trailer)
            .await
            .unwrap();

        assert!(subdir.exists());
        assert_eq!(subdir, trailers_dir);
    }

    #[tokio::test]
    async fn test_move_file_success() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let trailers_dir = movie_path.join("trailers");
        fs::create_dir(&trailers_dir).await.unwrap();

        let source_file = temp.path().join("test_trailer.mp4");
        fs::write(&source_file, b"test content").await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        organizer
            .move_file(&source_file, &trailers_dir, "test_trailer.mp4")
            .await
            .unwrap();

        assert!(!source_file.exists());
        assert!(trailers_dir.join("test_trailer.mp4").exists());
    }

    #[tokio::test]
    async fn test_cleanup_temp_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();
        fs::write(temp_dir.join("file.mp4"), b"test").await.unwrap();

        let organizer = Organizer::new(movie_path);
        organizer.cleanup_temp_dir(&temp_dir).await.unwrap();

        assert!(!temp_dir.exists());
    }

    #[tokio::test]
    async fn test_create_done_marker() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        organizer.create_done_marker().await.unwrap();

        let marker_path = movie_path.join("done.ext");
        assert!(marker_path.exists());

        let content = fs::read_to_string(&marker_path).await.unwrap();
        let marker: DoneMarker = serde_json::from_str(&content).unwrap();

        assert!(!marker.finished_at.is_empty());
        assert!(!marker.version.is_empty());
    }

    #[tokio::test]
    async fn test_organize_integration() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        // Create test conversion results
        let trailer_file = temp_dir.join("trailer.mp4");
        fs::write(&trailer_file, b"trailer content").await.unwrap();

        let featurette_file = temp_dir.join("featurette.mp4");
        fs::write(&featurette_file, b"featurette content")
            .await
            .unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("trailer.mp4"),
                output_path: trailer_file.clone(),
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("featurette.mp4"),
                output_path: featurette_file.clone(),
                category: ContentCategory::Featurette,
                season_number: None,
                success: true,
                error: None,
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &temp_dir).await.unwrap();

        // Verify subdirectories were created
        assert!(movie_path.join("trailers").exists());
        assert!(movie_path.join("featurettes").exists());

        // Verify files were moved
        assert!(movie_path.join("trailers/trailer.mp4").exists());
        assert!(movie_path.join("featurettes/featurette.mp4").exists());

        // Verify temp dir was cleaned up
        assert!(!temp_dir.exists());

        // Verify done marker was created
        assert!(movie_path.join("done.ext").exists());
    }

    #[tokio::test]
    async fn test_organize_skips_failed_conversions() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let success_file = temp_dir.join("success.mp4");
        fs::write(&success_file, b"success content").await.unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("success.mp4"),
                output_path: success_file.clone(),
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("failed.mp4"),
                output_path: temp_dir.join("failed.mp4"),
                category: ContentCategory::Featurette,
                season_number: None,
                success: false,
                error: Some("Conversion failed".to_string()),
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &temp_dir).await.unwrap();

        // Only successful conversion should be organized
        assert!(movie_path.join("trailers").exists());
        assert!(movie_path.join("trailers/success.mp4").exists());

        // Failed conversion should not create subdirectory
        assert!(!movie_path.join("featurettes").exists());
    }

    #[tokio::test]
    async fn test_organize_handles_multiple_categories() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        // Create files for different categories
        let trailer = temp_dir.join("trailer.mp4");
        let featurette = temp_dir.join("featurette.mp4");
        let behind = temp_dir.join("behind.mp4");
        let deleted = temp_dir.join("deleted.mp4");

        fs::write(&trailer, b"trailer").await.unwrap();
        fs::write(&featurette, b"featurette").await.unwrap();
        fs::write(&behind, b"behind").await.unwrap();
        fs::write(&deleted, b"deleted").await.unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("trailer.mp4"),
                output_path: trailer,
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("featurette.mp4"),
                output_path: featurette,
                category: ContentCategory::Featurette,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("behind.mp4"),
                output_path: behind,
                category: ContentCategory::BehindTheScenes,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("deleted.mp4"),
                output_path: deleted,
                category: ContentCategory::DeletedScene,
                season_number: None,
                success: true,
                error: None,
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &temp_dir).await.unwrap();

        // Verify all subdirectories were created
        assert!(movie_path.join("trailers").exists());
        assert!(movie_path.join("featurettes").exists());
        assert!(movie_path.join("behind the scenes").exists());
        assert!(movie_path.join("deleted scenes").exists());

        // Verify all files were moved
        assert!(movie_path.join("trailers/trailer.mp4").exists());
        assert!(movie_path.join("featurettes/featurette.mp4").exists());
        assert!(movie_path.join("behind the scenes/behind.mp4").exists());
        assert!(movie_path.join("deleted scenes/deleted.mp4").exists());
    }

    #[tokio::test]
    async fn test_done_marker_json_format() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        organizer.create_done_marker().await.unwrap();

        let marker_path = movie_path.join("done.ext");
        let content = fs::read_to_string(&marker_path).await.unwrap();

        // Verify it's valid JSON
        let marker: DoneMarker = serde_json::from_str(&content).unwrap();

        // Verify timestamp is ISO 8601 format
        assert!(chrono::DateTime::parse_from_rfc3339(&marker.finished_at).is_ok());

        // Verify version matches package version
        assert_eq!(marker.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_cleanup_temp_dir_handles_nonexistent_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let nonexistent_dir = temp.path().join("does_not_exist");

        let organizer = Organizer::new(movie_path);
        // Should not error when temp dir doesn't exist
        let result = organizer.cleanup_temp_dir(&nonexistent_dir).await;
        assert!(result.is_ok());
    }

    // Series organizer tests

    #[tokio::test]
    async fn test_series_organizer_organize_series_level_extras() {
        let temp = TempDir::new().unwrap();
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let trailer_file = temp_dir.join("trailer.mp4");
        fs::write(&trailer_file, b"trailer content").await.unwrap();

        let conversions = vec![ConversionResult {
            input_path: temp_dir.join("trailer.mp4"),
            output_path: trailer_file.clone(),
            category: ContentCategory::Trailer,
            season_number: None,
            success: true,
            error: None,
        }];

        let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
        organizer.organize_extras(conversions, None).await.unwrap();

        // Verify series-level subdirectory was created
        assert!(series_path.join("trailers").exists());
        assert!(series_path.join("trailers/trailer.mp4").exists());
    }

    #[tokio::test]
    async fn test_series_organizer_organize_season_specific_extras() {
        let temp = TempDir::new().unwrap();
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path).await.unwrap();

        let season_dir = series_path.join("Season 01");
        fs::create_dir(&season_dir).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let behind_file = temp_dir.join("behind.mp4");
        fs::write(&behind_file, b"behind content").await.unwrap();

        let conversions = vec![ConversionResult {
            input_path: temp_dir.join("behind.mp4"),
            output_path: behind_file.clone(),
            category: ContentCategory::BehindTheScenes,
            season_number: None,
            success: true,
            error: None,
        }];

        let organizer = SeriesOrganizer::new(series_path.clone(), vec![1]);
        organizer
            .organize_extras(conversions, Some(1))
            .await
            .unwrap();

        // Verify season-specific subdirectory was created
        assert!(series_path.join("Season 01/behind the scenes").exists());
        assert!(
            series_path
                .join("Season 01/behind the scenes/behind.mp4")
                .exists()
        );
    }

    #[tokio::test]
    async fn test_series_organizer_organize_specials() {
        let temp = TempDir::new().unwrap();
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let special_file = temp_dir.join("special.mp4");
        fs::write(&special_file, b"special content").await.unwrap();

        let specials = vec![SpecialEpisode {
            episode_number: 1,
            title: "Pilot".to_string(),
            air_date: None,
            url: None,
            local_path: Some(special_file.clone()),
            tvdb_id: None,
        }];

        let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
        organizer
            .organize_specials("Breaking Bad", specials, "Season 00")
            .await
            .unwrap();

        // Verify Season 00 folder was created
        assert!(series_path.join("Season 00").exists());
        // Updated to .mkv extension (Sonarr-compatible)
        assert!(
            series_path
                .join("Season 00/Breaking Bad - S00E01 - Pilot.mkv")
                .exists()
        );
    }

    #[tokio::test]
    async fn test_series_organizer_organize_specials_skips_existing() {
        let temp = TempDir::new().unwrap();
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path).await.unwrap();

        let season_00_dir = series_path.join("Season 00");
        fs::create_dir(&season_00_dir).await.unwrap();

        // Create an existing target file
        let existing_target = season_00_dir.join("Breaking Bad - S00E01 - Pilot.mkv");
        fs::write(&existing_target, b"existing content")
            .await
            .unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let special_file = temp_dir.join("special.mp4");
        fs::write(&special_file, b"new content").await.unwrap();

        let specials = vec![SpecialEpisode {
            episode_number: 1,
            title: "Pilot".to_string(),
            air_date: None,
            url: None,
            local_path: Some(special_file.clone()),
            tvdb_id: None,
        }];

        let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
        organizer
            .organize_specials("Breaking Bad", specials, "Season 00")
            .await
            .unwrap();

        // Verify the existing file was not overwritten
        let content = fs::read_to_string(&existing_target).await.unwrap();
        assert_eq!(content, "existing content");

        // Verify the source file still exists (wasn't moved)
        assert!(special_file.exists());
    }

    #[tokio::test]
    async fn test_series_organizer_sanitize_filename() {
        let test_cases = vec![
            ("Normal Title", "Normal Title"),
            ("Title: With Colon", "Title_ With Colon"),
            ("Title/With/Slashes", "Title_With_Slashes"),
            ("Title*With?Asterisk", "Title_With_Asterisk"),
            ("Title\"With\"Quotes", "Title_With_Quotes"),
            ("Title<With>Brackets", "Title_With_Brackets"),
            ("Title|With|Pipe", "Title_With_Pipe"),
            ("Title\\With\\Backslash", "Title_With_Backslash"),
        ];

        for (input, expected) in test_cases {
            let result = SeriesOrganizer::sanitize_filename(input);
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[tokio::test]
    async fn test_series_organizer_multiple_categories() {
        let temp = TempDir::new().unwrap();
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let trailer = temp_dir.join("trailer.mp4");
        let interview = temp_dir.join("interview.mp4");

        fs::write(&trailer, b"trailer").await.unwrap();
        fs::write(&interview, b"interview").await.unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("trailer.mp4"),
                output_path: trailer,
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("interview.mp4"),
                output_path: interview,
                category: ContentCategory::Interview,
                season_number: None,
                success: true,
                error: None,
            },
        ];

        let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
        organizer.organize_extras(conversions, None).await.unwrap();

        assert!(series_path.join("trailers/trailer.mp4").exists());
        assert!(series_path.join("interviews/interview.mp4").exists());
    }

    #[tokio::test]
    async fn test_series_organizer_skips_failed_conversions() {
        let temp = TempDir::new().unwrap();
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let success_file = temp_dir.join("success.mp4");
        fs::write(&success_file, b"success").await.unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("success.mp4"),
                output_path: success_file.clone(),
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("failed.mp4"),
                output_path: temp_dir.join("failed.mp4"),
                category: ContentCategory::Featurette,
                season_number: None,
                success: false,
                error: Some("Conversion failed".to_string()),
            },
        ];

        let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
        organizer.organize_extras(conversions, None).await.unwrap();

        assert!(series_path.join("trailers/success.mp4").exists());
        assert!(!series_path.join("featurettes").exists());
    }

    #[tokio::test]
    async fn test_series_organizer_skips_nonexistent_season() {
        let temp = TempDir::new().unwrap();
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path).await.unwrap();

        // Only Season 1 exists on disk
        let season_dir = series_path.join("Season 01");
        fs::create_dir(&season_dir).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let file = temp_dir.join("extra.mp4");
        fs::write(&file, b"content").await.unwrap();

        let conversions = vec![ConversionResult {
            input_path: temp_dir.join("extra.mp4"),
            output_path: file.clone(),
            category: ContentCategory::Trailer,
            season_number: None,
            success: true,
            error: None,
        }];

        // Try to organize for Season 5 (which doesn't exist)
        let organizer = SeriesOrganizer::new(series_path.clone(), vec![1]);
        organizer
            .organize_extras(conversions, Some(5))
            .await
            .unwrap();

        // Verify Season 5 folder was NOT created
        assert!(!series_path.join("Season 05").exists());
        // Verify Season 1 folder still exists
        assert!(series_path.join("Season 01").exists());
        // Verify file was not moved
        assert!(file.exists());
    }

    // --- Tests for Story 3.1: Numeric Filename Normalization ---

    #[test]
    fn test_is_opaque_numeric_filename() {
        // Pure digits → true
        assert!(is_opaque_numeric_filename(Path::new("10032.mp4")));
        assert!(is_opaque_numeric_filename(Path::new("98765.mkv")));
        assert!(is_opaque_numeric_filename(Path::new("0.mp4")));
        // No extension → true (stem is still digits)
        assert!(is_opaque_numeric_filename(Path::new("12345")));

        // Alphabetic stem → false
        assert!(!is_opaque_numeric_filename(Path::new("trailer.mp4")));
        // Mixed → false
        assert!(!is_opaque_numeric_filename(Path::new("trailer1.mp4")));
        assert!(!is_opaque_numeric_filename(Path::new("1trailer.mp4")));
        // Empty stem edge case
        assert!(!is_opaque_numeric_filename(Path::new(".mp4")));
        assert!(!is_opaque_numeric_filename(Path::new("")));
    }

    #[test]
    fn test_normalize_filename_opaque_numeric() {
        // Opaque numeric → "{Category} #{N}.{ext}"
        let result = normalize_filename(Path::new("10032.mp4"), ContentCategory::Trailer, 1);
        assert_eq!(result, "Trailer #1.mp4");

        let result =
            normalize_filename(Path::new("99999.MKV"), ContentCategory::BehindTheScenes, 3);
        assert_eq!(result, "Behind the Scenes #3.mkv");
    }

    #[test]
    fn test_normalize_filename_descriptive_preserved() {
        // Descriptive filename → preserved as-is (after sanitization)
        let result = normalize_filename(
            Path::new("Official Trailer.mp4"),
            ContentCategory::Trailer,
            1,
        );
        assert_eq!(result, "Official Trailer.mp4");
    }

    #[test]
    fn test_normalize_filename_sanitization_applied() {
        // Sanitization applied to both branches
        // Opaque numeric with no special chars — just verify format
        let result = normalize_filename(Path::new("555.mp4"), ContentCategory::Featurette, 2);
        assert_eq!(result, "Featurette #2.mp4");

        // Descriptive with special chars → sanitized
        let result = normalize_filename(
            Path::new("Behind: The Scenes.mp4"),
            ContentCategory::BehindTheScenes,
            1,
        );
        assert_eq!(result, "Behind- The Scenes.mp4");
    }

    #[test]
    fn test_sanitize_filename_module_level() {
        // ASCII special chars
        assert_eq!(sanitize_filename("a|b<c>d:e/f\\g*h"), "a-b-c-d-e-f-g-h");
        assert_eq!(sanitize_filename("say \"hello\""), "say 'hello'");
        assert_eq!(sanitize_filename("what?"), "what");

        // Unicode variants
        assert_eq!(sanitize_filename("a｜b＜c＞d"), "a-b-c-d");
        assert_eq!(sanitize_filename("\u{201c}hi\u{201d}"), "'hi'");
        assert_eq!(sanitize_filename("what？"), "what");

        // Clean string unchanged
        assert_eq!(sanitize_filename("clean name.mp4"), "clean name.mp4");
    }

    #[tokio::test]
    async fn test_organize_normalizes_numeric_filenames() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.expect("create movie dir");

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.expect("create temp dir");

        // Two numeric files in the same category
        let file1 = temp_dir.join("10032.mp4");
        let file2 = temp_dir.join("99887.mp4");
        fs::write(&file1, b"trailer 1").await.expect("write file1");
        fs::write(&file2, b"trailer 2").await.expect("write file2");

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("10032.mp4"),
                output_path: file1,
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("99887.mp4"),
                output_path: file2,
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer
            .organize(conversions, &temp_dir)
            .await
            .expect("organize");

        // Both should be renamed to Trailer #1.mp4 and Trailer #2.mp4
        let trailers_dir = movie_path.join("trailers");
        assert!(trailers_dir.join("Trailer #1.mp4").exists());
        assert!(trailers_dir.join("Trailer #2.mp4").exists());
    }

    #[tokio::test]
    async fn test_organize_preserves_descriptive_filenames() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.expect("create movie dir");

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.expect("create temp dir");

        let file = temp_dir.join("Official Trailer.mp4");
        fs::write(&file, b"trailer content")
            .await
            .expect("write file");

        let conversions = vec![ConversionResult {
            input_path: temp_dir.join("Official Trailer.mp4"),
            output_path: file,
            category: ContentCategory::Trailer,
            season_number: None,
            success: true,
            error: None,
        }];

        let organizer = Organizer::new(movie_path.clone());
        organizer
            .organize(conversions, &temp_dir)
            .await
            .expect("organize");

        // Descriptive filename should be preserved
        assert!(movie_path.join("trailers/Official Trailer.mp4").exists());
    }

    #[tokio::test]
    async fn test_series_organizer_normalizes_numeric_filenames() {
        let temp = TempDir::new().expect("failed to create temp dir");
        let series_path = temp.path().join("Breaking Bad (2008)");
        fs::create_dir(&series_path)
            .await
            .expect("create series dir");

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.expect("create temp dir");

        let file1 = temp_dir.join("44321.mp4");
        let file2 = temp_dir.join("55678.mp4");
        fs::write(&file1, b"trailer 1").await.expect("write file1");
        fs::write(&file2, b"trailer 2").await.expect("write file2");

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("44321.mp4"),
                output_path: file1,
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("55678.mp4"),
                output_path: file2,
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
        ];

        let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
        organizer
            .organize_extras(conversions, None)
            .await
            .expect("organize_extras");

        let trailers_dir = series_path.join("trailers");
        assert!(trailers_dir.join("Trailer #1.mp4").exists());
        assert!(trailers_dir.join("Trailer #2.mp4").exists());
    }

    #[test]
    fn test_normalize_filename_no_extension() {
        // Numeric file with no extension → fallback to "mp4"
        let result = normalize_filename(Path::new("12345"), ContentCategory::Trailer, 1);
        assert_eq!(result, "Trailer #1.mp4");

        let result = normalize_filename(Path::new("99999"), ContentCategory::Featurette, 2);
        assert_eq!(result, "Featurette #2.mp4");
    }

    #[tokio::test]
    async fn test_organize_normalizes_numeric_filenames_source_gone() {
        // Verify source files are removed (moved, not copied) after organize
        let temp = TempDir::new().expect("failed to create temp dir");
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.expect("create movie dir");

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.expect("create temp dir");

        let file1 = temp_dir.join("10032.mp4");
        let file2 = temp_dir.join("99887.mp4");
        fs::write(&file1, b"trailer 1").await.expect("write file1");
        fs::write(&file2, b"trailer 2").await.expect("write file2");

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("10032.mp4"),
                output_path: file1.clone(),
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("99887.mp4"),
                output_path: file2.clone(),
                category: ContentCategory::Trailer,
                season_number: None,
                success: true,
                error: None,
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer
            .organize(conversions, &temp_dir)
            .await
            .expect("organize");

        // Renamed files exist at destination
        let trailers_dir = movie_path.join("trailers");
        assert!(trailers_dir.join("Trailer #1.mp4").exists());
        assert!(trailers_dir.join("Trailer #2.mp4").exists());

        // Source numeric files are gone (temp dir itself is cleaned up by organize)
        assert!(!temp_dir.exists());
    }

    // Task 4.7: subtitle file is moved alongside its video into the Jellyfin subfolder
    #[tokio::test]
    async fn test_organize_moves_subtitle_alongside_video() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let tmp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&tmp_dir).await.unwrap();

        // Create a video file and a sibling subtitle file
        let video = tmp_dir.join("trailer.mp4");
        let subtitle = tmp_dir.join("trailer.en.vtt");
        fs::write(&video, b"video content").await.unwrap();
        fs::write(&subtitle, b"WEBVTT\n").await.unwrap();

        let conversions = vec![ConversionResult {
            input_path: video.clone(),
            output_path: video.clone(),
            category: ContentCategory::Trailer,
            season_number: None,
            success: true,
            error: None,
        }];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &tmp_dir).await.unwrap();

        let trailers_dir = movie_path.join("trailers");
        // Video was moved (descriptive name preserved)
        assert!(trailers_dir.join("trailer.mp4").exists());
        // Subtitle was moved alongside the video with its original name
        assert!(
            trailers_dir.join("trailer.en.vtt").exists(),
            "subtitle should be in trailers dir"
        );
    }

    // Task 4.8: numeric video is renamed; subtitle keeps its original name in the same subfolder
    #[tokio::test]
    async fn test_organize_subtitle_kept_original_name_when_video_renamed() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let tmp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&tmp_dir).await.unwrap();

        // Numeric video + sibling subtitle
        let video = tmp_dir.join("10032.mp4");
        let subtitle = tmp_dir.join("10032.en.vtt");
        fs::write(&video, b"video content").await.unwrap();
        fs::write(&subtitle, b"WEBVTT\n").await.unwrap();

        let conversions = vec![ConversionResult {
            input_path: video.clone(),
            output_path: video.clone(),
            category: ContentCategory::Trailer,
            season_number: None,
            success: true,
            error: None,
        }];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &tmp_dir).await.unwrap();

        let trailers_dir = movie_path.join("trailers");
        // Video was renamed to normalized form
        assert!(
            trailers_dir.join("Trailer #1.mp4").exists(),
            "video should be renamed"
        );
        // Subtitle keeps its original name (NOT renamed)
        assert!(
            trailers_dir.join("10032.en.vtt").exists(),
            "subtitle should keep original name"
        );
        // Subtitle is NOT renamed to match the video
        assert!(
            !trailers_dir.join("Trailer #1.en.vtt").exists(),
            "subtitle should not be renamed"
        );
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: extras-fetcher, Property 23: Content Category to Subdirectory Mapping
    // Validates: Requirements 8.1, 8.2, 8.3, 8.4
    proptest! {
        #[test]
        fn prop_category_to_subdirectory_mapping(
            category in prop_oneof![
                Just(ContentCategory::Trailer),
                Just(ContentCategory::Featurette),
                Just(ContentCategory::BehindTheScenes),
                Just(ContentCategory::DeletedScene),
                Just(ContentCategory::Interview),
                Just(ContentCategory::Short),
                Just(ContentCategory::Clip),
                Just(ContentCategory::Scene),
                Just(ContentCategory::Extras),
            ]
        ) {
            let expected_subdir = match category {
                ContentCategory::Trailer => "trailers",
                ContentCategory::Featurette => "featurettes",
                ContentCategory::BehindTheScenes => "behind the scenes",
                ContentCategory::DeletedScene => "deleted scenes",
                ContentCategory::Interview => "interviews",
                ContentCategory::Short => "shorts",
                ContentCategory::Clip => "clips",
                ContentCategory::Scene => "scenes",
                ContentCategory::Extras => "extras",
            };

            prop_assert_eq!(category.subdirectory(), expected_subdir);
        }
    }

    // Feature: extras-fetcher, Property 24: Subdirectory Creation
    // Validates: Requirements 8.5
    proptest! {
        #[test]
        fn prop_subdirectory_creation(
            category in prop_oneof![
                Just(ContentCategory::Trailer),
                Just(ContentCategory::Featurette),
                Just(ContentCategory::BehindTheScenes),
                Just(ContentCategory::DeletedScene),
                Just(ContentCategory::Interview),
                Just(ContentCategory::Short),
                Just(ContentCategory::Clip),
                Just(ContentCategory::Scene),
                Just(ContentCategory::Extras),
            ]
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let movie_path = temp.path().join("Movie (2020)");
                tokio::fs::create_dir(&movie_path).await.unwrap();

                let organizer = Organizer::new(movie_path.clone());
                let subdir = organizer.ensure_subdirectory(category).await.unwrap();

                // Verify subdirectory was created
                prop_assert!(subdir.exists());

                // Verify subdirectory is in the correct location
                let expected_path = movie_path.join(category.subdirectory());
                prop_assert_eq!(subdir, expected_path);

                Ok(())
            })?;
        }
    }

    // Feature: tv-series-extras, Property 3: Season Number Interpretation
    // Validates: Requirements 2.3, 2.4
    proptest! {
        #[test]
        fn prop_season_number_interpretation(
            season in prop_oneof![
                Just(None),
                (1u8..=99u8).prop_map(Some),
            ]
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let series_path = temp.path().join("Series (2020)");
                tokio::fs::create_dir(&series_path).await.unwrap();

                // Create season folder if needed
                let mut available_seasons = vec![];
                if let Some(s) = season {
                    let season_dir = series_path.join(format!("Season {:02}", s));
                    tokio::fs::create_dir(&season_dir).await.unwrap();
                    available_seasons.push(s);
                }

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                let file = temp_dir.join("extra.mp4");
                tokio::fs::write(&file, b"content").await.unwrap();

                let conversions = vec![ConversionResult {
                    input_path: temp_dir.join("extra.mp4"),
                    output_path: file,
                    category: ContentCategory::Trailer,
                season_number: None,
                    success: true,
                    error: None,
                }];

                let organizer = SeriesOrganizer::new(series_path.clone(), available_seasons);
                organizer.organize_extras(conversions, season).await.unwrap();

                // Verify file was organized in correct location
                let expected_path = if let Some(s) = season {
                    series_path.join(format!("Season {:02}/trailers/extra.mp4", s))
                } else {
                    series_path.join("trailers/extra.mp4")
                };

                prop_assert!(expected_path.exists());

                Ok(())
            })?;
        }
    }

    // Feature: tv-series-extras, Property 9: Content Category to Subdirectory Mapping
    // Validates: Requirements 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7
    proptest! {
        #[test]
        fn prop_series_category_to_subdirectory_mapping(
            category in prop_oneof![
                Just(ContentCategory::Trailer),
                Just(ContentCategory::Featurette),
                Just(ContentCategory::BehindTheScenes),
                Just(ContentCategory::DeletedScene),
                Just(ContentCategory::Interview),
                Just(ContentCategory::Short),
                Just(ContentCategory::Clip),
                Just(ContentCategory::Scene),
                Just(ContentCategory::Extras),
            ],
            season in prop_oneof![
                Just(None),
                (1u8..=10u8).prop_map(Some),
            ]
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let series_path = temp.path().join("Series (2020)");
                tokio::fs::create_dir(&series_path).await.unwrap();

                let mut available_seasons = vec![];
                if let Some(s) = season {
                    let season_dir = series_path.join(format!("Season {:02}", s));
                    tokio::fs::create_dir(&season_dir).await.unwrap();
                    available_seasons.push(s);
                }

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                let file = temp_dir.join("extra.mp4");
                tokio::fs::write(&file, b"content").await.unwrap();

                let conversions = vec![ConversionResult {
                    input_path: temp_dir.join("extra.mp4"),
                    output_path: file,
                    category,
                    season_number: None,
                    success: true,
                    error: None,
                }];

                let organizer = SeriesOrganizer::new(series_path.clone(), available_seasons);
                organizer.organize_extras(conversions, season).await.unwrap();

                // Verify file was organized in correct subdirectory
                let expected_subdir = category.subdirectory();
                let expected_path = if let Some(s) = season {
                    series_path.join(format!("Season {:02}/{}/extra.mp4", s, expected_subdir))
                } else {
                    series_path.join(format!("{}/extra.mp4", expected_subdir))
                };

                prop_assert!(expected_path.exists());

                Ok(())
            })?;
        }
    }

    // Feature: tv-series-extras, Property 10: Season 0 File Naming Format
    // Validates: Requirements 8.1, 8.2, 8.3, 8.4
    proptest! {
        #[test]
        fn prop_season_zero_file_naming_format(
            episode_num in 1u8..=99u8,
            title in "[a-zA-Z0-9 :',&!?.-]{1,50}",
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let series_path = temp.path().join("Series (2020)");
                tokio::fs::create_dir(&series_path).await.unwrap();

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                let file = temp_dir.join("special.mp4");
                tokio::fs::write(&file, b"content").await.unwrap();

                let specials = vec![SpecialEpisode {
                    episode_number: episode_num,
                    title: title.clone(),
                    air_date: None,
                    url: None,
                    local_path: Some(file),
                    tvdb_id: None,
                }];

                let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
                organizer.organize_specials("TestSeries", specials, "Season 00").await.unwrap();

                // Verify Season 00 folder exists
                prop_assert!(series_path.join("Season 00").exists());

                // Verify file naming format: "Series Name - S00E{num} - {title}.mkv" (Sonarr-compatible)
                let sanitized_title = SeriesOrganizer::sanitize_filename(&title);
                let expected_filename = format!(
                    "TestSeries - S00E{:02} - {}.mkv",
                    episode_num, sanitized_title
                );
                let expected_path = series_path.join("Season 00").join(&expected_filename);

                prop_assert!(expected_path.exists());

                Ok(())
            })?;
        }
    }

    // Feature: extras-fetcher, Property 25: Temp Folder Cleanup on Success
    // Validates: Requirements 8.6
    proptest! {
        #[test]
        fn prop_temp_folder_cleanup_on_success(
            num_files in 1usize..10usize,
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let movie_path = temp.path().join("Movie (2020)");
                tokio::fs::create_dir(&movie_path).await.unwrap();

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                // Create multiple files in temp directory
                for i in 0..num_files {
                    let file_path = temp_dir.join(format!("file_{}.mp4", i));
                    tokio::fs::write(&file_path, b"test content").await.unwrap();
                }

                // Verify temp directory exists with files
                prop_assert!(temp_dir.exists());
                let mut count = 0;
                let mut read_dir = tokio::fs::read_dir(&temp_dir).await.unwrap();
                while let Some(_entry) = read_dir.next_entry().await.unwrap() {
                    count += 1;
                }
                prop_assert_eq!(count, num_files);

                // Organize with empty conversions (just cleanup)
                let organizer = Organizer::new(movie_path.clone());
                organizer.organize(vec![], &temp_dir).await.unwrap();

                // Verify temp directory was deleted
                prop_assert!(!temp_dir.exists());

                Ok(())
            })?;
        }
    }

    // Feature: extras-fetcher, Property 26: Done Marker Creation on Completion
    // Validates: Requirements 2.1, 8.7
    proptest! {
        #[test]
        fn prop_done_marker_creation_on_completion(
            num_conversions in 0usize..5usize,
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let movie_path = temp.path().join("Movie (2020)");
                tokio::fs::create_dir(&movie_path).await.unwrap();

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                // Create conversion results
                let mut conversions = vec![];
                for i in 0..num_conversions {
                    let output_file = temp_dir.join(format!("file_{}.mp4", i));
                    tokio::fs::write(&output_file, b"test content").await.unwrap();

                    conversions.push(ConversionResult {
                        input_path: temp_dir.join(format!("file_{}.mp4", i)),
                        output_path: output_file,
                        category: ContentCategory::Trailer,
                        season_number: None,
                        success: true,
                        error: None,
                    });
                }

                // Organize
                let organizer = Organizer::new(movie_path.clone());
                organizer.organize(conversions, &temp_dir).await.unwrap();

                // Verify done marker was created
                let marker_path = movie_path.join("done.ext");
                prop_assert!(marker_path.exists());

                // Verify done marker content is valid JSON with required fields
                let content = tokio::fs::read_to_string(&marker_path).await.unwrap();
                let marker: DoneMarker = serde_json::from_str(&content).unwrap();

                // Verify timestamp is not empty and is valid ISO 8601
                prop_assert!(!marker.finished_at.is_empty());
                prop_assert!(chrono::DateTime::parse_from_rfc3339(&marker.finished_at).is_ok());

                // Verify version is not empty
                prop_assert!(!marker.version.is_empty());

                Ok(())
            })?;
        }
    }

    // Feature: tvdb-specials, Property 7: Sonarr-Compatible File Path Construction
    // Validates: Requirements 7.1, 7.2, 7.3
    proptest! {
        #[test]
        fn prop_sonarr_compatible_file_path_construction(
            series_title in "[A-Za-z0-9 ]{3,30}",
            folder_name in prop_oneof![
                Just("Specials"),
                Just("Season 00"),
                Just("Season 0"),
            ],
            episode_number in 1u8..=99u8,
            episode_title in "[A-Za-z0-9 :',&!?.-]{1,50}",
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let series_path = temp.path().join("TestSeries");
                tokio::fs::create_dir(&series_path).await.unwrap();

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                let source_file = temp_dir.join("special.mp4");
                tokio::fs::write(&source_file, b"content").await.unwrap();

                let specials = vec![SpecialEpisode {
                    episode_number,
                    title: episode_title.clone(),
                    air_date: None,
                    url: None,
                    local_path: Some(source_file),
                    tvdb_id: None,
                }];

                let organizer = SeriesOrganizer::new(series_path.clone(), vec![]);
                organizer.organize_specials(&series_title, specials, &folder_name).await.unwrap();

                // Requirement 7.1: File is placed in {series_folder}/{specials_folder_name}/
                let specials_dir = series_path.join(&folder_name);
                prop_assert!(specials_dir.exists());
                prop_assert!(specials_dir.is_dir());

                // Requirement 7.2: File naming pattern is {series_title} - S00E{episode_number:02} - {sanitized_title}.mkv
                let sanitized_series = SeriesOrganizer::sanitize_filename(&series_title);
                let sanitized_title = SeriesOrganizer::sanitize_filename(&episode_title);
                let expected_filename = format!(
                    "{} - S00E{:02} - {}.mkv",
                    sanitized_series, episode_number, sanitized_title
                );

                // Requirement 7.3: Uses aired_episode_number from TVDB (stored in episode_number field)
                let expected_path = specials_dir.join(&expected_filename);
                prop_assert!(expected_path.exists(), "Expected file not found: {:?}", expected_path);

                // Verify the file contains the expected content
                let content = tokio::fs::read(&expected_path).await.unwrap();
                prop_assert_eq!(content, b"content");

                Ok(())
            })?;
        }
    }

    // Feature: tvdb-specials, Property 8: Filename Sanitization Removes Windows-Invalid Characters
    // Validates: Requirement 7.4
    proptest! {
        #[test]
        fn prop_filename_sanitization_removes_invalid_chars(
            // Generate strings that may contain Windows-invalid characters
            input in "[\u{0020}-\u{007E}]{1,100}",
        ) {
            let sanitized = SeriesOrganizer::sanitize_filename(&input);

            // Property 1: Sanitized output SHALL contain none of the Windows-invalid characters
            let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
            for invalid_char in &invalid_chars {
                prop_assert!(
                    !sanitized.contains(*invalid_char),
                    "Sanitized output contains invalid character '{}': {}",
                    invalid_char,
                    sanitized
                );
            }

            // Property 2: Sanitized output SHALL have length <= original string length
            prop_assert!(
                sanitized.len() <= input.len(),
                "Sanitized output is longer than input: {} > {}",
                sanitized.len(),
                input.len()
            );

            // Additional verification: If input had no invalid chars, output should be identical
            let has_invalid = input.chars().any(|c| invalid_chars.contains(&c));
            if !has_invalid {
                prop_assert_eq!(&sanitized, &input, "Input without invalid chars should be unchanged");
            }
        }
    }
}
