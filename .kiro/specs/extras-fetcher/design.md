# Design Document: extras_fetcher

## Overview

The extras_fetcher is a Rust-based CLI tool that automates the enrichment of Jellyfin movie libraries by discovering, downloading, and organizing supplementary video content. The system follows a pipeline architecture with five distinct phases: Library Scanning, Content Discovery, Acquisition, Processing, and Organization.

The tool is built using Rust 2024 edition and emphasizes robustness, idempotency, and user-friendly CLI output. It integrates with external services (TMDB, Archive.org, YouTube) and system binaries (yt-dlp, ffmpeg) to provide a comprehensive solution for media library management.

## Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Interface                            │
│  (Argument Parsing, Configuration, Colored Output)              │
└────────────────────────┬────────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────────┐
│                    Orchestrator / Main Loop                      │
│  (Movie Queue Management, Phase Coordination)                   │
└─┬──────────┬──────────┬──────────┬──────────┬──────────────────┘
  │          │          │          │          │
  ▼          ▼          ▼          ▼          ▼
┌───────┐ ┌────────┐ ┌──────────┐ ┌─────────┐ ┌──────────────┐
│Scanner│ │Discovery│ │Downloader│ │Converter│ │Organizer     │
│       │ │         │ │          │ │         │ │              │
│Phase 1│ │Phase 2  │ │Phase 3   │ │Phase 4  │ │Phase 5       │
└───┬───┘ └────┬────┘ └─────┬────┘ └────┬────┘ └──────┬───────┘
    │          │            │           │             │
    │          ├─TMDB       │           │             │
    │          ├─Archive.org│           │             │
    │          └─YouTube    │           │             │
    │                       │           │             │
    ▼                       ▼           ▼             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    File System & External Tools                  │
│  (Directory I/O, yt-dlp, ffmpeg, Done Markers)                  │
└─────────────────────────────────────────────────────────────────┘
```

### Design Principles

1. **Idempotency**: The tool can be run multiple times safely; done markers prevent reprocessing
2. **Fail-Safe**: Errors in one movie don't affect others; temporary files are cleaned up
3. **Modularity**: Each phase is independent and testable
4. **User Experience**: Rich CLI output with colors and progress indicators
5. **Performance**: Optional parallel processing for multiple movies

## Components and Interfaces

### 1. CLI Module

**Responsibility**: Parse command-line arguments, validate configuration, display help/version information.

**Interface**:
```rust
pub struct CliConfig {
    pub root_directory: PathBuf,
    pub force: bool,
    pub mode: SourceMode,
    pub concurrency: usize,
    pub verbose: bool,
}

pub enum SourceMode {
    All,
    YoutubeOnly,
}

pub fn parse_args() -> Result<CliConfig, CliError>;
pub fn display_banner(version: &str);
pub fn display_config(config: &CliConfig);
```

**Dependencies**: clap crate for argument parsing, colored crate for terminal colors

### 2. Scanner Module

**Responsibility**: Recursively scan directory tree, parse movie folder names, check for done markers, build processing queue.

**Interface**:
```rust
pub struct MovieEntry {
    pub path: PathBuf,
    pub title: String,
    pub year: u16,
    pub has_done_marker: bool,
}

pub struct Scanner {
    root_dir: PathBuf,
    force: bool,
}

impl Scanner {
    pub fn new(root_dir: PathBuf, force: bool) -> Self;
    pub fn scan(&self) -> Result<Vec<MovieEntry>, ScanError>;
    fn parse_folder_name(name: &str) -> Option<(String, u16)>;
    fn check_done_marker(path: &Path) -> bool;
}
```

**Folder Name Parsing**: Use regex pattern `^(.+?)\s*\((\d{4})\)$` to extract title and year

### 3. Discovery Module

**Responsibility**: Query multiple sources (TMDB, Archive.org, YouTube) to find extra content URLs.

**Interface**:
```rust
pub struct VideoSource {
    pub url: String,
    pub source_type: SourceType,
    pub category: ContentCategory,
    pub title: String,
}

pub enum SourceType {
    TMDB,
    ArchiveOrg,
    YouTube,
}

pub enum ContentCategory {
    Trailer,
    Featurette,
    BehindTheScenes,
    DeletedScene,
}

pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}

pub struct TmdbDiscoverer {
    api_key: String,
    client: reqwest::Client,
}

