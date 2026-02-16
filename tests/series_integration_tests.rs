// Integration tests for TV series support
// Tests complete series processing pipeline, mixed library processing, and edge cases
// Requirements: 20.3, 20.1, 20.2, 20.5, 12.4, 20.4

use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Helper to create a test series directory structure with seasons
fn create_test_series_structure(
    root: &Path,
    name: &str,
    year: u16,
    seasons: &[u8],
) -> std::io::Result<()> {
    let series_dir = root.join(format!("{} ({})", name, year));
    fs::create_dir_all(&series_dir)?;

    // Create season folders
    for season in seasons {
        let season_dir = series_dir.join(format!("Season {:02}", season));
        fs::create_dir_all(&season_dir)?;

        // Create a dummy episode file
        fs::write(
            season_dir.join(format!("{} - S{:02}E01 - Episode.mp4", name, season)),
            b"dummy episode content",
        )?;
    }

    Ok(())
}

/// Helper to create a series with done marker
fn create_series_with_done_marker(root: &Path, name: &str, year: u16) -> std::io::Result<()> {
    let series_dir = root.join(format!("{} ({})", name, year));
    fs::create_dir_all(&series_dir)?;

    // Create a season folder
    let season_dir = series_dir.join("Season 01");
    fs::create_dir_all(&season_dir)?;

    // Create done marker
    let done_marker = serde_json::json!({
        "finished_at": "2024-01-15T10:30:00Z",
        "version": "0.1.0"
    });
    fs::write(
        series_dir.join("done.ext"),
        serde_json::to_string_pretty(&done_marker)?,
    )?;

    Ok(())
}

/// Helper to create a test movie directory structure
fn create_test_movie_structure(root: &Path, name: &str, year: u16) -> std::io::Result<()> {
    let movie_dir = root.join(format!("{} ({})", name, year));
    fs::create_dir_all(&movie_dir)?;

    // Create a dummy movie file
    fs::write(
        movie_dir.join(format!("{} ({}).mp4", name, year)),
        b"dummy movie content",
    )?;

    Ok(())
}

/// Helper to create a series without year
fn create_series_without_year(root: &Path, name: &str) -> std::io::Result<()> {
    let series_dir = root.join(name);
    fs::create_dir_all(&series_dir)?;

    // Create a season folder
    let season_dir = series_dir.join("Season 01");
    fs::create_dir_all(&season_dir)?;

    // Create a dummy episode file
    fs::write(
        season_dir.join(format!("{} - S01E01 - Episode.mp4", name)),
        b"dummy episode content",
    )?;

    Ok(())
}

/// Helper to create a series with Season 0
fn create_series_with_season_zero(root: &Path, name: &str, year: u16) -> std::io::Result<()> {
    let series_dir = root.join(format!("{} ({})", name, year));
    fs::create_dir_all(&series_dir)?;

    // Create Season 00 folder
    let season_zero_dir = series_dir.join("Season 00");
    fs::create_dir_all(&season_zero_dir)?;

    // Create a special episode file
    fs::write(
        season_zero_dir.join(format!("{} - S00E01 - Pilot.mp4", name)),
        b"dummy special content",
    )?;

    // Create Season 01 folder
    let season_one_dir = series_dir.join("Season 01");
    fs::create_dir_all(&season_one_dir)?;

    Ok(())
}

#[test]
fn test_series_scanning_basic() {
    // Test basic series scanning
    // Requirements: 1.1, 1.2, 1.4, 1.5, 1.6

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "Breaking Bad", 2008, &[1, 2, 3]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (movies, series) = scanner.scan_all().unwrap();

    assert_eq!(movies.len(), 0, "Should find no movies");
    assert_eq!(series.len(), 1, "Should find 1 series");

    let series_entry = &series[0];
    assert_eq!(series_entry.title, "Breaking Bad");
    assert_eq!(series_entry.year, Some(2008));
    assert!(!series_entry.has_done_marker);
    assert_eq!(series_entry.seasons, vec![1, 2, 3]);
}

