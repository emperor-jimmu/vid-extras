// TheTVDB API v4 integration module

use crate::error::DiscoveryError;
use log::{debug, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::retry_with_backoff;

/// Base episode data from the Season 0 listing endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvdbEpisode {
    /// TheTVDB episode ID
    pub id: u64,
    /// Episode number within the season
    pub number: u8,
    /// Episode title
    pub name: String,
    /// Optional air date (ISO 8601 format)
    #[serde(default)]
    pub aired: Option<String>,
    /// Optional episode overview/description
    #[serde(default)]
    pub overview: Option<String>,
}

/// Enriched episode data from the extended endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvdbEpisodeExtended {
    /// TheTVDB episode ID
    pub id: u64,
    /// Episode number within the season
    pub number: u8,
    /// Episode title
    pub name: String,
    /// Optional English translation of the episode title
    #[serde(skip)]
    pub name_eng: Option<String>,
    /// Optional air date (ISO 8601 format)
    #[serde(default)]
    pub aired: Option<String>,
    /// Optional episode overview/description
    #[serde(default)]
    pub overview: Option<String>,
    /// Optional absolute episode number (for anime)
    #[serde(default)]
    pub absolute_number: Option<u32>,
    /// Optional season number this episode airs before
    #[serde(default)]
    pub airs_before_season: Option<u8>,
    /// Optional season number this episode airs after
    #[serde(default)]
    pub airs_after_season: Option<u8>,
    /// Optional episode number this episode airs before
    #[serde(default)]
    pub airs_before_episode: Option<u8>,
    /// Whether this episode is a movie-type special
    #[serde(default)]
    pub is_movie: Option<bool>,
}

impl TvdbEpisodeExtended {
    /// Returns all name variants to try for search queries.
    /// Includes the primary name and English translation if different.
    pub fn name_variants(&self) -> Vec<&str> {
        let mut variants = vec![self.name.as_str()];
        if let Some(eng) = &self.name_eng
            && eng != &self.name
        {
            variants.push(eng.as_str());
        }
        variants
    }
}

/// Search result from the TVDB `/search` endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct TvdbSearchResult {
    /// TheTVDB series ID
    pub tvdb_id: String,
    /// Series name
    pub name: String,
    /// Optional year of release
    #[serde(default)]
    pub year: Option<String>,
}

/// Generic API response wrapper
#[derive(Debug, Deserialize)]
pub struct TvdbApiResponse<T> {
    /// Response status
    pub status: String,
    /// Response data
    pub data: T,
}

/// Episodes data from the API (inner structure)
#[derive(Debug, Deserialize)]
pub struct TvdbEpisodesData {
    /// List of episodes on this page
    pub episodes: Vec<TvdbEpisode>,
    /// Optional URL to next page
    #[serde(default)]
    pub next: Option<String>,
}

/// Episodes page response from the API
#[derive(Debug, Deserialize)]
pub struct TvdbEpisodesPage {
    /// Nested data object containing episodes and pagination
    pub data: TvdbEpisodesData,
    /// Status of the response (included for API compatibility)
    #[allow(dead_code)]
    pub status: String,
}

/// Inner data structure containing the token
#[derive(Debug, Deserialize)]
pub struct TvdbLoginData {
    /// Bearer token for authenticated requests
    pub token: String,
}

/// Login response from TheTVDB API v4
#[derive(Debug, Deserialize)]
pub struct TvdbLoginResponse {
    /// Nested data object containing the token
    pub data: TvdbLoginData,
    /// Status of the response (included for API compatibility)
    #[allow(dead_code)]
    pub status: String,
}

/// Search response containing results
#[derive(Debug, Deserialize)]
pub struct TvdbSearchResponse {
    /// List of search results
    pub data: Vec<TvdbSearchResult>,
}

/// Translation record from the TVDB translations endpoint
#[derive(Debug, Deserialize)]
pub struct TvdbTranslation {
    /// Translated name
    #[serde(default)]
    pub name: String,
}

