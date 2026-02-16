use log::{debug, warn};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Handles scanning and importing of local Season 0 files
pub struct Season0Importer;

impl Season0Importer {
    /// Scan series folder for S00Exx files
    pub async fn scan_for_season_zero_files(
        series_path: &Path,
    ) -> Result<Vec<PathBuf>, String> {
        let mut season_zero_files = Vec::new();
        let season_zero_regex = Regex::new(r"(?i)S00E\d{1,2}").map_err(|e| e.to_string())?;

        let mut entries = fs::read_dir(series_path)
            .await
            .map_err(|e| format!("Failed to read series directory: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read directory entry: {}", e))?
        {
            let path = entry.path();

            // Skip Season 00 folder itself
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str == "Season 00" {
                        continue;
                    }
                }
            }

            // Check if it's a video file matching S00Exx pattern
            if Self::is_video_file(&path) {
                if let Some(filename) = path.file_name() {
                    if let Some(filename_str) = filename.to_str() {
                        if season_zero_regex.is_match(filename_str) {
                            debug!("Found Season 0 file: {}", filename_str);
                            season_zero_files.push(path);
                        }
                    }
                }
            }
        }

        Ok(season_zero_files)
    }

    /// Extract episode number from S00Exx pattern
    pub fn extract_episode_number(filename: &str) -> Option<u8> {
        let regex = Regex::new(r"(?i)S00E(\d{1,2})").ok()?;
        regex
            .captures(filename)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<u8>().ok())
    }

    /// Sanitize filename by removing invalid characters
    pub fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect()
    }

    /// Generate standard Season 0 filename format
    pub fn generate_season_zero_filename(
        series_name: &str,
        episode_number: u8,
        episode_title: &str,
    ) -> String {
        let sanitized_title = Self::sanitize_filename(episode_title);
        format!(
            "{} - S00E{:02} - {}.mp4",
            series_name, episode_number, sanitized_title
        )
    }

    /// Move Season 0 file to Season 00 folder with correct naming
    pub async fn import_season_zero_file(
        file_path: &Path,
        series_path: &Path,
        series_name: &str,
    ) -> Result<(), String> {
        // Extract episode number
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid filename")?;

        let episode_number = Self::extract_episode_number(filename)
            .ok_or("Could not extract episode number")?;

        // Create Season 00 folder if it doesn't exist
        let season_00_dir = series_path.join("Season 00");
        fs::create_dir_all(&season_00_dir)
            .await
            .map_err(|e| format!("Failed to create Season 00 directory: {}", e))?;

        // Generate new filename (use original filename without extension for title)
        let original_stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Special");

        let new_filename = Self::generate_season_zero_filename(
            series_name,
            episode_number,
            original_stem,
        );

        let target_path = season_00_dir.join(&new_filename);

        // Check for duplicates
        if target_path.exists() {
            warn!(
                "Season 0 file already exists: {}. Skipping import.",
                new_filename
            );
            return Err(format!("Duplicate episode number: {}", episode_number));
        }

        // Move file to Season 00 folder
        fs::rename(file_path, &target_path)
            .await
            .map_err(|e| format!("Failed to move file: {}", e))?;

        debug!(
            "Imported Season 0 file: {} -> {}",
            filename, new_filename
        );

        Ok(())
    }

    /// Import all Season 0 files found in series folder
    pub async fn import_all_season_zero_files(
        series_path: &Path,
        series_name: &str,
    ) -> Result<(usize, usize), String> {
        let files = Self::scan_for_season_zero_files(series_path).await?;

        let mut imported = 0;
        let mut skipped = 0;
        let mut seen_episodes = HashSet::new();

        for file_path in files {
            if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
                if let Some(episode_num) = Self::extract_episode_number(filename) {
                    if seen_episodes.contains(&episode_num) {
                        warn!(
                            "Duplicate Season 0 episode number: {}. Skipping.",
                            episode_num
                        );
                        skipped += 1;
                        continue;
                    }

                    seen_episodes.insert(episode_num);

                    match Self::import_season_zero_file(&file_path, series_path, series_name)
                        .await
                    {
                        Ok(()) => imported += 1,
                        Err(e) => {
                            warn!("Failed to import Season 0 file: {}", e);
                            skipped += 1;
                        }
                    }
                }
            }
        }

        Ok((imported, skipped))
    }

    /// Check if file is a video file
    fn is_video_file(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                matches!(
                    ext_str.to_lowercase().as_str(),
                    "mp4" | "mkv" | "avi" | "mov" | "flv" | "wmv" | "webm" | "m4v"
                )
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_extract_episode_number_standard() {
        let num = Season0Importer::extract_episode_number("S00E01.mp4");
        assert_eq!(num, Some(1));
    }

    #[test]
    fn test_extract_episode_number_double_digit() {
        let num = Season0Importer::extract_episode_number("S00E12.mp4");
        assert_eq!(num, Some(12));
    }

    #[test]
    fn test_extract_episode_number_case_insensitive() {
        let num = Season0Importer::extract_episode_number("s00e05.mp4");
        assert_eq!(num, Some(5));
    }

    #[test]
    fn test_extract_episode_number_with_title() {
        let num = Season0Importer::extract_episode_number("S00E03 - Special Title.mp4");
        assert_eq!(num, Some(3));
    }

    #[test]
    fn test_extract_episode_number_invalid() {
        let num = Season0Importer::extract_episode_number("S01E01.mp4");
        assert_eq!(num, None);
    }

    #[test]
    fn test_sanitize_filename() {
        let sanitized = Season0Importer::sanitize_filename("Title: Part 1 (Special)");
        assert_eq!(sanitized, "Title_ Part 1 (Special)");
    }

    #[test]
    fn test_sanitize_filename_with_slashes() {
        let sanitized = Season0Importer::sanitize_filename("Path/To/File");
        assert_eq!(sanitized, "Path_To_File");
    }

    #[test]
    fn test_generate_season_zero_filename() {
        let filename = Season0Importer::generate_season_zero_filename(
            "Breaking Bad",
            5,
            "Pilot",
        );
        assert_eq!(filename, "Breaking Bad - S00E05 - Pilot.mp4");
    }

    #[test]
    fn test_generate_season_zero_filename_single_digit() {
        let filename = Season0Importer::generate_season_zero_filename(
            "The Office",
            1,
            "Unaired Pilot",
        );
        assert_eq!(filename, "The Office - S00E01 - Unaired Pilot.mp4");
    }

    #[test]
    fn test_generate_season_zero_filename_sanitizes_title() {
        let filename = Season0Importer::generate_season_zero_filename(
            "Game of Thrones",
            2,
            "Special: Behind the Scenes",
        );
        assert_eq!(
            filename,
            "Game of Thrones - S00E02 - Special_ Behind the Scenes.mp4"
        );
    }

    // Property 16: Local Season 0 Import
    // Validates: Requirements 16.1, 16.2, 16.3
    proptest! {
        #[test]
        fn prop_season_zero_episode_extraction(
            episode_num in 1u8..=99u8
        ) {
            let filename = format!("S00E{:02}.mp4", episode_num);
            let extracted = Season0Importer::extract_episode_number(&filename);
            prop_assert_eq!(extracted, Some(episode_num));
        }

        #[test]
        fn prop_season_zero_filename_generation(
            series_name in "[a-zA-Z0-9 ]{1,50}",
            episode_num in 1u8..=99u8,
            title in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let filename = Season0Importer::generate_season_zero_filename(
                &series_name,
                episode_num,
                &title,
            );

            let expected_episode = format!("S00E{:02}", episode_num);

            // Verify format
            prop_assert!(filename.contains(&expected_episode));
            prop_assert!(filename.ends_with(".mp4"));
            prop_assert!(filename.contains(&series_name));

            // Verify episode number can be extracted
            let extracted = Season0Importer::extract_episode_number(&filename);
            prop_assert_eq!(extracted, Some(episode_num));
        }

        #[test]
        fn prop_sanitize_removes_invalid_chars(
            filename in r"[a-zA-Z0-9 ]{1,50}[/:*?<>|][a-zA-Z0-9 ]{1,50}"
        ) {
            let sanitized = Season0Importer::sanitize_filename(&filename);

            // Verify invalid characters are removed
            prop_assert!(!sanitized.contains('/'));
            prop_assert!(!sanitized.contains('\\'));
            prop_assert!(!sanitized.contains(':'));
            prop_assert!(!sanitized.contains('*'));
            prop_assert!(!sanitized.contains('?'));
            prop_assert!(!sanitized.contains('<'));
            prop_assert!(!sanitized.contains('>'));
            prop_assert!(!sanitized.contains('|'));
        }

        #[test]
        fn prop_case_insensitive_episode_extraction(
            episode_num in 1u8..=99u8
        ) {
            let lower = format!("s00e{:02}.mp4", episode_num);
            let upper = format!("S00E{:02}.mp4", episode_num);
            let mixed = format!("S00e{:02}.mp4", episode_num);

            let lower_extracted = Season0Importer::extract_episode_number(&lower);
            let upper_extracted = Season0Importer::extract_episode_number(&upper);
            let mixed_extracted = Season0Importer::extract_episode_number(&mixed);

            prop_assert_eq!(lower_extracted, Some(episode_num));
            prop_assert_eq!(upper_extracted, Some(episode_num));
            prop_assert_eq!(mixed_extracted, Some(episode_num));
        }
    }
}
