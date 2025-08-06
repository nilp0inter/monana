use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use chrono::{DateTime, Utc};
use clap::Parser;
use nom_exif::{ExifIter, ExifTag, MediaParser, MediaSource};
use reverse_geocoder::ReverseGeocoder;
use std::fs;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "monana")]
#[command(about = "A simple MONANA prototype - extract metadata and copy with template")]
struct Args {
    /// Path to the image file or directory to process
    path: Utf8PathBuf,

    /// Output directory template (default: ./output/{media.type}/{time.yyyy}/{time.mm}/{time.dd}/{space.country}_{space.city}/{source.original})
    #[arg(
        short,
        long,
        default_value = "./output/{media.type}/{time.yyyy}/{time.mm}/{time.dd}/{space.country}_{space.city}/{source.original}"
    )]
    template: String,

    /// Process files recursively if path is a directory
    #[arg(short, long, default_value_t = false)]
    recursive: bool,
}

#[derive(Debug, Default)]
struct MediaContext {
    // Time variables
    time_yyyy: String,
    time_mm: String,
    time_dd: String,
    time_month_name: String,

    // Space variables
    space_country: String,
    space_city: String,
    space_lat: f64,
    space_lon: f64,

    // Source variables
    source_name: String,
    source_extension: String,
    source_original: String,

    // Media variables
    media_type: String,
    media_width: u32,
    media_height: u32,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("🌸 MONANA Prototype");
    println!("Processing: {}", args.path);

