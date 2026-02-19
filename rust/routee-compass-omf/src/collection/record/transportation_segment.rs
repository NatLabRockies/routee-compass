use std::fmt::{self, Debug};

use geo::{Coord, Geometry, Haversine, InterpolatableLine, Length, LineString};
use routee_compass_core::model::unit::SpeedUnit;
use serde::{Deserialize, Serialize};
use uom::si::f64::Velocity;

use super::{geometry_wkb_codec, OvertureMapsBbox, OvertureMapsNames, OvertureMapsSource};
use crate::collection::{
    record::during_expression::DuringExpression, OvertureMapsCollectionError, OvertureRecord,
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

/// Fully qualified segment type including type, class and subclass. E.g. road-service-driveway
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
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
#[allow(clippy::enum_variant_names)]
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

impl SegmentVehicleComparator {
    pub fn apply(&self, value: f64, restriction: f64) -> bool {
        match self {
            SegmentVehicleComparator::GreaterThan => value > restriction,
            SegmentVehicleComparator::GreaterThanEqual => value >= restriction,
            SegmentVehicleComparator::Equal => value == restriction,
            SegmentVehicleComparator::LessThan => value < restriction,
            SegmentVehicleComparator::LessThanEqual => value <= restriction,
        }
    }
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

impl SegmentLengthUnit {
    pub fn to_uom(&self, value: f64) -> uom::si::f64::Length {
        match self {
            SegmentLengthUnit::Inches => uom::si::f64::Length::new::<uom::si::length::inch>(value),
            SegmentLengthUnit::Feet => uom::si::f64::Length::new::<uom::si::length::foot>(value),
            SegmentLengthUnit::Yard => uom::si::f64::Length::new::<uom::si::length::yard>(value),
            SegmentLengthUnit::Mile => uom::si::f64::Length::new::<uom::si::length::mile>(value),
            SegmentLengthUnit::Centimeter => {
                uom::si::f64::Length::new::<uom::si::length::centimeter>(value)
            }
            SegmentLengthUnit::Meter => uom::si::f64::Length::new::<uom::si::length::meter>(value),
            SegmentLengthUnit::Kilometer => {
                uom::si::f64::Length::new::<uom::si::length::kilometer>(value)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum SegmentWeightUnit {
    Imperial(SegmentImperialWeightUnit),
    Metric(SegmentMetricWeightUnit),
}

impl SegmentWeightUnit {
    pub fn to_uom(&self, value: f64) -> uom::si::f64::Mass {
        use SegmentImperialWeightUnit as I;
        use SegmentMetricWeightUnit as M;
        use SegmentWeightUnit as SWU;

        match self {
            SWU::Imperial(I::Ounce) => uom::si::f64::Mass::new::<uom::si::mass::ounce>(value),
            SWU::Imperial(I::Pound) => uom::si::f64::Mass::new::<uom::si::mass::pound>(value),
            // Couldn't find "Stone" so we use the transformation to Kg
            SWU::Imperial(I::Stone) => {
                uom::si::f64::Mass::new::<uom::si::mass::kilogram>(value * 6.350288)
            }
            SWU::Imperial(I::LongTon) => uom::si::f64::Mass::new::<uom::si::mass::ton_long>(value),
            SWU::Metric(M::Kilogram) => uom::si::f64::Mass::new::<uom::si::mass::kilogram>(value),
            SWU::Metric(M::Gram) => uom::si::f64::Mass::new::<uom::si::mass::gram>(value),
            SWU::Metric(M::MetricTon) => uom::si::f64::Mass::new::<uom::si::mass::ton>(value),
        }
    }
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
    /// Used to filter limits based on a linear reference segment.
    /// Returns `true` if the open interval `(between[0], between[1])`
    /// overlaps with the open interval `(start, end)`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use bambam_omf::collection::SegmentSpeedLimit;
    ///
    /// let limit = SegmentSpeedLimit {
    ///     min_speed: None,
    ///     max_speed: None,
    ///     is_max_speed_variable: None,
    ///     when: None,
    ///     between: Some(vec![10.0, 20.0]),
    /// };
    ///
    /// // (15, 25) overlaps with (10, 20)
    /// assert!(limit.check_open_intersection(15.0, 25.0).unwrap());
    /// // (20, 30) does not overlap with open interval (10, 20)
    /// assert!(!limit.check_open_intersection(20.0, 30.0).unwrap());
    /// ```
    pub fn check_open_intersection(
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SegmentAccessRestrictionWhen {
    /// Time span or time spans during which something is open or active, specified
    /// in the OSM opening hours specification:
    /// see <https://wiki.openstreetmap.org/wiki/Key:opening_hours/specification>
    #[serde(skip_serializing_if = "Option::is_none", default = "default_none")]
    pub during: Option<DuringExpression>,
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
    pub dimension: SegmentVehicleDimension,
    pub comparison: SegmentVehicleComparator,
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unit: Option<SegmentUnit>,
}

impl SegmentAccessRestrictionWhenVehicle {
    /// returns true if the when provided would pass the restriction
    /// based on the comparison logic
    pub fn is_valid(&self, when: &SegmentAccessRestrictionWhenVehicle) -> bool {
        use SegmentUnit as SU;

        if when.dimension != self.dimension {
            return false;
        }

        match (&self.unit, &when.unit) {
            (Some(SU::Length(this_unit)), Some(SU::Length(other_unit))) => {
                let this_value_f64 = this_unit.to_uom(self.value).get::<uom::si::length::meter>();
                let other_value_f64 = other_unit
                    .to_uom(when.value)
                    .get::<uom::si::length::meter>();
                self.comparison.apply(other_value_f64, this_value_f64)
            }
            (Some(SU::Weight(this_unit)), Some(SU::Weight(other_unit))) => {
                let this_value_f64 = this_unit
                    .to_uom(self.value)
                    .get::<uom::si::mass::kilogram>();
                let other_value_f64 = other_unit
                    .to_uom(when.value)
                    .get::<uom::si::mass::kilogram>();
                self.comparison.apply(other_value_f64, this_value_f64)
            }

            // Should be handled by the if statement checking the dimension but
            // just to be sure
            (Some(SU::Weight(_)), Some(SU::Length(_))) => false,
            (Some(SU::Length(_)), Some(SU::Weight(_))) => false,

            // If we miss any unit, check the raw values
            _ => self.comparison.apply(when.value, self.value),
        }
    }
}

/// Describes objects that can be reached by following a transportation
/// segment in the same way those objects are described on signposts or
/// ground writing that a traveller following the segment would observe
/// in the real world. This allows navigation systems to refer to signs
/// and observable writing that a traveller actually sees.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentDestination {
    /// Labeled destinations that can be reached by following the segment.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub labels: Option<Vec<SegmentDestinationLabel>>,
    /// Indicates what special symbol/icon is present on a signpost, visible as road marking or similar.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub symbols: Option<Vec<SegmentSymbol>>,
    /// Identifies the point of physical connection on this segment before which the destination sign or marking is visible.
    pub from_connector_id: String,
    /// Identifies the segment to transition to reach the destination(s) labeled on the sign or marking.
    pub to_segment_id: String,
    /// Identifies the point of physical connection on the segment identified by 'to_segment_id' to transition to for reaching the destination(s).
    pub to_connector_id: String,
    /// Properties defining travel headings that match a rule.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub when: Option<SegmentDestinationWhen>,
    /// Enumerates possible travel headings along segment geometry.
    pub final_heading: SegmentHeading,
}

/// Indicates what special symbol/icon is present on a signpost, visible as road marking or similar.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SegmentSymbol {
    Motorway,
    Airport,
    Hospital,
    Center,
    Industrial,
    Parking,
    Bus,
    TrainStation,
    RestArea,
    Ferry,
    Motorroad,
    Fuel,
    Viewpoint,
    FuelDiesel,
    Food,
    Lodging,
    Info,
    CampSite,
    Interchange,
    Restrooms,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentDestinationLabel {
    pub value: String,
    pub r#type: SegmentDestinationLabelType,
}

/// The type of object of the destination label.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SegmentDestinationLabelType {
    Street,
    Country,
    RouteRef,
    TowardRouteRef,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentDestinationWhen {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub heading: Option<SegmentHeading>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub mode: Option<Vec<SegmentMode>>,
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
    pub min_speed: Option<SpeedLimitWithUnit>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_speed: Option<SpeedLimitWithUnit>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub is_max_speed_variable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub when: Option<SegmentAccessRestrictionWhen>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub between: Option<Vec<f64>>,
}

impl SegmentSpeedLimit {
    /// Used to filter limits based on a linear reference segment.
    /// Returns `true` if the open interval `(between[0], between[1])`
    /// overlaps with the open interval `(start, end)`.
    ///
    /// # Examples
    ///
    /// Basic overlap:
    /// ```
    /// # use bambam_omf::collection::SegmentSpeedLimit;
    ///
    /// let limit = SegmentSpeedLimit {
    ///     min_speed: None,
    ///     max_speed: None,
    ///     is_max_speed_variable: None,
    ///     when: None,
    ///     between: Some(vec![10.0, 20.0]),
    /// };
    ///
    /// // (15, 25) overlaps with (10, 20)
    /// assert!(limit.check_open_intersection(15.0, 25.0).unwrap());
    /// ```
    ///
    /// No overlap:
    /// ```
    /// # use bambam_omf::collection::SegmentSpeedLimit;
    /// # let limit = SegmentSpeedLimit {
    /// #    min_speed: None,
    /// #    max_speed: None,
    /// #    is_max_speed_variable: None,
    /// #    when: None,
    /// #    between: Some(vec![10.0, 20.0]),
    /// # };
    ///
    /// // (20, 30) does not overlap with open interval (10, 20)
    /// assert!(!limit.check_open_intersection(20.0, 30.0).unwrap());
    /// ```
    ///
    /// No `between` restriction means always applicable:
    /// ```
    /// # use bambam_omf::collection::SegmentSpeedLimit;
    /// let limit = SegmentSpeedLimit {
    ///     min_speed: None,
    ///     max_speed: None,
    ///     is_max_speed_variable: None,
    ///     when: None,
    ///     between: None,
    /// };
    ///
    /// assert!(limit.check_open_intersection(100.0, 200.0).unwrap());
    /// ```
    pub fn check_open_intersection(
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
    pub value: i32,
    pub unit: SegmentSpeedUnit,
}

impl SpeedLimitWithUnit {
    pub fn to_uom_value(&self) -> Velocity {
        self.unit.to_uom(self.value as f64)
    }
}

/// This function takes a [`Vec<f64>`]` and returns `a` and `b` if and only
/// if the vector has exactly two elements and the second one is higher than the
/// first one. Otherwise it returns an error.
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
