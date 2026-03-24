# vid-extras - Architecture

**Date:** 2026-03-24
**Architecture Pattern:** Sequential Pipeline with Async Orchestration
**Language:** Rust 2024 edition

## Executive Summary

`extras_fetcher` is a CLI tool built around a five-stage pipeline: Scan → Discover → Download → Convert → Organize. Each stage is an independent module with its own error type. The `Orchestrator` drives the pipeline, processing movies and TV series with configurable concurrency. All I/O is async via Tokio. External tools (`yt-dlp`, `ffmpeg`) are invoked as subprocesses.

## Architecture Pattern

The codebase uses a **pipeline/orchestrator** pattern:

- Each pipeline stage is a struct with a focused responsibility
- Stages communicate via owned data types (no shared mutable state between stages)
- The `Orchestrator` sequences stages and handles error isolation
- Concurrency is controlled by a `tokio::sync::Semaphore` — at most N items process simultaneously
- Failures in one item are logged and skipped; they do not stop the pipeline

```
┌─────────────────────────────────────────────────────────────┐
│                        Orchestrator                          │
│                                                             │
│  ┌─────────┐  ┌──────────┐  ┌──────────┐  ┌───────────┐  │
│  │ Scanner │→ │Discovery │→ │Downloader│→ │ Converter │  │
│  └─────────┘  └──────────┘  └──────────┘  └───────────┘  │
│                                                    ↓        │
│                                             ┌───────────┐  │
│                                             │ Organizer │  │
│                                             └───────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Module Responsibilities

### `main.rs` — Entry Point

Startup sequence only. No business logic.

1. Parse CLI args (`cli::parse_args()`)
2. Initialize logging (`env_logger`)
3. Display banner and config (`output::display_banner()`)
4. Validate dependencies (`Validator::validate_dependencies()`)
5. Create `Orchestrator` and call `.run()`
6. Display summary and exit with code 0 or 1

### `cli.rs` — CLI Argument Parsing

Uses `clap` derive macros. Validates directory existence and concurrency >= 1 before returning `CliConfig`. Provides `display_banner()` and `display_config()` for colored terminal output.

Key types: `CliArgs`, `CliConfig`, `CliError`

### `config.rs` — Configuration File

Reads and writes `config.cfg` (JSON). Prompts interactively for missing API keys on first run. Supports `tmdb_api_key`, `tvdb_api_key`, and `cookies_from_browser`.

### `models.rs` — Shared Data Types

Single source of truth for all data structures passed between pipeline stages.

Key types:
- `MovieEntry` — title, year, path, tmdb_id
- `SeriesEntry` — title, optional year, path, seasons list
- `VideoSource` — url, title, category (`ContentCategory`)
- `DownloadResult` — path to downloaded file, category, success/failure
- `ConversionResult` — path to converted file, category, success/failure
- `DoneMarker` — finished_at (ISO 8601), version
- `ContentCategory` — enum: Trailer, Featurette, BehindTheScenes, DeletedScene
- `Source` — enum: Tmdb, Archive, Youtube, Dailymotion, Vimeo, Bilibili (user-selectable sources)
- `HardwareAccel` — enum: Nvenc, Qsv, VideoToolbox, Software

### `error.rs` — Centralized Error Types

All error enums in one file using `thiserror`. One enum per module:
`ScanError`, `DiscoveryError`, `DownloadError`, `ConversionError`, `OrganizerError`, `ValidationError`, `OrchestratorError`, `CliError`, `SeriesScanError`, `SeriesDiscoveryError`, `SeriesOrganizerError`

### `validation.rs` — Startup Dependency Checks

Runs before any processing. Checks:
- `yt-dlp` binary in PATH
- `ffmpeg` binary in PATH
- `ffmpeg` has HEVC encoder support
- TMDB API key present (env var or config file)
- TVDB API key present when `--specials` is used

### `scanner.rs` — Directory Traversal

Scans the root directory for movie or series folders. Parses folder names with regex `^(.+?)\s*\((\d{4})\)$` for movies (title + year required) and a relaxed pattern for series (year optional). Checks for `done.ext` JSON files and skips completed items unless `--force` is set. Invalid done marker JSON is treated as missing.

### `discovery/` — Content Discovery Subsystem

The most complex subsystem. Organized into 16 focused modules:

#### Movie Discovery

```
DiscoveryOrchestrator
├── TmdbDiscoverer          (TMDB API: search + video list + collection sibling videos)
├── ArchiveOrgDiscoverer    (Archive.org: all movies — general, making-of, DVDXtras queries)
├── YoutubeDiscoverer       (yt-dlp ytsearch5: always queried)
├── DailymotionDiscoverer   (Dailymotion REST API: keyword search, paginated, 1 req/sec)
└── KinoCheckDiscoverer     (implicit fallback when TMDB returns 0 videos)
```

`DiscoveryOrchestrator` runs all three sources concurrently and aggregates results. In `YoutubeOnly` mode, only `YoutubeDiscoverer` runs. When TMDB is active but returns zero videos for a movie, `KinoCheckDiscoverer` is queried as an internal fallback using the movie's TMDB ID.

`title_matching.rs` provides shared filtering logic used by all movie discoverers:
- Title normalization (lowercase, remove special chars, collapse whitespace)
- Excluded keyword filtering (Review, Reaction, Analysis, Explained, Ending, Theory, React)
- Duration validation (30s–20min)
- YouTube Shorts detection (duration < 60s AND vertical aspect ratio)
- Category inference from video title keywords

#### Series Discovery

```
SeriesDiscoveryOrchestrator
├── TmdbSeriesDiscoverer    (TMDB TV API: series extras + Season 0)
├── YoutubeSeriesDiscoverer (yt-dlp: series-level + season-specific queries)
├── KinoCheckDiscoverer     (implicit fallback when TMDB returns 0 videos)
├── TvdbClient              (TVDB API v4: Season 0 episode metadata)
│   └── IdBridge            (TMDB→TVDB ID resolution)
│       └── IdMappingCache  (file-based, no expiration)
├── SpecialValidator        (pre-download candidate scoring)
│   └── SpecialSearcher     (search query construction)
├── MonitorPolicy           (episode filter: auto-monitor, exclusion list)
├── SeasonPackProcessor     (archive extraction: zip/7z/rar/tar)
├── Season0Importer         (local Season 0 file scanning)
├── SeriesMetadataCache     (JSON file cache, 7-day TTL)
└── FuzzyMatcher            (Levenshtein distance, 80% threshold)
```

#### TVDB Season 0 Specials Flow

```
1. IdBridge.resolve(tmdb_id)
   → query TMDB external_ids endpoint
   → fallback: TVDB search with FuzzyMatcher (80% threshold)
   → cache result in IdMappingCache

