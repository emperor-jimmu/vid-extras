# Bug Fix: Temp Directory Cleanup Deleting Regular Extras

## Issue Report
User reported: "if it finds specials it does not download the other files (season / tv-show clips)"

## Root Cause Analysis

The bug was in the downloader's temp directory management. When downloading Season 0 specials after regular extras:

1. Regular extras downloaded to `tmp_downloads/Breaking_Bad_2008/` ✅
2. Specials download initiated with same `movie_id` 
3. Downloader saw directory exists and **deleted everything** including regular extras ❌
4. Conversion phase tried to convert regular extras but files were gone
5. Only specials converted successfully

**Evidence from logs:**
```
[2026-02-17T09:29:49Z WARN] Temp directory already exists, cleaning up: "tmp_downloads\\Breaking_Bad_2008"
[2026-02-17T09:29:55Z ERROR] Conversion failed for Series Trailer: ffmpeg failed: No such file or directory
```

## The Fix

Changed `src/downloader.rs` `create_temp_dir()` method to **reuse** existing directories instead of deleting them:

**Before:**
```rust
// Clean up any pre-existing temp directory
if temp_dir.exists() {
    warn!("Temp directory already exists, cleaning up: {:?}", temp_dir);
    fs::remove_dir_all(&temp_dir).await?;  // ❌ Deletes all files!
}
```

**After:**
```rust
// If directory already exists, reuse it (don't clean up during same processing session)
// This allows multiple download batches (regular extras + specials) to coexist
if temp_dir.exists() {
    debug!("Temp directory already exists, reusing: {:?}", temp_dir);
    return Ok(temp_dir);  // ✅ Reuse existing directory
}
```

## Why This Works

The orchestrator already handles cleanup properly:
- Pre-existing temp cleanup happens at the start of processing (line 884 in orchestrator.rs)
- Post-processing cleanup happens after organization (Drop trait, line 911)

The downloader doesn't need to clean up during processing - it should only create the directory if it doesn't exist.

## Changes Made

### 1. Fixed `src/downloader.rs`
- Modified `create_temp_dir()` to reuse existing directories
- Changed log level from `warn` to `debug` for existing directory message
- Updated comment to explain the behavior

### 2. Updated Test `src/downloader.rs`
- Renamed `test_create_temp_dir_cleans_existing` → `test_create_temp_dir_reuses_existing`
- Updated assertions to verify files persist across multiple calls
- Test now validates directory reuse instead of cleanup

### 3. Enhanced Logging `src/orchestrator.rs`
- Added discovery phase breakdown logging
- Shows series-level, season-specific, and specials counts separately
- Makes it easier to diagnose discovery issues

## Testing

All tests pass:
- ✅ `cargo build` - no errors or warnings
- ✅ `cargo test` - 434 tests pass (14 downloader tests)
- ✅ `cargo clippy` - no warnings

## Expected Behavior After Fix

### With `--specials` flag:
1. Download 8 regular extras → `tmp_downloads/Breaking_Bad_2008/`
2. Download 2 specials → **reuses same directory** (doesn't delete)
3. Convert all 10 files successfully
4. Organize into appropriate subdirectories
5. Cleanup temp directory after organization

### Result:
- Regular extras: ✅ Downloaded and converted
- Season 0 specials: ✅ Downloaded and converted
- All files organized correctly
