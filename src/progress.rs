//! Progress reporting: one interface for the pipeline, three adapters — CLI text,
//! line-delimited JSON and the TUI.
//!
//! The pipeline says what happened (semantic operations); this module owns the event
//! payloads and hands every enabled sink the same facts. A sink never learns more than
//! the interface chooses to reveal.

use crate::discovery::SourceResult;
use crate::json_output::ProgressEvent;
use crate::models::{MovieEntry, SeriesEntry};
use crate::output;
use crate::tui::TuiState;
use std::sync::{Arc, Mutex};

/// Identifies the worker (pipeline task) an event belongs to, so the TUI can route
/// log lines to the right pane. It is opaque to the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkerId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Movie,
    Series,
}

/// The identity of the library item being processed, without the pipeline's internals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInfo {
    pub kind: ItemKind,
    pub title: String,
    pub year: Option<u16>,
}

impl ItemInfo {
    pub fn movie(movie: &MovieEntry) -> Self {
        Self {
            kind: ItemKind::Movie,
            title: movie.title.clone(),
            year: Some(movie.year),
        }
    }

    pub fn series(series: &SeriesEntry) -> Self {
        Self {
            kind: ItemKind::Series,
            title: series.title.clone(),
            year: series.year,
        }
    }

    pub fn label(&self) -> String {
        match self.year {
            Some(year) => format!("{} ({})", self.title, year),
            None => self.title.clone(),
        }
    }
}

/// A completed phase of one library item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Download,
    Conversion,
    Placement,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Download => "download",
            Phase::Conversion => "conversion",
            Phase::Placement => "placement",
        }
    }
}

/// The result of processing one library item.
#[derive(Debug, Clone)]
pub struct Outcome {
    pub success: bool,
    pub downloads: usize,
    pub conversions: usize,
    pub error: Option<String>,
}

/// An adapter at the reporting seam. Every operation is optional: a sink overrides
/// only the events it renders.
pub trait Sink: Send + Sync {
    fn run_started(&self, _total_items: usize) {}
    fn system_status(&self, _status: &str) {}
    fn item_started(&self, _worker: WorkerId, _item: &ItemInfo, _current: usize, _total: usize) {}
    fn discovery_finished(&self, _worker: WorkerId, _item: &ItemInfo, _candidates: usize) {}
    fn file_progress(&self, _worker: WorkerId, _filename: &str, _current: usize, _total: usize) {}
    fn phase_finished(
        &self,
        _worker: WorkerId,
        _item: &ItemInfo,
        _phase: Phase,
        _successful: usize,
        _total: usize,
    ) {
    }
    fn item_finished(&self, _worker: WorkerId, _item: &ItemInfo, _outcome: &Outcome) {}
    fn dry_run(
        &self,
        _item: &ItemInfo,
        _source_results: &[SourceResult],
        _total: usize,
        _duplicates_removed: usize,
    ) {
    }
    fn run_finished(&self) {}
}

/// The pipeline's view of progress. Cheap to share across tasks.
pub struct Progress {
    sinks: Vec<Box<dyn Sink>>,
    write_lock: Mutex<()>,
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

impl Progress {
    pub fn new() -> Self {
        Self {
            sinks: Vec::new(),
            write_lock: Mutex::new(()),
        }
    }

    pub fn add_sink(&mut self, sink: Box<dyn Sink>) {
        self.sinks.push(sink);
    }

    fn emit(&self, event: impl Fn(&dyn Sink)) {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for sink in &self.sinks {
            event(sink.as_ref());
        }
    }

    pub fn run_started(&self, total_items: usize) {
        self.emit(|sink| sink.run_started(total_items));
    }

    pub fn system_status(&self, status: &str) {
        self.emit(|sink| sink.system_status(status));
    }

    pub fn item_started(&self, worker: WorkerId, item: &ItemInfo, current: usize, total: usize) {
        self.emit(|sink| sink.item_started(worker, item, current, total));
    }

    pub fn discovery_finished(&self, worker: WorkerId, item: &ItemInfo, candidates: usize) {
        self.emit(|sink| sink.discovery_finished(worker, item, candidates));
    }

