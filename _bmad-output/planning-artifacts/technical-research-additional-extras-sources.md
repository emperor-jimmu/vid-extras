# Technical Research: Additional Extras Discovery Sources

**Date:** 2026-03-24
**Project:** vid-extras (extras_fetcher)
**Topic:** Candidate sources for broadening extras discovery beyond TMDB, Archive.org, and YouTube

---

## Decisions Recorded

The following design decisions were confirmed during research and apply to all sources below:

1. **New sources are always-on in `All` mode** — no per-source opt-in flags for standard sources. Opt-in only for sources requiring user registration (Vimeo) or niche audiences (Bilibili).
2. **`SourceMode` refactored to a `--sources` multi-value parameter** — e.g., `--sources tmdb,dailymotion,vimeo`. Replaces the current `SourceMode` enum. Enables fine-grained control without a flag explosion.
3. **Source priority order for deduplication and content limits** — sources are ranked by credibility/curation quality. Higher-ranked sources are preferred when the same content appears in multiple sources or when limits are applied:
   - **Tier 1 (highest):** TMDB, KinoCheck, TheTVDB — structured metadata from official/studio-partnered sources
   - **Tier 2:** Dailymotion, Vimeo, Archive.org — platform-hosted official content
   - **Tier 3 (lowest):** YouTube, Bilibili — open search, least curated
4. **KinoCheck as trailer fallback** — queried only when TMDB returns zero video results for a movie.
5. **Duplicate URL handling** — existing URL deduplication in `DiscoveryOrchestrator` is sufficient. Cross-platform duplicates (same video on YouTube and Dailymotion) are handled by the content limit + priority system: the higher-tier source's copy is kept.

---

## Evaluated Sources

### 1. Dailymotion ✅ Always-on in `All` mode

**What it is:** A French video platform with a large catalog of official movie trailers, featurettes, and clips uploaded by studios and distributors. Particularly strong for European and international cinema.

