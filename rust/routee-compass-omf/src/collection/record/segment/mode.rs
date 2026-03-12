use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SegmentHeading {
    Forward,
    Backward,
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
