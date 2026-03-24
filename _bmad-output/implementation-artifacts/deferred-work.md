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
