# extras_fetcher

A Rust-based automation tool for enriching Jellyfin movie libraries by discovering, downloading, and organizing supplementary video content (trailers, behind-the-scenes footage, deleted scenes, featurettes) from multiple online sources.

## Features

- **Multi-source content discovery**: Automatically finds extras from TheMovieDB, Archive.org, and YouTube
- **Automated downloading**: Uses yt-dlp to fetch videos from discovered sources
- **Hardware-accelerated conversion**: Converts videos to efficient x265/HEVC format using NVENC, QSV, or software encoding
- **Jellyfin-compatible organization**: Organizes content into standard subdirectories that Jellyfin automatically recognizes
- **Idempotent execution**: Safe to run multiple times - tracks completed movies with done markers
- **Configurable processing**: Control concurrency, source filtering, and processing modes
- **Rich CLI output**: Colored progress indicators and detailed status information

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
  <ROOT_DIRECTORY>  Path to the root directory containing movie folders

Options:
  -f, --force              Ignore done markers and reprocess all movies
  -m, --mode <MODE>        Content source mode [default: all] [possible values: all, youtube]
  -c, --concurrency <N>    Maximum number of movies to process concurrently [default: 2]
  -v, --verbose            Enable verbose logging output
  -h, --help               Print help information
  -V, --version            Print version information
```

### Examples

**Process a movie library with default settings:**
```bash
extras_fetcher /media/movies
```

**Reprocess all movies (ignore done markers):**
```bash
extras_fetcher --force /media/movies
```

**Use only YouTube as a content source:**
```bash
extras_fetcher --mode youtube /media/movies
```

**Process 4 movies concurrently with verbose logging:**
```bash
extras_fetcher --concurrency 4 --verbose /media/movies
```

**Enable debug logging:**
```bash
RUST_LOG=debug extras_fetcher /media/movies
```

## Directory Structure

### Input Structure

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

### Output Structure

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
    └── done.ext  (completion marker)
```

## How It Works

### Processing Pipeline

1. **Scanning**: Recursively scans the movie library directory
   - Parses folder names to extract movie title and year
   - Skips folders with `done.ext` marker (unless `--force` is used)

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
   - Creates `done.ext` marker to prevent reprocessing

### Content Filtering

YouTube content is intelligently filtered to exclude:
- Videos shorter than 30 seconds or longer than 20 minutes
- Videos with keywords: "Review", "Reaction", "Analysis", "Explained", "Ending", "Theory", "React"
- YouTube Shorts (vertical videos under 60 seconds)

### Idempotency

The tool is designed to be safely re-runnable:
- Completed movies are marked with a `done.ext` file
- Subsequent runs skip marked movies (unless `--force` is used)
- Interrupted processing can be safely resumed
- Temporary files are cleaned up on exit

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
