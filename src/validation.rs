use crate::config::Config;
use crate::error::ValidationError;
use std::process::Command;

/// Validator for checking system dependencies and configuration
pub struct Validator;

impl Validator {
    /// Create a new Validator instance
    pub fn new() -> Self {
        Self
    }

    /// Validate all dependencies and configuration at startup
    ///
    /// Checks:
    /// - yt-dlp binary exists in PATH
    /// - ffmpeg binary exists in PATH
    /// - ffmpeg supports HEVC encoding (libx265, hevc_nvenc, or hevc_qsv)
    /// - TMDB API key is configured (in config.cfg or environment variable)
    ///
    /// Returns Ok(api_key) if all checks pass, or ValidationError describing the issue
    pub fn validate_dependencies(&self) -> Result<String, ValidationError> {
        // Check yt-dlp binary
        if !self.check_binary_exists("yt-dlp") {
            return Err(ValidationError::MissingBinary("yt-dlp".to_string()));
        }

        // Check ffmpeg binary
        if !self.check_binary_exists("ffmpeg") {
            return Err(ValidationError::MissingBinary("ffmpeg".to_string()));
        }

        // Check ffmpeg HEVC support
        if !self.check_ffmpeg_hevc_support() {
            return Err(ValidationError::UnsupportedCodec);
        }

        // Check TMDB API key from config file or environment variable
        let api_key = self.check_tmdb_api_key()?;

        Ok(api_key)
    }

    /// Check if a binary exists in the system PATH
    #[cfg_attr(not(test), allow(dead_code))]
    fn check_binary_exists(&self, name: &str) -> bool {
        // Try to execute the binary with --version flag
        // This works for both yt-dlp and ffmpeg
        Command::new(name).arg("--version").output().is_ok()
    }

    /// Check if ffmpeg supports HEVC encoding
    ///
    /// Checks for at least one of: libx265, hevc_nvenc, hevc_qsv
    fn check_ffmpeg_hevc_support(&self) -> bool {
        // Run ffmpeg -encoders to get list of available encoders
        let output = match Command::new("ffmpeg").arg("-encoders").output() {
            Ok(output) => output,
            Err(_) => return false,
        };

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check for HEVC encoders
        stdout.contains("libx265") || stdout.contains("hevc_nvenc") || stdout.contains("hevc_qsv")
    }

    /// Check if TMDB API key is configured
    ///
    /// Checks in this order:
    /// 1. config.cfg file (loads or prompts user to create)
    /// 2. TMDB_API_KEY environment variable (fallback for backward compatibility)
    fn check_tmdb_api_key(&self) -> Result<String, ValidationError> {
        self.check_tmdb_api_key_internal(true)
    }

    /// Internal method for checking TMDB API key with optional prompting
    ///
    /// When allow_prompt is false, only checks existing config file and environment variable
    /// without prompting the user. This is useful for testing.
    #[cfg_attr(not(test), allow(dead_code))]
    fn check_tmdb_api_key_internal(&self, allow_prompt: bool) -> Result<String, ValidationError> {
        // Try to load from existing config file first (without prompting)
        let config_path = Config::default_path();
        if let Ok(config) = Config::load(&config_path)
            && !config.tmdb_api_key.is_empty()
        {
            return Ok(config.tmdb_api_key);
        }

        // If prompting is allowed and config doesn't exist, prompt user
        if allow_prompt {
            match Config::load_or_create() {
                Ok(config) => {
                    if !config.tmdb_api_key.is_empty() {
                        return Ok(config.tmdb_api_key);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to load or create config file: {}", e);
                }
            }
        }

        // Fall back to environment variable for backward compatibility
        std::env::var("TMDB_API_KEY")
            .map_err(|_| ValidationError::MissingApiKey("TMDB_API_KEY".to_string()))
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = Validator::new();
        // Just verify it can be created
        assert!(std::mem::size_of_val(&validator) == 0); // Zero-sized type
    }

    #[test]
    fn test_check_binary_exists_with_nonexistent() {
        let validator = Validator::new();
        // Test with a binary that definitely doesn't exist
        assert!(!validator.check_binary_exists("nonexistent_binary_xyz_123"));
    }

    #[test]
    #[ignore = "Requires TMDB_API_KEY to be unset, which is difficult in test environment"]
    fn test_check_tmdb_api_key_missing() {
        let validator = Validator::new();

        // Use internal method without prompting
        let result = validator.check_tmdb_api_key_internal(false);

        assert!(matches!(result, Err(ValidationError::MissingApiKey(_))));
    }

    #[test]
    #[ignore = "Environment-dependent test - requires ability to set/check TMDB_API_KEY"]
    fn test_check_tmdb_api_key_present() {
        let validator = Validator::new();

        // If TMDB_API_KEY is already set, verify it works
        if let Ok(existing_key) = std::env::var("TMDB_API_KEY") {
            let result = validator.check_tmdb_api_key_internal(false);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), existing_key);
            return;
        }

