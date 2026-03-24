# Deferred Work

## Deferred from: code review of 3-2-non-english-subtitle-auto-download (2026-03-24)

- `unwrap_or("subtitle")` fallback in `src/downloader.rs:231` — if `local_path.file_stem()` returns `None` or non-UTF-8, subtitle `base_name` becomes the literal `"subtitle"` which won't match any video stem during organizer scan, orphaning the subtitle files. Near-zero probability (requires a path with no filename component).

## Deferred from: code review of 4-1-archive-org-expanded-queries (2026-03-24)

- Three sequential HTTP calls with no rate-limit awareness — if Archive.org is down, all three calls (`search_general`, `search_making_of`, `search_dvdxtras`) fail sequentially before the error is surfaced; no circuit-breaker or early-exit. Pre-existing architecture pattern.
- Title with embedded double-quotes breaks query syntax — `build_making_of_query` (and pre-existing `build_general_query`, `build_dvdxtras_query`) embed the movie title directly into the query string without escaping internal double-quote characters. A title like `"Weird Al" Yankovic: The Movie` would produce malformed Archive.org query syntax. Pre-existing in all three query builders.
- No integration-level test for `discover()` three-query flow — unit tests cover individual query builders in isolation but no test asserts that all three search methods are actually invoked during a `discover()` call. Pre-existing test coverage gap.
