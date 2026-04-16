// YouTube content discoverer

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, error, info};

use super::ContentDiscoverer;
use super::title_matching;
use super::retry_with_backoff;

/// YouTube content discoverer
pub struct YoutubeDiscoverer {
    /// Browser to source cookies from for bot-detection bypass (e.g. "chrome", "firefox")
    cookies_from_browser: Option<String>,
}

impl Default for YoutubeDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

impl YoutubeDiscoverer {
    /// Create a new YouTube discoverer without cookie authentication
    pub fn new() -> Self {
        Self {
            cookies_from_browser: None,
        }
    }

    /// Create a new YouTube discoverer with browser cookie authentication
    pub fn with_cookies(browser: String) -> Self {
        Self {
            cookies_from_browser: Some(browser),
        }
    }

    /// Build search queries for different content types
    fn build_search_queries(title: &str, year: u16) -> Vec<(String, ContentCategory)> {
        vec![
            (
                format!("{} {} deleted scenes", title, year),
                ContentCategory::DeletedScene,
            ),
            (
                format!("{} {} behind the scenes", title, year),
                ContentCategory::BehindTheScenes,
            ),
            (
                format!("{} {} cast interview", title, year),
                ContentCategory::Interview,
            ),
        ]
    }

    /// Check if duration is within acceptable range (30s - 40min)
    fn is_duration_valid(duration_secs: u32) -> bool {
        (30..=2400).contains(&duration_secs) // 40 minutes = 2400 seconds
    }

    /// Check if video is a YouTube Short (duration < 60s and vertical aspect ratio)
    fn is_youtube_short(duration_secs: u32, width: u32, height: u32) -> bool {
        // YouTube Shorts are typically < 60 seconds and have vertical aspect ratio (9:16 or similar)
        if duration_secs >= 60 {
            return false;
        }

        // Check for vertical aspect ratio (height > width)
        // Allow some tolerance for aspect ratio detection
        height > width
    }

    /// Filter a video based on all criteria
    fn should_include_video(
        video_title: &str,
        movie_title: &str,
        duration_secs: u32,
        width: u32,
        height: u32,
        expected_year: u16,
    ) -> bool {
        // Check if movie title is in video title (with normalization)
        if !title_matching::contains_movie_title(video_title, movie_title) {
            debug!(
                "Excluding '{}' - does not contain movie title '{}' (normalized)",
                video_title, movie_title
            );
            return false;
        }
        // Fallback: Check for sequel numbers even if no collection info available
        if title_matching::mentions_sequel_number(video_title, movie_title) {
            debug!(
                "Excluding '{}' - mentions sequel number (fallback detection)",
                video_title
            );
            return false;
        }

        // Check duration range
        if !Self::is_duration_valid(duration_secs) {
            debug!(
                "Excluding '{}' - duration {}s out of range (30s-2400s)",
                video_title, duration_secs
            );
            return false;
        }

        // Check for excluded keywords
        if title_matching::contains_excluded_keywords(video_title) {
            debug!("Excluding '{}' - contains excluded keyword", video_title);
            return false;
        }

        // Check if it mentions a different year (potential sequel)
        if title_matching::mentions_different_year(video_title, expected_year) {
            debug!(
                "Excluding '{}' - mentions different year (expected: {})",
                video_title, expected_year
            );
            return false;
        }

        // Check if it's a YouTube Short
        if Self::is_youtube_short(duration_secs, width, height) {
            debug!("Excluding '{}' - detected as YouTube Short", video_title);
            return false;
        }

        debug!("Including '{}' - passed all filters", video_title);
        true
    }

    /// Search YouTube using yt-dlp for a specific query
    async fn search_youtube(
        &self,
        query: &str,
        movie_title: &str,
        category: ContentCategory,
        expected_year: u16,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        // Use yt-dlp with ytsearch5 to get top 10 results
        let search_query = format!("ytsearch5:{}", query);

        debug!("Searching YouTube with query: {}", query);

        // Wrap yt-dlp execution in retry logic
        let output = retry_with_backoff(3, 1000, || async {
            let mut cmd = tokio::process::Command::new("yt-dlp");
            cmd.arg("--dump-json")
                .arg("--no-playlist")
                .arg("--skip-download")
                .arg("--remote-components")
                .arg("ejs:github")
                .arg(&search_query);

            if let Some(browser) = &self.cookies_from_browser {
                cmd.arg("--cookies-from-browser").arg(browser);
            }

            cmd.output().await.map_err(|e| {
                error!("Failed to execute yt-dlp: {}", e);
                DiscoveryError::YtDlpError(format!("Failed to execute yt-dlp: {}", e))
            })
        })
        .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp search failed: {}", stderr);
            return Err(DiscoveryError::YtDlpError(format!(
                "yt-dlp search failed: {}",
                stderr
            )));
        }

        // Parse JSON output (yt-dlp outputs one JSON object per line)
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sources = Vec::new();

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(json) => {
                    // Extract video metadata
                    let title = json["title"].as_str().unwrap_or("Unknown").to_string();
                    let url = json["webpage_url"].as_str().unwrap_or("").to_string();
                    let duration = json["duration"].as_u64().unwrap_or(0) as u32;
                    let width = json["width"].as_u64().unwrap_or(1920) as u32;
                    let height = json["height"].as_u64().unwrap_or(1080) as u32;

                    // Apply filtering
                    if Self::should_include_video(
                        &title,
                        movie_title,
                        duration,
                        width,
                        height,
                        expected_year,
                    ) {
                        // Infer category from the video's actual title rather than
                        // blindly trusting the search query category.
                        let resolved_category =
                            title_matching::infer_category_from_title(&title).unwrap_or(category);

                        sources.push(VideoSource {
                            url,
                            source_type: SourceType::YouTube,
                            category: resolved_category,
                            title,
                            season_number: None,
                            duration_secs: Some(duration),
                        });
                        debug!("Added YouTube video: {}", sources.last().unwrap().title);
                    }
                }
                Err(e) => {
                    error!("Failed to parse yt-dlp JSON output: {}", e);
                    continue;
                }
            }
        }

        Ok(sources)
    }

    /// Discover YouTube content for a movie
    async fn discover_youtube(
        &self,
        movie: &MovieEntry,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Discovering YouTube content for: {}", movie);

        let queries = Self::build_search_queries(&movie.title, movie.year);
        let mut all_sources = Vec::new();

        for (query, category) in queries {
            match self
                .search_youtube(&query, &movie.title, category, movie.year)
                .await
            {
                Ok(mut sources) => {
                    info!(
                        "Found {} YouTube videos for query: {}",
                        sources.len(),
                        query
                    );
                    all_sources.append(&mut sources);
                }
                Err(e) => {
                    error!("YouTube search failed for query '{}': {}", query, e);
                }
            }
        }

        info!(
            "Discovered {} YouTube sources for: {}",
            all_sources.len(),
            movie
        );
        Ok(all_sources)
    }
}

impl ContentDiscoverer for YoutubeDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        self.discover_youtube(movie).await
    }
}