pub struct ArchiveOrgDiscoverer {
    client: reqwest::Client,
}

pub struct YoutubeDiscoverer {
    // Uses yt-dlp for search
}

pub struct DiscoveryOrchestrator {
    tmdb: TmdbDiscoverer,
    archive: ArchiveOrgDiscoverer,
    youtube: YoutubeDiscoverer,
    mode: SourceMode,
}

impl DiscoveryOrchestrator {
    pub async fn discover_all(&self, movie: &MovieEntry) -> Vec<VideoSource>;
}
```

**TMDB Integration**:
- Endpoint: `https://api.themoviedb.org/3/search/movie`
- Endpoint: `https://api.themoviedb.org/3/movie/{id}/videos`
- Map video types: Trailer→Trailer, "Behind the Scenes"→BehindTheScenes, "Deleted Scene"→DeletedScene, Featurette→Featurette, Bloopers→Featurette

**Archive.org Integration**:
- Only for movies with year < 2010
- Endpoint: `https://archive.org/advancedsearch.php`
- Query: `title:"{title}" AND (subject:"EPK" OR subject:"Making of")`
- Collection: `collection:moviesandfilms`

**YouTube Integration**:
- Use yt-dlp with `ytsearch5:` prefix for searches
- Search queries: "{title} {year} deleted scenes", "{title} {year} behind the scenes", "{title} {year} bloopers", "{title} {year} cast interview"
- Filtering logic:
  - Duration: 30s ≤ duration ≤ 30min
  - Exclude titles containing: "Review", "Reaction", "Analysis", "Explained", "Ending", "Theory", "React"
  - Exclude YouTube Shorts (duration < 60s and aspect ratio ~9:16)

### 4. Downloader Module

**Responsibility**: Download videos using yt-dlp, manage temporary storage, handle failures.

**Interface**:
```rust
pub struct Downloader {
    temp_base: PathBuf,
}

pub struct DownloadResult {
    pub source: VideoSource,
    pub local_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

impl Downloader {
    pub fn new(temp_base: PathBuf) -> Self;
    pub async fn download(&self, movie_id: &str, sources: Vec<VideoSource>) -> Vec<DownloadResult>;
    fn create_temp_dir(&self, movie_id: &str) -> Result<PathBuf, IoError>;
    async fn download_single(&self, source: &VideoSource, dest: &Path) -> Result<PathBuf, DownloadError>;
    fn cleanup_failed(&self, path: &Path);
}
```

**yt-dlp Integration**:
- Command: `yt-dlp -o "%(title)s.%(ext)s" <url>`
- Check exit code; delete partial files on failure
- Timeout: 5 minutes per download

### 5. Converter Module

**Responsibility**: Convert downloaded videos to x265 format using ffmpeg with hardware acceleration.

**Interface**:
```rust
pub struct Converter {
    hw_accel: HardwareAccel,
}

pub enum HardwareAccel {
    Nvenc,  // NVIDIA
    Qsv,    // Intel Quick Sync
    Software,
}

pub struct ConversionResult {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

impl Converter {
    pub fn new() -> Self;
    pub async fn convert_batch(&self, downloads: Vec<DownloadResult>) -> Vec<ConversionResult>;
    async fn convert_single(&self, input: &Path, output: &Path) -> Result<(), ConversionError>;
    fn detect_hardware_accel() -> HardwareAccel;
    fn build_ffmpeg_command(&self, input: &Path, output: &Path) -> Command;
}
```

**ffmpeg Command Construction**:
- Software: `ffmpeg -i input.mp4 -c:v libx265 -crf 25 -c:a copy output.mp4`
- NVENC: `ffmpeg -hwaccel cuda -i input.mp4 -c:v hevc_nvenc -crf 25 -c:a copy output.mp4`
- QSV: `ffmpeg -hwaccel qsv -i input.mp4 -c:v hevc_qsv -global_quality 25 -c:a copy output.mp4`

**Cleanup Strategy**:
- On success: delete original download, keep converted file
- On failure: delete failed output, keep original for inspection

### 6. Organizer Module

**Responsibility**: Move converted files to appropriate Jellyfin subdirectories, create done markers.