2. TvdbClient.get_season_zero(tvdb_id)
   → paginated fetch of all Season 0 episodes
   → enrich each with get_episode_extended() for metadata

3. MonitorPolicy.filter_monitored(episodes)
   → auto-monitor if airs_after_season matches latest season
   → auto-monitor if is_movie == true
   → exclude if in specials_exclude.json

4. SpecialValidator.select_best_candidates(series_title, episodes)
   → for each episode, build queries via SpecialSearcher
   → run yt-dlp --dump-json --flat-playlist ytsearch5:{query}
   → score candidates: title similarity (FuzzyMatcher, min 40%) + duration
   → movies (is_movie=true) must be >= 600s
   → return best URL per episode

5. Download + Convert + Organize into specials_folder/
```

### `downloader.rs` — Video Downloading

Invokes `yt-dlp` as a subprocess for each `VideoSource`. Creates `tmp_downloads/<title_year>/` temp directories. Cleans up partial files on failure. Configurable timeout (default 5 minutes). Failed downloads are logged and skipped without stopping the pipeline.

Filename sanitization runs after download to ensure Windows compatibility:
- `|`, `<`, `>`, `:`, `/`, `\`, `*` → `-`
- `"` → `'`
- `?` → removed

### `converter.rs` — Video Conversion

Invokes `ffmpeg` as a subprocess. Auto-detects hardware acceleration at construction time by parsing `ffmpeg -encoders` output. Priority: NVENC → QSV → VideoToolbox → Software.

