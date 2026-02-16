# Requirements Document

## Introduction

This feature integrates TheTVDB API v4 as a data source for discovering and organizing TV Series Season 0 (Specials) content. The existing system uses TMDB for series metadata, but TheTVDB provides richer Season 0 episode data including airing order context, absolute numbering for anime, and movie-type specials. The feature adds a TheTVDB client, a TMDB-to-TVDB ID bridging mechanism, enhanced episode metadata, search strategies for locating special episode videos, and Sonarr-compatible file naming and organization.

## Glossary

- **TvdbClient**: The HTTP client module responsible for authenticating with and querying the TheTVDB API v4.
- **TvdbAuthenticator**: The component that obtains and manages Bearer tokens from the TheTVDB `/login` endpoint.
- **IdBridge**: The component that resolves TMDB series IDs to TheTVDB series IDs using TMDB external_ids and TVDB search fallback.
- **SpecialEpisodeMetadata**: The enriched data structure representing a Season 0 episode fetched from TheTVDB, including airing context and absolute numbering.
- **SpecialSearcher**: The component that constructs search queries for locating downloadable video files of special episodes.
- **SpecialsOrganizer**: The component that names and places downloaded special episode files into Jellyfin/Sonarr-compatible directory structures.
- **MonitorPolicy**: The rule set that determines which Season 0 episodes are actively monitored for download.
- **SeriesEntry**: An existing data structure representing a TV series discovered during library scanning.
- **FuzzyMatcher**: The existing component that performs fuzzy title matching using Levenshtein distance with an 80% similarity threshold.
- **Config**: The existing application configuration struct that stores API keys in `config.cfg` as JSON. Currently holds `tmdb_api_key`; will be extended with `tvdb_api_key`.
- **Bearer_Token**: The authentication token obtained from TheTVDB `/login` endpoint, used for all subsequent API requests.

## Requirements

### Requirement 1: TheTVDB Authentication and API Key Storage

**User Story:** As a system operator, I want the system to authenticate with TheTVDB API v4 using an API key stored in config.cfg, so that it can fetch Season 0 episode data without requiring environment variables.

#### Acceptance Criteria

1. WHEN the `--specials` flag is enabled and no `tvdb_api_key` exists in Config, THE system SHALL prompt the user to enter a TheTVDB API key and save it to `config.cfg`
2. WHEN the TvdbAuthenticator receives a valid `tvdb_api_key` from Config, THE TvdbAuthenticator SHALL obtain a Bearer_Token from `https://api4.thetvdb.com/v4/login`
3. WHEN the TvdbAuthenticator receives an invalid `tvdb_api_key`, THE TvdbAuthenticator SHALL return a descriptive authentication error
4. WHEN a Bearer_Token expires or an API request returns HTTP 401, THE TvdbAuthenticator SHALL re-authenticate and retry the request once
5. THE TvdbClient SHALL include the Bearer_Token in the `Authorization` header of every API request
6. WHEN Config is serialized and then deserialized, THE Config SHALL preserve both `tmdb_api_key` and `tvdb_api_key` fields (round-trip property)

### Requirement 2: TMDB-to-TVDB ID Bridging

**User Story:** As a system operator, I want the system to resolve TMDB series IDs to TheTVDB IDs, so that it can query TheTVDB for Season 0 data.

#### Acceptance Criteria

1. WHEN a SeriesEntry is processed for specials discovery, THE IdBridge SHALL first query the TMDB `/tv/{tmdb_id}/external_ids` endpoint to retrieve the `tvdb_id`
2. WHEN the TMDB external_ids endpoint returns a valid `tvdb_id`, THE IdBridge SHALL store the `tvdb_id` alongside the SeriesEntry for subsequent lookups
3. WHEN the TMDB external_ids endpoint returns no `tvdb_id`, THE IdBridge SHALL fall back to querying TheTVDB `/search?q={series_title}` and select the result with the highest fuzzy match score above the 80% threshold
4. WHEN the IdBridge fuzzy search fallback finds no match above the 80% threshold, THE IdBridge SHALL log a warning and skip specials discovery for that series
5. WHEN a `tvdb_id` has been previously resolved and cached, THE IdBridge SHALL reuse the cached value without re-querying

### Requirement 3: Season 0 Episode Fetching

**User Story:** As a system operator, I want the system to fetch all Season 0 episodes from TheTVDB, so that it can identify available specials for a series.

#### Acceptance Criteria

1. WHEN the TvdbClient fetches Season 0 episodes, THE TvdbClient SHALL query `GET /series/{tvdb_id}/episodes/default` with `season=0` and `page=0`
2. WHEN the API response contains additional pages, THE TvdbClient SHALL paginate through all pages until no more results remain
3. WHEN the API returns episode data, THE TvdbClient SHALL parse each episode into a SpecialEpisodeMetadata containing: tvdb_episode_id, episode_number, title, air_date, and overview
4. IF the API returns an error or empty response for Season 0, THEN THE TvdbClient SHALL return an empty list and log the condition

### Requirement 4: Extended Episode Metadata Enrichment

**User Story:** As a system operator, I want the system to enrich Season 0 episodes with extended metadata, so that it can make informed decisions about airing context and episode type.

#### Acceptance Criteria