**Interface**:
```rust
pub struct Organizer {
    movie_path: PathBuf,
}

impl Organizer {
    pub fn new(movie_path: PathBuf) -> Self;
    pub fn organize(&self, conversions: Vec<ConversionResult>) -> Result<(), OrganizerError>;
    fn ensure_subdirectory(&self, category: ContentCategory) -> Result<PathBuf, IoError>;
    fn move_file(&self, source: &Path, dest: &Path) -> Result<(), IoError>;
    fn create_done_marker(&self) -> Result<(), IoError>;
    fn cleanup_temp_dir(&self, temp_dir: &Path) -> Result<(), IoError>;
}
```

**Subdirectory Mapping**:
- Trailer → `/trailers`
- Featurette → `/featurettes`
- BehindTheScenes → `/behind the scenes`
- DeletedScene → `/deleted scenes`

**Done Marker Format**:
```json
{
  "finished_at": "2024-01-15T10:30:00Z",
  "version": "0.1.0"
}
```

### 7. Orchestrator Module

**Responsibility**: Coordinate all phases, manage movie queue, handle parallel processing.

**Interface**:
```rust
pub struct Orchestrator {
    config: CliConfig,
    scanner: Scanner,
    discovery: DiscoveryOrchestrator,
    downloader: Downloader,
    converter: Converter,
}

impl Orchestrator {
    pub fn new(config: CliConfig) -> Result<Self, InitError>;
    pub async fn run(&self) -> Result<ProcessingSummary, OrchestratorError>;
    async fn process_movie(&self, movie: MovieEntry) -> Result<MovieResult, ProcessingError>;
}

pub struct ProcessingSummary {
    pub total_movies: usize,
    pub successful: usize,
    pub failed: usize,
    pub total_downloads: usize,
    pub total_conversions: usize,
}
```

**Parallel Processing**:
- Use tokio for async runtime
- Use semaphore to limit concurrent movie processing
- Default concurrency: 2 movies at a time
- Sequential downloads within each movie

### 8. Validation Module

**Responsibility**: Validate dependencies and configuration at startup.

**Interface**:
```rust
pub struct Validator;

impl Validator {
    pub fn validate_dependencies() -> Result<(), ValidationError>;
    fn check_binary_exists(name: &str) -> bool;
    fn check_ffmpeg_hevc_support() -> bool;
    fn check_tmdb_api_key() -> Result<String, ValidationError>;
}
```

**Validation Checks**:
1. yt-dlp in PATH
2. ffmpeg in PATH
3. ffmpeg supports libx265 or hevc_nvenc or hevc_qsv
4. TMDB_API_KEY environment variable set

## Data Models

### Core Data Structures

```rust
// Movie representation
pub struct MovieEntry {
    pub path: PathBuf,
    pub title: String,
    pub year: u16,
    pub has_done_marker: bool,
}

// Video source from discovery
pub struct VideoSource {
    pub url: String,
    pub source_type: SourceType,
    pub category: ContentCategory,
    pub title: String,
}

// Download result
pub struct DownloadResult {
    pub source: VideoSource,
    pub local_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

// Conversion result
pub struct ConversionResult {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

// Done marker file content
#[derive(Serialize, Deserialize)]
pub struct DoneMarker {
    pub finished_at: String,  // ISO 8601 timestamp
    pub version: String,
}
```

### Enumerations

```rust
pub enum SourceMode {
    All,
    YoutubeOnly,
}

pub enum SourceType {
    TMDB,
    ArchiveOrg,
    YouTube,
}

pub enum ContentCategory {
    Trailer,
    Featurette,
    BehindTheScenes,
    DeletedScene,
}

pub enum HardwareAccel {
    Nvenc,
    Qsv,
    Software,
}
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("Invalid root directory: {0}")]
    InvalidRootDir(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid folder name: {0}")]
    InvalidFolderName(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("yt-dlp failed: {0}")]
    YtDlpFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("ffmpeg failed: {0}")]
    FfmpegFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Missing binary: {0}")]
    MissingBinary(String),
    #[error("Missing API key: {0}")]
    MissingApiKey(String),
    #[error("Unsupported codec")]
    UnsupportedCodec,
}
```


## Correctness Properties

A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.

### Property 1: Folder Name Parsing Correctness

*For any* folder name matching the pattern "Title (Year)", parsing should extract the title and year correctly, and the extracted values should reconstruct a valid folder name.

**Validates: Requirements 1.7**

### Property 2: Done Marker Round-Trip

*For any* valid DoneMarker struct, serializing to JSON then deserializing should produce an equivalent struct with a valid ISO 8601 timestamp.

