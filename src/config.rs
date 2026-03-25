// Configuration file management

use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// TMDB API key for content discovery.
    /// SECURITY: Must never be logged, printed, or interpolated into log messages.
    pub tmdb_api_key: String,
    /// TheTVDB API key for Season 0 specials discovery.
    /// SECURITY: Must never be logged, printed, or interpolated into log messages.
    #[serde(default)]
    pub tvdb_api_key: Option<String>,
    /// Browser to source cookies from for yt-dlp (e.g. "chrome", "firefox", "edge")
    /// Resolves YouTube bot-detection errors when set
    #[serde(default)]
    pub cookies_from_browser: Option<String>,
    /// Vimeo Personal Access Token for opt-in Vimeo discovery.
    /// SECURITY: Must never be logged, printed, or interpolated into log messages.
    #[serde(default)]
    pub vimeo_access_token: Option<String>,
}

impl Config {
    /// Get the default config file path
    ///
    /// Returns the path to config.cfg in the current directory
    pub fn default_path() -> PathBuf {
        PathBuf::from("config.cfg")
    }

    /// Load configuration from file
    ///
    /// Attempts to read and parse the config file at the given path.
    /// Returns ConfigError if the file doesn't exist or is invalid.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents =
            fs::read_to_string(path).map_err(|e| ConfigError::ReadError(path.to_path_buf(), e))?;

