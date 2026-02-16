// Discovery module - handles content discovery from multiple sources

mod archive;
mod orchestrator;
mod series_orchestrator;
mod series_tmdb;
mod series_youtube;
mod title_matching;
mod tmdb;
mod youtube;

// Re-export public API
pub use orchestrator::DiscoveryOrchestrator;
#[allow(unused_imports)]
pub use series_orchestrator::SeriesDiscoveryOrchestrator;

use crate::error::DiscoveryError;
use crate::models::MovieEntry;
use crate::models::VideoSource;

/// Trait for content discoverers
#[allow(async_fn_in_trait)]
pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}
