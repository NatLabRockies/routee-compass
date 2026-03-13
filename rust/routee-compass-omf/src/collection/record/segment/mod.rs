mod access_restriction;
mod access_restriction_when;
mod class;
mod destination;
mod flags;
mod mode;
mod record;
mod route;
mod speed_limit;
mod subclass;
mod subtype;
mod value_between;
mod vehicle;

pub use access_restriction::{SegmentAccessRestriction, SegmentAccessType};
pub use access_restriction_when::{
    SegmentAccessRestrictionWhen, SegmentAccessRestrictionWhenVehicle,
};
pub use class::SegmentClass;
pub use destination::{
    SegmentDestination, SegmentDestinationLabel, SegmentDestinationLabelType,
    SegmentDestinationWhen, SegmentProhibitedTransitions, SegmentProhibitedTransitionsSequence,
    SegmentSymbol,
};
pub use flags::{SegmentRailFlags, SegmentRoadFlags, SegmentRoadSurfaceType};
pub use mode::{SegmentHeading, SegmentMode, SegmentRecognized, SegmentUsing};
pub use record::TransportationSegmentRecord;
pub use route::{ConnectorReference, SegmentRoute};
pub use speed_limit::{SegmentSpeedLimit, SegmentSpeedUnit, SpeedLimitWithUnit};
pub use subclass::SegmentSubclass;
pub use subtype::{SegmentFullType, SegmentSubtype};
pub use value_between::SegmentValueBetween;
pub use vehicle::{
    SegmentImperialWeightUnit, SegmentLengthUnit, SegmentMetricWeightUnit, SegmentUnit,
    SegmentVehicleComparator, SegmentVehicleDimension, SegmentWeightUnit,
};
