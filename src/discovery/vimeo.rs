// Vimeo discovery module — searches the Vimeo REST API for movie/series extras.
// Requires a Personal Access Token (PAT) for authentication.
// See: https://developer.vimeo.com/api/reference/videos#search_videos

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, SourceType, VideoSource};
use log::{debug, info, warn};
use serde::Deserialize;

use super::title_matching;

/// Maximum number of pages to fetch from Vimeo search results.
const MAX_PAGES: u32 = 3;

/// Number of results per page.
const PER_PAGE: u32 = 10;

// --- Serde structs (private) ---

#[derive(Debug, Deserialize)]
struct VimeoResponse {
    data: Vec<VimeoVideo>,
    paging: VimeoPaging,
}

#[derive(Debug, Deserialize)]
struct VimeoPaging {
    #[serde(default)]
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VimeoVideo {
    #[allow(dead_code)]
    uri: String,
    name: String,
    duration: u32,
    link: String,
}

// --- Public API ---

/// Discovers extras from Vimeo using a Personal Access Token.
///
/// Searches by title+year, filters by duration and keywords, and paginates
/// up to 3 pages. Handles HTTP 429 with a single retry after 1-second backoff.
pub(crate) struct VimeoDiscoverer {
    /// SECURITY: Personal Access Token — never log this value.
    access_token: String,
    client: reqwest::Client,
}

impl VimeoDiscoverer {
    /// Creates a new discoverer with a 30-second network timeout.
    pub fn new(access_token: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client for Vimeo");
        Self {
            access_token,
            client,
        }
    }

    /// Builds a broad search query from title and year.
    fn build_search_query(title: &str, year: u16) -> String {
        if year == 0 {
            title.to_string()
        } else {
            format!("{} {}", title, year)
        }
    }

    /// Constructs the full HTTPS request URL for a given query and page.
    fn build_url(query: &str, page: u32) -> String {
        format!(
            "https://api.vimeo.com/videos?query={}&fields=uri,name,duration,link&per_page={}&page={}",
            urlencoding::encode(query),
            PER_PAGE,
            page
        )
    }

    /// Filters a Vimeo video and maps it to a `VideoSource`.
    /// Returns `None` if the video fails title relevance, duration, or keyword filters.
    fn map_video_to_source(
        video: &VimeoVideo,
        movie_title: &str,
        year: u16,
    ) -> Option<VideoSource> {
        // Title relevance: video must mention the movie title
        if !title_matching::contains_movie_title(&video.name, movie_title) {
            debug!(
                "Vimeo: excluding '{}' - does not contain movie title '{}'",
                video.name, movie_title
            );
            return None;
        }
        // Reject videos that reference a sequel/different numbered entry
        if title_matching::mentions_sequel_number(&video.name, movie_title) {
            debug!(
                "Vimeo: excluding '{}' - mentions sequel number",
                video.name
            );
            return None;
        }
        // Reject videos that mention a different year (likely a different film)
        if year > 0 && title_matching::mentions_different_year(&video.name, year) {
            debug!(
                "Vimeo: excluding '{}' - mentions different year (expected {})",
                video.name, year
            );
            return None;
        }
        // Duration filter: 30s–2400s (40 minutes), same as DailymotionDiscoverer
        if !(30..=2400).contains(&video.duration) {
            return None;
        }
        // Keyword exclusion
        if title_matching::contains_excluded_keywords(&video.name) {
            return None;
        }
        let category = title_matching::infer_category_from_title(&video.name)
            .unwrap_or(ContentCategory::Extras);
        Some(VideoSource {
            url: video.link.clone(),
            source_type: SourceType::Vimeo,
            category,
            title: video.name.clone(),
            season_number: None,
            duration_secs: Some(video.duration),
        })
    }

    /// Searches Vimeo for extras matching the given title and year.
    ///
    /// Paginates up to 3 pages. Stops early when `paging.next` is `None`
    /// or a page fetch fails.
    pub async fn discover(
        &self,
        title: &str,
        year: u16,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        let query = Self::build_search_query(title, year);
        let mut all_sources = Vec::new();

        for page in 1u32..=MAX_PAGES {
            match self.fetch_page(&query, page).await {
                Ok(response) => {
                    for video in &response.data {
                        if let Some(source) = Self::map_video_to_source(video, title, year) {
                            all_sources.push(source);
                        }
                    }
                    if response.paging.next.is_none() {
                        break;
                    }
                }
                Err(e) => {
                    warn!("Vimeo page {} fetch failed: {}", page, e);
                    break;
                }
            }
        }

        info!(
            "Vimeo: found {} extras for {} ({})",
            all_sources.len(),
            title,
            year
        );
        Ok(all_sources)
    }

