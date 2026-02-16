# extras_fetcher

A Rust-based automation tool for enriching Jellyfin movie libraries by discovering, downloading, and organizing supplementary video content (trailers, behind-the-scenes footage, deleted scenes, featurettes) from multiple online sources.

## Features

### Movies & TV Series Support

- **Multi-source content discovery**: Automatically finds extras from TheMovieDB, Archive.org, and YouTube
- **Movie extras**: Trailers, behind-the-scenes footage, deleted scenes, featurettes, interviews
- **TV series extras**: Series-level and season-specific bonus content
- **Season 0 specials**: Discover and organize official special episodes
- **Automatic media detection**: Intelligently distinguishes between movies and TV series

### Processing & Organization

- **Automated downloading**: Uses yt-dlp to fetch videos from discovered sources
- **Hardware-accelerated conversion**: Converts videos to efficient x265/HEVC format using NVENC, QSV, or software encoding
- **Jellyfin-compatible organization**: Organizes content into standard subdirectories that Jellyfin automatically recognizes
- **Idempotent execution**: Safe to run multiple times - tracks completed items with done markers
- **Configurable processing**: Control concurrency, source filtering, and processing modes
- **Rich CLI output**: Colored progress indicators and detailed status information

### Advanced Features

- **Metadata caching**: 7-day cache for TMDB series metadata to reduce API calls
- **Season pack extraction**: Automatically extracts and organizes bonus content from season pack archives
- **Local Season 0 import**: Scans for existing Season 0 files and organizes them correctly
- **Fuzzy title matching**: 80% similarity threshold for matching extras to series/movies

## Requirements

### System Dependencies

The following tools must be installed and available in your system PATH:

1. **yt-dlp** - Video downloading
   - Installation: https://github.com/yt-dlp/yt-dlp#installation
   - Verify: `yt-dlp --version`

2. **ffmpeg** - Video conversion with HEVC/x265 support
   - Installation: https://ffmpeg.org/download.html
   - Verify: `ffmpeg -version` (should show libx265 or hevc_nvenc/hevc_qsv support)

### Configuration

#### TMDB API Key

A TMDB API key is required for discovering movie extras. The tool will check for the API key in this order:

1. **config.cfg file** (recommended) - The tool will automatically prompt you to enter your API key on first run if this file doesn't exist
2. **TMDB_API_KEY environment variable** (fallback for backward compatibility)

**Getting your API key:**

