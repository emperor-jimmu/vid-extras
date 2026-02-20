use crate::models::ContentCategory;
use log::debug;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Handles extraction and processing of season pack archives
pub struct SeasonPackProcessor;

impl SeasonPackProcessor {
    /// Check if a file is an archive (zip, rar, 7z, tar.gz)
    pub fn is_archive(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                matches!(
                    ext_str.to_lowercase().as_str(),
                    "zip" | "rar" | "7z" | "tar" | "gz" | "tgz"
                )
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Extract archive to temporary directory
    /// Returns path to extraction directory if successful
    pub async fn extract_archive(archive_path: &Path, temp_dir: &Path) -> Result<PathBuf, String> {
        // Create extraction directory
        let extract_dir = temp_dir.join("extracted");
        fs::create_dir_all(&extract_dir)
            .await
            .map_err(|e| format!("Failed to create extraction directory: {}", e))?;

        // Determine archive type and extract
        if let Some(ext) = archive_path.extension()
            && let Some(ext_str) = ext.to_str()
        {
            let ext_lower = ext_str.to_lowercase();
            match ext_lower.as_str() {
                "zip" => Self::extract_zip(archive_path, &extract_dir).await?,
                "7z" => Self::extract_7z(archive_path, &extract_dir).await?,
                "rar" => Self::extract_rar(archive_path, &extract_dir).await?,
                "tar" | "gz" | "tgz" => Self::extract_tar(archive_path, &extract_dir).await?,
                _ => return Err(format!("Unsupported archive format: {}", ext_str)),
            }
        }

        Ok(extract_dir)
    }

    /// Extract zip archive using system unzip command
    async fn extract_zip(archive_path: &Path, extract_dir: &Path) -> Result<(), String> {
        let output = tokio::process::Command::new("unzip")
            .arg("-q")
            .arg(archive_path)
            .arg("-d")
            .arg(extract_dir)
            .output()
            .await
            .map_err(|e| format!("Failed to execute unzip: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "unzip failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Extract 7z archive using system 7z command
    async fn extract_7z(archive_path: &Path, extract_dir: &Path) -> Result<(), String> {
        let output = tokio::process::Command::new("7z")
            .arg("x")
            .arg(archive_path)
            .arg(format!("-o{}", extract_dir.display()))
            .output()
            .await
            .map_err(|e| format!("Failed to execute 7z: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "7z extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Extract rar archive using system unrar command
    async fn extract_rar(archive_path: &Path, extract_dir: &Path) -> Result<(), String> {
        let output = tokio::process::Command::new("unrar")
            .arg("x")
            .arg("-y")
            .arg(archive_path)
            .arg(extract_dir)
            .output()
            .await
            .map_err(|e| format!("Failed to execute unrar: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "unrar extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Extract tar/tar.gz archive using system tar command
    async fn extract_tar(archive_path: &Path, extract_dir: &Path) -> Result<(), String> {
        let output = tokio::process::Command::new("tar")
            .arg("-xf")
            .arg(archive_path)
            .arg("-C")
            .arg(extract_dir)
            .output()
            .await
            .map_err(|e| format!("Failed to execute tar: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "tar extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        Ok(())
    }

    /// Identify bonus content files by filename patterns
    pub fn identify_bonus_content(filename: &str) -> Option<ContentCategory> {
        let lower = filename.to_lowercase();

        // Check for patterns in order of specificity
        if lower.contains("behind the scenes") || lower.contains("behind_the_scenes") {
            return Some(ContentCategory::BehindTheScenes);
        }

        if lower.contains("deleted scene") || lower.contains("deleted_scene") {
            return Some(ContentCategory::DeletedScene);
        }

        if lower.contains("interview") {
            return Some(ContentCategory::Interview);
        }

        if lower.contains("featurette") {
            return Some(ContentCategory::Featurette);
        }

        if lower.contains("blooper") || lower.contains("bloopers") {
            return Some(ContentCategory::Featurette);
        }

        None
    }

    /// Scan extracted directory for bonus content files
    pub async fn scan_extracted_files(
        extract_dir: &Path,
    ) -> Result<Vec<(PathBuf, ContentCategory)>, String> {
        let mut bonus_files = Vec::new();
        let mut dirs_to_scan = vec![extract_dir.to_path_buf()];

        while let Some(dir) = dirs_to_scan.pop() {
            let mut entries = fs::read_dir(&dir)
                .await
                .map_err(|e| format!("Failed to read directory: {}", e))?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| format!("Failed to read directory entry: {}", e))?
            {
                let path = entry.path();

                if path.is_dir() {
                    dirs_to_scan.push(path);
                } else if Self::is_video_file(&path)
                    && let Some(filename) = path.file_name()
                    && let Some(filename_str) = filename.to_str()
                    && let Some(category) = Self::identify_bonus_content(filename_str)
                {
                    debug!("Found bonus content: {} -> {:?}", filename_str, category);
                    bonus_files.push((path, category));
                }
            }
        }

        Ok(bonus_files)
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

    /// Clean up extraction directory
    pub async fn cleanup_extraction(extract_dir: &Path) -> Result<(), String> {
        if extract_dir.exists() {
            fs::remove_dir_all(extract_dir)
                .await
                .map_err(|e| format!("Failed to clean up extraction directory: {}", e))?;
            debug!("Cleaned up extraction directory: {}", extract_dir.display());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_is_archive_zip() {
        assert!(SeasonPackProcessor::is_archive(Path::new("archive.zip")));
    }

    #[test]
    fn test_is_archive_7z() {
        assert!(SeasonPackProcessor::is_archive(Path::new("archive.7z")));
    }

    #[test]
    fn test_is_archive_rar() {
        assert!(SeasonPackProcessor::is_archive(Path::new("archive.rar")));
    }

    #[test]
    fn test_is_archive_tar_gz() {
        assert!(SeasonPackProcessor::is_archive(Path::new("archive.tar.gz")));
    }

    #[test]
    fn test_is_archive_not_video() {
        assert!(!SeasonPackProcessor::is_archive(Path::new("video.mp4")));
    }

    #[test]
    fn test_identify_behind_the_scenes() {
        let category = SeasonPackProcessor::identify_bonus_content("behind the scenes.mp4");
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));
    }

    #[test]
    fn test_identify_deleted_scene() {
        let category = SeasonPackProcessor::identify_bonus_content("deleted_scene_01.mp4");
        assert_eq!(category, Some(ContentCategory::DeletedScene));
    }

    #[test]
    fn test_identify_interview() {
        let category = SeasonPackProcessor::identify_bonus_content("cast_interview.mp4");
        assert_eq!(category, Some(ContentCategory::Interview));
    }

    #[test]
    fn test_identify_featurette() {
        let category = SeasonPackProcessor::identify_bonus_content("featurette.mp4");
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_identify_blooper() {
        let category = SeasonPackProcessor::identify_bonus_content("bloopers.mp4");
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_identify_unknown_pattern() {
        let category = SeasonPackProcessor::identify_bonus_content("random_video.mp4");
        assert_eq!(category, None);
    }

    #[test]
    fn test_identify_case_insensitive() {
        let category = SeasonPackProcessor::identify_bonus_content("BEHIND THE SCENES.mp4");
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));
    }

    // Property 15: Season Pack File Identification
    // Validates: Requirements 15.2, 15.3, 15.4, 15.5, 15.6, 15.7, 15.8
    proptest! {
        #[test]
        fn prop_season_pack_file_identification(
            filename in r"[a-zA-Z0-9_\-\s]{1,50}(behind the scenes|deleted scene|interview|featurette|blooper)[a-zA-Z0-9_\-\s]{0,20}\.mp4"
        ) {
            let category = SeasonPackProcessor::identify_bonus_content(&filename);
            prop_assert!(category.is_some(), "Should identify bonus content in: {}", filename);

            // Verify the identified category matches the pattern
            let lower = filename.to_lowercase();
            if lower.contains("behind the scenes") {
                prop_assert_eq!(category, Some(ContentCategory::BehindTheScenes));
            } else if lower.contains("deleted scene") {
                prop_assert_eq!(category, Some(ContentCategory::DeletedScene));
            } else if lower.contains("interview") {
                prop_assert_eq!(category, Some(ContentCategory::Interview));
            } else if lower.contains("featurette") {
                prop_assert_eq!(category, Some(ContentCategory::Featurette));
            } else if lower.contains("blooper") {
                prop_assert_eq!(category, Some(ContentCategory::Featurette));
            }
        }

        #[test]
        fn prop_archive_detection_consistency(
            filename in r"[a-zA-Z0-9_\-]{1,50}\.(zip|7z|rar|tar|gz|tgz)"
        ) {
            let path = Path::new(&filename);
            let is_archive = SeasonPackProcessor::is_archive(path);
            prop_assert!(is_archive, "Should detect archive: {}", filename);
        }

        #[test]
        fn prop_non_archive_detection(
            filename in r"[a-zA-Z0-9_\-]{1,50}\.(mp4|mkv|avi|mov|txt|pdf)"
        ) {
            let path = Path::new(&filename);
            let is_archive = SeasonPackProcessor::is_archive(path);
            prop_assert!(!is_archive, "Should not detect as archive: {}", filename);
        }

        #[test]
        fn prop_unknown_pattern_returns_none(
            filename in r"[a-zA-Z0-9_\-]{1,50}\.mp4"
        ) {
            // Only test filenames that don't contain known patterns
            if !filename.to_lowercase().contains("behind the scenes")
                && !filename.to_lowercase().contains("deleted scene")
                && !filename.to_lowercase().contains("interview")
                && !filename.to_lowercase().contains("featurette")
                && !filename.to_lowercase().contains("blooper")
            {
                let category = SeasonPackProcessor::identify_bonus_content(&filename);
                prop_assert_eq!(category, None, "Should not identify unknown pattern: {}", filename);
            }
        }
    }
}
