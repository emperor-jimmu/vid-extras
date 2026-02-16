# Design Document: TV Series Extra Content Support

## Overview

This design extends the extras_fetcher tool to support TV series extra content while maintaining backward compatibility with existing movie functionality. The architecture follows the same pipeline pattern (Scan → Discovery → Download → Conversion → Organization) but introduces new data models and logic to handle the hierarchical structure of TV series (Series → Season → Episode).

The key distinction in this design is between:

- **Specials**: Season 0 episodes that are part of the official episode list
- **Extras**: Bonus material (interviews, behind-the-scenes, bloopers, featurettes) organized at series or season level

### Design Goals

1. Extend existing architecture without breaking movie functionality
2. Support hierarchical TV series structure (Series → Season → Episode)
3. Maintain Jellyfin/Plex compatibility for directory structure
4. Reuse existing modules (downloader, converter) where possible
5. Follow SOLID principles and maintain code quality standards

## Architecture

### High-Level Pipeline

```
┌─────────────┐
│   Scanner   │ ──> Identifies movies AND series
└─────────────┘
       │
       ├──> Movies ──> [Existing Movie Pipeline]
       │
       └──> Series ──> [New Series Pipeline]
                            │
                            ├──> Discovery (TMDB + YouTube)
                            ├──> Download (yt-dlp)
                            ├──> Conversion (ffmpeg)
                            └──> Organization (Jellyfin structure)
```

### Module Responsibilities

- **Scanner**: Detect and classify folders as movies or series
- **Models**: Define SeriesEntry and SeriesExtra data structures
- **Discovery**: Find extras from TMDB and YouTube for series
- **Downloader**: Download videos (reused from movie pipeline)
- **Converter**: Convert to x265 (reused from movie pipeline)
- **Organizer**: Organize files into Jellyfin-compatible structure
- **Orchestrator**: Coordinate processing for both movies and series

## Components and Interfaces

### 1. Enhanced Scanner Module

The Scanner module will be extended to detect both movies and TV series.

**Detection Logic:**

```rust
pub enum MediaType {
    Movie,
    Series,
    Unknown,
}

impl Scanner {
    /// Detect whether a folder contains a movie or series
    fn detect_media_type(path: &Path) -> MediaType {
        // Check for season folders (Season 01, Season 02, etc.)
        if Self::has_season_folders(path) {
            return MediaType::Series;
        }

        // Check for video files directly in folder
        if Self::has_video_files(path) {
            return MediaType::Movie;
        }

        MediaType::Unknown
    }

    /// Check if directory contains season folders
    fn has_season_folders(path: &Path) -> bool {
        // Look for folders matching "Season XX" or "Season 00"
        let season_regex = Regex::new(r"^Season \d{2}$").unwrap();

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if season_regex.is_match(name) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}
```

**Folder Name Parsing:**

```rust
/// Parse series folder name
/// Formats: "Series Name (YYYY)" or "Series Name"
pub fn parse_series_folder_name(name: &str) -> Option<(String, Option<u16>)> {
    // Try with year first: "Series Name (YYYY)"
    let re_with_year = Regex::new(r"^(.+?)\s*\((\d{4})\)$").ok()?;

    if let Some(caps) = re_with_year.captures(name) {
        let title = caps.get(1)?.as_str().trim().to_string();
        let year = caps.get(2)?.as_str().parse::<u16>().ok()?;
        return Some((title, Some(year)));
    }

    // Try without year: just the series name
    if !name.is_empty() && !name.starts_with('.') {
        return Some((name.trim().to_string(), None));
    }

    None
}
```

### 2. New Data Models

**SeriesEntry:**

```rust
/// Represents a TV series discovered during library scanning
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesEntry {
    /// Path to the series folder
    pub path: PathBuf,
    /// Series title
    pub title: String,
    /// Optional release year
    pub year: Option<u16>,
    /// Whether a done marker exists
    pub has_done_marker: bool,
    /// List of season numbers found in the folder
    pub seasons: Vec<u8>,
}

impl fmt::Display for SeriesEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(year) = self.year {
            write!(f, "{} ({})", self.title, year)
        } else {
            write!(f, "{}", self.title)
        }
    }
}
```

