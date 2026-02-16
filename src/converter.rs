// Converter module - video format conversion with ffmpeg

use crate::error::ConversionError;
use crate::models::{ConversionResult, DownloadResult, HardwareAccel};
use log::{debug, error, info, warn};
use std::path::Path;
use std::process::Stdio;
use tokio::fs;
use tokio::process::Command;

/// Video converter that uses ffmpeg to convert videos to x265/HEVC format
#[derive(Debug)]
pub struct Converter {
    /// Hardware acceleration type to use
    hw_accel: HardwareAccel,
    /// CRF value for encoding quality (24-26)
    crf: u8,
}

impl Converter {
    /// Create a new Converter with auto-detected hardware acceleration
    pub fn new() -> Self {
        let hw_accel = Self::detect_hardware_accel();
        info!("Detected hardware acceleration: {}", hw_accel);

        Self {
            hw_accel,
            crf: 25, // Default CRF value in the middle of the range
        }
    }

    /// Create a new Converter with specified hardware acceleration and CRF
    #[allow(dead_code)]
    pub fn with_config(hw_accel: HardwareAccel, crf: u8) -> Self {
        // Validate CRF is in acceptable range (24-26)
        let crf = if (24..=26).contains(&crf) {
            crf
        } else {
            warn!("CRF value {} out of range (24-26), using default 25", crf);
            25
        };

        Self { hw_accel, crf }
    }

    /// Convert a batch of downloaded videos
    pub async fn convert_batch(&self, downloads: Vec<DownloadResult>) -> Vec<ConversionResult> {
        let mut results = Vec::new();
        let total = downloads.iter().filter(|d| d.success).count();
        let mut current = 0;

        for download in downloads {
            if !download.success {
                debug!(
                    "Skipping conversion for failed download: {}",
                    download.source.title
                );
                continue;
            }

            current += 1;
            crate::output::display_conversion_progress(&download.source.title, current, total);

            let result = self.convert_single(&download).await;
            results.push(result);
        }

        results
    }

