//! Library item pipeline: one interface for processing a movie or a series.
//!
//! Behind `process` / `process_all`: discovery is partitioned into extras scopes,
//! each scope is downloaded, converted and placed, temp files are cleaned, and a
//! Done marker is written only when at least one extra was placed. Concurrency is a
//! property of this module, not of its callers.

use crate::converter::Converter;
use crate::discovery::{
    Discovery, DiscoveryQuery, SourceResult, SpecialDiscovery, TvdbEpisodeExtended,
};
use crate::done_marker::DoneMarker;
use crate::downloader::Downloader;
use crate::models::{
    ConversionResult, DownloadResult, MovieEntry, SeriesEntry, Source, SpecialEpisode, VideoSource,
};
use crate::organizer::{Organizer, SeriesOrganizer};
use crate::progress::{ItemInfo, Outcome, Phase, Progress, WorkerId};
use log::{debug, error, info, warn};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A movie or a series: the unit the pipeline processes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryItem {
    Movie(MovieEntry),
    Series(SeriesEntry),
}

impl LibraryItem {
    pub fn info(&self) -> ItemInfo {
        match self {
            LibraryItem::Movie(movie) => ItemInfo::movie(movie),
            LibraryItem::Series(series) => ItemInfo::series(series),
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            LibraryItem::Movie(movie) => &movie.path,
            LibraryItem::Series(series) => &series.path,
        }
    }

    pub fn label(&self) -> String {
        match self {
            LibraryItem::Movie(movie) => movie.to_string(),
            LibraryItem::Series(series) => series.to_string(),
        }
    }

    fn temp_id(&self) -> String {
        match self {
            LibraryItem::Movie(movie) => {
                format!("{}_{}", movie.title.replace(' ', "_"), movie.year)
            }
            LibraryItem::Series(series) => {
                format!(
                    "{}_{}",
                    series.title.replace(' ', "_"),
                    series.year.unwrap_or(0)
                )
            }
        }
    }
}

/// Where a batch of extras belongs in the library.
///
/// Season 0 is not a season: TheTVDB specials are extras for the whole series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrasScope {
    /// The library item itself: a movie folder, or a series root.
    Item,
    /// A real season (>= 1) of a series.
    Season(u8),
    /// Series-wide specials, placed in the configured specials folder.
    Specials,
}

/// Candidates discovered for one scope, plus what placement needs to know.
#[derive(Debug, Clone)]
pub struct ExtrasBatch {
    pub scope: ExtrasScope,
    pub candidates: Vec<VideoSource>,
    /// TheTVDB metadata aligned with `candidates` for the specials scope.
    pub episodes: Vec<TvdbEpisodeExtended>,
}

/// What discovery produces for one library item.
#[derive(Debug, Clone)]
pub struct Discovered {
    pub batches: Vec<ExtrasBatch>,
    pub source_results: Vec<SourceResult>,
    pub duplicates_removed: usize,
}

/// The phase seam: the production adapter composes the real modules; tests supply
/// an in-memory adapter. Private to the pipeline — not part of its interface.
pub(crate) trait Phases: Send + Sync {
    fn discover<'a>(&'a self, item: &'a LibraryItem) -> BoxFuture<'a, Discovered>;
    fn download<'a>(
        &'a self,
        item_id: &'a str,
        candidates: Vec<VideoSource>,
    ) -> BoxFuture<'a, Vec<DownloadResult>>;
    fn convert<'a>(
        &'a self,
        downloads: Vec<DownloadResult>,
        progress: &'a Progress,
        worker: WorkerId,
    ) -> BoxFuture<'a, Vec<ConversionResult>>;
    fn place<'a>(
        &'a self,
        item: &'a LibraryItem,
        batch: &'a ExtrasBatch,
        conversions: Vec<ConversionResult>,
    ) -> BoxFuture<'a, Result<usize, String>>;
}

/// The outcome of one library item.
#[derive(Debug, Clone)]
pub struct ItemOutcome {
    pub item: LibraryItem,
    pub success: bool,
    pub downloads: usize,
    pub conversions: usize,
    pub placed: usize,
    pub source_results: Vec<SourceResult>,
    pub duplicates_removed: usize,
    pub error: Option<String>,
}

