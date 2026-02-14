# Implementation Summary: YouTube Filtering Improvements

## Overview
Implemented two critical filtering improvements to the YouTube content discovery system to reduce false positives and improve content relevance.

## Changes Implemented

### 1. Movie Title Filtering (Requirement 5.11)
**Problem:** YouTube searches were returning videos about other movies that happened to match the search terms.

**Solution:** Added `contains_movie_title()` method that verifies the movie title appears in the video title before including it in results.

**Implementation:**
- Added case-insensitive movie title matching in `YoutubeDiscoverer::contains_movie_title()`
- Updated `should_include_video()` to check movie title presence as first filter
- Videos without the movie title in their title are now excluded

**Example:**
- Query: "REC behind the scenes"
- ✅ Includes: "REC Behind the Scenes Featurette"
- ❌ Excludes: "Some Other Movie Behind the Scenes"

### 2. Collection-Based Filtering (Requirements 3.9, 5.12)
**Problem:** For movies in franchises (e.g., REC, REC 2, REC 3), YouTube searches were returning content about sequels/prequels instead of the specific movie.

**Solution:** Integrated TMDB collection API to fetch all movies in a collection and exclude videos mentioning other collection movies.

**Implementation:**
- Added `TmdbCollection` and `TmdbCollectionResponse` structs for TMDB collection data
- Added `TmdbDiscoverer::fetch_collection()` to retrieve collection details
- Added `TmdbDiscoverer::get_metadata()` to fetch collection info for a movie
- Created `DiscoveryMetadata` struct to pass collection titles to YouTube discoverer
- Added `YoutubeDiscoverer::mentions_collection_movies()` to detect sequel/prequel mentions
- Updated `DiscoveryOrchestrator::discover_all()` to fetch metadata and pass to YouTube
- Added `YoutubeDiscoverer::discover_with_metadata()` for metadata-aware discovery

**Example:**
- Movie: "REC (2007)"
- Collection: ["REC", "REC 2", "REC 3", "REC 4"]
- ✅ Includes: "REC Behind the Scenes"
- ❌ Excludes: "REC 2 Behind the Scenes"
- ❌ Excludes: "REC 3 Genesis Deleted Scenes"

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
fn contains_movie_title(video_title: &str, movie_title: &str) -> bool
fn mentions_collection_movies(video_title: &str, collection_titles: &[String]) -> bool
pub async fn discover_with_metadata(&self, movie: &MovieEntry, metadata: &DiscoveryMetadata) -> Result<Vec<VideoSource>, DiscoveryError>
```

### Modified Methods
```rust
// Updated signature to include new parameters
fn should_include_video(
    video_title: &str,
    movie_title: &str,  // NEW
    duration_secs: u32,
    width: u32,
    height: u32,
    expected_year: u16,
    collection_titles: &[String],  // NEW
) -> bool

// Updated to fetch and use metadata
pub async fn discover_all(&self, movie: &MovieEntry) -> Vec<VideoSource>
```

## Testing

### New Tests Added
1. `test_youtube_movie_title_filtering` - Validates movie title presence check
2. `test_youtube_collection_filtering` - Validates collection movie exclusion

### Updated Tests
All existing `should_include_video` test calls updated to include new parameters:
- `test_youtube_should_include_video_valid`
- `test_youtube_should_include_video_excluded_by_duration`
- `test_youtube_should_include_video_excluded_by_keyword`
- `test_youtube_should_include_video_excluded_as_short`
- `test_youtube_should_include_video_multiple_exclusions`
- `test_youtube_year_filtering_same_year`
- `test_youtube_year_filtering_different_year`
- `test_youtube_year_filtering_no_year`

### Test Results
- ✅ All 57 discovery unit tests passing
- ✅ No clippy warnings
- ✅ Code properly formatted with rustfmt
- ✅ Compiles without errors or warnings

## Requirements Validated

### Updated Requirements
- **3.9**: TMDB collection details retrieval
- **5.11**: YouTube video title must contain movie title
- **5.12**: YouTube videos mentioning collection movies are excluded

## Backward Compatibility

The changes maintain backward compatibility:
- `ContentDiscoverer` trait unchanged
- `YoutubeDiscoverer::discover()` still works (uses empty metadata internally)
- New functionality accessed via `discover_with_metadata()` method
- Existing code continues to work without modifications

## Performance Considerations

- Collection metadata is fetched once per movie at the start of discovery
- Metadata fetch happens in parallel with TMDB video discovery (no additional latency)
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

1. **Reduced False Positives**: Videos about wrong movies are filtered out
2. **Better Franchise Support**: Correctly handles movie series and collections
3. **Improved Accuracy**: Only returns content specifically about the target movie
4. **Maintainable Design**: Clean separation of concerns with metadata struct
5. **Extensible**: Easy to add more metadata fields in the future

## Files Modified

- `src/discovery.rs` - Core implementation
- `.kiro/specs/extras-fetcher/requirements.md` - Updated requirements

## Next Steps

Consider future enhancements:
- Cache collection metadata to avoid repeated API calls
- Add fuzzy matching for movie titles with special characters
- Support for alternate titles and international releases
- Configurable strictness levels for filtering
