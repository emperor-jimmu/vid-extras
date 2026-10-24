//! Orchestrator: scans the library, hands items to the pipeline, folds outcomes into
//! the run summary. All per-item policy lives in `pipeline`.

use crate::converter::Converter;
use crate::discovery::{Discovery, SpecialDiscovery};
use crate::downloader::Downloader;
use crate::error::OrchestratorError;
use crate::models::{ProcessingMode, Source, SourceType};
use crate::pipeline::{ItemOutcome, LibraryItem, Pipeline, PipelineConfig, ProductionPhases};
use crate::progress::Progress;
use crate::scanner::Scanner;
use log::{debug, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

/// Summary statistics for processing run
#[derive(Debug, Clone, Default)]
pub struct ProcessingSummary {
    /// Total number of movies found
    pub total_movies: usize,
    /// Number of movies successfully processed
    pub successful_movies: usize,
    /// Number of movies that failed
    pub failed_movies: usize,
    /// Total number of series found
    pub total_series: usize,
    /// Number of series successfully processed
    pub successful_series: usize,
    /// Number of series that failed
    pub failed_series: usize,
    /// Total downloads across all items
    pub total_downloads: usize,
    /// Total conversions across all items
    pub total_conversions: usize,
    /// Per-source discovered video totals
    pub source_totals: std::collections::HashMap<SourceType, usize>,
    /// Total videos discovered across all sources
    pub total_videos_discovered: usize,
    /// Total duplicates removed by title+duration deduplication
    pub duplicates_removed: usize,
}

impl ProcessingSummary {
    /// Create a new empty summary
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge source results into the running per-source totals
    pub fn add_source_results(&mut self, results: &[crate::discovery::SourceResult]) {
        for sr in results {
            *self.source_totals.entry(sr.source).or_insert(0) += sr.videos_found;
            self.total_videos_discovered += sr.videos_found;
        }
    }

    /// Add the statistics of one processed library item
    pub(crate) fn add_outcome(&mut self, outcome: &ItemOutcome) {
        match &outcome.item {
            LibraryItem::Movie(_) => {
                if outcome.success {
                    self.successful_movies += 1;
                } else {
                    self.failed_movies += 1;
                }
            }
            LibraryItem::Series(_) => {
                if outcome.success {
                    self.successful_series += 1;
                } else {
                    self.failed_series += 1;
                }
            }
        }
        self.total_downloads += outcome.downloads;
        self.total_conversions += outcome.conversions;
        self.duplicates_removed += outcome.duplicates_removed;
        self.add_source_results(&outcome.source_results);
    }
}

/// Series-specific configuration options
pub struct SeriesConfig {
    /// Enable season-specific extras discovery for series
    pub season_extras: bool,
    /// Enable Season 0 specials discovery for series
    pub specials: bool,
    /// Folder name for Season 0 specials (e.g., "Specials", "Season 00")
    pub specials_folder: String,
}

impl Default for SeriesConfig {
    fn default() -> Self {
        Self {
            season_extras: false,
            specials: false,
            specials_folder: "Specials".to_string(),
        }
    }
}

/// Discovery and download configuration options
pub struct DiscoveryConfig {
    /// Discovery sources to query
    pub sources: Vec<Source>,
    /// Optional browser name for cookie-based authentication
    pub cookies_from_browser: Option<String>,
    /// Skip all downloads and file I/O when true
    pub dry_run: bool,
    /// Vimeo personal access token
    pub vimeo_access_token: String,
}

impl DiscoveryConfig {
    pub fn new(sources: Vec<Source>) -> Self {
        Self {
            sources,
            cookies_from_browser: None,
            dry_run: false,
            vimeo_access_token: String::new(),
        }
    }
}

/// Groups all parameters to avoid a long argument list.
pub struct OrchestratorConfig {
    /// Root directory containing media folders
    pub root_dir: PathBuf,
    /// TMDB API key for content discovery
    pub tmdb_api_key: String,
    /// Optional TVDB API key for Season 0 specials
    pub tvdb_api_key: Option<String>,
    /// Ignore done markers and reprocess everything
    pub force: bool,
    /// Maximum number of items processed in parallel
    pub concurrency: usize,
    /// Treat the root as a single library item
    pub single: bool,
    /// Which library item kinds to process
    pub processing_mode: ProcessingMode,
    /// Series-specific options
    pub series: SeriesConfig,
    /// Discovery and download options
    pub discovery: DiscoveryConfig,
}

/// Orchestrator coordinates scanning and the per-item pipeline
pub struct Orchestrator {
    scanner: Scanner,
    discovery: Arc<Discovery>,
    special_discovery: Arc<SpecialDiscovery>,
    downloader: Arc<Downloader>,
    converter: Arc<Converter>,
    pipeline_config: PipelineConfig,
    temp_base: PathBuf,
    concurrency: usize,
    processing_mode: ProcessingMode,
    progress: Arc<Progress>,
}

impl Orchestrator {
    /// Create a new Orchestrator from the given configuration
    pub fn new(config: OrchestratorConfig) -> Result<Self, OrchestratorError> {
        if !config.root_dir.exists() {
            return Err(OrchestratorError::Init(format!(
                "Root directory does not exist: {:?}",
                config.root_dir
            )));
        }

        if !config.root_dir.is_dir() {
            return Err(OrchestratorError::Init(format!(
                "Root path is not a directory: {:?}",
                config.root_dir
            )));
        }

        let temp_base = PathBuf::from("tmp_downloads");

        info!("Initializing Orchestrator");
        info!("  Root directory: {:?}", config.root_dir);
        info!("  Sources: {:?}", config.discovery.sources);
        info!("  Force: {}", config.force);
        info!("  Concurrency: {}", config.concurrency);
        info!("  Single folder mode: {}", config.single);
        info!("  Processing mode: {}", config.processing_mode);
        info!("  Season extras: {}", config.series.season_extras);
        info!("  Specials (Season 0): {}", config.series.specials);
        if config.series.specials {
            info!("  Specials folder: {}", config.series.specials_folder);
        }
        if let Some(ref browser) = config.discovery.cookies_from_browser {
            info!("  Cookie auth: {} browser", browser);
        }
        if config.discovery.dry_run {
            info!("  Dry run: enabled (discovery only)");
        }

        // Shared KinoCheck request counter — tracks API usage across the run
        let kinocheck_counter = Arc::new(AtomicU32::new(0));

        let discovery = Discovery::new(crate::discovery::DiscoveryConfig {
            tmdb_api_key: config.tmdb_api_key.clone(),
            vimeo_access_token: config.discovery.vimeo_access_token.clone(),
            cookies_from_browser: config.discovery.cookies_from_browser.clone(),
            kinocheck_request_count: kinocheck_counter,
        });

        if config.series.specials && config.tvdb_api_key.is_some() {
            info!("  TVDB support enabled for Season 0 specials");
        }
        let tvdb_api_key = if config.series.specials {
            config.tvdb_api_key
        } else {
            None
        };
        let specials_cache_dir = config.root_dir.join(".cache").join("tvdb_ids");
        let special_discovery = SpecialDiscovery::new(
            config.tmdb_api_key,
            tvdb_api_key,
            specials_cache_dir,
            config.discovery.cookies_from_browser.clone(),
        );

        // Build downloader — clones cookies_from_browser before the config is consumed
        let downloader = match config.discovery.cookies_from_browser {
            Some(browser) => Downloader::with_cookies(temp_base.clone(), browser),
            None => Downloader::new(temp_base.clone()),
        };

        let pipeline_config = PipelineConfig {
            dry_run: config.discovery.dry_run,
            sources: config.discovery.sources,
            season_extras: config.series.season_extras,
            specials: config.series.specials,
            specials_folder: config.series.specials_folder,
            temp_base: temp_base.clone(),
        };

        Ok(Self {
            scanner: Scanner::new(config.root_dir, config.force, config.single),
            discovery: Arc::new(discovery),
            special_discovery: Arc::new(special_discovery),
            downloader: Arc::new(downloader),
            converter: Arc::new(Converter::new()),
            pipeline_config,
            temp_base,
            concurrency: config.concurrency,
            processing_mode: config.processing_mode,
            progress: Arc::new(Progress::new()),
        })
    }

    /// Attach the progress instance built by the entry point.
    pub fn with_progress(mut self, progress: Arc<Progress>) -> Self {
        self.progress = progress;
        self
    }

    /// Run the orchestrator and process all movies and/or series
    pub async fn run(&self) -> Result<ProcessingSummary, OrchestratorError> {
        info!("Starting orchestrator run");

        self.cleanup_pre_existing_temp().await;

        info!("Phase 1: Scanning for media");
        self.progress.system_status("Scanning folder...");
        let (movies, series) = self
            .scanner
            .scan_all()
            .map_err(|e| OrchestratorError::Processing(format!("Scan failed: {}", e)))?;

        let movie_count = movies.len();
        let series_count = series.len();
        info!(
            "Found {} movies and {} series to process",
            movie_count, series_count
        );

        self.progress.run_started(movie_count + series_count);
        self.progress.system_status(&format!(
            "Found {} movies, {} series",
            movie_count, series_count
        ));

        let mut summary = ProcessingSummary::new();
        summary.total_movies = movie_count;
        summary.total_series = series_count;

        let phases = ProductionPhases::new(
            self.discovery.clone(),
            self.special_discovery.clone(),
            self.downloader.clone(),
            self.converter.clone(),
            self.pipeline_config.clone(),
        );
        let pipeline = Arc::new(Pipeline::new(
            phases,
            self.pipeline_config.clone(),
            self.progress.clone(),
            self.concurrency,
        ));

        if self.processing_mode != ProcessingMode::SeriesOnly && !movies.is_empty() {
            info!("Processing movies");
            self.progress
                .system_status(&format!("Processing {} movies...", movie_count));
            let items = movies.into_iter().map(LibraryItem::Movie).collect();
            let outcomes = pipeline.process_all(items).await;
            for outcome in outcomes {
                if let Some(ref error) = outcome.error {
                    log::error!("Movie {} failed: {}", outcome.item.label(), error);
                } else {
                    info!(
                        "Movie {} completed: {} downloads, {} conversions",
                        outcome.item.label(),
                        outcome.downloads,
                        outcome.conversions
                    );
                }
                summary.add_outcome(&outcome);
            }
        }

        if self.processing_mode != ProcessingMode::MoviesOnly && !series.is_empty() {
            info!("Processing series");
            self.progress
                .system_status(&format!("Processing {} series...", series.len()));
            let items = series.into_iter().map(LibraryItem::Series).collect();
            let outcomes = pipeline.process_all(items).await;
            for outcome in outcomes {
                if let Some(ref error) = outcome.error {
                    log::error!("Series {} failed: {}", outcome.item.label(), error);
                } else {
                    info!(
                        "Series {} completed: {} downloads, {} conversions",
                        outcome.item.label(),
                        outcome.downloads,
                        outcome.conversions
                    );
                }
                summary.add_outcome(&outcome);
            }
        }

        self.progress.system_status("Complete!");
        self.progress.run_finished();

        info!("Orchestrator run complete");
        info!("  Total movies: {}", summary.total_movies);
        info!("  Successful movies: {}", summary.successful_movies);
        info!("  Failed movies: {}", summary.failed_movies);
        info!("  Total series: {}", summary.total_series);
        info!("  Successful series: {}", summary.successful_series);
        info!("  Failed series: {}", summary.failed_series);
        info!("  Total downloads: {}", summary.total_downloads);
        info!("  Total conversions: {}", summary.total_conversions);

        Ok(summary)
    }

    /// Clean up any pre-existing temp directories before processing
    async fn cleanup_pre_existing_temp(&self) {
        if !self.temp_base.exists() {
            debug!("No pre-existing temp directory to clean up");
            return;
        }

        info!("Cleaning up pre-existing temp directories");
        match tokio::fs::remove_dir_all(&self.temp_base).await {
            Ok(_) => info!(
                "Cleaned up pre-existing temp directory: {:?}",
                self.temp_base
            ),
            Err(e) => warn!(
                "Failed to cleanup pre-existing temp directory {:?}: {}",
                self.temp_base, e
            ),
        }
    }
}

// Implement Drop to ensure temp directories are cleaned up on exit
impl Drop for Orchestrator {
    fn drop(&mut self) {
        if self.temp_base.exists() {
            debug!("Cleaning up temp directories on exit");
            if let Err(e) = std::fs::remove_dir_all(&self.temp_base) {
                warn!(
                    "Failed to cleanup temp directory on exit {:?}: {}",
                    self.temp_base, e
                );
            } else {
                debug!("Cleaned up temp directory on exit: {:?}", self.temp_base);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::SourceResult;
    use crate::models::{MovieEntry, SeriesEntry};
    use crate::progress::Outcome;
    use std::path::Path;

    fn test_config(root_dir: PathBuf) -> OrchestratorConfig {
        OrchestratorConfig {
            root_dir,
            tmdb_api_key: "fake_api_key".to_string(),
            tvdb_api_key: None,
            force: false,
            concurrency: 1,
            single: false,
            processing_mode: ProcessingMode::Both,
            series: SeriesConfig::default(),
            discovery: DiscoveryConfig::new(vec![Source::Youtube]),
        }
    }

    fn movie(dir: &Path) -> LibraryItem {
        LibraryItem::Movie(MovieEntry {
            path: dir.to_path_buf(),
            title: "Test Movie".to_string(),
            year: 2020,
        })
    }

    fn series(dir: &Path) -> LibraryItem {
        LibraryItem::Series(SeriesEntry {
            path: dir.to_path_buf(),
            title: "Test Series".to_string(),
            year: Some(2020),
            seasons: vec![1],
        })
    }

    fn outcome(
        item: LibraryItem,
        success: bool,
        downloads: usize,
        conversions: usize,
    ) -> ItemOutcome {
        ItemOutcome {
            item,
            success,
            downloads,
            conversions,
            placed: conversions,
            source_results: vec![SourceResult {
                source: SourceType::YouTube,
                videos_found: 3,
                error: None,
            }],
            duplicates_removed: 1,
            error: None,
        }
    }

    #[test]
    fn rejects_missing_root_directory() {
        let missing = std::env::temp_dir().join("definitely-not-a-real-library-dir");
        assert!(Orchestrator::new(test_config(missing)).is_err());
    }

    #[test]
    fn rejects_file_as_root() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(Orchestrator::new(test_config(file.path().to_path_buf())).is_err());
    }

    #[test]
    fn summary_folds_movie_and_series_outcomes() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut summary = ProcessingSummary::new();
        summary.add_outcome(&outcome(movie(dir.path()), true, 4, 3));
        summary.add_outcome(&outcome(series(dir.path()), false, 2, 0));

        assert_eq!(summary.successful_movies, 1);
        assert_eq!(summary.failed_movies, 0);
        assert_eq!(summary.successful_series, 0);
        assert_eq!(summary.failed_series, 1);
        assert_eq!(summary.total_downloads, 6);
        assert_eq!(summary.total_conversions, 3);
        assert_eq!(summary.total_videos_discovered, 6);
        assert_eq!(summary.duplicates_removed, 2);
        assert_eq!(summary.source_totals[&SourceType::YouTube], 6);
    }

    #[test]
    fn progress_outcome_mirrors_item_outcome() {
        let dir = tempfile::TempDir::new().unwrap();
        let outcome = outcome(movie(dir.path()), true, 4, 3);
        let progress: Outcome = outcome.progress_outcome();
        assert!(progress.success);
        assert_eq!(progress.downloads, 4);
        assert_eq!(progress.conversions, 3);
    }
}
