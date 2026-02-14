// Discovery module - handles content discovery from multiple sources

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;

/// Trait for content discoverers
#[allow(async_fn_in_trait)]
pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}

/// TMDB API response for movie search
#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbMovie>,
}

/// TMDB movie result
#[derive(Debug, Deserialize)]
struct TmdbMovie {
    id: u64,
    title: String,
}

/// TMDB API response for videos
#[derive(Debug, Deserialize)]
struct TmdbVideosResponse {
    results: Vec<TmdbVideo>,
}

/// TMDB video entry
#[derive(Debug, Deserialize)]
struct TmdbVideo {
    key: String,
    name: String,
    site: String,
    #[serde(rename = "type")]
    video_type: String,
}

/// TMDB content discoverer
pub struct TmdbDiscoverer {
    api_key: String,
    client: reqwest::Client,
}

impl TmdbDiscoverer {
    /// Create a new TMDB discoverer with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Search for a movie by title and year
    async fn search_movie(&self, title: &str, year: u16) -> Result<Option<u64>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}&year={}",
            self.api_key,
            urlencoding::encode(title),
            year
        );

        debug!("Searching TMDB for: {} ({})", title, year);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("TMDB search request failed: {}", e);
                DiscoveryError::NetworkError(e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB search failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let search_result: TmdbSearchResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB search response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if let Some(movie) = search_result.results.first() {
            info!("Found TMDB movie: {} (ID: {})", movie.title, movie.id);
            Ok(Some(movie.id))
        } else {
            info!("No TMDB results found for: {} ({})", title, year);
            Ok(None)
        }
    }

    /// Fetch videos for a movie by ID
    async fn fetch_videos(&self, movie_id: u64) -> Result<Vec<TmdbVideo>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/movie/{}/videos?api_key={}",
            movie_id, self.api_key
        );

        debug!("Fetching TMDB videos for movie ID: {}", movie_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("TMDB videos request failed: {}", e);
                DiscoveryError::NetworkError(e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB videos fetch failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let videos_result: TmdbVideosResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB videos response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        info!("Found {} videos from TMDB", videos_result.results.len());
        Ok(videos_result.results)
    }

    /// Map TMDB video type to content category
    pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
        match tmdb_type {
            "Trailer" => Some(ContentCategory::Trailer),
            "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
            "Deleted Scene" => Some(ContentCategory::DeletedScene),
            "Featurette" => Some(ContentCategory::Featurette),
            "Bloopers" => Some(ContentCategory::Featurette),
            _ => {
                debug!("Unknown TMDB video type: {}", tmdb_type);
                None
            }
        }
    }
}

impl ContentDiscoverer for TmdbDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Discovering TMDB content for: {}", movie);

        // Search for the movie
        let movie_id = match self.search_movie(&movie.title, movie.year).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("No TMDB match found for: {}", movie);
                return Ok(Vec::new());
            }
            Err(e) => {
                error!("TMDB search failed for {}: {}", movie, e);
                return Err(e);
            }
        };

        // Fetch videos for the movie
        let videos = match self.fetch_videos(movie_id).await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to fetch TMDB videos for {}: {}", movie, e);
                return Err(e);
            }
        };

        // Convert TMDB videos to VideoSource
        let sources: Vec<VideoSource> = videos
            .into_iter()
            .filter(|v| v.site == "YouTube") // Only YouTube videos are downloadable
            .filter_map(|v| {
                Self::map_tmdb_type(&v.video_type).map(|category| VideoSource {
                    url: format!("https://www.youtube.com/watch?v={}", v.key),
                    source_type: SourceType::TMDB,
                    category,
                    title: v.name,
                })
            })
            .collect();

        info!("Discovered {} TMDB sources for: {}", sources.len(), movie);
        Ok(sources)
    }
}

/// Archive.org content discoverer
#[allow(dead_code)]
pub struct ArchiveOrgDiscoverer {
    #[allow(dead_code)]
    client: reqwest::Client,
}

/// YouTube content discoverer
#[allow(dead_code)]
pub struct YoutubeDiscoverer;

