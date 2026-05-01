# AGENTS.md

## Build & Run

```bash
cargo build --release           # Build binary to target/release/extras_fetcher.exe
cargo run --release -- /path    # Run directly
cargo test                      # Run all tests
cargo test test_name            # Run specific test
cargo test -- --nocapture      # See test output
cargo clippy -- -D warnings     # Lint
cargo fmt                      # Format
```

## System Dependencies

- **yt-dlp** - Must be in PATH for video downloads and Bilibili/YouTube searches
- **ffmpeg** - Must be in PATH with HEVC/x265 support for conversion

## Configuration

Create `config.cfg` in the working directory (prompted on first run, saved with `0o600` perms on Unix):

```json
{
  "tmdb_api_key": "...",           // Required for all runs
  "tvdb_api_key": "...",           // Required only for --specials
  "cookies_from_browser": "chrome",// Optional; overridden by --cookies-from-browser CLI
  "vimeo_access_token": "..."      // Required only for --sources vimeo
}
```

**Environment variables:**
- `TMDB_API_KEY` - Fallback for `tmdb_api_key` when `config.cfg` is absent
- `RUST_LOG` - Log verbosity (e.g. `RUST_LOG=debug`); `--verbose` sets debug automatically

## CLI Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `ROOT_DIRECTORY` | — | *(required)* | Root dir containing movie/series folders |
| `--force` | `-f` | false | Ignore `done.ext` markers, reprocess all |
| `--sources <SOURCES>` | — | `tmdb,archive,dailymotion,youtube` | Comma-separated discovery sources |
| `--all` | — | false | Use all sources (tmdb, archive, dailymotion, youtube, vimeo, bilibili) |
| `--concurrency <N>` | `-c` | 2 | Max parallel items |
| `--verbose` | `-v` | false | Enable debug-level logging |
| `--single` | `-s` | false | Treat ROOT_DIRECTORY as a single folder (bypass scan loop) |
| `--series-only` | — | false | Skip movies, process only TV series |
| `--movies-only` | — | false | Skip series, process only movies |
| `--season-extras` | — | false | Enable per-season extras discovery for series |
| `--specials` | — | false | Enable Season 0 specials discovery (requires TVDB API key) |
| `--specials-folder <NAME>` | — | `"Specials"` | Folder name used for Season 0 |
| `--type <TYPE>` | — | (none) | Force classify as `movie` or `series` |
| `--cookies-from-browser <BROWSER>` | — | (none) | Pass browser cookies to yt-dlp (e.g. `chrome`, `firefox`, `edge`) |
| `--dry-run` | — | false | Discover only — no downloads, conversions, or file I/O |
| `--json-progress` | — | false | Emit line-delimited JSON progress events to stdout |
| `--tui` | — | false | Enable split-pane ratatui TUI with per-thread log panels |

**Available `--sources` values:** `tmdb`, `archive`, `dailymotion`, `youtube`, `vimeo`, `bilibili`

**Mutually exclusive:** `--series-only` / `--movies-only`, `--all` / `--sources`

## Directory Structure

Input folders must match Jellyfin naming:

- Movies: `Movie Title (2020)/Movie Title (2020).mkv`
- Series: `Series Name (2020)/Season 01/...`, `Season 00` for specials

Extras organized into: `trailers/`, `behind the scenes/`, `deleted scenes/`, `featurettes/`, `interviews/`

**Runtime files written:**
- `done.ext` - JSON marker in each processed folder: `{"finished_at": "...", "version": "..."}`
- `tmp_downloads/` - Temporary download dir, cleaned up on exit
- `tui_log.txt` - Log mirror when `--tui` is active
- `root_dir/.cache/tvdb_ids/{tmdb_id}.json` - TMDB→TVDB ID mapping cache (no TTL)
- `series_dir/.cache/` - TTL-based TMDB/TVDB metadata cache per series
- `series_dir/specials_exclude.json` - Optional user file listing Season 0 episode numbers to skip

## Architecture

### Core modules

