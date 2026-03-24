# Story 3.2: Non-English Subtitle Auto-Download

Status: done

## Story

As a user,
I want English subtitles automatically downloaded for non-English extras,
So that foreign-language content is usable without manual subtitle hunting.

## Acceptance Criteria

1. After a video is successfully downloaded, the system detects the audio language using yt-dlp's `--dump-json` metadata (`language` field on the best audio format) as the primary method
2. If the `language` field is absent or unparseable from yt-dlp metadata, ffprobe is used as fallback: `ffprobe -v quiet -print_format json -show_streams <file>` — check the first audio stream's `tags.language` field
3. If the detected language is not English (`"en"`, `"eng"`, or `null`/absent — treat absent as English to avoid false positives), yt-dlp is re-invoked with `--write-subs --write-auto-subs --sub-langs en` to fetch English subtitles
4. Manual subtitles are preferred over auto-generated; yt-dlp's `--write-subs --write-auto-subs` ordering handles this automatically
5. If English subtitles are not available from either source, the download still succeeds — subtitles are best-effort; no error is logged, only a debug message
6. Subtitle files (`.vtt`, `.srt`, `.ass`, `.en.vtt`, etc.) are placed alongside the video file in the same temp directory with matching base name
7. Subtitle files are carried through the organizer alongside their video: when the video is moved to its Jellyfin subfolder, all subtitle files sharing the same base name (before extension) are moved with it
8. Subtitle files are NOT passed through the converter — only video files are converted; subtitle files travel directly from download temp dir to final Jellyfin subfolder
9. No additional external tools are required beyond yt-dlp and ffprobe (which ships with ffmpeg)
10. `cargo build` compiles without errors; `cargo test` passes; `cargo clippy -- -D warnings` clean

## Tasks / Subtasks

