use crate::collection::{OvertureMapsCollectionError, OvertureRecord};

use super::deserialize_geometry;
use super::{OvertureMapsBbox, OvertureMapsSource};
use geo::Geometry;
use serde::{Deserialize, Serialize};

/// Represents a transportation connector record as defined in the Overture Maps Foundation schema.
/// This struct contains the fields describing a transportation connector, including its unique
/// identifier, geometry, bounding box, version, and data sources.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransportationConnectorRecord {
    pub id: String,
    #[serde(deserialize_with = "deserialize_geometry")]
    geometry: Option<Geometry>,
    bbox: OvertureMapsBbox,
    version: i32,
    sources: Option<Vec<Option<OvertureMapsSource>>>,
}

impl TryFrom<OvertureRecord> for TransportationConnectorRecord {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: OvertureRecord) -> Result<Self, Self::Error> {
        match value {
            OvertureRecord::Connector(record) => Ok(record),
            _ => Err(OvertureMapsCollectionError::DeserializeTypeError(format!(
                "Cannot transform record {value:#?} into TransportationConnectorRecord"
            ))),
        }
    }
}
