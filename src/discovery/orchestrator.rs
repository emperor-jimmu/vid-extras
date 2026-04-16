// Discovery orchestrator - coordinates discovery from all sources

use crate::models::{ContentCategory, MovieEntry, Source, SourceType, VideoSource};
use log::{info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use super::ContentDiscoverer;
use super::archive::ArchiveOrgDiscoverer;
use super::bilibili::BilibiliDiscoverer;
use super::dailymotion::DailymotionDiscoverer;
use super::kinocheck::KinoCheckDiscoverer;
use super::tmdb::TmdbDiscoverer;
use super::vimeo::VimeoDiscoverer;
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
    kinocheck: KinoCheckDiscoverer,
    dailymotion: DailymotionDiscoverer,
    vimeo: VimeoDiscoverer,
    bilibili: BilibiliDiscoverer,
    sources: Vec<Source>,
}

impl DiscoveryOrchestrator {
    /// Creates a new DiscoveryOrchestrator with the specified sources
    pub fn new(
        tmdb_api_key: String,
        sources: Vec<Source>,
        kinocheck_request_count: Arc<AtomicU32>,
        vimeo_access_token: String,
    ) -> Self {
        Self {
            tmdb: TmdbDiscoverer::new(tmdb_api_key),
            archive: ArchiveOrgDiscoverer::new(),
            youtube: YoutubeDiscoverer::new(),
            kinocheck: KinoCheckDiscoverer::new(kinocheck_request_count),
            dailymotion: DailymotionDiscoverer::new(),
            vimeo: VimeoDiscoverer::new(vimeo_access_token),
            bilibili: BilibiliDiscoverer::new(),
            sources,
        }
    }

    /// Creates a new DiscoveryOrchestrator with browser cookie authentication for YouTube
    pub fn with_cookies(
        tmdb_api_key: String,
        sources: Vec<Source>,
        browser: String,
        kinocheck_request_count: Arc<AtomicU32>,
        vimeo_access_token: String,
    ) -> Self {
        Self {
            tmdb: TmdbDiscoverer::new(tmdb_api_key),
            archive: ArchiveOrgDiscoverer::new(),
            youtube: YoutubeDiscoverer::with_cookies(browser),
            kinocheck: KinoCheckDiscoverer::new(kinocheck_request_count),
            dailymotion: DailymotionDiscoverer::new(),
            vimeo: VimeoDiscoverer::new(vimeo_access_token),
            bilibili: BilibiliDiscoverer::new(),
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
    pub async fn discover_all(
        &self,
        movie: &MovieEntry,
        _library: &[MovieEntry],
    ) -> (Vec<VideoSource>, Vec<SourceResult>, usize) {
        let mut all_sources = Vec::new();
        let mut source_results = Vec::new();
        let mut tmdb_movie_id: Option<u64> = None;

        if self.sources.contains(&Source::Tmdb) {
            match self.tmdb.discover_for_movie(movie).await {
                Ok((sources, movie_id)) => {
                    tmdb_movie_id = movie_id;
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

        // KinoCheck fallback: TMDB active + movie found on TMDB + returned 0 videos (no error).
        // Note: `tmdb_movie_id` being Some already guarantees the movie was found; the
        // `videos_found == 0 && error.is_none()` condition confirms TMDB succeeded but had
        // no videos (as opposed to a search failure or a movie not found on TMDB).
        let tmdb_found_zero = source_results
            .iter()
            .any(|r| r.source == Source::Tmdb && r.videos_found == 0 && r.error.is_none());

        if self.sources.contains(&Source::Tmdb)
            && tmdb_found_zero
            && let Some(movie_id) = tmdb_movie_id
        {
            match self.kinocheck.discover_for_tmdb_id(movie_id).await {
                Ok(sources) => {
                    info!(
                        "KinoCheck fallback found {} videos for {}",
                        sources.len(),
                        movie
                    );
                    all_sources.extend(sources);
                }
                Err(e) => {
                    info!("KinoCheck fallback failed for {}: {}", movie, e);
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
            match self.youtube.discover(movie).await {
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

        if self.sources.contains(&Source::Dailymotion) {
            match self.dailymotion.discover(&movie.title, movie.year).await {
                Ok(sources) => {
                    info!(
                        "Found {} sources from Dailymotion for {}",
                        sources.len(),
                        movie
                    );
                    source_results.push(SourceResult {
                        source: Source::Dailymotion,
                        videos_found: sources.len(),
                        error: None,
                    });
                    all_sources.extend(sources);
                }
                Err(e) => {
                    warn!("Dailymotion discovery failed for {}: {}", movie, e);
                    source_results.push(SourceResult {
                        source: Source::Dailymotion,
                        videos_found: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        if self.sources.contains(&Source::Vimeo) {
            match self.vimeo.discover(&movie.title, movie.year).await {
                Ok(sources) => {
                    info!("Found {} sources from Vimeo for {}", sources.len(), movie);
                    source_results.push(SourceResult {
                        source: Source::Vimeo,
                        videos_found: sources.len(),
                        error: None,
                    });
                    all_sources.extend(sources);
                }
                Err(e) => {
                    warn!("Vimeo discovery failed for {}: {}", movie, e);
                    source_results.push(SourceResult {
                        source: Source::Vimeo,
                        videos_found: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        if self.sources.contains(&Source::Bilibili) {
            match self.bilibili.discover(movie).await {
                Ok(sources) => {
                    info!(
                        "Found {} sources from Bilibili for {}",
                        sources.len(),
                        movie
                    );
                    source_results.push(SourceResult {
                        source: Source::Bilibili,
                        videos_found: sources.len(),
                        error: None,
                    });
                    all_sources.extend(sources);
                }
                Err(e) => {
                    warn!("Bilibili discovery failed for {}: {}", movie, e);
                    source_results.push(SourceResult {
                        source: Source::Bilibili,
                        videos_found: 0,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        // Title+duration deduplication — runs before URL dedup and content limits.
        // Prefers higher-tier sources (Tier 1 > Tier 2 > Tier 3) when duplicates are found.
        let (mut all_sources, title_dedup_removed) =
            crate::deduplication::deduplicate(all_sources, &self.sources);
        if title_dedup_removed > 0 {
            info!(
                "Removed {} title+duration duplicates for {}",
                title_dedup_removed, movie
            );
        }

        // Deduplicate by URL — safety net after title+duration dedup
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

        (all_sources, source_results, title_dedup_removed)
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