**Validates: Requirements 2.2**

### Property 3: Done Marker Skipping Behavior

*For any* movie folder containing a valid done marker file (when --force is not set), the folder should be excluded from the processing queue.

**Validates: Requirements 1.8, 2.3, 12.1**

### Property 4: Force Flag Overrides Done Markers

*For any* movie folder containing a done marker file, when --force flag is set, the folder should be included in the processing queue.

**Validates: Requirements 1.4**

### Property 5: Mode Filtering

*For any* set of discovered video sources, when mode is set to YoutubeOnly, all non-YouTube sources should be filtered out.

**Validates: Requirements 1.5**

### Property 6: Recursive Directory Traversal Completeness

*For any* directory tree structure, scanning should visit every subdirectory at least once.

**Validates: Requirements 1.6**

### Property 7: TMDB Video Type Mapping

*For any* TMDB video entry, the mapping from TMDB type to ContentCategory should be: Trailer→Trailer, "Behind the Scenes"→BehindTheScenes, "Deleted Scene"→DeletedScene, Featurette→Featurette, Bloopers→Featurette.

**Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8**

### Property 8: Archive.org Year-Based Querying

*For any* movie, Archive.org should be queried if and only if the release year is less than 2010.

**Validates: Requirements 4.1, 4.2**

### Property 9: Archive.org Query Construction

*For any* movie title, the Archive.org search query should contain the pattern: title:"{title}" AND (subject:"EPK" OR subject:"Making of").

**Validates: Requirements 4.4**

### Property 10: YouTube Always Queried

*For any* movie, YouTube discovery should be executed regardless of results from other sources.

**Validates: Requirements 5.1**

### Property 11: YouTube Duration Filtering

*For any* YouTube video, it should be excluded from results if duration > 20 minutes OR duration < 30 seconds.

**Validates: Requirements 5.7, 5.8**

### Property 12: YouTube Keyword Filtering

*For any* YouTube video title containing any of ["Review", "Reaction", "Analysis", "Explained", "Ending", "Theory", "React"], the video should be excluded from results.

**Validates: Requirements 5.9**

### Property 13: YouTube Shorts Exclusion

*For any* YouTube video identified as a Short (duration < 60s and aspect ratio ~9:16), it should be excluded from results.

**Validates: Requirements 5.10**

### Property 14: Temporary Directory Creation

*For any* movie ID, initiating downloads should create a temporary directory at /tmp_downloads/{movie_id}/.

**Validates: Requirements 6.1**

### Property 15: Download Failure Cleanup

*For any* download that fails (non-zero exit code), any partial files created should be deleted.

**Validates: Requirements 6.4**

### Property 16: Download Error Continuation

*For any* failed download within a movie's content list, processing should continue with remaining downloads.

**Validates: Requirements 6.5**

### Property 17: Network Timeout Graceful Handling

*For any* network timeout during download, the system should handle it without panicking or crashing.

**Validates: Requirements 6.6**

### Property 18: FFmpeg Codec Usage

*For any* video conversion, the ffmpeg command should specify x265/HEVC codec (libx265, hevc_nvenc, or hevc_qsv).

**Validates: Requirements 7.1**

### Property 19: CRF Value Range

*For any* video conversion, the CRF value should be between 24 and 26 (inclusive).

**Validates: Requirements 7.2**

### Property 20: Hardware Acceleration Selection

*For any* system with detected hardware acceleration (NVENC or QSV), the converter should use the hardware encoder; otherwise, it should fall back to software encoding.

**Validates: Requirements 7.3, 11.6**

### Property 21: Conversion Success Cleanup

*For any* successful video conversion, the original raw download file should be deleted.

**Validates: Requirements 7.4**

### Property 22: Conversion Failure Preservation

*For any* failed video conversion, the original download file should be retained and the failed output should be deleted.

**Validates: Requirements 7.5, 7.6**

### Property 23: Content Category to Subdirectory Mapping

*For any* converted file with a ContentCategory, it should be moved to the correct subdirectory: Trailer→/trailers, Featurette→/featurettes, BehindTheScenes→/behind the scenes, DeletedScene→/deleted scenes.

**Validates: Requirements 8.1, 8.2, 8.3, 8.4**

### Property 24: Subdirectory Creation

*For any* target subdirectory that doesn't exist, it should be created before moving files into it.

**Validates: Requirements 8.5**

### Property 25: Temp Folder Cleanup on Success

