use chrono::{DateTime, Utc};
use rhai::Dynamic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct MediaContext {
    pub time: TimeContext,
    pub space: SpaceContext,
    pub source: SourceContext,
    pub special: SpecialContext,
    pub r#type: String,
    pub meta: HashMap<String, Dynamic>,
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
pub struct SpecialContext {
    pub md5: String,
    pub md5_short: String,
    pub count: u32,
}
