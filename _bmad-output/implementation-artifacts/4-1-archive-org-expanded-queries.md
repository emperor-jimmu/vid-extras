# Story 4.1: Archive.org Expanded Queries

Status: done

## Story

As a user,
I want Archive.org searched for all movies regardless of release year,
So that I discover DVD extras and making-of content for my entire library, not just pre-2010 films.

## Acceptance Criteria

1. The `dvdextras` collection is queried without the year < 2010 gate — all movies trigger the DVDXtras search (FR13)
2. A `subject:"making of"` query is added for all movies regardless of year (FR14)
3. After this story: `search_general` runs for all years (gate removed), `search_making_of` runs for all years (new), `search_dvdxtras` continues to run for all years (unchanged) — no query strategy is removed, only the year guard on `search_general` is lifted
4. Network timeouts are capped at 30 seconds per API call (NFR9) — already enforced by `reqwest::Client` timeout; verify it applies to all new queries
5. Archive.org failure does not prevent other sources from completing (NFR8) — already enforced by orchestrator error isolation; verify new query paths follow the same pattern
6. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Remove the year < 2010 gate from `search_general` and add it to all-year discovery (AC: #1, #3)
  - [x] 1.1 In `ArchiveOrgDiscoverer::discover()` in `src/discovery/archive.rs`, remove the `if movie.year < 2010` guard around `search_general()` — call it unconditionally for all movies
  - [x] 1.2 Remove the `else` branch that logs "Skipping Archive.org general search for {} - year {} is >= 2010"
  - [x] 1.3 Update the doc comment on `search_general()` from "pre-2010 only" to "all movies" — same for the `ArchiveOrgDiscoverer` struct-level doc comment that says "General movie content for pre-2010 films"
  - [x] 1.4 Update the `prop_archive_general_query_year_based` property test in `archive.rs` — the property should now assert that `search_general` is called for all years, not just pre-2010 (the query itself already includes year, so the test logic is still valid; just verify the test name/comment still makes sense)

- [x] Task 2: Add `subject:"making of"` query for all movies (AC: #2)
  - [x] 2.1 Add a new private method `build_making_of_query(title: &str) -> String` to `ArchiveOrgDiscoverer` that returns a query targeting `subject:"making of"` for the given title: `format!("title:\"{}\" AND subject:\"making of\" AND mediatype:movies", title)`
  - [x] 2.2 Add a new private async method `search_making_of(&self, title: &str) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError>` that calls `execute_search` with the making-of query
  - [x] 2.3 In `discover()`, call `search_making_of(&movie.title).await` unconditionally (all years) and extend `all_docs` with the results — follow the same error-handling pattern as `search_dvdxtras`: log on failure, continue
  - [x] 2.4 Add a unit test `test_build_making_of_query` asserting the query contains `subject:"making of"`, the title, and `mediatype:movies`
  - [x] 2.5 Add a unit test `test_build_making_of_query_no_year` asserting the making-of query does NOT contain a `year:` constraint

- [x] Task 3: Verify existing tests still pass after removing year gate (AC: #1, #3)
  - [x] 3.1 `prop_archive_general_query_year_based` tests `build_general_query()` directly (not the call-site guard) — it does NOT need changes; verify it still passes after the guard removal
  - [x] 3.2 Confirm no test asserts the "Skipping Archive.org general search" debug log message — that log is removed with the else branch; a quick `grep` confirms no test checks for it

- [x] Task 4: Update `docs/architecture.md` to reflect expanded Archive.org scope (AC: #3)
  - [x] 4.1 In `docs/architecture.md`, under the `DiscoveryOrchestrator` diagram, change `ArchiveOrgDiscoverer (Archive.org: pre-2010 movies only)` to `ArchiveOrgDiscoverer (Archive.org: all movies — general, making-of, DVDXtras queries)`
  - [x] 4.2 In `docs/architecture.md`, under "Archive.org" in the External API Integrations section, change `Used for: Movies released before 2010 only` to `Used for: All movies — three query strategies: general EPK/extras, subject:"making of", and DVDXtras collection`

- [x] Task 5: Quality gate (AC: #6)
  - [x] 5.1 Run `cargo build` — fix any errors
  - [x] 5.2 Run `cargo test` — fix any failures
  - [x] 5.3 Run `cargo clippy -- -D warnings` — fix any warnings
  - [x] 5.4 Run `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### Scope — What Changes and What Doesn't

This story is a targeted change to `src/discovery/archive.rs` and `docs/architecture.md`. No other source files need modification:

- `src/discovery/orchestrator.rs` — no change; Archive.org error isolation already works
- `src/discovery/series_orchestrator.rs` — no change; Archive.org is NOT used in series discovery (series orchestrator only uses TMDB + YouTube + TVDB). FR38 ("all new sources apply to both pipelines") does not apply to Archive.org expansion — FR38 is scoped to the new discoverers added in Epics 5, 6, and 8 per the FR coverage map
- `src/models.rs` — no change
- `src/error.rs` — no change

The `ArchiveOrgDiscoverer` struct, `ContentDiscoverer` trait impl, and all helper methods remain structurally unchanged. This story adds one new query method, removes one conditional guard, and updates stale doc comments.

### Current `discover()` Flow (Before This Story)

```rust
// CURRENT (to be changed):
if movie.year < 2010 {
    // search_general — only pre-2010
}
// else: logs "Skipping..."

// search_dvdxtras — all years (already correct)
```

### Target `discover()` Flow (After This Story)

```rust
// AFTER:
// search_general — all years (gate removed)
// search_making_of — all years (new)
// search_dvdxtras — all years (unchanged)
```

### `build_making_of_query` — Exact Format

```rust
fn build_making_of_query(title: &str) -> String {
    format!(
        "title:\"{}\" AND subject:\"making of\" AND mediatype:movies",
        title
    )
}
```

No year constraint — this is intentional per FR14 ("all movies regardless of year").

### `search_making_of` — Error Handling Pattern

Follow the exact same pattern as `search_dvdxtras`:

```rust
async fn search_making_of(&self, title: &str) -> Result<Vec<ArchiveOrgDoc>, DiscoveryError> {
    let query = Self::build_making_of_query(title);
    debug!("Searching Archive.org making-of for: {}", title);
    self.execute_search(&query).await
}
```

In `discover()`, handle errors the same way as `search_dvdxtras`:

```rust
match self.search_making_of(&movie.title).await {
    Ok(docs) => {
        info!("Found {} results from Archive.org making-of for {}", docs.len(), movie);
        all_docs.extend(docs);
    }
    Err(e) => {
        info!("Archive.org making-of search failed for {}: {}", movie, e);
    }
}
```

Use `info!` (not `warn!`) for failures — consistent with existing `search_dvdxtras` error logging in this file.

### Deduplication — Already Handled

The `discover()` method already deduplicates by `identifier` after collecting all docs:

```rust
all_docs.sort_by(|a, b| a.identifier.cmp(&b.identifier));
all_docs.dedup_by(|a, b| a.identifier == b.identifier);
```

This means if the same Archive.org item appears in both the general query and the making-of query, it will be deduplicated automatically. No additional dedup logic needed.

### Timeout — Already Enforced

The `reqwest::Client` is built with `.timeout(std::time::Duration::from_secs(30))` in `ArchiveOrgDiscoverer::new()`. This timeout applies to all HTTP requests made through `self.client`, including the new making-of query. No additional timeout configuration needed.

### Key Code Location

| What | File | Notes |
|---|---|---|
| `discover()` method | `src/discovery/archive.rs` | Remove year gate, add making-of call |
| `build_general_query()` | `src/discovery/archive.rs` | No change to method itself |
| `build_dvdxtras_query()` | `src/discovery/archive.rs` | No change |
| New `build_making_of_query()` | `src/discovery/archive.rs` | Add after `build_dvdxtras_query` |
| New `search_making_of()` | `src/discovery/archive.rs` | Add after `search_dvdxtras` |
| Property tests | `src/discovery/archive.rs` | Update `prop_archive_general_query_year_based` comment if needed |

### What NOT To Do

- Do NOT modify `series_orchestrator.rs` — Archive.org is not used for series discovery
- Do NOT modify `orchestrator.rs` — error isolation already works correctly
- Do NOT add a year constraint to `build_making_of_query` — FR14 explicitly says "all movies regardless of year"
- Do NOT change the `build_general_query` method signature or logic — only the call site guard is removed
- Do NOT add `subject:"making of"` to the existing `build_general_query` — keep queries separate so dedup by identifier handles overlaps cleanly
- Do NOT change the `reqwest::Client` timeout — 30s is already set in `new()`
- Do NOT skip the `docs/architecture.md` update — the doc currently says "pre-2010 movies only" in two places, which will be wrong after this story

### Previous Story Patterns (from Stories 3.1, 3.2)

- Run `cargo build` immediately after any struct/method changes to catch compile errors early
- Quality gate order: build → test → clippy → fmt
- `tokio::process::Command` is used for external commands (not relevant here — no process calls in archive.rs)
- All 546 tests were passing after Story 3.2; this story should not break any existing tests

### Test Guidance

New unit tests to add in `archive.rs`:

```rust
#[test]
fn test_build_making_of_query() {
    let query = ArchiveOrgDiscoverer::build_making_of_query("The Matrix");
    assert!(query.contains("title:\"The Matrix\""));
    assert!(query.contains("subject:\"making of\""));
    assert!(query.contains("mediatype:movies"));
}

#[test]
fn test_build_making_of_query_no_year() {
    let query = ArchiveOrgDiscoverer::build_making_of_query("Inception");
    assert!(!query.contains("year:"));
}
```

Optionally add a property test mirroring `prop_dvdxtras_query_no_year`:

```rust
proptest! {
    #[test]
    fn prop_making_of_query_no_year(title in "[A-Za-z ]{1,50}") {
        let query = ArchiveOrgDiscoverer::build_making_of_query(&title);
        prop_assert!(query.contains("subject:\"making of\""));
        prop_assert!(!query.contains("year:"));
    }
}
```

## References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 4, Story 4.1]
- [Source: _bmad-output/planning-artifacts/prd.md — FR13, FR14, FR38 (FR38 scoped to Epics 5/6/8 per coverage map)]
- [Source: src/discovery/archive.rs — ArchiveOrgDiscoverer, discover(), build_dvdxtras_query(), search_dvdxtras()]
- [Source: src/discovery/orchestrator.rs — Archive.org error isolation pattern]
- [Source: src/discovery/series_orchestrator.rs — Archive.org NOT used for series (no changes needed)]
- [Source: src/models.rs — MovieEntry, VideoSource, SourceType::ArchiveOrg]
- [Source: docs/architecture.md — Archive.org section (two stale "pre-2010 only" references to update)]

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6

### Debug Log References

No issues encountered during implementation.

### Completion Notes List

- Removed `if movie.year < 2010` guard from `discover()` — `search_general` now runs for all movies unconditionally
- Added `build_making_of_query()` and `search_making_of()` methods following existing `search_dvdxtras` pattern
- Added making-of call in `discover()` between general and DVDXtras searches, with `info!`-level error logging
- Updated struct-level and method-level doc comments to reflect "all movies" scope
- Added 2 unit tests (`test_build_making_of_query`, `test_build_making_of_query_no_year`) and 1 property test (`prop_making_of_query_no_year`)
- Updated `docs/architecture.md` in two places: discovery diagram and External API Integrations section
- Verified `prop_archive_general_query_year_based` still passes unchanged (tests `build_general_query` directly, not the call-site guard)
- Confirmed no test referenced the removed "Skipping" log message via grep
- Existing deduplication by `identifier` handles overlapping results across all three query strategies
- 30s timeout already enforced by `reqwest::Client` — applies to new making-of query automatically
- Quality gate: 549 tests pass (500 lib + 15 main integration + 34 series integration), zero clippy warnings, formatting clean

### File List

- `src/discovery/archive.rs` — Modified (removed year gate, added making-of query methods, added tests, updated doc comments)
- `docs/architecture.md` — Modified (updated Archive.org description in two places)


## Change Log

- 2026-03-24: Story 4.1 implemented — removed year < 2010 gate from Archive.org general search, added `subject:"making of"` query strategy, updated architecture docs. All quality gates pass.

## Senior Developer Review (AI)

**Review Date:** 2026-03-24
**Reviewer Model:** Claude Sonnet 4.6
**Outcome:** Changes Requested

### Action Items

- [x] [Review][Patch] Rename `prop_archive_general_query_year_based` — name implies year-gated behavior that no longer exists at the call site; rename to `prop_archive_general_query_includes_year` to reflect what it actually tests [src/discovery/archive.rs:~line 655]
- [x] [Review][Defer] Three sequential HTTP calls with no rate-limit awareness — if Archive.org is down, all three calls fail sequentially before the error is logged; no circuit-breaker or early-exit [src/discovery/archive.rs:discover()] — deferred, pre-existing architecture pattern
- [x] [Review][Defer] Title with embedded double-quotes breaks query syntax — `build_making_of_query` (and pre-existing `build_general_query`, `build_dvdxtras_query`) embed title directly into query string without escaping internal quotes [src/discovery/archive.rs:build_making_of_query()] — deferred, pre-existing in all three query builders
- [x] [Review][Defer] No integration-level test for `discover()` three-query flow — unit tests cover individual query builders but no test asserts all three searches are invoked during a `discover()` call [src/discovery/archive.rs:discover()] — deferred, pre-existing gap

### Review Follow-ups (AI)

- [x] [Review][Patch] Rename `prop_archive_general_query_year_based` to `prop_archive_general_query_includes_year` [src/discovery/archive.rs:~line 655]
