use geo::{Bearing, Haversine, Length, Line, LineString};
use itertools::Itertools;
use rayon::prelude::*;
use routee_compass_core::model::network::{Edge, EdgeId, EdgeList, EdgeListId, Vertex, VertexId};
use std::collections::{HashMap, HashSet};

use crate::{
    collection::{
        OvertureMapsCollectionError, SegmentAccessRestrictionWhen, SegmentFullType,
        TransportationConnectorRecord, TransportationSegmentRecord,
    },
    graph::{consts, omf_graph::OmfEdgeList, segment_split::SegmentSplit},
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
        .par_iter()
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

/// get the tuples (segment_id, linear reference) referencing the original omf dataset
pub fn get_segment_omf_ids(
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
) -> Result<Vec<(String, f64)>, OvertureMapsCollectionError> {
    splits
        .par_iter()
        .map(|split| split.get_omf_segment_id_and_linear_ref(segments, segment_lookup))
        .collect()
}

pub fn get_global_average_speed(
    initial_speeds: &[Option<f64>],
    segments: &[&TransportationSegmentRecord],
    segment_lookup: &HashMap<String, usize>,
    splits: &[SegmentSplit],
) -> Result<f64, OvertureMapsCollectionError> {
    let split_lenghts = splits
        .par_iter()
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

    if total_length < consts::F64_DISTANCE_TOLERANCE {
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
        .par_iter()
        .map(|linestring| {
            let n = linestring.0.len();
            if n < 2 {
                return Err(OvertureMapsCollectionError::InternalError(format!(
                    "cannot compute bearing on linestring with less than two points: {linestring:?}"
                )));
            }

            let p1 = linestring.0[n - 1];
            let mut p0 = linestring.0[n - 2];

            // Loop backwards to find a point far enough away to yield a valid bearing vector
            for i in (0..n - 1).rev() {
                let candidate = linestring.0[i];
                let line = Line::new(candidate, p1);

                if Haversine.length(&line) > consts::F32_DISTANCE_TOLERANCE {
                    p0 = candidate;
                    break;
                }
            }

            Ok(Haversine.bearing(p0.into(), p1.into()) as f64)
        })
        .collect()
}

/// Auxiliary function used to determine which vertices need to be removed
/// and what the new id of the remaining vertices is. Each element
/// of the returned vector is the new VertexId if the vertex is kept, and
/// None if the vertex is removed.
pub fn compute_vertex_remapping(
    vertices: &[Vertex],
    edge_lists: &[OmfEdgeList],
    island_edges: &[(EdgeListId, EdgeId)],
) -> Result<Vec<Option<VertexId>>, OvertureMapsCollectionError> {
    // 1. Create a fast lookup for edges being removed
    let removed_edges: HashSet<(EdgeListId, EdgeId)> = island_edges.iter().cloned().collect();

    // 2. Identify all vertices that are part of at least one valid (non-island) edge
    // across ALL edge lists.
    let mut valid_vertices = HashSet::new();

    for edge_list in edge_lists {
        for edge in edge_list.edges.0.iter() {
            if !removed_edges.contains(&(edge.edge_list_id, edge.edge_id)) {
                valid_vertices.insert(edge.src_vertex_id);
                valid_vertices.insert(edge.dst_vertex_id);
            }
        }
    }

    // 3. Create the mapping
    let mut new_id: usize = 0;
    Ok((0..vertices.len())
        .map(|old_id| {
            if valid_vertices.contains(&VertexId(old_id)) {
                // If it is used by at least one connected mode, we keep it
                new_id += 1;
                Some(VertexId(new_id - 1))
            } else {
                // Orphaned entirely
                None
            }
        })
        .collect())
}
/// Given an OmfEdgeList and a boolean mask, returns an updated edge list
/// with the mask applied.
pub fn clean_omf_edge_list(
    omf_list: OmfEdgeList,
    mask: Vec<bool>,
    vertex_remapping: &[Option<VertexId>],
) -> Result<OmfEdgeList, OvertureMapsCollectionError> {
    let edges = EdgeList(
        omf_list
            .edges
            .0
            .iter()
            // Apply mask
            .filter(|edge| mask[edge.edge_id.0])
            // enumerate produces the new indices after some edges were removed
            .enumerate()
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(|(new_idx, edge)| {
                // Retrieve the correct vertex ids
                let new_src_id = vertex_remapping
                    .get(edge.src_vertex_id.0)
                    .copied()
                    .flatten()
                    .ok_or(OvertureMapsCollectionError::InternalError(format!(
                        "src vertex_id {} for edge ({},{}) was removed",
                        edge.src_vertex_id, edge.edge_list_id, edge.edge_id
                    )))?;
                let new_dst_id = vertex_remapping
                    .get(edge.dst_vertex_id.0)
                    .copied()
                    .flatten()
                    .ok_or(OvertureMapsCollectionError::InternalError(format!(
                        "dst vertex_id {} for edge ({},{}) was removed",
                        edge.dst_vertex_id, edge.edge_list_id, edge.edge_id
                    )))?;

                Ok(Edge {
                    edge_id: EdgeId(new_idx),
                    src_vertex_id: new_src_id,
                    dst_vertex_id: new_dst_id,
                    ..*edge
                })
            })
            .collect::<Result<Vec<Edge>, OvertureMapsCollectionError>>()?
            .into_boxed_slice(),
    );

    let geometries = omf_list
        .geometries
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| mask[*idx])
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(_, ls)| ls)
        .collect();

    let classes = omf_list
        .classes
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| mask[*idx])
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(_, cls)| cls)
        .collect();

    let speeds = omf_list
        .speeds
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| mask[*idx])
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(_, s)| s)
        .collect();

    let bearings = omf_list
        .bearings
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| mask[*idx])
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(_, b)| b)
        .collect();

    let omf_segment_ids = omf_list
        .omf_segment_ids
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| mask[*idx])
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(_, i)| i)
        .collect();

    Ok(OmfEdgeList {
        edge_list_id: omf_list.edge_list_id,
        edges,
        geometries,
        classes,
        speeds,
        speed_lookup: omf_list.speed_lookup,
        bearings,
        omf_segment_ids,
    })
}
