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
- `discovery.rs` - **[IMPLEMENTED]** Multi-source content discovery (TMDB, Archive.org, and YouTube complete; DiscoveryOrchestrator pending)
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
**Status:** ✅ TMDB implementation complete

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

#### Discovery Module - Archive.org (src/discovery.rs)
**Status:** ✅ Archive.org implementation complete

**Functionality:**
- Year-based conditional querying (only for movies < 2010)
- Search query construction with title and EPK/Making of subjects
- Result parsing and category mapping
- Graceful error handling with detailed logging
- URL encoding for safe API requests

**Public API:**
```rust
pub struct ArchiveOrgDiscoverer {
    // Creates Archive.org discoverer
    pub fn new() -> Self;
    
    // Builds search query for a movie
    fn build_query(title: &str) -> String;
    
    // Maps Archive.org subjects to content categories
    fn map_subjects(subjects: &[String]) -> Option<ContentCategory>;
}

impl ContentDiscoverer for ArchiveOrgDiscoverer {
    // Discovers video sources from Archive.org for a movie
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}
```

**Test Coverage:**
- 11 unit tests covering query construction, subject mapping, and API response parsing
- 2 property-based tests with 100+ iterations each:
  - Property 8: Archive.org Year-Based Querying
  - Property 9: Archive.org Query Construction
- All tests passing ✅

**Requirements Validated:**
- 4.1-4.2: Year-based querying (< 2010)
- 4.3-4.7: Query construction and result parsing

#### Discovery Module - YouTube (src/discovery.rs)
**Status:** ✅ YouTube implementation complete

**Functionality:**
- yt-dlp search integration with `ytsearch5:` operator
- Search query construction for 4 content types (deleted scenes, behind the scenes, bloopers, cast interviews)
- Duration filtering (30s - 20min range)
- Keyword filtering for 7 excluded terms (Review, Reaction, Analysis, Explained, Ending, Theory, React)
- YouTube Shorts detection (duration < 60s AND vertical aspect ratio)
- Graceful error handling with detailed logging
- Async search using tokio process execution

**Public API:**
```rust
pub struct YoutubeDiscoverer;

impl YoutubeDiscoverer {
    // Creates YouTube discoverer
    pub fn new() -> Self;
    
    // Builds search queries for different content types
    fn build_search_queries(title: &str, year: u16) -> Vec<(String, ContentCategory)>;
    
    // Checks if video title contains excluded keywords
    fn contains_excluded_keywords(title: &str) -> bool;
    
    // Validates duration is within acceptable range (30s - 20min)
    fn is_duration_valid(duration_secs: u32) -> bool;
    
    // Detects YouTube Shorts (duration < 60s and vertical aspect ratio)
    fn is_youtube_short(duration_secs: u32, width: u32, height: u32) -> bool;
    
    // Filters video based on all criteria
    fn should_include_video(title: &str, duration_secs: u32, width: u32, height: u32) -> bool;
}

impl ContentDiscoverer for YoutubeDiscoverer {
    // Discovers video sources from YouTube for a movie
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}
```

**Filtering Logic:**
- Duration: 30s ≤ duration ≤ 1200s (20 minutes)
- Keywords: Excludes Review, Reaction, Analysis, Explained, Ending, Theory, React (case-insensitive)
- Shorts: Excludes videos < 60s with vertical aspect ratio (height > width)

**Test Coverage:**
- 23 unit tests covering:
  - Search query construction and category mapping
  - All 7 keyword filters with case-insensitivity
  - Duration validation (valid range, too short, too long)
  - YouTube Shorts detection (vertical, horizontal, square, duration boundaries)
  - Complete video filtering logic with multiple exclusion criteria
- 4 property-based tests with 100+ iterations each:
  - Property 10: YouTube Always Queried
  - Property 11: YouTube Duration Filtering
  - Property 12: YouTube Keyword Filtering
  - Property 13: YouTube Shorts Exclusion
- All tests passing ✅

**Dependencies:**
- Uses `tokio::process::Command` for yt-dlp execution
- Uses `serde_json` for parsing yt-dlp JSON output
- Uses `log` for structured logging
- Test dependencies: `proptest`, `tokio`

**Requirements Validated:**
- 5.1: YouTube always queried regardless of other sources
- 5.2: yt-dlp ytsearch operator integration
- 5.3-5.6: Search query construction for different content types
- 5.7-5.8: Duration filtering (30s - 20min)
- 5.9: Keyword filtering
- 5.10: YouTube Shorts exclusion
- 5.11: Graceful error handling

**Pending Implementation:**
- DiscoveryOrchestrator (to coordinate all three sources)

### Pending Modules

The following modules are defined but not yet implemented:
- Discovery module - DiscoveryOrchestrator (to coordinate TMDB, Archive.org, and YouTube)
- Downloader module
- Converter module
- Organizer module
- Orchestrator module
- CLI module
