// Scanner module - handles directory traversal and movie discovery

use crate::error::ScanError;
use crate::models::{DoneMarker, MediaType, MovieEntry, SeriesEntry};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

const DONE_MARKER_FILENAME: &str = "done.ext";

/// Pre-compiled regex for parsing "Title (Year)" folder names
static FOLDER_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)\s*\((\d{4})\)$").expect("folder name regex is valid"));

/// Pre-compiled regex for detecting "Season N" folders
static SEASON_FOLDER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Season \d+$").expect("season folder regex is valid"));

/// Pre-compiled regex for extracting season numbers from "Season N" folders
static SEASON_NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Season (\d+)$").expect("season number regex is valid"));

/// Scanner for traversing movie library directories
pub struct Scanner {
    root_dir: PathBuf,
    force: bool,
    single: bool,
}

impl Scanner {
    /// Create a new Scanner instance
    pub fn new(root_dir: PathBuf, force: bool, single: bool) -> Self {
        Self {
            root_dir,
            force,
            single,
        }
    }

    /// Scan the root directory and return a list of movies to process
    /// Returns movies sorted alphabetically by title
    pub fn scan(&self) -> Result<Vec<MovieEntry>, ScanError> {
        // If single mode, treat root_dir as a single movie folder
        if self.single {
            return self.scan_single_folder();
        }

        // Otherwise, scan for multiple movie folders
        let mut movies = Vec::new();
        self.scan_directory(&self.root_dir, &mut movies)?;

        // Sort alphabetically by title
        movies.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

        Ok(movies)
    }

    /// Scan the root directory and return both movies and series
    /// Classifies each folder using detect_media_type
    /// Returns (movies, series) tuple sorted alphabetically by title
    pub fn scan_all(&self) -> Result<(Vec<MovieEntry>, Vec<SeriesEntry>), ScanError> {
        // If single mode, detect type of the single folder
        if self.single {
            return self.scan_single_folder_all();
        }

        // Otherwise, scan for both movies and series
        let mut movies = Vec::new();
        let mut series = Vec::new();
        self.scan_directory_all(&self.root_dir, &mut movies, &mut series)?;

        // Sort alphabetically by title
        movies.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        series.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

        Ok((movies, series))
    }

