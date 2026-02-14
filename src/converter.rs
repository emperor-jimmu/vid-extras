// Converter module - handles video conversion to x265 format

use crate::downloader::DownloadResult;
use crate::error::ConversionError;
use std::path::PathBuf;

/// Hardware acceleration type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareAccel {
    Nvenc,    // NVIDIA
    Qsv,      // Intel Quick Sync
    Software, // CPU-based
}

/// Result of a conversion operation
#[derive(Debug, Clone)]
pub struct ConversionResult {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

/// Converter for video format conversion
pub struct Converter {
    hw_accel: HardwareAccel,
}

impl Converter {
    pub fn new() -> Self {
        Self {
            hw_accel: Self::detect_hardware_accel(),
        }
    }

    fn detect_hardware_accel() -> HardwareAccel {
        // Placeholder - will be implemented in later tasks
        HardwareAccel::Software
    }
}
