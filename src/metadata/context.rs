use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MediaContext {
    pub time: TimeContext,
    pub space: SpaceContext,
    pub source: SourceContext,
    pub media: MediaInfo,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TimeContext {
    pub yyyy: String,
    pub mm: String,
    pub dd: String,
    pub hh: String,
    pub min: String,
    pub ss: String,
    pub month_name: String,
    pub weekday: String,
    pub timestamp: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SpaceContext {
    pub country: String,
    pub country_code: String,
    pub state: String,
    pub city: String,
    pub district: String,
    pub road: String,
    pub lat: f64,
    pub lon: f64,
    pub altitude: Option<f64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SourceContext {
    pub path: String,
    pub name: String,
    pub extension: String,
    pub original: String,
    pub size: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub r#type: String,
    pub width: u32,
    pub height: u32,
    pub duration: Option<f64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub orientation: Option<u32>,
}
