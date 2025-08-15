use anyhow::{Context, Result};
use camino::Utf8Path;
use chrono::{DateTime, Utc};
use nom_exif::{ExifIter, ExifTag, MediaParser, MediaSource};
use rhai::Dynamic;
use std::fs;
use std::sync::Arc;

use super::context::{MediaContext, SourceContext, TimeContext};
use super::location::reverse_geocode;
use super::location_history::LocationHistory;

pub fn extract_metadata(path: &Utf8Path) -> Result<MediaContext> {
    extract_metadata_with_location_history(path, None, None)
}

pub fn extract_metadata_with_location_history(
    path: &Utf8Path,
    location_history: Option<Arc<LocationHistory>>,
    max_hours: Option<u64>,
) -> Result<MediaContext> {
    let mut context = MediaContext {
        source: extract_source_info(path)?,
        ..Default::default()
    };

    // Detect media type
    context.r#type = detect_media_type(path);

    // Try EXIF extraction first
    match extract_exif_metadata(path) {
        Ok(exif_context) => {
            // Use EXIF data directly
            context.time = exif_context.time;
            context.space = exif_context.space;
            context.meta = exif_context.meta;
        }
        Err(_) => {
            // EXIF extraction failed completely, fallbacks will handle it
        }
    }

    // Apply fallbacks for missing data
    apply_fallbacks(&mut context, path, location_history, max_hours)?;

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

            // Log GPS source
            eprintln!(
                "🛰️  GPS from EXIF: {:.6}, {:.6} -> {}, {}",
                lat, lon, context.space.country, context.space.city
            );
        }
    }

    // Second pass: Parse again for all EXIF data
    let ms = MediaSource::file_path(path.as_std_path())?;
    let iter = parser.parse::<_, _, ExifIter>(ms)?;

    for mut entry in iter.into_iter() {
        if let Ok(value) = entry.take_result() {
            // Get tag name - use debug format of tag if no specific tag
            let tag_name = if let Some(tag) = entry.tag() {
                format!("{tag:?}")
            } else {
                // Use tag code if no enum variant
                format!("Tag_{}", entry.tag_code())
            };

            // Convert value to Dynamic based on its type using available methods
            let dynamic_value = if let Some(v) = value.as_u32() {
                Dynamic::from(v as i64)
            } else if let Some(v) = value.as_i32() {
                Dynamic::from(v as i64)
            } else if let Some(v) = value.as_u16() {
                Dynamic::from(v as i64)
            } else if let Some(v) = value.as_i16() {
                Dynamic::from(v as i64)
            } else if let Some(v) = value.as_u8() {
                Dynamic::from(v as i64)
            } else if let Some(v) = value.as_i8() {
                Dynamic::from(v as i64)
            } else if let Some(rational) = value.as_urational() {
                Dynamic::from(rational.0 as f64 / rational.1 as f64)
            } else if let Some(rational) = value.as_irational() {
                Dynamic::from(rational.0 as f64 / rational.1 as f64)
            } else if let Some(s) = value.as_str() {
                Dynamic::from(s.to_string())
            } else {
                // For other types (Time, NaiveDateTime, etc.), store as string
                Dynamic::from(format!("{value:?}"))
            };

            // Store in meta HashMap
            context.meta.insert(tag_name.clone(), dynamic_value);

            // Special handling for specific tags that affect other fields
            match entry.tag() {
                Some(ExifTag::DateTimeOriginal) | Some(ExifTag::CreateDate) => {
                    // Try as string first
                    if let Some(datetime_str) = value.as_str() {
                        if let Ok(dt) = parse_exif_datetime(datetime_str) {
                            context.time = create_time_context(dt);
                        }
                    } else {
                        // Try to parse from debug representation
                        let debug_str = format!("{value:?}");

                        // Handle Time(YYYY-MM-DDTHH:MM:SS+TZ:TZ) format
                        if debug_str.starts_with("Time(") && debug_str.ends_with(")") {
                            if let Some(dt_str) = debug_str
                                .strip_prefix("Time(")
                                .and_then(|s| s.strip_suffix(")"))
                            {
                                if let Ok(dt) = DateTime::parse_from_rfc3339(dt_str) {
                                    context.time = create_time_context(dt.with_timezone(&Utc));
                                }
                            }
                        }
                        // Handle NaiveDateTime format
                        else if debug_str.starts_with("NaiveDateTime(")
                            && debug_str.ends_with(")")
                        {
                            if let Some(dt_str) = debug_str
                                .strip_prefix("NaiveDateTime(")
                                .and_then(|s| s.strip_suffix(")"))
                            {
                                if let Ok(naive_dt) = chrono::NaiveDateTime::parse_from_str(
                                    dt_str,
                                    "%Y-%m-%dT%H:%M:%S",
                                ) {
                                    let dt =
                                        DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc);
                                    context.time = create_time_context(dt);
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Other tags are already stored in meta
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

fn apply_fallbacks(
    context: &mut MediaContext,
    path: &Utf8Path,
    location_history: Option<Arc<LocationHistory>>,
    max_hours: Option<u64>,
) -> Result<()> {
    // Use image crate for dimensions if not in meta
    let has_width =
        context.meta.contains_key("ImageWidth") || context.meta.contains_key("ExifImageWidth");
    let has_height =
        context.meta.contains_key("ImageHeight") || context.meta.contains_key("ExifImageHeight");

    if !has_width || !has_height {
        if let Ok(img) = image::open(path.as_std_path()) {
            if !has_width {
                context
                    .meta
                    .insert("ImageWidth".to_string(), Dynamic::from(img.width() as i64));
            }
            if !has_height {
                context.meta.insert(
                    "ImageHeight".to_string(),
                    Dynamic::from(img.height() as i64),
                );
            }
        }
    }

    // Try to extract date from filename for videos
    if context.time.timestamp.is_none() && context.r#type == "video" {
        if let Some(dt) = extract_date_from_filename(path) {
            context.time = create_time_context(dt);
        }
    }

    // Use filesystem timestamp if no EXIF date or filename date
    if context.time.timestamp.is_none() {
        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(created) = metadata.created() {
                let dt: DateTime<Utc> = created.into();
                context.time = create_time_context(dt);
            }
        }
    }

    // Use location history as fallback for GPS coordinates
    if context.space.lat == 0.0 && context.space.lon == 0.0 {
        eprintln!("🔍 No GPS in EXIF, checking Location History...");
        if let Some(ref location_history) = location_history {
            if let Some(ref timestamp) = context.time.timestamp {
                // Convert timestamp to milliseconds
                let photo_timestamp_ms = timestamp.timestamp_millis() as u64;
                eprintln!(
                    "📅 Photo timestamp: {} ms ({})",
                    photo_timestamp_ms,
                    timestamp.format("%Y-%m-%d %H:%M:%S UTC")
                );

                // Find closest location points
                let (before, after) = location_history.find_closest_points(photo_timestamp_ms);
                eprintln!(
                    "🔍 Found location points: before={:?}, after={:?}",
                    before.map(|p| (p.timestamp_ms, p.latitude_e7, p.longitude_e7)),
                    after.map(|p| (p.timestamp_ms, p.latitude_e7, p.longitude_e7))
                );

                // Convert max hours to milliseconds (default 48 hours)
                let max_hours_actual = max_hours.unwrap_or(48);
                let max_time_diff_ms = max_hours_actual * 60 * 60 * 1000;
                eprintln!(
                    "🕒 Using location history threshold: {max_hours_actual} hours ({max_time_diff_ms} ms)"
                );

                // Select the closest point within 48 hours
                let selected_point = match (before, after) {
                    (Some(b), Some(a)) => {
                        let diff_before = photo_timestamp_ms.saturating_sub(b.timestamp_ms);
                        let diff_after = a.timestamp_ms.saturating_sub(photo_timestamp_ms);

                        if diff_before <= max_time_diff_ms && diff_after <= max_time_diff_ms {
                            // Both within threshold, choose closer one
                            if diff_before <= diff_after {
                                Some(b)
                            } else {
                                Some(a)
                            }
                        } else if diff_before <= max_time_diff_ms {
                            Some(b)
                        } else if diff_after <= max_time_diff_ms {
                            Some(a)
                        } else {
                            None
                        }
                    }
                    (Some(b), None) => {
                        let diff = photo_timestamp_ms.saturating_sub(b.timestamp_ms);
                        if diff <= max_time_diff_ms {
                            Some(b)
                        } else {
                            None
                        }
                    }
                    (None, Some(a)) => {
                        let diff = a.timestamp_ms.saturating_sub(photo_timestamp_ms);
                        if diff <= max_time_diff_ms {
                            Some(a)
                        } else {
                            None
                        }
                    }
                    (None, None) => None,
                };

                // Apply the location if found
                if let Some(point) = selected_point {
                    // Convert E7 coordinates to decimal degrees
                    let lat = point.latitude_e7 as f64 / 1e7;
                    let lon = point.longitude_e7 as f64 / 1e7;

                    context.space.lat = lat;
                    context.space.lon = lon;

                    // Reverse geocode to get location details
                    if let Ok(mut location) = reverse_geocode(lat, lon) {
                        // Preserve the GPS coordinates
                        location.lat = lat;
                        location.lon = lon;
                        context.space = location;

                        // Log Location History source
                        eprintln!(
                            "🗺️  GPS from Location History: {:.6}, {:.6} -> {}, {}",
                            lat, lon, context.space.country, context.space.city
                        );
                    }
                } else {
                    eprintln!("❌ No location found in History within {max_hours_actual} hours");
                }
            } else {
                eprintln!("❌ No timestamp available for Location History lookup");
            }
        } else {
            eprintln!("❌ No Location History provided");
        }
    }

    // TODO: Add video duration extraction here when mp4parse is added
    // For now, videos won't have duration metadata

    Ok(())
}

fn extract_date_from_filename(path: &Utf8Path) -> Option<DateTime<Utc>> {
    let filename = path.file_name()?;

    // Common video filename patterns:
    // VID_20180120_185352.mp4
    // 2018-01-20 15.46.55.mp4
    // IMG_20180120_154209.jpg

    // Look for 8 consecutive digits (YYYYMMDD)
    let chars: Vec<char> = filename.chars().collect();
    for i in 0..chars.len().saturating_sub(7) {
        if chars[i..i + 8].iter().all(|c| c.is_ascii_digit()) {
            let date_str: String = chars[i..i + 8].iter().collect();
            if let Ok(year) = date_str[0..4].parse::<i32>() {
                if let Ok(month) = date_str[4..6].parse::<u32>() {
                    if let Ok(day) = date_str[6..8].parse::<u32>() {
                        if (1900..=2100).contains(&year)
                            && (1..=12).contains(&month)
                            && (1..=31).contains(&day)
                        {
                            if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                                if let Some(datetime) = date.and_hms_opt(0, 0, 0) {
                                    return Some(DateTime::from_naive_utc_and_offset(
                                        datetime, Utc,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Look for YYYY-MM-DD pattern
    if filename.contains('-') {
        let parts: Vec<&str> = filename.split(&['-', ' ', '.'][..]).collect();
        if parts.len() >= 3 {
            if let Ok(year) = parts[0].parse::<i32>() {
                if let Ok(month) = parts[1].parse::<u32>() {
                    if let Ok(day) = parts[2].parse::<u32>() {
                        if (1900..=2100).contains(&year)
                            && (1..=12).contains(&month)
                            && (1..=31).contains(&day)
                        {
                            if let Some(date) = chrono::NaiveDate::from_ymd_opt(year, month, day) {
                                if let Some(datetime) = date.and_hms_opt(0, 0, 0) {
                                    return Some(DateTime::from_naive_utc_and_offset(
                                        datetime, Utc,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
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
