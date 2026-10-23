// Discovery module — turns discovery queries into candidates plus a per-source-type report.
//
// One interface: a query list in, one report out. Behind it: identity resolution,
// sequential fan-out over the enabled sources, deduplication, and content limits.

use crate::deduplication::deduplicate;
use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SeriesEntry, Source, SourceType, VideoSource};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

use super::archive::ArchiveOrgDiscoverer;
use super::bilibili::BilibiliDiscoverer;
use super::dailymotion::DailymotionDiscoverer;
use super::kinocheck::KinoCheckDiscoverer;
use super::series_tmdb::TmdbSeriesDiscoverer;
use super::series_youtube::YoutubeSeriesDiscoverer;
use super::tmdb::TmdbDiscoverer;
use super::vimeo::VimeoDiscoverer;
use super::youtube::YoutubeDiscoverer;

/// Which kind of library item a discovery query targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryKind {
    Movie,
    Series,
}

/// Per-category content limits applied after deduplication.
#[derive(Debug, Clone)]
pub struct ContentLimits {
    limits: Vec<(ContentCategory, usize)>,
}

impl ContentLimits {
    /// One policy for both pipelines: per-category caps on candidates.
    pub fn per_category() -> Self {
        Self {
            limits: vec![
                (ContentCategory::Trailer, 4),
                (ContentCategory::DeletedScene, 8),
                (ContentCategory::Interview, 8),
                (ContentCategory::Featurette, 10),
                (ContentCategory::BehindTheScenes, 10),
            ],
        }
    }

    /// No caps. Intended for tests and diagnostics.
    pub fn none() -> Self {
        Self { limits: Vec::new() }
    }

    fn limit_for(&self, category: ContentCategory) -> Option<usize> {
        self.limits
            .iter()
            .find(|(c, _)| *c == category)
            .map(|(_, limit)| *limit)
    }
}

impl Default for ContentLimits {
    fn default() -> Self {
        Self::per_category()
    }
}

/// A normalized discovery request.
#[derive(Debug, Clone)]
pub struct DiscoveryQuery {
    pub kind: LibraryKind,
    pub title: String,
    pub year: Option<u16>,
    /// Seasons available on disk. Empty for movies.
    pub seasons: Vec<u8>,
    /// `Some` for a season-scoped query, `None` for an item-level query.
    pub season: Option<u8>,
    /// Enabled sources, in the order they should be queried.
    pub sources: Vec<Source>,
    pub limits: ContentLimits,
}

impl DiscoveryQuery {
    /// Item-level query for a movie.
    pub fn movie(movie: &MovieEntry, sources: Vec<Source>) -> Self {
        Self {
            kind: LibraryKind::Movie,
            title: movie.title.clone(),
            year: Some(movie.year),
            seasons: Vec::new(),
            season: None,
            sources,
            limits: ContentLimits::per_category(),
        }
    }

    /// Item-level query for a series.
    pub fn series(series: &SeriesEntry, sources: Vec<Source>) -> Self {
        Self {
            kind: LibraryKind::Series,
            title: series.title.clone(),
            year: series.year,
            seasons: series.seasons.clone(),
            season: None,
            sources,
            limits: ContentLimits::per_category(),
        }
    }

    /// The same query scoped to one season.
    pub fn for_season(&self, season: u8) -> Self {
        Self {
            season: Some(season),
            ..self.clone()
        }
    }
}

/// Outcome of querying one source type.
#[derive(Debug, Clone)]
pub struct SourceResult {
    /// Medium that produced the candidates
    pub source: SourceType,
    /// Candidates returned after the adapter's own filtering, before deduplication and limits.
    /// When a source is queried more than once for the same item, counts are summed.
    pub videos_found: usize,
    /// First error reported by the source, if any
    pub error: Option<String>,
}

/// Everything one discovery run produced.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryReport {
    pub candidates: Vec<VideoSource>,
    pub source_results: Vec<SourceResult>,
    pub duplicates_removed: usize,
    pub limits_removed: usize,
}

/// Interface implemented by every source adapter.
#[allow(async_fn_in_trait)]
pub(crate) trait CandidateSource {
    async fn discover(&self, query: &DiscoveryQuery) -> Result<Vec<VideoSource>, DiscoveryError>;
}

/// Configuration for the discovery module.
pub struct DiscoveryConfig {
    pub tmdb_api_key: String,
    pub vimeo_access_token: String,
    pub cookies_from_browser: Option<String>,
    pub kinocheck_request_count: Arc<AtomicU32>,
}

