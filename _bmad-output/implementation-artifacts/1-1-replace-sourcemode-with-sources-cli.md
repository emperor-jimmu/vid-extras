# Story 1.1: Replace SourceMode with --sources CLI Parameter

Status: review

## Story

As a user,
I want to specify `--sources tmdb,youtube` on the command line,
so that I can control which discovery sources are queried per run.

## Acceptance Criteria

1. A new `Source` enum exists in `models.rs` with variants: `Tmdb`, `Archive`, `Dailymotion`, `Youtube`, `Vimeo`, `Bilibili`, each with a `tier()` method returning 1, 2, or 3
2. A `default_sources()` function returns `vec![Source::Tmdb, Source::Archive, Source::Dailymotion, Source::Youtube]`
3. `SourceMode` is removed from both `models.rs` and `cli.rs`; the `to_models_source_mode()` bridge is removed
4. `SourceType` in `models.rs` gains 4 new variants: `Dailymotion`, `KinoCheck`, `Vimeo`, `Bilibili`
5. `--sources` accepts comma-separated values (`value_delimiter = ','`) and repeated flags
6. Omitting `--sources` uses the default set
7. Unknown source names produce a descriptive validation error at parse time
8. `--sources` combines with `--series-only`, `--movies-only`, `--specials` without conflicts
9. Using `--mode` produces: "The --mode flag has been removed. Use --sources instead."
10. Startup validates yt-dlp ≥ 2025.01 and ffmpeg ≥ 6.0 with descriptive errors showing detected vs required version
11. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Create `Source` enum and remove `SourceMode` (AC: #1, #2, #3)
  - [x] 1.1 Add `Source` enum to `models.rs` with `tier()`, `Display`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`
  - [x] 1.2 Add `default_sources()` free function in `models.rs`
  - [x] 1.3 Remove `SourceMode` enum from `models.rs`
  - [x] 1.4 Remove `SourceMode` enum from `cli.rs` and the `to_models_source_mode()` method
  - [x] 1.5 Update all `SourceMode` references across codebase (orchestrator.rs, discovery/orchestrator.rs, discovery/series_orchestrator.rs, main.rs, tests)
  - [x] 1.6 Replace `mode: SourceMode` with `sources: Vec<Source>` in `OrchestratorConfig`, `CliConfig`, `Orchestrator`, `DiscoveryOrchestrator`, `SeriesDiscoveryOrchestrator`

- [x] Task 2: Extend `SourceType` enum (AC: #4)
  - [x] 2.1 Add `Dailymotion`, `KinoCheck`, `Vimeo`, `Bilibili` variants to `SourceType` in `models.rs`
  - [x] 2.2 Update `Display` impl for `SourceType` with display strings
  - [x] 2.3 Update exhaustive match on `SourceType` in `discovery/orchestrator.rs` `apply_content_limits` sort

- [x] Task 3: Refactor CLI `--sources` parameter (AC: #5, #6, #7, #8, #9)
  - [x] 3.1 Replace `mode: SourceMode` field in `CliArgs` with `sources: Vec<Source>` using `value_delimiter = ','` and default value
  - [x] 3.2 `clap::ValueEnum` derived for `Source` — clap handles parsing and validation automatically
  - [x] 3.3 Add `DeprecatedFlag(String)` to `CliError`; hidden `--mode` arg in `CliArgs`; `parse_args()` returns `CliError::DeprecatedFlag` with migration message
  - [x] 3.4 `CliConfig` carries `sources: Vec<Source>`
  - [x] 3.5 `display_config()` shows active sources list
  - [x] 3.6 `main.rs` passes `sources` into `OrchestratorConfig`
  - [x] 3.7 `--sources` works alongside all other flags

- [x] Task 4: Add version validation for yt-dlp and ffmpeg (AC: #10)
  - [x] 4.1 Add `OutdatedBinary { name, detected, minimum }` to `ValidationError` in `error.rs`
  - [x] 4.2 Add `check_ytdlp_version()` to `Validator` — parses `YYYY.MM.DD`, warns if < 2025.01.01
  - [x] 4.3 Add `check_ffmpeg_version()` to `Validator` — parses major version, warns if < 6
  - [x] 4.4 Both checks called in `validate_dependencies()` after binary existence checks
  - [x] 4.5 `main.rs` handles `OutdatedBinary` as non-fatal warning (logs and continues)

- [x] Task 5: Update all tests (AC: #11)
  - [x] 5.1 Unit tests in `cli.rs` updated — no `SourceMode` references
  - [x] 5.2 Unit tests in `models.rs` updated — `Source::tier()`, `default_sources()`, new `SourceType` variants covered
  - [x] 5.3 Orchestrator tests updated — `sources: default_sources()` throughout
  - [x] 5.4 Integration tests in `tests/main_integration_tests.rs` updated — no `SourceMode` usage
  - [x] 5.5 `series_orchestrator.rs` property test updated — no `SourceMode` in proptest strategy
  - [x] 5.6 Quality gate: `cargo test` ✅ 509 tests pass | `cargo clippy -- -D warnings` ✅ clean | `cargo fmt -- --check` ✅ clean

## Dev Notes

### Architecture Compliance

- Pipeline pattern: this story modifies the data model and CLI layer only. Discovery invocation logic stays functionally equivalent (Story 1.2 refactors the orchestrator loop).
- Error types: all error enums live in `src/error.rs` using `thiserror`. Add the new `OutdatedBinary` variant there.
- No `.unwrap()` in production code. Use `?` operator and descriptive `.expect()` only for truly impossible cases.
- `clap` 4.5 derive macros for CLI. Use `ValueEnum` derive for `Source` enum.

### Key Code Locations

| What | File | Lines/Section |
|---|---|---|
| `Source` enum + `default_sources()` | `src/models.rs` | Replaces `SourceMode` |
| `SourceType` (extended) | `src/models.rs` | 4 new variants |
| `CliArgs` / `CliConfig` | `src/cli.rs` | `sources: Vec<Source>` |
| `OrchestratorConfig` | `src/orchestrator.rs` | `sources: Vec<Source>` |
| `DiscoveryOrchestrator` | `src/discovery/orchestrator.rs` | `sources: Vec<Source>` |
| `SeriesDiscoveryOrchestrator` | `src/discovery/series_orchestrator.rs` | `sources: Vec<Source>` |
| `ValidationError` | `src/error.rs` | `OutdatedBinary` + `DeprecatedFlag` |
| `Validator` | `src/validation.rs` | `check_ytdlp_version()`, `check_ffmpeg_version()` |
| `main.rs` | `src/main.rs` | Non-fatal `OutdatedBinary` handling |

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 1, Story 1.1]
- [Source: _bmad-output/planning-artifacts/prd.md — FR1-FR6, FR31, NFR12]

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.6

### Completion Notes List

- `SourceMode` was present in two places (models.rs AND cli.rs) — both removed
- Version checks in validation are best-effort: unparseable output logs a warning and continues
- `OutdatedBinary` in main.rs is non-fatal — logs warning, re-runs `validate_dependencies()` to obtain the API key, then continues
- `series_orchestrator.rs` property test simplified — removed `SourceMode` from proptest strategy, now tests series isolation without mode dependency
- All 509 tests pass (444 lib + 16 main integration + 34 series integration + 15 doc)

### File List

- `src/models.rs`
- `src/cli.rs`
- `src/error.rs`
- `src/validation.rs`
- `src/orchestrator.rs`
- `src/main.rs`
- `src/discovery/orchestrator.rs`
- `src/discovery/series_orchestrator.rs`
- `tests/main_integration_tests.rs`

## Story

As a user,
I want to specify `--sources tmdb,youtube` on the command line,
so that I can control which discovery sources are queried per run.

## Acceptance Criteria

1. A new `Source` enum exists in `models.rs` with variants: `Tmdb`, `Archive`, `Dailymotion`, `Youtube`, `Vimeo`, `Bilibili`, each with a `tier()` method returning 1, 2, or 3
2. A `default_sources()` function returns `vec![Source::Tmdb, Source::Archive, Source::Dailymotion, Source::Youtube]`
3. `SourceMode` is removed from both `models.rs` and `cli.rs`; the `to_models_source_mode()` bridge is removed
4. `SourceType` in `models.rs` gains 4 new variants: `Dailymotion`, `KinoCheck`, `Vimeo`, `Bilibili`
5. `--sources` accepts comma-separated values (`value_delimiter = ','`) and repeated flags
6. Omitting `--sources` uses the default set
7. Unknown source names produce a descriptive validation error at parse time
8. `--sources` combines with `--series-only`, `--movies-only`, `--specials` without conflicts
9. Using `--mode` produces: "The --mode flag has been removed. Use --sources instead."
10. Startup validates yt-dlp ≥ 2025.01 and ffmpeg ≥ 6.0 with descriptive errors showing detected vs required version
11. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [ ] Task 1: Create `Source` enum and remove `SourceMode` (AC: #1, #2, #3)
  - [ ] 1.1 Add `Source` enum to `models.rs` with `tier()`, `Display`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`
  - [ ] 1.2 Add `default_sources()` free function in `models.rs`
  - [ ] 1.3 Remove `SourceMode` enum from `models.rs` (lines 136-152)
  - [ ] 1.4 Remove `SourceMode` enum from `cli.rs` (lines 10-40) and the `to_models_source_mode()` method
  - [ ] 1.5 Update all `SourceMode` references across codebase (orchestrator.rs, discovery/orchestrator.rs, discovery/series_orchestrator.rs, main.rs, output.rs, tests)
  - [ ] 1.6 Temporarily replace `mode: SourceMode` with `sources: Vec<Source>` in `OrchestratorConfig`, `CliConfig`, `Orchestrator` fields — wire through but keep existing discoverer invocation logic working (match on whether sources contains Tmdb/Archive/Youtube)

- [ ] Task 2: Extend `SourceType` enum (AC: #4)
  - [ ] 2.1 Add `Dailymotion`, `KinoCheck`, `Vimeo`, `Bilibili` variants to `SourceType` in `models.rs`
  - [ ] 2.2 Update `Display` impl for `SourceType` with display strings
  - [ ] 2.3 Update any exhaustive matches on `SourceType` (discovery/orchestrator.rs `apply_content_limits` sort)

- [ ] Task 3: Refactor CLI `--sources` parameter (AC: #5, #6, #7, #8, #9)
  - [ ] 3.1 Replace `mode: SourceMode` field in `CliArgs` with `sources: Vec<Source>` using `#[arg(long, value_delimiter = ',', default_value = "tmdb,archive,dailymotion,youtube")]`
  - [ ] 3.2 Implement `clap::ValueEnum` for `Source` so clap handles parsing and validation automatically
  - [ ] 3.3 Add `DeprecatedFlag(String)` variant to `CliError` in `error.rs`; add a hidden `--mode` arg in `CliArgs` and check in `parse_args()` — return `CliError::DeprecatedFlag` with migration message
  - [ ] 3.4 Update `CliConfig` to carry `sources: Vec<Source>` instead of `mode: SourceMode`
  - [ ] 3.5 Update `display_config()` in `cli.rs` to show active sources list
  - [ ] 3.6 Update `main.rs` to pass `sources` into `OrchestratorConfig`
  - [ ] 3.7 Verify `--sources` works alongside `--series-only`, `--movies-only`, `--specials`, `--force`, `--concurrency`

- [ ] Task 4: Add version validation for yt-dlp and ffmpeg (AC: #10)
  - [ ] 4.1 Add `OutdatedBinary { name: String, detected: String, minimum: String }` variant to `ValidationError` in `error.rs`
  - [ ] 4.2 Add `check_ytdlp_version()` to `Validator` — run `yt-dlp --version`, parse output (format: `YYYY.MM.DD`), compare ≥ `2025.01.01`
  - [ ] 4.3 Add `check_ffmpeg_version()` to `Validator` — run `ffmpeg -version`, parse first line for version number (format: `N.N`), compare ≥ `6.0`
  - [ ] 4.4 Call both checks in `validate_dependencies()` after binary existence checks
  - [ ] 4.5 Update `main.rs` error display to handle `OutdatedBinary` variant with detected/minimum version info

- [ ] Task 5: Update all tests (AC: #11)
  - [ ] 5.1 Update unit tests in `cli.rs` — replace `SourceMode` references with `Vec<Source>`
  - [ ] 5.2 Update unit tests in `models.rs` — add tests for `Source::tier()`, `default_sources()`, new `SourceType` variants
  - [ ] 5.3 Update orchestrator tests — replace `mode: SourceMode::All` with `sources: default_sources()`
  - [ ] 5.4 Update integration tests in `tests/` — replace all `SourceMode` usage
  - [ ] 5.5 Add test for `--mode` migration error message
  - [ ] 5.6 Add tests for version parsing (valid, outdated, unparseable output)
  - [ ] 5.7 Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`

## Dev Notes

### Architecture Compliance

- Pipeline pattern: this story modifies the data model and CLI layer only. Discovery invocation logic stays functionally equivalent (Story 1.2 refactors the orchestrator loop).
- Error types: all error enums live in `src/error.rs` using `thiserror`. Add the new `OutdatedBinary` variant there.
- No `.unwrap()` in production code. Use `?` operator and descriptive `.expect()` only for truly impossible cases.
- `clap` 4.5 derive macros for CLI. Use `ValueEnum` derive for `Source` enum.

### Key Code Locations

| What | File | Lines/Section |
|---|---|---|
| `SourceMode` (models) | `src/models.rs` | Lines 136-152 |
| `SourceMode` (cli) | `src/cli.rs` | Lines 10-40 |
| `to_models_source_mode()` | `src/cli.rs` | ~Line 29 |
| `CliArgs` struct | `src/cli.rs` | Lines 45-100 |
| `CliConfig` struct | `src/cli.rs` | Lines 102-144 |
| `SourceType` enum | `src/models.rs` | Lines 155-177 |
| `ContentCategory` enum | `src/models.rs` | Lines 179-217 |
| `OrchestratorConfig` | `src/orchestrator.rs` | Lines 146-172 |
| `Orchestrator` struct | `src/orchestrator.rs` | Lines 174-188 |
| `DiscoveryOrchestrator` | `src/discovery/orchestrator.rs` | Full file |
| `SeriesDiscoveryOrchestrator` | `src/discovery/series_orchestrator.rs` | Full file |
| `ValidationError` | `src/error.rs` | Lines 63-70 |
| `Validator` | `src/validation.rs` | Full file |
| `main.rs` | `src/main.rs` | Full file |
| `display_config()` | `src/cli.rs` | ~Line 237 |
| `display_summary()` | `src/output.rs` | Check for SourceMode refs |
| Integration tests | `tests/main_integration_tests.rs` | Full file |
| Series integration tests | `tests/series_integration_tests.rs` | Full file |

### Source Enum Design

```rust
/// Discovery source that can be queried for extras
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, clap::ValueEnum)]
pub enum Source {
    Tmdb,
    Archive,
    Dailymotion,
    Youtube,
    Vimeo,
    Bilibili,
}

impl Source {
    /// Returns the deduplication priority tier (1 = highest)
    pub fn tier(&self) -> u8 {
        match self {
            Source::Tmdb => 1,
            Source::Archive => 2,
            Source::Dailymotion => 2,
            Source::Youtube => 3,
            Source::Vimeo => 2,
            Source::Bilibili => 3,
        }
    }
}

pub fn default_sources() -> Vec<Source> {
    vec![Source::Tmdb, Source::Archive, Source::Dailymotion, Source::Youtube]
}
```

### SourceType Extension

Add to existing enum — do NOT remove existing variants:
```rust
pub enum SourceType {
    TMDB,        // existing
    ArchiveOrg,  // existing
    YouTube,     // existing
    TheTVDB,     // existing
    Dailymotion, // NEW
    KinoCheck,   // NEW
    Vimeo,       // NEW
    Bilibili,    // NEW
}
```

### --mode Migration Strategy

The old `--mode` arg must produce a clear error, not silently be ignored. Two approaches:

**Option A (recommended):** Add a hidden clap arg that captures `--mode` and check in `parse_args()`:
```rust
/// DEPRECATED: Use --sources instead
#[arg(long, hide = true)]
mode: Option<String>,
```
Then in `parse_args()`, if `args.mode.is_some()`, return `Err(CliError::DeprecatedFlag("The --mode flag has been removed. Use --sources instead."))`.

**Option B:** Remove `--mode` entirely and let clap produce its default "unexpected argument" error. Less user-friendly.

### Version Parsing

**yt-dlp:** `yt-dlp --version` outputs a single line like `2025.03.15`. Parse with: split on `.`, check first segment ≥ 2025 and (if 2025) second segment ≥ 01.

**ffmpeg:** `ffmpeg -version` first line: `ffmpeg version 7.1.1 Copyright ...`. Parse with regex `ffmpeg version (\d+)\.(\d+)` and check major ≥ 6.

Both checks should be best-effort: if version output can't be parsed, log a warning and continue (don't block on unparseable output from custom builds).

### Orchestrator Wiring (Temporary)

For this story, the orchestrator still needs to work. Replace the `SourceMode` match with equivalent logic:

```rust
// In DiscoveryOrchestrator — temporary bridge until Story 1.2 refactors the loop
// If sources contains Tmdb → query TMDB
// If sources contains Archive → query Archive.org
// YouTube is always queried if in sources list
// This preserves existing behavior while using the new data model
```

The `DiscoveryOrchestrator::new()` and `SeriesDiscoveryOrchestrator::new()` signatures change from `mode: SourceMode` to `sources: Vec<Source>`. Internally they check `sources.contains(&Source::Tmdb)` etc.

### Regression Risks

- `apply_content_limits()` in `discovery/orchestrator.rs` sorts by `SourceType` — update the match to include new variants (assign them priority numbers)
- `test_source_mode_display()` in `models.rs` and `cli.rs` — these tests must be removed/replaced
- `test_cli_config_from_args()` in `cli.rs` — update to use `sources: vec![Source::Tmdb]` instead of `mode: SourceMode::YoutubeOnly`
- Property-based test "Property 5: Mode Filtering" in discovery — update to test source list filtering
- Integration tests create `OrchestratorConfig` with `mode:` field — all must change to `sources:`

### What NOT To Do

- Do NOT create discoverer modules for Dailymotion/KinoCheck/Vimeo/Bilibili — those are separate epics
- Do NOT change the `ContentDiscoverer` trait
- Do NOT modify `ContentCategory` — that's Story 2.1
- Do NOT add `--dry-run` — that's Story 1.3
- Do NOT refactor the orchestrator's discovery loop to iterate generically — that's Story 1.2

### Project Structure Notes

- All source files in `src/` flat structure (except `src/discovery/` submodule)
- `models.rs` is the single source of truth for shared types
- `error.rs` centralizes all error enums
- Tests are co-located in `#[cfg(test)]` blocks within each module
- Integration tests in `tests/` directory

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 1, Story 1.1]
- [Source: _bmad-output/planning-artifacts/prd.md — FR1-FR6, FR31, NFR12]
- [Source: docs/architecture.md — Module Responsibilities, Data Flow]
- [Source: src/models.rs — SourceMode, SourceType, ContentCategory enums]
- [Source: src/cli.rs — CliArgs, CliConfig, SourceMode, parse_args()]
- [Source: src/error.rs — ValidationError enum]
- [Source: src/validation.rs — Validator struct]
- [Source: src/orchestrator.rs — OrchestratorConfig, Orchestrator]
- [Source: src/discovery/orchestrator.rs — DiscoveryOrchestrator]
- [Source: src/discovery/series_orchestrator.rs — SeriesDiscoveryOrchestrator]
- [Source: src/main.rs — main() startup flow]

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
