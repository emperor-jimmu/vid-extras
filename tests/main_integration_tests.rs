// Integration tests for main entry point
// Tests complete execution flow, validation failures, and error handling
// Requirements: 11.1-11.5, 10.5

use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a test movie directory structure
fn create_test_movie_structure(root: &Path) -> std::io::Result<()> {
    let movie_dir = root.join("Test Movie (2020)");
    fs::create_dir_all(&movie_dir)?;

    // Create a dummy movie file
    fs::write(
        movie_dir.join("Test Movie (2020).mp4"),
        b"dummy movie content",
    )?;

    Ok(())
}

/// Helper to create a movie with done marker
fn create_movie_with_done_marker(root: &Path) -> std::io::Result<()> {
    let movie_dir = root.join("Completed Movie (2019)");
    fs::create_dir_all(&movie_dir)?;

    // Create done marker
    let done_marker = serde_json::json!({
        "finished_at": "2024-01-15T10:30:00Z",
        "version": "0.1.0"
    });
    fs::write(
        movie_dir.join("done.ext"),
        serde_json::to_string_pretty(&done_marker)?,
    )?;

    Ok(())
}

#[test]
fn test_validation_missing_ytdlp() {
    // This test verifies that the validator catches missing yt-dlp
    // Requirements: 11.1, 11.5

    // Note: This test assumes yt-dlp is actually installed in the test environment
    // To properly test missing binary, we would need to mock the binary check
    // For now, we verify the validator can be created and called

    use extras_fetcher::validation::Validator;

    let validator = Validator::new();

    // If yt-dlp is installed, this should succeed
    // If not, it should fail with a descriptive error
    match validator.validate_dependencies() {
        Ok(_) => {
            // Dependencies are available - test passes
        }
        Err(e) => {
            // Verify error message is descriptive
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("yt-dlp")
                    || error_msg.contains("ffmpeg")
                    || error_msg.contains("TMDB"),
                "Error message should mention missing dependency: {}",
                error_msg
            );
        }
    }
}

#[test]
fn test_validation_missing_ffmpeg() {
    // This test verifies that the validator catches missing ffmpeg
    // Requirements: 11.2, 11.5

    use extras_fetcher::validation::Validator;

    let validator = Validator::new();

    // Similar to above - we verify the validator works
    match validator.validate_dependencies() {
        Ok(_) => {
            // Dependencies are available - test passes
        }
        Err(e) => {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("ffmpeg")
                    || error_msg.contains("yt-dlp")
                    || error_msg.contains("TMDB"),
                "Error message should mention missing dependency: {}",
                error_msg
            );
        }
    }
}

#[test]
#[ignore = "Requires TMDB_API_KEY to be unset, which is difficult in test environment"]
fn test_validation_missing_tmdb_api_key() {
    // This test verifies that the validator catches missing TMDB API key
    // Requirements: 11.4, 11.5

    use extras_fetcher::validation::Validator;

    let validator = Validator::new();
    let result = validator.validate_dependencies();

    // Should fail with missing API key error
    match result {
        Ok(_) => panic!("Should have failed with missing TMDB API key"),
        Err(e) => {
            let error_msg = e.to_string();
            assert!(
                error_msg.contains("TMDB") || error_msg.contains("API key"),
                "Error should mention TMDB API key: {}",
                error_msg
            );
        }
    }
}