impl ItemOutcome {
    pub fn progress_outcome(&self) -> Outcome {
        Outcome {
            success: self.success,
            downloads: self.downloads,
            conversions: self.conversions,
            error: self.error.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub dry_run: bool,
    pub sources: Vec<Source>,
    pub season_extras: bool,
    pub specials: bool,
    pub specials_folder: String,
    pub temp_base: PathBuf,
}

/// Thin coordinator: run discovery, then one batch at a time through the phases.
pub struct Pipeline {
    phases: Box<dyn Phases>,
    config: PipelineConfig,
    progress: Arc<Progress>,
    concurrency: usize,
}

impl Pipeline {
    pub(crate) fn new(
        phases: impl Phases + 'static,
        config: PipelineConfig,
        progress: Arc<Progress>,
        concurrency: usize,
    ) -> Self {
        Self {
            phases: Box::new(phases),
            config,
            progress,
            concurrency,
        }
    }

    /// Process every item, emitting item lifecycle events. Sequential and parallel
    /// runs share one implementation; only the scheduling differs.
    pub async fn process_all(self: &Arc<Self>, items: Vec<LibraryItem>) -> Vec<ItemOutcome> {
        let total = items.len();
        if self.concurrency <= 1 {
            let mut outcomes = Vec::with_capacity(total);
            for (idx, item) in items.into_iter().enumerate() {
                let info = item.info();
                self.progress
                    .item_started(WorkerId(0), &info, idx + 1, total);
                let outcome = self.process(item, WorkerId(0)).await;
                self.progress.item_finished(
                    WorkerId(0),
                    &outcome.item.info(),
                    &outcome.progress_outcome(),
                );
                outcomes.push(outcome);
            }
            return outcomes;
        }

        let semaphore = Arc::new(Semaphore::new(self.concurrency));
        let counter = Arc::new(AtomicUsize::new(0));
        let worker_count = self.concurrency;
        let mut tasks = Vec::with_capacity(total);
        for item in items {
            let semaphore = semaphore.clone();
            let counter = counter.clone();
            let pipeline = self.clone();
            tasks.push(tokio::spawn(async move {
                let _permit = semaphore
                    .acquire()
                    .await
                    .expect("semaphore should not be closed");
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                let worker = WorkerId((current - 1) % worker_count);
                let info = item.info();
                pipeline
                    .progress
                    .item_started(worker, &info, current, total);
                let outcome = pipeline.process(item, worker).await;
                pipeline.progress.item_finished(
                    worker,
                    &outcome.item.info(),
                    &outcome.progress_outcome(),
                );
                outcome
            }));
        }

        let mut outcomes = Vec::with_capacity(total);
        for task in tasks {
            if let Ok(outcome) = task.await {
                outcomes.push(outcome);
            }
        }
        outcomes
    }

    async fn process(&self, item: LibraryItem, worker: WorkerId) -> ItemOutcome {
        let info = item.info();
        info!("Processing: {}", item.label());

        let discovered = self.phases.discover(&item).await;
        let candidate_count: usize = discovered.batches.iter().map(|b| b.candidates.len()).sum();
        self.progress
            .discovery_finished(worker, &info, candidate_count);

        if self.config.dry_run {
            self.progress.dry_run(
                &info,
                &discovered.source_results,
                candidate_count,
                discovered.duplicates_removed,
            );
            return ItemOutcome {
                item,
                success: true,
                downloads: 0,
                conversions: 0,
                placed: 0,
                source_results: discovered.source_results,
                duplicates_removed: discovered.duplicates_removed,
                error: None,
            };
        }

        // A stale marker is cleared before reprocessing; missing is fine.
        if let Err(e) = DoneMarker::remove(item.path()).await {
            warn!("Failed to remove done marker for {}: {}", item.label(), e);
        }

        if candidate_count == 0 {
            warn!("No extras found for {}", item.label());
            return self.finish_outcome(item, discovered, 0, 0, 0, None);
        }

        let item_id = item.temp_id();
        let mut downloads = 0;
        let mut conversions = 0;
        let mut placed = 0;
        let mut failure = None;

        for batch in &discovered.batches {
            if batch.candidates.is_empty() {
                continue;
            }

            let batch_downloads = self
                .phases
                .download(&item_id, batch.candidates.clone())
                .await;
            let successful_downloads = batch_downloads.iter().filter(|d| d.success).count();
            downloads += successful_downloads;
            self.progress.phase_finished(
                worker,
                &info,
                Phase::Download,
                successful_downloads,
                batch_downloads.len(),
            );
            if successful_downloads == 0 {
                continue;
            }

            let batch_conversions = self
                .phases
                .convert(batch_downloads, &self.progress, worker)
                .await;
            let successful_conversions = batch_conversions.iter().filter(|c| c.success).count();
            conversions += successful_conversions;
            self.progress.phase_finished(
                worker,
                &info,
                Phase::Conversion,
                successful_conversions,
                batch_conversions.len(),
            );
            if successful_conversions == 0 {
                continue;
            }

            match self.phases.place(&item, batch, batch_conversions).await {
                Ok(count) => {
                    placed += count;
                    self.progress.phase_finished(
                        worker,
                        &info,
                        Phase::Placement,
                        count,
                        successful_conversions,
                    );
                }
                Err(e) => {
                    error!("Placement failed for {}: {}", item.label(), e);
                    if failure.is_none() {
                        failure = Some(format!("organization phase failed: {}", e));
                    }
                }
            }
        }

        cleanup_temp_dir(&self.config.temp_base.join(&item_id)).await;

        // Done marker promises placed extras: a run that placed nothing stays open.
        if failure.is_none()
            && placed > 0
            && let Err(e) = DoneMarker::write(item.path()).await
        {
            warn!("Failed to create done marker for {}: {}", item.label(), e);
        }

        self.finish_outcome(item, discovered, downloads, conversions, placed, failure)
    }

    fn finish_outcome(
        &self,
        item: LibraryItem,
        discovered: Discovered,
        downloads: usize,
        conversions: usize,
        placed: usize,
        error: Option<String>,
    ) -> ItemOutcome {
        ItemOutcome {
            item,
            success: error.is_none(),
            downloads,
            conversions,
            placed,
            source_results: discovered.source_results,
            duplicates_removed: discovered.duplicates_removed,
            error,
        }
    }
}

/// Seasons that get their own extras scope. Season 0 is specials, handled separately.
fn seasons_to_query(seasons: &[u8]) -> impl Iterator<Item = u8> + '_ {
    seasons.iter().copied().filter(|&season| season > 0)
}

/// The production phase adapter: real Discovery, Downloader, Converter and placement.
pub(crate) struct ProductionPhases {
    discovery: Arc<Discovery>,
    special_discovery: Arc<SpecialDiscovery>,
    downloader: Arc<Downloader>,
    converter: Arc<Converter>,
    config: PipelineConfig,
}

impl ProductionPhases {
    pub(crate) fn new(
        discovery: Arc<Discovery>,
        special_discovery: Arc<SpecialDiscovery>,
        downloader: Arc<Downloader>,
        converter: Arc<Converter>,
        config: PipelineConfig,
    ) -> Self {
        Self {
            discovery,
            special_discovery,
            downloader,
            converter,
            config,
        }
    }

