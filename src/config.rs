// Configuration file management

use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// TMDB API key for content discovery
    pub tmdb_api_key: String,
    /// TheTVDB API key for Season 0 specials discovery
    #[serde(default)]
    pub tvdb_api_key: Option<String>,
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
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let contents = serde_json::to_string_pretty(self).map_err(ConfigError::SerializeError)?;

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
        };

        // Save config
        config.save(&config_path).unwrap();

        // Load config
        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.tmdb_api_key, "test_key_12345");
        assert_eq!(loaded.tvdb_api_key, None);
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
            tvdb_key in proptest::option::of("[a-zA-Z0-9]{10,50}")
        ) {
            let config = Config {
                tmdb_api_key: tmdb_key.clone(),
                tvdb_api_key: tvdb_key.clone(),
            };

            // Serialize to JSON
            let json = serde_json::to_string(&config).unwrap();

            // Deserialize from JSON
            let deserialized: Config = serde_json::from_str(&json).unwrap();

            // Verify round-trip preserves both fields
            prop_assert_eq!(&config.tmdb_api_key, &deserialized.tmdb_api_key);
            prop_assert_eq!(&config.tvdb_api_key, &deserialized.tvdb_api_key);
        }
    }
}
