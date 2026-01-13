use std::fmt::{self, Debug};

use geo::{Coord, Geometry, Haversine, InterpolatableLine, Length, LineString};
use opening_hours_syntax::rules::OpeningHoursExpression;
use routee_compass_core::model::unit::SpeedUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::Velocity;

use super::{geometry_wkb_codec, OvertureMapsBbox, OvertureMapsNames, OvertureMapsSource};
use crate::collection::{OvertureMapsCollectionError, OvertureRecord};

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

    pub fn get_routing_class(&self) -> Result<SegmentFullType, OvertureMapsCollectionError> {
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

    // pub fn first_matching_subclass
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSubtype {
    Road,
    Rail,
    Water,
}

impl fmt::Display for SegmentSubtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SegmentSubtype::Road => "road",
            SegmentSubtype::Rail => "rail",
            SegmentSubtype::Water => "water",
        };
        f.write_str(s)
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq, Hash)]
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
    #[serde(untagged)]
    Custom(String),
}

impl fmt::Display for SegmentClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SegmentClass::Motorway => "motorway",
            SegmentClass::Primary => "primary",
            SegmentClass::Secondary => "secondary",
            SegmentClass::Tertiary => "tertiary",
            SegmentClass::Residential => "residential",
            SegmentClass::LivingStreet => "living_street",
            SegmentClass::Trunk => "trunk",
            SegmentClass::Unclassified => "unclassified",
            SegmentClass::Service => "service",
            SegmentClass::Pedestrian => "pedestrian",
            SegmentClass::Footway => "footway",
            SegmentClass::Steps => "steps",
            SegmentClass::Path => "path",
            SegmentClass::Track => "track",
            SegmentClass::Cycleway => "cycleway",
            SegmentClass::Bridleway => "bridleway",
            SegmentClass::Unknown => "unknown",
            SegmentClass::Custom(s) => s.as_str(),
        };
        f.write_str(s)
    }
}

impl<'de> Deserialize<'de> for SegmentClass {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "motorway" => Self::Motorway,
            "primary" => Self::Primary,
            "secondary" => Self::Secondary,
            "tertiary" => Self::Tertiary,
            "residential" => Self::Residential,
            "living_street" => Self::LivingStreet,
            "trunk" => Self::Trunk,
            "unclassified" => Self::Unclassified,
            "service" => Self::Service,
            "pedestrian" => Self::Pedestrian,
            "footway" => Self::Footway,
            "steps" => Self::Steps,
            "path" => Self::Path,
            "track" => Self::Track,
            "cycleway" => Self::Cycleway,
            "bridleway" => Self::Bridleway,
            "unknown" => Self::Unknown,
            _ => Self::Custom(s),
        })
    }
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

impl fmt::Display for SegmentSubclass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SegmentSubclass::Link => "link",
            SegmentSubclass::Sidewalk => "sidewalk",
            SegmentSubclass::Crosswalk => "crosswalk",
            SegmentSubclass::ParkingAisle => "parking_aisle",
            SegmentSubclass::Driveway => "driveway",
            SegmentSubclass::Alley => "alley",
            SegmentSubclass::CycleCrossing => "cycle_crossing",
        };
        f.write_str(s)
    }
}

#[derive(Eq, PartialEq, Hash)]
pub struct SegmentFullType(SegmentSubtype, SegmentClass, Option<SegmentSubclass>);

impl SegmentFullType {
    pub fn has_subclass(&self) -> bool {
        self.2.is_some()
    }

    pub fn with_subclass(&self, subclass: SegmentSubclass) -> Self {
        Self(self.0.clone(), self.1.clone(), Some(subclass))
    }

