# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

MONANA is a high-performance, rule-driven media archival system written in Rust. It uses a declarative pipeline model powered by the Rhai scripting engine to organize media files based on metadata like EXIF data, timestamps, and GPS information. The core philosophy is that every photograph and video is a record of a specific moment at a specific location.

## Development Environment

This project uses **Nix + Direnv** as the official development environment. The user will drop you in an environment where `direnv allow` has been executed with a complete Rust development setup.

## Development Commands

### Devshell Commands (Preferred)

- `build` - Build the project (cargo build)
- `test` - Run tests (cargo test)
- `check` - Run cargo check
- `fmt` - Format all files with treefmt
- `install-hooks` - Install git hooks (auto-installed on devshell entry)

### Direct Cargo Commands (CI-equivalent)

- `cargo build` - Build the project
- `cargo test --verbose` - Run tests (matches CI)
- `cargo clippy --all-targets --all-features -- -D warnings` - Lint (matches CI)
- `cargo fmt --all -- --check` - Check formatting (matches CI)

## Pre-commit Hooks

Git hooks automatically run on commit and match CI requirements:

- `rustfmt` - Code formatting check
- `cargo-clippy` - Linting with warnings as errors
- `cargo-test` - Full test suite

## Architecture Overview

The system follows a declarative pipeline model with these core components:

### Data Acquisition Pipeline

1. **Ingestion** - Identify media files for processing
2. **Temporal Analysis** - Extract creation timestamps (EXIF DateTimeOriginal → filesystem fallback)
3. **Spatial Analysis** - Extract GPS coordinates (EXIF GPS → Google Maps History fallback)
4. **Data Augmentation** - Generate template variables from raw metadata

### Core Components

- `src/pipeline/` - Declarative pipeline engine and ruleset processing with Rhai script evaluation
- `src/metadata/` - EXIF extraction (ALL tags exposed), parsing, reverse geocoding, temporal/spatial analysis
- `src/actions/` - Built-in actions (move, copy, symlink, hardlink) and custom command invocation
- `src/template/` - Template variable resolution with dynamic type handling
- `src/main.rs` - CLI entry point with --input-cmdline flag for batch processing
- `tests/` - Unit and scenario coverage

## Key Concepts

### Rulesets

Pipeline stages that process media files. Each ruleset has:

- **name**: Unique identifier
- **input**: Source specification (`cmdline`, `path:`, `watch:`, `ruleset:`)
- **rules**: Ordered list of condition/template/action triplets

### Template Variables

Rich context variables available for path templates and conditions:

- **time**: `{time.yyyy}`, `{time.mm}`, `{time.dd}`, `{time.month_name}`, etc.
- **space**: `{space.country}`, `{space.city}`, `{space.road}`, etc. (uses country codes like ES, FR)
- **source**: `{source.name}`, `{source.extension}`, `{source.original}`, etc.
- **type**: Media type accessed as `type` in conditions ("image" or "video")
- **meta**: `{meta.*}` - Access ANY EXIF tag by name with proper types
  - Examples: `{meta.Make}`, `{meta.Model}`, `{meta.FNumber}`, `{meta.ISO}`, `{meta.FocalLength}`
  - Numeric values stay numeric for comparisons: `meta.FNumber <= 2.8`
  - Missing tags return empty Dynamic values (safe to reference)
- **special**: `{special.md5_short}`, `{special.count}` (collision handling)

### Actions

- **Built-in**: move, copy, symlink, hardlink
- **Custom**: User-defined commands with template variable substitution

## Key Dependencies

The project uses research-based, production-ready dependencies:

### Core Dependencies

- **nom-exif** - Unified metadata extraction for images and videos with zero-copy parsing
- **image** - Comprehensive image processing with support for all major formats
- **figment** - Advanced hierarchical configuration management (YAML, TOML, JSON, env)
- **rhai** - Safe, embedded scripting engine for custom logic
- **walkdir** - High-performance directory traversal
- **camino** - UTF-8 path handling with cross-platform guarantees
- **geo** - GPS coordinate processing and geographic calculations
- **tree_magic_mini** - Fast file format detection (~150ns per check)
- **chrono** - Date/time processing with EXIF compatibility
- **notify** - File system monitoring for daemon mode

### Optional Tier 2 Dependencies (add as needed)

- **rawloader** - RAW image format support
- **mp4parse** - Lightweight video metadata extraction
- **kamadak-exif** - Pure Rust EXIF fallback
- **zen-engine** - Business rules engine for complex decision logic

## Configuration Format

Uses Figment for flexible configuration management supporting:

- YAML, TOML, JSON formats
- Environment variable overrides
- Hierarchical profiles (default, debug, production)
- Automatic path resolution relative to config files

Configuration sections:

- **location_history_path** (optional): Path to Google Maps Timeline JSON export for GPS fallback
- **actions**: Custom command definitions
- **rulesets**: Pipeline stage definitions with input sources and processing rules

### Location History Integration

MONANA can use Google Maps Timeline location history as a fallback for photos without EXIF GPS data:

```yaml
location_history_path: "path/to/location_history.json"
```

When configured:
- Photos without EXIF GPS coordinates will search the location history
- The closest location point within 48 hours of the photo's timestamp is used
- Coordinates are converted from E7 format and reverse geocoded
- This provides seamless GPS data for all photos in your archive

## Testing Strategy

Run the full test suite that matches CI behavior:

```bash
cargo test --verbose
```

The project emphasizes safety and sandboxing - all user-defined conditions run in Rhai sandbox, and custom commands are opt-in and explicitly defined.

## CLI Usage

The main command runs all cmdline rulesets on a given path:

```bash
monana --config <CONFIG_FILE> --input-cmdline <PATH> [--location-history <JSON>] [--dry-run] [--verbose] [--recursive]
```

Options:

- `--config` / `-c`: Configuration file (default: monana.yaml)
- `--input-cmdline`: Path to process (required)
- `--location-history`: Google Maps Timeline JSON file (overrides config)
- `--dry-run` / `-d`: Preview actions without executing
- `--verbose` / `-v`: Show detailed processing information
- `--recursive` / `-R`: Process directories recursively

## Implementation Details

### EXIF Metadata Handling

- All EXIF tags are extracted and stored in `MediaContext.meta` as a `HashMap<String, Dynamic>`
- The Dynamic type from Rhai preserves numeric types for proper comparisons
- Template resolution converts Dynamic values to strings when needed
- Missing EXIF tags return empty values instead of errors

### Rhai Condition Evaluation

- Conditions use dot notation: `space.city`, `time.yyyy`, `meta.FNumber`
- The pipeline creates Rhai object maps to support this syntax
- Numeric comparisons work directly: `meta.ISO >= 3200`
- String comparisons require quotes: `space.country == "ES"`

## Developer Reminders

- Always run clippy before committing: `cargo clippy --all-targets --all-features -- -D warnings`
- Test with real photos to verify GPS/EXIF extraction
- Use `--dry-run` when testing configuration changes