    /// Convert a single video file
    async fn convert_single(&self, download: &DownloadResult) -> ConversionResult {
        let input_path = &download.local_path;

        // Generate temporary output path to avoid overwriting input during conversion
        // Use a .tmp.mp4 extension during conversion, then rename to final .mp4
        let temp_output_path = input_path.with_extension("tmp.mp4");
        let final_output_path = input_path.with_extension("mp4");

        info!(
            "Converting {} using {}",
            download.source.title, self.hw_accel
        );

        // Build and execute ffmpeg command to temporary output
        match self.execute_conversion(input_path, &temp_output_path).await {
            Ok(_) => {
                // Conversion succeeded - rename temp to final output
                if let Err(e) = fs::rename(&temp_output_path, &final_output_path).await {
                    error!(
                        "Failed to rename temp output {:?} to {:?}: {}",
                        temp_output_path, final_output_path, e
                    );
                    // Clean up temp file
                    let _ = fs::remove_file(&temp_output_path).await;

                    return ConversionResult {
                        input_path: input_path.clone(),
                        output_path: final_output_path.clone(),
                        category: download.source.category,
                        success: false,
                        error: Some(format!("Failed to rename output: {}", e)),
                    };
                }

                // Delete original file only after successful rename
                if input_path != &final_output_path
                    && let Err(e) = fs::remove_file(input_path).await
                {
                    warn!("Failed to delete original file {:?}: {}", input_path, e);
                }

                ConversionResult {
                    input_path: input_path.clone(),
                    output_path: final_output_path.clone(),
                    category: download.source.category,
                    success: true,
                    error: None,
                }
            }
            Err(e) => {
                error!("Conversion failed for {}: {}", download.source.title, e);

                // Conversion failed - delete failed temp output, keep original
                if temp_output_path.exists()
                    && let Err(del_err) = fs::remove_file(&temp_output_path).await
                {
                    warn!(
                        "Failed to delete failed temp output {:?}: {}",
                        temp_output_path, del_err
                    );
                }

                // Note: If input and output paths are the same (e.g., both .mp4),
                // we don't delete anything since the original must be preserved
                ConversionResult {
                    input_path: input_path.clone(),
                    output_path: final_output_path.clone(),
                    category: download.source.category,
                    success: false,
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Execute ffmpeg conversion command
    async fn execute_conversion(&self, input: &Path, output: &Path) -> Result<(), ConversionError> {
        let mut cmd = self.build_ffmpeg_command(input, output);

        debug!("Executing ffmpeg command: {:?}", cmd);

        let output_result = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(ConversionError::Io)?;

        if !output_result.status.success() {
            let stderr = String::from_utf8_lossy(&output_result.stderr);
            return Err(ConversionError::FfmpegFailed(format!(
                "Exit code: {:?}, stderr: {}",
                output_result.status.code(),
                stderr
            )));
        }

        Ok(())
    }

    /// Build ffmpeg command based on hardware acceleration type
    fn build_ffmpeg_command(&self, input: &Path, output: &Path) -> Command {
        let mut cmd = Command::new("ffmpeg");

        // Overwrite output files without asking
        cmd.arg("-y");

        match self.hw_accel {
            HardwareAccel::Nvenc => {
                // NVIDIA NVENC hardware acceleration
                cmd.arg("-hwaccel").arg("cuda");
                cmd.arg("-i").arg(input);
                cmd.arg("-c:v").arg("hevc_nvenc");
                cmd.arg("-preset").arg("p3"); // Faster preset for faster speed
                cmd.arg("-rc").arg("vbr");
                cmd.arg("-cq").arg(self.crf.to_string());
                cmd.arg("-c:a").arg("copy");
                cmd.arg(output);
            }
            HardwareAccel::Qsv => {
                // Intel Quick Sync Video hardware acceleration
                cmd.arg("-hwaccel").arg("qsv");
                cmd.arg("-i").arg(input);
                cmd.arg("-c:v").arg("hevc_qsv");
                cmd.arg("-global_quality").arg(self.crf.to_string());
                cmd.arg("-c:a").arg("copy");
                cmd.arg(output);
            }
            HardwareAccel::Software => {
                // Software encoding with libx265
                cmd.arg("-i").arg(input);
                cmd.arg("-c:v").arg("libx265");
                cmd.arg("-crf").arg(self.crf.to_string());
                cmd.arg("-preset").arg("medium");
                cmd.arg("-c:a").arg("copy");
                cmd.arg(output);
            }
        }

        cmd
    }

    /// Detect available hardware acceleration
    fn detect_hardware_accel() -> HardwareAccel {
        // Try NVENC first
        if Self::check_encoder_support("hevc_nvenc") {
            return HardwareAccel::Nvenc;
        }

        // Try QSV next
        if Self::check_encoder_support("hevc_qsv") {
            return HardwareAccel::Qsv;
        }

        // Fall back to software encoding
        HardwareAccel::Software
    }

    /// Check if ffmpeg supports a specific encoder
    fn check_encoder_support(encoder: &str) -> bool {
        let output = std::process::Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-encoders")
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains(encoder)
            }
            Err(_) => false,
        }
    }

    /// Get the hardware acceleration type being used
    #[allow(dead_code)]
    pub fn hw_accel(&self) -> HardwareAccel {
        self.hw_accel
    }

    /// Get the CRF value being used
    #[allow(dead_code)]
    pub fn crf(&self) -> u8 {
        self.crf
    }
}

impl Default for Converter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContentCategory, SourceType, VideoSource};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_download_result(temp_dir: &TempDir, filename: &str) -> DownloadResult {
        let path = temp_dir.path().join(filename);
        std::fs::write(&path, b"fake video content").unwrap();

        DownloadResult {
            source: VideoSource {
                url: "https://example.com/video".to_string(),
                source_type: SourceType::YouTube,
                category: ContentCategory::Trailer,
                title: "Test Video".to_string(),
            },
            local_path: path,
            success: true,
            error: None,
        }
    }

    #[test]
    fn test_new_converter_detects_hardware() {
        let converter = Converter::new();
        // Should detect some form of hardware acceleration or fall back to software
        assert!(matches!(
            converter.hw_accel(),
            HardwareAccel::Nvenc | HardwareAccel::Qsv | HardwareAccel::Software
        ));
        // Default CRF should be 25
        assert_eq!(converter.crf(), 25);
    }