    pub fn as_str(&self) -> String {
        match self.2.as_ref() {
            Some(subclass) => format!("{}-{}-{}", self.0, self.1, subclass),
            None => format!("{}-{}", self.0, self.1),
        }
    }
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

/// travel mode for this segment.
/// see <https://docs.overturemaps.org/schema/concepts/by-theme/transportation/travel-modes/>
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentMode {
    /// category including motorized and non-motorized vehicles
    Vehicle,
    /// category over any motor vehicle type
    MotorVehicle,
    /// personal motor vehicle supported
    Car,
    /// ? unsure if it's LD/MD/HD
    Truck,
    /// motorized bike
    Motorcycle,
    /// walking mode
    Foot,
    /// non-motorized pedal bike
    Bicycle,
    /// transit vehicle
    Bus,
    /// heavy goods vehicle
    Hgv,
    /// high-occupancy vehicle
    Hov,
    /// access for emergency vehicles only
    Emergency,
}

impl SegmentMode {
    /// describes the hierarchical relationship between modes as described in
    /// <https://docs.overturemaps.org/schema/concepts/by-theme/transportation/travel-modes/#the-travel-modes-taxonomy>
    pub fn parent(&self) -> Option<SegmentMode> {
        match self {
            Self::Vehicle => None,
            Self::Foot => None,
            Self::Bicycle => Some(Self::Vehicle),
            Self::MotorVehicle => Some(Self::Vehicle),
            Self::Car => Some(Self::MotorVehicle),
            Self::Truck => Some(Self::MotorVehicle),
            Self::Motorcycle => Some(Self::MotorVehicle),
            Self::Bus => Some(Self::MotorVehicle),
            Self::Hgv => Some(Self::MotorVehicle),
            Self::Hov => Some(Self::MotorVehicle),
            Self::Emergency => Some(Self::MotorVehicle),
        }
    }
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

/// units in vehicle restrictions which may be length or weight units.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
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
#[serde(untagged)]
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

impl SegmentSpeedUnit {
    pub fn to_uom(&self, value: f64) -> Velocity {
        match self {
            SegmentSpeedUnit::Kmh => SpeedUnit::KPH.to_uom(value),
            SegmentSpeedUnit::Mph => SpeedUnit::MPH.to_uom(value),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectorReference {
    pub connector_id: String,
    pub at: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentRoute {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub network: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none", default)]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wikidata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentValueBetween<T> {
    #[serde(skip_serializing_if = "Option::is_none", default = "default_none")]
    pub value: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub between: Option<Vec<f64>>,
}

impl<T: Debug> SegmentValueBetween<T> {
    pub fn between_intersects(
        &self,
        start: f64,
        end: f64,
    ) -> Result<bool, OvertureMapsCollectionError> {
        let b_vector =
            self.between
                .as_ref()
                .ok_or(OvertureMapsCollectionError::InvalidBetweenVector(format!(
                    "`between` vector is empty: {self:?}"
                )))?;
        let (low, high) = validate_between_vector(b_vector)?;

        Ok(start < *high && end > *low)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentAccessRestriction {
    pub access_type: SegmentAccessType,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub when: Option<SegmentAccessRestrictionWhen>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vehicle: Option<String>,
}

impl SegmentAccessRestriction {
    pub fn contains_mode(&self, mode: &SegmentMode) -> bool {
        self.when
            .as_ref()
            .and_then(|w| w.mode.as_ref())
            .map(|m| m.contains(mode))
            .unwrap_or_default()
    }
}

fn default_none<T>() -> Option<T> {
    None
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentAccessRestrictionWhen {
    /// Time span or time spans during which something is open or active, specified
    /// in the OSM opening hours specification:
    /// see <https://wiki.openstreetmap.org/wiki/Key:opening_hours/specification>
    #[serde(
        with = "opening_hours_codec",
        skip_serializing_if = "Option::is_none",
        default = "default_none"
    )]
    pub during: Option<OpeningHoursExpression>,
    /// Enumerates possible travel headings along segment geometry.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub heading: Option<SegmentHeading>,
    /// Reason why a person or entity travelling on the transportation network is
    /// using a particular location.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub using: Option<Vec<SegmentUsing>>,
    /// Status of the person or entity travelling as recognized by authorities
    /// controlling the particular location
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub recognized: Option<Vec<SegmentRecognized>>,
    /// Enumerates possible travel modes. Some modes represent groups of modes.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mode: Option<Vec<SegmentMode>>,
    /// Vehicle attributes for which the rule applies
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vehicle: Option<Vec<SegmentAccessRestrictionWhenVehicle>>,
}

mod opening_hours_codec {
    use opening_hours_syntax::rules::OpeningHoursExpression;
    use serde::Deserialize;

    pub fn serialize<S>(t: &Option<OpeningHoursExpression>, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match t {
            Some(expr) => s.serialize_str(&expr.to_string()),
            None => s.serialize_none(),
        }
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Option<OpeningHoursExpression>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: Option<&str> = Option::deserialize(d)?;
        match s {
            Some(text) => opening_hours_syntax::parse(text)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

impl SegmentAccessRestrictionWhen {
    pub fn contains_mode(&self, mode: &SegmentMode) -> bool {
        self.mode
            .as_ref()
            .map(|m| m.contains(mode))
            .unwrap_or_default()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentAccessRestrictionWhenVehicle {
    dimension: SegmentVehicleDimension,
    comparison: SegmentVehicleComparator,
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    unit: Option<SegmentUnit>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentDestination {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    labels: Option<Vec<SegmentDestinationLabel>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    symbols: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    from_connector_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    to_segment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    to_connector_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    when: Option<SegmentDestinationWhen>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    final_heading: Option<SegmentHeading>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentDestinationLabel {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    value: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none", default)]
    type_str: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentDestinationWhen {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    heading: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentProhibitedTransitions {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    sequence: Option<Vec<SegmentProhibitedTransitionsSequence>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    final_heading: Option<SegmentHeading>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    when: Option<SegmentAccessRestrictionWhen>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    between: Option<Vec<f64>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentProhibitedTransitionsSequence {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    connector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    segment: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentSpeedLimit {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    min_speed: Option<SpeedLimitWithUnit>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    max_speed: Option<SpeedLimitWithUnit>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    is_max_speed_variable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    when: Option<SegmentAccessRestrictionWhen>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    between: Option<Vec<f64>>,
}

impl SegmentSpeedLimit {
    /// used to filter limits based on linear reference segment
    pub fn check_between_intersection(
        &self,
        start: f64,
        end: f64,
    ) -> Result<bool, OvertureMapsCollectionError> {
        match self.between.as_ref() {
            Some(b_vector) => {
                let (low, high) = validate_between_vector(b_vector)?;
                Ok(start < *high && end > *low)
            }
            None => Ok(true),
        }
    }

    pub fn get_max_speed(&self) -> Option<SpeedLimitWithUnit> {
        self.max_speed.clone()
    }

    /// given a sub-segment linear reference (start, end), compute the total overlapping portion
    pub fn get_linear_reference_portion(
        &self,
        start: f64,
        end: f64,
    ) -> Result<f64, OvertureMapsCollectionError> {
        match self.between.as_ref() {
            Some(b_vector) => {
                let (low, high) = validate_between_vector(b_vector)?;

                Ok((high.min(end) - low.max(start)).max(0.))
            }
            None => Ok(end - start),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpeedLimitWithUnit {
    value: i32,
    unit: SegmentSpeedUnit,
}

impl SpeedLimitWithUnit {
    pub fn to_uom_value(&self) -> Velocity {
        self.unit.to_uom(self.value as f64)
    }
}

fn validate_between_vector(
    b_vector: &Vec<f64>,
) -> Result<(&f64, &f64), OvertureMapsCollectionError> {
    let [low, high] = b_vector.as_slice() else {
        return Err(OvertureMapsCollectionError::InvalidBetweenVector(
            "Between vector has length != 2".to_string(),
        ));
    };

    if high < low {
        return Err(OvertureMapsCollectionError::InvalidBetweenVector(format!(
            "`high` is lower than `low`: [{low}, {high}]"
        )));
    }

    Ok((low, high))
}
