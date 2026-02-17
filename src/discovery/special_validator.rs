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
const MIN_TITLE_SIMILARITY: u8 = 40;

/// Metadata for a single YouTube search candidate
#[derive(Debug, Clone, Deserialize)]
pub struct VideoCandidate {
    /// YouTube video URL
    #[serde(default, alias = "webpage_url")]
    pub url: String,
    /// Video title
    #[serde(default, alias = "title")]
    pub title: String,
    /// Duration in seconds
    #[serde(default, alias = "duration")]
    pub duration: f64,
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

                if let Some(selected) = Self::pick_best(&candidates, episode) {
                    info!(
                        "S00E{:02} '{}': selected '{}' (similarity={}%, duration={}s)",
                        episode.number,
                        episode.name,
                        selected.video_title,
                        FuzzyMatcher::get_similarity_score(&episode.name, &selected.video_title),
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
                Ok(c) if !c.url.is_empty() => candidates.push(c),
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

    /// Score and pick the best candidate for a given episode
    fn pick_best(
        candidates: &[VideoCandidate],
        episode: &TvdbEpisodeExtended,
    ) -> Option<SelectedSpecial> {
        let mut best: Option<(u32, SelectedSpecial)> = None;

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

            let similarity = FuzzyMatcher::get_similarity_score(&episode.name, &candidate.title);

            // Hard reject: title must meet minimum similarity
            if similarity < MIN_TITLE_SIMILARITY {
                debug!(
                    "Rejecting '{}': similarity {}% < {}%",
                    candidate.title, similarity, MIN_TITLE_SIMILARITY
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
                url: candidate.url.clone(),
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
                "THE LEVELING OF SOLO LEVELING Part 1 A Hunter Rises",
                1200.0,
                "https://yt/b",
            ),
            candidate("Solo Leveling Trailer", 90.0, "https://yt/c"),
        ];

        let result = SpecialValidator::pick_best(&candidates, &ep);
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
        let result = SpecialValidator::pick_best(&candidates, &ep);
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_best_accepts_long_movie() {
        let ep = episode(2, "ReAwakening", Some(true));
        let candidates = vec![
            candidate("ReAwakening Trailer", 120.0, "https://yt/a"),
            candidate("ReAwakening Full Movie", 7500.0, "https://yt/b"),
        ];

        // "ReAwakening Trailer" is under 600s so rejected as a movie
        // "ReAwakening Full Movie" passes duration and similarity
        let result = SpecialValidator::pick_best(&candidates, &ep);
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

        let result = SpecialValidator::pick_best(&candidates, &ep);
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_best_non_movie_allows_short_duration() {
        let ep = episode(1, "How to Get Stronger", None);
        let candidates = vec![candidate(
            "Solo Leveling How to Get Stronger Recap",
            300.0,
            "https://yt/a",
        )];

        let result = SpecialValidator::pick_best(&candidates, &ep);
        assert!(result.is_some());
    }

    #[test]
    fn test_pick_best_movie_boundary_600s() {
        let ep = episode(2, "ReAwakening", Some(true));

        // Exactly 600s should pass
        let pass = vec![candidate("ReAwakening", 600.0, "https://yt/a")];
        assert!(SpecialValidator::pick_best(&pass, &ep).is_some());

        // 599s should fail
        let fail = vec![candidate("ReAwakening", 599.0, "https://yt/b")];
        assert!(SpecialValidator::pick_best(&fail, &ep).is_none());
    }

    #[test]
    fn test_pick_best_empty_candidates() {
        let ep = episode(1, "Test", None);
        let result = SpecialValidator::pick_best(&[], &ep);
        assert!(result.is_none());
    }

    #[test]
    fn test_pick_best_prefers_longer_movie() {
        let ep = episode(2, "ReAwakening", Some(true));
        let candidates = vec![
            candidate("ReAwakening", 700.0, "https://yt/short"),
            candidate("ReAwakening", 7500.0, "https://yt/full"),
        ];

        let result = SpecialValidator::pick_best(&candidates, &ep);
        assert!(result.is_some());
        // Should prefer the longer one due to duration bonus
        assert_eq!(
            result.as_ref().expect("should match").url,
            "https://yt/full"
        );
    }
}
