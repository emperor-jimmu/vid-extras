# Deferred Work

## Deferred from: code review of 1-2-refactor-discovery-orchestrator-per-source-error-isolation (2026-03-24)

- Unimplemented sources invisible in `source_results` — deferred to Story 1.4 (per-source summary stats)
- Duplicate `SourceResult` push logic (~12 blocks) — valid code smell, cleanup task
- Empty sources list produces silent no-op — CLI validation prevents this in production
- `SourceResult` allows inconsistent state (`videos_found > 0` with `error`) — only constructed internally, no external risk
- `get_metadata()` failure silently swallowed — pre-existing behavior, not introduced by Story 1.2
- Tests only validate struct construction, not behavioral invariants — behavioral coverage via integration tests
- Duplicate `Source` entries cause duplicate stub warnings — CLI deduplication is Story 1.1 scope
- Single source active + fails → reported as success with 0 downloads — pre-existing, correct behavior (no extras ≠ error)

## Deferred from: code review of 2-1-extend-content-category-enum-and-organizer-mappings (2026-03-24)

- `Clip` and `Scene` doc comments are semantically near-identical ("Movie scene clips" vs "Full scenes from the movie") — pre-existing design ambiguity; Story 2.2 owns classification logic and should clarify the distinction when assigning these categories

## Deferred from: code review of 2-2-expand-category-classification-logic (2026-03-24)

- `series_tmdb.rs` and `tmdb.rs` both define their own `map_tmdb_type()` — duplicated logic that must be kept in sync manually. Pre-existing architectural smell; refactor to a shared function in a future story.

## Deferred from: Story 3.1 — Numeric Filename Normalization (2026-03-24)

- `SeriesOrganizer::sanitize_filename()` (used by `organize_specials()`) uses a simpler `chars().map()` approach that only handles ASCII special characters. The module-level `sanitize_filename()` (used by `Organizer` and `SeriesOrganizer::organize_extras()`) handles both ASCII and Unicode variants (e.g., `｜`, `＜`, `＞`, `：`, `"`, `"`, `？`). This inconsistency means specials filenames won't have Unicode special characters sanitized. Out of scope for Story 3.1 — unify in a future story.

## Deferred from: code review of 3-1-numeric-filename-normalization-and-sequential-numbering (2026-03-24)

- No property test covers numeric normalization behavior — `prop_done_marker_creation_on_completion` uses descriptive filenames (`file_0.mp4` etc.) and never exercises the opaque-numeric rename path. Add a property test in a future story.
- `sanitize_filename` removes `?` entirely while replacing other invalid chars with `-`. An all-`?` filename (e.g., `???.mp4`) would produce `.mp4` — a hidden file on Unix. Pre-existing design choice inherited from `downloader.rs`; unify sanitization policy in a future story.
