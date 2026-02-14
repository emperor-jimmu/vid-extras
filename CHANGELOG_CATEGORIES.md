# Category Changes - Interview Addition & Blooper Removal

## Summary

This document describes the changes made to add the Interview category and remove Blooper support from the extras_fetcher tool.

## Changes Made

### 1. Added Interview Category

**Models (src/models.rs)**
- Added `Interview` variant to `ContentCategory` enum
- Added subdirectory mapping: `Interview` → `"interviews"`
- Updated `Display` implementation to show "Interview"
- Updated test to verify interview subdirectory mapping

**Discovery (src/discovery.rs)**
- Updated YouTube search queries to map "cast interview" to `ContentCategory::Interview` (previously mapped to Featurette)
- Removed blooper search query entirely
- Updated query count from 4 to 3 queries per movie

**Organizer (src/organizer.rs)**
- Updated property tests to include Interview category
- Interview content now organized into `/interviews` subdirectory

### 2. Removed Blooper Support

**Discovery (src/discovery.rs)**
- Removed TMDB type mapping: `"Bloopers"` → `ContentCategory::Featurette`
- Removed YouTube search query for bloopers
- Removed "Bloopers" and "Gag Reel" from excluded keywords list (no longer needed since we don't search for them)
- Updated property tests to remove Blooper test cases
- Removed blooper-specific unit tests

**Requirements (.kiro/specs/extras-fetcher/requirements.md)**
- Updated glossary to replace "bloopers" with "interviews"
- Updated Target_Subdirectory definition to include `/interviews`
- Removed Requirement 3.8 (TMDB Bloopers mapping)
- Removed Requirement 5.5 (YouTube blooper search query)
- Added Requirement 8.5 (Interview subdirectory organization)

**Product Documentation (.kiro/steering/product.md)**
- Updated product overview to mention "interviews" instead of listing all categories

## Directory Structure

Movies will now be organized with the following subdirectories:
- `/trailers` - Movie trailers
- `/featurettes` - Featurettes and EPK content
- `/behind the scenes` - Behind-the-scenes footage
- `/deleted scenes` - Deleted scenes
- `/interviews` - Cast and crew interviews (NEW)

## Test Results

All tests passing:
- 200 unit/property tests passed
- 0 failures
- 9 ignored (environment-dependent tests)
- Zero clippy warnings

## Rationale

1. **Interview Addition**: Cast and crew interviews are valuable supplementary content that deserve their own category and subdirectory for better organization in Jellyfin.

2. **Blooper Removal**: Bloopers were previously mapped to the Featurettes category, which caused organizational confusion. Since they're not a primary content type for most users and were already being filtered out by keyword exclusion, removing them simplifies the codebase.

## Migration Notes

For existing installations:
- Movies processed before this change will not have an `/interviews` subdirectory
- Any previously downloaded blooper content (if any) will remain in `/featurettes` subdirectory
- Re-running with `--force` flag will reprocess movies and organize interview content into the new subdirectory
