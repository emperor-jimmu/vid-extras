# Bugfix: Filename Collision, Missing Completion Logs, Collection Detection, and YouTube Search Improvements

## Issue 1: Filename Collision During Download

### Problem
When processing movies with multiple video sources, yt-dlp would sometimes download different videos with identical filenames, causing file overwrites. This led to:
1. Conversion errors when trying to process non-existent files
2. Missing videos in the final output
3. Confusing error messages about files not found

### Root Cause
The downloader used `%(title)s.%(ext)s` as the yt-dlp output template, which uses the video's actual title from YouTube/Archive.org. Multiple different videos can have the same or very similar titles, causing filename collisions.

Example from Mad Max 2:
- Three different trailer URLs all resulted in the same filename: "Mad Max 2   Road Warrior Modern Trailer   YouTube.mkv"
- The file was downloaded, overwritten twice, then the converter tried to process all 3 references but only 1 file existed

### Solution
Added URL-based hash suffix to filenames to guarantee uniqueness:
- Changed output template to: `%(title)s_{HASH}.%(ext)s` where HASH is an 8-character hex hash of the URL
- Updated `find_downloaded_file()` to look for files with the hash suffix first
- Maintained backward compatibility with fallback to fuzzy matching if hash-based lookup fails

## Issue 2: Missing Completion Logs in Parallel Processing

### Problem
When processing movies in parallel (concurrency > 1), it was difficult to track when each movie completed because:
1. Logs from different movies were interleaved
2. No clear completion marker for each movie
3. Hard to diagnose if a movie failed silently or was still processing

### Solution
Added explicit completion logging:
- Added "✓ Movie processing complete: {movie}" log after successful organization
- Added "✗ Movie processing failed: {movie}" log after organization failure
- Added "Conversion batch complete for {movie}" log to mark end of conversion phase
- These markers make it easy to grep logs and verify each movie's status

## Issue 3: Collection Detection Not Working

### Problem
Movies that belong to collections (like Mad Max 2 in the Mad Max collection) were not being detected as part of a collection, resulting in the log message "No collection found" even though the movie clearly belongs to a collection.

### Root Cause
The TMDB search API (`/search/movie`) does **not** return the `belongs_to_collection` field. According to TMDB API documentation, only the movie details API (`/movie/{movie_id}`) includes collection information.

The code was trying to read `belongs_to_collection` from the search response, which always returned null.

### Solution
Added a second API call to fetch movie details after finding the movie:
- After search finds a movie ID, call `/movie/{movie_id}` to get full details
- Extract `belongs_to_collection` from the details response
- This properly detects collection membership for filtering YouTube results

## Issue 4: YouTube Search Results Too Limited and Slow NVENC Encoding

### Problem
YouTube searches were only returning 5 results per query (`ytsearch5:`), which was insufficient for finding relevant extras, especially for popular movies with many fan-made videos.

User reported that a specific Predator 2 deleted scene (https://www.youtube.com/watch?v=7I1hf1r3Ir4) was not being found.

### Solution
Increased YouTube search results from 5 to 10 per query:
- Changed `ytsearch5:` to `ytsearch10:` in YouTube discovery
- This doubles the number of potential results per search query
- Increases chances of finding official extras among fan content

### Problem
Download progress was not visible, making it hard to track which videos were being downloaded and whether downloads were succeeding or failing.

### Solution
Added detailed download progress indicators:
- Progress indicator: "Download progress [X/Y]: {title} from {url}"
- Success indicator: "✓ Downloaded [X/Y]: {title}"
- Failure indicator: "✗ Failed [X/Y]: {title} - {error}"
- Batch completion summary showing successful/total downloads

### Problem
FFmpeg NVENC conversions were taking too long due to using the p4 (medium) preset, which is balanced but not optimized for speed when processing thousands of movies.

### Solution
Changed NVENC preset from p4 to p3 for maximum encoding speed:
- p4 (medium) → p3 (faster) - significantly faster encoding
- Quality difference is minimal for extras content
- Better suited for batch processing large libraries
- According to [FFmpeg NVENC documentation](https://gist.github.com/nico-lab/c2d192cbb793dfd241c1eafeb52a21c3), p1 is the fastest preset

## Changes Made
- `src/downloader.rs`:
  - Modified `download_single()` to generate URL hash and include it in filename
  - Updated `find_downloaded_file()` to accept `url_hash` parameter and search for hash suffix
  - Fixed clippy warning about collapsible if statements
  - Updated tests to use hash-based filenames
  - Added download progress logging in `download_all()` method
  - Added success/failure indicators for each download
  - Added batch completion summary

- `src/converter.rs`:
  - Changed NVENC preset from p4 (medium) to p1 (fastest) for maximum encoding speed
  - Significantly reduces conversion time for large batch processing

- `src/orchestrator.rs`:
  - Added completion log markers for successful and failed movie processing
  - Added conversion batch completion log
  - Improved visibility of processing status in parallel execution

- `src/discovery.rs`:
  - Added `fetch_movie_details()` method to call `/movie/{movie_id}` API
  - Modified `search_movie()` to fetch details after finding movie ID
  - Added `TmdbMovieDetails` struct for details API response
  - Removed `belongs_to_collection` from `TmdbMovie` (search response) struct
  - Collection detection now works correctly for all movies
  - Changed YouTube search from `ytsearch5:` to `ytsearch10:` for more results

## Testing
- All 224 unit tests pass ✅
- All 15 integration tests pass ✅
- Zero clippy warnings ✅
- Builds successfully in release mode ✅

## Impact
- Prevents file overwrites during download phase
- Eliminates conversion errors caused by missing files
- Ensures all discovered videos are properly downloaded and converted
- Better visibility into parallel processing status
- Easier to diagnose issues when processing large libraries
- Collection filtering now works correctly for YouTube results
- Reduces irrelevant YouTube results for movies in collections
- Significantly faster NVENC encoding (p1 vs p4 preset)
- Better suited for batch processing thousands of movies
- No breaking changes to public API
