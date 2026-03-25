# Story 6.2: Dailymotion Download via yt-dlp

Status: done

## Story

As a user,
I want Dailymotion videos downloaded using yt-dlp,
So that no source-specific download implementation is needed and I get consistent download behavior.

## Acceptance Criteria

1. Each Dailymotion video is downloaded via yt-dlp using its `https://www.dailymotion.com/video/{id}` URL (FR9)
2. yt-dlp is the sole download backend — no Dailymotion-specific download code (NFR11)
3. Download failures are logged and do not prevent other downloads from proceeding
4. Downloaded files follow the existing temp directory and naming conventions

## Tasks / Subtasks

- [x] Task 1: Verify existing Downloader handles Dailymotion URLs (AC: #1, #2, #3, #4)
  - [x] 1.1 Add integration test `test_dailymotion_url_flows_through_downloader` in `src/downloader.rs` — construct a `VideoSource` with `source_type: SourceType::Dailymotion` and a `https://www.dailymotion.com/video/x7tgad2` URL, call `download_single` against a temp dir, verify the yt-dlp command is constructed identically to YouTube URLs (same args, same output template, same timeout). Since yt-dlp is an external binary, this test should verify the `Command` construction path, not actually execute the download. Use a non-existent URL and assert the `DownloadResult` has `success: false` with an error message (confirming yt-dlp was invoked, not a Dailymotion-specific path).
  - [x] 1.2 Add integration test `test_dailymotion_download_failure_does_not_stop_pipeline` — create a `Vec<VideoSource>` with 3 entries (YouTube, Dailymotion with bad URL, YouTube), call `download_all`, verify all 3 produce `DownloadResult` entries (the Dailymotion one with `success: false`, confirming error isolation per AC #3)
  - [x] 1.3 Add integration test `test_dailymotion_uses_temp_dir_conventions` — verify that `create_temp_dir` produces the same `tmp_downloads/{movie_id}/` path structure regardless of `SourceType`, confirming AC #4

- [x] Task 2: Quality gate (AC: all)
  - [x] 2.1 `cargo build` — fix any errors
  - [x] 2.2 `cargo test` — fix any failures
  - [x] 2.3 `cargo clippy -- -D warnings` — fix any warnings
  - [x] 2.4 `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### This Story Is a Verification Story, Not an Implementation Story

The existing `Downloader::download_single()` in `src/downloader.rs` already handles Dailymotion URLs transparently. It invokes `yt-dlp -o {template} {url} --no-playlist --quiet` for ANY `VideoSource.url` — there is no source-type branching in the download path. yt-dlp natively supports Dailymotion URLs (`https://www.dailymotion.com/video/{id}`).

Story 6.1 already wired `DailymotionDiscoverer` into both orchestrators, producing `VideoSource` entries with `url: "https://www.dailymotion.com/video/{id}"`. These flow into `Downloader::download_all()` identically to YouTube URLs.

The purpose of this story is to add tests that explicitly verify this behavior for Dailymotion URLs, confirming NFR11 ("no source-specific download implementations").

### What the Downloader Already Does

`Downloader::download_single()` (line 147 of `src/downloader.rs`):
1. Generates a URL hash for unique filenames
2. Builds `yt-dlp -o "%(title)s_{hash}.%(ext)s" {url} --no-playlist --quiet --no-warnings`
3. Adds `--cookies-from-browser` if configured
4. Adds `--windows-filenames` on Windows
5. Executes with configurable timeout (default 5 min)
6. On success: finds downloaded file, detects audio language, downloads subtitles if non-English
7. On failure: logs error, cleans up partial files, returns `DownloadResult { success: false }`

None of this is source-specific. Dailymotion URLs work identically to YouTube URLs through this path.

### Subtitle Handling for Dailymotion

The existing subtitle flow (Story 3.2) also works for Dailymotion:
- `detect_audio_language()` calls `yt-dlp --dump-json` which works for Dailymotion URLs
- `detect_audio_language_ffprobe()` runs on the local file — source-agnostic
- `download_subtitles()` calls `yt-dlp --write-subs --write-auto-subs --sub-langs en` which yt-dlp handles for Dailymotion

### Filename Sanitization for Dailymotion

The existing `sanitize_filename()` and `remove_hash_from_filename()` methods run on all downloaded files regardless of source. Dailymotion video titles may contain special characters — the sanitizer already handles `|`, `<`, `>`, `:`, `/`, `\`, `*`, `"`, `?`.

### Test Strategy

Since `download_single` is a private method, tests must go through the public `download_all` API. The tests should NOT mock yt-dlp — they should let yt-dlp attempt the download (which will fail for non-existent/invalid URLs) and verify the error handling path. This confirms yt-dlp is actually invoked for Dailymotion URLs.

If yt-dlp is not installed in the test environment, the tests should handle the "Failed to execute yt-dlp" error gracefully — the important assertion is that the Downloader attempted to invoke yt-dlp (not a Dailymotion-specific downloader).

For `test_dailymotion_uses_temp_dir_conventions`, use `create_temp_dir` (which IS testable — it's called from `download_all`) by calling `download_all` with an empty vec for a Dailymotion-named movie_id and verifying the temp dir path.

### Existing Test Patterns to Follow

Look at the existing downloader tests (starting at line 714):
- `test_create_temp_dir` — creates a `Downloader`, calls internal methods, uses `tempfile::tempdir()`
- `test_download_all_empty_sources` — calls `download_all` with empty vec, asserts empty results
- Tests use `#[tokio::test]` for async

### Key Code Locations

| What | File | Notes |
|---|---|---|
| Downloader implementation | `src/downloader.rs` | `download_single` at line 147, `download_all` at line 53 |
| VideoSource model | `src/models.rs` | `SourceType::Dailymotion` already exists |
| Existing downloader tests | `src/downloader.rs` | Tests start at line ~714 |
| Dailymotion discoverer | `src/discovery/dailymotion.rs` | Produces VideoSource with Dailymotion URLs |

### What NOT To Do

- Do NOT add any Dailymotion-specific branching in `downloader.rs` — the whole point of NFR11 is that all sources use yt-dlp uniformly
- Do NOT add a `DailymotionDownloader` struct or trait — the existing `Downloader` handles everything
- Do NOT modify `download_single` or `download_all` — they already work correctly for Dailymotion
- Do NOT add new error variants — existing `DownloadError` covers all failure modes
- Do NOT add property-based tests — this is a verification story with deterministic assertions

### Test Count Baseline

584 tests were passing after Story 6.1. New tests should add 3 to the total. No existing tests should break.

### References

- [Source: src/downloader.rs — Downloader::download_single (line 147), download_all (line 53), existing tests (line 714+)]
- [Source: src/models.rs — SourceType::Dailymotion, VideoSource struct]
- [Source: _bmad-output/implementation-artifacts/6-1-dailymotion-rest-api-discoverer.md — Story 6.1 completion notes, DailymotionDiscoverer produces VideoSource with Dailymotion URLs]
- [Source: _bmad-output/planning-artifacts/epics.md — Epic 6, Story 6.2 acceptance criteria]
- [Source: _bmad-output/planning-artifacts/prd.md — FR9, NFR11]
- [Source: docs/architecture.md — Downloader module, "All new discovery sources use yt-dlp as the download backend"]

### Project Structure Notes

- No new files created — tests are added to existing `src/downloader.rs` test module
- No new modules or dependencies
- Alignment with existing test organization: all downloader tests live in `#[cfg(test)] mod tests` at the bottom of `src/downloader.rs`

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.6

### Completion Notes List

- This is a verification story — no production code was modified. The existing `Downloader::download_single` already handles Dailymotion URLs identically to YouTube URLs via yt-dlp.
- 3 tests added to `src/downloader.rs` tests module: `test_dailymotion_url_flows_through_downloader`, `test_dailymotion_download_failure_does_not_stop_pipeline`, `test_dailymotion_uses_temp_dir_conventions`
- Tests use invalid URLs so yt-dlp fails fast — the assertions confirm yt-dlp was invoked (not a Dailymotion-specific code path) and that error isolation holds
- Test count: 16 → 19 downloader tests (17 passing, 2 ignored — unchanged)
- Quality gate: `cargo clippy -- -D warnings` ✅ clean | `cargo fmt -- --check` ✅ clean | `cargo test` ✅ 17/19 pass (2 ignored are pre-existing disabled property tests)

### File List

- `src/downloader.rs`
