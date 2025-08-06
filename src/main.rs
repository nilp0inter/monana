use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use std::fs;
use walkdir::WalkDir;

use monana::{
    actions::Action,
    metadata::extractor::extract_metadata,
    pipeline::{Pipeline, RuleEngine},
};

#[derive(Parser)]
#[command(name = "monana")]
#[command(about = "MONANA - Media Organization, Normalization, and Archival via Named Automation")]
struct Args {
    /// Paths to process (files or directories)
    #[arg(value_name = "PATH")]
    paths: Vec<Utf8PathBuf>,

    /// Configuration file
    #[arg(short, long, default_value = "monana.yaml")]
    config: String,

    /// Ruleset to use
    #[arg(short, long, default_value = "organize_photos")]
    ruleset: String,

    /// Process directories recursively
    #[arg(short = 'R', long)]
    recursive: bool,

    /// Dry run - show what would be done without doing it
    #[arg(short, long)]
    dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("🌸 MONANA - Media Archival System");

    // Load configuration
    let config_content = fs::read_to_string(&args.config)
        .with_context(|| format!("Failed to read config file: {}", args.config))?;

    let pipeline: Pipeline =
        serde_yaml::from_str(&config_content).with_context(|| "Failed to parse configuration")?;

    // Find the specified ruleset
    let ruleset = pipeline
        .rulesets
        .iter()
        .find(|r| r.name == args.ruleset && matches!(r.input, monana::pipeline::InputSpec::Cmdline))
        .ok_or_else(|| anyhow::anyhow!("Ruleset '{}' not found", args.ruleset))?;

    println!("📋 Using ruleset: {}", ruleset.name);

    // Create rule engine
    let engine = RuleEngine::new()?;

    // If no paths specified, use current directory
    let paths = if args.paths.is_empty() {
        vec![Utf8PathBuf::from(".")]
    } else {
        args.paths
    };

    // Collect all files to process
    let mut all_files = Vec::new();
    for path in &paths {
        if !path.exists() {
            eprintln!("⚠️  Path does not exist: {path}");
            continue;
        }

        let files = collect_files(path, args.recursive)?;
        all_files.extend(files);
    }

    if all_files.is_empty() {
        println!("⚠️  No media files found");
        return Ok(());
    }

    println!("📁 Found {} file(s) to process", all_files.len());

    if args.dry_run {
        println!("🔍 DRY RUN MODE - No files will be moved");
    }

    // Process each file
    let mut processed = 0;
    let mut matched = 0;
    let mut errors = 0;
    let mut unmatched_files = Vec::new();

    for file_path in all_files {
        match process_file(
            &file_path,
            &ruleset.rules,
            &engine,
            args.dry_run,
            args.verbose,
        ) {
            Ok(rule_matched) => {
                processed += 1;
                if rule_matched {
                    matched += 1;
                } else {
                    unmatched_files.push(file_path.clone());
                }
            }
            Err(e) => {
                errors += 1;
                eprintln!("❌ Error processing {file_path}: {e}");
            }
        }
    }

    // Log unmatched files
    if !unmatched_files.is_empty() {
        println!("\n⚠️  Files with no matching rules:");
        for file in &unmatched_files {
            println!("   - {file}");
        }
    }

    println!("\n🎉 Summary:");
    println!("   Files processed: {processed}");
    println!("   Rules matched: {matched}");
    println!("   No rules matched: {}", processed - matched);
    println!("   Errors: {errors}");

    Ok(())
}

fn collect_files(path: &Utf8Path, recursive: bool) -> Result<Vec<Utf8PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        if is_media_file(path)? {
            files.push(path.to_path_buf());
        }
    } else if path.is_dir() {
        let walker = if recursive {
            WalkDir::new(path).into_iter()
        } else {
            WalkDir::new(path).max_depth(1).into_iter()
        };

        for entry in walker {
            let entry = entry.with_context(|| "Failed to read directory entry")?;

            if entry.file_type().is_file() {
                let file_path = Utf8PathBuf::from_path_buf(entry.path().to_path_buf())
                    .map_err(|_| anyhow::anyhow!("Non-UTF8 path: {:?}", entry.path()))?;

                if is_media_file(&file_path)? {
                    files.push(file_path);
                }
            }
        }
    }

    Ok(files)
}

fn is_media_file(path: &Utf8Path) -> Result<bool> {
    let mime_type = tree_magic_mini::from_filepath(path.as_std_path());

    Ok(mime_type
        .map(|mime| mime.starts_with("image/") || mime.starts_with("video/"))
        .unwrap_or(false))
}

fn process_file(
    file_path: &Utf8PathBuf,
    rules: &[monana::pipeline::Rule],
    engine: &RuleEngine,
    dry_run: bool,
    verbose: bool,
) -> Result<bool> {
    if verbose {
        println!("\n📄 Processing: {file_path}");
    }

    // Extract metadata
    let context = extract_metadata(file_path)?;

    // Test each rule
    for (i, rule) in rules.iter().enumerate() {
        match engine.process_rule(rule, &context) {
            Ok(Some((destination, action_spec))) => {
                // Rule matched!
                if verbose || !dry_run {
                    println!("✅ {file_path} -> {destination}");
                    println!("   Rule {}: {}", i + 1, rule.condition);
                    println!("   Action: {action_spec:?}");
                }

                if !dry_run {
                    // Execute the action
                    let action = match &action_spec {
                        monana::pipeline::ActionSpec::Move => Action::Move,
                        monana::pipeline::ActionSpec::Copy => Action::Copy,
                        monana::pipeline::ActionSpec::Symlink => Action::Symlink,
                        monana::pipeline::ActionSpec::Hardlink => Action::Hardlink,
                        monana::pipeline::ActionSpec::Command(cmd) => {
                            if let Some((_, cmd_name)) = cmd.split_once(':') {
                                // TODO: Look up custom command from config
                                eprintln!("⚠️  Custom commands not yet implemented: {cmd_name}");
                                return Ok(true);
                            } else {
                                anyhow::bail!("Invalid command spec: {}", cmd);
                            }
                        }
                    };

                    // Create destination directory if needed
                    let dest_path = Utf8PathBuf::from(&destination);
                    if let Some(parent) = dest_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    // Execute the action
                    action.execute(file_path, &dest_path)?;
                }

                return Ok(true); // First matching rule wins
            }
            Ok(None) => {
                // Rule didn't match, continue to next
            }
            Err(e) => {
                if verbose {
                    eprintln!("⚠️  Rule {} error: {}", i + 1, e);
                }
            }
        }
    }

    Ok(false)
}
