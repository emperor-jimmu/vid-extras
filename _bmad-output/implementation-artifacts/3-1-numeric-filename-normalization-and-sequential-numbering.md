# Story 3.1: Numeric Filename Normalization and Sequential Numbering

Status: draft

## Story

As a user,
I want downloaded files with opaque numeric names renamed to readable names like `Trailer #1.mp4`,
So that my Jellyfin library has clean, meaningful filenames.

## Acceptance Criteria

1. A file is considered "opaque numeric" when its stem (filename without extension) consists entirely of digits (e.g., `10032.mp4`, `98765.mkv`) — no alphabetic characters present in the stem
2. Opaque numeric filenames are renamed to `{Category} #{N}.{ext}` (e.g., `Trailer #1.mp4`, `Trailer #2.mp4`) where `{Category}` is the `ContentCategory::Display` string and `{N}` is a 1-based sequential counter (FR27, FR29)
3. Sequential numbering is per-category within a single movie or series title — two trailers become `Trailer #1.mp4` and `Trailer #2.mp4`; a featurette in the same run becomes `Featurette #1.mp4` (FR29)
4. Files with descriptive original filenames (stem contains at least one alphabetic character) are preserved as-is — only filename sanitization for Windows compatibility is applied (same logic as `downloader.rs`, replicated in `organizer.rs`) (FR28)
5. All final filenames (both normalized and preserved) are sanitized for Windows compatibility using the existing `Downloader::sanitize_filename()` logic before placement (FR30)
6. Normalization occurs in the `Organizer::organize()` and `SeriesOrganizer::organize_extras()` methods, during the file-move phase, after conversion, when the final destination filename is determined — not during download or conversion
7. The `SeriesOrganizer::organize_specials()` method is NOT affected — specials already use Sonarr-compatible naming (`{Series} - S00E{N} - {title}.mkv`) and must not be changed
8. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [ ] Task 1: Add `Copy + Hash` derives to `ContentCategory` in `src/models.rs` (prerequisite for counter HashMap)
  - [ ] 1.1 Add `Copy` to the `#[derive(...)]` list on `ContentCategory` — it is a fieldless enum so `Copy` is safe and idiomatic
  - [ ] 1.2 Add `Hash` to the same derive list — required for `HashMap<ContentCategory, usize>`
  - [ ] 1.3 Run `cargo build` immediately to confirm no downstream breakage from the new derives

