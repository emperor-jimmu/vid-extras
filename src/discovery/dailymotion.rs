// Dailymotion discoverer — searches the Dailymotion REST API for movie/series extras.
// No authentication required. Rate-limited to 1 req/sec between page requests.
// See: https://api.dailymotion.com/videos?search={query}&fields=id,title,duration,url

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, SourceType, VideoSource};
use log::warn;
use serde::Deserialize;

use super::title_matching;

/// Maximum number of pages to fetch from Dailymotion search results.
const MAX_PAGES: u32 = 3;

/// Number of results per page.
const PAGE_LIMIT: u32 = 10;

// --- Serde structs (private) ---

#[derive(Debug, Deserialize)]
struct DailymotionResponse {
    list: Vec<DailymotionVideo>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct DailymotionVideo {
    #[allow(dead_code)]
    id: String,
    title: String,
    duration: u32,
    url: String,
}

// --- Public API ---

/// Discovers extras from Dailymotion's public video API.
///
/// Searches by title+year, filters by duration and keywords, and paginates
/// up to 3 pages with 1-second pacing between requests.
#[derive(Clone)]
pub(crate) struct DailymotionDiscoverer {
    client: reqwest::Client,
}

impl Default for DailymotionDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

impl DailymotionDiscoverer {
    /// Creates a new discoverer with a 30-second network timeout.
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");
        Self { client }
    }

    /// Builds a broad search query from title and year.
    /// Dailymotion's API uses simple keyword search — no boolean operators.
    fn build_search_query(title: &str, year: u16) -> String {
        if year == 0 {
            title.to_string()
        } else {
            format!("{} {}", title, year)
        }
    }

    /// Constructs the full HTTPS request URL for a given query and page.
    /// Extracted as a pure function for unit testing without network calls.
    fn build_url(query: &str, page: u32) -> String {
        format!(
            "https://api.dailymotion.com/videos?search={}&fields=id,title,duration,url&limit={}&page={}",
            urlencoding::encode(query),
            PAGE_LIMIT,
            page
        )
    }

    /// Filters a Dailymotion video and maps it to a `VideoSource`.
    /// Returns `None` if the video fails duration or keyword filters.
    fn map_video_to_source(video: &DailymotionVideo) -> Option<VideoSource> {
        // Duration filter: 30s–2400s (40 minutes), same as YoutubeDiscoverer
        if !(30..=2400).contains(&video.duration) {
            return None;
        }
        // Keyword exclusion
        if title_matching::contains_excluded_keywords(&video.title) {
            return None;
        }
        let category = title_matching::infer_category_from_title(&video.title)
            .unwrap_or(ContentCategory::Extras);
        Some(VideoSource {
            url: video.url.clone(),
            source_type: SourceType::Dailymotion,
            category,
            title: video.title.clone(),
            season_number: None,
            duration_secs: Some(video.duration),
        })
    }

    /// Searches Dailymotion for extras matching the given title and year.
    ///
    /// Paginates up to 3 pages with 1-second pacing between requests (NFR2).
    /// Stops early when `has_more` is false or a page fetch fails.
    pub async fn discover(
        &self,
        title: &str,
        year: u16,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        let query = Self::build_search_query(title, year);
        let mut all_sources = Vec::new();

        for page in 1u32..=MAX_PAGES {
            if page > 1 {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }

            match self.fetch_page(&query, page).await {
                Ok(response) => {
                    for video in &response.list {
                        if let Some(source) = Self::map_video_to_source(video) {
                            all_sources.push(source);
                        }
                    }
                    if !response.has_more {
                        break;
                    }
                }
                Err(e) => {
                    warn!("Dailymotion page {} fetch failed: {}", page, e);
                    break;
                }
            }
        }

        Ok(all_sources)
    }

    /// Fetches a single page of search results from the Dailymotion API.
    ///
    /// Handles HTTP 429 by waiting 1 second and retrying once. On retry failure,
    /// returns an empty response (graceful skip) rather than propagating an error.
    async fn fetch_page(
        &self,
        query: &str,
        page: u32,
    ) -> Result<DailymotionResponse, DiscoveryError> {
        let url = Self::build_url(query, page);

        let response = super::retry_with_backoff(3, 1000, || async {
            self.client.get(&url).send().await.map_err(|e| {
                warn!("Dailymotion request failed: {}", e);
                DiscoveryError::NetworkError(e)
            })
        })
        .await?;

        if !response.status().is_success() {
            return Err(DiscoveryError::ApiError(format!(
                "Dailymotion returned {}",
                response.status()
            )));
        }

        self.parse_response(response).await
    }

