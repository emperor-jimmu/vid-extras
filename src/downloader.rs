// Downloader module - handles video downloads using yt-dlp

use crate::discovery::VideoSource;
#[allow(unused_imports)]
use crate::error::DownloadError;
use std::path::PathBuf;

/// Result of a download operation
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub source: VideoSource,
    pub local_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

/// Downloader for fetching videos
#[allow(dead_code)]
pub struct Downloader {
    #[allow(dead_code)]
    temp_base: PathBuf,
}

impl Downloader {
    #[allow(dead_code)]
    pub fn new(temp_base: PathBuf) -> Self {
        Self { temp_base }
    }
}
