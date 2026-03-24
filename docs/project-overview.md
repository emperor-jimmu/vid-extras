# vid-extras - Project Overview

**Date:** 2026-03-24
**Type:** CLI Tool
**Architecture:** Pipeline / Orchestrator

## Executive Summary

vid-extras (`extras_fetcher`) is a Rust CLI tool that automates the discovery, downloading, conversion, and organization of supplemental video content (extras) for Jellyfin media libraries. It supports both movie and TV series libraries, pulling content from TMDB, Archive.org, YouTube, and TheTVDB, then converting everything to x265/HEVC and placing files into Jellyfin-compatible directory structures.

## Project Classification

- **Repository Type:** Monolith
- **Project Type:** CLI
- **Primary Language:** Rust (2024 edition)
- **Architecture Pattern:** Sequential pipeline with async orchestration

## Technology Stack Summary

| Category | Technology | Version | Role |
|---|---|---|---|
| Language | Rust | 2024 edition | Primary language |
| Async Runtime | Tokio | 1.41 | Async I/O and task scheduling |
| CLI Parsing | clap | 4.5 | Argument parsing with derive macros |
| HTTP Client | reqwest | 0.12 | TMDB, TVDB, Archive.org API calls |
| Serialization | serde + serde_json | 1.0 | JSON parsing and done-marker I/O |
| Error Types | thiserror | 2.0 | Per-module error enums |
| Pattern Matching | regex | 1.11 | Folder name parsing |
| Terminal Output | colored | 2.1 | Colored CLI output |
| Logging | log + env_logger | 0.4 / 0.11 | Structured logging |
| URL Encoding | urlencoding | 2.1 | Safe API query construction |
| Date/Time | chrono | 0.4 | ISO 8601 timestamps in done markers |
| Testing | proptest | 1.5 | Property-based testing |
| Testing | tempfile | 3.13 | Isolated filesystem tests |
| External Tool | yt-dlp | system PATH | Video downloading |
| External Tool | ffmpeg | system PATH | x265/HEVC conversion |

## Key Features

- Multi-source content discovery: TMDB API, Archive.org (pre-2010 movies), YouTube (always queried)
- Automated video downloading via yt-dlp with configurable timeout
- Hardware-accelerated x265 conversion: NVENC → QSV → VideoToolbox → software fallback
- Jellyfin-compatible directory organization (`/trailers`, `/featurettes`, `/behind the scenes`, `/deleted scenes`)
- Skip-on-completion done markers (JSON with ISO 8601 timestamp) to avoid reprocessing
- TV series support: series-level extras, season-specific extras, Season 0 specials via TheTVDB API v4
- Pre-download candidate scoring for Season 0 specials (title similarity + duration validation)
- Metadata caching with 7-day TTL to reduce API calls
- TMDB-to-TVDB ID resolution with fuzzy matching fallback (80% Levenshtein threshold)
- Configurable concurrency with semaphore-based enforcement
- Filename sanitization for Windows compatibility
- Configurable Season 0 folder name (default: "Specials")

## Architecture Highlights

The tool follows a strict linear pipeline:

```
Scan → Discover → Download → Convert → Organize
```

Each stage is a distinct module with its own error type. The `Orchestrator` coordinates the pipeline per movie/series, with a semaphore controlling how many items are processed in parallel. Failures in one item are isolated and do not stop processing of others.

For TV series, a parallel `SeriesDiscoveryOrchestrator` coordinates TMDB series discovery, YouTube series discovery, TVDB Season 0 specials, season pack extraction, and local Season 0 import.

## Development Overview

### Prerequisites

- Rust toolchain (2024 edition)
- `yt-dlp` in system PATH
- `ffmpeg` with HEVC encoding support in system PATH
- TMDB API key (prompted on first run, stored in `config.cfg`)
- TVDB API key (optional, only needed for `--specials` flag)

### Getting Started

```bash
# Build
cargo build --release

# Run (first run will prompt for API keys)
./target/release/extras_fetcher /path/to/media/library

# Run with verbose logging
RUST_LOG=debug ./target/release/extras_fetcher /path/to/library
```

### Key Commands

- **Build:** `cargo build --release`
- **Test:** `cargo test`
- **Check:** `cargo check`
- **Lint:** `cargo clippy -- -D warnings`
- **Format:** `cargo fmt`

## Repository Structure

```
extras_fetcher/
├── src/                    # All source code
│   ├── main.rs             # Entry point
│   ├── lib.rs              # Library root (re-exports)
│   ├── cli.rs              # CLI argument parsing
│   ├── config.rs           # Config file management
│   ├── models.rs           # Shared data structures
│   ├── error.rs            # Centralized error types
│   ├── orchestrator.rs     # Main pipeline coordinator
│   ├── scanner.rs          # Directory traversal
│   ├── downloader.rs       # yt-dlp integration
│   ├── converter.rs        # ffmpeg x265 conversion
│   ├── organizer.rs        # Jellyfin file organization
│   ├── validation.rs       # Dependency checking
│   ├── output.rs           # CLI output formatting
│   └── discovery/          # Multi-source content discovery
│       ├── mod.rs           # Module entry point
│       ├── tmdb.rs          # TMDB movie discovery
│       ├── archive.rs       # Archive.org discovery
│       ├── youtube.rs       # YouTube movie discovery
│       ├── orchestrator.rs  # Movie discovery coordinator
│       ├── title_matching.rs # Title normalization and filtering
│       ├── series_tmdb.rs   # TMDB series discovery
│       ├── series_youtube.rs # YouTube series discovery
│       ├── series_orchestrator.rs # Series discovery coordinator
│       ├── series_cache.rs  # Metadata cache (7-day TTL)
│       ├── fuzzy_matching.rs # Levenshtein-based matching
│       ├── tvdb.rs          # TheTVDB API v4 client
│       ├── id_bridge.rs     # TMDB→TVDB ID resolution
│       ├── monitor_policy.rs # Episode filtering logic
│       ├── special_searcher.rs # Season 0 search queries
│       ├── special_validator.rs # Pre-download candidate scoring
│       ├── season_pack.rs   # Archive extraction
│       └── season_zero_import.rs # Local S0 file import
├── tests/                  # Integration tests
├── Cargo.toml              # Project manifest
├── config.cfg              # Runtime config (API keys)
└── docs/                   # Generated documentation
```

## Documentation Map

- [index.md](./index.md) - Master documentation index
- [architecture.md](./architecture.md) - Detailed technical architecture
- [source-tree-analysis.md](./source-tree-analysis.md) - Annotated directory structure
- [development-guide.md](./development-guide.md) - Development workflow and setup

---

_Generated using BMAD Method `document-project` workflow_
