// Error types module - centralized error definitions using thiserror

use thiserror::Error;

/// CLI-related errors
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Invalid root directory: {0}")]
    InvalidRootDir(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Invalid concurrency: {0}")]
    InvalidConcurrency(String),
}

/// Scanner-related errors
#[derive(Debug, Error)]
pub enum ScanError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid folder name: {0}")]
    InvalidFolderName(String),
}

/// Discovery-related errors
#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum DiscoveryError {
    #[error("API error: {0}")]
    ApiError(String),
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("yt-dlp error: {0}")]
    YtDlpError(String),
    #[error("TVDB authentication failed: {0}")]
    TvdbAuthError(String),
    #[error("TVDB API error: {0}")]
    TvdbApiError(String),
}

/// Download-related errors
#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("yt-dlp failed: {0}")]
    YtDlpFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Conversion-related errors
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("ffmpeg failed: {0}")]
    FfmpegFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Validation-related errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Missing binary: {0}")]
    MissingBinary(String),
    #[error("Missing API key: {0}")]
    MissingApiKey(String),
    #[error("Unsupported codec")]
    UnsupportedCodec,
}

/// Organizer-related errors
#[derive(Debug, Error)]
pub enum OrganizerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to create subdirectory: {0}")]
    SubdirectoryCreation(String),
    #[error("Failed to move file: {0}")]
    FileMove(String),
}

/// Configuration-related errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file {0}: {1}")]
    ReadError(std::path::PathBuf, std::io::Error),
    #[error("Failed to parse config file {0}: {1}")]
    ParseError(std::path::PathBuf, serde_json::Error),
    #[error("Failed to write config file {0}: {1}")]
    WriteError(std::path::PathBuf, std::io::Error),
    #[error("Failed to serialize config: {0}")]
    SerializeError(serde_json::Error),
    #[error("IO error: {0}")]
    IoError(std::io::Error),
    #[error("API key cannot be empty")]
    EmptyApiKey,
}

/// Orchestrator-related errors
#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("Initialization error: {0}")]
    Init(String),
    #[error("Processing error: {0}")]
    Processing(String),
}

