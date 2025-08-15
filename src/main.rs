use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use std::fs;
use std::sync::Arc;
use walkdir::WalkDir;

use monana::{
    actions::Action,
    metadata::{LocationHistory, extractor::extract_metadata_with_location_history},
    pipeline::{Pipeline, RuleEngine, Ruleset},
};

#[derive(Parser)]
#[command(name = "monana")]
#[command(about = "MONANA - Media Organization, Normalization, and Archival via Named Automation")]
struct Args {
    /// Run all cmdline rulesets with the given path
    #[arg(long = "input-cmdline", value_name = "PATH")]
    input_cmdline: Utf8PathBuf,

    /// Configuration file
    #[arg(short, long, default_value = "monana.yaml")]
    config: String,

    /// Google Maps Timeline location history JSON file (overrides config)
    #[arg(long = "location-history", value_name = "PATH")]
    location_history: Option<String>,

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

    // Load location history - CLI argument takes precedence over config
    let location_history_path = args
        .location_history
        .as_ref()
        .or(pipeline.location_history_path.as_ref());

    let location_history = if let Some(path) = location_history_path {
        match LocationHistory::from_json_file(path) {
            Ok(history) => {
                println!("📍 Loaded location history from: {path}");
                if args.location_history.is_some() {
                    println!("   (from command line argument)");
                }
                Some(Arc::new(history))
            }
            Err(e) => {
                eprintln!("⚠️  Failed to load location history from {path}: {e}");
                None
            }
        }
    } else {
        None
    };

    // Find all cmdline rulesets
    let cmdline_rulesets: Vec<_> = pipeline
        .rulesets
        .iter()
        .filter(|r| matches!(r.input, monana::pipeline::InputSpec::Cmdline))
        .collect();

    if cmdline_rulesets.is_empty() {
        println!("⚠️  No cmdline rulesets found in configuration");
        return Ok(());
    }

    println!("📋 Found {} cmdline ruleset(s):", cmdline_rulesets.len());
    for ruleset in &cmdline_rulesets {
        println!("   - {}", ruleset.name);
    }

    // Create rule engine
    let engine = RuleEngine::new()?;

    // Check if input path exists
    if !args.input_cmdline.exists() {
        eprintln!("⚠️  Path does not exist: {}", args.input_cmdline);
        return Ok(());
    }

    // Collect all files to process
    let all_files = collect_files(&args.input_cmdline, args.recursive)?;

    if all_files.is_empty() {
        println!("⚠️  No media files found");
        return Ok(());
    }

    println!("📁 Found {} file(s) to process", all_files.len());

    if args.dry_run {
        println!("🔍 DRY RUN MODE - No files will be moved\n");
    }

    // Process each ruleset
    for ruleset in &cmdline_rulesets {
        println!("🔧 Processing ruleset: {}", ruleset.name);
        println!("   Rules: {}", ruleset.rules.len());

        let mut processed = 0;
        let mut matched = 0;
        let mut errors = 0;
        let mut no_match_files = Vec::new();

        for file_path in &all_files {
            processed += 1;

            if args.verbose {
                println!("\n🔄 Processing: {file_path}");
            }

            match process_file(
                file_path,
                ruleset,
                &engine,
                args.dry_run,
                args.verbose,
                location_history.clone(),
                Some(pipeline.location_history_max_hours),
            ) {
                Ok(true) => matched += 1,
                Ok(false) => no_match_files.push(file_path.clone()),
                Err(e) => {
                    eprintln!("❌ Error processing {file_path}: {e}");
                    errors += 1;
                }
            }
        }

        // Show summary for this ruleset
        println!("\n📊 Ruleset '{}' summary:", ruleset.name);
        println!("   Files processed: {processed}");
        println!("   Rules matched: {matched}");
        println!("   No rules matched: {}", no_match_files.len());
        println!("   Errors: {errors}");

        if args.verbose && !no_match_files.is_empty() {
            println!("   Files with no matching rules:");
            for file in &no_match_files {
                println!("     - {file}");
            }
        }
        println!();
    }

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
    ruleset: &Ruleset,
    engine: &RuleEngine,
    dry_run: bool,
    verbose: bool,
    location_history: Option<Arc<LocationHistory>>,
    max_hours: Option<u64>,
) -> Result<bool> {
    // Extract metadata
    let context = extract_metadata_with_location_history(file_path, location_history, max_hours)?;

    if verbose {
        println!("  📊 Type: {}", context.r#type);
        if !context.meta.is_empty() {
            println!("  📷 EXIF tags found: {}", context.meta.len());
        }
    }

    // Process rules in order
    for rule in &ruleset.rules {
        match engine.process_rule(rule, &context)? {
            Some((destination, action)) => {
                if verbose {
                    println!("  ✅ Rule matched: {}", rule.condition);
                    println!("  📁 Destination: {destination}");
                    println!("  🎯 Action: {action:?}");
                }

                if !dry_run {
                    // Convert ActionSpec to Action and execute
                    let action_enum = match &action {
                        monana::pipeline::ActionSpec::Move => Action::Move,
                        monana::pipeline::ActionSpec::Copy => Action::Copy,
                        monana::pipeline::ActionSpec::Symlink => Action::Symlink,
                        monana::pipeline::ActionSpec::Hardlink => Action::Hardlink,
                        monana::pipeline::ActionSpec::Command(cmd) => {
                            // For now, just print custom commands
                            println!("  🔧 Would run custom command: {cmd}");
                            return Ok(true);
                        }
                    };

                    let dest_path = Utf8PathBuf::from(&destination);
                    action_enum.execute(file_path, &dest_path)?;
                } else if !verbose {
                    println!("  {file_path} -> {destination}");
                }

                return Ok(true);
            }
            None => {
                if verbose {
                    println!("  ❌ Rule not matched: {}", rule.condition);
                }
            }
        }
    }

    Ok(false)
}