FFmpeg command variants:
- Software: `ffmpeg -y -i input -c:v libx265 -crf 25 -preset medium -c:a copy output`
- NVENC: `ffmpeg -y -hwaccel cuda -i input -c:v hevc_nvenc -preset p4 -rc vbr -cq 25 -c:a copy output`
- QSV: `ffmpeg -y -hwaccel qsv -i input -c:v hevc_qsv -global_quality 25 -c:a copy output`

CRF values outside 24–26 are clamped to 25. On success, the original download is deleted. On failure, the failed output is deleted and the original is preserved.

### `organizer.rs` — File Organization

Moves converted files into Jellyfin-compatible subdirectories within the movie/series folder:

| ContentCategory | Subdirectory |
|---|---|
| Trailer | `/trailers` |
| Featurette | `/featurettes` |
| BehindTheScenes | `/behind the scenes` |
| DeletedScene | `/deleted scenes` |

Creates subdirectories if missing. Handles cross-drive moves (Windows error 17) by falling back to copy+delete. Writes `done.ext` JSON marker on completion. Cleans up temp directories after successful organization.

For series, `SeriesOrganizer` places Season 0 specials in `<specials_folder>/` (configurable, default "Specials") using Sonarr-compatible naming: `Series Title - S00E01 - Episode Title.ext`.

### `orchestrator.rs` — Pipeline Coordinator

The top-level coordinator. Holds all pipeline stage instances and drives execution.

```rust
pub struct Orchestrator {
    scanner: Scanner,
    discovery: DiscoveryOrchestrator,
    downloader: Downloader,
    converter: Converter,
    organizer: Organizer,
    concurrency: usize,
    // ... series-specific fields
}
```

Processing flow:
1. `Scanner::scan()` → `Vec<MovieEntry>` (or `Vec<SeriesEntry>`)
2. For each item (up to `concurrency` in parallel via `Semaphore`):
   a. `DiscoveryOrchestrator::discover_all()` → `Vec<VideoSource>`
   b. `Downloader::download_all()` → `Vec<DownloadResult>`
   c. `Converter::convert_batch()` → `Vec<ConversionResult>`
   d. `Organizer::organize()` → writes files + done marker
3. Aggregate `ProcessingSummary`

Implements `Drop` to clean up temp directories even on panic.

### `output.rs` — Terminal Output

Colored progress display using the `colored` crate. Provides:
- `display_banner()` — ASCII box with tool name and version
- `display_config()` — all active parameters with color-coded values
- `display_summary()` — final statistics (total, successful, failed, downloads, conversions)
- Per-item progress messages

## Data Flow

```
CLI Args
    ↓
CliConfig
    ↓
Orchestrator::new()
    ↓
Scanner::scan() → Vec<MovieEntry | SeriesEntry>
    ↓ (per item, up to N concurrent)
DiscoveryOrchestrator::discover_all() → Vec<VideoSource>
    ↓
Downloader::download_all() → Vec<DownloadResult>
    ↓
Converter::convert_batch() → Vec<ConversionResult>
    ↓
Organizer::organize() → filesystem writes + done.ext
    ↓
ProcessingSummary → terminal output
```

## External API Integrations

### TMDB API v3

- **Auth:** `api_key` query parameter
- **Movie search:** `GET /3/search/movie?query={title}&year={year}`
- **Movie videos:** `GET /3/movie/{id}/videos`
- **Movie details:** `GET /3/movie/{id}` (for collection membership detection)
- **Collection:** `GET /3/collection/{id}` (for sibling movie IDs and titles)
- **Sibling videos:** `GET /3/movie/{sibling_id}/videos` (for collection cross-promotional extras)
- **TV search:** `GET /3/search/tv?query={title}`
- **TV details:** `GET /3/tv/{id}`
- **TV videos:** `GET /3/tv/{id}/videos`
- **TV season:** `GET /3/tv/{id}/season/0`
- **External IDs:** `GET /3/tv/{id}/external_ids` (for TVDB ID lookup)

### TheTVDB API v4

- **Auth:** `POST /v4/login` with API key → Bearer token (cached in memory, auto-refreshed on 401)
- **Season 0:** `GET /v4/series/{id}/episodes/official?season=0&page={n}` (paginated)
- **Episode extended:** `GET /v4/episodes/{id}/extended`
- **Episode translation:** `GET /v4/episodes/{id}/translations/eng`
- **Series search:** `GET /v4/search?query={title}&type=series` (fallback ID resolution)
- **Network resilience:** 2-second retry on timeout, re-authenticate on 401