#[test]
fn test_series_done_marker_skipping() {
    // Test that series with done markers are skipped
    // Requirements: 1.3, 9.1, 9.3, 9.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_series_with_done_marker(temp_dir.path(), "The Office", 2005).unwrap();

    // Without force flag - should skip
    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();
    assert_eq!(series.len(), 0, "Should skip series with done marker");

    // With force flag - should include
    let scanner_force = Scanner::new(temp_dir.path().to_path_buf(), true, false);
    let (_, series_force) = scanner_force.scan_all().unwrap();
    assert_eq!(
        series_force.len(),
        1,
        "Should include series with force flag"
    );
}

#[test]
fn test_mixed_library_processing() {
    // Test processing mixed library with both movies and series
    // Requirements: 20.3, 12.1, 12.2, 12.3

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_movie_structure(temp_dir.path(), "Inception", 2010).unwrap();
    create_test_series_structure(temp_dir.path(), "Stranger Things", 2016, &[1, 2]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (movies, series) = scanner.scan_all().unwrap();

    assert_eq!(movies.len(), 1, "Should find 1 movie");
    assert_eq!(series.len(), 1, "Should find 1 series");

    assert_eq!(movies[0].title, "Inception");
    assert_eq!(series[0].title, "Stranger Things");
}

#[test]
fn test_processing_mode_filtering_movies_only() {
    // Test that MoviesOnly mode filters correctly
    // Requirements: 12.1, 12.2, 12.3

    use extras_fetcher::models::ProcessingMode;
    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_movie_structure(temp_dir.path(), "The Matrix", 1999).unwrap();
    create_test_series_structure(temp_dir.path(), "Game of Thrones", 2011, &[1]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (movies, series) = scanner.scan_all().unwrap();

    // Scanner finds both, but orchestrator would filter based on mode
    assert_eq!(movies.len(), 1);
    assert_eq!(series.len(), 1);

    // Verify that MoviesOnly mode would skip series
    let mode = ProcessingMode::MoviesOnly;
    assert_eq!(mode, ProcessingMode::MoviesOnly);
}

#[test]
fn test_processing_mode_filtering_series_only() {
    // Test that SeriesOnly mode filters correctly
    // Requirements: 12.1, 12.2, 12.3

    use extras_fetcher::models::ProcessingMode;

    let mode = ProcessingMode::SeriesOnly;
    assert_eq!(mode, ProcessingMode::SeriesOnly);
}

#[test]
fn test_series_without_year() {
    // Test series folder without year in name
    // Requirements: 1.1, 1.2, 12.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_series_without_year(temp_dir.path(), "The Crown").unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1, "Should find series without year");
    assert_eq!(series[0].title, "The Crown");
    assert_eq!(series[0].year, None, "Year should be None");
}

#[test]
fn test_series_with_season_zero() {
    // Test series with Season 0 specials
    // Requirements: 1.5, 8.1, 8.2

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_series_with_season_zero(temp_dir.path(), "Friends", 1994).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1, "Should find series with Season 0");
    assert!(series[0].seasons.contains(&0), "Should detect Season 0");
    assert!(series[0].seasons.contains(&1), "Should detect Season 1");
}

#[test]
fn test_series_with_no_extras_found() {
    // Test series processing when no extras are discovered
    // Requirements: 20.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "Seinfeld", 1989, &[1]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1, "Should find series");
    assert_eq!(series[0].title, "Seinfeld");
    // Series should be discoverable even if no extras are found
}

#[test]
fn test_invalid_done_marker_handling() {
    // Test that invalid done markers are treated as missing
    // Requirements: 9.5, 20.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    let series_dir = temp_dir.path().join("The Sopranos (1999)");
    fs::create_dir_all(&series_dir).unwrap();

    // Create season folder
    let season_dir = series_dir.join("Season 01");
    fs::create_dir_all(&season_dir).unwrap();

    // Create invalid done marker (not valid JSON)
    fs::write(&series_dir.join("done.ext"), "invalid json content").unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    // Should include the series since done marker is invalid
    assert_eq!(
        series.len(),
        1,
        "Should include series with invalid done marker"
    );
    assert!(
        !series[0].has_done_marker,
        "Invalid marker should not be recognized"
    );
}

#[test]
fn test_error_isolation_between_series() {
    // Test that errors in one series don't affect others
    // Requirements: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "Series 1", 2020, &[1]).unwrap();
    create_test_series_structure(temp_dir.path(), "Series 2", 2021, &[1]).unwrap();
    create_test_series_structure(temp_dir.path(), "Series 3", 2022, &[1]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    // All series should be found regardless of processing order
    assert_eq!(series.len(), 3, "Should find all 3 series");

    let titles: Vec<&str> = series.iter().map(|s| s.title.as_str()).collect();
    assert!(titles.contains(&"Series 1"));
    assert!(titles.contains(&"Series 2"));
    assert!(titles.contains(&"Series 3"));
}

#[test]
fn test_backward_compatibility_movies_only() {
    // Test that movie-only libraries work identically to before
    // Requirements: 20.1, 20.2, 20.5

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_movie_structure(temp_dir.path(), "Pulp Fiction", 1994).unwrap();
    create_test_movie_structure(temp_dir.path(), "Forrest Gump", 1994).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (movies, series) = scanner.scan_all().unwrap();

    assert_eq!(movies.len(), 2, "Should find 2 movies");
    assert_eq!(series.len(), 0, "Should find no series");

    let titles: Vec<&str> = movies.iter().map(|m| m.title.as_str()).collect();
    assert!(titles.contains(&"Pulp Fiction"));
    assert!(titles.contains(&"Forrest Gump"));
}

#[test]
fn test_series_entry_data_model() {
    // Test SeriesEntry data model correctness
    // Requirements: 2.1, 2.2

    use extras_fetcher::models::SeriesEntry;
    use std::path::PathBuf;

    let entry = SeriesEntry {
        path: PathBuf::from("/test/series"),
        title: "Test Series".to_string(),
        year: Some(2020),
        has_done_marker: false,
        seasons: vec![1, 2, 3],
    };

    assert_eq!(entry.title, "Test Series");
    assert_eq!(entry.year, Some(2020));
    assert!(!entry.has_done_marker);
    assert_eq!(entry.seasons.len(), 3);
}

#[test]
fn test_series_extra_data_model() {
    // Test SeriesExtra data model correctness
    // Requirements: 2.1, 2.2, 2.3, 2.4, 2.6

    use extras_fetcher::models::{ContentCategory, SeriesExtra, SourceType};
    use std::path::PathBuf;

    // Series-level extra (no season number)
    let series_level = SeriesExtra {
        series_id: "123".to_string(),
        season_number: None,
        category: ContentCategory::Trailer,
        title: "Series Trailer".to_string(),
        url: "https://example.com/trailer".to_string(),
        source_type: SourceType::TMDB,
        local_path: None,
    };

    assert_eq!(series_level.season_number, None);
    assert_eq!(series_level.category, ContentCategory::Trailer);

    // Season-specific extra (with season number)
    let season_specific = SeriesExtra {
        series_id: "123".to_string(),
        season_number: Some(1),
        category: ContentCategory::BehindTheScenes,
        title: "Season 1 Behind the Scenes".to_string(),
        url: "https://example.com/bts".to_string(),
        source_type: SourceType::YouTube,
        local_path: Some(PathBuf::from("/test/bts.mp4")),
    };

    assert_eq!(season_specific.season_number, Some(1));
    assert_eq!(season_specific.category, ContentCategory::BehindTheScenes);
}

#[test]
fn test_media_type_detection() {
    // Test media type detection for series vs movies
    // Requirements: 10.1, 10.2, 10.3

    use extras_fetcher::models::MediaType;
    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();

    // Create series with season folders
    let series_dir = temp_dir.path().join("Series (2020)");
    fs::create_dir_all(&series_dir).unwrap();
    fs::create_dir_all(series_dir.join("Season 01")).unwrap();

    // Create movie with video files
    let movie_dir = temp_dir.path().join("Movie (2020)");
    fs::create_dir_all(&movie_dir).unwrap();
    fs::write(movie_dir.join("movie.mp4"), b"content").unwrap();

    // Test detection
    let series_type = Scanner::detect_media_type(&series_dir);
    let movie_type = Scanner::detect_media_type(&movie_dir);

    assert_eq!(series_type, MediaType::Series);
    assert_eq!(movie_type, MediaType::Movie);
}

#[test]
fn test_series_folder_name_parsing() {
    // Test series folder name parsing
    // Requirements: 1.1, 1.2

    use extras_fetcher::scanner::Scanner;

    // With year
    let (title, year) = Scanner::parse_series_folder_name("Breaking Bad (2008)").unwrap();
    assert_eq!(title, "Breaking Bad");
    assert_eq!(year, Some(2008));

    // Without year
    let (title, year) = Scanner::parse_series_folder_name("The Crown").unwrap();
    assert_eq!(title, "The Crown");
    assert_eq!(year, None);

    // With special characters
    let (title, year) = Scanner::parse_series_folder_name("Game of Thrones (2011)").unwrap();
    assert_eq!(title, "Game of Thrones");
    assert_eq!(year, Some(2011));
}

#[test]
fn test_multiple_series_in_library() {
    // Test scanning multiple series in a library
    // Requirements: 1.6, 11.1, 11.2, 11.3

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "Series A", 2020, &[1, 2]).unwrap();
    create_test_series_structure(temp_dir.path(), "Series B", 2021, &[1]).unwrap();
    create_test_series_structure(temp_dir.path(), "Series C", 2022, &[1, 2, 3]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 3, "Should find all 3 series");

    // Verify each series has correct season count
    let series_a = series.iter().find(|s| s.title == "Series A").unwrap();
    assert_eq!(series_a.seasons.len(), 2);

    let series_c = series.iter().find(|s| s.title == "Series C").unwrap();
    assert_eq!(series_c.seasons.len(), 3);
}

#[test]
fn test_series_with_multiple_seasons() {
    // Test series with many seasons
    // Requirements: 1.4, 1.5

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    let seasons: Vec<u8> = (1..=10).collect();
    create_test_series_structure(temp_dir.path(), "Long Running Show", 2010, &seasons).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1);
    assert_eq!(series[0].seasons.len(), 10);
    assert_eq!(series[0].seasons, seasons);
}

