# Story 8.2: Vimeo REST API Discoverer

Status: done

## Story

As a user,
I want extras discovered from Vimeo when I opt in via `--sources vimeo`,
So that I get high-quality official content from filmmakers and studios who publish on Vimeo.

## Acceptance Criteria

1. When Vimeo is in the active source list and a valid PAT is available in `config.cfg`, the system searches Vimeo's REST API (`https://api.vimeo.com/videos?query={title}&fields=uri,name,duration,link`) using `Authorization: bearer {token}`
2. The same duration validation (30s–2400s) and keyword exclusion filters from `title_matching.rs` are applied to Vimeo results — reusing `title_matching::contains_excluded_keywords()` and the same range check used by `DailymotionDiscoverer`
3. Network timeouts are capped at 30 seconds per API call (NFR9) — already set in the stub's `reqwest::Client` builder
4. The system handles HTTP 429 rate-limit responses by backing off 1 second and retrying once before skipping (NFR10) — same pattern as `DailymotionDiscoverer::fetch_page()`
5. Parsing errors are logged with the raw response snippet (first 200 chars) for debugging (NFR15)
6. Vimeo errors are logged and do not prevent other sources from completing (NFR8) — the orchestrator wraps the `discover()` call in a `match` block and handles `Err(e)` by logging a warning and continuing; `VimeoDiscoverer::discover()` should propagate errors via `?` and return `Err` on failure, matching the pattern of all other discoverers (`DailymotionDiscoverer`, `TmdbDiscoverer`, etc.)
7. Vimeo videos are downloaded via yt-dlp using their Vimeo URL (NFR11) — `VideoSource.url` must be set to the Vimeo video page URL (the `link` field from the API response), not the API `uri`
8. The discoverer works for both movie and series discovery pipelines (FR38) — `VimeoDiscoverer::discover()` signature is `(&self, title: &str, year: u16)` and is called from both `DiscoveryOrchestrator` and `SeriesDiscoveryOrchestrator` (already wired in Story 8.1)
9. The PAT is never logged to stdout or stderr, even in verbose mode (NFR17) — the token must not appear in any `log::*!` macro call or error message string
10. All Vimeo API requests use HTTPS (NFR14) — the base URL must be `https://api.vimeo.com`
11. `VideoSource` entries produced by `VimeoDiscoverer` use `SourceType::Vimeo` and populate `duration_secs` from the API `duration` field
12. Category is inferred via `title_matching::infer_category_from_title()`, falling back to `ContentCategory::Extras` when no match
13. The `_access_token` and `_client` fields in the stub are renamed to `access_token` and `client` (removing the `_` prefix) now that they are actively used
14. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Implement `VimeoDiscoverer` — replace stub with real API search (AC: #1, #2, #3, #4, #5, #6, #7, #9, #10, #11, #12, #13)
  - [x] 1.1 Add private serde structs `VimeoResponse` and `VimeoVideo` mirroring the Dailymotion pattern:
    - `VimeoResponse { data: Vec<VimeoVideo>, paging: VimeoPaging }` where `VimeoPaging { next: Option<String> }`
    - `VimeoVideo { uri: String, name: String, duration: u32, link: String }`
  - [x] 1.2 Rename `_access_token` → `access_token` and `_client` → `client` in the struct definition and `new()` constructor (AC: #13)
  - [x] 1.3 Add `fn build_url(query: &str, page: u32) -> String` — pure function returning `https://api.vimeo.com/videos?query={encoded}&fields=uri,name,duration,link&per_page=10&page={page}` (AC: #10)
  - [x] 1.4 Add `fn map_video_to_source(video: &VimeoVideo) -> Option<VideoSource>` — applies duration filter (30–2400s), keyword exclusion, category inference, sets `source_type: SourceType::Vimeo`, `url: video.link.clone()`, `duration_secs: Some(video.duration)`, `season_number: None` (AC: #2, #11, #12)
  - [x] 1.5 Add `async fn fetch_page(&self, query: &str, page: u32) -> Result<VimeoResponse, DiscoveryError>` — sends GET with `Authorization: bearer {token}` header, handles 429 with 1s retry, handles non-success status, calls `parse_response()` (AC: #1, #4, #9)
  - [x] 1.6 Add `async fn parse_response(&self, response: reqwest::Response) -> Result<VimeoResponse, DiscoveryError>` — reads body text, logs 200-char snippet on parse failure (AC: #5)
  - [x] 1.7 Replace the stub `discover()` body with real pagination loop: iterate pages 1..=3 (same cap as Dailymotion), call `fetch_page()`, map results via `map_video_to_source()`, stop when `paging.next` is `None` or a page fails (AC: #6)
  - [x] 1.8 Remove the stub `info!` log line from `discover()` and replace with `info!("Vimeo: found {} extras for {} ({})", count, title, year)` after the loop

- [x] Task 2: Add tests (AC: #1–#14)
  - [x] 2.1 `test_build_url_uses_https_and_encodes_query` — asserts URL starts with `https://api.vimeo.com`, contains encoded query, correct page param
  - [x] 2.2 `test_map_video_duration_too_short_filtered` — duration 20s → `None`
  - [x] 2.3 `test_map_video_duration_too_long_filtered` — duration 2401s → `None`
  - [x] 2.4 `test_map_video_duration_valid_included` — duration 120s → `Some(VideoSource)` with `source_type == SourceType::Vimeo`
  - [x] 2.5 `test_map_video_excluded_keyword_filtered` — title containing "Review" → `None`
  - [x] 2.6 `test_map_video_category_inferred_from_title` — title "Official Trailer" → `ContentCategory::Trailer`
  - [x] 2.7 `test_map_video_category_fallback_to_extras` — unrecognized title → `ContentCategory::Extras`
  - [x] 2.8 `test_map_video_url_uses_link_not_uri` — `VideoSource.url` equals `video.link`, not `video.uri`
  - [x] 2.9 `test_map_video_duration_secs_populated` — `VideoSource.duration_secs == Some(video.duration)`
  - [x] 2.10 `test_parse_vimeo_response_fixture` — deserialize a JSON fixture with 2 videos and `paging.next = null`, assert counts and field values
  - [x] 2.11 `test_parse_empty_data_returns_empty_vec` — `{"data": [], "paging": {"next": null}}` → empty list
  - [x] 2.12 `test_vimeo_discoverer_new_compiles` — `VimeoDiscoverer::new("tok")` constructs successfully; assert the discoverer is usable (e.g., call `discover` in a tokio test and assert `Ok` is returned — reuses the existing stub test pattern, now verifying the renamed fields compile without dead-code warnings)

- [x] Task 3: Quality gate (AC: #14)
  - [x] 3.1 `cargo build` — clean
  - [x] 3.2 `cargo test` — 619 passing (570 lib + 15 main integration + 34 series integration), 0 failed
  - [x] 3.3 `cargo clippy -- -D warnings` — clean
  - [x] 3.4 `cargo fmt -- --check` — clean

## Dev Notes

### Vimeo API Response Shape

The Vimeo API uses a `data` array (not `list` like Dailymotion) and a `paging` object for pagination:

```json
{
  "data": [
    {
      "uri": "/videos/123456",
      "name": "Inception - Official Trailer",
      "duration": 148,
      "link": "https://vimeo.com/123456"
    }
  ],
  "paging": {
    "next": "/videos?query=Inception&page=2"
  }
}
```

`paging.next` is `null` when there are no more pages. Use `Option<String>` with `#[serde(default)]`.

### Authorization Header

The PAT is sent as `Authorization: bearer {token}` (lowercase "bearer" per Vimeo docs). Use `.header(reqwest::header::AUTHORIZATION, format!("bearer {}", self.access_token))` on the request builder. Never interpolate the token into log messages.

### URL Field vs URI Field

`uri` is the API path (e.g., `/videos/123456`) — not a playable URL. `link` is the full Vimeo page URL (e.g., `https://vimeo.com/123456`) that yt-dlp can download. Always use `link` for `VideoSource.url`.

### Pagination Cap

Cap at 3 pages (same as Dailymotion) to bound latency. Stop early when `paging.next` is `None`. No explicit inter-page pacing is added for Vimeo — the Vimeo API has per-minute and per-day rate limits on authenticated requests, but these are generous enough for typical library sizes and the 429 retry logic handles the case if a limit is hit.

### PRD vs Epics Discrepancy — OAuth vs PAT

The PRD (`prd.md`) lists `vimeo_client_id` and `vimeo_client_secret` in the config schema and NFR13 specifies OAuth token refresh. The epics document supersedes the PRD for implementation details: Epic 8 / Story 8.1 explicitly specifies a Personal Access Token (PAT) with no OAuth flow and no token refresh, and NFR13 is struck through in the epics. Implemented PAT-only as specified in the epics — same decision as Story 8.1.

### Stub Field Rename

Story 8.1 used `_access_token` and `_client` to suppress dead-code lints. Remove the `_` prefix in this story — the fields are now actively used. No other files reference these private fields directly, so no cascading changes needed.

### Duration Range

Use the same 30–2400s range as `DailymotionDiscoverer` (not the 30–1200s range from the older `YoutubeDiscoverer`). The epics spec says "same duration validation" as `title_matching.rs` — the current shared upper bound across REST API discoverers is 2400s (40 min).

### No New Error Variants Needed

`DiscoveryError::NetworkError(reqwest::Error)` and `DiscoveryError::ApiError(String)` already cover all Vimeo failure modes. No additions to `error.rs` required.

### File Modified

Only `src/discovery/vimeo.rs` changes in this story. The orchestrator wiring was completed in Story 8.1.

## Senior Developer Review (AI)

**Review Date:** 2026-03-25
**Outcome:** Changes Requested
**Layers:** Blind Hunter ✅ | Edge Case Hunter ✅ | Acceptance Auditor ✅
**Dismissed:** 0 findings

### Action Items

- [x] [Review][Patch] Duplicated `Authorization` header in `fetch_page` — extract a private `fn build_request(&self, url: &str) -> reqwest::RequestBuilder` helper to avoid repeating `format!("bearer {}", self.access_token)` in both the initial request and the 429 retry [src/discovery/vimeo.rs:fetch_page]
- [x] [Review][Patch] `test_map_video_url_uses_link_not_uri` negative assertion is vacuously true — `assert_ne!(source.url, "/videos/999")` compares against a hardcoded string but the actual `uri` in the test fixture is `/videos/120` (from `make_video`); change to `assert_ne!(source.url, video.uri)` to actually guard against the bug [src/discovery/vimeo.rs:test_map_video_url_uses_link_not_uri]
- [x] [Review][Patch] `test_build_url_uses_https_and_encodes_query` weak assertion — `url.contains("query=Inception")` is a substring match that passes even if the year is missing; strengthen to `url.contains("query=Inception%202010")` [src/discovery/vimeo.rs:test_build_url_uses_https_and_encodes_query]
- [x] [Review][Patch] Missing boundary tests for duration filter — add `test_map_video_duration_boundary_min` (30s → Some) and `test_map_video_duration_boundary_max` (2400s → Some) [src/discovery/vimeo.rs:tests]
- [x] [Review][Defer] Page 1 failure → `Err`; page 2+ failure → `Ok(partial)` behavioral asymmetry [src/discovery/vimeo.rs:discover] — deferred, pre-existing pattern from DailymotionDiscoverer
- [x] [Review][Defer] `test_vimeo_discoverer_new_compiles` comment overstates what the test proves [src/discovery/vimeo.rs:test_vimeo_discoverer_new_compiles] — deferred, cosmetic only
- [x] [Review][Defer] `make_video` test helper uses duration as uri suffix — confusing if two videos share duration [src/discovery/vimeo.rs:make_video] — deferred, test helper only
- [x] [Review][Defer] Vimeo API `Accept: application/vnd.vimeo.*+json;version=3.4` version header not sent — deferred, best practice not required by any AC

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Completion Notes

Replaced the `VimeoDiscoverer` stub with a full Vimeo REST API implementation mirroring the `DailymotionDiscoverer` pattern. Renamed `_access_token`/`_client` fields, added serde structs for the Vimeo response shape (`data` array + `paging.next`), implemented `build_url`, `map_video_to_source`, `fetch_page` (with 429 retry), `parse_response` (with NFR15 snippet logging), and a paginated `discover()` loop capped at 3 pages. 14 unit tests pass (12 new + 2 existing orchestrator wiring tests). Full suite: 619 tests passing, clippy clean, fmt clean.

### File List

- `src/discovery/vimeo.rs` — replaced stub with full Vimeo REST API discoverer + 14 tests

### Change Log

- 2026-03-25: Implemented Story 8.2 — Vimeo REST API Discoverer (replaced stub, added 12 new tests, quality gate clean)
- 2026-03-25: Addressed code review findings — 4 items resolved: extracted `build_request` helper, fixed vacuous `assert_ne`, strengthened URL encoding assertion, added 2 boundary tests (621 tests passing)