### KinoCheck API

- **Auth:** None (free public API, 1,000 req/day limit)
- **Movie lookup:** `GET https://api.kinocheck.de/movies?tmdb_id={id}` — returns single trailer object with YouTube video ID
- **Used for:** Implicit TMDB fallback when TMDB returns zero videos for a movie or series

### Dailymotion API

- **Auth:** None (free public API)
- **Video search:** `GET https://api.dailymotion.com/videos?search={query}&fields=id,title,duration,url&limit=10&page={n}`
- **Rate limit:** Undocumented; paced at 1 request per second to be safe
- **Used for:** Discovering official distributor uploads and extras not available on YouTube
- **Pagination:** Up to 3 pages; `has_more` boolean indicates more results available

### Archive.org

- **Search:** `GET https://archive.org/advancedsearch.php?q={query}&fl[]=identifier,title,subject,description,mediatype&output=json`
- **Used for:** All movies — three query strategies: general EPK/extras, subject:"making of", and DVDXtras collection

### YouTube (via yt-dlp)

- **Search:** `yt-dlp --dump-json --flat-playlist "ytsearch5:{query}"` (metadata only, no download)
- **Download:** `yt-dlp --no-playlist --quiet -o "{output_path}" "{url}"`
- **Cookie support:** `--cookies-from-browser {browser}` when configured

## Concurrency Model

- Tokio multi-thread runtime (`#[tokio::main]`)
- `Arc<Semaphore>` limits concurrent item processing
- Within a single item, all stages run sequentially (download → convert → organize)
- Multiple items can be in different pipeline stages simultaneously (up to concurrency limit)
- No shared mutable state between concurrent tasks — each task owns its data

## Caching Strategy

| Cache | Location | TTL | Key |
|---|---|---|---|
| Series metadata (TMDB) | `<series_path>/.cache/<name>.json` | 7 days | series name |
| TVDB Season 0 episodes | `<series_path>/.cache/tvdb_<id>.json` | 7 days | TVDB series ID |
| TMDB→TVDB ID mapping | `<series_path>/.cache/id_mappings/<tmdb_id>.json` | None (permanent) | TMDB series ID |

## Error Handling Philosophy

- Each module defines its own error enum in `error.rs` using `thiserror`
- Errors propagate via `?` operator through the call stack
- At the orchestrator level, per-item errors are caught, logged, and counted — they do not abort the run
- Fatal errors (missing dependencies, invalid config) cause immediate exit with code 1 and a descriptive message
- No `.unwrap()` in production code; `.expect()` is used only for truly impossible cases with descriptive messages

## Testing Strategy

- **Unit tests:** Co-located in `#[cfg(test)]` blocks, covering all public APIs
- **Property-based tests:** `proptest` with 100+ iterations per property, covering mathematical invariants (fuzzy matching symmetry, duration range validity, filename sanitization idempotency, etc.)
- **Integration tests:** `tests/` directory, using `tempfile` for isolated filesystem operations, avoiding real network calls
- **Total:** 437+ tests (427 unit/property + 50 integration)
- **Quality gate:** `cargo test && cargo clippy -- -D warnings && cargo fmt -- --check`

## Design Decisions

**Why subprocess invocation for yt-dlp and ffmpeg?**
Both tools have complex, frequently-updated behavior that is difficult to replicate via library bindings. Subprocess invocation keeps the Rust code thin and delegates complexity to well-maintained external tools.

**Why per-module error types in a single file?**
Centralizing all error enums in `error.rs` makes it easy to see all failure modes at a glance while keeping each enum focused on its module's concerns.

**Why file-based caching instead of a database?**
The cache is per-series and accessed infrequently. JSON files are human-readable, easy to debug, and require no additional dependencies.

**Why sequential processing within a single item?**
Download → Convert → Organize must be sequential by nature (each stage depends on the previous stage's output). Parallelism is applied at the item level, not within an item.

---

_Generated using BMAD Method `document-project` workflow_
