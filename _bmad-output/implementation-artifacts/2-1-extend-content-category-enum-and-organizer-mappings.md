# Story 2.1: Extend ContentCategory Enum and Organizer Mappings

Status: done

## Story

As a user,
I want my extras organized into all 8 Jellyfin video extras categories,
So that interviews, shorts, clips, and scenes each have their own section in Jellyfin.

## Acceptance Criteria

1. 4 new variants added to `ContentCategory` enum: `Short`, `Clip`, `Scene`, `Extras` (FR19, FR39)
2. `ContentCategory::subdirectory()` maps the 4 new variants to Jellyfin subfolder names (FR20, FR39):
   - `Short` → `"shorts"`
   - `Clip` → `"clips"`
   - `Scene` → `"scenes"`
   - `Extras` → `"extras"` (catch-all per FR39 — not one of the 8 named categories in FR20)
3. Existing 5 category mappings remain unchanged: Trailer→`trailers`, Featurette→`featurettes`, BehindTheScenes→`behind the scenes`, DeletedScene→`deleted scenes`, Interview→`interviews`
4. The "Bloopers" TMDB type continues to map to `Featurette` — no change to `map_tmdb_type()` (FR22)
5. `ContentCategory::Display` impl covers all 9 variants without compiler warning
6. `cargo build` compiles without errors; existing tests pass; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Add 4 new variants to `ContentCategory` in `src/models.rs` (AC: #1, #5)
  - [x] 1.1 Add `Short`, `Clip`, `Scene`, `Extras` variants to the `ContentCategory` enum (after `Interview`)
  - [x] 1.2 Extend `impl fmt::Display for ContentCategory` with arms for all 4 new variants:
    - `Short` → `"Short"`
    - `Clip` → `"Clip"`
    - `Scene` → `"Scene"`
    - `Extras` → `"Extras"`
  - [x] 1.3 Extend `ContentCategory::subdirectory()` with arms for all 4 new variants (AC: #2)
  - [x] 1.4 Verify the existing `test_content_category_subdirectory` test in `src/models.rs` still passes; extend it to cover the 4 new variants

- [x] Task 2: Fix exhaustiveness gaps caused by new variants (AC: #3, #6)
  - [x] 2.1 Run `cargo build` — the compiler will report every `match` on `ContentCategory` that is now non-exhaustive; fix each one
  - [x] 2.2 In `src/discovery/title_matching.rs` — `infer_category_from_title()` returns `Option<ContentCategory>`; no match on the enum here, no change needed
  - [x] 2.3 In `src/discovery/tmdb.rs` — `map_tmdb_type()` uses string matching, not an enum match; no change needed
  - [x] 2.4 In `src/organizer.rs` — `Organizer::ensure_subdirectory()` and `SeriesOrganizer::ensure_subdirectory()` both call `category.subdirectory()` which is already exhaustive via the method; no direct match needed
  - [x] 2.5 In `src/output.rs` — check if any match on `ContentCategory` exists (e.g., in display helpers); add arms if needed
  - [x] 2.6 In `src/discovery/series_tmdb.rs`, `series_youtube.rs`, `series_orchestrator.rs` — check for any `match category` patterns; add arms if needed
  - [x] 2.7 In `tests/` — check integration tests for any `match category` patterns; add arms if needed

- [x] Task 3: Update proptest enumerations in `src/organizer.rs` (AC: #6)
  - [x] 3.1 In `prop_category_to_subdirectory_mapping` (line ~1114): add `Just(ContentCategory::Short)`, `Just(ContentCategory::Clip)`, `Just(ContentCategory::Scene)`, `Just(ContentCategory::Extras)` to the `prop_oneof!` list
  - [x] 3.2 In the `match category` block inside that same proptest (line ~1123): add arms for all 4 new variants mapping to their expected subdirectory strings (`"shorts"`, `"clips"`, `"scenes"`, `"extras"`)
  - [x] 3.3 In `prop_subdirectory_creation` (line ~1139): add the same 4 `Just(...)` entries to its `prop_oneof!` list
  - [x] 3.4 In `prop_series_category_to_subdirectory_mapping` (line ~1229): add the same 4 `Just(...)` entries to its `prop_oneof!` list

- [x] Task 4: Tests (AC: #6)
  - [x] 4.1 Extend `test_content_category_subdirectory` in `src/models.rs` to assert all 4 new subdirectory mappings
  - [x] 4.2 Add `test_content_category_display_new_variants` in `src/models.rs` asserting `Display` output for `Short`, `Clip`, `Scene`, `Extras`
  - [x] 4.3 Run `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt -- --check`

## Dev Notes

### What Already Exists (Do NOT Redo)

- `ContentCategory` is defined in `src/models.rs` at line 227 with 5 variants: `Trailer`, `Featurette`, `BehindTheScenes`, `DeletedScene`, `Interview`
- `subdirectory()` method already exists on `ContentCategory` — just add 4 new match arms
- `infer_category_from_title()` in `src/discovery/title_matching.rs` returns `Option<ContentCategory>` via `if/else` chains — no `match` on the enum, so no exhaustiveness issue there; leave it unchanged (Story 2.2 owns classification logic)
- `map_tmdb_type()` in `src/discovery/tmdb.rs` uses string matching, not an enum match — no exhaustiveness issue; leave it unchanged (Story 2.2 owns TMDB type mappings)
- `Organizer::ensure_subdirectory()` and `SeriesOrganizer::ensure_subdirectory()` both delegate to `category.subdirectory()` — no direct match on the enum, so they automatically support new variants once `subdirectory()` is extended
- `test_series_organizer_multiple_categories` in `src/organizer.rs` already tests `Interview` → `"interviews"` — this test passes today and must continue to pass

### Key Code Locations

| What | File | Line | Current State |
|---|---|---|---|
| `ContentCategory` enum | `src/models.rs` | 227 | 5 variants |
| `ContentCategory::Display` | `src/models.rs` | 241 | 5 arms |
| `ContentCategory::subdirectory()` | `src/models.rs` | 254 | 5 arms |
| `test_content_category_subdirectory` | `src/models.rs` | 525 | Tests 5 variants |
| `prop_category_to_subdirectory_mapping` | `src/organizer.rs` | ~1114 | `prop_oneof!` + `match` — both need new arms |
| `prop_subdirectory_creation` | `src/organizer.rs` | ~1139 | `prop_oneof!` — needs new entries |
| `prop_series_category_to_subdirectory_mapping` | `src/organizer.rs` | ~1229 | `prop_oneof!` — needs new entries |
| `Organizer::ensure_subdirectory()` | `src/organizer.rs` | 85 | Calls `category.subdirectory()` — no direct match |
| `SeriesOrganizer::ensure_subdirectory()` | `src/organizer.rs` | 424 | Calls `category.subdirectory()` — no direct match |

### Exhaustiveness: Where to Look

The compiler will catch all non-exhaustive `match` expressions on `ContentCategory` when you add the 4 new variants. Based on a full codebase search, the only non-exhaustive `match` is inside `prop_category_to_subdirectory_mapping` in `src/organizer.rs` (line ~1123). The `prop_oneof!` lists in the three proptests are not compiler-enforced but must be updated manually to keep test coverage complete.

`infer_category_from_title()` and `map_tmdb_type()` use `if/else` chains and string matching respectively — neither has a `match` on `ContentCategory`, so neither will produce a compile error. They are out of scope for this story.

Use `cargo build 2>&1` after adding the variants to confirm the exact set of errors before fixing.

### `ContentCategory` Derives

The enum currently derives `Debug, Clone, Copy, PartialEq, Eq, Hash`. All 4 new variants inherit these derives automatically — no changes needed to the derive list.

### Subdirectory Name Conventions

Jellyfin's expected subfolder names (lowercase, matching Jellyfin's internal scanner):
- `"shorts"` — not `"short films"` or `"short"`
- `"clips"` — not `"movie clips"`
- `"scenes"` — not `"movie scenes"`
- `"extras"` — catch-all for uncategorized content

These are the exact strings Jellyfin recognizes. Do not deviate.

### `Extras` Category Usage

`ContentCategory::Extras` is the catch-all for content that cannot be mapped to any of the 8 defined categories (FR39). It is NOT assigned by any discoverer in this story — that's Story 2.2's scope. For this story, `Extras` is a valid enum variant that compiles and maps to `"extras"`. Clippy will NOT warn about it being unused — Rust does not emit dead_code warnings for enum variants as long as the enum type itself is used.

### Classification Logic Is Story 2.2's Scope

`infer_category_from_title()` and `map_tmdb_type()` are intentionally left unchanged in this story. Story 2.2 owns all classification logic changes, including:
- Mapping TMDB types `"Interview"`, `"Short"`, `"Clip"` to their new categories
- Updating `infer_category_from_title()` to return `Clip` and `Short`
- Assigning `Extras` as the fallback when no category matches

Do not touch these functions in this story.

### Regression Risks

- Adding 4 new enum variants will cause a compile error in the `match category` block inside `prop_category_to_subdirectory_mapping` — fix it in Task 3.2
- The three `prop_oneof!` lists in `src/organizer.rs` are not compiler-enforced but must be updated (Tasks 3.1, 3.3, 3.4) to keep proptest coverage complete
- No other production code has a `match` on `ContentCategory` — confirmed by codebase search

### What NOT To Do

- Do NOT touch `infer_category_from_title()` in `src/discovery/title_matching.rs` — Story 2.2 owns this
- Do NOT touch `map_tmdb_type()` in `src/discovery/tmdb.rs` — Story 2.2 owns this
- Do NOT change the `"Bloopers"` → `Featurette` mapping — AC #4 explicitly preserves this
- Do NOT modify `Organizer::ensure_subdirectory()` or `SeriesOrganizer::ensure_subdirectory()` — they already delegate to `subdirectory()` and will work automatically

### Previous Story Learnings (from 1.4)

- When adding enum variants, run `cargo build` immediately to get the full list of non-exhaustive match errors — fix them all before writing tests
- Tests in `src/models.rs` use `#[cfg(test)]` blocks co-located in the file — add new tests there, not in a separate file
- The quality gate is: `cargo build` ✅ → `cargo test` ✅ → `cargo clippy -- -D warnings` ✅ → `cargo fmt -- --check` ✅
- No `.unwrap()` in production code; use `.expect("descriptive message")` in tests

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 2, Story 2.1]
- [Source: _bmad-output/planning-artifacts/prd.md — FR19, FR20, FR22, FR39]
- [Source: src/models.rs — ContentCategory enum, subdirectory(), Display impl]
- [Source: src/organizer.rs — prop_category_to_subdirectory_mapping, prop_subdirectory_creation, prop_series_category_to_subdirectory_mapping]
- [Source: docs/architecture.md — Organizer subdirectory mapping table]

## Senior Developer Review (AI)

**Review Date:** 2026-03-24
**Outcome:** Changes Requested
**Layers:** Blind Hunter ✅, Edge Case Hunter ✅, Acceptance Auditor ✅
**Dismissed:** 2 (noise/false positives)

### Action Items

- [x] [F-1][Low] Extend `test_content_category_display_new_variants` to cover all 9 variants, not just the 4 new ones [src/models.rs]
- [x] [F-2][Low] Remove internal planning reference `(FR39)` from `Extras` variant doc comment [src/models.rs]
- [x] [F-3][Low][Defer] `Clip` and `Scene` doc comments are semantically near-identical — pre-existing design ambiguity, deferred to Story 2.2 which owns classification logic

### Review Follow-ups (AI)

- [x] [AI-Review][Patch] Extend `test_content_category_display_new_variants` to assert Display output for all 9 variants (Trailer, Featurette, BehindTheScenes, DeletedScene, Interview, Short, Clip, Scene, Extras) [src/models.rs]
- [x] [AI-Review][Patch] Remove `(FR39)` from `Extras` doc comment — replace with a user-facing description only [src/models.rs]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

_None_

### Completion Notes List

- Added 4 new `ContentCategory` variants (`Short`, `Clip`, `Scene`, `Extras`) with `Display` and `subdirectory()` implementations
- No exhaustiveness errors in production code — all existing code uses `if/else` chains, string matching, or delegates to `subdirectory()` method
- Updated 3 proptests in `src/organizer.rs` to cover all 9 variants (added 4 `Just(...)` entries to each `prop_oneof!` and 4 match arms to the verification proptest)
- Added `test_content_category_display_new_variants` unit test and extended `test_content_category_subdirectory` with 4 new assertions
- Quality gates: `cargo build` ✅, `cargo test` (516 passed, 0 failed) ✅, `cargo clippy -- -D warnings` ✅, `cargo fmt -- --check` ✅

### File List

- `src/models.rs` — Added 4 enum variants, 4 Display arms, 4 subdirectory arms, extended existing test, added new test
- `src/organizer.rs` — Updated 3 proptests (`prop_category_to_subdirectory_mapping`, `prop_subdirectory_creation`, `prop_series_category_to_subdirectory_mapping`)

## Change Log

- 2026-03-24: Story created — ready for dev
- 2026-03-24: Review pass — removed out-of-scope Tasks 3/4 (classification logic and map_tmdb_type changes belong to Story 2.2); replaced with correct proptest update tasks; added FR citations to AC; clarified exhaustiveness scope based on codebase search
- 2026-03-24: Implementation complete — all 4 tasks done, 516 tests passing, all quality gates green; status → review
- 2026-03-24: Code review — 2 patches applied (expanded Display test to all 9 variants, removed FR39 from Extras doc comment), 1 deferred; status → done
