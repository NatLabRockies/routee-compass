mod building;
mod common;
mod overture_record;
mod place;
mod record_type;
mod transportation_collection;
mod transportation_connector;
mod transportation_segment;

pub use building::BuildingsRecord;
pub use overture_record::OvertureRecord;
pub use place::PlacesRecord;
pub use record_type::OvertureRecordType;
pub use transportation_collection::TransportationCollection;
pub use transportation_connector::TransportationConnectorRecord;
pub use transportation_segment::{
    SegmentAccessRestriction, SegmentAccessRestrictionWhen, SegmentAccessType, SegmentClass,
    SegmentDestination, SegmentFullType, SegmentHeading, SegmentMode, SegmentRecognized,
    SegmentSpeedLimit, SegmentSpeedUnit, SegmentSubclass, SegmentSubtype, SegmentUsing,
    TransportationSegmentRecord,
};

// Common structs and functions for many record types
pub use common::OvertureMapsBbox;
pub use common::OvertureMapsNames;
pub use common::OvertureMapsSource;
pub mod geometry_wkb_codec;
