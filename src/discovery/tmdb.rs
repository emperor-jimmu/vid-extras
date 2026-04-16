// TMDB content discoverer

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;

use super::ContentDiscoverer;

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

    /// Search for a movie by title and year, returns the TMDB movie ID
    async fn search_movie(&self, title: &str, year: u16) -> Result<Option<u64>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}&year={}",
            self.api_key,
            urlencoding::encode(title),
            year
        );

        debug!("Searching TMDB for: {} ({})", title, year);

        let response = super::retry_with_backoff(3, 500, || async {
            self.client.get(&url).send().await.map_err(|e| {
                error!("TMDB search request failed: {}", e);
                DiscoveryError::NetworkError(e)
            })
        })
        .await?;

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

        let response = super::retry_with_backoff(3, 500, || async {
            self.client.get(&url).send().await.map_err(|e| {
                error!("TMDB videos request failed: {}", e);
                DiscoveryError::NetworkError(e)
            })
        })
        .await?;

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
            "Teaser" => Some(ContentCategory::Trailer),
            "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
            "Deleted Scene" => Some(ContentCategory::DeletedScene),
            "Featurette" => Some(ContentCategory::Featurette),
            "Bloopers" => Some(ContentCategory::Featurette),
            "Interview" => Some(ContentCategory::Interview),
            "Short" => Some(ContentCategory::Short),
            _ => {
                debug!("Unknown TMDB video type: {}", tmdb_type);
                None
            }
        }
    }

    /// Discover TMDB content for a movie, returning sources and the TMDB movie ID.
    pub async fn discover_for_movie(
        &self,
        movie: &MovieEntry,
    ) -> Result<(Vec<VideoSource>, Option<u64>), DiscoveryError> {
        info!("Discovering TMDB content for: {}", movie);

        let movie_id = match self.search_movie(&movie.title, movie.year).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("No TMDB match found for: {}", movie);
                return Ok((Vec::new(), None));
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

        let sources: Vec<VideoSource> = videos
            .into_iter()
            .filter(|v| v.site == "YouTube")
            .filter_map(|v| {
                Self::map_tmdb_type(&v.video_type).map(|category| VideoSource {
                    url: format!("https://www.youtube.com/watch?v={}", v.key),
                    source_type: SourceType::TMDB,
                    category,
                    title: v.name,
                    season_number: None,
                    duration_secs: None,
                })
            })
            .collect();

        info!("Discovered {} TMDB sources for: {}", sources.len(), movie);
        Ok((sources, Some(movie_id)))
    }
}

impl ContentDiscoverer for TmdbDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        let (sources, _movie_id) = self.discover_for_movie(movie).await?;
        Ok(sources)
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
    fn test_map_tmdb_type_unknown() {
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Unknown"), None);
        assert_eq!(TmdbDiscoverer::map_tmdb_type(""), None);
    }
}
