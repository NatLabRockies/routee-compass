use geo::{Bearing, Coord, Haversine, LineString};
use itertools::Itertools;
use kdam::{tqdm, Bar, BarExt};
use rayon::prelude::*;
use routee_compass_core::model::network::{Edge, EdgeId, EdgeList, EdgeListId, Vertex};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::{
    collection::{
        OvertureMapsCollectionError, SegmentAccessRestrictionWhen, SegmentFullType,
        TransportationConnectorRecord, TransportationSegmentRecord,
    },
    graph::{omf_graph::OmfEdgeList, segment_split::SegmentSplit, ConnectorInSegment},
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
/// the application of split ops is parallelized over the segment records, as splits are
/// not ordered.
pub fn find_splits(
    segments: &[&TransportationSegmentRecord],
    when: Option<&SegmentAccessRestrictionWhen>,
    split_op: fn(
        &TransportationSegmentRecord,
        Option<&SegmentAccessRestrictionWhen>,
    ) -> Result<Vec<SegmentSplit>, OvertureMapsCollectionError>,
) -> Result<Vec<SegmentSplit>, OvertureMapsCollectionError> {
    let result = segments
        .par_iter()
        .map(|s| split_op(s, when))
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

pub fn create_speeds(
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
) -> Result<Vec<Option<f64>>, OvertureMapsCollectionError> {
    splits
        .par_iter()
        .map(|split| split.get_split_speed(segments, segment_lookup))
        .collect::<Result<Vec<Option<f64>>, OvertureMapsCollectionError>>()
}

pub fn create_segment_full_types(
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
) -> Result<Vec<SegmentFullType>, OvertureMapsCollectionError> {
    splits
        .par_iter()
        .map(|split| split.get_split_segment_full_type(segments, segment_lookup))
        .collect::<Result<Vec<SegmentFullType>, OvertureMapsCollectionError>>()
}

pub fn create_speed_by_segment_type_lookup<'a>(
    initial_speeds: &[Option<f64>],
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
    classes: &'a [SegmentFullType],
) -> Result<HashMap<&'a SegmentFullType, f64>, OvertureMapsCollectionError> {
    let split_lenghts = splits
        .iter()
        .map(|split| {
            split
                .get_split_length_meters(segments, segment_lookup)
                .map(|v_f32| v_f32 as f64)
        })
        .collect::<Result<Vec<f64>, OvertureMapsCollectionError>>()?;

    let mut speed_sum_lookup: HashMap<&SegmentFullType, (f64, f64)> = HashMap::new();

    for ((class, w), speed) in classes.iter().zip(split_lenghts).zip(initial_speeds) {
        let Some(x) = speed else { continue }; // skip missing speeds

        let element = speed_sum_lookup.entry(class).or_insert((0.0, 0.0));
        element.0 += w * x;
        element.1 += w;
    }

    Ok(speed_sum_lookup
        .into_iter()
        .filter(|&(_k, (_wx, w))| w != 0.0)
        .map(|(k, (wx, w))| (k, wx / w))
        .collect::<HashMap<&SegmentFullType, f64>>())
}

/// get the tuples (segment_id, (src_id, dst_id)) referencing the original omf dataset
pub fn get_omf_ids(
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
) -> Result<Vec<(String, (String, String))>, OvertureMapsCollectionError> {
    splits
        .par_iter()
        .map(|split| split.get_omf_segment_id(segments, segment_lookup))
        .collect()
}

pub fn get_global_average_speed(
    initial_speeds: &[Option<f64>],
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
) -> Result<f64, OvertureMapsCollectionError> {
    let split_lenghts = splits
        .iter()
        .map(|split| {
            split
                .get_split_length_meters(segments, segment_lookup)
                .map(|v_f32| v_f32 as f64)
        })
        .collect::<Result<Vec<f64>, OvertureMapsCollectionError>>()?;

    let mut total_length = 0.;
    let mut weighted_sum = 0.;
    for (opt_speed, length) in initial_speeds.iter().zip(split_lenghts) {
        let Some(speed) = opt_speed else { continue }; // skip missing speeds

        total_length += length;
        weighted_sum += length * speed;
    }

    if total_length < 1e-6 {
        return Err(OvertureMapsCollectionError::InternalError(format!(
            "internal division by zero when computing average speed: {initial_speeds:?}"
        )));
    }

    Ok(weighted_sum / total_length)
}

/// Computes the outward bearings of the geometries representing
/// segment splits using the last two point in the LineStrings
pub fn bearing_deg_from_geometries(
    geometries: &[LineString<f32>],
) -> Result<Vec<f64>, OvertureMapsCollectionError> {
    geometries
        .iter()
        .map(|linestring| {
            let n = linestring.0.len();
            if n < 2 {
                return Err(OvertureMapsCollectionError::InternalError(format!(
                    "cannot compute bearing on linestring with less than two points: {linestring:?}"
                )));
            }
            let p0 = linestring.0[n - 2];
            let p1 = linestring.0[n - 1];
            Ok(Haversine.bearing(p0.into(), p1.into()) as f64)
        })
        .collect()
}

/// Given an OmfEdgeList and a boolean mask, returns an updated edge list
/// with the mask applied.
pub fn clean_omf_edge_list(omf_list: OmfEdgeList, mask: Vec<bool>) -> OmfEdgeList {
    let edges = EdgeList(
        omf_list
            .edges
            .0
            .iter()
            .enumerate()
            .filter_map(|(idx, edge)| mask[idx].then_some(*edge))
            .collect::<Vec<Edge>>()
            .into_boxed_slice(),
    );

    let geometries = omf_list
        .geometries
        .into_iter()
        .enumerate()
        .filter_map(|(idx, ls)| mask[idx].then_some(ls))
        .collect();

    let classes = omf_list
        .classes
        .into_iter()
        .enumerate()
        .filter_map(|(idx, cls)| mask[idx].then_some(cls))
        .collect();

    let speeds = omf_list
        .speeds
        .into_iter()
        .enumerate()
        .filter_map(|(idx, s)| mask[idx].then_some(s))
        .collect();

    let bearings = omf_list
        .bearings
        .into_iter()
        .enumerate()
        .filter_map(|(idx, b)| mask[idx].then_some(b))
        .collect();
    
    let omf_segment_connector_ids = omf_list.omf_segment_connector_ids.map(
        |ids| ids.into_iter()
        .enumerate()
        .filter_map(|(idx, b)| mask[idx].then_some(b))
        .collect()
    );

    OmfEdgeList {
        edge_list_id: omf_list.edge_list_id,
        edges,
        geometries,
        classes,
        speeds,
        speed_lookup: omf_list.speed_lookup,
        bearings,
        omf_segment_connector_ids
    }
}
