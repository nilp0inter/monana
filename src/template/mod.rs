use anyhow::Result;
use camino::Utf8PathBuf;
use regex::Regex;
use rhai::Dynamic;

use crate::metadata::MediaContext;

lazy_static::lazy_static! {
    static ref TEMPLATE_VAR: Regex = Regex::new(r"\{([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)*)\}").unwrap();
}

pub fn apply_template(template: &str, context: &MediaContext) -> Result<Utf8PathBuf> {
    let result = TEMPLATE_VAR.replace_all(template, |caps: &regex::Captures| {
        let var_name = &caps[1];
        resolve_variable(var_name, context).unwrap_or_else(|| format!("{{unknown:{var_name}}}"))
    });

    Ok(Utf8PathBuf::from(result.to_string()))
}

fn resolve_variable(var_name: &str, context: &MediaContext) -> Option<String> {
    let parts: Vec<&str> = var_name.split('.').collect();

    match parts.as_slice() {
        ["time", field] => match *field {
            "yyyy" => Some(context.time.yyyy.clone()),
            "mm" => Some(context.time.mm.clone()),
            "dd" => Some(context.time.dd.clone()),
            "hh" => Some(context.time.hh.clone()),
            "min" => Some(context.time.min.clone()),
            "ss" => Some(context.time.ss.clone()),
            "month_name" => Some(context.time.month_name.clone()),
            "weekday" => Some(context.time.weekday.clone()),
            _ => None,
        },
        ["space", field] => match *field {
            "country" => Some(context.space.country.clone()),
            "country_code" => Some(context.space.country_code.clone()),
            "state" => Some(context.space.state.clone()),
            "city" => Some(context.space.city.clone()),
            "district" => Some(context.space.district.clone()),
            "road" => Some(context.space.road.clone()),
            "lat" => Some(context.space.lat.to_string()),
            "lon" => Some(context.space.lon.to_string()),
            _ => None,
        },
        ["source", field] => match *field {
            "path" => Some(context.source.path.clone()),
            "name" => Some(context.source.name.clone()),
            "extension" => Some(context.source.extension.clone()),
            "original" => Some(context.source.original.clone()),
            "size" => Some(context.source.size.to_string()),
            _ => None,
        },
        ["special", field] => match *field {
            "md5" => Some(context.special.md5.clone()),
            "md5_short" => Some(context.special.md5_short.clone()),
            "count" => Some(context.special.count.to_string()),
            _ => None,
        },
        ["type"] => Some(context.r#type.clone()),
        ["meta", tag] => context.meta.get(*tag).map(dynamic_to_string),
        _ => None,
    }
}

fn dynamic_to_string(value: &Dynamic) -> String {
    if value.is_string() {
        value.clone().into_string().unwrap_or_default()
    } else if value.is_int() {
        value.as_int().unwrap_or(0).to_string()
    } else if value.is_float() {
        let f = value.as_float().unwrap_or(0.0);
        // Format floats nicely, removing unnecessary decimals
        if f.fract() == 0.0 {
            format!("{f:.0}")
        } else {
            f.to_string()
        }
    } else if value.is_bool() {
        value.as_bool().unwrap_or(false).to_string()
    } else {
        // Fallback for other types
        value.to_string()
    }
}
