// Organizer module - handles file organization into Jellyfin directories

#[allow(unused_imports)]
use crate::converter::ConversionResult;
#[allow(unused_imports)]
use crate::error::OrganizerError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Done marker file content
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoneMarker {
    pub finished_at: String, // ISO 8601 timestamp
    pub version: String,
}

/// Organizer for moving files to target directories
#[allow(dead_code)]
pub struct Organizer {
    #[allow(dead_code)]
    movie_path: PathBuf,
}

impl Organizer {
    #[allow(dead_code)]
    pub fn new(movie_path: PathBuf) -> Self {
        Self { movie_path }
    }
}
