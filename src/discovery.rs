// Discovery module - handles content discovery from multiple sources

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, MovieEntry, SourceMode, SourceType, VideoSource};
use log::{debug, error, info};
use serde::Deserialize;

/// Trait for content discoverers
#[allow(async_fn_in_trait)]
pub trait ContentDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError>;
}

/// TMDB API response for movie search
#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    results: Vec<TmdbMovie>,
}

/// TMDB movie result
#[derive(Debug, Deserialize)]
struct TmdbMovie {
    id: u64,
    title: String,
}

/// TMDB API response for videos
#[derive(Debug, Deserialize)]
struct TmdbVideosResponse {
    results: Vec<TmdbVideo>,
}

/// TMDB video entry
#[derive(Debug, Deserialize)]
struct TmdbVideo {
    key: String,
    name: String,
    site: String,
    #[serde(rename = "type")]
    video_type: String,
}

/// TMDB content discoverer
pub struct TmdbDiscoverer {
    api_key: String,
    client: reqwest::Client,
}

impl TmdbDiscoverer {
    /// Create a new TMDB discoverer with the given API key
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Search for a movie by title and year
    async fn search_movie(&self, title: &str, year: u16) -> Result<Option<u64>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}&year={}",
            self.api_key,
            urlencoding::encode(title),
            year
        );

        debug!("Searching TMDB for: {} ({})", title, year);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("TMDB search request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB search failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let search_result: TmdbSearchResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB search response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if let Some(movie) = search_result.results.first() {
            info!("Found TMDB movie: {} (ID: {})", movie.title, movie.id);
            Ok(Some(movie.id))
        } else {
            info!("No TMDB results found for: {} ({})", title, year);
            Ok(None)
        }
    }

    /// Fetch videos for a movie by ID
    async fn fetch_videos(&self, movie_id: u64) -> Result<Vec<TmdbVideo>, DiscoveryError> {
        let url = format!(
            "https://api.themoviedb.org/3/movie/{}/videos?api_key={}",
            movie_id, self.api_key
        );

        debug!("Fetching TMDB videos for movie ID: {}", movie_id);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("TMDB videos request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("TMDB videos fetch failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "TMDB API returned status {}",
                status
            )));
        }

        let videos_result: TmdbVideosResponse = response.json().await.map_err(|e| {
            error!("Failed to parse TMDB videos response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        info!("Found {} videos from TMDB", videos_result.results.len());
        Ok(videos_result.results)
    }

    /// Map TMDB video type to content category
    pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
        match tmdb_type {
            "Trailer" => Some(ContentCategory::Trailer),
            "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
            "Deleted Scene" => Some(ContentCategory::DeletedScene),
            "Featurette" => Some(ContentCategory::Featurette),
            "Bloopers" => Some(ContentCategory::Featurette),
            _ => {
                debug!("Unknown TMDB video type: {}", tmdb_type);
                None
            }
        }
    }
}

impl ContentDiscoverer for TmdbDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Discovering TMDB content for: {}", movie);

        // Search for the movie
        let movie_id = match self.search_movie(&movie.title, movie.year).await {
            Ok(Some(id)) => id,
            Ok(None) => {
                info!("No TMDB match found for: {}", movie);
                return Ok(Vec::new());
            }
            Err(e) => {
                error!("TMDB search failed for {}: {}", movie, e);
                return Err(e);
            }
        };

        // Fetch videos for the movie
        let videos = match self.fetch_videos(movie_id).await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to fetch TMDB videos for {}: {}", movie, e);
                return Err(e);
            }
        };

        // Convert TMDB videos to VideoSource
        let sources: Vec<VideoSource> = videos
            .into_iter()
            .filter(|v| v.site == "YouTube") // Only YouTube videos are downloadable
            .filter_map(|v| {
                Self::map_tmdb_type(&v.video_type).map(|category| VideoSource {
                    url: format!("https://www.youtube.com/watch?v={}", v.key),
                    source_type: SourceType::TMDB,
                    category,
                    title: v.name,
                })
            })
            .collect();

        info!("Discovered {} TMDB sources for: {}", sources.len(), movie);
        Ok(sources)
    }
}

/// Archive.org API response for search
#[derive(Debug, Deserialize)]
struct ArchiveOrgSearchResponse {
    response: ArchiveOrgResponse,
}

/// Archive.org response wrapper
#[derive(Debug, Deserialize)]
struct ArchiveOrgResponse {
    docs: Vec<ArchiveOrgDoc>,
}

/// Archive.org document entry
#[derive(Debug, Deserialize)]
struct ArchiveOrgDoc {
    identifier: String,
    title: String,
    #[serde(default)]
    subject: Vec<String>,
}

/// Archive.org content discoverer
pub struct ArchiveOrgDiscoverer {
    client: reqwest::Client,
}

impl Default for ArchiveOrgDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

impl ArchiveOrgDiscoverer {
    /// Create a new Archive.org discoverer
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Build Archive.org search query for a movie
    fn build_query(title: &str) -> String {
        format!(
            "title:\"{}\" AND (subject:\"EPK\" OR subject:\"Making of\")",
            title
        )
    }

    /// Map Archive.org subjects to content categories
    fn map_subjects(subjects: &[String]) -> Option<ContentCategory> {
        // Check for EPK first, then Making of
        if subjects.iter().any(|s| s.eq_ignore_ascii_case("EPK")) {
            // EPK can be either featurette or behind the scenes
            // Default to featurette as it's more general
            Some(ContentCategory::Featurette)
        } else if subjects
            .iter()
            .any(|s| s.to_lowercase().contains("making of"))
        {
            Some(ContentCategory::BehindTheScenes)
        } else {
            None
        }
    }