    async fn discover_series(&self, series: &SeriesEntry) -> Discovered {
        let base = DiscoveryQuery::series(series, self.config.sources.clone());
        let mut queries = vec![base.clone()];
        if self.config.season_extras {
            for season in seasons_to_query(&series.seasons) {
                queries.push(base.for_season(season));
            }
        }
        let report = self.discovery.discover(&queries).await;

        let mut item_candidates = Vec::new();
        let mut by_season: BTreeMap<u8, Vec<VideoSource>> = BTreeMap::new();
        for candidate in report.candidates {
            match candidate.season_number {
                Some(season) if season > 0 => by_season.entry(season).or_default().push(candidate),
                _ => item_candidates.push(candidate),
            }
        }

        let mut batches = Vec::new();
        if !item_candidates.is_empty() {
            batches.push(ExtrasBatch {
                scope: ExtrasScope::Item,
                candidates: item_candidates,
                episodes: Vec::new(),
            });
        }
        for (season, candidates) in by_season {
            batches.push(ExtrasBatch {
                scope: ExtrasScope::Season(season),
                candidates,
                episodes: Vec::new(),
            });
        }

        if self.config.specials {
            let (candidates, episodes) = self.special_discovery.select(series).await;
            if !candidates.is_empty() {
                batches.push(ExtrasBatch {
                    scope: ExtrasScope::Specials,
                    candidates,
                    episodes,
                });
            }
        }

        Discovered {
            batches,
            source_results: report.source_results,
            duplicates_removed: report.duplicates_removed,
        }
    }
}

impl Phases for ProductionPhases {
    fn discover<'a>(&'a self, item: &'a LibraryItem) -> BoxFuture<'a, Discovered> {
        Box::pin(async move {
            match item {
                LibraryItem::Movie(movie) => {
                    let query = DiscoveryQuery::movie(movie, self.config.sources.clone());
                    let report = self.discovery.discover(std::slice::from_ref(&query)).await;
                    Discovered {
                        batches: vec![ExtrasBatch {
                            scope: ExtrasScope::Item,
                            candidates: report.candidates,
                            episodes: Vec::new(),
                        }],
                        source_results: report.source_results,
                        duplicates_removed: report.duplicates_removed,
                    }
                }
                LibraryItem::Series(series) => self.discover_series(series).await,
            }
        })
    }

