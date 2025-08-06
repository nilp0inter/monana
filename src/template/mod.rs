use anyhow::Result;
use camino::Utf8PathBuf;
use regex::Regex;
use std::collections::HashMap;

use crate::metadata::MediaContext;

lazy_static::lazy_static! {
    static ref TEMPLATE_VAR: Regex = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)*)\}").unwrap();
}

pub fn apply_template(template: &str, context: &MediaContext) -> Result<Utf8PathBuf> {
    let variables = build_variable_map(context);

    let result = TEMPLATE_VAR.replace_all(template, |caps: &regex::Captures| {
        let var_name = &caps[1];
        variables
            .get(var_name)
            .cloned()
            .unwrap_or_else(|| format!("{{unknown:{var_name}}}"))
    });

    Ok(Utf8PathBuf::from(result.to_string()))
}

fn build_variable_map(context: &MediaContext) -> HashMap<&'static str, String> {
    let mut vars = HashMap::new();

    // Time variables
    vars.insert("time.yyyy", context.time.yyyy.clone());
    vars.insert("time.mm", context.time.mm.clone());
    vars.insert("time.dd", context.time.dd.clone());
    vars.insert("time.hh", context.time.hh.clone());
    vars.insert("time.min", context.time.min.clone());
    vars.insert("time.ss", context.time.ss.clone());
    vars.insert("time.month_name", context.time.month_name.clone());
    vars.insert("time.weekday", context.time.weekday.clone());

    // Space variables
    vars.insert("space.country", context.space.country.clone());
    vars.insert("space.country_code", context.space.country_code.clone());
    vars.insert("space.state", context.space.state.clone());
    vars.insert("space.city", context.space.city.clone());
    vars.insert("space.district", context.space.district.clone());
    vars.insert("space.road", context.space.road.clone());
    vars.insert("space.lat", context.space.lat.to_string());
    vars.insert("space.lon", context.space.lon.to_string());

    // Source variables
    vars.insert("source.path", context.source.path.clone());
    vars.insert("source.name", context.source.name.clone());
    vars.insert("source.extension", context.source.extension.clone());
    vars.insert("source.original", context.source.original.clone());
    vars.insert("source.size", context.source.size.to_string());

    // Media variables
    vars.insert("media.type", context.media.r#type.clone());
    vars.insert("media.width", context.media.width.to_string());
    vars.insert("media.height", context.media.height.to_string());

    if let Some(make) = &context.media.camera_make {
        vars.insert("media.camera_make", make.clone());
    }
    if let Some(model) = &context.media.camera_model {
        vars.insert("media.camera_model", model.clone());
    }

    vars
}