| File | Description |
|------|-------------|
| `src/main.rs` | Entry point: parse CLI → init logging → validate deps → run pipeline |
| `src/lib.rs` | Module declarations, `TUI_ACTIVE` global flag, re-exports |
| `src/cli.rs` | `CliArgs` (clap derive), `CliConfig`, validation, banner display |
| `src/config.rs` | `config.cfg` load/save, interactive key prompting |
| `src/models.rs` | All shared types: `MovieEntry`, `SeriesEntry`, `VideoSource`, `ContentCategory`, etc. |
| `src/error.rs` | Structured error types via `thiserror` for every subsystem |
| `src/orchestrator.rs` | Top-level 5-phase pipeline coordinator (scan → discover → download → convert → organize) |
| `src/scanner.rs` | Directory traversal, `Movie`/`Series`/`Unknown` classification, `done.ext` skip logic |
| `src/downloader.rs` | yt-dlp subprocess wrapper; 5-min timeout; subtitle sidecar collection |
| `src/converter.rs` | ffmpeg HEVC conversion; auto-detects NVENC → QSV → VideoToolbox → software x265 |
| `src/organizer.rs` | Moves files into Jellyfin subdirs; sanitizes Windows filenames; writes `done.ext` |
| `src/deduplication.rs` | Tier-based fuzzy deduplication (title similarity + duration); higher tier wins |
| `src/validation.rs` | Startup checks: yt-dlp/ffmpeg in PATH, HEVC encoder present, yt-dlp version |
| `src/output.rs` | Colored CLI output, progress display, `ProcessingSummary`; suppressed when TUI active |
| `src/json_output.rs` | `ProgressEvent` struct; line-delimited JSON to stdout for `--json-progress` |
| `src/tui.rs` | Ratatui split-pane TUI; per-thread log ring buffers; ~60fps render loop |

### Discovery submodules (`src/discovery/`)

| File | Description |
|------|-------------|
| `orchestrator.rs` | Coordinates all movie discoverers |
| `series_orchestrator.rs` | Coordinates TV series discoverers + Season 0 specials flow |
| `tmdb.rs` | TMDB `/movie/{id}/videos` (trailers, featurettes, BTS, deleted scenes, interviews) |
| `series_tmdb.rs` | TMDB `/tv/{id}/videos` + season-specific videos |
| `youtube.rs` | yt-dlp `--flat-playlist` YouTube search for movie extras |
| `series_youtube.rs` | YouTube search for TV series extras |
| `archive.rs` | Internet Archive full-text search |
| `dailymotion.rs` | Dailymotion REST API (no key required) |
| `vimeo.rs` | Vimeo REST API (requires Personal Access Token); up to 30 results |
| `bilibili.rs` | yt-dlp search on Bilibili with Chinese-language terms |
| `kinocheck.rs` | KinoCheck API fallback when TMDB returns zero results; free, ~1000 req/day shared limit |
| `tvdb.rs` | TheTVDB API v4 client: JWT auth, Season 0 episode listing, pagination |
| `id_bridge.rs` | TMDB→TVDB ID translation with persistent disk cache |
| `fuzzy_matching.rs` | Levenshtein-based title similarity (0–100) |
| `title_matching.rs` | YouTube result filtering; Roman numeral / superscript normalization |
| `retry.rs` | `retry_with_backoff(max_retries, base_delay_ms, op)` with exponential backoff |
| `monitor_policy.rs` | Season 0 episode monitoring; reads `specials_exclude.json` for manual exclusions |
| `season_pack.rs` | Extracts zip/rar/7z/tar.gz season pack archives |
| `season_zero_import.rs` | Scans series folder for existing `S00Exx` files to skip re-downloads |
| `series_cache.rs` | TTL-based disk cache for TMDB/TVDB metadata per series |
| `special_searcher.rs` | Builds ordered YouTube search queries per TVDB special episode (incl. OVA variants) |
| `special_validator.rs` | Scores/selects best YouTube candidate via title similarity + duration matching |

Pipeline is idempotent: `done.ext` marker is written on completion. Use `--force` to reprocess.

## Testing

Tests use **proptest** for property-based testing. Integration tests live in `tests/`.

| Test file | Coverage |
|-----------|----------|
| `tests/discovery_retry_tests.rs` | `retry_with_backoff` behavior |
| `tests/main_integration_tests.rs` | Validator, scanner, done marker creation/skipping, orchestrator init |
| `tests/series_integration_tests.rs` | Series folder detection, season scanning, mixed library handling |

Inline `#[cfg(test)]` unit tests exist in most source files. Proptest regression inputs are in `proptest-regressions/`.
