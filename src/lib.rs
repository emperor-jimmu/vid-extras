// Library module declarations
pub mod cli;
pub mod config;
pub mod converter;
pub mod deduplication;
pub mod discovery;
pub mod downloader;
pub mod error;
pub mod json_output;
pub mod models;
pub mod orchestrator;
pub mod organizer;
pub mod output;
pub mod scanner;
pub mod validation;
pub use json_output::ProgressEvent;
