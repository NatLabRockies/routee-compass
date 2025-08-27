use geo::Geometry;
use geozero::{wkb::Wkb, ToGeo};
use serde::de::Deserializer;
use serde::de::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn deserialize_geometry<'de, D>(deserializer: D) -> Result<Option<Geometry>, D::Error>
where
    D: Deserializer<'de>,
{
    let hex_str: String = Option::deserialize(deserializer)?.ok_or(D::Error::custom(
        String::from("Could not deserialize hex string"),
    ))?;
    let wkb_bytes: Vec<u8> = hex::decode(&hex_str)
        .map_err(|e| D::Error::custom(format!("Could not decode wkb: {e}")))?;
    Ok(Wkb(&wkb_bytes).to_geo().ok())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OvertureMapsBbox {
    xmin: Option<f32>,
    xmax: Option<f32>,
    ymin: Option<f32>,
    ymax: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OvertureMapsSource {
    property: Option<String>,
    dataset: Option<String>,
    record_id: Option<String>,
    update_time: Option<String>,
    confidence: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OvertureMapsNames {
    primary: Option<String>,
    common: Option<HashMap<String, Option<String>>>,
    rules: Option<Vec<OvertureMapsNamesRule>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OvertureMapsNamesRule {
    variant: Option<String>,
    language: Option<String>,
    value: Option<String>,
    between: Option<Vec<f64>>,
    side: Option<String>,
}
