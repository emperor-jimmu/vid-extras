# Implementation Plan: TV Series Extra Content Support

## Overview

This implementation plan extends the extras_fetcher tool to support TV series extra content while maintaining backward compatibility with existing movie functionality. The implementation follows the same pipeline architecture (Scan → Discovery → Download → Conversion → Organization) but adds new data models and logic for the hierarchical structure of TV series.

The plan is organized into discrete, incremental tasks that build upon each other, with property-based tests integrated throughout to validate correctness early.

## Tasks

- [x] 1. Add core data models for TV series support
  - Add SeriesEntry struct to models.rs with path, title, optional year, done marker status, and seasons list
  - Add SeriesExtra struct with series ID, optional season number, category, title, URL, source type, and local path
  - Add SpecialEpisode struct for Season 0 episodes
  - Add MediaType enum (Movie, Series, Unknown)
  - Add ProcessingMode enum (Both, MoviesOnly, SeriesOnly)
  - Update ProcessingSummary to include series statistics
  - _Requirements: 2.1, 2.2, 10.1_

- [x] 1.1 Write property test for SeriesExtra serialization round-trip
  - **Property 4: SeriesExtra Serialization Round-Trip**
  - **Validates: Requirements 2.6**

- [x] 2. Extend Scanner module with media type detection
  - [x] 2.1 Add detect_media_type function to classify folders as Movie, Series, or Unknown
    - Implement has_season_folders helper to detect Season XX folders using regex
    - Implement has_video_files helper to detect video files in directory
    - _Requirements: 10.1, 10.2, 10.3_
  - [x] 2.2 Add parse_series_folder_name function
    - Support "{Series Name} (YYYY)" format with year
    - Support "{Series Name}" format without year
    - Return tuple of (title, Option<year>)
    - _Requirements: 1.1, 1.2_
  - [x] 2.3 Write property test for series folder name parsing
    - **Property 1: Series Folder Name Parsing**
    - **Validates: Requirements 1.1, 1.2**
  - [x] 2.4 Add scan_all method to return both movies and series
    - Classify each folder using detect_media_type
    - Parse folder names based on media type
    - Detect season folders for series
    - Check done markers for both types
    - _Requirements: 1.3, 1.4, 1.5, 1.6_
  - [x] 2.5 Write property test for series done marker skipping
    - **Property 2: Series Done Marker Skipping**
    - **Validates: Requirements 1.3, 9.1, 9.3, 9.4**
  - [x] 2.6 Write property test for media type detection consistency
    - **Property 11: Media Type Detection Consistency**
    - **Validates: Requirements 10.1, 10.2, 10.3**

- [x] 3. Checkpoint - Ensure scanner tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 4. Create series discovery module structure
  - [x] 4.1 Create src/discovery/series_tmdb.rs module
    - Define TmdbSeriesDiscoverer struct with api_key and reqwest client
    - Implement search_series method to query TMDB TV search endpoint
    - Implement discover_series_extras method to fetch videos from TMDB
    - Implement discover_season_zero method to fetch Season 0 episodes
    - Map TMDB video types to ContentCategory (Trailer, Behind the Scenes, Featurette, Bloopers)
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 4.1, 4.2, 4.3_
  - [x] 4.2 Write property test for TMDB video type mapping
    - **Property 5: TMDB Video Type Mapping Completeness**
    - **Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8**
  - [x] 4.3 Write unit tests for TMDB series discovery
    - Test search_series with mock responses
    - Test video type mapping for all types
    - Test Season 0 episode parsing
    - Test error handling for API failures
    - _Requirements: 3.9, 4.4_
  - [x] 4.4 Write property test for Season 0 episode separation
    - **Property 6: Season 0 Episode Separation**
    - **Validates: Requirements 4.5**

