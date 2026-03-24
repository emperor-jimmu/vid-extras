---
stepsCompleted: [step-01-validate-prerequisites, step-02-design-epics, step-03-create-stories, step-04-final-validation]
inputDocuments:
  - _bmad-output/planning-artifacts/prd.md
  - docs/architecture.md
---

# vid-extras - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for vid-extras, decomposing the requirements from the PRD and Architecture into implementable stories.

## Requirements Inventory

### Functional Requirements

- FR1: User can specify which discovery sources to query via a `--sources` comma-separated CLI parameter
- FR2: System queries TMDB, Archive.org, Dailymotion, and YouTube by default when no `--sources` parameter is provided
- FR3: User can opt in to Vimeo and Bilibili by adding them to the `--sources` list
- FR4: System validates source names at parse time and rejects unknown values with a descriptive error
- FR5: System produces a clear migration error when the deprecated `--mode` parameter is used, pointing to `--sources`
- FR6: User can combine `--sources` with existing flags (`--series-only`, `--movies-only`, `--specials`) without conflicts
- FR7: System can search Dailymotion's REST API for extras matching a movie or series title
- FR8: System applies the same duration validation and keyword exclusion filters to Dailymotion results as YouTube
- FR9: System downloads Dailymotion videos via yt-dlp using direct video URLs
- FR10: System automatically queries KinoCheck when TMDB is in the active source list and returns zero videos for a title
- FR11: System looks up KinoCheck content using the TMDB movie ID
- FR12: System skips KinoCheck entirely when TMDB returns one or more videos
- FR13: System queries Archive.org's `dvdextras` collection for movies of any release year (removing the current < 2010 gate)
- FR14: System adds `subject:"making of"` queries to Archive.org searches for all movies regardless of year
- FR15: System detects when a movie belongs to a TMDB collection (franchise)
- FR16: System fetches video lists from sibling movies in the same TMDB collection
- FR17: System filters collection videos to only include content relevant to the library movie (cross-promotional featurettes, franchise retrospectives)
- FR18: System tags collection-sourced videos with the originating sibling movie for traceability
- FR19: System maps discovered content to 8 Jellyfin extras categories: Trailer, Featurette, Behind the Scenes, Deleted Scene, Interview, Short, Clip, Scene
- FR20: System places organized files into the correct Jellyfin subfolder for each category (`/trailers`, `/featurettes`, `/behind the scenes`, `/deleted scenes`, `/interviews`, `/shorts`, `/clips`, `/scenes`)
- FR21: System classifies content into the appropriate category using title keywords, source metadata, and TMDB type mappings
- FR22: System maps the existing "Bloopers" TMDB type to the Featurette category (preserving current behavior)
- FR23: System detects duplicate content across sources using title similarity and duration comparison
- FR24: System resolves duplicates by preferring higher-tier sources (Tier 1: TMDB, KinoCheck, TheTVDB > Tier 2: Dailymotion, Vimeo, Archive.org > Tier 3: YouTube, Bilibili)
- FR25: System reports the number of duplicates removed in the processing summary
- FR26: System detects non-English audio in downloaded videos and automatically fetches English subtitles via yt-dlp `--write-subs`
- FR27: System renames downloaded files with opaque numeric filenames (e.g., `10032.mp4`) to human-readable names based on their content category (e.g., `Trailer #1.mp4`, `Trailer #2.mp4`)
- FR28: System preserves descriptive original filenames when they already contain meaningful titles
- FR29: System assigns sequential numbering within each category when multiple extras of the same type exist for a title
- FR30: System sanitizes filenames for cross-platform compatibility (Windows-safe characters)
- FR31: User can run `--dry-run` to execute discovery without downloading, converting, or organizing files
- FR32: System displays per-source discovery results and the deduplication summary in dry-run mode
- FR33: System continues processing remaining sources when any single source fails (per-source error isolation)
- FR34: System logs a warning with the source name and error details when a source is unavailable
- FR35: System writes the done marker when at least one source completes successfully, even if others fail
- FR36: System displays per-source video counts in the processing summary
- FR37: System prompts for Vimeo credentials on first use of `--sources vimeo` and caches them in `config.cfg`
- FR38: All new discovery sources apply to both movie and TV series processing pipelines
- FR39: System places content that cannot be mapped to any of the 8 defined categories into an `/extras` catch-all subfolder

