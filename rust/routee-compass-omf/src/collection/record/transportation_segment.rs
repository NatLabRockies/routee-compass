use geo::{Coord, Geometry, Haversine, InterpolatableLine, Length, LineString};
use serde::{Deserialize, Serialize};

use crate::collection::{OvertureMapsCollectionError, OvertureRecord};

use super::deserialize_geometry;
use super::{OvertureMapsBbox, OvertureMapsNames, OvertureMapsSource};

/// Represents a transportation segment record in the Overture Maps schema.
/// This struct contains information about a segment of transportation infrastructure,
/// such as roads or railways, including geometry, metadata, access restrictions,
/// and other attributes relevant to routing and mapping.
///
/// see <https://docs.overturemaps.org/schema/reference/transportation/segment/>
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransportationSegmentRecord {
    /// GERS identifier for this segment record
    pub id: String,
    #[serde(deserialize_with = "deserialize_geometry")]
    pub geometry: Option<Geometry<f32>>,
    bbox: OvertureMapsBbox,
    subtype: Option<SegmentSubtype>,
    class: Option<SegmentClass>,
    subclass: Option<SegmentSubclass>,
    version: i32,
    sources: Option<Vec<Option<OvertureMapsSource>>>,
    names: Option<OvertureMapsNames>,
    pub connectors: Option<Vec<ConnectorReference>>,
    routes: Option<Vec<SegmentRoute>>,
    subclass_rules: Option<Vec<SegmentValueBetween<SegmentSubclass>>>,
    access_restrictions: Option<Vec<SegmentAccessRestriction>>,
    level_rules: Option<Vec<SegmentValueBetween<i32>>>,
    destinations: Option<Vec<SegmentDestination>>,
    prohibited_transitions: Option<Vec<SegmentProhibitedTransitions>>,
    road_surface: Option<Vec<SegmentValueBetween<SegmentRoadSurfaceType>>>,
    road_flags: Option<Vec<SegmentValueBetween<Vec<SegmentRoadFlags>>>>,
    speed_limits: Option<Vec<SegmentSpeedLimit>>,
    width_rules: Option<Vec<SegmentValueBetween<f64>>>,
    rail_flags: Option<Vec<SegmentValueBetween<Vec<SegmentRailFlags>>>>,
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
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSubtype {
    Road,
    Railway,
    Waterway,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentClass {
    Motorway,
    Primary,
    Secondary,
    Tertiary,
    Residential,
    LivingStreet,
    Trunk,
    Unclassified,
    Service,
    Pedestrian,
    Footway,
    Steps,
    Path,
    Track,
    Cycleway,
    Bridleway,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSubclass {
    Link,
    Sidewalk,
    Crosswalk,
    ParkingAisle,
    Driveway,
    Alley,
    CycleCrossing,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentAccessType {
    Allowed,
    Denied,
    Designated,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentRoadSurfaceType {
    Unknown,
    Paved,
    Unpaved,
    Gravel,
    Dirt,
    PavingStones,
    Metal,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentHeading {
    Forward,
    Backward,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentRoadFlags {
    IsBridge,
    IsLink,
    IsTunnel,
    IsUnderConstruction,
    IsAbandoned,
    IsCovered,
    IsIndoor,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentRailFlags {
    IsBridge,
    IsTunnel,
    IsUnderConstruction,
    IsAbandoned,
    IsCovered,
    IsPassenger,
    IsFreight,
    IsDisused,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentUsing {
    AsCustomer,
    AtDestination,
    ToDeliver,
    ToFarm,
    ForForestry,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentRecognized {
    AsPermitted,
    AsPrivate,
    AsDisabled,
    AsEmployee,
    AsStudent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentMode {
    Vehicle,
    MotorVehicle,
    Car,
    Truck,
    Motorcycle,
    Foot,
    Bicycle,
    Bus,
    Hgv,
    Hov,
    Emergency,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentVehicleDimension {
    AxleCount,
    Height,
    Length,
    Weight,
    Width,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentVehicleComparator {
    GreaterThan,
    GreaterThanEqual,
    Equal,
    LessThan,
    LessThanEqual,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentUnit {
    Length(SegmentLengthUnit),
    Weight(SegmentWeightUnit),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentLengthUnit {
    #[serde(rename = "in")]
    Inches,
    #[serde(rename = "ft")]
    Feet,
    #[serde(rename = "yd")]
    Yard,
    #[serde(rename = "mi")]
    Mile,
    #[serde(rename = "cm")]
    Centimeter,
    #[serde(rename = "m")]
    Meter,
    #[serde(rename = "km")]
    Kilometer,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentWeightUnit {
    Imperial(SegmentImperialWeightUnit),
    Metric(SegmentMetricWeightUnit),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentImperialWeightUnit {
    #[serde(rename = "oz")]
    Ounce,
    #[serde(rename = "lb")]
    Pound,
    #[serde(rename = "st")]
    Stone,
    #[serde(rename = "lt")]
    LongTon,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentMetricWeightUnit {
    #[serde(rename = "g")]
    Gram,
    #[serde(rename = "kg")]
    Kilogram,
    #[serde(rename = "t")]
    MetricTon,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentSpeedUnit {
    #[serde(rename = "km/h")]
    Kmh,
    #[serde(rename = "mph")]
    Mph,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectorReference {
    pub connector_id: String,
    pub at: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentRoute {
    name: Option<String>,
    network: Option<String>,
    #[serde(rename = "ref")]
    reference: Option<String>,
    symbol: Option<String>,
    wikidata: Option<String>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentValueBetween<T> {
    value: Option<T>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentAccessRestriction {
    access_type: Option<SegmentAccessType>,
    when: Option<SegmentAccessRestrictionWhen>,
    vehicle: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentAccessRestrictionWhen {
    /// time when restriction is active, as defined via OSM opening hours
    /// see <https://wiki.openstreetmap.org/wiki/Key:opening_hours/specification>
    during: Option<String>,
    heading: Option<SegmentHeading>,
    using: Option<Vec<SegmentUsing>>,
    recognized: Option<Vec<SegmentRecognized>>,
    mode: Option<Vec<SegmentMode>>,
    vehicle: Option<Vec<SegmentAccessRestrictionWhenVehicle>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentAccessRestrictionWhenVehicle {
    dimension: SegmentVehicleDimension,
    comparison: SegmentVehicleComparator,
    value: f64,
    unit: Option<SegmentUnit>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentDestination {
    labels: Option<Vec<SegmentDestinationLabel>>,
    symbols: Option<Vec<String>>,
    from_connector_id: Option<String>,
    to_segment_id: Option<String>,
    to_connector_id: Option<String>,
    when: Option<SegmentDestinationWhen>,
    final_heading: Option<SegmentHeading>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentDestinationLabel {
    value: Option<String>,
    #[serde(rename = "type")]
    type_str: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentDestinationWhen {
    heading: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentProhibitedTransitions {
    sequence: Option<Vec<SegmentProhibitedTransitionsSequence>>,
    final_heading: Option<SegmentHeading>,
    when: Option<SegmentAccessRestrictionWhen>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentProhibitedTransitionsSequence {
    connector: Option<String>,
    segment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SegmentSpeedLimit {
    min_speed: Option<SpeedLimitWithUnit>,
    max_speed: Option<SpeedLimitWithUnit>,
    is_max_speed_variable: Option<bool>,
    when: Option<SegmentAccessRestrictionWhen>,
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SpeedLimitWithUnit {
    value: i32,
    unit: SegmentSpeedUnit,
}