**SeriesExtra:**

```rust
/// Represents an extra video for a TV series
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesExtra {
    /// Series identifier (for caching and organization)
    pub series_id: String,
    /// Optional season number (None = series-level extra)
    pub season_number: Option<u8>,
    /// Content category
    pub category: ContentCategory,
    /// Title/description
    pub title: String,
    /// Video URL
    pub url: String,
    /// Source type (TMDB, YouTube, etc.)
    pub source_type: SourceType,
    /// Local path after download (optional)
    pub local_path: Option<PathBuf>,
}
```

**ProcessingMode:**

```rust
/// Processing mode for media types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessingMode {
    /// Process both movies and series
    Both,
    /// Process only movies
    MoviesOnly,
    /// Process only series
    SeriesOnly,
}
```

### 3. Series Discovery Module

The discovery module will be extended with series-specific discoverers.

**TmdbSeriesDiscoverer:**

```rust
pub struct TmdbSeriesDiscoverer {
    api_key: String,
    client: reqwest::Client,
}

impl TmdbSeriesDiscoverer {
    /// Search for series by name and optional year
    pub async fn search_series(
        &self,
        title: &str,
        year: Option<u16>,
    ) -> Result<Option<i32>, DiscoveryError> {
        let encoded_title = urlencoding::encode(title);
        let mut url = format!(
            "https://api.themoviedb.org/3/search/tv?api_key={}&query={}",
            self.api_key, encoded_title
        );

        if let Some(y) = year {
            url.push_str(&format!("&first_air_date_year={}", y));
        }

        // Make API call and extract series ID
        // ...
    }
```

    /// Discover series-level extras from TMDB videos endpoint
    pub async fn discover_series_extras(
        &self,
        series_id: i32,
    ) -> Result<Vec<SeriesExtra>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/tv/{}/videos?api_key={}",
            series_id, self.api_key
        );

        // Fetch and map video types to categories
        // Similar to movie discovery but for series
        // ...
    }

    /// Discover Season 0 specials
    pub async fn discover_season_zero(
        &self,
        series_id: i32,
    ) -> Result<Vec<SpecialEpisode>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/tv/{}/season/0?api_key={}",
            series_id, self.api_key
        );

        // Fetch Season 0 episode list
        // Extract episode numbers, titles, air dates
        // ...
    }

}

````

**YoutubeSeriesDiscoverer:**

```rust
pub struct YoutubeSeriesDiscoverer;

impl YoutubeSeriesDiscoverer {
    /// Build search queries for series extras
    fn build_series_search_queries(
        title: &str,
        year: Option<u16>,
        season: Option<u8>,
    ) -> Vec<(String, ContentCategory)> {
        let mut queries = Vec::new();
        let year_str = year.map(|y| y.to_string()).unwrap_or_default();

        let base = if let Some(s) = season {
            format!("{} {} season {}", title, year_str, s)
        } else {
            format!("{} {}", title, year_str)
        };

        queries.push((
            format!("{} cast interview", base),
            ContentCategory::Interview,
        ));
        queries.push((
            format!("{} behind the scenes", base),
            ContentCategory::BehindTheScenes,
        ));
        queries.push((
            format!("{} bloopers", base),
            ContentCategory::Featurette,
        ));
        queries.push((
            format!("{} featurette", base),
            ContentCategory::Featurette,
        ));

        queries
    }
}
````

### 4. Series Organizer Module

The organizer will handle Jellyfin-compatible directory structure for series.

**Directory Structure:**

```
Series Name (YYYY)/
├── Season 00/                    # Specials
│   ├── Series Name - S00E01 - Special Title.mp4
│   └── Series Name - S00E02 - Another Special.mp4
├── Season 01/
│   ├── Series Name - S01E01 - Episode Title.mp4
│   └── ...
├── trailers/                     # Series-level extras
│   └── trailer1.mp4
├── interviews/
│   └── cast_interview.mp4
├── behind the scenes/
│   └── making_of.mp4
└── .extras_done                  # Done marker
```

**Season-Specific Extras:**

```
Series Name (YYYY)/
├── Season 01/
│   ├── Series Name - S01E01.mp4
│   ├── behind the scenes/       # Season 1 specific extras
│   │   └── s01_making_of.mp4
│   └── interviews/
│       └── s01_cast_interview.mp4
```

**SeriesOrganizer Implementation:**

```rust
pub struct SeriesOrganizer {
    series_path: PathBuf,
}

