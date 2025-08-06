use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use walkdir::WalkDir;

use monana::{actions::Action, metadata::extract_metadata, template::apply_template};

#[derive(Parser)]
#[command(name = "monana")]
#[command(about = "Media archival system - organize media by time and space")]
struct Args {
    /// Path to the image file or directory to process
    path: Utf8PathBuf,

    /// Output directory template
    #[arg(
        short,
        long,
        default_value = "./output/{media.type}/{time.yyyy}/{time.mm}/{time.dd}/{space.country}_{space.city}/{source.original}"
    )]
    template: String,

    /// Process files recursively if path is a directory
    #[arg(short, long, default_value_t = false)]
    recursive: bool,

    /// Action to perform: move, copy, symlink, hardlink
    #[arg(short, long, default_value = "copy")]
    action: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("🌸 MONANA - Media Archival System");
    println!("Processing: {}", args.path);

    // Validate input path exists
    if !args.path.exists() {
        anyhow::bail!("Path does not exist: {}", args.path);
    }

    // Parse action
    let action = match args.action.as_str() {
        "move" => Action::Move,
        "copy" => Action::Copy,
        "symlink" => Action::Symlink,
        "hardlink" => Action::Hardlink,
        _ => anyhow::bail!("Unknown action: {}", args.action),
    };

    // Collect files to process
    let files = collect_files(&args.path, args.recursive)?;
    println!("📁 Found {} file(s) to process", files.len());

    if files.is_empty() {
        println!("⚠️  No media files found");
        return Ok(());
    }

    // Process each file
    let mut processed = 0;
    let mut skipped = 0;

    for file_path in files {
        match process_file(&file_path, &args.template, &action) {
            Ok(()) => {
                processed += 1;
                println!("✅ Processed: {file_path}");
            }
            Err(e) => {
                skipped += 1;
                println!("⚠️  Skipped {file_path}: {e}");
            }
        }
    }

    println!("\n🎉 Summary: {processed} processed, {skipped} skipped");
    Ok(())
}

fn collect_files(path: &Utf8PathBuf, recursive: bool) -> Result<Vec<Utf8PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        // Single file - check if it's a media file
        if is_media_file(path)? {
            files.push(path.clone());
        }
    } else if path.is_dir() {
        // Directory - walk and collect media files
        println!(
            "📂 Scanning directory{}...",
            if recursive { " recursively" } else { "" }
        );

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

fn is_media_file(path: &Utf8PathBuf) -> Result<bool> {
    // Use tree_magic_mini for fast MIME type detection
    let mime_type = tree_magic_mini::from_filepath(path.as_std_path());

    Ok(mime_type
        .map(|mime| mime.starts_with("image/") || mime.starts_with("video/"))
        .unwrap_or(false))
}

fn process_file(file_path: &Utf8PathBuf, template: &str, action: &Action) -> Result<()> {
    println!("\n📄 Processing: {file_path}");

    // Extract metadata
    let context = extract_metadata(file_path)?;

    // Apply template
    let output_path = apply_template(template, &context)?;
    println!("📁 Output path: {output_path}");

    // Execute action
    action.execute(file_path, &output_path)?;

    Ok(())
}