    pub fn file_progress(&self, worker: WorkerId, filename: &str, current: usize, total: usize) {
        self.emit(|sink| sink.file_progress(worker, filename, current, total));
    }

    pub fn phase_finished(
        &self,
        worker: WorkerId,
        item: &ItemInfo,
        phase: Phase,
        successful: usize,
        total: usize,
    ) {
        self.emit(|sink| sink.phase_finished(worker, item, phase, successful, total));
    }

    pub fn item_finished(&self, worker: WorkerId, item: &ItemInfo, outcome: &Outcome) {
        self.emit(|sink| sink.item_finished(worker, item, outcome));
    }

    pub fn dry_run(
        &self,
        item: &ItemInfo,
        source_results: &[SourceResult],
        total: usize,
        duplicates_removed: usize,
    ) {
        self.emit(|sink| sink.dry_run(item, source_results, total, duplicates_removed));
    }

    pub fn run_finished(&self) {
        self.emit(|sink| sink.run_finished());
    }
}

/// Human-readable CLI output.
pub struct CliSink;

impl Sink for CliSink {
    fn item_started(&self, _worker: WorkerId, item: &ItemInfo, current: usize, total: usize) {
        output::display_item_start(item, current, total);
    }

    fn file_progress(&self, _worker: WorkerId, filename: &str, current: usize, total: usize) {
        output::display_conversion_progress(filename, current, total);
    }

    fn dry_run(
        &self,
        item: &ItemInfo,
        source_results: &[SourceResult],
        total: usize,
        duplicates_removed: usize,
    ) {
        output::display_dry_run_results(item, source_results, total, duplicates_removed);
    }
}

/// Line-delimited JSON progress events on stdout.
pub struct JsonSink;

impl Sink for JsonSink {
    fn item_started(&self, _worker: WorkerId, item: &ItemInfo, current: usize, total: usize) {
        ProgressEvent::new("started", current, total, item.title.clone(), item.year).emit();
    }

    fn discovery_finished(&self, _worker: WorkerId, item: &ItemInfo, candidates: usize) {
        let mut event = ProgressEvent::new("phase", 0, 0, item.title.clone(), item.year);
        event.phase = Some("discovery".to_string());
        event.discovered = Some(candidates);
        event.emit();
    }

    fn phase_finished(
        &self,
        _worker: WorkerId,
        item: &ItemInfo,
        phase: Phase,
        successful: usize,
        _total: usize,
    ) {
        let mut event = ProgressEvent::new("phase", 0, 0, item.title.clone(), item.year);
        event.phase = Some(phase.as_str().to_string());
        match phase {
            Phase::Download => event.downloads = Some(successful),
            Phase::Conversion | Phase::Placement => event.conversions = Some(successful),
        }
        event.emit();
    }

    fn item_finished(&self, _worker: WorkerId, item: &ItemInfo, outcome: &Outcome) {
        let mut event = ProgressEvent::new("completed", 0, 0, item.title.clone(), item.year);
        event.success = Some(outcome.success);
        event.downloads = Some(outcome.downloads);
        event.conversions = Some(outcome.conversions);
        event.error = outcome.error.clone();
        event.emit();
    }
}

/// Split-pane TUI adapter. It owns its own pane bookkeeping; the pipeline only sends
/// item identity and per-worker events.
pub struct TuiSink {
    state: Arc<TuiState>,
}

impl TuiSink {
    pub fn new(state: Arc<TuiState>) -> Self {
        Self { state }
    }
}

impl Sink for TuiSink {
    fn run_started(&self, total_items: usize) {
        self.state.set_total_items(total_items);
    }

    fn system_status(&self, status: &str) {
        self.state.set_system_status(status);
    }

    fn item_started(&self, worker: WorkerId, item: &ItemInfo, _current: usize, _total: usize) {
        let label = item.label();
        self.state.set_current_item(&label);
        if item.kind == ItemKind::Series {
            self.state.add_active_item(&label);
        }
        self.state
            .log(worker.0, &format!("▶ Processing: {}", label));
    }