impl SeriesOrganizer {
    pub fn new(series_path: PathBuf) -> Self {
        Self { series_path }
    }

    /// Organize series extras into appropriate directories
    pub async fn organize_extras(
        &self,
        extras: Vec<ConversionResult>,
        season: Option<u8>,
    ) -> Result<(), OrganizerError> {
        for extra in extras {
            if !extra.success {
                continue;
            }

            // Determine target directory
            let target_dir = if let Some(s) = season {
                // Season-specific extra
                self.series_path
                    .join(format!("Season {:02}", s))
                    .join(extra.category.subdirectory())
            } else {
                // Series-level extra
                self.series_path.join(extra.category.subdirectory())
            };

            // Create directory if needed
            tokio::fs::create_dir_all(&target_dir).await?;

            // Move file
            let filename = extra.output_path.file_name().unwrap();
            let target_path = target_dir.join(filename);
            tokio::fs::rename(&extra.output_path, &target_path).await?;
        }

        Ok(())
    }
```

    /// Organize Season 0 specials
    pub async fn organize_specials(
        &self,
        series_name: &str,
        specials: Vec<SpecialEpisode>,
    ) -> Result<(), OrganizerError> {
        let season_00_dir = self.series_path.join("Season 00");
        tokio::fs::create_dir_all(&season_00_dir).await?;

        for special in specials {
            if let Some(local_path) = special.local_path {
                // Format: "Series Name - S00E01 - Episode Title.mp4"
                let filename = format!(
                    "{} - S00E{:02} - {}.mp4",
                    series_name,
                    special.episode_number,
                    Self::sanitize_filename(&special.title)
                );

                let target_path = season_00_dir.join(filename);
                tokio::fs::rename(&local_path, &target_path).await?;
            }
        }

        Ok(())
    }

    /// Sanitize filename by removing invalid characters
    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect()
    }

}

````

### 5. Enhanced Orchestrator

The orchestrator will coordinate processing for both movies and series.

```rust
pub struct Orchestrator {
    scanner: Scanner,
    movie_discovery: Arc<DiscoveryOrchestrator>,
    series_discovery: Arc<SeriesDiscoveryOrchestrator>,
    downloader: Arc<Downloader>,
    converter: Arc<Converter>,
    processing_mode: ProcessingMode,
    concurrency: usize,
}

impl Orchestrator {
    pub async fn run(&self) -> Result<ProcessingSummary, OrchestratorError> {
        // Scan for both movies and series
        let (movies, series) = self.scanner.scan_all()?;

        let mut summary = ProcessingSummary::new();

        // Process movies if enabled
        if self.processing_mode != ProcessingMode::SeriesOnly {
            let movie_results = self.process_movies(movies).await;
            summary.add_movie_results(movie_results);
        }

        // Process series if enabled
        if self.processing_mode != ProcessingMode::MoviesOnly {
            let series_results = self.process_series(series).await;
            summary.add_series_results(series_results);
        }

        Ok(summary)
    }
````

    async fn process_series(&self, series_list: Vec<SeriesEntry>) -> Vec<SeriesResult> {
        // Similar to process_movies but for series
        // For each series:
        //   1. Discover extras (TMDB + YouTube)
        //   2. Download videos
        //   3. Convert to x265
        //   4. Organize into Jellyfin structure
        //   5. Create done marker
        // ...
    }

}

````

## Data Models

### Core Structures

```rust
// In src/models.rs

/// TV Series entry
pub struct SeriesEntry {
    pub path: PathBuf,
    pub title: String,
    pub year: Option<u16>,
    pub has_done_marker: bool,
    pub seasons: Vec<u8>,
}

/// Series extra video
pub struct SeriesExtra {
    pub series_id: String,
    pub season_number: Option<u8>,
    pub category: ContentCategory,
    pub title: String,
    pub url: String,
    pub source_type: SourceType,
    pub local_path: Option<PathBuf>,
}

/// Season 0 special episode
pub struct SpecialEpisode {
    pub episode_number: u8,
    pub title: String,
    pub air_date: Option<String>,
    pub url: Option<String>,
    pub local_path: Option<PathBuf>,
}

/// Media type classification
pub enum MediaType {
    Movie,
    Series,
    Unknown,
}

/// Processing mode
pub enum ProcessingMode {
    Both,
    MoviesOnly,
    SeriesOnly,
}
````

### Enhanced Processing Summary

```rust
pub struct ProcessingSummary {
    // Existing movie fields
    pub total_movies: usize,
    pub successful_movies: usize,
    pub failed_movies: usize,

