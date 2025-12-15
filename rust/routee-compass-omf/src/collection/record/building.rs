use geo::Geometry;
use serde::{Deserialize, Serialize};

use super::{deserialize_geometry, OvertureRecord};
use super::{OvertureMapsBbox, OvertureMapsNames, OvertureMapsSource};
use crate::collection::OvertureMapsCollectionError;

#[derive(Debug, Serialize, Deserialize)]
pub struct BuildingsRecord {
    id: Option<String>,
    #[serde(deserialize_with = "deserialize_geometry")]
    geometry: Option<Geometry>,
    bbox: OvertureMapsBbox,
    version: i32,
    sources: Option<Vec<Option<OvertureMapsSource>>>,
    names: Option<OvertureMapsNames>,
    subtype: Option<String>,
    class: Option<String>,
    level: Option<i32>,
    has_parts: Option<bool>,
    is_underground: Option<bool>,
    height: Option<f64>,
    num_floors: Option<i32>,
    num_floors_underground: Option<i32>,
    min_height: Option<f64>,
    min_floor: Option<i32>,
    facade_color: Option<String>,
    facade_material: Option<String>,
    roof_material: Option<String>,
    roof_shape: Option<String>,
    roof_direction: Option<f64>,
    roof_orientation: Option<String>,
    roof_color: Option<String>,
}

impl BuildingsRecord {
    pub fn get_class(&self) -> Option<String> {
        self.class.clone()
    }

    pub fn get_geometry(&self) -> Option<Geometry> {
        self.geometry.clone()
    }
}

impl TryFrom<OvertureRecord> for BuildingsRecord {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: OvertureRecord) -> Result<Self, Self::Error> {
        match value {
            OvertureRecord::Buildings(record) => Ok(record),
            _ => Err(OvertureMapsCollectionError::DeserializeTypeError(format!(
                "Cannot transform record {value:#?} into BuildingRecord"
            ))),
        }
    }
}