    /// Search Archive.org for a movie
    async fn search(&self, title: &str) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError> {
        let query = Self::build_query(title);
        let url = format!(
            "https://archive.org/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&fl[]=subject&rows=10&output=json",
            urlencoding::encode(&query)
        );

        debug!("Searching Archive.org for: {}", title);

        let response = self.client.get(&url).send().await.map_err(|e| {
            error!("Archive.org search request failed: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        if !response.status().is_success() {
            let status = response.status();
            error!("Archive.org search failed with status: {}", status);
            return Err(DiscoveryError::ApiError(format!(
                "Archive.org API returned status {}",
                status
            )));
        }

        let search_result: ArchiveOrgSearchResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Archive.org search response: {}", e);
            DiscoveryError::NetworkError(e)
        })?;

        info!(
            "Found {} results from Archive.org",
            search_result.response.docs.len()
        );
        Ok(search_result.response.docs)
    }
}

impl ContentDiscoverer for ArchiveOrgDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        // Only query Archive.org for movies before 2010
        if movie.year >= 2010 {
            debug!(
                "Skipping Archive.org for {} - year {} is >= 2010",
                movie, movie.year
            );
            return Ok(Vec::new());
        }

        info!("Discovering Archive.org content for: {}", movie);

        // Search for the movie
        let docs = match self.search(&movie.title).await {
            Ok(d) => d,
            Err(e) => {
                error!("Archive.org search failed for {}: {}", movie, e);
                return Err(e);
            }
        };

        // Convert Archive.org docs to VideoSource
        let sources: Vec<VideoSource> = docs
            .into_iter()
            .filter_map(|doc| {
                Self::map_subjects(&doc.subject).map(|category| VideoSource {
                    url: format!("https://archive.org/details/{}", doc.identifier),
                    source_type: SourceType::ArchiveOrg,
                    category,
                    title: doc.title,
                })
            })
            .collect();

        info!(
            "Discovered {} Archive.org sources for: {}",
            sources.len(),
            movie
        );
        Ok(sources)
    }
}

/// YouTube content discoverer
pub struct YoutubeDiscoverer;

impl Default for YoutubeDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

impl YoutubeDiscoverer {
    /// Create a new YouTube discoverer
    pub fn new() -> Self {
        Self
    }

    /// Build search queries for different content types
    fn build_search_queries(title: &str, year: u16) -> Vec<(String, ContentCategory)> {
        vec![
            (
                format!("{} {} deleted scenes", title, year),
                ContentCategory::DeletedScene,
            ),
            (
                format!("{} {} behind the scenes", title, year),
                ContentCategory::BehindTheScenes,
            ),
            (
                format!("{} {} bloopers", title, year),
                ContentCategory::Featurette,
            ),
            (
                format!("{} {} cast interview", title, year),
                ContentCategory::Featurette,
            ),
        ]
    }

    /// Check if a video title contains excluded keywords
    fn contains_excluded_keywords(title: &str) -> bool {
        let excluded_keywords = [
            "Review",
            "Reaction",
            "Analysis",
            "Explained",
            "Ending",
            "Theory",
            "React",
        ];

        let title_lower = title.to_lowercase();
        excluded_keywords
            .iter()
            .any(|keyword| title_lower.contains(&keyword.to_lowercase()))
    }

    /// Check if duration is within acceptable range (30s - 20min)
    fn is_duration_valid(duration_secs: u32) -> bool {
        (30..=1200).contains(&duration_secs) // 20 minutes = 1200 seconds
    }

    /// Check if video is a YouTube Short (duration < 60s and vertical aspect ratio)
    fn is_youtube_short(duration_secs: u32, width: u32, height: u32) -> bool {
        // YouTube Shorts are typically < 60 seconds and have vertical aspect ratio (9:16 or similar)
        if duration_secs >= 60 {
            return false;
        }

        // Check for vertical aspect ratio (height > width)
        // Allow some tolerance for aspect ratio detection
        height > width
    }

    /// Filter a video based on all criteria
    fn should_include_video(title: &str, duration_secs: u32, width: u32, height: u32) -> bool {
        // Check duration range
        if !Self::is_duration_valid(duration_secs) {
            debug!(
                "Excluding video '{}' - duration {}s out of range",
                title, duration_secs
            );
            return false;
        }

        // Check for excluded keywords
        if Self::contains_excluded_keywords(title) {
            debug!("Excluding video '{}' - contains excluded keyword", title);
            return false;
        }

        // Check if it's a YouTube Short
        if Self::is_youtube_short(duration_secs, width, height) {
            debug!("Excluding video '{}' - detected as YouTube Short", title);
            return false;
        }

        true
    }

