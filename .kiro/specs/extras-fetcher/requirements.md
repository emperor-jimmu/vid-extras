# Requirements Document: extras_fetcher

## Introduction

The extras_fetcher is a Rust-based automation utility designed to enrich Jellyfin movie libraries by automatically discovering, downloading, and organizing supplementary video content (trailers, behind-the-scenes footage, deleted scenes, featurettes) from multiple online sources. The system scans a movie library directory structure, identifies movies missing extra content, fetches relevant videos from TheMovieDB, Archive.org, and YouTube, converts them to efficient x265 format, and organizes them according to Jellyfin's directory conventions.

## Glossary

- **Extras_Fetcher**: The Rust-based video extras downloader system
- **Movie_Folder**: A directory containing a movie file, named in format "Movie Title (Year)"
- **Root_Directory**: The top-level directory containing all Movie_Folders
- **Extra_Content**: Supplementary video materials including trailers, behind-the-scenes, deleted scenes, featurettes, and bloopers
- **Done_Marker**: A file named "done.ext" containing JSON timestamp indicating completed processing
- **TMDB**: TheMovieDB API service for movie metadata and video links
- **Archive_Org**: Internet Archive's Moving Image Archive for historical EPK content
- **EPK**: Electronic Press Kit - promotional materials for movies
- **Target_Subdirectory**: Jellyfin-compliant folders (/trailers, /featurettes, /behind the scenes, /deleted scenes)
- **Temp_Folder**: Temporary staging directory for downloads before processing
- **CRF**: Constant Rate Factor - video quality parameter for ffmpeg encoding

## Requirements

### Requirement 1: Library Scanning and Discovery

**User Story:** As a Jellyfin administrator, I want the system to scan my movie library directory, so that it can identify which movies need extra content downloaded.

#### Acceptance Criteria

1. WHEN the system starts, THE Extras_Fetcher SHALL accept a Root_Directory path as a command-line argument
2. WHEN the --help flag is provided, THE Extras_Fetcher SHALL display usage information and exit
3. WHEN the --version flag is provided, THE Extras_Fetcher SHALL display version information and exit
4. WHEN the --force flag is provided, THE Extras_Fetcher SHALL ignore Done_Marker files and reprocess all movies
5. WHEN the --mode parameter is provided with value "youtube", THE Extras_Fetcher SHALL only use YouTube as a content source
6. WHEN scanning begins, THE Extras_Fetcher SHALL recursively traverse all subdirectories within the Root_Directory
7. WHEN a directory is encountered, THE Extras_Fetcher SHALL parse the folder name to extract Movie Title and Release Year using the pattern "Movie Title (Year)"
8. WHEN a Movie_Folder contains a Done_Marker file and --force is not set, THE Extras_Fetcher SHALL skip that folder and continue to the next
9. WHEN a Movie_Folder does not contain a Done_Marker file, THE Extras_Fetcher SHALL add it to the processing queue

### Requirement 2: Done Marker Management

**User Story:** As a system operator, I want the tool to track which movies have been processed, so that I can safely run the tool multiple times without reprocessing completed movies.

#### Acceptance Criteria

1. WHEN processing for a Movie_Folder completes successfully, THE Extras_Fetcher SHALL create a Done_Marker file in that folder
2. WHEN creating a Done_Marker, THE Extras_Fetcher SHALL write JSON content with a "finished_at" timestamp in ISO 8601 format
3. WHEN the system encounters a Done_Marker during scanning, THE Extras_Fetcher SHALL skip all processing for that Movie_Folder
4. WHEN parsing a Done_Marker fails, THE Extras_Fetcher SHALL treat the folder as unprocessed

### Requirement 3: TMDB Content Discovery

**User Story:** As a content curator, I want the system to fetch official extra content from TheMovieDB, so that I can have high-quality, verified supplementary materials.

#### Acceptance Criteria

