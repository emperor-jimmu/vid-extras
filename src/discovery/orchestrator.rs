// Discovery orchestrator - coordinates discovery from all sources

use crate::models::{ContentCategory, MovieEntry, Source, SourceType, VideoSource};
use log::info;
use std::collections::HashMap;

use super::ContentDiscoverer;
use super::archive::ArchiveOrgDiscoverer;
use super::tmdb::TmdbDiscoverer;
use super::youtube::YoutubeDiscoverer;

/// Orchestrates discovery from all sources
pub struct DiscoveryOrchestrator {
    tmdb: TmdbDiscoverer,
    archive: ArchiveOrgDiscoverer,
    youtube: YoutubeDiscoverer,
    sources: Vec<Source>,
}

impl DiscoveryOrchestrator {
    /// Creates a new DiscoveryOrchestrator with the specified sources
    pub fn new(tmdb_api_key: String, sources: Vec<Source>) -> Self {
        Self {
            tmdb: TmdbDiscoverer::new(tmdb_api_key),
            archive: ArchiveOrgDiscoverer::new(),
            youtube: YoutubeDiscoverer::new(),
            sources,
        }
    }

    /// Creates a new DiscoveryOrchestrator with browser cookie authentication for YouTube
    pub fn with_cookies(tmdb_api_key: String, sources: Vec<Source>, browser: String) -> Self {
        Self {
            tmdb: TmdbDiscoverer::new(tmdb_api_key),
            archive: ArchiveOrgDiscoverer::new(),
            youtube: YoutubeDiscoverer::with_cookies(browser),
            sources,
        }
    }

    /// Discovers video sources from all configured sources
    ///
    /// Queries each enabled source and aggregates results.
    /// Applies content limits per category:
    /// - Trailers: max 4
    /// - Deleted Scenes: max 8
    /// - Interviews: max 8
    /// - Featurettes: max 10
    /// - Behind the Scenes: max 10
    ///
    /// When limits are exceeded, prioritizes TMDB > Archive.org > YouTube
    pub async fn discover_all(&self, movie: &MovieEntry) -> Vec<VideoSource> {
        let mut all_sources = Vec::new();

        // Get metadata from TMDB (collection info for YouTube filtering)
        let metadata = self.tmdb.get_metadata(movie).await;

        if self.sources.contains(&Source::Tmdb) {
            match self.tmdb.discover(movie).await {
                Ok(sources) => {
                    info!("Found {} sources from TMDB for {}", sources.len(), movie);
                    all_sources.extend(sources);
                }
                Err(e) => info!("TMDB discovery failed for {}: {}", movie, e),
            }
        }

        if self.sources.contains(&Source::Archive) {
            match self.archive.discover(movie).await {
                Ok(sources) => {
                    info!(
                        "Found {} sources from Archive.org for {}",
                        sources.len(),
                        movie
                    );
                    all_sources.extend(sources);
                }
                Err(e) => info!("Archive.org discovery failed for {}: {}", movie, e),
            }
        }

        if self.sources.contains(&Source::Youtube) {
            match self.youtube.discover_with_metadata(movie, &metadata).await {
                Ok(sources) => {
                    info!("Found {} sources from YouTube for {}", sources.len(), movie);
                    all_sources.extend(sources);
                }
                Err(e) => info!("YouTube discovery failed for {}: {}", movie, e),
            }
        }

        // Dailymotion, Vimeo, Bilibili stubs — discoverers not yet implemented.
        // Log when a user-requested source is skipped so it's not silently ignored.
        for source in &self.sources {
            match source {
                Source::Dailymotion | Source::Vimeo | Source::Bilibili => {
                    info!(
                        "{} source requested but discoverer not yet implemented — skipping for {}",
                        source, movie
                    );
                }
                _ => {} // handled above
            }
        }

        // Deduplicate by URL
        let initial_count = all_sources.len();
        all_sources.sort_by(|a, b| a.url.cmp(&b.url));
        all_sources.dedup_by(|a, b| a.url == b.url);

        if all_sources.len() < initial_count {
            info!(
                "Removed {} duplicate URL(s) for {}",
                initial_count - all_sources.len(),
                movie
            );
        }

        // Apply content limits per category
        let before_limit = all_sources.len();
        all_sources = Self::apply_content_limits(all_sources);

        if all_sources.len() < before_limit {
            info!(
                "Applied content limits, reduced from {} to {} sources for {}",
                before_limit,
                all_sources.len(),
                movie
            );
        }

        info!(
            "Total sources discovered for {}: {}",
            movie,
            all_sources.len()
        );
        all_sources
    }

    /// Apply content limits per category, prioritizing TMDB > Archive.org > YouTube
    fn apply_content_limits(sources: Vec<VideoSource>) -> Vec<VideoSource> {
        use ContentCategory::*;

        let limits: HashMap<ContentCategory, usize> = [
            (Trailer, 4),
            (DeletedScene, 8),
            (Interview, 8),
            (Featurette, 10),
            (BehindTheScenes, 10),
        ]
        .iter()
        .cloned()
        .collect();

        let mut by_category: HashMap<ContentCategory, Vec<VideoSource>> = HashMap::new();
        for source in sources {
            by_category.entry(source.category).or_default().push(source);
        }

        let mut limited_sources = Vec::new();
        for (category, mut sources) in by_category {
            let limit = limits.get(&category).copied().unwrap_or(usize::MAX);

            // Sort by source priority: TMDB (0) > Archive.org (1) > YouTube/others (2+)
            sources.sort_by_key(|s| match s.source_type {
                SourceType::TMDB => 0u8,
                SourceType::ArchiveOrg => 1,
                SourceType::KinoCheck => 1,
                SourceType::Dailymotion => 2,
                SourceType::Vimeo => 2,
                SourceType::YouTube => 3,
                SourceType::Bilibili => 3,
                SourceType::TheTVDB => 4,
            });

            sources.truncate(limit);
            limited_sources.extend(sources);
        }

        limited_sources
    }
}