### Non-Functional Requirements

- NFR1: Discovery phase completes within 60 seconds for a single movie when all default sources are queried (excluding network latency beyond the tool's control)
- NFR2: Dailymotion API requests are paced at no more than 1 request per second to avoid undocumented rate limits
- NFR3: KinoCheck API usage stays within the free tier limit of 1,000 requests per day; the system logs a warning when approaching 80% of the limit
- NFR4: Deduplication processing adds no more than 100ms overhead per movie regardless of the number of discovered videos
- NFR5: Dry-run mode completes the full discovery phase without any file I/O beyond logging
- NFR6: The system is idempotent — running the same command twice on the same library produces no duplicate files or side effects
- NFR7: The system recovers gracefully from interrupted runs — partial state is cleaned up on the next execution
- NFR8: Any single source failure does not prevent other sources from completing (error isolation)
- NFR9: Network timeouts for any API call are capped at 30 seconds before the system moves on
- NFR10: The system handles API rate-limit responses (HTTP 429) by backing off and retrying once before skipping the source
- NFR11: All new discovery sources use yt-dlp as the download backend — no source-specific download implementations
- NFR12: The system works with yt-dlp versions 2025.01+ and ffmpeg versions 6.0+
- NFR13: Vimeo OAuth tokens are cached in `config.cfg` and refreshed automatically when expired
- NFR14: All REST API integrations use HTTPS exclusively
- NFR15: The system degrades gracefully when an external API changes its response format — parsing errors are logged with the raw response snippet for debugging
- NFR16: API keys and OAuth tokens stored in `config.cfg` are readable only by the file owner (file permissions 600 on Unix systems)
- NFR17: API keys are never logged to stdout or stderr, even in verbose mode

### Additional Requirements

From Architecture:

- `SourceMode` enum in `models.rs` must be replaced by `Vec<Source>` with tier ordering
- `ContentCategory` enum must be extended with 4 new variants: Interview, Short, Clip, Scene
- New discoverer modules must implement the existing `ContentDiscoverer` trait
- `DiscoveryOrchestrator` must iterate over active source list instead of matching on `SourceMode`
- KinoCheck fallback is orchestrator-internal logic — not a separate discoverer in the source list
- New error variants needed in `error.rs` for Dailymotion, KinoCheck, and deduplication errors
- Organizer subdirectory mapping table must be extended for 4 new content categories plus `/extras` catch-all
- `output.rs` summary display must include per-source statistics and deduplication count
- `config.rs` must support new optional fields: `vimeo_client_id`, `vimeo_client_secret`
- Existing `--mode` CLI arg must be removed from `clap` derive with a migration error pointing to `--sources`

### UX Design Requirements

N/A — CLI tool, no UI design document.

### FR Coverage Map

- FR1:  Epic 1 — --sources CLI parameter
- FR2:  Epic 1 — Default source list
- FR3:  Epic 1 — Opt-in sources (Vimeo/Bilibili)
- FR4:  Epic 1 — Source name validation
- FR5:  Epic 1 — --mode migration error
- FR6:  Epic 1 — --sources + existing flags compatibility
- FR7:  Epic 6 — Dailymotion REST API search
- FR8:  Epic 6 — Dailymotion filtering
- FR9:  Epic 6 — Dailymotion yt-dlp download
- FR10: Epic 5 — KinoCheck auto-query
- FR11: Epic 5 — KinoCheck TMDB ID lookup
- FR12: Epic 5 — KinoCheck skip logic
- FR13: Epic 4 — Archive.org year gate removal
- FR14: Epic 4 — Archive.org making-of queries
- FR15: Epic 4 — TMDB collection detection
- FR16: Epic 4 — Sibling movie video fetch
- FR17: Epic 4 — Collection video filtering
- FR18: Epic 4 — Collection video tagging
- FR19: Epic 2 — 8 Jellyfin categories
- FR20: Epic 2 — Correct subfolder placement
- FR21: Epic 2 — Category classification
- FR22: Epic 2 — Bloopers → Featurette mapping
- FR23: Epic 7 — Duplicate detection
- FR24: Epic 7 — Tier-based resolution
- FR25: Epic 7 — Dedup count in summary
- FR26: Epic 3 — Non-English subtitle download
- FR27: Epic 3 — Numeric filename normalization
- FR28: Epic 3 — Descriptive filename preservation
- FR29: Epic 3 — Sequential numbering
- FR30: Epic 3 — Filename sanitization
- FR31: Epic 1 — --dry-run flag
- FR32: Epic 1 — Dry-run output display
- FR33: Epic 1 — Per-source error isolation
- FR34: Epic 1 — Source failure logging
- FR35: Epic 1 — Done marker on partial success
- FR36: Epic 1 — Per-source summary stats
- FR37: Epic 8 — Vimeo credential prompting
- FR38: Epic 5/6/8 — Movie + series pipeline support (per-discoverer AC)
- FR39: Epic 2 — /extras catch-all folder

## Epic List

### Epic 1: CLI Refactor & Source Management Foundation
User can control which discovery sources are queried via the new `--sources` parameter, replacing the old `--mode` flag. The system validates inputs, provides migration guidance, supports `--dry-run` for discovery-only runs, and displays per-source statistics. Error isolation ensures individual source failures don't stop the pipeline.
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6, FR31, FR32, FR33, FR34, FR35, FR36
**NFRs addressed:** NFR5, NFR7, NFR8, NFR16, NFR17

### Epic 2: Expanded Content Taxonomy
User sees extras organized into all 8 Jellyfin categories instead of 4. Interviews land in `/interviews`, shorts in `/shorts`, etc. Uncategorizable content goes to `/extras` as a catch-all.
**FRs covered:** FR19, FR20, FR21, FR22, FR39

### Epic 3: Post-Download Processing
User gets clean, readable filenames and English subtitles for non-English content. Opaque numeric filenames are normalized to `Trailer #1.mp4` etc. Filenames are sanitized for cross-platform compatibility.
**FRs covered:** FR26, FR27, FR28, FR29, FR30
**NFRs addressed:** NFR11

### Epic 4: Archive.org Expansion & TMDB Collections
User discovers more content from existing sources — Archive.org now searches all years (not just pre-2010) and TMDB Collections surfaces franchise-related extras from sibling movies.
**FRs covered:** FR13, FR14, FR15, FR16, FR17, FR18
**NFRs addressed:** NFR8, NFR9

### Epic 5: KinoCheck Fallback
User gets official trailer coverage for movies where TMDB has no videos. KinoCheck is queried automatically as a TMDB fallback using the TMDB movie ID. Works for both movie and series pipelines.
**FRs covered:** FR10, FR11, FR12, FR38 (partial — KinoCheck)
**NFRs addressed:** NFR3, NFR9, NFR14

### Epic 6: Dailymotion Discovery
User discovers extras from Dailymotion — a source with official distributor uploads not found on YouTube. Same filtering rules apply. Downloads via yt-dlp. Works for both movie and series pipelines.
**FRs covered:** FR7, FR8, FR9, FR38 (partial — Dailymotion)
**NFRs addressed:** NFR2, NFR8, NFR9, NFR10, NFR11, NFR14

### Epic 7: Tier-Based Deduplication
User gets clean results with no duplicate extras. When the same content appears from multiple sources, the higher-tier source wins. The summary shows how many duplicates were removed.
**FRs covered:** FR23, FR24, FR25
**NFRs addressed:** NFR4

### Epic 8: Vimeo Discovery (Growth — Post-MVP)
User can opt in to Vimeo as a source. OAuth credentials are prompted on first use and cached. Token refresh is automatic. Works for both movie and series pipelines.
**FRs covered:** FR37, FR38 (partial — Vimeo)
**NFRs addressed:** NFR13, NFR14

**Note:** Bilibili (FR3 partial) is deferred to Phase 2. The `Source::Bilibili` enum variant is shipped in MVP (Epic 1) so the CLI accepts `--sources bilibili`, but no discoverer implementation exists yet — the orchestrator logs a warning and skips it. A dedicated Bilibili epic will be added in Phase 2 planning.


## Epic 1: CLI Refactor & Source Management Foundation

User can control which discovery sources are queried via the new `--sources` parameter, replacing the old `--mode` flag. The system validates inputs, provides migration guidance, supports `--dry-run` for discovery-only runs, and displays per-source statistics. Error isolation ensures individual source failures don't stop the pipeline.

**Note:** NFR1 (discovery phase completes within 60s for a single movie) is a performance target validated via integration testing, not a dedicated story.

### Story 1.1: Replace SourceMode with --sources CLI Parameter and Dynamic Orchestrator

As a user,
I want to specify `--sources tmdb,youtube` on the command line,
So that I can control which discovery sources are queried per run.

**Acceptance Criteria:**

**Given** the existing `SourceMode` enum (All, YoutubeOnly) in `models.rs`
**When** this story is complete
**Then** a new `Source` enum exists with variants: Tmdb, Archive, Dailymotion, Youtube, Vimeo, Bilibili
**And** each `Source` variant has an associated tier (1, 2, or 3) accessible via a `tier()` method
**And** a `default_sources()` function returns `vec![Tmdb, Archive, Dailymotion, Youtube]`
**And** the old `SourceMode` enum is removed from both `models.rs` and `cli.rs`, the `to_models_source_mode()` bridge method is removed, and all references updated to use `Vec<Source>`
**And** `SourceType` enum in `models.rs` is extended with new variants: `Dailymotion`, `KinoCheck`, `Vimeo`, `Bilibili`
**And** `--sources` accepts comma-separated values via `value_delimiter = ','`
**And** `--sources` also accepts repeated flags: `--sources tmdb --sources youtube`
**And** when `--sources` is omitted, the default set (tmdb, archive, dailymotion, youtube) is used
**And** unknown source names produce a descriptive validation error at parse time
**And** `--sources` combines with `--series-only`, `--movies-only`, `--specials` without conflicts
**And** using the deprecated `--mode` flag produces a clear error: "The --mode flag has been removed. Use --sources instead."
**And** the startup validation checks yt-dlp version ≥ 2025.01 and ffmpeg version ≥ 6.0, producing a descriptive error with the detected version and minimum required version if too old (NFR12)
**And** version checks use `yt-dlp --version` and `ffmpeg -version` output parsing
**And** `cargo build` compiles without errors

### Story 1.2: Refactor DiscoveryOrchestrator for Per-Source Error Isolation

As a user,
I want discovery to continue even when one source fails,
So that a Dailymotion outage doesn't prevent me from getting TMDB and YouTube results.

**Acceptance Criteria:**

**Given** the orchestrator currently matches on `SourceMode::All` vs `SourceMode::YoutubeOnly`
**When** this story is complete
**Then** the orchestrator accepts a `Vec<Source>` and iterates over it to invoke discoverers
**And** only discoverers for sources in the active list are invoked
**And** sources without an implemented discoverer (e.g., Bilibili in MVP) are logged as warnings and skipped
**And** the orchestrator uses the active `Vec<Source>` to conditionally invoke concrete discoverer instances (matching the existing pattern of concrete struct fields), not trait objects — new discoverer fields are added in their respective epic stories
**And** `SeriesDiscoveryOrchestrator` is updated to accept `Vec<Source>` and conditionally invoke discoverers, mirroring the movie orchestrator changes; its `new()` and `new_with_tvdb()` constructors are updated accordingly
**And** if a source's discoverer fails, the error is logged with source name and details (FR34)
**And** processing continues with remaining sources (FR33, NFR8)
**And** the done marker is written if at least one source completes successfully (FR35)
**And** existing tests are updated to use the new source list API
**And** `SeriesDiscoveryOrchestrator` is similarly updated

### Story 1.3: Dry-Run Mode

As a user,
I want to run `--dry-run` to see what extras would be discovered without downloading anything,
So that I can preview results and validate source value before committing to a full run.

**Acceptance Criteria:**

**Given** the user runs `extras_fetcher --dry-run /media/movies`
**When** the discovery phase completes
**Then** the pipeline stops after discovery — no downloads, conversions, or file organization occur
**And** no file I/O occurs beyond logging (NFR5)
**And** the per-source discovery results are displayed (FR32)
**And** the done marker is NOT written in dry-run mode

### Story 1.4: Per-Source Summary Statistics and Security Hardening

As a user,
I want to see how many videos each source found in the processing summary,
So that I can understand which sources are contributing value.

**Acceptance Criteria:**

**Given** a processing run completes (normal or dry-run)
**When** the summary is displayed
**Then** each active source shows its video count (e.g., "TMDB: 42 videos found")
**And** the total unique video count is displayed
**And** API keys and OAuth tokens are never logged to stdout or stderr, even in verbose mode (NFR17)
**And** `config.cfg` file permissions are set to 600 on Unix systems when written (NFR16)


## Epic 2: Expanded Content Taxonomy

User sees extras organized into all 8 Jellyfin categories instead of 4. Interviews land in `/interviews`, shorts in `/shorts`, etc. Uncategorizable content goes to `/extras` as a catch-all.

### Story 2.1: Extend ContentCategory Enum and Organizer Mappings

As a user,
I want my extras organized into all 8 Jellyfin video extras categories,
So that interviews, shorts, clips, and scenes each have their own section in Jellyfin.

**Acceptance Criteria:**

**Given** the existing `ContentCategory` enum has 5 variants (Trailer, Featurette, BehindTheScenes, DeletedScene, Interview)
**When** this story is complete
**Then** 4 new variants are added: Short, Clip, Scene, Extras
**And** the organizer maps each new category to the correct Jellyfin subfolder:
  - Short → `/shorts`
  - Clip → `/clips`
  - Scene → `/scenes`
  - Extras → `/extras`
**And** existing 5 category mappings remain unchanged (including Interview → `/interviews`)
**And** the "Bloopers" TMDB type continues to map to Featurette (FR22)
**And** `cargo build` compiles without errors and existing tests pass

### Story 2.2: Expand Category Classification Logic

As a user,
I want discovered content automatically classified into the correct category using title keywords and source metadata,
So that extras land in the right Jellyfin subfolder without manual intervention.

**Acceptance Criteria:**

**Given** a discovered video with title and source metadata
**When** the system classifies the content
**Then** TMDB type mappings are extended: "Interview" → Interview, "Short" → Short, "Clip" → Clip
**And** title keyword matching detects category from video titles (e.g., titles containing "interview" → Interview, "short film" → Short)
**And** content that cannot be mapped to any of the 8 defined categories is placed in `/extras` (FR39)
**And** classification is case-insensitive
**And** the classification logic is shared across all discoverers via `title_matching.rs`


## Epic 3: Post-Download Processing

User gets clean, readable filenames and English subtitles for non-English content. Opaque numeric filenames are normalized to `Trailer #1.mp4` etc. Filenames are sanitized for cross-platform compatibility.

### Story 3.1: Numeric Filename Normalization and Sequential Numbering

As a user,
I want downloaded files with opaque numeric names renamed to readable names like `Trailer #1.mp4`,
So that my Jellyfin library has clean, meaningful filenames.

**Acceptance Criteria:**

**Given** a downloaded file with an opaque numeric filename (e.g., `10032.mp4`)
**When** the organization phase runs
**Then** the file is renamed to `{Category} #{N}.mp4` (e.g., `Trailer #1.mp4`, `Trailer #2.mp4`)
**And** sequential numbering is assigned per category within a single movie/series (FR29)
**And** files with descriptive original filenames (containing alphabetic words beyond just numbers) are preserved as-is (FR28)
**And** all filenames are sanitized for Windows compatibility using the existing sanitization logic (FR30)
**And** the renaming occurs during the organization phase, after conversion, when the final filename is determined for placement into the Jellyfin subfolder

### Story 3.2: Non-English Subtitle Auto-Download

As a user,
I want English subtitles automatically downloaded for non-English extras,
So that foreign-language content is usable without manual subtitle hunting.

**Acceptance Criteria:**

**Given** a video is being downloaded via yt-dlp
**When** the download completes
**Then** the system detects the audio language using yt-dlp's `--dump-json` metadata (`language` field) or ffprobe's audio stream language tag as fallback
**And** if the audio language is not English, yt-dlp is re-invoked with `--write-subs --write-auto-subs --sub-langs en` to fetch English subtitles (manual subs preferred, auto-generated as fallback)
**And** if English subtitles are not available from either source, the download still succeeds (subtitles are best-effort)
**And** subtitle files are placed alongside the video file with matching name
**And** subtitle files are carried through conversion and organization alongside their video
**And** no additional external tools are required beyond yt-dlp and ffprobe (which ships with ffmpeg) (NFR11)


## Epic 4: Archive.org Expansion & TMDB Collections

User discovers more content from existing sources — Archive.org now searches all years (not just pre-2010) and TMDB Collections surfaces franchise-related extras from sibling movies.

### Story 4.1: Archive.org Expanded Queries

As a user,
I want Archive.org searched for all movies regardless of release year,
So that I discover DVD extras and making-of content for my entire library, not just pre-2010 films.

**Acceptance Criteria:**

**Given** a movie of any release year in the library
**When** Archive.org discovery runs
**Then** the `dvdextras` collection is queried without the year < 2010 gate (FR13)
**And** a `subject:"making of"` query is added for all movies regardless of year (FR14)
**And** existing query strategies (EPK, general search) remain active
**And** network timeouts are capped at 30 seconds (NFR9)
**And** Archive.org failure does not prevent other sources from completing (NFR8)

### Story 4.2: TMDB Collections Discovery

As a user,
I want extras from related movies in the same franchise discovered automatically,
So that cross-promotional featurettes and franchise retrospectives appear in my library.

**Acceptance Criteria:**

**Given** a movie in the library (e.g., "Iron Man (2008)")
**When** TMDB discovery runs
**Then** the system checks if the movie belongs to a TMDB collection via the `/3/movie/{id}` endpoint (FR15)
**And** if a collection exists, video lists are fetched from sibling movies via `/3/collection/{id}` then `/3/movie/{sibling_id}/videos` (FR16)
**And** collection videos are filtered by TMDB video type: only "Featurette" and "Behind the Scenes" types are kept from siblings; "Trailer", "Teaser", and "Clip" types for sibling movies are excluded since they promote the sibling, not the library movie (FR17)
**And** each collection-sourced video is tagged with the originating sibling movie title for traceability (FR18)
**And** collection videos are added to the library movie's extras, not the sibling's
**And** network timeouts are capped at 30 seconds per API call (NFR9)


## Epic 5: KinoCheck Fallback

User gets official trailer coverage for movies where TMDB has no videos. KinoCheck is queried automatically as a TMDB fallback using the TMDB movie ID. Works for both movie and series pipelines.

### Story 5.1: KinoCheck Discoverer as TMDB Fallback

As a user,
I want official trailers found automatically for movies where TMDB has no videos,
So that I get trailer coverage even when TMDB's database is incomplete.

**Acceptance Criteria:**

**Given** TMDB is in the active source list and returns zero videos for a movie
**When** the discovery orchestrator detects the empty TMDB result
**Then** KinoCheck is queried using the movie's TMDB ID via `https://api.kinocheck.de/movies?tmdb_id={id}` (FR11)
**And** returned YouTube URLs are added as `VideoSource` entries with appropriate `ContentCategory`
**And** KinoCheck is skipped entirely when TMDB returns one or more videos (FR12)
**And** KinoCheck is not queried when `tmdb` is not in the active source list
**And** all API requests use HTTPS (NFR14)
**And** network timeouts are capped at 30 seconds (NFR9)
**And** the system logs a warning when approaching 80% of the 1,000 req/day free tier limit (NFR3)
**And** KinoCheck errors are logged and do not prevent other sources from completing
**And** the discoverer works for both movie and series discovery pipelines (FR38)


## Epic 6: Dailymotion Discovery

User discovers extras from Dailymotion — a source with official distributor uploads not found on YouTube. Same filtering rules apply. Downloads via yt-dlp. Works for both movie and series pipelines.

### Story 6.1: Dailymotion REST API Discoverer

As a user,
I want extras discovered from Dailymotion's video library,
So that I get official distributor uploads and content not available on YouTube.

**Acceptance Criteria:**

**Given** Dailymotion is in the active source list
**When** discovery runs for a movie or series title
**Then** the system searches Dailymotion's REST API (`https://api.dailymotion.com/videos?search={query}&fields=id,title,duration,url`) for extras matching the title
**And** the same duration validation (30s–20min) and keyword exclusion filters from `title_matching.rs` are applied to Dailymotion results (FR8)
**And** paginated results are followed (using Dailymotion's `page` and `limit` parameters) up to a reasonable cap (e.g., 3 pages) to avoid missing relevant content
**And** API requests are paced at no more than 1 request per second (NFR2)
**And** the system handles HTTP 429 rate-limit responses by backing off and retrying once before skipping (NFR10)
**And** network timeouts are capped at 30 seconds per API call (NFR9)
**And** all API requests use HTTPS (NFR14)
**And** parsing errors are logged with the raw response snippet for debugging (NFR15)
**And** Dailymotion errors are logged and do not prevent other sources from completing (NFR8)
**And** the discoverer works for both movie and series discovery pipelines (FR38)

### Story 6.2: Dailymotion Download via yt-dlp

As a user,
I want Dailymotion videos downloaded using yt-dlp,
So that no source-specific download implementation is needed and I get consistent download behavior.

**Acceptance Criteria:**

**Given** Dailymotion discovery returns video results with direct URLs
**When** the download phase runs
**Then** each Dailymotion video is downloaded via yt-dlp using its `https://www.dailymotion.com/video/{id}` URL (FR9)
**And** yt-dlp is the sole download backend — no Dailymotion-specific download code (NFR11)
**And** download failures are logged and do not prevent other downloads from proceeding
**And** downloaded files follow the existing temp directory and naming conventions


## Epic 7: Tier-Based Deduplication

User gets clean results with no duplicate extras. When the same content appears from multiple sources, the higher-tier source wins. The summary shows how many duplicates were removed.

### Story 7.1: Duplicate Detection Engine

As a user,
I want duplicate extras detected across sources before downloading,
So that I don't waste bandwidth and storage on the same content from multiple platforms.

**Acceptance Criteria:**

**Given** discovery results from multiple sources for a single movie or series
**When** the orchestrator calls the deduplication module (a new phase inserted between discovery and download in the pipeline)
**Then** a `duration_secs: Option<u32>` field is added to `VideoSource` to enable duration-based deduplication; discoverers populate this from API metadata where available
**And** the new title+duration deduplication runs BEFORE the existing URL-based dedup and content limits; URL dedup remains as a final safety net, content limits remain unchanged
**And** the system detects duplicates using title similarity (fuzzy matching with existing `FuzzyMatcher`) and duration comparison (FR23)
**And** two videos are considered duplicates when title similarity ≥ 80% AND duration is within 10% tolerance, OR when title similarity ≥ 95% regardless of duration (to handle re-edits of the same content)
**And** when two videos are considered duplicates, the one from the higher-tier source is kept:
  - Tier 1: TMDB, KinoCheck, TheTVDB
  - Tier 2: Dailymotion, Vimeo, Archive.org
  - Tier 3: YouTube, Bilibili (FR24)
**And** within the same tier, the first source in the active list wins
**And** deduplication processing adds no more than 100ms overhead per movie regardless of video count (NFR4)
**And** the deduplication logic is a standalone module (`deduplication.rs`) that receives `Vec<VideoSource>` and returns a deduplicated `Vec<VideoSource>` plus a count of removed duplicates

### Story 7.2: Deduplication Reporting in Summary

As a user,
I want to see how many duplicates were removed in the processing summary,
So that I understand the value of multi-source discovery and tier-based resolution.

**Acceptance Criteria:**

**Given** deduplication has run for one or more movies/series
**When** the processing summary is displayed
**Then** the total number of duplicates removed is shown (e.g., "Duplicates: 12 removed (tier dedup)") (FR25)
**And** in `--dry-run` mode, the deduplication summary is included in the output (FR32)
**And** the `ProcessingSummary` struct is extended with a `duplicates_removed: usize` field
**And** the `output.rs` summary display includes the deduplication count


## Epic 8: Vimeo Discovery (Growth — Post-MVP)

User can opt in to Vimeo as a source. OAuth credentials are prompted on first use and cached. Token refresh is automatic. Works for both movie and series pipelines.

### Story 8.1: Vimeo OAuth Credential Management

As a user,
I want to be prompted for Vimeo credentials on first use and have them cached,
So that I don't need to re-enter credentials on every run.

**Acceptance Criteria:**

**Given** the user runs with `--sources vimeo` for the first time
**When** no Vimeo credentials exist in `config.cfg`
**Then** the system prompts for `vimeo_client_id` and `vimeo_client_secret` interactively (FR37)
**And** credentials are saved to `config.cfg` with file permissions 600 on Unix systems (NFR16)
**And** on subsequent runs, credentials are loaded from `config.cfg` without prompting
**And** OAuth tokens are obtained via the client_credentials flow and cached in `config.cfg` (NFR13)
**And** expired tokens are refreshed automatically before making API calls (NFR13)
**And** credentials and tokens are never logged to stdout or stderr, even in verbose mode (NFR17)
**And** all Vimeo API requests use HTTPS (NFR14)

### Story 8.2: Vimeo REST API Discoverer

As a user,
I want extras discovered from Vimeo when I opt in via `--sources vimeo`,
So that I get high-quality official content from filmmakers and studios who publish on Vimeo.

**Acceptance Criteria:**

**Given** Vimeo is in the active source list and valid credentials are available
**When** discovery runs for a movie or series title
**Then** the system searches Vimeo's REST API (`https://api.vimeo.com/videos?query={title}&fields=uri,name,duration,link`) using the OAuth bearer token
**And** the same duration validation and keyword exclusion filters from `title_matching.rs` are applied to Vimeo results
**And** network timeouts are capped at 30 seconds per API call (NFR9)
**And** the system handles HTTP 429 rate-limit responses by backing off and retrying once before skipping (NFR10)
**And** parsing errors are logged with the raw response snippet for debugging (NFR15)
**And** Vimeo errors are logged and do not prevent other sources from completing (NFR8)
**And** Vimeo videos are downloaded via yt-dlp using their Vimeo URL (NFR11)
**And** the discoverer works for both movie and series discovery pipelines (FR38)
