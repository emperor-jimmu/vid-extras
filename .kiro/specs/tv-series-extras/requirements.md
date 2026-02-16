# Requirements Document: TV Series Extra Content Support

## Introduction

This document specifies the requirements for extending the extras_fetcher tool to support TV series extra content. The tool currently handles movie extras (trailers, featurettes, behind-the-scenes content, deleted scenes, interviews) and will be extended to support similar content for TV series, including both Season 0 specials and bonus material organized by series and season.

The feature distinguishes between two types of TV content:

- **Specials**: Season 0 episodes that are part of the official episode list (e.g., pilot episodes, holiday specials, recap episodes)
- **Extras**: Bonus material similar to movie extras (interviews, behind-the-scenes, bloopers, featurettes) that are not part of the episode list

## Glossary

- **Series**: A TV show with one or more seasons
- **Season**: A collection of episodes within a series, numbered starting from 1 (Season 0 is reserved for specials)
- **Episode**: A single video file within a season, identified by season and episode number (SxxExx format)
- **Special**: A Season 0 episode that is part of the official episode list but not part of regular seasons
- **Extra**: Bonus content (interviews, behind-the-scenes, bloopers, featurettes) that is not part of the episode list
- **Series_Entry**: A data structure representing a discovered TV series in the library
- **Series_Extra**: A data structure representing an extra video associated with a series or season
- **TMDB**: The Movie Database API, used for metadata and content discovery
- **Scanner**: The module responsible for discovering series in the library
- **Discovery_Module**: The module responsible for finding extras from external sources
- **Organizer**: The module responsible for organizing downloaded content into the correct directory structure
- **Done_Marker**: A JSON file indicating that processing has been completed for a series or season
- **Jellyfin_Structure**: The directory layout expected by Jellyfin media server for TV series
- **Season_Pack**: A downloaded archive containing multiple episodes and bonus content for a season

## Requirements

### Requirement 1: Series Library Scanning

**User Story:** As a user, I want the tool to scan my TV series library, so that it can discover series that need extras processing.

#### Acceptance Criteria

1. WHEN the Scanner processes a directory, THE Scanner SHALL identify TV series folders using the pattern `{Series Name} (YYYY)` or `{Series Name}`
2. WHEN a series folder is identified, THE Scanner SHALL parse the series name and optional year from the folder name
3. WHEN a series folder contains a done marker file, THE Scanner SHALL skip that series unless the force flag is enabled
4. WHEN the Scanner encounters nested season folders (Season 01, Season 02, etc.), THE Scanner SHALL recognize them as part of the parent series
5. WHEN the Scanner encounters a Season 00 folder, THE Scanner SHALL recognize it as the specials season
6. THE Scanner SHALL return a list of Series_Entry objects containing the series path, name, year, and done marker status

### Requirement 2: Series Data Model

**User Story:** As a developer, I want a data model for TV series, so that the system can represent series, seasons, and extras consistently.

#### Acceptance Criteria

1. THE System SHALL define a Series_Entry struct with fields for path, title, optional year, and done marker status
2. THE System SHALL define a Series_Extra struct with fields for series ID, season number (optional), category, title, URL, source type, and local path
3. WHEN a Series_Extra has no season number, THE System SHALL treat it as series-level content
4. WHEN a Series_Extra has a season number, THE System SHALL treat it as season-specific content
5. THE System SHALL support the same ContentCategory enum values as movie extras (Trailer, Featurette, BehindTheScenes, DeletedScene, Interview)
6. THE System SHALL serialize and deserialize Series_Extra objects to JSON format for storage

### Requirement 3: TMDB Series Discovery

**User Story:** As a user, I want the tool to discover series extras from TMDB, so that I can automatically find official bonus content.

#### Acceptance Criteria

1. WHEN the Discovery_Module searches for a series, THE Discovery_Module SHALL query the TMDB TV search endpoint with the series name and optional year
2. WHEN TMDB returns multiple results, THE Discovery_Module SHALL select the first result with a matching year or the first result if no year is specified
3. WHEN a TMDB series ID is found, THE Discovery_Module SHALL query the TMDB TV videos endpoint to retrieve the videos list
4. WHEN TMDB returns video type "Trailer", THE Discovery_Module SHALL map it to ContentCategory::Trailer
5. WHEN TMDB returns video type "Behind the Scenes", THE Discovery_Module SHALL map it to ContentCategory::BehindTheScenes
6. WHEN TMDB returns video type "Featurette", THE Discovery_Module SHALL map it to ContentCategory::Featurette
7. WHEN TMDB returns video type "Bloopers", THE Discovery_Module SHALL map it to ContentCategory::Featurette
8. WHEN TMDB returns an unknown video type, THE Discovery_Module SHALL skip that video
9. WHEN TMDB API calls fail, THE Discovery_Module SHALL log the error and continue processing