    // New series fields
    pub total_series: usize,
    pub successful_series: usize,
    pub failed_series: usize,

    // Combined statistics
    pub total_downloads: usize,
    pub total_conversions: usize,
}
```

## Correctness Properties

_A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees._

### Property Reflection

After analyzing all acceptance criteria, I've identified the following redundancies and consolidations:

**Redundancies Eliminated:**

- Requirements 1.1 and 1.2 both test folder name parsing → Combined into Property 1
- Requirements 2.3 and 2.4 both test season number interpretation → Combined into Property 3
- Requirements 5.6-5.8 duplicate movie filtering logic → Reference existing movie properties
- Requirements 9.1-9.5 duplicate movie done marker logic → Reference existing movie properties
- Requirements 11.1-11.5 duplicate movie parallel processing → Reference existing movie properties

**Properties Consolidated:**

- TMDB type mappings (3.4-3.8) → Single comprehensive property
- YouTube query construction (5.1-5.5) → Single property covering all query types
- Directory organization (7.1-7.9) → Single property for category-to-subdirectory mapping
- Season 0 organization (8.1-8.5) → Combined into file naming and placement property

This reflection ensures each property provides unique validation value without logical redundancy.

### Series-Specific Properties

**Property 1: Series Folder Name Parsing**

_For any_ folder name matching the patterns "{Series Name} (YYYY)" or "{Series Name}", parsing should correctly extract the series title and optional year, and the extracted values should be valid.

**Validates: Requirements 1.1, 1.2**

**Property 2: Series Done Marker Skipping**

_For any_ series folder containing a valid done marker file, when the force flag is not set, the scanner should exclude that series from the processing queue; when the force flag is set, the series should be included.

**Validates: Requirements 1.3, 9.1, 9.3, 9.4**

**Property 3: Season Number Interpretation**

_For any_ SeriesExtra object, if season_number is None, the extra should be organized at the series root level; if season_number is Some(n), the extra should be organized under the Season n folder.

**Validates: Requirements 2.3, 2.4**

**Property 4: SeriesExtra Serialization Round-Trip**

_For any_ valid SeriesExtra object, serializing to JSON then deserializing should produce an equivalent object with all fields preserved.

**Validates: Requirements 2.6**

**Property 5: TMDB Video Type Mapping Completeness**

_For any_ TMDB video type in the set {Trailer, Behind the Scenes, Featurette, Bloopers}, the discovery module should map it to a valid ContentCategory; for unknown types, the video should be skipped without error.

**Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8**

**Property 6: Season 0 Episode Separation**

_For any_ series discovery result, Season 0 episodes should be stored in a separate collection from regular extras, and the two collections should not overlap.

**Validates: Requirements 4.5**

**Property 7: YouTube Series Query Construction**

_For any_ series with title and optional year, the YouTube discoverer should construct search queries for all content types (interviews, behind-the-scenes, bloopers, featurettes) with the correct format including series name and year.

**Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5**

**Property 8: Season-Specific Query Tagging**

_For any_ season-specific search query, discovered results should be tagged with the correct season number, and the season number should be included in the search query string.

**Validates: Requirements 6.1, 6.2, 6.3**

**Property 9: Content Category to Subdirectory Mapping**

_For any_ SeriesExtra with a given ContentCategory, the organizer should place the file in the correct subdirectory (trailers, featurettes, behind the scenes, deleted scenes, or interviews) at either the series or season level based on the season_number field.

**Validates: Requirements 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7**

**Property 10: Season 0 File Naming Format**

_For any_ Season 0 special episode, the organized file should follow the naming format "{Series Name} - S00E{episode_number} - {episode_title}.mp4" with zero-padded episode numbers and sanitized titles.

**Validates: Requirements 8.1, 8.2, 8.3, 8.4**

**Property 11: Media Type Detection Consistency**

_For any_ directory, if it contains season folders (Season 01, Season 02, etc.), it should be classified as a Series; if it contains video files directly, it should be classified as a Movie; the classification should be deterministic and consistent across multiple scans.

**Validates: Requirements 10.1, 10.2, 10.3**

**Property 12: Processing Mode Filtering**

_For any_ configured ProcessingMode, the orchestrator should process only the specified media types: MoviesOnly should skip all series, SeriesOnly should skip all movies, and Both should process everything.

**Validates: Requirements 12.1, 12.2, 12.3**

**Property 13: Series Error Isolation**

_For any_ series that fails during discovery, download, conversion, or organization, other series in the processing queue should continue processing without interruption, and the error should be logged with the series identifier.

**Validates: Requirements 13.1, 13.2, 13.3, 13.4, 13.5, 13.6**

**Property 14: Metadata Cache Freshness**

_For any_ series metadata cached on disk, if the cache age is less than 7 days, the cached data should be used; if the cache age is 7 days or older, fresh metadata should be fetched from TMDB; if the force flag is set, the cache should be ignored regardless of age.

**Validates: Requirements 14.1, 14.2, 14.3, 14.4**

**Property 15: Season Pack File Identification**

_For any_ file extracted from a season pack, if the filename matches patterns for bonus content (behind the scenes, deleted scene, interview, featurette, blooper), it should be classified with the correct ContentCategory and organized into the appropriate subdirectory.

**Validates: Requirements 15.2, 15.3, 15.4, 15.5, 15.6, 15.7, 15.8**

**Property 16: Local Season 0 Import**

_For any_ file in a series folder matching the pattern S00E{number}, the scanner should identify it as a Season 0 episode, and if it's not already in the Season 00 folder, it should be moved there with the correct naming format.

**Validates: Requirements 16.1, 16.2, 16.3**

**Property 17: Fuzzy Title Matching Threshold**

_For any_ two strings being compared for title matching, after normalization (lowercase, special character removal), if the Levenshtein distance similarity is above 80%, they should be considered a match; if below 80%, they should be considered non-matching.

**Validates: Requirements 17.1, 17.2, 17.3, 17.4**

**Property 18: Series Summary Statistics Accuracy**

_For any_ set of series processing results, the summary statistics should accurately reflect the counts: total_series should equal successful_series + failed_series, and total_downloads and total_conversions should equal the sum across all series.

**Validates: Requirements 19.1, 19.2, 19.3, 19.4, 19.5**

**Property 19: Backward Compatibility Preservation**

_For any_ library containing only movies (no series folders), processing should produce identical results to the previous version, including the same done marker format, directory structure, and file organization.

**Validates: Requirements 20.1, 20.2, 20.5**

### Reused Properties from Movie Implementation

The following properties are already validated by the existing movie implementation and apply equally to series:

- **YouTube Duration Filtering**: 30 seconds ≤ duration ≤ 1200 seconds (Requirements 5.6, 5.7)
- **YouTube Keyword Filtering**: Exclude videos with Review, Reaction, Analysis, Explained, Ending, Theory, React (Requirements 5.8)
- **YouTube Shorts Exclusion**: Exclude videos < 60s with vertical aspect ratio (Requirements 5.8)
- **Done Marker JSON Format**: ISO 8601 timestamp and version field (Requirements 9.2)
- **Parallel Processing Concurrency**: At most N items processed simultaneously (Requirements 11.2, 11.3, 11.4)
- **Sequential Downloads Within Item**: Downloads for a single item execute sequentially (Requirements 11.4)

## Error Handling

### Error Types

New error types will be added to `src/error.rs`:

```rust
/// Series-specific scanning errors
#[derive(Debug, thiserror::Error)]
pub enum SeriesScanError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid series folder structure: {0}")]
    InvalidStructure(String),

