# Implementation Plan: extras_fetcher

## Overview

This implementation plan breaks down the extras_fetcher tool into discrete, incremental coding tasks. The approach follows a bottom-up strategy: building core modules first, then integrating them into the orchestrator, and finally adding the CLI interface. Each task builds on previous work, with property-based tests placed close to implementation to catch errors early.

## Tasks

- [x] 1. Project initialization and core setup
  - Create new Rust project with Cargo
  - Configure Cargo.toml with edition = "2024" and dependencies (clap, tokio, reqwest, serde, serde_json, colored, log, env_logger, regex, thiserror, proptest)
  - Set up project structure with modules: cli, scanner, discovery, downloader, converter, organizer, orchestrator, validation
  - Create error types module with all custom error enums
  - _Requirements: 14.1, 14.5_

- [x] 2. Implement data models and core types
  - [x] 2.1 Create data model structs
    - Define MovieEntry, VideoSource, DownloadResult, ConversionResult, DoneMarker structs
    - Define enums: SourceMode, SourceType, ContentCategory, HardwareAccel
    - Implement Display and Debug traits for key types
    - _Requirements: 1.1, 2.2, 3.4-3.8_
  
  - [x] 2.2 Write property test for DoneMarker serialization
    - **Property 2: Done Marker Round-Trip**
    - **Validates: Requirements 2.2**

- [x] 3. Implement Scanner module
  - [x] 3.1 Create Scanner struct with directory traversal logic
    - Implement recursive directory walking
    - Implement folder name parsing with regex pattern "^(.+?)\\s*\\((\\d{4})\\)$"
    - Implement done marker detection
    - Build MovieEntry queue with filtering logic
    - _Requirements: 1.6, 1.7, 1.8, 1.9, 2.3_
  
  - [x] 3.2 Write property test for folder name parsing
    - **Property 1: Folder Name Parsing Correctness**
    - **Validates: Requirements 1.7**
  
  - [x] 3.3 Write property test for done marker skipping
    - **Property 3: Done Marker Skipping Behavior**
    - **Validates: Requirements 1.8, 2.3, 12.1**
  
  - [x] 3.4 Write property test for recursive traversal
    - **Property 6: Recursive Directory Traversal Completeness**
    - **Validates: Requirements 1.6**
  
  - [x] 3.5 Write unit tests for scanner edge cases
    - Test invalid folder names
    - Test empty directories
    - Test nested directory structures
    - _Requirements: 1.7_

- [x] 4. Implement Validation module
  - [x] 4.1 Create Validator struct with dependency checking
    - Implement binary existence checks (yt-dlp, ffmpeg)
    - Implement ffmpeg HEVC support detection
    - Implement TMDB API key validation from environment
    - Return descriptive errors for missing dependencies
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5_
  
  - [x] 4.2 Write property test for dependency validation
    - **Property 32: Dependency Validation at Startup**
    - **Validates: Requirements 11.1, 11.2, 11.4**
  
  - [x] 4.3 Write property test for missing dependency error reporting
    - **Property 34: Missing Dependency Error Reporting**
    - **Validates: Requirements 11.5, 10.5**
  
  - [x] 4.4 Write unit tests for validation scenarios
    - Test with missing binaries
    - Test with invalid API key
    - Test ffmpeg codec detection
    - _Requirements: 11.1-11.5_

- [x] 5. Checkpoint - Core infrastructure complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 6. Implement Discovery module - TMDB
  - [ ] 6.1 Create TmdbDiscoverer struct
    - Implement movie search by title and year
    - Implement video list fetching
    - Implement type-to-category mapping (Trailer, Behind the Scenes, Deleted Scene, Featurette, Bloopers)
    - Handle API errors gracefully with logging
    - _Requirements: 3.1, 3.2, 3.3, 3.4-3.8, 3.9_
  
  - [ ] 6.2 Write property test for TMDB type mapping
    - **Property 7: TMDB Video Type Mapping**
    - **Validates: Requirements 3.4-3.8**
  
  - [ ] 6.3 Write unit tests for TMDB integration
    - Test API response parsing with mock responses
    - Test error handling for failed requests
    - Test movie search query construction
    - _Requirements: 3.1, 3.2, 3.9_