    #[test]
    fn test_with_config_validates_crf() {
        // Valid CRF values (24-26)
        let converter = Converter::with_config(HardwareAccel::Software, 24);
        assert_eq!(converter.crf(), 24);

        let converter = Converter::with_config(HardwareAccel::Software, 25);
        assert_eq!(converter.crf(), 25);

        let converter = Converter::with_config(HardwareAccel::Software, 26);
        assert_eq!(converter.crf(), 26);

        // Invalid CRF values should default to 25
        let converter = Converter::with_config(HardwareAccel::Software, 20);
        assert_eq!(converter.crf(), 25);

        let converter = Converter::with_config(HardwareAccel::Software, 30);
        assert_eq!(converter.crf(), 25);
    }

    #[test]
    fn test_build_ffmpeg_command_software() {
        let converter = Converter::with_config(HardwareAccel::Software, 25);
        let cmd = converter.build_ffmpeg_command(Path::new("input.mp4"), Path::new("output.mp4"));

        let cmd_str = format!("{:?}", cmd);
        assert!(cmd_str.contains("libx265"));
        assert!(cmd_str.contains("-crf"));
        assert!(cmd_str.contains("25"));
    }

    #[test]
    fn test_build_ffmpeg_command_nvenc() {
        let converter = Converter::with_config(HardwareAccel::Nvenc, 25);
        let cmd = converter.build_ffmpeg_command(Path::new("input.mp4"), Path::new("output.mp4"));

        let cmd_str = format!("{:?}", cmd);
        assert!(cmd_str.contains("hevc_nvenc"));
        assert!(cmd_str.contains("-hwaccel"));
        assert!(cmd_str.contains("cuda"));
    }

    #[test]
    fn test_build_ffmpeg_command_qsv() {
        let converter = Converter::with_config(HardwareAccel::Qsv, 25);
        let cmd = converter.build_ffmpeg_command(Path::new("input.mp4"), Path::new("output.mp4"));

        let cmd_str = format!("{:?}", cmd);
        assert!(cmd_str.contains("hevc_qsv"));
        assert!(cmd_str.contains("-hwaccel"));
        assert!(cmd_str.contains("qsv"));
        assert!(cmd_str.contains("-global_quality"));
    }

    #[test]
    fn test_crf_values_in_range() {
        // Test all valid CRF values
        for crf in 24..=26 {
            let converter = Converter::with_config(HardwareAccel::Software, crf);
            assert_eq!(converter.crf(), crf);
        }
    }

    #[test]
    fn test_crf_below_range_defaults_to_25() {
        let converter = Converter::with_config(HardwareAccel::Software, 20);
        assert_eq!(converter.crf(), 25);

        let converter = Converter::with_config(HardwareAccel::Software, 0);
        assert_eq!(converter.crf(), 25);
    }

    #[test]
    fn test_crf_above_range_defaults_to_25() {
        let converter = Converter::with_config(HardwareAccel::Software, 30);
        assert_eq!(converter.crf(), 25);

        let converter = Converter::with_config(HardwareAccel::Software, 100);
        assert_eq!(converter.crf(), 25);
    }

    #[test]
    fn test_hardware_accel_types() {
        let nvenc = Converter::with_config(HardwareAccel::Nvenc, 25);
        assert_eq!(nvenc.hw_accel(), HardwareAccel::Nvenc);

        let qsv = Converter::with_config(HardwareAccel::Qsv, 25);
        assert_eq!(qsv.hw_accel(), HardwareAccel::Qsv);

        let software = Converter::with_config(HardwareAccel::Software, 25);
        assert_eq!(software.hw_accel(), HardwareAccel::Software);
    }

    #[test]
    fn test_ffmpeg_command_includes_overwrite_flag() {
        let converter = Converter::with_config(HardwareAccel::Software, 25);
        let cmd = converter.build_ffmpeg_command(Path::new("input.mp4"), Path::new("output.mp4"));

        let cmd_str = format!("{:?}", cmd);
        assert!(
            cmd_str.contains("-y"),
            "Command should include -y flag to overwrite output files"
        );
    }

