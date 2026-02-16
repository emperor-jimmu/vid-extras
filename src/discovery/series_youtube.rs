// YouTube content discoverer for TV series

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, SeriesEntry, SeriesExtra, SourceType};
use log::{debug, error, info};

use super::title_matching;

/// YouTube content discoverer for TV series
pub struct YoutubeSeriesDiscoverer;

impl YoutubeSeriesDiscoverer {
    /// Create a new YouTube series discoverer
    pub fn new() -> Self {
        Self
    }

    /// Build search queries for series-level extras
    pub fn build_series_search_queries(
        title: &str,
        year: Option<u16>,
    ) -> Vec<(String, ContentCategory)> {
        let year_str = year.map(|y| y.to_string()).unwrap_or_default();
        let base = if year_str.is_empty() {
            title.to_string()
        } else {
            format!("{} {}", title, year_str)
        };

        vec![
            (
                format!("{} cast interview", base),
                ContentCategory::Interview,
            ),
            (
                format!("{} behind the scenes", base),
                ContentCategory::BehindTheScenes,
            ),
            (format!("{} featurette", base), ContentCategory::Featurette),
        ]
    }

    /// Build search queries for season-specific extras
    pub fn build_season_search_queries(
        title: &str,
        year: Option<u16>,
        season: u8,
    ) -> Vec<(String, ContentCategory)> {
        let year_str = year.map(|y| y.to_string()).unwrap_or_default();
        let base = if year_str.is_empty() {
            format!("{} season {}", title, season)
        } else {
            format!("{} {} season {}", title, year_str, season)
        };

        vec![
            (
                format!("{} cast interview", base),
                ContentCategory::Interview,
            ),
            (
                format!("{} behind the scenes", base),
                ContentCategory::BehindTheScenes,
            ),
            (format!("{} featurette", base), ContentCategory::Featurette),
        ]
    }

    /// Check if duration is within acceptable range (30s - 20min)
    fn is_duration_valid(duration_secs: u32) -> bool {
        (30..=1200).contains(&duration_secs) // 20 minutes = 1200 seconds
    }

    /// Check if video is a YouTube Short (duration < 60s and vertical aspect ratio)
    fn is_youtube_short(duration_secs: u32, width: u32, height: u32) -> bool {
        if duration_secs >= 60 {
            return false;
        }
        height > width
    }

