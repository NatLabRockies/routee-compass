use serde::{Deserialize, Serialize};

use super::{
    access_restriction_when::SegmentAccessRestrictionWhen,
    mode::{SegmentHeading, SegmentMode},
};

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