*For any* movie that completes organization successfully, the temporary download folder should be deleted.

**Validates: Requirements 8.6**

### Property 26: Done Marker Creation on Completion

*For any* movie that completes all phases successfully, a done marker file should be created in the movie folder.

**Validates: Requirements 2.1, 8.7**

### Property 27: Sequential Downloads Within Movie

*For any* movie being processed, downloads should execute sequentially (no overlapping downloads for the same movie).

**Validates: Requirements 9.1**

### Property 28: Concurrency Limit Enforcement

*For any* configured concurrency limit N, at most N movies should be processed simultaneously.

**Validates: Requirements 9.3, 9.4**

### Property 29: Error Isolation Between Movies

*For any* movie that fails processing, other movies in the queue should continue processing unaffected.

**Validates: Requirements 10.2**

### Property 30: Temp Folder Cleanup on Exit

*For any* system exit (normal or abnormal), no temporary files should remain in /tmp_downloads/ locations.

**Validates: Requirements 10.3**

### Property 31: Pre-existing Temp Cleanup

*For any* movie about to be processed, if its temp folder contains files from a previous run, those files should be cleaned before starting new downloads.

**Validates: Requirements 10.4**

### Property 32: Dependency Validation at Startup

*For any* system startup, validation should verify that yt-dlp, ffmpeg, and TMDB API key are all available before processing begins.

**Validates: Requirements 11.1, 11.2, 11.4**

### Property 33: FFmpeg HEVC Support Validation

*For any* system startup, validation should verify that ffmpeg supports at least one of: libx265, hevc_nvenc, or hevc_qsv.

**Validates: Requirements 11.3**

### Property 34: Missing Dependency Error Reporting

*For any* missing required dependency, the system should exit with an error message that identifies which specific dependency is missing.

**Validates: Requirements 11.5, 10.5**

### Property 35: Idempotent Re-execution

*For any* library directory, running the tool multiple times should only process folders without done markers (unless --force is used).

**Validates: Requirements 12.2, 12.3**

### Property 36: Configuration Display Completeness

*For any* CLI configuration, displaying the config should show all values: root_directory, mode, force flag, concurrency, and verbose flag.

**Validates: Requirements 13.2**

### Property 37: Error Message Formatting

*For any* error that occurs during processing, the error message should include the movie title and operation type.

**Validates: Requirements 10.1, 13.7**

### Property 38: Verbose Flag Effect

*For any* operation, when --verbose flag is set, output should contain more detailed information than when the flag is not set.

**Validates: Requirements 13.8**


## Error Handling

### Error Handling Strategy

The extras_fetcher follows a "fail-safe" error handling approach where errors in processing one movie do not affect others, and the system always attempts to clean up resources.

### Error Categories

**1. Fatal Errors (System Exit)**
- Missing required binaries (yt-dlp, ffmpeg)
- Missing TMDB API key
- FFmpeg lacks HEVC support
- Invalid root directory path

These errors prevent the tool from functioning and should cause immediate exit with descriptive error messages.

**2. Recoverable Errors (Log and Continue)**
- TMDB API request failures
- Archive.org API request failures
- YouTube search failures
- Individual download failures
- Individual conversion failures
- Network timeouts

These errors are logged with context (movie title, operation) and processing continues with remaining items.

**3. Cleanup Errors (Log Warning)**
- Failed to delete temporary files
- Failed to delete original after conversion

These errors are logged as warnings but don't stop processing.

### Error Propagation

```rust
// Fatal errors propagate to main
fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let validator = Validator::new();
    validator.validate_dependencies()?;  // Fatal if fails
    
    let orchestrator = Orchestrator::new(config)?;
    let summary = orchestrator.run().await?;
    
    display_summary(&summary);
    Ok(())
}

// Recoverable errors are caught and logged
async fn process_movie(&self, movie: MovieEntry) -> MovieResult {
    let sources = match self.discovery.discover_all(&movie).await {
        Ok(s) => s,
        Err(e) => {
            log::error!("Discovery failed for {}: {}", movie.title, e);
            return MovieResult::failed(movie, "discovery");
        }
    };
    
    // Continue with downloads even if some sources failed
    let downloads = self.downloader.download_all(sources).await;
    // Filter out failed downloads, continue with successful ones
    let successful: Vec<_> = downloads.into_iter()
        .filter(|d| d.success)
        .collect();
    
    // ... continue processing
}
```

