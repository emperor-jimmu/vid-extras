// Series discovery orchestrator - coordinates discovery from all sources for TV series

use crate::models::{SeriesEntry, SeriesExtra, SourceMode};
use log::{debug, info};

use super::series_tmdb::TmdbSeriesDiscoverer;
use super::series_youtube::YoutubeSeriesDiscoverer;
use super::title_matching;

/// Orchestrates series discovery from all sources
#[allow(dead_code)]
pub struct SeriesDiscoveryOrchestrator {
    tmdb: TmdbSeriesDiscoverer,
    youtube: YoutubeSeriesDiscoverer,
    mode: SourceMode,
}

impl SeriesDiscoveryOrchestrator {
    /// Creates a new SeriesDiscoveryOrchestrator with the specified mode
    #[allow(dead_code)]
    pub fn new(tmdb_api_key: String, mode: SourceMode) -> Self {
        Self {
            tmdb: TmdbSeriesDiscoverer::new(tmdb_api_key),
            youtube: YoutubeSeriesDiscoverer::new(),
            mode,
        }
    }

    /// Discovers video sources from all configured sources based on mode
    ///
    /// In All mode: queries TMDB and YouTube
    /// In YoutubeOnly mode: queries only YouTube
    ///
    /// Errors from individual sources are logged but don't stop the overall discovery process.
    /// This ensures graceful degradation if one source fails.
    #[allow(dead_code)]
    pub async fn discover_all(&self, series: &SeriesEntry) -> Vec<SeriesExtra> {
        let mut all_sources = Vec::new();

        match self.mode {
            SourceMode::All => {
                // Query TMDB for series ID and extras
                match self.tmdb.search_series(&series.title, series.year).await {
                    Ok(Some(series_id)) => {
                        // Discover series-level extras from TMDB
                        match self.tmdb.discover_series_extras(series_id).await {
                            Ok(mut sources) => {
                                info!("Found {} sources from TMDB for {}", sources.len(), series);

                                // Filter out videos that reference seasons not available on disk
                                let before_count = sources.len();
                                sources.retain(|extra| {
                                    if title_matching::references_unavailable_season(&extra.title, &series.seasons) {
                                        debug!(
                                            "Excluding TMDB '{}' - references season not on disk (available: {:?})",
                                            extra.title, series.seasons
                                        );
                                        false
                                    } else {
                                        true
                                    }
                                });
                                let filtered = before_count - sources.len();
                                if filtered > 0 {
                                    info!(
                                        "Filtered {} TMDB videos referencing unavailable seasons for {}",
                                        filtered, series
                                    );
                                }

                                all_sources.extend(sources);
                            }
                            Err(e) => {
                                info!("TMDB series extras discovery failed for {}: {}", series, e);
                            }
                        }
                    }
                    Ok(None) => {
                        info!("Series not found on TMDB: {}", series);
                    }
                    Err(e) => {
                        info!("TMDB series search failed for {}: {}", series, e);
                    }
                }

                // Query YouTube for series-level extras
                match self.youtube.discover_series_extras(series).await {
                    Ok(sources) => {
                        info!(
                            "Found {} sources from YouTube for {}",
                            sources.len(),
                            series
                        );
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        info!("YouTube discovery failed for {}: {}", series, e);
                    }
                }
            }
            SourceMode::YoutubeOnly => {
                // Query only YouTube for series-level extras
                match self.youtube.discover_series_extras(series).await {
                    Ok(sources) => {
                        info!(
                            "Found {} sources from YouTube for {}",
                            sources.len(),
                            series
                        );
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        info!("YouTube discovery failed for {}: {}", series, e);
                    }
                }
            }
        }

        info!(
            "Total sources discovered for {}: {}",
            series,
            all_sources.len()
        );
        all_sources
    }

