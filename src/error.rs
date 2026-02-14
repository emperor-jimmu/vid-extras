// Error types module - centralized error definitions using thiserror

#![allow(dead_code)]

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
