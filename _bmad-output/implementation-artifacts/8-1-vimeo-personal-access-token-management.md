# Story 8.1: Vimeo Personal Access Token Management

Status: done

## Story

As a user,
I want to be prompted for my Vimeo Personal Access Token on first use and have it cached,
So that I don't need to re-enter it on every run.

## Acceptance Criteria

1. When the user runs with `--sources vimeo` and no `vimeo_access_token` exists in `config.cfg`, the system prompts the user to generate a PAT at `https://developer.vimeo.com/apps` with `public` scope and enter it interactively (FR37)
2. The token is saved to `config.cfg` as `vimeo_access_token` with file permissions 600 on Unix systems (NFR16) — the existing `Config::save()` already handles this; no new permission logic needed
3. On subsequent runs with `--sources vimeo`, the token is loaded from `config.cfg` without prompting
4. No client_id, client_secret, or token refresh logic is required — the PAT does not expire unless revoked
5. The token is never logged to stdout or stderr, even in verbose mode (NFR17) — do NOT pass the token to any `log::*!` macro or `println!`
6. `Config` struct gains a new optional field `vimeo_access_token: Option<String>` with `#[serde(default)]`
7. `Config::load_or_create_with_vimeo()` is added — loads config, then if `require_vimeo_token` is true and `vimeo_access_token` is `None`, prompts and saves; returns the updated config
8. The stub warning in `DiscoveryOrchestrator::discover_all()` and `SeriesDiscoveryOrchestrator::discover_all()` that logs `"Vimeo source requested but discoverer not yet implemented"` is replaced with a proper `VimeoDiscoverer` that is wired in — but the discoverer itself returns `Ok(vec![])` with an `info!` log until Story 8.2 implements the actual API search
9. `VimeoDiscoverer` is a new struct in `src/discovery/vimeo.rs` with a `new(access_token: String)` constructor and a `discover(&self, title: &str, year: u16) -> Result<Vec<VideoSource>, DiscoveryError>` method that returns `Ok(vec![])` (stub for Story 8.2)
10. `VimeoDiscoverer` is wired into `DiscoveryOrchestrator` and `SeriesDiscoveryOrchestrator` — added as a field, constructed in `new()` and `with_cookies()`, and invoked in `discover_all()` when `Source::Vimeo` is in the active list
11. `SourceType::Vimeo` already exists in `models.rs` — no changes needed there
12. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Add `vimeo_access_token` field to `Config` and add `Config::prompt_for_vimeo_token()` and `Config::load_or_create_with_vimeo()` (AC: #1, #2, #3, #4, #5, #6, #7)
  - [x] 1.1 Add `#[serde(default)] pub vimeo_access_token: Option<String>` to the `Config` struct in `src/config.rs` with a `SECURITY` doc comment matching the pattern on `tmdb_api_key`
  - [x] 1.2 Add `pub fn prompt_for_vimeo_token() -> Result<String, ConfigError>` — mirrors the existing `prompt_for_tvdb_api_key()` pattern
  - [x] 1.3 Add `pub fn load_or_create_with_vimeo(require_vimeo_token: bool) -> Result<Self, ConfigError>`
  - [x] 1.4 Update all existing Config struct literal constructions in `src/config.rs` tests to include `vimeo_access_token: None`
  - [x] 1.5 Add unit tests (see Task 4)

- [x] Task 2: Create `src/discovery/vimeo.rs` with stub `VimeoDiscoverer` (AC: #9, #11)
  - [x] 2.1 Define `pub(crate) struct VimeoDiscoverer { _access_token: String, _client: reqwest::Client }`
  - [x] 2.2 Implement `pub fn new(access_token: String) -> Self`
  - [x] 2.3 Implement `pub async fn discover(&self, title: &str, year: u16) -> Result<Vec<VideoSource>, DiscoveryError>` stub
  - [x] 2.4 Register `mod vimeo;` in `src/discovery/mod.rs`
  - [x] 2.5 Add unit tests (see Task 4)

- [x] Task 3: Wire `VimeoDiscoverer` into both discovery orchestrators (AC: #8, #10)
  - [x] 3.1 Add `vimeo: VimeoDiscoverer` field to `DiscoveryOrchestrator`
  - [x] 3.2 Update `DiscoveryOrchestrator::new()` to accept `vimeo_access_token: String`
  - [x] 3.3 Update `DiscoveryOrchestrator::with_cookies()` to accept `vimeo_access_token: String`
  - [x] 3.4 Replace `Source::Vimeo` stub warning with proper discover block in `DiscoveryOrchestrator::discover_all()`
  - [x] 3.5 Add `vimeo: VimeoDiscoverer` field to `SeriesDiscoveryOrchestrator`
  - [x] 3.6 Update `SeriesDiscoveryOrchestrator::new()` to accept `vimeo_access_token: String`
  - [x] 3.7 Update `SeriesDiscoveryOrchestrator::new_with_tvdb()` to accept `vimeo_access_token: String`
  - [x] 3.8 Replace `Source::Vimeo` stub warning with proper discover block in `SeriesDiscoveryOrchestrator::discover_all()`
  - [x] 3.9 Update `DiscoveryConfig` with `vimeo_access_token: String` field; thread through `Orchestrator::new()`
  - [x] 3.10 Update `src/main.rs` — load Vimeo token via `Config::load_or_create_with_vimeo()`, populate `DiscoveryConfig`
  - [x] 3.11 Update test constructors in `orchestrator.rs` and `series_orchestrator.rs`

- [x] Task 4: Add tests (AC: #1–#12)
  - [x] 4.1 `test_config_vimeo_token_serialization`
  - [x] 4.2 `test_config_vimeo_token_deserialization`
  - [x] 4.3 `test_config_vimeo_token_default_none`
  - [x] 4.4 `test_config_save_and_load_with_vimeo_token`
  - [x] 4.5 `test_vimeo_discoverer_new`
  - [x] 4.6 `test_vimeo_discoverer_discover_returns_empty`
  - [x] 4.7 `test_discovery_orchestrator_with_vimeo_source`
  - [x] 4.8 `test_series_orchestrator_with_vimeo_source`

- [x] Task 5: Quality gate (AC: #12)
  - [x] 5.1 `cargo build` — clean
  - [x] 5.2 `cargo test` — 606 passing (557 lib + 15 main integration + 34 series integration), 0 failed
  - [x] 5.3 `cargo clippy -- -D warnings` — clean
  - [x] 5.4 `cargo fmt -- --check` — clean

## Dev Notes

### PRD vs Epics Discrepancy — PAT vs OAuth

The PRD (`prd.md`) lists `vimeo_client_id` and `vimeo_client_secret` in the config schema and NFR13 mentions OAuth token refresh. The epics document supersedes the PRD for implementation details: Epic 8 / Story 8.1 explicitly specifies a Personal Access Token (PAT) with no OAuth flow and no token refresh. Implemented PAT-only as specified in the epics.

### Config Field Addition — Backward Compatibility

The `#[serde(default)]` attribute on `vimeo_access_token` ensures existing `config.cfg` files without this key deserialize without error. The field defaults to `None`. Same pattern as `tvdb_api_key` and `cookies_from_browser`.

### VimeoDiscoverer Stub — Dead-Code Lint

Fields prefixed with `_` (`_access_token`, `_client`) to suppress dead-code lints idiomatically. When Story 8.2 implements the actual API search, the `_` prefix is simply removed.

### Token Security

The token is never logged, printed, or included in error messages. The `vimeo_access_token` field in both `Config` and `DiscoveryConfig` has a `SECURITY` doc comment.

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Completion Notes

All 5 tasks completed. Added `vimeo_access_token` to `Config` with PAT prompt/cache, created `VimeoDiscoverer` stub in `src/discovery/vimeo.rs`, wired it into both orchestrators replacing the stub warnings, threaded the token through `DiscoveryConfig` → `Orchestrator::new()` → both discovery orchestrators, and updated `main.rs` to load the token when `--sources vimeo` is active. 8 new tests added, all 606 tests passing, quality gate clean.

### File List

- `src/config.rs` — added `vimeo_access_token` field, `prompt_for_vimeo_token()`, `load_or_create_with_vimeo()`, 4 new tests, updated existing test fixtures
- `src/discovery/vimeo.rs` — new file, `VimeoDiscoverer` stub + 4 tests
- `src/discovery/mod.rs` — added `pub(crate) mod vimeo;`
- `src/discovery/orchestrator.rs` — added `vimeo` field, updated `new()`/`with_cookies()` signatures, replaced Vimeo stub warning with proper discover block
- `src/discovery/series_orchestrator.rs` — added `vimeo` field, updated `new()`/`new_with_tvdb()` signatures, replaced Vimeo stub warning with proper discover block, updated test constructors
- `src/orchestrator.rs` — added `vimeo_access_token` to `DiscoveryConfig`, threaded through `Orchestrator::new()`, updated test fixtures
- `src/main.rs` — added `Source` import, Vimeo token loading block, populated `DiscoveryConfig.vimeo_access_token`

## Senior Developer Review (AI)

**Review Date:** 2026-03-25
**Outcome:** Changes Requested
**Layers:** Blind Hunter ✅ | Edge Case Hunter ✅ | Acceptance Auditor ✅
**Dismissed:** 3 findings (noise/false positives)

### Action Items

- [x] [Review][Patch] Bilibili stub loop uses `for`+`matches!` — replace with `contains()` in both orchestrators [src/discovery/orchestrator.rs, src/discovery/series_orchestrator.rs]
- [x] [Review][Patch] Vimeo failure block in `main.rs` missing user guidance — add instructions matching TVDB failure block pattern [src/main.rs:126-133]
- [x] [Review][Patch] `prop_config_serialization_round_trip` does not verify `vimeo_access_token` round-trip [src/config.rs:property_tests]
- [x] [Review][Patch] `DiscoveryConfig.vimeo_access_token` SECURITY doc comment truncated — extend to match `tmdb_api_key` pattern [src/orchestrator.rs:270]
- [x] [Review][Patch] `pub(crate) mod vimeo` in `mod.rs` should be private `mod vimeo` — module visibility is redundant [src/discovery/mod.rs]
- [x] [Review][Defer] Config loaded multiple times in `main.rs` when multiple optional sources active — pre-existing architectural pattern [src/main.rs] — deferred, pre-existing
- [x] [Review][Defer] `reqwest::Client` allocated unconditionally in `VimeoDiscoverer::new` even when Vimeo not active — pre-existing pattern (Dailymotion does same) [src/discovery/vimeo.rs:new] — deferred, pre-existing
- [x] [Review][Defer] `vimeo_access_token` on public `DiscoveryConfig` — security-sensitive field is pub — pre-existing pattern (all DiscoveryConfig fields are pub) [src/orchestrator.rs] — deferred, pre-existing
- [x] [Review][Defer] `unwrap_or_default()` on vimeo token silently passes empty string — theoretical only, prompt guards against empty [src/main.rs:126] — deferred, pre-existing