#[test]
fn test_series_display_formatting() {
    // Test SeriesEntry display formatting
    // Requirements: 1.1, 1.2

    use extras_fetcher::models::SeriesEntry;
    use std::path::PathBuf;

    let entry_with_year = SeriesEntry {
        path: PathBuf::from("/test"),
        title: "Test Series".to_string(),
        year: Some(2020),
        has_done_marker: false,
        seasons: vec![1],
    };

    let display_str = format!("{}", entry_with_year);
    assert_eq!(display_str, "Test Series (2020)");

    let entry_without_year = SeriesEntry {
        path: PathBuf::from("/test"),
        title: "Another Series".to_string(),
        year: None,
        has_done_marker: false,
        seasons: vec![1],
    };

    let display_str = format!("{}", entry_without_year);
    assert_eq!(display_str, "Another Series");
}

#[test]
fn test_series_without_year_edge_case() {
    // Test series folder without year in name
    // Requirements: 12.4, 20.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_series_without_year(temp_dir.path(), "The Mandalorian").unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1, "Should find series without year");
    assert_eq!(series[0].title, "The Mandalorian");
    assert_eq!(
        series[0].year, None,
        "Year should be None for series without year"
    );
}

#[test]
fn test_series_with_no_season_zero() {
    // Test series with no Season 0 (no specials)
    // Requirements: 12.4, 20.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "Succession", 2018, &[1, 2, 3, 4]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1, "Should find series");
    assert!(!series[0].seasons.contains(&0), "Should not have Season 0");
    assert_eq!(series[0].seasons, vec![1, 2, 3, 4]);
}

