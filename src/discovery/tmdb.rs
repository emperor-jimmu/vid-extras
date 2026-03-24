// TMDB content discoverer

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;

use super::ContentDiscoverer;

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
    parts: Vec<TmdbCollectionPart>,
}

/// TMDB collection part (movie in collection)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TmdbCollectionPart {
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

    /// Fetch collection details including all movie titles
    async fn fetch_collection(&self, collection_id: u64) -> Result<Vec<String>, DiscoveryError> {
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

        let titles: Vec<String> = collection.parts.iter().map(|p| p.title.clone()).collect();
        info!(
            "Found {} movies in collection '{}': {:?}",
            titles.len(),
            collection.name,
            titles
        );
        Ok(titles)
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

    /// Map TMDB video type to content category
    pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
        match tmdb_type {
            "Trailer" => Some(ContentCategory::Trailer),
            "Teaser" => Some(ContentCategory::Trailer), // Teasers are short trailers
            "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
            "Deleted Scene" => Some(ContentCategory::DeletedScene),
            "Featurette" => Some(ContentCategory::Featurette),
            "Clip" => Some(ContentCategory::Featurette), // Clips are treated as featurettes
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
                    Ok(titles) => {
                        // Exclude the current movie title from the list
                        metadata.collection_movie_titles = titles
                            .into_iter()
                            .filter(|t| !t.eq_ignore_ascii_case(&movie.title))
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
}

impl ContentDiscoverer for TmdbDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Discovering TMDB content for: {}", movie);

        // Search for the movie
        let (movie_id, _collection) = match self.search_movie(&movie.title, movie.year).await {
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
                    season_number: None,
                })
            })
            .collect();

        info!("Discovered {} TMDB sources for: {}", sources.len(), movie);
        Ok(sources)
    }
}
