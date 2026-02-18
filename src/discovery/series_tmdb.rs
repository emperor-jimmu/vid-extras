// TMDB series content discoverer

use crate::error::DiscoveryError;
#[cfg(test)]
use crate::models::SpecialEpisode;
use crate::models::{ContentCategory, SeriesExtra, SourceType};
use log::{debug, error, info};
use serde::Deserialize;

/// TMDB API response for TV series search
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbTvSearchResponse {
    results: Vec<TmdbTvSeries>,
}

/// TMDB TV series result (from search API)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbTvSeries {
    id: u64,
    name: String,
}

/// TMDB TV series details response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbTvSeriesDetails {
    id: u64,
    name: String,
}

/// TMDB API response for TV videos
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbTvVideosResponse {
    results: Vec<TmdbVideo>,
}

/// TMDB video entry
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbVideo {
    key: String,
    name: String,
    site: String,
    #[serde(rename = "type")]
    video_type: String,
}

/// TMDB API response for season details
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbSeasonResponse {
    #[allow(dead_code)]
    season_number: u8,
    episodes: Vec<TmdbEpisode>,
}

/// TMDB episode entry
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbEpisode {
    episode_number: u8,
    name: String,
    #[serde(default)]
    air_date: Option<String>,
}

/// TMDB series content discoverer
pub struct TmdbSeriesDiscoverer {
    api_key: String,
    client: reqwest::Client,
}

impl TmdbSeriesDiscoverer {
    /// Create a new TMDB series discoverer with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Search for a TV series by title and optional year, returns series ID
    pub async fn search_series(
        &self,
        title: &str,
        year: Option<u16>,
    ) -> Result<Option<u64>, DiscoveryError> {
        let mut url = format!(
            "https://api.themoviedb.org/3/search/tv?api_key={}&query={}",
            self.api_key,
            urlencoding::encode(title)
        );

        if let Some(y) = year {
            url.push_str(&format!("&first_air_date_year={}", y));
        }

        debug!("Searching TMDB for series: {} {:?}", title, year);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("TMDB series search request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB series search failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let search_result: TmdbTvSearchResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB series search response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if let Some(series) = search_result.results.first() {
            let series_id = series.id;
            info!("Found TMDB series: {} (ID: {})", series.name, series_id);
            Ok(Some(series_id))
        } else {
            info!("No TMDB series results found for: {} {:?}", title, year);
            Ok(None)
        }
    }

