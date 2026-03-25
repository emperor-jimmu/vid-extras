// Discovery module - handles content discovery from multiple sources

mod archive;
pub(crate) mod dailymotion;
mod fuzzy_matching;
mod id_bridge;
mod kinocheck;
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
mod vimeo;
mod youtube;

// Re-export public API
pub use fuzzy_matching::FuzzyMatcher;
pub use id_bridge::IdBridge;
pub use monitor_policy::MonitorPolicy;
pub use orchestrator::DiscoveryOrchestrator;
pub use orchestrator::SourceResult;
pub use season_pack::SeasonPackProcessor;
pub use season_zero_import::Season0Importer;
pub use series_cache::{CachedSeriesMetadata, SeriesMetadataCache};
pub use series_orchestrator::SeriesDiscoveryOrchestrator;
pub use special_searcher::SpecialSearcher;
pub use special_validator::SpecialValidator;
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
