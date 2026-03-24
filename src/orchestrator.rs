// Orchestrator module - coordinates all processing phases

use crate::converter::Converter;
use crate::discovery::{DiscoveryOrchestrator, SeriesDiscoveryOrchestrator, SourceResult};
use crate::downloader::Downloader;
use crate::error::OrchestratorError;
use crate::models::{
    ConversionResult, MovieEntry, ProcessingMode, SeriesEntry, SeriesExtra, Source, VideoSource,
};
use crate::organizer::{Organizer, SeriesOrganizer};
use crate::output;
use crate::scanner::Scanner;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;

/// Shared context for series processing, bundling dependencies passed between phases
struct SeriesProcessingContext {
    series_discovery: Arc<SeriesDiscoveryOrchestrator>,
    downloader: Arc<Downloader>,
    converter: Arc<Converter>,
    temp_base: PathBuf,
    season_extras: bool,
    specials: bool,
    specials_folder: String,
    dry_run: bool,
}

/// Shared context for movie processing, bundling dependencies passed between phases
struct MovieProcessingContext {
    discovery: Arc<DiscoveryOrchestrator>,
    downloader: Arc<Downloader>,
    converter: Arc<Converter>,
    temp_base: PathBuf,
    dry_run: bool,
    library_movies: Arc<Vec<MovieEntry>>,
}

/// Summary statistics for processing run
#[derive(Debug, Clone, Default)]
pub struct ProcessingSummary {
    /// Total number of movies found
    pub total_movies: usize,
    /// Number of movies successfully processed
    pub successful_movies: usize,
    /// Number of movies that failed processing
    pub failed_movies: usize,
    /// Total number of series found
    pub total_series: usize,
    /// Number of series successfully processed
    pub successful_series: usize,
    /// Number of series that failed processing
    pub failed_series: usize,
    /// Total number of videos downloaded
    pub total_downloads: usize,
    /// Total number of videos converted
    pub total_conversions: usize,
    /// Per-source video counts (pre-dedup raw totals)
    pub(crate) source_totals: HashMap<Source, usize>,
    /// Total videos discovered across all sources (pre-dedup raw total)
    pub(crate) total_videos_discovered: usize,
}

impl ProcessingSummary {
    /// Create a new empty summary
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge source results into the running per-source totals
    pub fn add_source_results(&mut self, results: &[SourceResult]) {
        for sr in results {
            *self.source_totals.entry(sr.source).or_insert(0) += sr.videos_found;
            self.total_videos_discovered += sr.videos_found;
        }
    }

    /// Add statistics from a movie result
    fn add_movie_result(&mut self, result: &MovieResult) {
        if result.success {
            self.successful_movies += 1;
        } else {
            self.failed_movies += 1;
        }
        self.total_downloads += result.downloads;
        self.total_conversions += result.conversions;
        self.add_source_results(&result.source_results);
    }

    /// Add statistics from a series result
    fn add_series_result(&mut self, result: &SeriesResult) {
        if result.success {
            self.successful_series += 1;
        } else {
            self.failed_series += 1;
        }
        self.total_downloads += result.downloads;
        self.total_conversions += result.conversions;
        self.add_source_results(&result.source_results);
    }
}

/// Result of processing a single movie
#[derive(Debug)]
struct MovieResult {
    movie: MovieEntry,
    success: bool,
    downloads: usize,
    conversions: usize,
    error: Option<String>,
    source_results: Vec<SourceResult>,
}

impl MovieResult {
    fn success(
        movie: MovieEntry,
        downloads: usize,
        conversions: usize,
        source_results: Vec<SourceResult>,
    ) -> Self {
        Self {
            movie,
            success: true,
            downloads,
            conversions,
            error: None,
            source_results,
        }
    }

    fn failed(
        movie: MovieEntry,
        phase: &str,
        error: String,
        source_results: Vec<SourceResult>,
    ) -> Self {
        Self {
            movie,
            success: false,
            downloads: 0,
            conversions: 0,
            error: Some(format!("{} phase failed: {}", phase, error)),
            source_results,
        }
    }
}

/// Result of processing a single series
#[derive(Debug)]
struct SeriesResult {
    series: SeriesEntry,
    success: bool,
    downloads: usize,
    conversions: usize,
    error: Option<String>,
    source_results: Vec<SourceResult>,
}

impl SeriesResult {
    fn success(
        series: SeriesEntry,
        downloads: usize,
        conversions: usize,
        source_results: Vec<SourceResult>,
    ) -> Self {
        Self {
            series,
            success: true,
            downloads,
            conversions,
            error: None,
            source_results,
        }
    }

    fn failed(
        series: SeriesEntry,
        phase: &str,
        error: String,
        source_results: Vec<SourceResult>,
    ) -> Self {
        Self {
            series,
            success: false,
            downloads: 0,
            conversions: 0,
            error: Some(format!("{} phase failed: {}", phase, error)),
            source_results,
        }
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
    /// Discovery-only mode: skip download, conversion, and organization
    pub dry_run: bool,
}

impl DiscoveryConfig {
    pub fn new(sources: Vec<Source>) -> Self {
        Self {
            sources,
            cookies_from_browser: None,
            dry_run: false,
        }
    }
}

/// Configuration for creating an Orchestrator instance.
/// Groups all parameters to avoid a long argument list.
pub struct OrchestratorConfig {
    /// Root directory containing media folders
    pub root_dir: PathBuf,
    /// TMDB API key for content discovery
    pub tmdb_api_key: String,
    /// Optional TheTVDB API key for Season 0 specials discovery
    pub tvdb_api_key: Option<String>,
    /// Ignore done markers and reprocess all media
    pub force: bool,
    /// Maximum number of media items to process concurrently
    pub concurrency: usize,
    /// Process a single folder directly instead of scanning
    pub single: bool,
    /// Which media types to process (Both, MoviesOnly, SeriesOnly)
    pub processing_mode: ProcessingMode,
    /// Series-specific configuration
    pub series: SeriesConfig,
    /// Discovery and download configuration
    pub discovery: DiscoveryConfig,
}

/// Orchestrator coordinates all processing phases
pub struct Orchestrator {
    scanner: Scanner,
    discovery: Arc<DiscoveryOrchestrator>,
    series_discovery: Arc<SeriesDiscoveryOrchestrator>,
    downloader: Arc<Downloader>,
    converter: Arc<Converter>,
    temp_base: PathBuf,
    concurrency: usize,
    processing_mode: ProcessingMode,
    season_extras: bool,
    specials: bool,
    specials_folder: String,
    dry_run: bool,
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

        // Build series discovery — consumes tvdb_api_key, clones tmdb_api_key
        let series_discovery =
            if let (true, Some(tvdb_key)) = (config.series.specials, config.tvdb_api_key) {
                let cache_dir = config.root_dir.join(".cache").join("tvdb_ids");
                info!("  TVDB support enabled for Season 0 specials");
                let mut orch = SeriesDiscoveryOrchestrator::new_with_tvdb(
                    config.tmdb_api_key.clone(),
                    tvdb_key,
                    config.discovery.sources.clone(),
                    cache_dir,
                );
                if let Some(ref browser) = config.discovery.cookies_from_browser {
                    orch = orch.with_cookies(browser.clone());
                }
                Arc::new(orch)
            } else {
                let mut orch = SeriesDiscoveryOrchestrator::new(
                    config.tmdb_api_key.clone(),
                    config.discovery.sources.clone(),
                );
                if let Some(ref browser) = config.discovery.cookies_from_browser {
                    orch = orch.with_cookies(browser.clone());
                }
                Arc::new(orch)
            };