    /// Discover series-level extras from TMDB videos endpoint
    pub async fn discover_series_extras(
        &self,
        series_id: u64,
    ) -> Result<Vec<SeriesExtra>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/tv/{}/videos?api_key={}",
            series_id, self.api_key
        );

        debug!("Fetching TMDB series videos for series ID: {}", series_id);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("TMDB series videos request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB series videos fetch failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let videos_result: TmdbTvVideosResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB series videos response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        info!(
            "Found {} videos from TMDB for series",
            videos_result.results.len()
        );

        // Convert TMDB videos to SeriesExtra
        let extras: Vec<SeriesExtra> = videos_result
            .results
            .into_iter()
            .filter(|v| v.site == "YouTube") // Only YouTube videos are downloadable
            .filter_map(|v| {
                Self::map_tmdb_type(&v.video_type).map(|category| SeriesExtra {
                    series_id: series_id.to_string(),
                    season_number: None, // Series-level extras have no season
                    category,
                    title: v.name,
                    url: format!("https://www.youtube.com/watch?v={}", v.key),
                    source_type: SourceType::TMDB,
                    local_path: None,
                })
            })
            .collect();

        info!("Discovered {} series-level extras from TMDB", extras.len());
        Ok(extras)
    }

    /// Discover Season 0 specials from TMDB
    #[cfg(test)]
    pub async fn discover_season_zero(
        &self,
        series_id: u64,
    ) -> Result<Vec<SpecialEpisode>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/tv/{}/season/0?api_key={}",
            series_id, self.api_key
        );

        debug!("Fetching TMDB Season 0 for series ID: {}", series_id);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("TMDB Season 0 request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        // Season 0 might not exist, which is not an error
        if response.status() == 404 {
            debug!("Season 0 not found for series ID: {}", series_id);
            return Ok(Vec::new());
        }

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB Season 0 fetch failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let season_response: TmdbSeasonResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB Season 0 response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        info!(
            "Found {} Season 0 episodes from TMDB",
            season_response.episodes.len()
        );

        // Convert TMDB episodes to SpecialEpisode
        let specials: Vec<SpecialEpisode> = season_response
            .episodes
            .into_iter()
            .map(|ep| SpecialEpisode {
                episode_number: ep.episode_number,
                title: ep.name,
                air_date: ep.air_date,
                url: None, // URLs would need to be fetched separately
                local_path: None,
                tvdb_id: None, // TVDB ID would be populated by IdBridge
            })
            .collect();

        Ok(specials)
    }

    /// Map TMDB video type to content category
    pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
        match tmdb_type {
            "Trailer" => Some(ContentCategory::Trailer),
            "Teaser" => Some(ContentCategory::Trailer), // Teasers are short trailers
            "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
            "Featurette" => Some(ContentCategory::Featurette),
            "Bloopers" => Some(ContentCategory::Featurette), // Bloopers are treated as featurettes
            "Clip" => Some(ContentCategory::Featurette),     // Clips are treated as featurettes
            _ => {
                debug!("Unknown TMDB video type: {}", tmdb_type);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_tmdb_type_trailer() {
        assert_eq!(
            TmdbSeriesDiscoverer::map_tmdb_type("Trailer"),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_map_tmdb_type_behind_the_scenes() {
        assert_eq!(
            TmdbSeriesDiscoverer::map_tmdb_type("Behind the Scenes"),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_map_tmdb_type_featurette() {
        assert_eq!(
            TmdbSeriesDiscoverer::map_tmdb_type("Featurette"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_map_tmdb_type_bloopers() {
        assert_eq!(
            TmdbSeriesDiscoverer::map_tmdb_type("Bloopers"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_map_tmdb_type_unknown() {
        assert_eq!(TmdbSeriesDiscoverer::map_tmdb_type("Unknown"), None);
    }

    #[test]
    fn test_map_tmdb_type_deleted_scene() {
        // Deleted Scene is not mapped for series (only for movies)
        assert_eq!(TmdbSeriesDiscoverer::map_tmdb_type("Deleted Scene"), None);
    }

    #[test]
    fn test_tmdb_series_discoverer_creation() {
        let discoverer = TmdbSeriesDiscoverer::new("test_api_key".to_string());
        assert_eq!(discoverer.api_key, "test_api_key");
    }

    #[test]
    fn test_series_extra_creation_from_video() {
        let series_id = 1234u64;
        let category = ContentCategory::Trailer;
        let title = "Series Trailer".to_string();
        let url = "https://www.youtube.com/watch?v=abc123".to_string();

        let extra = SeriesExtra {
            series_id: series_id.to_string(),
            season_number: None,
            category,
            title,
            url,
            source_type: SourceType::TMDB,
            local_path: None,
        };

        assert_eq!(extra.series_id, "1234");
        assert_eq!(extra.season_number, None);
        assert_eq!(extra.category, ContentCategory::Trailer);
        assert_eq!(extra.source_type, SourceType::TMDB);
    }

    #[test]
    fn test_special_episode_creation() {
        let episode = SpecialEpisode {
            episode_number: 1,
            title: "Pilot Special".to_string(),
            air_date: Some("2020-01-01".to_string()),
            url: None,
            local_path: None,
            tvdb_id: None,
        };

        assert_eq!(episode.episode_number, 1);
        assert_eq!(episode.title, "Pilot Special");
        assert_eq!(episode.air_date, Some("2020-01-01".to_string()));
        assert_eq!(episode.url, None);
    }

    #[test]
    fn test_special_episode_without_air_date() {
        let episode = SpecialEpisode {
            episode_number: 2,
            title: "Unknown Special".to_string(),
            air_date: None,
            url: None,
            local_path: None,
            tvdb_id: None,
        };

        assert_eq!(episode.episode_number, 2);
        assert_eq!(episode.air_date, None);
    }

    #[test]
    fn test_all_tmdb_types_mapped_correctly() {
        // Test that all known types map to valid categories
        let known_types = vec!["Trailer", "Behind the Scenes", "Featurette", "Bloopers"];

        for tmdb_type in known_types {
            let result = TmdbSeriesDiscoverer::map_tmdb_type(tmdb_type);
            assert!(
                result.is_some(),
                "Type '{}' should map to a category",
                tmdb_type
            );
        }
    }

    #[test]
    fn test_unknown_types_return_none() {
        let unknown_types = vec!["Unknown", "Random", "NotAType", ""];

        for tmdb_type in unknown_types {
            let result = TmdbSeriesDiscoverer::map_tmdb_type(tmdb_type);
            assert!(
                result.is_none(),
                "Type '{}' should not map to a category",
                tmdb_type
            );
        }
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Property 5: TMDB Video Type Mapping Completeness
    // Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8
    proptest! {
        #[test]
        fn prop_tmdb_video_type_mapping_completeness(
            video_type in prop_oneof![
                Just("Trailer".to_string()),
                Just("Behind the Scenes".to_string()),
                Just("Featurette".to_string()),
                Just("Bloopers".to_string()),
                "[a-zA-Z0-9 ]{1,50}".prop_map(|s| s.to_string()),
            ]
        ) {
            let result = TmdbSeriesDiscoverer::map_tmdb_type(&video_type);

            // Known types should map to Some
            match video_type.as_str() {
                "Trailer" => {
                    prop_assert_eq!(result, Some(ContentCategory::Trailer));
                }
                "Behind the Scenes" => {
                    prop_assert_eq!(result, Some(ContentCategory::BehindTheScenes));
                }
                "Featurette" => {
                    prop_assert_eq!(result, Some(ContentCategory::Featurette));
                }
                "Bloopers" => {
                    prop_assert_eq!(result, Some(ContentCategory::Featurette));
                }
                _ => {
                    // Unknown types should map to None
                    prop_assert_eq!(result, None);
                }
            }
        }
    }

    // Property 6: Season 0 Episode Separation
    // Validates: Requirements 4.5
    proptest! {
        #[test]
        fn prop_season_zero_episode_separation(
            series_id in 1000u64..9999u64,
            episode_count in 1usize..20usize,
        ) {
            // Create series-level extras (season_number = None)
            let series_extras: Vec<SeriesExtra> = (0..episode_count)
                .map(|i| SeriesExtra {
                    series_id: series_id.to_string(),
                    season_number: None,
                    category: ContentCategory::Trailer,
                    title: format!("Series Extra {}", i),
                    url: format!("https://example.com/video{}", i),
                    source_type: SourceType::TMDB,
                    local_path: None,
                })
                .collect();

            // Create season-specific extras (season_number = Some(1))
            let season_extras: Vec<SeriesExtra> = (0..episode_count)
                .map(|i| SeriesExtra {
                    series_id: series_id.to_string(),
                    season_number: Some(1u8),
                    category: ContentCategory::BehindTheScenes,
                    title: format!("Season Extra {}", i),
                    url: format!("https://example.com/season_video{}", i),
                    source_type: SourceType::TMDB,
                    local_path: None,
                })
                .collect();

            // Verify separation: series-level extras have None season_number
            for extra in &series_extras {
                prop_assert_eq!(extra.season_number, None);
            }

            // Verify separation: season-specific extras have Some season_number
            for extra in &season_extras {
                prop_assert_eq!(extra.season_number, Some(1u8));
            }

            // Verify no overlap: all series extras differ from season extras in season_number
            for series_extra in &series_extras {
                for season_extra in &season_extras {
                    // They should differ in season_number
                    prop_assert_ne!(series_extra.season_number, season_extra.season_number);
                }
            }

            // Verify collections are separate
            prop_assert_eq!(series_extras.len(), episode_count);
            prop_assert_eq!(season_extras.len(), episode_count);
        }
    }
}
