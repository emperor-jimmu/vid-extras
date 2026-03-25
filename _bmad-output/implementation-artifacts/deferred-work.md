# Deferred Work

## Deferred from: code review of 3-2-non-english-subtitle-auto-download (2026-03-24)

- ~~`unwrap_or("subtitle")` fallback in `src/downloader.rs:231`~~ **FIXED (2026-03-25)**: Now uses `if let Some(base_name)` — skips subtitle download entirely when stem is unavailable, preventing orphaned subtitle files.

## Deferred from: code review of 4-1-archive-org-expanded-queries (2026-03-24)

- Three sequential HTTP calls with no rate-limit awareness — if Archive.org is down, all three calls (`search_general`, `search_making_of`, `search_dvdxtras`) fail sequentially before the error is surfaced; no circuit-breaker or early-exit. Pre-existing architecture pattern.
- ~~Title with embedded double-quotes breaks query syntax~~ **FIXED (2026-03-25)**: All three query builders (`build_general_query`, `build_dvdxtras_query`, `build_making_of_query`) now escape `"` → `\"` before embedding the title.
- No integration-level test for `discover()` three-query flow — unit tests cover individual query builders in isolation but no test asserts that all three search methods are actually invoked during a `discover()` call. Pre-existing test coverage gap.

## Deferred from: code review of 5-1-kinocheck-discoverer-as-tmdb-fallback (2026-03-25)

- ~~**D1** `youtube_video_id` not validated before URL construction~~ **FIXED (2026-03-25)**: `discover_for_tmdb_id` now validates the ID is non-empty and exactly 11 characters before constructing the URL; logs a warning and returns `Ok(vec![])` on invalid IDs.
- **D2** No circuit-breaker for persistent KinoCheck failures — if KinoCheck returns 401/403/500 on every call (e.g., API deprecation), the discoverer will attempt a request for every eligible movie in the library with no back-off or disable-after-N-failures logic. Pre-existing pattern across all discoverers.
- ~~**D3** 429 retry counter not incremented~~ **FIXED (2026-03-25)**: The retry request after a 429 now calls `fetch_add` on `request_count`, keeping the counter accurate and triggering the 80% warning threshold correctly.
- **D4** `series_id` collision for yearless series — `SeriesDiscoveryOrchestrator` constructs `series_id` as `{title}_{year}` where `year` defaults to `0` for yearless series. Two different yearless series with the same title would share a `series_id`. Pre-existing pattern in the series pipeline.
- **D5** Concurrent sibling tasks can hit TMDB rate limits — `DiscoveryOrchestrator::discover_all` fires TMDB, Archive.org, and YouTube concurrently per movie; when `--concurrency > 1`, multiple movies run simultaneously, multiplying TMDB API calls. No per-source rate limiting. Pre-existing architecture pattern.

## Deferred from: code review of 6-1-dailymotion-rest-api-discoverer (2026-03-25)

- ~~VideoSource→SeriesExtra conversion closure duplicated in `discover_all` and `discover_season_extras`~~ **FIXED (2026-03-25)**: Extracted `video_source_to_series_extra()` free function; both Dailymotion and Vimeo conversion sites now call it.

## Deferred from: code review of 7-1-duplicate-detection-engine (2026-03-25)

- `sources` field on `Orchestrator` duplicates data already held inside `DiscoveryOrchestrator` — added to pass active source list to `SeriesProcessingContext` for dedup. Pre-existing architectural pattern: the series pipeline doesn't have access to `DiscoveryOrchestrator`'s internals. Could be resolved by exposing a `sources()` accessor on `DiscoveryOrchestrator` or passing the list through the series discovery orchestrator.

## Deferred from: code review of 8-1-vimeo-personal-access-token-management (2026-03-25)

- Config loaded multiple times in `main.rs` when multiple optional sources active (TVDB + Vimeo) — pre-existing architectural pattern; fixing requires consolidating all optional-key config loads into a single call.
- `reqwest::Client` allocated unconditionally in `VimeoDiscoverer::new` even when Vimeo is not in the active source list — pre-existing pattern (DailymotionDiscoverer does the same); fixing requires lazy init or `Option<reqwest::Client>`.
- `vimeo_access_token` on public `DiscoveryConfig` is a security-sensitive field exposed as `pub` — pre-existing pattern (all `DiscoveryConfig` fields are `pub`); fixing requires a visibility audit of the entire struct.
- `unwrap_or_default()` on vimeo token in `main.rs` silently passes empty string to `VimeoDiscoverer` if `vimeo_access_token` is somehow `None` after a successful `load_or_create_with_vimeo(true)` call — theoretical only; the prompt function guards against empty input.

## Deferred from: code review of 8-2-vimeo-rest-api-discoverer (2026-03-25)

- Page 1 failure → `Err`; page 2+ failure → `Ok(partial)` behavioral asymmetry in `discover()` — pre-existing pattern from `DailymotionDiscoverer`, consistent with project conventions.
- `test_vimeo_discoverer_new_compiles` comment says "Verify the discoverer is functional" but the test only verifies the error path returns `Ok` — cosmetic, test is functionally correct.
- `make_video` test helper uses `duration` as the numeric suffix in `uri` — confusing if two test videos share the same duration value; test helper only, no production impact.
- Vimeo API `Accept: application/vnd.vimeo.*+json;version=3.4` version header not sent — best practice for API version stability, not required by any AC.