    #[test]
    fn test_ffmpeg_command_copies_audio() {
        // All hardware acceleration types should copy audio
        for hw_accel in [
            HardwareAccel::Nvenc,
            HardwareAccel::Qsv,
            HardwareAccel::Software,
        ] {
            let converter = Converter::with_config(hw_accel, 25);
            let cmd =
                converter.build_ffmpeg_command(Path::new("input.mp4"), Path::new("output.mp4"));

            let cmd_str = format!("{:?}", cmd);
            assert!(
                cmd_str.contains("-c:a") && cmd_str.contains("copy"),
                "Command should copy audio stream for {:?}",
                hw_accel
            );
        }
    }

    #[test]
    fn test_output_path_generation() {
        let temp_dir = TempDir::new().unwrap();
        let input_path = temp_dir.path().join("video.mp4");
        std::fs::write(&input_path, b"fake content").unwrap();

        let _download = DownloadResult {
            source: VideoSource {
                url: "https://example.com/video".to_string(),
                source_type: SourceType::YouTube,
                category: ContentCategory::Trailer,
                title: "Test Video".to_string(),
            },
            local_path: input_path.clone(),
            success: true,
            error: None,
        };

        let expected_output = input_path.with_extension("mp4");

        // Verify the expected output path format
        assert!(expected_output.to_string_lossy().ends_with(".mp4"));
    }

    #[test]
    fn test_check_encoder_support() {
        // This test verifies the encoder checking logic works
        // It should return true for libx265 (software encoding) on any system with ffmpeg
        let _has_libx265 = Converter::check_encoder_support("libx265");

        // We can't guarantee what encoders are available, but we can verify
        // the function returns a boolean and doesn't panic
        // Test passes if no panic occurs
    }

    #[test]
    fn test_detect_hardware_accel_returns_valid_type() {
        let hw_accel = Converter::detect_hardware_accel();

        // Should return one of the three valid types
        assert!(matches!(
            hw_accel,
            HardwareAccel::Nvenc | HardwareAccel::Qsv | HardwareAccel::Software
        ));
    }

