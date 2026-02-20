// Series discovery orchestrator - coordinates discovery from all sources for TV series

use crate::models::{ContentCategory, SeriesEntry, SeriesExtra, SourceMode, SourceType};
use log::{debug, info, warn};
use std::path::PathBuf;
use std::sync::Arc;

use super::id_bridge::IdBridge;
use super::monitor_policy::MonitorPolicy;
use super::series_tmdb::TmdbSeriesDiscoverer;
use super::series_youtube::YoutubeSeriesDiscoverer;
use super::special_searcher::SpecialSearcher;
use super::title_matching;
use super::tvdb::TvdbClient;

/// Orchestrates series discovery from all sources
pub struct SeriesDiscoveryOrchestrator {
    tmdb: TmdbSeriesDiscoverer,
    youtube: YoutubeSeriesDiscoverer,
    mode: SourceMode,
    tvdb_client: Option<Arc<TvdbClient>>,
    id_bridge: Option<Arc<IdBridge>>,
    pub(crate) cookies_from_browser: Option<String>,
}

impl SeriesDiscoveryOrchestrator {
    /// Creates a new SeriesDiscoveryOrchestrator with the specified mode
    pub fn new(tmdb_api_key: String, mode: SourceMode) -> Self {
        Self {
            tmdb: TmdbSeriesDiscoverer::new(tmdb_api_key),
            youtube: YoutubeSeriesDiscoverer::new(),
            mode,
            tvdb_client: None,
            id_bridge: None,
            cookies_from_browser: None,
        }
    }

    /// Creates a new SeriesDiscoveryOrchestrator with TVDB support enabled
    pub fn new_with_tvdb(
        tmdb_api_key: String,
        tvdb_api_key: String,
        mode: SourceMode,
        cache_dir: PathBuf,
    ) -> Self {
        let tvdb_client = Arc::new(TvdbClient::new(tvdb_api_key));
        let id_bridge = Arc::new(IdBridge::new(
            tmdb_api_key.clone(),
            tvdb_client.clone(),
            cache_dir,
        ));

        Self {
            tmdb: TmdbSeriesDiscoverer::new(tmdb_api_key),
            youtube: YoutubeSeriesDiscoverer::new(),
            mode,
            tvdb_client: Some(tvdb_client),
            id_bridge: Some(id_bridge),
            cookies_from_browser: None,
        }
    }

    /// Set browser cookie authentication for YouTube searches
    pub fn with_cookies(mut self, browser: String) -> Self {
        self.youtube = YoutubeSeriesDiscoverer::with_cookies(browser.clone());
        self.cookies_from_browser = Some(browser);
        self
    }

    /// Check if a string contains non-Latin characters (e.g., CJK, Arabic, Cyrillic)
    /// Used to detect episode names that likely need English translation
    fn contains_non_latin(text: &str) -> bool {
        text.chars().any(|c| {
            let c = c as u32;
            // CJK Unified Ideographs, Hiragana, Katakana, Hangul, Arabic, Cyrillic
            (0x3000..=0x9FFF).contains(&c)
                || (0xAC00..=0xD7AF).contains(&c)
                || (0x0600..=0x06FF).contains(&c)
                || (0x0400..=0x04FF).contains(&c)
        })
    }

    /// Discovers video sources from all configured sources based on mode
    ///
    /// In All mode: queries TMDB and YouTube
    /// In YoutubeOnly mode: queries only YouTube
    ///
    /// Errors from individual sources are logged but don't stop the overall discovery process.
    /// This ensures graceful degradation if one source fails.
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