### Logging Strategy

Use the `log` crate with `env_logger` for structured logging:

- **ERROR**: Fatal errors and recoverable errors that prevent operations
- **WARN**: Cleanup failures, fallback to software encoding
- **INFO**: Phase transitions, movie processing start/end, summary statistics
- **DEBUG**: API requests, command executions, file operations (when --verbose)

### Resource Cleanup

**Guaranteed Cleanup Points**:
1. After successful conversion: delete original download
2. After successful organization: delete temp folder
3. On download failure: delete partial files
4. On conversion failure: delete failed output
5. On system exit: clean all temp folders (use Drop trait)

**Implementation**:
```rust
struct TempDirGuard {
    path: PathBuf,
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            if let Err(e) = fs::remove_dir_all(&self.path) {
                log::warn!("Failed to cleanup temp dir {:?}: {}", self.path, e);
            }
        }
    }
}
```

### Timeout Handling

- Download timeout: 5 minutes per file (configurable)
- API request timeout: 30 seconds
- Use tokio::time::timeout for async operations

```rust
match timeout(Duration::from_secs(300), download_file(url)).await {
    Ok(Ok(path)) => path,
    Ok(Err(e)) => return Err(DownloadError::Failed(e)),
    Err(_) => return Err(DownloadError::Timeout),
}
```

## Testing Strategy

### Dual Testing Approach

The extras_fetcher uses both unit tests and property-based tests for comprehensive coverage:

- **Unit tests**: Verify specific examples, edge cases, and integration points
- **Property tests**: Verify universal properties across randomized inputs

### Property-Based Testing

**Library**: Use `proptest` crate for property-based testing in Rust

**Configuration**: Each property test should run minimum 100 iterations

**Test Tagging**: Each property test must include a comment referencing the design property:
```rust
// Feature: extras-fetcher, Property 1: Folder Name Parsing Correctness
#[test]
fn prop_folder_name_parsing() {
    // test implementation
}
```

**Property Test Examples**:

```rust
use proptest::prelude::*;

// Property 1: Folder name parsing round-trip
proptest! {
    // Feature: extras-fetcher, Property 1: Folder Name Parsing Correctness
    #[test]
    fn prop_folder_name_round_trip(
        title in "[a-zA-Z0-9 ]{1,50}",
        year in 1900u16..2100u16
    ) {
        let folder_name = format!("{} ({})", title.trim(), year);
        let parsed = parse_folder_name(&folder_name);
        
        prop_assert!(parsed.is_some());
        let (parsed_title, parsed_year) = parsed.unwrap();
        prop_assert_eq!(parsed_title, title.trim());
        prop_assert_eq!(parsed_year, year);
    }
}

// Property 2: Done marker serialization round-trip
proptest! {
    // Feature: extras-fetcher, Property 2: Done Marker Round-Trip
    #[test]
    fn prop_done_marker_round_trip(timestamp in any::<i64>()) {
        let marker = DoneMarker {
            finished_at: format_iso8601(timestamp),
            version: "0.1.0".to_string(),
        };
        
        let json = serde_json::to_string(&marker).unwrap();
        let deserialized: DoneMarker = serde_json::from_str(&json).unwrap();
        
        prop_assert_eq!(marker.finished_at, deserialized.finished_at);
        prop_assert_eq!(marker.version, deserialized.version);
        
        // Verify ISO 8601 format
        prop_assert!(DateTime::parse_from_rfc3339(&deserialized.finished_at).is_ok());
    }
}

// Property 7: TMDB type mapping
proptest! {
    // Feature: extras-fetcher, Property 7: TMDB Video Type Mapping
    #[test]
    fn prop_tmdb_type_mapping(tmdb_type in prop_oneof![
        Just("Trailer"),
        Just("Behind the Scenes"),
        Just("Deleted Scene"),
        Just("Featurette"),
        Just("Bloopers"),
    ]) {
        let category = map_tmdb_type(&tmdb_type);
        
        match tmdb_type {
            "Trailer" => prop_assert_eq!(category, ContentCategory::Trailer),
            "Behind the Scenes" => prop_assert_eq!(category, ContentCategory::BehindTheScenes),
            "Deleted Scene" => prop_assert_eq!(category, ContentCategory::DeletedScene),
            "Featurette" => prop_assert_eq!(category, ContentCategory::Featurette),
            "Bloopers" => prop_assert_eq!(category, ContentCategory::Featurette),
            _ => unreachable!(),
        }
    }
}

// Property 11: YouTube duration filtering
proptest! {
    // Feature: extras-fetcher, Property 11: YouTube Duration Filtering
    #[test]
    fn prop_youtube_duration_filter(duration_secs in 0u32..3600u32) {
        let should_exclude = duration_secs < 30 || duration_secs > 1200;
        let actual_excluded = !passes_duration_filter(duration_secs);
        
        prop_assert_eq!(should_exclude, actual_excluded);
    }
}
```

