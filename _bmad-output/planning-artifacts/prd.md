---
stepsCompleted: [step-01-init, step-02-discovery, step-02b-vision, step-02c-executive-summary, step-03-success, step-04-journeys, step-05-domain, step-06-innovation, step-07-project-type, step-08-scoping, step-09-functional, step-10-nonfunctional, step-11-polish, step-12-complete]
inputDocuments:
  - docs/project-overview.md
  - docs/architecture.md
  - _bmad-output/planning-artifacts/technical-research-additional-extras-sources.md
documentCounts:
  briefCount: 0
  researchCount: 1
  brainstormingCount: 0
  projectDocsCount: 2
classification:
  projectType: cli_tool
  domain: media_entertainment
  complexity: low-medium
  projectContext: brownfield
workflowType: 'prd'
---

# Product Requirements Document - vid-extras

**Author:** Nimrod
**Date:** 2026-03-24

## Executive Summary

vid-extras is a Rust CLI tool that automates the discovery, downloading, conversion, and organization of supplemental video content (extras) for Jellyfin media libraries. It scans a movie or TV series library, discovers extras from multiple online sources, downloads them via yt-dlp, converts to x265/HEVC with ffmpeg, and places files into Jellyfin-compatible directory structures.

The tool currently supports 3 discovery sources (TMDB, Archive.org, YouTube) and 4 content categories (trailers, featurettes, behind the scenes, deleted scenes). This PRD defines a feature expansion that broadens discovery to 7+ sources (adding Dailymotion, KinoCheck, Vimeo, Bilibili, expanded Archive.org queries, and TMDB Collections) and extends the content taxonomy to 8 categories — matching Jellyfin's full video extras support by adding interviews, shorts, clips, and scenes.

The existing `SourceMode` enum (`All` / `YoutubeOnly`) is replaced by a `--sources` multi-value CLI parameter, giving users fine-grained control over which sources are queried. Sources are organized into priority tiers for deduplication: Tier 1 (TMDB, KinoCheck, TheTVDB) > Tier 2 (Dailymotion, Vimeo, Archive.org) > Tier 3 (YouTube, Bilibili).

### What Makes This Special

Users who download movies and series typically get only the main content — no specials, no commentaries, no trailers. Jellyfin can display a rich set of extras categories, but provides no way to source them. vid-extras bridges that gap by aggregating content from every credible platform and mapping it to Jellyfin's complete video extras taxonomy — 8 categories, each in its proper subfolder.

Movie extras are fragmented across dozens of platforms with no single aggregator. By casting a wide net across tiered sources and applying smart filtering (keyword exclusion, duration validation, title similarity scoring, deduplication by priority), the tool delivers Blu-ray-style completeness without noise or irrelevant content.

## Project Classification

- **Project Type:** CLI Tool — terminal-based, scriptable, config-driven
- **Domain:** Media/Entertainment — Jellyfin media library automation
- **Complexity:** Low-Medium — straightforward domain, but multi-source API integration and external tool orchestration add technical depth
- **Project Context:** Brownfield — fully implemented and production-ready; this PRD covers a feature expansion (new discovery sources, expanded content taxonomy, CLI refactor)

## Success Criteria

### User Success

- A user with a 100+ movie library discovers extras from sources that were previously invisible (Dailymotion, KinoCheck, Vimeo) — content they wouldn't have found with the current 3-source setup
- Extras land in the correct Jellyfin subfolder for all 8 categories — interviews show up in `/interviews`, not dumped into `/featurettes`
- The `--sources` parameter lets users tune their source mix without understanding internals — e.g., `--sources tmdb,youtube` for a quick run, or adding `vimeo,bilibili` for deep coverage
- Deduplication by priority tier prevents the same trailer from being downloaded from both TMDB and YouTube — the higher-quality source wins

### Business Success

- Open-source personal tool — business success = user satisfaction and project completeness
- The feature expansion makes vid-extras the most comprehensive Jellyfin extras automation tool available — no competing tool covers 7+ sources with 8 content categories
- New sources are added without breaking existing workflows — users who never touch `--sources` get the same defaults plus Dailymotion, KinoCheck, and expanded Archive.org for free

### Technical Success