1. WHEN a SpecialEpisodeMetadata is fetched, THE TvdbClient SHALL query `GET /episodes/{tvdb_episode_id}/extended` to retrieve extended details
2. WHEN extended data is available, THE TvdbClient SHALL populate the following fields on SpecialEpisodeMetadata: absolute_number, airs_before_season, airs_after_season, airs_before_episode, and is_movie flag
3. IF the extended endpoint returns an error for a specific episode, THEN THE TvdbClient SHALL retain the base metadata and log the enrichment failure

### Requirement 5: Monitor Policy for Specials

**User Story:** As a system operator, I want the system to selectively monitor only relevant specials, so that it avoids downloading hundreds of irrelevant episodes for long-running series.

#### Acceptance Criteria

1. THE MonitorPolicy SHALL default all Season 0 episodes to unmonitored status
2. WHEN a SpecialEpisodeMetadata has `airs_after_season` equal to the latest season number found on disk, THE MonitorPolicy SHALL automatically set that episode to monitored status
3. WHEN a SpecialEpisodeMetadata has `is_movie` set to true, THE MonitorPolicy SHALL automatically set that episode to monitored status
4. WHEN a user provides a manual monitor list via a configuration file in the series folder, THE MonitorPolicy SHALL set the specified episodes to monitored status
5. THE SpecialSearcher SHALL only search for episodes that the MonitorPolicy has marked as monitored

### Requirement 6: Special Episode Search Strategy

**User Story:** As a system operator, I want the system to construct effective search queries for special episodes, so that it can locate downloadable video content.

#### Acceptance Criteria

1. WHEN searching for a standard special episode, THE SpecialSearcher SHALL construct a query using the pattern `{series_title} S00E{aired_episode_number:02} {episode_title}`
2. WHEN the standard query returns no results, THE SpecialSearcher SHALL fall back to a query using only `{series_title} {episode_title}`
3. WHEN a SpecialEpisodeMetadata has `is_movie` set to true, THE SpecialSearcher SHALL construct a query using the pattern `{series_title} {episode_title} movie`
4. WHEN a SpecialEpisodeMetadata has a non-null `absolute_number`, THE SpecialSearcher SHALL include an additional query using the pattern `{series_title} OVA {absolute_number}`
5. THE SpecialSearcher SHALL pass all constructed queries to the existing YouTube discovery pipeline for video resolution
6. WHEN a YouTube search result has a title with fuzzy match similarity below 60% compared to the expected episode title, THE SpecialSearcher SHALL skip that result and log a warning

### Requirement 7: Sonarr-Compatible File Naming and Organization

**User Story:** As a system operator, I want downloaded specials organized with Sonarr-compatible naming, so that Jellyfin correctly identifies and displays them.

#### Acceptance Criteria

1. THE SpecialsOrganizer SHALL place downloaded special episode files into the path `{series_folder}/{specials_folder_name}/`
2. THE SpecialsOrganizer SHALL name files using the pattern `{series_title} - S00E{episode_number:02} - {sanitized_episode_title}.mkv`
3. WHEN the `episode_number` in the filename is set, THE SpecialsOrganizer SHALL use the `aired_episode_number` from TheTVDB as the episode number
4. THE SpecialsOrganizer SHALL sanitize episode titles by removing characters that are invalid on Windows filesystems
5. WHEN a file with the same target name already exists, THE SpecialsOrganizer SHALL skip the file and log a message

### Requirement 8: Validation of TheTVDB Dependencies

**User Story:** As a system operator, I want the system to validate TheTVDB configuration at startup, so that I receive clear feedback if the API key is missing.

#### Acceptance Criteria

1. WHEN the `--specials` flag is enabled and the `tvdb_api_key` field is missing from Config, THE Validator SHALL prompt the user to enter the key and save it to `config.cfg`
2. WHEN the `--specials` flag is not enabled, THE Validator SHALL skip TVDB API key validation
3. WHEN the `tvdb_api_key` is present in Config, THE Validator SHALL verify connectivity by performing a test authentication request

### Requirement 9: Caching of TVDB Metadata

**User Story:** As a system operator, I want TheTVDB metadata cached locally, so that repeated runs do not make redundant API calls.

#### Acceptance Criteria

1. WHEN Season 0 episode data is fetched for a series, THE SeriesMetadataCache SHALL store the results with a timestamp
2. WHEN cached data exists and is less than 7 days old, THE TvdbClient SHALL use the cached data instead of querying the API
3. WHEN the `--force` flag is set, THE TvdbClient SHALL bypass the cache and re-fetch from the API
4. THE SeriesMetadataCache SHALL store TVDB ID mappings separately from episode data, with no expiration

### Requirement 10: Error Handling and Logging

**User Story:** As a system operator, I want clear error messages and logging for TheTVDB operations, so that I can diagnose issues with specials discovery.

#### Acceptance Criteria

1. WHEN a TheTVDB API request fails, THE TvdbClient SHALL log the HTTP status code, endpoint URL, and response body at error level
2. WHEN a series has no TVDB ID mapping, THE system SHALL log the series name at warning level and continue processing other series
3. WHEN specials discovery completes for a series, THE system SHALL log the count of monitored episodes found at info level
4. IF a network timeout occurs during a TheTVDB request, THEN THE TvdbClient SHALL retry once after a 2-second delay before reporting failure