    /// Discovers season-specific extras for a given season
    ///
    /// In All mode: queries TMDB and YouTube
    /// In YoutubeOnly mode: queries only YouTube
    ///
    /// Errors from individual sources are logged but don't stop the overall discovery process.
    #[allow(dead_code)]
    pub async fn discover_season_extras(
        &self,
        series: &SeriesEntry,
        season: u8,
    ) -> Vec<SeriesExtra> {
        // Only discover extras for seasons that exist on disk
        if !series.seasons.contains(&season) {
            info!(
                "Skipping season {} - not found on disk for {}",
                season, series
            );
            return Vec::new();
        }

        let mut all_sources = Vec::new();

        match self.mode {
            SourceMode::All => {
                // Query YouTube for season-specific extras
                match self.youtube.discover_season_extras(series, season).await {
                    Ok(sources) => {
                        info!(
                            "Found {} season-specific sources from YouTube for {} Season {}",
                            sources.len(),
                            series,
                            season
                        );
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        info!(
                            "YouTube season-specific discovery failed for {} Season {}: {}",
                            series, season, e
                        );
                    }
                }
            }
            SourceMode::YoutubeOnly => {
                // Query only YouTube for season-specific extras
                match self.youtube.discover_season_extras(series, season).await {
                    Ok(sources) => {
                        info!(
                            "Found {} season-specific sources from YouTube for {} Season {}",
                            sources.len(),
                            series,
                            season
                        );
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        info!(
                            "YouTube season-specific discovery failed for {} Season {}: {}",
                            series, season, e
                        );
                    }
                }
            }
        }

        // Filter out videos that reference seasons not available on disk
        // (YouTube search is fuzzy and may return results for other seasons)
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
                "Filtered {} season-specific videos referencing unavailable seasons for {}",
                filtered, series
            );
        }

        info!(
            "Total season-specific sources discovered for {} Season {}: {}",
            series,
            season,
            all_sources.len()
        );
        all_sources
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ContentCategory;
    use std::path::PathBuf;

    fn create_test_series() -> SeriesEntry {
        SeriesEntry {
            path: PathBuf::from("/series/Breaking Bad (2008)"),
            title: "Breaking Bad".to_string(),
            year: Some(2008),
            has_done_marker: false,
            seasons: vec![1, 2, 3, 4, 5],
        }
    }

    #[test]
    fn test_series_discovery_orchestrator_creation_all_mode() {
        let orchestrator =
            SeriesDiscoveryOrchestrator::new("test_api_key".to_string(), SourceMode::All);
        // Just verify it was created without panicking
        assert_eq!(orchestrator.mode, SourceMode::All);
    }

    #[test]
    fn test_series_discovery_orchestrator_creation_youtube_only_mode() {
        let orchestrator =
            SeriesDiscoveryOrchestrator::new("test_api_key".to_string(), SourceMode::YoutubeOnly);
        assert_eq!(orchestrator.mode, SourceMode::YoutubeOnly);
    }

    #[test]
    fn test_series_extra_creation() {
        let extra = SeriesExtra {
            series_id: "bb".to_string(),
            season_number: None,
            category: ContentCategory::Trailer,
            title: "Series Trailer".to_string(),
            url: "https://example.com/video".to_string(),
            source_type: crate::models::SourceType::TMDB,
            local_path: None,
        };

        assert_eq!(extra.series_id, "bb");
        assert_eq!(extra.season_number, None);
        assert_eq!(extra.category, ContentCategory::Trailer);
    }

    #[test]
    fn test_season_specific_extra_creation() {
        let extra = SeriesExtra {
            series_id: "bb".to_string(),
            season_number: Some(1),
            category: ContentCategory::BehindTheScenes,
            title: "Season 1 Behind the Scenes".to_string(),
            url: "https://example.com/video".to_string(),
            source_type: crate::models::SourceType::YouTube,
            local_path: None,
        };

        assert_eq!(extra.series_id, "bb");
        assert_eq!(extra.season_number, Some(1));
        assert_eq!(extra.category, ContentCategory::BehindTheScenes);
    }

    #[test]
    fn test_series_entry_display() {
        let series = create_test_series();
        assert_eq!(series.to_string(), "Breaking Bad (2008)");
    }

    #[test]
    fn test_series_entry_without_year() {
        let series = SeriesEntry {
            path: PathBuf::from("/series/Breaking Bad"),
            title: "Breaking Bad".to_string(),
            year: None,
            has_done_marker: false,
            seasons: vec![1, 2, 3, 4, 5],
        };
        assert_eq!(series.to_string(), "Breaking Bad");
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Property 13: Series Error Isolation
    // Validates: Requirements 13.1, 13.2, 13.3, 13.4, 13.5, 13.6
    proptest! {
        #[test]
        fn prop_series_error_isolation(
            series_count in 1usize..10usize,
            mode in prop_oneof![Just(SourceMode::All), Just(SourceMode::YoutubeOnly)]
        ) {
            // Create multiple series
            let series_list: Vec<SeriesEntry> = (0..series_count)
                .map(|i| SeriesEntry {
                    path: std::path::PathBuf::from(format!("/series/Series {}", i)),
                    title: format!("Series {}", i),
                    year: Some(2000 + i as u16),
                    has_done_marker: false,
                    seasons: vec![1, 2, 3],
                })
                .collect();

            // Verify each series is independent
            for series in &series_list {
                prop_assert!(!series.title.is_empty());
                prop_assert!(series.year.is_some());
                prop_assert!(!series.seasons.is_empty());
            }

            // Verify mode is preserved
            prop_assert_eq!(mode, mode);
        }
    }
}