/// Deep module: one interface, nine adapters behind it.
pub struct Discovery {
    tmdb: TmdbDiscoverer,
    tmdb_series: TmdbSeriesDiscoverer,
    archive: ArchiveOrgDiscoverer,
    youtube: YoutubeDiscoverer,
    youtube_series: YoutubeSeriesDiscoverer,
    dailymotion: DailymotionDiscoverer,
    vimeo: VimeoDiscoverer,
    bilibili: BilibiliDiscoverer,
    kinocheck: KinoCheckDiscoverer,
    #[cfg(test)]
    stubs: Vec<(Source, Vec<VideoSource>)>,
}

/// Candidates and per-source totals collected from the fan-out, before assembly.
#[derive(Default)]
struct Collected {
    candidates: Vec<VideoSource>,
    totals: Vec<(SourceType, usize, Option<String>)>,
}

impl Collected {
    fn record(&mut self, source: SourceType, result: Result<Vec<VideoSource>, DiscoveryError>) {
        let entry = match self.totals.iter_mut().find(|(s, _, _)| *s == source) {
            Some(entry) => entry,
            None => {
                self.totals.push((source, 0, None));
                self.totals.last_mut().expect("just pushed")
            }
        };

        match result {
            Ok(sources) => {
                entry.1 += sources.len();
                self.candidates.extend(sources);
            }
            Err(e) => {
                warn!("Discovery failed for {}: {}", source, e);
                if entry.2.is_none() {
                    entry.2 = Some(e.to_string());
                }
            }
        }
    }
}

impl Discovery {
    pub fn new(config: DiscoveryConfig) -> Self {
        let youtube = match &config.cookies_from_browser {
            Some(browser) => YoutubeDiscoverer::with_cookies(browser.clone()),
            None => YoutubeDiscoverer::new(),
        };
        let youtube_series = match &config.cookies_from_browser {
            Some(browser) => YoutubeSeriesDiscoverer::with_cookies(browser.clone()),
            None => YoutubeSeriesDiscoverer::new(),
        };

        Self {
            tmdb: TmdbDiscoverer::new(config.tmdb_api_key.clone()),
            tmdb_series: TmdbSeriesDiscoverer::new(config.tmdb_api_key),
            archive: ArchiveOrgDiscoverer::new(),
            youtube,
            youtube_series,
            dailymotion: DailymotionDiscoverer::new(),
            vimeo: VimeoDiscoverer::new(config.vimeo_access_token),
            bilibili: BilibiliDiscoverer::new(),
            kinocheck: KinoCheckDiscoverer::new(config.kinocheck_request_count),
            #[cfg(test)]
            stubs: Vec::new(),
        }
    }

    /// Run discovery for one item, given its item-level query and any season queries.
    ///
    /// All queries are fanned out first; deduplication and limits are applied once over
    /// the union, so a candidate returned by two queries is downloaded once.
    pub async fn discover(&self, queries: &[DiscoveryQuery]) -> DiscoveryReport {
        let mut collected = Collected::default();

        for query in queries {
            self.run_query(query, &mut collected).await;
        }

        let Some(query) = queries.first() else {
            return DiscoveryReport::default();
        };

        let source_results = collected
            .totals
            .into_iter()
            .map(|(source, videos_found, error)| SourceResult {
                source,
                videos_found,
                error,
            })
            .collect();

        Self::assemble(collected.candidates, source_results, query)
    }

    /// Fan out one query: resolve identity, query each supported source in order,
    /// and add the KinoCheck fallback when TMDB identified a movie but had no videos.
    async fn run_query(&self, query: &DiscoveryQuery, collected: &mut Collected) {
        let mut tmdb_movie_id: Option<u64> = None;
        let mut tmdb_found_zero = false;

        for &source in &query.sources {
            if !Self::supports(query.kind, source, query.season) {
                debug!(
                    "Skipping {} for {:?} discovery (season: {:?})",
                    source, query.kind, query.season
                );
                continue;
            }

            #[cfg(test)]
            if let Some(stubbed) = self.stub_for(source) {
                collected.record(Self::source_type(source), Ok(stubbed));
                continue;
            }

            // TMDB identity is needed by the KinoCheck fallback, so it runs here
            // rather than through the generic dispatch.
            if query.kind == LibraryKind::Movie && source == Source::Tmdb {
                match self
                    .tmdb
                    .discover_for_movie(&query.title, query.year.unwrap_or(0))
                    .await
                {
                    Ok((sources, movie_id)) => {
                        tmdb_found_zero = sources.is_empty();
                        tmdb_movie_id = movie_id;
                        collected.record(SourceType::TMDB, Ok(sources));
                    }
                    Err(e) => collected.record(SourceType::TMDB, Err(e)),
                }
                continue;
            }

            collected.record(
                Self::source_type(source),
                self.run_source(source, query).await,
            );
        }

        // Fallback policy: TMDB identified the movie but returned no videos.
        if query.kind == LibraryKind::Movie
            && tmdb_found_zero
            && let Some(movie_id) = tmdb_movie_id
        {
            info!(
                "TMDB returned no videos for {} — falling back to KinoCheck",
                query.title
            );
            collected.record(
                SourceType::KinoCheck,
                self.kinocheck.discover_for_tmdb_id(movie_id).await,
            );
        }
    }