### Requirement 4: TMDB Season 0 Specials Discovery

**User Story:** As a user, I want the tool to discover Season 0 specials from TMDB, so that I can download official special episodes.

#### Acceptance Criteria

1. WHEN the Discovery_Module processes a series, THE Discovery_Module SHALL query the TMDB TV season endpoint for Season 0
2. WHEN TMDB returns Season 0 episodes, THE Discovery_Module SHALL extract episode numbers, titles, and air dates
3. WHEN a Season 0 episode has an associated video, THE Discovery_Module SHALL include the video URL in the discovery results
4. WHEN Season 0 does not exist for a series, THE Discovery_Module SHALL continue processing without error
5. THE Discovery_Module SHALL store Season 0 episodes separately from regular extras

### Requirement 5: YouTube Series Extras Discovery

**User Story:** As a user, I want the tool to discover series extras from YouTube, so that I can find interviews, behind-the-scenes content, and other bonus material.

#### Acceptance Criteria

1. WHEN the Discovery_Module searches YouTube for series extras, THE Discovery_Module SHALL construct search queries using the series name and year
2. WHEN searching for interviews, THE Discovery_Module SHALL use the query pattern "{series_name} {year} cast interview"
3. WHEN searching for behind-the-scenes content, THE Discovery_Module SHALL use the query pattern "{series_name} {year} behind the scenes"
4. WHEN searching for bloopers, THE Discovery_Module SHALL use the query pattern "{series_name} {year} bloopers"
5. WHEN searching for featurettes, THE Discovery_Module SHALL use the query pattern "{series_name} {year} featurette"
6. THE Discovery_Module SHALL apply the same duration filtering as movie extras (30 seconds to 20 minutes)
7. THE Discovery_Module SHALL apply the same keyword filtering as movie extras (excluding Review, Reaction, Analysis, Explained, Ending, Theory, React)
8. THE Discovery_Module SHALL exclude YouTube Shorts (videos under 60 seconds with vertical aspect ratio)
9. WHEN YouTube search fails, THE Discovery_Module SHALL log the error and continue processing

### Requirement 6: Season-Specific Extras Discovery

**User Story:** As a user, I want the tool to discover season-specific extras, so that bonus content is organized by season.

#### Acceptance Criteria

1. WHEN the Discovery_Module searches for season-specific extras, THE Discovery_Module SHALL include the season number in the search query
2. WHEN searching for Season 1 extras, THE Discovery_Module SHALL use the query pattern "{series_name} season 1 {content_type}"
3. WHEN season-specific results are found, THE Discovery_Module SHALL tag them with the season number
4. WHEN season-specific search returns no results, THE Discovery_Module SHALL fall back to series-level search
5. THE Discovery_Module SHALL support season-specific discovery for seasons 1 through 99

### Requirement 7: Directory Structure Organization

**User Story:** As a user, I want extras organized in a Jellyfin-compatible directory structure, so that my media server can recognize and display them.

#### Acceptance Criteria

1. WHEN the Organizer processes series-level extras, THE Organizer SHALL place them in subdirectories under the series root folder
2. WHEN the Organizer processes season-specific extras, THE Organizer SHALL place them in subdirectories under the season folder
3. WHEN organizing Trailer content, THE Organizer SHALL create a `trailers` subdirectory
4. WHEN organizing Featurette content, THE Organizer SHALL create a `featurettes` subdirectory
5. WHEN organizing BehindTheScenes content, THE Organizer SHALL create a `behind the scenes` subdirectory
6. WHEN organizing DeletedScene content, THE Organizer SHALL create a `deleted scenes` subdirectory
7. WHEN organizing Interview content, THE Organizer SHALL create an `interviews` subdirectory
8. THE Organizer SHALL create subdirectories if they do not exist
9. THE Organizer SHALL preserve the existing directory structure for episodes

### Requirement 8: Season 0 Specials Organization

**User Story:** As a user, I want Season 0 specials organized correctly, so that Jellyfin recognizes them as special episodes.

#### Acceptance Criteria

