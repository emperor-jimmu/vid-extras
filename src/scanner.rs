// Scanner module - handles directory traversal and movie discovery

use crate::error::ScanError;
use crate::models::{DoneMarker, MovieEntry};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
const DONE_MARKER_FILENAME: &str = "done.ext";

/// Scanner for traversing movie library directories
pub struct Scanner {
    root_dir: PathBuf,
    force: bool,
}

impl Scanner {
    /// Create a new Scanner instance
    pub fn new(root_dir: PathBuf, force: bool) -> Self {
        Self { root_dir, force }
    }

    /// Scan the root directory and return a list of movies to process
    pub fn scan(&self) -> Result<Vec<MovieEntry>, ScanError> {
        let mut movies = Vec::new();
        self.scan_directory(&self.root_dir, &mut movies)?;
        Ok(movies)
    }

    /// Recursively scan a directory for movie folders
    #[cfg_attr(not(test), allow(dead_code))]
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
        // Regex pattern: ^(.+?)\s*\((\d{4})\)$
        // Captures: title (non-greedy) and 4-digit year
        let re = Regex::new(r"^(.+?)\s*\((\d{4})\)$").ok()?;

        let caps = re.captures(name)?;

        let title = caps.get(1)?.as_str().trim().to_string();
        let year_str = caps.get(2)?.as_str();
        let year = year_str.parse::<u16>().ok()?;

        // Validate that title is not empty
        if title.is_empty() {
            return None;
        }

        Some((title, year))
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

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false);
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

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false);
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

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false);
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

        let scanner = Scanner::new(temp_dir.path().to_path_buf(), false);
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
        let scanner = Scanner::new(nonexistent, false);

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

        let scanner = Scanner::new(file_path, false);
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
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

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
            let scanner = Scanner::new(temp_root.path().to_path_buf(), force_flag);

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
            let scanner = Scanner::new(temp_root.path().to_path_buf(), false);
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
}