1. WHEN querying TMDB, THE Extras_Fetcher SHALL use the Movie Title and Release Year to search for the movie
2. WHEN a TMDB movie match is found, THE Extras_Fetcher SHALL retrieve the movie's unique identifier
3. WHEN fetching videos, THE Extras_Fetcher SHALL request the "Videos" list from the TMDB API
4. WHEN processing TMDB video entries with type "Trailer", THE Extras_Fetcher SHALL map them to the /trailers Target_Subdirectory
5. WHEN processing TMDB video entries with type "Behind the Scenes", THE Extras_Fetcher SHALL map them to the /behind the scenes Target_Subdirectory
6. WHEN processing TMDB video entries with type "Deleted Scene", THE Extras_Fetcher SHALL map them to the /deleted scenes Target_Subdirectory
7. WHEN processing TMDB video entries with type "Featurette", THE Extras_Fetcher SHALL map them to the /featurettes Target_Subdirectory
8. WHEN processing TMDB video entries with type "Bloopers", THE Extras_Fetcher SHALL map them to the /featurettes Target_Subdirectory
9. WHEN TMDB API requests fail, THE Extras_Fetcher SHALL log the error and continue processing with other sources

### Requirement 4: Archive.org Content Discovery

**User Story:** As a classic film enthusiast, I want the system to search Archive.org for historical EPK content, so that older movies can have supplementary materials that may not be available elsewhere.

#### Acceptance Criteria

1. WHEN a movie's Release Year is less than 2010, THE Extras_Fetcher SHALL query Archive_Org for EPK content
2. WHEN a movie's Release Year is 2010 or later, THE Extras_Fetcher SHALL skip Archive_Org queries for that movie
3. WHEN querying Archive_Org, THE Extras_Fetcher SHALL search the Moving Image Archive collection
4. WHEN constructing Archive_Org search queries, THE Extras_Fetcher SHALL use the pattern: title:"Movie Title" AND (subject:"EPK" OR subject:"Making of")
5. WHEN Archive_Org results contain "EPK" in the subject, THE Extras_Fetcher SHALL map them to /featurettes or /behind the scenes Target_Subdirectory
6. WHEN Archive_Org results contain "Making of" in the subject, THE Extras_Fetcher SHALL map them to /behind the scenes Target_Subdirectory
7. WHEN Archive_Org API requests fail, THE Extras_Fetcher SHALL log the error and continue processing with other sources

### Requirement 5: YouTube Content Discovery

**User Story:** As a completionist, I want the system to search YouTube for additional extra content, so that gaps in coverage from official sources can be filled with community-uploaded materials.

#### Acceptance Criteria

1. THE Extras_Fetcher SHALL query YouTube for all movies regardless of content found from other sources
2. WHEN searching YouTube, THE Extras_Fetcher SHALL use yt-dlp's ytsearch operator
3. WHEN searching for deleted scenes, THE Extras_Fetcher SHALL use the query pattern "{Movie Title} {Year} deleted scenes"
4. WHEN searching for behind-the-scenes content, THE Extras_Fetcher SHALL use the query pattern "{Movie Title} {Year} behind the scenes"
5. WHEN searching for bloopers, THE Extras_Fetcher SHALL use the query pattern "{Movie Title} {Year} bloopers"
6. WHEN searching for interviews, THE Extras_Fetcher SHALL use the query pattern "{Movie Title} {Year} cast interview"
7. WHEN a YouTube video duration exceeds 20 minutes, THE Extras_Fetcher SHALL exclude it from results
8. WHEN a YouTube video duration is less than 30 seconds, THE Extras_Fetcher SHALL exclude it from results
9. WHEN a YouTube video title contains "Review", "Reaction", "Analysis", "Explained", "Ending", "Theory", or "React", THE Extras_Fetcher SHALL exclude it from results
10. WHEN a YouTube video is identified as a YouTube Short, THE Extras_Fetcher SHALL exclude it from results
11. WHEN YouTube search fails, THE Extras_Fetcher SHALL log the error and continue with available content

### Requirement 6: Content Acquisition