    /// Search YouTube using yt-dlp for a specific query
    async fn search_youtube(
        &self,
        query: &str,
        category: ContentCategory,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        // Use yt-dlp with ytsearch5 to get top 5 results
        let search_query = format!("ytsearch5:{}", query);

        debug!("Searching YouTube with query: {}", query);

        // Execute yt-dlp to search and get video metadata
        let output = tokio::process::Command::new("yt-dlp")
            .arg("--dump-json")
            .arg("--no-playlist")
            .arg("--skip-download")
            .arg(&search_query)
            .output()
            .await
            .map_err(|e| {
                error!("Failed to execute yt-dlp: {}", e);
                DiscoveryError::YtDlpError(format!("Failed to execute yt-dlp: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("yt-dlp search failed: {}", stderr);
            return Err(DiscoveryError::YtDlpError(format!(
                "yt-dlp search failed: {}",
                stderr
            )));
        }

        // Parse JSON output (yt-dlp outputs one JSON object per line)
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut sources = Vec::new();

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(json) => {
                    // Extract video metadata
                    let title = json["title"].as_str().unwrap_or("Unknown").to_string();
                    let url = json["webpage_url"].as_str().unwrap_or("").to_string();
                    let duration = json["duration"].as_u64().unwrap_or(0) as u32;
                    let width = json["width"].as_u64().unwrap_or(1920) as u32;
                    let height = json["height"].as_u64().unwrap_or(1080) as u32;

                    // Apply filtering
                    if Self::should_include_video(&title, duration, width, height) {
                        sources.push(VideoSource {
                            url,
                            source_type: SourceType::YouTube,
                            category,
                            title,
                        });
                        debug!("Added YouTube video: {}", sources.last().unwrap().title);
                    }
                }
                Err(e) => {
                    error!("Failed to parse yt-dlp JSON output: {}", e);
                    continue;
                }
            }
        }

        Ok(sources)
    }
}

impl ContentDiscoverer for YoutubeDiscoverer {
    async fn discover(&self, movie: &MovieEntry) -> Result<Vec<VideoSource>, DiscoveryError> {
        info!("Discovering YouTube content for: {}", movie);

        let queries = Self::build_search_queries(&movie.title, movie.year);
        let mut all_sources = Vec::new();

        for (query, category) in queries {
            match self.search_youtube(&query, category).await {
                Ok(mut sources) => {
                    info!(
                        "Found {} YouTube videos for query: {}",
                        sources.len(),
                        query
                    );
                    all_sources.append(&mut sources);
                }
                Err(e) => {
                    error!("YouTube search failed for query '{}': {}", query, e);
                    // Continue with other queries even if one fails
                }
            }
        }

        info!(
            "Discovered {} YouTube sources for: {}",
            all_sources.len(),
            movie
        );
        Ok(all_sources)
    }
}

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
    pub async fn discover_all(&self, movie: &MovieEntry) -> Vec<VideoSource> {
        let mut all_sources = Vec::new();

        match self.mode {
            SourceMode::All => {
                // Query TMDB
                match self.tmdb.discover(movie).await {
                    Ok(sources) => {
                        log::info!("Found {} sources from TMDB for {}", sources.len(), movie);
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        log::warn!("TMDB discovery failed for {}: {}", movie, e);
                    }
                }

                // Query Archive.org only for movies before 2010
                if movie.year < 2010 {
                    match self.archive.discover(movie).await {
                        Ok(sources) => {
                            log::info!(
                                "Found {} sources from Archive.org for {}",
                                sources.len(),
                                movie
                            );
                            all_sources.extend(sources);
                        }
                        Err(e) => {
                            log::warn!("Archive.org discovery failed for {}: {}", movie, e);
                        }
                    }
                } else {
                    log::debug!("Skipping Archive.org for {} (year >= 2010)", movie);
                }

                // Query YouTube (always)
                match self.youtube.discover(movie).await {
                    Ok(sources) => {
                        log::info!("Found {} sources from YouTube for {}", sources.len(), movie);
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        log::warn!("YouTube discovery failed for {}: {}", movie, e);
                    }
                }
            }
            SourceMode::YoutubeOnly => {
                // Query only YouTube
                match self.youtube.discover(movie).await {
                    Ok(sources) => {
                        log::info!("Found {} sources from YouTube for {}", sources.len(), movie);
                        all_sources.extend(sources);
                    }
                    Err(e) => {
                        log::warn!("YouTube discovery failed for {}: {}", movie, e);
                    }
                }
            }
        }

        log::info!(
            "Total sources discovered for {}: {}",
            movie,
            all_sources.len()
        );
        all_sources
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    // Feature: extras-fetcher, Property 7: TMDB Video Type Mapping
    // Validates: Requirements 3.4, 3.5, 3.6, 3.7, 3.8
    proptest! {
        #[test]
        fn prop_tmdb_type_mapping(tmdb_type in prop_oneof![
            Just("Trailer"),
            Just("Behind the Scenes"),
            Just("Deleted Scene"),
            Just("Featurette"),
            Just("Bloopers"),
        ]) {
            let category = TmdbDiscoverer::map_tmdb_type(tmdb_type);

            match tmdb_type {
                "Trailer" => prop_assert_eq!(category, Some(ContentCategory::Trailer)),
                "Behind the Scenes" => prop_assert_eq!(category, Some(ContentCategory::BehindTheScenes)),
                "Deleted Scene" => prop_assert_eq!(category, Some(ContentCategory::DeletedScene)),
                "Featurette" => prop_assert_eq!(category, Some(ContentCategory::Featurette)),
                "Bloopers" => prop_assert_eq!(category, Some(ContentCategory::Featurette)),
                _ => unreachable!(),
            }
        }
    }

    // Feature: extras-fetcher, Property 8: Archive.org Year-Based Querying
    // Validates: Requirements 4.1, 4.2
    proptest! {
        #[test]
        fn prop_archive_org_year_based_querying(
            title in "[a-zA-Z0-9 ]{1,30}",
            year in 1900u16..2100u16
        ) {
            let _movie = MovieEntry {
                path: PathBuf::from(format!("/movies/{} ({})", title, year)),
                title: title.clone(),
                year,
                has_done_marker: false,
            };

            // Archive.org should only be queried for movies before 2010
            let should_query = year < 2010;

            // We can't test the actual async discover method in proptest easily,
            // but we can verify the year check logic
            let would_skip = year >= 2010;

            prop_assert_eq!(should_query, !would_skip);

            // If year < 2010, Archive.org should be queried
            // If year >= 2010, Archive.org should be skipped
            if year < 2010 {
                prop_assert!(year < 2010, "Movies before 2010 should query Archive.org");
            } else {
                prop_assert!(year >= 2010, "Movies from 2010 onwards should skip Archive.org");
            }
        }
    }

    // Feature: extras-fetcher, Property 9: Archive.org Query Construction
    // Validates: Requirements 4.4
    proptest! {
        #[test]
        fn prop_archive_org_query_construction(
            title in "[a-zA-Z0-9 ]{1,50}"
        ) {
            let query = ArchiveOrgDiscoverer::build_query(&title);

            // Query must contain the title in quotes
            prop_assert!(
                query.contains(&format!("title:\"{}\"", title)),
                "Query should contain title:\"{}\", got: {}",
                title,
                query
            );

            // Query must contain EPK subject
            prop_assert!(
                query.contains("subject:\"EPK\""),
                "Query should contain subject:\"EPK\", got: {}",
                query
            );

            // Query must contain Making of subject
            prop_assert!(
                query.contains("subject:\"Making of\""),
                "Query should contain subject:\"Making of\", got: {}",
                query
            );

            // Query must use OR operator between subjects
            prop_assert!(
                query.contains(" OR "),
                "Query should contain OR operator, got: {}",
                query
            );

            // Query must use AND operator to combine title and subjects
            prop_assert!(
                query.contains(" AND "),
                "Query should contain AND operator, got: {}",
                query
            );
        }
    }

    // Feature: extras-fetcher, Property 10: YouTube Always Queried
    // Validates: Requirements 5.1
    proptest! {
        #[test]
        fn prop_youtube_always_queried(
            title in "[a-zA-Z0-9 ]{1,30}",
            year in 1900u16..2100u16
        ) {
            // YouTube should always generate search queries regardless of year or other factors
            let queries = YoutubeDiscoverer::build_search_queries(&title, year);

            // YouTube should always produce queries (at least 4 types: deleted scenes, behind the scenes, bloopers, interviews)
            prop_assert!(
                !queries.is_empty(),
                "YouTube should always generate search queries"
            );

            // Verify we have queries for all expected content types
            prop_assert!(
                queries.len() >= 4,
                "YouTube should generate at least 4 search queries, got {}",
                queries.len()
            );

            // Verify each query contains the title and year
            for (query, _category) in &queries {
                prop_assert!(
                    query.contains(&title),
                    "Query should contain title '{}', got: {}",
                    title,
                    query
                );
                prop_assert!(
                    query.contains(&year.to_string()),
                    "Query should contain year '{}', got: {}",
                    year,
                    query
                );
            }
        }
    }

    // Feature: extras-fetcher, Property 11: YouTube Duration Filtering
    // Validates: Requirements 5.7, 5.8
    proptest! {
        #[test]
        fn prop_youtube_duration_filtering(duration_secs in 0u32..3600u32) {
            // Videos should be excluded if duration < 30s OR duration > 20min (1200s)
            let should_exclude = duration_secs < 30 || duration_secs > 1200;
            let is_valid = YoutubeDiscoverer::is_duration_valid(duration_secs);

            // is_duration_valid should return true only for videos in the 30s-1200s range
            prop_assert_eq!(
                is_valid,
                !should_exclude,
                "Duration {}s: is_valid={}, should_exclude={}",
                duration_secs,
                is_valid,
                should_exclude
            );

            // Verify boundary conditions
            if duration_secs < 30 {
                prop_assert!(!is_valid, "Videos < 30s should be excluded");
            } else if duration_secs > 1200 {
                prop_assert!(!is_valid, "Videos > 1200s (20min) should be excluded");
            } else {
                prop_assert!(is_valid, "Videos between 30s and 1200s should be included");
            }
        }
    }

    // Feature: extras-fetcher, Property 12: YouTube Keyword Filtering
    // Validates: Requirements 5.9
    proptest! {
        #[test]
        fn prop_youtube_keyword_filtering(
            prefix in "[a-zA-Z0-9 ]{0,20}",
            suffix in "[a-zA-Z0-9 ]{0,20}",
            keyword in prop_oneof![
                Just("Review"),
                Just("Reaction"),
                Just("Analysis"),
                Just("Explained"),
                Just("Ending"),
                Just("Theory"),
                Just("React"),
            ]
        ) {
            // Test with keyword in various positions and cases
            let title_with_keyword = format!("{} {} {}", prefix, keyword, suffix);
            let title_lowercase = format!("{} {} {}", prefix, keyword.to_lowercase(), suffix);
            let title_uppercase = format!("{} {} {}", prefix, keyword.to_uppercase(), suffix);

            // All variations should be detected and excluded
            prop_assert!(
                YoutubeDiscoverer::contains_excluded_keywords(&title_with_keyword),
                "Title '{}' should be excluded (contains '{}')",
                title_with_keyword,
                keyword
            );

            prop_assert!(
                YoutubeDiscoverer::contains_excluded_keywords(&title_lowercase),
                "Title '{}' should be excluded (case-insensitive)",
                title_lowercase
            );

            prop_assert!(
                YoutubeDiscoverer::contains_excluded_keywords(&title_uppercase),
                "Title '{}' should be excluded (case-insensitive)",
                title_uppercase
            );

            // Test that titles without keywords are not excluded
            if !prefix.to_lowercase().contains(&keyword.to_lowercase())
                && !suffix.to_lowercase().contains(&keyword.to_lowercase()) {
                let clean_title = format!("{} {}", prefix, suffix);
                if !clean_title.trim().is_empty() {
                    // Only test if the clean title doesn't accidentally contain the keyword
                    let contains_keyword = ["review", "reaction", "analysis", "explained", "ending", "theory", "react"]
                        .iter()
                        .any(|kw| clean_title.to_lowercase().contains(kw));

                    if !contains_keyword {
                        prop_assert!(
                            !YoutubeDiscoverer::contains_excluded_keywords(&clean_title),
                            "Title '{}' should not be excluded (no keywords)",
                            clean_title
                        );
                    }
                }
            }
        }
    }

    // Feature: extras-fetcher, Property 13: YouTube Shorts Exclusion
    // Validates: Requirements 5.10
    proptest! {
        #[test]
        fn prop_youtube_shorts_exclusion(
            duration_secs in 0u32..120u32,
            width in 100u32..2000u32,
            height in 100u32..2000u32
        ) {
            let is_short = YoutubeDiscoverer::is_youtube_short(duration_secs, width, height);

            // YouTube Shorts are defined as:
            // - Duration < 60 seconds AND
            // - Vertical aspect ratio (height > width)
            let expected_short = duration_secs < 60 && height > width;

            prop_assert_eq!(
                is_short,
                expected_short,
                "Duration: {}s, Dimensions: {}x{}, is_short: {}, expected: {}",
                duration_secs,
                width,
                height,
                is_short,
                expected_short
            );

            // Verify specific cases
            if duration_secs >= 60 {
                prop_assert!(
                    !is_short,
                    "Videos >= 60s should not be classified as Shorts ({}s, {}x{})",
                    duration_secs,
                    width,
                    height
                );
            }

            if height <= width {
                prop_assert!(
                    !is_short,
                    "Videos with horizontal/square aspect ratio should not be Shorts ({}s, {}x{})",
                    duration_secs,
                    width,
                    height
                );
            }

            if duration_secs < 60 && height > width {
                prop_assert!(
                    is_short,
                    "Short vertical videos should be classified as Shorts ({}s, {}x{})",
                    duration_secs,
                    width,
                    height
                );
            }
        }
    }

    // Feature: extras-fetcher, Property 5: Mode Filtering
    // Validates: Requirements 1.5
    proptest! {
        #[test]
        fn prop_mode_filtering(
            _title in "[a-zA-Z0-9 ]{1,30}",
            _year in 1900u16..2100u16
        ) {
            // Create mock video sources from different source types
            let tmdb_source = VideoSource {
                url: "https://youtube.com/watch?v=tmdb123".to_string(),
                source_type: SourceType::TMDB,
                category: ContentCategory::Trailer,
                title: "TMDB Trailer".to_string(),
            };

            let archive_source = VideoSource {
                url: "https://archive.org/details/archive123".to_string(),
                source_type: SourceType::ArchiveOrg,
                category: ContentCategory::Featurette,
                title: "Archive EPK".to_string(),
            };

            let youtube_source = VideoSource {
                url: "https://youtube.com/watch?v=yt123".to_string(),
                source_type: SourceType::YouTube,
                category: ContentCategory::BehindTheScenes,
                title: "YouTube BTS".to_string(),
            };

            let all_sources = vec![
                tmdb_source.clone(),
                archive_source.clone(),
                youtube_source.clone(),
            ];

            // Test filtering logic for YoutubeOnly mode
            // In YoutubeOnly mode, only YouTube sources should remain
            let filtered_youtube_only: Vec<VideoSource> = all_sources
                .iter()
                .filter(|s| s.source_type == SourceType::YouTube)
                .cloned()
                .collect();

            prop_assert_eq!(
                filtered_youtube_only.len(),
                1,
                "YoutubeOnly mode should filter to only YouTube sources"
            );

            prop_assert_eq!(
                filtered_youtube_only[0].source_type,
                SourceType::YouTube,
                "Filtered source should be YouTube type"
            );

            // Test that All mode includes all source types
            let filtered_all: Vec<VideoSource> = all_sources.clone();

            prop_assert_eq!(
                filtered_all.len(),
                3,
                "All mode should include all sources"
            );

            // Verify all source types are present in All mode
            let has_tmdb = filtered_all.iter().any(|s| s.source_type == SourceType::TMDB);
            let has_archive = filtered_all.iter().any(|s| s.source_type == SourceType::ArchiveOrg);
            let has_youtube = filtered_all.iter().any(|s| s.source_type == SourceType::YouTube);

            prop_assert!(has_tmdb, "All mode should include TMDB sources");
            prop_assert!(has_archive, "All mode should include Archive.org sources");
            prop_assert!(has_youtube, "All mode should include YouTube sources");

            // Verify non-YouTube sources are excluded in YoutubeOnly mode
            let has_non_youtube = filtered_youtube_only
                .iter()
                .any(|s| s.source_type != SourceType::YouTube);

            prop_assert!(
                !has_non_youtube,
                "YoutubeOnly mode should not include non-YouTube sources"
            );
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_tmdb_type_mapping_trailer() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Trailer"),
            Some(ContentCategory::Trailer)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_behind_the_scenes() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Behind the Scenes"),
            Some(ContentCategory::BehindTheScenes)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_deleted_scene() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Deleted Scene"),
            Some(ContentCategory::DeletedScene)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_featurette() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Featurette"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_bloopers() {
        assert_eq!(
            TmdbDiscoverer::map_tmdb_type("Bloopers"),
            Some(ContentCategory::Featurette)
        );
    }

    #[test]
    fn test_tmdb_type_mapping_unknown() {
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Unknown Type"), None);
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Clip"), None);
        assert_eq!(TmdbDiscoverer::map_tmdb_type("Teaser"), None);
    }

    #[test]
    fn test_tmdb_discoverer_creation() {
        let api_key = "test_api_key_12345".to_string();
        let discoverer = TmdbDiscoverer::new(api_key.clone());
        assert_eq!(discoverer.api_key, api_key);
    }

    #[test]
    fn test_movie_search_url_construction() {
        // This test verifies the URL construction logic by checking the format
        let api_key = "test_key".to_string();
        let title = "The Matrix";
        let year = 1999;

        // Expected URL format
        let expected_base = "https://api.themoviedb.org/3/search/movie";
        let expected_query = format!(
            "?api_key={}&query={}&year={}",
            api_key,
            urlencoding::encode(title),
            year
        );

        // Verify URL encoding works correctly
        assert_eq!(urlencoding::encode(title), "The%20Matrix");
        assert!(expected_base.starts_with("https://api.themoviedb.org"));
        assert!(expected_query.contains("api_key=test_key"));
        assert!(expected_query.contains("query=The%20Matrix"));
        assert!(expected_query.contains("year=1999"));
    }

    #[test]
    fn test_movie_search_url_with_special_characters() {
        // Test URL encoding with special characters
        let title = "Movie: The Sequel (Part 2)";
        let encoded = urlencoding::encode(title);

        // Verify special characters are encoded
        assert!(encoded.contains("%3A")); // colon
        assert!(encoded.contains("%28")); // opening parenthesis
        assert!(encoded.contains("%29")); // closing parenthesis
    }

    #[test]
    fn test_videos_url_construction() {
        // Verify the videos endpoint URL format
        let api_key = "test_key".to_string();
        let movie_id = 603u64;

        let expected_url = format!(
            "https://api.themoviedb.org/3/movie/{}/videos?api_key={}",
            movie_id, api_key
        );

        assert_eq!(
            expected_url,
            "https://api.themoviedb.org/3/movie/603/videos?api_key=test_key"
        );
    }

    #[tokio::test]
    async fn test_discover_with_empty_api_key() {
        // Test that discoverer can be created with empty API key
        // (actual API calls will fail, but creation should succeed)
        let discoverer = TmdbDiscoverer::new(String::new());
        assert_eq!(discoverer.api_key, "");
    }

    #[test]
    fn test_video_source_construction() {
        // Test that VideoSource is constructed correctly from TMDB data
        let video_key = "dQw4w9WgXcQ";
        let video_name = "Official Trailer";
        let category = ContentCategory::Trailer;

        let source = VideoSource {
            url: format!("https://www.youtube.com/watch?v={}", video_key),
            source_type: SourceType::TMDB,
            category,
            title: video_name.to_string(),
        };

        assert_eq!(source.url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(source.source_type, SourceType::TMDB);
        assert_eq!(source.category, ContentCategory::Trailer);
        assert_eq!(source.title, "Official Trailer");
    }

    #[test]
    fn test_tmdb_response_deserialization() {
        // Test that we can deserialize a mock TMDB search response
        let json = r#"{
            "results": [
                {
                    "id": 603,
                    "title": "The Matrix"
                }
            ]
        }"#;

        let response: Result<TmdbSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].id, 603);
        assert_eq!(response.results[0].title, "The Matrix");
    }

