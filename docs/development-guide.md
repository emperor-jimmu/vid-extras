# vid-extras - Development Guide

**Date:** 2026-03-24

## Prerequisites

### Required Tools

| Tool | Purpose | Install |
|---|---|---|
| Rust (2024 edition) | Build toolchain | https://rustup.rs |
| yt-dlp | Video downloading | `pip install yt-dlp` or system package |
| ffmpeg | Video conversion (must have HEVC/x265 support) | https://ffmpeg.org/download.html |

### Optional Tools

| Tool | Purpose |
|---|---|
| TMDB API key | Required for movie/series discovery (prompted on first run) |
| TVDB API key | Required only when using `--specials` flag |

### Verify Prerequisites

```bash
# Check Rust
rustc --version   # should be 2024 edition compatible

# Check yt-dlp
yt-dlp --version

# Check ffmpeg with HEVC support
ffmpeg -encoders | grep -E "hevc|x265"
# Should show: libx265, hevc_nvenc (NVIDIA), hevc_qsv (Intel), or hevc_videotoolbox (macOS)
```

## Environment Setup

### API Keys

On first run, the tool prompts for API keys and stores them in `config.cfg`:

```json
{
  "tmdb_api_key": "your_tmdb_key_here",
  "tvdb_api_key": "your_tvdb_key_here",
  "cookies_from_browser": "chrome"
}
```

You can also set them via environment variables (takes precedence over config file):

```bash
export TMDB_API_KEY=your_key_here
export TVDB_API_KEY=your_key_here
```

### Logging

Control log verbosity with `RUST_LOG`:

```bash
RUST_LOG=debug cargo run -- /path/to/library    # verbose
RUST_LOG=info cargo run -- /path/to/library     # normal
RUST_LOG=warn cargo run -- /path/to/library     # warnings only
```

Or use the `--verbose` flag for debug-level output without setting the env var.

## Build Commands

```bash
# Development build
cargo build

# Optimized release build
cargo build --release

# Check for errors without producing binary (fastest feedback)
cargo check

# Run directly
cargo run -- /path/to/media/library

# Run release binary
./target/release/extras_fetcher /path/to/media/library
```

## Test Commands

```bash
# Run all tests
cargo test

# Run tests with output (don't capture stdout)
cargo test -- --nocapture

# Run a specific test
cargo test test_name

# Run tests in a specific module
cargo test scanner::

# Run only integration tests
cargo test --test main_integration_tests
cargo test --test series_integration_tests

# Run with logging during tests
RUST_LOG=debug cargo test -- --nocapture

# Run property-based tests with more iterations
PROPTEST_CASES=1000 cargo test
```

## Code Quality Commands

```bash
# Lint (must pass with zero warnings)
cargo clippy -- -D warnings

# Format check
cargo fmt -- --check

# Auto-format
cargo fmt

# All quality checks (run before committing)
cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt -- --check
```

## CLI Usage

```bash
# Basic usage — process all movies in library
extras_fetcher /path/to/movies

# Process only TV series
extras_fetcher --series-only /path/to/tv

# Process only movies
extras_fetcher --movies-only /path/to/movies

# Enable season-specific extras for series
extras_fetcher --series-only --season-extras /path/to/tv

# Enable Season 0 specials via TVDB (requires TVDB API key)
extras_fetcher --series-only --specials /path/to/tv

# Custom Season 0 folder name
extras_fetcher --series-only --specials --specials-folder "Season 0" /path/to/tv

# Force reprocess (ignore done markers)
extras_fetcher --force /path/to/movies

# YouTube-only mode (skip TMDB and Archive.org)
extras_fetcher --mode youtube /path/to/movies

# Parallel processing (default: 1)
extras_fetcher --concurrency 3 /path/to/movies

# Verbose logging
extras_fetcher --verbose /path/to/movies

# Show help
extras_fetcher --help

# Show version
extras_fetcher --version
```

## Project Structure for Development

```
src/
├── main.rs          ← startup sequence, do not add business logic here
├── lib.rs           ← re-exports only
├── models.rs        ← add new shared types here
├── error.rs         ← add new error variants here
├── cli.rs           ← add new CLI flags here
├── config.rs        ← config file read/write
├── orchestrator.rs  ← pipeline coordination
├── scanner.rs       ← directory traversal
├── downloader.rs    ← yt-dlp integration
├── converter.rs     ← ffmpeg integration
├── organizer.rs     ← file organization
├── validation.rs    ← startup checks
├── output.rs        ← terminal output
└── discovery/       ← all external API integrations
```

## Adding a New Discovery Source

1. Create `src/discovery/my_source.rs`
2. Implement the `ContentDiscoverer` trait (for movies) or equivalent async methods (for series)
3. Add the new discoverer to `src/discovery/orchestrator.rs` (movies) or `src/discovery/series_orchestrator.rs` (series)
4. Add any new error variants to `src/error.rs`
5. Export the new module from `src/discovery/mod.rs`
6. Write unit tests in the new file and add property-based tests if applicable

## Adding a New CLI Flag

1. Add the field to `CliArgs` in `src/cli.rs`
2. Add the field to `CliConfig` in `src/cli.rs`
3. Update the `From<CliArgs>` impl for `CliConfig`
4. Pass the value through `Orchestrator::new()` in `src/orchestrator.rs`
5. Update `display_config()` in `src/cli.rs` to show the new flag
6. Update tests in `src/cli.rs`

## Done Marker Format

When a movie or series is fully processed, a `done.ext` file is written to the folder:

```json
{
  "finished_at": "2026-03-24T10:30:00Z",
  "version": "0.8.1"
}
```

The scanner reads this file and skips the folder unless `--force` is passed. Invalid JSON in a done marker is treated as missing (the folder will be reprocessed).

## Metadata Cache

Series metadata is cached in `.cache/` inside each series folder:

- `<series_name>.json` — TMDB series metadata (7-day TTL)
- `tvdb_<tvdb_id>.json` — TVDB Season 0 episode list (7-day TTL)
- `id_mappings/<tmdb_id>.json` — TMDB→TVDB ID mapping (no expiration)

Cache files are plain JSON. Delete them manually to force a refresh, or use `--force` to bypass the cache.

## Hardware Acceleration

The converter auto-detects available hardware encoders at startup by running `ffmpeg -encoders`. Priority order:

1. NVENC (NVIDIA GPU) — `hevc_nvenc`
2. QSV (Intel Quick Sync) — `hevc_qsv`
3. VideoToolbox (macOS) — `hevc_videotoolbox`
4. Software fallback — `libx265`

To force software encoding, you can temporarily rename or remove the hardware encoder from ffmpeg's available encoders list, or modify `converter.rs` directly.

## Temporary Files

During processing, temporary files are stored in `tmp_downloads/<title_year>/` relative to the working directory. These are cleaned up automatically on successful completion. If the process is interrupted, leftover temp directories are cleaned up on the next run before processing begins.

## Testing Approach

- **Unit tests:** Co-located in `#[cfg(test)]` blocks in each source file
- **Property-based tests:** Using `proptest` — each module has 2-8 property tests covering invariants
- **Integration tests:** In `tests/` — exercise the full pipeline with real filesystem operations
- **Network:** Integration tests avoid real network calls; unit tests mock API responses with inline JSON

When adding new functionality:
1. Write unit tests for the new logic
2. Add at least one property-based test if the function has mathematical invariants
3. Run `cargo test` to verify all tests pass
4. Run `cargo clippy -- -D warnings` to check for lint issues

---

_Generated using BMAD Method `document-project` workflow_
