use anyhow::{Context, Result};
use camino::Utf8Path;
use chrono::{DateTime, Utc};
use nom_exif::{ExifIter, ExifTag, MediaParser, MediaSource};
use std::fs;

use super::context::{MediaContext, SourceContext, TimeContext};
use super::location::reverse_geocode;

pub fn extract_metadata(path: &Utf8Path) -> Result<MediaContext> {
    let mut context = MediaContext {
        source: extract_source_info(path)?,
        ..Default::default()
    };

    // Detect media type
    context.media.r#type = detect_media_type(path);

    // Try EXIF extraction first
    match extract_exif_metadata(path) {
        Ok(exif_context) => {
            // Use EXIF data directly instead of merging
            context.time = exif_context.time;
            context.space = exif_context.space;
            if exif_context.media.width > 0 {
                context.media.width = exif_context.media.width;
            }
            if exif_context.media.height > 0 {
                context.media.height = exif_context.media.height;
            }
            if exif_context.media.camera_make.is_some() {
                context.media.camera_make = exif_context.media.camera_make;
            }
            if exif_context.media.camera_model.is_some() {
                context.media.camera_model = exif_context.media.camera_model;
            }
            if exif_context.media.orientation.is_some() {
                context.media.orientation = exif_context.media.orientation;
            }
        }
        Err(_) => {
            // EXIF extraction failed completely, fallbacks will handle it
        }
    }

    // Apply fallbacks for missing data
    apply_fallbacks(&mut context, path)?;

    // Ensure defaults for required fields
    apply_defaults(&mut context);

    Ok(context)
}

fn extract_source_info(path: &Utf8Path) -> Result<SourceContext> {
    let metadata = fs::metadata(path)?;

    Ok(SourceContext {
        path: path.parent().unwrap_or(Utf8Path::new(".")).to_string(),
        name: path.file_stem().unwrap_or("unknown").to_string(),
        extension: path.extension().unwrap_or("").to_string(),
        original: path.file_name().unwrap_or("unknown").to_string(),
        size: metadata.len(),
    })
}

fn detect_media_type(path: &Utf8Path) -> String {
    if let Some(mime) = tree_magic_mini::from_filepath(path.as_std_path()) {
        if mime.starts_with("image/") {
            "image".to_string()
        } else if mime.starts_with("video/") {
            "video".to_string()
        } else {
            "unknown".to_string()
        }
    } else {
        "unknown".to_string()
    }
}