- [ ] 7. Implement Discovery module - Archive.org
  - [ ] 7.1 Create ArchiveOrgDiscoverer struct
    - Implement year-based conditional querying (< 2010)
    - Implement search query construction with title and EPK/Making of subjects
    - Implement result parsing and category mapping
    - Handle API errors gracefully with logging
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7_
  
  - [ ] 7.2 Write property test for year-based querying
    - **Property 8: Archive.org Year-Based Querying**
    - **Validates: Requirements 4.1, 4.2**
  
  - [ ] 7.3 Write property test for query construction
    - **Property 9: Archive.org Query Construction**
    - **Validates: Requirements 4.4**
  
  - [ ] 7.4 Write unit tests for Archive.org integration
    - Test query string formatting
    - Test result parsing
    - Test error handling
    - _Requirements: 4.3, 4.4, 4.7_

- [ ] 8. Implement Discovery module - YouTube
  - [ ] 8.1 Create YoutubeDiscoverer struct
    - Implement yt-dlp search integration with ytsearch operator
    - Implement search query construction for different content types
    - Implement duration filtering (30s - 20min)
    - Implement keyword filtering (Review, Reaction, Analysis, etc.)
    - Implement YouTube Shorts detection and exclusion
    - Handle search errors gracefully with logging
    - _Requirements: 5.1, 5.2, 5.3-5.6, 5.7, 5.8, 5.9, 5.10, 5.11_
  
  - [ ] 8.2 Write property test for YouTube always queried
    - **Property 10: YouTube Always Queried**
    - **Validates: Requirements 5.1**
  
  - [ ] 8.3 Write property test for duration filtering
    - **Property 11: YouTube Duration Filtering**
    - **Validates: Requirements 5.7, 5.8**
  
  - [ ] 8.4 Write property test for keyword filtering
    - **Property 12: YouTube Keyword Filtering**
    - **Validates: Requirements 5.9**
  
  - [ ] 8.5 Write property test for Shorts exclusion
    - **Property 13: YouTube Shorts Exclusion**
    - **Validates: Requirements 5.10**
  
  - [ ] 8.6 Write unit tests for YouTube integration
    - Test search query construction
    - Test filtering logic with various inputs
    - Test error handling
    - _Requirements: 5.2, 5.3-5.6, 5.11_

- [ ] 9. Implement DiscoveryOrchestrator
  - [ ] 9.1 Create DiscoveryOrchestrator struct
    - Integrate TMDB, Archive.org, and YouTube discoverers
    - Implement mode-based filtering (All vs YoutubeOnly)
    - Coordinate discovery from all sources
    - Aggregate results from multiple sources
    - _Requirements: 1.5, 3.1-3.9, 4.1-4.7, 5.1-5.11_
  
  - [ ] 9.2 Write property test for mode filtering
    - **Property 5: Mode Filtering**
    - **Validates: Requirements 1.5**

- [ ] 10. Checkpoint - Discovery phase complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 11. Implement Downloader module
  - [ ] 11.1 Create Downloader struct
    - Implement temporary directory creation (/tmp_downloads/{movie_id}/)
    - Implement yt-dlp command execution for downloads
    - Implement exit code verification
    - Implement partial file cleanup on failure
    - Implement timeout handling (5 minutes per download)
    - Implement error logging and continuation
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7_
  
  - [ ] 11.2 Write property test for temp directory creation
    - **Property 14: Temporary Directory Creation**
    - **Validates: Requirements 6.1**
  
  - [ ] 11.3 Write property test for download failure cleanup
    - **Property 15: Download Failure Cleanup**
    - **Validates: Requirements 6.4**
  
  - [ ] 11.4 Write property test for error continuation
    - **Property 16: Download Error Continuation**
    - **Validates: Requirements 6.5**
  
  - [ ] 11.5 Write property test for timeout handling
    - **Property 17: Network Timeout Graceful Handling**
    - **Validates: Requirements 6.6**
  
  - [ ] 11.6 Write unit tests for downloader
    - Test yt-dlp command construction
    - Test temp directory management
    - Test error scenarios
    - _Requirements: 6.2, 6.4, 6.5_

