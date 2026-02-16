# Implementation Plan: TV Series Specials via TheTVDB

## Overview

Incremental implementation of TheTVDB API v4 integration for Season 0 specials discovery. Tasks build on each other, starting with data models and config, then the TVDB client, ID bridging, monitor policy, search strategy, and finally organization and wiring into the orchestrator.

## Tasks

- [x] 1. Extend Config and data models
  - [x] 1.1 Add `tvdb_api_key: Option<String>` field to `Config` struct in `src/config.rs` with `#[serde(default)]`
    - Update `prompt_for_api_key` to support prompting for TVDB key
    - Update `load_or_create` to handle missing TVDB key when specials are enabled
    - _Requirements: 1.1, 1.6_
  - [x] 1.2 Write property test for Config serialization round-trip with tvdb_api_key
    - **Property 1: Config Serialization Round-Trip**
    - **Validates: Requirements 1.6**
  - [x] 1.3 Add TVDB data model structs in `src/discovery/tvdb.rs`
    - Define `TvdbEpisode`, `TvdbEpisodeExtended`, `TvdbSearchResult`, `TvdbApiResponse<T>`, `TvdbEpisodesPage`, `TvdbLoginResponse`, `TvdbSearchResponse`
    - Add `TheTVDB` variant to `SourceType` enum in `src/models.rs`
    - Add optional `tvdb_id: Option<u64>` field to `SpecialEpisode` in `src/models.rs`
    - _Requirements: 3.3, 4.2_
  - [x] 1.4 Write property test for TVDB episode parsing completeness
    - **Property 3: TVDB Episode Parsing Completeness**
    - **Validates: Requirements 3.3, 4.2**
  - [x] 1.5 Add `TvdbAuthError` and `TvdbApiError` variants to `DiscoveryError` in `src/error.rs`
    - _Requirements: 1.3, 10.1_

- [x] 2. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 3. Implement TvdbClient authentication and API communication
  - [x] 3.1 Implement `TvdbClient` struct with `new`, `authenticate`, `ensure_token`, and `authenticated_get` methods in `src/discovery/tvdb.rs`
    - Use `tokio::sync::RwLock<Option<String>>` for token storage
    - Implement auto-retry on HTTP 401 (re-authenticate once, then fail)
    - Implement network timeout retry (2-second delay, one retry)
    - Include Bearer token in Authorization header for all requests
    - _Requirements: 1.2, 1.3, 1.4, 1.5, 10.4_
  - [x] 3.2 Write unit tests for TvdbClient authentication flow
    - Test valid key authentication, invalid key error, 401 retry logic, timeout retry
    - _Requirements: 1.2, 1.3, 1.4, 10.4_
  - [x] 3.3 Implement `get_season_zero` method with pagination support
    - Query `GET /series/{tvdb_id}/episodes/default?season=0&page=0`
    - Paginate through all pages until no `next` URL remains
    - Return empty list on error or empty response
    - _Requirements: 3.1, 3.2, 3.4_
  - [x] 3.4 Write property test for TVDB API URL construction
    - **Property 2: TVDB API URL Construction**
    - **Validates: Requirements 3.1, 4.1**
  - [x] 3.5 Implement `get_episode_extended` method for enrichment
    - Query `GET /episodes/{tvdb_episode_id}/extended`
    - Populate absolute_number, airs_before_season, airs_after_season, airs_before_episode, is_movie
    - Retain base metadata on enrichment failure
    - _Requirements: 4.1, 4.2, 4.3_

- [x] 4. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Implement IdBridge for TMDB-to-TVDB ID resolution
  - [x] 5.1 Implement `IdMappingCache` in `src/discovery/id_bridge.rs`
    - File-based cache with no TTL expiration for ID mappings
    - Store as JSON files in `.cache/tvdb_ids/` under series path
    - _Requirements: 2.2, 2.5, 9.4_
  - [x] 5.2 Write property test for ID mapping cache no expiration
    - **Property 9: ID Mapping Cache Has No Expiration**
    - **Validates: Requirements 9.4**
  - [x] 5.3 Implement `IdBridge` with `resolve`, `query_tmdb_external_ids`, and `search_tvdb_fallback` methods
    - First try TMDB `/tv/{tmdb_id}/external_ids` for tvdb_id
    - Fallback to TVDB `/search?q={title}` with fuzzy matching (80% threshold)
    - Log warning and return None if no match found
    - Cache resolved IDs
    - _Requirements: 2.1, 2.3, 2.4, 2.5_
  - [x] 5.4 Write property test for fuzzy match ID resolution
    - **Property 10: Fuzzy Match ID Resolution Selects Highest Score Above Threshold**
    - **Validates: Requirements 2.3**