    fn download<'a>(
        &'a self,
        item_id: &'a str,
        candidates: Vec<VideoSource>,
    ) -> BoxFuture<'a, Vec<DownloadResult>> {
        Box::pin(async move { self.downloader.download_all(item_id, candidates).await })
    }

    fn convert<'a>(
        &'a self,
        downloads: Vec<DownloadResult>,
        progress: &'a Progress,
        worker: WorkerId,
    ) -> BoxFuture<'a, Vec<ConversionResult>> {
        Box::pin(async move {
            self.converter
                .convert_batch(downloads, progress, worker)
                .await
        })
    }

    fn place<'a>(
        &'a self,
        item: &'a LibraryItem,
        batch: &'a ExtrasBatch,
        conversions: Vec<ConversionResult>,
    ) -> BoxFuture<'a, Result<usize, String>> {
        Box::pin(async move {
            match (item, batch.scope) {
                (LibraryItem::Movie(movie), _) => Organizer::new(movie.path.clone())
                    .organize(conversions)
                    .await
                    .map_err(|e| e.to_string()),
                (LibraryItem::Series(series), ExtrasScope::Item) => {
                    SeriesOrganizer::new(series.path.clone(), series.seasons.clone())
                        .organize_extras(conversions, None)
                        .await
                        .map_err(|e| e.to_string())
                }
                (LibraryItem::Series(series), ExtrasScope::Season(season)) => {
                    SeriesOrganizer::new(series.path.clone(), series.seasons.clone())
                        .organize_extras(conversions, Some(season))
                        .await
                        .map_err(|e| e.to_string())
                }
                (LibraryItem::Series(series), ExtrasScope::Specials) => {
                    let mut special_episodes = Vec::new();
                    for (idx, conversion) in conversions.iter().enumerate() {
                        if conversion.success
                            && let Some(episode) = batch.episodes.get(idx)
                        {
                            special_episodes.push(SpecialEpisode {
                                episode_number: episode.number,
                                title: episode.name.clone(),
                                air_date: episode.aired.clone(),
                                url: batch.candidates.get(idx).map(|c| c.url.clone()),
                                local_path: Some(conversion.output_path.clone()),
                                tvdb_id: Some(episode.id),
                            });
                        }
                    }
                    if special_episodes.is_empty() {
                        debug!("No specials to place for {}", series);
                        return Ok(0);
                    }
                    SeriesOrganizer::new(series.path.clone(), series.seasons.clone())
                        .organize_specials(
                            &series.title,
                            special_episodes,
                            &self.config.specials_folder,
                        )
                        .await
                        .map_err(|e| e.to_string())
                }
            }
        })
    }
}

