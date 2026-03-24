# vid-extras - Source Tree Analysis

**Date:** 2026-03-24

## Overview

`extras_fetcher` is a single-crate Rust project (binary + library). All source code lives under `src/`, with the discovery subsystem further organized into its own subdirectory. The project follows a flat module structure — each file is a focused module with a single responsibility.

## Complete Directory Structure

```
extras_fetcher/
├── src/
│   ├── main.rs                          # Binary entry point, tokio runtime, startup sequence
│   ├── lib.rs                           # Library root, re-exports public modules
│   ├── cli.rs                           # CLI argument parsing (clap derive)
│   ├── config.rs                        # config.cfg read/write, API key prompting
│   ├── models.rs                        # Shared data types (MovieEntry, SeriesEntry, VideoSource, etc.)
│   ├── error.rs                         # All error enums (per-module, thiserror)
│   ├── orchestrator.rs                  # Main pipeline coordinator (movies + series)
│   ├── scanner.rs                       # Directory traversal, folder name parsing
│   ├── downloader.rs                    # yt-dlp subprocess management
│   ├── converter.rs                     # ffmpeg x265 conversion, HW accel detection
│   ├── organizer.rs                     # Jellyfin directory organization, done markers
│   ├── validation.rs                    # Startup dependency checks
│   ├── output.rs                        # Colored terminal output, progress display
│   └── discovery/
│       ├── mod.rs                       # Module entry point, public re-exports
│       ├── orchestrator.rs              # Movie discovery coordinator (all sources)
│       ├── tmdb.rs                      # TMDB movie search + video list API
│       ├── archive.rs                   # Archive.org search API (pre-2010 movies)
│       ├── youtube.rs                   # YouTube search via yt-dlp (movies)
│       ├── title_matching.rs            # Title normalization, keyword filtering, category inference
│       ├── series_orchestrator.rs       # Series discovery coordinator (all sources)
│       ├── series_tmdb.rs               # TMDB TV series search + Season 0 API
│       ├── series_youtube.rs            # YouTube search via yt-dlp (series/seasons)
│       ├── series_cache.rs              # JSON file cache with 7-day TTL
│       ├── fuzzy_matching.rs            # Levenshtein distance, 80% similarity threshold
│       ├── tvdb.rs                      # TheTVDB API v4 client, token management
│       ├── id_bridge.rs                 # TMDB→TVDB ID resolution, file-based cache
│       ├── monitor_policy.rs            # Episode monitor/exclude filtering
│       ├── special_searcher.rs          # Season 0 search query construction
│       ├── special_validator.rs         # Pre-download candidate scoring (yt-dlp metadata)
│       ├── season_pack.rs               # Archive extraction (zip/7z/rar/tar), bonus content ID
│       └── season_zero_import.rs        # Local Season 0 file scanning and import
├── tests/
│   ├── main_integration_tests.rs        # 16 end-to-end integration tests
│   └── series_integration_tests.rs      # 34 series-specific integration tests
├── proptest-regressions/                # Proptest failure case regression files
├── docs/                                # Generated documentation (this folder)
├── Cargo.toml                           # Crate manifest, dependencies
├── Cargo.lock                           # Locked dependency versions
├── config.cfg                           # Runtime config (API keys, browser cookie source)
├── config.cfg.example                   # Example config for reference
├── .gitignore
└── README.md
```

## Critical Directories

### `src/`

The entire application source. No subdirectories except `discovery/`. Each `.rs` file is a focused module.

**Purpose:** All application logic
**Entry Points:** `src/main.rs` (binary), `src/lib.rs` (library)

### `src/discovery/`

The most complex subsystem. Handles all external API calls and content discovery for both movies and TV series. Split into focused single-responsibility modules.

**Purpose:** Multi-source content discovery
**Contains:** 16 modules covering TMDB, Archive.org, YouTube, TVDB, fuzzy matching, caching, and Season 0 specials
**Integration:** Called by `orchestrator.rs` and `series_orchestrator.rs`

### `tests/`

Integration tests that exercise the full pipeline with real filesystem operations but mocked/avoided network calls.

**Purpose:** End-to-end validation
**Contains:** 50 integration tests (16 main + 34 series)

## Entry Points

- **Main Entry:** `src/main.rs` — parses CLI args, initializes logging, validates dependencies, creates `Orchestrator`, runs pipeline, displays summary
- **Library Root:** `src/lib.rs` — re-exports all public modules for use in integration tests

## File Organization Patterns

- One module per `.rs` file, named after its responsibility
- Error types for all modules are centralized in `src/error.rs`
- Shared data structures live in `src/models.rs`
- Tests are co-located in `#[cfg(test)]` blocks within each source file
- Integration tests live in `tests/`
- Property-based test regression cases are stored in `proptest-regressions/`

## Key File Types

### Source Modules (`src/*.rs`)
- **Pattern:** `src/<module_name>.rs`
- **Purpose:** Application logic, one responsibility per file
- **Examples:** `scanner.rs`, `downloader.rs`, `converter.rs`

### Discovery Submodules (`src/discovery/*.rs`)
- **Pattern:** `src/discovery/<concern>.rs`
- **Purpose:** External API integrations and content matching logic
- **Examples:** `tmdb.rs`, `tvdb.rs`, `fuzzy_matching.rs`

### Integration Tests (`tests/*.rs`)
- **Pattern:** `tests/<scope>_integration_tests.rs`
- **Purpose:** End-to-end pipeline validation
- **Examples:** `main_integration_tests.rs`, `series_integration_tests.rs`

### Proptest Regressions (`proptest-regressions/*.txt`)
- **Pattern:** `proptest-regressions/<module>.txt`
- **Purpose:** Stores failing property-based test cases for regression prevention
- **Examples:** `scanner.txt`, `converter.txt`

## Configuration Files

- **`Cargo.toml`** — Crate manifest: name, version, edition, all dependencies
- **`Cargo.lock`** — Locked dependency tree (committed for binary crates)
- **`config.cfg`** — Runtime JSON config: `tmdb_api_key`, `tvdb_api_key` (optional), `cookies_from_browser` (optional)
- **`config.cfg.example`** — Template showing all supported config keys

## Asset Locations

No static assets. The tool generates:
- `done.ext` JSON files in processed movie/series folders (done markers)
- `.cache/` directory inside each series folder (metadata cache, 7-day TTL)
- `tmp_downloads/<title_year>/` temporary directories during processing (cleaned up on completion)

## Notes for Development

- The `discovery/` subdirectory is where most new feature work happens (new sources, new matching logic)
- `models.rs` is the single source of truth for shared types — add new data structures here
- `error.rs` centralizes all error enums — add new error variants here when adding new failure modes
- Hardware acceleration detection in `converter.rs` runs at startup via `ffmpeg -encoders`
- The `SeriesMetadataCache` stores JSON files in `.cache/` inside each series folder — not a global cache
- ID mapping cache (`IdMappingCache`) stores TMDB→TVDB mappings with no expiration in `.cache/id_mappings/`

---

_Generated using BMAD Method `document-project` workflow_