    async fn run_source(
        &self,
        source: Source,
        query: &DiscoveryQuery,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        match (query.kind, source) {
            (LibraryKind::Movie, Source::Tmdb) => self.tmdb.discover(query).await,
            (LibraryKind::Movie, Source::Archive) => self.archive.discover(query).await,
            (LibraryKind::Movie, Source::Youtube) => self.youtube.discover(query).await,
            (LibraryKind::Movie, Source::Dailymotion) => self.dailymotion.discover(query).await,
            (LibraryKind::Movie, Source::Vimeo) => self.vimeo.discover(query).await,
            (LibraryKind::Movie, Source::Bilibili) => self.bilibili.discover(query).await,
            (LibraryKind::Series, Source::Tmdb) => self.tmdb_series.discover(query).await,
            (LibraryKind::Series, Source::Youtube) => self.youtube_series.discover(query).await,
            (LibraryKind::Series, Source::Dailymotion) => self.dailymotion.discover(query).await,
            (LibraryKind::Series, Source::Vimeo) => self.vimeo.discover(query).await,
            (LibraryKind::Series, Source::Archive | Source::Bilibili) => Ok(Vec::new()),
        }
    }

    /// Whether a source serves this kind and scope. Series season queries are
    /// YouTube-only; archive and Bilibili have no series search.
    fn supports(kind: LibraryKind, source: Source, season: Option<u8>) -> bool {
        match (kind, season) {
            (LibraryKind::Movie, None) => true,
            (LibraryKind::Movie, Some(_)) => false,
            (LibraryKind::Series, Some(_)) => source == Source::Youtube,
            (LibraryKind::Series, None) => !matches!(source, Source::Archive | Source::Bilibili),
        }
    }

    fn source_type(source: Source) -> SourceType {
        match source {
            Source::Tmdb => SourceType::TMDB,
            Source::Archive => SourceType::ArchiveOrg,
            Source::Dailymotion => SourceType::Dailymotion,
            Source::Youtube => SourceType::YouTube,
            Source::Vimeo => SourceType::Vimeo,
            Source::Bilibili => SourceType::Bilibili,
        }
    }

    /// Deduplicate, strip duplicate URLs, apply content limits, and build the report.
    fn assemble(
        candidates: Vec<VideoSource>,
        source_results: Vec<SourceResult>,
        query: &DiscoveryQuery,
    ) -> DiscoveryReport {
        let (mut candidates, title_duplicates) = deduplicate(candidates, &query.sources);
        if title_duplicates > 0 {
            info!(
                "Removed {} title+duration duplicates for {}",
                title_duplicates, query.title
            );
        }

        // URL dedup — safety net after title+duration dedup.
        candidates.sort_by(|a, b| a.url.cmp(&b.url));
        let before_url_dedup = candidates.len();
        candidates.dedup_by(|a, b| a.url == b.url);
        let duplicates_removed = title_duplicates + (before_url_dedup - candidates.len());

        let before_limits = candidates.len();
        candidates = Self::apply_limits(candidates, &query.limits, &query.sources);
        let limits_removed = before_limits - candidates.len();

        info!(
            "Discovery for {}: {} candidates ({} duplicates, {} over limits)",
            query.title,
            candidates.len(),
            duplicates_removed,
            limits_removed
        );

        DiscoveryReport {
            candidates,
            source_results,
            duplicates_removed,
            limits_removed,
        }
    }

    fn apply_limits(
        candidates: Vec<VideoSource>,
        limits: &ContentLimits,
        active_sources: &[Source],
    ) -> Vec<VideoSource> {
        let mut by_category: HashMap<ContentCategory, Vec<VideoSource>> = HashMap::new();
        let mut category_order: Vec<ContentCategory> = Vec::new();

        for candidate in candidates {
            if !by_category.contains_key(&candidate.category) {
                category_order.push(candidate.category);
            }
            by_category
                .entry(candidate.category)
                .or_default()
                .push(candidate);
        }

        let mut limited = Vec::new();
        for category in category_order {
            let mut group = by_category.remove(&category).unwrap_or_default();
            // Higher-tier sources win; within a tier, earlier enabled source wins.
            group.sort_by_key(|candidate| {
                (
                    candidate.source_type.tier(),
                    crate::deduplication::source_order(&candidate.source_type, active_sources),
                )
            });
            if let Some(limit) = limits.limit_for(category) {
                group.truncate(limit);
            }
            limited.extend(group);
        }

        limited
    }

