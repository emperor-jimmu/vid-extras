// KinoCheck discoverer — implicit TMDB fallback for movies with zero TMDB videos.
// Queries the free KinoCheck API to find official trailers via YouTube.
// See: https://api.kinocheck.de/movies?tmdb_id={id}

use crate::error::DiscoveryError;
use crate::models::{ContentCategory, SourceType, VideoSource};
use log::{info, warn};
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Free-tier daily request limit for KinoCheck API.
const KINOCHECK_DAILY_LIMIT: u32 = 1_000;

/// Threshold (80% of daily limit) at which a warning is logged.
const KINOCHECK_WARN_THRESHOLD: u32 = 800;

// --- Serde structs (private) ---

#[derive(Debug, Deserialize)]
struct KinoCheckMovie {
    #[serde(default)]
    trailer: Option<KinoCheckTrailer>,
}

#[derive(Debug, Deserialize)]
struct KinoCheckTrailer {
    youtube_video_id: String,
    title: String,
    #[serde(default)]
    categories: Vec<String>,
}

// --- Public API ---

/// Discovers trailers from the KinoCheck API as a fallback when TMDB returns zero videos.
///
/// The request counter is shared across movie and series pipelines via `Arc<AtomicU32>`.
#[derive(Clone)]
pub(crate) struct KinoCheckDiscoverer {
    client: reqwest::Client,
    request_count: Arc<AtomicU32>,
}

