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
pub mod tui;
pub mod validation;
pub use json_output::ProgressEvent;

use std::sync::atomic::AtomicBool;

static TUI_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_tui_active(active: bool) {
    TUI_ACTIVE.store(active, std::sync::atomic::Ordering::SeqCst);
}

pub fn is_tui_active() -> bool {
    TUI_ACTIVE.load(std::sync::atomic::Ordering::SeqCst)
}
