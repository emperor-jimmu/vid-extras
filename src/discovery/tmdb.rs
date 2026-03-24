// TMDB content discoverer

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;
use std::collections::HashSet;

use super::ContentDiscoverer;

/// TMDB video types allowed from collection siblings (FR17).
/// Only cross-promotional content is included — trailers/teasers promote the sibling, not the library movie.
const SIBLING_ALLOWED_TYPES: [&str; 2] = ["Featurette", "Behind the Scenes"];

/// Discovery metadata including collection information
#[derive(Debug, Clone, Default)]
pub struct DiscoveryMetadata {
    /// Titles of other movies in the same collection (for exclusion filtering)
    pub collection_movie_titles: Vec<String>,
}

/// TMDB API response for movie search
#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbMovie>,
}

/// TMDB movie result (from search API)
#[derive(Debug, Deserialize)]
struct TmdbMovie {
    id: u64,
    title: String,
    // Note: search API doesn't return belongs_to_collection
}

/// TMDB movie details (from movie details API)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbMovieDetails {
    id: u64,
    title: String,
    #[serde(default)]
    belongs_to_collection: Option<TmdbCollection>,
}

/// TMDB collection information
#[derive(Debug, Deserialize, Clone)]
struct TmdbCollection {
    id: u64,
    name: String,
}

/// TMDB collection details response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbCollectionResponse {
    id: u64,
    name: String,
    #[serde(default)]
    parts: Vec<TmdbCollectionPart>,
}

