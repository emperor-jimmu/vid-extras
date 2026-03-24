# Story 5.1: KinoCheck Discoverer as TMDB Fallback

Status: done

## Story

As a user,
I want official trailers found automatically for movies where TMDB has no videos,
So that I get trailer coverage even when TMDB's database is incomplete.

## Acceptance Criteria

1. When TMDB is in the active source list and returns zero videos for a movie, KinoCheck is queried using the movie's TMDB ID via `https://api.kinocheck.de/movies?tmdb_id={id}` (FR11)
2. Returned YouTube video IDs are added as `VideoSource` entries with `SourceType::KinoCheck` and appropriate `ContentCategory` mapped from the `categories` array
3. KinoCheck is skipped entirely when TMDB returns one or more videos (FR12)
4. KinoCheck is not queried when `tmdb` is not in the active source list
5. All API requests use HTTPS (NFR14)
6. Network timeouts are capped at 30 seconds (NFR9) — use the same `reqwest::Client` pattern as `TmdbDiscoverer`
7. The system logs a `warn!` when the in-process KinoCheck request count reaches 80% of the 1,000 req/day free tier limit (NFR3) — the `Arc<AtomicU32>` counter is passed in at construction time so both movie and series orchestrators share the same counter instance
8. The system handles HTTP 429 responses from KinoCheck by waiting 1 second and retrying once before returning `Ok(vec![])` (NFR10)
9. KinoCheck API response parse failures are logged at `warn!` level with the raw response snippet (up to 200 chars) for debugging (NFR15)
10. KinoCheck network/API failures are logged with `info!` and do not prevent other sources from completing
11. The discoverer works for both movie and series discovery pipelines (FR38) — wired into `DiscoveryOrchestrator` and `SeriesDiscoveryOrchestrator`
12. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Create `src/discovery/kinocheck.rs` with `KinoCheckDiscoverer` (AC: #1, #2, #5, #6, #7, #8, #9, #10)
  - [x] 1.1 Define private serde structs for the KinoCheck API response (see Dev Notes for exact shape)
  - [x] 1.2 Define `KinoCheckDiscoverer` struct with `client: reqwest::Client` and `request_count: Arc<AtomicU32>`; derive `Clone` (cloning `Arc` is cheap and needed by `DiscoveryOrchestrator::with_cookies`)
  - [x] 1.3 Implement `KinoCheckDiscoverer::new(request_count: Arc<AtomicU32>)` — builds `reqwest::Client` with 30s timeout (same pattern as `TmdbDiscoverer::new()`); takes the counter as a parameter so callers share one instance across movie and series orchestrators
  - [x] 1.4 Implement `async fn fetch_movie(&self, tmdb_id: u64) -> Result<Option<KinoCheckMovie>, DiscoveryError>` — GET `https://api.kinocheck.de/movies?tmdb_id={id}`, returns `Ok(None)` on 404; on HTTP 429 wait 1 second and retry once, then return `Ok(vec![])` from the caller (AC #8); on parse failure log `warn!` with raw response snippet ≤ 200 chars (AC #9)
  - [x] 1.5 Implement `fn map_category(categories: &[String]) -> ContentCategory` — maps KinoCheck `categories` array to `ContentCategory` (see Dev Notes for mapping table)
  - [x] 1.6 Implement `pub async fn discover_for_tmdb_id(&self, tmdb_id: u64) -> Result<Vec<VideoSource>, DiscoveryError>` — calls `fetch_movie`, maps `trailer` field to `VideoSource`, increments request counter, logs 80% warning at `warn!` level
  - [x] 1.7 Add unit tests (see Task 5)

- [x] Task 2: Register module in `src/discovery/mod.rs` (AC: #11)
  - [x] 2.1 Add `mod kinocheck;` to the module list
  - [x] 2.2 Add `pub(crate) use kinocheck::KinoCheckDiscoverer;` to re-exports

- [x] Task 3: Wire KinoCheck into `DiscoveryOrchestrator` (AC: #1, #3, #4, #10)
  - [x] 3.1 Add `kinocheck: KinoCheckDiscoverer` field to `DiscoveryOrchestrator`
  - [x] 3.2 Add `kinocheck_request_count: Arc<AtomicU32>` to `DiscoveryOrchestrator::new()` and `with_cookies()` signatures — callers (i.e., `orchestrator.rs`) create one `Arc::new(AtomicU32::new(0))` and pass it to both `DiscoveryOrchestrator` and `SeriesDiscoveryOrchestrator` so the counter is shared across both pipelines
  - [x] 3.3 Refactor `TmdbDiscoverer::discover_with_library()` to return `(Vec<VideoSource>, Option<u64>)` — the second element is the TMDB movie ID found during `search_movie`. This avoids a second TMDB API call and is the preferred approach. Update all callers of `discover_with_library` accordingly.
  - [x] 3.4 In `discover_all()`, capture the movie ID from the refactored `discover_with_library` return value and store it in `tmdb_movie_id: Option<u64>`
  - [x] 3.5 After the TMDB block, if `Source::Tmdb` is active AND `source_results` has a TMDB entry with `videos_found == 0` and `error.is_none()`, AND `tmdb_movie_id` is `Some`, call `self.kinocheck.discover_for_tmdb_id(movie_id)` and extend `all_sources`
  - [x] 3.6 KinoCheck results are added to `all_sources` but NOT added to `source_results` (KinoCheck is an internal TMDB fallback, not a user-visible source in the `--sources` list)
  - [x] 3.7 Log KinoCheck results: `info!("KinoCheck fallback found {} videos for {}", count, movie)`

- [x] Task 4: Wire KinoCheck into `SeriesDiscoveryOrchestrator` (AC: #11)
  - [x] 4.1 Add `kinocheck: KinoCheckDiscoverer` field to `SeriesDiscoveryOrchestrator`
  - [x] 4.2 Update `new()` and `new_with_tvdb()` to accept `kinocheck_request_count: Arc<AtomicU32>` and construct `KinoCheckDiscoverer::new(kinocheck_request_count)`
  - [x] 4.3 In `discover_all()` for series, capture `tmdb_series_id` from the existing `search_series` call (it's already available as a local — just store it before the inner match). After the TMDB block: if TMDB is active AND returned zero videos AND `tmdb_series_id` is `Some`, call `self.kinocheck.discover_for_tmdb_id(series_id)`, convert results via `video_source_to_series_extra` (see Dev Notes), and extend `all_sources`
  - [x] 4.4 Log KinoCheck results: `info!("KinoCheck fallback found {} videos for {}", count, series)`

- [x] Task 5: Update `orchestrator.rs` (top-level) to create and share the counter (AC: #7)
  - [x] 5.1 In `Orchestrator::new()`, create `let kinocheck_counter = Arc::new(AtomicU32::new(0));`
  - [x] 5.2 Pass `kinocheck_counter.clone()` to `DiscoveryOrchestrator::new()` and `SeriesDiscoveryOrchestrator::new()` / `new_with_tvdb()` so both share the same counter

- [x] Task 6: Add tests (AC: #1–#12)
  - [x] 6.1 `test_map_category_trailer` — `["Trailer"]` → `ContentCategory::Trailer`
  - [x] 6.2 `test_map_category_featurette` — `["Featurette"]` → `ContentCategory::Featurette`
  - [x] 6.3 `test_map_category_behind_the_scenes` — `["Behind the Scenes"]` → `ContentCategory::BehindTheScenes`
  - [x] 6.4 `test_map_category_unknown_defaults_to_extras` — `["SomethingNew"]` → `ContentCategory::Extras`
  - [x] 6.5 `test_map_category_empty_defaults_to_extras` — `[]` → `ContentCategory::Extras`
  - [x] 6.6 `test_video_source_url_construction` — verify YouTube URL is `https://www.youtube.com/watch?v={youtube_video_id}`
  - [x] 6.7 `test_request_counter_increments` — verify `request_count` increments after `discover_for_tmdb_id`
  - [x] 6.8 `test_80_percent_warning_threshold` — set counter to 800, verify the threshold constant is `>= 800`; test the counter value directly (no mock needed — just verify `KINOCHECK_WARN_THRESHOLD == 800`)
  - [x] 6.9 `test_no_trailer_returns_empty` — parse JSON with `"trailer": null`, verify `discover_for_tmdb_id` returns empty vec
  - [x] 6.10 `test_discover_for_tmdb_id_parses_response` — parse a hardcoded JSON fixture matching the real API shape (see Dev Notes), verify `VideoSource` fields: `url`, `title`, `source_type == SourceType::KinoCheck`, `category`
  - [x] 6.11 `test_shared_counter_across_instances` — create two `KinoCheckDiscoverer` instances sharing the same `Arc<AtomicU32>`, verify that incrementing via one is visible in the other

- [x] Task 7: Update `docs/architecture.md` (AC: #1)
  - [x] 7.1 Add KinoCheck to the Movie Discovery diagram under `DiscoveryOrchestrator` as an internal fallback (not a top-level discoverer): `└── KinoCheckDiscoverer (implicit fallback when TMDB returns 0 videos)`
  - [x] 7.2 Add KinoCheck API endpoint to the External API Integrations section: `GET https://api.kinocheck.de/movies?tmdb_id={id}` — no auth required, free tier 1,000 req/day
  - [x] 7.3 Fix stale entry in `models.rs` key types: remove `SourceMode — enum: All, YoutubeOnly` (replaced by `Source` enum + `Vec<Source>` in Story 1.1)

- [x] Task 8: Quality gate (AC: #12)
  - [x] 8.1 `cargo build` — fix any errors
  - [x] 8.2 `cargo test` — fix any failures
  - [x] 8.3 `cargo clippy -- -D warnings` — fix any warnings
  - [x] 8.4 `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### KinoCheck API Response Shape

Live response from `https://api.kinocheck.de/movies?tmdb_id=550` (Fight Club):

```json
{
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
  "recommendations": [...]
}
```

Key observations:
- `trailer` is a single object (not an array), and can be `null` when no trailer exists
- `youtube_video_id` is the YouTube video key — construct URL as `https://www.youtube.com/watch?v={youtube_video_id}`
- `categories` is a `Vec<String>` — use first element for category mapping
- `recommendations` is irrelevant — ignore it entirely; serde ignores unknown fields by default so no `#[serde(default)]` needed for it
- No API key required — free public API

Serde structs to define (all private):

```rust
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
```

### HTTP 429 and Parse Error Handling

```rust
async fn fetch_movie(&self, tmdb_id: u64) -> Result<Option<KinoCheckMovie>, DiscoveryError> {
    let url = format!("https://api.kinocheck.de/movies?tmdb_id={}", tmdb_id);
    
    let response = self.client.get(&url).send().await.map_err(DiscoveryError::NetworkError)?;
    
    // 404 = movie not in KinoCheck database — not an error
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    
    // 429 = rate limited — wait 1s and retry once (NFR10)
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        warn!("KinoCheck rate limited (429), retrying after 1s");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let retry = self.client.get(&url).send().await.map_err(DiscoveryError::NetworkError)?;
        if !retry.status().is_success() {
            info!("KinoCheck retry failed with status {}", retry.status());
            return Ok(None); // treat as not found, don't propagate error
        }
        return self.parse_response(retry).await;
    }
    
    if !response.status().is_success() {
        return Err(DiscoveryError::ApiError(format!("KinoCheck returned {}", response.status())));
    }
    
    self.parse_response(response).await
}

async fn parse_response(&self, response: reqwest::Response) -> Result<Option<KinoCheckMovie>, DiscoveryError> {
    let text = response.text().await.map_err(DiscoveryError::NetworkError)?;
    match serde_json::from_str::<KinoCheckMovie>(&text) {
        Ok(movie) => Ok(Some(movie)),
        Err(e) => {
            // NFR15: log raw snippet for debugging
            let snippet = &text[..text.len().min(200)];
            warn!("KinoCheck response parse failed: {}. Raw: {}", e, snippet);
            Ok(None) // degrade gracefully
        }
    }
}
```

### Request Counter — Shared Across Pipelines

The counter must be shared between `DiscoveryOrchestrator` (movie pipeline) and `SeriesDiscoveryOrchestrator` (series pipeline) so the 1,000 req/day limit is tracked across both. The top-level `Orchestrator` in `orchestrator.rs` creates one `Arc<AtomicU32>` and passes it to both:

```rust
// In Orchestrator::new():
let kinocheck_counter = Arc::new(AtomicU32::new(0));
let discovery = DiscoveryOrchestrator::new(tmdb_api_key.clone(), sources.clone(), kinocheck_counter.clone());
let series_discovery = SeriesDiscoveryOrchestrator::new(tmdb_api_key, sources, kinocheck_counter);
```

`KinoCheckDiscoverer::new()` signature:
```rust
pub fn new(request_count: Arc<AtomicU32>) -> Self
```

The 80% warning logic in `discover_for_tmdb_id`:
```rust
const KINOCHECK_DAILY_LIMIT: u32 = 1_000;
const KINOCHECK_WARN_THRESHOLD: u32 = 800; // 80%

let count = self.request_count.fetch_add(1, Ordering::Relaxed) + 1;
if count >= KINOCHECK_WARN_THRESHOLD {
    warn!(
        "KinoCheck request count at {}/{} ({}% of daily limit)",
        count,
        KINOCHECK_DAILY_LIMIT,
        count * 100 / KINOCHECK_DAILY_LIMIT
    );
}
```

Note: The counter is in-process only (resets on restart). This is sufficient for NFR3.

### `KinoCheckDiscoverer` Must Be `Clone`

`DiscoveryOrchestrator::with_cookies()` constructs a new `Self` by re-creating fields. Since `KinoCheckDiscoverer` contains `Arc<AtomicU32>` (which is `Clone`) and `reqwest::Client` (which is `Clone`), deriving `Clone` is straightforward:

```rust
#[derive(Clone)]
pub struct KinoCheckDiscoverer {
    client: reqwest::Client,
    request_count: Arc<AtomicU32>,
}
```

### Refactoring `discover_with_library` to Return Movie ID

Preferred approach (avoids a second TMDB API call):

```rust
// Change signature:
pub async fn discover_with_library(
    &self,
    movie: &MovieEntry,
    library: &[MovieEntry],
) -> Result<(Vec<VideoSource>, Option<u64>), DiscoveryError>
// Returns (sources, tmdb_movie_id)
```

Inside the function, the `movie_id` is already available from `search_movie`. Return it alongside `sources`. Update the one call site in `orchestrator.rs` to destructure the tuple.

Do NOT add a separate `search_movie_id` method — the refactor is cleaner and avoids the double API call.

### TMDB Movie ID Availability in `discover_all()`

After the `discover_with_library` refactor:

```rust
let mut tmdb_movie_id: Option<u64> = None;

if self.sources.contains(&Source::Tmdb) {
    match self.tmdb.discover_with_library(movie, library).await {
        Ok((sources, movie_id)) => {
            tmdb_movie_id = movie_id;
            info!("Found {} sources from TMDB for {}", sources.len(), movie);
            source_results.push(SourceResult { source: Source::Tmdb, videos_found: sources.len(), error: None });
            all_sources.extend(sources);
        }
        Err(e) => {
            warn!("TMDB discovery failed for {}: {}", movie, e);
            source_results.push(SourceResult { source: Source::Tmdb, videos_found: 0, error: Some(e.to_string()) });
        }
    }
}

// KinoCheck fallback: TMDB active + found movie + returned 0 videos
let tmdb_found_zero = source_results
    .iter()
    .any(|r| r.source == Source::Tmdb && r.videos_found == 0 && r.error.is_none());

if self.sources.contains(&Source::Tmdb) && tmdb_found_zero {
    if let Some(movie_id) = tmdb_movie_id {
        match self.kinocheck.discover_for_tmdb_id(movie_id).await {
            Ok(sources) => {
                info!("KinoCheck fallback found {} videos for {}", sources.len(), movie);
                all_sources.extend(sources);
            }
            Err(e) => {
                info!("KinoCheck fallback failed for {}: {}", movie, e);
            }
        }
    }
}
```

### Series Pipeline: VideoSource → SeriesExtra Conversion

`SeriesDiscoveryOrchestrator::discover_all()` works with `Vec<SeriesExtra>`, but `KinoCheckDiscoverer::discover_for_tmdb_id()` returns `Vec<VideoSource>`. The existing `From<SeriesExtra> for VideoSource` goes the wrong direction. Add a conversion in the series orchestrator:

```rust
fn video_source_to_series_extra(vs: VideoSource, series: &SeriesEntry) -> SeriesExtra {
    SeriesExtra {
        series_id: format!("{}_{}", series.title.replace(' ', "_"), series.year.unwrap_or(0)),
        season_number: None, // series-level extra
        category: vs.category,
        title: vs.title,
        url: vs.url,
        source_type: vs.source_type,
        local_path: None,
    }
}
```

Use this inline in the series orchestrator's KinoCheck block — no need to add it to `models.rs`.

### Series Pipeline: Getting the TMDB Series ID

`SeriesDiscoveryOrchestrator::discover_all()` already calls `self.tmdb.search_series(&series.title, series.year)` and stores the result as `series_id: u64`. Capture this in a local variable before the TMDB block so it's available for the KinoCheck fallback:

```rust
let mut tmdb_series_id: Option<u64> = None;

if self.sources.contains(&Source::Tmdb) {
    match self.tmdb.search_series(&series.title, series.year).await {
        Ok(Some(series_id)) => {
            tmdb_series_id = Some(series_id);
            // ... existing extras discovery ...
        }
        // ...
    }
}

// KinoCheck fallback for series
let tmdb_found_zero = source_results
    .iter()
    .any(|r| r.source == Source::Tmdb && r.videos_found == 0 && r.error.is_none());

if self.sources.contains(&Source::Tmdb) && tmdb_found_zero {
    if let Some(series_id) = tmdb_series_id {
        match self.kinocheck.discover_for_tmdb_id(series_id).await {
            Ok(sources) => {
                info!("KinoCheck fallback found {} videos for {}", sources.len(), series);
                let extras: Vec<SeriesExtra> = sources
                    .into_iter()
                    .map(|vs| video_source_to_series_extra(vs, series))
                    .collect();
                all_sources.extend(extras);
            }
            Err(e) => {
                info!("KinoCheck fallback failed for {}: {}", series, e);
            }
        }
    }
}
```

### Error Handling — Use `info!` Not `warn!`

KinoCheck is an optional fallback. Failures should use `info!` (not `warn!`) — consistent with the established pattern for non-fatal source failures in `tmdb.rs` and `archive.rs`. The `warn!` level is reserved for user-visible source failures in `source_results`.

### 404 Handling

KinoCheck returns HTTP 404 when a movie is not in their database. This is not an error — return `Ok(None)` from `fetch_movie` and `Ok(vec![])` from `discover_for_tmdb_id`. Do NOT add a `SourceResult` entry for KinoCheck (it's internal, not user-visible).

### What NOT To Do

- Do NOT add `Source::KinoCheck` to the `Source` enum — KinoCheck is an internal TMDB fallback, not a user-selectable source
- Do NOT add KinoCheck to `source_results` — it's not a user-visible source in the `--sources` list
- Do NOT query KinoCheck when TMDB returned an error (only when TMDB succeeded but found zero videos)
- Do NOT query KinoCheck when `tmdb` is not in the active source list
- Do NOT use `warn!` for KinoCheck failures — use `info!`
- Do NOT add `KinoCheckError` to `error.rs` — reuse `DiscoveryError::ApiError` and `DiscoveryError::NetworkError`

### Key Code Locations

| What | File | Notes |
|---|---|---|
| `DiscoveryOrchestrator` | `src/discovery/orchestrator.rs` | Add `kinocheck` field, wire fallback |
| `SeriesDiscoveryOrchestrator` | `src/discovery/series_orchestrator.rs` | Add `kinocheck` field, wire fallback |
| `TmdbDiscoverer` | `src/discovery/tmdb.rs` | Add `search_movie_id()` helper |
| `DiscoveryError` | `src/error.rs` | Reuse existing variants — no new variants needed |
| `SourceType::KinoCheck` | `src/models.rs` | Already exists — use it |
| `ContentCategory::Extras` | `src/models.rs` | Already exists — use as fallback category |
| `mod.rs` | `src/discovery/mod.rs` | Register new module |

### Test Count Baseline

559 tests were passing after Story 4.2. This story should not break any existing tests. New tests in `kinocheck.rs` will add to the total.

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 5, Story 5.1]
- [Source: src/discovery/orchestrator.rs — DiscoveryOrchestrator, discover_all, apply_content_limits]
- [Source: src/discovery/series_orchestrator.rs — SeriesDiscoveryOrchestrator, discover_all]
- [Source: src/discovery/tmdb.rs — TmdbDiscoverer, search_movie, discover_with_library]
- [Source: src/models.rs — SourceType::KinoCheck, ContentCategory::Extras, VideoSource, SeriesExtra]
- [Source: src/error.rs — DiscoveryError variants]
- [Source: docs/architecture.md — Movie Discovery diagram, External API Integrations]
- [Source: _bmad-output/implementation-artifacts/4-2-tmdb-collections-discovery.md — error handling patterns, quality gate order]
- [KinoCheck API: https://api.kinocheck.de/movies?tmdb_id={id} — live response verified 2026-03-24]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- Clippy required collapsing nested `if` + `if let` into single `if ... && let Some(...)` expressions (Rust 2024 edition feature)
- Removed `pub(crate) use kinocheck::KinoCheckDiscoverer;` from `mod.rs` — unused because orchestrators import via `super::kinocheck::KinoCheckDiscoverer`
- `discover_with_library` return type changed from `Result<Vec<VideoSource>, DiscoveryError>` to `Result<(Vec<VideoSource>, Option<u64>), DiscoveryError>` — `ContentDiscoverer` trait impl destructures the tuple

### Completion Notes List

- All 8 tasks completed, all 11 KinoCheck tests passing
- Total test count: 573 (524 lib + 15 main integration + 34 series integration)
- Quality gate passed: build clean, test clean, clippy clean, fmt clean
- Architecture docs updated with KinoCheck API section and stale SourceMode fix

### File List

- `src/discovery/kinocheck.rs` — new file, KinoCheckDiscoverer implementation + 11 tests
- `src/discovery/mod.rs` — added `mod kinocheck;`
- `src/discovery/orchestrator.rs` — added kinocheck field, wired fallback in discover_all
- `src/discovery/series_orchestrator.rs` — added kinocheck field, wired fallback + video_source_to_series_extra helper
- `src/discovery/tmdb.rs` — changed discover_with_library return type to include Option<u64> movie ID
- `src/orchestrator.rs` — created shared Arc<AtomicU32> counter, passed to both orchestrators
- `docs/architecture.md` — added KinoCheck API section, KinoCheck in discovery diagrams, fixed stale SourceMode entry

## Review Findings

### Patched

**P1 — KinoCheck queried with TMDB series ID on a movie-only API**
- File: `src/discovery/series_orchestrator.rs`
- Finding: `https://api.kinocheck.de/movies?tmdb_id={id}` is a movie database. TMDB uses separate ID namespaces for movies and TV series, so querying it with a series ID returns wrong content or 404.
- Fix: Removed the KinoCheck fallback block from `SeriesDiscoveryOrchestrator::discover_all()` entirely. Added explanatory comment. Also removed the now-dead `kinocheck` field, `KinoCheckDiscoverer` import, `kinocheck_request_count` constructor parameters, `video_source_to_series_extra` helper, and `AtomicU32` import. Updated call sites in `orchestrator.rs` and tests.

**P2 — `warn!` fires on every request ≥ 800, not just once**
- File: `src/discovery/kinocheck.rs:82`
- Finding: `if count >= KINOCHECK_WARN_THRESHOLD` fires on every call from 800 onward, flooding logs.
- Fix: Changed to `if count == KINOCHECK_WARN_THRESHOLD` — fires exactly once at the boundary.

**P3 — Network error on 429 retry propagates as `Err` instead of `Ok(vec![])`**
- File: `src/discovery/kinocheck.rs:120`
- Finding: The retry `.send().await.map_err(DiscoveryError::NetworkError)?` used `?` which propagated network errors. NFR10 requires the entire 429 path to return `Ok(vec![])`.
- Fix: Replaced `?` with explicit `match`, returning `Ok(None)` on network error with an `info!` log.

**P4 — `tmdb_found_zero` comment was misleading**
- Files: `src/discovery/orchestrator.rs`, `src/discovery/series_orchestrator.rs`
- Finding: Comment said "TMDB active + found movie + returned 0 videos" but the condition also matches "movie not found on TMDB". The `tmdb_movie_id` being `Some` is what actually guards the "found" case.
- Fix: Updated comment in `orchestrator.rs` to accurately describe the condition. Series orchestrator comment was removed along with the entire KinoCheck block (P1).

**P5 — UTF-8 byte-slice in parse failure snippet can panic on multi-byte chars**
- File: `src/discovery/kinocheck.rs:143`
- Finding: `&text[..text.len().min(200)]` slices by bytes; a response with multi-byte UTF-8 (accented chars, CJK) panics at a non-char boundary.
- Fix: Changed to `text.chars().take(200).collect::<String>()`.

### Deferred

See `_bmad-output/implementation-artifacts/deferred-work.md` — section "Deferred from: code review of 5-1-kinocheck-discoverer-as-tmdb-fallback (2026-03-25)" for D1–D5.

### Quality Gate (post-patch)

- `cargo build` — ✅ clean
- `cargo test` — ✅ 573 tests passing (524 lib + 15 main integration + 34 series integration)
- `cargo clippy -- -D warnings` — ✅ clean
- `cargo fmt -- --check` — ✅ clean
