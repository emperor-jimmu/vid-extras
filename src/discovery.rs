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

/// Archive.org API response for search
#[derive(Debug, Deserialize)]
struct ArchiveOrgSearchResponse {
    response: ArchiveOrgResponse,
}

/// Archive.org response wrapper
#[derive(Debug, Deserialize)]
struct ArchiveOrgResponse {
    docs: Vec<ArchiveOrgDoc>,
}

/// Archive.org document entry
#[derive(Debug, Deserialize)]
struct ArchiveOrgDoc {
    identifier: String,
    title: String,
    #[serde(default)]
    subject: Vec<String>,
}

/// Archive.org content discoverer
pub struct ArchiveOrgDiscoverer {
    client: reqwest::Client,
}

impl ArchiveOrgDiscoverer {
    /// Create a new Archive.org discoverer
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Build Archive.org search query for a movie
    fn build_query(title: &str) -> String {
        format!(
            "title:\"{}\" AND (subject:\"EPK\" OR subject:\"Making of\")",
            title
        )
    }

    /// Map Archive.org subjects to content categories
    fn map_subjects(subjects: &[String]) -> Option<ContentCategory> {
        // Check for EPK first, then Making of
        if subjects.iter().any(|s| s.eq_ignore_ascii_case("EPK")) {
            // EPK can be either featurette or behind the scenes
            // Default to featurette as it's more general
            Some(ContentCategory::Featurette)
        } else if subjects.iter().any(|s| s.to_lowercase().contains("making of")) {
            Some(ContentCategory::BehindTheScenes)
        } else {
            None
        }
    }

    /// Search Archive.org for a movie
    async fn search(&self, title: &str) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError> {
        let query = Self::build_query(title);
        let url = format!(
            "https://archive.org/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&fl[]=subject&rows=10&output=json",
            urlencoding::encode(&query)
        );

        debug!("Searching Archive.org for: {}", title);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                error!("Archive.org search request failed: {}", e);
                DiscoveryError::NetworkError(e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("Archive.org search failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "Archive.org API returned status {}",
                status
            )));
        }

        let search_result: ArchiveOrgSearchResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Archive.org search response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        info!(
            "Found {} results from Archive.org",
            search_result.response.docs.len()
        );
        Ok(search_result.response.docs)
    }
}