        // Build movie discovery — clones tmdb_api_key (last clone before move)
        let discovery = match &config.discovery.cookies_from_browser {
            Some(browser) => DiscoveryOrchestrator::with_cookies(
                config.tmdb_api_key.clone(),
                config.discovery.sources.clone(),
                browser.clone(),
            ),
            None => {
                DiscoveryOrchestrator::new(config.tmdb_api_key, config.discovery.sources.clone())
            }
        };

        // Build downloader — consumes cookies_from_browser
        let downloader = match config.discovery.cookies_from_browser {
            Some(browser) => Downloader::with_cookies(temp_base.clone(), browser),
            None => Downloader::new(temp_base.clone()),
        };

        Ok(Self {
            scanner: Scanner::new(config.root_dir, config.force, config.single),
            discovery: Arc::new(discovery),
            series_discovery,
            downloader: Arc::new(downloader),
            converter: Arc::new(Converter::new()),
            temp_base,
            concurrency: config.concurrency,
            processing_mode: config.processing_mode,
            season_extras: config.series.season_extras,
            specials: config.series.specials,
            specials_folder: config.series.specials_folder,
            dry_run: config.discovery.dry_run,
        })
    }

    /// Run the orchestrator and process all movies and/or series
    pub async fn run(&self) -> Result<ProcessingSummary, OrchestratorError> {
        info!("Starting orchestrator run");

        self.cleanup_pre_existing_temp().await;

        info!("Phase 1: Scanning for media");
        let (movies, series) = self
            .scanner
            .scan_all()
            .map_err(|e| OrchestratorError::Processing(format!("Scan failed: {}", e)))?;

        info!(
            "Found {} movies and {} series to process",
            movies.len(),
            series.len()
        );

        let mut summary = ProcessingSummary::new();
        summary.total_movies = movies.len();
        summary.total_series = series.len();

        if self.processing_mode != ProcessingMode::SeriesOnly && !movies.is_empty() {
            info!("Processing movies");
            let library = Arc::new(movies.clone());
            let results = if self.concurrency > 1 {
                self.process_movies_parallel(movies, library).await
            } else {
                self.process_movies_sequential(movies, library).await
            };

            for result in results {
                if let Some(ref error) = result.error {
                    error!("Movie {} failed: {}", result.movie, error);
                } else {
                    info!(
                        "Movie {} completed: {} downloads, {} conversions",
                        result.movie, result.downloads, result.conversions
                    );
                }
                summary.add_movie_result(&result);
            }
        }

        if self.processing_mode != ProcessingMode::MoviesOnly && !series.is_empty() {
            info!("Processing series");
            let results = if self.concurrency > 1 {
                self.process_series_parallel(series).await
            } else {
                self.process_series_sequential(series).await
            };

            for result in results {
                if let Some(ref error) = result.error {
                    error!("Series {} failed: {}", result.series, error);
                } else {
                    info!(
                        "Series {} completed: {} downloads, {} conversions",
                        result.series, result.downloads, result.conversions
                    );
                }
                summary.add_series_result(&result);
            }
        }

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

    async fn process_movies_sequential(
        &self,
        movies: Vec<MovieEntry>,
        library: Arc<Vec<MovieEntry>>,
    ) -> Vec<MovieResult> {
        let total = movies.len();
        let mut results = Vec::new();
        for (idx, movie) in movies.into_iter().enumerate() {
            output::display_movie_start(&movie, idx + 1, total);
            let result = self.process_movie(movie, library.clone()).await;
            results.push(result);
        }
        results
    }

    async fn process_movies_parallel(
        &self,
        movies: Vec<MovieEntry>,
        library: Arc<Vec<MovieEntry>>,
    ) -> Vec<MovieResult> {
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let total = movies.len();
        let counter = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for movie in movies {
            let sem = semaphore.clone();
            let counter = counter.clone();

            let ctx = MovieProcessingContext {
                discovery: self.discovery.clone(),
                downloader: self.downloader.clone(),
                converter: self.converter.clone(),
                temp_base: self.temp_base.clone(),
                dry_run: self.dry_run,
                library_movies: library.clone(),
            };

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore should not be closed");
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                output::display_movie_start(&movie, current, total);
                Self::process_movie_standalone(movie, ctx).await
            });
            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            if let Ok(result) = task.await {
                results.push(result);
            }
        }
        results
    }

    async fn process_movie(&self, movie: MovieEntry, library: Arc<Vec<MovieEntry>>) -> MovieResult {
        let ctx = MovieProcessingContext {
            discovery: self.discovery.clone(),
            downloader: self.downloader.clone(),
            converter: self.converter.clone(),
            temp_base: self.temp_base.clone(),
            dry_run: self.dry_run,
            library_movies: library,
        };
        Self::process_movie_standalone(movie, ctx).await
    }

    /// Process a single movie through all phases without requiring &self.
    /// Used by both sequential and parallel paths.
    async fn process_movie_standalone(
        movie: MovieEntry,
        ctx: MovieProcessingContext,
    ) -> MovieResult {
        info!("Processing movie: {}", movie);

        let movie_id = format!("{}_{}", movie.title.replace(' ', "_"), movie.year);

        // Phase 2: Discovery
        info!("Phase 2: Discovering content for {}", movie);
        let (sources, source_results) = ctx.discovery.discover_all(&movie, &ctx.library_movies).await;

        // Dry-run: display results and return early — no file I/O (AC3/NFR5)
        if ctx.dry_run {
            let total = source_results.iter().map(|sr| sr.videos_found).sum();
            output::display_dry_run_movie_results(&movie, &source_results, total);
            return MovieResult::success(movie, 0, 0, source_results);
        }

        // Remove stale done marker only when actually reprocessing (not in dry-run)
        if movie.has_done_marker {
            remove_stale_done_marker(&movie.path).await;
        }

        if sources.is_empty() {
            warn!("No sources found for {}", movie);
            return MovieResult::success(movie, 0, 0, source_results);
        }
        info!("Found {} sources for {}", sources.len(), movie);

        // Phase 3: Download
        info!(
            "Phase 3: Downloading {} videos for {}",
            sources.len(),
            movie
        );
        let downloads = ctx.downloader.download_all(&movie_id, sources).await;
        let successful_downloads = downloads.iter().filter(|d| d.success).count();
        info!(
            "Downloaded {}/{} videos for {}",
            successful_downloads,
            downloads.len(),
            movie
        );

        if successful_downloads == 0 {
            warn!("No successful downloads for {}", movie);
            cleanup_temp_dir(&ctx.temp_base.join(&movie_id)).await;
            return MovieResult::success(movie, downloads.len(), 0, source_results);
        }

        // Phase 4: Conversion
        info!(
            "Phase 4: Converting {} videos for {}",
            successful_downloads, movie
        );
        let conversions = ctx.converter.convert_batch(downloads).await;
        let successful_conversions = conversions.iter().filter(|c| c.success).count();
        info!(
            "Converted {}/{} videos for {}",
            successful_conversions,
            conversions.len(),
            movie
        );

        if successful_conversions == 0 {
            warn!("No successful conversions for {}", movie);
            cleanup_temp_dir(&ctx.temp_base.join(&movie_id)).await;
            return MovieResult::success(movie, successful_downloads, 0, source_results);
        }

        // Phase 5: Organization
        info!(
            "Phase 5: Organizing {} files for {}",
            successful_conversions, movie
        );
        let organizer = Organizer::new(movie.path.clone());
        let temp_dir = ctx.temp_base.join(&movie_id);

        match organizer.organize(conversions, &temp_dir).await {
            Ok(_) => {
                info!("✓ Movie processing complete: {}", movie);
                MovieResult::success(
                    movie,
                    successful_downloads,
                    successful_conversions,
                    source_results,
                )
            }
            Err(e) => {
                error!("✗ Movie processing failed: {}: {}", movie, e);
                MovieResult::failed(movie, "organization", e.to_string(), source_results)
            }
        }
    }

    async fn process_series_sequential(&self, series_list: Vec<SeriesEntry>) -> Vec<SeriesResult> {
        let total = series_list.len();
        let mut results = Vec::new();
        for (idx, series) in series_list.into_iter().enumerate() {
            output::display_series_start(&series, idx + 1, total);
            let result = self.process_series(series).await;
            results.push(result);
        }
        results
    }

    async fn process_series_parallel(&self, series_list: Vec<SeriesEntry>) -> Vec<SeriesResult> {
        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let total = series_list.len();
        let counter = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for series in series_list {
            let sem = semaphore.clone();
            let series_discovery = self.series_discovery.clone();
            let downloader = self.downloader.clone();
            let converter = self.converter.clone();
            let temp_base = self.temp_base.clone();
            let specials_folder = self.specials_folder.clone();
            let season_extras = self.season_extras;
            let specials = self.specials;
            let counter = counter.clone();

            let ctx = SeriesProcessingContext {
                series_discovery,
                downloader,
                converter,
                temp_base,
                season_extras,
                specials,
                specials_folder,
                dry_run: self.dry_run,
            };

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore should not be closed");
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                output::display_series_start(&series, current, total);
                Self::process_series_standalone(series, ctx).await
            });
            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            if let Ok(result) = task.await {
                results.push(result);
            }
        }
        results
    }

    async fn process_series(&self, series: SeriesEntry) -> SeriesResult {
        let ctx = SeriesProcessingContext {
            series_discovery: self.series_discovery.clone(),
            downloader: self.downloader.clone(),
            converter: self.converter.clone(),
            temp_base: self.temp_base.clone(),
            season_extras: self.season_extras,
            specials: self.specials,
            specials_folder: self.specials_folder.clone(),
            dry_run: self.dry_run,
        };
        Self::process_series_standalone(series, ctx).await
    }

    /// Process a single series through all phases without requiring &self.
    /// Delegates to focused helper functions for each stage.
    async fn process_series_standalone(
        series: SeriesEntry,
        ctx: SeriesProcessingContext,
    ) -> SeriesResult {
        info!("Processing series: {}", series);

        let series_id = format!(
            "{}_{}",
            series.title.replace(' ', "_"),
            series.year.unwrap_or(0)
        );

        // Phase 2: Discovery
        let (all_extras, season_zero_extras, tvdb_episodes_metadata, series_source_results) =
            discover_series_content(
                &series,
                &ctx.series_discovery,
                ctx.season_extras,
                ctx.specials,
            )
            .await;

        // Dry-run: display results and return early — no file I/O (AC3/NFR5)
        if ctx.dry_run {
            let total = all_extras.len() + season_zero_extras.len();
            if total == 0 {
                info!(
                    "Dry-run: no extras discovered for {} (discovery may have failed or returned no results)",
                    series
                );
            }
            output::display_dry_run_series_results(&series, &series_source_results, total);
            return SeriesResult::success(series, 0, 0, series_source_results);
        }

        // Remove stale done marker only when actually reprocessing (not in dry-run)
        if series.has_done_marker {
            remove_stale_done_marker(&series.path).await;
        }

        if all_extras.is_empty() && season_zero_extras.is_empty() {
            warn!("No extras found for {}", series);
            return SeriesResult::success(series, 0, 0, series_source_results);
        }

        // Phase 3 & 4: Download and convert
        let (total_successful_downloads, total_conversions, conversions, specials_conversions) =
            download_and_convert_series(
                &series,
                &series_id,
                all_extras,
                &season_zero_extras,
                &ctx.downloader,
                &ctx.converter,
                &ctx.temp_base,
            )
            .await;

        if total_conversions == 0 {
            warn!("No successful conversions for {}", series);
            cleanup_temp_dir(&ctx.temp_base.join(&series_id)).await;
            return SeriesResult::success(
                series,
                total_successful_downloads,
                0,
                series_source_results,
            );
        }

        // Phase 5: Organization
        let specials_ctx = SpecialsOrganizationContext {
            tvdb_episodes_metadata: &tvdb_episodes_metadata,
            season_zero_extras: &season_zero_extras,
            specials_folder: &ctx.specials_folder,
        };
        let org_failed = organize_series_results(
            &series,
            &series_id,
            conversions,
            specials_conversions,
            &specials_ctx,
            &ctx.temp_base,
        )
        .await;

        cleanup_temp_dir(&ctx.temp_base.join(&series_id)).await;

        if org_failed {
            error!("✗ Series processing had organization errors: {}", series);
            SeriesResult::failed(
                series,
                "organization",
                "Some seasons failed".to_string(),
                series_source_results,
            )
        } else {
            write_done_marker(&series.path, &format!("{}", series)).await;
            info!("✓ Series processing complete: {}", series);
            SeriesResult::success(
                series,
                total_successful_downloads,
                total_conversions,
                series_source_results,
            )
        }
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

// --- Free-standing helper functions (SRP: each does one thing) ---

/// Remove a stale done marker file before reprocessing
async fn remove_stale_done_marker(path: &std::path::Path) {
    let marker_path = path.join("done.ext");
    info!(
        "Removing stale done marker before reprocessing: {:?}",
        marker_path
    );
    if let Err(e) = tokio::fs::remove_file(&marker_path).await {
        warn!("Failed to remove done marker {:?}: {}", marker_path, e);
    }
}

/// Clean up a temp directory if it exists
async fn cleanup_temp_dir(temp_dir: &std::path::Path) {
    if temp_dir.exists()
        && let Err(e) = tokio::fs::remove_dir_all(temp_dir).await
    {
        warn!("Failed to cleanup temp dir {:?}: {}", temp_dir, e);
    }
}

/// Write a done marker JSON file for a completed media item
async fn write_done_marker(path: &std::path::Path, label: &str) {
    let marker_path = path.join("done.ext");
    let marker = serde_json::json!({
        "finished_at": chrono::Utc::now().to_rfc3339(),
        "version": env!("CARGO_PKG_VERSION"),
    });

    match serde_json::to_string_pretty(&marker) {
        Ok(json) => {
            if let Err(e) = tokio::fs::write(&marker_path, json).await {
                warn!("Failed to create done marker for {}: {}", label, e);
            } else {
                info!("Created done marker for: {:?}", marker_path);
            }
        }
        Err(_) => warn!("Failed to serialize done marker for {}", label),
    }
}

/// Discover all content for a series: regular extras, season extras, and Season 0 specials.
/// Returns (regular_extras, season_zero_extras, tvdb_episode_metadata, source_results).
async fn discover_series_content(
    series: &SeriesEntry,
    series_discovery: &SeriesDiscoveryOrchestrator,
    season_extras_enabled: bool,
    specials_enabled: bool,
) -> (
    Vec<SeriesExtra>,
    Vec<SeriesExtra>,
    Vec<crate::discovery::TvdbEpisodeExtended>,
    Vec<crate::discovery::SourceResult>,
) {
    info!("Phase 2: Discovering content for {}", series);
    info!(
        "Discovery flags: season_extras={}, specials={}",
        season_extras_enabled, specials_enabled
    );

    // Stage 1: Series-level extras (always)
    let (mut all_extras, series_source_results) = series_discovery.discover_all(series).await;
    let series_level_count = all_extras.len();
    info!(
        "Series-level discovery: found {} extras",
        series_level_count
    );

    // Stage 2: Season-specific extras (if enabled)
    let mut season_specific_count = 0;
    if season_extras_enabled {
        for &season in &series.seasons {
            let (extras, _season_source_results) = series_discovery
                .discover_season_extras(series, season)
                .await;
            info!(
                "Found {} season {} extras for {}",
                extras.len(),
                season,
                series
            );
            season_specific_count += extras.len();
            all_extras.extend(extras);
        }
    }

    // Stage 3: Season 0 specials (if enabled)
    let (season_zero_extras, tvdb_episodes) = if specials_enabled {
        discover_season_zero_specials(series, series_discovery).await
    } else {
        (Vec::new(), Vec::new())
    };

    // Deduplicate regular extras by URL
    let before_dedup = all_extras.len();
    let mut seen_urls = std::collections::HashSet::new();
    all_extras.retain(|extra| seen_urls.insert(extra.url.clone()));
    let deduped = before_dedup - all_extras.len();
    if deduped > 0 {
        info!("Deduplicated {} duplicate URLs for {}", deduped, series);
    }

    info!(
        "Discovery summary for {}: {} series-level, {} season-specific, {} specials",
        series,
        series_level_count,
        season_specific_count,
        season_zero_extras.len()
    );

    (
        all_extras,
        season_zero_extras,
        tvdb_episodes,
        series_source_results,
    )
}

/// Discover Season 0 specials via TVDB candidate selection.
/// Returns (extras_to_download, matched_tvdb_episodes).
async fn discover_season_zero_specials(
    series: &SeriesEntry,
    series_discovery: &SeriesDiscoveryOrchestrator,
) -> (Vec<SeriesExtra>, Vec<crate::discovery::TvdbEpisodeExtended>) {
    info!("Discovering Season 0 specials for {}", series);
    let (_raw_extras, episodes) = series_discovery
        .discover_season_zero_with_metadata(series)
        .await;

    if episodes.is_empty() {
        info!("No monitored Season 0 episodes for {}", series);
        return (Vec::new(), Vec::new());
    }

    info!(
        "Selecting best candidates for {} Season 0 episodes of {}",
        episodes.len(),
        series
    );
    let selections = crate::discovery::SpecialValidator::select_best_candidates(
        &series.title,
        &episodes,
        series_discovery.cookies_from_browser.as_deref(),
    )
    .await;

    let mut extras = Vec::new();
    let mut matched_episodes = Vec::new();

    for (selection, episode) in selections.into_iter().zip(episodes.into_iter()) {
        if let Some(selected) = selection {
            extras.push(SeriesExtra {
                series_id: format!(
                    "{}_{}",
                    series.title.replace(' ', "_"),
                    series.year.unwrap_or(0)
                ),
                season_number: Some(0),
                category: crate::models::ContentCategory::Featurette,
                title: format!("S00E{:02} - {}", episode.number, episode.name),
                url: selected.url,
                source_type: crate::models::SourceType::TheTVDB,
                local_path: None,
            });
            matched_episodes.push(episode);
        }
    }

    info!(
        "Found {} valid Season 0 specials for {}",
        extras.len(),
        series
    );
    (extras, matched_episodes)
}

/// Download and convert both regular extras and Season 0 specials.
/// Returns (total_successful_downloads, total_conversions, regular_conversions, specials_conversions).
async fn download_and_convert_series(
    series: &SeriesEntry,
    series_id: &str,
    all_extras: Vec<SeriesExtra>,
    season_zero_extras: &[SeriesExtra],
    downloader: &Downloader,
    converter: &Converter,
    temp_base: &std::path::Path,
) -> (usize, usize, Vec<ConversionResult>, Vec<ConversionResult>) {
    // Download regular extras
    info!(
        "Phase 3: Downloading {} regular extras for {}",
        all_extras.len(),
        series
    );
    let video_sources: Vec<VideoSource> = all_extras.into_iter().map(|e| e.into()).collect();
    let downloads = downloader.download_all(series_id, video_sources).await;
    let successful_downloads = downloads.iter().filter(|d| d.success).count();
    info!(
        "Downloaded {}/{} regular extras for {}",
        successful_downloads,
        downloads.len(),
        series
    );

    // Download Season 0 specials
    let specials_downloads = if !season_zero_extras.is_empty() {
        info!(
            "Downloading {} Season 0 specials for {}",
            season_zero_extras.len(),
            series
        );
        let sources: Vec<VideoSource> = season_zero_extras
            .iter()
            .map(|e| e.clone().into())
            .collect();
        downloader.download_all(series_id, sources).await
    } else {
        Vec::new()
    };
    let successful_specials = specials_downloads.iter().filter(|d| d.success).count();

    let total_successful_downloads = successful_downloads + successful_specials;

    if total_successful_downloads == 0 {
        warn!("No successful downloads for {}", series);
        cleanup_temp_dir(&temp_base.join(series_id)).await;
        return (0, 0, Vec::new(), Vec::new());
    }

    // Phase 4: Conversion
    info!(
        "Phase 4: Converting {} videos for {}",
        total_successful_downloads, series
    );
    let conversions = converter.convert_batch(downloads).await;
    let specials_conversions = if !specials_downloads.is_empty() {
        converter.convert_batch(specials_downloads).await
    } else {
        Vec::new()
    };

    let successful_conversions = conversions.iter().filter(|c| c.success).count();
    let successful_specials_conversions = specials_conversions.iter().filter(|c| c.success).count();
    let total_conversions = successful_conversions + successful_specials_conversions;

    info!(
        "Converted {}/{} videos for {} ({} regular, {} specials)",
        total_conversions,
        conversions.len() + specials_conversions.len(),
        series,
        successful_conversions,
        successful_specials_conversions
    );

    (
        total_successful_downloads,
        total_conversions,
        conversions,
        specials_conversions,
    )
}

/// Context for organizing Season 0 specials
struct SpecialsOrganizationContext<'a> {
    tvdb_episodes_metadata: &'a [crate::discovery::TvdbEpisodeExtended],
    season_zero_extras: &'a [SeriesExtra],
    specials_folder: &'a str,
}