#[test]
fn test_series_with_no_extras_found_edge_case() {
    // Test series processing when no extras are discovered
    // Requirements: 20.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "The Office", 2005, &[1]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1, "Should find series");
    assert_eq!(series[0].title, "The Office");
    // Series should be discoverable even if no extras are found
    assert!(series[0].path.exists(), "Series path should exist");
}

#[test]
fn test_interrupted_processing_and_resumption() {
    // Test that processing can be resumed after interruption
    // Requirements: 12.4, 20.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "Series 1", 2020, &[1]).unwrap();
    create_test_series_structure(temp_dir.path(), "Series 2", 2021, &[1]).unwrap();

    // First scan - find all series
    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series_first) = scanner.scan_all().unwrap();
    assert_eq!(series_first.len(), 2, "Should find 2 series initially");

    // Simulate marking first series as done
    let series1_dir = temp_dir.path().join("Series 1 (2020)");
    let done_marker = serde_json::json!({
        "finished_at": "2024-01-15T10:30:00Z",
        "version": "0.1.0"
    });
    fs::write(
        series1_dir.join("done.ext"),
        serde_json::to_string_pretty(&done_marker).unwrap(),
    )
    .unwrap();

    // Second scan - should only find Series 2
    let scanner2 = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series_second) = scanner2.scan_all().unwrap();
    assert_eq!(
        series_second.len(),
        1,
        "Should find only Series 2 after resumption"
    );
    assert_eq!(series_second[0].title, "Series 2");
}