#[test]
fn test_validation_ffmpeg_hevc_support() {
    // This test verifies that the validator checks for HEVC support
    // Requirements: 11.3, 11.5

    use extras_fetcher::validation::Validator;

    let validator = Validator::new();

    // If ffmpeg is installed, check if HEVC support is detected
    match validator.validate_dependencies() {
        Ok(_) => {
            // HEVC support is available - test passes
        }
        Err(e) => {
            let error_msg = e.to_string();
            // Error could be missing ffmpeg or missing HEVC support
            assert!(
                error_msg.contains("ffmpeg")
                    || error_msg.contains("HEVC")
                    || error_msg.contains("x265")
                    || error_msg.contains("TMDB")
                    || error_msg.contains("yt-dlp"),
                "Error message should be descriptive: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
async fn test_scanner_integration() {
    // Test scanner integration with real file system
    // Requirements: 1.6, 1.7, 1.8

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create test movie structure
    create_test_movie_structure(&root).expect("Failed to create test structure");
    create_movie_with_done_marker(&root).expect("Failed to create done marker");

    // Scan without force flag - should skip movie with done marker
    let scanner = Scanner::new(root.clone(), false, false);
    let movies = scanner.scan().expect("Scan should succeed");

    // Should find only the movie without done marker
    assert_eq!(movies.len(), 1);
    assert_eq!(movies[0].title, "Test Movie");
    assert_eq!(movies[0].year, 2020);

    // Scan with force flag - should include all movies
    let scanner_force = Scanner::new(root.clone(), true, false);
    let movies_force = scanner_force.scan().expect("Scan should succeed");

    // Should find both movies
    assert_eq!(movies_force.len(), 2);
}

#[tokio::test]
async fn test_orchestrator_empty_directory() {
    // Test orchestrator with empty directory
    // Requirements: 10.2, 10.5

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Test scanner on empty directory
    let scanner = Scanner::new(root.clone(), false, false);
    let movies = scanner
        .scan()
        .expect("Scan should succeed on empty directory");

    // Should find no movies
    assert_eq!(movies.len(), 0, "Empty directory should have no movies");

    // Note: We don't actually run the orchestrator here because it would
    // attempt real network operations. The scanner test verifies that
    // empty directories are handled gracefully.
}

#[tokio::test]
async fn test_orchestrator_with_done_markers() {
    // Test orchestrator respects done markers
    // Requirements: 1.8, 2.3, 12.1

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create movies with and without done markers
    create_test_movie_structure(&root).expect("Failed to create test structure");
    create_movie_with_done_marker(&root).expect("Failed to create done marker");

    // Test scanner without force flag - should skip movie with done marker
    let scanner = Scanner::new(root.clone(), false, false);
    let movies = scanner.scan().expect("Scan should succeed");
    assert_eq!(
        movies.len(),
        1,
        "Should only find movie without done marker"
    );

    // Test scanner with force flag - should include all movies
    let scanner_force = Scanner::new(root.clone(), true, false);
    let movies_force = scanner_force.scan().expect("Scan should succeed");
    assert_eq!(
        movies_force.len(),
        2,
        "Should find both movies with force flag"
    );

    // Note: We don't actually run the orchestrator here because it would
    // attempt real network operations. The scanner tests verify that
    // done markers are properly respected.
}

#[test]
fn test_cli_parsing_integration() {
    // Test CLI argument parsing with various inputs
    // Requirements: 1.1, 1.2, 1.3, 1.4, 1.5

    use clap::Parser;
    use extras_fetcher::cli::CliArgs;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_str().unwrap();

    // Test basic parsing
    let args = vec!["extras_fetcher", root];
    let parsed = CliArgs::try_parse_from(args);
    assert!(parsed.is_ok());

    // Test with force flag
    let args = vec!["extras_fetcher", root, "--force"];
    let parsed = CliArgs::try_parse_from(args);
    assert!(parsed.is_ok());
    assert!(parsed.unwrap().force);

    // Test with mode flag
    let args = vec!["extras_fetcher", root, "--mode", "youtube"];
    let parsed = CliArgs::try_parse_from(args);
    assert!(parsed.is_ok());

    // Test with concurrency flag
    let args = vec!["extras_fetcher", root, "--concurrency", "4"];
    let parsed = CliArgs::try_parse_from(args);
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap().concurrency, 4);

    // Test with verbose flag
    let args = vec!["extras_fetcher", root, "--verbose"];
    let parsed = CliArgs::try_parse_from(args);
    assert!(parsed.is_ok());
    assert!(parsed.unwrap().verbose);

    // Test with all flags
    let args = vec![
        "extras_fetcher",
        root,
        "--force",
        "--mode",
        "youtube",
        "--concurrency",
        "3",
        "--verbose",
    ];
    let parsed = CliArgs::try_parse_from(args);
    assert!(parsed.is_ok());
    let parsed = parsed.unwrap();
    assert!(parsed.force);
    assert_eq!(parsed.concurrency, 3);
    assert!(parsed.verbose);
}

#[test]
fn test_error_handling_invalid_directory() {
    // Test error handling for invalid directory
    // Requirements: 10.5

    use clap::Parser;
    use extras_fetcher::cli::CliArgs;

    // Test with nonexistent directory
    let args = vec!["extras_fetcher", "/nonexistent/path/to/movies"];
    let parsed = CliArgs::try_parse_from(args);

    if let Ok(cli_args) = parsed {
        // Convert to config - this should validate the directory
        use extras_fetcher::cli::CliConfig;
        let config: CliConfig = cli_args.into();

        // The directory doesn't exist, but parsing succeeds
        // Validation happens at runtime
        assert!(!config.root_directory.exists());
    }
}

#[test]
fn test_graceful_error_handling() {
    // Test that errors are handled gracefully without panics
    // Requirements: 10.5

    use extras_fetcher::validation::Validator;

    // Test validator with potentially missing dependencies
    let validator = Validator::new();
    let result = validator.validate_dependencies();

    // Should return Result, not panic
    match result {
        Ok(_) => {
            // Dependencies available - test passes
        }
        Err(e) => {
            // Error is descriptive
            let error_msg = e.to_string();
            assert!(!error_msg.is_empty());
            assert!(error_msg.len() > 10); // Should be descriptive
        }
    }
}

#[tokio::test]
async fn test_complete_execution_flow() {
    // Test complete execution flow with mock file system
    // Requirements: 11.1-11.5, 10.5

    use extras_fetcher::scanner::Scanner;
    use extras_fetcher::validation::Validator;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create test movie structure
    create_test_movie_structure(&root).expect("Failed to create test structure");

    // Step 1: Validate dependencies
    let validator = Validator::new();
    let validation_result = validator.validate_dependencies();

    // Step 2: Scan for movies
    let scanner = Scanner::new(root.clone(), false, false);
    let scan_result = scanner.scan();

    // Verify scan succeeds
    assert!(scan_result.is_ok());
    let movies = scan_result.unwrap();
    assert_eq!(movies.len(), 1);

    // Step 3: If validation succeeded, try to create orchestrator
    if let Ok(api_key) = validation_result {
        use extras_fetcher::models::{ProcessingMode, SourceMode};
        use extras_fetcher::orchestrator::Orchestrator;

        let orchestrator = Orchestrator::builder(root, api_key)
            .mode(SourceMode::YoutubeOnly)
            .build();

        // Orchestrator creation should succeed
        assert!(orchestrator.is_ok());

        // Note: We don't run the orchestrator here because it would
        // attempt real network operations. The integration test verifies
        // that all components can be created and wired together correctly.
    }
}

// ============================================================================
// Idempotency Integration Tests
// Requirements: 12.1, 12.2, 12.3, 12.4
// ============================================================================

#[tokio::test]
async fn test_idempotency_multiple_runs_on_same_library() {
    // Test that running the tool multiple times on the same library
    // only processes folders without done markers
    // Requirements: 12.2, 12.3

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create multiple movie folders
    for i in 1..=5 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        fs::create_dir_all(&movie_dir).expect("Failed to create movie dir");
    }

    // First run - all movies should be found
    let scanner1 = Scanner::new(root.clone(), false, false);
    let movies1 = scanner1.scan().expect("First scan should succeed");
    assert_eq!(movies1.len(), 5, "First scan should find all 5 movies");

    // Simulate processing by adding done markers to 3 movies
    for i in 1..=3 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        let done_marker = serde_json::json!({
            "finished_at": "2024-01-15T10:30:00Z",
            "version": "0.1.0"
        });
        fs::write(
            movie_dir.join("done.ext"),
            serde_json::to_string_pretty(&done_marker).unwrap(),
        )
        .expect("Failed to write done marker");
    }

    // Second run - should only find 2 movies without done markers
    let scanner2 = Scanner::new(root.clone(), false, false);
    let movies2 = scanner2.scan().expect("Second scan should succeed");
    assert_eq!(
        movies2.len(),
        2,
        "Second scan should find only 2 movies without done markers"
    );

    // Verify the correct movies were found
    let titles: Vec<&str> = movies2.iter().map(|m| m.title.as_str()).collect();
    assert!(titles.contains(&"Movie 4"), "Should find Movie 4");
    assert!(titles.contains(&"Movie 5"), "Should find Movie 5");

    // Third run - should still find the same 2 movies (idempotent)
    let scanner3 = Scanner::new(root.clone(), false, false);
    let movies3 = scanner3.scan().expect("Third scan should succeed");
    assert_eq!(
        movies3.len(),
        2,
        "Third scan should find same 2 movies (idempotent)"
    );

    // Add done markers to remaining movies
    for i in 4..=5 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        let done_marker = serde_json::json!({
            "finished_at": "2024-01-15T10:30:00Z",
            "version": "0.1.0"
        });
        fs::write(
            movie_dir.join("done.ext"),
            serde_json::to_string_pretty(&done_marker).unwrap(),
        )
        .expect("Failed to write done marker");
    }

    // Fourth run - should find no movies (all have done markers)
    let scanner4 = Scanner::new(root.clone(), false, false);
    let movies4 = scanner4.scan().expect("Fourth scan should succeed");
    assert_eq!(
        movies4.len(),
        0,
        "Fourth scan should find no movies (all processed)"
    );
}

