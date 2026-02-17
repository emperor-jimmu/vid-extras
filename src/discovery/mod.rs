// Discovery module - handles content discovery from multiple sources

mod archive;
mod fuzzy_matching;
mod id_bridge;
mod monitor_policy;
mod orchestrator;
mod season_pack;
mod season_zero_import;
mod series_cache;
mod series_orchestrator;
mod series_tmdb;
mod series_youtube;
mod special_searcher;
mod special_validator;
mod title_matching;
mod tmdb;
mod tvdb;
mod youtube;

// Re-export public API
#[allow(dead_code, unused_imports)]
pub use fuzzy_matching::FuzzyMatcher;
#[allow(dead_code, unused_imports)]
pub use id_bridge::IdBridge;
#[allow(dead_code, unused_imports)]
pub use monitor_policy::MonitorPolicy;
pub use orchestrator::DiscoveryOrchestrator;
#[allow(dead_code, unused_imports)]
pub use season_pack::SeasonPackProcessor;
#[allow(dead_code, unused_imports)]
pub use season_zero_import::Season0Importer;
#[allow(unused_imports)]
pub use series_cache::{CachedSeriesMetadata, SeriesMetadataCache};
#[allow(unused_imports)]
pub use series_orchestrator::SeriesDiscoveryOrchestrator;
#[allow(dead_code, unused_imports)]
pub use special_searcher::SpecialSearcher;
#[allow(dead_code, unused_imports)]
pub use special_validator::SpecialValidator;
#[allow(dead_code, unused_imports)]
pub use tvdb::{
    TvdbApiResponse, TvdbClient, TvdbEpisode, TvdbEpisodeExtended, TvdbEpisodesPage,
    TvdbLoginResponse, TvdbSearchResponse, TvdbSearchResult,
};

use crate::error::DiscoveryError;
use crate::models::MovieEntry;
use crate::models::VideoSource;

/// Trait for content discoverers
#[allow(async_fn_in_trait)]
pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}
