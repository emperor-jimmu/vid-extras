// Discovery orchestrator - coordinates discovery from all sources

use crate::models::{ContentCategory, MovieEntry, SourceMode, SourceType, VideoSource};
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
    mode: SourceMode,
}

impl DiscoveryOrchestrator {
    /// Creates a new DiscoveryOrchestrator with the specified mode
    pub fn new(tmdb_api_key: String, mode: SourceMode) -> Self {
        Self {
            tmdb: TmdbDiscoverer::new(tmdb_api_key),
            archive: ArchiveOrgDiscoverer::new(),
            youtube: YoutubeDiscoverer::new(),
            mode,
        }
    }

    /// Discovers video sources from all configured sources based on mode
    ///
    /// In All mode: queries TMDB, Archive.org (for movies < 2010), and YouTube
    /// In YoutubeOnly mode: queries only YouTube
    ///
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

        match self.mode {
            SourceMode::All => {
                // Query TMDB
                match self.tmdb.discover(movie).await {
                    Ok(sources) => {
                        info!("Found {} sources from TMDB for {}", sources.len(), movie);
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        info!("TMDB discovery failed for {}: {}", movie, e);
                    }
                }

                // Query Archive.org only for movies before 2010
                if movie.year < 2010 {
                    match self.archive.discover(movie).await {
                        Ok(sources) => {
                            info!(
                                "Found {} sources from Archive.org for {}",
                                sources.len(),
                                movie
                            );
                            all_sources.extend(sources);
                        }
                        Err(e) => {
                            info!("Archive.org discovery failed for {}: {}", movie, e);
                        }
                    }
                } else {
                    info!("Skipping Archive.org for {} (year >= 2010)", movie);
                }

                // Query YouTube with metadata for better filtering
                match self.youtube.discover_with_metadata(movie, &metadata).await {
                    Ok(sources) => {
                        info!("Found {} sources from YouTube for {}", sources.len(), movie);
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        info!("YouTube discovery failed for {}: {}", movie, e);
                    }
                }
            }
            SourceMode::YoutubeOnly => {
                // Query only YouTube with metadata
                match self.youtube.discover_with_metadata(movie, &metadata).await {
                    Ok(sources) => {
                        info!("Found {} sources from YouTube for {}", sources.len(), movie);
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        info!("YouTube discovery failed for {}: {}", movie, e);
                    }
                }
            }
        }

        // Deduplicate by URL to avoid downloading the same video twice
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

        // Define limits per category
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

        // Group sources by category
        let mut by_category: HashMap<ContentCategory, Vec<VideoSource>> = HashMap::new();
        for source in sources {
            by_category.entry(source.category).or_default().push(source);
        }

        // Apply limits to each category, prioritizing by source type
        let mut limited_sources = Vec::new();
        for (category, mut sources) in by_category {
            let limit = limits.get(&category).copied().unwrap_or(usize::MAX);

            // Sort by source priority: TMDB (0) > Archive.org (1) > YouTube (2)
            sources.sort_by_key(|s| match s.source_type {
                SourceType::TMDB => 0,
                SourceType::ArchiveOrg => 1,
                SourceType::YouTube => 2,
            });

            // Take only up to the limit
            sources.truncate(limit);
            limited_sources.extend(sources);
        }

        limited_sources
    }
}
