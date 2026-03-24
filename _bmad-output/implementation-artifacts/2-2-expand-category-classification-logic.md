# Story 2.2: Expand Category Classification Logic

Status: done

## Story

As a user,
I want discovered content automatically classified into the correct category using title keywords and source metadata,
So that extras land in the right Jellyfin subfolder without manual intervention.

## Acceptance Criteria

1. TMDB type mappings extended in `map_tmdb_type()` in `src/discovery/tmdb.rs` (FR21):
   - `"Interview"` → `ContentCategory::Interview`
   - `"Short"` → `ContentCategory::Short`
   - `"Clip"` → `ContentCategory::Clip` (replaces the current `Featurette` mapping for `"Clip"`)
2. `"Bloopers"` TMDB type continues to map to `Featurette` — no change (FR22)
3. `infer_category_from_title()` in `src/discovery/title_matching.rs` updated (FR21):
   - Titles containing `"short film"`, `"animated short"`, `"short"` (standalone) → `ContentCategory::Short`
   - Titles containing `"movie clip"`, `"clip -"`, `"| clip"` → `ContentCategory::Clip` (replaces current `Featurette` fallback)
   - Titles containing `"interview"`, `"q&a"`, `"press conference"`, `"talks about"`, `"podcast"` → `ContentCategory::Interview` (already exists — verify no regression)
4. Content that cannot be mapped to any of the 8 defined categories falls back to `ContentCategory::Extras` (FR39):
   - `infer_category_from_title()` returns `None` for unrecognized titles (unchanged behavior)
   - The caller (YouTube discoverer and series YouTube discoverer) assigns `ContentCategory::Extras` when `infer_category_from_title()` returns `None` AND no search-query category is available
   - TMDB `map_tmdb_type()` returns `None` for unknown types — the TMDB discoverer already skips those videos; no change to that behavior
