# Deferred Work

## Deferred from: code review of 3-2-non-english-subtitle-auto-download (2026-03-24)

- `unwrap_or("subtitle")` fallback in `src/downloader.rs:231` — if `local_path.file_stem()` returns `None` or non-UTF-8, subtitle `base_name` becomes the literal `"subtitle"` which won't match any video stem during organizer scan, orphaning the subtitle files. Near-zero probability (requires a path with no filename component).

## Deferred from: code review of 4-1-archive-org-expanded-queries (2026-03-24)

- Three sequential HTTP calls with no rate-limit awareness — if Archive.org is down, all three calls (`search_general`, `search_making_of`, `search_dvdxtras`) fail sequentially before the error is surfaced; no circuit-breaker or early-exit. Pre-existing architecture pattern.
- Title with embedded double-quotes breaks query syntax — `build_making_of_query` (and pre-existing `build_general_query`, `build_dvdxtras_query`) embed the movie title directly into the query string without escaping internal double-quote characters. A title like `"Weird Al" Yankovic: The Movie` would produce malformed Archive.org query syntax. Pre-existing in all three query builders.
- No integration-level test for `discover()` three-query flow — unit tests cover individual query builders in isolation but no test asserts that all three search methods are actually invoked during a `discover()` call. Pre-existing test coverage gap.

## Deferred from: code review of 5-1-kinocheck-discoverer-as-tmdb-fallback (2026-03-25)

- **D1** `youtube_video_id` not validated before URL construction — `KinoCheckDiscoverer` constructs `https://www.youtube.com/watch?v={youtube_video_id}` without checking that the ID is non-empty or matches the expected 11-char YouTube format. An empty or malformed ID would produce a broken URL that yt-dlp would fail on at download time. Low probability (KinoCheck API is well-formed), but no defensive guard.
- **D2** No circuit-breaker for persistent KinoCheck failures — if KinoCheck returns 401/403/500 on every call (e.g., API deprecation), the discoverer will attempt a request for every eligible movie in the library with no back-off or disable-after-N-failures logic. Pre-existing pattern across all discoverers.
- **D3** 429 retry counter not incremented — the retry request after a 429 response does not call `fetch_add` on `request_count`, so the shared counter under-counts by one per rate-limited request. Minor accuracy issue; does not affect correctness of the 80% warning threshold in practice.
- **D4** `series_id` collision for yearless series — `SeriesDiscoveryOrchestrator` constructs `series_id` as `{title}_{year}` where `year` defaults to `0` for yearless series. Two different yearless series with the same title would share a `series_id`. Pre-existing pattern in the series pipeline.
- **D5** Concurrent sibling tasks can hit TMDB rate limits — `DiscoveryOrchestrator::discover_all` fires TMDB, Archive.org, and YouTube concurrently per movie; when `--concurrency > 1`, multiple movies run simultaneously, multiplying TMDB API calls. No per-source rate limiting. Pre-existing architecture pattern.

## Deferred from: code review of 6-1-dailymotion-rest-api-discoverer (2026-03-25)

- VideoSource→SeriesExtra conversion closure duplicated in `discover_all` and `discover_season_extras` in `src/discovery/series_orchestrator.rs` — pre-existing pattern (same duplication exists for YouTube). Could extract a helper fn but the closures differ in `season_number` field.

## Deferred from: code review of 7-1-duplicate-detection-engine (2026-03-25)

- `sources` field on `Orchestrator` duplicates data already held inside `DiscoveryOrchestrator` — added to pass active source list to `SeriesProcessingContext` for dedup. Pre-existing architectural pattern: the series pipeline doesn't have access to `DiscoveryOrchestrator`'s internals. Could be resolved by exposing a `sources()` accessor on `DiscoveryOrchestrator` or passing the list through the series discovery orchestrator.

## Deferred from: code review of 8-1-vimeo-personal-access-token-management (2026-03-25)

- Config loaded multiple times in `main.rs` when multiple optional sources active (TVDB + Vimeo) — pre-existing architectural pattern; fixing requires consolidating all optional-key config loads into a single call.
- `reqwest::Client` allocated unconditionally in `VimeoDiscoverer::new` even when Vimeo is not in the active source list — pre-existing pattern (DailymotionDiscoverer does the same); fixing requires lazy init or `Option<reqwest::Client>`.
- `vimeo_access_token` on public `DiscoveryConfig` is a security-sensitive field exposed as `pub` — pre-existing pattern (all `DiscoveryConfig` fields are `pub`); fixing requires a visibility audit of the entire struct.
- `unwrap_or_default()` on vimeo token in `main.rs` silently passes empty string to `VimeoDiscoverer` if `vimeo_access_token` is somehow `None` after a successful `load_or_create_with_vimeo(true)` call — theoretical only; the prompt function guards against empty input.