- [ ] 6. Implement MonitorPolicy
  - [ ] 6.1 Create `src/discovery/monitor_policy.rs` with `MonitorPolicy::should_monitor` and `MonitorPolicy::filter_monitored`
    - Default all episodes to unmonitored
    - Auto-monitor if airs_after_season == latest season on disk
    - Auto-monitor if is_movie == true
    - Monitor if episode number is in manual monitor list
    - Read manual list from `{series_folder}/specials_monitor.json`
    - _Requirements: 5.1, 5.2, 5.3, 5.4_
  - [ ] 6.2 Write property test for monitor policy correctness
    - **Property 4: Monitor Policy Correctness**
    - **Validates: Requirements 5.1, 5.2, 5.3, 5.4**

- [ ] 7. Implement SpecialSearcher
  - [ ] 7.1 Create `src/discovery/special_searcher.rs` with `SpecialSearcher::build_queries`
    - Standard query: `{title} S00E{number:02} {episode_title}`
    - Fallback query: `{title} {episode_title}`
    - Movie query: `{title} {episode_title} movie` (when is_movie=true)
    - Anime query: `{title} OVA {absolute_number}` (when absolute_number present)
    - Title similarity filtering: skip YouTube results below 60% match
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.6_
  - [ ] 7.2 Write property test for search query construction
    - **Property 6: Search Query Construction Correctness**
    - **Validates: Requirements 6.1, 6.2, 6.3, 6.4**
  - [ ] 7.3 Write property test for monitored-only query generation
    - **Property 5: Only Monitored Episodes Produce Search Queries**
    - **Validates: Requirements 5.5**

- [ ] 8. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 9. Extend SpecialsOrganizer for Sonarr-compatible naming
  - [ ] 9.1 Update `organize_specials` in `src/organizer.rs` to use TVDB episode numbers and Sonarr naming pattern
    - File pattern: `{series_title} - S00E{episode_number:02} - {sanitized_title}.mkv`
    - Use aired_episode_number from TVDB
    - Skip files when target already exists
    - _Requirements: 7.1, 7.2, 7.3, 7.5_
  - [ ] 9.2 Write property test for Sonarr-compatible file path construction
    - **Property 7: Sonarr-Compatible File Path Construction**
    - **Validates: Requirements 7.1, 7.2, 7.3**
  - [ ] 9.3 Write property test for filename sanitization
    - **Property 8: Filename Sanitization Removes Windows-Invalid Characters**
    - **Validates: Requirements 7.4**

- [ ] 10. Extend validation and caching
  - [ ] 10.1 Update `Validator` in `src/validation.rs` to check TVDB API key when `--specials` is enabled
    - Skip validation when specials not enabled
    - Prompt for key if missing from config, save to config.cfg
    - Verify connectivity with test authentication
    - _Requirements: 8.1, 8.2, 8.3_
  - [ ] 10.2 Extend `SeriesMetadataCache` to store TVDB Season 0 episode data with 7-day TTL
    - Reuse existing cache infrastructure
    - Support force flag to bypass cache
    - _Requirements: 9.1, 9.2, 9.3_

- [ ] 11. Wire into orchestrator and discovery pipeline
  - [ ] 11.1 Register `tvdb` module in `src/discovery/mod.rs` and re-export public types
    - Add `mod tvdb`, `mod id_bridge`, `mod monitor_policy`, `mod special_searcher`
    - Re-export `TvdbClient`, `IdBridge`, `MonitorPolicy`, `SpecialSearcher`
    - _Requirements: 6.5_
  - [ ] 11.2 Update `SeriesDiscoveryOrchestrator` in `src/discovery/series_orchestrator.rs` to integrate TVDB specials flow
    - When specials enabled: resolve TVDB ID via IdBridge, fetch Season 0, enrich, filter via MonitorPolicy, build queries via SpecialSearcher, pass to YouTube pipeline
    - Wire downloaded specials through existing download/convert/organize pipeline
    - _Requirements: 5.5, 6.5_
  - [ ] 11.3 Update `Orchestrator::new` in `src/orchestrator.rs` to accept and pass TVDB API key
    - Create `TvdbClient` and `IdBridge` when specials are enabled
    - Pass through to `SeriesDiscoveryOrchestrator`
    - _Requirements: 1.1, 1.2_
  - [ ] 11.4 Update `main.rs` to load TVDB API key from Config and pass to orchestrator
    - Load config, check for tvdb_api_key when specials enabled
    - Pass key to orchestrator
    - _Requirements: 1.1, 8.1_

- [ ] 12. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties
- Unit tests validate specific examples and edge cases
- The TVDB client reuses the existing `reqwest` HTTP client and `serde` JSON parsing
- The monitor policy is a pure function module with no I/O, making it straightforward to property-test