    /// Scan a single folder and classify it as movie or series
    fn scan_single_folder_all(&self) -> Result<(Vec<MovieEntry>, Vec<SeriesEntry>), ScanError> {
        // Check if directory exists and is readable
        if !self.root_dir.exists() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {:?}", self.root_dir),
            )));
        }

        if !self.root_dir.is_dir() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Path is not a directory: {:?}", self.root_dir),
            )));
        }

        // Try to parse the folder name
        let folder_name = self
            .root_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                ScanError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid folder name: {:?}", self.root_dir),
                ))
            })?;

        // Detect media type
        let media_type = Self::detect_media_type(&self.root_dir);

        match media_type {
            MediaType::Movie => {
                // Parse as movie
                let (title, year) = Self::parse_folder_name(folder_name).ok_or_else(|| {
                    ScanError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "Folder name does not match expected format 'Title (Year)': {}",
                            folder_name
                        ),
                    ))
                })?;

                let has_done_marker = Self::check_done_marker(&self.root_dir);

                if has_done_marker && !self.force {
                    log::info!(
                        "Skipping {} (done marker found, use --force to reprocess)",
                        folder_name
                    );
                    return Ok((Vec::new(), Vec::new()));
                }

                Ok((
                    vec![MovieEntry {
                        path: self.root_dir.clone(),
                        title,
                        year,
                        has_done_marker,
                    }],
                    Vec::new(),
                ))
            }
            MediaType::Series => {
                // Parse as series
                let (title, year) =
                    Self::parse_series_folder_name(folder_name).ok_or_else(|| {
                        ScanError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("Invalid series folder name: {}", folder_name),
                        ))
                    })?;

                let has_done_marker = Self::check_done_marker(&self.root_dir);

                if has_done_marker && !self.force {
                    log::info!(
                        "Skipping {} (done marker found, use --force to reprocess)",
                        folder_name
                    );
                    return Ok((Vec::new(), Vec::new()));
                }

                let seasons = Self::detect_season_folders(&self.root_dir);

                Ok((
                    Vec::new(),
                    vec![SeriesEntry {
                        path: self.root_dir.clone(),
                        title,
                        year,
                        has_done_marker,
                        seasons,
                    }],
                ))
            }
            MediaType::Unknown => {
                log::warn!("Could not determine media type for: {}", folder_name);
                Ok((Vec::new(), Vec::new()))
            }
        }
    }

    /// Recursively scan a directory for both movies and series
    fn scan_directory_all(
        &self,
        dir: &Path,
        movies: &mut Vec<MovieEntry>,
        series: &mut Vec<SeriesEntry>,
    ) -> Result<(), ScanError> {
        // Check if directory exists and is readable
        if !dir.exists() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {:?}", dir),
            )));
        }

        if !dir.is_dir() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Path is not a directory: {:?}", dir),
            )));
        }

        // Read directory entries
        let entries = fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only process directories
            if !path.is_dir() {
                continue;
            }

            // Try to parse folder name
            if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                // Detect media type
                let media_type = Self::detect_media_type(&path);

                match media_type {
                    MediaType::Movie => {
                        // Try to parse as movie
                        if let Some((title, year)) = Self::parse_folder_name(folder_name) {
                            let has_done_marker = Self::check_done_marker(&path);

                            if has_done_marker && !self.force {
                                log::debug!("Skipping {} (done marker found)", folder_name);
                                continue;
                            }

                            movies.push(MovieEntry {
                                path: path.clone(),
                                title,
                                year,
                                has_done_marker,
                            });
                        } else {
                            // Movie type detected but name doesn't parse, recurse
                            self.scan_directory_all(&path, movies, series)?;
                        }
                    }
                    MediaType::Series => {
                        // Try to parse as series
                        if let Some((title, year)) = Self::parse_series_folder_name(folder_name) {
                            let has_done_marker = Self::check_done_marker(&path);

                            if has_done_marker && !self.force {
                                log::debug!("Skipping {} (done marker found)", folder_name);
                                continue;
                            }

                            let seasons = Self::detect_season_folders(&path);

                            series.push(SeriesEntry {
                                path: path.clone(),
                                title,
                                year,
                                has_done_marker,
                                seasons,
                            });
                        } else {
                            // Series type detected but name doesn't parse, recurse
                            self.scan_directory_all(&path, movies, series)?;
                        }
                    }
                    MediaType::Unknown => {
                        // Unknown type, recurse into it
                        self.scan_directory_all(&path, movies, series)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Scan a single movie folder directly
    fn scan_single_folder(&self) -> Result<Vec<MovieEntry>, ScanError> {
        // Check if directory exists and is readable
        if !self.root_dir.exists() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {:?}", self.root_dir),
            )));
        }

        if !self.root_dir.is_dir() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Path is not a directory: {:?}", self.root_dir),
            )));
        }

        // Try to parse the folder name
        let folder_name = self
            .root_dir
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                ScanError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Invalid folder name: {:?}", self.root_dir),
                ))
            })?;

        let (title, year) = Self::parse_folder_name(folder_name).ok_or_else(|| {
            ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "Folder name does not match expected format 'Title (Year)': {}",
                    folder_name
                ),
            ))
        })?;

        // Check for done marker
        let has_done_marker = Self::check_done_marker(&self.root_dir);

        // Skip if done marker exists and force flag is not set
        if has_done_marker && !self.force {
            log::info!(
                "Skipping {} (done marker found, use --force to reprocess)",
                folder_name
            );
            return Ok(Vec::new());
        }

        // Return single movie entry
        Ok(vec![MovieEntry {
            path: self.root_dir.clone(),
            title,
            year,
            has_done_marker,
        }])
    }

    /// Recursively scan a directory for movie folders
    fn scan_directory(&self, dir: &Path, movies: &mut Vec<MovieEntry>) -> Result<(), ScanError> {
        // Check if directory exists and is readable
        if !dir.exists() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory not found: {:?}", dir),
            )));
        }

        if !dir.is_dir() {
            return Err(ScanError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Path is not a directory: {:?}", dir),
            )));
        }

        // Read directory entries
        let entries = fs::read_dir(dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Only process directories
            if !path.is_dir() {
                continue;
            }

            // Try to parse folder name
            if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some((title, year)) = Self::parse_folder_name(folder_name) {
                    // This looks like a movie folder
                    let has_done_marker = Self::check_done_marker(&path);

                    // Skip if done marker exists and force flag is not set
                    if has_done_marker && !self.force {
                        log::debug!("Skipping {} (done marker found)", folder_name);
                        continue;
                    }

                    // Add to processing queue
                    movies.push(MovieEntry {
                        path: path.clone(),
                        title,
                        year,
                        has_done_marker,
                    });
                } else {
                    // Not a movie folder, recurse into it
                    self.scan_directory(&path, movies)?;
                }
            }
        }

        Ok(())
    }

    /// Parse a folder name to extract title and year
    /// Expected format: "Movie Title (Year)"
    /// Returns Some((title, year)) if valid, None otherwise
    pub fn parse_folder_name(name: &str) -> Option<(String, u16)> {
        let caps = FOLDER_NAME_RE.captures(name)?;

        let title = caps.get(1)?.as_str().trim().to_string();
        let year_str = caps.get(2)?.as_str();
        let year = year_str.parse::<u16>().ok()?;

        // Validate that title is not empty
        if title.is_empty() {
            return None;
        }

        Some((title, year))
    }

    /// Parse a series folder name to extract title and optional year
    /// Supports formats:
    /// - "{Series Name} (YYYY)" - with year
    /// - "{Series Name}" - without year
    ///
    /// Returns Some((title, year)) where year is None if not present
    pub fn parse_series_folder_name(name: &str) -> Option<(String, Option<u16>)> {
        // Try with year first: "Series Name (YYYY)"
        if let Some(caps) = FOLDER_NAME_RE.captures(name) {
            let title = caps.get(1)?.as_str().trim().to_string();
            let year = caps.get(2)?.as_str().parse::<u16>().ok()?;

            // Validate that title is not empty
            if !title.is_empty() {
                return Some((title, Some(year)));
            }
        }

        // Try without year: just the series name
        let trimmed = name.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('.') && trimmed != "." {
            return Some((trimmed.to_string(), None));
        }

        None
    }

    /// Check if a done marker file exists in the given directory
    fn check_done_marker(path: &Path) -> bool {
        let marker_path = path.join(DONE_MARKER_FILENAME);

        if !marker_path.exists() {
            return false;
        }

        // Try to read and parse the done marker to ensure it's valid
        match fs::read_to_string(&marker_path) {
            Ok(content) => {
                // Try to parse as JSON to validate format
                serde_json::from_str::<DoneMarker>(&content).is_ok()
            }
            Err(_) => false,
        }
    }

    /// Detect whether a folder contains a movie or a TV series
    /// Returns MediaType::Series if season folders are detected
    /// Returns MediaType::Movie if video files are found directly
    /// Returns MediaType::Unknown if neither condition is met
    pub fn detect_media_type(path: &Path) -> MediaType {
        // Check for season folders first (takes precedence)
        if Self::has_season_folders(path) {
            return MediaType::Series;
        }

        // Check for video files directly in folder
        if Self::has_video_files(path) {
            return MediaType::Movie;
        }

        MediaType::Unknown
    }

    /// Check if directory contains season folders (Season 1, Season 01, Season 001, etc.)
    /// Uses regex pattern to match "Season X+" format (one or more digits)
    fn has_season_folders(path: &Path) -> bool {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().is_dir()
                    && let Some(name) = entry.file_name().to_str()
                    && SEASON_FOLDER_RE.is_match(name)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check if directory contains video files directly
    /// Looks for common video file extensions
    fn has_video_files(path: &Path) -> bool {
        let video_extensions = [
            "mp4", "mkv", "avi", "mov", "flv", "wmv", "webm", "m4v", "mpg", "mpeg",
        ];

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_file()
                    && let Some(ext) = entry_path.extension().and_then(|e| e.to_str())
                    && video_extensions.contains(&ext.to_lowercase().as_str())
                {
                    return true;
                }
            }
        }
        false
    }

    /// Detect all season folders in a series directory
    /// Returns a sorted vector of season numbers found
    /// Matches "Season 1", "Season 01", "Season 001", etc.
    fn detect_season_folders(path: &Path) -> Vec<u8> {
        let mut seasons = Vec::new();

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if entry.path().is_dir()
                    && let Some(name) = entry.file_name().to_str()
                    && let Some(caps) = SEASON_NUMBER_RE.captures(name)
                    && let Some(season_str) = caps.get(1)
                    && let Ok(season_num) = season_str.as_str().parse::<u8>()
                {
                    seasons.push(season_num);
                }
            }
        }

        seasons.sort_unstable();
        seasons
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_folder_name_valid() {
        assert_eq!(
            Scanner::parse_folder_name("The Matrix (1999)"),
            Some(("The Matrix".to_string(), 1999))
        );

        assert_eq!(
            Scanner::parse_folder_name("Inception (2010)"),
            Some(("Inception".to_string(), 2010))
        );

        // With extra spaces
        assert_eq!(
            Scanner::parse_folder_name("The Dark Knight  (2008)"),
            Some(("The Dark Knight".to_string(), 2008))
        );
    }

    #[test]
    fn test_parse_folder_name_invalid() {
        // No year
        assert_eq!(Scanner::parse_folder_name("No Year Here"), None);

        // Only year
        assert_eq!(Scanner::parse_folder_name("(2020)"), None);

        // Invalid year format
        assert_eq!(Scanner::parse_folder_name("Movie (abcd)"), None);

        // Year with wrong number of digits
        assert_eq!(Scanner::parse_folder_name("Movie (20)"), None);
        assert_eq!(Scanner::parse_folder_name("Movie (20200)"), None);

        // Empty string
        assert_eq!(Scanner::parse_folder_name(""), None);
    }

    #[test]
    fn test_parse_folder_name_edge_cases() {
        // Title with parentheses
        assert_eq!(
            Scanner::parse_folder_name("Movie (Part 1) (2020)"),
            Some(("Movie (Part 1)".to_string(), 2020))
        );

        // Title with numbers
        assert_eq!(
            Scanner::parse_folder_name("2001: A Space Odyssey (1968)"),
            Some(("2001: A Space Odyssey".to_string(), 1968))
        );

        // Title with special characters
        assert_eq!(
            Scanner::parse_folder_name("The Lord of the Rings: The Fellowship of the Ring (2001)"),
            Some((
                "The Lord of the Rings: The Fellowship of the Ring".to_string(),
                2001
            ))
        );
    }

    #[test]
    fn test_scan_empty_directory() {
        use tempfile::TempDir;

        // Create an empty temporary directory
        let temp_dir = TempDir::new().unwrap();

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
        let movies = scanner.scan().unwrap();

        // Should return empty list
        assert_eq!(movies.len(), 0);
    }

    #[test]
    fn test_scan_nested_directory_structure() {
        use std::fs;
        use tempfile::TempDir;

        // Create nested directory structure
        let temp_dir = TempDir::new().unwrap();

        // Create movies at different nesting levels
        let movie1_path = temp_dir.path().join("The Matrix (1999)");
        fs::create_dir(&movie1_path).unwrap();

        let subdir = temp_dir.path().join("Action");
        fs::create_dir(&subdir).unwrap();

        let movie2_path = subdir.join("Inception (2010)");
        fs::create_dir(&movie2_path).unwrap();

        let subsubdir = subdir.join("SciFi");
        fs::create_dir(&subsubdir).unwrap();

        let movie3_path = subsubdir.join("Interstellar (2014)");
        fs::create_dir(&movie3_path).unwrap();

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
        let movies = scanner.scan().unwrap();

        // Should find all 3 movies regardless of nesting
        assert_eq!(movies.len(), 3);

        let titles: Vec<&str> = movies.iter().map(|m| m.title.as_str()).collect();
        assert!(titles.contains(&"The Matrix"));
        assert!(titles.contains(&"Inception"));
        assert!(titles.contains(&"Interstellar"));
    }

    #[test]
    fn test_scan_invalid_folder_names() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create directories with invalid movie folder names
        fs::create_dir(temp_dir.path().join("Not a Movie")).unwrap();
        fs::create_dir(temp_dir.path().join("(2020)")).unwrap();
        fs::create_dir(temp_dir.path().join("Movie (abcd)")).unwrap();

        // Create one valid movie folder
        fs::create_dir(temp_dir.path().join("Valid Movie (2020)")).unwrap();

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
        let movies = scanner.scan().unwrap();

        // Should only find the valid movie
        assert_eq!(movies.len(), 1);
        assert_eq!(movies[0].title, "Valid Movie");
        assert_eq!(movies[0].year, 2020);
    }

    #[test]
    fn test_scan_with_invalid_done_marker() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let movie_path = temp_dir.path().join("Test Movie (2020)");
        fs::create_dir(&movie_path).unwrap();

        // Create an invalid done marker (not valid JSON)
        let marker_path = movie_path.join("done.ext");
        fs::write(&marker_path, "invalid json content").unwrap();

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
        let movies = scanner.scan().unwrap();

        // Should include the movie since done marker is invalid
        assert_eq!(movies.len(), 1);
        assert_eq!(movies[0].title, "Test Movie");
        assert!(!movies[0].has_done_marker);
    }

    #[test]
    fn test_scan_nonexistent_directory() {
        use std::path::PathBuf;

        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let scanner = Scanner::new(nonexistent, false, false);

        let result = scanner.scan();

        // Should return an error
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_file_instead_of_directory() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("not_a_directory.txt");
        fs::write(&file_path, "test content").unwrap();

        let scanner = Scanner::new(file_path, false, false);
        let result = scanner.scan();

        // Should return an error
        assert!(result.is_err());
    }

    #[test]
    fn test_check_done_marker_with_valid_marker() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create a valid done marker
        let done_marker = DoneMarker {
            finished_at: "2024-01-15T10:30:00Z".to_string(),
            version: "0.1.0".to_string(),
        };
        let marker_json = serde_json::to_string(&done_marker).unwrap();
        let marker_path = temp_dir.path().join("done.ext");
        fs::write(&marker_path, marker_json).unwrap();

        // Check if done marker exists
        assert!(Scanner::check_done_marker(temp_dir.path()));
    }

    #[test]
    fn test_check_done_marker_without_marker() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // No done marker file
        assert!(!Scanner::check_done_marker(temp_dir.path()));
    }

    #[test]
    fn test_detect_season_folders_varied_formats() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let series_path = temp_dir.path().join("Test Series (2020)");
        fs::create_dir(&series_path).unwrap();

        // Create season folders with different naming formats
        fs::create_dir(series_path.join("Season 1")).unwrap(); // Single digit
        fs::create_dir(series_path.join("Season 02")).unwrap(); // Two digits with leading zero
        fs::create_dir(series_path.join("Season 003")).unwrap(); // Three digits with leading zeros
        fs::create_dir(series_path.join("Season 10")).unwrap(); // Two digits, no leading zero
        fs::create_dir(series_path.join("Season 99")).unwrap(); // Max two-digit season

        // Create some non-season folders that should be ignored
        fs::create_dir(series_path.join("Extras")).unwrap();
        fs::create_dir(series_path.join("Behind the Scenes")).unwrap();

        let seasons = Scanner::detect_season_folders(&series_path);

        // Should detect all 5 season folders
        assert_eq!(seasons.len(), 5);
        assert_eq!(seasons, vec![1, 2, 3, 10, 99]);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: tv-series-extras, Property 1: Series Folder Name Parsing
    // Validates: Requirements 1.1, 1.2
    // For any folder name matching the patterns "{Series Name} (YYYY)" or "{Series Name}",
    // parsing should correctly extract the series title and optional year.
    proptest! {
        #[test]
        fn prop_series_folder_name_parsing(
            title in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            year in proptest::option::of(1900u16..2100u16)
        ) {
            let title_trimmed = title.trim();

            // Skip empty titles, titles that are just dots, or titles that start with a dot
            if title_trimmed.is_empty() || title_trimmed.starts_with('.') {
                return Ok(());
            }

            // Construct folder name based on whether year is present
            let folder_name = if let Some(y) = year {
                format!("{} ({})", title_trimmed, y)
            } else {
                title_trimmed.to_string()
            };

            // Parse the folder name
            let parsed = Scanner::parse_series_folder_name(&folder_name);

            // Should successfully parse
            prop_assert!(parsed.is_some(), "Failed to parse valid series folder name: {}", folder_name);

            let (parsed_title, parsed_year) = parsed.unwrap();

            // Extracted title should match
            prop_assert_eq!(&parsed_title, title_trimmed, "Title mismatch for folder: {}", folder_name);

            // Extracted year should match
            prop_assert_eq!(parsed_year, year, "Year mismatch for folder: {}", folder_name);
        }
    }

    // Feature: tv-series-extras, Property 2: Series Done Marker Skipping
    // Validates: Requirements 1.3, 9.1, 9.3, 9.4
    // For any series folder containing a valid done marker file, when the force flag is not set,
    // the series should be excluded from the processing queue; when the force flag is set,
    // the series should be included.
    proptest! {
        #[test]
        fn prop_series_done_marker_skipping(
            title in "[a-zA-Z0-9 ]{1,50}",
            year in proptest::option::of(1900u16..2100u16),
            force_flag in proptest::bool::ANY
        ) {
            use std::fs;
            use tempfile::TempDir;

            let title_trimmed = title.trim();
            if title_trimmed.is_empty() {
                return Ok(());
            }

            // Create a temporary directory structure
            let temp_root = TempDir::new().unwrap();
            let series_folder_name = if let Some(y) = year {
                format!("{} ({})", title_trimmed, y)
            } else {
                title_trimmed.to_string()
            };
            let series_path = temp_root.path().join(&series_folder_name);
            fs::create_dir(&series_path).unwrap();

            // Create a Season 01 folder to make it a series
            fs::create_dir(series_path.join("Season 01")).unwrap();

            // Create a valid done marker
            let done_marker = DoneMarker {
                finished_at: "2024-01-15T10:30:00Z".to_string(),
                version: "0.1.0".to_string(),
            };
            let marker_json = serde_json::to_string(&done_marker).unwrap();
            let marker_path = series_path.join("done.ext");
            fs::write(&marker_path, marker_json).unwrap();

            // Create scanner with the force flag
            let scanner = Scanner::new(temp_root.path().to_path_buf(), force_flag, false);

            // Scan the directory
            let (_movies, series) = scanner.scan_all().unwrap();

            if force_flag {
                // With force flag, the series should be included even with done marker
                prop_assert_eq!(
                    series.len(),
                    1,
                    "With force flag, series should be included despite done marker"
                );
                prop_assert_eq!(&series[0].title, title_trimmed);
                prop_assert_eq!(series[0].year, year);
                prop_assert!(series[0].has_done_marker, "has_done_marker should be true");
            } else {
                // Without force flag, the series should be skipped
                prop_assert_eq!(
                    series.len(),
                    0,
                    "Without force flag, series with done marker should be skipped"
                );
            }
        }
    }

    // Feature: tv-series-extras, Property 11: Media Type Detection Consistency
    // Validates: Requirements 10.1, 10.2, 10.3
    // For any directory, if it contains season folders (Season 01, Season 02, etc.),
    // it should be classified as a Series; if it contains video files directly,
    // it should be classified as a Movie; the classification should be deterministic
    // and consistent across multiple scans.
    proptest! {
        #[test]
        fn prop_media_type_detection_consistency(
            has_seasons in proptest::bool::ANY,
            has_videos in proptest::bool::ANY
        ) {
            use std::fs;
            use tempfile::TempDir;

            let temp_dir = TempDir::new().unwrap();

            // Create season folders if needed
            if has_seasons {
                fs::create_dir(temp_dir.path().join("Season 01")).unwrap();
            }

            // Create video files if needed
            if has_videos && !has_seasons {
                fs::write(temp_dir.path().join("video.mp4"), "fake video").unwrap();
            }

            // Detect media type multiple times
            let type1 = Scanner::detect_media_type(temp_dir.path());
            let type2 = Scanner::detect_media_type(temp_dir.path());
            let type3 = Scanner::detect_media_type(temp_dir.path());

            // All detections should be consistent
            prop_assert_eq!(type1, type2, "Media type detection should be consistent");
            prop_assert_eq!(type2, type3, "Media type detection should be consistent");

            // Verify correct classification
            if has_seasons {
                prop_assert_eq!(type1, MediaType::Series, "Should detect as Series when season folders exist");
            } else if has_videos {
                prop_assert_eq!(type1, MediaType::Movie, "Should detect as Movie when video files exist");
            } else {
                prop_assert_eq!(type1, MediaType::Unknown, "Should detect as Unknown when neither condition is met");
            }
        }
    }

    // Feature: extras-fetcher, Property 1: Folder Name Parsing Correctness
    // Validates: Requirements 1.7
    // For any folder name matching the pattern "Title (Year)", parsing should extract
    // the title and year correctly, and the extracted values should reconstruct a valid folder name.
    proptest! {
        #[test]
        fn prop_folder_name_parsing_correctness(
            title in "[a-zA-Z0-9 :',&!?.-]{1,100}",
            year in 1900u16..2100u16
        ) {
            // Trim the title to avoid leading/trailing spaces
            let title_trimmed = title.trim();

            // Skip empty titles (invalid case)
            if title_trimmed.is_empty() {
                return Ok(());
            }

            // Construct a folder name in the expected format
            let folder_name = format!("{} ({})", title_trimmed, year);

            // Parse the folder name
            let parsed = Scanner::parse_folder_name(&folder_name);

            // Should successfully parse
            prop_assert!(parsed.is_some(), "Failed to parse valid folder name: {}", folder_name);

            let (parsed_title, parsed_year) = parsed.unwrap();

            // Extracted title should match (after trimming)
            prop_assert_eq!(
                &parsed_title,
                title_trimmed,
                "Title mismatch for folder: {}",
                folder_name
            );

            // Extracted year should match exactly
            prop_assert_eq!(
                parsed_year,
                year,
                "Year mismatch for folder: {}",
                folder_name
            );

            // Round-trip: reconstructing the folder name should produce a parseable result
            let reconstructed = format!("{} ({})", parsed_title, parsed_year);
            let reparsed = Scanner::parse_folder_name(&reconstructed);
            prop_assert!(
                reparsed.is_some(),
                "Reconstructed folder name failed to parse: {}",
                reconstructed
            );
        }
    }

    // Feature: extras-fetcher, Property 3: Done Marker Skipping Behavior
    // Validates: Requirements 1.8, 2.3, 12.1
    // For any movie folder containing a valid done marker file (when --force is not set),
    // the folder should be excluded from the processing queue.
    proptest! {
        #[test]
        fn prop_done_marker_skipping_behavior(
            title in "[a-zA-Z0-9 ]{1,50}",
            year in 1900u16..2100u16,
            force_flag in proptest::bool::ANY
        ) {
            use std::fs;
            use tempfile::TempDir;

            let title_trimmed = title.trim();
            if title_trimmed.is_empty() {
                return Ok(());
            }

            // Create a temporary directory structure
            let temp_root = TempDir::new().unwrap();
            let movie_folder_name = format!("{} ({})", title_trimmed, year);
            let movie_path = temp_root.path().join(&movie_folder_name);
            fs::create_dir(&movie_path).unwrap();

            // Create a valid done marker
            let done_marker = DoneMarker {
                finished_at: "2024-01-15T10:30:00Z".to_string(),
                version: "0.1.0".to_string(),
            };
            let marker_json = serde_json::to_string(&done_marker).unwrap();
            let marker_path = movie_path.join("done.ext");
            fs::write(&marker_path, marker_json).unwrap();

            // Create scanner with the force flag
            let scanner = Scanner::new(temp_root.path().to_path_buf(), force_flag, false);

            // Scan the directory
            let movies = scanner.scan().unwrap();

            if force_flag {
                // With force flag, the movie should be included even with done marker
                prop_assert_eq!(
                    movies.len(),
                    1,
                    "With force flag, movie should be included despite done marker"
                );
                prop_assert_eq!(&movies[0].title, title_trimmed);
                prop_assert_eq!(movies[0].year, year);
                prop_assert!(movies[0].has_done_marker, "has_done_marker should be true");
            } else {
                // Without force flag, the movie should be skipped
                prop_assert_eq!(
                    movies.len(),
                    0,
                    "Without force flag, movie with done marker should be skipped"
                );
            }
        }
    }

    // Feature: extras-fetcher, Property 6: Recursive Directory Traversal Completeness
    // Validates: Requirements 1.6
    // For any directory tree structure, scanning should visit every subdirectory at least once
    // and discover all movie folders regardless of nesting depth.
    proptest! {
        #[test]
        fn prop_recursive_traversal_completeness(
            depth in 1usize..=4,  // Test nesting depths from 1 to 4
            movies_per_level in 1usize..=3,  // 1-3 movies per level
        ) {
            use std::fs;
            use tempfile::TempDir;

            // Create a temporary directory structure
            let temp_root = TempDir::new().unwrap();
            let mut expected_movies = Vec::new();

            // Create nested directory structure with movies at various levels
            let mut current_path = temp_root.path().to_path_buf();

            for level in 0..depth {
                // Create movies at this level
                for i in 0..movies_per_level {
                    let title = format!("Movie L{} N{}", level, i);
                    let year = 2000 + (level * 10 + i) as u16;
                    let movie_folder = format!("{} ({})", title, year);
                    let movie_path = current_path.join(&movie_folder);
                    fs::create_dir(&movie_path).unwrap();

                    expected_movies.push((title.clone(), year));
                }

                // Create a subdirectory for the next level
                if level < depth - 1 {
                    let subdir = format!("Level{}", level + 1);
                    current_path = current_path.join(subdir);
                    fs::create_dir(&current_path).unwrap();
                }
            }

            // Scan the directory
            let scanner = Scanner::new(temp_root.path().to_path_buf(), false, false);
            let movies = scanner.scan().unwrap();

            // All movies should be discovered
            prop_assert_eq!(
                movies.len(),
                expected_movies.len(),
                "Scanner should discover all movies at all nesting levels"
            );

            // Verify each expected movie was found
            for (expected_title, expected_year) in &expected_movies {
                let found = movies.iter().any(|m| {
                    &m.title == expected_title && m.year == *expected_year
                });
                prop_assert!(
                    found,
                    "Movie '{}' ({}) should be discovered",
                    expected_title,
                    expected_year
                );
            }
        }
    }

    // Feature: tv-series-extras, Property 19: Backward Compatibility Preservation
    // Validates: Requirements 20.1, 20.2, 20.5
    // For any library containing only movies (no series folders), processing should produce
    // identical results to the previous version, including the same done marker format,
    // directory structure, and file organization.
    proptest! {
        #[test]
        fn prop_backward_compatibility_movies_only(
            num_movies in 1usize..5usize,
        ) {
            use tempfile::TempDir;

            let temp_root = TempDir::new().unwrap();

            // Create only movie folders (no series)
            for i in 0..num_movies {
                let title = format!("Movie {}", i);
                let year = 2000 + i as u16;
                let movie_folder = format!("{} ({})", title, year);
                let movie_path = temp_root.path().join(&movie_folder);
                fs::create_dir(&movie_path).unwrap();

                // Create a dummy movie file
                fs::write(
                    movie_path.join(format!("{}.mp4", movie_folder)),
                    b"dummy content",
                ).unwrap();
            }

            // Scan the directory
            let scanner = Scanner::new(temp_root.path().to_path_buf(), false, false);
            let (movies, series) = scanner.scan_all().unwrap();

            // Should find only movies, no series
            prop_assert_eq!(
                movies.len(),
                num_movies,
                "Should find all {} movies",
                num_movies
            );
            prop_assert_eq!(
                series.len(),
                0,
                "Should find no series in movie-only library"
            );

            // Verify all movies have valid structure
            for movie in &movies {
                prop_assert!(!movie.title.is_empty(), "Movie title should not be empty");
                prop_assert!(movie.path.exists(), "Movie path should exist");
            }
        }
    }

    // Additional backward compatibility test: done marker format preservation
    proptest! {
        #[test]
        fn prop_backward_compatibility_done_marker_format(
            _dummy in 0u8..1u8,
        ) {
            use tempfile::TempDir;

            let temp_root = TempDir::new().unwrap();
            let movie_dir = temp_root.path().join("Test Movie (2020)");
            fs::create_dir(&movie_dir).unwrap();

            // Create a dummy movie file to make it a movie folder
            fs::write(movie_dir.join("movie.mp4"), b"dummy").unwrap();

            // Create done marker in the expected format
            let done_marker = serde_json::json!({
                "finished_at": "2024-01-15T10:30:00Z",
                "version": "0.1.0"
            });
            fs::write(
                movie_dir.join("done.ext"),
                serde_json::to_string_pretty(&done_marker).unwrap(),
            ).unwrap();

            // Scan should recognize the done marker
            let scanner = Scanner::new(temp_root.path().to_path_buf(), false, false);
            let (movies, _) = scanner.scan_all().unwrap();

            // Movie should be skipped due to done marker
            prop_assert_eq!(
                movies.len(),
                0,
                "Movie with done marker should be skipped"
            );

            // With force flag, should be included
            let scanner_force = Scanner::new(temp_root.path().to_path_buf(), true, false);
            let (movies_force, _) = scanner_force.scan_all().unwrap();

            prop_assert_eq!(
                movies_force.len(),
                1,
                "Movie with done marker should be included with force flag"
            );
        }
    }
}
