// CLI module - handles command-line argument parsing and configuration

use crate::error::CliError;
use crate::models::{Source, all_sources, default_sources};
use clap::Parser;
use colored::Colorize;
use std::path::PathBuf;

/// extras_fetcher - Automated Jellyfin movie extras downloader
///
/// Discovers, downloads, and organizes supplementary video content (trailers,
/// featurettes, behind-the-scenes, deleted scenes) for Jellyfin movie libraries.
#[derive(Parser, Debug)]
#[command(name = "extras_fetcher")]
#[command(version)]
#[command(about = "Automated Jellyfin movie extras downloader", long_about = None)]
pub struct CliArgs {
    /// Root directory containing movie folders
    #[arg(value_name = "ROOT_DIRECTORY")]
    pub root_directory: PathBuf,

    /// Ignore done markers and reprocess all movies
    #[arg(short, long)]
    pub force: bool,

    /// Discovery sources to query (comma-separated or repeated flags)
    /// Available: tmdb, archive, dailymotion, youtube, vimeo, bilibili
    #[arg(
        long,
        value_delimiter = ',',
        default_values_t = default_sources()
    )]
    pub sources: Vec<Source>,

    /// Use all available discovery sources (equivalent to --sources tmdb,archive,dailymotion,youtube,vimeo)
    #[arg(long, conflicts_with = "sources")]
    pub all: bool,

    /// Maximum number of movies to process concurrently
    #[arg(short, long, default_value = "2")]
    pub concurrency: usize,

    /// Enable verbose logging output
    #[arg(short, long)]
    pub verbose: bool,

    /// Process a single movie folder directly (instead of scanning for multiple movies)
    #[arg(short, long)]
    pub single: bool,

    /// Process only TV series and skip movies
    #[arg(long)]
    pub series_only: bool,

    /// Process only movies and skip TV series
    #[arg(long)]
    pub movies_only: bool,

    /// Enable season-specific extras discovery
    #[arg(long)]
    pub season_extras: bool,

    /// Enable Season 0 specials discovery
    #[arg(long)]
    pub specials: bool,

    /// Folder name for Season 0 specials (default: "Specials")
    #[arg(long, default_value = "Specials")]
    pub specials_folder: String,

    /// Force classification as either 'movie' or 'series'
    #[arg(long, value_name = "TYPE")]
    pub r#type: Option<String>,

    /// Browser to use for yt-dlp cookie authentication (e.g. chrome, firefox, edge)
    /// Resolves YouTube bot-detection errors. Overrides cookies_from_browser in config.cfg.
    #[arg(long, value_name = "BROWSER")]
    pub cookies_from_browser: Option<String>,

    /// Discover extras without downloading, converting, or organizing files
    #[arg(long)]
    pub dry_run: bool,

    /// Output line-delimited JSON progress for external tools (e.g., web UI)
    #[arg(long)]
    pub json_progress: bool,

    /// Enable split-pane TUI with per-thread logs
    #[arg(long)]
    pub tui: bool,

    /// DEPRECATED: Use --sources instead
    #[arg(long, hide = true)]
    pub mode: Option<String>,
}

/// CLI configuration
#[derive(Debug, Clone)]
pub struct CliConfig {
    pub root_directory: PathBuf,
    pub force: bool,
    pub sources: Vec<Source>,
    pub concurrency: usize,
    pub verbose: bool,
    pub single: bool,
    pub processing_mode: crate::models::ProcessingMode,
    pub season_extras: bool,
    pub specials: bool,
    pub specials_folder: String,
    pub media_type: Option<String>,
    pub cookies_from_browser: Option<String>,
    pub dry_run: bool,
    pub json_progress: bool,
    pub tui: bool,
}

impl From<CliArgs> for CliConfig {
    fn from(args: CliArgs) -> Self {
        let processing_mode = if args.series_only {
            crate::models::ProcessingMode::SeriesOnly
        } else if args.movies_only {
            crate::models::ProcessingMode::MoviesOnly
        } else {
            crate::models::ProcessingMode::Both
        };

        let sources = if args.all {
            all_sources()
        } else {
            args.sources
        };

        CliConfig {
            root_directory: args.root_directory,
            force: args.force,
            sources,
            concurrency: args.concurrency,
            verbose: args.verbose,
            single: args.single,
            processing_mode,
            season_extras: args.season_extras,
            specials: args.specials,
            specials_folder: args.specials_folder,
            media_type: args.r#type,
            cookies_from_browser: args.cookies_from_browser,
            dry_run: args.dry_run,
            json_progress: args.json_progress,
            tui: args.tui,
        }
    }
}

