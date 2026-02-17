// Special episode validation module
//
// Validates downloaded videos against TVDB episode metadata to ensure
// they match the expected special episode criteria.

use crate::discovery::fuzzy_matching::FuzzyMatcher;
use crate::discovery::tvdb::TvdbEpisodeExtended;
use crate::models::DownloadResult;
use log::{debug, info, warn};
use std::path::Path;

/// Validates downloaded special episodes against TVDB metadata
pub struct SpecialValidator;

impl SpecialValidator {
    /// Validate a downloaded video against TVDB episode metadata
    ///
    /// Returns true if the video should be kept, false if it should be rejected.
    ///
    /// Validation criteria:
    /// 1. For movies (is_movie=true): duration must be >= 10 minutes (600 seconds)
    /// 2. Video title should have reasonable similarity to episode title (>= 40% match)
    /// 3. Video must have valid metadata (duration, title)
    ///
    /// # Arguments
    /// * `download` - The download result to validate
    /// * `episode` - The TVDB episode metadata
    /// * `video_title` - The actual title from the downloaded video metadata
    /// * `video_duration` - The duration in seconds from the downloaded video metadata
    pub fn validate(
        _download: &DownloadResult,
        episode: &TvdbEpisodeExtended,
        video_title: &str,
        video_duration: u32,
    ) -> bool {
        // Check movie duration requirement
        if episode.is_movie == Some(true) && video_duration < 600 {
            info!(
                "Rejecting S00E{:02} '{}': Movie duration {} seconds < 10 minutes",
                episode.number, episode.name, video_duration
            );
            return false;
        }

        // Check title similarity (40% threshold for specials since titles can vary)
        let similarity_score = FuzzyMatcher::get_similarity_score(&episode.name, video_title);
        if similarity_score < 40 {
            info!(
                "Rejecting S00E{:02} '{}': Title similarity {}% < 40% (video title: '{}')",
                episode.number, episode.name, similarity_score, video_title
            );
            return false;
        }

        debug!(
            "Validated S00E{:02} '{}': duration={}s, similarity={}%",
            episode.number, episode.name, video_duration, similarity_score
        );

        true
    }

