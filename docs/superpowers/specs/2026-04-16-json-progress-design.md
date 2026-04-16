# JSON Progress Output Design

## Overview

Add machine-readable line-delimited JSON output alongside existing human-readable CLI output for use by external tools (e.g., web UI) monitoring processing progress.

## Requirements

- Runs alongside human-readable output (not replacing it)
- Line-delimited JSON (one JSON object per line)
- Each line represents current state snapshot
- Tracks: item started, completed, errors

## Implementation

### CLI Flag

Add `--json-progress` flag to `src/cli.rs`:
- Type: boolean flag
- Default: false

### New Module

Create `src/json_output.rs` with `emit_progress()` function:

**Struct: ProgressEvent**
```rust
pub struct ProgressEvent {
    pub event: String,        // "started" | "completed" | "error"
    pub item_type: String,     // "movie" | "series"
    pub current: usize,      // current item index (1-based)
    pub total: usize,        // total items
    pub title: String,     // item title
    pub year: Option<u16>,  // year if available
    pub phase: Option<String>,          // "discovery" | "downloading" | "converting" | "organizing"
    pub downloads: Option<usize>,      // downloads so far
    pub conversions: Option<usize>,  // conversions so far
    pub discovered: Option<usize>, // videos found (discovery phase)
    pub success: Option<bool>,     // only on completed
    pub error: Option<String>,      // only on error/completed=false
}
```

### Integration Points

1. **Movie processing** (`src/orchestrator.rs`):
   - Emit `started` when beginning item
   - Emit `completed` when finished (success/failure)
   - Emit `error` on any failure

2. **Series processing** (`src/orchestrator.rs`):
   - Same as movie

3. **Phase transitions** in orchestrator:
   - Emit intermediate events with `phase` field

### Output Format

Example lines:
```json
{"event":"started","item_type":"movie","current":1,"total":10,"title":"The Matrix","year":1999}
{"event":"started","item_type":"movie","current":1,"total":10,"title":"The Matrix","year":1999,"phase":"discovery","discovered":15}
{"event":"started","item_type":"movie","current":1,"total":10,"title":"The Matrix","year":1999,"phase":"downloading","downloads":1}
{"event":"completed","item_type":"movie","current":1,"total":10,"title":"The Matrix","year":1999,"success":true,"downloads":5,"conversions":4}
```

## Files to Modify

- `src/cli.rs` - Add `--json-progress` flag
- `src/lib.rs` - Export new module
- `src/orchestrator.rs` - Emit progress events at key points

## Testing

- Unit tests for JSON serialization
- Verify valid JSON output with flag
- Verify no output without flag