- [ ] 12. Implement Converter module
  - [ ] 12.1 Create Converter struct
    - Implement hardware acceleration detection (NVENC, QSV, Software)
    - Implement ffmpeg command construction with x265/HEVC codec
    - Implement CRF value configuration (24-26)
    - Implement conversion execution with error handling
    - Implement original file deletion on success
    - Implement failed output deletion and original preservation on failure
    - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 11.6_
  
  - [ ] 12.2 Write property test for codec usage
    - **Property 18: FFmpeg Codec Usage**
    - **Validates: Requirements 7.1**
  
  - [ ] 12.3 Write property test for CRF value range
    - **Property 19: CRF Value Range**
    - **Validates: Requirements 7.2**
  
  - [ ] 12.4 Write property test for hardware acceleration selection
    - **Property 20: Hardware Acceleration Selection**
    - **Validates: Requirements 7.3, 11.6**
  
  - [ ] 12.5 Write property test for conversion success cleanup
    - **Property 21: Conversion Success Cleanup**
    - **Validates: Requirements 7.4**
  
  - [ ] 12.6 Write property test for conversion failure preservation
    - **Property 22: Conversion Failure Preservation**
    - **Validates: Requirements 7.5, 7.6**
  
  - [ ] 12.7 Write unit tests for converter
    - Test ffmpeg command construction for different hardware
    - Test CRF value validation
    - Test file cleanup scenarios
    - _Requirements: 7.1, 7.2, 7.3_

- [ ] 13. Implement Organizer module
  - [ ] 13.1 Create Organizer struct
    - Implement category-to-subdirectory mapping
    - Implement subdirectory creation if missing
    - Implement file moving to target subdirectories
    - Implement temp folder cleanup after organization
    - Implement done marker creation with JSON timestamp
    - _Requirements: 8.1-8.7, 2.1_
  
  - [ ] 13.2 Write property test for category mapping
    - **Property 23: Content Category to Subdirectory Mapping**
    - **Validates: Requirements 8.1-8.4**
  
  - [ ] 13.3 Write property test for subdirectory creation
    - **Property 24: Subdirectory Creation**
    - **Validates: Requirements 8.5**
  
  - [ ] 13.4 Write property test for temp folder cleanup
    - **Property 25: Temp Folder Cleanup on Success**
    - **Validates: Requirements 8.6**
  
  - [ ] 13.5 Write property test for done marker creation
    - **Property 26: Done Marker Creation on Completion**
    - **Validates: Requirements 2.1, 8.7**
  
  - [ ] 13.6 Write unit tests for organizer
    - Test file moving operations
    - Test directory creation
    - Test done marker JSON format
    - _Requirements: 8.1-8.7_

- [ ] 14. Checkpoint - Processing modules complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 15. Implement Orchestrator module
  - [ ] 15.1 Create Orchestrator struct
    - Integrate all modules (scanner, discovery, downloader, converter, organizer)
    - Implement movie processing pipeline (all 5 phases)
    - Implement sequential downloads within a movie
    - Implement parallel movie processing with tokio and semaphore
    - Implement concurrency limit enforcement
    - Implement error isolation between movies
    - Implement temp folder cleanup on exit (Drop trait)
    - Implement pre-existing temp cleanup before processing
    - Generate processing summary statistics
    - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 10.1, 10.2, 10.3, 10.4_
  
  - [ ] 15.2 Write property test for sequential downloads
    - **Property 27: Sequential Downloads Within Movie**
    - **Validates: Requirements 9.1**
  
  - [ ] 15.3 Write property test for concurrency limit
    - **Property 28: Concurrency Limit Enforcement**
    - **Validates: Requirements 9.3, 9.4**
  
  - [ ] 15.4 Write property test for error isolation
    - **Property 29: Error Isolation Between Movies**
    - **Validates: Requirements 10.2**
  
  - [ ] 15.5 Write property test for temp cleanup on exit
    - **Property 30: Temp Folder Cleanup on Exit**
    - **Validates: Requirements 10.3**
  
  - [ ] 15.6 Write property test for pre-existing temp cleanup
    - **Property 31: Pre-existing Temp Cleanup**
    - **Validates: Requirements 10.4**
  
  - [ ] 15.7 Write integration tests for orchestrator
    - Test end-to-end movie processing with mocks
    - Test parallel processing behavior
    - Test error recovery scenarios
    - _Requirements: 9.1-9.5, 10.1-10.4_

