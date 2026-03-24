# Story 6.1: Dailymotion REST API Discoverer

Status: done

## Story

As a user,
I want extras discovered from Dailymotion's video library,
So that I get official distributor uploads and content not available on YouTube.

## Acceptance Criteria

1. When Dailymotion is in the active source list, the system searches Dailymotion's REST API (`https://api.dailymotion.com/videos?search={query}&fields=id,title,duration,url`) for extras matching the title
2. The same duration validation (30s–40min) and keyword exclusion filters from `title_matching.rs` are applied to Dailymotion results — reuse `title_matching::contains_excluded_keywords()` and `title_matching::infer_category_from_title()`; replicate the 30–2400s range check inline (it is private in `youtube.rs`)
3. Paginated results are followed using Dailymotion's `page` and `limit` parameters up to a cap of 3 pages to avoid missing relevant content
4. API requests are paced at no more than 1 request per second (NFR2) — use `tokio::time::sleep(Duration::from_secs(1))` between page requests
5. The system handles HTTP 429 rate-limit responses by backing off and retrying once before skipping (NFR10) — same pattern as `KinoCheckDiscoverer::fetch_movie()`
6. Network timeouts are capped at 30 seconds per API call (NFR9) — same `reqwest::Client` builder pattern as `TmdbDiscoverer` and `KinoCheckDiscoverer`
7. All API requests use HTTPS (NFR14)
8. Parsing errors are logged at `warn!` level with the raw response snippet (up to 200 chars, using `text.chars().take(200).collect::<String>()`) for debugging (NFR15)
9. Dailymotion errors are logged with `warn!` and do not prevent other sources from completing (NFR8)
10. The discoverer works for both movie and series discovery pipelines (FR38) — wired into `DiscoveryOrchestrator` and `SeriesDiscoveryOrchestrator`
11. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Create `src/discovery/dailymotion.rs` with `DailymotionDiscoverer` (AC: #1–#9)
  - [x] 1.1 Define private serde structs for the Dailymotion API response (see Dev Notes for exact shape)
  - [x] 1.2 Define `DailymotionDiscoverer` struct with `client: reqwest::Client`; derive `Clone` (needed by `DiscoveryOrchestrator::with_cookies`)
  - [x] 1.3 Implement `DailymotionDiscoverer::new()` — builds `reqwest::Client` with 30s timeout (same pattern as `KinoCheckDiscoverer::new()`)
  - [x] 1.4 Implement `fn build_search_query(title: &str, year: u16) -> String` — returns `"{title} {year}"` as a broad query; Dailymotion's public API does not support boolean OR operators — filtering is handled by `map_video_to_source` after results are returned (URL-encoded by reqwest via `.query()`, not manually)
  - [x] 1.4a Implement `fn build_url(query: &str, page: u32) -> String` — returns the full HTTPS URL string with query params embedded; used by `fetch_page` and directly testable without network calls (see test 5.10)
  - [x] 1.5 Implement `async fn fetch_page(&self, query: &str, page: u32) -> Result<DailymotionResponse, DiscoveryError>` — GET with `search`, `fields`, `page`, `limit=10` params; on HTTP 429 wait 1s and retry once, returning `Ok(DailymotionResponse { list: vec![], has_more: false })` on retry failure (graceful skip, not `Err`) (AC #5); logs parse failures with snippet (AC #8)
  - [x] 1.6 Implement `fn map_video_to_source(video: &DailymotionVideo) -> Option<VideoSource>` — applies duration filter (30–2400s) and keyword exclusion; infers category via `title_matching::infer_category_from_title()`, falling back to `ContentCategory::Extras`; returns `None` if filtered out
  - [x] 1.7 Implement `pub async fn discover(&self, title: &str, year: u16) -> Result<Vec<VideoSource>, DiscoveryError>` — loops up to 3 pages; sleeps 1s between page requests (AC #4); stops early if `has_more == false`; collects all passing videos
  - [x] 1.8 Implement `Default for DailymotionDiscoverer` — delegates to `new()`, consistent with `ArchiveOrgDiscoverer` and `YoutubeDiscoverer`
  - [x] 1.9 Add unit tests (see Task 5)

- [x] Task 2: Register module in `src/discovery/mod.rs` (AC: #10)
  - [x] 2.1 Add `pub(crate) mod dailymotion;` to the module list
  - [x] 2.2 No re-export needed — orchestrators import via `super::dailymotion::DailymotionDiscoverer`

- [x] Task 3: Wire Dailymotion into `DiscoveryOrchestrator` (AC: #1, #9, #10)
  - [x] 3.1 Add `dailymotion: DailymotionDiscoverer` field to `DiscoveryOrchestrator`
  - [x] 3.2 Construct `DailymotionDiscoverer::new()` in both `DiscoveryOrchestrator::new()` and `with_cookies()`
  - [x] 3.3 In `discover_all()`, replace the existing Dailymotion stub `warn!` block with a real invocation: call `self.dailymotion.discover(&movie.title, movie.year).await`, push a `SourceResult`, extend `all_sources`
  - [x] 3.4 On error: log with `warn!`, push `SourceResult` with `error: Some(e.to_string())`, continue (AC #9)

- [x] Task 4: Wire Dailymotion into `SeriesDiscoveryOrchestrator` (AC: #10)
  - [x] 4.1 Add `dailymotion: DailymotionDiscoverer` field to `SeriesDiscoveryOrchestrator`
  - [x] 4.2 Construct `DailymotionDiscoverer::new()` in both `new()` and `new_with_tvdb()`
  - [x] 4.3 In `discover_all()`, replace the existing Dailymotion stub `warn!` block with a real invocation: call `self.dailymotion.discover(&series.title, series.year.unwrap_or(0)).await`
  - [x] 4.4 On error: log with `warn!`, push `SourceResult` with error, continue
  - [x] 4.5 In `discover_season_extras()`, also replace the Dailymotion stub `warn!` block — call `self.dailymotion.discover(&series.title, series.year.unwrap_or(0)).await` with the same error handling pattern; without this, `--season-extras` runs will still log the "not yet implemented" warning for every season

- [x] Task 5: Add tests in `src/discovery/dailymotion.rs` (AC: #1–#9)
  - [x] 5.1 `test_build_search_query_includes_title_and_year` — verify query contains title and year
  - [x] 5.2 `test_map_video_duration_too_short_filtered` — video with `duration: 20` returns `None`
  - [x] 5.3 `test_map_video_duration_too_long_filtered` — video with `duration: 2401` returns `None`
  - [x] 5.4 `test_map_video_duration_valid_included` — video with `duration: 120` returns `Some`
  - [x] 5.5 `test_map_video_excluded_keyword_filtered` — video title containing "Review" returns `None`
  - [x] 5.6 `test_map_video_category_inferred_from_title` — title "Movie Trailer" → `ContentCategory::Trailer`
  - [x] 5.7 `test_map_video_category_fallback_to_extras` — title with no category keywords → `ContentCategory::Extras`
  - [x] 5.8 `test_parse_dailymotion_response_fixture` — parse hardcoded JSON fixture (see Dev Notes), verify `VideoSource` fields
  - [x] 5.9 `test_parse_empty_list_returns_empty_vec` — `{"list": [], "has_more": false}` → empty vec
  - [x] 5.10 `test_url_construction_uses_https` — call `DailymotionDiscoverer::build_url("Inception 2010", 1)` (the private URL-builder extracted in task 1.4a) and assert the result starts with `https://api.dailymotion.com`; this is a pure string test with no network call

- [x] Task 6: Update `docs/architecture.md`
  - [x] 6.1 Add `DailymotionDiscoverer` to the Movie Discovery diagram under `DiscoveryOrchestrator`
  - [x] 6.2 Add Dailymotion API endpoint to the External API Integrations section: `GET https://api.dailymotion.com/videos?search={query}&fields=id,title,duration,url` — no auth required, 1 req/sec rate limit

- [x] Task 7: Quality gate (AC: #11)
  - [x] 7.1 `cargo build` — fix any errors
  - [x] 7.2 `cargo test` — fix any failures
  - [x] 7.3 `cargo clippy -- -D warnings` — fix any warnings
  - [x] 7.4 `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### Dailymotion API Response Shape

The Dailymotion public API requires no authentication. Example request:

```
GET https://api.dailymotion.com/videos?search=Inception+2010+trailer&fields=id,title,duration,url&limit=10&page=1
```

Response shape:

```json
{
  "list": [
    {
      "id": "x7tgad2",
      "title": "Inception Official Trailer",
      "duration": 148,
      "url": "https://www.dailymotion.com/video/x7tgad2"
    }
  ],
  "has_more": true,
  "total": 42
}
```

Key observations:
- `list` is an array of video objects
- `duration` is in seconds (integer)
- `url` is the direct Dailymotion video URL — pass this directly to yt-dlp for download (FR9)
- `has_more` is a boolean — stop pagination when `false`
- No API key required — public API
- Rate limit: undocumented, but 1 req/sec is safe (NFR2)

Serde structs to define (all private):

```rust
#[derive(Debug, Deserialize)]
struct DailymotionResponse {
    list: Vec<DailymotionVideo>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct DailymotionVideo {
    id: String,
    title: String,
    duration: u32,
    url: String,
}
```

### Search Query Construction

Dailymotion's public API uses simple keyword search — it does NOT support boolean OR operators. Using `"trailer OR behind the scenes"` would treat "OR" as a literal keyword. Use a broad `"{title} {year}"` query and rely on `map_video_to_source` filtering to discard irrelevant results:

```rust
fn build_search_query(title: &str, year: u16) -> String {
    if year == 0 {
        title.to_string()
    } else {
        format!("{} {}", title, year)
    }
}
```

The `build_url` helper constructs the full request URL for testability:

```rust
fn build_url(query: &str, page: u32) -> String {
    format!(
        "https://api.dailymotion.com/videos?search={}&fields=id,title,duration,url&limit=10&page={}",
        urlencoding::encode(query),
        page
    )
}
```

Alternatively, use `reqwest`'s `.query(&[...])` builder to let it handle encoding — either approach is fine, but `build_url` must be a pure function for unit testing.

### HTTP Client Pattern

Follow `KinoCheckDiscoverer::new()` exactly:

```rust
pub fn new() -> Self {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("failed to build reqwest client");
    Self { client }
}
```

### Pagination Loop

```rust
pub async fn discover(&self, title: &str, year: u16) -> Result<Vec<VideoSource>, DiscoveryError> {
    let query = Self::build_search_query(title, year);
    let mut all_sources = Vec::new();

    for page in 1u32..=3 {
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
                break; // stop pagination on error, return what we have
            }
        }
    }

    Ok(all_sources)
}
```

### 429 Handling in `fetch_page`

Follow the corrected KinoCheck pattern (post Story 5.1 P3 patch) — on retry failure, return `Ok` with an empty response rather than `Err`. This ensures the pagination loop degrades gracefully instead of surfacing a source error:

```rust
if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
    warn!("Dailymotion rate limited (429), retrying after 1s");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    let retry = match self.client.get(&url).query(&params).send().await {
        Ok(r) => r,
        Err(e) => {
            info!("Dailymotion retry network error: {}", e);
            return Ok(DailymotionResponse { list: vec![], has_more: false });
        }
    };
    if !retry.status().is_success() {
        info!("Dailymotion retry failed with status {}", retry.status());
        return Ok(DailymotionResponse { list: vec![], has_more: false });
    }
    return self.parse_response(retry).await;
}
```

Note: `info!` (not `warn!`) for the retry failure path — consistent with KinoCheck's post-patch behavior and the "non-fatal source failure" logging convention.

### Parse Error Logging

Follow `KinoCheckDiscoverer::parse_response()` exactly — use `text.chars().take(200).collect::<String>()` (NOT `&text[..200]` which panics on multi-byte UTF-8):

```rust
Err(e) => {
    let snippet: String = text.chars().take(200).collect();
    warn!("Dailymotion response parse failed: {}. Raw: {}", e, snippet);
    Err(DiscoveryError::ApiError(format!("Dailymotion parse error: {}", e)))
}
```

### Duration Filter

The YouTube discoverer uses a private `is_duration_valid()` method with range 30–2400s. Replicate inline in `map_video_to_source`:

```rust
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
    })
}
```

### Stub Removal in Orchestrators

Both orchestrators currently have a stub block that logs a warning for unimplemented sources:

```rust
// In DiscoveryOrchestrator::discover_all() and SeriesDiscoveryOrchestrator::discover_all():
for source in &self.sources {
    match source {
        Source::Dailymotion | Source::Vimeo | Source::Bilibili => {
            warn!("{} source requested but discoverer not yet implemented — skipping for {}", source, movie);
        }
        _ => {}
    }
}
```

After this story, `Source::Dailymotion` must be removed from this stub match arm. `Source::Vimeo` and `Source::Bilibili` remain as stubs. The Dailymotion invocation should be added as a proper `if self.sources.contains(&Source::Dailymotion)` block, following the same pattern as TMDB, Archive, and YouTube blocks.

### `DailymotionDiscoverer` Must Be `Clone`

`DiscoveryOrchestrator::with_cookies()` constructs a new `Self` by re-creating fields. `reqwest::Client` is `Clone`, so:

```rust
#[derive(Clone)]
pub(crate) struct DailymotionDiscoverer {
    client: reqwest::Client,
}
```

### Series Pipeline: `year` Handling

`SeriesEntry.year` is `Option<u16>`. Use `series.year.unwrap_or(0)` when calling `self.dailymotion.discover(&series.title, series.year.unwrap_or(0)).await`. A year of `0` produces a slightly less targeted query but is safe — the API will still return results.

### No New Error Variants Needed

Reuse existing `DiscoveryError::ApiError(String)` and `DiscoveryError::NetworkError(reqwest::Error)` — same as `KinoCheckDiscoverer`. Do NOT add `DailymotionError` to `error.rs`.

### `SourceResult` for Dailymotion

Dailymotion IS a user-visible source (unlike KinoCheck). It MUST be added to `source_results` in both orchestrators, following the same pattern as TMDB, Archive, and YouTube:

```rust
source_results.push(SourceResult {
    source: Source::Dailymotion,
    videos_found: sources.len(),
    error: None,
});
```

### Content Limits

`DiscoveryOrchestrator::apply_content_limits()` already handles `SourceType::Dailymotion` with priority `2` (same as Archive.org). No changes needed there.

### Test Count Baseline

573 tests were passing after Story 5.1. This story should not break any existing tests. New tests in `dailymotion.rs` will add to the total.

### Key Code Locations

| What | File | Notes |
|---|---|---|
| New discoverer | `src/discovery/dailymotion.rs` | Create new file |
| Module registration | `src/discovery/mod.rs` | Add `pub(crate) mod dailymotion;` |
| Movie orchestrator | `src/discovery/orchestrator.rs` | Add field, replace stub, wire invocation |
| Series orchestrator | `src/discovery/series_orchestrator.rs` | Add field, replace stub, wire invocation |
| Filtering logic | `src/discovery/title_matching.rs` | Reuse `contains_excluded_keywords`, `infer_category_from_title` |
| Error types | `src/error.rs` | Reuse existing variants — no new variants |
| Models | `src/models.rs` | `SourceType::Dailymotion` already exists |
| Architecture docs | `docs/architecture.md` | Add Dailymotion to diagrams and API section |

### What NOT To Do

- Do NOT add `DailymotionError` to `error.rs` — reuse `DiscoveryError::ApiError` and `DiscoveryError::NetworkError`
- Do NOT implement a custom download path — Dailymotion URLs are passed to yt-dlp exactly like YouTube URLs (NFR11); the existing `Downloader` handles this transparently
- Do NOT add `Source::Dailymotion` to the KinoCheck fallback logic — KinoCheck is TMDB-only
- Do NOT use `&text[..200]` for snippet logging — use `text.chars().take(200).collect::<String>()` to avoid UTF-8 panics (learned from Story 5.1 P5 patch)
- Do NOT use `warn!` for the 429 retry network error — use `info!` (learned from Story 5.1 error handling pattern)
- Do NOT forget to remove `Source::Dailymotion` from the stub match arm in both orchestrators after wiring the real discoverer

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 6, Story 6.1]
- [Source: src/discovery/kinocheck.rs — HTTP client pattern, 429 handling, parse error logging, Clone derive]
- [Source: src/discovery/orchestrator.rs — DiscoveryOrchestrator, discover_all, stub block, apply_content_limits]
- [Source: src/discovery/series_orchestrator.rs — SeriesDiscoveryOrchestrator, discover_all, stub block]
- [Source: src/discovery/youtube.rs — duration filter range (30–2400s), keyword exclusion pattern]
- [Source: src/discovery/title_matching.rs — contains_excluded_keywords, infer_category_from_title]
- [Source: src/models.rs — SourceType::Dailymotion, ContentCategory::Extras, VideoSource, Source::Dailymotion]
- [Source: src/error.rs — DiscoveryError variants]
- [Source: docs/architecture.md — Movie Discovery diagram, External API Integrations]
- [Source: _bmad-output/implementation-artifacts/5-1-kinocheck-discoverer-as-tmdb-fallback.md — P5 UTF-8 snippet fix, error handling patterns]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

- `cargo fmt` fixed two minor formatting issues in `dailymotion.rs` (warn! macro line wrapping, trailing blank line before tests)
- `#[allow(dead_code)]` added to `DailymotionVideo::id` field — used by serde deserialization but not read in Rust code

### Completion Notes List

- All 7 tasks completed, all 11 Dailymotion tests passing
- Total test count: 584 (535 lib + 15 main integration + 34 series integration)
- Quality gate passed: build clean, test clean, clippy clean, fmt clean
- Architecture docs updated with DailymotionDiscoverer in discovery diagram and API section
- Stub blocks in both orchestrators updated: `Source::Dailymotion` removed from stub match arm, replaced with real invocation
- Series pipeline: `discover_all()` and `discover_season_extras()` both wired with VideoSource→SeriesExtra conversion

### File List

- `src/discovery/dailymotion.rs` — new file, DailymotionDiscoverer implementation + 11 tests
- `src/discovery/mod.rs` — added `pub(crate) mod dailymotion;`
- `src/discovery/orchestrator.rs` — added dailymotion field, import, constructor init, real invocation replacing stub
- `src/discovery/series_orchestrator.rs` — added dailymotion field, import, constructor init, real invocation in discover_all and discover_season_extras
- `docs/architecture.md` — added DailymotionDiscoverer to discovery diagram, added Dailymotion API section

## Review Findings

- [x] [Review][Patch] Dailymotion fires identical queries per season in `discover_season_extras`, wasting API calls and producing duplicate extras [`src/discovery/series_orchestrator.rs`] — Removed Dailymotion block from `discover_season_extras` entirely; series-level results from `discover_all` already cover it. Added explanatory comment.
- [x] [Review][Defer] VideoSource→SeriesExtra conversion closure duplicated in `discover_all` and `discover_season_extras` [`src/discovery/series_orchestrator.rs`] — deferred, pre-existing pattern (same duplication exists for YouTube). Could extract a helper fn but the closures differ in `season_number` field.
