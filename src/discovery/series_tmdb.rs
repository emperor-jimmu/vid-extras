// TMDB series content discoverer

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;

use super::orchestrator::{CandidateSource, DiscoveryQuery};
use super::retry_with_backoff;

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
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client builder should not fail with default TLS"),
        }
    }

    /// Search for a TV series by title and optional year, returns series ID
    pub async fn search_series(
        &self,
        title: &str,
        year: Option<u16>,
    ) -> Result<Option<u64>, DiscoveryError> {
        let mut url = format!(
            "https://api.themoviedb.org/3/search/tv?query={}",
            urlencoding::encode(title)
        );

        if let Some(y) = year {
            url.push_str(&format!("&first_air_date_year={}", y));
        }

        debug!("Searching TMDB for series: {} {:?}", title, year);

        let response = retry_with_backoff(3, 500, || async {
            self.client
                .get(&url)
                .bearer_auth(&self.api_key)
                .send()
                .await
                .map_err(|e| {
                    error!("TMDB series search request failed: {}", e);
                    DiscoveryError::NetworkError(e)
                })
        })
        .await?;

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
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        let url = format!("https://api.themoviedb.org/3/tv/{}/videos", series_id);

        debug!("Fetching TMDB series videos for series ID: {}", series_id);

        let response = retry_with_backoff(3, 500, || async {
            self.client
                .get(&url)
                .bearer_auth(&self.api_key)
                .send()
                .await
                .map_err(|e| {
                    error!("TMDB series videos request failed: {}", e);
                    DiscoveryError::NetworkError(e)
                })
        })
        .await?;

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

        // Convert TMDB videos to candidates
        let sources: Vec<VideoSource> = videos_result
            .results
            .into_iter()
            .filter(|v| v.site == "YouTube") // Only YouTube videos are downloadable
            .filter_map(|v| {
                Self::map_tmdb_type(&v.video_type).map(|category| VideoSource {
                    season_number: None, // Series-level extras have no season
                    category,
                    title: v.name,
                    url: format!("https://www.youtube.com/watch?v={}", v.key),
                    source_type: SourceType::TMDB,
                    duration_secs: None,
                })
            })
            .collect();

        info!("Discovered {} series-level extras from TMDB", sources.len());
        Ok(sources)
    }

    /// Map TMDB video type to content category
    pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
        match tmdb_type {
            "Trailer" => Some(ContentCategory::Trailer),
            "Teaser" => Some(ContentCategory::Trailer),
            "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
            "Featurette" => Some(ContentCategory::Featurette),
            "Bloopers" => Some(ContentCategory::Featurette),
            "Interview" => Some(ContentCategory::Interview),
            "Short" => Some(ContentCategory::Short),
            "Clip" => Some(ContentCategory::Featurette),
            _ => {
                debug!("Unknown TMDB video type: {}", tmdb_type);
                None
            }
        }
    }
}

impl CandidateSource for TmdbSeriesDiscoverer {
    async fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<VideoSource>, DiscoveryError> {
        // TMDB has no season-scoped videos endpoint in use here.
        if query.season.is_some() {
            return Ok(Vec::new());
        }

        match self.search_series(&query.title, query.year).await? {
            Some(series_id) => self.discover_series_extras(series_id).await,
            None => Ok(Vec::new()),
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
    fn test_map_tmdb_type_teaser() {
        assert_eq!(
            TmdbSeriesDiscoverer::map_tmdb_type("Teaser"),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_map_tmdb_type_interview() {
        assert_eq!(
            TmdbSeriesDiscoverer::map_tmdb_type("Interview"),
            Some(ContentCategory::Interview)
        );
    }

    #[test]
    fn test_map_tmdb_type_short() {
        assert_eq!(
            TmdbSeriesDiscoverer::map_tmdb_type("Short"),
            Some(ContentCategory::Short)
        );
    }

    #[test]
    fn test_tmdb_series_discoverer_creation() {
        let discoverer = TmdbSeriesDiscoverer::new("test_api_key".to_string());
        assert_eq!(discoverer.api_key, "test_api_key");
    }

    #[test]
    fn test_all_tmdb_types_mapped_correctly() {
        // Test that all known types map to valid categories
        let known_types = vec![
            "Trailer",
            "Teaser",
            "Behind the Scenes",
            "Featurette",
            "Bloopers",
            "Interview",
            "Short",
            "Clip",
        ];

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
                Just("Teaser".to_string()),
                Just("Behind the Scenes".to_string()),
                Just("Featurette".to_string()),
                Just("Bloopers".to_string()),
                Just("Interview".to_string()),
                Just("Short".to_string()),
                Just("Clip".to_string()),
                "[a-zA-Z0-9 ]{1,50}".prop_map(|s| s.to_string()),
            ]
        ) {
            let result = TmdbSeriesDiscoverer::map_tmdb_type(&video_type);

            // Known types should map to Some
            match video_type.as_str() {
                "Trailer" | "Teaser" => {
                    prop_assert_eq!(result, Some(ContentCategory::Trailer));
                }
                "Behind the Scenes" => {
                    prop_assert_eq!(result, Some(ContentCategory::BehindTheScenes));
                }
                "Featurette" | "Bloopers" | "Clip" => {
                    prop_assert_eq!(result, Some(ContentCategory::Featurette));
                }
                "Interview" => {
                    prop_assert_eq!(result, Some(ContentCategory::Interview));
                }
                "Short" => {
                    prop_assert_eq!(result, Some(ContentCategory::Short));
                }
                _ => {
                    // Unknown types should map to None
                    prop_assert_eq!(result, None);
                }
            }
        }
    }
}