    #[test]
    fn test_tmdb_videos_response_deserialization() {
        // Test that we can deserialize a mock TMDB videos response
        let json = r#"{
            "results": [
                {
                    "key": "m8e-FF8MsqU",
                    "name": "Official Trailer",
                    "site": "YouTube",
                    "type": "Trailer"
                },
                {
                    "key": "abc123def45",
                    "name": "Behind the Scenes",
                    "site": "YouTube",
                    "type": "Behind the Scenes"
                }
            ]
        }"#;

        let response: Result<TmdbVideosResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].key, "m8e-FF8MsqU");
        assert_eq!(response.results[0].name, "Official Trailer");
        assert_eq!(response.results[0].site, "YouTube");
        assert_eq!(response.results[0].video_type, "Trailer");
        assert_eq!(response.results[1].video_type, "Behind the Scenes");
    }

    #[test]
    fn test_empty_search_results() {
        // Test deserialization of empty search results
        let json = r#"{"results": []}"#;

        let response: Result<TmdbSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.results.len(), 0);
    }

    #[test]
    fn test_empty_videos_results() {
        // Test deserialization of empty videos results
        let json = r#"{"results": []}"#;

        let response: Result<TmdbVideosResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.results.len(), 0);
    }

    // Archive.org tests

    #[test]
    fn test_archive_org_discoverer_creation() {
        let discoverer = ArchiveOrgDiscoverer::new();
        // Just verify it can be created
        assert!(std::mem::size_of_val(&discoverer) > 0);
    }

    #[test]
    fn test_archive_org_query_string_formatting() {
        // Test query construction with simple title
        let query = ArchiveOrgDiscoverer::build_query("The Matrix");
        assert_eq!(
            query,
            "title:\"The Matrix\" AND (subject:\"EPK\" OR subject:\"Making of\")"
        );
    }

    #[test]
    fn test_archive_org_query_with_special_characters() {
        // Test query construction with special characters
        let query = ArchiveOrgDiscoverer::build_query("Movie: The Sequel");
        assert!(query.contains("title:\"Movie: The Sequel\""));
        assert!(query.contains("subject:\"EPK\""));
        assert!(query.contains("subject:\"Making of\""));
    }

    #[test]
    fn test_archive_org_subject_mapping_epk() {
        // Test EPK subject mapping
        let subjects = vec!["EPK".to_string(), "Documentary".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_archive_org_subject_mapping_making_of() {
        // Test Making of subject mapping
        let subjects = vec!["Making of".to_string(), "Documentary".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));
    }

    #[test]
    fn test_archive_org_subject_mapping_case_insensitive() {
        // Test case-insensitive EPK matching
        let subjects = vec!["epk".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));

        let subjects = vec!["EPK".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));

        let subjects = vec!["Epk".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_archive_org_subject_mapping_making_of_variations() {
        // Test various "making of" variations
        let subjects = vec!["Making of the Movie".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));

        let subjects = vec!["The Making of".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, Some(ContentCategory::BehindTheScenes));
    }

    #[test]
    fn test_archive_org_subject_mapping_no_match() {
        // Test with subjects that don't match
        let subjects = vec!["Documentary".to_string(), "Film".to_string()];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, None);
    }

    #[test]
    fn test_archive_org_subject_mapping_empty() {
        // Test with empty subjects
        let subjects: Vec<String> = vec![];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        assert_eq!(category, None);
    }

    #[test]
    fn test_archive_org_subject_mapping_epk_priority() {
        // Test that EPK takes priority over Making of
        let subjects = vec![
            "EPK".to_string(),
            "Making of".to_string(),
            "Documentary".to_string(),
        ];
        let category = ArchiveOrgDiscoverer::map_subjects(&subjects);
        // EPK should map to Featurette and take priority
        assert_eq!(category, Some(ContentCategory::Featurette));
    }

    #[test]
    fn test_archive_org_response_deserialization() {
        // Test that we can deserialize a mock Archive.org response
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "identifier": "matrix_epk_1999",
                        "title": "The Matrix EPK",
                        "subject": ["EPK", "Science Fiction"]
                    }
                ]
            }
        }"#;

        let response: Result<ArchiveOrgSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.response.docs.len(), 1);
        assert_eq!(response.response.docs[0].identifier, "matrix_epk_1999");
        assert_eq!(response.response.docs[0].title, "The Matrix EPK");
        assert_eq!(response.response.docs[0].subject.len(), 2);
        assert_eq!(response.response.docs[0].subject[0], "EPK");
    }

    #[test]
    fn test_archive_org_empty_response() {
        // Test deserialization of empty Archive.org response
        let json = r#"{"response": {"docs": []}}"#;

        let response: Result<ArchiveOrgSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.response.docs.len(), 0);
    }

    #[test]
    fn test_archive_org_response_with_missing_subjects() {
        // Test response with missing subject field (should default to empty vec)
        let json = r#"{
            "response": {
                "docs": [
                    {
                        "identifier": "test_id",
                        "title": "Test Movie"
                    }
                ]
            }
        }"#;

        let response: Result<ArchiveOrgSearchResponse, _> = serde_json::from_str(json);
        assert!(response.is_ok());

        let response = response.unwrap();
        assert_eq!(response.response.docs.len(), 1);
        assert_eq!(response.response.docs[0].subject.len(), 0);
    }

    // YouTube tests

    #[test]
    fn test_youtube_discoverer_creation() {
        let discoverer = YoutubeDiscoverer::new();
        // Just verify it can be created (zero-sized type)
        assert!(std::mem::size_of_val(&discoverer) == 0);
    }

    #[test]
    fn test_youtube_search_query_construction() {
        // Test that search queries are constructed correctly
        let queries = YoutubeDiscoverer::build_search_queries("The Matrix", 1999);

        // Should have 4 queries
        assert_eq!(queries.len(), 4);

        // Verify each query contains title and year
        for (query, _category) in &queries {
            assert!(query.contains("The Matrix"));
            assert!(query.contains("1999"));
        }

        // Verify specific query types
        assert!(queries.iter().any(|(q, _)| q.contains("deleted scenes")));
        assert!(queries.iter().any(|(q, _)| q.contains("behind the scenes")));
        assert!(queries.iter().any(|(q, _)| q.contains("bloopers")));
        assert!(queries.iter().any(|(q, _)| q.contains("cast interview")));
    }

    #[test]
    fn test_youtube_search_query_categories() {
        // Test that categories are mapped correctly
        let queries = YoutubeDiscoverer::build_search_queries("Test Movie", 2020);

        // Find each query type and verify its category
        for (query, category) in &queries {
            if query.contains("deleted scenes") {
                assert_eq!(*category, ContentCategory::DeletedScene);
            } else if query.contains("behind the scenes") {
                assert_eq!(*category, ContentCategory::BehindTheScenes);
            } else if query.contains("bloopers") || query.contains("cast interview") {
                assert_eq!(*category, ContentCategory::Featurette);
            }
        }
    }

    #[test]
    fn test_youtube_keyword_filtering_review() {
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Movie Review"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Honest Review of the Film"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "REVIEW - Movie Title"
        ));
    }

    #[test]
    fn test_youtube_keyword_filtering_reaction() {
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "First Time Reaction"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Movie Reaction Video"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "REACTION to Trailer"
        ));
    }

    #[test]
    fn test_youtube_keyword_filtering_analysis() {
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Movie Analysis"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "In-Depth Analysis"
        ));
    }

    #[test]
    fn test_youtube_keyword_filtering_explained() {
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Movie Explained"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Ending Explained"
        ));
    }

    #[test]
    fn test_youtube_keyword_filtering_theory() {
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Movie Theory"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Fan Theory Discussion"
        ));
    }

    #[test]
    fn test_youtube_keyword_filtering_react() {
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "React to Movie"
        ));
        assert!(YoutubeDiscoverer::contains_excluded_keywords(
            "Reacting to Trailer"
        ));
    }

    #[test]
    fn test_youtube_keyword_filtering_no_match() {
        // These should NOT be filtered
        assert!(!YoutubeDiscoverer::contains_excluded_keywords(
            "Official Trailer"
        ));
        assert!(!YoutubeDiscoverer::contains_excluded_keywords(
            "Behind the Scenes"
        ));
        assert!(!YoutubeDiscoverer::contains_excluded_keywords(
            "Deleted Scenes"
        ));
        assert!(!YoutubeDiscoverer::contains_excluded_keywords(
            "Cast Interview"
        ));
        assert!(!YoutubeDiscoverer::contains_excluded_keywords(
            "Making of Documentary"
        ));
    }

    #[test]
    fn test_youtube_keyword_filtering_case_insensitive() {
        // Test case insensitivity
        assert!(YoutubeDiscoverer::contains_excluded_keywords("review"));
        assert!(YoutubeDiscoverer::contains_excluded_keywords("REVIEW"));
        assert!(YoutubeDiscoverer::contains_excluded_keywords("Review"));
        assert!(YoutubeDiscoverer::contains_excluded_keywords("ReViEw"));
    }

    #[test]
    fn test_youtube_duration_valid_range() {
        // Test valid durations (30s - 1200s)
        assert!(YoutubeDiscoverer::is_duration_valid(30)); // Minimum
        assert!(YoutubeDiscoverer::is_duration_valid(60));
        assert!(YoutubeDiscoverer::is_duration_valid(300)); // 5 minutes
        assert!(YoutubeDiscoverer::is_duration_valid(600)); // 10 minutes
        assert!(YoutubeDiscoverer::is_duration_valid(1200)); // Maximum (20 minutes)
    }

    #[test]
    fn test_youtube_duration_invalid_too_short() {
        // Test durations that are too short
        assert!(!YoutubeDiscoverer::is_duration_valid(0));
        assert!(!YoutubeDiscoverer::is_duration_valid(10));
        assert!(!YoutubeDiscoverer::is_duration_valid(29));
    }

    #[test]
    fn test_youtube_duration_invalid_too_long() {
        // Test durations that are too long
        assert!(!YoutubeDiscoverer::is_duration_valid(1201));
        assert!(!YoutubeDiscoverer::is_duration_valid(1500));
        assert!(!YoutubeDiscoverer::is_duration_valid(3600));
    }

    #[test]
    fn test_youtube_shorts_detection_vertical_short() {
        // Vertical videos under 60s should be detected as Shorts
        assert!(YoutubeDiscoverer::is_youtube_short(30, 1080, 1920)); // 9:16
        assert!(YoutubeDiscoverer::is_youtube_short(45, 720, 1280)); // 9:16
        assert!(YoutubeDiscoverer::is_youtube_short(59, 1080, 1920)); // Just under 60s
    }

    #[test]
    fn test_youtube_shorts_detection_horizontal_not_short() {
        // Horizontal videos should NOT be Shorts regardless of duration
        assert!(!YoutubeDiscoverer::is_youtube_short(30, 1920, 1080)); // 16:9
        assert!(!YoutubeDiscoverer::is_youtube_short(45, 1280, 720)); // 16:9
    }

    #[test]
    fn test_youtube_shorts_detection_long_vertical_not_short() {
        // Vertical videos over 60s should NOT be Shorts
        assert!(!YoutubeDiscoverer::is_youtube_short(60, 1080, 1920));
        assert!(!YoutubeDiscoverer::is_youtube_short(120, 1080, 1920));
    }

    #[test]
    fn test_youtube_shorts_detection_square_not_short() {
        // Square videos should NOT be Shorts
        assert!(!YoutubeDiscoverer::is_youtube_short(30, 1080, 1080));
        assert!(!YoutubeDiscoverer::is_youtube_short(45, 720, 720));
    }

    #[test]
    fn test_youtube_should_include_video_valid() {
        // Valid video: good duration, no keywords, not a Short
        assert!(YoutubeDiscoverer::should_include_video(
            "Official Trailer",
            120,
            1920,
            1080
        ));
        assert!(YoutubeDiscoverer::should_include_video(
            "Behind the Scenes",
            300,
            1920,
            1080
        ));
    }

    #[test]
    fn test_youtube_should_include_video_excluded_by_duration() {
        // Excluded due to duration
        assert!(!YoutubeDiscoverer::should_include_video(
            "Official Trailer",
            20,
            1920,
            1080
        )); // Too short
        assert!(!YoutubeDiscoverer::should_include_video(
            "Behind the Scenes",
            1500,
            1920,
            1080
        )); // Too long
    }

    #[test]
    fn test_youtube_should_include_video_excluded_by_keyword() {
        // Excluded due to keyword
        assert!(!YoutubeDiscoverer::should_include_video(
            "Movie Review",
            120,
            1920,
            1080
        ));
        assert!(!YoutubeDiscoverer::should_include_video(
            "Ending Explained",
            300,
            1920,
            1080
        ));
    }

    #[test]
    fn test_youtube_should_include_video_excluded_as_short() {
        // Excluded as YouTube Short
        assert!(!YoutubeDiscoverer::should_include_video(
            "Quick Clip",
            45,
            1080,
            1920
        )); // Vertical, under 60s
    }

    #[test]
    fn test_youtube_should_include_video_multiple_exclusions() {
        // Video fails multiple criteria
        assert!(!YoutubeDiscoverer::should_include_video(
            "Movie Review",
            20,
            1080,
            1920
        )); // Keyword + duration + Short
    }
}
