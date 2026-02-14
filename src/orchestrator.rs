// Orchestrator module - coordinates all processing phases

use crate::cli::CliConfig;
use crate::error::OrchestratorError;
use crate::models::MovieEntry;
use crate::scanner::Scanner;

/// Summary of processing results
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ProcessingSummary {
    pub total_movies: usize,
    pub successful: usize,
    pub failed: usize,
    pub total_downloads: usize,
    pub total_conversions: usize,
}

/// Result of processing a single movie
#[allow(dead_code)]
#[derive(Debug)]
pub struct MovieResult {
    pub movie: MovieEntry,
    pub success: bool,
    pub phase: String,
}

/// Main orchestrator for the processing pipeline
#[allow(dead_code)]
pub struct Orchestrator {
    #[allow(dead_code)]
    config: CliConfig,
    #[allow(dead_code)]
    scanner: Scanner,
}

impl Orchestrator {
    #[allow(dead_code)]
    pub fn new(config: CliConfig) -> Result<Self, OrchestratorError> {
        let scanner = Scanner::new(config.root_directory.clone(), config.force);

        Ok(Self { config, scanner })
    }
}
