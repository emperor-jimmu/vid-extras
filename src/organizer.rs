// Organizer module - moves converted files to Jellyfin subdirectories and creates done markers

use crate::error::OrganizerError;
use crate::models::{ContentCategory, ConversionResult, DoneMarker};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Organizer handles file organization into Jellyfin-compatible directory structure
pub struct Organizer {
    /// Path to the movie folder
    movie_path: PathBuf,
}

impl Organizer {
    /// Create a new Organizer for a specific movie folder
    pub fn new(movie_path: PathBuf) -> Self {
        Self { movie_path }
    }

    /// Organize converted files into appropriate subdirectories and create done marker
    ///
    /// This method:
    /// 1. Creates subdirectories for each content category if they don't exist
    /// 2. Moves converted files to their appropriate subdirectories
    /// 3. Cleans up the temporary download folder
    /// 4. Creates a done marker file to indicate completion
    pub async fn organize(
        &self,
        conversions: Vec<ConversionResult>,
        temp_dir: &Path,
    ) -> Result<(), OrganizerError> {
        info!(
            "Organizing {} files for movie at {:?}",
            conversions.len(),
            self.movie_path
        );

        // Group conversions by category
        let mut files_by_category: std::collections::HashMap<ContentCategory, Vec<PathBuf>> =
            std::collections::HashMap::new();

        for conversion in conversions {
            if !conversion.success {
                warn!("Skipping failed conversion: {:?}", conversion.input_path);
                continue;
            }

            // Use the category from the conversion result
            files_by_category
                .entry(conversion.category)
                .or_default()
                .push(conversion.output_path);
        }

        // Move files to their subdirectories
        for (category, files) in files_by_category {
            let subdir = self.ensure_subdirectory(category).await?;

            for file_path in files {
                self.move_file(&file_path, &subdir).await?;
            }
        }

        // Clean up temp directory
        self.cleanup_temp_dir(temp_dir).await?;

        // Create done marker
        self.create_done_marker().await?;

        info!("Organization complete for {:?}", self.movie_path);
        Ok(())
    }

    /// Ensure a subdirectory exists for the given content category
    async fn ensure_subdirectory(
        &self,
        category: ContentCategory,
    ) -> Result<PathBuf, OrganizerError> {
        let subdir_name = category.subdirectory();
        let subdir_path = self.movie_path.join(subdir_name);

        debug!("Ensuring subdirectory exists: {:?}", subdir_path);

        if !subdir_path.exists() {
            fs::create_dir_all(&subdir_path).await.map_err(|e| {
                OrganizerError::SubdirectoryCreation(format!(
                    "Failed to create {:?}: {}",
                    subdir_path, e
                ))
            })?;
            info!("Created subdirectory: {:?}", subdir_path);
        }

        Ok(subdir_path)
    }

    /// Move a file to the target subdirectory
    async fn move_file(&self, source: &Path, dest_dir: &Path) -> Result<(), OrganizerError> {
        let file_name = source
            .file_name()
            .ok_or_else(|| OrganizerError::FileMove("Invalid source path".to_string()))?;

        let dest_path = dest_dir.join(file_name);

        debug!("Moving file: {:?} -> {:?}", source, dest_path);

        // Try rename first (fast, atomic), but fall back to copy+delete for cross-drive moves
        match fs::rename(source, &dest_path).await {
            Ok(_) => {
                info!("Moved file to: {:?}", dest_path);
                Ok(())
            }
            Err(e) if e.raw_os_error() == Some(17) => {
                // Error 17 on Windows: "The system cannot move the file to a different disk drive"
                // Fall back to copy + delete
                debug!(
                    "Cross-drive move detected, using copy+delete: {:?} -> {:?}",
                    source, dest_path
                );

                fs::copy(source, &dest_path).await.map_err(|e| {
                    OrganizerError::FileMove(format!(
                        "Failed to copy {:?} to {:?}: {}",
                        source, dest_path, e
                    ))
                })?;

                fs::remove_file(source).await.map_err(|e| {
                    OrganizerError::FileMove(format!(
                        "Failed to delete source file {:?} after copy: {}",
                        source, e
                    ))
                })?;

                info!("Copied and deleted file to: {:?}", dest_path);
                Ok(())
            }
            Err(e) => Err(OrganizerError::FileMove(format!(
                "Failed to move {:?} to {:?}: {}",
                source, dest_path, e
            ))),
        }
    }

