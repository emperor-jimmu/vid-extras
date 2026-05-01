use extras_fetcher::cli::{display_banner, display_config, parse_args};
use extras_fetcher::config::Config;
use extras_fetcher::error::ValidationError;
use extras_fetcher::json_output;
use extras_fetcher::models::Source;
use extras_fetcher::orchestrator::{
    DiscoveryConfig, Orchestrator, OrchestratorConfig, SeriesConfig,
};
use extras_fetcher::output::display_summary;
use extras_fetcher::tui::TuiState;
use extras_fetcher::validation::Validator;
use std::sync::Arc;

fn init_logging(config: &extras_fetcher::cli::CliConfig) {
    use std::io::Write;

    let log_file_path = std::env::temp_dir().join("extras_fetcher_tui.log");
    if config.tui {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_file_path);
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
            .format(move |buf, record| {
                let msg = format!("[{}] {}\n", record.level(), record.args());
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file_path)
                {
                    let _ = f.write_all(msg.as_bytes());
                }
                writeln!(buf, "[{}] {}", record.level(), record.args())
            })
            .init();
    } else if config.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }
}

fn validate_dependencies() -> Result<String, ValidationError> {
    let validator = Validator::new();
    validator.validate_dependencies()
}

fn handle_validation_error(e: &ValidationError) {
    eprintln!("\n✗ Dependency validation failed");
    match e {
        ValidationError::MissingBinary(name) => {
            eprintln!("  Missing binary: {}", name);
            match name.as_str() {
                "yt-dlp" => {
                    eprintln!("\n  Install yt-dlp:");
                    eprintln!("    https://github.com/yt-dlp/yt-dlp#installation");
                }
                "ffmpeg" => {
                    eprintln!("\n  Install ffmpeg:");
                    eprintln!("    https://ffmpeg.org/download.html");
                }
                _ => eprintln!("\n  Please install {} and ensure it is in PATH", name),
            }
        }
        ValidationError::UnsupportedCodec => {
            eprintln!("  ffmpeg is missing HEVC/x265 codec support");
            eprintln!("\n  Rebuild or reinstall ffmpeg with libx265 support");
            eprintln!("    https://ffmpeg.org/download.html");
        }
        ValidationError::MissingApiKey(key) => {
            eprintln!("  Missing API key: {}", key);
            eprintln!("\n  Configure your TMDB API key in config.cfg");
            eprintln!("    (You will be prompted to enter it if config.cfg doesn't exist)");
            eprintln!("    Get a key: https://www.themoviedb.org/settings/api");
        }
    }
    std::process::exit(1);
}

fn load_tvdb_config(specials: bool) -> (Option<String>, Option<String>) {
    if specials {
        match Config::load_or_create_with_tvdb(true) {
            Ok(cfg) => (cfg.tvdb_api_key, cfg.cookies_from_browser),
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
        let cfg_cookies = Config::load(&Config::default_path())
            .ok()
            .and_then(|c| c.cookies_from_browser);
        (None, cfg_cookies)
    }
}

fn load_vimeo_token(sources: &[Source]) -> String {
    if sources.contains(&Source::Vimeo) {
        match Config::load_or_create_with_vimeo(true) {
            Ok(cfg) => cfg.vimeo_access_token.unwrap_or_default(),
            Err(e) => {
                eprintln!("\n✗ Failed to load Vimeo Personal Access Token");
                eprintln!("  Error: {}", e);
                eprintln!("\nPlease ensure:");
                eprintln!("  • A Vimeo Personal Access Token is configured in config.cfg");
                eprintln!("    (You will be prompted to enter it)");
                eprintln!("\nHow to get a Vimeo Personal Access Token:");
                eprintln!("  1. Visit: https://developer.vimeo.com/apps");
                eprintln!("  2. Create or select an app");
                eprintln!("  3. Under 'Authentication', generate a Personal Access Token");
                eprintln!("  4. Select the 'public' scope");
                std::process::exit(1);
            }
        }
    } else {
        String::new()
    }
}

fn create_orchestrator(
    config: &extras_fetcher::cli::CliConfig,
    tmdb_api_key: String,
    tvdb_api_key: Option<String>,
    cookies_from_browser: Option<String>,
    vimeo_access_token: String,
) -> Orchestrator {
    Orchestrator::new(OrchestratorConfig {
        root_dir: config.root_directory.clone(),
        tmdb_api_key,
        tvdb_api_key,
        force: config.force,
        concurrency: config.concurrency,
        single: config.single,
        processing_mode: config.processing_mode,
        series: SeriesConfig {
            season_extras: config.season_extras,
            specials: config.specials,
            specials_folder: config.specials_folder.clone(),
        },
        discovery: DiscoveryConfig {
            sources: config.sources.clone(),
            cookies_from_browser,
            dry_run: config.dry_run,
            vimeo_access_token,
        },
    })
    .expect("Failed to create orchestrator")
}

fn handle_orchestrator_run_error(e: &extras_fetcher::error::OrchestratorError) {
    eprintln!("\n✗ Processing failed");
    eprintln!("  Error: {}", e);
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    let config = match parse_args() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("\nRun with --help for usage information");
            std::process::exit(1);
        }
    };

    if config.json_progress {
        json_output::set_json_progress_enabled(true);
    }

    init_logging(&config);
    display_banner();

    let tmdb_api_key = match validate_dependencies() {
        Ok(api_key) => {
            log::info!("All dependencies validated successfully");
            api_key
        }
        Err(e) => {
            handle_validation_error(&e);
            unreachable!();
        }
    };

    let (tvdb_api_key, config_cookies) = load_tvdb_config(config.specials);
    let cookies_from_browser = config.cookies_from_browser.clone().or(config_cookies);

    if let Some(ref browser) = cookies_from_browser {
        log::info!("Cookie authentication: {} browser", browser);
    }

    let vimeo_access_token = load_vimeo_token(&config.sources);

    let mut display = config.clone();
    display.cookies_from_browser = cookies_from_browser.clone();
    display_config(&display);

    let mut orchestrator = create_orchestrator(
        &config,
        tmdb_api_key,
        tvdb_api_key,
        cookies_from_browser,
        vimeo_access_token,
    );

    if config.tui {
        extras_fetcher::set_tui_active(true);
        let tui_state = Arc::new(TuiState::new());
        orchestrator = orchestrator.with_tui(tui_state);
    }

    log::info!("Starting processing pipeline");
    let summary = match orchestrator.run().await {
        Ok(sum) => {
            log::info!("Processing pipeline completed");
            sum
        }
        Err(e) => {
            handle_orchestrator_run_error(&e);
            unreachable!();
        }
    };

    display_summary(&summary);

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
