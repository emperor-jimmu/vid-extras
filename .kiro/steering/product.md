# Product Overview

extras_fetcher is a Jellyfin media extras automation tool that discovers, downloads, and organizes supplemental video content for both movie and TV series libraries.

The tool scans a Jellyfin library, discovers extras from multiple sources (TMDB, Archive.org, YouTube), downloads them using yt-dlp, converts them to x265 format with ffmpeg, and organizes them into Jellyfin-compatible directory structures.

## Supported Content Types

### Movies

- Trailers
- Featurettes
- Behind-the-scenes content
- Deleted scenes
- Interviews

### TV Series

- Series-level extras (trailers, interviews, behind-the-scenes)
- Season-specific extras (organized by season)
- Season 0 specials (official special episodes)
- Bonus content from season packs

## Key Features

### Core Functionality

- Multi-source content discovery (TMDB API, Archive.org, YouTube)
- Automated video downloading with yt-dlp
- Hardware-accelerated video conversion (NVENC, QSV, or software fallback)
- Jellyfin-compatible file organization
- Skip-on-completion markers to avoid reprocessing
- Configurable concurrency and source filtering

### TV Series Support

- Automatic series vs movie detection
- Series folder name parsing (with/without year)
- Season 0 specials discovery and organization
- Season-specific extras discovery
- Metadata caching with 7-day TTL
- Season pack post-processing with bonus content extraction
- Local Season 0 file import and organization
- Fuzzy title matching (80% similarity threshold)
- Comprehensive error handling with graceful degradation
- **Customizable Season 0 folder name** (default: "Specials")

### Configuration Options

- `--series-only` - Process only TV series
- `--movies-only` - Process only movies
- `--season-extras` - Enable season-specific extras discovery
- `--specials` - Enable Season 0 specials discovery
- `--specials-folder <NAME>` - Customize Season 0 folder name (default: "Specials")
- `--force` - Reprocess completed items
- `--mode` - Content source filtering (all or youtube-only)
- `--concurrency` - Parallel processing limit
- `--verbose` - Detailed logging output