/// TMDB collection part (movie in collection)
#[derive(Debug, Deserialize)]
pub(crate) struct TmdbCollectionPart {
    pub id: u64,
    pub title: String,
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
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client builder should not fail with default TLS"),
        }
    }

    /// Search for a movie by title and year, returns movie ID and optional collection info
    async fn search_movie(
        &self,
        title: &str,
        year: u16,
    ) -> Result<Option<(u64, Option<TmdbCollection>)>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}&year={}",
            self.api_key,
            urlencoding::encode(title),
            year
        );

        debug!("Searching TMDB for: {} ({})", title, year);

        let response = self.client.get(&url).send().await.map_err(|e| {
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
            let movie_id = movie.id;
            info!("Found TMDB movie: {} (ID: {})", movie.title, movie_id);

            // Fetch full movie details to get collection information
            // The search API doesn't return belongs_to_collection, so we need a second call
            let collection = self.fetch_movie_details(movie_id).await?;

            if let Some(ref coll) = collection {
                info!(
                    "Movie belongs to collection: {} (ID: {})",
                    coll.name, coll.id
                );
            } else {
                info!("No collection found for: {} ({})", title, year);
            }

            Ok(Some((movie_id, collection)))
        } else {
            info!("No TMDB results found for: {} ({})", title, year);
            Ok(None)
        }
    }

    /// Fetch movie details to get collection information
    async fn fetch_movie_details(
        &self,
        movie_id: u64,
    ) -> Result<Option<TmdbCollection>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/movie/{}?api_key={}",
            movie_id, self.api_key
        );

        debug!("Fetching TMDB movie details for ID: {}", movie_id);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("TMDB movie details request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB movie details failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let movie_details: TmdbMovieDetails = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB movie details response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        Ok(movie_details.belongs_to_collection)
    }

    /// Fetch collection details including all movies in the collection
    async fn fetch_collection(
        &self,
        collection_id: u64,
    ) -> Result<Vec<TmdbCollectionPart>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/collection/{}?api_key={}",
            collection_id, self.api_key
        );

        debug!("Fetching TMDB collection ID: {}", collection_id);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("TMDB collection request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB collection fetch failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let collection: TmdbCollectionResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB collection response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        let part_titles: Vec<&str> = collection.parts.iter().map(|p| p.title.as_str()).collect();
        info!(
            "Found {} movies in collection '{}': {:?}",
            collection.parts.len(),
            collection.name,
            part_titles
        );
        Ok(collection.parts)
    }

    /// Fetch videos for a movie by ID
    async fn fetch_videos(&self, movie_id: u64) -> Result<Vec<TmdbVideo>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/movie/{}/videos?api_key={}",
            movie_id, self.api_key
        );

        debug!("Fetching TMDB videos for movie ID: {}", movie_id);

        let response = self.client.get(&url).send().await.map_err(|e| {
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

    /// Fetch videos from a sibling movie in the same collection, filtered to
    /// cross-promotional types only (Featurette, Behind the Scenes).
    /// Skips the library movie itself.
    async fn fetch_sibling_videos(
        &self,
        sibling: &TmdbCollectionPart,
        library_movie_id: u64,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        if sibling.id == library_movie_id {
            return Ok(Vec::new());
        }

        let videos = self.fetch_videos(sibling.id).await?;

        let sources = videos
            .into_iter()
            .filter(|v| {
                v.site == "YouTube" && SIBLING_ALLOWED_TYPES.contains(&v.video_type.as_str())
            })
            .filter_map(|v| {
                Self::map_tmdb_type(&v.video_type).map(|category| VideoSource {
                    url: format!("https://www.youtube.com/watch?v={}", v.key),
                    source_type: SourceType::TMDB,
                    category,
                    title: format!("{} - {}", sibling.title, v.name),
                    season_number: None,
                })
            })
            .collect();

        Ok(sources)
    }

    /// Map TMDB video type to content category
    pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
        match tmdb_type {
            "Trailer" => Some(ContentCategory::Trailer),
            "Teaser" => Some(ContentCategory::Trailer),
            "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
            "Deleted Scene" => Some(ContentCategory::DeletedScene),
            "Featurette" => Some(ContentCategory::Featurette),
            "Bloopers" => Some(ContentCategory::Featurette),
            "Interview" => Some(ContentCategory::Interview),
            "Short" => Some(ContentCategory::Short),
            "Clip" => Some(ContentCategory::Clip),
            _ => {
                debug!("Unknown TMDB video type: {}", tmdb_type);
                None
            }
        }
    }

    /// Get discovery metadata including collection information
    pub async fn get_metadata(&self, movie: &MovieEntry) -> DiscoveryMetadata {
        let mut metadata = DiscoveryMetadata::default();

        // Search for the movie to get collection info
        match self.search_movie(&movie.title, movie.year).await {
            Ok(Some((_movie_id, Some(collection)))) => {
                info!(
                    "Movie '{}' is part of collection: {}",
                    movie, collection.name
                );
                // Fetch collection details
                match self.fetch_collection(collection.id).await {
                    Ok(parts) => {
                        // Exclude the current movie title from the list
                        metadata.collection_movie_titles = parts
                            .into_iter()
                            .filter(|p| !p.title.eq_ignore_ascii_case(&movie.title))
                            .map(|p| p.title)
                            .collect();

                        if metadata.collection_movie_titles.is_empty() {
                            info!(
                                "Collection '{}' has no other movies besides '{}'",
                                collection.name, movie.title
                            );
                        } else {
                            info!(
                                "Collection movies to exclude for '{}': {:?}",
                                movie, metadata.collection_movie_titles
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to fetch collection details for '{}': {}",
                            collection.name, e
                        );
                    }
                }
            }
            Ok(_) => {
                // No collection or movie not found
                info!(
                    "No collection found for: {} - will not filter collection movies",
                    movie
                );
            }
            Err(e) => {
                error!("Failed to search movie for metadata: {}", e);
            }
        }

        metadata
    }

    /// Returns true if a collection sibling is already present in the scanned library.
    ///
    /// Siblings in the library will process their own extras — fetching their featurettes
    /// for the current movie would produce duplicates across library folders.
    fn is_sibling_in_library(sibling: &TmdbCollectionPart, library: &[MovieEntry]) -> bool {
        library
            .iter()
            .any(|m| m.title.eq_ignore_ascii_case(&sibling.title))
    }

    /// Discover TMDB content, skipping collection siblings already present in the library.
    ///
    /// Pass the full scanned `library` so siblings that will be processed on their own
    /// turn are excluded from this movie's collection extras fetch.
    pub async fn discover_with_library(
        &self,
        movie: &MovieEntry,
        library: &[MovieEntry],
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Discovering TMDB content for: {}", movie);

        let (movie_id, collection_opt) = match self.search_movie(&movie.title, movie.year).await {
            Ok(Some(result)) => result,
            Ok(None) => {
                info!("No TMDB match found for: {}", movie);
                return Ok(Vec::new());
            }
            Err(e) => {
                error!("TMDB search failed for {}: {}", movie, e);
                return Err(e);
            }
        };

        let videos = match self.fetch_videos(movie_id).await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to fetch TMDB videos for {}: {}", movie, e);
                return Err(e);
            }
        };

        let mut sources: Vec<VideoSource> = videos
            .into_iter()
            .filter(|v| v.site == "YouTube")
            .filter_map(|v| {
                Self::map_tmdb_type(&v.video_type).map(|category| VideoSource {
                    url: format!("https://www.youtube.com/watch?v={}", v.key),
                    source_type: SourceType::TMDB,
                    category,
                    title: v.name,
                    season_number: None,
                })
            })
            .collect();

        info!("Discovered {} TMDB sources for: {}", sources.len(), movie);

        // Fetch collection sibling videos if movie belongs to a collection.
        // Siblings already in the library are skipped — they will fetch their own extras.
        // Requests are staggered by 100ms to avoid TMDB rate limits.
        if let Some(coll) = collection_opt {
            let parts = match self.fetch_collection(coll.id).await {
                Ok(p) => p,
                Err(e) => {
                    info!("TMDB collection fetch failed for {}: {}", movie, e);
                    vec![]
                }
            };

            let mut seen_urls: HashSet<String> = sources.iter().map(|s| s.url.clone()).collect();

            let mut handles = Vec::with_capacity(parts.len());
            for (i, part) in parts.into_iter().enumerate() {
                if Self::is_sibling_in_library(&part, library) {
                    info!(
                        "Skipping collection sibling '{}' — already in library",
                        part.title
                    );
                    continue;
                }
                let discoverer = TmdbDiscoverer::new(self.api_key.clone());
                let delay = tokio::time::Duration::from_millis(100 * i as u64);
                handles.push(tokio::spawn(async move {
                    tokio::time::sleep(delay).await;
                    (
                        part.title.clone(),
                        discoverer.fetch_sibling_videos(&part, movie_id).await,
                    )
                }));
            }

            let mut collection_count = 0;
            for handle in handles {
                match handle.await {
                    Ok((_sibling_title, Ok(extras))) => {
                        for extra in extras {
                            if seen_urls.insert(extra.url.clone()) {
                                collection_count += 1;
                                sources.push(extra);
                            }
                        }
                    }
                    Ok((sibling_title, Err(e))) => {
                        info!(
                            "TMDB sibling video fetch failed for '{}': {}",
                            sibling_title, e
                        );
                    }
                    Err(e) => {
                        info!("TMDB sibling task panicked: {}", e);
                    }
                }
            }

            if collection_count > 0 {
                info!(
                    "Discovered {} collection extras for: {}",
                    collection_count, movie
                );
            }
        }

        Ok(sources)
    }
}

