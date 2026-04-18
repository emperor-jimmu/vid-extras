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

## Dependencies

- **yt-dlp** - Must be in PATH for video downloads
- **ffmpeg** - Must be in PATH with HEVC/x265 support for conversion
- **TMDB API key** - Via `config.cfg` (JSON with `tmdb_api_key` field) or `TMDB_API_KEY` env var

## Configuration

Create `config.cfg` in project root:

```json
{ "tmdb_api_key": "your_key_here" }
```

Or set `TMDB_API_KEY` environment variable. On first run, tool prompts for key and saves to config.cfg.

## CLI Options

- `--tui` - Enable split-pane TUI with per-thread logs and progress bar (Recommended for parallel runs)
- `-f, --force` - Reprocess all (ignores `.extras_done` markers)
- `--sources <SOURCES>` - Discovery sources (comma-separated) [default: tmdb,youtube]
- `--all` - Use all available discovery sources
- `-c, --concurrency <N>` - Parallel items (default: 2)
- `--series-only` - Skip movies, process only TV series
- `--movies-only` - Skip series, process only movies
- `--specials` - Enable Season 0 specials discovery
- `--season-extras` - Enable season-specific extras
- `-v, --verbose` - Detailed logging
- `RUST_LOG=debug` - Debug-level logging

## Directory Structure

Input folders must match Jellyfin naming:

- Movies: `Movie Title (2020)/Movie Title (2020).mkv`
- Series: `Series Name (2020)/Season 01/...`, `Season 00` for specials

Extras organized to: `trailers/`, `behind the scenes/`, `deleted scenes/`, `featurettes/`, `interviews/` subdirs.

## Architecture

- `src/main.rs` - CLI entry point
- `src/cli.rs` - Argument parsing
- `src/scanner.rs` - Directory scanning
- `src/discovery/` - Multi-source content discovery (TMDB, YouTube, Archive.org, etc.)
- `src/downloader.rs` - yt-dlp integration
- `src/converter.rs` - ffmpeg HEVC conversion
- `src/organizer.rs` - File organization & `.extras_done` markers

Idempotent: creates `.extras_done` marker when complete. Use `--force` to reprocess.

## Testing

Tests use proptest for property-based testing. Some tests require fixtures; see proptest-regressions/ for known regressions.