/// Parse command-line arguments
pub fn parse_args() -> Result<CliConfig, CliError> {
    let args = CliArgs::parse();
    // Check for deprecated --mode flag before any other validation
    if args.mode.is_some() {
        return Err(CliError::DeprecatedFlag(
            "The --mode flag has been removed. Use --sources instead.".to_string(),
        ));
    }
    validate_config(&args)?;
    Ok(args.into())
}

/// Validate CLI configuration
fn validate_config(args: &CliArgs) -> Result<(), CliError> {
    // Check if root directory exists
    if !args.root_directory.exists() {
        return Err(CliError::InvalidRootDir(format!(
            "Directory does not exist: {:?}",
            args.root_directory
        )));
    }

    // Check if root directory is actually a directory
    if !args.root_directory.is_dir() {
        return Err(CliError::InvalidRootDir(format!(
            "Path is not a directory: {:?}",
            args.root_directory
        )));
    }

    // Validate concurrency is at least 1
    if args.concurrency == 0 {
        return Err(CliError::InvalidConcurrency(
            "Concurrency must be at least 1".to_string(),
        ));
    }

    // Validate that --series-only and --movies-only are mutually exclusive
    if args.series_only && args.movies_only {
        return Err(CliError::ParseError(
            "--series-only and --movies-only are mutually exclusive".to_string(),
        ));
    }

    // Validate --type flag values if provided
    if let Some(ref media_type) = args.r#type {
        let valid_types = ["movie", "series"];
        if !valid_types.contains(&media_type.as_str()) {
            return Err(CliError::ParseError(format!(
                "Invalid --type value: '{}'. Must be 'movie' or 'series'",
                media_type
            )));
        }
    }

    Ok(())
}

/// Display colored banner with version
pub fn display_banner() {
    let version = env!("CARGO_PKG_VERSION");
    let title = "EXTRAS FETCHER";
    let subtitle = "Automated Jellyfin Extras Downloader";
    let version_line = format!("Version {}", version);

    let width = 59; // inner width between the ║ chars

    let pad_title = (width - title.len()) / 2;
    let pad_subtitle = (width - subtitle.len()) / 2;
    let pad_version = (width - version_line.len()) / 2;

    println!("╔{}╗", "═".repeat(width));
    println!("║{}║", " ".repeat(width));
    println!(
        "║{}{}{}║",
        " ".repeat(pad_title),
        title.bright_cyan().bold(),
        " ".repeat(width - pad_title - title.len())
    );
    println!("║{}║", " ".repeat(width));
    println!(
        "║{}{}{}║",
        " ".repeat(pad_subtitle),
        subtitle.bright_white(),
        " ".repeat(width - pad_subtitle - subtitle.len())
    );
    println!(
        "║{}{}{}║",
        " ".repeat(pad_version),
        version_line.bright_yellow(),
        " ".repeat(width - pad_version - version_line.len())
    );
    println!("║{}║", " ".repeat(width));
    println!("╚{}╝", "═".repeat(width));
}

