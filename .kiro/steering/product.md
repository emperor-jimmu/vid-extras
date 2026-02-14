# Product Overview

extras_fetcher is a Jellyfin movie extras automation tool that discovers, downloads, and organizes supplemental video content (trailers, featurettes, behind-the-scenes, deleted scenes) for movie libraries.

The tool scans a Jellyfin movie library, discovers extras from multiple sources (TMDB, Archive.org, YouTube), downloads them using yt-dlp, converts them to x265 format with ffmpeg, and organizes them into Jellyfin-compatible directory structures.

Key features:
- Multi-source content discovery (TMDB API, Archive.org, YouTube)
- Automated video downloading with yt-dlp
- Hardware-accelerated video conversion (NVENC, QSV, or software fallback)
- Jellyfin-compatible file organization
- Skip-on-completion markers to avoid reprocessing
- Configurable concurrency and source filtering
