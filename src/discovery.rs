// Discovery module - handles content discovery from multiple sources

use crate::error::DiscoveryError;
use crate::models::MovieEntry;

/// Type of content source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    TMDB,
    ArchiveOrg,
    YouTube,
}

/// Content category for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentCategory {
    Trailer,
    Featurette,
    BehindTheScenes,
    DeletedScene,
}

/// Represents a video source discovered from an API
#[derive(Debug, Clone)]
pub struct VideoSource {
    pub url: String,
    pub source_type: SourceType,
    pub category: ContentCategory,
    pub title: String,
}

/// Trait for content discoverers
pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}

/// TMDB content discoverer
pub struct TmdbDiscoverer {
    api_key: String,
    client: reqwest::Client,
}

/// Archive.org content discoverer
pub struct ArchiveOrgDiscoverer {
    client: reqwest::Client,
}

/// YouTube content discoverer
pub struct YoutubeDiscoverer;

/// Orchestrates discovery from all sources
pub struct DiscoveryOrchestrator {
    tmdb: TmdbDiscoverer,
    archive: ArchiveOrgDiscoverer,
    youtube: YoutubeDiscoverer,
}
