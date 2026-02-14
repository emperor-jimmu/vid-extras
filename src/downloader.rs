// Downloader module - handles video downloads using yt-dlp

use crate::discovery::VideoSource;
use crate::error::DownloadError;
use std::path::PathBuf;

/// Result of a download operation
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub source: VideoSource,
    pub local_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

/// Downloader for fetching videos
pub struct Downloader {
    temp_base: PathBuf,
}

impl Downloader {
    pub fn new(temp_base: PathBuf) -> Self {
        Self { temp_base }
    }
}