1. WHEN the Organizer processes Season 0 content, THE Organizer SHALL place files in a `Season 00` folder under the series root
2. WHEN naming Season 0 files, THE Organizer SHALL use the format `{Series Name} - S00E{episode_number} - {episode_title}.mp4`
3. WHEN the episode number is less than 10, THE Organizer SHALL zero-pad it to two digits
4. WHEN the episode title contains invalid filename characters, THE Organizer SHALL sanitize them
5. THE Organizer SHALL preserve the original video quality and format for Season 0 episodes

### Requirement 9: Done Marker Management for Series

**User Story:** As a user, I want the tool to track which series have been processed, so that it does not reprocess completed series.

#### Acceptance Criteria

1. WHEN processing completes for a series, THE Organizer SHALL create a `.extras_done` file in the series root folder
2. WHEN creating a done marker, THE Organizer SHALL include an ISO 8601 timestamp and the tool version
3. WHEN the Scanner encounters a done marker, THE Scanner SHALL skip that series unless the force flag is enabled
4. WHEN the force flag is enabled, THE Scanner SHALL ignore all done markers and process all series
5. WHEN a done marker file is invalid or corrupted, THE Scanner SHALL treat it as missing and process the series

### Requirement 10: Series vs Movie Detection

**User Story:** As a user, I want the tool to automatically detect whether a folder contains a movie or a TV series, so that it applies the correct processing logic.

#### Acceptance Criteria

1. WHEN the Scanner encounters a folder with season subfolders (Season 01, Season 02, etc.), THE Scanner SHALL classify it as a series
2. WHEN the Scanner encounters a folder with video files directly inside, THE Scanner SHALL classify it as a movie
3. WHEN the Scanner encounters a folder with both season subfolders and video files, THE Scanner SHALL classify it as a series
4. WHEN the Scanner cannot determine the type, THE Scanner SHALL log a warning and skip the folder
5. THE Scanner SHALL support a `--type` flag to force classification as either movie or series

### Requirement 11: Parallel Processing for Series

**User Story:** As a user, I want the tool to process multiple series in parallel, so that large libraries are processed efficiently.

#### Acceptance Criteria

1. WHEN the Orchestrator processes multiple series, THE Orchestrator SHALL respect the concurrency limit parameter
2. WHEN the concurrency limit is set to 1, THE Orchestrator SHALL process series sequentially
3. WHEN the concurrency limit is greater than 1, THE Orchestrator SHALL process up to N series in parallel
4. WHEN processing a single series, THE Orchestrator SHALL download extras sequentially to avoid overwhelming the network
5. WHEN one series fails, THE Orchestrator SHALL continue processing other series without interruption

### Requirement 12: Configuration Options for Series

**User Story:** As a user, I want configuration options for series processing, so that I can customize the tool's behavior.

#### Acceptance Criteria

1. THE CLI SHALL support a `--series-only` flag to process only TV series and skip movies
2. THE CLI SHALL support a `--movies-only` flag to process only movies and skip TV series
3. WHEN neither flag is specified, THE CLI SHALL process both movies and series
4. THE CLI SHALL support a `--season-extras` flag to enable season-specific extras discovery
5. WHEN the `--season-extras` flag is disabled, THE Discovery_Module SHALL only discover series-level extras
6. THE CLI SHALL support a `--specials` flag to enable Season 0 specials discovery
7. WHEN the `--specials` flag is disabled, THE Discovery_Module SHALL skip Season 0 discovery

### Requirement 13: Error Handling for Series Processing

**User Story:** As a developer, I want comprehensive error handling for series processing, so that failures are logged and do not crash the application.

#### Acceptance Criteria

1. WHEN TMDB API calls fail for a series, THE System SHALL log the error and continue processing
2. WHEN YouTube search fails for a series, THE System SHALL log the error and continue processing
3. WHEN a download fails for a series extra, THE System SHALL log the error and continue with other extras
4. WHEN conversion fails for a series extra, THE System SHALL preserve the original file and log the error
5. WHEN organization fails for a series, THE System SHALL log the error and not create a done marker
6. THE System SHALL provide clear error messages indicating which series and operation failed

### Requirement 14: Series Metadata Caching

**User Story:** As a user, I want the tool to cache series metadata, so that repeated runs do not make unnecessary API calls.

#### Acceptance Criteria

1. WHEN the Discovery_Module retrieves series metadata from TMDB, THE Discovery_Module SHALL cache the results to disk
2. WHEN the cache contains valid metadata for a series, THE Discovery_Module SHALL use the cached data instead of making API calls
3. WHEN the cache is older than 7 days, THE Discovery_Module SHALL refresh the metadata from TMDB
4. WHEN the force flag is enabled, THE Discovery_Module SHALL ignore the cache and fetch fresh metadata
5. THE Discovery_Module SHALL store cache files in a `.cache` directory under the series folder

