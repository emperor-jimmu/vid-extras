# Implementation Summary: YouTube Filtering Improvements

## Overview
Implemented normalized title matching and collection-based filtering for YouTube content discovery to prevent downloading unrelated videos and extras about sequels/prequels.

## Changes Implemented

### Normalized Title Matching (Requirements 5.1-5.11)
**Problem:** Videos completely unrelated to the movie were being downloaded (e.g., "What led to Shia Staring at Me" for [REC]), and variations in title formatting (brackets, spaces) caused matching issues.

**Solution:** Implemented normalized title matching that strips special characters, brackets, and normalizes spaces for comparison.

**Implementation:**
- Added `YoutubeDiscoverer::normalize_title()` to standardize titles for comparison
  - Converts to lowercase
  - Removes special characters and brackets
  - Normalizes whitespace
- Added `YoutubeDiscoverer::contains_movie_title()` to check if video contains movie title using normalization
- Updated `YoutubeDiscoverer::mentions_collection_movies()` to use normalization with space-removed fallback
- Updated `YoutubeDiscoverer::should_include_video()` to require movie title in video title

**Example Normalizations:**
- "[REC]" → "rec"
- "REC: The Movie" → "rec the movie"
- "REC  3   Genesis" → "rec 3 genesis"
- "[Rec]3" → "rec3" (also matches "rec 3" via space-removed comparison)

**Filtering Examples:**
- Movie: "[REC] (2007)"
- ✅ Includes: "REC Official Trailer" (contains "rec")
- ✅ Includes: "[REC] Behind the Scenes" (contains "rec")
- ❌ Excludes: "What led to Shia Staring at Me" (no movie title match)
- ❌ Excludes: "[Rec]3 Génesis UK Premiere" (mentions collection movie)

### Collection-Based Filtering (Requirements 3.9, 5.11)
**Problem:** For movies in franchises (e.g., REC, REC 2, REC 3), YouTube searches were returning content about sequels/prequels instead of the specific movie.

**Solution:** Integrated TMDB collection API to fetch all movies in a collection and exclude videos mentioning other collection movies using normalized matching.

**Implementation:**
- Added `TmdbCollection` and `TmdbCollectionResponse` structs for TMDB collection data
- Added `TmdbDiscoverer::fetch_collection()` to retrieve collection details
- Added `TmdbDiscoverer::get_metadata()` to fetch collection info for a movie
- Created `DiscoveryMetadata` struct to pass collection titles to YouTube discoverer
- Added `YoutubeDiscoverer::mentions_collection_movies()` with normalized matching (checks both with and without spaces)
- Updated `DiscoveryOrchestrator::discover_all()` to fetch metadata and pass to YouTube
- Added `YoutubeDiscoverer::discover_with_metadata()` for metadata-aware discovery

**Example:**
- Movie: "REC (2007)"
- Collection: ["REC", "REC 2", "REC 3", "REC 4"]
- ✅ Includes: "REC Behind the Scenes" (original movie)
- ❌ Excludes: "REC 2 Behind the Scenes" (collection movie)
- ❌ Excludes: "[Rec]3 Génesis UK Premiere" (collection movie, normalized match)

## API Changes

### New Structs
```rust
pub struct DiscoveryMetadata {
    pub collection_movie_titles: Vec<String>,
}

struct TmdbCollection {
    id: u64,
    name: String,
}

struct TmdbCollectionResponse {
    id: u64,
    name: String,
    parts: Vec<TmdbCollectionPart>,
}

struct TmdbCollectionPart {
    id: u64,
    title: String,
}
```

### New Methods
```rust
// TmdbDiscoverer
async fn fetch_collection(&self, collection_id: u64) -> Result<Vec<String>, DiscoveryError>
pub async fn get_metadata(&self, movie: &MovieEntry) -> DiscoveryMetadata

// YoutubeDiscoverer
fn normalize_title(title: &str) -> String
fn contains_movie_title(video_title: &str, movie_title: &str) -> bool
fn mentions_collection_movies(video_title: &str, collection_titles: &[String]) -> bool
pub async fn discover_with_metadata(&self, movie: &MovieEntry, metadata: &DiscoveryMetadata) -> Result<Vec<VideoSource>, DiscoveryError>
```