impl ContentDiscoverer for TmdbDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        // No library context — collection siblings are not filtered.
        // Use discover_with_library when the scanned library is available.
        self.discover_with_library(movie, &[]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_tmdb_type_trailer() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Trailer"),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_map_tmdb_type_teaser() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Teaser"),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_map_tmdb_type_behind_the_scenes() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Behind the Scenes"),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_map_tmdb_type_deleted_scene() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Deleted Scene"),
            Some(ContentCategory::DeletedScene)
        );
    }

    #[test]
    fn test_map_tmdb_type_featurette() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Featurette"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_map_tmdb_type_bloopers() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Bloopers"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_map_tmdb_type_interview() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Interview"),
            Some(ContentCategory::Interview)
        );
    }

    #[test]
    fn test_map_tmdb_type_short() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Short"),
            Some(ContentCategory::Short)
        );
    }

    #[test]
    fn test_map_tmdb_type_clip() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Clip"),
            Some(ContentCategory::Clip)
        );
    }

    #[test]
    fn test_map_tmdb_type_unknown() {
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Unknown"), None);
        assert_eq!(TmdbDiscoverer::map_tmdb_type(""), None);
    }

    #[test]
    fn test_collection_video_type_filter_allows_featurette() {
        assert!(SIBLING_ALLOWED_TYPES.contains(&"Featurette"));
    }

    #[test]
    fn test_collection_video_type_filter_allows_behind_the_scenes() {
        assert!(SIBLING_ALLOWED_TYPES.contains(&"Behind the Scenes"));
    }

    #[test]
    fn test_collection_video_type_filter_rejects_trailer() {
        assert!(!SIBLING_ALLOWED_TYPES.contains(&"Trailer"));
    }

    #[test]
    fn test_collection_video_type_filter_rejects_teaser() {
        assert!(!SIBLING_ALLOWED_TYPES.contains(&"Teaser"));
    }

    #[test]
    fn test_collection_video_type_filter_rejects_clip() {
        assert!(!SIBLING_ALLOWED_TYPES.contains(&"Clip"));
    }

    #[test]
    fn test_collection_video_type_filter_rejects_bloopers() {
        assert!(!SIBLING_ALLOWED_TYPES.contains(&"Bloopers"));
    }

    #[test]
    fn test_collection_video_title_prefix() {
        let sibling_title = "Iron Man 3";
        let video_name = "Making of the Trilogy";
        let result = format!("{} - {}", sibling_title, video_name);
        assert_eq!(result, "Iron Man 3 - Making of the Trilogy");
    }

    #[test]
    fn test_collection_video_title_prefix_special_chars() {
        let sibling_title = "Spider-Man: No Way Home";
        let video_name = "Behind the Scenes Featurette";
        let result = format!("{} - {}", sibling_title, video_name);
        assert_eq!(
            result,
            "Spider-Man: No Way Home - Behind the Scenes Featurette"
        );
    }

    #[tokio::test]
    async fn test_fetch_sibling_videos_skips_library_movie() {
        let discoverer = TmdbDiscoverer::new("fake_key".to_string());
        let sibling = TmdbCollectionPart {
            id: 12345,
            title: "Same Movie".to_string(),
        };
        // When sibling.id == library_movie_id, should return empty without API call
        let result = discoverer.fetch_sibling_videos(&sibling, 12345).await;
        assert!(result.is_ok());
        assert!(result.expect("should succeed").is_empty());
    }

    #[test]
    fn test_sibling_allowed_types_covers_all_fr17_types() {
        // FR17: only Featurette and Behind the Scenes pass from siblings
        assert_eq!(SIBLING_ALLOWED_TYPES.len(), 2);
        assert!(SIBLING_ALLOWED_TYPES.contains(&"Featurette"));
        assert!(SIBLING_ALLOWED_TYPES.contains(&"Behind the Scenes"));
        // Verify map_tmdb_type agrees — no allowed type should return None
        for t in SIBLING_ALLOWED_TYPES {
            assert!(
                TmdbDiscoverer::map_tmdb_type(t).is_some(),
                "SIBLING_ALLOWED_TYPES contains '{}' but map_tmdb_type returns None for it",
                t
            );
        }
    }

    #[test]
    fn test_is_sibling_in_library_match() {
        use std::path::PathBuf;
        let library = vec![
            MovieEntry {
                path: PathBuf::from("/movies/Iron Man (2008)"),
                title: "Iron Man".to_string(),
                year: 2008,
                has_done_marker: false,
            },
            MovieEntry {
                path: PathBuf::from("/movies/Iron Man 2 (2010)"),
                title: "Iron Man 2".to_string(),
                year: 2010,
                has_done_marker: false,
            },
        ];
        let sibling_present = TmdbCollectionPart {
            id: 99,
            title: "Iron Man 2".to_string(),
        };
        let sibling_absent = TmdbCollectionPart {
            id: 100,
            title: "Iron Man 3".to_string(),
        };
        assert!(TmdbDiscoverer::is_sibling_in_library(&sibling_present, &library));
        assert!(!TmdbDiscoverer::is_sibling_in_library(&sibling_absent, &library));
    }

    #[test]
    fn test_is_sibling_in_library_case_insensitive() {
        use std::path::PathBuf;
        let library = vec![MovieEntry {
            path: PathBuf::from("/movies/Thor (2011)"),
            title: "Thor".to_string(),
            year: 2011,
            has_done_marker: false,
        }];
        let sibling = TmdbCollectionPart {
            id: 42,
            title: "THOR".to_string(),
        };
        assert!(TmdbDiscoverer::is_sibling_in_library(&sibling, &library));
    }

    #[test]
    fn test_is_sibling_in_library_empty_library() {
        let sibling = TmdbCollectionPart {
            id: 1,
            title: "Any Movie".to_string(),
        };
        assert!(!TmdbDiscoverer::is_sibling_in_library(&sibling, &[]));
    }
}
