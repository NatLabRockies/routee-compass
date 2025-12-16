use routee_compass_core::model::network::{Edge, EdgeId, EdgeListId, VertexId};
use uom::si::f64::Length;

use crate::{
    collection::{OvertureMapsCollectionError, TransportationSegmentRecord},
    graph::omf_graph::OmfGraphVectorized,
};

pub enum SegmentSplit {
    ConnectorSplit {
        connector_id_src: String,
        at_src: f64,
        connector_id_dst: String,
        at_dst: f64,
    },
}

impl SegmentSplit {
    /// Modify in-place a vectorized graph according to a split logic
    pub fn split(
        &self,
        vectorized_graph: &mut OmfGraphVectorized,
        segment: &TransportationSegmentRecord,
    ) -> Result<(), OvertureMapsCollectionError> {
        match self {
            SegmentSplit::ConnectorSplit {
                connector_id_src,
                at_src,
                connector_id_dst,
                at_dst,
            } => {
                // get src, dst VertexId via lookup into mapping->vertices
                // Asumming `missing` is not valid in this case
                let src_id = vectorized_graph.vertex_lookup.get(connector_id_src).ok_or(
                    OvertureMapsCollectionError::InvalidSegmentConnectors(format!(
                        "segment references unknown connector {connector_id_src}"
                    )),
                )?;

                let dst_id = vectorized_graph.vertex_lookup.get(connector_id_dst).ok_or(
                    OvertureMapsCollectionError::InvalidSegmentConnectors(format!(
                        "segment references unknown connector {connector_id_dst}"
                    )),
                )?;

                // create this edge, push onto edges
                let distance =
                    segment.get_distance_at(*at_dst)? - segment.get_distance_at(*at_src)?;
                let edge = Edge {
                    edge_list_id: EdgeListId(vectorized_graph.edge_list_id),
                    edge_id: EdgeId(vectorized_graph.edges.len()),
                    src_vertex_id: VertexId(*src_id),
                    dst_vertex_id: VertexId(*dst_id),
                    distance: Length::new::<uom::si::length::meter>(distance),
                };

                vectorized_graph.edges.push(edge);
            }
        };

        Ok(())
    }
}