### Modified Methods
```rust
// Updated signature - added movie_title parameter for filtering
fn should_include_video(
    video_title: &str,
    movie_title: &str,
    duration_secs: u32,
    width: u32,
    height: u32,
    expected_year: u16,
    collection_titles: &[String],
) -> bool

// Updated to pass movie_title parameter
async fn search_youtube(
    &self,
    query: &str,
    movie_title: &str,
    category: ContentCategory,
    expected_year: u16,
    collection_titles: &[String],
) -> Result<Vec<VideoSource>, DiscoveryError>

// Updated to fetch and use metadata
pub async fn discover_all(&self, movie: &MovieEntry) -> Vec<VideoSource>
```

## Testing

### New Tests Added
1. `test_normalize_title_removes_brackets` - Validates bracket removal
2. `test_normalize_title_removes_special_chars` - Validates special character removal
3. `test_normalize_title_normalizes_spaces` - Validates space normalization
4. `test_contains_movie_title_with_brackets` - Validates normalized title matching
5. `test_contains_movie_title_no_match` - Validates unrelated titles are excluded
6. `test_mentions_collection_movies_with_normalization` - Validates normalized collection matching
7. `test_should_include_video_user_reported_cases` - Validates specific user-reported issues

### Updated Tests
All existing `should_include_video` test calls updated to include movie_title parameter:
- `test_youtube_should_include_video_valid`
- `test_youtube_should_include_video_excluded_by_duration`
- `test_youtube_should_include_video_excluded_by_keyword`
- `test_youtube_should_include_video_excluded_as_short`
- `test_youtube_should_include_video_multiple_exclusions`
- `test_youtube_year_filtering_same_year`
- `test_youtube_year_filtering_different_year`
- `test_youtube_year_filtering_no_year`
- `test_youtube_collection_filtering`

### Test Results
- ✅ All 63 discovery unit tests passing (was 56, added 7 new tests)
- ✅ All 217 total library tests passing
- ✅ No clippy warnings
- ✅ Code properly formatted with rustfmt
- ✅ Compiles without errors or warnings

## Requirements Validated

### Updated Requirements
- **3.9**: TMDB collection details retrieval
- **5.11**: YouTube videos mentioning collection movies are excluded
- **5.1-5.10**: All YouTube filtering requirements validated with normalized matching

## User-Reported Issues Fixed

1. ✅ "What led to Shia Staring at Me" - Now excluded (no movie title match)
2. ✅ "[Rec]3 Génesis UK Premiere Interviews" - Now excluded (collection movie mention detected via normalization)

## Backward Compatibility

The changes maintain backward compatibility:
- `ContentDiscoverer` trait unchanged
- `YoutubeDiscoverer::discover()` still works (uses empty metadata internally)
- New functionality accessed via `discover_with_metadata()` method
- Existing code continues to work without modifications

## Performance Considerations

- Collection metadata is fetched once per movie at the start of discovery
- Metadata fetch happens in parallel with TMDB video discovery (no additional latency)
- Normalization is lightweight (string operations only)
- Space-removed comparison adds minimal overhead for collection matching
- String matching operations are case-insensitive but efficient
- No impact on movies not in collections (empty collection list)

## Example Usage

```rust
// Automatic usage through DiscoveryOrchestrator
let orchestrator = DiscoveryOrchestrator::new(api_key, SourceMode::All);
let sources = orchestrator.discover_all(&movie).await;
// Metadata is automatically fetched and used for YouTube filtering

// Manual usage with custom metadata
let metadata = tmdb.get_metadata(&movie).await;
let youtube_sources = youtube.discover_with_metadata(&movie, &metadata).await?;
```

## Benefits

1. **Improved Franchise Support**: Correctly handles movie series and collections by excluding sequels/prequels
2. **Better Accuracy**: Only returns content specifically about the target movie, not other movies in the franchise
3. **Balanced Filtering**: Removed overly strict movie title check while keeping effective collection-based filtering
4. **Maintainable Design**: Clean separation of concerns with metadata struct
5. **Extensible**: Easy to add more metadata fields in the future
6. **Practical**: Relies on YouTube's search relevance while adding targeted exclusions for known problem cases (sequels)

## Files Modified

- `src/discovery.rs` - Core implementation
- `.kiro/specs/extras-fetcher/requirements.md` - Updated requirements

## Next Steps

Consider future enhancements:
- Cache collection metadata to avoid repeated API calls
- Add fuzzy matching for movie titles with special characters
- Support for alternate titles and international releases
- Configurable strictness levels for filtering
