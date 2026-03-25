# Story 7.2: Deduplication Reporting in Summary

Status: done

## Story

As a user,
I want to see how many duplicates were removed in the processing summary,
So that I understand the value of multi-source discovery and tier-based resolution.

## Acceptance Criteria

1. The total number of duplicates removed is shown in the processing summary (e.g., "Duplicates: 12 removed (tier dedup)") (FR25) — label format matches the PRD output example
2. In `--dry-run` mode, the deduplication summary is included in the dry-run output (FR32)
3. The `ProcessingSummary` struct already has a `duplicates_removed: usize` field (added in Story 7.1) — this story only adds the display logic
4. The `output.rs` summary display includes the deduplication count in the Discovery section
5. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [ ] Task 1: Add dedup count to `display_summary()` in `src/output.rs` (AC: #1, #4)
  - [ ] 1.1 In the Discovery section of `display_summary()` (after the per-source video counts and "Total Discovered" line), add a "Duplicates:" line showing `summary.duplicates_removed` — only display when `duplicates_removed > 0` to avoid noise on runs with no duplicates
  - [ ] 1.2 Format: `"  Duplicates:  {count} removed (tier dedup)"` using `.bright_magenta()` for the count to visually distinguish it from discovery counts — this matches the PRD output example exactly
  - [ ] 1.3 If `source_totals` is empty (discovery was never called), do NOT display the dedup line even if `duplicates_removed > 0` — the dedup line belongs inside the Discovery section

- [ ] Task 2: Add dedup count to dry-run display functions (AC: #2)
  - [ ] 2.1 Update `display_dry_run_movie_results()` signature to accept `duplicates_removed: usize`
  - [ ] 2.2 After the "Total: X videos (would download)" line, add `"    Duplicates:  {count} removed (tier dedup)"` when `duplicates_removed > 0`
  - [ ] 2.3 Update `display_dry_run_series_results()` signature to accept `duplicates_removed: usize`
  - [ ] 2.4 After the "Total: X extras (would download)" line, add `"    Duplicates:  {count} removed (tier dedup)"` when `duplicates_removed > 0`
  - [ ] 2.5 Update all call sites of `display_dry_run_movie_results` and `display_dry_run_series_results` in `src/orchestrator.rs` to pass the `dedup_removed` count

- [ ] Task 3: Update existing tests and add new tests (AC: #1, #2, #5)
  - [ ] 3.1 Update `test_display_dry_run_movie_results_empty` — pass `0` for duplicates_removed
  - [ ] 3.2 Update `test_display_dry_run_movie_results_populated` — pass `0` for duplicates_removed
  - [ ] 3.3 Update `test_display_dry_run_movie_results_with_errors` — pass `0` for duplicates_removed
  - [ ] 3.4 Update `test_display_dry_run_series_results_empty` — pass `0` for duplicates_removed
  - [ ] 3.5 Update `test_display_dry_run_series_results_populated` — pass `0` for duplicates_removed
  - [ ] 3.6 Add `test_display_summary_with_duplicates_removed` — construct `ProcessingSummary` with `duplicates_removed: 12` and non-empty `source_totals`, call `display_summary()`, verify no panic
  - [ ] 3.7 Add `test_display_summary_duplicates_hidden_when_zero` — construct `ProcessingSummary` with `duplicates_removed: 0` and non-empty `source_totals`, call `display_summary()`, verify no panic. NOTE: `test_display_summary_with_source_totals` already covers this scenario with `duplicates_removed: 0` — this new test is only needed if you want an explicit named test for the "hidden when zero" behavior; otherwise skip and rely on the existing test
  - [ ] 3.8 Add `test_display_dry_run_movie_with_duplicates` — call `display_dry_run_movie_results` with `duplicates_removed: 5`, verify no panic
  - [ ] 3.9 Add `test_display_dry_run_series_with_duplicates` — call `display_dry_run_series_results` with `duplicates_removed: 3`, verify no panic

- [ ] Task 4: Quality gate (AC: #5)
  - [ ] 4.1 `cargo build` — fix any errors
  - [ ] 4.2 `cargo test` — fix any failures
  - [ ] 4.3 `cargo clippy -- -D warnings` — fix any warnings
  - [ ] 4.4 `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### Scope — Display Only

This story is purely about display logic in `src/output.rs`. Story 7.1 already:
- Added `duplicates_removed: usize` to `ProcessingSummary`, `MovieResult`, and `SeriesResult`
- Wired the dedup count through the pipeline from `deduplicate()` → orchestrator → summary
- Accumulated counts via `add_movie_result()` / `add_series_result()`

This story reads `summary.duplicates_removed` and prints it. No plumbing changes needed.

### Where to Insert the Dedup Line in `display_summary()`

The Discovery section in `display_summary()` (around line 290–310 of `src/output.rs`) currently looks like:

```
  ─────────────────────────────────────────────
  Discovery:
        TMDB:  42 videos
     ARCHIVE:   3 videos
     YOUTUBE:  18 videos
  Total Discovered:  63 videos
═══════════════════════════════════════════════
```

After this story, when duplicates were removed:

```
  ─────────────────────────────────────────────
  Discovery:
        TMDB:  42 videos
     ARCHIVE:   3 videos
     YOUTUBE:  18 videos
  Total Discovered:  63 videos
  Duplicates:  12 removed (tier dedup)
═══════════════════════════════════════════════
```

Insert the dedup line AFTER the "Total Discovered" `println!` and BEFORE the closing `}` of the `if !summary.source_totals.is_empty()` block. This keeps it inside the Discovery section guard.

### Dry-Run Display — Call Site Updates

The dry-run display functions are called in two places in `src/orchestrator.rs`:

1. `process_movie_standalone()` around line 596:
   ```rust
   output::display_dry_run_movie_results(&movie, &source_results, total);
   ```
   The `dedup_removed` variable is already in scope here (returned from `discover_all()`). Pass it as the new parameter.

2. `process_series_standalone()` around line 802:
   ```rust
   output::display_dry_run_series_results(&series, &series_source_results, total);
   ```
   The `dedup_removed` variable is already in scope here (returned from `discover_series_content()`). Pass it as the new parameter.

### Color Choice

Use `.bright_magenta()` for the dedup count to visually distinguish it from:
- `.bright_cyan()` used for per-source video counts
- `.bright_yellow()` used for total discovered count
- `.green()` used for success counts

### What NOT To Do

- Do NOT modify `ProcessingSummary`, `MovieResult`, or `SeriesResult` — they already have `duplicates_removed` from Story 7.1
- Do NOT modify `deduplication.rs` — the dedup engine is complete
- Do NOT modify `discover_all()` or `discover_series_content()` return types — they already return dedup counts
- Do NOT add any new dependencies
- Do NOT change the dedup algorithm or thresholds
- Do NOT change the dry-run "Total: X videos (would download)" count to post-dedup — the `total` variable at the call site is computed from `source_results` (pre-dedup raw counts per source), which is correct for showing what each source found. The dedup line you're adding shows how many were removed, which gives the user enough info to mentally compute the post-dedup count. Changing the total would require plumbing the post-dedup count separately, which is out of scope.

### Test Count Baseline

595 tests were passing after Story 7.1. This story adds ~4 new tests and updates ~5 existing tests (signature changes only). All existing tests should continue to pass.

### Key Code Locations

| What | File | Notes |
|---|---|---|
| Summary display | `src/output.rs` → `display_summary()` | Add dedup line in Discovery section |
| Dry-run movie display | `src/output.rs` → `display_dry_run_movie_results()` | Add `duplicates_removed` param |
| Dry-run series display | `src/output.rs` → `display_dry_run_series_results()` | Add `duplicates_removed` param |
| Movie dry-run call site | `src/orchestrator.rs` ~line 596 | Pass `dedup_removed` |
| Series dry-run call site | `src/orchestrator.rs` ~line 802 | Pass `dedup_removed` |
| ProcessingSummary struct | `src/orchestrator.rs` ~line 45 | Already has `duplicates_removed` — read only |

### Previous Story Intelligence

From Story 7.1 completion notes:
- `duplicates_removed` field is already wired through the entire pipeline
- `MovieResult::success_with_dedup` and `SeriesResult::success_with_dedup` constructors exist and are used at the dry-run return points
- The `dedup_removed` variable is in scope at both dry-run call sites in the orchestrator
- 595 tests passing, clippy clean, fmt clean
- `FuzzyMatcher` import path is `crate::discovery::FuzzyMatcher` (public re-export) — not relevant to this story but noted for context

### References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 7, Story 7.2]
- [Source: _bmad-output/implementation-artifacts/7-1-duplicate-detection-engine.md — Previous story with pipeline plumbing]
- [Source: src/output.rs — display_summary() line 220, display_dry_run_movie_results() line 141, display_dry_run_series_results() line 180]
- [Source: src/orchestrator.rs — ProcessingSummary line 45, dry-run call sites lines ~596 and ~802]

### Project Structure Notes

- No new files created
- No new modules or dependencies
- Changes confined to `src/output.rs` (display logic + tests) and `src/orchestrator.rs` (2 call site updates)

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.6

### Debug Log References

None — story was already fully implemented.

### Completion Notes List

- All display logic was already in place from a prior session (duplicates_removed param on both dry-run functions, dedup line in display_summary Discovery section, both orchestrator call sites updated)
- 598 tests passing (549 lib + 15 main integration + 34 series integration)
- cargo fmt clean, cargo clippy -D warnings clean

### File List

- src/output.rs (already complete)
- src/orchestrator.rs (already complete)
