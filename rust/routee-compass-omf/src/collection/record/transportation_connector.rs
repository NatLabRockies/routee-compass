use super::deserialize_geometry;
use super::{OvertureMapsBbox, OvertureMapsSource};
use geo::Geometry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TransportationConnectorRecord {
    id: Option<String>,
    #[serde(deserialize_with = "deserialize_geometry")]
    geometry: Option<Geometry>,
    bbox: OvertureMapsBbox,
    version: i32,
    sources: Option<Vec<Option<OvertureMapsSource>>>,
}
