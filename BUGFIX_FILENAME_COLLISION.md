# Bugfix: Filename Collision and Missing Completion Logs

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

## Changes Made
- `src/downloader.rs`:
  - Modified `download_single()` to generate URL hash and include it in filename
  - Updated `find_downloaded_file()` to accept `url_hash` parameter and search for hash suffix
  - Fixed clippy warning about collapsible if statements
  - Updated tests to use hash-based filenames

- `src/orchestrator.rs`:
  - Added completion log markers for successful and failed movie processing
  - Added conversion batch completion log
  - Improved visibility of processing status in parallel execution

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
- No breaking changes to public API
