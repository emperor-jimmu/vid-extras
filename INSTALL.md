# Installation Instructions

## Prerequisites

The extras_fetcher tool requires three external dependencies to function:

1. **yt-dlp** - Video downloading utility
2. **ffmpeg** - Video conversion with HEVC/x265 support
3. **TMDB API Key** - For movie metadata and content discovery

## Installing Dependencies

### 1. yt-dlp Installation

**Windows:**
```powershell
# Using winget
winget install yt-dlp

# Or download the executable
# Visit: https://github.com/yt-dlp/yt-dlp/releases
# Download yt-dlp.exe and add to PATH
```

**Linux:**
```bash
# Using pip
pip install yt-dlp

# Or using package manager (Ubuntu/Debian)
sudo apt install yt-dlp

# Or download binary
sudo curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp
```

**macOS:**
```bash
# Using Homebrew
brew install yt-dlp

# Or using pip
pip install yt-dlp
```

### 2. ffmpeg Installation

**Windows:**
```powershell
# Using winget
winget install ffmpeg

# Or using Chocolatey
choco install ffmpeg

# Or download from: https://ffmpeg.org/download.html
# Extract and add to PATH
```

**Linux:**
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install ffmpeg

# Fedora
sudo dnf install ffmpeg

# Arch Linux
sudo pacman -S ffmpeg
```

**macOS:**
```bash
# Using Homebrew
brew install ffmpeg
```

**Verify HEVC Support:**
```bash
ffmpeg -codecs | grep hevc
```

You should see output containing `libx265`, `hevc_nvenc`, or `hevc_qsv`.

### 3. TMDB API Key

1. Create a free account at [TheMovieDB](https://www.themoviedb.org/signup)
2. Navigate to [API Settings](https://www.themoviedb.org/settings/api)
3. Request an API key (choose "Developer" option)
4. Copy your API key (v3 auth)

**Configuration Options:**

**Option 1: Using config.cfg (Recommended)**

On first run, extras_fetcher will automatically prompt you to enter your API key and save it to `config.cfg`:

```bash
extras_fetcher /path/to/movies
# You'll be prompted: "Enter your TMDB API key: "
# The key will be saved to config.cfg for future use
```

You can also manually create the config file in the same directory as the executable:

**config.cfg:**
```json
{
  "tmdb_api_key": "your_api_key_here"
}
```

**Option 2: Using Environment Variable (Fallback)**

For backward compatibility, you can still use an environment variable:

**Windows (PowerShell):**
```powershell
# Temporary (current session only)
$env:TMDB_API_KEY = "your_api_key_here"

# Permanent (user-level)
[System.Environment]::SetEnvironmentVariable('TMDB_API_KEY', 'your_api_key_here', 'User')
```

**Windows (Command Prompt):**
```cmd
# Temporary
set TMDB_API_KEY=your_api_key_here

# Permanent
setx TMDB_API_KEY "your_api_key_here"
```

**Linux/macOS:**
```bash
# Temporary (current session)
export TMDB_API_KEY="your_api_key_here"

# Permanent (add to ~/.bashrc or ~/.zshrc)
echo 'export TMDB_API_KEY="your_api_key_here"' >> ~/.bashrc
source ~/.bashrc
```

**Note:** The tool checks for the API key in this order:
1. config.cfg file (prompts if missing)
2. TMDB_API_KEY environment variable

## Installing extras_fetcher

### Option 1: Build from Source

**Requirements:**
- Rust 2024 edition or later
- Cargo (comes with Rust)

**Steps:**
```bash
# Clone the repository
git clone <repository_url>
cd extras_fetcher

# Build release binary
cargo build --release

# Binary will be at: target/release/extras_fetcher (or extras_fetcher.exe on Windows)
```

**Install to system:**

**Linux/macOS:**
```bash
sudo cp target/release/extras_fetcher /usr/local/bin/
```

**Windows:**
```powershell
# Copy to a directory in your PATH, e.g.:
copy target\release\extras_fetcher.exe C:\Windows\System32\
```

### Option 2: Download Pre-built Binary

*(If releases are available)*

1. Download the appropriate binary for your platform from the releases page
2. Extract the archive
3. Move the binary to a directory in your PATH
4. Make it executable (Linux/macOS): `chmod +x extras_fetcher`

## Verification

Verify all dependencies are correctly installed:

```bash
# Check yt-dlp
yt-dlp --version

# Check ffmpeg
ffmpeg -version

# Check TMDB API key (if using environment variable)
echo $TMDB_API_KEY  # Linux/macOS
echo %TMDB_API_KEY%  # Windows CMD
echo $env:TMDB_API_KEY  # Windows PowerShell

# Check if config.cfg exists (if using config file)
# Linux/macOS: cat config.cfg
# Windows: type config.cfg

# Check extras_fetcher
extras_fetcher --version
```

## Quick Start

Once all dependencies are installed:

```bash
# Basic usage
extras_fetcher /path/to/movie/library

# With options
extras_fetcher /path/to/movie/library --mode youtube --concurrency 4 --verbose

# Force reprocess all movies
extras_fetcher /path/to/movie/library --force
```

## Troubleshooting

### "Missing binary: yt-dlp"
- Ensure yt-dlp is installed and in your system PATH
- Try running `yt-dlp --version` to verify

### "Missing binary: ffmpeg"
- Ensure ffmpeg is installed and in your system PATH
- Try running `ffmpeg -version` to verify

### "Unsupported codec"
- Your ffmpeg installation doesn't support HEVC/x265
- Reinstall ffmpeg with full codec support
- On Linux, you may need to install from a different repository (e.g., RPM Fusion for Fedora)

### "Missing API key: TMDB_API_KEY"
- On first run, you'll be prompted to enter your API key
- The key will be saved to config.cfg automatically
- If you prefer environment variables, ensure TMDB_API_KEY is set correctly
- Restart your terminal/shell after setting the variable
- Verify with: `echo $TMDB_API_KEY` (Linux/macOS) or `echo %TMDB_API_KEY%` (Windows)
- Check if config.cfg exists and contains a valid API key

### Hardware Acceleration Not Working
- The tool will automatically fall back to software encoding
- Check ffmpeg encoders: `ffmpeg -encoders | grep hevc`
- For NVIDIA: Ensure CUDA drivers are installed
- For Intel: Ensure QSV drivers are installed

## System Requirements

**Minimum:**
- CPU: Dual-core processor
- RAM: 2 GB
- Storage: 10 GB free space (for temporary downloads)
- Network: Broadband internet connection

**Recommended:**
- CPU: Quad-core processor or better
- RAM: 4 GB or more
- Storage: 50 GB free space
- GPU: NVIDIA GPU with NVENC support or Intel CPU with Quick Sync Video
- Network: High-speed internet connection

## Support

For issues, questions, or feature requests, please visit the project repository or open an issue.
