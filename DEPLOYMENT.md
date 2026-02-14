# Deployment Checklist

## Build Information

**Version:** 0.1.0  
**Build Date:** 2026-02-14  
**Binary Size:** ~6.3 MB (6,351,360 bytes)  
**Target:** Release (optimized)

## Pre-Deployment Verification

### ✅ Build Quality Checks

- [x] `cargo build --release` - Compiles without errors
- [x] `cargo test` - All 412 tests passing
- [x] `cargo clippy -- -D warnings` - No warnings
- [x] `cargo fmt -- --check` - Code properly formatted
- [x] Binary created successfully at `target/release/extras_fetcher.exe`

### ✅ CLI Functionality Tests

- [x] `--help` flag displays usage information correctly
- [x] `--version` flag displays version (0.1.0)
- [x] `--force` flag recognized and displayed in config
- [x] `--mode` flag accepts "all" and "youtube" values
- [x] `--concurrency` flag accepts numeric values
- [x] `--verbose` flag recognized and displayed in config
- [x] Colored output working (banner, config, errors)
- [x] Error messages properly formatted with symbols (✗, •)

### ✅ Validation Tests

- [x] Missing TMDB_API_KEY detected and reported
- [x] Installation instructions displayed on validation failure
- [x] Exit code 1 on validation failure
- [x] Descriptive error messages for missing dependencies

### ✅ Documentation

- [x] README.md - Comprehensive usage guide
- [x] INSTALL.md - Detailed installation instructions
- [x] DEPLOYMENT.md - This deployment checklist
- [x] Cargo.toml - Proper metadata and dependencies
- [x] Code comments and documentation

## Binary Distribution

### Release Artifacts

The following files should be included in the release:

1. **Binary:**
   - `extras_fetcher` (Linux/macOS)
   - `extras_fetcher.exe` (Windows)

2. **Documentation:**
   - `README.md`
   - `INSTALL.md`
   - `LICENSE` (if applicable)

3. **Optional:**
   - `CHANGELOG.md` (for version history)
   - Example configuration files

### Platform-Specific Builds

To create builds for different platforms:

```bash
# Linux (x86_64)
cargo build --release --target x86_64-unknown-linux-gnu

# macOS (Intel)
cargo build --release --target x86_64-apple-darwin

# macOS (Apple Silicon)
cargo build --release --target aarch64-apple-darwin

# Windows (x86_64)
cargo build --release --target x86_64-pc-windows-msvc
```

## Installation Verification

After installation, users should verify:

```bash
# 1. Check binary is accessible
extras_fetcher --version
# Expected: extras_fetcher 0.1.0

# 2. Check dependencies
yt-dlp --version
ffmpeg -version
echo $TMDB_API_KEY  # Should show API key

# 3. Test with help flag
extras_fetcher --help
# Should display usage information with colored output
```

## Known Limitations

1. **Network Dependency:** Requires active internet connection for content discovery and downloading
2. **External Tools:** Depends on yt-dlp and ffmpeg being installed and in PATH
3. **API Rate Limits:** TMDB API has rate limits (40 requests per 10 seconds)
4. **Storage Requirements:** Temporary downloads require significant disk space
5. **Processing Time:** Large libraries may take hours to process depending on content availability

## Performance Characteristics

- **Concurrency:** Default 2 movies, configurable up to system limits
- **Memory Usage:** ~50-100 MB base + temporary file buffers
- **CPU Usage:** High during video conversion (especially software encoding)
- **Network Usage:** Varies based on video sizes (typically 10-500 MB per video)
- **Disk I/O:** Heavy during download and conversion phases

## Troubleshooting Guide

### Common Issues

1. **"Missing binary" errors:**
   - Solution: Install yt-dlp and ffmpeg, ensure they're in PATH
   - Verification: Run `yt-dlp --version` and `ffmpeg -version`

2. **"Missing API key" error:**
   - Solution: Set TMDB_API_KEY environment variable
   - Verification: `echo $TMDB_API_KEY` (should show key)

3. **"Unsupported codec" error:**
   - Solution: Install ffmpeg with HEVC/x265 support
   - Verification: `ffmpeg -codecs | grep hevc`

4. **Slow processing:**
   - Solution: Increase concurrency with `--concurrency 4`
   - Note: Hardware acceleration (NVENC/QSV) significantly improves speed

5. **Download failures:**
   - Solution: Update yt-dlp to latest version
   - Note: Some content may be region-restricted or removed

## Security Considerations

1. **API Key Protection:**
   - Store TMDB_API_KEY securely
   - Don't commit API keys to version control
   - Use environment variables, not hardcoded values

2. **File System Access:**
   - Tool requires read/write access to movie library
   - Creates temporary directories in `/tmp_downloads/`
   - Cleans up temporary files on exit

3. **Network Security:**
   - All API calls use HTTPS
   - yt-dlp handles video downloads securely
   - No sensitive data transmitted except API key

4. **Input Validation:**
   - Directory paths validated before processing
   - Folder names parsed with regex to prevent injection
   - Exit codes properly handled

## Support and Maintenance

### Updating the Tool

```bash
# Pull latest changes
git pull origin main

# Rebuild
cargo build --release

# Reinstall
sudo cp target/release/extras_fetcher /usr/local/bin/  # Linux/macOS
```

### Reporting Issues

When reporting issues, include:
- extras_fetcher version (`extras_fetcher --version`)
- Operating system and version
- yt-dlp version (`yt-dlp --version`)
- ffmpeg version (`ffmpeg -version`)
- Error messages and logs (use `--verbose` flag)
- Steps to reproduce the issue

### Log Collection

For debugging, enable verbose logging:

```bash
# Verbose mode
extras_fetcher --verbose /path/to/library

# Debug logging
RUST_LOG=debug extras_fetcher /path/to/library

# Trace logging (very detailed)
RUST_LOG=trace extras_fetcher /path/to/library 2>&1 | tee extras_fetcher.log
```

## Release Checklist

Before creating a new release:

- [ ] Update version in `Cargo.toml`
- [ ] Update version in `README.md`
- [ ] Update `CHANGELOG.md` with changes
- [ ] Run full test suite: `cargo test`
- [ ] Run clippy: `cargo clippy -- -D warnings`
- [ ] Build release binaries for all platforms
- [ ] Test binaries on target platforms
- [ ] Update documentation if needed
- [ ] Create git tag: `git tag v0.1.0`
- [ ] Push tag: `git push origin v0.1.0`
- [ ] Create GitHub release with binaries and notes

## Post-Deployment

After deployment, monitor:

1. **User Feedback:** Issues, feature requests, bug reports
2. **Performance:** Processing times, resource usage
3. **Compatibility:** New versions of yt-dlp, ffmpeg, TMDB API
4. **Dependencies:** Security updates, breaking changes

## Future Enhancements

Potential improvements for future versions:

- [ ] Configuration file support (TOML/YAML)
- [ ] Resume interrupted downloads
- [ ] Parallel downloads within a movie
- [ ] Custom content filtering rules
- [ ] Web UI for monitoring progress
- [ ] Docker container support
- [ ] Automatic dependency installation
- [ ] Progress bar for individual downloads
- [ ] Dry-run mode to preview actions
- [ ] Statistics and reporting

---

**Deployment Status:** ✅ Ready for Release  
**Last Updated:** 2026-02-14  
**Approved By:** [Your Name/Team]
