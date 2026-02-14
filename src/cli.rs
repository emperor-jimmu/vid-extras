// CLI module - handles command-line argument parsing and configuration

#[allow(unused_imports)]
use crate::error::CliError;
use std::path::PathBuf;

/// Source mode for content discovery
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMode {
    All,
    YoutubeOnly,
}

/// CLI configuration
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub root_directory: PathBuf,
    pub force: bool,
    pub mode: SourceMode,
    pub concurrency: usize,
    pub verbose: bool,
}