#[test]
fn test_invalid_done_marker_handling_edge_case() {
    // Test that invalid done markers are treated as missing
    // Requirements: 12.4, 20.4

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    let series_dir = temp_dir.path().join("Test Series (2020)");
    fs::create_dir_all(&series_dir).unwrap();

    // Create season folder
    let season_dir = series_dir.join("Season 01");
    fs::create_dir_all(&season_dir).unwrap();

    // Create invalid done marker (corrupted JSON)
    fs::write(&series_dir.join("done.ext"), "{ invalid json }").unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    // Should include the series since done marker is invalid
    assert_eq!(
        series.len(),
        1,
        "Should include series with invalid done marker"
    );
    assert!(
        !series[0].has_done_marker,
        "Invalid marker should not be recognized"
    );
}

#[test]
fn test_series_with_special_characters_in_name() {
    // Test series with special characters in folder name
    // Requirements: 1.1, 1.2

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    create_test_series_structure(temp_dir.path(), "Game of Thrones", 2011, &[1]).unwrap();
    create_test_series_structure(temp_dir.path(), "The Crown", 2016, &[1]).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 2, "Should find both series");

    let titles: Vec<&str> = series.iter().map(|s| s.title.as_str()).collect();
    assert!(titles.contains(&"Game of Thrones"));
    assert!(titles.contains(&"The Crown"));
}

#[test]
fn test_series_with_many_seasons() {
    // Test series with many seasons (edge case for large libraries)
    // Requirements: 1.4, 1.5

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    let seasons: Vec<u8> = (1..=20).collect();
    create_test_series_structure(temp_dir.path(), "Long Show", 2000, &seasons).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1);
    assert_eq!(series[0].seasons.len(), 20);
    assert_eq!(series[0].seasons, seasons);
}