fn extract_exif_metadata(path: &Utf8Path) -> Result<MediaContext> {
    let mut context = MediaContext::default();
    let mut parser = MediaParser::new();

    let ms = MediaSource::file_path(path.as_std_path())?;
    if !ms.has_exif() {
        anyhow::bail!("No EXIF data found");
    }

    // First pass: Try to extract GPS info
    let iter = parser.parse::<_, _, ExifIter>(ms)?;
    if let Ok(Some(gps_info)) = iter.parse_gps_info() {
        let lat = convert_gps_coordinate(gps_info.latitude, gps_info.latitude_ref == 'S');
        let lon = convert_gps_coordinate(gps_info.longitude, gps_info.longitude_ref == 'W');

        context.space.lat = lat;
        context.space.lon = lon;

        // altitude is a Rational, not an Option
        context.space.altitude = Some(gps_info.altitude.0 as f64 / gps_info.altitude.1 as f64);

        // Reverse geocode if we have coordinates
        if let Ok(mut location) = reverse_geocode(lat, lon) {
            // Preserve the GPS coordinates we just calculated
            location.lat = lat;
            location.lon = lon;
            location.altitude = context.space.altitude;
            context.space = location;
        }
    }

    // Second pass: Parse again for other EXIF data
    let ms = MediaSource::file_path(path.as_std_path())?;
    let iter = parser.parse::<_, _, ExifIter>(ms)?;

    for mut entry in iter.into_iter() {
        if let Ok(value) = entry.take_result() {
            if let Some(tag) = entry.tag() {
                match tag {
                    ExifTag::DateTimeOriginal | ExifTag::CreateDate => {
                        eprintln!("DEBUG: Found date tag {tag:?} with value: {value:?}");
                        // Try as string first
                        if let Some(datetime_str) = value.as_str() {
                            if let Ok(dt) = parse_exif_datetime(datetime_str) {
                                context.time = create_time_context(dt);
                                eprintln!("DEBUG: Parsed EXIF date from string: {datetime_str}");
                            }
                        } else {
                            // Try to get NaiveDateTime from the debug representation
                            let debug_str = format!("{value:?}");
                            if debug_str.starts_with("NaiveDateTime(") && debug_str.ends_with(")") {
                                if let Some(dt_str) = debug_str
                                    .strip_prefix("NaiveDateTime(")
                                    .and_then(|s| s.strip_suffix(")"))
                                {
                                    if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(
                                        dt_str,
                                        "%Y-%m-%dT%H:%M:%S",
                                    ) {
                                        let dt = DateTime::<Utc>::from_naive_utc_and_offset(
                                            naive_dt, Utc,
                                        );
                                        context.time = create_time_context(dt);
                                        eprintln!(
                                            "DEBUG: Parsed EXIF date from NaiveDateTime: {dt_str}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ExifTag::ImageWidth | ExifTag::ExifImageWidth => {
                        if let Some(w) = value.as_u32() {
                            context.media.width = w;
                        }
                    }
                    ExifTag::ImageHeight | ExifTag::ExifImageHeight => {
                        if let Some(h) = value.as_u32() {
                            context.media.height = h;
                        }
                    }
                    ExifTag::Make => {
                        if let Some(make) = value.as_str() {
                            context.media.camera_make = Some(make.to_string());
                        }
                    }
                    ExifTag::Model => {
                        if let Some(model) = value.as_str() {
                            context.media.camera_model = Some(model.to_string());
                        }
                    }
                    ExifTag::Orientation => {
                        if let Some(orientation) = value.as_u32() {
                            context.media.orientation = Some(orientation);
                        }
                    }
                    _ => {
                        eprintln!("DEBUG: Found tag {tag:?} = {value:?}");
                    }
                }
            }
        }
    }

    Ok(context)
}

fn convert_gps_coordinate(coord: nom_exif::LatLng, negative: bool) -> f64 {
    let degrees = coord.0.0 as f64 / coord.0.1 as f64;
    let minutes = coord.1.0 as f64 / coord.1.1 as f64;
    let seconds = coord.2.0 as f64 / coord.2.1 as f64;

    let decimal = degrees + minutes / 60.0 + seconds / 3600.0;
    if negative { -decimal } else { decimal }
}

fn parse_exif_datetime(datetime_str: &str) -> Result<DateTime<Utc>> {
    // EXIF datetime format: "YYYY:MM:DD HH:MM:SS"
    let parts: Vec<&str> = datetime_str.splitn(2, ' ').collect();

    if parts.len() == 2 {
        // Only replace colons in the date part, not the time part
        let date_part = parts[0].replace(':', "-");
        let time_part = parts[1];
        let full_datetime = format!("{date_part}T{time_part}Z");
        DateTime::parse_from_rfc3339(&full_datetime)
            .map(|dt| dt.with_timezone(&Utc))
            .with_context(|| format!("Failed to parse datetime: {datetime_str}"))
    } else {
        anyhow::bail!("Invalid datetime format: {}", datetime_str)
    }
}

fn create_time_context(dt: DateTime<Utc>) -> TimeContext {
    TimeContext {
        yyyy: dt.format("%Y").to_string(),
        mm: dt.format("%m").to_string(),
        dd: dt.format("%d").to_string(),
        hh: dt.format("%H").to_string(),
        min: dt.format("%M").to_string(),
        ss: dt.format("%S").to_string(),
        month_name: dt.format("%B").to_string(),
        weekday: dt.format("%A").to_string(),
        timestamp: Some(dt),
    }
}

fn apply_fallbacks(context: &mut MediaContext, path: &Utf8Path) -> Result<()> {
    // Use image crate for dimensions if not set
    if context.media.width == 0 || context.media.height == 0 {
        if let Ok(img) = image::open(path.as_std_path()) {
            context.media.width = img.width();
            context.media.height = img.height();
        }
    }

    // Use filesystem timestamp if no EXIF date
    if context.time.timestamp.is_none() {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(created) = metadata.created() {
                let dt: DateTime<Utc> = created.into();
                context.time = create_time_context(dt);
            }
        }
    }

    Ok(())
}

fn apply_defaults(context: &mut MediaContext) {
    if context.time.yyyy.is_empty() {
        context.time.yyyy = "unknown".to_string();
        context.time.mm = "00".to_string();
        context.time.dd = "00".to_string();
        context.time.month_name = "Unknown".to_string();
        context.time.weekday = "Unknown".to_string();
    }

    if context.space.city.is_empty() {
        context.space.city = "unknown".to_string();
        context.space.country = "unknown".to_string();
    }
}