        let config: Config = serde_json::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(path.to_path_buf(), e))?;

        Ok(config)
    }

    /// Save configuration to file
    ///
    /// Writes the configuration to the specified path in JSON format.
    /// On Unix systems, creates the file with 0o600 permissions atomically
    /// (using OpenOptions with mode) so the file is never world-readable,
    /// even briefly. On non-Unix systems, falls back to a plain write.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let contents = serde_json::to_string_pretty(self).map_err(ConfigError::SerializeError)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(path)
                .map_err(|e| ConfigError::WriteError(path.to_path_buf(), e))?;
            file.write_all(contents.as_bytes())
                .map_err(|e| ConfigError::WriteError(path.to_path_buf(), e))?;
            // Also correct permissions on pre-existing files (mode() only applies at creation).
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o600);
            if let Err(e) = std::fs::set_permissions(path, perms) {
                log::warn!("Could not set config file permissions to 600: {}", e);
            }
        }

        #[cfg(not(unix))]
        fs::write(path, contents).map_err(|e| ConfigError::WriteError(path.to_path_buf(), e))?;

        Ok(())
    }

    /// Prompt user for TMDB API key via CLI
    ///
    /// Displays instructions and prompts the user to enter their API key.
    /// Returns the entered key or an error if input fails.
    pub fn prompt_for_api_key() -> Result<String, ConfigError> {
        println!("\n{}", "TMDB API Key Required".to_uppercase());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("To discover movie extras, you need a free TMDB API key.");
        println!("\nHow to get your API key:");
        println!("  1. Visit: https://www.themoviedb.org/settings/api");
        println!("  2. Sign up for a free account (if you don't have one)");
        println!("  3. Request an API key from your account settings");
        println!("  4. Copy the 'API Key (v3 auth)' value");
        println!("\nYour API key will be saved to config.cfg for future use.");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        print!("Enter your TMDB API key: ");
        io::stdout().flush().map_err(ConfigError::IoError)?;

        let mut api_key = String::new();
        io::stdin()
            .read_line(&mut api_key)
            .map_err(ConfigError::IoError)?;

        let api_key = api_key.trim().to_string();

        if api_key.is_empty() {
            return Err(ConfigError::EmptyApiKey);
        }

        Ok(api_key)
    }

    /// Prompt user for TheTVDB API key via CLI
    ///
    /// Displays instructions and prompts the user to enter their API key.
    /// Returns the entered key or an error if input fails.
    pub fn prompt_for_tvdb_api_key() -> Result<String, ConfigError> {
        println!("\n{}", "TheTVDB API Key Required".to_uppercase());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("To discover Season 0 specials, you need a free TheTVDB API key.");
        println!("\nHow to get your API key:");
        println!("  1. Visit: https://www.thetvdb.com/api-information");
        println!("  2. Sign up for a free account (if you don't have one)");
        println!("  3. Request an API key from your account settings");
        println!("  4. Copy the API key value");
        println!("\nYour API key will be saved to config.cfg for future use.");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        print!("Enter your TheTVDB API key: ");
        io::stdout().flush().map_err(ConfigError::IoError)?;

        let mut api_key = String::new();
        io::stdin()
            .read_line(&mut api_key)
            .map_err(ConfigError::IoError)?;

        let api_key = api_key.trim().to_string();

        if api_key.is_empty() {
            return Err(ConfigError::EmptyApiKey);
        }

        Ok(api_key)
    }

    /// Load or create configuration
    ///
    /// Attempts to load config from file. If the file doesn't exist or is invalid,
    /// prompts the user for the API key and creates a new config file.
    pub fn load_or_create() -> Result<Self, ConfigError> {
        let config_path = Self::default_path();

        // Try to load existing config
        match Self::load(&config_path) {
            Ok(config) => {
                log::info!("Loaded configuration from {:?}", config_path);
                Ok(config)
            }
            Err(ConfigError::ReadError(_, _)) => {
                // Config file doesn't exist, prompt for API key
                log::info!("Config file not found, prompting for API key");
                let api_key = Self::prompt_for_api_key()?;

                let config = Config {
                    tmdb_api_key: api_key,
                    tvdb_api_key: None,
                    cookies_from_browser: None,
                    vimeo_access_token: None,
                };

                // Save the new config
                config.save(&config_path)?;
                println!("\n✓ Configuration saved to {:?}", config_path);

                Ok(config)
            }
            Err(e) => {
                // Other errors (parse errors, etc.)
                Err(e)
            }
        }
    }

    /// Load or create configuration, optionally prompting for TVDB key
    ///
    /// Attempts to load config from file. If the file doesn't exist or is invalid,
    /// prompts the user for the API keys and creates a new config file.
    /// If `require_tvdb_key` is true and the loaded config doesn't have a TVDB key,
    /// prompts the user to enter one and saves it.
    pub fn load_or_create_with_tvdb(require_tvdb_key: bool) -> Result<Self, ConfigError> {
        let config_path = Self::default_path();
        let mut config = Self::load_or_create()?;

        // If TVDB key is required and missing, prompt for it
        if require_tvdb_key && config.tvdb_api_key.is_none() {
            log::info!("TVDB key required but not found in config");
            let tvdb_key = Self::prompt_for_tvdb_api_key()?;
            config.tvdb_api_key = Some(tvdb_key);

            // Save the updated config
            config.save(&config_path)?;
            println!("\n✓ Configuration updated and saved to {:?}", config_path);
        }

        Ok(config)
    }

    /// Prompt user for Vimeo Personal Access Token via CLI
    ///
    /// Displays instructions and prompts the user to enter their token.
    /// Returns the entered token or an error if input fails.
    pub fn prompt_for_vimeo_token() -> Result<String, ConfigError> {
        println!(
            "\n{}",
            "Vimeo Personal Access Token Required".to_uppercase()
        );
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("To discover extras from Vimeo, you need a Personal Access Token.");
        println!("\nHow to get your token:");
        println!("  1. Visit: https://developer.vimeo.com/apps");
        println!("  2. Create or select an app");
        println!("  3. Under 'Authentication', generate a Personal Access Token");
        println!("  4. Select the 'public' scope");
        println!("  5. Copy the generated token");
        println!("\nYour token will be saved to config.cfg for future use.");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        print!("Enter your Vimeo Personal Access Token: ");
        io::stdout().flush().map_err(ConfigError::IoError)?;

        let mut token = String::new();
        io::stdin()
            .read_line(&mut token)
            .map_err(ConfigError::IoError)?;

        let token = token.trim().to_string();

        if token.is_empty() {
            return Err(ConfigError::EmptyApiKey);
        }

        Ok(token)
    }

    /// Load or create configuration, optionally prompting for Vimeo token
    ///
    /// Attempts to load config from file. If `require_vimeo_token` is true
    /// and the loaded config doesn't have a Vimeo token, prompts the user
    /// to enter one and saves it.
    pub fn load_or_create_with_vimeo(require_vimeo_token: bool) -> Result<Self, ConfigError> {
        let config_path = Self::default_path();
        let mut config = Self::load_or_create()?;

        if require_vimeo_token && config.vimeo_access_token.is_none() {
            log::info!("Vimeo token required but not found in config");
            let token = Self::prompt_for_vimeo_token()?;
            config.vimeo_access_token = Some(token);

            config.save(&config_path)?;
            println!("\n✓ Configuration updated and saved to {:?}", config_path);
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.cfg");

        let config = Config {
            tmdb_api_key: "test_key_12345".to_string(),
            tvdb_api_key: None,
            cookies_from_browser: None,
            vimeo_access_token: None,
        };

        // Save config
        config.save(&config_path).unwrap();

        // Load config
        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.tmdb_api_key, "test_key_12345");
        assert_eq!(loaded.tvdb_api_key, None);
        assert_eq!(loaded.vimeo_access_token, None);
    }

    #[test]
    fn test_config_load_nonexistent() {
        let result = Config::load(Path::new("/nonexistent/config.cfg"));
        assert!(matches!(result, Err(ConfigError::ReadError(_, _))));
    }

    #[test]
    fn test_config_load_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.cfg");

        // Write invalid JSON
        fs::write(&config_path, "not valid json").unwrap();

        let result = Config::load(&config_path);
        assert!(matches!(result, Err(ConfigError::ParseError(_, _))));
    }

    #[test]
    fn test_config_default_path() {
        let path = Config::default_path();
        assert_eq!(path, PathBuf::from("config.cfg"));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            tmdb_api_key: "test_key".to_string(),
            tvdb_api_key: None,
            cookies_from_browser: None,
            vimeo_access_token: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("tmdb_api_key"));
        assert!(json.contains("test_key"));
    }

    #[test]
    fn test_config_deserialization() {
        let json = r#"{"tmdb_api_key":"my_key"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.tmdb_api_key, "my_key");
        assert_eq!(config.tvdb_api_key, None);
    }

    #[test]
    fn test_config_with_tvdb_key_serialization() {
        let config = Config {
            tmdb_api_key: "tmdb_key".to_string(),
            tvdb_api_key: Some("tvdb_key".to_string()),
            cookies_from_browser: None,
            vimeo_access_token: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("tmdb_api_key"));
        assert!(json.contains("tvdb_api_key"));
        assert!(json.contains("tmdb_key"));
        assert!(json.contains("tvdb_key"));
    }

    #[test]
    fn test_config_with_tvdb_key_deserialization() {
        let json = r#"{"tmdb_api_key":"my_tmdb_key","tvdb_api_key":"my_tvdb_key"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.tmdb_api_key, "my_tmdb_key");
        assert_eq!(config.tvdb_api_key, Some("my_tvdb_key".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn test_config_save_sets_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.cfg");

        let config = Config {
            tmdb_api_key: "test_key".to_string(),
            tvdb_api_key: None,
            cookies_from_browser: None,
            vimeo_access_token: None,
        };

        config
            .save(&config_path)
            .expect("save should succeed on a writable temp path");

        let metadata = fs::metadata(&config_path).expect("metadata should be readable after save");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Config file should have 600 permissions on Unix"
        );
    }

    #[test]
    fn test_config_vimeo_token_serialization() {
        let config = Config {
            tmdb_api_key: "tmdb_key".to_string(),
            tvdb_api_key: None,
            cookies_from_browser: None,
            vimeo_access_token: Some("pat_abc123".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("vimeo_access_token"));
        assert!(json.contains("pat_abc123"));
    }

    #[test]
    fn test_config_vimeo_token_deserialization() {
        let json = r#"{"tmdb_api_key":"key","vimeo_access_token":"pat_abc123"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.vimeo_access_token, Some("pat_abc123".to_string()));
    }

    #[test]
    fn test_config_vimeo_token_default_none() {
        let json = r#"{"tmdb_api_key":"key"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.vimeo_access_token, None);
    }

    #[test]
    fn test_config_save_and_load_with_vimeo_token() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.cfg");

        let config = Config {
            tmdb_api_key: "tmdb_key".to_string(),
            tvdb_api_key: None,
            cookies_from_browser: None,
            vimeo_access_token: Some("pat_xyz".to_string()),
        };

        config.save(&config_path).unwrap();
        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.vimeo_access_token, Some("pat_xyz".to_string()));
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: tvdb-specials, Property 1: Config Serialization Round-Trip
    // Validates: Requirements 1.6
    proptest! {
        #[test]
        fn prop_config_serialization_round_trip(
            tmdb_key in "[a-zA-Z0-9]{10,50}",
            tvdb_key in proptest::option::of("[a-zA-Z0-9]{10,50}"),
            vimeo_key in proptest::option::of("[a-zA-Z0-9]{10,50}")
        ) {
            let config = Config {
                tmdb_api_key: tmdb_key.clone(),
                tvdb_api_key: tvdb_key.clone(),
                cookies_from_browser: None,
                vimeo_access_token: vimeo_key.clone(),
            };

            // Serialize to JSON
            let json = serde_json::to_string(&config).unwrap();

            // Deserialize from JSON
            let deserialized: Config = serde_json::from_str(&json).unwrap();

            // Verify round-trip preserves all key fields
            prop_assert_eq!(&config.tmdb_api_key, &deserialized.tmdb_api_key);
            prop_assert_eq!(&config.tvdb_api_key, &deserialized.tvdb_api_key);
            prop_assert_eq!(&config.vimeo_access_token, &deserialized.vimeo_access_token);
        }
    }
}