    fn discovery_finished(&self, worker: WorkerId, _item: &ItemInfo, candidates: usize) {
        self.state
            .log(worker.0, &format!("  Found {} candidates", candidates));
    }

    fn phase_finished(
        &self,
        worker: WorkerId,
        _item: &ItemInfo,
        phase: Phase,
        successful: usize,
        total: usize,
    ) {
        let line = match phase {
            Phase::Download => format!("  ↓ Downloaded {}/{}", successful, total),
            Phase::Conversion => format!("  ⇄ Converted {}/{}", successful, total),
            Phase::Placement => format!("  ✓ Placed {} files", successful),
        };
        self.state.log(worker.0, &line);
    }

    fn item_finished(&self, worker: WorkerId, item: &ItemInfo, outcome: &Outcome) {
        if item.kind == ItemKind::Series {
            self.state.remove_active_item(&item.label());
        }
        self.state.increment_processed();
        if outcome.success {
            self.state.log(
                worker.0,
                &format!(
                    "✓ Done: {} ({} dl, {} conv)",
                    item.label(),
                    outcome.downloads,
                    outcome.conversions
                ),
            );
        } else {
            self.state.log(
                worker.0,
                &format!(
                    "✗ Failed: {} — {}",
                    item.label(),
                    outcome.error.as_deref().unwrap_or("unknown")
                ),
            );
        }
    }

    fn dry_run(
        &self,
        item: &ItemInfo,
        _source_results: &[SourceResult],
        total: usize,
        duplicates_removed: usize,
    ) {
        self.state.log(
            0,
            &format!(
                "[DRY RUN] {}: {} discovered, {} duplicates",
                item.label(),
                total,
                duplicates_removed
            ),
        );
    }

    fn run_finished(&self) {
        self.state.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingSink {
        lines: Mutex<Vec<String>>,
    }

    impl RecordingSink {
        fn lines(&self) -> Vec<String> {
            self.lines.lock().unwrap().clone()
        }
    }

    impl Sink for RecordingSink {
        fn item_started(&self, worker: WorkerId, item: &ItemInfo, current: usize, total: usize) {
            self.lines.lock().unwrap().push(format!(
                "start {} {} {}/{}",
                worker.0,
                item.label(),
                current,
                total
            ));
        }

        fn phase_finished(
            &self,
            worker: WorkerId,
            item: &ItemInfo,
            phase: Phase,
            successful: usize,
            total: usize,
        ) {
            self.lines.lock().unwrap().push(format!(
                "{} {} {}/{}/{}",
                phase.as_str(),
                item.label(),
                worker.0,
                successful,
                total
            ));
        }
    }

    #[test]
    fn report_reaches_every_sink_with_identical_facts() {
        let first = Arc::new(RecordingSink::default());
        let second = Arc::new(RecordingSink::default());
        let mut progress = Progress::new();
        progress.add_sink(Box::new(SinkProbe(first.clone())));
        progress.add_sink(Box::new(SinkProbe(second.clone())));

        let item = ItemInfo {
            kind: ItemKind::Movie,
            title: "The Matrix".to_string(),
            year: Some(1999),
        };
        progress.item_started(WorkerId(2), &item, 1, 3);
        progress.phase_finished(WorkerId(2), &item, Phase::Conversion, 4, 5);

        assert_eq!(
            first.lines(),
            vec![
                "start 2 The Matrix (1999) 1/3",
                "conversion The Matrix (1999) 2/4/5"
            ]
        );
        assert_eq!(first.lines(), second.lines());
    }

    /// Forwards sink calls to a shared recording sink.
    struct SinkProbe(Arc<RecordingSink>);

    impl Sink for SinkProbe {
        fn item_started(&self, w: WorkerId, i: &ItemInfo, c: usize, t: usize) {
            self.0.item_started(w, i, c, t);
        }

        fn phase_finished(&self, w: WorkerId, i: &ItemInfo, p: Phase, s: usize, t: usize) {
            self.0.phase_finished(w, i, p, s, t);
        }
    }
}