**User Story:** As a system administrator, I want downloads to be staged in a temporary location, so that incomplete or failed downloads don't corrupt the organized library structure.

#### Acceptance Criteria

1. WHEN downloading content, THE Extras_Fetcher SHALL create a Temp_Folder at /tmp_downloads/{MovieID}/
2. WHEN initiating a download, THE Extras_Fetcher SHALL use yt-dlp to fetch the video URL
3. WHEN a download completes, THE Extras_Fetcher SHALL verify the yt-dlp exit code is zero
4. WHEN a download fails with non-zero exit code, THE Extras_Fetcher SHALL delete any partial files created
5. WHEN a download fails, THE Extras_Fetcher SHALL log the error and continue with remaining content
6. WHEN network timeouts occur, THE Extras_Fetcher SHALL handle them gracefully without crashing
7. WHEN all downloads for a movie complete, THE Extras_Fetcher SHALL proceed to the processing phase

### Requirement 7: Video Processing and Conversion

**User Story:** As a storage-conscious administrator, I want all downloaded videos converted to efficient x265 format, so that my library storage requirements are minimized while maintaining quality.

#### Acceptance Criteria

1. WHEN converting video files, THE Extras_Fetcher SHALL use ffmpeg with x265 (HEVC) codec
2. WHEN encoding video, THE Extras_Fetcher SHALL use CRF value between 24 and 26
3. WHERE hardware acceleration is available, THE Extras_Fetcher SHALL use hevc_nvenc or hevc_qsv encoder
4. WHEN ffmpeg conversion completes successfully, THE Extras_Fetcher SHALL delete the original raw download file
5. WHEN ffmpeg conversion fails, THE Extras_Fetcher SHALL delete the failed output file and log the error
6. WHEN conversion fails, THE Extras_Fetcher SHALL retain the original download for manual inspection
7. WHEN all conversions for a movie complete, THE Extras_Fetcher SHALL proceed to the organization phase

### Requirement 8: Content Organization

**User Story:** As a Jellyfin user, I want extra content organized into standard subdirectories, so that Jellyfin automatically recognizes and displays the supplementary materials.

#### Acceptance Criteria

1. WHEN organizing trailer content, THE Extras_Fetcher SHALL move files to the /trailers Target_Subdirectory
2. WHEN organizing featurette content, THE Extras_Fetcher SHALL move files to the /featurettes Target_Subdirectory
3. WHEN organizing behind-the-scenes content, THE Extras_Fetcher SHALL move files to the /behind the scenes Target_Subdirectory
4. WHEN organizing deleted scene content, THE Extras_Fetcher SHALL move files to the /deleted scenes Target_Subdirectory
5. WHEN a Target_Subdirectory does not exist, THE Extras_Fetcher SHALL create it before moving files
6. WHEN file moves complete successfully, THE Extras_Fetcher SHALL delete the Temp_Folder
7. WHEN all content is organized, THE Extras_Fetcher SHALL create the Done_Marker file

### Requirement 9: Parallel Processing

**User Story:** As a performance-conscious operator, I want the ability to process multiple movies simultaneously, so that large libraries can be enriched in reasonable time.

#### Acceptance Criteria

1. THE Extras_Fetcher SHALL process downloads sequentially within a single movie
2. WHERE parallel processing is enabled, THE Extras_Fetcher SHALL process multiple movies concurrently
3. WHEN parallel processing is configured, THE Extras_Fetcher SHALL accept a concurrency limit parameter
4. WHEN the concurrency limit is reached, THE Extras_Fetcher SHALL queue additional movies until slots become available
5. WHEN parallel processing is disabled, THE Extras_Fetcher SHALL process movies one at a time

### Requirement 10: Error Handling and Cleanup

**User Story:** As a system maintainer, I want the tool to handle errors gracefully and clean up temporary files, so that failed operations don't leave the system in an inconsistent state.

#### Acceptance Criteria