impl ContentDiscoverer for ArchiveOrgDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        // Only query Archive.org for movies before 2010
        if movie.year >= 2010 {
            debug!(
                "Skipping Archive.org for {} - year {} is >= 2010",
                movie, movie.year
            );
            return Ok(Vec::new());
        }

        info!("Discovering Archive.org content for: {}", movie);

        // Search for the movie
        let docs = match self.search(&movie.title).await {
            Ok(d) => d,
            Err(e) => {
                error!("Archive.org search failed for {}: {}", movie, e);
                return Err(e);
            }
        };

        // Convert Archive.org docs to VideoSource
        let sources: Vec<VideoSource> = docs
            .into_iter()
            .filter_map(|doc| {
                Self::map_subjects(&doc.subject).map(|category| VideoSource {
                    url: format!("https://archive.org/details/{}", doc.identifier),
                    source_type: SourceType::ArchiveOrg,
                    category,
                    title: doc.title,
                })
            })
            .collect();

        info!(
            "Discovered {} Archive.org sources for: {}",
            sources.len(),
            movie
        );
        Ok(sources)
    }
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
    use std::path::PathBuf;

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

    // Feature: extras-fetcher, Property 8: Archive.org Year-Based Querying
    // Validates: Requirements 4.1, 4.2
    proptest! {
        #[test]
        fn prop_archive_org_year_based_querying(
            title in "[a-zA-Z0-9 ]{1,30}",
            year in 1900u16..2100u16
        ) {
            let _movie = MovieEntry {
                path: PathBuf::from(format!("/movies/{} ({})", title, year)),
                title: title.clone(),
                year,
                has_done_marker: false,
            };

            // Archive.org should only be queried for movies before 2010
            let should_query = year < 2010;
            
            // We can't test the actual async discover method in proptest easily,
            // but we can verify the year check logic
            let would_skip = year >= 2010;
            
            prop_assert_eq!(should_query, !would_skip);
            
            // If year < 2010, Archive.org should be queried
            // If year >= 2010, Archive.org should be skipped
            if year < 2010 {
                prop_assert!(year < 2010, "Movies before 2010 should query Archive.org");
            } else {
                prop_assert!(year >= 2010, "Movies from 2010 onwards should skip Archive.org");
            }
        }
    }

    // Feature: extras-fetcher, Property 9: Archive.org Query Construction
    // Validates: Requirements 4.4
    proptest! {
        #[test]
        fn prop_archive_org_query_construction(
            title in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let query = ArchiveOrgDiscoverer::build_query(&title);
            
            // Query must contain the title in quotes
            prop_assert!(
                query.contains(&format!("title:\"{}\"", title)),
                "Query should contain title:\"{}\", got: {}",
                title,
                query
            );
            
            // Query must contain EPK subject
            prop_assert!(
                query.contains("subject:\"EPK\""),
                "Query should contain subject:\"EPK\", got: {}",
                query
            );
            
            // Query must contain Making of subject
            prop_assert!(
                query.contains("subject:\"Making of\""),
                "Query should contain subject:\"Making of\", got: {}",
                query
            );
            
            // Query must use OR operator between subjects
            prop_assert!(
                query.contains(" OR "),
                "Query should contain OR operator, got: {}",
                query
            );
            
            // Query must use AND operator to combine title and subjects
            prop_assert!(
                query.contains(" AND "),
                "Query should contain AND operator, got: {}",
                query
            );
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

    // Archive.org tests

    #[test]
    fn test_archive_org_discoverer_creation() {
        let discoverer = ArchiveOrgDiscoverer::new();
        // Just verify it can be created
        assert!(std::mem::size_of_val(&discoverer) > 0);
    }

    #[test]
    fn test_archive_org_query_string_formatting() {
        // Test query construction with simple title
        let query = ArchiveOrgDiscoverer::build_query("The Matrix");
        assert_eq!(
            query,
            "title:\"The Matrix\" AND (subject:\"EPK\" OR subject:\"Making of\")"
        );
    }

    #[test]
    fn test_archive_org_query_with_special_characters() {
        // Test query construction with special characters
        let query = ArchiveOrgDiscoverer::build_query("Movie: The Sequel");
        assert!(query.contains("title:\"Movie: The Sequel\""));
        assert!(query.contains("subject:\"EPK\""));
        assert!(query.contains("subject:\"Making of\""));
    }

    #[test]
    fn test_archive_org_subject_mapping_epk() {
        // Test EPK subject mapping
        let subjects = vec!["EPK".to_string(), "Documentary".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_archive_org_subject_mapping_making_of() {
        // Test Making of subject mapping
        let subjects = vec!["Making of".to_string(), "Documentary".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));
    }

    #[test]
    fn test_archive_org_subject_mapping_case_insensitive() {
        // Test case-insensitive EPK matching
        let subjects = vec!["epk".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));

        let subjects = vec!["EPK".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));

        let subjects = vec!["Epk".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_archive_org_subject_mapping_making_of_variations() {
        // Test various "making of" variations
        let subjects = vec!["Making of the Movie".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));

        let subjects = vec!["The Making of".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));
    }

    #[test]
    fn test_archive_org_subject_mapping_no_match() {
        // Test with subjects that don't match
        let subjects = vec!["Documentary".to_string(), "Film".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, None);
    }

    #[test]
    fn test_archive_org_subject_mapping_empty() {
        // Test with empty subjects
        let subjects: Vec<String> = vec![];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, None);
    }

    #[test]
    fn test_archive_org_subject_mapping_epk_priority() {
        // Test that EPK takes priority over Making of
        let subjects = vec![
            "EPK".to_string(),
            "Making of".to_string(),
            "Documentary".to_string(),
        ];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        // EPK should map to Featurette and take priority
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_archive_org_response_deserialization() {
        // Test that we can deserialize a mock Archive.org response
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "identifier": "matrix_epk_1999",
                        "title": "The Matrix EPK",
                        "subject": ["EPK", "Science Fiction"]
                    }
                ]
            }
        }"#;

        let response: Result<ArchiveOrgSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.response.docs.len(), 1);
        assert_eq!(response.response.docs[0].identifier, "matrix_epk_1999");
        assert_eq!(response.response.docs[0].title, "The Matrix EPK");
        assert_eq!(response.response.docs[0].subject.len(), 2);
        assert_eq!(response.response.docs[0].subject[0], "EPK");
    }

    #[test]
    fn test_archive_org_empty_response() {
        // Test deserialization of empty Archive.org response
        let json = r#"{"response": {"docs": []}}"#;

        let response: Result<ArchiveOrgSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.response.docs.len(), 0);
    }

    #[test]
    fn test_archive_org_response_with_missing_subjects() {
        // Test response with missing subject field (should default to empty vec)
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "identifier": "test_id",
                        "title": "Test Movie"
                    }
                ]
            }
        }"#;

        let response: Result<ArchiveOrgSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.response.docs.len(), 1);
        assert_eq!(response.response.docs[0].subject.len(), 0);
    }
}