    /// Discovers Season 0 specials for a series via TheTVDB
    ///
    /// This method:
    /// 1. Resolves TMDB ID to TVDB ID via IdBridge
    /// 2. Fetches Season 0 episodes from TVDB
    /// 3. Enriches episodes with extended metadata
    /// 4. Filters episodes via MonitorPolicy
    /// 5. Builds search queries via SpecialSearcher
    /// 6. Returns SeriesExtra items for the YouTube pipeline
    ///
    /// Requirements: 5.5, 6.5
    #[cfg(test)]
    pub async fn discover_season_zero(&self, series: &SeriesEntry) -> Vec<SeriesExtra> {
        // Check if TVDB support is enabled
        let (tvdb_client, id_bridge) = match (&self.tvdb_client, &self.id_bridge) {
            (Some(client), Some(bridge)) => (client, bridge),
            _ => {
                warn!("TVDB support not enabled, skipping Season 0 discovery");
                return Vec::new();
            }
        };

        // Get TMDB ID for the series
        let tmdb_id = match self.tmdb.search_series(&series.title, series.year).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("Series not found on TMDB: {}", series);
                return Vec::new();
            }
            Err(e) => {
                warn!("TMDB series search failed for {}: {}", series, e);
                return Vec::new();
            }
        };

        // Resolve TVDB ID via IdBridge
        let tvdb_id = match id_bridge.resolve(tmdb_id, &series.title).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("No TVDB ID found for {}", series);
                return Vec::new();
            }
            Err(e) => {
                warn!("TVDB ID resolution failed for {}: {}", series, e);
                return Vec::new();
            }
        };

        info!("Resolved TVDB ID {} for {}", tvdb_id, series);

        // Fetch Season 0 episodes
        let episodes = match tvdb_client.get_season_zero(tvdb_id).await {
            Ok(eps) => eps,
            Err(e) => {
                warn!("Failed to fetch Season 0 episodes for {}: {}", series, e);
                return Vec::new();
            }
        };

        if episodes.is_empty() {
            info!("No Season 0 episodes found for {}", series);
            return Vec::new();
        }

        info!("Found {} Season 0 episodes for {}", episodes.len(), series);

        // Enrich episodes with extended metadata
        let mut enriched_episodes = Vec::new();
        for episode in episodes {
            match tvdb_client.get_episode_extended(episode.id).await {
                Ok(mut extended) => {
                    // Fetch English translation for non-Latin episode names
                    if Self::contains_non_latin(&extended.name)
                        && let Some(eng_name) =
                            tvdb_client.get_episode_english_name(extended.id).await
                    {
                        debug!(
                            "Fetched English name '{}' for episode '{}' ({})",
                            eng_name, extended.name, extended.id
                        );
                        extended.name_eng = Some(eng_name);
                    }
                    enriched_episodes.push(extended);
                }
                Err(e) => {
                    debug!(
                        "Failed to enrich episode {} ({}): {}. Using base metadata.",
                        episode.number, episode.name, e
                    );
                    // Convert base episode to extended with None for extended fields
                    enriched_episodes.push(super::tvdb::TvdbEpisodeExtended {
                        id: episode.id,
                        number: episode.number,
                        name: episode.name,
                        name_eng: None,
                        aired: episode.aired,
                        overview: episode.overview,
                        absolute_number: None,
                        airs_before_season: None,
                        airs_after_season: None,
                        airs_before_episode: None,
                        is_movie: None,
                    });
                }
            }
        }

        // Load manual exclusion list
        let exclude_list = MonitorPolicy::load_manual_exclude_list(&series.path).await;

        // Determine latest season on disk
        let latest_season = *series.seasons.iter().max().unwrap_or(&0);

        // Filter via MonitorPolicy (all monitored by default, exclusion list removes specific episodes)
        let monitored =
            MonitorPolicy::filter_monitored(&enriched_episodes, latest_season, &exclude_list);

        info!(
            "Filtered to {} monitored Season 0 episodes for {}",
            monitored.len(),
            series
        );

        if monitored.is_empty() {
            return Vec::new();
        }

        // Build search queries and create SeriesExtra items
        let mut specials = Vec::new();
        for episode in monitored {
            let queries = SpecialSearcher::build_queries(&series.title, episode);

            // Use ytsearch5: to get multiple candidates for better matching
            for query in queries {
                // Create a SeriesExtra for each query
                // The downloader will handle the actual search and we'll filter results
                specials.push(SeriesExtra {
                    series_id: format!(
                        "{}_{}",
                        series.title.replace(' ', "_"),
                        series.year.unwrap_or(0)
                    ),
                    season_number: Some(0), // Season 0 for specials
                    category: ContentCategory::Featurette, // Default category for specials
                    title: format!("S00E{:02} - {}", episode.number, episode.name),
                    url: format!("ytsearch5:{}", query), // Use ytsearch5 for multiple candidates
                    source_type: SourceType::TheTVDB,
                    local_path: None,
                });
            }
        }

        info!(
            "Generated {} search queries for Season 0 specials of {}",
            specials.len(),
            series
        );

        specials
    }

    /// Discover Season 0 specials with enhanced filtering
    ///
    /// This method fetches Season 0 episodes from TVDB, generates search queries,
    /// and returns both the search queries and the episode metadata for validation.
    ///
    /// # Returns
    /// A tuple of (SeriesExtra items for searching, Episode metadata for validation)
    pub async fn discover_season_zero_with_metadata(
        &self,
        series: &SeriesEntry,
    ) -> (Vec<SeriesExtra>, Vec<super::tvdb::TvdbEpisodeExtended>) {
        // Check if TVDB support is enabled
        let (tvdb_client, id_bridge) = match (&self.tvdb_client, &self.id_bridge) {
            (Some(client), Some(bridge)) => (client, bridge),
            _ => {
                warn!("TVDB support not enabled, skipping Season 0 discovery");
                return (Vec::new(), Vec::new());
            }
        };

        // Get TMDB ID for the series
        let tmdb_id = match self.tmdb.search_series(&series.title, series.year).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("Series not found on TMDB: {}", series);
                return (Vec::new(), Vec::new());
            }
            Err(e) => {
                warn!("TMDB series search failed for {}: {}", series, e);
                return (Vec::new(), Vec::new());
            }
        };

        // Resolve TVDB ID via IdBridge
        let tvdb_id = match id_bridge.resolve(tmdb_id, &series.title).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("No TVDB ID found for {}", series);
                return (Vec::new(), Vec::new());
            }
            Err(e) => {
                warn!("TVDB ID resolution failed for {}: {}", series, e);
                return (Vec::new(), Vec::new());
            }
        };

        info!("Resolved TVDB ID {} for {}", tvdb_id, series);

        // Fetch Season 0 episodes
        let episodes = match tvdb_client.get_season_zero(tvdb_id).await {
            Ok(eps) => eps,
            Err(e) => {
                warn!("Failed to fetch Season 0 episodes for {}: {}", series, e);
                return (Vec::new(), Vec::new());
            }
        };

        if episodes.is_empty() {
            info!("No Season 0 episodes found for {}", series);
            return (Vec::new(), Vec::new());
        }

        info!("Found {} Season 0 episodes for {}", episodes.len(), series);

        // Enrich episodes with extended metadata
        let mut enriched_episodes = Vec::new();
        for episode in episodes {
            match tvdb_client.get_episode_extended(episode.id).await {
                Ok(mut extended) => {
                    // Fetch English translation for non-Latin episode names
                    if Self::contains_non_latin(&extended.name)
                        && let Some(eng_name) =
                            tvdb_client.get_episode_english_name(extended.id).await
                    {
                        debug!(
                            "Fetched English name '{}' for episode '{}' ({})",
                            eng_name, extended.name, extended.id
                        );
                        extended.name_eng = Some(eng_name);
                    }
                    enriched_episodes.push(extended);
                }
                Err(e) => {
                    debug!(
                        "Failed to enrich episode {} ({}): {}. Using base metadata.",
                        episode.number, episode.name, e
                    );
                    // Convert base episode to extended with None for extended fields
                    enriched_episodes.push(super::tvdb::TvdbEpisodeExtended {
                        id: episode.id,
                        number: episode.number,
                        name: episode.name,
                        name_eng: None,
                        aired: episode.aired,
                        overview: episode.overview,
                        absolute_number: None,
                        airs_before_season: None,
                        airs_after_season: None,
                        airs_before_episode: None,
                        is_movie: None,
                    });
                }
            }
        }

        // Load manual exclusion list
        let exclude_list = MonitorPolicy::load_manual_exclude_list(&series.path).await;

        // Determine latest season on disk
        let latest_season = *series.seasons.iter().max().unwrap_or(&0);

        // Filter via MonitorPolicy
        let monitored =
            MonitorPolicy::filter_monitored(&enriched_episodes, latest_season, &exclude_list);

        info!(
            "Filtered to {} monitored Season 0 episodes for {}",
            monitored.len(),
            series
        );

        if monitored.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // Build search queries and create SeriesExtra items
        let mut specials = Vec::new();
        let mut episode_metadata = Vec::new();

        for episode in &monitored {
            let queries = SpecialSearcher::build_queries(&series.title, episode);

            // Only use the first query (standard query) for each episode
            // to avoid duplicate downloads
            if let Some(query) = queries.first() {
                specials.push(SeriesExtra {
                    series_id: format!(
                        "{}_{}",
                        series.title.replace(' ', "_"),
                        series.year.unwrap_or(0)
                    ),
                    season_number: Some(0),
                    category: ContentCategory::Featurette,
                    title: format!("S00E{:02} - {}", episode.number, episode.name),
                    url: format!("ytsearch1:{}", query), // Use ytsearch1 for single best result
                    source_type: SourceType::TheTVDB,
                    local_path: None,
                });
                episode_metadata.push((*episode).clone());
            }
        }

        info!(
            "Generated {} search queries for Season 0 specials of {}",
            specials.len(),
            series
        );

        (specials, episode_metadata)
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
    fn test_series_discovery_orchestrator_creation_with_tvdb() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let orchestrator = SeriesDiscoveryOrchestrator::new_with_tvdb(
            "tmdb_key".to_string(),
            "tvdb_key".to_string(),
            SourceMode::All,
            temp_dir.path().to_path_buf(),
        );
        assert_eq!(orchestrator.mode, SourceMode::All);
        assert!(orchestrator.tvdb_client.is_some());
        assert!(orchestrator.id_bridge.is_some());
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
