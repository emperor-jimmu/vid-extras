// Module declarations
mod cli;
mod converter;
mod discovery;
mod downloader;
mod error;
mod models;
mod orchestrator;
mod organizer;
mod scanner;
mod validation;

use scanner::Scanner;
use std::env;
use std::path::PathBuf;
use validation::Validator;

fn main() {
    println!("extras_fetcher v0.1.0 - Jellyfin movie extras automation tool");
    println!();

    // Validate dependencies first
    let validator = Validator::new();
    match validator.validate_dependencies() {
        Ok(api_key) => {
            println!("✓ All dependencies validated");
            println!(
                "✓ TMDB API key configured: {}...",
                &api_key[..8.min(api_key.len())]
            );
        }
        Err(e) => {
            eprintln!("✗ Dependency validation failed: {}", e);
            eprintln!("\nPlease ensure:");
            eprintln!("  - yt-dlp is installed and in PATH");
            eprintln!("  - ffmpeg is installed with HEVC support");
            eprintln!("  - TMDB_API_KEY environment variable is set");
            std::process::exit(1);
        }
    }

    // Get root directory from command line args or use current directory
    let root_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    println!("\nScanning library: {:?}", root_dir);

    // Scan for movies
    let scanner = Scanner::new(root_dir, false);
    match scanner.scan() {
        Ok(movies) => {
            println!("✓ Found {} movie(s) to process", movies.len());
            for movie in movies.iter().take(5) {
                println!("  - {}", movie);
            }
            if movies.len() > 5 {
                println!("  ... and {} more", movies.len() - 5);
            }
        }
        Err(e) => {
            eprintln!("✗ Scan failed: {}", e);
            std::process::exit(1);
        }
    }

    println!("\nNote: Full processing pipeline not yet implemented.");
    println!("Core infrastructure (Scanner, Validation) is complete and tested.");
}