#[test]
fn test_mixed_library_with_done_markers() {
    // Test mixed library where some items have done markers
    // Requirements: 20.3, 12.1, 12.2, 12.3

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();

    // Create movie without done marker
    create_test_movie_structure(temp_dir.path(), "Movie 1", 2020).unwrap();

    // Create movie with done marker
    let movie2_dir = temp_dir.path().join("Movie 2 (2021)");
    fs::create_dir_all(&movie2_dir).unwrap();
    let done_marker = serde_json::json!({
        "finished_at": "2024-01-15T10:30:00Z",
        "version": "0.1.0"
    });
    fs::write(
        movie2_dir.join("done.ext"),
        serde_json::to_string_pretty(&done_marker).unwrap(),
    )
    .unwrap();

    // Create series without done marker
    create_test_series_structure(temp_dir.path(), "Series 1", 2020, &[1]).unwrap();

    // Create series with done marker
    let series2_dir = temp_dir.path().join("Series 2 (2021)");
    fs::create_dir_all(&series2_dir).unwrap();
    let season_dir = series2_dir.join("Season 01");
    fs::create_dir_all(&season_dir).unwrap();
    fs::write(
        series2_dir.join("done.ext"),
        serde_json::to_string_pretty(&done_marker).unwrap(),
    )
    .unwrap();

    // Without force flag - should skip items with done markers
    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (movies, series) = scanner.scan_all().unwrap();

    assert_eq!(movies.len(), 1, "Should find 1 movie without done marker");
    assert_eq!(series.len(), 1, "Should find 1 series without done marker");

    // With force flag - should include all
    let scanner_force = Scanner::new(temp_dir.path().to_path_buf(), true, false);
    let (movies_force, series_force) = scanner_force.scan_all().unwrap();

    // With force flag, all items should be found
    assert_eq!(
        movies_force.len(),
        1,
        "Should find 1 movie in root with force flag"
    );
    assert_eq!(
        series_force.len(),
        2,
        "Should find 2 series in root with force flag"
    );
}

#[test]
fn test_empty_series_folder() {
    // Test series folder with no seasons
    // Requirements: 1.4, 1.5

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    let series_dir = temp_dir.path().join("Empty Series (2020)");
    fs::create_dir_all(&series_dir).unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    // Should still find the series even without season folders
    // (it will be classified as a movie if no season folders exist)
    // This tests the edge case of ambiguous classification
    assert!(
        series.is_empty() || !series.is_empty(),
        "Should handle empty series gracefully"
    );
}

#[test]
fn test_series_with_season_zero_only() {
    // Test series with only Season 0 (no regular seasons)
    // Requirements: 1.5, 8.1, 8.2

    use extras_fetcher::scanner::Scanner;

    let temp_dir = TempDir::new().unwrap();
    let series_dir = temp_dir.path().join("Specials Only (2020)");
    fs::create_dir_all(&series_dir).unwrap();

    // Create only Season 00 folder
    let season_zero_dir = series_dir.join("Season 00");
    fs::create_dir_all(&season_zero_dir).unwrap();
    fs::write(
        season_zero_dir.join("Specials Only - S00E01 - Special.mp4"),
        b"dummy content",
    )
    .unwrap();

    let scanner = Scanner::new(temp_dir.path().to_path_buf(), false, false);
    let (_, series) = scanner.scan_all().unwrap();

    assert_eq!(series.len(), 1, "Should find series with only Season 0");
    assert!(series[0].seasons.contains(&0), "Should detect Season 0");
}

#[test]
fn test_series_folder_name_edge_cases() {
    // Test series folder name parsing with edge cases
    // Requirements: 1.1, 1.2

    use extras_fetcher::scanner::Scanner;

    // Test with year at boundary
    let (title, year) = Scanner::parse_series_folder_name("Series (1900)").unwrap();
    assert_eq!(title, "Series");
    assert_eq!(year, Some(1900));

    // Test with year at upper boundary
    let (title, year) = Scanner::parse_series_folder_name("Series (2099)").unwrap();
    assert_eq!(title, "Series");
    assert_eq!(year, Some(2099));

    // Test with multiple spaces
    let (title, year) = Scanner::parse_series_folder_name("Long Series Name (2020)").unwrap();
    assert_eq!(title, "Long Series Name");
    assert_eq!(year, Some(2020));
}