1. Visit: https://www.themoviedb.org/settings/api
2. Sign up for a free account (if you don't have one)
3. Request an API key from your account settings
4. Copy the 'API Key (v3 auth)' value

**Option 1: Using config.cfg (Recommended)**

On first run, the tool will prompt you to enter your API key and automatically create a `config.cfg` file:

```bash
extras_fetcher /path/to/movies
# You'll be prompted: "Enter your TMDB API key: "
# The key will be saved to config.cfg for future use
```

You can also manually create the config file:

```json
{
  "tmdb_api_key": "your_api_key_here"
}
```

A sample configuration file is provided as `config.cfg.example` for reference.

**Option 2: Using Environment Variable**

Set the environment variable (for backward compatibility):

- Linux/macOS: `export TMDB_API_KEY=your_api_key_here`
- Windows: `set TMDB_API_KEY=your_api_key_here`

### Optional: Logging

- `RUST_LOG` - Control logging verbosity
  - Values: `error`, `warn`, `info`, `debug`, `trace`
  - Example: `export RUST_LOG=debug`

## Installation

For detailed installation instructions including dependency setup, see [INSTALL.md](INSTALL.md).

### Quick Install from Source

```bash
# Clone the repository
git clone <repository-url>
cd extras_fetcher

# Build release binary
cargo build --release

# Binary will be at: target/release/extras_fetcher (or extras_fetcher.exe on Windows)

# Optional: Install to system PATH
# Linux/macOS: sudo cp target/release/extras_fetcher /usr/local/bin/
# Windows: copy target\release\extras_fetcher.exe C:\Windows\System32\
```

## Usage

### Basic Usage

```bash
extras_fetcher /path/to/movie/library
```

### Command-Line Options

```
extras_fetcher [OPTIONS] <ROOT_DIRECTORY>

Arguments:
  <ROOT_DIRECTORY>  Path to the root directory containing movie and/or series folders

Options:
  -f, --force              Ignore done markers and reprocess all items
  -m, --mode <MODE>        Content source mode [default: all] [possible values: all, youtube]
  -c, --concurrency <N>    Maximum number of items to process concurrently [default: 2]
  -v, --verbose            Enable verbose logging output

  Series-specific options:
  --series-only            Process only TV series (skip movies)
  --movies-only            Process only movies (skip TV series)
  --season-extras          Enable season-specific extras discovery
  --specials               Enable Season 0 specials discovery
  --specials-folder <NAME> Folder name for Season 0 specials [default: Specials]
  --type <TYPE>            Force media type classification [possible values: movie, series]

  -h, --help               Print help information
  -V, --version            Print version information
```

### Examples

**Process a movie library with default settings:**

```bash
extras_fetcher /media/movies
```

**Process a TV series library:**

```bash
extras_fetcher --series-only /media/tv
```

**Process a mixed library (movies and series):**

```bash
extras_fetcher /media/library
```

**Process series with Season 0 specials and season-specific extras:**

```bash
extras_fetcher --series-only --specials --season-extras /media/tv
```

**Reprocess all items (ignore done markers):**

```bash
extras_fetcher --force /media/library
```

**Use only YouTube as a content source:**

```bash
extras_fetcher --mode youtube /media/library
```

**Process 4 items concurrently with verbose logging:**

```bash
extras_fetcher --concurrency 4 --verbose /media/library
```

**Enable debug logging:**

```bash
RUST_LOG=debug extras_fetcher /media/library
```

## TV Series Configuration

### Series Library Setup

For the tool to properly detect TV series, follow these naming conventions:

**Series Folder Names:**

- With year: `Series Name (YYYY)` - Recommended for accurate TMDB matching
- Without year: `Series Name` - Supported but may match multiple series on TMDB

**Season Folders:**

- Must be named exactly as `Season XX` with zero-padded numbers
- Examples: `Season 00`, `Season 01`, `Season 02`, etc.
- Season 00 is reserved for specials (pilot episodes, holiday specials, etc.)

**Example Structure:**

```
/media/tv/
├── Breaking Bad (2008)/
│   ├── Season 01/
│   ├── Season 02/
│   └── Season 05/
└── The Office (2005)/
    ├── Season 01/
    └── Season 09/
```

### Series Processing Modes

**Process Everything (Default):**

```bash
extras_fetcher /media/library
```

Automatically detects and processes both movies and TV series.

**Series Only:**

```bash
extras_fetcher --series-only /media/tv
```

Processes only TV series, skips all movie folders.

**Movies Only:**

```bash
extras_fetcher --movies-only /media/movies
```

Processes only movies, skips all series folders.

### Series-Specific Features

**Enable Season 0 Specials Discovery:**

```bash
extras_fetcher --series-only --specials /media/tv
```

Discovers and downloads official special episodes from TMDB Season 0.

**Customize Season 0 Folder Name:**

```bash
# Use custom folder name "Season 00"
extras_fetcher --series-only --specials --specials-folder "Season 00" /media/tv

# Use custom folder name "Season 0"
extras_fetcher --series-only --specials --specials-folder "Season 0" /media/tv

# Use default "Specials" folder
extras_fetcher --series-only --specials /media/tv
```

**Enable Season-Specific Extras:**

```bash
extras_fetcher --series-only --season-extras /media/tv
```

Discovers extras specific to individual seasons (e.g., "Season 1 Behind the Scenes").

**Enable Both Features:**

```bash
extras_fetcher --series-only --specials --season-extras /media/tv
```

Full series support with all available content types.

### Best Practices for Series Libraries

1. **Use consistent naming**: Always include the year in series folder names for accurate TMDB matching
2. **Organize by season**: Keep episodes organized in `Season XX` folders before running the tool
3. **Run with appropriate flags**: Use `--specials` and `--season-extras` based on your preferences
4. **Monitor first run**: Use `--verbose` on first run to verify correct detection and organization
5. **Reprocess selectively**: Use `--force` only when needed; the tool tracks completed series with `.extras_done` markers

## Directory Structure

### Input Structure - Movies

Your movie library should follow this naming convention:

```
/media/movies/
├── Movie Title (2020)/
│   └── Movie Title (2020).mkv
├── Another Movie (2019)/
│   └── Another Movie (2019).mp4
└── Classic Film (1999)/
    └── Classic Film (1999).avi
```

### Input Structure - TV Series

Your TV series library should follow this naming convention:

```
/media/tv/
├── Series Name (2020)/
│   ├── Season 01/
│   │   ├── Series Name - S01E01 - Episode Title.mkv
│   │   └── Series Name - S01E02 - Another Episode.mkv
│   └── Season 02/
│       └── Series Name - S02E01 - Episode Title.mkv
└── Another Series (2018)/
    ├── Season 01/
    │   └── Another Series - S01E01.mp4
    └── Season 02/
        └── Another Series - S02E01.mp4
```

### Output Structure - Movies

After processing, extras are organized into Jellyfin-compatible subdirectories:

```
/media/movies/
└── Movie Title (2020)/
    ├── Movie Title (2020).mkv
    ├── trailers/
    │   ├── Official Trailer.mp4
    │   └── Teaser Trailer.mp4
    ├── behind the scenes/
    │   └── Making of Documentary.mp4
    ├── deleted scenes/
    │   └── Deleted Scene 1.mp4
    ├── featurettes/
    │   └── Cast Interviews.mp4
    └── .extras_done  (completion marker)
```

### Output Structure - TV Series

After processing, series extras are organized by series and season:

```
/media/tv/
└── Series Name (2020)/
    ├── Specials/                     # Season 0 specials (default folder name)
    │   ├── Series Name - S00E01 - Pilot.mp4
    │   └── Series Name - S00E02 - Holiday Special.mp4
    ├── Season 01/
    │   ├── Series Name - S01E01 - Episode Title.mkv
    │   ├── behind the scenes/        # Season 1 specific extras
    │   │   └── S01 Making Of.mp4
    │   └── interviews/
    │       └── S01 Cast Interview.mp4
    ├── Season 02/
    │   └── Series Name - S02E01.mkv
    ├── trailers/                     # Series-level extras
    │   ├── Official Trailer.mp4
    │   └── Season 2 Trailer.mp4
    ├── behind the scenes/
    │   └── Series Overview.mp4
    ├── interviews/
    │   └── Creator Interview.mp4
    └── .extras_done  (completion marker)
```

**Note**: The Season 0 folder name can be customized using the `--specials-folder` parameter. Default is "Specials", but you can use "Season 00", "Season 0", or any custom name.

## How It Works

### Processing Pipeline

The tool automatically detects whether each folder contains a movie or TV series and applies the appropriate processing pipeline.

#### For Movies:

1. **Scanning**: Recursively scans the library directory
   - Detects folders with video files directly inside (movie folders)
   - Parses folder names to extract movie title and year
   - Skips folders with `.extras_done` marker (unless `--force` is used)

2. **Discovery**: Queries multiple sources for extra content
   - **TMDB**: Official trailers, behind-the-scenes, deleted scenes, featurettes
   - **Archive.org**: Historical EPK content for movies before 2010
   - **YouTube**: Community-uploaded extras with smart filtering

3. **Downloading**: Downloads discovered videos using yt-dlp
   - Sequential downloads within each movie
   - Parallel processing across multiple movies (configurable)
   - Automatic cleanup of failed downloads

4. **Conversion**: Converts videos to x265/HEVC format
   - Hardware acceleration (NVENC/QSV) when available
   - Software encoding (libx265) as fallback
   - CRF 24-26 for optimal quality/size balance

5. **Organization**: Moves converted files to appropriate subdirectories
   - Creates Jellyfin-compatible folder structure
   - Cleans up temporary files
   - Creates `.extras_done` marker to prevent reprocessing

#### For TV Series:

1. **Scanning**: Recursively scans the library directory
   - Detects folders with Season XX subfolders (series folders)
   - Parses folder names to extract series title and optional year
   - Identifies available seasons and checks for Season 0 (specials)
   - Skips folders with `.extras_done` marker (unless `--force` is used)

2. **Discovery**: Queries multiple sources for series extras
   - **TMDB**: Series-level trailers, interviews, behind-the-scenes content
   - **TMDB Season 0**: Official special episodes (if `--specials` enabled)
   - **Season-specific extras**: Bonus content for individual seasons (if `--season-extras` enabled)
   - **YouTube**: Community-uploaded series extras with smart filtering
   - **Season packs**: Extracts and organizes bonus content from downloaded archives
   - **Local Season 0 import**: Scans for existing Season 0 files and organizes them

3. **Downloading**: Downloads discovered videos using yt-dlp
   - Sequential downloads within each series
   - Parallel processing across multiple series (configurable)
   - Automatic cleanup of failed downloads

4. **Conversion**: Converts videos to x265/HEVC format
   - Same hardware acceleration and quality settings as movies
   - Preserves original quality for special episodes

5. **Organization**: Moves converted files to appropriate subdirectories
   - Series-level extras go to series root subdirectories (trailers, interviews, etc.)
   - Season-specific extras go to season subdirectories
   - Season 0 specials go to `Season 00` folder with proper naming format
   - Creates `.extras_done` marker to prevent reprocessing

### Content Filtering

YouTube content is intelligently filtered to exclude:

- Videos shorter than 30 seconds or longer than 20 minutes
- Videos with keywords: "Review", "Reaction", "Analysis", "Explained", "Ending", "Theory", "React"
- YouTube Shorts (vertical videos under 60 seconds)

### Advanced Features

**Metadata Caching**: Series metadata from TMDB is cached for 7 days to reduce API calls on repeated runs.

**Fuzzy Title Matching**: Uses 80% similarity threshold to match extras to series/movies, handling minor title variations.

**Season Pack Extraction**: Automatically extracts bonus content from season pack archives and organizes by content type.

**Local Season 0 Import**: Scans for existing Season 0 files in series folders and organizes them with proper naming.

### Idempotency

The tool is designed to be safely re-runnable:

- Completed items are marked with a `.extras_done` file
- Subsequent runs skip marked items (unless `--force` is used)
- Interrupted processing can be safely resumed
- Temporary files are cleaned up on exit
- Mixed libraries (movies + series) are handled correctly

## Troubleshooting

### "Missing binary: yt-dlp"

- Install yt-dlp: https://github.com/yt-dlp/yt-dlp#installation
- Ensure it's in your system PATH

### "Missing binary: ffmpeg"

- Install ffmpeg: https://ffmpeg.org/download.html
- Ensure it's in your system PATH

### "Unsupported codec"

- Your ffmpeg installation doesn't support HEVC/x265
- Install a version with libx265, hevc_nvenc, or hevc_qsv support

### "Missing API key: TMDB_API_KEY"

- The tool couldn't find your TMDB API key in config.cfg or environment variable
- On first run, you'll be prompted to enter your API key
- Get an API key from: https://www.themoviedb.org/settings/api
- The key will be saved to config.cfg automatically
- Alternatively, set the environment variable: `export TMDB_API_KEY=your_key`

### Downloads failing

- Check your internet connection
- Verify yt-dlp is up to date: `yt-dlp -U`
- Some videos may be region-restricted or removed

### Slow processing

- Increase concurrency: `--concurrency 4`
- Check if hardware acceleration is being used (look for NVENC/QSV in logs)
- Network speed affects download times

## TV Series Troubleshooting

### Series not being detected

- Ensure series folders follow the naming convention: `Series Name (YYYY)` or `Series Name`
- Verify series folders contain `Season XX` subfolders (e.g., `Season 01`, `Season 02`)
- Check that season folders are named exactly as `Season XX` with zero-padded numbers
- Use `--verbose` flag to see detailed scanning output

### Season 0 specials not being discovered

- Ensure `--specials` flag is enabled: `extras_fetcher --series-only --specials /media/tv`
- Not all series have Season 0 content available on TMDB
- Check TMDB directly to verify if Season 0 exists for the series

### Season-specific extras not being found

- Ensure `--season-extras` flag is enabled: `extras_fetcher --series-only --season-extras /media/tv`
- Season-specific extras may not be available for all seasons
- YouTube search may have limited results for specific seasons

### Series extras organized incorrectly

- Verify series folder structure matches Jellyfin requirements
- Check that season folders are named `Season XX` with zero-padded numbers
- Season 0 specials should be in `Season 00` folder
- Series-level extras go to subdirectories at series root (trailers, interviews, etc.)
- Season-specific extras go to subdirectories within season folders

### Mixed library processing issues

- Use `--series-only` to process only series: `extras_fetcher --series-only /media/library`
- Use `--movies-only` to process only movies: `extras_fetcher --movies-only /media/library`
- Omit both flags to process everything: `extras_fetcher /media/library`
- Ensure movies and series are in the same root directory for mixed processing

### Cache-related issues

- Clear metadata cache to force fresh TMDB queries: `rm -rf /path/to/series/.cache`
- Use `--force` flag to bypass cache and reprocess: `extras_fetcher --force --series-only /media/tv`
- Cache is automatically refreshed after 7 days

### Season pack extraction not working

- Verify downloaded files are valid archive formats (zip, rar, 7z, etc.)
- Check that yt-dlp successfully downloaded the archive
- Use `--verbose` flag to see extraction details
- Manually extract and organize if automatic extraction fails

### Local Season 0 import not finding files

- Ensure Season 0 files follow the naming pattern: `S00E{number}` (e.g., `S00E01`, `S00E02`)
- Files can be anywhere in the series folder; they'll be moved to `Season 00`
- Use `--verbose` flag to see which files are being imported

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Code Quality

```bash
# Check for errors
cargo check

# Run linter
cargo clippy -- -D warnings

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

### Project Structure

```
src/
├── main.rs          - Entry point and orchestration
├── cli.rs           - Command-line interface
├── scanner.rs       - Directory scanning and movie discovery
├── discovery.rs     - Multi-source content discovery
├── downloader.rs    - Video downloading with yt-dlp
├── converter.rs     - Video format conversion with ffmpeg
├── organizer.rs     - File organization and done markers
├── validation.rs    - Dependency validation
├── output.rs        - CLI output formatting
├── models.rs        - Data structures and types
└── error.rs         - Error type definitions
```

## License

[Add your license information here]

## Contributing

[Add contribution guidelines here]

## Acknowledgments

- Built with Rust 2024 edition
- Uses yt-dlp for video downloading
- Uses ffmpeg for video conversion
- TheMovieDB for official content metadata
- Archive.org for historical content