- [ ] 5. Create YouTube series discovery module
  - [ ] 5.1 Create src/discovery/series_youtube.rs module
    - Define YoutubeSeriesDiscoverer struct
    - Implement build_series_search_queries for all content types
    - Support series-level queries (title + year)
    - Support season-specific queries (title + year + season)
    - Reuse existing YouTube filtering (duration, keywords, shorts)
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 6.1, 6.2_
  - [ ] 5.2 Write property test for YouTube series query construction
    - **Property 7: YouTube Series Query Construction**
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5**
  - [ ] 5.3 Write property test for season-specific query tagging
    - **Property 8: Season-Specific Query Tagging**
    - **Validates: Requirements 6.1, 6.2, 6.3**
  - [ ] 5.4 Write unit tests for YouTube series discovery
    - Test query construction for all content types
    - Test season-specific query format
    - Test fallback to series-level when season search fails
    - Test error handling
    - _Requirements: 5.9, 6.4_

- [ ] 6. Create series discovery orchestrator
  - Add SeriesDiscoveryOrchestrator to coordinate TMDB and YouTube discovery
  - Implement discover_all method to aggregate results from both sources
  - Handle errors gracefully (continue if one source fails)
  - Support season-specific discovery when enabled
  - _Requirements: 13.1, 13.2_

- [ ] 7. Checkpoint - Ensure discovery tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. Implement series organizer module
  - [ ] 8.1 Create SeriesOrganizer struct in src/organizer.rs
    - Add organize_extras method for series extras
    - Support series-level extras (no season number)
    - Support season-specific extras (with season number)
    - Create subdirectories based on ContentCategory
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 2.3, 2.4_
  - [ ]\* 8.2 Write property test for season number interpretation
    - **Property 3: Season Number Interpretation**
    - **Validates: Requirements 2.3, 2.4**
  - [ ]\* 8.3 Write property test for content category to subdirectory mapping
    - **Property 9: Content Category to Subdirectory Mapping**
    - **Validates: Requirements 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7**
  - [ ] 8.4 Add organize_specials method for Season 0 episodes
    - Create Season 00 folder
    - Format filenames as "{Series Name} - S00E{num} - {title}.mp4"
    - Zero-pad episode numbers
    - Sanitize filenames (remove invalid characters)
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_
  - [ ]\* 8.5 Write property test for Season 0 file naming format
    - **Property 10: Season 0 File Naming Format**
    - **Validates: Requirements 8.1, 8.2, 8.3, 8.4**
  - [ ]\* 8.6 Write unit tests for series organizer
    - Test subdirectory creation
    - Test file moving with cross-drive support
    - Test filename sanitization
    - Test temp folder cleanup
    - _Requirements: 7.9, 8.5_

- [ ] 9. Add metadata caching support
  - [ ] 9.1 Create cache module for series metadata
    - Store TMDB series metadata in .cache directory under series folder
    - Include 7-day TTL in cache files
    - Implement cache validation (check age)
    - Support force flag to bypass cache
    - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5_
  - [ ]\* 9.2 Write property test for metadata cache freshness
    - **Property 14: Metadata Cache Freshness**
    - **Validates: Requirements 14.1, 14.2, 14.3, 14.4**
  - [ ]\* 9.3 Write unit tests for cache module
    - Test cache creation and reading
    - Test TTL expiration
    - Test force flag behavior
    - Test invalid cache handling

- [ ] 10. Checkpoint - Ensure organizer and cache tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 11. Extend orchestrator for series processing
  - [ ] 11.1 Update Orchestrator struct to include series_discovery field
    - Add SeriesDiscoveryOrchestrator alongside movie discovery
    - Add processing_mode field (ProcessingMode enum)
    - Update constructor to accept processing mode
    - _Requirements: 12.1, 12.2, 12.3_
  - [ ] 11.2 Implement process_series method
    - Follow same pipeline as movies: discovery → download → convert → organize
    - Process series-level extras
    - Process season-specific extras if enabled
    - Process Season 0 specials if enabled
    - Create done marker on success
    - _Requirements: 9.2, 9.5_
  - [ ] 11.3 Update run method to handle both movies and series
    - Call scanner.scan_all() to get both movies and series
    - Filter based on processing_mode
    - Process movies if mode allows
    - Process series if mode allows
    - Aggregate statistics for both types
    - _Requirements: 12.1, 12.2, 12.3_
  - [ ]\* 11.4 Write property test for processing mode filtering
    - **Property 12: Processing Mode Filtering**
    - **Validates: Requirements 12.1, 12.2, 12.3**
  - [ ]\* 11.5 Write property test for series error isolation
    - **Property 13: Series Error Isolation**
    - **Validates: Requirements 13.1, 13.2, 13.3, 13.4, 13.5, 13.6**
  - [ ]\* 11.6 Write unit tests for series orchestration
    - Test series processing pipeline
    - Test mixed library processing
    - Test error handling and recovery
    - Test done marker creation