5. Classification is case-insensitive (already enforced via `lower = title.to_lowercase()` — verify)
6. Classification logic is shared across all discoverers via `title_matching.rs` — no per-discoverer classification code
7. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Extend `map_tmdb_type()` in `src/discovery/tmdb.rs` (AC: #1, #2)
  - [x] 1.1 Add `"Interview"` → `Some(ContentCategory::Interview)` arm
  - [x] 1.2 Add `"Short"` → `Some(ContentCategory::Short)` arm
  - [x] 1.3 Change `"Clip"` arm from `Some(ContentCategory::Featurette)` to `Some(ContentCategory::Clip)`
  - [x] 1.4 Verify `"Bloopers"` → `Some(ContentCategory::Featurette)` is unchanged
  - [x] 1.5 Update the existing `test_map_tmdb_type` unit test in `src/discovery/tmdb.rs` to assert the 3 changed/new mappings

- [x] Task 2: Update `infer_category_from_title()` in `src/discovery/title_matching.rs` (AC: #3, #5)
  - [x] 2.1 Add `Short` detection block (before the `Featurette` block, after `Interview`):
    ```rust
    if lower.contains("short film")
        || lower.contains("animated short")
        || lower.contains("short film")
        || (lower.contains(" short") && lower.split_whitespace().any(|w| w == "short"))
    {
        return Some(ContentCategory::Short);
    }
    ```
    Use word-boundary matching for the bare `"short"` case to avoid false positives from `"shortcut"`, `"short-listed"`, `"shortly"`, `"shortage"`. The safest approach: only match bare `"short"` when it appears as a standalone word (surrounded by spaces or at string boundaries). The `split_whitespace().any(|w| w == "short")` check handles this correctly. `"short film"` and `"animated short"` are explicit compound phrases and don't need the word-boundary guard.
  - [x] 2.2 Replace the existing `Clip` detection block (currently returns `Featurette`) with:
    ```rust
    if lower.contains("movie clip") || lower.contains("clip -") || lower.contains("| clip") {
        return Some(ContentCategory::Clip);
    }
    ```
  - [x] 2.3 Verify the `Interview` detection block is unchanged and still covers `"interview"`, `"q&a"`, `"q & a"`, `"press conference"`, `"talks about"`, `"podcast"`
  - [x] 2.4 No change needed to `test_infer_category_from_title_featurette` — the existing test only asserts `"Cobra (1986) - Bonus Clip: Actor Brian Thompson"` → `Featurette`, which remains correct because `"bonus clip"` matches the `Featurette` block, not the `Clip` block. Do NOT modify this test.
  - [x] 2.5 Add `test_infer_category_from_title_short` test covering `"short film"`, `"animated short"`, and bare `"short"` keyword cases; also verify `"shortcut"` and `"shortly"` do NOT return `Short`
  - [x] 2.6 Add `test_infer_category_from_title_clip` test covering `"movie clip"`, `"clip -"`, `"| clip"` cases

- [x] Task 3: Extend `map_tmdb_type()` in `src/discovery/series_tmdb.rs` (AC: #1, #2)
  - [x] 3.1 Add `"Interview"` → `Some(ContentCategory::Interview)` arm
  - [x] 3.2 Add `"Short"` → `Some(ContentCategory::Short)` arm
  - [x] 3.3 Change `"Clip"` arm from `Some(ContentCategory::Featurette)` to `Some(ContentCategory::Clip)`
  - [x] 3.4 Verify `"Bloopers"` → `Some(ContentCategory::Featurette)` is unchanged (FR22)
  - [x] 3.5 Note: `"Deleted Scene"` intentionally returns `None` for series (series don't have deleted scenes in TMDB) — do NOT add it
  - [x] 3.6 Fix pre-existing gap: add `"Teaser"` → `Some(ContentCategory::Trailer)` arm — it exists in `tmdb.rs` but is missing from `series_tmdb.rs`, causing teasers to fall through to `None` for series. Since we're already touching this function, fix it now.
  - [x] 3.7 Update the proptest in `series_tmdb.rs` that lists known TMDB types — add `"Interview"`, `"Short"`, `"Teaser"` to the known types list; the proptest only checks `is_some()` (not specific category values), so no category-specific assertion update is needed. The `"Clip"` type is already in the known list and will now return `Clip` instead of `Featurette` — since the proptest only checks `is_some()`, no change needed for `"Clip"` in the proptest.
  - [x] 3.8 Update individual `test_map_tmdb_type_*` tests in `series_tmdb.rs` to cover the 3 new/changed mappings (`"Interview"` → `Interview`, `"Short"` → `Short`, `"Clip"` → `Clip`) and add `test_map_tmdb_type_teaser` for the pre-existing gap fix

- [x] Task 4: Assign `ContentCategory::Extras` as fallback in `youtube.rs` (AC: #4, #6)
  - [x] 4.1 In `src/discovery/youtube.rs` — the current pattern is `infer_category_from_title(&title).unwrap_or(category)` where `category` is the search-query category; this is already correct since every YouTube search query has an associated category. No change needed here — the `Extras` fallback is a future-proofing concern for discoverers without per-query categories (e.g., Dailymotion in Epic 6). Confirm the pattern and leave it unchanged.
  - [x] 4.2 In `src/discovery/series_youtube.rs` — `search_youtube()` receives `category: ContentCategory` as a parameter and assigns it directly to `SeriesExtra.category` without calling `infer_category_from_title()`. Add `infer_category_from_title()` inference before the push, using the search-query `category` as fallback:
    ```rust
    let resolved_category = title_matching::infer_category_from_title(&title)
        .unwrap_or(category);
    // then use resolved_category in SeriesExtra { ... }
    ```
  - [x] 4.3 Do NOT change TMDB discoverer behavior — `map_tmdb_type()` returning `None` already causes the video to be skipped (correct: TMDB unknowns are excluded, not placed in `/extras`)
  - [x] 4.4 Do NOT change Archive.org discoverer behavior — it has its own `infer_category_from_text()` and already skips unrecognized content

- [x] Task 5: Tests (AC: #7)
  - [x] 5.1 Update `test_map_tmdb_type` tests in `src/discovery/tmdb.rs` to cover `"Interview"` → `Interview`, `"Short"` → `Short`, `"Clip"` → `Clip`
  - [x] 5.2 Update `test_map_tmdb_type` tests in `src/discovery/series_tmdb.rs` to cover `"Interview"` → `Interview`, `"Short"` → `Short`, `"Clip"` → `Clip`, `"Teaser"` → `Trailer`
  - [x] 5.3 Add `test_infer_category_from_title_short` in `src/discovery/title_matching.rs` — assert `"short film"` → `Short`, `"animated short"` → `Short`, `"a short"` (bare word) → `Short`; assert `"shortcut to hollywood"` → `None`, `"shortly after"` → `None`
  - [x] 5.4 Add `test_infer_category_from_title_clip` in `src/discovery/title_matching.rs`
  - [x] 5.5 `test_infer_category_from_title_featurette` does NOT need updating — the `"bonus clip"` assertion stays `Featurette` (correct), and there is no `"movie clip"` → `Featurette` assertion in that test to remove. Do not touch it.
  - [x] 5.6 Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`

## Dev Notes

### What Already Exists (Do NOT Redo)

- `ContentCategory` enum in `src/models.rs` already has all 9 variants including `Short`, `Clip`, `Scene`, `Extras` — added in Story 2.1
- `infer_category_from_title()` in `src/discovery/title_matching.rs` already handles `Interview`, `Trailer`, `BehindTheScenes`, `DeletedScene`, `Featurette` — only add `Short` and update `Clip`
- `map_tmdb_type()` in `src/discovery/tmdb.rs` already handles `Trailer`, `Teaser`, `Behind the Scenes`, `Deleted Scene`, `Featurette`, `Clip` (→ Featurette), `Bloopers` (→ Featurette) — only add `Interview`, `Short`, change `Clip`
- The `Interview` detection in `infer_category_from_title()` already returns `ContentCategory::Interview` — no change needed there
- `contains_excluded_keywords()` already filters `"Blooper"` and `"Gag"` keywords — these are excluded before classification, so `Bloopers` TMDB type is only reached via the TMDB API response, not YouTube title matching

### Key Code Locations

| What | File | Line | Current State |
|---|---|---|---|
| `map_tmdb_type()` (movie) | `src/discovery/tmdb.rs` | 262 | 7 arms; `"Clip"` → `Featurette`; no `"Interview"` or `"Short"` |
| `map_tmdb_type()` (series) | `src/discovery/series_tmdb.rs` | 255 | 6 arms; `"Clip"` → `Featurette`; no `"Interview"`, `"Short"`, or `"Deleted Scene"` |
| `test_map_tmdb_type` (movie) | `src/discovery/tmdb.rs` | ~290 | Tests existing 7 mappings |
| `test_map_tmdb_type_*` (series) | `src/discovery/series_tmdb.rs` | ~276 | Individual tests per type; proptest at ~385 lists known types |
| `infer_category_from_title()` | `src/discovery/title_matching.rs` | ~100 | Returns `Featurette` for clip patterns; no `Short` arm |
| `test_infer_category_from_title_featurette` | `src/discovery/title_matching.rs` | ~576 | Asserts `"Cobra (1986) - Bonus Clip: ..."` → `Featurette` (this stays); does NOT assert `"movie clip"` → `Featurette` (no change needed here) |
| YouTube category assignment | `src/discovery/youtube.rs` | 217 | `infer_category_from_title(&title).unwrap_or(category)` — already correct, no change |
| Series YouTube category assignment | `src/discovery/series_youtube.rs` | ~230 | Assigns `category` directly to `SeriesExtra.category` — does NOT call `infer_category_from_title()` — MUST be updated |

### `map_tmdb_type()` After This Story — Both Movie and Series Discoverers

Both `src/discovery/tmdb.rs` and `src/discovery/series_tmdb.rs` have their own `map_tmdb_type()` implementations. Both must be updated.

Movie discoverer (`tmdb.rs`) after this story:
```rust
pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
    match tmdb_type {
        "Trailer" => Some(ContentCategory::Trailer),
        "Teaser" => Some(ContentCategory::Trailer),
        "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
        "Deleted Scene" => Some(ContentCategory::DeletedScene),
        "Featurette" => Some(ContentCategory::Featurette),
        "Bloopers" => Some(ContentCategory::Featurette),  // FR22: preserved
        "Interview" => Some(ContentCategory::Interview),   // NEW
        "Short" => Some(ContentCategory::Short),           // NEW
        "Clip" => Some(ContentCategory::Clip),             // CHANGED from Featurette
        _ => {
            debug!("Unknown TMDB video type: {}", tmdb_type);
            None
        }
    }
}
```

Series discoverer (`series_tmdb.rs`) after this story:
```rust
pub fn map_tmdb_type(tmdb_type: &str) -> Option<ContentCategory> {
    match tmdb_type {
        "Trailer" => Some(ContentCategory::Trailer),
        "Teaser" => Some(ContentCategory::Trailer),
        "Behind the Scenes" => Some(ContentCategory::BehindTheScenes),
        "Featurette" => Some(ContentCategory::Featurette),
        "Bloopers" => Some(ContentCategory::Featurette),  // FR22: preserved
        "Interview" => Some(ContentCategory::Interview),   // NEW
        "Short" => Some(ContentCategory::Short),           // NEW
        "Clip" => Some(ContentCategory::Clip),             // CHANGED from Featurette
        // Note: "Deleted Scene" intentionally absent for series — TMDB doesn't provide deleted scenes for TV
        _ => {
            debug!("Unknown TMDB video type: {}", tmdb_type);
            None
        }
    }
}
```

### `series_tmdb.rs` Proptest Update

`series_tmdb.rs` has a proptest (around line 385) that lists known TMDB types in a `prop_oneof!` and asserts they all return `Some` with specific category values. Add `"Interview"`, `"Short"`, and `"Teaser"` to the `prop_oneof!` list. The proptest checks specific category values per type — add the corresponding `match` arms:
- `"Interview"` → `prop_assert_eq!(result, Some(ContentCategory::Interview))`
- `"Short"` → `prop_assert_eq!(result, Some(ContentCategory::Short))`
- `"Teaser"` → `prop_assert_eq!(result, Some(ContentCategory::Trailer))`
- `"Clip"` arm already exists asserting `Featurette` — update to `Clip`

### `series_tmdb.rs` — Pre-existing `"Teaser"` Gap

`series_tmdb.rs` `map_tmdb_type()` is missing `"Teaser"` → `Trailer`, which exists in `tmdb.rs`. Since we're already modifying this function, fix it in Task 3.6. This is a one-line addition with no risk.

### `series_youtube.rs` — Missing `infer_category_from_title()` Call

This is the most important behavioral fix in this story. `search_youtube()` in `series_youtube.rs` receives `category: ContentCategory` as a parameter and assigns it directly to `SeriesExtra.category` without calling `infer_category_from_title()`. This means a series YouTube search for "behind the scenes" content that returns a video titled "Cast Interview" would be incorrectly categorized as `BehindTheScenes` instead of `Interview`.

The fix is to add title inference before the push:
```rust
// Before (current):
sources.push(SeriesExtra {
    series_id: series_title.to_lowercase().replace(' ', "_"),
    season_number: final_season,
    category,          // ← always the search-query category
    title: title.clone(),
    url,
    source_type: SourceType::YouTube,
    local_path: None,
});

// After (fixed):
let resolved_category = title_matching::infer_category_from_title(&title)
    .unwrap_or(category);
sources.push(SeriesExtra {
    series_id: series_title.to_lowercase().replace(' ', "_"),
    season_number: final_season,
    category: resolved_category,   // ← title-inferred, with search-query as fallback
    title: title.clone(),
    url,
    source_type: SourceType::YouTube,
    local_path: None,
});
```

`title_matching` is already imported in `series_youtube.rs` (it's used for `extract_season_numbers()` and `should_include_video()`). No new import needed.

### `infer_category_from_title()` Short Detection — Word Boundary Required

The bare `"short"` keyword is too broad for substring matching. `"shortcut"`, `"short-listed"`, `"shortly"`, `"shortage"` would all false-positive. Use word-boundary logic for the bare case:

```rust
if lower.contains("short film")
    || lower.contains("animated short")
    || lower.split_whitespace().any(|w| w == "short")
{
    return Some(ContentCategory::Short);
}
```

`"short film"` and `"animated short"` are explicit compound phrases — no false-positive risk. The `split_whitespace().any(|w| w == "short")` check matches only when `"short"` is a standalone word (e.g., `"a short"`, `"short:"`, `"short -"`). It will NOT match `"shortcut"`, `"shortly"`, `"shortage"`, or `"short-listed"` because those contain `"short"` as a prefix, not a standalone word.

The `Short` block must be placed AFTER the `Interview` block and BEFORE the `Featurette` block in `infer_category_from_title()` to maintain correct priority ordering.

### `Extras` Fallback in YouTube Discoverers

`youtube.rs` already calls `infer_category_from_title(&title).unwrap_or(category)` at line 217 — this is correct. Every YouTube search query has an associated `category`, so `Extras` will never be assigned here in practice. No change needed.

`series_youtube.rs` does NOT call `infer_category_from_title()` — it assigns the search-query `category` directly. Task 4.2 fixes this. After the fix, the fallback chain is: title inference → search-query category. `Extras` is not explicitly assigned here either, since every series YouTube query also has a category. The fix is about correctness (title-based inference), not about `Extras` assignment.

The `Extras` catch-all will be most relevant for future discoverers (Dailymotion, Epic 6) that may surface content without a pre-assigned category. The infrastructure is already in place via `ContentCategory::Extras` (Story 2.1) and `infer_category_from_title()` returning `None` for unrecognized titles.

### TMDB Discoverer: `None` Means Skip, Not `Extras`

When `map_tmdb_type()` returns `None` for an unknown TMDB video type, the TMDB discoverer skips that video entirely. This is intentional — TMDB videos with unknown types are not placed in `/extras`. Do NOT change this behavior.

### Archive.org `infer_category_from_text()` — Known Gap, Deferred

`archive.rs` has its own private `infer_category_from_text()` that does NOT use `title_matching.rs`. It currently handles `BehindTheScenes`, `DeletedScene`, `Interview`, `Trailer`, `Featurette` — but has no `Short` or `Clip` detection. This is inconsistent with FR21 and the architecture's goal of shared classification logic.

This story does NOT fix `archive.rs` — that's a larger refactor (replacing the private method with the shared `title_matching::infer_category_from_title()` call, which has different signature and behavior). Deferring to a future cleanup story. The gap is acceptable for MVP since Archive.org primarily surfaces DVD extras (featurettes, behind-the-scenes) — `Short` and `Clip` content from Archive.org is rare.

AC #6 is scoped to YouTube and TMDB discoverers only. Archive.org is explicitly out of scope.

### Regression Risk: `test_infer_category_from_title_featurette`

The existing test at line ~576 asserts:
```rust
assert_eq!(
    infer_category_from_title("Coach Carter Documentary - the real coach carter"),
    Some(ContentCategory::Featurette)
);
assert_eq!(
    infer_category_from_title("Cobra (1986) - Bonus Clip: Actor Brian Thompson"),
    Some(ContentCategory::Featurette)
);
```

After this story, `"bonus clip"` still matches the `Featurette` block (via `lower.contains("bonus clip")`). The `Clip` detection only triggers for `"movie clip"`, `"clip -"`, and `"| clip"` — not `"bonus clip"`. Both assertions remain correct and do NOT need to change.

The current test does NOT assert `"movie clip"` → `Featurette`, so there is no assertion to update in this test. The new `test_infer_category_from_title_clip` test (Task 5.4) will assert `"movie clip"` → `Clip`.

### Regression Risk: `series_tmdb.rs` Proptest

The proptest around line 385 in `series_tmdb.rs` has a `known_types` list. After adding `"Interview"` and `"Short"`, the proptest will also generate those types and assert they return `Some`. The `"Clip"` type is in the known list — its expected category changes from `Featurette` to `Clip`. Check if the proptest asserts specific category values or just `is_some()`. If it only checks `is_some()`, no category-specific update is needed there. If it checks specific values, update `"Clip"` → `Clip`.

### `"Clip"` TMDB Type: Behavior Change

Currently `"Clip"` TMDB type maps to `Featurette` in both movie and series discoverers. After this story it maps to `Clip`. Previously-downloaded TMDB clips placed in `/featurettes` will not be re-organized (done markers prevent reprocessing). This is acceptable — the change applies to future runs only.

### What NOT To Do

- Do NOT touch `infer_category_from_title()` for the `Interview` block — it already returns `ContentCategory::Interview` correctly
- Do NOT add `"Deleted Scene"` to `series_tmdb.rs` `map_tmdb_type()` — it intentionally returns `None` for series
- Do NOT change `youtube.rs` category assignment — it already calls `infer_category_from_title()` correctly
- Do NOT change Archive.org's `infer_category_from_text()` — deferred gap, out of scope for this story
- Do NOT modify `test_infer_category_from_title_featurette` — the `"bonus clip"` → `Featurette` assertion is correct and unchanged; there is no `"movie clip"` → `Featurette` assertion in that test

### Previous Story Learnings (from 2.1)

- When modifying `infer_category_from_title()`, run `cargo build` immediately to catch any exhaustiveness issues — though this function uses `if/else` chains, not `match`, so no compiler errors expected
- Tests in `src/discovery/tmdb.rs` and `src/discovery/title_matching.rs` are co-located in `#[cfg(test)]` blocks within each file
- The quality gate is: `cargo build` ✅ → `cargo test` ✅ → `cargo clippy -- -D warnings` ✅ → `cargo fmt -- --check` ✅
- No `.unwrap()` in production code; use `.expect("descriptive message")` in tests
- Story 2.1 confirmed: `infer_category_from_title()` uses `if/else` chains — no exhaustiveness compiler errors when adding new `ContentCategory` variants

### Previous Story Learnings (from 1.4)

- When modifying shared functions used by multiple discoverers, search for all call sites before changing behavior
- `infer_category_from_title()` is called in `youtube.rs`, `series_youtube.rs`, and potentially `archive.rs` — check all call sites

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 2, Story 2.2]
- [Source: _bmad-output/planning-artifacts/prd.md — FR19, FR20, FR21, FR22, FR39]
- [Source: src/discovery/tmdb.rs — map_tmdb_type()]
- [Source: src/discovery/series_tmdb.rs — map_tmdb_type(), proptest known_types list]
- [Source: src/discovery/title_matching.rs — infer_category_from_title()]
- [Source: src/discovery/youtube.rs — category assignment at line 217 (already correct)]
- [Source: src/discovery/series_youtube.rs — search_youtube() category assignment at line ~230 (needs fix)]
- [Source: _bmad-output/implementation-artifacts/2-1-extend-content-category-enum-and-organizer-mappings.md — completion notes, regression patterns]

---

## Review Findings

- [x] [Review][Patch] `"short"` with trailing punctuation not detected as `Short` — `split_whitespace().any(|w| w == "short")` misses tokens like `"short:"` or `"short."` [src/discovery/title_matching.rs:Short detection block] — fixed: use `trim_matches(|c: char| !c.is_alphabetic())` before equality check
- [x] [Review][Patch] No unit test verifying `infer_category_from_title()` overrides search-query category in `series_youtube.rs` — behavioral fix (Task 4.2) had zero test coverage [src/discovery/series_youtube.rs] — fixed: added `test_infer_category_overrides_search_query_category` and `test_infer_category_falls_back_to_search_query_category`
- [x] [Review][Defer] `series_tmdb.rs` and `tmdb.rs` duplicate `map_tmdb_type()` — pre-existing architectural smell, not introduced by this story — deferred to a future refactor story

## Dev Agent Record

- Implementation Date: 2026-03-24
- All 5 tasks completed in a single pass
- Quality gate passed: `cargo build` ✅, `cargo test` (532 tests, 0 failures) ✅, `cargo clippy -- -D warnings` ✅, `cargo fmt -- --check` ✅
- Note: `tmdb.rs` had no pre-existing test module — created one with 10 unit tests covering all 9 known type mappings plus unknown types
- Note: `series_tmdb.rs` proptest was updated to check specific category values per type (not just `is_some()`), which is more rigorous than the story originally assumed
- Pre-existing gap fixed: `series_tmdb.rs` `map_tmdb_type()` was missing `"Teaser"` → `Trailer` (Task 3.6)
- Behavioral fix: `series_youtube.rs` `search_youtube()` now calls `infer_category_from_title()` before assigning category (Task 4.2)

## File List

| File | Change Type |
|---|---|
| `src/discovery/tmdb.rs` | Modified — extended `map_tmdb_type()` with 3 new/changed arms; added test module with 10 tests |
| `src/discovery/title_matching.rs` | Modified — added `Short` detection block; changed `Clip` block from `Featurette` to `Clip`; added 2 new tests |
| `src/discovery/series_tmdb.rs` | Modified — extended `map_tmdb_type()` with 4 new/changed arms; added 4 new unit tests; updated proptest and known-types test |
| `src/discovery/series_youtube.rs` | Modified — added `infer_category_from_title()` call before category assignment in `search_youtube()` |

## Change Log

- `map_tmdb_type()` in `tmdb.rs`: Added `"Interview"` → `Interview`, `"Short"` → `Short`, `"Bloopers"` → `Featurette`; changed `"Clip"` from `Featurette` to `Clip`
- `map_tmdb_type()` in `series_tmdb.rs`: Same changes as `tmdb.rs` plus added `"Teaser"` → `Trailer` (pre-existing gap fix)
- `infer_category_from_title()` in `title_matching.rs`: Added `Short` detection with word-boundary guard; changed clip detection from `Featurette` to `Clip`
- `search_youtube()` in `series_youtube.rs`: Added `title_matching::infer_category_from_title()` call with search-query category as fallback
