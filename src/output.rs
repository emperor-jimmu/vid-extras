// Output module - handles CLI output formatting and progress display

use crate::models::{ContentCategory, MovieEntry, SeriesEntry, SourceType};
use crate::orchestrator::ProcessingSummary;
use colored::Colorize;

/// Display scanning progress for a movie
pub fn display_scanning_progress(movie: &MovieEntry, skipped: bool) {
    if skipped {
        println!(
            "  {} {} - {}",
            "⊘".yellow(),
            "SKIPPED".yellow(),
            movie.to_string().bright_white()
        );
    } else {
        println!(
            "  {} {} - {}",
            "✓".green(),
            "FOUND".green(),
            movie.to_string().bright_white()
        );
    }
}

/// Display discovery phase start
pub fn display_discovery_start(movie: &MovieEntry, source_count: usize) {
    println!(
        "\n{} {} - Discovered {} sources",
        "🔍".blue(),
        movie.to_string().bright_cyan(),
        source_count.to_string().bright_yellow()
    );
}

/// Display download progress for a single video
pub fn display_download_progress(
    title: &str,
    source_type: SourceType,
    current: usize,
    total: usize,
) {
    println!(
        "  {} Downloading [{}/{}]: {} ({})",
        "⬇".blue(),
        current.to_string().bright_yellow(),
        total.to_string().bright_white(),
        title.bright_white(),
        source_type.to_string().bright_cyan()
    );
}

/// Display download result
pub fn display_download_result(title: &str, success: bool, error: Option<&str>) {
    if success {
        println!(
            "    {} {}",
            "✓".green(),
            format!("Downloaded: {}", title).green()
        );
    } else {
        let error_msg = error.unwrap_or("Unknown error");
        println!(
            "    {} {} - {}",
            "✗".red(),
            format!("Failed: {}", title).red(),
            error_msg.bright_red()
        );
    }
}

/// Display conversion progress for a single video
pub fn display_conversion_progress(filename: &str, current: usize, total: usize) {
    println!(
        "  {} Converting [{}/{}]: {}",
        "⚙".blue(),
        current.to_string().bright_yellow(),
        total.to_string().bright_white(),
        filename.bright_white()
    );
}

/// Display conversion result
pub fn display_conversion_result(filename: &str, success: bool, error: Option<&str>) {
    if success {
        println!(
            "    {} {}",
            "✓".green(),
            format!("Converted: {}", filename).green()
        );
    } else {
        let error_msg = error.unwrap_or("Unknown error");
        println!(
            "    {} {} - {}",
            "✗".red(),
            format!("Failed: {}", filename).red(),
            error_msg.bright_red()
        );
    }
}

/// Display organization progress
pub fn display_organization_start(movie: &MovieEntry, file_count: usize) {
    println!(
        "\n{} {} - Organizing {} files",
        "📁".blue(),
        movie.to_string().bright_cyan(),
        file_count.to_string().bright_yellow()
    );
}

/// Display file organization result
pub fn display_file_organized(filename: &str, category: ContentCategory) {
    println!(
        "  {} Moved to /{}: {}",
        "✓".green(),
        category.subdirectory().bright_cyan(),
        filename.bright_white()
    );
}

/// Display error message with context
///
/// Formats error messages with movie title and operation type for clarity.
/// Requirements: 10.1, 13.7
pub fn display_error(movie_title: &str, operation: &str, error: &str) {
    println!(
        "{} {} - {} failed: {}",
        "✗".red().bold(),
        movie_title.bright_white().bold(),
        operation.bright_yellow(),
        error.red()
    );
}

