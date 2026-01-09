//! functions mapped onto [TransportationSegmentRecord] rows to create [SegmentSplit] values

use crate::{
    collection::{OvertureMapsCollectionError, TransportationSegmentRecord},
    graph::{segment_split::SegmentSplit, ConnectorInSegment},
};
use itertools::Itertools;

/// creates simple connector splits from a record.
pub fn process_simple_connector_splits(
    segment: &TransportationSegmentRecord,
) -> Result<Vec<SegmentSplit>, OvertureMapsCollectionError> {
    let result = segment
        .connectors
        .as_ref()
        .ok_or(OvertureMapsCollectionError::InvalidSegmentConnectors(
            format!("connectors is empty for segment record '{}'", segment.id),
        ))?
        .iter()
        .tuple_windows()
        .map(|(src, dst)| {
            // todo: we are currently incorporating heading. whether we reverse src/dst here for backward headings
            // or within SegmentSplit (current solution), we still need to figure out what the heading is from the
            // "destinations" key.
            let src = ConnectorInSegment::new(segment.id.clone(), src.connector_id.clone(), src.at);
            let dst = ConnectorInSegment::new(segment.id.clone(), dst.connector_id.clone(), dst.at);
            SegmentSplit::SimpleConnectorSplit { src, dst }
        })
        .collect::<Vec<SegmentSplit>>();
    Ok(result)
}
