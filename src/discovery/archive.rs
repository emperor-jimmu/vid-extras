// Archive.org content discoverer

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;

use super::ContentDiscoverer;

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

impl Default for ArchiveOrgDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveOrgDiscoverer {
    /// Create a new Archive.org discoverer
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Build Archive.org search query for a movie
    fn build_query(title: &str, year: u16) -> String {
        format!(
            "title:\"{}\" AND year:{} AND (subject:\"EPK\" OR subject:\"Making of\")",
            title, year
        )
    }

    /// Map Archive.org subjects to content categories
    fn map_subjects(subjects: &[String]) -> Option<ContentCategory> {
        // Check for EPK first, then Making of
        if subjects.iter().any(|s| s.eq_ignore_ascii_case("EPK")) {
            // EPK can be either featurette or behind the scenes
            // Default to featurette as it's more general
            Some(ContentCategory::Featurette)
        } else if subjects
            .iter()
            .any(|s| s.to_lowercase().contains("making of"))
        {
            Some(ContentCategory::BehindTheScenes)
        } else {
            None
        }
    }

    /// Search Archive.org for a movie
    async fn search(&self, title: &str, year: u16) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError> {
        let query = Self::build_query(title, year);
        let url = format!(
            "https://archive.org/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&fl[]=subject&rows=10&output=json",
            urlencoding::encode(&query)
        );

        debug!("Searching Archive.org for: {}", title);

        let response = self.client.get(&url).send().await.map_err(|e| {
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
        let docs = match self.search(&movie.title, movie.year).await {
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
