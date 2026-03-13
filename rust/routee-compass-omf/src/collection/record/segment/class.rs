use std::fmt;

use serde::{Deserialize, Serialize};

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
