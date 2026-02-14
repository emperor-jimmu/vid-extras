// Discovery module - handles content discovery from multiple sources

use crate::error::DiscoveryError;
use crate::models::MovieEntry;

/// Type of content source
#[allow(dead_code)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    TMDB,
    ArchiveOrg,
    YouTube,
}

/// Content category for organization
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentCategory {
    Trailer,
    Featurette,
    BehindTheScenes,
    DeletedScene,
}

/// Represents a video source discovered from an API
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct VideoSource {
    pub url: String,
    pub source_type: SourceType,
    pub category: ContentCategory,
    pub title: String,
}

/// Trait for content discoverers
#[allow(dead_code)]
#[allow(async_fn_in_trait)]
pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}

/// TMDB content discoverer
#[allow(dead_code)]
pub struct TmdbDiscoverer {
    #[allow(dead_code)]
    api_key: String,
    #[allow(dead_code)]
    client: reqwest::Client,
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