    /// Clean up the temporary download directory
    async fn cleanup_temp_dir(&self, temp_dir: &Path) -> Result<(), OrganizerError> {
        if !temp_dir.exists() {
            debug!(
                "Temp directory does not exist, skipping cleanup: {:?}",
                temp_dir
            );
            return Ok(());
        }

        debug!("Cleaning up temp directory: {:?}", temp_dir);

        fs::remove_dir_all(temp_dir).await.map_err(|e| {
            // Log as warning but don't fail the operation
            warn!("Failed to cleanup temp directory {:?}: {}", temp_dir, e);
            OrganizerError::Io(e)
        })?;

        info!("Cleaned up temp directory: {:?}", temp_dir);
        Ok(())
    }

    /// Create a done marker file indicating successful completion
    async fn create_done_marker(&self) -> Result<(), OrganizerError> {
        let marker_path = self.movie_path.join("done.ext");

        debug!("Creating done marker: {:?}", marker_path);

        let marker = DoneMarker {
            finished_at: chrono::Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        let json = serde_json::to_string_pretty(&marker)
            .map_err(|e| OrganizerError::Io(std::io::Error::other(e)))?;

        fs::write(&marker_path, json).await?;

        info!("Created done marker: {:?}", marker_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_ensure_subdirectory_creates_missing_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        let subdir = organizer
            .ensure_subdirectory(ContentCategory::Trailer)
            .await
            .unwrap();

        assert!(subdir.exists());
        assert_eq!(subdir, movie_path.join("trailers"));
    }

    #[tokio::test]
    async fn test_ensure_subdirectory_handles_existing_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let trailers_dir = movie_path.join("trailers");
        fs::create_dir(&trailers_dir).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        let subdir = organizer
            .ensure_subdirectory(ContentCategory::Trailer)
            .await
            .unwrap();

        assert!(subdir.exists());
        assert_eq!(subdir, trailers_dir);
    }

    #[tokio::test]
    async fn test_move_file_success() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let trailers_dir = movie_path.join("trailers");
        fs::create_dir(&trailers_dir).await.unwrap();

        let source_file = temp.path().join("test_trailer.mp4");
        fs::write(&source_file, b"test content").await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        organizer
            .move_file(&source_file, &trailers_dir)
            .await
            .unwrap();

        assert!(!source_file.exists());
        assert!(trailers_dir.join("test_trailer.mp4").exists());
    }

    #[tokio::test]
    async fn test_cleanup_temp_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();
        fs::write(temp_dir.join("file.mp4"), b"test").await.unwrap();

        let organizer = Organizer::new(movie_path);
        organizer.cleanup_temp_dir(&temp_dir).await.unwrap();