**API access:**
- Public REST API at `https://api.dailymotion.com`
- Video search: `GET /videos?search={query}&fields=id,title,url,duration,description`
- No API key required for read-only public search ([developers.dailymotion.com](https://developers.dailymotion.com/api/platform-api/perform-api-calls/))
- Rate limits undocumented for unauthenticated reads; unrestricted for reasonable usage
- Optional API key available for higher limits (free registration)

**Download feasibility:**
- yt-dlp has a native Dailymotion extractor; direct URL download works reliably
- No `dailymotionsearch:` operator — discovery goes through the REST API, URLs passed to yt-dlp

**Content relevance:** Trailers, featurettes. Less useful for deleted scenes/bloopers. Good for non-English content.

**Implementation:**
```
DailymotionDiscoverer
  → GET https://api.dailymotion.com/videos?search={title}+{year}+trailer&fields=id,title,url,duration
  → filter: duration 30s–20min, keyword exclusion (same list as YouTube)
  → VideoSource { url: "https://www.dailymotion.com/video/{id}", source_type: SourceType::Dailymotion }
  → existing Downloader handles the URL
```

**Complexity:** Low. New module `src/discovery/dailymotion.rs` implementing `ContentDiscoverer`.

**Risks:** Geo-blocking on some content; API terms have changed historically; duplicate content with YouTube.

---

### 2. KinoCheck ✅ Fallback when TMDB returns zero videos

**What it is:** A European movie trailer aggregator with direct studio partnerships (Apple TV+, Netflix, Amazon Prime Video). 80K+ assets, 4K/1080p minimum. Returns YouTube video IDs — no new download path needed.

**API access:**
- Base URL: `https://api.kinocheck.com/`
- Free tier: 1,000 requests/day without a key; higher limits with free registration
- Lookup by TMDB ID: `GET /movies?tmdb={tmdb_id}` — returns categorized YouTube video IDs
- Also supports title search

**Download feasibility:** Returns YouTube URLs → existing downloader handles them transparently.

**Content relevance:** Official trailers, teasers, clips only. No deleted scenes or BTS. Best for recent releases and streaming originals that TMDB misses.

**Implementation:**
```
KinoCheckDiscoverer (called only when TmdbDiscoverer returns empty)
  → GET https://api.kinocheck.com/movies?tmdb={tmdb_id}
  → map YouTube video IDs to VideoSource { url: "https://youtube.com/watch?v={id}", category: Trailer }
  → existing Downloader handles YouTube URLs
```

**Complexity:** Very low. New module `src/discovery/kinocheck.rs`. TMDB ID already on `MovieEntry`.

**Risks:** Overlap with TMDB (mitigated by fallback-only usage); 1,000 req/day limit on large libraries.

---

### 3. Vimeo ✅ Opt-in via `--sources` (requires app registration)

**What it is:** Professional video hosting used by filmmakers and studios for behind-the-scenes content, making-of films, and press materials. Higher production quality than YouTube for this content type.

**API access:**
- REST API at `https://api.vimeo.com`
- Search: `GET /videos?query={title}+{year}+behind+the+scenes&fields=uri,name,duration,link`
- Requires OAuth 2.0 `client_credentials` grant — app-level, no user login needed
- Free app registration at developer.vimeo.com; generates Client ID + Client Secret
- Bearer token is long-lived; cached in `config.cfg` (same pattern as TVDB)

**Download feasibility:**
- yt-dlp supports Vimeo download natively
- Password-protected/domain-restricted videos fail gracefully (non-zero exit code, existing error isolation handles it)

**Content relevance:** Excellent for `BehindTheScenes` and `Featurette`. Less useful for mainstream trailers.

**Implementation:**
```
VimeoDiscoverer
  → on first use: POST /oauth/authorize/client → cache Bearer token in config.cfg
  → GET https://api.vimeo.com/videos?query={title}+{year}+behind+the+scenes
  → filter: duration, keyword exclusion
  → VideoSource { url: "https://vimeo.com/{id}", source_type: SourceType::Vimeo }
  → yt-dlp downloads the URL
```

**Complexity:** Medium. OAuth client_credentials flow (same pattern as TVDB Bearer token already in codebase). New module `src/discovery/vimeo.rs`. Token cached in `config.cfg`.

**Risks:** App registration required (user setup step); Vimeo has been tightening API access; some content domain-restricted.

---

### 4. Internet Archive — Expanded Queries ✅ Modify existing module

**What it is:** The existing `ArchiveOrgDiscoverer` only queries pre-2010 movies with two strategies. Several untapped collections and query patterns exist.

**Untapped opportunities:**
- `dvdextras` collection: dedicated DVD bonus content, not year-gated
- `subject:"making of"` and `subject:"bonus features"` queries for all years
- `collection:dvdxtras` (note: different from `dvdextras`) — already partially queried but can be broadened

**Implementation:**
```
// In existing src/discovery/archive.rs:
// 1. Add dvdextras collection query (remove year gate)
GET /advancedsearch.php?q=title:"{title}"+AND+collection:dvdextras&...

// 2. Relax year restriction for subject-based queries
GET /advancedsearch.php?q=title:"{title}"+AND+subject:"making of"&...
```

**Complexity:** Very low. Modify existing module, no new dependencies.

**Risks:** Minimal. May slightly increase false positives.

---

### 5. Bilibili ✅ Opt-in via `--sources` (niche audience)

**What it is:** China's largest video platform. Extensive anime, Asian cinema, and international film extras. Studios upload official trailers and BTS content in Chinese and Japanese.

**API access:**
- No official public search API
- yt-dlp supports `bilisearch:` search operator — same pattern as `ytsearch:`
- `yt-dlp --dump-json --flat-playlist "bilisearch5:{query}"` works

**Download feasibility:** yt-dlp handles natively. Cookie support already in codebase for higher quality.

**Content relevance:** High value for anime/Asian cinema libraries. Low value for Western content.

**Implementation:**
```
BilibiliDiscoverer
  → yt-dlp --dump-json --flat-playlist "bilisearch5:{title} {year} trailer"
  → same metadata parsing and filtering as YoutubeDiscoverer
  → VideoSource { url, source_type: SourceType::Bilibili }
```

**Complexity:** Very low. Near-identical to `YoutubeDiscoverer`. New module `src/discovery/bilibili.rs`.

**Risks:** Geo-blocking outside China; `bilisearch:` operator is unofficial and may break with yt-dlp updates; title matching unreliable for non-Asian titles.

---

### 6. TMDB Collections ✅ Modify existing module

**What it is:** TMDB's `/collection/{id}` endpoint returns all movies in a franchise. The current discoverer only queries the specific movie's videos. Sibling movies in the same collection may have cross-promotional featurettes.

**Untapped opportunity:**
- `GET /3/movie/{id}` → `belongs_to_collection.id`
- `GET /3/collection/{id}` → list of sibling movie IDs
- Fetch `/videos` for each sibling, tag results as `Featurette`

**Complexity:** Low. Modify existing `src/discovery/tmdb.rs`. No new module.

**Risks:** Large franchises (MCU, etc.) could produce many results — existing content limits handle this.

---

### 7. Odysee / LBRY ❌ Not recommended

**What it is:** A decentralized video platform (blockchain-based). Some creators mirror YouTube content here.

**Why not recommended:**
- No search API; yt-dlp has an Odysee extractor but no `odyseesearch:` operator
- Content catalog is small and not curated for movie extras
- Platform is primarily creator-focused, not studio-focused
- Significant implementation effort for minimal content gain

---

### 8. IMDb Videos ❌ Not recommended

**What it is:** IMDb hosts official trailers and clips on its site, linked to its movie database.

**Why not recommended:**
- IMDb's official API (via AWS Data Exchange) requires a paid subscription and enterprise approval — not viable for an open-source tool
- The unofficial scraping approach is fragile and violates ToS
- Content largely overlaps with TMDB (IMDb and TMDB share video sources)

---

### 9. Letterboxd ❌ Not recommended

**What it is:** A social film discovery platform. Has an API in beta.

**Why not recommended:**
- Letterboxd is a metadata/review platform, not a video host — it doesn't serve video content
- Its API provides film metadata and user lists, not extras or trailers
- Not relevant to this use case

---

### 10. Plex Metadata API ❌ Not recommended

**What it is:** Plex recently opened its metadata API (April 2025). It provides movie/series metadata including trailer links.

**Why not recommended:**
- Requires a Plex account and server — not a dependency we want to introduce
- Trailer links point to YouTube/Vimeo anyway — better to query those sources directly
- Adds a hard dependency on a third-party service the user may not have

---

## Updated Comparative Summary

| Source | Auth Required | Change Type | yt-dlp Download | Content Types | Complexity | Decision |
|---|---|---|---|---|---|---|
| TMDB Collections | Existing key | Modify existing | N/A | Featurettes | Low | ✅ Always-on |
| Archive.org (expanded) | None | Modify existing | Existing | All | Very Low | ✅ Always-on |
| KinoCheck | None (optional key) | New module | Via YouTube URL | Trailers only | Very Low | ✅ TMDB fallback |
| Dailymotion | None (optional key) | New module | Direct URL | Trailers, Featurettes | Low | ✅ Always-on |
| Vimeo | OAuth app reg. | New module | Direct URL | BTS, Featurettes | Medium | ✅ Opt-in |
| Bilibili | None (optional login) | New module | Direct URL | All (Asian content) | Very Low | ✅ Opt-in |
| Odysee/LBRY | None | — | Partial | Misc | High | ❌ Skip |
| IMDb Videos | Paid enterprise | — | N/A | Trailers | N/A | ❌ Skip |
| Letterboxd | OAuth | — | N/A | None (no video) | N/A | ❌ Skip |
| Plex Metadata API | Plex account | — | Via YouTube | Trailers | Medium | ❌ Skip |

---

## Source Priority Tiers (for deduplication and content limits)

When the same content appears from multiple sources, or when per-category limits are applied, sources are preferred in this order:

```
Tier 1 (most credible — structured/official metadata):
  TMDB → KinoCheck → TheTVDB

Tier 2 (platform-hosted official content):
  Dailymotion → Vimeo → Archive.org

Tier 3 (open search — least curated):
  YouTube → Bilibili
```

This replaces the current `SourceType` priority ordering in `DiscoveryOrchestrator::apply_content_limits()`. The `SourceType` enum in `models.rs` needs new variants: `Dailymotion`, `KinoCheck`, `Vimeo`, `Bilibili`.

---

## `--sources` Parameter Design

Replace the current `SourceMode` enum (`All` / `YoutubeOnly`) with a multi-value `--sources` parameter:

```bash
# Default (all always-on sources)
extras_fetcher /media/movies

# Explicit source selection
extras_fetcher --sources tmdb,dailymotion,youtube /media/movies

# Include opt-in sources
extras_fetcher --sources tmdb,dailymotion,youtube,vimeo,bilibili /media/movies

# YouTube only (equivalent to current --mode youtube)
extras_fetcher --sources youtube /media/movies
```

Default value when `--sources` is omitted: `tmdb,archive,dailymotion,kinocheck,youtube`

Opt-in sources not included in default: `vimeo`, `bilibili`

---

## Recommended Implementation Order

1. **Archive.org expanded queries** — modify existing module, zero new dependencies
2. **TMDB Collections** — modify existing module, zero new dependencies
3. **`--sources` parameter refactor** — replace `SourceMode` enum, update CLI and orchestrator
4. **KinoCheck** — new module, very low complexity, no new auth
5. **Dailymotion** — new module, low complexity, no auth
6. **Bilibili** — new module, very low complexity (clone of YouTube pattern)
7. **Vimeo** — new module, medium complexity (OAuth), opt-in

---

*Content was rephrased for compliance with licensing restrictions.*
*Sources: [Dailymotion Developer Docs](https://developers.dailymotion.com/api/platform-api/perform-api-calls/), [KinoCheck API](https://api.kinocheck.com/), [Vimeo Developer Overview](https://help.vimeo.com/hc/en-us/articles/12427697678865-Vimeo-Developer-Overview), [yt-dlp Extractors](https://github.com/yt-dlp/yt-dlp/wiki/Extractors), [Letterboxd API](https://api-docs.letterboxd.com/), [Plex API Announcement](https://www.plex.tv/blog/plex-pro-week-25-api-unlocked/)*