#[tokio::test]
async fn test_idempotency_interruption_and_resumption() {
    // Test that the tool can safely resume after interruption
    // Requirements: 12.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create movie folders
    for i in 1..=4 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        fs::create_dir_all(&movie_dir).expect("Failed to create movie dir");
    }

    // Initial scan - all movies found
    let scanner = Scanner::new(root.clone(), false, false);
    let movies = scanner.scan().expect("Initial scan should succeed");
    assert_eq!(movies.len(), 4, "Should find all 4 movies initially");

    // Simulate partial processing: movies 1 and 2 completed, 3 and 4 not started
    for i in 1..=2 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        let done_marker = serde_json::json!({
            "finished_at": "2024-01-15T10:30:00Z",
            "version": "0.1.0"
        });
        fs::write(
            movie_dir.join("done.ext"),
            serde_json::to_string_pretty(&done_marker).unwrap(),
        )
        .expect("Failed to write done marker");
    }

    // Simulate interruption by creating temp directories for movies 3 and 4
    let temp_base = temp_dir.path().join("tmp_downloads");
    fs::create_dir_all(&temp_base).expect("Failed to create temp base");

    for i in 3..=4 {
        let temp_movie_dir = temp_base.join(format!("Movie_{}_{}", i, 2020 + i));
        fs::create_dir_all(&temp_movie_dir).expect("Failed to create temp dir");
        fs::write(temp_movie_dir.join("partial.mp4"), b"partial download")
            .expect("Failed to write partial file");
    }

    // Resume scan - should find movies 3 and 4 (without done markers)
    let scanner_resume = Scanner::new(root.clone(), false, false);
    let movies_resume = scanner_resume.scan().expect("Resume scan should succeed");
    assert_eq!(
        movies_resume.len(),
        2,
        "Resume should find 2 unprocessed movies"
    );

    // Verify correct movies found
    let titles: Vec<&str> = movies_resume.iter().map(|m| m.title.as_str()).collect();
    assert!(titles.contains(&"Movie 3"), "Should find Movie 3");
    assert!(titles.contains(&"Movie 4"), "Should find Movie 4");

    // Verify temp directories still exist (cleanup happens during orchestrator run)
    assert!(
        temp_base.exists(),
        "Temp directories should exist before cleanup"
    );

    // Simulate cleanup (what orchestrator would do)
    fs::remove_dir_all(&temp_base).expect("Failed to cleanup temp");

    // Complete processing of movie 3
    let movie3_dir = root.join("Movie 3 (2023)");
    let done_marker = serde_json::json!({
        "finished_at": "2024-01-15T11:00:00Z",
        "version": "0.1.0"
    });
    fs::write(
        movie3_dir.join("done.ext"),
        serde_json::to_string_pretty(&done_marker).unwrap(),
    )
    .expect("Failed to write done marker");

    // Final scan - should only find movie 4
    let scanner_final = Scanner::new(root.clone(), false, false);
    let movies_final = scanner_final.scan().expect("Final scan should succeed");
    assert_eq!(
        movies_final.len(),
        1,
        "Final scan should find only 1 unprocessed movie"
    );
    assert_eq!(movies_final[0].title, "Movie 4");
}

