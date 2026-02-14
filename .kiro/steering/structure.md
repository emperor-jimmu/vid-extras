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
- `discovery.rs` - **[TMDB IMPLEMENTED]** Multi-source content discovery (TMDB complete, Archive.org and YouTube pending)
- `downloader.rs` - Video downloading via yt-dlp
- `converter.rs` - Video format conversion with ffmpeg, hardware acceleration detection
- `organizer.rs` - File organization into Jellyfin structure, done marker management
- `validation.rs` - **[IMPLEMENTED]** Dependency checking (binaries, API keys, codec support)

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

#### Validation Module (src/validation.rs)
**Status:** ✅ Fully implemented and tested

**Functionality:**
- System dependency verification (yt-dlp, ffmpeg binaries)
- FFmpeg HEVC codec support detection (libx265, hevc_nvenc, hevc_qsv)
- TMDB API key validation from environment variables
- Descriptive error reporting for missing dependencies
- Zero-sized type for efficient validation checks

**Public API:**
```rust
pub struct Validator;

impl Validator {
    // Create a new Validator instance
    pub fn new() -> Self;
    
    // Validate all dependencies and return TMDB API key if successful
    pub fn validate_dependencies(&self) -> Result<String, ValidationError>;
    
    // Check if a binary exists in system PATH
    fn check_binary_exists(&self, name: &str) -> bool;
    
    // Check if ffmpeg supports HEVC encoding
    fn check_ffmpeg_hevc_support(&self) -> bool;
    
    // Check if TMDB API key is configured
    fn check_tmdb_api_key(&self) -> Result<String, ValidationError>;
}
```

**Test Coverage:**
- 11 unit tests covering all validation scenarios
- 4 property-based tests with 100+ iterations each:
  - Property 32: Dependency Validation at Startup
  - Property 34: Missing Dependency Error Reporting
- All tests passing ✅

**Dependencies:**
- Uses `std::process::Command` for binary verification
- Uses `std::env` for environment variable access
- Test dependencies: `proptest`

**Requirements Validated:**
- 11.1: yt-dlp binary verification
- 11.2: ffmpeg binary verification
- 11.3: ffmpeg HEVC support detection
- 11.4: TMDB API key validation
- 11.5: Descriptive error messages
- 10.5: Clear error reporting

#### Discovery Module - TMDB (src/discovery.rs)
**Status:** ✅ TMDB implementation complete (Archive.org and YouTube pending)

**Functionality:**
- Movie search by title and year via TMDB API
- Video list fetching from TMDB movie endpoints
- Type-to-category mapping for 5 content types
- YouTube URL construction from TMDB video keys
- Graceful error handling with detailed logging
- URL encoding for safe API requests

**Public API:**
```rust
pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}

pub struct TmdbDiscoverer {
    // Creates TMDB discoverer with API key
    pub fn new(api_key: String) -> Self;
    
    // Maps TMDB video types to content categories
    pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory>;
}

impl ContentDiscoverer for TmdbDiscoverer {
    // Discovers video sources from TMDB for a movie
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}
```

**Type Mappings:**
- "Trailer" → ContentCategory::Trailer
- "Behind the Scenes" → ContentCategory::BehindTheScenes
- "Deleted Scene" → ContentCategory::DeletedScene
- "Featurette" → ContentCategory::Featurette
- "Bloopers" → ContentCategory::Featurette

**Test Coverage:**
- 16 unit tests covering:
  - API response parsing with mock JSON
  - URL construction and encoding
  - Error handling scenarios
  - All type mappings including unknown types
  - Special character handling
- 1 property-based test with 100+ iterations:
  - Property 7: TMDB Video Type Mapping
- All tests passing ✅

**Dependencies:**
- Uses `reqwest` for HTTP API calls
- Uses `serde` and `serde_json` for JSON parsing
- Uses `urlencoding` for safe URL construction
- Uses `log` for structured logging
- Test dependencies: `proptest`, `tokio`

**Requirements Validated:**
- 3.1: Movie search by title and year
- 3.2: TMDB movie identifier retrieval
- 3.3: Videos list fetching
- 3.4-3.8: Type-to-category mapping for all video types
- 3.9: Graceful API error handling

**API Endpoints Used:**
- Search: `https://api.themoviedb.org/3/search/movie`
- Videos: `https://api.themoviedb.org/3/movie/{id}/videos`

**Pending Implementation:**
- Archive.org discoverer (for movies < 2010)
- YouTube discoverer (with duration and keyword filtering)
- DiscoveryOrchestrator (to coordinate all sources)

### Pending Modules

The following modules are defined but not yet implemented:
- Discovery module - Archive.org and YouTube discoverers
- Downloader module
- Converter module
- Organizer module
- Orchestrator module
- CLI module
