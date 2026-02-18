use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use super::serialize_ops as ops;
use crate::{
    app::network::{IslandDetectionAlgorithmConfiguration, NetworkEdgeListConfiguration},
    collection::{
        record::SegmentHeading, OvertureMapsCollectionError, SegmentAccessRestrictionWhen,
        SegmentFullType, TransportationCollection, TransportationSegmentRecord,
    },
    graph::{
        component_algorithm::island_detection_algorithm, segment_ops,
        serialize_ops::clean_omf_edge_list, vertex_serializable::VertexSerializable,
    },
};
use geo::LineString;
use itertools::Itertools;
use kdam::tqdm;
use rayon::prelude::*;
use routee_compass_core::model::network::{EdgeConfig, EdgeId, EdgeList, EdgeListId, Vertex};
use wkt::ToWkt;

pub const COMPASS_VERTEX_FILENAME: &str = "vertices-compass.csv.gz";
pub const COMPASS_EDGES_FILENAME: &str = "edges-compass.csv.gz";
pub const GEOMETRIES_FILENAME: &str = "edges-geometries-enumerated.txt.gz";
pub const SPEEDS_FILENAME: &str = "edges-speeds-mph-enumerated.txt.gz";
pub const CLASSES_FILENAME: &str = "edges-classes-enumerated.txt.gz";
pub const SPEED_MAPPING_FILENAME: &str = "edges-classes-speed-mapping.csv.gz";
pub const OMF_SEGMENT_IDS_FILENAME: &str = "edges-omf-segment-ids.csv.gz";
pub const OMF_CONNECTOR_IDS_FILENAME: &str = "vertices-omf-connector-ids.txt.gz";
pub const BEARINGS_FILENAME: &str = "edges-bearings-enumerated.txt.gz";

pub struct OmfGraphVectorized {
    pub vertices: Vec<Vertex>,
    pub edge_lists: Vec<OmfEdgeList>,
    pub edge_list_config: Vec<NetworkEdgeListConfiguration>,
    /// for each OMF ID, the vertex index
    pub vertex_lookup: HashMap<String, usize>,
}

pub struct OmfEdgeList {
    pub edge_list_id: EdgeListId,
    pub edges: EdgeList,
    pub geometries: Vec<LineString<f32>>,
    pub classes: Vec<SegmentFullType>,
    pub speeds: Vec<f64>,
    pub speed_lookup: HashMap<String, f64>,
    pub bearings: Vec<f64>,
    pub omf_segment_ids: Vec<(String, f64)>,
}

