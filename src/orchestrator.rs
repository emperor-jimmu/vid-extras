// Orchestrator module - coordinates all processing phases

use crate::cli::CliConfig;
use crate::converter::Converter;
use crate::discovery::DiscoveryOrchestrator;
use crate::downloader::Downloader;
use crate::error::OrchestratorError;
use crate::scanner::{MovieEntry, Scanner};

/// Summary of processing results
#[derive(Debug, Clone)]
pub struct ProcessingSummary {
    pub total_movies: usize,
    pub successful: usize,
    pub failed: usize,
    pub total_downloads: usize,
    pub total_conversions: usize,
}

/// Result of processing a single movie
#[derive(Debug)]
pub struct MovieResult {
    pub movie: MovieEntry,
    pub success: bool,
    pub phase: String,
}

/// Main orchestrator for the processing pipeline
pub struct Orchestrator {
    config: CliConfig,
    scanner: Scanner,
}

impl Orchestrator {
    pub fn new(config: CliConfig) -> Result<Self, OrchestratorError> {
        let scanner = Scanner::new(config.root_directory.clone(), config.force);
        
        Ok(Self { config, scanner })
    }
}