- All new discoverer modules implement the existing `ContentDiscoverer` trait — no changes to the pipeline architecture
- `cargo test` passes with coverage for all new modules (unit + property-based tests)
- `cargo clippy -- -D warnings` clean, `cargo fmt` clean
- No new external dependencies beyond what yt-dlp and the REST APIs provide
- KinoCheck fallback adds < 1 second latency when TMDB returns results (it's skipped entirely)
- Vimeo OAuth token is cached in `config.cfg` — no re-auth on every run

### Measurable Outcomes

- For a typical movie library (50-200 titles), the expanded sources yield 15-30% more unique extras compared to the current 3-source setup
- Zero false-positive downloads from new sources (same keyword exclusion, duration validation, and title matching filters apply)
- All 8 Jellyfin extras folder types are populated when relevant content exists

## User Journeys

### Journey 1: First Run with Expanded Sources (Happy Path)

Nimrod has a Jellyfin library with 150 movies. He's been running vid-extras with the default 3 sources and has decent coverage — trailers and featurettes for most films. He upgrades to the new version.

He runs `extras_fetcher /media/movies` with no extra flags. The default `--sources` now queries TMDB, Archive.org, Dailymotion, and YouTube — plus KinoCheck as automatic fallback when TMDB returns no videos. Every credible source is searched by default; the user doesn't need to configure anything.

For "Dune: Part Two (2024)", TMDB returned trailers but no featurettes. Since TMDB had results, KinoCheck is skipped. Dailymotion surfaces a French-language behind-the-scenes featurette uploaded by the distributor. The tool detects the non-English audio and automatically downloads English subtitles via yt-dlp — the content is usable without manual intervention.

For "The Substance (2024)", TMDB returned zero videos — KinoCheck automatically kicks in and finds 2 official trailers via its TMDB ID lookup. The downloaded files arrive as `10032.mp4` and `10040.mp4` — the tool normalizes these to `Trailer #1.mp4` and `Trailer #2.mp4` before placing them in `/trailers`.

The new `ContentCategory` mapping puts a cast interview into `/interviews` instead of `/featurettes` where it used to land. When Nimrod opens Jellyfin, the extras are organized into proper categories with clean, readable filenames — interviews have their own section for the first time.

He checks the summary output: 23% more extras discovered across the library compared to his last run. No duplicates — the tier system kept the TMDB trailer and skipped the identical YouTube copy.

### Journey 2: Custom Source Selection (Power User)

Nimrod has an anime collection alongside his Western films. He wants to pull extras from Bilibili for his anime titles but doesn't want Bilibili cluttering results for his Western movies.

He runs two passes:
- `extras_fetcher --movies-only /media/movies` — default sources, Western library
- `extras_fetcher --series-only --sources tmdb,youtube,bilibili /media/anime` — adds Bilibili for anime

The `--sources` parameter gives him control without needing separate config files. Bilibili surfaces OVA trailers and behind-the-scenes content for "Solo Leveling" that weren't on YouTube.

### Journey 3: TMDB Collections Discovery

Nimrod has "Iron Man (2008)" in his library. The current TMDB discoverer only fetches videos for that specific movie. With TMDB Collections enabled, the tool detects that Iron Man belongs to the "Iron Man Collection" and also checks sibling movies (Iron Man 2, Iron Man 3) for cross-promotional featurettes.

It finds a "Making of the Iron Man Trilogy" featurette attached to Iron Man 3's TMDB entry that's relevant to the original film. The featurette is tagged and placed in Iron Man's `/featurettes` folder.

### Journey 4: Error Recovery and Graceful Degradation

Nimrod runs vid-extras but Dailymotion's API is temporarily down (HTTP 503). The tool logs a warning: "Dailymotion API unavailable, skipping source" and continues with the remaining sources. No crash, no partial state. The done marker is still written because the other sources completed successfully.

On the next run, Dailymotion is back up. Since the done marker exists, those movies are skipped — unless Nimrod uses `--force` to reprocess and pick up the Dailymotion content he missed.

### Journey Requirements Summary

| Journey | Capabilities Revealed |
|---|---|
| Journey 1 (Happy Path) | Default `--sources` with all always-on sources, KinoCheck fallback, Dailymotion discoverer, expanded ContentCategory enum, tier-based deduplication, correct Jellyfin folder mapping, non-English subtitle auto-download, numeric filename normalization |
| Journey 2 (Power User) | `--sources` multi-value parameter, per-run source selection, Bilibili discoverer |
| Journey 3 (Collections) | TMDB Collections lookup, sibling movie video fetching, cross-movie content tagging |
| Journey 4 (Error Recovery) | Per-source error isolation, graceful degradation logging, done marker behavior with partial source failures |

## CLI Tool Specific Requirements

### Command Structure

The `--sources` parameter replaces the existing `--mode` parameter. KinoCheck is not a standalone source — it activates implicitly as a TMDB fallback when `tmdb` is in the active source list and TMDB returns zero videos.

```
extras_fetcher [OPTIONS] <ROOT_DIRECTORY>

Options:
  --sources <LIST>          Comma-separated source list (default: tmdb,archive,dailymotion,youtube)
                            Supports both --sources tmdb,youtube and --sources tmdb --sources youtube
                            Valid values: tmdb, archive, dailymotion, youtube, vimeo, bilibili
  --dry-run                 Discover extras without downloading — print results per source and exit
  --movies-only             Process only movies
  --series-only             Process only TV series
  --season-extras           Enable season-specific extras
  --specials                Enable Season 0 specials via TheTVDB
  --specials-folder <NAME>  Season 0 folder name (default: Specials)
  --force                   Reprocess completed items
  --concurrency <N>         Parallel processing limit (default: 1)
  --verbose                 Enable debug logging
  -h, --help                Show help
  -V, --version             Show version
```

The `--mode` parameter is removed; `--sources youtube` is equivalent to the old `--mode youtube`. Unknown source names produce a clear validation error via `clap`.

### Configuration Schema

`config.cfg` gains new optional fields for opt-in sources:

```
tmdb_api_key = "..."           # Required (existing)
tvdb_api_key = "..."           # Optional, for --specials (existing)
vimeo_client_id = "..."        # Optional, for --sources vimeo
vimeo_client_secret = "..."    # Optional, for --sources vimeo
cookies_from_browser = "..."   # Optional (existing)
```

Vimeo credentials are prompted on first use of `--sources vimeo`, same pattern as TVDB key prompting with `--specials`.

### Output Formats

Terminal output only (no JSON/file export). The summary display adds source-level statistics:

```
Discovery Summary:
  TMDB:        42 videos found
  KinoCheck:   3 videos found (fallback for 2 movies)
  Dailymotion: 8 videos found
  YouTube:     31 videos found
  Archive.org: 5 videos found
  Duplicates:  12 removed (tier dedup)
  Total:       77 unique videos
```

In `--dry-run` mode, the same summary is displayed but no downloads, conversions, or file organization occur. The pipeline stops after the discovery phase.

### Scripting Support

- Exit code 0 on success, 1 on any failure (unchanged)
- `--verbose` enables debug-level logging to stderr (unchanged)
- `--sources` uses `clap` `value_delimiter = ','` — accepts both comma-separated and repeated flag syntax
- `--dry-run` enables scripted discovery checks without side effects
- Non-interactive by design — all configuration via args, env vars, or config file
- Using the deprecated `--mode` produces a clear error pointing to `--sources`

### Implementation Considerations

- `SourceMode` enum in `models.rs` is replaced by a `Vec<Source>` (ordered, preserving tier priority for deduplication)
- `DiscoveryOrchestrator` iterates over the active source list instead of matching on `SourceMode`
- KinoCheck is orchestrator-internal logic: if `tmdb` is active and returns empty, query KinoCheck — not exposed as a user-facing source
- Each source name is validated against the `Source` enum at parse time — unknown values fail fast with a descriptive error

## Product Scope & Phased Development

### MVP Strategy

Problem-solving MVP — deliver the broadest possible extras coverage with zero additional user setup. A user who upgrades and runs the same command they always have should immediately get more content from more sources, organized into more Jellyfin categories.

Solo developer. All new modules follow the existing `ContentDiscoverer` trait pattern — each is an isolated unit of work with no cross-dependencies between new sources.

### Phase 1 — MVP

Core journeys supported: Journey 1 (Happy Path), Journey 3 (Collections), Journey 4 (Error Recovery).

1. **`--sources` CLI parameter** — replace `SourceMode` enum with multi-value `--sources`; default: `tmdb,archive,dailymotion,youtube`
2. **Expanded `ContentCategory` enum** — add `Interview`, `Short`, `Clip`, `Scene` with Jellyfin folder mappings
3. **`--dry-run` flag** — stop after discovery, print per-source results
4. **Archive.org expanded queries** — remove year gate for `dvdextras`, add `subject:"making of"` for all years
5. **TMDB Collections** — fetch sibling movie videos from same franchise
6. **KinoCheck fallback** — implicit when `tmdb` active and returns empty, lookup by TMDB ID
7. **Dailymotion discoverer** — REST API search, duration/keyword filtering, yt-dlp download
8. **Tier-based deduplication** — prefer higher-tier sources when same content appears from multiple sources
9. **Non-English subtitle auto-download** — detect non-English audio, fetch English subs via yt-dlp `--write-subs`
10. **Numeric filename normalization** — rename opaque filenames (e.g., `10032.mp4`) to `Trailer #1.mp4`

Explicitly out of MVP: Vimeo (requires OAuth app registration), Bilibili (niche audience), category-aware search queries, per-category content limits.

### Phase 2 — Growth

- **Bilibili discoverer** — opt-in via `--sources`, `bilisearch:` operator, same filtering as YouTube
- **Vimeo discoverer** — opt-in via `--sources`, OAuth client_credentials flow, token cached in `config.cfg`
- **Category-aware search queries** — tailor search terms per content category
- **Per-category content limits** — configurable max extras per category to prevent over-downloading

### Phase 3 — Vision

- Plugin architecture for community-contributed discoverers
- Web UI for browsing discovered extras before downloading
- Subtitle/metadata extraction from extras for Jellyfin display

### Risk Mitigation

**Technical Risks:**
- Dailymotion API rate limits are undocumented for unauthenticated reads → mitigate with conservative request pacing (1 req/sec) and optional API key support in `config.cfg`
- KinoCheck free tier is 1,000 req/day → sufficient for most libraries (fallback-only usage), log a warning when approaching limit
- `bilisearch:` yt-dlp operator is unofficial → Phase 2 only, graceful degradation if it breaks

**Resource Risks:**
- Solo developer → MVP is scoped to 10 deliverables, each independently implementable and testable. If time is constrained, items 4-6 (Archive.org, TMDB Collections, KinoCheck) are the lowest-effort highest-value items to ship first.

## Functional Requirements

### Source Management

- FR1: User can specify which discovery sources to query via a `--sources` comma-separated CLI parameter
- FR2: System queries TMDB, Archive.org, Dailymotion, and YouTube by default when no `--sources` parameter is provided
- FR3: User can opt in to Vimeo and Bilibili by adding them to the `--sources` list
- FR4: System validates source names at parse time and rejects unknown values with a descriptive error
- FR5: System produces a clear migration error when the deprecated `--mode` parameter is used, pointing to `--sources`
- FR6: User can combine `--sources` with existing flags (`--series-only`, `--movies-only`, `--specials`) without conflicts

### Dailymotion Discovery

- FR7: System can search Dailymotion's REST API for extras matching a movie or series title
- FR8: System applies the same duration validation and keyword exclusion filters to Dailymotion results as YouTube
- FR9: System downloads Dailymotion videos via yt-dlp using direct video URLs

### KinoCheck Fallback

- FR10: System automatically queries KinoCheck when TMDB is in the active source list and returns zero videos for a title
- FR11: System looks up KinoCheck content using the TMDB movie ID
- FR12: System skips KinoCheck entirely when TMDB returns one or more videos

### Archive.org Expansion

- FR13: System queries Archive.org's `dvdextras` collection for movies of any release year (removing the current < 2010 gate)
- FR14: System adds `subject:"making of"` queries to Archive.org searches for all movies regardless of year

### TMDB Collections

- FR15: System detects when a movie belongs to a TMDB collection (franchise)
- FR16: System fetches video lists from sibling movies in the same TMDB collection
- FR17: System filters collection videos to only include content relevant to the library movie (cross-promotional featurettes, franchise retrospectives)
- FR18: System tags collection-sourced videos with the originating sibling movie for traceability

### Content Taxonomy

- FR19: System maps discovered content to 8 Jellyfin extras categories: Trailer, Featurette, Behind the Scenes, Deleted Scene, Interview, Short, Clip, Scene
- FR20: System places organized files into the correct Jellyfin subfolder for each category (`/trailers`, `/featurettes`, `/behind the scenes`, `/deleted scenes`, `/interviews`, `/shorts`, `/clips`, `/scenes`)
- FR21: System classifies content into the appropriate category using title keywords, source metadata, and TMDB type mappings
- FR22: System maps the existing "Bloopers" TMDB type to the Featurette category (preserving current behavior)

### Deduplication

- FR23: System detects duplicate content across sources using title similarity and duration comparison
- FR24: System resolves duplicates by preferring higher-tier sources (Tier 1: TMDB, KinoCheck, TheTVDB > Tier 2: Dailymotion, Vimeo, Archive.org > Tier 3: YouTube, Bilibili)
- FR25: System reports the number of duplicates removed in the processing summary

### Post-Download Processing

- FR26: System detects non-English audio in downloaded videos and automatically fetches English subtitles via yt-dlp `--write-subs`
- FR27: System renames downloaded files with opaque numeric filenames (e.g., `10032.mp4`) to human-readable names based on their content category (e.g., `Trailer #1.mp4`, `Trailer #2.mp4`)
- FR28: System preserves descriptive original filenames when they already contain meaningful titles
- FR29: System assigns sequential numbering within each category when multiple extras of the same type exist for a title
- FR30: System sanitizes filenames for cross-platform compatibility (Windows-safe characters)

### Dry Run

- FR31: User can run `--dry-run` to execute discovery without downloading, converting, or organizing files
- FR32: System displays per-source discovery results and the deduplication summary in dry-run mode

### Error Handling & Resilience

- FR33: System continues processing remaining sources when any single source fails (per-source error isolation)
- FR34: System logs a warning with the source name and error details when a source is unavailable
- FR35: System writes the done marker when at least one source completes successfully, even if others fail

### Output & Configuration

- FR36: System displays per-source video counts in the processing summary
- FR37: System prompts for Vimeo credentials on first use of `--sources vimeo` and caches them in `config.cfg`

### Cross-Cutting

- FR38: All new discovery sources apply to both movie and TV series processing pipelines
- FR39: System places content that cannot be mapped to any of the 8 defined categories into an `/extras` catch-all subfolder

## Non-Functional Requirements

### Performance

- NFR1: Discovery phase completes within 60 seconds for a single movie when all default sources are queried (excluding network latency beyond the tool's control)
- NFR2: Dailymotion API requests are paced at no more than 1 request per second to avoid undocumented rate limits
- NFR3: KinoCheck API usage stays within the free tier limit of 1,000 requests per day; the system logs a warning when approaching 80% of the limit
- NFR4: Deduplication processing adds no more than 100ms overhead per movie regardless of the number of discovered videos
- NFR5: Dry-run mode completes the full discovery phase without any file I/O beyond logging

### Reliability

- NFR6: The system is idempotent — running the same command twice on the same library produces no duplicate files or side effects
- NFR7: The system recovers gracefully from interrupted runs — partial state (temp directories, incomplete downloads) is cleaned up on the next execution
- NFR8: Any single source failure does not prevent other sources from completing (error isolation)
- NFR9: Network timeouts for any API call are capped at 30 seconds before the system moves on
- NFR10: The system handles API rate-limit responses (HTTP 429) by backing off and retrying once before skipping the source

### Integration

- NFR11: All new discovery sources use yt-dlp as the download backend — no source-specific download implementations
- NFR12: The system works with yt-dlp versions 2025.01+ and ffmpeg versions 6.0+
- NFR13: Vimeo OAuth tokens are cached in `config.cfg` and refreshed automatically when expired
- NFR14: All REST API integrations use HTTPS exclusively
- NFR15: The system degrades gracefully when an external API changes its response format — parsing errors are logged with the raw response snippet for debugging

### Security

- NFR16: API keys and OAuth tokens stored in `config.cfg` are readable only by the file owner (file permissions 600 on Unix systems)
- NFR17: API keys are never logged to stdout or stderr, even in verbose mode