    /// Filter a video based on all criteria
    fn should_include_video(
        video_title: &str,
        series_title: &str,
        duration_secs: u32,
        width: u32,
        height: u32,
    ) -> bool {
        // Check if series title is in video title (with normalization)
        if !title_matching::contains_movie_title(video_title, series_title) {
            debug!(
                "Excluding '{}' - does not contain series title '{}' (normalized)",
                video_title, series_title
            );
            return false;
        }

        // Check duration range
        if !Self::is_duration_valid(duration_secs) {
            debug!(
                "Excluding '{}' - duration {}s out of range (30s-1200s)",
                video_title, duration_secs
            );
            return false;
        }

        // Check for excluded keywords
        if title_matching::contains_excluded_keywords(video_title) {
            debug!("Excluding '{}' - contains excluded keyword", video_title);
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
        series_title: &str,
        category: ContentCategory,
        _season_number: Option<u8>,
    ) -> Result<Vec<SeriesExtra>, DiscoveryError> {
        let search_query = format!("ytsearch5:{}", query);

        debug!("Searching YouTube with query: {}", query);

        // Execute yt-dlp to search and get video metadata
        let output = tokio::process::Command::new("yt-dlp")
            .arg("--dump-json")
            .arg("--no-playlist")
            .arg("--skip-download")
            .arg(&search_query)
            .output()
            .await
            .map_err(|e| {
                error!("Failed to execute yt-dlp: {}", e);
                DiscoveryError::YtDlpError(format!("Failed to execute yt-dlp: {}", e))
            })?;

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
                    if Self::should_include_video(&title, series_title, duration, width, height) {
                        // Extract actual season number from title
                        let extracted_seasons = title_matching::extract_season_numbers(&title);

                        // If title mentions specific seasons, use those; otherwise treat as
                        // series-level content (None) so it gets placed in the show root folder
                        // rather than a season subfolder.
                        let final_season = if !extracted_seasons.is_empty() {
                            // Use the first extracted season (most specific match)
                            Some(extracted_seasons[0])
                        } else {
                            // No season in title — this is general series content
                            None
                        };

                        sources.push(SeriesExtra {
                            series_id: series_title.to_lowercase().replace(' ', "_"),
                            season_number: final_season,
                            category,
                            title: title.clone(),
                            url,
                            source_type: SourceType::YouTube,
                            local_path: None,
                        });
                        debug!(
                            "Added YouTube video: {} (season: {:?})",
                            title, final_season
                        );
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

    /// Discover series-level YouTube extras
    pub async fn discover_series_extras(
        &self,
        series: &SeriesEntry,
    ) -> Result<Vec<SeriesExtra>, DiscoveryError> {
        info!("Discovering YouTube series-level extras for: {}", series);

        let queries = Self::build_series_search_queries(&series.title, series.year);
        let mut all_sources = Vec::new();

        for (query, category) in queries {
            match self
                .search_youtube(&query, &series.title, category, None)
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

        // Filter out videos that reference seasons not available on disk
        let before_count = all_sources.len();
        all_sources.retain(|extra| {
            if title_matching::references_unavailable_season(&extra.title, &series.seasons) {
                debug!(
                    "Excluding '{}' - references season not on disk (available: {:?})",
                    extra.title, series.seasons
                );
                false
            } else {
                true
            }
        });
        let filtered = before_count - all_sources.len();
        if filtered > 0 {
            info!(
                "Filtered {} videos referencing unavailable seasons for {}",
                filtered, series
            );
        }

        info!(
            "Discovered {} YouTube series-level sources for: {}",
            all_sources.len(),
            series
        );
        Ok(all_sources)
    }

    /// Discover season-specific YouTube extras
    pub async fn discover_season_extras(
        &self,
        series: &SeriesEntry,
        season: u8,
    ) -> Result<Vec<SeriesExtra>, DiscoveryError> {
        info!(
            "Discovering YouTube season-specific extras for: {} Season {}",
            series, season
        );

        let queries = Self::build_season_search_queries(&series.title, series.year, season);
        let mut all_sources = Vec::new();

        for (query, category) in queries {
            match self
                .search_youtube(&query, &series.title, category, Some(season))
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
                    // Continue with other queries even if one fails
                }
            }
        }

        info!(
            "Discovered {} YouTube season-specific sources for: {} Season {}",
            all_sources.len(),
            series,
            season
        );
        Ok(all_sources)
    }
}

impl Default for YoutubeSeriesDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Property 7: YouTube Series Query Construction
    // Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5
    proptest! {
        #[test]
        fn prop_youtube_series_query_construction(
            title in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            year in proptest::option::of(1900u16..2100u16)
        ) {
            let title_trimmed = title.trim();
            if title_trimmed.is_empty() {
                return Ok(());
            }

            let queries = YoutubeSeriesDiscoverer::build_series_search_queries(title_trimmed, year);

            // Should have 3 content types
            prop_assert_eq!(queries.len(), 3);

            // Extract query strings and categories
            let query_strings: Vec<_> = queries.iter().map(|(q, _)| q.clone()).collect();
            let categories: Vec<_> = queries.iter().map(|(_, c)| c.clone()).collect();

            // Verify all queries contain the series title
            for query in &query_strings {
                prop_assert!(query.contains(title_trimmed), "Query '{}' should contain title '{}'", query, title_trimmed);
            }

            // Verify year is included if present
            if let Some(y) = year {
                let year_str = y.to_string();
                for query in &query_strings {
                    prop_assert!(query.contains(&year_str), "Query '{}' should contain year '{}'", query, year_str);
                }
            }

            // Verify content types are correct
            prop_assert_eq!(categories[0], ContentCategory::Interview);
            prop_assert_eq!(categories[1], ContentCategory::BehindTheScenes);
            prop_assert_eq!(categories[2], ContentCategory::Featurette);

            // Verify query patterns contain expected keywords
            prop_assert!(query_strings[0].contains("interview"));
            prop_assert!(query_strings[1].contains("behind the scenes"));
            prop_assert!(query_strings[2].contains("featurette"));
        }
    }

    // Property 8: Season-Specific Query Tagging
    // Validates: Requirements 6.1, 6.2, 6.3
    proptest! {
        #[test]
        fn prop_season_specific_query_tagging(
            title in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            year in proptest::option::of(1900u16..2100u16),
            season in 1u8..100u8
        ) {
            let title_trimmed = title.trim();
            if title_trimmed.is_empty() {
                return Ok(());
            }

            let queries = YoutubeSeriesDiscoverer::build_season_search_queries(title_trimmed, year, season);

            // Should have 3 content types
            prop_assert_eq!(queries.len(), 3);

            // Extract query strings
            let query_strings: Vec<_> = queries.iter().map(|(q, _)| q.clone()).collect();

            // Verify all queries contain the series title
            for query in &query_strings {
                prop_assert!(query.contains(title_trimmed), "Query '{}' should contain title '{}'", query, title_trimmed);
            }

            // Verify season number is included in all queries
            let season_str = format!("season {}", season);
            for query in &query_strings {
                prop_assert!(query.contains(&season_str), "Query '{}' should contain '{}'", query, season_str);
            }

            // Verify year is included if present
            if let Some(y) = year {
                let year_str = y.to_string();
                for query in &query_strings {
                    prop_assert!(query.contains(&year_str), "Query '{}' should contain year '{}'", query, year_str);
                }
            }
        }
    }

    #[test]
    fn test_series_query_construction_with_year() {
        let queries =
            YoutubeSeriesDiscoverer::build_series_search_queries("Breaking Bad", Some(2008));
        assert_eq!(queries.len(), 3);
        assert!(queries[0].0.contains("Breaking Bad"));
        assert!(queries[0].0.contains("2008"));
        assert!(queries[0].0.contains("interview"));
    }

    #[test]
    fn test_series_query_construction_without_year() {
        let queries = YoutubeSeriesDiscoverer::build_series_search_queries("Breaking Bad", None);
        assert_eq!(queries.len(), 3);
        assert!(queries[0].0.contains("Breaking Bad"));
        assert!(!queries[0].0.contains("cast interview cast interview")); // No double year
    }

    #[test]
    fn test_season_query_construction_with_year() {
        let queries =
            YoutubeSeriesDiscoverer::build_season_search_queries("Breaking Bad", Some(2008), 1);
        assert_eq!(queries.len(), 3);
        assert!(queries[0].0.contains("Breaking Bad"));
        assert!(queries[0].0.contains("2008"));
        assert!(queries[0].0.contains("season 1"));
    }

    #[test]
    fn test_season_query_construction_without_year() {
        let queries = YoutubeSeriesDiscoverer::build_season_search_queries("Breaking Bad", None, 5);
        assert_eq!(queries.len(), 3);
        assert!(queries[0].0.contains("Breaking Bad"));
        assert!(queries[0].0.contains("season 5"));
    }

    #[test]
    fn test_season_query_construction_high_season_number() {
        let queries =
            YoutubeSeriesDiscoverer::build_season_search_queries("Game of Thrones", Some(2011), 8);
        assert_eq!(queries.len(), 3);
        assert!(queries[0].0.contains("season 8"));
    }

    #[test]
    fn test_duration_validation() {
        // Valid durations
        assert!(YoutubeSeriesDiscoverer::is_duration_valid(30)); // Minimum
        assert!(YoutubeSeriesDiscoverer::is_duration_valid(600)); // 10 minutes
        assert!(YoutubeSeriesDiscoverer::is_duration_valid(1200)); // Maximum (20 minutes)

        // Invalid durations
        assert!(!YoutubeSeriesDiscoverer::is_duration_valid(29)); // Too short
        assert!(!YoutubeSeriesDiscoverer::is_duration_valid(1201)); // Too long
        assert!(!YoutubeSeriesDiscoverer::is_duration_valid(0)); // Zero
    }

    #[test]
    fn test_youtube_short_detection() {
        // YouTube Short: < 60s and vertical
        assert!(YoutubeSeriesDiscoverer::is_youtube_short(30, 1080, 1920)); // Vertical

        // Not a short: >= 60s
        assert!(!YoutubeSeriesDiscoverer::is_youtube_short(60, 1080, 1920));

        // Not a short: horizontal
        assert!(!YoutubeSeriesDiscoverer::is_youtube_short(30, 1920, 1080));

        // Not a short: square
        assert!(!YoutubeSeriesDiscoverer::is_youtube_short(30, 1080, 1080));
    }

    #[test]
    fn test_should_include_video_basic() {
        // Should include: contains title, valid duration, no excluded keywords, not a short
        assert!(YoutubeSeriesDiscoverer::should_include_video(
            "Breaking Bad Season 1 Behind the Scenes",
            "Breaking Bad",
            600,
            1920,
            1080
        ));
    }

    #[test]
    fn test_should_include_video_missing_title() {
        // Should exclude: doesn't contain title
        assert!(!YoutubeSeriesDiscoverer::should_include_video(
            "Game of Thrones Behind the Scenes",
            "Breaking Bad",
            600,
            1920,
            1080
        ));
    }

    #[test]
    fn test_should_include_video_invalid_duration() {
        // Should exclude: duration too short
        assert!(!YoutubeSeriesDiscoverer::should_include_video(
            "Breaking Bad Behind the Scenes",
            "Breaking Bad",
            20,
            1920,
            1080
        ));

        // Should exclude: duration too long
        assert!(!YoutubeSeriesDiscoverer::should_include_video(
            "Breaking Bad Behind the Scenes",
            "Breaking Bad",
            2000,
            1920,
            1080
        ));
    }

    #[test]
    fn test_should_include_video_excluded_keywords() {
        // Should exclude: contains "Review"
        assert!(!YoutubeSeriesDiscoverer::should_include_video(
            "Breaking Bad Review",
            "Breaking Bad",
            600,
            1920,
            1080
        ));

        // Should exclude: contains "Reaction"
        assert!(!YoutubeSeriesDiscoverer::should_include_video(
            "Breaking Bad Reaction",
            "Breaking Bad",
            600,
            1920,
            1080
        ));
    }

    #[test]
    fn test_should_include_video_youtube_short() {
        // Should exclude: YouTube Short (< 60s and vertical)
        assert!(!YoutubeSeriesDiscoverer::should_include_video(
            "Breaking Bad Behind the Scenes",
            "Breaking Bad",
            30,
            1080,
            1920
        ));
    }
}
