# Configuration File Implementation

## Summary

Implemented a configuration file system for storing the TMDB API key, replacing the previous environment variable-only approach.

## Changes Made

### New Files
- `src/config.rs` - Configuration file management module
- `config.cfg.example` - Sample configuration file for users
- `CHANGELOG_CONFIG.md` - This file documenting the changes

### Modified Files

#### Core Implementation
- `src/lib.rs` - Added config module declaration
- `src/main.rs` - Updated module imports and error messages
- `src/error.rs` - Added ConfigError enum for configuration-related errors
- `src/validation.rs` - Updated to check config file first, then environment variable

#### Documentation
- `README.md` - Updated with config file instructions and examples
- `INSTALL.md` - Updated installation instructions with config file approach
- `.gitignore` - Added config.cfg to prevent committing API keys

### Features

#### Configuration Priority
The tool now checks for the TMDB API key in this order:
1. `config.cfg` file (recommended)
2. `TMDB_API_KEY` environment variable (backward compatibility)

#### Interactive Prompt
On first run, if no config file exists and no environment variable is set, the tool will:
1. Display instructions on how to get a TMDB API key
2. Prompt the user to enter their API key
3. Save the key to `config.cfg` for future use

#### Config File Format
```json
{
  "tmdb_api_key": "your_api_key_here"
}
```

### Testing

#### Test Updates
- Updated validation tests to avoid interactive prompts during testing
- Added internal `check_tmdb_api_key_internal()` method with `allow_prompt` parameter
- All existing tests pass without hanging

#### New Tests
- `config::tests::test_config_save_and_load` - Config serialization/deserialization
- `config::tests::test_config_load_nonexistent` - Missing file handling
- `config::tests::test_config_load_invalid_json` - Invalid JSON handling
- `config::tests::test_config_default_path` - Default path verification
- `config::tests::test_config_serialization` - JSON serialization
- `config::tests::test_config_deserialization` - JSON deserialization

### Code Quality

All code quality checks pass:
- ✅ `cargo build` - Compiles without errors or warnings
- ✅ `cargo test` - All 206 tests pass
- ✅ `cargo clippy -- -D warnings` - No clippy warnings
- ✅ `cargo fmt` - Code properly formatted

### Backward Compatibility

The implementation maintains full backward compatibility:
- Existing users with `TMDB_API_KEY` environment variable will continue to work
- Environment variable is checked as a fallback if config file is not found
- No breaking changes to existing functionality

### User Experience Improvements

1. **Easier Setup**: Users no longer need to set environment variables
2. **Persistent Configuration**: API key is saved and doesn't need to be set per session
3. **Clear Instructions**: Interactive prompt provides step-by-step guidance
4. **Better Security**: Config file can be easily excluded from version control

### Migration Guide

For existing users:

**Option 1: Continue using environment variable**
- No changes needed, everything works as before

**Option 2: Migrate to config file**
1. Create `config.cfg` in the project directory
2. Add your API key in JSON format
3. Remove the environment variable (optional)

**Option 3: Let the tool create it**
1. Remove the environment variable
2. Run the tool - it will prompt for your API key
3. The config file will be created automatically
