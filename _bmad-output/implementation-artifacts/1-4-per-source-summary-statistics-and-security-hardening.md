# Story 1.4: Per-Source Summary Statistics and Security Hardening

Status: done

## Story

As a user,
I want to see how many videos each source found in the processing summary,
So that I can understand which sources are contributing value.

## Acceptance Criteria

1. Each active source shows its video count in the processing summary (e.g., "TMDB: 42 videos found")
2. The total discovered video count is displayed in the summary (pre-dedup raw total; true "unique" count requires Epic 7 deduplication)
3. API keys and OAuth tokens are never logged to stdout or stderr, even in verbose mode (NFR17)
4. `config.cfg` file permissions are set to 600 on Unix systems when written (NFR16)
5. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Accumulate per-source video counts in `ProcessingSummary` (AC: #1, #2)
  - [x] 1.1 Add `source_totals: HashMap<Source, usize>` field to `ProcessingSummary` in `src/orchestrator.rs`
  - [x] 1.2 Add `add_source_results(results: &[SourceResult])` method to `ProcessingSummary` that merges `videos_found` counts into `source_totals` (accumulates across multiple movies/series)
  - [x] 1.3 Call `summary.add_source_results(&source_results)` from `process_movie_standalone()` — the `source_results` are already available from `ctx.discovery.discover_all(&movie).await`; pass them back through `MovieResult` (see Task 2)
  - [x] 1.4 Call `summary.add_source_results(&source_results)` from `process_series_standalone()` — the `source_results` are already available from `discover_series_content()`; pass them back through `SeriesResult` (see Task 2)
  - [x] 1.5 Update `ProcessingSummary::new()` to initialize `source_totals: HashMap::new()`

- [x] Task 2: Thread `source_results` back through `MovieResult` and `SeriesResult` (AC: #1)
  - [x] 2.1 Add `source_results: Vec<SourceResult>` field to `MovieResult` struct
  - [x] 2.2 Update `MovieResult::success()` and `MovieResult::failed()` constructors to accept `source_results: Vec<SourceResult>` — pass `vec![]` for failed results where discovery didn't run
  - [x] 2.3 Update `add_movie_result()` on `ProcessingSummary` to call `self.add_source_results(&result.source_results)`
  - [x] 2.4 Add `source_results: Vec<SourceResult>` field to `SeriesResult` struct
  - [x] 2.5 Update `SeriesResult::success()` and `SeriesResult::failed()` constructors similarly
  - [x] 2.6 Update `add_series_result()` on `ProcessingSummary` to call `self.add_source_results(&result.source_results)`
  - [x] 2.7 Update all call sites of `MovieResult::success()`, `MovieResult::failed()`, `SeriesResult::success()`, `SeriesResult::failed()` in `src/orchestrator.rs` to pass the appropriate `source_results`

- [x] Task 3: Display per-source stats in `display_summary()` (AC: #1, #2)
  - [x] 3.1 Update `display_summary()` signature in `src/output.rs` to accept `summary: &ProcessingSummary` (no change — it already does; the new `source_totals` field is read from it)
  - [x] 3.2 After the existing downloads/conversions lines, add a "Sources" section that iterates `summary.source_totals` sorted by source tier (Tier 1 first, then Tier 2, then Tier 3) and prints each source with its total video count
  - [x] 3.3 Skip sources with 0 videos found (don't clutter the summary with inactive sources)
  - [x] 3.4 Display format: `  TMDB:        42 videos`  (right-aligned source name, consistent column width)
  - [x] 3.5 Add a "Total Discovered:" line showing `summary.total_videos_discovered` (see 3.6) — this is the pre-dedup raw total; label it "Total Discovered" not "Total unique" since dedup doesn't exist yet (Epic 7)
  - [x] 3.6 Add `total_videos_discovered: usize` field to `ProcessingSummary`; populate it in `add_source_results()` as the sum of all `videos_found` across all sources (pre-dedup total, for informational display)

- [x] Task 4: Enforce NFR17 — API keys never logged (AC: #3)
  - [x] 4.1 Audit all `log::debug!`, `log::info!`, `log::warn!`, `log::error!` calls in `src/config.rs`, `src/validation.rs`, `src/discovery/tmdb.rs`, `src/discovery/series_tmdb.rs`, `src/discovery/tvdb.rs`, `src/discovery/id_bridge.rs` for any interpolation of `tmdb_api_key`, `tvdb_api_key`, or token strings
  - [x] 4.2 Replace any log statements that include API key values with redacted versions (e.g., log the key length or first 4 chars + `***` instead of the full value)
  - [x] 4.3 Audit `src/main.rs` — confirm `tmdb_api_key` and `tvdb_api_key` are never passed to `eprintln!` or `println!`
  - [x] 4.4 Add a compile-time note (doc comment) on `Config.tmdb_api_key` and `Config.tvdb_api_key` fields warning that these must never be logged

- [x] Task 5: Enforce NFR16 — `config.cfg` file permissions 600 on Unix (AC: #4)
  - [x] 5.1 In `Config::save()` in `src/config.rs`, after `fs::write(path, contents)` succeeds, set file permissions to 0o600 on Unix using `#[cfg(unix)]` and `std::os::unix::fs::PermissionsExt`
  - [x] 5.2 Use `fs::set_permissions(path, Permissions::from_mode(0o600))` — import `std::os::unix::fs::PermissionsExt` inside the `#[cfg(unix)]` block to avoid Windows compile errors
  - [x] 5.3 If `set_permissions` fails, log a `warn!` (not an error — the file was written successfully, permissions are best-effort)
  - [x] 5.4 On Windows (`#[cfg(not(unix))]`), no action needed — Windows uses ACLs, not Unix permissions

- [x] Task 6: Update tests (AC: #5)
  - [x] 6.1 Update `ProcessingSummary` construction in all tests that construct it directly (in `src/orchestrator.rs` and `tests/`) to include `source_totals: HashMap::new()` and `total_videos_discovered: 0`
  - [x] 6.2 Update `MovieResult::success()` / `MovieResult::failed()` call sites in tests to pass `vec![]` for `source_results`
  - [x] 6.3 Add unit test: `add_source_results()` accumulates counts correctly across multiple calls
  - [x] 6.4 Add unit test: `display_summary()` does not panic when `source_totals` is empty
  - [x] 6.5 Add unit test: `display_summary()` does not panic when `source_totals` has entries
  - [x] 6.6 Add unit test: `Config::save()` sets file permissions to 600 on Unix (use `tempfile`, check `metadata().permissions().mode()`)
  - [x] 6.7 Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`

## Dev Notes

### What Already Exists (Do NOT Redo)

- `SourceResult` struct is already defined in `src/discovery/orchestrator.rs` and re-exported from `src/discovery/mod.rs` as `pub use orchestrator::SourceResult`
- `discover_all()` already returns `(Vec<VideoSource>, Vec<SourceResult>)` — the `source_results` are already captured in `process_movie_standalone()` (used for dry-run display) and in `discover_series_content()`
- `display_dry_run_movie_results()` and `display_dry_run_series_results()` already show per-source breakdowns inline during processing — Story 1.4 adds the aggregate totals to the final summary, not inline display
- `Source` enum already has a `tier()` method returning 1, 2, or 3 — use this for sorting in the summary display
- `check_ytdlp_version()` and `check_ffmpeg_version()` already exist in `src/validation.rs` and emit `warn!` for outdated versions — these are already non-fatal warnings, which is correct per Story 1.1's implementation

### Key Code Locations

| What | File | Current State |
|---|---|---|
| `ProcessingSummary` | `src/orchestrator.rs` | 8 fields, no `source_totals` or `total_videos_discovered` |
| `MovieResult` | `src/orchestrator.rs` | No `source_results` field |
| `SeriesResult` | `src/orchestrator.rs` | No `source_results` field |
| `add_movie_result()` | `src/orchestrator.rs` | Does not call `add_source_results()` |
| `process_movie_standalone()` | `src/orchestrator.rs` | `source_results` captured, used for dry-run only |
| `discover_series_content()` | `src/orchestrator.rs` | Returns `source_results` but caller discards them |
| `display_summary()` | `src/output.rs` | No per-source section |
| `Config::save()` | `src/config.rs` | No permission setting after write |
| `Config.tmdb_api_key` | `src/config.rs` | No doc warning about logging |

### `MovieResult` Constructor Change

Currently:
```rust
fn success(movie: MovieEntry, downloads: usize, conversions: usize) -> Self
fn failed(movie: MovieEntry, phase: &str, error: String) -> Self
```

After this story:
```rust
fn success(movie: MovieEntry, downloads: usize, conversions: usize, source_results: Vec<SourceResult>) -> Self
fn failed(movie: MovieEntry, phase: &str, error: String) -> Self  // source_results defaults to vec![]
```

For `failed()`, pass `source_results: vec![]` — when a movie fails early (e.g., during scanning or before discovery), there are no source results to report. This is correct behavior.

For the dry-run early return in `process_movie_standalone()`:
```rust
if ctx.dry_run {
    let total = source_results.iter().map(|sr| sr.videos_found).sum();
    output::display_dry_run_movie_results(&movie, &source_results, total);
    return MovieResult::success(movie, 0, 0, source_results);  // pass source_results through
}
```

### `discover_series_content()` Source Results

Currently `discover_series_content()` returns `(Vec<SeriesExtra>, bool)`. The `source_results` from `series_discovery.discover_all()` and `series_discovery.discover_season_extras()` are already captured inside the function but not returned.

Extend the return type to `(Vec<SeriesExtra>, bool, Vec<SourceResult>)` and collect all source results (series-level + all season-level) into a single `Vec<SourceResult>` to return. Update the caller in `process_series_standalone()` to destructure the new tuple.

### `ProcessingSummary` HashMap Import

Add `use std::collections::HashMap;` at the top of `src/orchestrator.rs` if not already present. The `source_totals` field uses `HashMap<Source, usize>`.

`Source` is already imported in `orchestrator.rs` via `use crate::models::Source` (or similar) — verify the exact import path before writing.

### Sorting Sources in Summary Display

`Source` does not implement `Ord`. Sort by tier then by display name for deterministic output:
```rust
let mut entries: Vec<(&Source, &usize)> = summary.source_totals.iter().collect();
entries.sort_by_key(|(s, _)| (s.tier(), s.to_string()));
```

Only display entries where `*count > 0`.

### Summary Display Format

Add a "Discovery" section between the existing downloads/conversions lines and the separator:
```
  Total Downloads:    15
  Total Conversions:  12
  ─────────────────────────────────────────────────────────
  Discovery:
    TMDB:             42 videos
    Archive:           3 videos
    YouTube:          18 videos
  Total Discovered:   63 videos
══════════════════════════════════════════════════════════
```

Use a thinner separator (`─`) for the inner section break to visually distinguish it from the outer `═` border. Keep the format consistent with the existing `bright_white()` / `bright_cyan()` / `bright_yellow()` color scheme.

Note: The PRD's summary format example also shows a "Duplicates: 12 removed (tier dedup)" line in this block. That line will be added in Story 7.2 when `duplicates_removed` is added to `ProcessingSummary`. Design the display function so the "Duplicates" line can be inserted between the per-source lines and the "Total Discovered" line without restructuring — e.g., check `summary.duplicates_removed > 0` when that field exists. For now, no "Duplicates" line is shown.

### NFR17: API Key Audit Scope

The most likely places where keys could leak into logs:
- `src/discovery/tmdb.rs` — URL construction logs (check if `api_key` appears in logged URLs)
- `src/discovery/series_tmdb.rs` — same pattern as `tmdb.rs`: constructs TMDB API URLs with `?api_key={key}` in query params; audit all `debug!`/`info!` calls that log URLs
- `src/discovery/tvdb.rs` — Bearer token in Authorization header (never log the header value)
- `src/discovery/id_bridge.rs` — Any debug logs during ID resolution (uses TMDB external_ids endpoint with `api_key` param)
- `src/config.rs` — `load_or_create()` logs the config path, not the key — verify

TMDB API key appears in query parameters: `?api_key={key}`. If any `debug!` or `info!` logs the full URL, the key is exposed. Replace with a sanitized URL (strip the `api_key` param from logged URLs) or log only the path without query params.

### NFR16: Unix Permissions Implementation

```rust
pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
    let contents = serde_json::to_string_pretty(self).map_err(ConfigError::SerializeError)?;
    fs::write(path, contents).map_err(|e| ConfigError::WriteError(path.to_path_buf(), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        if let Err(e) = std::fs::set_permissions(path, permissions) {
            log::warn!("Could not set config file permissions to 600: {}", e);
        }
    }

    Ok(())
}
```

The `#[cfg(unix)]` block compiles only on Unix targets. On Windows, the block is omitted entirely — no dead code warning.

### `total_videos_discovered` vs `source_totals` Sum

`total_videos_discovered` is the sum of all `videos_found` across all `SourceResult`s across all movies/series. This is the pre-dedup, pre-limit raw count. It gives users a sense of how much content was found before filtering. It is NOT the number of videos downloaded (that's `total_downloads`).

`add_source_results()` implementation:
```rust
fn add_source_results(&mut self, results: &[SourceResult]) {
    for sr in results {
        *self.source_totals.entry(sr.source.clone()).or_insert(0) += sr.videos_found;
        self.total_videos_discovered += sr.videos_found;
    }
}
```

`Source` must implement `Clone` for `HashMap` key insertion. Verified: `Source` already derives `Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum` — all traits needed for `HashMap<Source, usize>` are present. No changes to the `Source` enum are needed.

### Regression Risks

- Adding `source_results: Vec<SourceResult>` to `MovieResult` and `SeriesResult` changes their constructors — all call sites in `src/orchestrator.rs` must be updated
- Adding `source_totals` and `total_videos_discovered` to `ProcessingSummary` breaks all direct struct construction in tests — search for `ProcessingSummary {` in `src/orchestrator.rs`, `tests/main_integration_tests.rs`, and `tests/series_integration_tests.rs`
- `discover_series_content()` return type change from `(Vec<SeriesExtra>, bool)` to `(Vec<SeriesExtra>, bool, Vec<SourceResult>)` — only one caller in `process_series_standalone()`
- `display_summary()` in `src/output.rs` now reads `source_totals` — tests that call `display_summary()` directly must construct `ProcessingSummary` with the new fields

### Constructor Call Sites to Update

**`MovieResult::success()` — 6 production + 5 test call sites:**
- `process_movie_standalone()` line ~484 (dry-run early return) — pass `source_results`
- `process_movie_standalone()` line ~494 (empty sources) — pass `source_results`
- `process_movie_standalone()` line ~516 (no successful downloads) — pass `source_results`
- `process_movie_standalone()` line ~536 (no successful conversions) — pass `source_results`
- `process_movie_standalone()` line ~550 (success) — pass `source_results`
- `process_movie_standalone()` line ~554 (organization failed) — pass `source_results`
- Tests: `test_processing_summary_add_successful_result`, `test_processing_summary_add_failed_result`, `test_movie_result_success`, `test_movie_result_failed`, `test_movie_result_display`, `test_processing_summary_aggregation` (2 calls)

**`MovieResult::failed()` — 1 production + 4 test call sites (see above)**

**`SeriesResult::success()` — 4 production + 2 test call sites:**
- `process_series_standalone()` line ~664 (dry-run early return) — pass `series_source_results`
- `process_series_standalone()` line ~674 (no extras found) — pass `series_source_results`
- `process_series_standalone()` line ~693 (no successful conversions) — pass `series_source_results`
- `process_series_standalone()` line ~720 (success) — pass `series_source_results`
- Tests: `test_series_result_success`, `test_processing_summary_add_series_result`

**`SeriesResult::failed()` — 1 production + 1 test call site:**
- `process_series_standalone()` line ~716 (organization failed) — pass `series_source_results`
- Test: `test_series_result_failed`

### What NOT To Do

- Do NOT add per-source stats to the inline processing output (that's already done by `display_dry_run_*` functions in Story 1.3) — Story 1.4 is about the final aggregate summary only
- Do NOT add a new `--stats` flag — per-source stats are always shown in the summary
- Do NOT change `SourceResult` struct — it already has the right shape (`source`, `videos_found`, `error`)
- Do NOT implement deduplication counting here — that's Epic 7 (Story 7.2 adds `duplicates_removed` to `ProcessingSummary`)
- Do NOT change `check_ytdlp_version()` or `check_ffmpeg_version()` — they already emit `warn!` for outdated versions, which is the correct non-fatal behavior per Story 1.1

### Dry-Run and `display_summary()` Interaction

`display_summary()` IS called in dry-run mode. The dry-run short-circuit happens inside `process_movie_standalone()` / `process_series_standalone()` (after discovery, before download). But `Orchestrator::run()` still returns a `ProcessingSummary`, and `main.rs` calls `display_summary(&summary)` unconditionally. This means:

- In dry-run mode, `source_totals` will be populated from discovery results (via `MovieResult` / `SeriesResult` carrying `source_results`)
- `total_downloads` and `total_conversions` will be 0
- The per-source "Discovery" section in the summary will show real discovery counts even in dry-run — this is correct and consistent with FR32 ("System displays per-source discovery results in dry-run mode")
- The inline `display_dry_run_movie_results()` / `display_dry_run_series_results()` show per-item breakdowns during processing; the summary shows aggregate totals at the end — both are useful and complementary

### Previous Story Learnings (from 1.3)

- When adding fields to `OrchestratorConfig`/`CliConfig`/`ProcessingSummary`, search for ALL direct struct construction sites — the regression risk is high and easy to miss
- `MovieProcessingContext` was introduced in Story 1.3 to bundle movie processing dependencies — follow the same pattern if adding new context to movie processing
- Tests in `src/orchestrator.rs` use a `test_config()` helper function — update it when adding new fields to `OrchestratorConfig`
- The `#[cfg(test)]` blocks are co-located within each module file, not in separate test files (except `tests/` integration tests)
- Story 1.3 added 9 tests; expect a similar count here (6-8 new tests)

### Previous Story Learnings (from 1.2)

- `SourceResult` is defined in `src/discovery/orchestrator.rs` and re-exported from `src/discovery/mod.rs` — import via `crate::discovery::SourceResult` in `src/orchestrator.rs`
- The series path goes through `discover_series_content()` (a free-standing async fn in `src/orchestrator.rs`), not directly through `process_series_standalone()` — the source results must be threaded through this intermediate function
- `SeriesDiscoveryOrchestrator::discover_all()` and `discover_season_extras()` both return `(Vec<SeriesExtra>, Vec<SourceResult>)` — both sets of results should be merged when returning from `discover_series_content()`

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 1, Story 1.4]
- [Source: _bmad-output/planning-artifacts/prd.md — FR36, NFR16, NFR17]
- [Source: src/orchestrator.rs — ProcessingSummary, MovieResult, SeriesResult, process_movie_standalone(), discover_series_content()]
- [Source: src/output.rs — display_summary()]
- [Source: src/config.rs — Config::save()]
- [Source: src/discovery/orchestrator.rs — SourceResult]
- [Source: _bmad-output/implementation-artifacts/1-3-dry-run-mode.md — completion notes, regression patterns]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

_None_

### Completion Notes List

- Added `source_totals: HashMap<Source, usize>` and `total_videos_discovered: usize` to `ProcessingSummary` with `#[derive(Default)]` per clippy recommendation
- Added `add_source_results(&mut self, results: &[SourceResult])` method that accumulates per-source video counts and total discovered count
- Added `source_results: Vec<SourceResult>` field to both `MovieResult` and `SeriesResult`; updated `success()` constructors to accept 4th param; `failed()` defaults to `vec![]`
- Updated all 6 production call sites in `process_movie_standalone()` and all 6 in `process_series_standalone()` to pass `source_results` through
- Updated `add_movie_result()` and `add_series_result()` to call `self.add_source_results()`
- Added "Discovery" section to `display_summary()` in `src/output.rs` — sources sorted by tier then name, 0-count sources skipped, "Total Discovered" line at end
- NFR17 audit: confirmed no API keys are logged anywhere in production code; added SECURITY doc comments on `Config.tmdb_api_key` and `Config.tvdb_api_key`
- NFR16: added `#[cfg(unix)]` block in `Config::save()` to set 0o600 permissions after write, with `warn!` on failure
- Added 7 new tests: `test_add_source_results_accumulates_correctly`, `test_add_movie_result_merges_source_results`, `test_add_series_result_merges_source_results`, `test_failed_result_has_empty_source_results`, `test_display_summary_with_empty_source_totals`, `test_display_summary_with_source_totals`, `test_config_save_sets_unix_permissions` (cfg(unix) only)
- Updated all existing `ProcessingSummary` direct constructions and `MovieResult`/`SeriesResult` call sites in tests to include new fields
- Quality gate: `cargo build` ✅, `cargo test` (515 passed, 0 failed) ✅, `cargo clippy -- -D warnings` ✅, `cargo fmt -- --check` ✅

### File List

- `src/orchestrator.rs` — Added `source_totals`, `total_videos_discovered` to `ProcessingSummary`; added `source_results` to `MovieResult`/`SeriesResult`; updated all constructors and call sites; added `add_source_results()` method; added 4 new tests; updated all existing test constructions
- `src/output.rs` — Added per-source "Discovery" section to `display_summary()`; added `Source` import; added 2 new tests; updated 6 existing test `ProcessingSummary` constructions
- `src/config.rs` — Added `#[cfg(unix)]` permissions block in `Config::save()`; added SECURITY doc comments on API key fields; added 1 new test (cfg(unix))

## Change Log

- 2026-03-24: Implemented Story 1.4 — per-source summary statistics (Tasks 1-3), NFR17 API key audit (Task 4), NFR16 Unix file permissions (Task 5), and tests (Task 6). All quality gates pass.
- 2026-03-24: Applied all 6 code review patch findings. Fixed TOCTOU in `Config::save()` (OpenOptions + mode 0o600 + set_permissions for existing files), preserved source_results through org-failure paths, made `source_totals`/`total_videos_discovered` pub(crate), uppercased source names in summary display, show Discovery section even when all counts are 0, replaced bare `.unwrap()` with `.expect()` in Unix permissions test. All quality gates pass (515 tests, 0 failed).

### Review Findings

- [x] [Review][Patch] TOCTOU: config file world-readable between write and chmod [src/config.rs:Config::save] — `fs::write` creates the file with default umask (typically 0o644) before `set_permissions` is called; API keys are transiently world-readable. Fix: open the file with `OpenOptions` + `mode(0o600)` via `OpenOptionsExt` so it is never created world-readable.
- [x] [Review][Patch] Source results dropped on organization failure — summary undercounts discovered videos [src/orchestrator.rs:process_movie_standalone, process_series_standalone] — `MovieResult::failed()` and `SeriesResult::failed()` always set `source_results: vec![]`; when organization fails after successful discovery the per-source counts are silently lost. Fix: add `source_results` parameter to `failed()` constructors and pass the real results at the organization failure call sites.
- [x] [Review][Patch] `source_totals` public field can diverge from `total_videos_discovered` [src/orchestrator.rs:ProcessingSummary] — `source_totals` is `pub`, so external code can mutate it without updating `total_videos_discovered`, causing the "Total Discovered" line to not match the per-source sum. Fix: make `source_totals` and `total_videos_discovered` private (or `pub(crate)`) and expose only `add_source_results` for mutation.
- [x] [Review][Patch] Source names display lowercase in summary, spec requires uppercase [src/output.rs:display_summary] — `format!("{:>12}:", source)` calls `source.to_string()` which returns lowercase (`"tmdb"`, `"youtube"`, `"archive"`); AC1 example shows `TMDB:`. Fix: use `source.to_string().to_uppercase()` or implement `Display` to return uppercase.
- [x] [Review][Patch] Discovery section hidden when all sources return 0 videos [src/output.rs:display_summary] — `filter(count > 0)` + `is_empty()` omits the entire Discovery section when all sources found 0 results; operator cannot distinguish "discovery ran and found nothing" from "discovery was never called". Fix: show the section (with 0 counts) whenever `source_totals` is non-empty, regardless of counts.
- [x] [Review][Patch] Test uses `.unwrap()` instead of `.expect()` with message [src/config.rs:test_config_save_sets_unix_permissions] — `TempDir::new().unwrap()` and `config.save(&config_path).unwrap()` violate the project's no-bare-unwrap standard. Fix: replace with `.expect("descriptive message")`.
- [x] [Review][Defer] Download/conversion counts zeroed on organization failure [src/orchestrator.rs:MovieResult::failed, SeriesResult::failed] — pre-existing design; `failed()` always zeros downloads/conversions regardless of phase. Not introduced by this story. — deferred, pre-existing
- [x] [Review][Defer] Parallel task panic silently drops result [src/orchestrator.rs:process_movies_parallel, process_series_parallel] — `if let Ok(result) = task.await` silently discards panicking tasks; pre-existing pattern not introduced by this story. — deferred, pre-existing