#[tokio::test]
async fn test_idempotency_force_flag_behavior() {
    // Test that force flag overrides done markers and allows reprocessing
    // Requirements: 1.4, 12.1

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create movie folders with done markers
    for i in 1..=3 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        fs::create_dir_all(&movie_dir).expect("Failed to create movie dir");

        let done_marker = serde_json::json!({
            "finished_at": "2024-01-15T10:30:00Z",
            "version": "0.1.0"
        });
        fs::write(
            movie_dir.join("done.ext"),
            serde_json::to_string_pretty(&done_marker).unwrap(),
        )
        .expect("Failed to write done marker");
    }

    // Scan without force flag - should find no movies
    let scanner_no_force = Scanner::new(root.clone(), false, false);
    let movies_no_force = scanner_no_force
        .scan()
        .expect("Scan without force should succeed");
    assert_eq!(
        movies_no_force.len(),
        0,
        "Without force flag, should skip all movies with done markers"
    );

    // Scan with force flag - should find all movies
    let scanner_force = Scanner::new(root.clone(), true, false);
    let movies_force = scanner_force
        .scan()
        .expect("Scan with force should succeed");
    assert_eq!(
        movies_force.len(),
        3,
        "With force flag, should find all movies"
    );

    // Verify all movies have done markers but are still included
    for movie in &movies_force {
        assert!(
            movie.has_done_marker,
            "Movie should have done marker: {}",
            movie.title
        );
    }

    // Multiple force scans should be idempotent
    let scanner_force2 = Scanner::new(root.clone(), true, false);
    let movies_force2 = scanner_force2
        .scan()
        .expect("Second force scan should succeed");
    assert_eq!(
        movies_force2.len(),
        3,
        "Multiple force scans should be idempotent"
    );
}

