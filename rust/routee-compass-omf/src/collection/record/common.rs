use geo::Geometry;
use geo::MapCoords;
use geozero::error::GeozeroError;
use geozero::{wkb::Wkb, ToGeo};
use serde::de::Deserializer;
use serde::de::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn deserialize_geometry<'de, D>(deserializer: D) -> Result<Option<Geometry<f32>>, D::Error>
where
    D: Deserializer<'de>,
{
    // Assumption that this data is binary and not string.
    // convert here at the boundary of the program into f32 values.
    Option::<Vec<u8>>::deserialize(deserializer)?
        .map(|v| {
            let g = Wkb(v).to_geo()?;

            g.try_map_coords(|geo::Coord { x, y }| {
                Ok(geo::Coord {
                    x: x as f32,
                    y: y as f32,
                })
            })
        })
        .transpose()
        .map_err(|e: GeozeroError| D::Error::custom(format!("Could not decode wkb: {e}")))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OvertureMapsBbox {
    xmin: Option<f32>,
    xmax: Option<f32>,
    ymin: Option<f32>,
    ymax: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OvertureMapsSource {
    property: Option<String>,
    dataset: Option<String>,
    record_id: Option<String>,
    update_time: Option<String>,
    confidence: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OvertureMapsNames {
    primary: Option<String>,
    common: Option<HashMap<String, Option<String>>>,
    rules: Option<Vec<OvertureMapsNamesRule>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OvertureMapsNamesRule {
    variant: Option<String>,
    language: Option<String>,
    value: Option<String>,
    between: Option<Vec<f64>>,
    side: Option<String>,
}