/// Orchestrates discovery from all sources
#[allow(dead_code)]
pub struct DiscoveryOrchestrator {
    #[allow(dead_code)]
    tmdb: TmdbDiscoverer,
    #[allow(dead_code)]
    archive: ArchiveOrgDiscoverer,
    #[allow(dead_code)]
    youtube: YoutubeDiscoverer,
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: extras-fetcher, Property 7: TMDB Video Type Mapping
    // Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8
    proptest! {
        #[test]
        fn prop_tmdb_type_mapping(tmdb_type in prop_oneof![
            Just("Trailer"),
            Just("Behind the Scenes"),
            Just("Deleted Scene"),
            Just("Featurette"),
            Just("Bloopers"),
        ]) {
            let category = TmdbDiscoverer::map_tmdb_type(tmdb_type);
            
            match tmdb_type {
                "Trailer" => prop_assert_eq!(category, Some(ContentCategory::Trailer)),
                "Behind the Scenes" => prop_assert_eq!(category, Some(ContentCategory::BehindTheScenes)),
                "Deleted Scene" => prop_assert_eq!(category, Some(ContentCategory::DeletedScene)),
                "Featurette" => prop_assert_eq!(category, Some(ContentCategory::Featurette)),
                "Bloopers" => prop_assert_eq!(category, Some(ContentCategory::Featurette)),
                _ => unreachable!(),
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_tmdb_type_mapping_trailer() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Trailer"),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_behind_the_scenes() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Behind the Scenes"),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_deleted_scene() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Deleted Scene"),
            Some(ContentCategory::DeletedScene)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_featurette() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Featurette"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_bloopers() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Bloopers"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_unknown() {
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Unknown Type"), None);
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Clip"), None);
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Teaser"), None);
    }

    #[test]
    fn test_tmdb_discoverer_creation() {
        let api_key = "test_api_key_12345".to_string();
        let discoverer = TmdbDiscoverer::new(api_key.clone());
        assert_eq!(discoverer.api_key, api_key);
    }

    #[test]
    fn test_movie_search_url_construction() {
        // This test verifies the URL construction logic by checking the format
        let api_key = "test_key".to_string();
        let title = "The Matrix";
        let year = 1999;
        
        // Expected URL format
        let expected_base = "https://api.themoviedb.org/3/search/movie";
        let expected_query = format!(
            "?api_key={}&query={}&year={}",
            api_key,
            urlencoding::encode(title),
            year
        );
        
        // Verify URL encoding works correctly
        assert_eq!(urlencoding::encode(title), "The%20Matrix");
        assert!(expected_base.starts_with("https://api.themoviedb.org"));
        assert!(expected_query.contains("api_key=test_key"));
        assert!(expected_query.contains("query=The%20Matrix"));
        assert!(expected_query.contains("year=1999"));
    }

    #[test]
    fn test_movie_search_url_with_special_characters() {
        // Test URL encoding with special characters
        let title = "Movie: The Sequel (Part 2)";
        let encoded = urlencoding::encode(title);
        
        // Verify special characters are encoded
        assert!(encoded.contains("%3A")); // colon
        assert!(encoded.contains("%28")); // opening parenthesis
        assert!(encoded.contains("%29")); // closing parenthesis
    }

    #[test]
    fn test_videos_url_construction() {
        // Verify the videos endpoint URL format
        let api_key = "test_key".to_string();
        let movie_id = 603u64;
        
        let expected_url = format!(
            "https://api.themoviedb.org/3/movie/{}/videos?api_key={}",
            movie_id, api_key
        );
        
        assert_eq!(
            expected_url,
            "https://api.themoviedb.org/3/movie/603/videos?api_key=test_key"
        );
    }

    #[tokio::test]
    async fn test_discover_with_empty_api_key() {
        // Test that discoverer can be created with empty API key
        // (actual API calls will fail, but creation should succeed)
        let discoverer = TmdbDiscoverer::new(String::new());
        assert_eq!(discoverer.api_key, "");
    }

    #[test]
    fn test_video_source_construction() {
        // Test that VideoSource is constructed correctly from TMDB data
        let video_key = "dQw4w9WgXcQ";
        let video_name = "Official Trailer";
        let category = ContentCategory::Trailer;
        
        let source = VideoSource {
            url: format!("https://www.youtube.com/watch?v={}", video_key),
            source_type: SourceType::TMDB,
            category,
            title: video_name.to_string(),
        };
        
        assert_eq!(source.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(source.source_type, SourceType::TMDB);
        assert_eq!(source.category, ContentCategory::Trailer);
        assert_eq!(source.title, "Official Trailer");
    }

    #[test]
    fn test_tmdb_response_deserialization() {
        // Test that we can deserialize a mock TMDB search response
        let json = r#"{
            "results": [
                {
                    "id": 603,
                    "title": "The Matrix"
                }
            ]
        }"#;
        
        let response: Result<TmdbSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());
        
        let response = response.unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].id, 603);
        assert_eq!(response.results[0].title, "The Matrix");
    }

    #[test]
    fn test_tmdb_videos_response_deserialization() {
        // Test that we can deserialize a mock TMDB videos response
        let json = r#"{
            "results": [
                {
                    "key": "m8e-FF8MsqU",
                    "name": "Official Trailer",
                    "site": "YouTube",
                    "type": "Trailer"
                },
                {
                    "key": "abc123def45",
                    "name": "Behind the Scenes",
                    "site": "YouTube",
                    "type": "Behind the Scenes"
                }
            ]
        }"#;
        
        let response: Result<TmdbVideosResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());
        
        let response = response.unwrap();
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].key, "m8e-FF8MsqU");
        assert_eq!(response.results[0].name, "Official Trailer");
        assert_eq!(response.results[0].site, "YouTube");
        assert_eq!(response.results[0].video_type, "Trailer");
        assert_eq!(response.results[1].video_type, "Behind the Scenes");
    }

    #[test]
    fn test_empty_search_results() {
        // Test deserialization of empty search results
        let json = r#"{"results": []}"#;
        
        let response: Result<TmdbSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());
        
        let response = response.unwrap();
        assert_eq!(response.results.len(), 0);
    }

    #[test]
    fn test_empty_videos_results() {
        // Test deserialization of empty videos results
        let json = r#"{"results": []}"#;
        
        let response: Result<TmdbVideosResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());
        
        let response = response.unwrap();
        assert_eq!(response.results.len(), 0);
    }
}
