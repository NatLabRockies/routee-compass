use geo::{Coord, Geometry, Haversine, InterpolatableLine, Length, LineString};
use serde::{Deserialize, Serialize};

use crate::collection::{
    record::{geometry_wkb_codec, OvertureMapsBbox, OvertureMapsNames, OvertureMapsSource},
    OvertureMapsCollectionError, OvertureRecord,
};

use super::{
    access_restriction::SegmentAccessRestriction,
    class::SegmentClass,
    destination::{SegmentDestination, SegmentProhibitedTransitions},
    flags::{SegmentRailFlags, SegmentRoadFlags, SegmentRoadSurfaceType},
    route::{ConnectorReference, SegmentRoute},
    speed_limit::SegmentSpeedLimit,
    subclass::SegmentSubclass,
    subtype::{SegmentFullType, SegmentSubtype},
    value_between::SegmentValueBetween,
};

/// Represents a transportation segment record in the Overture Maps schema.
/// This struct contains information about a segment of transportation infrastructure,
/// such as roads or railways, including geometry, metadata, access restrictions,
/// and other attributes relevant to routing and mapping.
///
/// see <https://docs.overturemaps.org/schema/reference/transportation/segment/>
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TransportationSegmentRecord {
    /// GERS identifier for this segment record
    pub id: String,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        with = "geometry_wkb_codec"
    )]
    pub geometry: Option<Geometry<f32>>,
    pub bbox: OvertureMapsBbox,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub subtype: Option<SegmentSubtype>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub class: Option<SegmentClass>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub subclass: Option<SegmentSubclass>,
    pub version: i32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sources: Option<Vec<Option<OvertureMapsSource>>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub names: Option<OvertureMapsNames>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub connectors: Option<Vec<ConnectorReference>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub routes: Option<Vec<SegmentRoute>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub subclass_rules: Option<Vec<SegmentValueBetween<SegmentSubclass>>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub access_restrictions: Option<Vec<SegmentAccessRestriction>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub level_rules: Option<Vec<SegmentValueBetween<i32>>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub destinations: Option<Vec<SegmentDestination>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prohibited_transitions: Option<Vec<SegmentProhibitedTransitions>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub road_surface: Option<Vec<SegmentValueBetween<SegmentRoadSurfaceType>>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub road_flags: Option<Vec<SegmentValueBetween<Vec<SegmentRoadFlags>>>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub speed_limits: Option<Vec<SegmentSpeedLimit>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub width_rules: Option<Vec<SegmentValueBetween<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rail_flags: Option<Vec<SegmentValueBetween<Vec<SegmentRailFlags>>>>,
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
    /// retrieve geometry mapped to linestring variant. returns Err if geometry is empty or
    /// if it is not a linestring
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

    pub fn get_distance_at_meters(&self, at: f64) -> Result<f32, OvertureMapsCollectionError> {
        if !(0.0..=1.0).contains(&at) {
            return Err(OvertureMapsCollectionError::InvalidLinearReference(at));
        }
        let linestring = self.get_linestring()?;
        Ok(Haversine.length(linestring) * at as f32)
    }

    /// gets a coordinate from this linestring at some linear reference.
    pub fn get_coord_at(&self, at: f64) -> Result<Coord<f32>, OvertureMapsCollectionError> {
        if !(0.0..=1.0).contains(&at) {
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

    pub fn get_segment_full_type(&self) -> Result<SegmentFullType, OvertureMapsCollectionError> {
        use OvertureMapsCollectionError as E;

        Ok(SegmentFullType(
            self.subtype.clone().ok_or(E::MissingAttribute(format!(
                "`subtype` not found in segment: {self:?}"
            )))?,
            self.class.clone().ok_or(E::MissingAttribute(format!(
                "`class` not found in segment: {self:?}"
            )))?,
            self.subclass.clone(),
        ))
    }
}