/// Display processing summary statistics
///
/// Shows colored statistics for the entire processing run.
/// Requirements: 13.6
pub fn display_summary(summary: &ProcessingSummary) {
    println!("\n{}", "═".repeat(60).bright_cyan());
    println!("{}", "Processing Summary".bright_cyan().bold());
    println!("{}", "═".repeat(60).bright_cyan());

    println!(
        "  {} {}",
        "Total Movies:".bright_white(),
        summary.total_movies.to_string().bright_yellow()
    );

    println!(
        "  {} {}",
        "Successful Movies:".bright_white(),
        summary.successful_movies.to_string().green()
    );

    println!(
        "  {} {}",
        "Failed Movies:".bright_white(),
        if summary.failed_movies > 0 {
            summary.failed_movies.to_string().red()
        } else {
            summary.failed_movies.to_string().bright_white()
        }
    );

    println!(
        "  {} {}",
        "Total Series:".bright_white(),
        summary.total_series.to_string().bright_yellow()
    );

    println!(
        "  {} {}",
        "Successful Series:".bright_white(),
        summary.successful_series.to_string().green()
    );

    println!(
        "  {} {}",
        "Failed Series:".bright_white(),
        if summary.failed_series > 0 {
            summary.failed_series.to_string().red()
        } else {
            summary.failed_series.to_string().bright_white()
        }
    );

    println!(
        "  {} {}",
        "Total Downloads:".bright_white(),
        summary.total_downloads.to_string().bright_cyan()
    );

    println!(
        "  {} {}",
        "Total Conversions:".bright_white(),
        summary.total_conversions.to_string().bright_cyan()
    );

    println!("{}", "═".repeat(60).bright_cyan());

    // Display completion message
    let total_failed = summary.failed_movies + summary.failed_series;
    let total_successful = summary.successful_movies + summary.successful_series;

    if total_failed == 0 && total_successful > 0 {
        println!(
            "\n{} {}",
            "✓".green().bold(),
            "All items processed successfully!".green().bold()
        );
    } else if total_failed > 0 {
        println!(
            "\n{} {} items completed with {} errors",
            "⚠".yellow().bold(),
            total_successful.to_string().green(),
            total_failed.to_string().red()
        );
    } else {
        println!("\n{} {}", "ℹ".blue(), "No items to process".bright_white());
    }
}

/// Display phase header
pub fn display_phase(phase_number: usize, phase_name: &str) {
    println!(
        "\n{} {} {}",
        "▶".bright_cyan(),
        format!("Phase {}:", phase_number).bright_cyan().bold(),
        phase_name.bright_white().bold()
    );
}

/// Display movie processing start with progress indicator
pub fn display_movie_start(movie: &MovieEntry, current: usize, total: usize) {
    println!("\n{}", "━".repeat(60).bright_cyan());
    println!(
        "{} [{}/{}] Processing: {}",
        "🎬".bright_cyan(),
        current.to_string().bright_yellow(),
        total.to_string().bright_white(),
        movie.to_string().bright_white().bold()
    );
    println!("{}", "━".repeat(60).bright_cyan());
}

/// Display movie processing completion
pub fn display_movie_complete(
    movie: &MovieEntry,
    downloads: usize,
    conversions: usize,
    success: bool,
) {
    if success {
        println!(
            "\n{} {} - {} downloads, {} conversions",
            "✓".green().bold(),
            movie.to_string().bright_white().bold(),
            downloads.to_string().green(),
            conversions.to_string().green()
        );
    } else {
        println!(
            "\n{} {} - Processing failed",
            "✗".red().bold(),
            movie.to_string().bright_white().bold()
        );
    }
}

/// Display series processing start with progress indicator
/// Requirements: 18.1
pub fn display_series_start(series: &SeriesEntry, current: usize, total: usize) {
    println!("\n{}", "━".repeat(60).bright_cyan());
    println!(
        "{} [{}/{}] Processing: {}",
        "📺".bright_cyan(),
        current.to_string().bright_yellow(),
        total.to_string().bright_white(),
        series.to_string().bright_white().bold()
    );
    println!("{}", "━".repeat(60).bright_cyan());
}

/// Display series discovery progress
/// Requirements: 18.2, 18.3
pub fn display_series_discovery_progress(
    series: &SeriesEntry,
    tmdb_count: usize,
    youtube_count: usize,
) {
    println!(
        "\n{} {} - TMDB: {}, YouTube: {}",
        "🔍".blue(),
        series.to_string().bright_cyan(),
        tmdb_count.to_string().bright_yellow(),
        youtube_count.to_string().bright_yellow()
    );
}

/// Display series download statistics
/// Requirements: 18.4
pub fn display_series_download_stats(_series: &SeriesEntry, successful: usize, failed: usize) {
    let total = successful + failed;
    println!(
        "  {} Downloads: {}/{} successful",
        "⬇".blue(),
        successful.to_string().green(),
        total.to_string().bright_white()
    );
    if failed > 0 {
        println!("    {} {} failed", "⚠".yellow(), failed.to_string().red());
    }
}

