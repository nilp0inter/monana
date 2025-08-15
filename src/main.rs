use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use std::fs;
use std::sync::Arc;
use walkdir::WalkDir;

use monana::{
    actions::Action,
    metadata::{
        LocationHistory,
        context::{MediaContext, SourceContext},
        extractor::extract_metadata_with_location_history,
    },
    pipeline::{InputSpec, Pipeline, RuleEngine, Ruleset},
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

    // Process each file through the entire pipeline
    let mut total_processed = 0;
    let mut total_matched = 0;
    let mut total_errors = 0;

    for file_path in &all_files {
        total_processed += 1;

        if args.verbose {
            println!("\n🔄 Processing file: {file_path}");
        }

        // Extract metadata once per file
        let context = match extract_metadata_with_location_history(
            file_path,
            location_history.clone(),
            Some(pipeline.location_history_max_hours),
        ) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("❌ Error extracting metadata from {file_path}: {e}");
                total_errors += 1;
                continue;
            }
        };

        if args.verbose {
            println!("  📊 Type: {}", context.r#type);
            if !context.meta.is_empty() {
                println!("  📷 EXIF tags found: {}", context.meta.len());
            }
        }

        // Process through all cmdline rulesets (entry points)
        let mut file_matched = false;
        for ruleset in &cmdline_rulesets {
            if args.verbose {
                println!("  🔧 Starting pipeline with ruleset: {}", ruleset.name);
            }

            match process_file_recursive(
                file_path,
                &context,
                ruleset,
                &pipeline,
                &engine,
                args.dry_run,
                args.verbose,
                0, // Initial depth
            ) {
                Ok(true) => {
                    file_matched = true;
                    total_matched += 1;
                }
                Ok(false) => {
                    if args.verbose {
                        println!("  ⚠️  No rules matched in ruleset: {}", ruleset.name);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "❌ Error processing {file_path} through ruleset '{}': {e}",
                        ruleset.name
                    );
                    total_errors += 1;
                }
            }
        }

        if !file_matched && args.verbose {
            println!("  ⚠️  File did not match any rules: {file_path}");
        }
    }

    // Show overall summary
    println!("\n📊 Overall summary:");
    println!("   Files processed: {total_processed}");
    println!("   Files matched: {total_matched}");
    println!("   Errors: {total_errors}");

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

#[allow(clippy::too_many_arguments)]
fn process_file_recursive(
    file_path: &Utf8PathBuf,
    context: &MediaContext,
    ruleset: &Ruleset,
    pipeline: &Pipeline,
    engine: &RuleEngine,
    dry_run: bool,
    verbose: bool,
    depth: usize,
) -> Result<bool> {
    let indent = "  ".repeat(depth + 2);

    if verbose && depth > 0 {
        println!("{indent}↳ Processing through ruleset: {}", ruleset.name);
    }

    // Process rules in order
    let mut destination_path: Option<Utf8PathBuf> = None;

    for rule in &ruleset.rules {
        match engine.process_rule(rule, context)? {
            Some((destination, action)) => {
                if verbose {
                    println!("{indent}✅ Rule matched: {}", rule.condition);
                    println!("{indent}📁 Destination: {destination}");
                    println!("{indent}🎯 Action: {action:?}");
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
                            println!("{indent}🔧 Would run custom command: {cmd}");
                            destination_path = Some(Utf8PathBuf::from(&destination));
                            break;
                        }
                    };

                    let dest_path = Utf8PathBuf::from(&destination);
                    action_enum.execute(file_path, &dest_path)?;
                    destination_path = Some(dest_path);
                } else {
                    println!("{indent}{file_path} -> {destination}");
                    destination_path = Some(Utf8PathBuf::from(&destination));
                }

                // First matching rule wins, exit the loop
                break;
            }
            None => {
                if verbose {
                    println!("{indent}❌ Rule not matched: {}", rule.condition);
                }
            }
        }
    }

    // If a rule matched and produced a destination, process dependent rulesets
    if let Some(dest_path) = destination_path {
        // Find all rulesets that depend on this one
        let dependents = find_dependent_rulesets(&ruleset.name, &pipeline.rulesets);

        if !dependents.is_empty() && verbose {
            println!("{indent}🔗 Found {} dependent ruleset(s)", dependents.len());
        }

        for dependent in dependents {
            // Create new context with updated source path but preserve all other metadata
            let mut new_context = context.clone();
            new_context.source = create_source_context(&dest_path)?;

            // Recursively process through the dependent ruleset
            process_file_recursive(
                &dest_path,
                &new_context,
                dependent,
                pipeline,
                engine,
                dry_run,
                verbose,
                depth + 1,
            )?;
        }

        Ok(true)
    } else {
        Ok(false)
    }
}

fn find_dependent_rulesets<'a>(
    ruleset_name: &str,
    all_rulesets: &'a [Ruleset],
) -> Vec<&'a Ruleset> {
    let expected_input = format!("ruleset:{ruleset_name}");

    all_rulesets
        .iter()
        .filter(|r| match &r.input {
            InputSpec::Cmdline => false,
            InputSpec::Prefixed(s) => s == &expected_input,
        })
        .collect()
}

fn create_source_context(path: &Utf8PathBuf) -> Result<SourceContext> {
    let name = path.file_stem().unwrap_or("").to_string();

    let extension = path.extension().unwrap_or("").to_string();

    let original = path.file_name().unwrap_or("").to_string();

    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    Ok(SourceContext {
        path: path.to_string(),
        name,
        extension,
        original,
        size,
    })
}
