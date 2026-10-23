//! Season 0 special discovery — TVDB metadata plus candidate selection.
//!
//! This flow is deliberately outside the discovery module: it turns episode
//! metadata into one selected video per special, rather than querying sources
//! for candidates.

use crate::models::{ContentCategory, SeriesEntry, SourceType, VideoSource};
use log::{debug, info, warn};
use std::path::PathBuf;
use std::sync::Arc;

use super::id_bridge::IdBridge;
use super::monitor_policy::MonitorPolicy;
use super::series_tmdb::TmdbSeriesDiscoverer;
use super::special_validator::SpecialValidator;
use super::tvdb::{TvdbClient, TvdbEpisodeExtended};

/// Discovers Season 0 specials for a series and selects one video per episode.
pub struct SpecialDiscovery {
    tmdb: TmdbSeriesDiscoverer,
    tvdb_client: Option<Arc<TvdbClient>>,
    id_bridge: Option<Arc<IdBridge>>,
    cookies_from_browser: Option<String>,
}

impl SpecialDiscovery {
    /// Builds the Season 0 module. TVDB support is enabled only when a key is supplied.
    pub fn new(
        tmdb_api_key: String,
        tvdb_api_key: Option<String>,
        cache_dir: PathBuf,
        cookies_from_browser: Option<String>,
    ) -> Self {
        let (tvdb_client, id_bridge) = match tvdb_api_key {
            Some(key) => {
                let client = Arc::new(TvdbClient::new(key));
                let bridge = Arc::new(IdBridge::new(
                    tmdb_api_key.clone(),
                    client.clone(),
                    cache_dir,
                ));
                (Some(client), Some(bridge))
            }
            None => (None, None),
        };

        Self {
            tmdb: TmdbSeriesDiscoverer::new(tmdb_api_key),
            tvdb_client,
            id_bridge,
            cookies_from_browser,
        }
    }

    /// Select the best video for each monitored Season 0 episode.
    ///
    /// Returns the selected candidates and the episode metadata they pair with,
    /// in the same order, so organization can name the files.
    pub async fn select(
        &self,
        series: &SeriesEntry,
    ) -> (Vec<VideoSource>, Vec<TvdbEpisodeExtended>) {
        let episodes = self.monitored_episodes(series).await;
        if episodes.is_empty() {
            return (Vec::new(), Vec::new());
        }

        info!(
            "Selecting best candidates for {} Season 0 episodes of {}",
            episodes.len(),
            series
        );
        let selections = SpecialValidator::select_best_candidates(
            &series.title,
            &episodes,
            self.cookies_from_browser.as_deref(),
        )
        .await;

        let mut candidates = Vec::new();
        let mut matched_episodes = Vec::new();

        for (selection, episode) in selections.into_iter().zip(episodes) {
            if let Some(selected) = selection {
                candidates.push(VideoSource {
                    url: selected.url,
                    source_type: SourceType::TheTVDB,
                    category: ContentCategory::Featurette,
                    title: format!("S00E{:02} - {}", episode.number, episode.name),
                    season_number: Some(0),
                    duration_secs: None,
                });
                matched_episodes.push(episode);
            }
        }

        info!(
            "Found {} valid Season 0 specials for {}",
            candidates.len(),
            series
        );
        (candidates, matched_episodes)
    }

    /// Fetch, enrich, and filter the monitored Season 0 episodes.
    async fn monitored_episodes(&self, series: &SeriesEntry) -> Vec<TvdbEpisodeExtended> {
        let (tvdb_client, id_bridge) = match (&self.tvdb_client, &self.id_bridge) {
            (Some(client), Some(bridge)) => (client, bridge),
            _ => {
                warn!("TVDB support not enabled, skipping Season 0 discovery");
                return Vec::new();
            }
        };

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

        let mut enriched_episodes = Vec::new();
        for episode in episodes {
            match tvdb_client.get_episode_extended(episode.id).await {
                Ok(mut extended) => {
                    // Fetch English translation for non-Latin episode names
                    if contains_non_latin(&extended.name)
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
                    enriched_episodes.push(TvdbEpisodeExtended {
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

        let exclude_list = MonitorPolicy::load_manual_exclude_list(&series.path).await;
        let monitored = MonitorPolicy::filter_monitored(&enriched_episodes, &exclude_list);

        info!(
            "Filtered to {} monitored Season 0 episodes for {}",
            monitored.len(),
            series
        );
        monitored.into_iter().cloned().collect()
    }
}

/// Check if a string contains non-Latin characters (e.g., CJK, Arabic, Cyrillic).
/// Used to detect episode names that likely need English translation.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_non_latin_detects_cjk() {
        assert!(contains_non_latin("特別編"));
        assert!(!contains_non_latin("Behind the Scenes"));
    }

    #[test]
    fn test_without_tvdb_has_no_clients() {
        let discovery =
            SpecialDiscovery::new("tmdb_key".to_string(), None, PathBuf::from(".cache"), None);
        assert!(discovery.tvdb_client.is_none());
        assert!(discovery.id_bridge.is_none());
    }

    #[test]
    fn test_with_tvdb_builds_clients() {
        let discovery = SpecialDiscovery::new(
            "tmdb_key".to_string(),
            Some("tvdb_key".to_string()),
            PathBuf::from(".cache"),
            Some("chrome".to_string()),
        );
        assert!(discovery.tvdb_client.is_some());
        assert!(discovery.id_bridge.is_some());
        assert_eq!(discovery.cookies_from_browser.as_deref(), Some("chrome"));
    }
}
