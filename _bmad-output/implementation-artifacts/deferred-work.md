# Deferred Work

## Deferred from: code review of 3-2-non-english-subtitle-auto-download (2026-03-24)

- `unwrap_or("subtitle")` fallback in `src/downloader.rs:231` — if `local_path.file_stem()` returns `None` or non-UTF-8, subtitle `base_name` becomes the literal `"subtitle"` which won't match any video stem during organizer scan, orphaning the subtitle files. Near-zero probability (requires a path with no filename component).
