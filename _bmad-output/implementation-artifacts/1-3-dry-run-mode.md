# Story 1.3: Dry-Run Mode

Status: draft

## Story

As a user,
I want to run `--dry-run` to see what extras would be discovered without downloading anything,
So that I can preview results and validate source value before committing to a full run.

## Acceptance Criteria

1. `--dry-run` flag is added to `CliArgs` and `CliConfig`
2. When `--dry-run` is active, the pipeline stops after discovery — no downloads, conversions, or file organization occur
3. No file I/O occurs beyond logging in dry-run mode (NFR5)
4. The per-source discovery results are displayed in dry-run mode (FR32)
5. The done marker is NOT written in dry-run mode
6. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [ ] Task 1: Add `--dry-run` flag to CLI (AC: #1)
  - [ ] 1.1 Add `dry_run: bool` field to `CliArgs` in `src/cli.rs` with `#[arg(long)]`
  - [ ] 1.2 Add `dry_run: bool` field to `CliConfig` in `src/cli.rs`
  - [ ] 1.3 Propagate `dry_run` in `From<CliArgs> for CliConfig`
  - [ ] 1.4 Add `dry_run: bool` field to `OrchestratorConfig` in `src/orchestrator.rs`
  - [ ] 1.5 Add `dry_run: bool` field to `Orchestrator` struct in `src/orchestrator.rs`
  - [ ] 1.6 Pass `dry_run` through `Orchestrator::new()` and store it on the struct
  - [ ] 1.7 Update `display_config()` in `src/cli.rs` to show dry-run status when enabled
  - [ ] 1.8 Update `main.rs` to pass `dry_run` from `CliConfig` to `OrchestratorConfig`

- [ ] Task 2: Implement dry-run pipeline short-circuit (AC: #2, #3, #5)
  - [ ] 2.1 In `process_movie_standalone()`: after discovery, if `dry_run` is true, log the discovered sources and return `MovieResult::success(movie, 0, 0)` — no download, conversion, or organization
  - [ ] 2.2 In `process_series_standalone()`: after `discover_series_content()`, if `dry_run` is true, log the discovered extras and return `SeriesResult::success(series, 0, 0)` — no download, conversion, or organization
  - [ ] 2.3 Verify no file I/O occurs in the dry-run path (no temp dir creation, no done marker write)
  - [ ] 2.4 `process_movie_standalone()` needs `dry_run: bool` added to its parameter list (or passed via a context struct); use the same approach as `SeriesProcessingContext` if needed
  - [ ] 2.5 `process_series_standalone()` already uses `SeriesProcessingContext` — add `dry_run: bool` to that struct

- [ ] Task 3: Display per-source discovery results in dry-run mode (AC: #4)
  - [ ] 3.1 Add `display_dry_run_results()` function to `src/output.rs` that accepts `&[SourceResult]` and `total_videos: usize` and prints a formatted per-source table
  - [ ] 3.2 Call `display_dry_run_results()` from the dry-run early-return path in `process_movie_standalone()` and `process_series_standalone()`
  - [ ] 3.3 The display should show: source name, video count (or "failed: <error>") for each `SourceResult`
  - [ ] 3.4 The `_source_results` variable currently discarded in `process_movie_standalone()` must be used — remove the `_` prefix and pass it to the display function

- [ ] Task 4: Update tests (AC: #6)
  - [ ] 4.1 Add unit test: `--dry-run` flag is parsed correctly and sets `dry_run: true` in `CliConfig`
  - [ ] 4.2 Add unit test: dry-run mode returns 0 downloads and 0 conversions in `ProcessingSummary`
  - [ ] 4.3 Add unit test: `display_dry_run_results()` does not panic with empty or populated `SourceResult` slices
  - [ ] 4.4 Add unit test: done marker is NOT written when `dry_run` is true (verify no `done.ext` file created)
  - [ ] 4.5 Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`

## Dev Notes

### What Already Exists (Do NOT Redo)

- `SourceResult` struct is already defined in `src/discovery/orchestrator.rs` and re-exported from `src/discovery/mod.rs`
- `discover_all()` already returns `(Vec<VideoSource>, Vec<SourceResult>)` — the `_source_results` is currently discarded in `process_movie_standalone()`
- `discover_series_content()` already discards `_series_source_results` and `_season_source_results` — these need to be collected and passed to the display function

### Key Code Locations

| What | File | Current State |
|---|---|---|
| `CliArgs` | `src/cli.rs` | No `dry_run` field |
| `CliConfig` | `src/cli.rs` | No `dry_run` field |
| `OrchestratorConfig` | `src/orchestrator.rs` | No `dry_run` field |
| `Orchestrator` struct | `src/orchestrator.rs` | No `dry_run` field |
| `process_movie_standalone()` | `src/orchestrator.rs` | Discards `_source_results` |
| `process_series_standalone()` | `src/orchestrator.rs` | Calls `discover_series_content()` which discards source results |
| `discover_series_content()` | `src/orchestrator.rs` | Discards `_series_source_results` and `_season_source_results` |
| `display_summary()` | `src/output.rs` | No dry-run awareness |
| `main.rs` | `src/main.rs` | Builds `OrchestratorConfig` — needs `dry_run` field |

### `process_movie_standalone()` Signature Change

Currently:
```rust
async fn process_movie_standalone(
    movie: MovieEntry,
    discovery: Arc<DiscoveryOrchestrator>,
    downloader: Arc<Downloader>,
    converter: Arc<Converter>,
    temp_base: PathBuf,
) -> MovieResult
```

After this story, `dry_run: bool` must be accessible inside this function. Two options:
1. Add `dry_run: bool` as a direct parameter (simple, but adds to an already long list)
2. Create a `MovieProcessingContext` struct mirroring `SeriesProcessingContext` (cleaner, consistent)

Prefer option 2 for consistency with the series path — create `MovieProcessingContext` with fields: `discovery`, `downloader`, `converter`, `temp_base`, `dry_run`. Update `process_movie()`, `process_movies_parallel()`, and `process_movies_sequential()` accordingly.

### `discover_series_content()` Source Result Collection

Currently `discover_series_content()` discards source results:
```rust
let (mut all_extras, _series_source_results) = series_discovery.discover_all(series).await;
// ...
let (extras, _season_source_results) = series_discovery.discover_season_extras(series, season).await;
```

For dry-run display, these need to be collected. The function signature should return an additional `Vec<SourceResult>` alongside the existing tuple, or the display logic can be inlined in `process_series_standalone()` before the dry-run early return.

The simplest approach: collect all `SourceResult`s inside `discover_series_content()` and return them as a fourth element of the tuple. Update the caller in `process_series_standalone()` to destructure the new return.

### Dry-Run Display Format

The `display_dry_run_results()` function should produce output like:
```
  [DRY RUN] Discovery results:
    tmdb       — 3 videos
    archive    — 0 videos
    youtube    — 7 videos
    dailymotion — failed: connection refused
  Total: 10 videos (would download)
```

Keep it simple — no fancy table formatting needed. Use `info!` level for the per-source lines and `warn!` for failed sources, consistent with the orchestrator's own logging.

### NFR5: No File I/O in Dry-Run

The dry-run path must not:
- Create `tmp_downloads/` directory
- Write `done.ext` marker
- Move or copy any files

The current `cleanup_pre_existing_temp()` in `Orchestrator::run()` runs before processing. This is fine — it cleans up leftover state from a previous interrupted run, which is valid even in dry-run mode. No change needed there.

### Done Marker Not Written

The done marker is written in two places:
1. `Organizer::organize()` — called from `process_movie_standalone()` (Phase 5)
2. `write_done_marker()` — called from `process_series_standalone()` after successful organization

Both are skipped naturally when the dry-run early return fires after discovery. No special "don't write done marker" guard is needed — the early return before Phase 3 ensures neither path is reached.

### `display_config()` Update

Add a dry-run indicator to `display_config()`:
```rust
if config.dry_run {
    println!("  {} {}", "Dry Run:".bright_white(), "Yes (discovery only)".bright_yellow());
}
```

### What NOT To Do

- Do NOT add dry-run awareness to `Organizer`, `Downloader`, or `Converter` — the short-circuit happens in the orchestrator before those are called
- Do NOT add a `--dry-run` flag to `OrchestratorConfig` and then check it in every phase — one check after discovery is sufficient
- Do NOT display dry-run results in `display_summary()` — the per-source display happens inline during processing, not in the final summary
- Do NOT skip `cleanup_pre_existing_temp()` in dry-run mode — cleaning up leftover temp dirs is safe and desirable

### Regression Risks

- Adding `dry_run` to `OrchestratorConfig` will require updating all test code that constructs `OrchestratorConfig` directly — check `tests/main_integration_tests.rs` and `tests/series_integration_tests.rs`
- Adding `MovieProcessingContext` changes the signature of `process_movie_standalone()` — update both call sites: `process_movie()` and the closure in `process_movies_parallel()`
- Adding a fourth element to `discover_series_content()`'s return tuple requires updating its single caller in `process_series_standalone()`

### Project Structure Notes

- `src/discovery/mod.rs` already re-exports `SourceResult` — no change needed there
- Tests are co-located in `#[cfg(test)]` blocks within each module
- Integration tests in `tests/` directory

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 1, Story 1.3]
- [Source: _bmad-output/planning-artifacts/prd.md — FR31, FR32, NFR5]
- [Source: src/orchestrator.rs — process_movie_standalone(), process_series_standalone(), discover_series_content()]
- [Source: src/cli.rs — CliArgs, CliConfig, display_config()]
- [Source: src/output.rs — display_summary()]
- [Source: src/discovery/orchestrator.rs — SourceResult]

## Dev Agent Record

### Agent Model Used

_TBD_

### Debug Log References

_None_

### Completion Notes List

_TBD_

### File List

_TBD_