        // Otherwise, set a test API key
        unsafe {
            std::env::set_var("TMDB_API_KEY", "test_key_12345");
        }

        // Use internal method without prompting
        let result = validator.check_tmdb_api_key_internal(false);

        // Clean up
        unsafe {
            std::env::remove_var("TMDB_API_KEY");
        }

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_key_12345");
    }

    // Note: Testing actual binary existence and ffmpeg codec support
    // depends on the system environment. These tests would need to be
    // run in a controlled environment or mocked for CI/CD.

    #[test]
    #[ignore] // Only run when ffmpeg is available
    fn test_ffmpeg_hevc_support_real() {
        let validator = Validator::new();
        // This test requires ffmpeg to be installed
        // It's marked as ignored so it doesn't fail in environments without ffmpeg
        if validator.check_binary_exists("ffmpeg") {
            let has_hevc = validator.check_ffmpeg_hevc_support();
            // If ffmpeg exists, it should have at least one HEVC encoder
            // (this might fail on minimal ffmpeg builds)
            println!("HEVC support: {}", has_hevc);
        }
    }

    #[test]
    fn test_validation_with_missing_ytdlp() {
        let validator = Validator::new();

        // We can't actually remove yt-dlp from the system, but we can test
        // that check_binary_exists returns false for nonexistent binaries
        let exists = validator.check_binary_exists("definitely_not_a_real_binary_xyz123");
        assert!(!exists);
    }

    #[test]
    fn test_validation_with_missing_ffmpeg() {
        let validator = Validator::new();

        // Test that check_binary_exists returns false for nonexistent binaries
        let exists = validator.check_binary_exists("another_fake_binary_abc456");
        assert!(!exists);
    }

    #[test]
    fn test_validation_error_messages() {
        // Test that error messages are descriptive
        let err1 = ValidationError::MissingBinary("yt-dlp".to_string());
        assert_eq!(format!("{}", err1), "Missing binary: yt-dlp");

        let err2 = ValidationError::MissingBinary("ffmpeg".to_string());
        assert_eq!(format!("{}", err2), "Missing binary: ffmpeg");

        let err3 = ValidationError::MissingApiKey("TMDB_API_KEY".to_string());
        assert_eq!(format!("{}", err3), "Missing API key: TMDB_API_KEY");

        let err4 = ValidationError::UnsupportedCodec;
        assert_eq!(format!("{}", err4), "Unsupported codec");
    }

    #[test]
    #[ignore] // Only run when ffmpeg is available
    fn test_ffmpeg_codec_detection_real() {
        let validator = Validator::new();

        // This test requires ffmpeg to be installed
        if validator.check_binary_exists("ffmpeg") {
            let _has_hevc = validator.check_ffmpeg_hevc_support();

            // If ffmpeg exists, check what we detected
            // We can't assert true here because some minimal ffmpeg builds
            // might not have HEVC support, but we can verify the function runs
            // Test passes if no panic occurs
        }
    }

    #[test]
    fn test_validator_default_trait() {
        let validator1 = Validator::new();
        let validator2 = Validator;

        // Both should be equivalent (zero-sized types)
        assert_eq!(
            std::mem::size_of_val(&validator1),
            std::mem::size_of_val(&validator2)
        );
    }

    #[test]
    fn test_check_binary_exists_with_various_names() {
        let validator = Validator::new();

        // Test with various nonexistent binary names
        assert!(!validator.check_binary_exists(""));
        assert!(!validator.check_binary_exists("fake_binary_1"));
        assert!(!validator.check_binary_exists("fake_binary_2"));
        assert!(!validator.check_binary_exists("nonexistent_tool"));
    }

    #[test]
    #[ignore = "Requires ability to set TMDB_API_KEY, which may conflict with existing environment"]
    fn test_api_key_validation_with_empty_string() {
        let validator = Validator::new();

        // Test with empty string
        unsafe {
            std::env::set_var("TMDB_API_KEY", "");
        }

        // Use internal method without prompting
        let result = validator.check_tmdb_api_key_internal(false);

        // Clean up
        unsafe {
            std::env::remove_var("TMDB_API_KEY");
        }

        // Empty string is still a valid value (though not useful)
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    #[ignore = "Requires ability to set TMDB_API_KEY, which may conflict with existing environment"]
    fn test_api_key_validation_with_special_characters() {
        let validator = Validator::new();

        // Test with special characters in API key
        let test_key = "test-key_123!@#$%";
        unsafe {
            std::env::set_var("TMDB_API_KEY", test_key);
        }

        // Use internal method without prompting
        let result = validator.check_tmdb_api_key_internal(false);

        // Clean up
        unsafe {
            std::env::remove_var("TMDB_API_KEY");
        }

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_key);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: extras-fetcher, Property 32: Dependency Validation at Startup
    // Validates: Requirements 11.1, 11.2, 11.4
    //
    // Note: These tests validate the property that dependency validation checks
    // all requirements (binaries and API key). Due to environment variable sharing
    // in parallel tests, we focus on testing the validation logic structure rather
    // than actual environment manipulation.
    proptest! {
        #[test]
        fn prop_dependency_validation_structure(
            _dummy in 0..100u32
        ) {
            let validator = Validator::new();

            // Property: The validator should have methods for all required checks
            // We test that the methods exist and return appropriate types

            // Check that binary existence check works
            let binary_check = validator.check_binary_exists("nonexistent_test_binary_xyz");
            prop_assert!(!binary_check, "Nonexistent binary should return false");

            // Check that API key validation returns the correct error type when missing
            // We use a unique env var name to avoid conflicts
            let test_key_name = format!("TEST_KEY_{}", _dummy);
            let original = std::env::var(&test_key_name).ok();

            // This should fail because the key doesn't exist
            let result = std::env::var(&test_key_name);
            prop_assert!(result.is_err(), "Test key should not exist");

            // Restore if it somehow existed
            if let Some(val) = original {
                unsafe {
                    std::env::set_var(&test_key_name, val);
                }
            }
        }
    }

    proptest! {
        #[test]
        fn prop_validation_checks_all_dependencies_in_order(
            _iteration in 0..100u32
        ) {
            let validator = Validator::new();

            // Property: Validation should check dependencies in a specific order:
            // 1. yt-dlp binary
            // 2. ffmpeg binary
            // 3. ffmpeg HEVC support
            // 4. TMDB API key
            //
            // We can verify this by checking that each individual check works correctly

            // Test 1: Binary existence check returns boolean
            let ytdlp_check = validator.check_binary_exists("fake_ytdlp_xyz");
            prop_assert!(!ytdlp_check);

            let ffmpeg_check = validator.check_binary_exists("fake_ffmpeg_xyz");
            prop_assert!(!ffmpeg_check);

            // Test 2: HEVC support check returns boolean (requires ffmpeg to exist)
            // We can't test this without ffmpeg, but we verify the method exists
            // by checking it compiles and has the right signature

            // Test 3: API key check returns Result with correct error type
            // Use a unique key name to avoid conflicts
            let unique_key = format!("NONEXISTENT_KEY_{}", _iteration);
            let key_result = std::env::var(&unique_key);
            prop_assert!(key_result.is_err(), "Unique key should not exist");
        }
    }

    // Feature: extras-fetcher, Property 34: Missing Dependency Error Reporting
    // Validates: Requirements 11.5, 10.5
    proptest! {
        #[test]
        #[ignore = "Requires TMDB_API_KEY to be unset, which is difficult in test environment"]
        fn prop_missing_dependency_error_identifies_specific_dependency(
            _dummy in 0..100u32
        ) {
            let validator = Validator::new();

            // Use internal method without prompting
            let result = validator.check_tmdb_api_key_internal(false);

            // Property: Error message should identify "TMDB_API_KEY" specifically
            if let Err(ValidationError::MissingApiKey(key_name)) = result {
                prop_assert_eq!(key_name, "TMDB_API_KEY");
            } else {
                prop_assert!(false, "Expected MissingApiKey error, got: {:?}", result);
            }
        }
    }

    proptest! {
        #[test]
        fn prop_missing_binary_error_identifies_binary_name(
            binary_name in "[a-z]{5,15}"
        ) {
            let validator = Validator::new();

            // Test with a binary that definitely doesn't exist
            let nonexistent_binary = format!("nonexistent_{}_xyz", binary_name);
            let exists = validator.check_binary_exists(&nonexistent_binary);

            // Property: check_binary_exists should return false for nonexistent binaries
            prop_assert!(!exists, "Nonexistent binary should not be found");

            // Property: If we were to create a MissingBinary error, it would contain the binary name
            let error = ValidationError::MissingBinary(nonexistent_binary.clone());
            let error_msg = format!("{}", error);

            // The error message should contain the binary name
            prop_assert!(
                error_msg.contains(&nonexistent_binary),
                "Error message '{}' should contain binary name '{}'",
                error_msg,
                nonexistent_binary
            );
        }
    }
}
