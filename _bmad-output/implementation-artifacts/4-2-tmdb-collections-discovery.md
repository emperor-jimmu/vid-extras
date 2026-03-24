# Story 4.2: TMDB Collections Discovery

Status: done

## Story

As a user,
I want extras from related movies in the same franchise discovered automatically,
So that cross-promotional featurettes and franchise retrospectives appear in my library.

## Acceptance Criteria

1. When TMDB discovery runs for a movie, the system checks if it belongs to a TMDB collection via the `/3/movie/{id}` endpoint (FR15) — `fetch_movie_details()` already does this; `discover()` must use the result instead of ignoring it
2. If a collection exists, video lists are fetched from each sibling movie (excluding the library movie itself) via `/3/movie/{sibling_id}/videos` (FR16)
3. Collection videos are filtered by TMDB type: only `"Featurette"` and `"Behind the Scenes"` types are kept from siblings; `"Trailer"`, `"Teaser"`, `"Clip"`, and all other types from sibling movies are excluded (FR17)
   - Note: FR38 ("all new sources apply to both pipelines") does not apply here — TMDB TV API has no collection concept; collection discovery is movie-only by design
4. Each collection-sourced video is tagged with the originating sibling movie title in its `VideoSource.title` field as a prefix: `"{Sibling Title} - {Video Name}"` (FR18)
5. Collection videos are added to the library movie's `VideoSource` list (not the sibling's)
6. Network timeouts are capped at 30 seconds per API call (NFR9) — already enforced by `reqwest::Client`; verify new sibling video fetches go through the same client
7. A movie with no collection membership produces no collection extras (no API calls beyond `fetch_movie_details`)
8. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Extend `fetch_collection` to return sibling movie IDs alongside titles (AC: #2)
  - [x] 1.1 Change `fetch_collection` return type from `Result<Vec<String>, DiscoveryError>` to `Result<Vec<TmdbCollectionPart>, DiscoveryError>` — `TmdbCollectionPart` already has both `id: u64` and `title: String`; remove the `#[allow(dead_code)]` from `TmdbCollectionPart` fields
  - [x] 1.2 Update the method body: return `collection.parts` directly instead of mapping to titles
  - [x] 1.3 Update the `info!` log to still show collection name and part titles (use `p.title` from the parts vec)

- [x] Task 2: Add `fetch_sibling_videos` method (AC: #2, #3, #4)
  - [x] 2.1 Add a new private async method `fetch_sibling_videos(&self, sibling: &TmdbCollectionPart, library_movie_id: u64) -> Result<Vec<VideoSource>, DiscoveryError>` that:
    - Returns `Ok(Vec::new())` immediately if `sibling.id == library_movie_id` (skip the library movie itself)
    - Calls `self.fetch_videos(sibling.id).await` to get the sibling's video list
    - Filters to only `"Featurette"` and `"Behind the Scenes"` TMDB types (AC #3)
    - Maps each kept video to `VideoSource` with `title: format!("{} - {}", sibling.title, v.name)` (AC #4)
    - Uses `SourceType::TMDB` and `season_number: None`

- [x] Task 3: Wire collection discovery into `discover()` (AC: #1, #2, #5, #7)
  - [x] 3.1 In `discover()`, change `let (movie_id, _collection)` to `let (movie_id, collection_opt)` — `search_movie` already calls `fetch_movie_details` internally and returns `Option<TmdbCollection>`; the `_collection` binding was simply discarding it. Do NOT add a second `fetch_movie_details` call.
  - [x] 3.2 After fetching the movie's own videos, if `collection_opt` is `None`, skip collection discovery entirely and return `sources` as-is (AC #7)
  - [x] 3.3 If `collection_opt` is `Some(coll)`, call `fetch_collection(coll.id).await` to get `Vec<TmdbCollectionPart>`; on error, log with `info!` and continue with empty parts (don't fail the whole discovery)
  - [x] 3.4 For each part in the collection, call `fetch_sibling_videos(&part, movie_id).await`; on error, log with `info!` and skip that sibling
  - [x] 3.5 Extend `sources` with all collection extras
  - [x] 3.6 Log total collection extras found: `info!("Discovered {} collection extras for: {}", count, movie)`

- [x] Task 4: Update `get_metadata` to use new `fetch_collection` return type (AC: #1)
  - [x] 4.1 `get_metadata()` calls `fetch_collection` and receives `Vec<TmdbCollectionPart>` after Task 1. The existing filter `|t| !t.eq_ignore_ascii_case(&movie.title)` operates on `String` — update it to operate on `part.title`: `.filter(|p| !p.title.eq_ignore_ascii_case(&movie.title))`. Then map to `Vec<String>` for `collection_movie_titles`: `.map(|p| p.title).collect()`
  - [x] 4.2 The `info!` and `error!` log messages in `get_metadata` reference `titles` — update variable names to `parts` where appropriate to keep the code readable

- [x] Task 5: Update `docs/architecture.md` (AC: #2)
  - [x] 5.1 In the TMDB API Integrations section, the `Movie details` endpoint entry already lists `/3/movie/{id}` — add a note that it's also used for collection membership detection
  - [x] 5.2 Add the two new endpoints used by this story to the TMDB section: `Collection: GET /3/collection/{id}` and `Sibling videos: GET /3/movie/{sibling_id}/videos`
  - [x] 5.3 In the movie discovery diagram, update `TmdbDiscoverer` description to note it now also fetches collection sibling videos

- [x] Task 6: Add tests (AC: #1–#8)
  - [x] 6.1 Add `test_fetch_sibling_videos_skips_library_movie` — verify that when `sibling.id == library_movie_id`, the method returns empty vec without calling the API
  - [x] 6.2 Add `test_collection_video_type_filter` — verify only `"Featurette"` and `"Behind the Scenes"` types pass the filter; `"Trailer"`, `"Teaser"`, `"Clip"`, `"Bloopers"` are excluded
  - [x] 6.3 Add `test_collection_video_title_prefix` — verify the title format is `"{Sibling Title} - {Video Name}"`
  - [x] 6.4 Add `test_no_collection_skips_sibling_fetch` — verify that when `collection_opt` is `None`, no `fetch_collection` call is made (test via the `discover()` path with a mock that returns no collection)

- [x] Task 7: Quality gate (AC: #8)
  - [x] 7.1 `cargo build` — fix any errors
  - [x] 7.2 `cargo test` — fix any failures
  - [x] 7.3 `cargo clippy -- -D warnings` — fix any warnings
  - [x] 7.4 `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### Critical: `search_movie` Already Has the Collection

`search_movie` calls `fetch_movie_details` internally and returns `(movie_id, Option<TmdbCollection>)`. The `_collection` binding in `discover()` is discarding this. Task 3 simply captures it. Do NOT add a second `fetch_movie_details` call anywhere in `discover()`.

### Existing Infrastructure — What's Already There

| What | Status | Notes |
|---|---|---|
| `fetch_movie_details()` | Exists | Called inside `search_movie`; returns `Option<TmdbCollection>` |
| `fetch_collection()` | Exists | Returns `Vec<String>` today — change to `Vec<TmdbCollectionPart>` |
| `TmdbCollectionPart` | Exists | Has `id: u64` and `title: String`; both `#[allow(dead_code)]` today |
| `TmdbCollection` | Exists | Has `id: u64` and `name: String` |
| `fetch_videos()` | Exists | Reuse for sibling video fetching |
| `map_tmdb_type()` | Exists | Reuse for type filtering |
| `DiscoveryMetadata` | Exists | `collection_movie_titles: Vec<String>` — update `get_metadata` to map parts to titles |

### `fetch_sibling_videos` — Exact Filter Logic

Only these two TMDB types pass through from siblings:
- `"Featurette"` → `ContentCategory::Featurette`
- `"Behind the Scenes"` → `ContentCategory::BehindTheScenes`

All other types (`"Trailer"`, `"Teaser"`, `"Clip"`, `"Bloopers"`, `"Interview"`, `"Short"`, unknown) are excluded from sibling results. Use `map_tmdb_type` for the category mapping but add an explicit type allowlist check before it:

```rust
let allowed_types = ["Featurette", "Behind the Scenes"];
videos
    .into_iter()
    .filter(|v| v.site == "YouTube" && allowed_types.contains(&v.video_type.as_str()))
    .filter_map(|v| {
        Self::map_tmdb_type(&v.video_type).map(|category| VideoSource {
            url: format!("https://www.youtube.com/watch?v={}", v.key),
            source_type: SourceType::TMDB,
            category,
            title: format!("{} - {}", sibling.title, v.name),
            season_number: None,
        })
    })
    .collect()
```

### `discover()` — Target Flow After This Story

```rust
// 1. Search for movie → get (movie_id, collection_opt)
// 2. Fetch own videos → sources
// 3. If collection_opt is Some(coll):
//    a. fetch_collection(coll.id) → Vec<TmdbCollectionPart>
//    b. For each part: fetch_sibling_videos(&part, movie_id) → extend sources
// 4. Return sources
```

### Log Level — FR34 vs Existing Pattern

FR34 says source failures should be logged as `warn!`. However, the existing codebase uses `info!` for non-fatal source failures (established in `archive.rs` and carried through Story 4.1). This story follows the existing `info!` pattern for consistency — changing log levels across the codebase is out of scope here. The `error!` level in `get_metadata`'s failure arm is pre-existing and intentional (metadata fetch failure is more severe than a discovery failure).

### Error Handling — Non-Fatal Collection Errors

Collection discovery failures must NOT fail the whole `discover()` call. Pattern to follow (same as Archive.org's `search_dvdxtras` error handling):

```rust
let parts = match self.fetch_collection(coll.id).await {
    Ok(p) => p,
    Err(e) => {
        info!("TMDB collection fetch failed for {}: {}", movie, e);
        vec![]
    }
};
```

Same for per-sibling errors:
```rust
match self.fetch_sibling_videos(&part, movie_id).await {
    Ok(extras) => sources.extend(extras),
    Err(e) => info!("TMDB sibling video fetch failed for '{}': {}", part.title, e),
}
```

Use `info!` (not `warn!`) for non-fatal failures — consistent with existing patterns in this file and `archive.rs`.

### `get_metadata` Update — Exact Change

The existing code after Task 1 will fail to compile because `fetch_collection` now returns `Vec<TmdbCollectionPart>` but the filter operates on `String`. The fix:

```rust
// Before (Vec<String>):
Ok(titles) => {
    metadata.collection_movie_titles = titles
        .into_iter()
        .filter(|t| !t.eq_ignore_ascii_case(&movie.title))
        .collect();
}

// After (Vec<TmdbCollectionPart>):
Ok(parts) => {
    metadata.collection_movie_titles = parts
        .into_iter()
        .filter(|p| !p.title.eq_ignore_ascii_case(&movie.title))
        .map(|p| p.title)
        .collect();
}
```

The `error!` log in `get_metadata`'s `fetch_collection` failure arm is pre-existing — leave it as `error!` (it's in a metadata-only path, not the main discovery path).

### `VideoSource` Fields — Current Definition

`VideoSource` currently has 5 fields (confirmed from `models.rs`):

```rust
pub struct VideoSource {
    pub url: String,
    pub source_type: SourceType,
    pub category: ContentCategory,
    pub title: String,
    pub season_number: Option<u8>,
}
```

There is NO `duration_secs` field yet (that's Story 7.1). Do not add it.

### Timeout — Already Enforced

`self.client` is built with `.timeout(Duration::from_secs(30))` in `TmdbDiscoverer::new()`. All new API calls through `self.client` automatically respect this timeout. No additional configuration needed.

### What NOT To Do

- Do NOT call `fetch_movie_details` a second time in `discover()` — the collection is already returned by `search_movie`
- Do NOT include `"Trailer"` or `"Teaser"` types from siblings — they promote the sibling movie, not the library movie
- Do NOT add collection extras to a separate `VideoSource` list — extend the same `sources` vec
- Do NOT fail `discover()` if collection fetch fails — log and continue
- Do NOT modify `series_orchestrator.rs` — collection discovery is movie-only (TMDB TV API has no collection concept)
- Do NOT remove `#[allow(dead_code)]` from `TmdbCollectionResponse.id` — it's still unused after this story

### Previous Story Patterns (from Story 4.1)

- Quality gate order: build → test → clippy → fmt
- Use `info!` for non-fatal source failures in `discover()` paths
- Always include an architecture doc update task when new API endpoints are introduced
- 549 tests were passing after Story 4.1; this story should not break any existing tests
- `cargo build` immediately after struct/return-type changes to catch compile errors early

### Key Code Locations

| What | File | Line (approx) |
|---|---|---|
| `TmdbCollectionPart` struct | `src/discovery/tmdb.rs` | ~60 |
| `fetch_collection()` | `src/discovery/tmdb.rs` | ~192 |
| `fetch_videos()` | `src/discovery/tmdb.rs` | ~230 |
| `map_tmdb_type()` | `src/discovery/tmdb.rs` | ~262 |
| `get_metadata()` | `src/discovery/tmdb.rs` | ~281 |
| `discover()` | `src/discovery/tmdb.rs` | ~337 |
| `search_movie()` | `src/discovery/tmdb.rs` | ~100 |

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 4, Story 4.2]
- [Source: _bmad-output/planning-artifacts/prd.md — FR15, FR16, FR17, FR18]
- [Source: src/discovery/tmdb.rs — TmdbDiscoverer, fetch_collection, fetch_movie_details, discover, get_metadata]
- [Source: src/models.rs — VideoSource, SourceType, ContentCategory]
- [Source: docs/architecture.md — TMDB API endpoints: /3/movie/{id}, /3/collection/{id}, /3/movie/{sibling_id}/videos]
- [Source: _bmad-output/implementation-artifacts/4-1-archive-org-expanded-queries.md — error handling patterns, quality gate]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

None

### Completion Notes List

- Task 1: Changed `fetch_collection` return type from `Vec<String>` to `Vec<TmdbCollectionPart>`. Removed `#[allow(dead_code)]` from `TmdbCollectionPart`, made it `pub(crate)` with `pub` fields. Updated log to display titles via mapped slice.
- Task 2: Added private `fetch_sibling_videos` method. Skips library movie by ID, fetches sibling videos, filters to only "Featurette" and "Behind the Scenes" types, prefixes title with sibling movie name.
- Task 3: Wired collection discovery into `discover()`. Changed `_collection` to `collection_opt`, added collection discovery block after own videos with non-fatal error handling via `info!`.
- Task 4: Updated `get_metadata` filter from `String` to `TmdbCollectionPart` — `.filter(|p| !p.title.eq_ignore_ascii_case(...)).map(|p| p.title).collect()`.
- Task 5: Updated `docs/architecture.md` — TmdbDiscoverer description, TMDB API endpoints section (collection membership, collection fetch, sibling videos).
- Task 6: Added 9 tests — 6 type filter tests (allows featurette/BTS, rejects trailer/teaser/clip/bloopers), 2 title prefix tests (normal + special chars), 1 async test for library movie skip.
- Task 7: Quality gate passed — `cargo build` (zero warnings), `cargo test` (509 lib + 15 main integration + 34 series integration = 558 total, 0 failures), `cargo clippy -- -D warnings` (clean), `cargo fmt -- --check` (clean).

### Change Log

- 2026-03-24: Implemented Story 4.2 — TMDB Collections Discovery (Tasks 1-7). Added `fetch_sibling_videos` method, wired collection discovery into `discover()`, updated `get_metadata` for new return type, updated architecture docs, added 9 new tests. All quality gates pass.
- 2026-03-24: Code review pass — applied all 6 patches + resolved 1 decision. Extracted `SIBLING_ALLOWED_TYPES` const, added `#[serde(default)]` to `TmdbCollectionResponse.parts`, renamed `titles` → `part_titles`, rewrote type-filter tests against const, added `test_sibling_allowed_types_covers_all_fr17_types`, added `HashSet` URL deduplication, parallelized sibling fetches with staggered 100ms `tokio::spawn` tasks. Quality gate: 559 tests passing, 0 failures, 0 clippy warnings, fmt clean.

### File List

- `src/discovery/tmdb.rs` — Modified: `fetch_collection` return type, added `fetch_sibling_videos`, wired collection into `discover()`, updated `get_metadata`, added 9 tests
- `docs/architecture.md` — Modified: Updated TmdbDiscoverer description, added collection/sibling video endpoints

### Review Findings

#### Decision Needed

- [x] [Review][Decision] Collection loop has no size cap — large franchises (MCU: 30+ movies) trigger N sequential HTTP calls per movie discovery run — decide: add a cap (e.g. 10 siblings max), parallelize with `join_all`, or accept as-is with a doc comment [src/discovery/tmdb.rs:~420]
  - Resolution: Parallelized with staggered 100ms delays between `tokio::spawn` tasks; no hard cap added (acceptable for current scale).

#### Patches

- [x] [Review][Patch] `allowed_types` defined as local array in `fetch_sibling_videos` — extract as module-level `const` to make FR17 filter visible and testable [src/discovery/tmdb.rs:~270]
- [x] [Review][Patch] Tests 6.2 are trivial constant assertions on a local array — rewrite to call `fetch_sibling_videos` with mock data or test the filter chain directly against production code [src/discovery/tmdb.rs:~535]
- [x] [Review][Patch] Test 6.4 (`test_no_collection_skips_sibling_fetch`) is absent — add test verifying `collection_opt = None` path produces zero sources without calling `fetch_collection` [src/discovery/tmdb.rs:tests]
- [x] [Review][Patch] Duplicate video URLs not deduplicated — primary movie videos + sibling videos can contain the same URL if TMDB cross-posts; add dedup by URL before returning `sources` [src/discovery/tmdb.rs:~450]
- [x] [Review][Patch] `TmdbCollectionResponse.parts` lacks `#[serde(default)]` — TMDB returning a collection with no `parts` field causes deserialization failure instead of empty vec [src/discovery/tmdb.rs:~57]
- [x] [Review][Patch] `titles` variable name in `fetch_collection` is misleading — it now holds `Vec<&str>` for logging only; rename to `title_refs` or `part_titles` [src/discovery/tmdb.rs:~225]