- [x] Task 1: Add language detection helper in `src/downloader.rs` (AC: #1, #2)
  - [x] 1.1 Add `async fn detect_audio_language(url: &str) -> Option<String>` — runs `yt-dlp --dump-json --no-playlist --quiet <url>`, parses JSON, finds the format with `"acodec" != "none"` and highest `"abr"` (or first audio format), returns its `"language"` field value
  - [x] 1.2 Add `async fn detect_audio_language_ffprobe(path: &Path) -> Option<String>` — runs `ffprobe -v quiet -print_format json -show_streams <path>`, parses JSON, finds first stream with `"codec_type": "audio"`, returns `tags.language` value
  - [x] 1.3 Both functions return `None` on any error (command failure, JSON parse error, missing field) — errors are logged at `debug!` level only

- [x] Task 2: Add subtitle download helper in `src/downloader.rs` (AC: #3, #4, #5, #6)
  - [x] 2.1 Add `async fn download_subtitles(&self, url: &str, dest_dir: &Path, base_name: &str) -> Vec<PathBuf>` as a method on `Downloader` (not a free function) — this gives access to `self.download_timeout` and `self.cookies_from_browser`
  - [x] 2.2 The method runs `yt-dlp --write-subs --write-auto-subs --sub-langs en --skip-download --no-playlist --quiet -o <dest_dir>/<base_name>.%(ext)s <url>`, wrapped in `timeout(self.download_timeout, ...)`, and passes `--cookies-from-browser <browser>` when `self.cookies_from_browser` is set
  - [x] 2.3 After the yt-dlp call, scan `dest_dir` for subtitle files: collect all entries whose extension is one of `["vtt", "srt", "ass", "ttml", "srv3", "srv2", "srv1", "json3"]` AND whose stem equals `base_name` OR starts with `format!("{}.", base_name)` — return those `PathBuf`s
  - [x] 2.4 If yt-dlp exits non-zero, times out, or no subtitle files are found, return empty `Vec` — log at `debug!` level, do NOT log at `warn!` or `error!` (subtitles are best-effort)

- [x] Task 3: Integrate language detection + subtitle download into `download_single` (AC: #1, #2, #3, #6)
  - [x] 3.1 After a successful download (file found at `local_path`), call `detect_audio_language(&source.url).await`
  - [x] 3.2 If language is `None`, `"en"`, or `"eng"` → skip subtitle download (treat absent as English)
  - [x] 3.3 If language is non-English → call `detect_audio_language_ffprobe(&local_path).await` as confirmation (avoids false positives from yt-dlp metadata gaps); if ffprobe also returns non-English or `None` (absent from file metadata), proceed with subtitle download
  - [x] 3.4 Call `self.download_subtitles(&source.url, dest_dir, base_name).await` where `base_name` is the stem of `local_path` (filename without extension)
  - [x] 3.5 Store the subtitle paths in `DownloadResult` — add `pub subtitle_paths: Vec<PathBuf>` field to `DownloadResult` in `models.rs`; set to empty `Vec` when no subtitles downloaded

- [x] Task 4: Carry subtitle files through the organizer (AC: #7, #8)
  - [x] 4.1 In `Organizer::organize()`, after `self.move_file(&file_path, &subdir, &dest_filename).await?`, scan `file_path.parent()` (the temp dir — still exists at this point) for subtitle files whose stem equals `file_path.file_stem()` OR starts with `format!("{}.", stem)`, with extensions in `["vtt", "srt", "ass", "ttml", "srv3", "srv2", "srv1", "json3"]`
  - [x] 4.2 Note: use `file_path` (the pre-move source path, i.e., `conversion.input_path` or `conversion.output_path` depending on the loop variable) to derive the stem and parent dir — the subtitle files are siblings of the original downloaded file in the temp dir, not siblings of the converted output
  - [x] 4.3 Move each found subtitle file to `subdir` (the same Jellyfin subfolder as the video), preserving the subtitle filename as-is
  - [x] 4.4 Apply the same pattern in `SeriesOrganizer::organize_extras()` — subtitle files travel with their video
  - [x] 4.5 Subtitle files are NOT renamed by the `normalize_filename()` logic — only video files get the `{Category} #{N}` treatment; subtitle files keep their yt-dlp-assigned names
  - [x] 4.6 If a subtitle file move fails, log a `warn!` and continue — do not fail the entire organize operation
  - [x] 4.7 Add a unit test: create a temp dir with a video file and a sibling `.en.vtt` subtitle file, call `organize()`, assert the subtitle file was moved to the same Jellyfin subfolder as the video
  - [x] 4.8 Add a unit test for the normalized-filename interaction: create a numeric video file (`10032.mp4`) with a sibling `10032.en.vtt`, call `organize()`, assert the video is renamed to `Trailer #1.mp4` and the subtitle `10032.en.vtt` is moved to the same subfolder with its original name

- [x] Task 5: Update `DownloadResult` struct and all construction sites (AC: #5)
  - [x] 5.1 Add `pub subtitle_paths: Vec<PathBuf>` to `DownloadResult` in `src/models.rs`
  - [x] 5.2 Update all `DownloadResult { ... }` construction sites in `downloader.rs` to include `subtitle_paths: vec![]` (failed downloads have no subtitles)
  - [x] 5.3 Run `cargo build` immediately after struct change to catch all construction sites via compiler errors

- [x] Task 6: Quality gate (AC: #10)
  - [x] 6.1 Run `cargo build` — fix any errors
  - [x] 6.2 Run `cargo test` — fix any failures
  - [x] 6.3 Run `cargo clippy -- -D warnings` — fix any warnings
  - [x] 6.4 Run `cargo fmt -- --check` — fix any formatting issues

## Dev Notes

### Language Detection Strategy — Two-Stage Approach

The two-stage approach (yt-dlp metadata first, ffprobe fallback) is intentional:

- **Stage 1 (yt-dlp `--dump-json`):** Runs against the URL before/after download. The `language` field in yt-dlp's format metadata is populated for most YouTube/Dailymotion videos. Parse the JSON output: look for `formats` array, find the entry with `"acodec" != "none"` and the highest `"abr"` value (best audio bitrate). Return its `"language"` field.

- **Stage 2 (ffprobe on local file):** Used as confirmation when yt-dlp metadata is absent. Runs against the already-downloaded local file. Parse `streams` array, find first entry with `"codec_type": "audio"`, return `tags.language`.

**Treat absent language as English** — if both stages return `None`, skip subtitle download. This avoids false positives for content where language metadata is simply missing (common for older YouTube uploads). The user can always manually add subtitles if needed.

**Language code normalization:** Both `"en"` and `"eng"` are English. Any other non-null value (e.g., `"fr"`, `"ja"`, `"de"`, `"zh"`) triggers subtitle download.

### `detect_audio_language` — yt-dlp JSON Parsing

This is a free async function (not a method) — it only needs the URL, no `self` state.

Note: `max_by` on an empty iterator returns `None`. The trailing `?` propagates that `None` out of the function, which is the correct behavior (no audio format found → treat as English → skip subtitle download). Do not "fix" this by adding a fallback — the `?` is intentional.

Note on latency: `--dump-json` runs a network round-trip against the URL after the video is already downloaded. For the majority of content (English), this adds ~1–3 seconds of overhead per video. This is acceptable given it only triggers subtitle download for non-English content, and the ffprobe confirmation stage runs locally (fast). If this becomes a concern in future, the metadata could be cached from the discovery phase.

```rust
async fn detect_audio_language(url: &str) -> Option<String> {
    let output = Command::new("yt-dlp")
        .args(["--dump-json", "--no-playlist", "--quiet", url])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

    // Find best audio format by abr
    let formats = json.get("formats")?.as_array()?;
    let best_audio = formats.iter()
        .filter(|f| f.get("acodec").and_then(|v| v.as_str()) != Some("none"))
        .max_by(|a, b| {
            let abr_a = a.get("abr").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let abr_b = b.get("abr").and_then(|v| v.as_f64()).unwrap_or(0.0);
            abr_a.partial_cmp(&abr_b).unwrap_or(std::cmp::Ordering::Equal)
        })?;

    best_audio.get("language")?.as_str().map(|s| s.to_string())
}
```

### `detect_audio_language_ffprobe` — ffprobe JSON Parsing

```rust
async fn detect_audio_language_ffprobe(path: &Path) -> Option<String> {
    let output = Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_streams"])
        .arg(path)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let streams = json.get("streams")?.as_array()?;

    streams.iter()
        .find(|s| s.get("codec_type").and_then(|v| v.as_str()) == Some("audio"))
        .and_then(|s| s.get("tags"))
        .and_then(|t| t.get("language"))
        .and_then(|l| l.as_str())
        .map(|s| s.to_string())
}
```

### `download_subtitles` — yt-dlp Subtitle Invocation

`download_subtitles` is a method on `Downloader` (not a free function) so it can access `self.download_timeout` and `self.cookies_from_browser`.

```rust
async fn download_subtitles(&self, url: &str, dest_dir: &Path, base_name: &str) -> Vec<PathBuf> {
    let output_template = dest_dir.join(format!("{}.%(ext)s", base_name));
    let output_template_str = output_template.to_string_lossy().to_string();

    let mut cmd = Command::new("yt-dlp");
    cmd.args([
        "--write-subs",
        "--write-auto-subs",
        "--sub-langs", "en",
        "--skip-download",
        "--no-playlist",
        "--quiet",
        "-o", &output_template_str,
        url,
    ]);

    // Pass browser cookies when configured — same bot-detection bypass as download_single
    if let Some(browser) = &self.cookies_from_browser {
        cmd.arg("--cookies-from-browser").arg(browser);
    }

    let result = timeout(self.download_timeout, cmd.output()).await;

    let subtitle_extensions = ["vtt", "srt", "ass", "ttml", "srv3", "srv2", "srv1", "json3"];

    match result {
        Ok(Ok(output)) if output.status.success() => {
            // Scan dest_dir for subtitle files matching base_name
            // yt-dlp writes: base_name.en.vtt, base_name.en.srt, etc.
            let mut found = Vec::new();
            if let Ok(mut entries) = tokio::fs::read_dir(dest_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if subtitle_extensions.contains(&ext)
                        && (stem == base_name || stem.starts_with(&format!("{}.", base_name)))
                    {
                        found.push(path);
                    }
                }
            }
            if found.is_empty() {
                debug!("No subtitle files found after yt-dlp for: {}", url);
            }
            found
        }
        Ok(Ok(_)) => {
            debug!("yt-dlp returned non-zero exit for subtitles: {}", url);
            vec![]
        }
        Ok(Err(e)) => {
            debug!("Failed to execute yt-dlp for subtitles: {}", e);
            vec![]
        }
        Err(_) => {
            debug!("Subtitle download timed out for: {}", url);
            vec![]
        }
    }
}
```

### Integration Point in `download_single`

Insert subtitle logic after the `local_path.exists()` check succeeds (line ~220 in current code):

```rust
// After: info!("Successfully downloaded: {:?}", local_path);
// Add:
let subtitle_paths = {
    let lang = detect_audio_language(&source.url).await;
    let is_non_english = match lang.as_deref() {
        None | Some("en") | Some("eng") => false,
        Some(_) => true,
    };

    if is_non_english {
        // Confirm with ffprobe before downloading subs
        let ffprobe_lang = detect_audio_language_ffprobe(&local_path).await;
        let confirmed_non_english = match ffprobe_lang.as_deref() {
            Some("en") | Some("eng") => false,  // ffprobe says English, trust it
            _ => true,  // ffprobe says non-English or absent — proceed
        };

        if confirmed_non_english {
            let base_name = local_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("subtitle");
            self.download_subtitles(&source.url, dest_dir, base_name).await
        } else {
            vec![]
        }
    } else {
        vec![]
    }
};

DownloadResult {
    source: source.clone(),
    local_path,
    success: true,
    error: None,
    subtitle_paths,
}
```

### Subtitle Carry-Through in Organizer

The organizer loop iterates over `file_path` values from `files_by_category` — these are `conversion.output_path` values (the converted `.mp4` files). However, subtitle files are siblings of the **original downloaded file** (`conversion.input_path`), not the converted output. Both live in the same temp dir, so `file_path.parent()` is the correct directory to scan in either case. The stem to match against is `conversion.input_path.file_stem()` — the pre-conversion filename, which matches what yt-dlp used when writing the subtitle file.

In `Organizer::organize()`, the loop variable is `file_path` which comes from `conversion.output_path`. To get the correct stem for subtitle matching, you need to track `conversion.input_path` alongside `output_path` in the `files_by_category` map, or scan using `file_path.parent()` with the stem derived from `file_path` itself (which works when input and output share the same stem, which they do — `convert_single` uses `input_path.with_extension("mp4")`).

Concretely: `file_path` is e.g. `tmp/My Trailer.mp4` (output), `input_path` was `tmp/My Trailer.mkv`. Both have stem `My Trailer`. The subtitle is `tmp/My Trailer.en.vtt`. So scanning `file_path.parent()` with stem from `file_path.file_stem()` works correctly.

In `Organizer::organize()`, after `self.move_file(&file_path, &subdir, &dest_filename).await?`, add:

```rust
// Move sibling subtitle files alongside the video
let subtitle_extensions = ["vtt", "srt", "ass", "ttml", "srv3", "srv2", "srv1", "json3"];
let stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
if !stem.is_empty() {
    if let Ok(mut entries) = tokio::fs::read_dir(
        file_path.parent().unwrap_or(Path::new("."))
    ).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();
            let ext = entry_path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let entry_stem = entry_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if subtitle_extensions.contains(&ext)
                && (entry_stem == stem || entry_stem.starts_with(&format!("{}.", stem)))
            {
                if let Some(sub_filename) = entry_path.file_name() {
                    let sub_dest = subdir.join(sub_filename);
                    if let Err(e) = tokio::fs::rename(&entry_path, &sub_dest).await {
                        warn!("Failed to move subtitle {:?}: {}", entry_path, e);
                    }
                }
            }
        }
    }
}
```

Apply the same pattern in `SeriesOrganizer::organize_extras()`.

**Numeric filename interaction (Story 3.1):** When a video is renamed from `10032.mp4` to `Trailer #1.mp4`, the subtitle file `10032.en.vtt` is scanned using the stem of `file_path` (which is `10032` at scan time, before the move). The subtitle is found and moved to `subdir/10032.en.vtt` — it keeps its original name. This is correct: subtitle files are never renamed by the `{Category} #{N}` logic.

### Subtitle Files Are NOT Converted

The converter only processes `DownloadResult` entries. Subtitle files are sidecar files in the temp dir. The organizer is responsible for moving them. The converter does not need to be modified.

**Important:** The converter deletes the original video file after successful conversion (`fs::remove_file(input_path)`). Subtitle files have different extensions and are not touched by the converter. They remain in the temp dir until the organizer moves them. This is correct by design.

### `DownloadResult` Struct Change — All Construction Sites

After adding `subtitle_paths: Vec<PathBuf>` to `DownloadResult`, `cargo build` will fail at every construction site. There are multiple `DownloadResult { ... }` literals in `download_single` (success path, yt-dlp failure, command failure, timeout). All failure paths get `subtitle_paths: vec![]`. Only the success path gets the actual subtitle paths.

Construction sites in `downloader.rs` — all must be updated:

`download_all()` (line ~71):
- The `map(|source| DownloadResult { ... })` in the temp-dir-creation failure path → `subtitle_paths: vec![]`

`download_single()` (approximate lines):
- Line ~201: success path → `subtitle_paths` (computed above)
- Line ~212: file not found after download (path mismatch) → `subtitle_paths: vec![]`
- Line ~225: file not found error (find_downloaded_file Err) → `subtitle_paths: vec![]`
- Line ~246: yt-dlp non-zero exit → `subtitle_paths: vec![]`
- Line ~262: command execution failure → `subtitle_paths: vec![]`
- Line ~280: timeout → `subtitle_paths: vec![]`

Construction sites in `src/converter.rs` tests (all in `#[cfg(test)]`):
- `create_test_download_result()` helper (line ~294) → `subtitle_paths: vec![]`
- `let _download = DownloadResult { ... }` (line ~471) → `subtitle_paths: vec![]`
- `let failed = DownloadResult { ... }` (line ~521) → `subtitle_paths: vec![]`
- `let download = DownloadResult { ... }` (line ~793) → `subtitle_paths: vec![]`
- `let download = DownloadResult { ... }` (line ~845) → `subtitle_paths: vec![]`

Also check `tests/main_integration_tests.rs` for any `DownloadResult { ... }` literals.

Running `cargo build` immediately after the struct change will enumerate every missed site as a compile error — use this as the definitive checklist.

### Subtitle File Naming Convention

yt-dlp writes subtitle files as: `{output_template_stem}.{lang}.{format}` — e.g., `My Trailer.en.vtt`, `My Trailer.en.srt`. The `-o` template uses `%(ext)s` which yt-dlp replaces with the subtitle format extension.

When scanning for subtitle files in the organizer, match files where the stem is either:
- Exactly the video stem (e.g., `My Trailer.vtt` — rare)
- The video stem + `.` + anything (e.g., `My Trailer.en.vtt` — common)

Use `entry_stem.starts_with(&format!("{}.", stem))` for the second case.

### Converter Does Not Need Changes

The converter processes `DownloadResult.local_path` (the video file). Subtitle files are not in `DownloadResult.local_path` — they're sidecar files in the same directory. The converter's `convert_single` only touches the video file and its `.tmp.mp4` intermediate. Subtitle files are unaffected.

The `ConversionResult` struct does NOT need a `subtitle_paths` field — the organizer discovers subtitle files by scanning the temp dir at organize time, not by tracking them through conversion.

### Key Code Locations

| What | File | Line | Notes |
|---|---|---|---|
| `DownloadResult` struct | `src/models.rs` | 53 | Add `subtitle_paths: Vec<PathBuf>` |
| `download_single()` | `src/downloader.rs` | 146 | Insert language detection + subtitle download after success |
| `Organizer::organize()` | `src/organizer.rs` | 77 | Add subtitle carry-through after `move_file()` |
| `SeriesOrganizer::organize_extras()` | `src/organizer.rs` | 283 | Same subtitle carry-through |
| `SeriesOrganizer::organize_specials()` | `src/organizer.rs` | 375 | DO NOT MODIFY — specials use Sonarr naming |
| `Converter::convert_single()` | `src/converter.rs` | 72 | DO NOT MODIFY |
| Integration tests | `tests/main_integration_tests.rs` | Various | Add `subtitle_paths: vec![]` to any `DownloadResult` literals |
| Converter tests | `src/converter.rs` | ~294, ~471, ~521, ~793, ~845 | All `DownloadResult { ... }` in `#[cfg(test)]` blocks need `subtitle_paths: vec![]` |

### What NOT To Do

- Do NOT make `download_subtitles` a free function — it must be a method on `Downloader` to access `self.download_timeout` and `self.cookies_from_browser`
- Do NOT omit `--cookies-from-browser` from the subtitle yt-dlp invocation — sources that require cookie auth for video download also require it for subtitle fetch
- Do NOT modify `organize_specials()` — Sonarr naming is intentional
- Do NOT pass subtitle files through the converter
- Do NOT add `subtitle_paths` to `ConversionResult` — organizer discovers subs by dir scan
- Do NOT log subtitle unavailability at `warn!` or `error!` — `debug!` only (subtitles are best-effort)
- Do NOT block the download success on subtitle availability — subtitle failure must not fail the download
- Do NOT treat absent language metadata as non-English — absent = English (avoid false positives)
- Do NOT rename subtitle files with the `{Category} #{N}` pattern — only video files get normalized names

### Previous Story Learnings (from Story 3.1)

- Run `cargo build` immediately after struct field additions — the compiler will enumerate all construction sites that need updating
- `SeriesOrganizer` and `Organizer` are separate structs in the same file; changes to one don't automatically apply to the other
- Quality gate order: build → test → clippy → fmt
- `tokio::process::Command` is used throughout (not `std::process::Command`) — import from `tokio::process`
- The `--quiet` and `--no-warnings` flags are used on all yt-dlp invocations to reduce noise

### Test Guidance

Add unit tests for:
- `is_english_language()` helper (if extracted): `"en"` → true, `"eng"` → true, `None` → true (absent = English), `"fr"` → false, `"ja"` → false
- `download_subtitles()` with a mock that returns empty — verify empty `Vec` returned without error
- `Organizer::organize()` with a subtitle file alongside the video — create a temp dir with `trailer.mp4` and `trailer.en.vtt`, call `organize()`, assert `trailer.en.vtt` was moved to `movie/trailers/trailer.en.vtt`
- Numeric filename + subtitle interaction: create `10032.mp4` and `10032.en.vtt` in temp dir, call `organize()` with `ContentCategory::Trailer`, assert video is renamed to `Trailer #1.mp4` in `trailers/` and subtitle `10032.en.vtt` is also in `trailers/` with its original name
- `DownloadResult` construction with `subtitle_paths` populated — verify field is accessible

Existing tests that construct `DownloadResult` directly will need `subtitle_paths: vec![]` added — the compiler will identify them all (including the 5 sites in `src/converter.rs` tests).

## References

- [Source: _bmad-output/planning-artifacts/epics.md — Epic 3, Story 3.2]
- [Source: _bmad-output/planning-artifacts/prd.md — FR26, NFR11]
- [Source: src/downloader.rs — download_single(), remove_hash_from_filename(), sanitize_filename()]
- [Source: src/models.rs — DownloadResult, VideoSource structs]
- [Source: src/organizer.rs — Organizer::organize(), SeriesOrganizer::organize_extras()]
- [Source: src/converter.rs — convert_single() — reference only, no changes needed]

## Dev Agent Record

### Implementation Plan

Implemented in task order: Task 5 (struct change) first to use the compiler as a construction-site finder, then Tasks 1+2 (helpers), Task 3 (integration), Task 4 (organizer carry-through), Task 6 (quality gate).

### Completion Notes

All 6 tasks and 18 subtasks implemented and verified:

- `DownloadResult.subtitle_paths: Vec<PathBuf>` added to `src/models.rs`; all 12 construction sites updated (7 in `downloader.rs`, 5 in `converter.rs` tests)
- `detect_audio_language()` and `detect_audio_language_ffprobe()` added as private async functions on `Downloader`; both return `None` on any error (debug-only logging)
- `download_subtitles()` added as a method on `Downloader` with timeout and cookie-browser passthrough; returns empty `Vec` on any failure (debug-only logging)
- Two-stage language detection integrated into `download_single()` success path: yt-dlp metadata first, ffprobe confirmation, then subtitle fetch only for confirmed non-English
- `move_sibling_subtitles()` free async function added to `organizer.rs`; called after `move_file()` in both `Organizer::organize()` and `SeriesOrganizer::organize_extras()`; subtitle files keep original names, failures are `warn!`-logged and non-fatal
- `organize_specials()` not modified (Sonarr naming intentional)
- Converter not modified (subtitle files are not video files)
- 2 new unit tests added: `test_organize_moves_subtitle_alongside_video` and `test_organize_subtitle_kept_original_name_when_video_renamed`
- Quality gate: `cargo build` ✅ · `cargo test` 546 passed ✅ · `cargo clippy -- -D warnings` ✅ · `cargo fmt -- --check` ✅

## File List

- `src/models.rs` — added `subtitle_paths: Vec<PathBuf>` field to `DownloadResult`
- `src/downloader.rs` — added `detect_audio_language()`, `detect_audio_language_ffprobe()`, `download_subtitles()`; integrated subtitle logic into `download_single()`; updated all `DownloadResult` construction sites
- `src/organizer.rs` — added `move_sibling_subtitles()` helper; called in `Organizer::organize()` and `SeriesOrganizer::organize_extras()`; added 2 unit tests
- `src/converter.rs` — updated 5 `DownloadResult` construction sites in test code to include `subtitle_paths: vec![]`

### Review Findings

- [x] [Review][Patch] No timeout on `detect_audio_language` / `detect_audio_language_ffprobe` — both helper functions call external commands (`yt-dlp --dump-json`, `ffprobe`) without any timeout wrapper, unlike the main download path which uses `timeout()`. If either command hangs, the pipeline blocks indefinitely. [src/downloader.rs:518-590] — FIXED: wrapped in `timeout(30s)` and `timeout(15s)` respectively
- [x] [Review][Patch] Cross-drive subtitle move fails silently on Windows — `move_sibling_subtitles` uses bare `tokio::fs::rename` without the cross-drive fallback (copy+delete) that exists in `Organizer::move_file` and `organize_specials`. On Windows with temp dir on a different drive, all subtitle moves fail with `warn!` and subtitles are lost. [src/organizer.rs:91-96] — FIXED: added `raw_os_error() == Some(17)` fallback with copy+delete
- [x] [Review][Patch] Duplicated `subtitle_extensions` array — the list `["vtt", "srt", "ass", ...]` appears independently in `download_subtitles` and `move_sibling_subtitles`. Should be a shared constant to avoid divergence. [src/downloader.rs:631, src/organizer.rs:65] — FIXED: extracted to `SUBTITLE_EXTENSIONS` constant in `models.rs`, imported in both modules
- [x] [Review][Defer] `unwrap_or("subtitle")` fallback produces orphaned subtitles — if `local_path.file_stem()` returns `None` or non-UTF-8, `base_name` becomes `"subtitle"` which won't match any video during organizer scan. Extremely unlikely (requires path with no filename). [src/downloader.rs:231] — deferred, pre-existing edge case with near-zero probability

## Change Log

- 2026-03-24: Implemented Story 3.2 — non-English subtitle auto-download. Added two-stage language detection (yt-dlp metadata + ffprobe confirmation), subtitle download via yt-dlp `--write-subs`, and subtitle carry-through in both movie and series organizers. 546 tests passing.
