// Discovery module - handles content discovery from multiple sources

mod archive;
mod bilibili;
pub(crate) mod dailymotion;
mod fuzzy_matching;
mod id_bridge;
mod kinocheck;
mod monitor_policy;
mod orchestrator;
mod retry;
mod season_pack;
mod season_zero_import;
mod series_cache;
mod series_tmdb;
mod series_youtube;
mod special_searcher;
mod special_validator;
mod specials;
mod title_matching;
mod tmdb;
mod tvdb;
mod vimeo;
mod youtube;

// Re-export public API
pub use fuzzy_matching::FuzzyMatcher;
pub use id_bridge::IdBridge;
pub use monitor_policy::MonitorPolicy;
pub use orchestrator::{
    ContentLimits, Discovery, DiscoveryConfig, DiscoveryQuery, DiscoveryReport, LibraryKind,
    SourceResult,
};
pub use retry::retry_with_backoff;
pub use season_pack::SeasonPackProcessor;
pub use season_zero_import::Season0Importer;
pub use series_cache::{CachedSeriesMetadata, SeriesMetadataCache};
pub use special_searcher::SpecialSearcher;
pub use special_validator::SpecialValidator;
pub use specials::SpecialDiscovery;
pub use tvdb::{
    TvdbApiResponse, TvdbClient, TvdbEpisode, TvdbEpisodeExtended, TvdbEpisodesPage,
    TvdbLoginResponse, TvdbSearchResponse, TvdbSearchResult,
};
