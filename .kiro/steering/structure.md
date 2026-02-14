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
- `models.rs` - Shared data structures (MovieEntry, VideoSource, DoneMarker, enums)
- `orchestrator.rs` - Main pipeline coordinator, manages processing flow
- `scanner.rs` - **[IMPLEMENTED]** Directory traversal, movie discovery, folder name parsing
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

## Implementation Status

### Completed Modules

#### Scanner Module (src/scanner.rs)
**Status:** ✅ Fully implemented and tested

**Functionality:**
- Recursive directory traversal with depth-first search
- Folder name parsing using regex pattern `^(.+?)\s*\((\d{4})\)$`
- Done marker detection with JSON validation
- Force flag support to override done marker skipping
- Comprehensive error handling for invalid paths and permissions

**Public API:**
```rust
pub struct Scanner {
    // Creates scanner with root directory and force flag
    pub fn new(root_dir: PathBuf, force: bool) -> Self;
    
    // Scans directory tree and returns list of movies to process
    pub fn scan(&self) -> Result<Vec<MovieEntry>, ScanError>;
    
    // Parses folder name to extract title and year
    pub fn parse_folder_name(name: &str) -> Option<(String, u16)>;
}
```

**Test Coverage:**
- 11 unit tests covering edge cases (empty dirs, nested structures, invalid names)
- 3 property-based tests with 100+ iterations each:
  - Property 1: Folder Name Parsing Correctness
  - Property 3: Done Marker Skipping Behavior  
  - Property 6: Recursive Directory Traversal Completeness
- All tests passing ✅

**Dependencies:**
- Uses `regex` crate for folder name pattern matching
- Uses `serde_json` for done marker validation
- Test dependencies: `proptest`, `tempfile`

### Pending Modules

The following modules are defined but not yet implemented:
- Validation module
- Discovery module (TMDB, Archive.org, YouTube)
- Downloader module
- Converter module
- Organizer module
- Orchestrator module
- CLI module