### Unit Testing

**Focus Areas**:
1. CLI argument parsing edge cases (empty strings, special characters)
2. Regex pattern matching for folder names
3. API response parsing (mock HTTP responses)
4. File system operations (use tempdir for isolation)
5. Command construction for yt-dlp and ffmpeg
6. Error message formatting

**Unit Test Examples**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_help_flag_displays_usage() {
        let args = vec!["extras_fetcher", "--help"];
        let result = parse_args_from(args);
        assert!(matches!(result, Err(CliError::HelpRequested)));
    }
    
    #[test]
    fn test_version_flag_displays_version() {
        let args = vec!["extras_fetcher", "--version"];
        let result = parse_args_from(args);
        assert!(matches!(result, Err(CliError::VersionRequested)));
    }
    
    #[test]
    fn test_invalid_folder_name_returns_none() {
        assert_eq!(parse_folder_name("No Year Here"), None);
        assert_eq!(parse_folder_name("(2020)"), None);
        assert_eq!(parse_folder_name("Movie (abcd)"), None);
    }
    
    #[test]
    fn test_archive_org_query_construction() {
        let movie = MovieEntry {
            title: "The Matrix".to_string(),
            year: 1999,
            path: PathBuf::from("/movies/The Matrix (1999)"),
            has_done_marker: false,
        };
        
        let query = build_archive_query(&movie);
        assert!(query.contains("title:\"The Matrix\""));
        assert!(query.contains("subject:\"EPK\""));
        assert!(query.contains("subject:\"Making of\""));
    }
    
    #[test]
    fn test_youtube_keyword_filtering() {
        assert!(should_exclude_title("Movie Review"));
        assert!(should_exclude_title("Ending Explained"));
        assert!(should_exclude_title("React to Trailer"));
        assert!(!should_exclude_title("Official Trailer"));
        assert!(!should_exclude_title("Behind the Scenes"));
    }
    
    #[test]
    fn test_ffmpeg_command_with_nvenc() {
        let converter = Converter { hw_accel: HardwareAccel::Nvenc };
        let cmd = converter.build_ffmpeg_command(
            Path::new("input.mp4"),
            Path::new("output.mp4")
        );
        
        let cmd_str = format!("{:?}", cmd);
        assert!(cmd_str.contains("hevc_nvenc"));
        assert!(cmd_str.contains("-crf") || cmd_str.contains("-global_quality"));
    }
}
```

### Integration Testing

**Test Scenarios**:
1. End-to-end processing with mock APIs and file system
2. Parallel processing with multiple movies
3. Interruption and resumption (simulate crashes)
4. Error recovery (API failures, download failures)

**Mock Strategy**:
- Use `mockito` for HTTP API mocking (TMDB, Archive.org)
- Use `tempdir` for isolated file system testing
- Use `assert_cmd` for CLI integration tests

### Test Organization

```
tests/
├── unit/
│   ├── cli_tests.rs
│   ├── scanner_tests.rs
│   ├── discovery_tests.rs
│   ├── downloader_tests.rs
│   ├── converter_tests.rs
│   └── organizer_tests.rs
├── property/
│   ├── parsing_properties.rs
│   ├── filtering_properties.rs
│   ├── mapping_properties.rs
│   └── idempotency_properties.rs
└── integration/
    ├── end_to_end_tests.rs
    ├── parallel_processing_tests.rs
    └── error_recovery_tests.rs
```

### Continuous Testing

- Run unit tests on every commit
- Run property tests (100 iterations) on every commit
- Run integration tests on pull requests
- Use GitHub Actions or similar CI/CD

### Test Coverage Goals

- Unit test coverage: >80% of non-trivial functions
- Property test coverage: All 38 correctness properties implemented
- Integration test coverage: All major workflows (happy path, error paths, edge cases)
