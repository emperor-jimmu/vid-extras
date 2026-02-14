// Organizer module - handles file organization into Jellyfin directories

use crate::converter::ConversionResult;
use crate::error::OrganizerError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Done marker file content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneMarker {
    pub finished_at: String, // ISO 8601 timestamp
    pub version: String,
}

/// Organizer for moving files to target directories
pub struct Organizer {
    movie_path: PathBuf,
}

impl Organizer {
    pub fn new(movie_path: PathBuf) -> Self {
        Self { movie_path }
    }
}