- [ ] Task 2: Add `is_opaque_numeric_filename()` helper function in `src/organizer.rs` (AC: #1)
  - [ ] 1.1 Add a private `fn is_opaque_numeric_filename(path: &Path) -> bool` that returns `true` when the file stem consists entirely of ASCII digits (use `chars().all(|c| c.is_ascii_digit())`)
  - [ ] 1.2 Return `false` for empty stems
  - [ ] 1.3 Add unit test `test_is_opaque_numeric_filename` covering: pure digits (`"10032"` → true), digits with extension (`"10032.mp4"` → true via stem extraction), alphabetic stem (`"trailer"` → false), mixed (`"trailer1"` → false), empty stem edge case

- [ ] Task 2: Add `normalize_filename()` helper function in `src/organizer.rs` (AC: #2, #4, #5)
  - [ ] 2.1 Add a private `fn normalize_filename(path: &Path, category: ContentCategory, counter: usize) -> String` that:
    - If `is_opaque_numeric_filename(path)` → returns `"{category} #{counter}.{ext}"` where `{category}` is `category.to_string()` and `{ext}` is the original extension (lowercased)
    - Otherwise → returns the original filename as-is (descriptive names preserved)
  - [ ] 2.2 Apply `Downloader::sanitize_filename()` to the result in both branches — import or inline the same logic (see note in Dev Notes about avoiding cross-module dependency)
  - [ ] 2.3 Add unit tests covering: opaque numeric → normalized, descriptive → preserved, extension preserved, sanitization applied to both branches

- [ ] Task 3: Thread per-category counters through `Organizer::organize()` (AC: #3, #6)
  - [ ] 3.1 In `Organizer::organize()`, after grouping `files_by_category`, initialize a `HashMap<ContentCategory, usize>` counter map (all starting at 0)
  - [ ] 3.2 In the file-move loop, before calling `move_file()`, increment the counter for the current category and call `normalize_filename()` to determine the final destination filename
  - [ ] 3.3 Update `move_file()` to accept an explicit `dest_filename: &str` parameter instead of deriving the filename from the source path — the caller now controls the final name
  - [ ] 3.4 Update all existing call sites of `move_file()` in `Organizer` to pass the filename (for the non-normalize path, pass the original filename)
  - [ ] 3.5 Add unit test `test_organize_normalizes_numeric_filenames` — create temp dir with two numeric `.mp4` files in the same category, call `organize()`, assert both are renamed to `Trailer #1.mp4` and `Trailer #2.mp4`
  - [ ] 3.6 Add unit test `test_organize_preserves_descriptive_filenames` — create temp dir with a descriptive `.mp4` file, call `organize()`, assert filename is unchanged

- [ ] Task 4: Thread per-category counters through `SeriesOrganizer::organize_extras()` (AC: #3, #6)
  - [ ] 4.1 Apply the same counter + `normalize_filename()` pattern to `SeriesOrganizer::organize_extras()` as done in Task 3 for `Organizer::organize()`
  - [ ] 4.2 Update `SeriesOrganizer::move_file()` to accept `dest_filename: &str` parameter (same change as Task 3.3)
  - [ ] 4.3 Verify `SeriesOrganizer::organize_specials()` is NOT modified — it uses its own Sonarr naming and must remain unchanged
  - [ ] 4.4 Add unit test `test_series_organizer_normalizes_numeric_filenames` mirroring Task 3.5

- [ ] Task 5: Quality gate (AC: #8)
  - [ ] 5.1 Run `cargo build` — fix any errors
  - [ ] 5.2 Run `cargo test` — fix any failures
  - [ ] 5.3 Run `cargo clippy -- -D warnings` — fix any warnings
  - [ ] 5.4 Run `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### Where Normalization Lives

Normalization belongs in the **organizer**, not the downloader or converter. The organizer is the only phase that knows both the final `ContentCategory` (needed for the `{Category} #N` label) and the final destination directory. The downloader already handles hash-suffix removal and sanitization; adding category-aware renaming there would violate SRP.

### `is_opaque_numeric_filename()` — Stem-Only Check

Use `path.file_stem().and_then(|s| s.to_str())` to extract the stem. Check `stem.chars().all(|c| c.is_ascii_digit())`. A stem of `""` (empty) returns `false`. The extension is irrelevant to the check.

Examples:
- `"10032.mp4"` → stem `"10032"` → all digits → `true`
- `"98765"` (no extension) → stem `"98765"` → `true`
- `"trailer.mp4"` → stem `"trailer"` → `false`
- `"trailer1.mp4"` → stem `"trailer1"` → `false` (contains alphabetic)
- `"1trailer.mp4"` → stem `"1trailer"` → `false`

### `normalize_filename()` — Extension Handling

Preserve the original extension. Use `path.extension().and_then(|e| e.to_str()).unwrap_or("mp4")` as fallback. Lowercase the extension for consistency: `.MKV` → `.mkv`.

Output format: `"{} #{}.{}"`, e.g., `"Trailer #1.mp4"`, `"Behind the Scenes #2.mkv"`.

`ContentCategory::to_string()` (via `Display`) returns the human-readable label:
- `Trailer` → `"Trailer"`
- `BehindTheScenes` → `"Behind the Scenes"`
- `DeletedScene` → `"Deleted Scene"`
- `Featurette` → `"Featurette"`
- `Interview` → `"Interview"`
- `Short` → `"Short"`
- `Clip` → `"Clip"`
- `Scene` → `"Scene"`
- `Extras` → `"Extras"`

### Sanitization — Avoid Cross-Module Dependency

`Downloader::sanitize_filename()` is a private `fn` in `downloader.rs`. Do NOT make it `pub` just to call it from `organizer.rs` — that creates an undesirable cross-module coupling.

Instead, add a private `fn sanitize_filename(name: &str) -> String` directly in `organizer.rs` with the same logic. `SeriesOrganizer` already has its own `sanitize_filename()` (line 503) — use that for the series organizer. For `Organizer`, add an equivalent at the `Organizer` impl level or as a module-level private function.

The sanitization logic to replicate (from `downloader.rs`):
```rust
fn sanitize_filename(filename: &str) -> String {
    filename
        .replace(['|', '<', '>', ':', '/', '\\', '*'], "-")
        .replace('"', "'")
        .replace('?', "")
        .replace(['｜', '＜', '＞', '：', '／', '＼', '＊'], "-")
        .replace(['"', '"'], "'")
        .replace('？', "")
}
```

Note: `SeriesOrganizer::sanitize_filename()` (line 503) uses a simpler `chars().map()` approach that only handles ASCII. For consistency with the downloader's Unicode handling, use the `replace`-chain version for `Organizer` too. Do NOT modify `SeriesOrganizer::sanitize_filename()` — it's used by `organize_specials()` and changing it is out of scope. Log this inconsistency to `deferred-work.md` as part of completing this story.

### `move_file()` Signature Change

Both `Organizer::move_file()` and `SeriesOrganizer::move_file()` currently derive the destination filename from `source.file_name()`. Change the signature to accept an explicit `dest_filename: &str`:

```rust
// Before:
async fn move_file(&self, source: &Path, dest_dir: &Path) -> Result<(), OrganizerError>

// After:
async fn move_file(&self, source: &Path, dest_dir: &Path, dest_filename: &str) -> Result<(), OrganizerError>
```

Inside `move_file()`, replace `dest_dir.join(file_name)` with `dest_dir.join(dest_filename)`.

All existing call sites in `organize()` and `organize_extras()` must be updated to pass the filename. For the non-numeric case, pass `source.file_name().and_then(|n| n.to_str()).unwrap_or_default()` (or the sanitized version).

### Counter Initialization and Ordering

The `files_by_category` HashMap does not guarantee iteration order. This is fine — sequential numbering within a category is stable within a single run (files in the same category are numbered in the order they appear in the `Vec<PathBuf>` for that category). The order of categories relative to each other doesn't matter.

Counter map initialization:
```rust
let mut category_counters: HashMap<ContentCategory, usize> = HashMap::new();
```

In the loop, `category` must be copied before the `entry()` call — `entry()` takes ownership, and `category` is needed again for `normalize_filename()`. With `Copy` added in Task 1, this compiles cleanly:
```rust
let counter = category_counters.entry(category).or_insert(0);
*counter += 1;
let n = *counter;
let dest_filename = normalize_filename(&file_path, category, n);
self.move_file(&file_path, &subdir, &dest_filename).await?;
```

Without `Copy` on `ContentCategory`, `entry(category)` would move `category` and the subsequent use in `normalize_filename()` would be a compile error. Task 1 resolves this.

### `--force` Re-run Behavior

On a `--force` re-run, the done marker is ignored and the movie is reprocessed. If `/trailers` already contains `Trailer #1.mp4` from a previous run, the new `Trailer #1.mp4` will overwrite it silently (same behavior as the current `move_file()` which does not check for existing destination files). This is acceptable — `--force` is an explicit user intent to reprocess. No special handling is needed.

### Dry-Run Interaction

Normalization runs in the organizer, which is never reached in `--dry-run` mode (the pipeline stops after discovery per Story 1.3). This is correct by design — dry-run shows what would be discovered, not what filenames would be assigned. No changes to the dry-run path are needed.

### `ContentCategory` — `Copy + Hash` Required

`HashMap<ContentCategory, usize>` requires `ContentCategory: Hash + Eq`. The counter loop also requires `Copy` to avoid a move-after-use compile error (see Counter Initialization section). Both derives are added in Task 1. `Eq` is already derived. `ContentCategory` is a fieldless enum so `Copy` is safe and idiomatic.

### Key Code Locations

| What | File | Line | Notes |
|---|---|---|---|
| `Organizer::organize()` | `src/organizer.rs` | 28 | Add counter map + normalize_filename call here |
| `Organizer::move_file()` | `src/organizer.rs` | 108 | Add `dest_filename: &str` parameter |
| `SeriesOrganizer::organize_extras()` | `src/organizer.rs` | 221 | Add counter map + normalize_filename call here |
| `SeriesOrganizer::move_file()` | `src/organizer.rs` | 455 | Add `dest_filename: &str` parameter |
| `SeriesOrganizer::organize_specials()` | `src/organizer.rs` | 301 | DO NOT MODIFY |
| `SeriesOrganizer::sanitize_filename()` | `src/organizer.rs` | 503 | DO NOT MODIFY — used by specials |
| `ContentCategory::subdirectory()` | `src/models.rs` | 266 | Reference for category → subdir mapping |
| `ContentCategory::Display` | `src/models.rs` | 249 | Use `category.to_string()` for the label |
| `Downloader::sanitize_filename()` | `src/downloader.rs` | 461 | Reference implementation — replicate, don't import |

### Existing Tests That Touch `move_file()`

The following existing tests call `move_file()` directly or indirectly and will need updating when the signature changes:

- `test_move_file_success` (line 554) — calls `move_file()` directly; update to pass filename
- `test_organize_integration` (line 611) — calls `organize()` which calls `move_file()`; verify it still passes
- `test_organize_handles_multiple_categories` (line 708) — same
- `test_series_organizer_organize_series_level_extras` (line 817) — calls `organize_extras()`
- `test_series_organizer_organize_season_specific_extras` (line 846) — same
- `test_series_organizer_multiple_categories` (line 985) — same

These tests use real filenames (not numeric), so they exercise the "preserve descriptive filename" path. They should continue to pass after the change — just update the `move_file()` call in `test_move_file_success` to pass the filename argument.

### What NOT To Do

- Do NOT modify `organize_specials()` — Sonarr naming is intentional and must not be touched
- Do NOT move sanitization logic to `models.rs` — it belongs in the organizer
- Do NOT make `Downloader::sanitize_filename()` public — replicate the logic instead
- Do NOT apply normalization during download or conversion — only in the organizer
- Do NOT number across runs — counters are per-run, per-category; done markers prevent reprocessing

### Previous Story Learnings

- `cargo build` immediately after signature changes catches all call-site errors at once — do this before writing tests
- `SeriesOrganizer` and `Organizer` are separate structs in the same file; changes to one don't automatically apply to the other
- The quality gate order matters: build → test → clippy → fmt

## References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 3, Story 3.1]
- [Source: _bmad-output/planning-artifacts/prd.md — FR27, FR28, FR29, FR30]
- [Source: src/organizer.rs — Organizer::organize(), move_file(), SeriesOrganizer::organize_extras(), organize_specials(), sanitize_filename()]
- [Source: src/models.rs — ContentCategory Display impl, subdirectory(), ConversionResult]
- [Source: src/downloader.rs — sanitize_filename() reference implementation]
