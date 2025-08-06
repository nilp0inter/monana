use anyhow::Result;
use reverse_geocoder::ReverseGeocoder;

use super::context::SpaceContext;

lazy_static::lazy_static! {
    static ref GEOCODER: ReverseGeocoder = ReverseGeocoder::new();
}

pub fn reverse_geocode(latitude: f64, longitude: f64) -> Result<SpaceContext> {
    let result = GEOCODER.search((latitude, longitude));

    Ok(SpaceContext {
        city: result.record.name.clone(),
        country: result.record.cc.clone(),
        country_code: result.record.cc.clone(),
        state: result.record.admin1.clone(),
        lat: latitude,
        lon: longitude,
        ..Default::default()
    })
}