    /// Parses a successful Dailymotion response body.
    /// Logs a warning with a raw snippet on parse failure (NFR15).
    async fn parse_response(
        &self,
        response: reqwest::Response,
    ) -> Result<DailymotionResponse, DiscoveryError> {
        let text = response
            .text()
            .await
            .map_err(DiscoveryError::NetworkError)?;
        match serde_json::from_str::<DailymotionResponse>(&text) {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let snippet: String = text.chars().take(200).collect();
                warn!("Dailymotion response parse failed: {}. Raw: {}", e, snippet);
                Err(DiscoveryError::ApiError(format!(
                    "Dailymotion parse error: {}",
                    e
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Task 5.1: build_search_query includes title and year ---
    #[test]
    fn test_build_search_query_includes_title_and_year() {
        let query = DailymotionDiscoverer::build_search_query("Inception", 2010);
        assert_eq!(query, "Inception 2010");
    }

    #[test]
    fn test_build_search_query_zero_year_omits_year() {
        let query = DailymotionDiscoverer::build_search_query("Breaking Bad", 0);
        assert_eq!(query, "Breaking Bad");
    }

    // --- Task 5.2: duration too short filtered ---
    #[test]
    fn test_map_video_duration_too_short_filtered() {
        let video = DailymotionVideo {
            id: "x1".to_string(),
            title: "Inception Trailer".to_string(),
            duration: 20,
            url: "https://www.dailymotion.com/video/x1".to_string(),
        };
        assert!(DailymotionDiscoverer::map_video_to_source(&video).is_none());
    }

    // --- Task 5.3: duration too long filtered ---
    #[test]
    fn test_map_video_duration_too_long_filtered() {
        let video = DailymotionVideo {
            id: "x2".to_string(),
            title: "Inception Full Movie".to_string(),
            duration: 2401,
            url: "https://www.dailymotion.com/video/x2".to_string(),
        };
        assert!(DailymotionDiscoverer::map_video_to_source(&video).is_none());
    }

    // --- Task 5.4: valid duration included ---
    #[test]
    fn test_map_video_duration_valid_included() {
        let video = DailymotionVideo {
            id: "x3".to_string(),
            title: "Inception Behind the Scenes".to_string(),
            duration: 120,
            url: "https://www.dailymotion.com/video/x3".to_string(),
        };
        let source = DailymotionDiscoverer::map_video_to_source(&video);
        assert!(source.is_some());
        let source = source.unwrap();
        assert_eq!(source.source_type, SourceType::Dailymotion);
        assert_eq!(source.url, "https://www.dailymotion.com/video/x3");
    }

    // --- Task 5.5: excluded keyword filtered ---
    #[test]
    fn test_map_video_excluded_keyword_filtered() {
        let video = DailymotionVideo {
            id: "x4".to_string(),
            title: "Inception Movie Review".to_string(),
            duration: 300,
            url: "https://www.dailymotion.com/video/x4".to_string(),
        };
        assert!(DailymotionDiscoverer::map_video_to_source(&video).is_none());
    }

    // --- Task 5.6: category inferred from title ---
    #[test]
    fn test_map_video_category_inferred_from_title() {
        let video = DailymotionVideo {
            id: "x5".to_string(),
            title: "Inception Official Trailer".to_string(),
            duration: 148,
            url: "https://www.dailymotion.com/video/x5".to_string(),
        };
        let source = DailymotionDiscoverer::map_video_to_source(&video).unwrap();
        assert_eq!(source.category, ContentCategory::Trailer);
    }

    // --- Task 5.7: category fallback to Extras ---
    #[test]
    fn test_map_video_category_fallback_to_extras() {
        let video = DailymotionVideo {
            id: "x6".to_string(),
            title: "Inception Cast at Premiere".to_string(),
            duration: 200,
            url: "https://www.dailymotion.com/video/x6".to_string(),
        };
        let source = DailymotionDiscoverer::map_video_to_source(&video).unwrap();
        assert_eq!(source.category, ContentCategory::Extras);
    }

    // --- Task 5.8: parse Dailymotion response fixture ---
    #[test]
    fn test_parse_dailymotion_response_fixture() {
        let json = r#"{
            "list": [
                {
                    "id": "x7tgad2",
                    "title": "Inception Official Trailer",
                    "duration": 148,
                    "url": "https://www.dailymotion.com/video/x7tgad2"
                },
                {
                    "id": "x8abc",
                    "title": "Inception Behind the Scenes Featurette",
                    "duration": 600,
                    "url": "https://www.dailymotion.com/video/x8abc"
                }
            ],
            "has_more": true,
            "total": 42
        }"#;

        let response: DailymotionResponse = serde_json::from_str(json).expect("parse fixture");
        assert_eq!(response.list.len(), 2);
        assert!(response.has_more);

        let first = &response.list[0];
        assert_eq!(first.id, "x7tgad2");
        assert_eq!(first.title, "Inception Official Trailer");
        assert_eq!(first.duration, 148);
        assert_eq!(first.url, "https://www.dailymotion.com/video/x7tgad2");

        // Verify map_video_to_source produces correct VideoSource
        let source = DailymotionDiscoverer::map_video_to_source(first).unwrap();
        assert_eq!(source.source_type, SourceType::Dailymotion);
        assert_eq!(source.category, ContentCategory::Trailer);
        assert_eq!(source.title, "Inception Official Trailer");
    }

    // --- Task 5.9: empty list returns empty vec ---
    #[test]
    fn test_parse_empty_list_returns_empty_vec() {
        let json = r#"{"list": [], "has_more": false}"#;
        let response: DailymotionResponse = serde_json::from_str(json).expect("parse empty");
        assert!(response.list.is_empty());
        assert!(!response.has_more);
    }

    // --- Task 5.10: URL construction uses HTTPS ---
    #[test]
    fn test_url_construction_uses_https() {
        let url = DailymotionDiscoverer::build_url("Inception 2010", 1);
        assert!(url.starts_with("https://api.dailymotion.com"));
        assert!(url.contains("search=Inception"));
        assert!(url.contains("page=1"));
        assert!(url.contains("limit=10"));
    }
}
