// Deduplication module — tier-based fuzzy deduplication of discovered video sources.
//
// Two videos are considered duplicates when:
//   - Title similarity ≥ 95% (regardless of duration), OR
//   - Title similarity ≥ 80% AND duration is within 10% tolerance
//
// When duplicates are found, the higher-tier source wins:
//   Tier 1: TMDB, KinoCheck, TheTVDB
//   Tier 2: Dailymotion, Vimeo, Archive.org
//   Tier 3: YouTube, Bilibili
//
// Within the same tier, the source that appears earlier in the active source list wins.

use crate::discovery::FuzzyMatcher;
use crate::models::{SeriesExtra, Source, SourceType, VideoSource};
use log::debug;
use std::collections::HashSet;

/// Deduplicate a list of video sources using title similarity and duration comparison.
///
/// Returns the deduplicated list and the number of removed duplicates.
pub(crate) fn deduplicate(
    sources: Vec<VideoSource>,
    active_sources: &[Source],
) -> (Vec<VideoSource>, usize) {
    let to_remove = find_duplicate_indices(
        sources.len(),
        |i| sources[i].title.as_str(),
        |i| sources[i].duration_secs,
        |i| sources[i].source_type.tier(),
        |i| source_order(&sources[i].source_type, active_sources),
    );

    let removed = to_remove.len();
    let deduped = sources
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !to_remove.contains(idx))
        .map(|(_, vs)| vs)
        .collect();

    (deduped, removed)
}

/// Deduplicate a list of series extras using the same logic as `deduplicate`.
///
/// Returns the deduplicated list and the number of removed duplicates.
pub(crate) fn deduplicate_series(
    extras: Vec<SeriesExtra>,
    active_sources: &[Source],
) -> (Vec<SeriesExtra>, usize) {
    let to_remove = find_duplicate_indices(
        extras.len(),
        |i| extras[i].title.as_str(),
        |i| extras[i].duration_secs,
        |i| extras[i].source_type.tier(),
        |i| source_order(&extras[i].source_type, active_sources),
    );

    let removed = to_remove.len();
    let deduped = extras
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !to_remove.contains(idx))
        .map(|(_, e)| e)
        .collect();

    (deduped, removed)
}

/// Core O(n²) pairwise duplicate detection.
///
/// Accepts closures to extract title, duration, tier, and source order from
/// any item type — shared by both `deduplicate` and `deduplicate_series`.
fn find_duplicate_indices<'a, FTitle, FDuration, FTier, FOrder>(
    len: usize,
    title: FTitle,
    duration: FDuration,
    tier: FTier,
    order: FOrder,
) -> HashSet<usize>
where
    FTitle: Fn(usize) -> &'a str,
    FDuration: Fn(usize) -> Option<u32>,
    FTier: Fn(usize) -> u8,
    FOrder: Fn(usize) -> usize,
{
    let mut to_remove: HashSet<usize> = HashSet::new();

    for i in 0..len {
        if to_remove.contains(&i) {
            continue;
        }
        for j in (i + 1)..len {
            if to_remove.contains(&j) {
                continue;
            }

            let similarity = FuzzyMatcher::get_similarity_score(title(i), title(j));

            let is_duplicate = if similarity >= 95 {
                // Very high title similarity — treat as duplicate regardless of duration
                true
            } else if similarity >= 80 {
                // Moderate similarity — require duration match within 10%
                match (duration(i), duration(j)) {
                    (Some(d1), Some(d2)) => {
                        let max_d = d1.max(d2) as f64;
                        let diff = (d1 as f64 - d2 as f64).abs();
                        max_d > 0.0 && (diff / max_d) <= 0.10
                    }
                    // If either duration is None, can't confirm match — not a duplicate
                    _ => false,
                }
            } else {
                false
            };

            if is_duplicate {
                let loser = pick_loser(i, j, &tier, &order);
                debug!(
                    "Duplicate detected (similarity={}%): indices {} and {} — removing {}",
                    similarity, i, j, loser
                );
                to_remove.insert(loser);
            }
        }
    }

    to_remove
}

/// Determine which of two duplicate items to remove.
///
/// Keeps the higher-tier (lower tier number) item. Within the same tier,
/// keeps the item whose source appears earlier in the active source list.
fn pick_loser<FTier, FOrder>(i: usize, j: usize, tier: &FTier, order: &FOrder) -> usize
where
    FTier: Fn(usize) -> u8,
    FOrder: Fn(usize) -> usize,
{
    let tier_i = tier(i);
    let tier_j = tier(j);

    if tier_i != tier_j {
        // Lower tier number = higher priority; loser has higher tier number
        if tier_i < tier_j { j } else { i }
    } else {
        // Same tier — prefer the source that appears earlier in the active list
        let order_i = order(i);
        let order_j = order(j);
        if order_i <= order_j { j } else { i }
    }
}