        assert!(!temp_dir.exists());
    }

    #[tokio::test]
    async fn test_create_done_marker() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        organizer.create_done_marker().await.unwrap();

        let marker_path = movie_path.join("done.ext");
        assert!(marker_path.exists());

        let content = fs::read_to_string(&marker_path).await.unwrap();
        let marker: DoneMarker = serde_json::from_str(&content).unwrap();

        assert!(!marker.finished_at.is_empty());
        assert!(!marker.version.is_empty());
    }

    #[tokio::test]
    async fn test_organize_integration() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        // Create test conversion results
        let trailer_file = temp_dir.join("trailer.converted.mp4");
        fs::write(&trailer_file, b"trailer content").await.unwrap();

        let featurette_file = temp_dir.join("featurette.converted.mp4");
        fs::write(&featurette_file, b"featurette content")
            .await
            .unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("trailer.mp4"),
                output_path: trailer_file.clone(),
                category: ContentCategory::Trailer,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("featurette.mp4"),
                output_path: featurette_file.clone(),
                category: ContentCategory::Featurette,
                success: true,
                error: None,
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &temp_dir).await.unwrap();

        // Verify subdirectories were created
        assert!(movie_path.join("trailers").exists());
        assert!(movie_path.join("featurettes").exists());

        // Verify files were moved
        assert!(movie_path.join("trailers/trailer.converted.mp4").exists());
        assert!(
            movie_path
                .join("featurettes/featurette.converted.mp4")
                .exists()
        );

        // Verify temp dir was cleaned up
        assert!(!temp_dir.exists());

        // Verify done marker was created
        assert!(movie_path.join("done.ext").exists());
    }

    #[tokio::test]
    async fn test_organize_skips_failed_conversions() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        let success_file = temp_dir.join("success.converted.mp4");
        fs::write(&success_file, b"success content").await.unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("success.mp4"),
                output_path: success_file.clone(),
                category: ContentCategory::Trailer,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("failed.mp4"),
                output_path: temp_dir.join("failed.converted.mp4"),
                category: ContentCategory::Featurette,
                success: false,
                error: Some("Conversion failed".to_string()),
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &temp_dir).await.unwrap();

        // Only successful conversion should be organized
        assert!(movie_path.join("trailers").exists());
        assert!(movie_path.join("trailers/success.converted.mp4").exists());

        // Failed conversion should not create subdirectory
        assert!(!movie_path.join("featurettes").exists());
    }

    #[tokio::test]
    async fn test_organize_handles_multiple_categories() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let temp_dir = temp.path().join("tmp_downloads");
        fs::create_dir(&temp_dir).await.unwrap();

        // Create files for different categories
        let trailer = temp_dir.join("trailer.converted.mp4");
        let featurette = temp_dir.join("featurette.converted.mp4");
        let behind = temp_dir.join("behind.converted.mp4");
        let deleted = temp_dir.join("deleted.converted.mp4");

        fs::write(&trailer, b"trailer").await.unwrap();
        fs::write(&featurette, b"featurette").await.unwrap();
        fs::write(&behind, b"behind").await.unwrap();
        fs::write(&deleted, b"deleted").await.unwrap();

        let conversions = vec![
            ConversionResult {
                input_path: temp_dir.join("trailer.mp4"),
                output_path: trailer,
                category: ContentCategory::Trailer,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("featurette.mp4"),
                output_path: featurette,
                category: ContentCategory::Featurette,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("behind.mp4"),
                output_path: behind,
                category: ContentCategory::BehindTheScenes,
                success: true,
                error: None,
            },
            ConversionResult {
                input_path: temp_dir.join("deleted.mp4"),
                output_path: deleted,
                category: ContentCategory::DeletedScene,
                success: true,
                error: None,
            },
        ];

        let organizer = Organizer::new(movie_path.clone());
        organizer.organize(conversions, &temp_dir).await.unwrap();

        // Verify all subdirectories were created
        assert!(movie_path.join("trailers").exists());
        assert!(movie_path.join("featurettes").exists());
        assert!(movie_path.join("behind the scenes").exists());
        assert!(movie_path.join("deleted scenes").exists());

        // Verify all files were moved
        assert!(movie_path.join("trailers/trailer.converted.mp4").exists());
        assert!(
            movie_path
                .join("featurettes/featurette.converted.mp4")
                .exists()
        );
        assert!(
            movie_path
                .join("behind the scenes/behind.converted.mp4")
                .exists()
        );
        assert!(
            movie_path
                .join("deleted scenes/deleted.converted.mp4")
                .exists()
        );
    }

    #[tokio::test]
    async fn test_done_marker_json_format() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let organizer = Organizer::new(movie_path.clone());
        organizer.create_done_marker().await.unwrap();

        let marker_path = movie_path.join("done.ext");
        let content = fs::read_to_string(&marker_path).await.unwrap();

        // Verify it's valid JSON
        let marker: DoneMarker = serde_json::from_str(&content).unwrap();

        // Verify timestamp is ISO 8601 format
        assert!(chrono::DateTime::parse_from_rfc3339(&marker.finished_at).is_ok());

        // Verify version matches package version
        assert_eq!(marker.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_cleanup_temp_dir_handles_nonexistent_dir() {
        let temp = TempDir::new().unwrap();
        let movie_path = temp.path().join("Movie (2020)");
        fs::create_dir(&movie_path).await.unwrap();

        let nonexistent_dir = temp.path().join("does_not_exist");

        let organizer = Organizer::new(movie_path);
        // Should not error when temp dir doesn't exist
        let result = organizer.cleanup_temp_dir(&nonexistent_dir).await;
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: extras-fetcher, Property 23: Content Category to Subdirectory Mapping
    // Validates: Requirements 8.1, 8.2, 8.3, 8.4
    proptest! {
        #[test]
        fn prop_category_to_subdirectory_mapping(
            category in prop_oneof![
                Just(ContentCategory::Trailer),
                Just(ContentCategory::Featurette),
                Just(ContentCategory::BehindTheScenes),
                Just(ContentCategory::DeletedScene),
                Just(ContentCategory::Interview),
            ]
        ) {
            let expected_subdir = match category {
                ContentCategory::Trailer => "trailers",
                ContentCategory::Featurette => "featurettes",
                ContentCategory::BehindTheScenes => "behind the scenes",
                ContentCategory::DeletedScene => "deleted scenes",
                ContentCategory::Interview => "interviews",
            };

            prop_assert_eq!(category.subdirectory(), expected_subdir);
        }
    }

    // Feature: extras-fetcher, Property 24: Subdirectory Creation
    // Validates: Requirements 8.5
    proptest! {
        #[test]
        fn prop_subdirectory_creation(
            category in prop_oneof![
                Just(ContentCategory::Trailer),
                Just(ContentCategory::Featurette),
                Just(ContentCategory::BehindTheScenes),
                Just(ContentCategory::DeletedScene),
                Just(ContentCategory::Interview),
            ]
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let movie_path = temp.path().join("Movie (2020)");
                tokio::fs::create_dir(&movie_path).await.unwrap();

                let organizer = Organizer::new(movie_path.clone());
                let subdir = organizer.ensure_subdirectory(category).await.unwrap();

                // Verify subdirectory was created
                prop_assert!(subdir.exists());

                // Verify subdirectory is in the correct location
                let expected_path = movie_path.join(category.subdirectory());
                prop_assert_eq!(subdir, expected_path);

                Ok(())
            })?;
        }
    }

    // Feature: extras-fetcher, Property 25: Temp Folder Cleanup on Success
    // Validates: Requirements 8.6
    proptest! {
        #[test]
        fn prop_temp_folder_cleanup_on_success(
            num_files in 1usize..10usize,
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let movie_path = temp.path().join("Movie (2020)");
                tokio::fs::create_dir(&movie_path).await.unwrap();

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                // Create multiple files in temp directory
                for i in 0..num_files {
                    let file_path = temp_dir.join(format!("file_{}.mp4", i));
                    tokio::fs::write(&file_path, b"test content").await.unwrap();
                }

                // Verify temp directory exists with files
                prop_assert!(temp_dir.exists());
                let mut count = 0;
                let mut read_dir = tokio::fs::read_dir(&temp_dir).await.unwrap();
                while let Some(_entry) = read_dir.next_entry().await.unwrap() {
                    count += 1;
                }
                prop_assert_eq!(count, num_files);

                // Organize with empty conversions (just cleanup)
                let organizer = Organizer::new(movie_path.clone());
                organizer.organize(vec![], &temp_dir).await.unwrap();

                // Verify temp directory was deleted
                prop_assert!(!temp_dir.exists());

                Ok(())
            })?;
        }
    }

    // Feature: extras-fetcher, Property 26: Done Marker Creation on Completion
    // Validates: Requirements 2.1, 8.7
    proptest! {
        #[test]
        fn prop_done_marker_creation_on_completion(
            num_conversions in 0usize..5usize,
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let temp = tempfile::TempDir::new().unwrap();
                let movie_path = temp.path().join("Movie (2020)");
                tokio::fs::create_dir(&movie_path).await.unwrap();

                let temp_dir = temp.path().join("tmp_downloads");
                tokio::fs::create_dir(&temp_dir).await.unwrap();

                // Create conversion results
                let mut conversions = vec![];
                for i in 0..num_conversions {
                    let output_file = temp_dir.join(format!("file_{}.converted.mp4", i));
                    tokio::fs::write(&output_file, b"test content").await.unwrap();

                    conversions.push(ConversionResult {
                        input_path: temp_dir.join(format!("file_{}.mp4", i)),
                        output_path: output_file,
                        category: ContentCategory::Trailer,
                        success: true,
                        error: None,
                    });
                }

                // Organize
                let organizer = Organizer::new(movie_path.clone());
                organizer.organize(conversions, &temp_dir).await.unwrap();

                // Verify done marker was created
                let marker_path = movie_path.join("done.ext");
                prop_assert!(marker_path.exists());

                // Verify done marker content is valid JSON with required fields
                let content = tokio::fs::read_to_string(&marker_path).await.unwrap();
                let marker: DoneMarker = serde_json::from_str(&content).unwrap();

                // Verify timestamp is not empty and is valid ISO 8601
                prop_assert!(!marker.finished_at.is_empty());
                prop_assert!(chrono::DateTime::parse_from_rfc3339(&marker.finished_at).is_ok());

                // Verify version is not empty
                prop_assert!(!marker.version.is_empty());

                Ok(())
            })?;
        }
    }
}
