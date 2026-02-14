// Scanner module - handles directory traversal and movie discovery

use crate::error::ScanError;
use std::path::PathBuf;

/// Represents a movie entry found during scanning
#[derive(Debug, Clone)]
pub struct MovieEntry {
    pub path: PathBuf,
    pub title: String,
    pub year: u16,
    pub has_done_marker: bool,
}

/// Scanner for traversing movie library directories
pub struct Scanner {
    root_dir: PathBuf,
    force: bool,
}

impl Scanner {
    pub fn new(root_dir: PathBuf, force: bool) -> Self {
        Self { root_dir, force }
    }
}