    /// Builds an authenticated GET request for the given URL.
    /// Centralizes auth header construction — token must never appear in logs.
    fn build_request(&self, url: &str) -> reqwest::RequestBuilder {
        self.client.get(url).header(
            reqwest::header::AUTHORIZATION,
            format!("bearer {}", self.access_token),
        )
    }

    /// Fetches a single page of search results from the Vimeo API.
    ///
    /// Handles HTTP 429 by waiting 1 second and retrying once (NFR10).
    async fn fetch_page(&self, query: &str, page: u32) -> Result<VimeoResponse, DiscoveryError> {
        let url = Self::build_url(query, page);

        let response = self
            .build_request(&url)
            .send()
            .await
            .map_err(DiscoveryError::NetworkError)?;

        // HTTP 429 — wait 1s and retry once (NFR10)
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            warn!("Vimeo rate limited (429), retrying after 1s");
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let retry = match self.build_request(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    info!("Vimeo retry network error: {}", e);
                    return Ok(VimeoResponse {
                        data: vec![],
                        paging: VimeoPaging { next: None },
                    });
                }
            };
            if !retry.status().is_success() {
                info!("Vimeo retry failed with status {}", retry.status());
                return Ok(VimeoResponse {
                    data: vec![],
                    paging: VimeoPaging { next: None },
                });
            }
            return self.parse_response(retry).await;
        }

        if !response.status().is_success() {
            return Err(DiscoveryError::ApiError(format!(
                "Vimeo returned {}",
                response.status()
            )));
        }