1. WHEN any operation fails, THE Extras_Fetcher SHALL log detailed error information including movie title and operation type
2. WHEN processing for a movie fails, THE Extras_Fetcher SHALL continue processing remaining movies in the queue
3. WHEN the system exits, THE Extras_Fetcher SHALL ensure no temporary files remain in Temp_Folder locations
4. WHEN a Temp_Folder contains files from a previous failed run, THE Extras_Fetcher SHALL clean them before starting new downloads
5. WHEN critical errors occur (missing binaries, invalid API keys), THE Extras_Fetcher SHALL exit with a descriptive error message

### Requirement 11: Configuration and Dependencies

**User Story:** As a system installer, I want clear validation of required dependencies and configuration, so that I can ensure the tool will function correctly before processing begins.

#### Acceptance Criteria

1. WHEN the system starts, THE Extras_Fetcher SHALL verify that yt-dlp binary is available in the system PATH
2. WHEN the system starts, THE Extras_Fetcher SHALL verify that ffmpeg binary is available in the system PATH
3. WHEN the system starts, THE Extras_Fetcher SHALL verify that ffmpeg supports x265/HEVC encoding
4. WHEN the system starts, THE Extras_Fetcher SHALL verify that a TMDB API key is configured
5. WHEN any required dependency is missing, THE Extras_Fetcher SHALL exit with an error message indicating which dependency is missing
6. WHEN hardware acceleration is unavailable, THE Extras_Fetcher SHALL fall back to software encoding and log a warning

### Requirement 12: Idempotency and Safe Re-execution

**User Story:** As an automation engineer, I want the tool to be safely re-runnable, so that I can schedule it as a recurring job without risk of duplicate work or data corruption.

#### Acceptance Criteria

1. WHEN the system processes a Movie_Folder that already has a Done_Marker, THE Extras_Fetcher SHALL skip all operations for that folder
2. WHEN the system is interrupted mid-processing, THE Extras_Fetcher SHALL safely resume on the next run by checking for Done_Marker files
3. WHEN re-running on a partially processed library, THE Extras_Fetcher SHALL only process Movie_Folders without Done_Marker files
4. WHEN a Movie_Folder has some but not all Target_Subdirectories, THE Extras_Fetcher SHALL treat it as incomplete if no Done_Marker exists

### Requirement 13: CLI Output and User Experience

**User Story:** As a system operator, I want colorful, informative CLI output, so that I can monitor progress and understand what the tool is doing at each stage.

#### Acceptance Criteria

1. WHEN the system starts, THE Extras_Fetcher SHALL display a colored banner with the tool name and version
2. WHEN displaying parameters, THE Extras_Fetcher SHALL show all active configuration values including Root_Directory, mode, and flags
3. WHEN scanning directories, THE Extras_Fetcher SHALL display progress with colored status indicators (green for found, yellow for skipped, red for errors)
4. WHEN downloading content, THE Extras_Fetcher SHALL display a progress indicator showing current file, source, and download percentage
5. WHEN converting videos, THE Extras_Fetcher SHALL display conversion progress with colored status (blue for processing, green for complete)
6. WHEN operations complete, THE Extras_Fetcher SHALL display a summary with colored statistics (total movies processed, files downloaded, errors encountered)
7. WHEN errors occur, THE Extras_Fetcher SHALL display error messages in red with clear context
8. WHEN verbose logging is needed, THE Extras_Fetcher SHALL support a --verbose flag for detailed output

### Requirement 14: Language and Tooling Standards

**User Story:** As a Rust developer, I want the codebase to use modern Rust standards and leverage available development tools, so that the code is maintainable and follows best practices.

#### Acceptance Criteria

1. THE Extras_Fetcher SHALL be implemented using Rust 2024 edition
2. WHERE API documentation is needed, THE Extras_Fetcher development SHALL use Context7 MCP for language references and API documentation
3. WHERE automated testing is required, THE Extras_Fetcher development SHALL use Playwright MCP for browser automation testing
4. THE Extras_Fetcher SHALL follow Rust 2024 idioms and best practices
5. THE Extras_Fetcher SHALL use cargo for dependency management and building