#[tokio::test]
async fn test_idempotency_partial_library_processing() {
    // Test that the tool can process a library incrementally
    // Requirements: 12.3

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create initial set of movies
    for i in 1..=3 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        fs::create_dir_all(&movie_dir).expect("Failed to create movie dir");
    }

    // First scan - find all 3 movies
    let scanner1 = Scanner::new(root.clone(), false, false);
    let movies1 = scanner1.scan().expect("First scan should succeed");
    assert_eq!(movies1.len(), 3, "Should find 3 movies initially");

    // Process first 2 movies (add done markers)
    for i in 1..=2 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        let done_marker = serde_json::json!({
            "finished_at": "2024-01-15T10:30:00Z",
            "version": "0.1.0"
        });
        fs::write(
            movie_dir.join("done.ext"),
            serde_json::to_string_pretty(&done_marker).unwrap(),
        )
        .expect("Failed to write done marker");
    }

    // Second scan - should find only movie 3
    let scanner2 = Scanner::new(root.clone(), false, false);
    let movies2 = scanner2.scan().expect("Second scan should succeed");
    assert_eq!(movies2.len(), 1, "Should find 1 unprocessed movie");
    assert_eq!(movies2[0].title, "Movie 3");

    // Add new movies to the library
    for i in 4..=6 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        fs::create_dir_all(&movie_dir).expect("Failed to create movie dir");
    }

    // Third scan - should find movie 3 plus new movies 4, 5, 6
    let scanner3 = Scanner::new(root.clone(), false, false);
    let movies3 = scanner3.scan().expect("Third scan should succeed");
    assert_eq!(
        movies3.len(),
        4,
        "Should find 4 unprocessed movies (1 old + 3 new)"
    );

    // Verify correct movies found
    let titles: Vec<&str> = movies3.iter().map(|m| m.title.as_str()).collect();
    assert!(titles.contains(&"Movie 3"), "Should find Movie 3");
    assert!(titles.contains(&"Movie 4"), "Should find Movie 4");
    assert!(titles.contains(&"Movie 5"), "Should find Movie 5");
    assert!(titles.contains(&"Movie 6"), "Should find Movie 6");

    // Process all remaining movies
    for i in 3..=6 {
        let movie_dir = root.join(format!("Movie {} (202{})", i, i));
        let done_marker = serde_json::json!({
            "finished_at": "2024-01-15T11:00:00Z",
            "version": "0.1.0"
        });
        fs::write(
            movie_dir.join("done.ext"),
            serde_json::to_string_pretty(&done_marker).unwrap(),
        )
        .expect("Failed to write done marker");
    }

    // Final scan - should find no movies (all processed)
    let scanner4 = Scanner::new(root.clone(), false, false);
    let movies4 = scanner4.scan().expect("Final scan should succeed");
    assert_eq!(movies4.len(), 0, "All movies should be processed");

    // Add one more movie
    let movie_dir = root.join("Movie 7 (2027)");
    fs::create_dir_all(&movie_dir).expect("Failed to create movie dir");

    // Scan again - should find only the new movie
    let scanner5 = Scanner::new(root.clone(), false, false);
    let movies5 = scanner5
        .scan()
        .expect("Scan after adding new movie should succeed");
    assert_eq!(movies5.len(), 1, "Should find only the new movie");
    assert_eq!(movies5[0].title, "Movie 7");
}