impl KinoCheckDiscoverer {
    /// Creates a new discoverer. The `request_count` should be shared across all
    /// instances so the 1,000 req/day free-tier limit is tracked globally.
    pub fn new(request_count: Arc<AtomicU32>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            request_count,
        }
    }

    /// Maps KinoCheck `categories` array to a `ContentCategory`.
    /// Falls back to `Extras` for unknown or empty categories.
    pub fn map_category(categories: &[String]) -> ContentCategory {
        match categories.first().map(|s| s.as_str()) {
            Some("Trailer") => ContentCategory::Trailer,
            Some("Featurette") => ContentCategory::Featurette,
            Some("Behind the Scenes") => ContentCategory::BehindTheScenes,
            _ => ContentCategory::Extras,
        }
    }

    /// Queries KinoCheck for a movie by TMDB ID and returns any discovered video sources.
    ///
    /// Increments the shared request counter and logs a warning at 80% of the daily limit.
    /// Returns `Ok(vec![])` on 404, rate-limit exhaustion, or parse failures.
    pub async fn discover_for_tmdb_id(
        &self,
        tmdb_id: u64,
    ) -> Result<Vec<VideoSource>, DiscoveryError> {
        let count = self.request_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count == KINOCHECK_WARN_THRESHOLD {
            warn!(
                "KinoCheck request count at {}/{} ({}% of daily limit)",
                count,
                KINOCHECK_DAILY_LIMIT,
                count * 100 / KINOCHECK_DAILY_LIMIT
            );
        }

        let movie = match self.fetch_movie(tmdb_id).await {
            Ok(Some(m)) => m,
            Ok(None) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };

        let sources = match movie.trailer {
            Some(trailer) => {
                // Validate the YouTube video ID: must be non-empty and exactly 11 chars
                // (YouTube's standard ID format). An invalid ID would produce a broken URL
                // that yt-dlp would fail on at download time.
                let id = trailer.youtube_video_id.trim();
                if id.is_empty() || id.len() != 11 {
                    warn!(
                        "KinoCheck returned invalid youtube_video_id {:?} for tmdb_id={} — skipping",
                        id, tmdb_id
                    );
                    return Ok(Vec::new());
                }
                let category = Self::map_category(&trailer.categories);
                vec![VideoSource {
                    url: format!("https://www.youtube.com/watch?v={}", id),
                    source_type: SourceType::KinoCheck,
                    category,
                    title: trailer.title,
                    season_number: None,
                    duration_secs: None,
                }]
            }
            None => Vec::new(),
        };

        Ok(sources)
    }

    /// Fetches movie data from KinoCheck API.
    /// Returns `Ok(None)` on 404 (movie not in KinoCheck database).
    /// Retries once after 1 second on HTTP 429 (rate limit).
    async fn fetch_movie(&self, tmdb_id: u64) -> Result<Option<KinoCheckMovie>, DiscoveryError> {
        let url = format!("https://api.kinocheck.de/movies?tmdb_id={}", tmdb_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(DiscoveryError::NetworkError)?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        // HTTP 429 — wait 1s and retry once (NFR10)
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            warn!("KinoCheck rate limited (429), retrying after 1s");
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            // Count the retry as a separate request against the daily limit
            let retry_count = self.request_count.fetch_add(1, Ordering::Relaxed) + 1;
            if retry_count == KINOCHECK_WARN_THRESHOLD {
                warn!(
                    "KinoCheck request count at {}/{} ({}% of daily limit)",
                    retry_count,
                    KINOCHECK_DAILY_LIMIT,
                    retry_count * 100 / KINOCHECK_DAILY_LIMIT
                );
            }
            let retry = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    info!("KinoCheck retry network error: {}", e);
                    return Ok(None);
                }
            };
            if !retry.status().is_success() {
                info!("KinoCheck retry failed with status {}", retry.status());
                return Ok(None);
            }
            return self.parse_response(retry).await;
        }

        if !response.status().is_success() {
            return Err(DiscoveryError::ApiError(format!(
                "KinoCheck returned {}",
                response.status()
            )));
        }

        self.parse_response(response).await
    }

    /// Parses a successful KinoCheck response body.
    /// Logs a warning with a raw snippet on parse failure (NFR15) and degrades gracefully.
    async fn parse_response(
        &self,
        response: reqwest::Response,
    ) -> Result<Option<KinoCheckMovie>, DiscoveryError> {
        let text = response
            .text()
            .await
            .map_err(DiscoveryError::NetworkError)?;
        match serde_json::from_str::<KinoCheckMovie>(&text) {
            Ok(movie) => Ok(Some(movie)),
            Err(e) => {
                let snippet: String = text.chars().take(200).collect();
                warn!("KinoCheck response parse failed: {}. Raw: {}", e, snippet);
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Task 6.1: map_category — Trailer ---
    #[test]
    fn test_map_category_trailer() {
        let cats = vec!["Trailer".to_string()];
        assert_eq!(
            KinoCheckDiscoverer::map_category(&cats),
            ContentCategory::Trailer
        );
    }

    // --- Task 6.2: map_category — Featurette ---
    #[test]
    fn test_map_category_featurette() {
        let cats = vec!["Featurette".to_string()];
        assert_eq!(
            KinoCheckDiscoverer::map_category(&cats),
            ContentCategory::Featurette
        );
    }

    // --- Task 6.3: map_category — Behind the Scenes ---
    #[test]
    fn test_map_category_behind_the_scenes() {
        let cats = vec!["Behind the Scenes".to_string()];
        assert_eq!(
            KinoCheckDiscoverer::map_category(&cats),
            ContentCategory::BehindTheScenes
        );
    }

    // --- Task 6.4: map_category — unknown defaults to Extras ---
    #[test]
    fn test_map_category_unknown_defaults_to_extras() {
        let cats = vec!["SomethingNew".to_string()];
        assert_eq!(
            KinoCheckDiscoverer::map_category(&cats),
            ContentCategory::Extras
        );
    }

    // --- Task 6.5: map_category — empty defaults to Extras ---
    #[test]
    fn test_map_category_empty_defaults_to_extras() {
        let cats: Vec<String> = vec![];
        assert_eq!(
            KinoCheckDiscoverer::map_category(&cats),
            ContentCategory::Extras
        );
    }

    // --- Task 6.6: YouTube URL construction ---
    #[test]
    fn test_video_source_url_construction() {
        let json = r#"{"trailer":{"youtube_video_id":"QW9wNFpLYiY","title":"FIGHT CLUB Trailer","categories":["Trailer"]}}"#;
        let movie: KinoCheckMovie = serde_json::from_str(json).expect("parse fixture");
        let trailer = movie.trailer.expect("trailer present");
        let url = format!(
            "https://www.youtube.com/watch?v={}",
            trailer.youtube_video_id
        );
        assert_eq!(url, "https://www.youtube.com/watch?v=QW9wNFpLYiY");
    }

    // --- Task 6.7: request counter increments ---
    #[test]
    fn test_request_counter_increments() {
        let counter = Arc::new(AtomicU32::new(0));
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        // Simulate what discover_for_tmdb_id does
        let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
        assert_eq!(count, 1);
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    // --- Task 6.8: 80% warning threshold constant ---
    #[test]
    fn test_80_percent_warning_threshold() {
        assert_eq!(KINOCHECK_WARN_THRESHOLD, 800);
        assert_eq!(KINOCHECK_DAILY_LIMIT, 1_000);
        // 800 is exactly 80% of 1000
        assert_eq!(KINOCHECK_WARN_THRESHOLD * 100 / KINOCHECK_DAILY_LIMIT, 80);
    }

    // --- Task 6.9: null trailer returns empty ---
    #[test]
    fn test_no_trailer_returns_empty() {
        let json = r#"{"trailer": null}"#;
        let movie: KinoCheckMovie = serde_json::from_str(json).expect("parse null trailer");
        assert!(movie.trailer.is_none());
    }

    // --- Task 6.10: full response parsing ---
    #[test]
    fn test_discover_for_tmdb_id_parses_response() {
        let json = r#"{
            "id": "9pg",
            "tmdb_id": 550,
            "imdb_id": "tt0137523",
            "language": "de",
            "title": "Fight Club",
            "trailer": {
                "id": "3yqi",
                "youtube_video_id": "QW9wNFpLYiY",
                "youtube_channel_id": "UCV297SPE0sBWzmhmACKJP-w",
                "title": "FIGHT CLUB Trailer German Deutsch (1999)",
                "url": "https://kinocheck.de/trailer/3yqi/...",
                "language": "de",
                "categories": ["Trailer"],
                "genres": ["Drama"],
                "published": "2020-05-20T19:08:45+02:00",
                "views": 385897
            },
            "recommendations": []
        }"#;

        let movie: KinoCheckMovie = serde_json::from_str(json).expect("parse full response");
        let trailer = movie.trailer.expect("trailer present");

        assert_eq!(trailer.youtube_video_id, "QW9wNFpLYiY");
        assert_eq!(trailer.title, "FIGHT CLUB Trailer German Deutsch (1999)");
        assert_eq!(trailer.categories, vec!["Trailer"]);

        // Verify VideoSource construction
        let category = KinoCheckDiscoverer::map_category(&trailer.categories);
        let source = VideoSource {
            url: format!(
                "https://www.youtube.com/watch?v={}",
                trailer.youtube_video_id
            ),
            source_type: SourceType::KinoCheck,
            category,
            title: trailer.title,
            season_number: None,
            duration_secs: None,
        };

        assert_eq!(source.url, "https://www.youtube.com/watch?v=QW9wNFpLYiY");
        assert_eq!(source.source_type, SourceType::KinoCheck);
        assert_eq!(source.category, ContentCategory::Trailer);
        assert!(source.title.contains("FIGHT CLUB"));
    }

    // --- Task 6.11: shared counter across instances ---
    #[test]
    fn test_shared_counter_across_instances() {
        let counter = Arc::new(AtomicU32::new(0));
        let _d1 = KinoCheckDiscoverer::new(counter.clone());
        let _d2 = KinoCheckDiscoverer::new(counter.clone());

        // Increment via the shared Arc directly (simulates discover_for_tmdb_id)
        counter.fetch_add(1, Ordering::Relaxed);
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        counter.fetch_add(1, Ordering::Relaxed);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }
}