- [ ] 12. Update CLI module with series flags
  - [ ] 12.1 Add new CLI flags to CliArgs struct
    - Add --series-only flag (bool)
    - Add --movies-only flag (bool)
    - Add --season-extras flag (bool)
    - Add --specials flag (bool)
    - Add --type flag (Option<String>)
    - _Requirements: 12.1, 12.2, 12.4, 12.6, 12.7, 10.5_
  - [ ] 12.2 Update parse_args to validate flag combinations
    - Ensure --series-only and --movies-only are mutually exclusive
    - Convert flags to ProcessingMode enum
    - Validate --type flag values (movie or series)
    - _Requirements: 12.1, 12.2, 12.3_
  - [ ]\* 12.3 Write unit tests for CLI flag parsing
    - Test all flag combinations
    - Test mutually exclusive flags
    - Test default values
    - Test type flag validation

- [ ] 13. Update output module for series progress reporting
  - Add series-specific progress messages
  - Display series name and year during processing
  - Display extras count per source
  - Display download and conversion statistics
  - Update final summary to include series statistics
  - _Requirements: 18.1, 18.2, 18.3, 18.4, 18.5, 18.6, 19.1, 19.2, 19.3, 19.4, 19.5, 19.6_

- [ ]\* 13.1 Write property test for series summary statistics accuracy
  - **Property 18: Series Summary Statistics Accuracy**
  - **Validates: Requirements 19.1, 19.2, 19.3, 19.4, 19.5**

- [ ] 14. Checkpoint - Ensure orchestrator and CLI tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 15. Add advanced features
  - [ ] 15.1 Implement season pack post-processing
    - Detect archive files in downloads
    - Extract to temporary directory
    - Identify bonus content by filename patterns
    - Map patterns to ContentCategory (behind the scenes, deleted scene, interview, featurette, blooper)
    - Organize extracted extras
    - Clean up temporary extraction directory
    - _Requirements: 15.1, 15.2, 15.3, 15.4, 15.5, 15.6, 15.7, 15.8, 15.9_
  - [ ]\* 15.2 Write property test for season pack file identification
    - **Property 15: Season Pack File Identification**
    - **Validates: Requirements 15.2, 15.3, 15.4, 15.5, 15.6, 15.7, 15.8**
  - [ ] 15.3 Implement local Season 0 import scanning
    - Scan series folder for S00Exx files
    - Detect files outside Season 00 folder
    - Move to Season 00 folder with correct naming
    - Handle duplicate episode numbers
    - Preserve original quality
    - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.5_
  - [ ]\* 15.4 Write property test for local Season 0 import
    - **Property 16: Local Season 0 Import**
    - **Validates: Requirements 16.1, 16.2, 16.3**
  - [ ] 15.5 Implement fuzzy title matching
    - Normalize strings (lowercase, remove special characters)
    - Calculate Levenshtein distance
    - Apply 80% similarity threshold
    - Log similarity scores for debugging
    - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5_
  - [ ]\* 15.6 Write property test for fuzzy title matching threshold
    - **Property 17: Fuzzy Title Matching Threshold**
    - **Validates: Requirements 17.1, 17.2, 17.3, 17.4**
  - [ ]\* 15.7 Write unit tests for advanced features
    - Test archive extraction
    - Test filename pattern matching
    - Test Season 0 file detection
    - Test string normalization
    - Test similarity calculation

