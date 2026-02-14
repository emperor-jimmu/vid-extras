// CLI module - handles command-line argument parsing and configuration

use crate::error::CliError;
use std::path::PathBuf;

/// Source mode for content discovery
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    All,
    YoutubeOnly,
}

/// CLI configuration
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub root_directory: PathBuf,
    pub force: bool,
    pub mode: SourceMode,
    pub concurrency: usize,
    pub verbose: bool,
}