/// Remove a temp directory if it exists.
async fn cleanup_temp_dir(temp_dir: &Path) {
    if temp_dir.exists()
        && let Err(e) = tokio::fs::remove_dir_all(temp_dir).await
    {
        warn!("Failed to cleanup temp dir {:?}: {}", temp_dir, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContentCategory, SourceType};
    use std::sync::Mutex;

    struct FakePhases {
        batches: Vec<ExtrasBatch>,
        downloads: Vec<DownloadResult>,
        conversions: Vec<ConversionResult>,
        placed: Result<usize, String>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl FakePhases {
        fn new() -> Self {
            Self {
                batches: Vec::new(),
                downloads: Vec::new(),
                conversions: Vec::new(),
                placed: Ok(0),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls_handle(&self) -> Arc<Mutex<Vec<String>>> {
            self.calls.clone()
        }
    }

    fn calls(calls: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        calls.lock().unwrap().clone()
    }

    fn candidate(title: &str) -> VideoSource {
        VideoSource {
            url: format!("https://example.com/{}", title),
            source_type: SourceType::YouTube,
            category: ContentCategory::Trailer,
            title: title.to_string(),
            season_number: None,
            duration_secs: Some(60),
        }
    }

    fn movie(dir: &Path) -> LibraryItem {
        LibraryItem::Movie(MovieEntry {
            path: dir.to_path_buf(),
            title: "Test Movie".to_string(),
            year: 2020,
        })
    }

    impl Phases for FakePhases {
        fn discover<'a>(&'a self, _item: &'a LibraryItem) -> BoxFuture<'a, Discovered> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("discover".to_string());
                Discovered {
                    batches: self.batches.clone(),
                    source_results: vec![],
                    duplicates_removed: 0,
                }
            })
        }

        fn download<'a>(
            &'a self,
            _item_id: &'a str,
            _candidates: Vec<VideoSource>,
        ) -> BoxFuture<'a, Vec<DownloadResult>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("download".to_string());
                self.downloads.clone()
            })
        }

        fn convert<'a>(
            &'a self,
            _downloads: Vec<DownloadResult>,
            _progress: &'a Progress,
            _worker: WorkerId,
        ) -> BoxFuture<'a, Vec<ConversionResult>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("convert".to_string());
                self.conversions.clone()
            })
        }

        fn place<'a>(
            &'a self,
            _item: &'a LibraryItem,
            _batch: &'a ExtrasBatch,
            _conversions: Vec<ConversionResult>,
        ) -> BoxFuture<'a, Result<usize, String>> {
            Box::pin(async move {
                self.calls.lock().unwrap().push("place".to_string());
                self.placed.clone()
            })
        }
    }

    fn config(root: &Path) -> PipelineConfig {
        PipelineConfig {
            dry_run: false,
            sources: vec![Source::Youtube],
            season_extras: false,
            specials: false,
            specials_folder: "Specials".to_string(),
            temp_base: root.join("tmp_downloads"),
        }
    }

    fn batch(candidates: Vec<VideoSource>) -> ExtrasBatch {
        ExtrasBatch {
            scope: ExtrasScope::Item,
            candidates,
            episodes: Vec::new(),
        }
    }

    fn download(dir: &Path) -> DownloadResult {
        DownloadResult {
            source: candidate("One"),
            local_path: dir.join("one.mp4"),
            success: true,
            error: None,
            subtitle_paths: vec![],
        }
    }

    fn conversion(dir: &Path) -> ConversionResult {
        ConversionResult {
            input_path: dir.join("one.mp4"),
            output_path: dir.join("out.mp4"),
            category: ContentCategory::Trailer,
            season_number: None,
            success: true,
            error: None,
        }
    }

    fn build_pipeline(phases: FakePhases, root: &Path) -> Arc<Pipeline> {
        Arc::new(Pipeline::new(
            phases,
            config(root),
            Arc::new(Progress::new()),
            1,
        ))
    }

    #[tokio::test]
    async fn no_candidates_skips_every_phase_and_writes_no_marker() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut fake = FakePhases::new();
        fake.batches = vec![batch(vec![])];
        let recorded = fake.calls_handle();
        let pipeline = build_pipeline(fake, dir.path());
        let outcomes = pipeline.process_all(vec![movie(dir.path())]).await;

        assert_eq!(calls(&recorded), vec!["discover"]);
        assert!(outcomes[0].success);
        assert_eq!(outcomes[0].downloads, 0);
        assert!(!DoneMarker::exists(dir.path()));
    }

    #[tokio::test]
    async fn marker_written_only_when_something_was_placed() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut fake = FakePhases::new();
        fake.batches = vec![batch(vec![candidate("One")])];
        fake.downloads = vec![download(dir.path())];
        fake.conversions = vec![conversion(dir.path())];
        fake.placed = Ok(0);

        let pipeline = build_pipeline(fake, dir.path());
        pipeline.process_all(vec![movie(dir.path())]).await;
        assert!(
            !DoneMarker::exists(dir.path()),
            "nothing placed means the item stays open"
        );

        let mut fake = FakePhases::new();
        fake.batches = vec![batch(vec![candidate("One")])];
        fake.downloads = vec![download(dir.path())];
        fake.conversions = vec![conversion(dir.path())];
        fake.placed = Ok(1);
        let pipeline = build_pipeline(fake, dir.path());
        let outcomes = pipeline.process_all(vec![movie(dir.path())]).await;
        assert!(outcomes[0].success);
        assert!(DoneMarker::exists(dir.path()));
    }

    #[tokio::test]
    async fn zero_conversions_skips_placement_and_marker() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut fake = FakePhases::new();
        fake.batches = vec![batch(vec![candidate("One")])];
        fake.downloads = vec![download(dir.path())];
        fake.conversions = vec![];
        let recorded = fake.calls_handle();
        let pipeline = build_pipeline(fake, dir.path());
        let outcomes = pipeline.process_all(vec![movie(dir.path())]).await;

        assert_eq!(calls(&recorded), vec!["discover", "download", "convert"]);
        assert!(outcomes[0].success);
        assert_eq!(outcomes[0].downloads, 1);
        assert_eq!(outcomes[0].conversions, 0);
        assert!(!DoneMarker::exists(dir.path()));
    }

    #[tokio::test]
    async fn placement_failure_fails_the_item_and_keeps_it_open() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut fake = FakePhases::new();
        fake.batches = vec![batch(vec![candidate("One")])];
        fake.downloads = vec![download(dir.path())];
        fake.conversions = vec![conversion(dir.path())];
        fake.placed = Err("disk full".to_string());
        let pipeline = build_pipeline(fake, dir.path());
        let outcomes = pipeline.process_all(vec![movie(dir.path())]).await;

        assert!(!outcomes[0].success);
        assert!(
            outcomes[0]
                .error
                .as_deref()
                .is_some_and(|e| e.contains("disk full"))
        );
        assert!(!DoneMarker::exists(dir.path()));
    }

    #[tokio::test]
    async fn every_scope_runs_the_same_phases_in_order() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut fake = FakePhases::new();
        fake.batches = vec![
            batch(vec![candidate("Series")]),
            ExtrasBatch {
                scope: ExtrasScope::Season(2),
                candidates: vec![candidate("Season")],
                episodes: Vec::new(),
            },
        ];
        fake.downloads = vec![download(dir.path())];
        fake.conversions = vec![conversion(dir.path())];
        fake.placed = Ok(1);
        let recorded = fake.calls_handle();
        let pipeline = build_pipeline(fake, dir.path());
        let outcomes = pipeline.process_all(vec![movie(dir.path())]).await;

        assert_eq!(
            calls(&recorded),
            vec![
                "discover", "download", "convert", "place", "download", "convert", "place"
            ]
        );
        assert_eq!(outcomes[0].placed, 2);
        assert_eq!(outcomes[0].downloads, 2);
    }

    #[tokio::test]
    async fn dry_run_returns_after_discovery() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut fake = FakePhases::new();
        fake.batches = vec![batch(vec![candidate("One")])];
        let recorded = fake.calls_handle();
        let mut cfg = config(dir.path());
        cfg.dry_run = true;
        let pipeline = Arc::new(Pipeline::new(fake, cfg, Arc::new(Progress::new()), 1));
        let outcomes = pipeline.process_all(vec![movie(dir.path())]).await;

        assert_eq!(calls(&recorded), vec!["discover"]);
        assert!(outcomes[0].success);
        assert!(!DoneMarker::exists(dir.path()));
    }

    #[tokio::test]
    async fn concurrent_run_matches_sequential_calls() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut fake = FakePhases::new();
        fake.batches = vec![batch(vec![candidate("One")])];
        fake.downloads = vec![download(dir.path())];
        fake.conversions = vec![conversion(dir.path())];
        fake.placed = Ok(1);
        let pipeline = Arc::new(Pipeline::new(
            fake,
            config(dir.path()),
            Arc::new(Progress::new()),
            3,
        ));
        let outcomes = pipeline
            .process_all(vec![
                movie(dir.path()),
                movie(dir.path()),
                movie(dir.path()),
            ])
            .await;

        assert_eq!(outcomes.len(), 3);
        assert!(outcomes.iter().all(|o| o.success && o.placed == 1));
        assert!(DoneMarker::exists(dir.path()));
    }

    #[test]
    fn season_zero_is_not_queried_as_a_season() {
        assert_eq!(seasons_to_query(&[0, 1, 3]).collect::<Vec<_>>(), vec![1, 3]);
    }
}