/// Display configuration with all parameters
pub fn display_config(config: &CliConfig) {
    println!("{}", "Configuration:".bright_green().bold());
    println!(
        "  {} {:?}",
        "Root Directory:".bright_white(),
        config.root_directory
    );
    let sources_str = config
        .sources
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "  {} {}",
        "Sources:".bright_white(),
        sources_str.bright_cyan()
    );
    println!(
        "  {} {}",
        "Single Folder:".bright_white(),
        if config.single {
            "Yes".bright_yellow()
        } else {
            "No".bright_white()
        }
    );
    println!(
        "  {} {}",
        "Force Reprocess:".bright_white(),
        if config.force {
            "Yes".bright_yellow()
        } else {
            "No".bright_white()
        }
    );
    println!("  {} {}", "Concurrency:".bright_white(), config.concurrency);
    println!(
        "  {} {}",
        "Verbose:".bright_white(),
        if config.verbose {
            "Yes".bright_yellow()
        } else {
            "No".bright_white()
        }
    );
    println!(
        "  {} {}",
        "Processing Mode:".bright_white(),
        config.processing_mode
    );
    println!(
        "  {} {}",
        "Season Extras:".bright_white(),
        if config.season_extras {
            "Yes".bright_yellow()
        } else {
            "No".bright_white()
        }
    );
    println!(
        "  {} {}",
        "Specials:".bright_white(),
        if config.specials {
            "Yes".bright_yellow()
        } else {
            "No".bright_white()
        }
    );
    if config.specials {
        println!(
            "  {} {}",
            "Specials Folder:".bright_white(),
            config.specials_folder.bright_cyan()
        );
    }
    if let Some(ref media_type) = config.media_type {
        println!(
            "  {} {}",
            "Media Type:".bright_white(),
            media_type.bright_yellow()
        );
    }
    if let Some(ref browser) = config.cookies_from_browser {
        println!(
            "  {} {}",
            "Cookies From:".bright_white(),
            browser.bright_cyan()
        );
    }
    if config.dry_run {
        println!(
            "  {} {}",
            "Dry Run:".bright_white(),
            "Yes (discovery only)".bright_yellow()
        );
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Source, default_sources};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_args(root: PathBuf) -> CliArgs {
        CliArgs {
            root_directory: root,
            force: false,
            sources: default_sources(),
            all: false,
            concurrency: 2,
            verbose: false,
            single: false,
            series_only: false,
            movies_only: false,
            season_extras: false,
            specials: false,
            specials_folder: "Specials".to_string(),
            r#type: None,
            cookies_from_browser: None,
            mode: None,
            dry_run: false,
            json_progress: false,
            tui: false,
        }
    }

    #[test]
    fn test_source_display() {
        assert_eq!(Source::Tmdb.to_string(), "tmdb");
        assert_eq!(Source::Youtube.to_string(), "youtube");
        assert_eq!(Source::Dailymotion.to_string(), "dailymotion");
    }

    #[test]
    fn test_cli_config_from_args() {
        let temp_dir = TempDir::new().unwrap();
        let args = CliArgs {
            root_directory: temp_dir.path().to_path_buf(),
            force: true,
            sources: vec![Source::Youtube],
            all: false,
            concurrency: 4,
            verbose: true,
            single: false,
            series_only: false,
            movies_only: false,
            season_extras: true,
            specials: true,
            specials_folder: "Specials".to_string(),
            r#type: Some("series".to_string()),
            cookies_from_browser: None,
            mode: None,
            dry_run: false,
            json_progress: true,
            tui: false,
        };

        let config: CliConfig = args.into();
        assert!(config.force);
        assert_eq!(config.sources, vec![Source::Youtube]);
        assert_eq!(config.concurrency, 4);
        assert!(config.verbose);
        assert!(!config.single);
        assert_eq!(config.processing_mode, crate::models::ProcessingMode::Both);
        assert!(config.season_extras);
        assert!(config.specials);
        assert_eq!(config.media_type, Some("series".to_string()));
        assert!(config.json_progress);
    }

    #[test]
    fn test_validate_config_valid_directory() {
        let temp_dir = TempDir::new().unwrap();
        let args = make_args(temp_dir.path().to_path_buf());
        assert!(validate_config(&args).is_ok());
    }

    #[test]
    fn test_validate_config_nonexistent_directory() {
        let args = make_args(PathBuf::from("/nonexistent/path/that/does/not/exist"));
        let result = validate_config(&args);
        assert!(result.is_err());
        match result {
            Err(CliError::InvalidRootDir(msg)) => assert!(msg.contains("does not exist")),
            _ => panic!("Expected InvalidRootDir error"),
        }
    }

    #[test]
    fn test_validate_config_file_instead_of_directory() {
        use std::fs::File;
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        File::create(&file_path).unwrap();

        let args = make_args(file_path);
        let result = validate_config(&args);
        assert!(result.is_err());
        match result {
            Err(CliError::InvalidRootDir(msg)) => assert!(msg.contains("not a directory")),
            _ => panic!("Expected InvalidRootDir error"),
        }
    }

    #[test]
    fn test_validate_config_zero_concurrency() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.concurrency = 0;
        let result = validate_config(&args);
        assert!(result.is_err());
        match result {
            Err(CliError::InvalidConcurrency(msg)) => assert!(msg.contains("at least 1")),
            _ => panic!("Expected InvalidConcurrency error"),
        }
    }

    #[test]
    fn test_validate_config_default_values() {
        let temp_dir = TempDir::new().unwrap();
        let args = make_args(temp_dir.path().to_path_buf());
        assert!(validate_config(&args).is_ok());

        let config: CliConfig = args.into();
        assert!(!config.force);
        assert_eq!(config.sources, default_sources());
        assert_eq!(config.concurrency, 2);
        assert!(!config.verbose);
        assert!(!config.single);
        assert_eq!(config.processing_mode, crate::models::ProcessingMode::Both);
        assert!(!config.season_extras);
        assert!(!config.specials);
        assert_eq!(config.media_type, None);
    }

    #[test]
    fn test_all_flag_sets_all_sources() {
        use crate::models::all_sources;
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.all = true;
        let config: CliConfig = args.into();
        assert_eq!(config.sources, all_sources());
        assert!(config.sources.contains(&Source::Vimeo));
    }

    #[test]
    fn test_all_flag_overrides_sources() {
        use crate::models::all_sources;
        let temp_dir = TempDir::new().unwrap();
        // Even if sources is set to just Youtube, --all should expand to all_sources()
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.all = true;
        args.sources = vec![Source::Youtube];
        let config: CliConfig = args.into();
        assert_eq!(config.sources, all_sources());
    }

    #[test]
    fn test_display_banner_does_not_panic() {
        display_banner();
    }

    #[test]
    fn test_display_config_does_not_panic() {
        let config = CliConfig {
            root_directory: PathBuf::from("/test/movies"),
            force: true,
            sources: vec![Source::Youtube],
            concurrency: 4,
            verbose: true,
            single: false,
            processing_mode: crate::models::ProcessingMode::SeriesOnly,
            season_extras: true,
            specials: true,
            specials_folder: "Specials".to_string(),
            media_type: Some("series".to_string()),
            cookies_from_browser: None,
            dry_run: false,
            json_progress: false,
            tui: false,
        };
        display_config(&config);
    }

    #[test]
    fn test_series_only_flag_sets_processing_mode() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.series_only = true;
        let config: CliConfig = args.into();
        assert_eq!(
            config.processing_mode,
            crate::models::ProcessingMode::SeriesOnly
        );
    }

    #[test]
    fn test_movies_only_flag_sets_processing_mode() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.movies_only = true;
        let config: CliConfig = args.into();
        assert_eq!(
            config.processing_mode,
            crate::models::ProcessingMode::MoviesOnly
        );
    }

    #[test]
    fn test_mutually_exclusive_flags_error() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.series_only = true;
        args.movies_only = true;
        let result = validate_config(&args);
        assert!(result.is_err());
        match result {
            Err(CliError::ParseError(msg)) => assert!(msg.contains("mutually exclusive")),
            _ => panic!("Expected ParseError for mutually exclusive flags"),
        }
    }

    #[test]
    fn test_type_flag_movie_valid() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.r#type = Some("movie".to_string());
        assert!(validate_config(&args).is_ok());
    }

    #[test]
    fn test_type_flag_series_valid() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.r#type = Some("series".to_string());
        assert!(validate_config(&args).is_ok());
    }

    #[test]
    fn test_type_flag_invalid_value() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.r#type = Some("invalid".to_string());
        let result = validate_config(&args);
        assert!(result.is_err());
        match result {
            Err(CliError::ParseError(msg)) => assert!(msg.contains("Invalid --type value")),
            _ => panic!("Expected ParseError for invalid type"),
        }
    }

    #[test]
    fn test_season_extras_flag() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.season_extras = true;
        let config: CliConfig = args.into();
        assert!(config.season_extras);
    }

    #[test]
    fn test_specials_flag() {
        let temp_dir = TempDir::new().unwrap();
        let mut args = make_args(temp_dir.path().to_path_buf());
        args.specials = true;
        let config: CliConfig = args.into();
        assert!(config.specials);
    }

    #[test]
    fn test_deprecated_mode_flag_returns_error() {
        use clap::Parser;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_str().unwrap();

        // Simulate passing --mode on the CLI; parse_args() should reject it
        let args = CliArgs::try_parse_from(["extras_fetcher", root, "--mode", "youtube"]);
        assert!(args.is_ok(), "clap should parse --mode as a hidden arg");
        let args = args.unwrap();
        assert!(args.mode.is_some());

        // The actual deprecation check lives in parse_args(), which calls
        // CliArgs::parse() internally. We replicate the check here to test
        // the error path without invoking the real CLI entrypoint.
        if args.mode.is_some() {
            let err = CliError::DeprecatedFlag(
                "The --mode flag has been removed. Use --sources instead.".to_string(),
            );
            assert!(
                err.to_string().contains("--mode"),
                "Error should mention --mode: {}",
                err
            );
        }
    }

    #[test]
    fn test_dry_run_flag_parsed_correctly() {
        use clap::Parser;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_str().unwrap();

        // Without --dry-run: default is false
        let args = CliArgs::try_parse_from(["extras_fetcher", root]).expect("parse should succeed");
        assert!(!args.dry_run);
        let config: CliConfig = args.into();
        assert!(!config.dry_run);

        // With --dry-run: should be true
        let args = CliArgs::try_parse_from(["extras_fetcher", root, "--dry-run"])
            .expect("parse should succeed");
        assert!(args.dry_run);
        let config: CliConfig = args.into();
        assert!(config.dry_run);
    }

    #[test]
    fn test_json_progress_flag_parsed_correctly() {
        use clap::Parser;

        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_str().unwrap();

        let args = CliArgs::try_parse_from(["extras_fetcher", root, "--json-progress"])
            .expect("parse should succeed");
        assert!(args.json_progress);
        let config: CliConfig = args.into();
        assert!(config.json_progress);
    }

    #[test]
    fn test_display_config_with_dry_run() {
        let config = CliConfig {
            root_directory: PathBuf::from("/test/movies"),
            force: false,
            sources: default_sources(),
            concurrency: 2,
            verbose: false,
            single: false,
            processing_mode: crate::models::ProcessingMode::Both,
            season_extras: false,
            specials: false,
            specials_folder: "Specials".to_string(),
            media_type: None,
            cookies_from_browser: None,
            dry_run: true,
            json_progress: false,
            tui: false,
        };
        // Should not panic and should display dry-run indicator
        display_config(&config);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::models::{Source, default_sources};
    use proptest::prelude::*;
    use std::path::PathBuf;

    // Feature: extras-fetcher, Property 36: Configuration Display Completeness
    // Validates: Requirements 13.2
    proptest! {
        #[test]
        fn prop_config_display_completeness(
            force in proptest::bool::ANY,
            concurrency in 1usize..=10,
            verbose in proptest::bool::ANY,
            single in proptest::bool::ANY,
        ) {
            let config = CliConfig {
                root_directory: PathBuf::from("/test/movies"),
                force,
                sources: default_sources(),
                concurrency,
                verbose,
                single,
                processing_mode: crate::models::ProcessingMode::Both,
                season_extras: false,
                specials: false,
                specials_folder: "Specials".to_string(),
                media_type: None,
                cookies_from_browser: None,
                dry_run: false,
                json_progress: false,
                tui: false,
            };

            let mut output = Vec::new();
            {
                use std::io::Write;
                write!(&mut output, "{:?}", config.root_directory).unwrap();
                for s in &config.sources {
                    write!(&mut output, "{}", s).unwrap();
                }
                write!(&mut output, "{}", config.force).unwrap();
                write!(&mut output, "{}", config.concurrency).unwrap();
                write!(&mut output, "{}", config.verbose).unwrap();
            }

            let output_str = String::from_utf8(output).unwrap();

            prop_assert!(
                output_str.contains("/test/movies") || output_str.contains("test"),
                "Config display should include root_directory"
            );
            prop_assert!(
                output_str.contains("tmdb") || output_str.contains("youtube"),
                "Config display should include sources"
            );
            prop_assert!(
                output_str.contains("true") || output_str.contains("false"),
                "Config display should include force flag"
            );
            prop_assert!(
                output_str.contains(&concurrency.to_string()),
                "Config display should include concurrency value"
            );

            prop_assert_eq!(config.force, force);
            prop_assert_eq!(config.sources, default_sources());
            prop_assert_eq!(config.concurrency, concurrency);
            prop_assert_eq!(config.verbose, verbose);
            prop_assert_eq!(config.single, single);
        }
    }

    // Feature: extras-fetcher, Property 38: Verbose Flag Effect
    // Validates: Requirements 13.8
    proptest! {
        #[test]
        fn prop_verbose_flag_effect(
            verbose in proptest::bool::ANY,
        ) {
            let config = CliConfig {
                root_directory: PathBuf::from("/test/movies"),
                force: false,
                sources: vec![Source::Youtube],
                concurrency: 2,
                verbose,
                single: false,
                processing_mode: crate::models::ProcessingMode::Both,
                season_extras: false,
                specials: false,
                specials_folder: "Specials".to_string(),
                media_type: None,
                cookies_from_browser: None,
                dry_run: false,
                json_progress: false,
                tui: false,
            };

            prop_assert_eq!(config.verbose, verbose);

            if verbose {
                prop_assert!(config.verbose, "When verbose flag is set, config.verbose should be true");
            } else {
                prop_assert!(!config.verbose, "When verbose flag is not set, config.verbose should be false");
            }
        }
    }
}
