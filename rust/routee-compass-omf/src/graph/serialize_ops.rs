use itertools::Itertools;
use routee_compass_core::model::network::Vertex;
use std::collections::HashMap;

use crate::{
    collection::{
        OvertureMapsCollectionError, TransportationConnectorRecord,
        TransportationSegmentRecord,
    },
    graph::segment_split::SegmentSplit,
};

pub fn get_connectors_mapping(
    connectors: &Vec<TransportationConnectorRecord>,
) -> Result<(Vec<Vertex>, HashMap<String, usize>), OvertureMapsCollectionError> {
    let vertices = connectors
        .iter()
        .enumerate()
        .map(|(idx, c)| c.try_to_vertex(idx))
        .collect::<Result<Vec<Vertex>, OvertureMapsCollectionError>>()?;

    let mapping: HashMap<String, usize> = connectors
        .iter()
        .enumerate()
        .map(|(idx, c)| (c.id.clone(), idx))
        .collect();

    Ok((vertices, mapping))
}

pub fn get_connector_splits(
    segment: &TransportationSegmentRecord,
) -> Result<Vec<SegmentSplit>, OvertureMapsCollectionError> {
    Ok(segment
        .connectors
        .as_ref()
        .ok_or(OvertureMapsCollectionError::InvalidSegmentConnectors(
            String::from("connectors is None"),
        ))?
        .iter()
        .tuple_windows()
        .map(|(src, dst)| SegmentSplit::ConnectorSplit {
            connector_id_src: src.connector_id.clone(),
            at_src: src.at,
            connector_id_dst: dst.connector_id.clone(),
            at_dst: dst.at,
        })
        .collect::<Vec<SegmentSplit>>())
}