        self.parse_response(response).await
    }

    /// Parses a successful Vimeo response body.
    /// Logs a warning with a raw snippet on parse failure (NFR15).
    async fn parse_response(
        &self,
        response: reqwest::Response,
    ) -> Result<VimeoResponse, DiscoveryError> {
        let text = response
            .text()
            .await
            .map_err(DiscoveryError::NetworkError)?;
        match serde_json::from_str::<VimeoResponse>(&text) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let snippet: String = text.chars().take(200).collect();
                warn!("Vimeo response parse failed: {}. Raw: {}", e, snippet);
                Err(DiscoveryError::ApiError(format!(
                    "Vimeo parse error: {}",
                    e
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    fn make_video(name: &str, duration: u32, link: &str) -> VimeoVideo {
        VimeoVideo {
            uri: format!("/videos/{}", duration),
            name: name.to_string(),
            duration,
            link: link.to_string(),
        }
    }

    // --- Task 2.1: URL construction ---
    #[test]
    fn test_build_url_uses_https_and_encodes_query() {
        let url = VimeoDiscoverer::build_url("Inception 2010", 1);
        assert!(url.starts_with("https://api.vimeo.com"));
        assert!(url.contains("query=Inception%202010"));
        assert!(url.contains("page=1"));
        assert!(url.contains("per_page=10"));
        assert!(url.contains("fields=uri,name,duration,link"));
    }

    // --- Task 2.2: duration too short ---
    #[test]
    fn test_map_video_duration_too_short_filtered() {
        let video = make_video("Inception Trailer", 20, "https://vimeo.com/111");
        assert!(VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010).is_none());
    }

    // --- Task 2.3: duration too long ---
    #[test]
    fn test_map_video_duration_too_long_filtered() {
        let video = make_video("Inception Full Movie", 2401, "https://vimeo.com/222");
        assert!(VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010).is_none());
    }

    // --- Task 2.4: valid duration included ---
    #[test]
    fn test_map_video_duration_valid_included() {
        let video = make_video("Inception Behind the Scenes", 120, "https://vimeo.com/333");
        let source = VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010);
        assert!(source.is_some());
        let source = source.expect("should be Some");
        assert_eq!(source.source_type, SourceType::Vimeo);
        assert_eq!(source.url, "https://vimeo.com/333");
    }

    // --- Task 2.5: excluded keyword ---
    #[test]
    fn test_map_video_excluded_keyword_filtered() {
        let video = make_video("Inception Movie Review", 300, "https://vimeo.com/444");
        assert!(VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010).is_none());
    }

    // --- Task 2.6: category inferred ---
    #[test]
    fn test_map_video_category_inferred_from_title() {
        let video = make_video("Inception Official Trailer", 148, "https://vimeo.com/555");
        let source = VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010)
            .expect("should be Some");
        assert_eq!(source.category, ContentCategory::Trailer);
    }

    // --- Task 2.7: category fallback ---
    #[test]
    fn test_map_video_category_fallback_to_extras() {
        let video = make_video("Inception Cast at Premiere", 200, "https://vimeo.com/666");
        let source = VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010)
            .expect("should be Some");
        assert_eq!(source.category, ContentCategory::Extras);
    }

    // --- Task 2.8: url uses link not uri ---
    #[test]
    fn test_map_video_url_uses_link_not_uri() {
        let video = VimeoVideo {
            uri: "/videos/999".to_string(),
            name: "Inception Trailer".to_string(),
            duration: 120,
            link: "https://vimeo.com/999".to_string(),
        };
        let source = VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010)
            .expect("should be Some");
        assert_eq!(source.url, video.link);
        assert_ne!(source.url, video.uri);
    }

    // --- Task 2.8b: unrelated video is filtered by title relevance ---
    #[test]
    fn test_map_video_unrelated_title_filtered() {
        let video = make_video("Ben Wilkins Showreel", 120, "https://vimeo.com/unrelated");
        assert!(
            VimeoDiscoverer::map_video_to_source(&video, "2 Fast 2 Furious", 2003).is_none()
        );
    }

    // --- Boundary: duration filter inclusive bounds ---
    #[test]
    fn test_map_video_duration_boundary_min() {
        let video = make_video("Inception Clip", 30, "https://vimeo.com/min");
        assert!(VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010).is_some());
    }

    #[test]
    fn test_map_video_duration_boundary_max() {
        let video = make_video("Inception Documentary", 2400, "https://vimeo.com/max");
        assert!(VimeoDiscoverer::map_video_to_source(&video, "Inception", 2400).is_some());
    }

    // --- Task 2.9: duration_secs populated ---
    #[test]
    fn test_map_video_duration_secs_populated() {
        let video = make_video("Inception Featurette", 360, "https://vimeo.com/777");
        let source = VimeoDiscoverer::map_video_to_source(&video, "Inception", 2010)
            .expect("should be Some");
        assert_eq!(source.duration_secs, Some(360));
    }

    // --- Task 2.10: parse response fixture ---
    #[test]
    fn test_parse_vimeo_response_fixture() {
        let json = r#"{
            "data": [
                {
                    "uri": "/videos/123456",
                    "name": "Inception - Official Trailer",
                    "duration": 148,
                    "link": "https://vimeo.com/123456"
                },
                {
                    "uri": "/videos/789012",
                    "name": "Inception Behind the Scenes Featurette",
                    "duration": 600,
                    "link": "https://vimeo.com/789012"
                }
            ],
            "paging": {
                "next": null
            }
        }"#;

        let response: VimeoResponse = serde_json::from_str(json).expect("parse fixture");
        assert_eq!(response.data.len(), 2);
        assert!(response.paging.next.is_none());

        let first = &response.data[0];
        assert_eq!(first.uri, "/videos/123456");
        assert_eq!(first.name, "Inception - Official Trailer");
        assert_eq!(first.duration, 148);
        assert_eq!(first.link, "https://vimeo.com/123456");

        // Verify map_video_to_source produces correct VideoSource
        let source = VimeoDiscoverer::map_video_to_source(first, "Inception", 2010)
            .expect("should be Some");
        assert_eq!(source.source_type, SourceType::Vimeo);
        assert_eq!(source.category, ContentCategory::Trailer);
        assert_eq!(source.title, "Inception - Official Trailer");
        assert_eq!(source.url, "https://vimeo.com/123456");
    }

    // --- Task 2.11: empty data ---
    #[test]
    fn test_parse_empty_data_returns_empty_vec() {
        let json = r#"{"data": [], "paging": {"next": null}}"#;
        let response: VimeoResponse = serde_json::from_str(json).expect("parse empty");
        assert!(response.data.is_empty());
        assert!(response.paging.next.is_none());
    }

    // --- Task 2.12: constructor compiles with renamed fields ---
    #[tokio::test]
    async fn test_vimeo_discoverer_new_compiles() {
        let discoverer = VimeoDiscoverer::new("test_token".to_string());
        // Verify the discoverer is functional — discover returns Ok
        let result = discoverer.discover("Test", 2020).await;
        assert!(result.is_ok());
    }

    // --- Existing orchestrator wiring tests (from Story 8.1) ---
    #[test]
    fn test_discovery_orchestrator_with_vimeo_source() {
        use crate::discovery::DiscoveryOrchestrator;
        use crate::models::Source;

        let _orch = DiscoveryOrchestrator::new(
            "tmdb_key".to_string(),
            vec![Source::Vimeo],
            Arc::new(AtomicU32::new(0)),
            "token".to_string(),
        );
    }

    #[test]
    fn test_series_orchestrator_with_vimeo_source() {
        use crate::discovery::SeriesDiscoveryOrchestrator;
        use crate::models::Source;

        let _orch = SeriesDiscoveryOrchestrator::new(
            "tmdb_key".to_string(),
            vec![Source::Vimeo],
            "token".to_string(),
        );
    }
}
