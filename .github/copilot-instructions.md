# MONANA - GitHub Copilot Instructions

Always reference these instructions first and fallback to search or bash commands only when you encounter unexpected information that does not match the info here.

## Working Effectively

### Bootstrap, Build, and Test
- **NEVER CANCEL builds or tests** - Set timeouts to 60+ minutes for builds, 30+ minutes for tests
- **Initial setup (fresh clone):**
  - `cd /path/to/monana`
  - `cargo build` -- NEVER CANCEL: Takes 45-60 seconds on fresh clone. Set timeout to 10+ minutes.
  - `cargo test --verbose` -- Takes 3-5 seconds. All tests must pass (4 tests in pipeline module).
- **Development commands:**
  - `cargo check` -- Fast compilation check (under 1 second after initial build)
  - `cargo clippy --all-targets --all-features -- -D warnings` -- NEVER CANCEL: Takes 20-30 seconds. Must pass with no warnings.
  - `cargo fmt --all -- --check` -- Instant formatting check. Must pass.

### Running the Application
- **ALWAYS build first** before running the application
- **CLI Usage:**
  ```bash
  ./target/debug/monana --help  # Show all options
  ./target/debug/monana --config monana.yaml --input-cmdline /path/to/media --dry-run --verbose
  ```
- **Required arguments:**
  - `--input-cmdline <PATH>` - Path to media files or directory
  - `--config <CONFIG>` - Configuration file (default: monana.yaml)
- **Common options:**
  - `--dry-run` - Preview actions without executing (ALWAYS use for testing)
  - `--verbose` - Show detailed processing information
  - `--recursive` - Process directories recursively

### Development Environment
- **Standard Rust environment** - Uses cargo for all operations
- **No special build tools required** - Standard cargo commands work
- **Optional Nix + direnv setup** available but not required
- **Pre-commit hooks** configured (rustfmt, clippy, cargo test) but not required

## Validation

### ALWAYS Validate Changes
1. **Build and test after any code changes:**
   ```bash
   cargo build  # NEVER CANCEL: 3-4 minutes
   cargo test --verbose  # 3-5 seconds, 4 tests must pass
   ```