    // Validate input path exists
    if !args.path.exists() {
        anyhow::bail!("Path does not exist: {}", args.path);
    }

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
        match process_file(&file_path, &args.template) {
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

fn reverse_geocode(latitude: f64, longitude: f64) -> Result<(String, String)> {
    // Create a reverse geocoder instance (this loads the data)
    let geocoder = ReverseGeocoder::new();

    // Perform the reverse geocoding lookup
    let search_result = geocoder.search((latitude, longitude));

    let city = search_result.record.name.clone();
    let country = search_result.record.cc.clone(); // Country code

    Ok((city, country))
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

    let is_media = if let Some(mime) = mime_type {
        mime.starts_with("image/") || mime.starts_with("video/")
    } else {
        false
    };

    if is_media {
        println!(
            "🎯 Media file detected: {} ({})",
            path.file_name().unwrap_or("unknown"),
            mime_type.unwrap_or("unknown")
        );
    }

    Ok(is_media)
}

fn process_file(file_path: &Utf8PathBuf, template: &str) -> Result<()> {
    println!("\n📄 Processing: {file_path}");

    // Extract metadata
    let context = extract_metadata(file_path)?;

    // Apply template
    let output_path = apply_template(template, &context)?;
    println!("📁 Output path: {output_path}");

    // Create directory and copy file
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {parent}"))?;
    }

    fs::copy(file_path, &output_path)
        .with_context(|| format!("Failed to copy {file_path} to {output_path}"))?;

    Ok(())
}

fn extract_metadata(path: &Utf8PathBuf) -> Result<MediaContext> {
    // Extract source information
    let mut context = MediaContext {
        source_original: path.file_name().unwrap_or("unknown").to_string(),
        source_name: path.file_stem().unwrap_or("unknown").to_string(),
        source_extension: path.extension().unwrap_or("").to_string(),
        ..Default::default()
    };

    // Detect media type using tree_magic_mini
    let mime_type = tree_magic_mini::from_filepath(path.as_std_path());
    context.media_type = if let Some(mime) = mime_type {
        if mime.starts_with("image/") {
            "image".to_string()
        } else if mime.starts_with("video/") {
            "video".to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    };
    println!(
        "🎯 Detected type: {} ({})",
        context.media_type,
        mime_type.unwrap_or("unknown")
    );

    // Parse EXIF data with nom-exif (proper API usage)
    let mut parser = MediaParser::new();
    match MediaSource::file_path(path.as_std_path()) {
        Ok(ms) => {
            if ms.has_exif() {
                println!("📷 Found EXIF data, parsing...");
                match parser.parse::<_, _, ExifIter>(ms) {
                    Ok(iter) => {
                        println!("📷 Processing EXIF entries...");

                        // Try to extract GPS info first - check if parse_gps_info works
                        let gps_result = iter.parse_gps_info();
                        println!("📍 GPS parse result: {gps_result:?}");

                        let mut gps_already_processed = false;
                        if let Ok(Some(gps_info)) = gps_result {
                            gps_already_processed = true;
                            // Debug print to see the structure
                            println!("📍 GPS info found: {gps_info:?}");

                            // Convert LatLng coordinates to decimal degrees manually
                            // LatLng(degrees, minutes, seconds) where each is a Rational
                            let lat_degrees =
                                gps_info.latitude.0.0 as f64 / gps_info.latitude.0.1 as f64;
                            let lat_minutes =
                                gps_info.latitude.1.0 as f64 / gps_info.latitude.1.1 as f64;
                            let lat_seconds =
                                gps_info.latitude.2.0 as f64 / gps_info.latitude.2.1 as f64;
                            let lat = lat_degrees + lat_minutes / 60.0 + lat_seconds / 3600.0;
                            let lat = lat
                                * if gps_info.latitude_ref == 'S' {
                                    -1.0
                                } else {
                                    1.0
                                };

                            let lon_degrees =
                                gps_info.longitude.0.0 as f64 / gps_info.longitude.0.1 as f64;
                            let lon_minutes =
                                gps_info.longitude.1.0 as f64 / gps_info.longitude.1.1 as f64;
                            let lon_seconds =
                                gps_info.longitude.2.0 as f64 / gps_info.longitude.2.1 as f64;
                            let lon = lon_degrees + lon_minutes / 60.0 + lon_seconds / 3600.0;
                            let lon = lon
                                * if gps_info.longitude_ref == 'W' {
                                    -1.0
                                } else {
                                    1.0
                                };

                            context.space_lat = lat;
                            context.space_lon = lon;
                            println!("📍 GPS coordinates: {lat:.6}, {lon:.6}");

                            // Perform offline reverse geocoding
                            match reverse_geocode(lat, lon) {
                                Ok((city, country)) => {
                                    context.space_city = city;
                                    context.space_country = country;
                                    println!(
                                        "🌍 Location: {}, {}",
                                        context.space_city, context.space_country
                                    );
                                }
                                Err(e) => {
                                    println!("⚠️ Reverse geocoding failed: {e}");
                                    context.space_city = "unknown".to_string();
                                    context.space_country = "unknown".to_string();
                                }
                            }
                        } else {
                            println!(
                                "📍 No GPS info found via parse_gps_info, will look for GPS tags manually"
                            );
                        }

                        // Store GPS coordinates found manually
                        let mut gps_latitude: Option<f64> = None;
                        let mut gps_longitude: Option<f64> = None;
                        let gps_lat_ref: Option<char> = None;
                        let gps_lon_ref: Option<char> = None;

                        // Now iterate through EXIF entries for other metadata and GPS data
                        for mut entry in iter.into_iter() {
                            if let Ok(value) = entry.take_result() {
                                if let Some(tag) = entry.tag() {
                                    match tag {
                                        ExifTag::GPSInfo => {
                                            println!("🔍 Found GPSInfo: {value:?}");
                                            // GPSInfo tag indicates GPS data exists, but we can't parse the coordinates
                                            // For now, mark that GPS info was found
                                            println!(
                                                "📍 GPS data detected but coordinates not extractable"
                                            );
                                            // Set placeholder coordinates to indicate GPS was present
                                            gps_latitude = Some(0.0); // Placeholder
                                            gps_longitude = Some(0.0); // Placeholder
                                        }
                                        ExifTag::DateTimeOriginal | ExifTag::CreateDate => {
                                            // Try as string first, then as NaiveDateTime
                                            if let Some(datetime_str) = value.as_str() {
                                                if let Ok(dt) = parse_exif_datetime(datetime_str) {
                                                    context.time_yyyy = dt.format("%Y").to_string();
                                                    context.time_mm = dt.format("%m").to_string();
                                                    context.time_dd = dt.format("%d").to_string();
                                                    context.time_month_name =
                                                        dt.format("%B").to_string();
                                                    println!(
                                                        "📅 EXIF DateTime: {} -> {}-{}-{}",
                                                        datetime_str,
                                                        context.time_yyyy,
                                                        context.time_mm,
                                                        context.time_dd
                                                    );
                                                }
                                            } else {
                                                // Try to parse it from debug string if it's a NaiveDateTime
                                                let debug_str = format!("{value:?}");
                                                if debug_str.starts_with("NaiveDateTime(") {
                                                    if let Some(dt_str) = debug_str
                                                        .strip_prefix("NaiveDateTime(")
                                                        .and_then(|s| s.strip_suffix(")"))
                                                    {
                                                        if let Ok(naive_dt) =
                                                            chrono::NaiveDateTime::parse_from_str(
                                                                dt_str,
                                                                "%Y-%m-%dT%H:%M:%S",
                                                            )
                                                        {
                                                            let dt = DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                                                            context.time_yyyy =
                                                                dt.format("%Y").to_string();
                                                            context.time_mm =
                                                                dt.format("%m").to_string();
                                                            context.time_dd =
                                                                dt.format("%d").to_string();
                                                            context.time_month_name =
                                                                dt.format("%B").to_string();
                                                            println!(
                                                                "📅 EXIF NaiveDateTime: {} -> {}-{}-{}",
                                                                dt_str,
                                                                context.time_yyyy,
                                                                context.time_mm,
                                                                context.time_dd
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        ExifTag::ImageWidth | ExifTag::ExifImageWidth => {
                                            if let Some(w) = value.as_u32() {
                                                context.media_width = w;
                                                println!("📐 EXIF Width: {w}");
                                            }
                                        }
                                        ExifTag::ImageHeight | ExifTag::ExifImageHeight => {
                                            if let Some(h) = value.as_u32() {
                                                context.media_height = h;
                                                println!("📐 EXIF Height: {h}");
                                            }
                                        }
                                        ExifTag::Make => {
                                            if let Some(make) = value.as_str() {
                                                println!("📱 Camera Make: {make}");
                                            }
                                        }
                                        ExifTag::Model => {
                                            if let Some(model) = value.as_str() {
                                                println!("📱 Camera Model: {model}");
                                            }
                                        }
                                        _ => {
                                            // Debug: show all other tags we're finding
                                            println!("🔍 Found tag: {tag:?} = {value:?}");
                                        }
                                    }
                                }
                            }
                        }

                        // Process manually found GPS coordinates (only if GPS wasn't already processed)
                        if !gps_already_processed
                            && let (Some(lat), Some(lon)) = (gps_latitude, gps_longitude)
                        {
                            if lat == 0.0 && lon == 0.0 {
                                // Placeholder coordinates - GPS info was detected but not parseable
                                println!("📍 GPS info present but coordinates not extractable");
                                context.space_city = "gps_present".to_string();
                                context.space_country = "unknown".to_string();
                            } else {
                                // Real coordinates
                                let lat = lat * if gps_lat_ref == Some('S') { -1.0 } else { 1.0 };
                                let lon = lon * if gps_lon_ref == Some('W') { -1.0 } else { 1.0 };

                                context.space_lat = lat;
                                context.space_lon = lon;
                                println!("📍 Manual GPS coordinates: {lat:.6}, {lon:.6}");

                                // Perform offline reverse geocoding
                                match reverse_geocode(lat, lon) {
                                    Ok((city, country)) => {
                                        context.space_city = city;
                                        context.space_country = country;
                                        println!(
                                            "🌍 Location: {}, {}",
                                            context.space_city, context.space_country
                                        );
                                    }
                                    Err(e) => {
                                        println!("⚠️ Reverse geocoding failed: {e}");
                                        context.space_city = "unknown".to_string();
                                        context.space_country = "unknown".to_string();
                                    }
                                }
                            }
                        }

                        println!(
                            "📐 Final dimensions: {}x{}",
                            context.media_width, context.media_height
                        );
                    }
                    Err(e) => {
                        println!("⚠️ EXIF parsing failed: {e}");
                        fallback_metadata_extraction(&mut context, path)?;
                    }
                }
            } else {
                println!("📷 No EXIF data in file");
                fallback_metadata_extraction(&mut context, path)?;
            }
        }
        Err(e) => {
            println!("⚠️ Failed to create MediaSource: {e}");
            fallback_metadata_extraction(&mut context, path)?;
        }
    }

    // Set defaults for missing values
    if context.time_yyyy.is_empty() {
        context.time_yyyy = "unknown".to_string();
        context.time_mm = "00".to_string();
        context.time_dd = "00".to_string();
        context.time_month_name = "Unknown".to_string();
    }

    if context.space_city.is_empty() {
        context.space_city = "unknown".to_string();
        context.space_country = "unknown".to_string();
    }

    Ok(context)
}

fn fallback_metadata_extraction(context: &mut MediaContext, path: &Utf8PathBuf) -> Result<()> {
    println!("📷 Using fallback metadata extraction");

    // Use image crate to get dimensions
    if let Ok(img) = image::open(path.as_std_path()) {
        context.media_width = img.width();
        context.media_height = img.height();
        println!(
            "📐 Image dimensions: {}x{}",
            context.media_width, context.media_height
        );
    }

    // Use filesystem timestamp
    if let Ok(metadata) = fs::metadata(path) {
        if let Ok(created) = metadata.created() {
            let dt: DateTime<Utc> = created.into();
            context.time_yyyy = dt.format("%Y").to_string();
            context.time_mm = dt.format("%m").to_string();
            context.time_dd = dt.format("%d").to_string();
            context.time_month_name = dt.format("%B").to_string();
            println!(
                "📅 Filesystem timestamp: {}-{}-{}",
                context.time_yyyy, context.time_mm, context.time_dd
            );
        }
    }

    Ok(())
}

fn parse_exif_datetime(datetime_str: &str) -> Result<DateTime<Utc>> {
    // EXIF datetime format: "YYYY:MM:DD HH:MM:SS"
    let normalized = datetime_str.replace(':', "-");
    let parts: Vec<&str> = normalized.splitn(2, ' ').collect();
    if parts.len() == 2 {
        let date_part = parts[0];
        let time_part = parts[1];
        let full_datetime = format!("{date_part}T{time_part}Z");
        DateTime::parse_from_rfc3339(&full_datetime)
            .map(|dt| dt.with_timezone(&Utc))
            .with_context(|| format!("Failed to parse datetime: {datetime_str}"))
    } else {
        anyhow::bail!("Invalid datetime format: {}", datetime_str)
    }
}

fn apply_template(template: &str, context: &MediaContext) -> Result<Utf8PathBuf> {
    let mut result = template.to_string();

    // Replace time variables
    result = result.replace("{time.yyyy}", &context.time_yyyy);
    result = result.replace("{time.mm}", &context.time_mm);
    result = result.replace("{time.dd}", &context.time_dd);
    result = result.replace("{time.month_name}", &context.time_month_name);

    // Replace space variables
    result = result.replace("{space.country}", &context.space_country);
    result = result.replace("{space.city}", &context.space_city);

    // Replace source variables
    result = result.replace("{source.name}", &context.source_name);
    result = result.replace("{source.extension}", &context.source_extension);
    result = result.replace("{source.original}", &context.source_original);

    // Replace media variables
    result = result.replace("{media.type}", &context.media_type);
    result = result.replace("{media.width}", &context.media_width.to_string());
    result = result.replace("{media.height}", &context.media_height.to_string());

    Ok(Utf8PathBuf::from(result))
}
