use std::{collections::HashMap, fs::File, path::Path};

use super::serialize_ops as ops;
use crate::{
    app::network::NetworkEdgeListConfiguration,
    collection::{
        OvertureMapsCollectionError, TransportationCollection, TransportationSegmentRecord,
    },
    graph::{segment_ops, vertex_serializable::VertexSerializable},
};
use csv::QuoteStyle;
use flate2::{write::GzEncoder, Compression};
use geo::LineString;
use kdam::tqdm;
use rayon::prelude::*;
use routee_compass_core::model::network::{EdgeConfig, EdgeList, EdgeListId, Vertex};
use wkt::ToWkt;

pub struct OmfGraphVectorized {
    pub vertices: Vec<Vertex>,
    pub edge_lists: Vec<OmfEdgeList>,
    pub edge_list_config: Vec<NetworkEdgeListConfiguration>,
    /// for each OMF ID, the vertex index
    pub vertex_lookup: HashMap<String, usize>,
}

pub struct OmfEdgeList {
    pub edges: EdgeList,
    pub geometries: Vec<LineString<f32>>,
}

impl OmfGraphVectorized {
    /// create a vectorized graph dataset from a [TransportationCollection]
    pub fn new(
        collection: &TransportationCollection,
        configuration: &[NetworkEdgeListConfiguration],
    ) -> Result<Self, OvertureMapsCollectionError> {
        // process all connectors into vertices
        let (mut vertices, mut vertex_lookup) =
            ops::create_vertices_and_lookup(&collection.connectors, None)?;

        // for each mode configuration, create an edge list
        let mut edge_lists: Vec<OmfEdgeList> = vec![];
        for (index, edge_list_config) in configuration.iter().enumerate() {
            let edge_list_id = EdgeListId(index);
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
            // which may not yet exist on the graph
            let splits = ops::find_splits(&segments, segment_ops::process_simple_connector_splits)?;

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
            let edge_list = OmfEdgeList {
                edges: EdgeList(edges.into_boxed_slice()),
                geometries: geometries,
            };
            edge_lists.push(edge_list);
        }

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
    ) -> Result<(), OvertureMapsCollectionError> {
        kdam::term::init(false);
        kdam::term::hide_cursor().map_err(|e| {
            OvertureMapsCollectionError::InternalError(format!("progress bar error: {e}"))
        })?;
        // create output directory if missing
        crate::util::fs::create_dirs(output_directory)?;

        // write vertices
        let mut vertex_writer = create_writer(
            output_directory,
            "vertices-compass.csv.gz",
            true,
            QuoteStyle::Necessary,
            overwrite,
        );
        let v_iter = tqdm!(
            self.vertices.iter(),
            total = self.vertices.len(),
            desc = "write vertex dataset"
        );
        for vertex in v_iter {
            if let Some(ref mut writer) = vertex_writer {
                let vertex_ser = VertexSerializable::from(*vertex);
                writer.serialize(vertex_ser).map_err(|e| {
                    OvertureMapsCollectionError::CsvWriteError(format!(
                        "Failed to write to vertices-compass.csv.gz: {e}"
                    ))
                })?;
            }
        }
        eprintln!();
        if let Some(ref mut writer) = vertex_writer {
            writer.flush().map_err(|e| {
                OvertureMapsCollectionError::CsvWriteError(format!(
                    "Failed to flush vertices-compass.csv.gz: {e}"
                ))
            })?;
        }

        // write each edge list
        let edge_list_iter = tqdm!(
            self.edge_lists.iter().zip(self.edge_list_config.iter()),
            desc = "edge list",
            total = self.edge_lists.len(),
            position = 0
        );
        for (edge_list, edge_list_config) in edge_list_iter {
            let mode_dir = output_directory.join(&edge_list_config.mode);
            crate::util::fs::create_dirs(&mode_dir)?;

            let mut edge_writer = create_writer(
                &mode_dir,
                "edges-compass.csv.gz",
                true,
                QuoteStyle::Necessary,
                overwrite,
            );
            let mut geometries_writer = create_writer(
                &mode_dir,
                "edges-geometries-enumerated.txt.gz",
                false,
                QuoteStyle::Never,
                overwrite,
            );

            // Write Edges
            let e_iter = tqdm!(
                edge_list.edges.0.iter(),
                total = edge_list.edges.len(),
                desc = "edges",
                position = 1
            );
            for row in e_iter {
                if let Some(ref mut writer) = edge_writer {
                    let edge = EdgeConfig {
                        edge_id: row.edge_id,
                        src_vertex_id: row.src_vertex_id,
                        dst_vertex_id: row.dst_vertex_id,
                        distance: row.distance.get::<uom::si::length::meter>(),
                    };
                    writer.serialize(edge).map_err(|e| {
                        OvertureMapsCollectionError::CsvWriteError(format!(
                            "Failed to write to edges-compass.csv.gz: {e}"
                        ))
                    })?;
                }
            }
            eprintln!();

            if let Some(ref mut writer) = edge_writer {
                writer.flush().map_err(|e| {
                    OvertureMapsCollectionError::CsvWriteError(format!(
                        "Failed to flush edges-compass.csv.gz: {e}"
                    ))
                })?;
            }

            // Write geometries
            let g_iter = tqdm!(
                edge_list.geometries.iter(),
                total = edge_list.geometries.len(),
                desc = "edges",
                position = 1
            );
            for row in g_iter {
                if let Some(ref mut writer) = geometries_writer {
                    writer
                    .serialize(row.to_wkt().to_string())
                    .map_err(|e| {
                        OvertureMapsCollectionError::CsvWriteError(format!(
                            "Failed to write to geometry file edges-geometries-enumerated.txt.gz: {}",
                            e
                        ))
                    })?;
                }
            }
            eprintln!();

            if let Some(ref mut writer) = geometries_writer {
                writer.flush().map_err(|e| {
                    OvertureMapsCollectionError::CsvWriteError(format!(
                        "Failed to flush edges-geometries-enumerated.txt.gz: {e}"
                    ))
                })?;
            }
        }
        eprintln!();

        kdam::term::show_cursor().map_err(|e| {
            OvertureMapsCollectionError::InternalError(format!("progress bar error: {e}"))
        })?;

        Ok(())
    }
}

/// helper function to build a filewriter for writing either .csv.gz or
/// .txt.gz files for compass datasets while respecting the user's overwrite
/// preferences and properly formatting WKT outputs.
fn create_writer(
    directory: &Path,
    filename: &str,
    has_headers: bool,
    quote_style: QuoteStyle,
    overwrite: bool,
) -> Option<csv::Writer<GzEncoder<File>>> {
    let filepath = directory.join(filename);
    if filepath.exists() && !overwrite {
        return None;
    }
    let file = File::create(filepath).unwrap();
    let buffer = GzEncoder::new(file, Compression::default());
    let writer = csv::WriterBuilder::new()
        .has_headers(has_headers)
        .quote_style(quote_style)
        .from_writer(buffer);
    Some(writer)
}