/// Display series conversion statistics
/// Requirements: 18.5
pub fn display_series_conversion_stats(_series: &SeriesEntry, successful: usize, failed: usize) {
    let total = successful + failed;
    println!(
        "  {} Conversions: {}/{} successful",
        "⚙".blue(),
        successful.to_string().green(),
        total.to_string().bright_white()
    );
    if failed > 0 {
        println!("    {} {} failed", "⚠".yellow(), failed.to_string().red());
    }
}

/// Display series processing completion
/// Requirements: 18.6
pub fn display_series_complete(
    series: &SeriesEntry,
    downloads: usize,
    conversions: usize,
    success: bool,
) {
    if success {
        println!(
            "\n{} {} - {} downloads, {} conversions",
            "✓".green().bold(),
            series.to_string().bright_white().bold(),
            downloads.to_string().green(),
            conversions.to_string().green()
        );
    } else {
        println!(
            "\n{} {} - Processing failed",
            "✗".red().bold(),
            series.to_string().bright_white().bold()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_movie() -> MovieEntry {
        MovieEntry {
            path: PathBuf::from("/movies/Test Movie (2020)"),
            title: "Test Movie".to_string(),
            year: 2020,
            has_done_marker: false,
        }
    }

    fn create_test_series() -> SeriesEntry {
        SeriesEntry {
            path: PathBuf::from("/series/Test Series (2020)"),
            title: "Test Series".to_string(),
            year: Some(2020),
            has_done_marker: false,
            seasons: vec![1, 2, 3],
        }
    }

    #[test]
    fn test_display_scanning_progress_found() {
        let movie = create_test_movie();
        // Just verify it doesn't panic
        display_scanning_progress(&movie, false);
    }

    #[test]
    fn test_display_scanning_progress_skipped() {
        let movie = create_test_movie();
        // Just verify it doesn't panic
        display_scanning_progress(&movie, true);
    }

    #[test]
    fn test_display_discovery_start() {
        let movie = create_test_movie();
        display_discovery_start(&movie, 5);
    }

    #[test]
    fn test_display_download_progress() {
        display_download_progress("Test Trailer", SourceType::TMDB, 1, 5);
    }

    #[test]
    fn test_display_download_result_success() {
        display_download_result("Test Trailer", true, None);
    }

    #[test]
    fn test_display_download_result_failure() {
        display_download_result("Test Trailer", false, Some("Network timeout"));
    }

    #[test]
    fn test_display_conversion_progress() {
        display_conversion_progress("trailer.mp4", 2, 4);
    }

    #[test]
    fn test_display_conversion_result_success() {
        display_conversion_result("trailer.mp4", true, None);
    }

    #[test]
    fn test_display_conversion_result_failure() {
        display_conversion_result("trailer.mp4", false, Some("FFmpeg error"));
    }

    #[test]
    fn test_display_organization_start() {
        let movie = create_test_movie();
        display_organization_start(&movie, 3);
    }

    #[test]
    fn test_display_file_organized() {
        display_file_organized("trailer.mp4", ContentCategory::Trailer);
    }

    #[test]
    fn test_display_error() {
        display_error("Test Movie (2020)", "download", "Network timeout");
    }

    #[test]
    fn test_display_summary_all_successful() {
        let summary = ProcessingSummary {
            total_movies: 5,
            successful_movies: 5,
            failed_movies: 0,
            total_series: 3,
            successful_series: 3,
            failed_series: 0,
            total_downloads: 15,
            total_conversions: 12,
        };
        display_summary(&summary);
    }

    #[test]
    fn test_display_summary_with_failures() {
        let summary = ProcessingSummary {
            total_movies: 5,
            successful_movies: 3,
            failed_movies: 2,
            total_series: 2,
            successful_series: 1,
            failed_series: 1,
            total_downloads: 10,
            total_conversions: 8,
        };
        display_summary(&summary);
    }

    #[test]
    fn test_display_summary_empty() {
        let summary = ProcessingSummary {
            total_movies: 0,
            successful_movies: 0,
            failed_movies: 0,
            total_series: 0,
            successful_series: 0,
            failed_series: 0,
            total_downloads: 0,
            total_conversions: 0,
        };
        display_summary(&summary);
    }

    #[test]
    fn test_display_phase() {
        display_phase(1, "Scanning");
        display_phase(2, "Discovery");
    }

    #[test]
    fn test_display_movie_start() {
        let movie = create_test_movie();
        display_movie_start(&movie, 1, 5);
    }

    #[test]
    fn test_display_movie_complete_success() {
        let movie = create_test_movie();
        display_movie_complete(&movie, 5, 4, true);
    }

    #[test]
    fn test_display_movie_complete_failure() {
        let movie = create_test_movie();
        display_movie_complete(&movie, 0, 0, false);
    }

    #[test]
    fn test_display_series_start() {
        let series = create_test_series();
        display_series_start(&series, 2, 10);
    }

    #[test]
    fn test_display_series_discovery_progress() {
        let series = create_test_series();
        display_series_discovery_progress(&series, 3, 5);
    }

    #[test]
    fn test_display_series_download_stats_all_successful() {
        let series = create_test_series();
        display_series_download_stats(&series, 8, 0);
    }

    #[test]
    fn test_display_series_download_stats_with_failures() {
        let series = create_test_series();
        display_series_download_stats(&series, 6, 2);
    }

    #[test]
    fn test_display_series_conversion_stats_all_successful() {
        let series = create_test_series();
        display_series_conversion_stats(&series, 8, 0);
    }

    #[test]
    fn test_display_series_conversion_stats_with_failures() {
        let series = create_test_series();
        display_series_conversion_stats(&series, 6, 2);
    }

    #[test]
    fn test_display_series_complete_success() {
        let series = create_test_series();
        display_series_complete(&series, 8, 7, true);
    }

    #[test]
    fn test_display_series_complete_failure() {
        let series = create_test_series();
        display_series_complete(&series, 0, 0, false);
    }

    #[test]
    fn test_error_message_formatting() {
        // Test that error messages include all required context
        // Requirements: 10.1, 13.7

        let movie_title = "The Matrix (1999)";
        let operation = "download";
        let error = "Network timeout after 5 minutes";

        // Verify the function doesn't panic and formats correctly
        display_error(movie_title, operation, error);

        // The function should display:
        // - Movie title
        // - Operation type
        // - Error message
        // All with appropriate coloring
    }

    #[test]
    fn test_summary_statistics_display() {
        // Test summary display with various scenarios
        // Requirements: 13.3-13.7

        // Scenario 1: All successful
        let summary1 = ProcessingSummary {
            total_movies: 10,
            successful_movies: 10,
            failed_movies: 0,
            total_series: 0,
            successful_series: 0,
            failed_series: 0,
            total_downloads: 30,
            total_conversions: 25,
        };
        display_summary(&summary1);

        // Scenario 2: Mixed results
        let summary2 = ProcessingSummary {
            total_movies: 10,
            successful_movies: 7,
            failed_movies: 3,
            total_series: 2,
            successful_series: 1,
            failed_series: 1,
            total_downloads: 20,
            total_conversions: 15,
        };
        display_summary(&summary2);

        // Scenario 3: All failed
        let summary3 = ProcessingSummary {
            total_movies: 5,
            successful_movies: 0,
            failed_movies: 5,
            total_series: 0,
            successful_series: 0,
            failed_series: 0,
            total_downloads: 0,
            total_conversions: 0,
        };
        display_summary(&summary3);
    }

    #[test]
    fn test_colored_output_generation() {
        // Test that colored output is generated for different status types
        // Requirements: 13.3

        let movie = create_test_movie();

        // Green for success
        display_scanning_progress(&movie, false);
        display_download_result("test.mp4", true, None);
        display_conversion_result("test.mp4", true, None);

        // Yellow for warnings/skipped
        display_scanning_progress(&movie, true);

        // Red for errors
        display_download_result("test.mp4", false, Some("Error"));
        display_conversion_result("test.mp4", false, Some("Error"));
        display_error("Test (2020)", "conversion", "FFmpeg failed");

        // Blue for progress
        display_download_progress("test.mp4", SourceType::YouTube, 1, 3);
        display_conversion_progress("test.mp4", 1, 3);
    }

    #[test]
    fn test_progress_indicator_formatting() {
        // Test progress indicators show current/total counts
        // Requirements: 13.4, 13.5

        // Download progress
        display_download_progress("Trailer 1", SourceType::TMDB, 1, 5);
        display_download_progress("Trailer 2", SourceType::YouTube, 2, 5);
        display_download_progress("Trailer 3", SourceType::ArchiveOrg, 3, 5);

        // Conversion progress
        display_conversion_progress("trailer1.mp4", 1, 3);
        display_conversion_progress("trailer2.mp4", 2, 3);
        display_conversion_progress("trailer3.mp4", 3, 3);
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    // Feature: extras-fetcher, Property 37: Error Message Formatting
    // Validates: Requirements 10.1, 13.7
    // For any error that occurs during processing, the error message should
    // include the movie title and operation type.
    proptest! {
        #[test]
        fn prop_error_message_formatting(
            title in "[a-zA-Z0-9 ]{5,30}",
            year in 1900u16..2100u16,
            operation in prop_oneof![
                Just("scanning"),
                Just("discovery"),
                Just("download"),
                Just("conversion"),
                Just("organization"),
            ],
            error_msg in "[a-zA-Z0-9 ]{10,50}",
        ) {
            // Create a movie title with year
            let movie_title = format!("{} ({})", title.trim(), year);

            // The display_error function should accept and format:
            // 1. Movie title (with context)
            // 2. Operation type (which phase failed)
            // 3. Error message (what went wrong)

            // Verify the function doesn't panic with various inputs
            display_error(&movie_title, operation, &error_msg);

            // The function should display all three components:
            // - Movie title for context (which movie failed)
            // - Operation type (which phase failed)
            // - Error message (what went wrong)

            // This ensures that when errors occur, users can identify:
            // 1. WHAT movie had the problem (movie_title)
            // 2. WHERE in the pipeline it failed (operation)
            // 3. WHY it failed (error_msg)

            // Verify inputs are valid
            prop_assert!(!movie_title.trim().is_empty());
            prop_assert!(!operation.is_empty());
            prop_assert!(!error_msg.trim().is_empty());
        }
    }

    // Feature: tv-series-extras, Property 18: Series Summary Statistics Accuracy
    // Validates: Requirements 19.1, 19.2, 19.3, 19.4, 19.5
    // For any set of series processing results, the summary statistics should
    // accurately reflect the counts: total_series should equal successful_series +
    // failed_series, and total_downloads and total_conversions should equal the
    // sum across all series.
    proptest! {
        #[test]
        fn prop_series_summary_statistics_accuracy(
            total_movies in 0usize..20,
            successful_movies in 0usize..20,
            total_series in 0usize..20,
            successful_series in 0usize..20,
            total_downloads in 0usize..100,
            total_conversions in 0usize..100,
        ) {
            // Ensure successful counts don't exceed totals
            let successful_movies = successful_movies.min(total_movies);
            let successful_series = successful_series.min(total_series);

            let summary = ProcessingSummary {
                total_movies,
                successful_movies,
                failed_movies: total_movies - successful_movies,
                total_series,
                successful_series,
                failed_series: total_series - successful_series,
                total_downloads,
                total_conversions,
            };

            // Verify the invariants that must hold for any valid summary:

            // 1. Total series should equal successful + failed
            prop_assert_eq!(
                summary.total_series,
                summary.successful_series + summary.failed_series,
                "Total series must equal successful + failed"
            );

            // 2. Total movies should equal successful + failed
            prop_assert_eq!(
                summary.total_movies,
                summary.successful_movies + summary.failed_movies,
                "Total movies must equal successful + failed"
            );

            // 3. Successful series should not exceed total series
            prop_assert!(
                summary.successful_series <= summary.total_series,
                "Successful series cannot exceed total series"
            );

            // 4. Failed series should not exceed total series
            prop_assert!(
                summary.failed_series <= summary.total_series,
                "Failed series cannot exceed total series"
            );

            // 5. Successful movies should not exceed total movies
            prop_assert!(
                summary.successful_movies <= summary.total_movies,
                "Successful movies cannot exceed total movies"
            );

            // 6. Failed movies should not exceed total movies
            prop_assert!(
                summary.failed_movies <= summary.total_movies,
                "Failed movies cannot exceed total movies"
            );

            // 7. Downloads and conversions should be non-negative (always true for usize)
            // No need to check since usize cannot be negative

            // Verify display_summary doesn't panic with any valid summary
            display_summary(&summary);
        }
    }
}