impl OmfGraphVectorized {
    /// create a vectorized graph dataset from a [TransportationCollection]
    pub fn new(
        collection: &TransportationCollection,
        configuration: &[NetworkEdgeListConfiguration],
        island_detection_configuration: Option<IslandDetectionAlgorithmConfiguration>,
    ) -> Result<Self, OvertureMapsCollectionError> {
        // process all connectors into vertices
        let (mut vertices, mut vertex_lookup) =
            ops::create_vertices_and_lookup(&collection.connectors, None)?;

        // for each mode configuration, create an edge list
        let mut edge_lists: Vec<OmfEdgeList> = vec![];
        for (index, edge_list_config) in configuration.iter().enumerate() {
            let edge_list_id = EdgeListId(index);

            // create arguments for segment processing into edges
            let mut filter = edge_list_config.filter.clone();
            filter.sort(); // sort for performance

            // filter to the segments that match our travel mode filter(s)
            let segments: Vec<&TransportationSegmentRecord> = collection
                .segments
                .par_iter()
                .filter(|r| edge_list_config.filter.iter().all(|f| f.matches_filter(r)))
                .collect();
            let segment_lookup = ops::create_segment_lookup(&segments);

            // the splits are locations in each segment record where we want to define a vertex
            // which may not yet exist on the graph. this is where we begin to impose directivity
            // in our records.
            let mut splits = vec![];
            for heading in [SegmentHeading::Forward, SegmentHeading::Backward] {
                let mut when: SegmentAccessRestrictionWhen = edge_list_config.into();
                when.heading = Some(heading);

                let directed_splits = ops::find_splits(
                    &segments,
                    Some(&when),
                    segment_ops::process_simple_connector_splits,
                )?;
                splits.extend(directed_splits);
            }

            // depending on the split method, we may need to create additional vertices at locations
            // which are not OvertureMaps-defined connector types.
            ops::extend_vertices(
                &splits,
                &segments,
                &segment_lookup,
                &mut vertices,
                &mut vertex_lookup,
            )?;

            // create all edges based on the above split points using all vertices.
            let edges = ops::create_edges(
                &segments,
                &segment_lookup,
                &splits,
                &vertices,
                &vertex_lookup,
                edge_list_id,
            )?;
            let geometries = ops::create_geometries(&segments, &segment_lookup, &splits)?;
            let bearings = ops::bearing_deg_from_geometries(&geometries)?;
            let classes = ops::create_segment_full_types(&segments, &segment_lookup, &splits)?;

            let speeds = ops::create_speeds(&segments, &segment_lookup, &splits)?;
            let speed_lookup = ops::create_speed_by_segment_type_lookup(
                &speeds,
                &segments,
                &segment_lookup,
                &splits,
                &classes,
            )?;

            // insert global speed value for reference
            let global_speed =
                ops::get_global_average_speed(&speeds, &segments, &segment_lookup, &splits)?;

            // omf ids
            let omf_segment_ids = ops::get_segment_omf_ids(&segments, &segment_lookup, &splits)?;

            // match speeds according to classes
            let speeds = speeds
                .into_par_iter()
                .zip(&classes)
                .map(|(opt_speed, class)| match opt_speed {
                    Some(speed) => Some(speed),
                    None => speed_lookup.get(class).copied(),
                })
                // Fix the None with -1 for now
                .map(|opt| match opt {
                    Some(v) => v,
                    None => global_speed,
                })
                .collect::<Vec<f64>>();

            // transform speed lookup into owned string
            let mut speed_lookup = speed_lookup
                .iter()
                .map(|(&k, v)| (k.as_str(), *v))
                .collect::<HashMap<String, f64>>();
            speed_lookup.insert(String::from("_global_"), global_speed);

            let edge_list = OmfEdgeList {
                edge_list_id,
                edges: EdgeList(edges.into_boxed_slice()),
                geometries,
                classes,
                speeds,
                speed_lookup,
                bearings,
                omf_segment_ids,
            };
            edge_lists.push(edge_list);
        }

        // Compute islands in resulting edge lists and remove island edges
        if let Some(algorithm_config) = island_detection_configuration {
            let ref_edge_lists = edge_lists
                .iter()
                .map(|e| &e.edges)
                .collect::<Vec<&EdgeList>>();
            let island_edges = island_detection_algorithm(
                &ref_edge_lists,
                &vertices,
                algorithm_config.min_distance,
                algorithm_config.distance_unit,
                algorithm_config.parallel_execution,
            )?;

            // Refactor Vec into Hashmap
            let mut edges_lookup: HashMap<EdgeListId, Vec<EdgeId>> = HashMap::new();
            for (a, b) in island_edges {
                edges_lookup.entry(a).or_default().push(b);
            }

            // Clean the edge lists
            edge_lists = edge_lists
                .into_iter()
                .map(|omf_list| {
                    let empty_vec = vec![];
                    let edges_to_remove: HashSet<&EdgeId> = edges_lookup
                        .get(&omf_list.edge_list_id)
                        .unwrap_or(&empty_vec)
                        .iter()
                        .collect();

                    let mask = omf_list
                        .edges
                        .0
                        .iter()
                        .map(|edge| !edges_to_remove.contains(&edge.edge_id))
                        .collect::<Vec<bool>>();

                    clean_omf_edge_list(omf_list, mask)
                })
                .collect::<Vec<OmfEdgeList>>();
        };

        let result = Self {
            vertices,
            edge_lists,
            edge_list_config: configuration.to_vec(),
            vertex_lookup,
        };

        Ok(result)
    }

