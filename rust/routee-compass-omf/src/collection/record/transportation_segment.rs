use geo::{Coord, Geometry, Haversine, InterpolatableLine, Length, LineString};
use serde::{Deserialize, Serialize};

use crate::collection::{OvertureMapsCollectionError, OvertureRecord};

use super::deserialize_geometry;
use super::{OvertureMapsBbox, OvertureMapsNames, OvertureMapsSource};

/// Represents a transportation segment record in the Overture Maps schema.
/// This struct contains information about a segment of transportation infrastructure,
/// such as roads or railways, including geometry, metadata, access restrictions,
/// and other attributes relevant to routing and mapping.
#[derive(Debug, Serialize, Deserialize)]
pub struct TransportationSegmentRecord {
    /// GERS identifier for this segment record
    pub id: String,
    #[serde(deserialize_with = "deserialize_geometry")]
    pub geometry: Option<Geometry<f32>>,
    bbox: OvertureMapsBbox,
    version: i32,
    sources: Option<Vec<Option<OvertureMapsSource>>>,
    subtype: Option<String>,
    class: Option<String>,
    names: Option<OvertureMapsNames>,
    pub connectors: Option<Vec<ConnectorReference>>,
    routes: Option<Vec<SegmentRoute>>,
    subclass_rules: Option<Vec<SegmentValueBetween<String>>>,
    access_restrictions: Option<Vec<SegmentAccessRestriction>>,
    level_rules: Option<Vec<SegmentValueBetween<i32>>>,
    destinations: Option<Vec<SegmentDestination>>,
    prohibited_transitions: Option<Vec<SegmentProhibitedTransitions>>,
    road_surface: Option<Vec<SegmentValueBetween<String>>>,
    road_flags: Option<Vec<SegmentValueBetween<Vec<String>>>>,
    speed_limits: Option<Vec<SegmentSpeedLimit>>,
    width_rules: Option<Vec<SegmentValueBetween<f64>>>,
    subclass: Option<String>,
    rail_flags: Option<Vec<SegmentValueBetween<Vec<String>>>>,
}

impl TryFrom<OvertureRecord> for TransportationSegmentRecord {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: OvertureRecord) -> Result<Self, Self::Error> {
        match value {
            OvertureRecord::Segment(record) => Ok(record),
            _ => Err(OvertureMapsCollectionError::DeserializeTypeError(format!(
                "Cannot transform record {value:#?} into TransportationSegmentRecord"
            ))),
        }
    }
}

impl TransportationSegmentRecord {
    pub fn get_linestring(&self) -> Result<&LineString<f32>, OvertureMapsCollectionError> {
        let geometry = self.geometry.as_ref().ok_or_else(|| {
            OvertureMapsCollectionError::InvalidGeometry("empty geometry".to_string())
        })?;
        match geometry {
            Geometry::LineString(line_string) => Ok(line_string),
            _ => Err(OvertureMapsCollectionError::InvalidGeometry(format!(
                "geometry was not a linestring {geometry:?}"
            ))),
        }
    }

    pub fn get_distance_at(&self, at: f64) -> Result<f32, OvertureMapsCollectionError> {
        if at < 0.0 || 1.0 < at {
            return Err(OvertureMapsCollectionError::InvalidLinearReference(at));
        }
        let linestring = self.get_linestring()?;
        Ok(Haversine.length(linestring) * at as f32)
    }

    /// gets a coordinate from this linestring at some linear reference.
    pub fn get_coord_at(&self, at: f64) -> Result<Coord<f32>, OvertureMapsCollectionError> {
        if at < 0.0 || 1.0 < at {
            return Err(OvertureMapsCollectionError::InvalidLinearReference(at));
        }
        let linestring = self.get_linestring()?;
        match linestring.point_at_ratio_from_start(&Haversine, at as f32) {
            Some(pt) => Ok(pt.0),
            None => {
                let msg = format!(
                    "unexpected error getting point for segment {} at {at}",
                    self.id
                );
                Err(OvertureMapsCollectionError::InternalError(msg))
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectorReference {
    pub connector_id: String,
    pub at: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentRoute {
    name: Option<String>,
    network: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    symbol: Option<String>,
    wikidata: Option<String>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentValueBetween<T> {
    value: Option<T>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentAccessRestriction {
    access_type: Option<String>,
    when: Option<SegmentAccessRestrictionWhen>,
    vehicle: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentAccessRestrictionWhen {
    during: Option<String>,
    heading: Option<String>,
    using: Option<Vec<String>>,
    recognized: Option<Vec<String>>,
    mode: Option<Vec<String>>,
    vehicle: Option<Vec<SegmentAccessRestrictionWhenVehicle>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentAccessRestrictionWhenVehicle {
    dimension: Option<String>,
    comparison: Option<String>,
    value: Option<f64>,
    unit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentDestination {
    labels: Option<Vec<SegmentDestinationLabel>>,
    symbols: Option<Vec<String>>,
    from_connector_id: Option<String>,
    to_segment_id: Option<String>,
    to_connector_id: Option<String>,
    when: Option<SegmentDestinationWhen>,
    final_heading: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentDestinationLabel {
    value: Option<String>,
    #[serde(rename = "type")]
    type_str: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentDestinationWhen {
    heading: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentProhibitedTransitions {
    sequence: Option<Vec<SegmentProhibitedTransitionsSequence>>,
    final_heading: Option<String>,
    when: Option<SegmentAccessRestrictionWhen>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentProhibitedTransitionsSequence {
    connector: Option<String>,
    segment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentSpeedLimit {
    min_speed: Option<SegmentValueUnit<i32>>,
    max_speed: Option<SegmentValueUnit<i32>>,
    is_max_speed_variable: Option<bool>,
    when: Option<SegmentAccessRestrictionWhen>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SegmentValueUnit<T> {
    value: Option<T>,
    unit: Option<String>,
}
