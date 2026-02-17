// Special episode validation module
//
// Searches YouTube for candidate videos matching TVDB special episodes,
// scores them by title similarity and duration, and selects the best match.

use crate::discovery::fuzzy_matching::FuzzyMatcher;
use crate::discovery::special_searcher::SpecialSearcher;
use crate::discovery::tvdb::TvdbEpisodeExtended;
use log::{debug, info, warn};
use serde::Deserialize;
use tokio::process::Command;

/// Minimum duration in seconds for a movie-flagged special (10 minutes)
const MOVIE_MIN_DURATION_SECS: u32 = 600;

/// Minimum title similarity score (0-100) to consider a candidate
const MIN_TITLE_SIMILARITY: u8 = 60;

/// Metadata for a single YouTube search candidate
#[derive(Debug, Clone, Deserialize)]
pub struct VideoCandidate {
    /// YouTube video URL (primary field from yt-dlp)
    #[serde(default)]
    pub url: String,
    /// Fallback URL field from yt-dlp
    #[serde(default)]
    pub webpage_url: String,
    /// Video title
    #[serde(default)]
    pub title: String,
    /// Duration in seconds
    #[serde(default)]
    pub duration: f64,
}

impl VideoCandidate {
    /// Get the effective URL, preferring `url` over `webpage_url`
    pub fn get_url(&self) -> String {
        if !self.url.is_empty() {
            self.url.clone()
        } else {
            self.webpage_url.clone()
        }
    }
}

/// Result of selecting the best candidate for a special episode
#[derive(Debug, Clone)]
pub struct SelectedSpecial {
    /// The chosen video URL to download
    pub url: String,
    /// The video title from YouTube
    pub video_title: String,
    /// Duration in seconds
    pub duration: u32,
}

/// Searches YouTube for candidates and selects the best match per episode
pub struct SpecialValidator;

impl SpecialValidator {
    /// Search YouTube for candidates and select the best match for each episode.
    ///
    /// For each TVDB episode, this method:
    /// 1. Runs `yt-dlp --dump-json ytsearch5:{query}` to get metadata for 5 candidates
    /// 2. Tries each query variant (standard, fallback, movie, anime) until a match is found
    /// 3. Scores candidates by title similarity and duration requirements
    /// 4. Returns the single best URL per episode (or None if no match)
    pub async fn select_best_candidates(
        series_title: &str,
        episodes: &[TvdbEpisodeExtended],
    ) -> Vec<Option<SelectedSpecial>> {
        let mut results = Vec::with_capacity(episodes.len());

        for episode in episodes {
            let queries = SpecialSearcher::build_queries(series_title, episode);
            let mut best: Option<SelectedSpecial> = None;

            for query in &queries {
                let candidates = Self::fetch_candidates(query).await;
                if candidates.is_empty() {
                    debug!("No candidates for query: {}", query);
                    continue;
                }

                if let Some(selected) = Self::pick_best(&candidates, episode, series_title) {
                    let best_variant = episode
                        .name_variants()
                        .iter()
                        .map(|v| FuzzyMatcher::get_similarity_score(v, &selected.video_title))
                        .max()
                        .unwrap_or(0);
                    info!(
                        "S00E{:02} '{}': selected '{}' (similarity={}%, duration={}s)",
                        episode.number,
                        episode.name,
                        selected.video_title,
                        best_variant,
                        selected.duration,
                    );
                    best = Some(selected);
                    break; // First query that yields a valid match wins
                }
            }

            if best.is_none() {
                warn!(
                    "S00E{:02} '{}': no valid candidate found across {} queries",
                    episode.number,
                    episode.name,
                    queries.len()
                );
            }

            results.push(best);
        }

        results
    }

    /// Run `yt-dlp --dump-json` to fetch metadata for up to 5 search results
    async fn fetch_candidates(query: &str) -> Vec<VideoCandidate> {
        let search_url = format!("ytsearch5:{}", query);

        let output = Command::new("yt-dlp")
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg("--no-download")
            .arg("--no-warnings")
            .arg("--quiet")
            .arg("--js-runtimes")
            .arg("node")
            .arg(&search_url)
            .output()
            .await;

        let output = match output {
            Ok(o) if o.status.success() => o,
            Ok(o) => {
                debug!(
                    "yt-dlp search failed for '{}': {}",
                    query,
                    String::from_utf8_lossy(&o.stderr)
                );
                return Vec::new();
            }
            Err(e) => {
                warn!("Failed to execute yt-dlp for search '{}': {}", query, e);
                return Vec::new();
            }
        };

        // Each line of stdout is a separate JSON object
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut candidates = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str::<VideoCandidate>(trimmed) {
                Ok(c) if !c.get_url().is_empty() => candidates.push(c),
                Ok(_) => debug!("Skipping candidate with empty URL"),
                Err(e) => debug!("Failed to parse candidate JSON: {}", e),
            }
        }