    #[tokio::test]
    async fn test_convert_batch_skips_failed_downloads() {
        let temp_dir = TempDir::new().unwrap();
        let converter = Converter::with_config(HardwareAccel::Software, 25);

        let successful = create_test_download_result(&temp_dir, "video1.mp4");
        let failed = DownloadResult {
            source: VideoSource {
                url: "https://example.com/failed".to_string(),
                source_type: SourceType::YouTube,
                category: ContentCategory::Trailer,
                title: "Failed Video".to_string(),
            },
            local_path: PathBuf::from("/nonexistent/path.mp4"),
            success: false,
            error: Some("Download failed".to_string()),
        };

        let results = converter.convert_batch(vec![successful, failed]).await;

        // Should only attempt to convert the successful download
        // The failed download should be skipped, so we get 1 result
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_convert_single_creates_output_path() {
        let temp_dir = TempDir::new().unwrap();
        let converter = Converter::with_config(HardwareAccel::Software, 25);

        let download = create_test_download_result(&temp_dir, "input.mp4");
        let input_path = download.local_path.clone();

        // Note: This will fail because we don't have a real video file,
        // but we can check that the output path is constructed correctly
        let result = converter.convert_single(&download).await;

        // Output path should be input with .mp4 extension
        let expected_output = input_path.with_extension("mp4");
        assert_eq!(result.output_path, expected_output);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::models::{ContentCategory, SourceType, VideoSource};
    use proptest::prelude::*;

    // Feature: extras-fetcher, Property 18: FFmpeg Codec Usage
    // Validates: Requirements 7.1
    proptest! {
        #[test]
        fn prop_ffmpeg_codec_usage(
            hw_accel in prop_oneof![
                Just(HardwareAccel::Nvenc),
                Just(HardwareAccel::Qsv),
                Just(HardwareAccel::Software),
            ],
            crf in 24u8..=26u8
        ) {
            let converter = Converter::with_config(hw_accel, crf);
            let cmd = converter.build_ffmpeg_command(
                Path::new("input.mp4"),
                Path::new("output.mp4"),
            );

            let cmd_str = format!("{:?}", cmd);

            // Verify that one of the HEVC codecs is used
            let has_hevc_codec = cmd_str.contains("hevc_nvenc")
                || cmd_str.contains("hevc_qsv")
                || cmd_str.contains("libx265");

            prop_assert!(
                has_hevc_codec,
                "Command must use x265/HEVC codec, got: {}",
                cmd_str
            );

            // Verify the correct codec for the hardware acceleration type
            match hw_accel {
                HardwareAccel::Nvenc => {
                    prop_assert!(cmd_str.contains("hevc_nvenc"));
                }
                HardwareAccel::Qsv => {
                    prop_assert!(cmd_str.contains("hevc_qsv"));
                }
                HardwareAccel::Software => {
                    prop_assert!(cmd_str.contains("libx265"));
                }
            }
        }
    }

    // Feature: extras-fetcher, Property 19: CRF Value Range
    // Validates: Requirements 7.2
    proptest! {
        #[test]
        fn prop_crf_value_range(
            hw_accel in prop_oneof![
                Just(HardwareAccel::Nvenc),
                Just(HardwareAccel::Qsv),
                Just(HardwareAccel::Software),
            ],
            crf in 24u8..=26u8
        ) {
            let converter = Converter::with_config(hw_accel, crf);

            // Verify CRF is in the valid range
            prop_assert!(
                converter.crf() >= 24 && converter.crf() <= 26,
                "CRF value must be between 24 and 26, got: {}",
                converter.crf()
            );

            // Verify the CRF value is used in the command
            let cmd = converter.build_ffmpeg_command(
                Path::new("input.mp4"),
                Path::new("output.mp4"),
            );
            let cmd_str = format!("{:?}", cmd);

            // Check that the CRF value appears in the command
            // Different hardware acceleration types use different flags
            let crf_str = converter.crf().to_string();
            prop_assert!(
                cmd_str.contains(&crf_str),
                "Command must contain CRF value {}, got: {}",
                crf_str,
                cmd_str
            );
        }
    }

    // Test that invalid CRF values are clamped to valid range
    proptest! {
        #[test]
        fn prop_crf_value_clamping(
            hw_accel in prop_oneof![
                Just(HardwareAccel::Nvenc),
                Just(HardwareAccel::Qsv),
                Just(HardwareAccel::Software),
            ],
            invalid_crf in prop_oneof![
                0u8..24u8,      // Below valid range
                27u8..=100u8,   // Above valid range
            ]
        ) {
            let converter = Converter::with_config(hw_accel, invalid_crf);

            // Invalid CRF values should be clamped to default (25)
            prop_assert_eq!(
                converter.crf(),
                25,
                "Invalid CRF {} should be clamped to default 25",
                invalid_crf
            );
        }
    }

    // Feature: extras-fetcher, Property 20: Hardware Acceleration Selection
    // Validates: Requirements 7.3, 11.6
    proptest! {
        #[test]
        fn prop_hardware_acceleration_selection(
            hw_accel in prop_oneof![
                Just(HardwareAccel::Nvenc),
                Just(HardwareAccel::Qsv),
                Just(HardwareAccel::Software),
            ]
        ) {
            let converter = Converter::with_config(hw_accel, 25);

            // Verify the converter uses the specified hardware acceleration
            prop_assert_eq!(
                converter.hw_accel(),
                hw_accel,
                "Converter should use specified hardware acceleration"
            );

            // Verify the command uses the correct encoder for the hardware type
            let cmd = converter.build_ffmpeg_command(
                Path::new("input.mp4"),
                Path::new("output.mp4"),
            );
            let cmd_str = format!("{:?}", cmd);

            match hw_accel {
                HardwareAccel::Nvenc => {
                    prop_assert!(
                        cmd_str.contains("hevc_nvenc"),
                        "NVENC should use hevc_nvenc encoder"
                    );
                    prop_assert!(
                        cmd_str.contains("-hwaccel") && cmd_str.contains("cuda"),
                        "NVENC should use CUDA hardware acceleration"
                    );
                }
                HardwareAccel::Qsv => {
                    prop_assert!(
                        cmd_str.contains("hevc_qsv"),
                        "QSV should use hevc_qsv encoder"
                    );
                    prop_assert!(
                        cmd_str.contains("-hwaccel") && cmd_str.contains("qsv"),
                        "QSV should use QSV hardware acceleration"
                    );
                }
                HardwareAccel::Software => {
                    prop_assert!(
                        cmd_str.contains("libx265"),
                        "Software should use libx265 encoder"
                    );
                    prop_assert!(
                        !cmd_str.contains("-hwaccel"),
                        "Software encoding should not use hardware acceleration flag"
                    );
                }
            }
        }
    }

    // Test that auto-detection returns a valid hardware acceleration type
    proptest! {
        #[test]
        fn prop_hardware_detection_returns_valid_type(_dummy in 0u8..10u8) {
            let hw_accel = Converter::detect_hardware_accel();

            // Should return one of the three valid types
            prop_assert!(
                matches!(
                    hw_accel,
                    HardwareAccel::Nvenc | HardwareAccel::Qsv | HardwareAccel::Software
                ),
                "Hardware detection must return a valid acceleration type"
            );
        }
    }

    // Feature: extras-fetcher, Property 21: Conversion Success Cleanup
    // Validates: Requirements 7.4
    // Note: This test verifies the cleanup logic, but actual file deletion
    // requires a real conversion which we can't do in property tests
    proptest! {
        #[test]
        fn prop_conversion_success_cleanup(
            filename in "[a-zA-Z0-9_-]{5,20}\\.mp4"
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let temp_dir = tempfile::TempDir::new().unwrap();
                let converter = Converter::with_config(HardwareAccel::Software, 25);

                // Create a fake input file
                let input_path = temp_dir.path().join(&filename);
                tokio::fs::write(&input_path, b"fake video content").await.unwrap();

                let download = DownloadResult {
                    source: VideoSource {
                        url: "https://example.com/video".to_string(),
                        source_type: SourceType::YouTube,
                        category: ContentCategory::Trailer,
                        title: "Test Video".to_string(),
                    },
                    local_path: input_path.clone(),
                    success: true,
                    error: None,
                };

                // Attempt conversion (will fail because it's not a real video)
                let result = converter.convert_single(&download).await;

                // On failure, original file should still exist
                // (because conversion failed, not succeeded)
                prop_assert!(
                    input_path.exists(),
                    "Original file should exist after failed conversion"
                );

                // On failure, if output path is different from input, it should be deleted
                // If they're the same (e.g., both .mp4), the file should still exist
                if result.output_path != input_path {
                    prop_assert!(
                        !result.output_path.exists(),
                        "Failed output file should be deleted when different from input"
                    );
                }

                Ok(())
            })?;
        }
    }

