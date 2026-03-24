# Story 7.1: Duplicate Detection Engine

Status: done

## Story

As a user,
I want duplicate extras detected across sources before downloading,
So that I don't waste bandwidth and storage on the same content from multiple platforms.

## Acceptance Criteria

1. A `duration_secs: Option<u32>` field is added to `VideoSource` to enable duration-based deduplication; discoverers populate this from API metadata where available
2. A new title+duration deduplication phase runs BEFORE the existing URL-based dedup and content limits in `DiscoveryOrchestrator::discover_all()`; URL dedup remains as a final safety net, content limits remain unchanged
3. Two videos are considered duplicates when title similarity ≥ 80% AND duration is within 10% tolerance, OR when title similarity ≥ 95% regardless of duration (to handle re-edits of the same content)
4. When two videos are considered duplicates, the one from the higher-tier source is kept: Tier 1 (TMDB, KinoCheck, TheTVDB) > Tier 2 (Dailymotion, Vimeo, Archive.org) > Tier 3 (YouTube, Bilibili) (FR24)
5. Within the same tier, the first source in the active list wins
6. Deduplication processing adds no more than 100ms overhead per movie regardless of video count (NFR4)
7. The deduplication logic is a standalone module (`src/deduplication.rs`) that receives `Vec<VideoSource>` and returns a deduplicated `Vec<VideoSource>` plus a count of removed duplicates
8. The same deduplication is applied in the series pipeline (`discover_series_content` in `orchestrator.rs`) for `Vec<SeriesExtra>`
9. Season 0 specials (returned separately from `discover_series_content`) are NOT subject to title+duration dedup — they use a different selection mechanism (per-episode best-candidate scoring via `SpecialValidator`)
10. A `duplicates_removed: usize` field is added to `ProcessingSummary` and accumulated from both movie and series pipelines, so Story 7.2 can display it without re-plumbing
11. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Add `duration_secs` field to `VideoSource` and `SeriesExtra` (AC: #1)
  - [x] 1.1 Add `pub duration_secs: Option<u32>` to `VideoSource` in `src/models.rs`
  - [x] 1.2 Add `pub duration_secs: Option<u32>` to `SeriesExtra` in `src/models.rs`
  - [x] 1.3 Update `From<SeriesExtra> for VideoSource` impl to carry `duration_secs` through
  - [x] 1.4 Fix all existing `VideoSource { ... }` and `SeriesExtra { ... }` construction sites across the codebase to include `duration_secs: None` (or a populated value where the API provides it)
  - [x] 1.5 Fix all existing test construction sites to include `duration_secs`

- [x] Task 2: Populate `duration_secs` from discoverer API metadata (AC: #1)
  - [x] 2.1 In `DailymotionDiscoverer::map_video_to_source()` — set `duration_secs: Some(video.duration)` (Dailymotion API returns duration in seconds as `u32`)
  - [x] 2.2 In `YoutubeDiscoverer` — set `duration_secs: Some(duration_secs)` from the yt-dlp JSON `duration` field already parsed for filtering
  - [x] 2.3 In `TmdbDiscoverer` — TMDB video list API does NOT return duration; set `duration_secs: None`
  - [x] 2.4 In `ArchiveOrgDiscoverer` — Archive.org metadata does NOT reliably include duration; set `duration_secs: None`
  - [x] 2.5 In `KinoCheckDiscoverer` — KinoCheck API does NOT return duration; set `duration_secs: None`
  - [x] 2.6 In `SeriesYoutubeDiscoverer` — set `duration_secs` from yt-dlp JSON where available
  - [x] 2.7 In `TmdbSeriesDiscoverer` — set `duration_secs: None`
  - [x] 2.8 In `DailymotionDiscoverer` series path (via `SeriesDiscoveryOrchestrator`) — the `VideoSource→SeriesExtra` conversion in `series_orchestrator.rs` must carry `duration_secs` through

- [x] Task 3: Create `src/deduplication.rs` module (AC: #3, #4, #5, #6, #7)
  - [x] 3.1 Create `src/deduplication.rs` with a `pub(crate)` function: `pub(crate) fn deduplicate(sources: Vec<VideoSource>, active_sources: &[Source]) -> (Vec<VideoSource>, usize)`
  - [x] 3.2 Implement `SourceType::tier()` method on `SourceType` enum in `models.rs` — mirrors `Source::tier()` mapping: TMDB→1, KinoCheck→1, TheTVDB→1, ArchiveOrg→2, Dailymotion→2, Vimeo→2, YouTube→3, Bilibili→3
  - [x] 3.3 Implement `fn source_order(source_type: &SourceType, active_sources: &[Source]) -> usize` — returns the index of the matching `Source` in the active list (for same-tier tiebreaking); unmatched sources get `usize::MAX`
  - [x] 3.4 Implement the dedup algorithm: iterate all pairs, use `FuzzyMatcher::get_similarity_score()` for title comparison, check duration tolerance, mark lower-priority duplicates for removal
  - [x] 3.5 Add a parallel function for series: `pub(crate) fn deduplicate_series(extras: Vec<SeriesExtra>, active_sources: &[Source]) -> (Vec<SeriesExtra>, usize)`
  - [x] 3.6 Add unit tests (see Task 6)

- [x] Task 4: Wire dedup into `DiscoveryOrchestrator::discover_all()` (AC: #2)
  - [x] 4.1 Import `crate::deduplication::deduplicate`
  - [x] 4.2 Insert `deduplicate()` call AFTER source aggregation, BEFORE the existing URL dedup block
  - [x] 4.3 Log the dedup count at `info!` level: `"Removed {} title+duration duplicates for {}"`
  - [x] 4.4 Pass `&self.sources` as the active source list for same-tier tiebreaking
  - [x] 4.5 Return the dedup count alongside the existing return tuple so the orchestrator can accumulate it

- [x] Task 5: Wire dedup into series pipeline (AC: #8, #9)
  - [x] 5.1 In `discover_series_content()` in `src/orchestrator.rs`, insert `deduplicate_series()` call AFTER all extras are collected, BEFORE the existing URL dedup block — apply only to `all_extras`, NOT to `season_zero_extras` (AC #9)
  - [x] 5.2 The series pipeline needs access to the active `sources` list — pass it as a parameter to `discover_series_content()`
  - [x] 5.3 Log the dedup count at `info!` level
  - [x] 5.4 Return the dedup count so the orchestrator can accumulate it

- [x] Task 5b: Add `duplicates_removed` to `ProcessingSummary` (AC: #10)
  - [x] 5b.1 Add `pub duplicates_removed: usize` field to `ProcessingSummary` in `src/orchestrator.rs`
  - [x] 5b.2 Accumulate the dedup count from `discover_all()` return value in `process_movie_standalone`
  - [x] 5b.3 Accumulate the dedup count from `discover_series_content()` return value in `process_series_standalone`
  - [x] 5b.4 Add `duplicates_removed` to `MovieResult` and `SeriesResult` so counts flow through `add_movie_result` / `add_series_result`
  - [x] 5b.5 Update all existing `ProcessingSummary`, `MovieResult`, and `SeriesResult` construction sites in tests to include the new field

- [x] Task 6: Add tests in `src/deduplication.rs` (AC: #3, #4, #5, #6, #7)
  - [x] 6.1 `test_no_duplicates_returns_unchanged` — 3 unique videos → 3 returned, 0 removed
  - [x] 6.2 `test_title_and_duration_match_removes_lower_tier` — two videos with 85% title similarity and duration within 10% → lower tier removed
  - [x] 6.3 `test_high_similarity_ignores_duration` — two videos with 96% title similarity but very different durations → lower tier removed
  - [x] 6.4 `test_same_tier_prefers_earlier_in_source_list` — two Tier 2 videos (Archive vs Dailymotion) → the one whose Source appears first in active list wins
  - [x] 6.5 `test_below_threshold_not_deduped` — two videos with 70% title similarity → both kept
  - [x] 6.6 `test_duration_outside_tolerance_not_deduped` — two videos with 85% title similarity but duration differs by 30% → both kept (unless title ≥ 95%)
  - [x] 6.7 `test_none_duration_skips_duration_check` — when either video has `duration_secs: None`, only the ≥ 95% title-only rule applies
  - [x] 6.8 `test_empty_input_returns_empty` — empty vec → empty vec, 0 removed
  - [x] 6.9 `test_series_dedup_works_same_as_movie` — verify `deduplicate_series` applies same logic to `SeriesExtra`
  - [x] 6.10 `test_multiple_duplicates_across_tiers` — 3 copies of same content from Tier 1, 2, 3 → only Tier 1 kept, 2 removed

- [x] Task 7: Register module and quality gate (AC: #11)
  - [x] 7.1 Add `pub mod deduplication;` to `src/lib.rs`
  - [x] 7.2 `cargo build` — fix any errors
  - [x] 7.3 `cargo test` — fix any failures
  - [x] 7.4 `cargo clippy -- -D warnings` — fix any warnings
  - [x] 7.5 `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### Architecture: Standalone Module

The dedup logic lives in `src/deduplication.rs` as a pure function module — no struct, no state. It receives a `Vec<VideoSource>` and returns a deduplicated `Vec<VideoSource>` plus the count of removed items. This follows SRP and keeps the module independently testable.

### `duration_secs` Field Addition — Compilation Impact

Adding `duration_secs: Option<u32>` to `VideoSource` and `SeriesExtra` will break every existing construction site. There are many of these across the codebase. Use `duration_secs: None` for all existing sites initially, then populate from API metadata in Task 2.

Key files with `VideoSource` construction:
- `src/discovery/tmdb.rs` — `VideoSource { ... }` in `discover()` and collection video mapping
- `src/discovery/archive.rs` — `VideoSource { ... }` in `map_result_to_source()`
- `src/discovery/youtube.rs` — `VideoSource { ... }` in `process_search_result()`
- `src/discovery/kinocheck.rs` — `VideoSource { ... }` in `discover_for_tmdb_id()`
- `src/discovery/dailymotion.rs` — `VideoSource { ... }` in `map_video_to_source()`
- `src/discovery/series_tmdb.rs` — `SeriesExtra { ... }` in multiple methods
- `src/discovery/series_youtube.rs` — `SeriesExtra { ... }` in multiple methods
- `src/discovery/season_pack.rs` — `SeriesExtra { ... }`
- `src/discovery/season_zero_import.rs` — `SeriesExtra { ... }`
- `src/discovery/special_validator.rs` — `SelectedSpecial` (does not use `VideoSource` directly)
- `src/discovery/series_orchestrator.rs` — `VideoSource→SeriesExtra` conversion closures
- `src/orchestrator.rs` — test helpers
- `src/organizer.rs` — test helpers
- `src/downloader.rs` — test helpers
- `src/converter.rs` — test helpers
- `src/output.rs` — test helpers
- `tests/main_integration_tests.rs` — test helpers
- `tests/series_integration_tests.rs` — test helpers

Use `grep -rn "VideoSource {" src/ tests/` and `grep -rn "SeriesExtra {" src/ tests/` to find all sites. This is the most tedious part of the story — be thorough.

### Dedup Algorithm

The algorithm is O(n²) pairwise comparison. For typical discovery results (10–50 videos per movie), this is well within the 100ms NFR4 budget. No optimization needed.

```rust
pub(crate) fn deduplicate(
    mut sources: Vec<VideoSource>,
    active_sources: &[Source],
) -> (Vec<VideoSource>, usize) {
    let mut to_remove: HashSet<usize> = HashSet::new();

    for i in 0..sources.len() {
        if to_remove.contains(&i) { continue; }
        for j in (i + 1)..sources.len() {
            if to_remove.contains(&j) { continue; }

            let similarity = FuzzyMatcher::get_similarity_score(
                &sources[i].title, &sources[j].title
            );

            let is_duplicate = if similarity >= 95 {
                // Very high title similarity — treat as duplicate regardless of duration
                true
            } else if similarity >= 80 {
                // Moderate similarity — require duration match
                match (sources[i].duration_secs, sources[j].duration_secs) {
                    (Some(d1), Some(d2)) => {
                        let max_d = d1.max(d2) as f64;
                        let diff = (d1 as f64 - d2 as f64).abs();
                        max_d > 0.0 && (diff / max_d) <= 0.10
                    }
                    // If either is None, can't confirm duration match — not a duplicate
                    _ => false,
                }
            } else {
                false
            };

            if is_duplicate {
                // Keep the higher-priority source; remove the other
                let loser = pick_loser(i, j, &sources, active_sources);
                to_remove.insert(loser);
            }
        }
    }

    let removed = to_remove.len();
    let deduped: Vec<VideoSource> = sources
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !to_remove.contains(idx))
        .map(|(_, vs)| vs)
        .collect();

    (deduped, removed)
}
```

### Tier Resolution: `pick_loser`

```rust
fn pick_loser(
    i: usize, j: usize,
    sources: &[VideoSource],
    active_sources: &[Source],
) -> usize {
    let tier_i = sources[i].source_type.tier();
    let tier_j = sources[j].source_type.tier();

    if tier_i != tier_j {
        // Lower tier number = higher priority; loser has higher tier number
        if tier_i < tier_j { j } else { i }
    } else {
        // Same tier — prefer the one whose Source appears earlier in active list
        let order_i = source_order(&sources[i].source_type, active_sources);
        let order_j = source_order(&sources[j].source_type, active_sources);
        if order_i <= order_j { j } else { i }
    }
}
```

### `SourceType::tier()` Method

Add to `SourceType` in `src/models.rs`, mirroring `Source::tier()`:

```rust
impl SourceType {
    pub fn tier(&self) -> u8 {
        match self {
            SourceType::TMDB => 1,
            SourceType::KinoCheck => 1,
            SourceType::TheTVDB => 1,
            SourceType::ArchiveOrg => 2,
            SourceType::Dailymotion => 2,
            SourceType::Vimeo => 2,
            SourceType::YouTube => 3,
            SourceType::Bilibili => 3,
        }
    }
}
```

### `source_order` Helper

Maps `SourceType` back to `Source` for position lookup in the active list:

```rust
fn source_type_to_source(st: &SourceType) -> Option<Source> {
    match st {
        SourceType::TMDB => Some(Source::Tmdb),
        SourceType::ArchiveOrg => Some(Source::Archive),
        SourceType::YouTube => Some(Source::Youtube),
        SourceType::Dailymotion => Some(Source::Dailymotion),
        SourceType::Vimeo => Some(Source::Vimeo),
        SourceType::Bilibili => Some(Source::Bilibili),
        // KinoCheck and TheTVDB are not user-selectable sources
        SourceType::KinoCheck | SourceType::TheTVDB => None,
    }
}

fn source_order(source_type: &SourceType, active_sources: &[Source]) -> usize {
    source_type_to_source(source_type)
        .and_then(|s| active_sources.iter().position(|a| *a == s))
        .unwrap_or(usize::MAX)
}
```

### Series Dedup: `deduplicate_series`

`SeriesExtra` has the same `title`, `source_type`, and (new) `duration_secs` fields. The dedup logic is identical — extract the comparison into a shared helper or duplicate the function with `SeriesExtra` types. Given Rust's lack of structural typing, the cleanest approach is a parallel function that operates on `SeriesExtra` directly, using the same `pick_loser` logic adapted for `SeriesExtra` fields.

### Insertion Point in Movie Pipeline

In `DiscoveryOrchestrator::discover_all()` (line ~270 of `src/discovery/orchestrator.rs`), the current flow is:

```
1. Aggregate all_sources from TMDB, KinoCheck, Archive, YouTube, Dailymotion
2. URL dedup (sort + dedup_by URL)
3. apply_content_limits()
4. Return
```

After this story:

```
1. Aggregate all_sources from TMDB, KinoCheck, Archive, YouTube, Dailymotion
2. NEW: Title+duration dedup via deduplicate()
3. URL dedup (sort + dedup_by URL) — safety net
4. apply_content_limits()
5. Return
```

### Insertion Point in Series Pipeline

In `discover_series_content()` (line ~889 of `src/orchestrator.rs`), the current flow is:

```
1. Series-level extras
2. Season-specific extras (if enabled)
3. Season 0 specials (if enabled)
4. URL dedup (HashSet)
```

After this story:

```
1. Series-level extras
2. Season-specific extras (if enabled)
3. Season 0 specials (if enabled)
4. NEW: Title+duration dedup via deduplicate_series()
5. URL dedup (HashSet) — safety net
```

The `discover_series_content` function currently doesn't have access to the active `sources` list. Add it as a parameter: `active_sources: &[Source]`. Update the call site in `process_series_standalone` to pass `&self.sources` (from `SeriesConfig`).

### Transitive Duplicate Chains

The O(n²) algorithm is greedy: if A≈B and B≈C but A≉C, B is removed as a dup of A, and C survives because it's only compared against the remaining items. This is correct and expected — transitive-closure dedup would be overly aggressive for fuzzy matching. The greedy approach is conservative: it only removes items with a direct match to a higher-priority survivor.

### `ProcessingSummary` Plumbing

The dedup count must flow from `discover_all()` → `process_movie_standalone()` → `MovieResult` → `ProcessingSummary`. The cleanest approach:

1. Change `discover_all()` return type from `(Vec<VideoSource>, Vec<SourceResult>)` to `(Vec<VideoSource>, Vec<SourceResult>, usize)` where the third element is `duplicates_removed`
2. Add `duplicates_removed: usize` to `MovieResult` and `SeriesResult`
3. Add `duplicates_removed: usize` to `ProcessingSummary`, accumulated in `add_movie_result` / `add_series_result`

This sets up Story 7.2 to simply read `summary.duplicates_removed` in `display_summary()` without any plumbing changes.

### Discoverers That Provide Duration

| Discoverer | Duration Available | Notes |
|---|---|---|
| Dailymotion | Yes (`video.duration` in seconds) | Already parsed for filtering |
| YouTube | Yes (`duration` from yt-dlp JSON) | Already parsed for filtering |
| TMDB | No | Video list API has no duration field |
| Archive.org | No | Metadata unreliable for duration |
| KinoCheck | No | Returns YouTube URL only, no metadata |
| TheTVDB | No | Episode metadata, not video duration |
| Series YouTube | Yes | Same yt-dlp JSON parsing |
| Series TMDB | No | Same as movie TMDB |

When both videos have `duration_secs: None`, only the ≥ 95% title-only rule applies. This means TMDB-vs-TMDB duplicates (e.g., from collection sibling fetching) can only be caught by near-exact title match or URL dedup.

### What NOT To Do

- Do NOT modify `apply_content_limits()` — it stays as-is after the dedup phase. Its internal priority mapping (TMDB→0, ArchiveOrg→1, etc.) is intentionally different from `SourceType::tier()` — content limits use a finer-grained ordering for truncation, while dedup uses the 3-tier system from the PRD. Do NOT "unify" these.
- Do NOT remove the existing URL dedup — it remains as a safety net after title+duration dedup
- Do NOT add dedup to the `SpecialValidator` pipeline — Season 0 specials use a different selection mechanism (best-candidate scoring per episode); they are returned separately from `discover_series_content` and bypass the dedup call naturally
- Do NOT use `HashMap` for O(1) dedup — title similarity requires pairwise comparison; there's no hash-based shortcut for fuzzy matching
- Do NOT add a `DeduplicationError` to `error.rs` — dedup is infallible (it's a pure filter on in-memory data)
- Do NOT log individual duplicate removals at `info!` level — log the total count only; use `debug!` for individual pairs to avoid noisy output
- Do NOT copy-paste the full dedup algorithm for `deduplicate_series` — extract the core comparison logic (`is_duplicate` check and `pick_loser` resolution) into shared private helpers that both `deduplicate` and `deduplicate_series` call, passing title/duration/source_type via arguments or a small trait. This avoids violating DRY per the code quality steering doc.

### Test Count Baseline

584 tests were passing after Story 6.1. This story adds ~10 new tests. Many existing tests will need `duration_secs` field additions but should otherwise pass unchanged.

### Key Code Locations

| What | File | Notes |
|---|---|---|
| New dedup module | `src/deduplication.rs` | Create new file |
| Module registration | `src/lib.rs` | Add `pub mod deduplication;` |
| VideoSource struct | `src/models.rs` | Add `duration_secs: Option<u32>` |
| SeriesExtra struct | `src/models.rs` | Add `duration_secs: Option<u32>` |
| SourceType tier | `src/models.rs` | Add `tier()` method to `SourceType` |
| Movie orchestrator | `src/discovery/orchestrator.rs` | Insert dedup call in `discover_all()`, update return type |
| Series pipeline | `src/orchestrator.rs` | Insert dedup call in `discover_series_content()`, add `duplicates_removed` to `ProcessingSummary`, `MovieResult`, `SeriesResult` |
| FuzzyMatcher | `src/discovery/fuzzy_matching.rs` | Reuse `get_similarity_score()` |
| Dailymotion discoverer | `src/discovery/dailymotion.rs` | Populate `duration_secs` |
| YouTube discoverer | `src/discovery/youtube.rs` | Populate `duration_secs` |
| Series YouTube | `src/discovery/series_youtube.rs` | Populate `duration_secs` |
| Series orchestrator | `src/discovery/series_orchestrator.rs` | Carry `duration_secs` in conversion closures |

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 7, Story 7.1]
- [Source: _bmad-output/planning-artifacts/prd.md — FR23, FR24, NFR4]
- [Source: src/models.rs — VideoSource (line 32), SeriesExtra (line 356), Source::tier() (line 159), SourceType (line 197)]
- [Source: src/discovery/orchestrator.rs — discover_all() URL dedup block (line ~270), apply_content_limits() (line 291)]
- [Source: src/orchestrator.rs — discover_series_content() URL dedup block (line ~940), ProcessingSummary (line 44)]
- [Source: src/discovery/fuzzy_matching.rs — FuzzyMatcher::get_similarity_score()]
- [Source: src/discovery/dailymotion.rs — map_video_to_source() already has video.duration]
- [Source: src/discovery/youtube.rs — duration_secs already parsed for filtering]
- [Source: _bmad-output/implementation-artifacts/6-1-dailymotion-rest-api-discoverer.md — Story 6.1 patterns]

### Project Structure Notes

- New file: `src/deduplication.rs` — standalone module, no submodules
- `src/lib.rs` — add `pub mod deduplication;`
- No new dependencies — uses existing `FuzzyMatcher` and `log` crate

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.6

### Debug Log References

- `cargo check` revealed duplicate code in `archive.rs` after initial strReplace — fixed by removing the orphaned lines
- `FuzzyMatcher` import path needed to be `crate::discovery::FuzzyMatcher` (public re-export), not `crate::discovery::fuzzy_matching::FuzzyMatcher` (private module)
- `MovieResult::success` and `SeriesResult::success` marked `#[cfg(test)]` to satisfy clippy dead_code lint (they're only used in test blocks)

### Completion Notes List

- New `src/deduplication.rs` module with `deduplicate()` and `deduplicate_series()` sharing core logic via `find_duplicate_indices()` closure-based helper
- `duration_secs: Option<u32>` added to `VideoSource` and `SeriesExtra`; populated from Dailymotion and YouTube APIs, `None` for TMDB/Archive/KinoCheck
- `SourceType::tier()` method added to `models.rs` mirroring `Source::tier()`
- `discover_all()` return type extended to 3-tuple `(Vec<VideoSource>, Vec<SourceResult>, usize)`
- `discover_series_content()` accepts `active_sources: &[Source]` and returns 5-tuple including dedup count
- `ProcessingSummary.duplicates_removed`, `MovieResult.duplicates_removed`, `SeriesResult.duplicates_removed` added and wired through the pipeline
- 595 tests passing (546 lib + 15 main integration + 34 series integration), 0 failures
- `cargo clippy -- -D warnings` clean, `cargo fmt -- --check` clean

### Review Findings

- [x] [Review][Patch] Unnecessary `let mut` rebinding after dedup call — used `let (mut all_sources, ...)` directly [`src/discovery/orchestrator.rs:250`, `src/orchestrator.rs:1032`]
- [x] [Review][Patch] `test_duration_outside_tolerance_not_deduped` and `test_none_duration_skips_duration_check` had conditional assertions that could silently pass — replaced with titles definitively in 80–94% range and unconditional assertions [`src/deduplication.rs`]
- [x] [Review][Patch] `SourceType::tier()` and `Source::tier()` can drift silently — added `test_source_type_tier_matches_source_tier` consistency test [`src/models.rs`]
- [x] [Review][Defer] `sources` field on `Orchestrator` duplicates data already held inside `DiscoveryOrchestrator` — pre-existing architectural pattern, series pipeline doesn't have access to `DiscoveryOrchestrator` internals

### File List

- `src/deduplication.rs` — new file, dedup module with 10 unit tests
- `src/lib.rs` — added `pub mod deduplication;`
- `src/models.rs` — added `duration_secs` to `VideoSource` and `SeriesExtra`, `SourceType::tier()` method, updated `From<SeriesExtra> for VideoSource`, fixed test constructions
- `src/discovery/archive.rs` — added `duration_secs: None` to `VideoSource` construction
- `src/discovery/dailymotion.rs` — added `duration_secs: Some(video.duration)` to `VideoSource` construction
- `src/discovery/youtube.rs` — added `duration_secs: Some(duration)` to `VideoSource` construction
- `src/discovery/tmdb.rs` — added `duration_secs: None` to both `VideoSource` constructions
- `src/discovery/kinocheck.rs` — added `duration_secs: None` to `VideoSource` constructions (production + test)
- `src/discovery/series_tmdb.rs` — added `duration_secs: None` to `SeriesExtra` constructions (production + property tests)
- `src/discovery/series_youtube.rs` — added `duration_secs: Some(duration)` to `SeriesExtra` construction
- `src/discovery/series_orchestrator.rs` — added `duration_secs: vs.duration_secs` in Dailymotion conversion, `duration_secs: None` in specials constructions, fixed test constructions
- `src/discovery/orchestrator.rs` — updated `discover_all()` return type to 3-tuple, inserted dedup call before URL dedup
- `src/orchestrator.rs` — added `sources` field to `Orchestrator` and `active_sources` to `SeriesProcessingContext`, updated `discover_series_content()` signature and return type, added `duplicates_removed` to `ProcessingSummary`/`MovieResult`/`SeriesResult`, updated all call sites
- `src/converter.rs` — added `duration_secs: None` to all `VideoSource` test constructions
- `src/downloader.rs` — added `duration_secs: None` to `VideoSource` test construction
- `src/output.rs` — added `duplicates_removed: 0` to all `ProcessingSummary` test constructions
- `tests/series_integration_tests.rs` — added `duration_secs: None` to `SeriesExtra` test constructions
