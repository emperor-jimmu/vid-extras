//! Human-readable CLI views. Rendered by the CLI sink in `progress`; no other module
//! prints progress.

use crate::discovery::SourceResult;
use crate::models::SourceType;
use crate::orchestrator::ProcessingSummary;
use crate::progress::{ItemInfo, ItemKind};
use colored::Colorize;

/// Header for one library item.
pub fn display_item_start(item: &ItemInfo, current: usize, total: usize) {
    let icon = match item.kind {
        ItemKind::Movie => "🎬",
        ItemKind::Series => "📺",
    };
    println!("\n{}", "━".repeat(60).bright_cyan());
    println!(
        "{} [{}/{}] Processing: {}",
        icon.bright_cyan(),
        current.to_string().bright_yellow(),
        total.to_string().bright_white(),
        item.label().bright_white().bold()
    );
    println!("{}", "━".repeat(60).bright_cyan());
}

/// Conversion progress for a single file.
pub fn display_conversion_progress(filename: &str, current: usize, total: usize) {
    println!(
        "  {} Converting [{}/{}]: {}",
        "⚙".blue(),
        current.to_string().bright_yellow(),
        total.to_string().bright_white(),
        filename.bright_white()
    );
}

/// Per-source discovery results in dry-run mode.
pub fn display_dry_run_results(
    item: &ItemInfo,
    source_results: &[SourceResult],
    total: usize,
    duplicates_removed: usize,
) {
    let noun = match item.kind {
        ItemKind::Movie => "videos",
        ItemKind::Series => "extras",
    };
    println!(
        "\n  {} {} {}",
        "[DRY RUN]".bright_yellow().bold(),
        "Discovery results for".bright_white(),
        item.label().bright_cyan()
    );
    for sr in source_results {
        if let Some(ref err) = sr.error {
            println!(
                "    {} {} {}",
                format!("{:>12}", sr.source).bright_white(),
                "—".bright_white(),
                format!("failed: {}", err).red()
            );
        } else {
            println!(
                "    {} {} {}",
                format!("{:>12}", sr.source).bright_white(),
                "—".bright_white(),
                format!("{} {}", sr.videos_found, noun).bright_cyan()
            );
        }
    }
    println!(
        "    {} {} {}",
        "Total:".bright_white().bold(),
        total.to_string().bright_yellow(),
        format!("{} (would download)", noun).bright_white()
    );
    if duplicates_removed > 0 {
        println!(
            "    {} {} {}",
            "Duplicates:".bright_white().bold(),
            duplicates_removed.to_string().bright_magenta(),
            "removed (tier dedup)".bright_white()
        );
    }
}

/// Display processing summary statistics
///
/// Shows colored statistics for the entire processing run.
/// Requirements: 13.6
pub fn display_summary(summary: &ProcessingSummary) {
    println!("\n{}", "═".repeat(60).bright_cyan());
    println!("{}", "Processing Summary".bright_cyan().bold());
    println!("{}", "═".repeat(60).bright_cyan());

    if summary.total_movies > 0 {
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
    }

    if summary.total_series > 0 {
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
    }

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

    // Per-source discovery section
    if !summary.source_totals.is_empty() {
        let mut entries: Vec<(&SourceType, &usize)> = summary.source_totals.iter().collect();
        entries.sort_by_key(|(s, _)| (s.tier(), s.to_string()));
        println!("  {}", "─".repeat(45).bright_white());
        println!("  {}", "Discovery:".bright_white().bold());
        for (source, count) in &entries {
            println!(
                "    {} {} {}",
                format!("{:>12}:", source.to_string().to_uppercase()).bright_white(),
                count.to_string().bright_cyan(),
                "videos".bright_white()
            );
        }
        println!(
            "  {} {} {}",
            "Total Discovered:".bright_white().bold(),
            summary.total_videos_discovered.to_string().bright_yellow(),
            "videos".bright_white()
        );
        if summary.duplicates_removed > 0 {
            println!(
                "  {} {} {}",
                "Duplicates:".bright_white().bold(),
                summary.duplicates_removed.to_string().bright_magenta(),
                "removed (tier dedup)".bright_white()
            );
        }
        println!(
            "  {} {} {}",
            "Total Downloaded:".bright_white().bold(),
            summary.total_downloads.to_string().green(),
            "videos".bright_white()
        );
    }

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