        debug!(
            "Fetched {} candidates for query '{}'",
            candidates.len(),
            query
        );
        candidates
    }

    /// Score and pick the best candidate for a given episode.
    ///
    /// Strips the series title from candidate video titles before comparing
    /// to the episode name, and tries all name variants (including English
    /// translations) to find the best match.
    fn pick_best(
        candidates: &[VideoCandidate],
        episode: &TvdbEpisodeExtended,
        series_title: &str,
    ) -> Option<SelectedSpecial> {
        let mut best: Option<(u32, SelectedSpecial)> = None;
        let name_variants = episode.name_variants();
        let normalized_series = FuzzyMatcher::normalize(series_title);

        for candidate in candidates {
            let duration_secs = candidate.duration as u32;

            // Hard reject: movies must be >= 10 minutes
            if episode.is_movie == Some(true) && duration_secs < MOVIE_MIN_DURATION_SECS {
                debug!(
                    "Rejecting '{}': movie duration {}s < {}s",
                    candidate.title, duration_secs, MOVIE_MIN_DURATION_SECS
                );
                continue;
            }

            // Strip series title from candidate title before comparing
            let normalized_candidate = FuzzyMatcher::normalize(&candidate.title);
            let stripped_candidate = normalized_candidate
                .replacen(&normalized_series, "", 1)
                .trim()
                .to_string();

            // Use the stripped title if it's non-empty, otherwise fall back to original
            let compare_title = if stripped_candidate.is_empty() {
                &candidate.title
            } else {
                &stripped_candidate
            };

            // Try all name variants and take the best similarity
            let similarity = name_variants
                .iter()
                .map(|variant| FuzzyMatcher::get_similarity_score(variant, compare_title))
                .max()
                .unwrap_or(0);

            // Hard reject: title must meet minimum similarity
            if similarity < MIN_TITLE_SIMILARITY {
                debug!(
                    "Rejecting '{}': best similarity {}% < {}% (stripped: '{}')",
                    candidate.title, similarity, MIN_TITLE_SIMILARITY, compare_title
                );
                continue;
            }

            // Composite score: similarity is primary, duration bonus for movies
            let mut score = similarity as u32 * 10;

            // Bonus for movies with longer duration (closer to expected runtime)
            if episode.is_movie == Some(true) && duration_secs >= MOVIE_MIN_DURATION_SECS {
                // Prefer longer videos for movies (up to 3 hours)
                let duration_bonus = (duration_secs / 60).min(30);
                score += duration_bonus;
            }

            let selected = SelectedSpecial {
                url: candidate.get_url(),
                video_title: candidate.title.clone(),
                duration: duration_secs,
            };

            if let Some((current_best_score, _)) = &best {
                if score > *current_best_score {
                    best = Some((score, selected));
                }
            } else {
                best = Some((score, selected));
            }
        }

        best.map(|(_, selected)| selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episode(number: u8, name: &str, is_movie: Option<bool>) -> TvdbEpisodeExtended {
        TvdbEpisodeExtended {
            id: number as u64,
            number,
            name: name.to_string(),
            name_eng: None,
            aired: None,
            overview: None,
            absolute_number: None,
            airs_before_season: None,
            airs_after_season: None,
            airs_before_episode: None,
            is_movie,
        }
    }

    fn candidate(title: &str, duration: f64, url: &str) -> VideoCandidate {
        VideoCandidate {
            url: url.to_string(),
            webpage_url: String::new(),
            title: title.to_string(),
            duration,
        }
    }

    #[test]
    fn test_pick_best_selects_highest_similarity() {
        let ep = episode(3, "THE LEVELING OF SOLO LEVELING Part 1", None);
        let candidates = vec![
            candidate("Random Anime Clip", 300.0, "https://yt/a"),
            candidate(
                "Solo Leveling THE LEVELING OF SOLO LEVELING Part 1 A Hunter Rises",
                1200.0,
                "https://yt/b",
            ),
            candidate("Solo Leveling Trailer", 90.0, "https://yt/c"),
        ];

        let result = SpecialValidator::pick_best(&candidates, &ep, "Solo Leveling");
        assert!(result.is_some());
        assert_eq!(result.as_ref().expect("should match").url, "https://yt/b");
    }

    #[test]
    fn test_pick_best_rejects_short_movie() {
        let ep = episode(2, "ReAwakening", Some(true));
        let candidates = vec![
            candidate("ReAwakening Trailer", 120.0, "https://yt/a"),
            candidate("ReAwakening Clip", 300.0, "https://yt/b"),
        ];

        // Both are under 10 minutes, should reject all for a movie
        let result = SpecialValidator::pick_best(&candidates, &ep, "Solo Leveling");
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_best_accepts_long_movie() {
        let ep = episode(2, "ReAwakening", Some(true));
        let candidates = vec![
            candidate("ReAwakening Trailer", 120.0, "https://yt/a"),
            candidate("Solo Leveling ReAwakening", 7500.0, "https://yt/b"),
        ];

        let result = SpecialValidator::pick_best(&candidates, &ep, "Solo Leveling");
        assert!(result.is_some());
        assert_eq!(result.as_ref().expect("should match").url, "https://yt/b");
    }

    #[test]
    fn test_pick_best_rejects_low_similarity() {
        let ep = episode(1, "How to Get Stronger", None);
        let candidates = vec![candidate(
            "Completely Unrelated Video About Cooking",
            600.0,
            "https://yt/a",
        )];

        let result = SpecialValidator::pick_best(&candidates, &ep, "Solo Leveling");
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_best_non_movie_allows_short_duration() {
        let ep = episode(1, "How to Get Stronger", None);
        let candidates = vec![candidate(
            "How to Get Stronger Recap",
            300.0,
            "https://yt/a",
        )];

        let result = SpecialValidator::pick_best(&candidates, &ep, "Some Series");
        assert!(result.is_some());
    }

    #[test]
    fn test_pick_best_movie_boundary_600s() {
        let ep = episode(2, "ReAwakening", Some(true));

        // Exactly 600s should pass
        let pass = vec![candidate("ReAwakening", 600.0, "https://yt/a")];
        assert!(SpecialValidator::pick_best(&pass, &ep, "Test").is_some());

        // 599s should fail
        let fail = vec![candidate("ReAwakening", 599.0, "https://yt/b")];
        assert!(SpecialValidator::pick_best(&fail, &ep, "Test").is_none());
    }

    #[test]
    fn test_pick_best_empty_candidates() {
        let ep = episode(1, "Test", None);
        let result = SpecialValidator::pick_best(&[], &ep, "Test");
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_best_prefers_longer_movie() {
        let ep = episode(2, "ReAwakening", Some(true));
        let candidates = vec![
            candidate("ReAwakening", 700.0, "https://yt/short"),
            candidate("ReAwakening", 7500.0, "https://yt/full"),
        ];

        let result = SpecialValidator::pick_best(&candidates, &ep, "Test");
        assert!(result.is_some());
        // Should prefer the longer one due to duration bonus
        assert_eq!(
            result.as_ref().expect("should match").url,
            "https://yt/full"
        );
    }

    #[test]
    fn test_pick_best_strips_series_title() {
        // Episode name is Japanese, but English translation is available
        let mut ep = episode(1, "強くなる方法", None);
        ep.name_eng = Some("How to Get Stronger".to_string());

        // Candidate title contains the series name + episode name in English
        let candidates = vec![candidate(
            "Solo Leveling How to Get Stronger Special",
            600.0,
            "https://yt/a",
        )];

        // After stripping "Solo Leveling", the candidate becomes "How to Get Stronger Special"
        // which should match the English variant "How to Get Stronger"
        let result = SpecialValidator::pick_best(&candidates, &ep, "Solo Leveling");
        assert!(result.is_some());
    }

    #[test]
    fn test_pick_best_uses_english_variant() {
        // Episode with Japanese name and English translation
        let mut ep = episode(2, "再覚醒", Some(true));
        ep.name_eng = Some("ReAwakening".to_string());

        let candidates = vec![candidate(
            "Solo Leveling ReAwakening",
            7500.0,
            "https://yt/a",
        )];

        let result = SpecialValidator::pick_best(&candidates, &ep, "Solo Leveling");
        assert!(result.is_some());
    }

    #[test]
    fn test_deserialize_with_both_url_and_webpage_url() {
        // Test that deserialization handles both url and webpage_url fields
        let json = r#"{"url": "https://www.youtube.com/watch?v=abc123", "webpage_url": "https://www.youtube.com/watch?v=abc123", "title": "Test Video", "duration": 300.0}"#;
        let candidate: VideoCandidate = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(
            candidate.get_url(),
            "https://www.youtube.com/watch?v=abc123"
        );
        assert_eq!(candidate.title, "Test Video");
        assert_eq!(candidate.duration, 300.0);
    }

    #[test]
    fn test_deserialize_prefers_url_over_webpage_url() {
        // Test that url field is preferred when both are present
        let json = r#"{"url": "https://www.youtube.com/watch?v=preferred", "webpage_url": "https://www.youtube.com/watch?v=fallback", "title": "Test", "duration": 100.0}"#;
        let candidate: VideoCandidate = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(
            candidate.get_url(),
            "https://www.youtube.com/watch?v=preferred"
        );
    }

    #[test]
    fn test_deserialize_fallback_to_webpage_url() {
        // Test that webpage_url is used when url is empty
        let json = r#"{"url": "", "webpage_url": "https://www.youtube.com/watch?v=fallback", "title": "Test", "duration": 100.0}"#;
        let candidate: VideoCandidate = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(
            candidate.get_url(),
            "https://www.youtube.com/watch?v=fallback"
        );
    }

    #[test]
    fn test_deserialize_missing_optional_fields() {
        // Test that missing optional fields default to empty/zero
        let json = r#"{"url": "https://www.youtube.com/watch?v=test"}"#;
        let candidate: VideoCandidate = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(candidate.get_url(), "https://www.youtube.com/watch?v=test");
        assert_eq!(candidate.title, "");
        assert_eq!(candidate.duration, 0.0);
    }
}