### Requirement 15: Season Pack Post-Processing

**User Story:** As a user, I want the tool to extract extras from season pack downloads, so that bonus content included in season packs is organized correctly.

#### Acceptance Criteria

1. WHEN a season pack archive is downloaded, THE System SHALL extract all files to a temporary directory
2. WHEN extracted files include bonus content (files matching extra patterns), THE System SHALL identify them by filename
3. WHEN a file matches the pattern "behind the scenes", THE System SHALL classify it as ContentCategory::BehindTheScenes
4. WHEN a file matches the pattern "deleted scene", THE System SHALL classify it as ContentCategory::DeletedScene
5. WHEN a file matches the pattern "interview", THE System SHALL classify it as ContentCategory::Interview
6. WHEN a file matches the pattern "featurette", THE System SHALL classify it as ContentCategory::Featurette
7. WHEN a file matches the pattern "blooper", THE System SHALL classify it as ContentCategory::Featurette
8. THE System SHALL move identified extras to the appropriate subdirectories
9. THE System SHALL delete the temporary extraction directory after processing

### Requirement 16: Local Import Scanning

**User Story:** As a user, I want the tool to scan for existing Season 0 files in my library, so that it can import and organize them correctly.

#### Acceptance Criteria

1. WHEN the Scanner processes a series folder, THE Scanner SHALL search for files matching the pattern `S00E{number}`
2. WHEN a Season 0 file is found outside the Season 00 folder, THE Scanner SHALL move it to the Season 00 folder
3. WHEN a Season 0 file has an incorrect naming format, THE Scanner SHALL rename it to the standard format
4. WHEN multiple Season 0 files have the same episode number, THE Scanner SHALL log a warning and skip duplicates
5. THE Scanner SHALL preserve the original file quality and format during import

### Requirement 17: Fuzzy Title Matching

**User Story:** As a user, I want the tool to use fuzzy matching for extra titles, so that it can identify relevant content even with slight title variations.

#### Acceptance Criteria

1. WHEN comparing extra titles to series names, THE Discovery_Module SHALL normalize both strings by removing special characters and converting to lowercase
2. WHEN calculating title similarity, THE Discovery_Module SHALL use a string distance algorithm (Levenshtein distance)
3. WHEN the similarity score is above 80%, THE Discovery_Module SHALL consider the titles a match
4. WHEN the similarity score is below 80%, THE Discovery_Module SHALL exclude the extra from results
5. THE Discovery_Module SHALL log the similarity score for debugging purposes

### Requirement 18: Series Progress Reporting

**User Story:** As a user, I want progress reporting for series processing, so that I can monitor the tool's activity.

#### Acceptance Criteria

1. WHEN processing begins for a series, THE Output_Module SHALL display the series name and year
2. WHEN extras are discovered, THE Output_Module SHALL display the count of extras found per source
3. WHEN downloads complete, THE Output_Module SHALL display the count of successful and failed downloads
4. WHEN conversions complete, THE Output_Module SHALL display the count of successful and failed conversions
5. WHEN processing completes for a series, THE Output_Module SHALL display a summary with total extras organized
6. THE Output_Module SHALL use colored output to distinguish success, warning, and error messages

### Requirement 19: Series Summary Statistics

**User Story:** As a user, I want summary statistics after processing, so that I can see the overall results.

#### Acceptance Criteria

1. WHEN all series processing completes, THE Output_Module SHALL display the total number of series processed
2. THE Output_Module SHALL display the number of series that completed successfully
3. THE Output_Module SHALL display the number of series that failed
4. THE Output_Module SHALL display the total number of extras downloaded across all series
5. THE Output_Module SHALL display the total number of extras converted across all series
6. THE Output_Module SHALL display the total processing time

### Requirement 20: Backward Compatibility

**User Story:** As a user, I want the tool to maintain backward compatibility with movie processing, so that existing functionality is not broken.

#### Acceptance Criteria

1. WHEN processing a library with only movies, THE System SHALL function identically to the previous version
2. WHEN processing a library with only series, THE System SHALL use the new series processing logic
3. WHEN processing a mixed library, THE System SHALL correctly identify and process both movies and series
4. THE System SHALL maintain the same CLI interface for movie-only processing
5. THE System SHALL maintain the same done marker format for movies