    #[cfg(test)]
    fn with_stubs(stubs: Vec<(Source, Vec<VideoSource>)>) -> Self {
        let mut discovery = Self::new(DiscoveryConfig {
            tmdb_api_key: String::new(),
            vimeo_access_token: String::new(),
            cookies_from_browser: None,
            kinocheck_request_count: Arc::new(AtomicU32::new(0)),
        });
        discovery.stubs = stubs;
        discovery
    }

    #[cfg(test)]
    fn stub_for(&self, source: Source) -> Option<Vec<VideoSource>> {
        self.stubs
            .iter()
            .find(|(s, _)| *s == source)
            .map(|(_, sources)| sources.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(title: &str, source_type: SourceType, category: ContentCategory) -> VideoSource {
        VideoSource {
            url: format!("https://example.com/{}", title.replace(' ', "_")),
            source_type,
            category,
            title: title.to_string(),
            season_number: None,
            duration_secs: None,
        }
    }

    fn query(kind: LibraryKind, sources: Vec<Source>) -> DiscoveryQuery {
        DiscoveryQuery {
            kind,
            title: "Example".to_string(),
            year: Some(2000),
            seasons: vec![1, 2],
            season: None,
            sources,
            limits: ContentLimits::per_category(),
        }
    }

    #[tokio::test]
    async fn test_report_merges_queries_and_dedupes_once() {
        let discovery = Discovery::with_stubs(vec![
            (
                Source::Tmdb,
                vec![candidate(
                    "Example Trailer",
                    SourceType::TMDB,
                    ContentCategory::Trailer,
                )],
            ),
            (
                Source::Youtube,
                vec![
                    // Same video found by two different queries.
                    candidate(
                        "Example Trailer",
                        SourceType::YouTube,
                        ContentCategory::Trailer,
                    ),
                    candidate(
                        "Example Featurette",
                        SourceType::YouTube,
                        ContentCategory::Featurette,
                    ),
                ],
            ),
        ]);

        let base = query(LibraryKind::Series, vec![Source::Tmdb, Source::Youtube]);
        let report = discovery
            .discover(&[base.clone(), base.for_season(1)])
            .await;

        // Only categories present in the stubs, and the higher tier survives dedupe.
        let trailers: Vec<&VideoSource> = report
            .candidates
            .iter()
            .filter(|c| c.title == "Example Trailer")
            .collect();
        assert_eq!(trailers.len(), 1);
        assert_eq!(trailers[0].source_type, SourceType::TMDB);
        assert_eq!(report.candidates.len(), 2);
        assert!(report.duplicates_removed >= 1);

        // Counts are summed across queries; one line per source type.
        let youtube = report
            .source_results
            .iter()
            .find(|r| r.source == SourceType::YouTube)
            .expect("youtube line");
        assert_eq!(youtube.videos_found, 4);
    }

    #[tokio::test]
    async fn test_support_table_and_limits() {
        let many_trailers: Vec<VideoSource> = (0..6)
            .map(|i| {
                candidate(
                    &format!("Trailer number {}", i),
                    SourceType::YouTube,
                    ContentCategory::Trailer,
                )
            })
            .collect();
        let discovery = Discovery::with_stubs(vec![
            (Source::Youtube, many_trailers),
            (
                Source::Archive,
                vec![candidate(
                    "Archive Extra",
                    SourceType::ArchiveOrg,
                    ContentCategory::Extras,
                )],
            ),
        ]);

        let sources = vec![Source::Youtube, Source::Archive];

        let movie_report = discovery
            .discover(&[query(LibraryKind::Movie, sources.clone())])
            .await;
        assert!(
            movie_report
                .source_results
                .iter()
                .any(|r| r.source == SourceType::ArchiveOrg)
        );
        assert_eq!(movie_report.candidates.len(), 5);
        assert_eq!(movie_report.limits_removed, 2);

        let series_report = discovery
            .discover(&[query(LibraryKind::Series, sources)])
            .await;
        assert!(
            !series_report
                .source_results
                .iter()
                .any(|r| r.source == SourceType::ArchiveOrg)
        );

        // Season-scoped queries only reach YouTube.
        let season_report = discovery
            .discover(&[
                query(LibraryKind::Series, vec![Source::Youtube, Source::Archive]).for_season(1),
            ])
            .await;
        assert_eq!(season_report.source_results.len(), 1);
        assert_eq!(season_report.source_results[0].source, SourceType::YouTube);
    }
}