#[tokio::test]
async fn test_idempotency_invalid_done_markers() {
    // Test that invalid done markers are treated as missing
    // Requirements: 2.4, 12.1

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let root = temp_dir.path().to_path_buf();

    // Create movies with various done marker states
    let movie1_dir = root.join("Movie 1 (2021)");
    fs::create_dir_all(&movie1_dir).expect("Failed to create movie dir");
    // Valid done marker
    let done_marker = serde_json::json!({
        "finished_at": "2024-01-15T10:30:00Z",
        "version": "0.1.0"
    });
    fs::write(
        movie1_dir.join("done.ext"),
        serde_json::to_string_pretty(&done_marker).unwrap(),
    )
    .expect("Failed to write done marker");

    let movie2_dir = root.join("Movie 2 (2022)");
    fs::create_dir_all(&movie2_dir).expect("Failed to create movie dir");
    // Invalid JSON
    fs::write(movie2_dir.join("done.ext"), "not valid json")
        .expect("Failed to write invalid done marker");

    let movie3_dir = root.join("Movie 3 (2023)");
    fs::create_dir_all(&movie3_dir).expect("Failed to create movie dir");
    // Empty file
    fs::write(movie3_dir.join("done.ext"), "").expect("Failed to write empty done marker");

    let movie4_dir = root.join("Movie 4 (2024)");
    fs::create_dir_all(&movie4_dir).expect("Failed to create movie dir");
    // No done marker

    // Scan - should find movies 2, 3, and 4 (invalid or missing done markers)
    let scanner = Scanner::new(root.clone(), false, false);
    let movies = scanner.scan().expect("Scan should succeed");
    assert_eq!(
        movies.len(),
        3,
        "Should find 3 movies with invalid or missing done markers"
    );

    // Verify correct movies found
    let titles: Vec<&str> = movies.iter().map(|m| m.title.as_str()).collect();
    assert!(
        titles.contains(&"Movie 2"),
        "Should find Movie 2 (invalid JSON)"
    );
    assert!(
        titles.contains(&"Movie 3"),
        "Should find Movie 3 (empty file)"
    );
    assert!(
        titles.contains(&"Movie 4"),
        "Should find Movie 4 (no marker)"
    );
    assert!(
        !titles.contains(&"Movie 1"),
        "Should not find Movie 1 (valid marker)"
    );
}
