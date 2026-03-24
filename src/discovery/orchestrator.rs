// Discovery orchestrator - coordinates discovery from all sources

use crate::models::{ContentCategory, MovieEntry, Source, SourceType, VideoSource};
use log::{info, warn};
use std::collections::HashMap;

use super::ContentDiscoverer;
use super::archive::ArchiveOrgDiscoverer;
use super::tmdb::TmdbDiscoverer;
use super::youtube::YoutubeDiscoverer;

/// Result of querying a single discovery source
#[derive(Debug, Clone)]
pub struct SourceResult {
    /// Which source was queried
    pub source: Source,
    /// Number of videos returned by this source before deduplication and content limits are applied
    pub videos_found: usize,
    /// Error message if the source failed, None on success
    pub error: Option<String>,
}

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
    ///
    /// `library` is the full scanned movie list. TMDB collection siblings already present
    /// in the library are skipped — they will fetch their own extras when processed.
    pub async fn discover_all(
        &self,
        movie: &MovieEntry,
        library: &[MovieEntry],
    ) -> (Vec<VideoSource>, Vec<SourceResult>) {
        let mut all_sources = Vec::new();
        let mut source_results = Vec::new();

        // Fetch TMDB collection metadata only when at least one source that uses it is active.
        // YouTube filtering uses collection titles to exclude trailers for other films in the same
        // franchise; TMDB discovery uses it implicitly via search. Skip the extra network call
        // when neither source is requested.
        let needs_metadata =
            self.sources.contains(&Source::Tmdb) || self.sources.contains(&Source::Youtube);
        let metadata = if needs_metadata {
            self.tmdb.get_metadata(movie).await
        } else {
            Default::default()
        };

        if self.sources.contains(&Source::Tmdb) {
            match self.tmdb.discover_with_library(movie, library).await {
                Ok(sources) => {
                    info!("Found {} sources from TMDB for {}", sources.len(), movie);
                    source_results.push(SourceResult {
                        source: Source::Tmdb,
                        videos_found: sources.len(),
                        error: None,
                    });
                    all_sources.extend(sources);
                }
                Err(e) => {
                    warn!("TMDB discovery failed for {}: {}", movie, e);
                    source_results.push(SourceResult {
                        source: Source::Tmdb,
                        videos_found: 0,
                        error: Some(e.to_string()),
                    });
                }
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
                    source_results.push(SourceResult {
                        source: Source::Archive,
                        videos_found: sources.len(),
                        error: None,
                    });
                    all_sources.extend(sources);
                }
                Err(e) => {
                    warn!("Archive.org discovery failed for {}: {}", movie, e);
                    source_results.push(SourceResult {
                        source: Source::Archive,
                        videos_found: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        if self.sources.contains(&Source::Youtube) {
            match self.youtube.discover_with_metadata(movie, &metadata).await {
                Ok(sources) => {
                    info!("Found {} sources from YouTube for {}", sources.len(), movie);
                    source_results.push(SourceResult {
                        source: Source::Youtube,
                        videos_found: sources.len(),
                        error: None,
                    });
                    all_sources.extend(sources);
                }
                Err(e) => {
                    warn!("YouTube discovery failed for {}: {}", movie, e);
                    source_results.push(SourceResult {
                        source: Source::Youtube,
                        videos_found: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        // Dailymotion, Vimeo, Bilibili stubs — discoverers not yet implemented.
        // Log when a user-requested source is skipped so it's not silently ignored.
        // These are NOT added to source_results as errors — they are intentionally
        // unimplemented stubs, not runtime failures. Including them as errors would
        // make every default run (which includes Dailymotion) appear to have failures.
        for source in &self.sources {
            match source {
                Source::Dailymotion | Source::Vimeo | Source::Bilibili => {
                    warn!(
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

        // Log per-source summary
        for sr in &source_results {
            if let Some(ref err) = sr.error {
                warn!("  {} — failed: {}", sr.source, err);
            } else {
                info!("  {} — {} videos", sr.source, sr.videos_found);
            }
        }

        (all_sources, source_results)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_result_success() {
        let result = SourceResult {
            source: Source::Tmdb,
            videos_found: 5,
            error: None,
        };
        assert_eq!(result.source, Source::Tmdb);
        assert_eq!(result.videos_found, 5);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_source_result_failure() {
        let result = SourceResult {
            source: Source::Youtube,
            videos_found: 0,
            error: Some("Network timeout".to_string()),
        };
        assert_eq!(result.source, Source::Youtube);
        assert_eq!(result.videos_found, 0);
        assert_eq!(result.error.as_deref(), Some("Network timeout"));
    }

    #[test]
    fn test_source_result_with_zero_videos_and_error() {
        // A SourceResult can represent any source that returned an error
        let result = SourceResult {
            source: Source::Dailymotion,
            videos_found: 0,
            error: Some("connection refused".to_string()),
        };
        assert_eq!(result.source, Source::Dailymotion);
        assert!(result.error.is_some());
        assert_eq!(result.videos_found, 0);
    }

    #[test]
    fn test_source_result_clone() {
        let result = SourceResult {
            source: Source::Archive,
            videos_found: 3,
            error: None,
        };
        let cloned = result.clone();
        assert_eq!(cloned.source, result.source);
        assert_eq!(cloned.videos_found, result.videos_found);
        assert_eq!(cloned.error, result.error);
    }

    #[test]
    fn test_source_result_debug() {
        let result = SourceResult {
            source: Source::Tmdb,
            videos_found: 2,
            error: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Tmdb"));
        assert!(debug_str.contains("2"));
    }

    #[test]
    fn test_source_result_filtering_by_error_state() {
        // Verify the SourceResult struct can represent all source states
        let results = vec![
            SourceResult {
                source: Source::Tmdb,
                videos_found: 3,
                error: None,
            },
            SourceResult {
                source: Source::Archive,
                videos_found: 0,
                error: Some("API error".to_string()),
            },
            SourceResult {
                source: Source::Youtube,
                videos_found: 7,
                error: None,
            },
        ];

        let successful: Vec<_> = results.iter().filter(|r| r.error.is_none()).collect();
        let failed: Vec<_> = results.iter().filter(|r| r.error.is_some()).collect();

        assert_eq!(successful.len(), 2);
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].source, Source::Archive);
    }
}
