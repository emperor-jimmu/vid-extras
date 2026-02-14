# Project Structure

## Directory Layout
```
extras_fetcher/
├── src/              # Source code modules
├── target/           # Build artifacts (gitignored)
├── .kiro/            # Kiro configuration and specs
│   ├── specs/        # Feature specifications
│   └── steering/     # Project guidance documents
├── Cargo.toml        # Project manifest and dependencies
├── Cargo.lock        # Dependency lock file
└── .gitignore        # Git ignore rules
```

## Module Architecture

The codebase follows a pipeline architecture with clear separation of concerns:

### Core Modules (src/)

- `main.rs` - Entry point, module declarations
- `cli.rs` - Command-line interface, argument parsing, configuration
- `error.rs` - Centralized error types using thiserror (one enum per module)
- `orchestrator.rs` - Main pipeline coordinator, manages processing flow
- `scanner.rs` - Directory traversal, movie discovery, folder name parsing
- `discovery.rs` - Multi-source content discovery (TMDB, Archive.org, YouTube)
- `downloader.rs` - Video downloading via yt-dlp
- `converter.rs` - Video format conversion with ffmpeg, hardware acceleration detection
- `organizer.rs` - File organization into Jellyfin structure, done marker management
- `validation.rs` - Dependency checking (binaries, API keys, codec support)

## Processing Pipeline

The tool follows this execution flow:
1. Validation - Check dependencies and configuration
2. Scanning - Discover movies in library
3. Discovery - Find extras from multiple sources
4. Downloading - Fetch videos with yt-dlp
5. Conversion - Convert to x265 format
6. Organization - Move to Jellyfin directories and mark complete

## Code Conventions

- Each module has a focused responsibility
- Error types are defined per-module in error.rs using thiserror
- Async operations use tokio runtime
- Structs use builder pattern where configuration is complex
- Public APIs are documented with doc comments
- Test files use proptest for property-based testing
