use std::{collections::HashMap, fs::File, path::Path};

use csv::QuoteStyle;
use flate2::{write::GzEncoder, Compression};
use kdam::tqdm;
use routee_compass_core::model::network::{Edge, EdgeConfig, EdgeListId, Vertex};

use super::serialize_ops as ops;
use crate::{
    collection::{OvertureMapsCollectionError, TransportationCollection},
    graph::{segment_ops, vertex_serializable::VertexSerializable},
};

#[derive(Debug)]
pub struct OmfGraphVectorized {
    pub edge_list_id: EdgeListId,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    /// for each OMF ID, the vertex index
    pub vertex_lookup: HashMap<String, usize>,
}

impl OmfGraphVectorized {
    /// create a vectorized graph dataset from a [TransportationCollection]
    pub fn new(
        collection: TransportationCollection,
        edge_list_id: EdgeListId,
    ) -> Result<Self, OvertureMapsCollectionError> {
        // Process initial set of connectors
        let (mut vertices, mut vertex_lookup) =
            ops::create_vertices_and_lookup(&collection.connectors)?;
        let segment_lookup = ops::create_segment_lookup(&collection.segments);

        // the splits are locations in each segment record where we want to define a vertex
        // which may not yet exist on the graph
        let splits = ops::find_splits(
            &collection.segments,
            segment_ops::process_simple_connector_splits,
        )?;

        // depending on the split method, we may need to create additional vertices at locations
        // which are not OvertureMaps-defined connector types.
        ops::extend_vertices(
            &splits,
            &collection.segments,
            &segment_lookup,
            &mut vertices,
            &mut vertex_lookup,
        )?;

        // create all edges based on the above split points using all vertices.
        let edges = ops::create_edges(
            &collection.segments,
            &segment_lookup,
            &splits,
            &vertices,
            &vertex_lookup,
            edge_list_id,
        )?;

        let result = Self {
            edge_list_id,
            vertices,
            edges,
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
        let mut vertex_writer = create_writer(
            output_directory,
            "vertices-compass.csv.gz",
            true,
            QuoteStyle::Necessary,
            overwrite,
        );
        let mut edge_writer = create_writer(
            output_directory,
            "edges-compass.csv.gz",
            true,
            QuoteStyle::Necessary,
            overwrite,
        );

        // Write vertices
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

        // Write Edges
        let e_iter = tqdm!(
            self.edges.iter(),
            total = self.edges.len(),
            desc = "write edges dataset"
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

        // Explicitly flush the writers to ensure all data is written
        if let Some(ref mut writer) = vertex_writer {
            writer.flush().map_err(|e| {
                OvertureMapsCollectionError::CsvWriteError(format!(
                    "Failed to flush vertices-compass.csv.gz: {e}"
                ))
            })?;
        }
        if let Some(ref mut writer) = edge_writer {
            writer.flush().map_err(|e| {
                OvertureMapsCollectionError::CsvWriteError(format!(
                    "Failed to flush edges-compass.csv.gz: {e}"
                ))
            })?;
        }
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