- [ ] 16. Implement CLI module
  - [ ] 16.1 Create CLI argument parser
    - Implement argument parsing with clap (root_directory, --help, --version, --force, --mode, --concurrency, --verbose)
    - Implement configuration validation
    - Implement colored banner display with version
    - Implement configuration display with all parameters
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 13.1, 13.2, 13.8_
  
  - [ ] 16.2 Write property test for force flag behavior
    - **Property 4: Force Flag Overrides Done Markers**
    - **Validates: Requirements 1.4**
  
  - [ ] 16.3 Write property test for config display completeness
    - **Property 36: Configuration Display Completeness**
    - **Validates: Requirements 13.2**
  
  - [ ] 16.4 Write property test for verbose flag effect
    - **Property 38: Verbose Flag Effect**
    - **Validates: Requirements 13.8**
  
  - [ ] 16.5 Write unit tests for CLI parsing
    - Test --help flag
    - Test --version flag
    - Test invalid arguments
    - Test default values
    - _Requirements: 1.2, 1.3_

- [ ] 17. Implement CLI output and progress display
  - [ ] 17.1 Create output formatting module
    - Implement colored status indicators (green, yellow, red, blue)
    - Implement scanning progress display
    - Implement download progress indicators
    - Implement conversion progress display
    - Implement error message formatting with context
    - Implement summary statistics display
    - _Requirements: 13.1, 13.3, 13.4, 13.5, 13.6, 13.7, 10.1_
  
  - [ ] 17.2 Write property test for error message formatting
    - **Property 37: Error Message Formatting**
    - **Validates: Requirements 10.1, 13.7**
  
  - [ ] 17.3 Write unit tests for output formatting
    - Test colored output generation
    - Test progress indicator formatting
    - Test summary statistics display
    - _Requirements: 13.3-13.7_

- [ ] 18. Implement main entry point
  - [ ] 18.1 Create main.rs
    - Wire CLI parsing to orchestrator
    - Initialize logging with env_logger
    - Call validator before processing
    - Execute orchestrator and handle results
    - Display final summary
    - Handle fatal errors with descriptive messages
    - _Requirements: 11.1-11.5, 10.5_
  
  - [ ] 18.2 Write integration tests for main flow
    - Test complete execution with mock file system
    - Test validation failures
    - Test graceful error handling
    - _Requirements: 11.1-11.5, 10.5_

- [ ] 19. Implement idempotency features
  - [ ] 19.1 Add idempotency checks throughout pipeline
    - Verify done marker checking in scanner
    - Verify force flag overrides done markers
    - Verify partial library processing
    - Verify safe resumption after interruption
    - _Requirements: 12.1, 12.2, 12.3, 12.4_
  
  - [ ] 19.2 Write property test for idempotent re-execution
    - **Property 35: Idempotent Re-execution**
    - **Validates: Requirements 12.2, 12.3**
  
  - [ ] 19.3 Write integration tests for idempotency
    - Test multiple runs on same library
    - Test interruption and resumption
    - Test force flag behavior
    - _Requirements: 12.1-12.4_

- [ ] 20. Final checkpoint and polish
  - Ensure all tests pass (unit, property, integration)
  - Run cargo clippy for linting
  - Run cargo fmt for formatting
  - Verify all 38 correctness properties are implemented
  - Test with real TMDB API (if available)
  - Create README.md with usage instructions
  - Document environment variables (TMDB_API_KEY)

- [ ] 21. Build and package
  - Build release binary with cargo build --release
  - Test binary on sample movie library
  - Verify colored output in terminal
  - Verify all CLI flags work correctly
  - Create installation instructions

## Notes

- Tasks marked with `*` are optional property-based and unit tests that can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties with 100+ iterations
- Unit tests validate specific examples and edge cases
- Integration tests validate end-to-end workflows
- Checkpoints ensure incremental validation at major milestones
- The implementation follows a bottom-up approach: core modules → integration → CLI
- All external API calls should be mockable for testing
- Use tempdir crate for file system testing isolation