    /// Extract video metadata from a downloaded file using yt-dlp
    ///
    /// Returns (title, duration_seconds) if successful, None otherwise.
    pub async fn extract_metadata(video_path: &Path) -> Option<(String, u32)> {
        let output = tokio::process::Command::new("yt-dlp")
            .arg("--print")
            .arg("%(title)s|||%(duration)s")
            .arg("--no-playlist")
            .arg(video_path)
            .output()
            .await;

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = stdout.trim().split("|||").collect();

                if parts.len() == 2 {
                    let title = parts[0].to_string();
                    let duration = parts[1].parse::<u32>().ok()?;
                    return Some((title, duration));
                }

                warn!(
                    "Failed to parse yt-dlp metadata output: {}",
                    stdout.trim()
                );
                None
            }
            Ok(output) => {
                warn!(
                    "yt-dlp metadata extraction failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                None
            }
            Err(e) => {
                warn!("Failed to execute yt-dlp for metadata extraction: {}", e);
                None
            }
        }
    }

    /// Filter a list of downloads to keep only valid special episodes
    ///
    /// This method validates each download against its corresponding TVDB episode
    /// and returns only the downloads that pass validation.
    ///
    /// # Arguments
    /// * `downloads` - List of download results
    /// * `episodes` - Corresponding TVDB episode metadata (must match downloads length)
    ///
    /// # Returns
    /// Filtered list of valid downloads
    pub async fn filter_valid_specials(
        downloads: Vec<DownloadResult>,
        episodes: &[TvdbEpisodeExtended],
    ) -> Vec<DownloadResult> {
        if downloads.len() != episodes.len() {
            warn!(
                "Download count ({}) doesn't match episode count ({}), skipping validation",
                downloads.len(),
                episodes.len()
            );
            return downloads;
        }

        let mut valid_downloads = Vec::new();

        for (download, episode) in downloads.into_iter().zip(episodes.iter()) {
            if !download.success {
                // Keep failed downloads as-is (they'll be filtered later)
                valid_downloads.push(download);
                continue;
            }

            // Extract metadata from downloaded file
            match Self::extract_metadata(&download.local_path).await {
                Some((video_title, video_duration)) => {
                    if Self::validate(&download, episode, &video_title, video_duration) {
                        info!(
                            "✓ Validated S00E{:02} '{}' - keeping download",
                            episode.number, episode.name
                        );
                        valid_downloads.push(download);
                    } else {
                        info!(
                            "✗ Rejected S00E{:02} '{}' - removing download",
                            episode.number, episode.name
                        );
                        // Delete the rejected file
                        if let Err(e) = tokio::fs::remove_file(&download.local_path).await {
                            warn!(
                                "Failed to delete rejected file {:?}: {}",
                                download.local_path, e
                            );
                        }
                        // Mark as failed so it won't be converted
                        valid_downloads.push(DownloadResult {
                            success: false,
                            error: Some("Failed validation criteria".to_string()),
                            ..download
                        });
                    }
                }
                None => {
                    warn!(
                        "Failed to extract metadata for S00E{:02} '{}', keeping download anyway",
                        episode.number, episode.name
                    );
                    valid_downloads.push(download);
                }
            }
        }

        valid_downloads
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContentCategory, SourceType, VideoSource};
    use std::path::PathBuf;

    fn create_test_episode(
        number: u8,
        name: &str,
        is_movie: Option<bool>,
    ) -> TvdbEpisodeExtended {
        TvdbEpisodeExtended {
            id: 1,
            number,
            name: name.to_string(),
            aired: None,
            overview: None,
            absolute_number: None,
            airs_before_season: None,
            airs_after_season: None,
            airs_before_episode: None,
            is_movie,
        }
    }

    fn create_test_download(title: &str, success: bool) -> DownloadResult {
        DownloadResult {
            source: VideoSource {
                url: "https://example.com/video".to_string(),
                source_type: SourceType::YouTube,
                category: ContentCategory::Featurette,
                title: title.to_string(),
                season_number: Some(0),
            },
            local_path: PathBuf::from("/tmp/video.mp4"),
            success,
            error: None,
        }
    }

    #[test]
    fn test_validate_movie_duration_too_short() {
        let episode = create_test_episode(1, "Test Movie", Some(true));
        let download = create_test_download("Test Movie", true);

        // 5 minutes (300 seconds) - should fail for movies
        assert!(!SpecialValidator::validate(
            &download,
            &episode,
            "Test Movie",
            300
        ));
    }

    #[test]
    fn test_validate_movie_duration_sufficient() {
        let episode = create_test_episode(1, "Test Movie", Some(true));
        let download = create_test_download("Test Movie", true);

        // 15 minutes (900 seconds) - should pass for movies
        assert!(SpecialValidator::validate(
            &download,
            &episode,
            "Test Movie",
            900
        ));
    }

    #[test]
    fn test_validate_non_movie_short_duration() {
        let episode = create_test_episode(1, "Test Special", None);
        let download = create_test_download("Test Special", true);

        // 5 minutes - should pass for non-movies
        assert!(SpecialValidator::validate(
            &download,
            &episode,
            "Test Special",
            300
        ));
    }

    #[test]
    fn test_validate_title_similarity_high() {
        let episode = create_test_episode(1, "Behind the Scenes", None);
        let download = create_test_download("Behind the Scenes", true);

        // Exact match - should pass
        assert!(SpecialValidator::validate(
            &download,
            &episode,
            "Behind the Scenes Special",
            600
        ));
    }

    #[test]
    fn test_validate_title_similarity_low() {
        let episode = create_test_episode(1, "Behind the Scenes", None);
        let download = create_test_download("Behind the Scenes", true);

        // Completely different title - should fail
        assert!(!SpecialValidator::validate(
            &download,
            &episode,
            "Unrelated Video Title",
            600
        ));
    }

    #[test]
    fn test_validate_movie_at_boundary() {
        let episode = create_test_episode(1, "Test Movie", Some(true));
        let download = create_test_download("Test Movie", true);

        // Exactly 10 minutes (600 seconds) - should pass
        assert!(SpecialValidator::validate(
            &download,
            &episode,
            "Test Movie",
            600
        ));

        // Just under 10 minutes (599 seconds) - should fail
        assert!(!SpecialValidator::validate(
            &download,
            &episode,
            "Test Movie",
            599
        ));
    }

    #[test]
    fn test_validate_complex_title_matching() {
        let episode = create_test_episode(
            3,
            "THE LEVELING OF SOLO LEVELING - Part 1 - A Hunter Rises",
            None,
        );
        let download = create_test_download("Test", true);

        // Similar title with extra words - should pass
        assert!(SpecialValidator::validate(
            &download,
            &episode,
            "Solo Leveling - THE LEVELING OF SOLO LEVELING Part 1 A Hunter Rises",
            1200
        ));

        // Partial match - might pass depending on similarity
        let result = SpecialValidator::validate(
            &download,
            &episode,
            "Solo Leveling Part 1 Hunter Rises",
            1200,
        );
        // This should pass as it contains key terms
        assert!(result);
    }
}
