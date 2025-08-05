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

### Core Components (planned structure)
- `src/pipeline/` - Declarative pipeline engine and ruleset processing
- `src/metadata/` - EXIF extraction, parsing, reverse geocoding, temporal/spatial analysis
- `src/actions/` - Built-in actions (move, copy, symlink, hardlink) and custom command invocation
- `src/config/` - YAML configuration deserialization and validation
- `rhai/` - Sandboxed scripting hooks for custom conditions
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
- **space**: `{space.country}`, `{space.city}`, `{space.road}`, etc.
- **source**: `{source.name}`, `{source.extension}`, `{source.original}`, etc.
- **media**: `{media.type}`, `{media.width}`, `{media.height}`, `{media.duration}`, etc.
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
- **actions**: Custom command definitions
- **rulesets**: Pipeline stage definitions with input sources and processing rules

## Testing Strategy

Run the full test suite that matches CI behavior:
```bash
cargo test --verbose
```

The project emphasizes safety and sandboxing - all user-defined conditions run in Rhai sandbox, and custom commands are opt-in and explicitly defined.