/// Map a `SourceType` to its corresponding user-facing `Source` variant.
///
/// KinoCheck and TheTVDB are not user-selectable sources, so they return `None`.
fn source_type_to_source(st: &SourceType) -> Option<Source> {
    match st {
        SourceType::TMDB => Some(Source::Tmdb),
        SourceType::ArchiveOrg => Some(Source::Archive),
        SourceType::YouTube => Some(Source::Youtube),
        SourceType::Dailymotion => Some(Source::Dailymotion),
        SourceType::Vimeo => Some(Source::Vimeo),
        SourceType::Bilibili => Some(Source::Bilibili),
        // Not user-selectable — no position in the active source list
        SourceType::KinoCheck | SourceType::TheTVDB => None,
    }
}

/// Return the position of a source type in the active source list.
///
/// Sources not in the active list (or non-user-selectable types) get `usize::MAX`,
/// meaning they lose all same-tier tiebreaks.
fn source_order(source_type: &SourceType, active_sources: &[Source]) -> usize {
    source_type_to_source(source_type)
        .and_then(|s| active_sources.iter().position(|a| *a == s))
        .unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ContentCategory, SourceType, VideoSource};

    fn make_source(title: &str, source_type: SourceType, duration: Option<u32>) -> VideoSource {
        VideoSource {
            url: format!("https://example.com/{}", title.replace(' ', "_")),
            source_type,
            category: ContentCategory::Trailer,
            title: title.to_string(),
            season_number: None,
            duration_secs: duration,
        }
    }

    fn default_sources() -> Vec<Source> {
        vec![
            Source::Tmdb,
            Source::Archive,
            Source::Dailymotion,
            Source::Youtube,
        ]
    }

    // 6.1: No duplicates — all unique titles → all returned, 0 removed
    #[test]
    fn test_no_duplicates_returns_unchanged() {
        let sources = vec![
            make_source("Iron Man Trailer", SourceType::TMDB, Some(120)),
            make_source("Behind the Scenes", SourceType::YouTube, Some(300)),
            make_source("Deleted Scene 1", SourceType::ArchiveOrg, Some(180)),
        ];
        let (result, removed) = deduplicate(sources, &default_sources());
        assert_eq!(result.len(), 3);
        assert_eq!(removed, 0);
    }

    // 6.2: Title ≥ 80% + duration within 10% → lower tier removed
    #[test]
    fn test_title_and_duration_match_removes_lower_tier() {
        let sources = vec![
            make_source("Iron Man Official Trailer", SourceType::TMDB, Some(120)),
            make_source("Iron Man Official Trailer", SourceType::YouTube, Some(125)),
        ];
        let (result, removed) = deduplicate(sources, &default_sources());
        assert_eq!(result.len(), 1);
        assert_eq!(removed, 1);
        // TMDB (tier 1) should survive over YouTube (tier 3)
        assert_eq!(result[0].source_type, SourceType::TMDB);
    }

    // 6.3: Title ≥ 95% → duplicate regardless of duration difference
    #[test]
    fn test_high_similarity_ignores_duration() {
        let sources = vec![
            make_source("Iron Man Trailer", SourceType::TMDB, Some(120)),
            make_source("Iron Man Trailer", SourceType::YouTube, Some(600)),
        ];
        let (result, removed) = deduplicate(sources, &default_sources());
        assert_eq!(result.len(), 1);
        assert_eq!(removed, 1);
        assert_eq!(result[0].source_type, SourceType::TMDB);
    }

    // 6.4: Same tier — earlier in active source list wins
    #[test]
    fn test_same_tier_prefers_earlier_in_source_list() {
        // Archive (tier 2) vs Dailymotion (tier 2) — Archive is earlier in default list
        let sources = vec![
            make_source("Making of Iron Man", SourceType::Dailymotion, Some(300)),
            make_source("Making of Iron Man", SourceType::ArchiveOrg, Some(305)),
        ];
        let active = vec![
            Source::Tmdb,
            Source::Archive,
            Source::Dailymotion,
            Source::Youtube,
        ];
        let (result, removed) = deduplicate(sources, &active);
        assert_eq!(result.len(), 1);
        assert_eq!(removed, 1);
        // Archive appears before Dailymotion in active list → Archive wins
        assert_eq!(result[0].source_type, SourceType::ArchiveOrg);
    }

    // 6.5: Similarity below 80% → not a duplicate
    #[test]
    fn test_below_threshold_not_deduped() {
        let sources = vec![
            make_source("Iron Man Trailer", SourceType::TMDB, Some(120)),
            make_source("Thor Ragnarok Trailer", SourceType::YouTube, Some(120)),
        ];
        let (result, removed) = deduplicate(sources, &default_sources());
        assert_eq!(result.len(), 2);
        assert_eq!(removed, 0);
    }

    // 6.6: Title 80–94% similar but duration differs by >10% → not a duplicate
    #[test]
    fn test_duration_outside_tolerance_not_deduped() {
        // "Iron Man Official Trailer" (25 chars) vs "Iron Man Official Trail" (23 chars)
        // Similarity ~92% — firmly in the 80–94% range, never ≥95%
        let t1 = "Iron Man Official Trailer";
        let t2 = "Iron Man Official Trail";
        let similarity = FuzzyMatcher::get_similarity_score(t1, t2);
        assert!(
            similarity >= 80 && similarity < 95,
            "Test precondition: titles must be 80-94% similar, got {}%",
            similarity
        );

        let sources = vec![
            make_source(t1, SourceType::TMDB, Some(120)),
            make_source(t2, SourceType::YouTube, Some(200)), // 67% diff — outside 10%
        ];
        let (result, removed) = deduplicate(sources, &default_sources());
        // Duration outside tolerance → not a duplicate
        assert_eq!(
            result.len(),
            2,
            "Should not dedup when duration outside tolerance"
        );
        assert_eq!(removed, 0);
    }

    // 6.7: None duration → only ≥95% title rule applies
    #[test]
    fn test_none_duration_skips_duration_check() {
        // Identical titles, one has None duration — should still dedup via ≥95% rule
        let sources = vec![
            make_source("Iron Man Trailer", SourceType::TMDB, None),
            make_source("Iron Man Trailer", SourceType::YouTube, Some(120)),
        ];
        let (result, removed) = deduplicate(sources, &default_sources());
        assert_eq!(result.len(), 1);
        assert_eq!(removed, 1);

        // Both None — still dedup via ≥95% rule
        let sources2 = vec![
            make_source("Iron Man Trailer", SourceType::TMDB, None),
            make_source("Iron Man Trailer", SourceType::YouTube, None),
        ];
        let (result2, removed2) = deduplicate(sources2, &default_sources());
        assert_eq!(result2.len(), 1);
        assert_eq!(removed2, 1);

        // 80–94% similar, both None → NOT a duplicate (can't confirm duration match)
        let t1 = "Iron Man Official Trailer";
        let t2 = "Iron Man Official Trail";
        let similarity = FuzzyMatcher::get_similarity_score(t1, t2);
        assert!(
            similarity >= 80 && similarity < 95,
            "Test precondition: titles must be 80-94% similar, got {}%",
            similarity
        );
        let sources3 = vec![
            make_source(t1, SourceType::TMDB, None),
            make_source(t2, SourceType::YouTube, None),
        ];
        let (result3, removed3) = deduplicate(sources3, &default_sources());
        assert_eq!(
            result3.len(),
            2,
            "80-94% similar with None durations should not dedup"
        );
        assert_eq!(removed3, 0);
    }

    // 6.8: Empty input → empty output
    #[test]
    fn test_empty_input_returns_empty() {
        let (result, removed) = deduplicate(vec![], &default_sources());
        assert_eq!(result.len(), 0);
        assert_eq!(removed, 0);
    }

    // 6.9: deduplicate_series applies same logic to SeriesExtra
    #[test]
    fn test_series_dedup_works_same_as_movie() {
        use crate::models::SeriesExtra;

        let extras = vec![
            SeriesExtra {
                series_id: "bb".to_string(),
                season_number: None,
                category: ContentCategory::Trailer,
                title: "Breaking Bad Trailer".to_string(),
                url: "https://tmdb.com/trailer".to_string(),
                source_type: SourceType::TMDB,
                local_path: None,
                duration_secs: Some(120),
            },
            SeriesExtra {
                series_id: "bb".to_string(),
                season_number: None,
                category: ContentCategory::Trailer,
                title: "Breaking Bad Trailer".to_string(),
                url: "https://youtube.com/trailer".to_string(),
                source_type: SourceType::YouTube,
                local_path: None,
                duration_secs: Some(122),
            },
        ];
        let (result, removed) = deduplicate_series(extras, &default_sources());
        assert_eq!(result.len(), 1);
        assert_eq!(removed, 1);
        assert_eq!(result[0].source_type, SourceType::TMDB);
    }

    // 6.10: 3 copies from Tier 1, 2, 3 → only Tier 1 kept, 2 removed
    #[test]
    fn test_multiple_duplicates_across_tiers() {
        let sources = vec![
            make_source("Iron Man Trailer", SourceType::YouTube, Some(120)), // tier 3
            make_source("Iron Man Trailer", SourceType::TMDB, Some(118)),    // tier 1
            make_source("Iron Man Trailer", SourceType::ArchiveOrg, Some(121)), // tier 2
        ];
        let (result, removed) = deduplicate(sources, &default_sources());
        assert_eq!(result.len(), 1);
        assert_eq!(removed, 2);
        assert_eq!(result[0].source_type, SourceType::TMDB);
    }
}