- [ ] 16. Add error handling and logging
  - [ ] 16.1 Define series-specific error types in src/error.rs
    - Add SeriesScanError enum
    - Add SeriesDiscoveryError enum
    - Add SeriesOrganizerError enum
    - Implement Display and Error traits using thiserror
    - _Requirements: 13.6_
  - [ ] 16.2 Add comprehensive error logging
    - Log errors with series context (name, year)
    - Log operation that failed
    - Log error details
    - Continue processing on non-fatal errors
    - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_
  - [ ]\* 16.3 Write unit tests for error handling
    - Test error propagation
    - Test error logging
    - Test graceful degradation
    - Test partial success scenarios

- [ ] 17. Checkpoint - Ensure advanced features tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 18. Create integration tests
  - [ ] 18.1 Create tests/series_integration_tests.rs
    - Test complete series processing pipeline (scan → discover → download → convert → organize)
    - Test mixed library processing (movies + series)
    - Test processing mode filtering (movies-only, series-only, both)
    - Test done marker behavior with series
    - Test error recovery and isolation
    - Test Season 0 specials processing
    - Test season-specific extras processing
    - _Requirements: 20.3_
  - [ ]\* 18.2 Write property test for backward compatibility
    - **Property 19: Backward Compatibility Preservation**
    - **Validates: Requirements 20.1, 20.2, 20.5**
  - [ ]\* 18.3 Write integration tests for edge cases
    - Test series without year
    - Test series with no Season 0
    - Test series with no extras found
    - Test interrupted processing and resumption
    - Test invalid done markers
    - _Requirements: 12.4, 20.4_

- [ ] 19. Update documentation
  - Update README.md with series support information
  - Add examples for series processing
  - Document new CLI flags
  - Add troubleshooting section for series
  - Update installation instructions if needed

- [ ] 20. Final validation and polish
  - [ ] 20.1 Run full test suite
    - Run `cargo test` to ensure all tests pass
    - Verify all 19 property tests pass with 100+ iterations
    - Verify all unit tests pass
    - Verify all integration tests pass
  - [ ] 20.2 Run code quality checks
    - Run `cargo clippy -- -D warnings` to ensure no warnings
    - Run `cargo fmt -- --check` to ensure proper formatting
    - Run `cargo check` to verify compilation
  - [ ] 20.3 Test backward compatibility manually
    - Test movie-only library (should work identically to before)
    - Test mixed library (should process both types)
    - Test series-only library (should process only series)
    - Verify done markers work for both types
  - [ ] 20.4 Performance testing
    - Test with large library (100+ series)
    - Verify concurrency limits work correctly
    - Verify memory usage is reasonable
    - Verify no resource leaks

- [ ] 21. Final checkpoint - Complete feature
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional test-related sub-tasks and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation throughout implementation
- Property tests validate universal correctness properties with 100+ iterations
- Unit tests validate specific examples and edge cases
- Integration tests validate end-to-end flows
- The implementation maintains backward compatibility with existing movie functionality
- All modules follow SOLID principles and code quality standards
- Error handling ensures graceful degradation and clear error messages

## Dependencies

This feature requires the following existing dependencies:

- `regex` - For folder name and season folder pattern matching
- `serde` and `serde_json` - For SeriesExtra serialization and cache storage
- `reqwest` - For TMDB API calls (already used for movies)
- `tokio` - For async operations (already used throughout)
- `thiserror` - For error type definitions (already used)
- `proptest` - For property-based testing (already used)
- `tempfile` - For integration tests (already used)

Optional new dependency for fuzzy matching:

- `strsim` or `levenshtein` - For string similarity calculation (Requirement 17.2)

## Implementation Strategy

The implementation follows an incremental approach:

1. **Foundation (Tasks 1-3)**: Add core data models and extend scanner
2. **Discovery (Tasks 4-7)**: Implement TMDB and YouTube series discovery
3. **Organization (Tasks 8-10)**: Implement series file organization and caching
4. **Integration (Tasks 11-14)**: Wire everything together in orchestrator and CLI
5. **Advanced Features (Tasks 15-17)**: Add season packs, local import, fuzzy matching
6. **Testing & Polish (Tasks 18-21)**: Integration tests, documentation, validation

Each phase builds on the previous one, with checkpoints to ensure stability before proceeding.