    /// write the graph to disk in vectorized Compass format.
    pub fn write_compass(
        &self,
        output_directory: &Path,
        overwrite: bool,
        export_omf_ids: bool,
    ) -> Result<(), OvertureMapsCollectionError> {
        kdam::term::init(false);
        kdam::term::hide_cursor().map_err(|e| {
            OvertureMapsCollectionError::InternalError(format!("progress bar error: {e}"))
        })?;

        // create output directory if missing
        crate::util::fs::create_dirs(output_directory)?;
        use crate::util::fs::serialize_into_csv;
        use crate::util::fs::serialize_into_enumerated_txt;

        // write vertices
        serialize_into_csv(
            self.vertices.iter().map(|v| VertexSerializable::from(*v)),
            COMPASS_VERTEX_FILENAME,
            output_directory,
            overwrite,
            "write vertex dataset",
        )?;

        // reversing the vertex lookup to get the connector id of each vertex
        if export_omf_ids {
            let connectors_omf_ids = self
                .vertex_lookup
                .iter()
                .sorted_by_key(|(_, v)| *v)
                .map(|(k, _)| k.clone())
                .collect::<Vec<String>>();

            // Write connector OMF IDs
            serialize_into_enumerated_txt(
                &connectors_omf_ids,
                OMF_CONNECTOR_IDS_FILENAME,
                &output_directory,
                overwrite,
                "write connector OMF ids",
            )?;
        }

        // write each edge list
        let edge_list_iter = tqdm!(
            self.edge_lists.iter().zip(self.edge_list_config.iter()),
            desc = "edge list",
            total = self.edge_lists.len(),
            position = 0
        );
        for (edge_list, edge_list_config) in edge_list_iter {
            let mode_str = &edge_list_config.mode;
            let mode_dir = output_directory.join(mode_str);
            crate::util::fs::create_dirs(&mode_dir)?;

            // Write Edges
            serialize_into_csv(
                edge_list.edges.0.iter().map(|row| EdgeConfig {
                    edge_id: row.edge_id,
                    src_vertex_id: row.src_vertex_id,
                    dst_vertex_id: row.dst_vertex_id,
                    distance: row.distance.get::<uom::si::length::meter>(),
                }),
                COMPASS_EDGES_FILENAME,
                &mode_dir,
                overwrite,
                "write edges",
            )?;

            // Write geometries
            serialize_into_enumerated_txt(
                edge_list
                    .geometries
                    .iter()
                    .map(|row| row.to_wkt().to_string()),
                GEOMETRIES_FILENAME,
                &mode_dir,
                overwrite,
                "write geometries",
            )?;

            // Write speeds
            serialize_into_enumerated_txt(
                &edge_list.speeds,
                SPEEDS_FILENAME,
                &mode_dir,
                overwrite,
                "write speeds",
            )?;

            // Write classes
            serialize_into_enumerated_txt(
                edge_list.classes.iter().map(|class| class.as_str()),
                CLASSES_FILENAME,
                &mode_dir,
                overwrite,
                "write classes",
            )?;

            // Write speed_mapping
            serialize_into_csv(
                edge_list.speed_lookup.iter(),
                SPEED_MAPPING_FILENAME,
                &mode_dir,
                overwrite,
                "write speed mapping",
            )?;

            // Write bearings
            serialize_into_enumerated_txt(
                &edge_list.bearings,
                BEARINGS_FILENAME,
                &mode_dir,
                overwrite,
                "write bearings",
            )?;

            // Write OMF ids
            if export_omf_ids {
                serialize_into_csv(
                    &edge_list.omf_segment_ids,
                    OMF_SEGMENT_IDS_FILENAME,
                    &mode_dir,
                    overwrite,
                    "write omf ids",
                )?;
            }
        }
        eprintln!();

        kdam::term::show_cursor().map_err(|e| {
            OvertureMapsCollectionError::InternalError(format!("progress bar error: {e}"))
        })?;

        Ok(())
    }
}