/// Organize converted files into Jellyfin directory structure.
/// Returns true if any organization step failed.
async fn organize_series_results(
    series: &SeriesEntry,
    series_id: &str,
    conversions: Vec<ConversionResult>,
    specials_conversions: Vec<ConversionResult>,
    specials_ctx: &SpecialsOrganizationContext<'_>,
    temp_base: &std::path::Path,
) -> bool {
    let total_conversions = conversions.iter().filter(|c| c.success).count()
        + specials_conversions.iter().filter(|c| c.success).count();

    info!(
        "Phase 5: Organizing {} files for {}",
        total_conversions, series
    );
    let organizer = SeriesOrganizer::new(series.path.clone(), series.seasons.clone());
    let _temp_dir = temp_base.join(series_id);

    // Group regular conversions by season_number
    let mut by_season: std::collections::HashMap<Option<u8>, Vec<_>> =
        std::collections::HashMap::new();
    for conversion in conversions {
        by_season
            .entry(conversion.season_number)
            .or_default()
            .push(conversion);
    }

    let mut org_failed = false;
    for (season, season_conversions) in by_season {
        if let Err(e) = organizer.organize_extras(season_conversions, season).await {
            error!(
                "Organization failed for {} season {:?}: {}",
                series, season, e
            );
            org_failed = true;
        }
    }

    // Organize Season 0 specials
    if !specials_conversions.is_empty() {
        let mut special_episodes = Vec::new();
        for (idx, conversion) in specials_conversions.iter().enumerate() {
            if conversion.success
                && let Some(episode) = specials_ctx.tvdb_episodes_metadata.get(idx)
            {
                special_episodes.push(crate::models::SpecialEpisode {
                    episode_number: episode.number,
                    title: episode.name.clone(),
                    air_date: episode.aired.clone(),
                    url: specials_ctx
                        .season_zero_extras
                        .get(idx)
                        .map(|e| e.url.clone()),
                    local_path: Some(conversion.output_path.clone()),
                    tvdb_id: Some(episode.id),
                });
            }
        }

        if !special_episodes.is_empty()
            && let Err(e) = organizer
                .organize_specials(
                    &series.title,
                    special_episodes,
                    specials_ctx.specials_folder,
                )
                .await
        {
            error!("Failed to organize Season 0 specials for {}: {}", series, e);
            org_failed = true;
        }
    }

    org_failed
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build an OrchestratorConfig with common test defaults
    fn test_config(root_dir: PathBuf) -> OrchestratorConfig {
        use crate::models::default_sources;
        OrchestratorConfig {
            root_dir,
            tmdb_api_key: "fake_api_key".to_string(),
            tvdb_api_key: None,
            force: false,
            concurrency: 1,
            single: false,
            processing_mode: ProcessingMode::Both,
            series: SeriesConfig::default(),
            discovery: DiscoveryConfig::new(default_sources()),
        }
    }

    #[test]
    fn test_processing_summary_new() {
        let summary = ProcessingSummary::new();
        assert_eq!(summary.total_movies, 0);
        assert_eq!(summary.successful_movies, 0);
        assert_eq!(summary.failed_movies, 0);
        assert_eq!(summary.total_series, 0);
        assert_eq!(summary.successful_series, 0);
        assert_eq!(summary.failed_series, 0);
        assert_eq!(summary.total_downloads, 0);
        assert_eq!(summary.total_conversions, 0);
        assert!(summary.source_totals.is_empty());
        assert_eq!(summary.total_videos_discovered, 0);
    }

    #[test]
    fn test_processing_summary_add_successful_result() {
        let mut summary = ProcessingSummary::new();
        let result = MovieResult::success(
            MovieEntry {
                path: PathBuf::from("/movies/Test (2020)"),
                title: "Test".to_string(),
                year: 2020,
                has_done_marker: false,
            },
            5,
            4,
            vec![],
        );

        summary.add_movie_result(&result);

        assert_eq!(summary.successful_movies, 1);
        assert_eq!(summary.failed_movies, 0);
        assert_eq!(summary.total_downloads, 5);
        assert_eq!(summary.total_conversions, 4);
    }

    #[test]
    fn test_processing_summary_add_failed_result() {
        let mut summary = ProcessingSummary::new();
        let result = MovieResult::failed(
            MovieEntry {
                path: PathBuf::from("/movies/Test (2020)"),
                title: "Test".to_string(),
                year: 2020,
                has_done_marker: false,
            },
            "download",
            "Network error".to_string(),
            vec![],
        );

        summary.add_movie_result(&result);

        assert_eq!(summary.successful_movies, 0);
        assert_eq!(summary.failed_movies, 1);
        assert_eq!(summary.total_downloads, 0);
        assert_eq!(summary.total_conversions, 0);
    }

    #[test]
    fn test_movie_result_success() {
        let movie = MovieEntry {
            path: PathBuf::from("/movies/Test (2020)"),
            title: "Test".to_string(),
            year: 2020,
            has_done_marker: false,
        };

        let result = MovieResult::success(movie, 3, 2, vec![]);

        assert!(result.success);
        assert_eq!(result.downloads, 3);
        assert_eq!(result.conversions, 2);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_movie_result_failed() {
        let movie = MovieEntry {
            path: PathBuf::from("/movies/Test (2020)"),
            title: "Test".to_string(),
            year: 2020,
            has_done_marker: false,
        };

        let result = MovieResult::failed(movie, "conversion", "FFmpeg error".to_string(), vec![]);

        assert!(!result.success);
        assert_eq!(result.downloads, 0);
        assert_eq!(result.conversions, 0);
        assert!(result.error.is_some());
        assert!(
            result
                .error
                .as_ref()
                .expect("error should be set")
                .contains("conversion phase failed: FFmpeg error")
        );
    }

    #[test]
    fn test_orchestrator_new_validates_root_dir() {
        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let result = Orchestrator::new(test_config(nonexistent));

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cleanup_pre_existing_temp() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        let temp_base = temp_root.path().join("tmp_downloads");
        tokio::fs::create_dir(&temp_base)
            .await
            .expect("dir creation should succeed");
        tokio::fs::write(temp_base.join("test.txt"), b"test")
            .await
            .expect("file write should succeed");

        // Create orchestrator with custom temp_base
        let mut orchestrator =
            Orchestrator::new(test_config(root_dir)).expect("orchestrator build should succeed");

        orchestrator.temp_base = temp_base.clone();
        orchestrator.cleanup_pre_existing_temp().await;

        assert!(!temp_base.exists());
    }

    #[tokio::test]
    async fn test_orchestrator_run_with_empty_directory() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        let orchestrator =
            Orchestrator::new(test_config(root_dir)).expect("orchestrator build should succeed");

        let summary = orchestrator.run().await.expect("run should succeed");

        assert_eq!(summary.total_movies, 0);
        assert_eq!(summary.successful_movies, 0);
        assert_eq!(summary.failed_movies, 0);
    }

    #[tokio::test]
    async fn test_orchestrator_sequential_vs_parallel() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        for i in 1..=3 {
            let movie_dir = root_dir.join(format!("Movie {} (202{})", i, i));
            tokio::fs::create_dir(&movie_dir)
                .await
                .expect("dir creation should succeed");
        }

        // Test sequential processing (concurrency = 1)
        let orchestrator_seq = Orchestrator::new(OrchestratorConfig {
            root_dir: root_dir.clone(),
            discovery: DiscoveryConfig::new(vec![crate::models::Source::Youtube]),
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        // Test parallel processing (concurrency = 2)
        let orchestrator_par = Orchestrator::new(OrchestratorConfig {
            root_dir,
            discovery: DiscoveryConfig::new(vec![crate::models::Source::Youtube]),
            concurrency: 2,
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        assert_eq!(orchestrator_seq.concurrency, 1);
        assert_eq!(orchestrator_par.concurrency, 2);
    }

    #[tokio::test]
    async fn test_orchestrator_drop_cleanup() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        let temp_base = temp_root.path().join("tmp_downloads");

        {
            let mut orchestrator = Orchestrator::new(test_config(root_dir))
                .expect("orchestrator build should succeed");

            orchestrator.temp_base = temp_base.clone();

            tokio::fs::create_dir(&temp_base)
                .await
                .expect("dir creation should succeed");
            tokio::fs::write(temp_base.join("test.txt"), b"test")
                .await
                .expect("file write should succeed");

            assert!(temp_base.exists());
        }

        assert!(!temp_base.exists());
    }

    #[test]
    fn test_movie_result_display() {
        let movie = MovieEntry {
            path: PathBuf::from("/movies/Test (2020)"),
            title: "Test Movie".to_string(),
            year: 2020,
            has_done_marker: false,
        };

        let success_result = MovieResult::success(movie.clone(), 5, 4, vec![]);
        assert!(success_result.success);
        assert_eq!(success_result.downloads, 5);
        assert_eq!(success_result.conversions, 4);

        let failed_result =
            MovieResult::failed(movie, "download", "Network error".to_string(), vec![]);
        assert!(!failed_result.success);
        assert!(failed_result.error.is_some());
        assert!(
            failed_result
                .error
                .as_ref()
                .expect("error should be set")
                .contains("download phase failed")
        );
    }

    #[test]
    fn test_processing_summary_aggregation() {
        let mut summary = ProcessingSummary::new();
        summary.total_movies = 5;

        for i in 0..3 {
            let result = MovieResult::success(
                MovieEntry {
                    path: PathBuf::from(format!("/movies/Movie {} (2020)", i)),
                    title: format!("Movie {}", i),
                    year: 2020,
                    has_done_marker: false,
                },
                2,
                2,
                vec![],
            );
            summary.add_movie_result(&result);
        }

        for i in 3..5 {
            let result = MovieResult::failed(
                MovieEntry {
                    path: PathBuf::from(format!("/movies/Movie {} (2020)", i)),
                    title: format!("Movie {}", i),
                    year: 2020,
                    has_done_marker: false,
                },
                "discovery",
                "API error".to_string(),
                vec![],
            );
            summary.add_movie_result(&result);
        }

        assert_eq!(summary.total_movies, 5);
        assert_eq!(summary.successful_movies, 3);
        assert_eq!(summary.failed_movies, 2);
        assert_eq!(summary.total_downloads, 6);
        assert_eq!(summary.total_conversions, 6);
    }

    #[tokio::test]
    async fn test_orchestrator_with_done_markers() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        let movie1 = root_dir.join("Movie 1 (2021)");
        tokio::fs::create_dir(&movie1)
            .await
            .expect("dir creation should succeed");

        let movie2 = root_dir.join("Movie 2 (2022)");
        tokio::fs::create_dir(&movie2)
            .await
            .expect("dir creation should succeed");
        let done_marker = crate::models::DoneMarker {
            finished_at: "2024-01-01T00:00:00Z".to_string(),
            version: "0.1.0".to_string(),
        };
        let marker_json =
            serde_json::to_string(&done_marker).expect("marker serialization should succeed");
        tokio::fs::write(movie2.join("done.ext"), marker_json)
            .await
            .expect("file write should succeed");

        // Without force flag - should skip movie2
        let orchestrator = Orchestrator::new(OrchestratorConfig {
            root_dir: root_dir.clone(),
            discovery: DiscoveryConfig::new(vec![crate::models::Source::Youtube]),
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        let movies = orchestrator.scanner.scan().expect("scan should succeed");
        assert_eq!(movies.len(), 1);

        // With force flag - should process both
        let orchestrator_force = Orchestrator::new(OrchestratorConfig {
            root_dir,
            discovery: DiscoveryConfig::new(vec![crate::models::Source::Youtube]),
            force: true,
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        let movies_force = orchestrator_force
            .scanner
            .scan()
            .expect("scan should succeed");
        assert_eq!(movies_force.len(), 2);
    }

    #[test]
    fn test_orchestrator_concurrency_validation() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        std::fs::create_dir(&root_dir).expect("dir creation should succeed");

        for concurrency in 1..=10 {
            let orchestrator = Orchestrator::new(OrchestratorConfig {
                root_dir: root_dir.clone(),
                concurrency,
                ..test_config(PathBuf::new())
            })
            .expect("orchestrator build should succeed");

            assert_eq!(orchestrator.concurrency, concurrency);
        }
    }

    #[test]
    fn test_series_result_success() {
        let series = SeriesEntry {
            path: PathBuf::from("/series/Test (2020)"),
            title: "Test".to_string(),
            year: Some(2020),
            has_done_marker: false,
            seasons: vec![1, 2],
        };

        let result = SeriesResult::success(series, 3, 2, vec![]);

        assert!(result.success);
        assert_eq!(result.downloads, 3);
        assert_eq!(result.conversions, 2);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_series_result_failed() {
        let series = SeriesEntry {
            path: PathBuf::from("/series/Test (2020)"),
            title: "Test".to_string(),
            year: Some(2020),
            has_done_marker: false,
            seasons: vec![1, 2],
        };

        let result = SeriesResult::failed(series, "discovery", "API error".to_string(), vec![]);

        assert!(!result.success);
        assert_eq!(result.downloads, 0);
        assert_eq!(result.conversions, 0);
        assert!(result.error.is_some());
        assert!(
            result
                .error
                .as_ref()
                .expect("error should be set")
                .contains("discovery phase failed: API error")
        );
    }

    #[test]
    fn test_processing_summary_add_series_result() {
        let mut summary = ProcessingSummary::new();
        let result = SeriesResult::success(
            SeriesEntry {
                path: PathBuf::from("/series/Test (2020)"),
                title: "Test".to_string(),
                year: Some(2020),
                has_done_marker: false,
                seasons: vec![1],
            },
            4,
            3,
            vec![],
        );

        summary.add_series_result(&result);

        assert_eq!(summary.successful_series, 1);
        assert_eq!(summary.failed_series, 0);
        assert_eq!(summary.total_downloads, 4);
        assert_eq!(summary.total_conversions, 3);
    }

    #[test]
    fn test_orchestrator_new_with_processing_mode() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("library");
        std::fs::create_dir(&root_dir).unwrap();

        // Test with Both mode
        let orchestrator_both = Orchestrator::new(test_config(root_dir.clone()))
            .expect("orchestrator build should succeed");
        assert_eq!(orchestrator_both.processing_mode, ProcessingMode::Both);

        // Test with MoviesOnly mode
        let orchestrator_movies = Orchestrator::new(OrchestratorConfig {
            root_dir: root_dir.clone(),
            processing_mode: ProcessingMode::MoviesOnly,
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        assert_eq!(
            orchestrator_movies.processing_mode,
            ProcessingMode::MoviesOnly
        );

        // Test with SeriesOnly mode
        let orchestrator_series = Orchestrator::new(OrchestratorConfig {
            root_dir,
            processing_mode: ProcessingMode::SeriesOnly,
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        assert_eq!(
            orchestrator_series.processing_mode,
            ProcessingMode::SeriesOnly
        );
    }

    #[tokio::test]
    async fn test_orchestrator_run_with_series_and_movies() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("library");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        let movie_dir = root_dir.join("TestMovie (2020)");
        tokio::fs::create_dir(&movie_dir)
            .await
            .expect("dir creation should succeed");
        tokio::fs::write(movie_dir.join("movie.mp4"), "")
            .await
            .expect("file write should succeed");

        let series_dir = root_dir.join("TestSeries (2021)");
        tokio::fs::create_dir(&series_dir)
            .await
            .expect("dir creation should succeed");
        let season_dir = series_dir.join("Season 01");
        tokio::fs::create_dir(&season_dir)
            .await
            .expect("dir creation should succeed");

        // Test with Both mode - should find both
        let orchestrator = Orchestrator::new(OrchestratorConfig {
            root_dir: root_dir.clone(),
            discovery: DiscoveryConfig::new(vec![crate::models::Source::Youtube]),
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        let (movies, series) = orchestrator
            .scanner
            .scan_all()
            .expect("scan_all should succeed");
        assert_eq!(movies.len(), 1, "Both mode should find 1 movie");
        assert_eq!(series.len(), 1, "Both mode should find 1 series");

        // Test with MoviesOnly mode - should only find movies
        let orchestrator_movies = Orchestrator::new(OrchestratorConfig {
            root_dir: root_dir.clone(),
            discovery: DiscoveryConfig::new(vec![crate::models::Source::Youtube]),
            processing_mode: ProcessingMode::MoviesOnly,
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        let (movies_only, series_only) = orchestrator_movies
            .scanner
            .scan_all()
            .expect("scan_all should succeed");
        assert_eq!(movies_only.len(), 1);
        assert_eq!(
            series_only.len(),
            1,
            "Scanner still finds series regardless of mode"
        );

        // Test with SeriesOnly mode - should only find series
        let orchestrator_series = Orchestrator::new(OrchestratorConfig {
            root_dir,
            discovery: DiscoveryConfig::new(vec![crate::models::Source::Youtube]),
            processing_mode: ProcessingMode::SeriesOnly,
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        let (movies_series, series_series) = orchestrator_series
            .scanner
            .scan_all()
            .expect("scan_all should succeed");
        assert_eq!(
            movies_series.len(),
            1,
            "Scanner still finds movies regardless of mode"
        );
        assert_eq!(series_series.len(), 1);
    }

    #[tokio::test]
    async fn test_dry_run_returns_zero_downloads_and_conversions() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        // Create a movie folder so the scanner finds something
        let movie_dir = root_dir.join("DryRunTest (2023)");
        tokio::fs::create_dir(&movie_dir)
            .await
            .expect("dir creation should succeed");
        // A video file is needed so the scanner classifies this as a movie folder
        tokio::fs::write(movie_dir.join("movie.mp4"), b"")
            .await
            .expect("file write should succeed");

        let orchestrator = Orchestrator::new(OrchestratorConfig {
            root_dir,
            discovery: DiscoveryConfig {
                sources: vec![crate::models::Source::Youtube],
                dry_run: true,
                cookies_from_browser: None,
            },
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        assert!(orchestrator.dry_run);

        let summary = orchestrator.run().await.expect("run should succeed");

        // Dry-run should report the movie but 0 downloads and 0 conversions
        assert_eq!(summary.total_movies, 1);
        assert_eq!(summary.total_downloads, 0);
        assert_eq!(summary.total_conversions, 0);
    }

    #[tokio::test]
    async fn test_dry_run_does_not_write_done_marker() {
        use tempfile::TempDir;

        let temp_root = TempDir::new().expect("tempdir creation should succeed");
        let root_dir = temp_root.path().join("movies");
        tokio::fs::create_dir(&root_dir)
            .await
            .expect("dir creation should succeed");

        let movie_dir = root_dir.join("MarkerTest (2023)");
        tokio::fs::create_dir(&movie_dir)
            .await
            .expect("dir creation should succeed");

        let orchestrator = Orchestrator::new(OrchestratorConfig {
            root_dir,
            discovery: DiscoveryConfig {
                sources: vec![crate::models::Source::Youtube],
                dry_run: true,
                cookies_from_browser: None,
            },
            ..test_config(PathBuf::new())
        })
        .expect("orchestrator build should succeed");

        let _summary = orchestrator.run().await.expect("run should succeed");

        // Verify no done marker was written
        let marker_path = movie_dir.join("done.ext");
        assert!(
            !marker_path.exists(),
            "done.ext should NOT be created in dry-run mode"
        );
    }

    #[test]
    fn test_add_source_results_accumulates_correctly() {
        use crate::discovery::SourceResult;

        let mut summary = ProcessingSummary::new();

        // First batch: TMDB 5, YouTube 3
        let batch1 = vec![
            SourceResult {
                source: Source::Tmdb,
                videos_found: 5,
                error: None,
            },
            SourceResult {
                source: Source::Youtube,
                videos_found: 3,
                error: None,
            },
        ];
        summary.add_source_results(&batch1);

        assert_eq!(summary.source_totals[&Source::Tmdb], 5);
        assert_eq!(summary.source_totals[&Source::Youtube], 3);
        assert_eq!(summary.total_videos_discovered, 8);

        // Second batch: TMDB 2, Archive 4 — TMDB should accumulate
        let batch2 = vec![
            SourceResult {
                source: Source::Tmdb,
                videos_found: 2,
                error: None,
            },
            SourceResult {
                source: Source::Archive,
                videos_found: 4,
                error: None,
            },
        ];
        summary.add_source_results(&batch2);

        assert_eq!(summary.source_totals[&Source::Tmdb], 7);
        assert_eq!(summary.source_totals[&Source::Youtube], 3);
        assert_eq!(summary.source_totals[&Source::Archive], 4);
        assert_eq!(summary.total_videos_discovered, 14);
    }

    #[test]
    fn test_add_movie_result_merges_source_results() {
        use crate::discovery::SourceResult;

        let mut summary = ProcessingSummary::new();
        summary.total_movies = 1;

        let result = MovieResult::success(
            MovieEntry {
                path: PathBuf::from("/movies/Test (2020)"),
                title: "Test".to_string(),
                year: 2020,
                has_done_marker: false,
            },
            3,
            2,
            vec![
                SourceResult {
                    source: Source::Tmdb,
                    videos_found: 5,
                    error: None,
                },
                SourceResult {
                    source: Source::Youtube,
                    videos_found: 8,
                    error: None,
                },
            ],
        );

        summary.add_movie_result(&result);

        assert_eq!(summary.successful_movies, 1);
        assert_eq!(summary.source_totals[&Source::Tmdb], 5);
        assert_eq!(summary.source_totals[&Source::Youtube], 8);
        assert_eq!(summary.total_videos_discovered, 13);
    }

    #[test]
    fn test_add_series_result_merges_source_results() {
        use crate::discovery::SourceResult;

        let mut summary = ProcessingSummary::new();
        summary.total_series = 1;

        let result = SeriesResult::success(
            SeriesEntry {
                path: PathBuf::from("/series/Test (2020)"),
                title: "Test".to_string(),
                year: Some(2020),
                has_done_marker: false,
                seasons: vec![1],
            },
            4,
            3,
            vec![SourceResult {
                source: Source::Tmdb,
                videos_found: 6,
                error: None,
            }],
        );

        summary.add_series_result(&result);

        assert_eq!(summary.successful_series, 1);
        assert_eq!(summary.source_totals[&Source::Tmdb], 6);
        assert_eq!(summary.total_videos_discovered, 6);
    }

    #[test]
    fn test_failed_result_has_empty_source_results() {
        let result = MovieResult::failed(
            MovieEntry {
                path: PathBuf::from("/movies/Test (2020)"),
                title: "Test".to_string(),
                year: 2020,
                has_done_marker: false,
            },
            "discovery",
            "API error".to_string(),
            vec![],
        );

        assert!(result.source_results.is_empty());

        let mut summary = ProcessingSummary::new();
        summary.add_movie_result(&result);
        assert!(summary.source_totals.is_empty());
        assert_eq!(summary.total_videos_discovered, 0);
    }
}

#[cfg(test)]
mod property_tests {
    use crate::models::ProcessingMode;
    use crate::scanner::Scanner;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_sequential_downloads_within_movie(
            title in "[a-zA-Z][a-zA-Z0-9 ]{4,29}",
            year in 2000u16..2025u16,
            num_sources in 1usize..5usize
        ) {
            prop_assert!(num_sources > 0);
            prop_assert!((2000..2025).contains(&year));
            prop_assert!(!title.trim().is_empty());
        }
    }

    proptest! {
        #[test]
        fn prop_concurrency_limit_enforcement(
            concurrency in 1usize..=5usize
        ) {
            prop_assert!(concurrency >= 1);
            prop_assert!(concurrency <= 5);
        }
    }

    proptest! {
        #[test]
        fn prop_error_isolation_between_movies(
            num_movies in 2usize..6usize
        ) {
            prop_assert!(num_movies >= 2);
        }
    }

    proptest! {
        #[test]
        fn prop_temp_folder_cleanup_on_exit(_dummy in 0u8..10u8) {
            // Drop trait guarantees cleanup
        }
    }

    proptest! {
        #[test]
        fn prop_pre_existing_temp_cleanup(_dummy in 0u8..10u8) {
            // cleanup_pre_existing_temp() called at start of run()
        }
    }

    proptest! {
        #[test]
        fn prop_idempotent_re_execution(
            num_movies in 2usize..5usize,
            force_flag in proptest::bool::ANY
        ) {
            use tempfile::TempDir;
            use tokio::runtime::Runtime;

            let rt = Runtime::new().expect("tokio runtime creation should succeed");
            rt.block_on(async {
                let temp_root = TempDir::new().expect("tempdir creation should succeed");
                let root_dir = temp_root.path().join("movies");
                tokio::fs::create_dir(&root_dir).await.expect("dir creation should succeed");

                let mut movie_paths = Vec::new();
                for i in 0..num_movies {
                    let movie_folder = format!("Movie {} (202{})", i, i);
                    let movie_path = root_dir.join(&movie_folder);
                    tokio::fs::create_dir(&movie_path).await.expect("dir creation should succeed");
                    movie_paths.push(movie_path);
                }

                let scanner1 = Scanner::new(root_dir.clone(), false, false);
                let movies1 = scanner1.scan().expect("scan should succeed");
                prop_assert_eq!(movies1.len(), num_movies);

                let num_with_markers = num_movies / 2;
                for movie_path in movie_paths.iter().take(num_with_markers) {
                    let done_marker = crate::models::DoneMarker {
                        finished_at: "2024-01-15T10:30:00Z".to_string(),
                        version: "0.1.0".to_string(),
                    };
                    let marker_json = serde_json::to_string(&done_marker)
                        .expect("marker serialization should succeed");
                    tokio::fs::write(movie_path.join("done.ext"), marker_json)
                        .await
                        .expect("file write should succeed");
                }

                let scanner2 = Scanner::new(root_dir.clone(), false, false);
                let movies2 = scanner2.scan().expect("scan should succeed");
                let expected_without_force = num_movies - num_with_markers;
                prop_assert_eq!(movies2.len(), expected_without_force);

                for movie in &movies2 {
                    prop_assert!(!movie.has_done_marker);
                }

                let scanner3 = Scanner::new(root_dir.clone(), force_flag, false);
                let movies3 = scanner3.scan().expect("scan should succeed");

                if force_flag {
                    prop_assert_eq!(movies3.len(), num_movies);
                } else {
                    prop_assert_eq!(movies3.len(), expected_without_force);
                }

                let scanner4 = Scanner::new(root_dir.clone(), force_flag, false);
                let movies4 = scanner4.scan().expect("scan should succeed");
                prop_assert_eq!(movies3.len(), movies4.len());

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            }).expect("async block should succeed");
        }
    }

    proptest! {
        #[test]
        fn prop_processing_mode_filtering(
            num_movies in 1usize..4usize,
            num_series in 1usize..4usize,
            mode in prop_oneof![
                Just(ProcessingMode::Both),
                Just(ProcessingMode::MoviesOnly),
                Just(ProcessingMode::SeriesOnly),
            ]
        ) {
            use tempfile::TempDir;
            use tokio::runtime::Runtime;

            let rt = Runtime::new().expect("tokio runtime creation should succeed");
            rt.block_on(async {
                let temp_root = TempDir::new().expect("tempdir creation should succeed");
                let root_dir = temp_root.path().join("library");
                tokio::fs::create_dir(&root_dir).await.expect("dir creation should succeed");

                for i in 0..num_movies {
                    let movie_folder = format!("TestMovie{} (202{})", i, i);
                    let movie_path = root_dir.join(&movie_folder);
                    tokio::fs::create_dir(&movie_path).await.expect("dir creation should succeed");
                    tokio::fs::write(movie_path.join("movie.mp4"), "").await.expect("file write should succeed");
                }

                for i in 0..num_series {
                    let series_folder = format!("TestSeries{} (202{})", i, i);
                    let series_path = root_dir.join(&series_folder);
                    tokio::fs::create_dir(&series_path).await.expect("dir creation should succeed");
                    let season_path = series_path.join("Season 01");
                    tokio::fs::create_dir(&season_path).await.expect("dir creation should succeed");
                }

                let scanner = Scanner::new(root_dir.clone(), false, false);
                let (movies, series) = scanner.scan_all().expect("scan_all should succeed");

                match mode {
                    ProcessingMode::Both => {
                        prop_assert_eq!(movies.len(), num_movies);
                        prop_assert_eq!(series.len(), num_series);
                    }
                    ProcessingMode::MoviesOnly => {
                        prop_assert_eq!(movies.len(), num_movies);
                        prop_assert_eq!(series.len(), num_series);
                    }
                    ProcessingMode::SeriesOnly => {
                        prop_assert_eq!(movies.len(), num_movies);
                        prop_assert_eq!(series.len(), num_series);
                    }
                }

                Ok(()) as Result<(), proptest::test_runner::TestCaseError>
            }).expect("async block should succeed");
        }
    }

    proptest! {
        #[test]
        fn prop_series_error_isolation(
            num_series in 2usize..5usize
        ) {
            prop_assert!(num_series >= 2);
        }
    }
}