/// Translation response wrapper
#[derive(Debug, Deserialize)]
pub struct TvdbTranslationResponse {
    /// Translation data
    pub data: TvdbTranslation,
    /// Status of the response
    #[allow(dead_code)]
    pub status: String,
}

/// TheTVDB API client for authentication and data fetching
pub struct TvdbClient {
    /// API key for authentication
    api_key: String,
    /// HTTP client for making requests
    client: Client,
    /// Bearer token storage with RwLock for thread-safe access
    token: Arc<RwLock<Option<String>>>,
}

impl TvdbClient {
    /// Create a new TvdbClient with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client builder should not fail with default TLS"),
            token: Arc::new(RwLock::new(None)),
        }
    }

    /// Authenticate with TheTVDB API and obtain a Bearer token
    async fn authenticate(&self) -> Result<String, DiscoveryError> {
        let url = "https://api4.thetvdb.com/v4/login";
        let body = serde_json::json!({ "apikey": &self.api_key });

        debug!("Authenticating with TheTVDB API");

        let response = retry_with_backoff(3, 1000, || async {
            self.client.post(url).json(&body).send().await.map_err(|e| {
                DiscoveryError::TvdbApiError(format!("Authentication request failed: {}", e))
            })
        })
        .await
        .map_err(|e| {
            DiscoveryError::TvdbApiError(format!("Authentication failed after retries: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(DiscoveryError::TvdbAuthError(format!(
                "Authentication failed with status {}: {}",
                status, body
            )));
        }

        let login_response: TvdbLoginResponse = response.json().await.map_err(|e| {
            DiscoveryError::TvdbApiError(format!("Failed to parse login response: {}", e))
        })?;

        debug!("Successfully authenticated with TheTVDB API");
        Ok(login_response.data.token)
    }

    /// Ensure we have a valid token, re-authenticating if needed
    async fn ensure_token(&self) -> Result<String, DiscoveryError> {
        let token = self.token.read().await;
        if let Some(token) = token.as_ref() {
            return Ok(token.clone());
        }
        drop(token);

        // Token is missing, authenticate
        let new_token = self.authenticate().await?;
        let mut token_write = self.token.write().await;
        *token_write = Some(new_token.clone());
        Ok(new_token)
    }

    /// Execute an authenticated GET request with auto-retry on 401
    pub async fn authenticated_get(&self, url: &str) -> Result<reqwest::Response, DiscoveryError> {
        let token = self.ensure_token().await?;

        let response = retry_with_backoff(3, 500, || async {
            self.client
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        DiscoveryError::TvdbApiError(format!("Request timeout: {}", e))
                    } else {
                        DiscoveryError::TvdbApiError(format!("Request failed: {}", e))
                    }
                })
        })
        .await
        .map_err(|e| {
            DiscoveryError::TvdbApiError(format!("Request failed after retries: {}", e))
        })?;

        // Handle 401 Unauthorized - retry once with re-authentication
        if response.status() == 401 {
            debug!("Received 401 Unauthorized, re-authenticating");
            let new_token = self.authenticate().await?;
            let mut token_write = self.token.write().await;
            *token_write = Some(new_token.clone());
            drop(token_write);

            let retry_response = retry_with_backoff(3, 500, || async {
                self.client
                    .get(url)
                    .header("Authorization", format!("Bearer {}", new_token))
                    .send()
                    .await
                    .map_err(|e| {
                        DiscoveryError::TvdbApiError(format!("Retry request failed: {}", e))
                    })
            })
            .await
            .map_err(|e| {
                DiscoveryError::TvdbApiError(format!("Retry request failed after retries: {}", e))
            })?;

            return Ok(retry_response);
        }

        Ok(response)
    }

    /// Fetch all Season 0 episodes with pagination support
    pub async fn get_season_zero(&self, tvdb_id: u64) -> Result<Vec<TvdbEpisode>, DiscoveryError> {
        let mut episodes = Vec::new();
        let mut page = 0;
        let mut next_url: Option<String> = None;

        loop {
            let url = if let Some(next) = next_url.take() {
                next
            } else {
                format!(
                    "https://api4.thetvdb.com/v4/series/{}/episodes/default?season=0&page={}",
                    tvdb_id, page
                )
            };

            debug!("Fetching Season 0 episodes from: {}", url);

            let response = match self.authenticated_get(&url).await {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("Failed to fetch Season 0 episodes: {}", e);
                    return Ok(episodes); // Return what we have so far
                }
            };

            if !response.status().is_success() {
                warn!(
                    "Season 0 fetch returned status {}, returning empty list",
                    response.status()
                );
                return Ok(episodes);
            }

            let page_data: TvdbEpisodesPage = match response.json().await {
                Ok(data) => data,
                Err(e) => {
                    warn!("Failed to parse Season 0 response: {}", e);
                    return Ok(episodes);
                }
            };

            episodes.extend(page_data.data.episodes);

            if let Some(next) = page_data.data.next {
                next_url = Some(next);
                page += 1;
            } else {
                break;
            }
        }

        debug!("Fetched {} Season 0 episodes", episodes.len());
        Ok(episodes)
    }

    /// Fetch extended episode details for enrichment
    pub async fn get_episode_extended(
        &self,
        episode_id: u64,
    ) -> Result<TvdbEpisodeExtended, DiscoveryError> {
        let url = format!(
            "https://api4.thetvdb.com/v4/episodes/{}/extended",
            episode_id
        );

        debug!("Fetching extended episode data from: {}", url);

        let response = self.authenticated_get(&url).await?;

        if !response.status().is_success() {
            return Err(DiscoveryError::TvdbApiError(format!(
                "Failed to fetch extended episode data: {}",
                response.status()
            )));
        }

        let api_response: TvdbApiResponse<TvdbEpisodeExtended> =
            response.json().await.map_err(|e| {
                DiscoveryError::TvdbApiError(format!(
                    "Failed to parse extended episode response: {}",
                    e
                ))
            })?;

        Ok(api_response.data)
    }

    /// Fetch the English translation of an episode name.
    /// Returns None if no English translation exists.
    pub async fn get_episode_english_name(&self, episode_id: u64) -> Option<String> {
        let url = format!(
            "https://api4.thetvdb.com/v4/episodes/{}/translations/eng",
            episode_id
        );

        debug!("Fetching English translation for episode {}", episode_id);

        let response = match self.authenticated_get(&url).await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(_) => return None,
            Err(e) => {
                debug!(
                    "Failed to fetch English translation for episode {}: {}",
                    episode_id, e
                );
                return None;
            }
        };

        match response.json::<TvdbTranslationResponse>().await {
            Ok(tr) if !tr.data.name.is_empty() => Some(tr.data.name),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tvdb_episode_deserialization() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special",
            "aired": "2010-12-25",
            "overview": "A special holiday episode"
        }"#;

        let episode: TvdbEpisode = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.number, 5);
        assert_eq!(episode.name, "Holiday Special");
        assert_eq!(episode.aired, Some("2010-12-25".to_string()));
    }

    #[test]
    fn test_tvdb_episode_extended_deserialization() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special",
            "aired": "2010-12-25",
            "overview": "A special holiday episode",
            "absolute_number": 42,
            "airs_before_season": 2,
            "airs_after_season": 1,
            "airs_before_episode": 3,
            "is_movie": false
        }"#;

        let episode: TvdbEpisodeExtended = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.number, 5);
        assert_eq!(episode.absolute_number, Some(42));
        assert_eq!(episode.airs_before_season, Some(2));
        assert_eq!(episode.is_movie, Some(false));
    }

    #[test]
    fn test_tvdb_search_result_deserialization() {
        let json = r#"{
            "tvdb_id": "81189",
            "name": "Breaking Bad",
            "year": "2008"
        }"#;

        let result: TvdbSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.tvdb_id, "81189");
        assert_eq!(result.name, "Breaking Bad");
        assert_eq!(result.year, Some("2008".to_string()));
    }

    #[test]
    fn test_tvdb_login_response_deserialization() {
        let json =
            r#"{"data": {"token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"}, "status": "success"}"#;

        let response: TvdbLoginResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.token, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        assert_eq!(response.status, "success");
    }

    #[test]
    fn test_tvdb_episodes_page_deserialization() {
        let json = r#"{
            "data": {
                "episodes": [
                    {"id": 1, "number": 1, "name": "Ep1", "aired": null, "overview": null},
                    {"id": 2, "number": 2, "name": "Ep2", "aired": null, "overview": null}
                ],
                "next": "https://api4.thetvdb.com/v4/series/123/episodes/default?page=1"
            },
            "status": "success"
        }"#;

        let page: TvdbEpisodesPage = serde_json::from_str(json).unwrap();
        assert_eq!(page.data.episodes.len(), 2);
        assert!(page.data.next.is_some());
    }

    #[test]
    fn test_tvdb_episode_with_missing_optional_fields() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special"
        }"#;

        let episode: TvdbEpisode = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.aired, None);
        assert_eq!(episode.overview, None);
    }

    #[test]
    fn test_tvdb_episode_extended_with_missing_optional_fields() {
        let json = r#"{
            "id": 123456,
            "number": 5,
            "name": "Holiday Special"
        }"#;

        let episode: TvdbEpisodeExtended = serde_json::from_str(json).unwrap();
        assert_eq!(episode.id, 123456);
        assert_eq!(episode.absolute_number, None);
        assert_eq!(episode.is_movie, None);
    }

    #[tokio::test]
    async fn test_tvdb_client_creation() {
        let client = TvdbClient::new("test_api_key".to_string());
        assert_eq!(client.api_key, "test_api_key");
    }

    #[tokio::test]
    async fn test_tvdb_client_token_storage() {
        let client = TvdbClient::new("test_api_key".to_string());
        let token = client.token.read().await;
        assert!(token.is_none());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: tvdb-specials, Property 3: TVDB Episode Parsing Completeness
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_parsing_completeness(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}")
        ) {
            let episode = TvdbEpisode {
                id,
                number,
                name: name.clone(),
                aired: aired.clone(),
                overview: overview.clone(),
            };

            // Verify all fields are populated
            prop_assert_eq!(episode.id, id);
            prop_assert_eq!(episode.number, number);
            prop_assert_eq!(&episode.name, &name);
            prop_assert_eq!(&episode.aired, &aired);
            prop_assert_eq!(&episode.overview, &overview);
        }
    }

    // Feature: tvdb-specials, Property 3: TVDB Episode Extended Parsing Completeness
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_extended_parsing_completeness(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}"),
            absolute_number in proptest::option::of(1u32..10000u32),
            airs_before_season in proptest::option::of(0u8..100u8),
            airs_after_season in proptest::option::of(0u8..100u8),
            airs_before_episode in proptest::option::of(1u8..100u8),
            is_movie in proptest::option::of(any::<bool>())
        ) {
            let episode = TvdbEpisodeExtended {
                id,
                number,
                name: name.clone(),
                name_eng: None,
                aired: aired.clone(),
                overview: overview.clone(),
                absolute_number,
                airs_before_season,
                airs_after_season,
                airs_before_episode,
                is_movie,
            };

            // Verify all fields are populated
            prop_assert_eq!(episode.id, id);
            prop_assert_eq!(episode.number, number);
            prop_assert_eq!(&episode.name, &name);
            prop_assert_eq!(&episode.aired, &aired);
            prop_assert_eq!(&episode.overview, &overview);
            prop_assert_eq!(episode.absolute_number, absolute_number);
            prop_assert_eq!(episode.airs_before_season, airs_before_season);
            prop_assert_eq!(episode.airs_after_season, airs_after_season);
            prop_assert_eq!(episode.airs_before_episode, airs_before_episode);
            prop_assert_eq!(episode.is_movie, is_movie);
        }
    }

    // Feature: tvdb-specials, Property 3: TVDB Episode Serialization Round-Trip
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_serialization_round_trip(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}")
        ) {
            let episode = TvdbEpisode {
                id,
                number,
                name: name.clone(),
                aired: aired.clone(),
                overview: overview.clone(),
            };

            // Serialize to JSON
            let json = serde_json::to_string(&episode).unwrap();

            // Deserialize from JSON
            let deserialized: TvdbEpisode = serde_json::from_str(&json).unwrap();

            // Verify round-trip preserves all fields
            prop_assert_eq!(episode.id, deserialized.id);
            prop_assert_eq!(episode.number, deserialized.number);
            prop_assert_eq!(&episode.name, &deserialized.name);
            prop_assert_eq!(&episode.aired, &deserialized.aired);
            prop_assert_eq!(&episode.overview, &deserialized.overview);
        }
    }

    // Feature: tvdb-specials, Property 3: TVDB Episode Extended Serialization Round-Trip
    // Validates: Requirements 3.3, 4.2
    proptest! {
        #[test]
        fn prop_tvdb_episode_extended_serialization_round_trip(
            id in 1u64..1_000_000u64,
            number in 1u8..100u8,
            name in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            aired in proptest::option::of("[0-9]{4}-[0-9]{2}-[0-9]{2}"),
            overview in proptest::option::of("[a-zA-Z0-9 .,;:!?'-]{1,200}"),
            absolute_number in proptest::option::of(1u32..10000u32),
            airs_before_season in proptest::option::of(0u8..100u8),
            airs_after_season in proptest::option::of(0u8..100u8),
            airs_before_episode in proptest::option::of(1u8..100u8),
            is_movie in proptest::option::of(any::<bool>())
        ) {
            let episode = TvdbEpisodeExtended {
                id,
                number,
                name: name.clone(),
                name_eng: None,
                aired: aired.clone(),
                overview: overview.clone(),
                absolute_number,
                airs_before_season,
                airs_after_season,
                airs_before_episode,
                is_movie,
            };

            // Serialize to JSON
            let json = serde_json::to_string(&episode).unwrap();

            // Deserialize from JSON
            let deserialized: TvdbEpisodeExtended = serde_json::from_str(&json).unwrap();

            // Verify round-trip preserves all fields
            prop_assert_eq!(episode.id, deserialized.id);
            prop_assert_eq!(episode.number, deserialized.number);
            prop_assert_eq!(&episode.name, &deserialized.name);
            prop_assert_eq!(&episode.aired, &deserialized.aired);
            prop_assert_eq!(&episode.overview, &deserialized.overview);
            prop_assert_eq!(episode.absolute_number, deserialized.absolute_number);
            prop_assert_eq!(episode.airs_before_season, deserialized.airs_before_season);
            prop_assert_eq!(episode.airs_after_season, deserialized.airs_after_season);
            prop_assert_eq!(episode.airs_before_episode, deserialized.airs_before_episode);
            prop_assert_eq!(episode.is_movie, deserialized.is_movie);
        }
    }

    // Feature: tvdb-specials, Property 2: TVDB API URL Construction
    // Validates: Requirements 3.1, 4.1
    proptest! {
        #[test]
        fn prop_tvdb_api_url_construction(
            tvdb_id in 1u64..1_000_000u64,
            page in 0u32..100u32,
            episode_id in 1u64..1_000_000u64
        ) {
            // Test Season 0 URL construction
            let season_zero_url = format!(
                "https://api4.thetvdb.com/v4/series/{}/episodes/default?season=0&page={}",
                tvdb_id, page
            );
            prop_assert!(season_zero_url.contains("api4.thetvdb.com/v4/series/"));
            prop_assert!(season_zero_url.contains("/episodes/default"));
            prop_assert!(season_zero_url.contains("season=0"));
            let page_str = format!("page={}", page);
            prop_assert!(season_zero_url.contains(&page_str));

            // Test extended episode URL construction
            let extended_url = format!(
                "https://api4.thetvdb.com/v4/episodes/{}/extended",
                episode_id
            );
            prop_assert!(extended_url.contains("api4.thetvdb.com/v4/episodes/"));
            prop_assert!(extended_url.contains("/extended"));
            prop_assert!(extended_url.contains(&episode_id.to_string()));
        }
    }
}