    #[error("Failed to parse series name: {0}")]
    ParseError(String),
}

/// Series discovery errors
#[derive(Debug, thiserror::Error)]
pub enum SeriesDiscoveryError {
    #[error("TMDB API error: {0}")]
    TmdbApi(String),

    #[error("YouTube search error: {0}")]
    YoutubeSearch(String),

    #[error("Series not found: {0}")]
    NotFound(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

/// Series organization errors
#[derive(Debug, thiserror::Error)]
pub enum SeriesOrganizerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid season number: {0}")]
    InvalidSeason(u8),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
}
```

### Error Handling Strategy

1. **Graceful Degradation**: Errors in one series should not stop processing of other series
2. **Detailed Logging**: All errors logged with context (series name, operation, error details)
3. **Error Propagation**: Use `?` operator for clean error propagation within functions
4. **User-Friendly Messages**: Convert technical errors to actionable user messages
5. **Partial Success**: Allow partial completion (e.g., some extras downloaded, others failed)

### Error Recovery

- **TMDB API Failures**: Log error, continue with YouTube-only discovery
- **YouTube Failures**: Log error, continue with TMDB-only discovery
- **Download Failures**: Log error, continue with remaining downloads
- **Conversion Failures**: Preserve original file, log error, continue
- **Organization Failures**: Don't create done marker, log error for manual intervention

## Testing Strategy

### Dual Testing Approach

This feature will use both unit tests and property-based tests for comprehensive coverage:

**Unit Tests:**

- Specific examples of series folder parsing
- TMDB API response parsing with mock JSON
- Season folder detection edge cases
- File naming and sanitization
- Error handling scenarios
- Integration points between modules

**Property-Based Tests:**

- Universal properties across all inputs (100+ iterations per test)
- Series folder name parsing correctness
- Done marker skipping behavior
- Season number interpretation
- Serialization round-trips
- Media type detection consistency
- Processing mode filtering
- Error isolation between series

### Property Test Configuration

All property tests will:

- Run minimum 100 iterations (due to randomization)
- Use `proptest` crate for input generation
- Tag tests with feature name and property number
- Reference design document properties in comments

**Example Property Test:**

```rust
// Feature: tv-series-extras, Property 1: Series Folder Name Parsing
// Validates: Requirements 1.1, 1.2
proptest! {
    #[test]
    fn prop_series_folder_name_parsing(
        title in "[a-zA-Z0-9 :',&!?.-]{1,100}",
        year in proptest::option::of(1900u16..2100u16)
    ) {
        let title_trimmed = title.trim();
        if title_trimmed.is_empty() {
            return Ok(());
        }

        let folder_name = if let Some(y) = year {
            format!("{} ({})", title_trimmed, y)
        } else {
            title_trimmed.to_string()
        };

        let parsed = Scanner::parse_series_folder_name(&folder_name);
        prop_assert!(parsed.is_some());

        let (parsed_title, parsed_year) = parsed.unwrap();
        prop_assert_eq!(&parsed_title, title_trimmed);
        prop_assert_eq!(parsed_year, year);
    }
}
```

### Test Organization

Tests will be organized by module:

- `src/scanner.rs`: Series detection and parsing tests
- `src/models.rs`: SeriesEntry and SeriesExtra tests
- `src/discovery/series_tmdb.rs`: TMDB series discovery tests
- `src/discovery/series_youtube.rs`: YouTube series discovery tests
- `src/organizer.rs`: Series organization tests
- `src/orchestrator.rs`: Series orchestration tests
- `tests/series_integration_tests.rs`: End-to-end series processing tests

### Integration Tests

Comprehensive integration tests will cover:

1. Complete series processing pipeline (scan → discover → download → convert → organize)
2. Mixed library processing (movies + series)
3. Processing mode filtering (movies-only, series-only, both)
4. Done marker behavior with series
5. Error recovery and isolation
6. Backward compatibility with movie-only libraries

## Implementation Notes

### SOLID Principles Application

**Single Responsibility Principle:**

- Scanner: Only responsible for detecting and classifying media
- SeriesDiscoverer: Only responsible for finding series extras
- SeriesOrganizer: Only responsible for organizing series files
- Each module has one reason to change

**Open/Closed Principle:**

- MediaType enum allows extension without modifying Scanner
- ContentDiscoverer trait allows new discovery sources without changing orchestrator
- ProcessingMode enum allows new modes without changing core logic

**Liskov Substitution Principle:**

- All ContentDiscoverer implementations (TMDB, YouTube, Archive.org) are substitutable
- SeriesEntry and MovieEntry can be processed through similar pipelines

**Interface Segregation Principle:**

- Separate traits for discovery, downloading, conversion, organization
- Modules only depend on interfaces they use

**Dependency Inversion Principle:**

- Orchestrator depends on ContentDiscoverer trait, not concrete implementations
- Downloader and Converter are reused without modification

### Code Quality Considerations

**Readability:**

- Clear naming: SeriesEntry, SeriesExtra, SpecialEpisode
- Self-documenting code with minimal comments
- Consistent formatting with rustfmt

**Simplicity:**

- Reuse existing downloader and converter modules
- Avoid premature optimization
- Straightforward control flow

**Encapsulation:**

- Private helper functions for parsing and validation
- Public APIs expose only necessary functionality
- Internal implementation details hidden

**Error Handling:**

- Custom error types with thiserror
- Descriptive error messages
- Graceful degradation on failures

### Performance Considerations

**Parallel Processing:**

- Series processed in parallel (configurable concurrency)
- Downloads within a series remain sequential
- Semaphore-based concurrency control

**Caching:**

- TMDB metadata cached to disk (7-day TTL)
- Reduces API calls on repeated runs
- Cache invalidation with force flag

**Async I/O:**

- All file operations use tokio::fs
- Network requests use async reqwest
- Non-blocking throughout pipeline

### Backward Compatibility

**Preserving Movie Functionality:**

- No changes to existing MovieEntry struct
- Movie processing pipeline unchanged
- Same done marker format for movies
- Same CLI interface for movie-only mode

**Migration Path:**

- Existing movie libraries work without changes
- New series functionality opt-in via flags
- Mixed libraries supported automatically

## API Endpoints

### TMDB API Endpoints

**TV Series Search:**

```
GET https://api.themoviedb.org/3/search/tv
Parameters:
  - api_key: string (required)
  - query: string (required, URL-encoded series name)
  - first_air_date_year: integer (optional, year filter)
  - page: integer (optional, default 1)

Response: JSON with results array containing series objects
```

**TV Series Videos:**

```
GET https://api.themoviedb.org/3/tv/{series_id}/videos
Parameters:
  - api_key: string (required)

Response: JSON with results array containing video objects
Fields: id, key, name, site, type, official
```

**TV Season Details:**

```
GET https://api.themoviedb.org/3/tv/{series_id}/season/{season_number}
Parameters:
  - api_key: string (required)

Response: JSON with season details including episodes array
Episode fields: episode_number, name, air_date, overview
```

### YouTube Search (via yt-dlp)

**Search Command:**

```bash
yt-dlp --dump-json --skip-download "ytsearch5:{query}"
```

**Output:** JSON lines with video metadata

- id: YouTube video ID
- title: Video title
- duration: Duration in seconds
- width/height: Video dimensions
- url: Video URL

## CLI Interface Extensions

### New Flags

```bash
# Processing mode flags
--series-only          Process only TV series, skip movies
--movies-only          Process only movies, skip series (default behavior)

# Series-specific flags
--season-extras        Enable season-specific extras discovery (default: off)
--specials             Enable Season 0 specials discovery (default: off)

# Type override flag
--type <movie|series>  Force classification of root directory
```

### Updated Help Text

```
extras_fetcher [OPTIONS] <ROOT_DIRECTORY>

OPTIONS:
    --force                Ignore done markers and reprocess all items
    --mode <MODE>          Content source mode [all, youtube] (default: all)
    --concurrency <N>      Max concurrent items to process (default: 2)
    --verbose              Enable verbose logging output
    --series-only          Process only TV series
    --movies-only          Process only movies
    --season-extras        Enable season-specific extras discovery
    --specials             Enable Season 0 specials discovery
    --type <TYPE>          Force media type classification [movie, series]
    --help                 Display this help message
    --version              Display version information
```

### Example Usage

```bash
# Process both movies and series
extras_fetcher /media/library

# Process only series with season-specific extras
extras_fetcher --series-only --season-extras /media/library

# Process single series folder with specials
extras_fetcher --type series --specials "/media/library/Breaking Bad (2008)"

# Force reprocess all series with verbose output
extras_fetcher --series-only --force --verbose /media/library
```

## File Structure Examples

### Series with Extras

```
Breaking Bad (2008)/
├── Season 01/
│   ├── Breaking Bad - S01E01 - Pilot.mkv
│   ├── Breaking Bad - S01E02 - Cat's in the Bag.mkv
│   └── ...
├── Season 02/
│   ├── Breaking Bad - S02E01 - Seven Thirty-Seven.mkv
│   └── ...
├── trailers/
│   ├── breaking_bad_season_1_trailer.mp4
│   └── breaking_bad_season_2_trailer.mp4
├── behind the scenes/
│   ├── making_of_breaking_bad.mp4
│   └── chemistry_consultant_interview.mp4
├── interviews/
│   ├── bryan_cranston_interview.mp4
│   └── vince_gilligan_interview.mp4
└── .extras_done
```

### Series with Season-Specific Extras

```
Game of Thrones (2011)/
├── Season 01/
│   ├── Game of Thrones - S01E01 - Winter Is Coming.mkv
│   ├── behind the scenes/
│   │   ├── s01_making_of_episode_1.mp4
│   │   └── s01_costume_design.mp4
│   └── interviews/
│       └── s01_cast_roundtable.mp4
├── Season 02/
│   ├── Game of Thrones - S02E01 - The North Remembers.mkv
│   ├── behind the scenes/
│   │   └── s02_battle_of_blackwater.mp4
│   └── deleted scenes/
│       └── s02_deleted_scene_1.mp4
├── trailers/
│   └── got_series_trailer.mp4
└── .extras_done
```

### Series with Season 0 Specials

```
Doctor Who (2005)/
├── Season 00/
│   ├── Doctor Who - S00E01 - The Christmas Invasion.mkv
│   ├── Doctor Who - S00E02 - The Runaway Bride.mkv
│   └── Doctor Who - S00E03 - Voyage of the Damned.mkv
├── Season 01/
│   ├── Doctor Who - S01E01 - Rose.mkv
│   └── ...
├── trailers/
│   └── doctor_who_trailer.mp4
└── .extras_done
```

### Mixed Library Structure

```
/media/library/
├── Movies/
│   ├── The Matrix (1999)/
│   │   ├── The Matrix (1999).mkv
│   │   ├── trailers/
│   │   └── .extras_done
│   └── Inception (2010)/
│       ├── Inception (2010).mkv
│       └── behind the scenes/
├── TV Shows/
│   ├── Breaking Bad (2008)/
│   │   ├── Season 01/
│   │   ├── trailers/
│   │   └── .extras_done
│   └── The Wire (2002)/
│       ├── Season 01/
│       └── interviews/
```

## Migration Strategy

### Phase 1: Core Infrastructure (Weeks 1-2)

- Add SeriesEntry and SeriesExtra models
- Extend Scanner with media type detection
- Add ProcessingMode enum and CLI flags
- Update Orchestrator to handle both media types

### Phase 2: Series Discovery (Weeks 3-4)

- Implement TmdbSeriesDiscoverer
- Implement YoutubeSeriesDiscoverer
- Add metadata caching
- Implement fuzzy title matching

### Phase 3: Organization (Week 5)

- Implement SeriesOrganizer
- Add Season 0 handling
- Add season-specific extras support
- Implement file naming and sanitization

### Phase 4: Testing (Week 6)

- Write unit tests for all modules
- Write property-based tests (19 properties)
- Write integration tests
- Test backward compatibility

### Phase 5: Polish (Week 7)

- Add progress reporting for series
- Update documentation
- Performance optimization
- Bug fixes and refinement
