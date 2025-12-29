use std::collections::HashMap;

use routee_compass_core::model::network::{Edge, EdgeId, EdgeListId, Vertex, VertexId};
use uom::si::f64::Length;

use crate::{
    collection::{OvertureMapsCollectionError, TransportationSegmentRecord},
    graph::connector_in_segment::ConnectorInSegment,
};

pub enum SegmentSplit {
    /// splits at the connectors (vertices) ignoring linear-referenced split points
    /// for other attributes such as speed. does not require creating additional vertices.
    SimpleConnectorSplit {
        src: ConnectorInSegment,
        dst: ConnectorInSegment,
    },
}

impl SegmentSplit {
    /// identifies any locations where additional coordinates are needed.

    /// when creating any missing connectors, call [ConnectorInSegment::new_without_connector_id]
    /// which generates a new connector_id based on the segment_id and linear referencing position.
    pub fn missing_connectors(&self) -> Vec<ConnectorInSegment> {
        match self {
            SegmentSplit::SimpleConnectorSplit { .. } => vec![],
        }
    }

    /// Modify in-place a vectorized graph according to a split logic.
    ///
    /// # Invariants
    ///
    /// all expected connectors must exist in the vertices collection before calling this method.
    pub fn create_edge_from_split(
        &self,
        edge_id: EdgeId,
        edge_list_id: EdgeListId,
        segments: &[TransportationSegmentRecord],
        segment_lookup: &HashMap<String, usize>,
        _vertices: &[Vertex],
        vertex_lookup: &HashMap<String, usize>,
    ) -> Result<Edge, OvertureMapsCollectionError> {
        use OvertureMapsCollectionError as E;
        match self {
            SegmentSplit::SimpleConnectorSplit { src, dst } => {
                // get the shared segment id for src + dst
                let segment_id = if src.segment_id != dst.segment_id {
                    let msg = format!(
                        "attempting to create edge were src segment != dst segment ('{}' != '{}')",
                        src.segment_id, dst.segment_id
                    );
                    return Err(E::InvalidSegmentConnectors(msg));
                } else {
                    &src.segment_id
                };

                // get src, dst VertexId via lookup into mapping->vertices
                // Asumming `missing` is not valid in this case
                let src_id =
                    vertex_lookup
                        .get(&src.connector_id)
                        .ok_or(E::InvalidSegmentConnectors(format!(
                            "segment references unknown connector {}",
                            src.connector_id
                        )))?;

                let dst_id =
                    vertex_lookup
                        .get(&dst.connector_id)
                        .ok_or(E::InvalidSegmentConnectors(format!(
                            "segment references unknown connector {}",
                            dst.connector_id
                        )))?;

                // create this edge, push onto edges
                if dst.linear_reference < src.linear_reference {
                    return Err(E::InvalidSegmentConnectors(format!(
                        "SimpleConnectorSplit: at_dst ({}) < at_src ({}) for connectors {} -> {}",
                        dst.linear_reference,
                        src.linear_reference,
                        src.connector_id,
                        dst.connector_id
                    )));
                }
                let segment_idx = segment_lookup.get(segment_id).ok_or_else(|| {
                    let msg = format!("missing lookup entry for segment {segment_id}");
                    E::InvalidSegmentConnectors(msg)
                })?;
                let segment = segments.get(*segment_idx).ok_or_else(|| {
                    let msg = format!(
                        "missing lookup entry for segment {segment_id} with index {segment_idx}"
                    );
                    E::InvalidSegmentConnectors(msg)
                })?;
                let dst_distance = segment.get_distance_at(dst.linear_reference.0)?;
                let src_distance = segment.get_distance_at(src.linear_reference.0)?;
                let distance = dst_distance - src_distance;
                let edge = Edge {
                    edge_list_id,
                    edge_id,
                    src_vertex_id: VertexId(*src_id),
                    dst_vertex_id: VertexId(*dst_id),
                    distance: Length::new::<uom::si::length::meter>(distance as f64),
                };

                Ok(edge)
            }
        }
    }
}
