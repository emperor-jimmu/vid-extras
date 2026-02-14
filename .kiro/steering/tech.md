# Technology Stack

## Language & Edition
- Rust 2024 edition
- Async runtime: Tokio with full features

## Core Dependencies
- `clap` (4.5) - CLI argument parsing with derive macros
- `tokio` (1.41) - Async runtime with full feature set
- `reqwest` (0.12) - HTTP client with JSON support
- `serde` (1.0) + `serde_json` - Serialization/deserialization
- `thiserror` (2.0) - Error type derivation
- `regex` (1.11) - Pattern matching
- `colored` (2.1) - Terminal output formatting
- `log` (0.4) + `env_logger` (0.11) - Logging infrastructure

## Testing
- `proptest` (1.5) - Property-based testing framework

## External Tools
- `yt-dlp` - Video downloading (must be in PATH)
- `ffmpeg` - Video conversion with HEVC/x265 support (must be in PATH)

## Build Commands
```bash
# Build the project
cargo build

# Build optimized release binary
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run

# Check for errors without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy
```

## Environment Variables
- `TMDB_API_KEY` - Required for TMDB content discovery
- `RUST_LOG` - Controls logging verbosity (debug, info, warn, error)
