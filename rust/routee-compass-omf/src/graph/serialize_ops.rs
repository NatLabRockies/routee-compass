use geo::{Coord, LineString};
use itertools::Itertools;
use kdam::{tqdm, Bar, BarExt};
use rayon::prelude::*;
use routee_compass_core::model::network::{Edge, EdgeId, EdgeListId, Vertex};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::{
    collection::{
        OvertureMapsCollectionError, TransportationConnectorRecord, TransportationSegmentRecord,
    },
    graph::{segment_split::SegmentSplit, ConnectorInSegment},
};

/// serializes the Connector records into Vertices and creates a GERS id -> index mapping.
/// optionally filter to a 'keep list' of Connector ids. the vertex creation is parallelized.
pub fn create_vertices_and_lookup(
    connectors: &[TransportationConnectorRecord],
    keep_list: Option<&HashSet<&String>>,
) -> Result<(Vec<Vertex>, HashMap<String, usize>), OvertureMapsCollectionError> {
    let keep_connectors = match keep_list {
        Some(keep) => connectors
            .iter()
            .filter(|c| keep.contains(&c.id))
            .collect_vec(),
        None => connectors.iter().collect_vec(),
    };

    let vertices = keep_connectors
        .par_iter()
        .enumerate()
        .map(|(idx, c)| c.try_to_vertex(idx))
        .collect::<Result<Vec<Vertex>, OvertureMapsCollectionError>>()?;

    let mapping: HashMap<String, usize> = keep_connectors
        .iter()
        .enumerate()
        .map(|(idx, c)| (c.id.clone(), idx))
        .collect();

    Ok((vertices, mapping))
}

/// builds a lookup function from segment id to segment index
pub fn create_segment_lookup(segments: &[&TransportationSegmentRecord]) -> HashMap<String, usize> {
    segments
        .iter()
        .enumerate()
        .map(|(idx, c)| (c.id.clone(), idx))
        .collect()
}

/// collects all splits from all segment records, used to create edges.
/// the application of split ops is parallelized over the segment records.
pub fn find_splits(
    segments: &[&TransportationSegmentRecord],
    split_op: fn(
        &TransportationSegmentRecord,
    ) -> Result<Vec<SegmentSplit>, OvertureMapsCollectionError>,
) -> Result<Vec<SegmentSplit>, OvertureMapsCollectionError> {
    let result = segments
        .par_iter()
        .map(|s| split_op(s))
        .collect::<Result<Vec<Vec<SegmentSplit>>, OvertureMapsCollectionError>>()?
        .into_iter()
        .flatten()
        .collect();
    Ok(result)
}

/// identifies if any split points require creating new vertices and makes them, appending
/// them to the collections of vertex data.
pub fn extend_vertices(
    splits: &[SegmentSplit],
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    vertices: &mut Vec<Vertex>,
    vertex_lookup: &mut HashMap<String, usize>,
) -> Result<(), OvertureMapsCollectionError> {
    let bar = Bar::builder()
        .desc("locating missing connectors")
        .build()
        .map_err(|e| {
            OvertureMapsCollectionError::InternalError(format!("progress bar error: {e}"))
        })?;
    let bar = Arc::new(Mutex::new(bar));
    type MissingConnectorsResult =
        Result<Vec<Vec<(ConnectorInSegment, Coord<f32>)>>, OvertureMapsCollectionError>;
    let missing_connectors = splits
        .par_iter()
        .map(|split| {
            if let Ok(mut b) = bar.clone().lock() {
                let _ = b.update(1);
            }
            connectors_from_split(split, segments, segment_lookup)
        })
        .collect::<MissingConnectorsResult>()?
        .into_iter()
        .flatten()
        .collect_vec();
    eprintln!(); // end progress bar

    if missing_connectors.is_empty() {
        log::info!("all connectors accounted for");
        return Ok(());
    }

    // use any missing connectors to create new vertices and inject them into the vertex collections.
    let add_connectors_iter = tqdm!(
        missing_connectors.iter().enumerate(),
        total = missing_connectors.len(),
        desc = "add missing connectors"
    );
    let base_id = vertices.len();
    for (idx, (connector, coord)) in add_connectors_iter {
        let vertex_id = base_id + idx;
        let vertex_uuid = connector.connector_id.clone();
        let vertex = Vertex::new(vertex_id, coord.x, coord.y);
        vertices.push(vertex);
        let _ = vertex_lookup.insert(vertex_uuid, vertex_id);
    }
    eprintln!(); // end progress bar

    Ok(())
}

/// helper function to collect any [ConnectorInSegment] values that represent currently missing Vertices in the graph.
fn connectors_from_split(
    split: &SegmentSplit,
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
) -> Result<Vec<(ConnectorInSegment, Coord<f32>)>, OvertureMapsCollectionError> {
    split.missing_connectors().into_iter().map(|c| {
        let seg_idx = segment_lookup.get(&c.segment_id)
            .ok_or_else(|| {
                let msg = format!("while extending vertices, expected segment id {} missing from lookup", c.segment_id);
                OvertureMapsCollectionError::InvalidSegmentConnectors(msg)
            })?;
        let segment = segments.get(*seg_idx)
            .ok_or_else(|| {
                let msg = format!("while extending vertices, expected segment id {} with index {} missing from lookup", c.segment_id, seg_idx);
                OvertureMapsCollectionError::InvalidSegmentConnectors(msg)
            })?;
        let coord = segment.get_coord_at(c.linear_reference.0)?;
        Ok((c, coord))
    }).collect()
}

/// creates all edges along the provided set of splits.
///
/// # Invariants
/// the complete list of vertices (from connectors) should exist at this point.
pub fn create_edges(
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
    vertices: &[Vertex],
    vertex_lookup: &HashMap<String, usize>,
    edge_list_id: EdgeListId,
) -> Result<Vec<Edge>, OvertureMapsCollectionError> {
    splits
        .iter()
        .enumerate()
        .collect_vec()
        .par_iter()
        .map(|(idx, split)| {
            split.create_edge_from_split(
                EdgeId(*idx),
                edge_list_id,
                segments,
                segment_lookup,
                vertices,
                vertex_lookup,
            )
        })
        .collect::<Result<Vec<Edge>, OvertureMapsCollectionError>>()
}

pub fn create_geometries(
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
) -> Result<Vec<LineString<f32>>, OvertureMapsCollectionError> {
    splits
        .par_iter()
        .map(|split| split.create_geometry_from_split(segments, segment_lookup))
        .collect::<Result<Vec<LineString<f32>>, OvertureMapsCollectionError>>()
}
