# Content Limits Implementation

## Overview

Implemented content quantity limitations per category to prevent library clutter with excessive extras content.

## Requirements Added

Added Requirement 14 to the requirements document with the following acceptance criteria:

1. Maximum 4 trailers per movie
2. Maximum 8 deleted scenes per movie
3. Maximum 8 interviews per movie
4. Maximum 10 featurettes per movie
5. Maximum 10 behind-the-scenes videos per movie
6. Limits applied after aggregating all sources
7. Priority: TMDB > Archive.org > YouTube when limits are exceeded

## Implementation Details

### Modified Files

1. **`.kiro/specs/extras-fetcher/requirements.md`**
   - Added Requirement 14: Content Quantity Limitations
   - Renumbered existing Requirement 14 to Requirement 15

2. **`src/discovery.rs`**
   - Added `apply_content_limits()` private method to `DiscoveryOrchestrator`
   - Modified `discover_all()` to apply limits after deduplication
   - Implemented source prioritization (TMDB > Archive.org > YouTube)

### Key Implementation

```rust
fn apply_content_limits(sources: Vec<VideoSource>) -> Vec<VideoSource> {
    // Define limits per category
    let limits: HashMap<ContentCategory, usize> = [
        (Trailer, 4),
        (DeletedScene, 8),
        (Interview, 8),
        (Featurette, 10),
        (BehindTheScenes, 10),
    ].iter().cloned().collect();

    // Group by category
    // Sort by source priority (TMDB > Archive.org > YouTube)
    // Truncate to limit
    // Return aggregated results
}
```

## Test Coverage

Added 9 comprehensive unit tests:

1. `test_content_limits_trailers` - Verifies 4 trailer limit with source prioritization
2. `test_content_limits_deleted_scenes` - Verifies 8 deleted scene limit
3. `test_content_limits_interviews` - Verifies 8 interview limit
4. `test_content_limits_featurettes` - Verifies 10 featurette limit
5. `test_content_limits_behind_the_scenes` - Verifies 10 behind-the-scenes limit
6. `test_content_limits_mixed_categories` - Tests multiple categories simultaneously
7. `test_content_limits_source_priority` - Validates TMDB > Archive.org > YouTube priority
8. `test_content_limits_empty_input` - Edge case: empty input
9. `test_content_limits_under_limit` - Edge case: content under limit

All tests pass ✅

## Code Quality

- ✅ All tests pass (233 tests in lib, 233 in main, 16 integration)
- ✅ No clippy warnings
- ✅ Code properly formatted with rustfmt
- ✅ Follows SOLID principles and project coding standards

## Behavior

When discovering content:
1. All sources are queried (TMDB, Archive.org, YouTube based on mode)
2. Results are deduplicated by URL
3. Content limits are applied per category
4. Within each category, sources are prioritized by type
5. Logging indicates when limits are applied

Example log output:
```
Found 6 sources from YouTube for The Matrix (1999)
Found 3 sources from TMDB for The Matrix (1999)
Applied content limits, reduced from 9 to 4 sources for The Matrix (1999)
Total sources discovered for The Matrix (1999): 4
```

## Impact

- Prevents excessive content downloads
- Maintains library organization and cleanliness
- Prioritizes official sources (TMDB) over community sources (YouTube)
- Transparent logging of limit application
- No breaking changes to existing functionality