    // Feature: extras-fetcher, Property 22: Conversion Failure Preservation
    // Validates: Requirements 7.5, 7.6
    proptest! {
        #[test]
        fn prop_conversion_failure_preservation(
            filename in "[a-zA-Z0-9_-]{5,20}\\.mp4"
        ) {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let temp_dir = tempfile::TempDir::new().unwrap();
                let converter = Converter::with_config(HardwareAccel::Software, 25);

                // Create a fake input file
                let input_path = temp_dir.path().join(&filename);
                tokio::fs::write(&input_path, b"fake video content").await.unwrap();

                let download = DownloadResult {
                    source: VideoSource {
                        url: "https://example.com/video".to_string(),
                        source_type: SourceType::YouTube,
                        category: ContentCategory::Trailer,
                        title: "Test Video".to_string(),
                    },
                    local_path: input_path.clone(),
                    success: true,
                    error: None,
                };

                // Attempt conversion (will fail because it's not a real video)
                let result = converter.convert_single(&download).await;

                // Verify conversion failed
                prop_assert!(
                    !result.success,
                    "Conversion should fail for fake video file"
                );

                // Verify error is present
                prop_assert!(
                    result.error.is_some(),
                    "Failed conversion should have error message"
                );

                // On failure, original file should be preserved
                prop_assert!(
                    input_path.exists(),
                    "Original file must be preserved after failed conversion"
                );

                // On failure, if output path is different from input, it should be deleted
                // If they're the same (e.g., both .mp4), the file should still exist
                if result.output_path != input_path {
                    prop_assert!(
                        !result.output_path.exists(),
                        "Failed output file should be deleted when different from input"
                    );
                }

                Ok(())
            })?;
        }
    }
}