2. **Lint and format before committing:**
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings  # NEVER CANCEL: 20-30 seconds
   cargo fmt --all -- --check  # Instant
   ```
3. **Test CLI functionality:**
   ```bash
   ./target/debug/monana --help
   ./target/debug/monana --config test_config.yaml --input-cmdline /tmp/test --dry-run
   ```

### CRITICAL: Timeout Values
- **cargo build**: Set timeout to 10+ minutes (typically 45-60 seconds clean, 3-5 seconds incremental)
- **cargo test**: Set timeout to 5+ minutes (typically 3-5 seconds)
- **cargo clippy**: Set timeout to 5+ minutes (typically 20-30 seconds)
- **NEVER CANCEL** any build or test command - they may appear to hang but are still working

### Validation Scenarios
Always test these scenarios after making changes:
1. **Configuration validation**: 
   ```bash
   ./target/debug/monana --config monana.yaml --input-cmdline /tmp --dry-run
   ./target/debug/monana --config test_config.yaml --input-cmdline /tmp --dry-run
   ```
2. **CLI help**: `./target/debug/monana --help` must show usage information
3. **Error handling**: 
   ```bash
   # Test with invalid config
   echo "invalid: yaml" > /tmp/bad.yaml
   ./target/debug/monana --config /tmp/bad.yaml --input-cmdline /tmp --dry-run
   # Should show: "Error: Failed to parse configuration"
   
   # Test with non-existent path
   ./target/debug/monana --config monana.yaml --input-cmdline /nonexistent --dry-run
   # Should show: "Path does not exist: /nonexistent"
   ```
4. **Dry run validation**: Always use `--dry-run` for testing - no files should be modified
5. **Verbose output**: Test with `--verbose` to ensure detailed information is shown

## Common Tasks

### Repository Structure
```
.
├── Cargo.toml              # Rust project configuration
├── README.md               # Project documentation
├── CLAUDE.md               # Development environment guide
├── src/                    # Source code
│   ├── main.rs            # CLI entry point
│   ├── lib.rs             # Library root
│   ├── pipeline/          # Rule engine and processing
│   ├── metadata/          # EXIF extraction and analysis
│   ├── actions/           # File operations (move, copy, etc.)
│   ├── config/            # Configuration management
│   └── template/          # Template variable resolution
├── monana.yaml            # Default configuration
├── test_config.yaml       # Test configuration with examples
├── .github/               # GitHub configuration
├── flake.nix              # Nix development environment (optional)
└── target/                # Build artifacts (not committed)
```

### Key Modules
- **src/main.rs**: CLI interface and argument parsing
- **src/pipeline/**: Core rule processing engine with Rhai scripting
- **src/metadata/**: EXIF extraction, GPS processing, reverse geocoding
- **src/actions/**: File operations (move, copy, symlink, hardlink, custom commands)
- **src/template/**: Variable substitution system

### Configuration Files
- **monana.yaml**: Production-ready configuration with photo/video organization
- **test_config.yaml**: Comprehensive test configuration with EXIF examples
- Both use YAML format with `actions` and `rulesets` sections

### Dependencies
- **Rust 1.88.0+** required
- **Key dependencies**: rhai (scripting), nom-exif (metadata), chrono (time), clap (CLI)
- **No external tools required** for basic functionality
- **Optional**: ImageMagick for image processing actions, ffmpeg for video processing

## Project Architecture

### Pipeline Model
- **Declarative rulesets** with conditions, templates, and actions
- **Input sources**: cmdline, path, watch, ruleset chaining
- **Safe Rhai scripting** for complex conditions
- **Template variables**: time, space, source, meta, type, special

### Template Variables Available
- **time**: `{time.yyyy}`, `{time.mm}`, `{time.dd}`, `{time.month_name}`, etc.
- **space**: `{space.country}`, `{space.city}`, `{space.road}` (from GPS/reverse geocoding)
- **source**: `{source.name}`, `{source.extension}`, `{source.original}`
- **meta**: `{meta.*}` - ANY EXIF tag by name (e.g., `{meta.Make}`, `{meta.FNumber}`)
- **type**: "image" or "video" (used in conditions)
- **special**: `{special.md5_short}`, `{special.count}` (for collision handling)

### Actions Available
- **Built-in**: move, copy, symlink, hardlink
- **Custom**: User-defined commands with template variable substitution
- **All operations respect dry-run mode**

## Troubleshooting

### Common Issues
1. **"No media files found"**: Application only processes real image/video files, not test files
2. **Build failures**: Check Rust version (1.88.0+), run `cargo clean` and retry
3. **Test failures**: Usually indicates code issues, all 4 tests should pass
4. **Clippy warnings**: Must be fixed before committing, use `-D warnings` flag

### Performance Notes
- **First build**: 45-60 seconds (downloads and compiles dependencies)
- **Incremental builds**: 3-5 seconds (only changed files)
- **Tests**: Very fast (3-5 seconds) with good coverage
- **Media processing**: Depends on file count and EXIF complexity

## Development Tips
- **Use `--dry-run` extensively** when testing configuration changes
- **Test with `--verbose`** to understand rule matching behavior
- **The test_config.yaml file** contains comprehensive examples of all features
- **EXIF metadata access is type-aware**: numeric values stay numeric for comparisons
- **GPS coordinates enable reverse geocoding** for location-based organization
- **Template variables are case-sensitive**: Use exact names like `{time.yyyy}`, `{meta.Make}`
- **Rhai conditions use dot notation**: `space.city`, `time.yyyy`, `meta.FNumber`
- **Always test with real media files** for full functionality validation (app only processes actual images/videos)

### Development Workflow
1. **Make code changes**
2. **Build and test immediately**: `cargo build && cargo test --verbose`
3. **Test CLI functionality**: `./target/debug/monana --help`
4. **Test with configurations**: Try both `monana.yaml` and `test_config.yaml`
5. **Run linting**: `cargo clippy --all-targets --all-features -- -D warnings`
6. **Check formatting**: `cargo fmt --all -- --check`
7. **Validate with dry-run scenarios** before committing

### Code Organization
- **Pipeline rules**: Core logic in `src/pipeline/mod.rs` with 4 unit tests
- **EXIF handling**: All metadata extraction in `src/metadata/` modules
- **Template system**: Variable resolution in `src/template/mod.rs`
- **CLI interface**: Argument parsing and main logic in `src/main.rs`
- **Actions**: File operations implemented in `src/actions/mod.rs`