use extras_fetcher::cli::{display_banner, display_config, parse_args};
use extras_fetcher::config::Config;
use extras_fetcher::orchestrator::Orchestrator;
use extras_fetcher::output::display_summary;
use extras_fetcher::validation::Validator;

/// Main entry point for extras_fetcher
///
/// Coordinates the complete processing pipeline:
/// 1. Parse CLI arguments and validate configuration
/// 2. Initialize logging based on verbosity flag
/// 3. Validate system dependencies (yt-dlp, ffmpeg, TMDB API key)
/// 4. Create and execute the orchestrator
/// 5. Display final processing summary
///
/// Requirements: 11.1-11.5, 10.5
#[tokio::main]
async fn main() {
    // Parse command-line arguments
    let config = match parse_args() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("\nRun with --help for usage information");
            std::process::exit(1);
        }
    };

    // Initialize logging based on verbose flag
    // Requirements: 13.8
    if config.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    // Display banner and configuration
    display_banner();
    display_config(&config);

    // Validate dependencies before processing
    // Requirements: 11.1, 11.2, 11.3, 11.4, 11.5
    let validator = Validator::new();
    let tmdb_api_key = match validator.validate_dependencies() {
        Ok(api_key) => {
            log::info!("All dependencies validated successfully");
            api_key
        }
        Err(e) => {
            // Fatal error: missing dependencies
            // Requirements: 10.5, 11.5
            eprintln!("\n✗ Dependency validation failed");
            eprintln!("  Error: {}", e);
            eprintln!("\nPlease ensure:");
            eprintln!("  • yt-dlp is installed and available in PATH");
            eprintln!("  • ffmpeg is installed with HEVC/x265 support");
            eprintln!("  • TMDB API key is configured in config.cfg");
            eprintln!("    (You will be prompted to enter it if config.cfg doesn't exist)");
            eprintln!("\nInstallation instructions:");
            eprintln!("  yt-dlp:  https://github.com/yt-dlp/yt-dlp#installation");
            eprintln!("  ffmpeg:  https://ffmpeg.org/download.html");
            eprintln!("  TMDB:    https://www.themoviedb.org/settings/api");
            std::process::exit(1);
        }
    };

    // Load configuration with TVDB key if specials are enabled
    // Requirements: 1.1, 8.1
    let (tvdb_api_key, config_cookies) = if config.specials {
        log::info!("Season 0 specials enabled, loading TVDB configuration");
        match Config::load_or_create_with_tvdb(true) {
            Ok(cfg) => {
                log::info!("TVDB API key loaded successfully");
                (cfg.tvdb_api_key, cfg.cookies_from_browser)
            }
            Err(e) => {
                eprintln!("\n✗ Failed to load TVDB API key");
                eprintln!("  Error: {}", e);
                eprintln!("\nPlease ensure:");
                eprintln!("  • TheTVDB API key is configured in config.cfg");
                eprintln!("    (You will be prompted to enter it)");
                eprintln!("\nHow to get a TheTVDB API key:");
                eprintln!("  1. Visit: https://www.thetvdb.com/api-information");
                eprintln!("  2. Sign up for a free account");
                eprintln!("  3. Request an API key from your account settings");
                std::process::exit(1);
            }
        }
    } else {
        // Still load config to get cookies_from_browser fallback
        let cfg_cookies = Config::load(&Config::default_path())
            .ok()
            .and_then(|c| c.cookies_from_browser);
        (None, cfg_cookies)
    };

    // CLI flag takes priority over config file for cookie auth
    let cookies_from_browser = config.cookies_from_browser.or(config_cookies);

    // Create orchestrator with validated configuration
    let orchestrator = match Orchestrator::new(
        config.root_directory.clone(),
        tmdb_api_key,
        tvdb_api_key,
        config.mode.to_models_source_mode(),
        config.force,
        config.concurrency,
        config.single,
        config.processing_mode,
        config.season_extras,
        config.specials,
        config.specials_folder,
        cookies_from_browser,
    ) {
        Ok(orch) => orch,
        Err(e) => {
            // Fatal error: orchestrator initialization failed
            // Requirements: 10.5
            eprintln!("\n✗ Initialization failed");
            eprintln!("  Error: {}", e);
            std::process::exit(1);
        }
    };

    // Execute the orchestrator
    log::info!("Starting processing pipeline");
    let summary = match orchestrator.run().await {
        Ok(sum) => {
            log::info!("Processing pipeline completed");
            sum
        }
        Err(e) => {
            // Fatal error: orchestrator execution failed
            // Requirements: 10.5
            eprintln!("\n✗ Processing failed");
            eprintln!("  Error: {}", e);
            std::process::exit(1);
        }
    };

    // Display final summary
    // Requirements: 13.6
    display_summary(&summary);

    // Exit with appropriate code
    if summary.failed_movies > 0 || summary.failed_series > 0 {
        log::warn!(
            "Processing completed with {} movie failure(s) and {} series failure(s)",
            summary.failed_movies,
            summary.failed_series
        );
        std::process::exit(1);
    } else {
        log::info!("All items processed successfully");
        std::process::exit(0);
    }
}