/// Movie processing result errors
#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("Scan error: {0}")]
    Scan(#[from] ScanError),
    #[error("Discovery error: {0}")]
    Discovery(#[from] DiscoveryError),
    #[error("Download error: {0}")]
    Download(#[from] DownloadError),
    #[error("Conversion error: {0}")]
    Conversion(#[from] ConversionError),
    #[error("Organizer error: {0}")]
    Organizer(#[from] OrganizerError),
}

/// Series scanning errors
#[derive(Debug, Error)]
pub enum SeriesScanError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid series folder structure: {0}")]
    InvalidStructure(String),
    #[error("Failed to parse series name: {0}")]
    ParseError(String),
}

/// Series discovery errors
#[derive(Debug, Error)]
pub enum SeriesDiscoveryError {
    #[error("TMDB API error: {0}")]
    TmdbApi(String),
    #[error("YouTube search error: {0}")]
    YoutubeSearch(String),
    #[error("Series not found: {0}")]
    NotFound(String),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

/// Series organizer errors
#[derive(Debug, Error)]
pub enum SeriesOrganizerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid season number: {0}")]
    InvalidSeason(u8),
    #[error("File not found: {0}")]
    FileNotFound(std::path::PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_series_scan_error_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error = SeriesScanError::Io(io_error);
        assert!(error.to_string().contains("IO error"));
    }

    #[test]
    fn test_series_scan_error_invalid_structure() {
        let error = SeriesScanError::InvalidStructure("missing season folders".to_string());
        assert_eq!(
            error.to_string(),
            "Invalid series folder structure: missing season folders"
        );
    }

    #[test]
    fn test_series_scan_error_parse_error() {
        let error = SeriesScanError::ParseError("invalid folder name format".to_string());
        assert_eq!(
            error.to_string(),
            "Failed to parse series name: invalid folder name format"
        );
    }

    #[test]
    fn test_series_discovery_error_tmdb_api() {
        let error = SeriesDiscoveryError::TmdbApi("API rate limit exceeded".to_string());
        assert_eq!(error.to_string(), "TMDB API error: API rate limit exceeded");
    }

    #[test]
    fn test_series_discovery_error_youtube_search() {
        let error = SeriesDiscoveryError::YoutubeSearch("yt-dlp not found".to_string());
        assert_eq!(error.to_string(), "YouTube search error: yt-dlp not found");
    }

    #[test]
    fn test_series_discovery_error_not_found() {
        let error = SeriesDiscoveryError::NotFound("Breaking Bad".to_string());
        assert_eq!(error.to_string(), "Series not found: Breaking Bad");
    }

    #[test]
    fn test_series_organizer_error_io() {
        let io_error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let error = SeriesOrganizerError::Io(io_error);
        assert!(error.to_string().contains("IO error"));
    }

    #[test]
    fn test_series_organizer_error_invalid_season() {
        let error = SeriesOrganizerError::InvalidSeason(100);
        assert_eq!(error.to_string(), "Invalid season number: 100");
    }

    #[test]
    fn test_series_organizer_error_file_not_found() {
        let path = std::path::PathBuf::from("/path/to/file.mp4");
        let error = SeriesOrganizerError::FileNotFound(path);
        assert!(error.to_string().contains("File not found"));
    }

    #[test]
    fn test_error_propagation_io_to_series_scan() {
        let io_error = std::io::Error::new(std::io::ErrorKind::Other, "disk error");
        let series_error: SeriesScanError = io_error.into();
        assert!(series_error.to_string().contains("IO error"));
    }

    #[test]
    fn test_error_propagation_io_to_series_organizer() {
        let io_error = std::io::Error::new(std::io::ErrorKind::Other, "disk error");
        let organizer_error: SeriesOrganizerError = io_error.into();
        assert!(organizer_error.to_string().contains("IO error"));
    }

    #[test]
    fn test_error_display_trait_series_scan() {
        let error = SeriesScanError::ParseError("test error".to_string());
        let display_string = format!("{}", error);
        assert_eq!(display_string, "Failed to parse series name: test error");
    }

    #[test]
    fn test_error_display_trait_series_discovery() {
        let error = SeriesDiscoveryError::TmdbApi("test error".to_string());
        let display_string = format!("{}", error);
        assert_eq!(display_string, "TMDB API error: test error");
    }

    #[test]
    fn test_error_display_trait_series_organizer() {
        let error = SeriesOrganizerError::InvalidSeason(50);
        let display_string = format!("{}", error);
        assert_eq!(display_string, "Invalid season number: 50");
    }

    #[test]
    fn test_error_trait_implementation() {
        use std::error::Error;

        let error: Box<dyn Error> = Box::new(SeriesScanError::ParseError("test".to_string()));
        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn test_graceful_degradation_discovery_error() {
        // Simulate graceful degradation: one source fails, others continue
        let tmdb_error = SeriesDiscoveryError::TmdbApi("API down".to_string());
        let youtube_error = SeriesDiscoveryError::YoutubeSearch("network error".to_string());

        // Both errors should be loggable and handleable
        let _ = format!("TMDB failed: {}", tmdb_error);
        let _ = format!("YouTube failed: {}", youtube_error);
    }

    #[test]
    fn test_partial_success_scenario() {
        // Simulate partial success: some operations succeed, some fail
        let mut results = Vec::new();

        // Success case
        results.push(Ok::<(), SeriesOrganizerError>(()));

        // Failure case
        results.push(Err(SeriesOrganizerError::InvalidSeason(100)));

        // Success case
        results.push(Ok::<(), SeriesOrganizerError>(()));

        // Count successes and failures
        let successes = results.iter().filter(|r| r.is_ok()).count();
        let failures = results.iter().filter(|r| r.is_err()).count();

        assert_eq!(successes, 2);
        assert_eq!(failures, 1);
    }

    #[test]
    fn test_error_context_preservation() {
        // Verify that error context is preserved through the error chain
        let original_message = "Series 'Breaking Bad' not found in TMDB";
        let error = SeriesDiscoveryError::NotFound(original_message.to_string());

        let error_string = error.to_string();
        assert!(error_string.contains("Breaking Bad"));
        assert!(error_string.contains("not found"));
    }

    #[test]
    fn test_series_scan_error_debug_format() {
        let error = SeriesScanError::ParseError("invalid format".to_string());
        let debug_string = format!("{:?}", error);
        assert!(debug_string.contains("ParseError"));
        assert!(debug_string.contains("invalid format"));
    }

    #[test]
    fn test_series_discovery_error_debug_format() {
        let error = SeriesDiscoveryError::TmdbApi("timeout".to_string());
        let debug_string = format!("{:?}", error);
        assert!(debug_string.contains("TmdbApi"));
        assert!(debug_string.contains("timeout"));
    }

    #[test]
    fn test_series_organizer_error_debug_format() {
        let path = std::path::PathBuf::from("/test/path");
        let error = SeriesOrganizerError::FileNotFound(path);
        let debug_string = format!("{:?}", error);
        assert!(debug_string.contains("FileNotFound"));
    }
}
