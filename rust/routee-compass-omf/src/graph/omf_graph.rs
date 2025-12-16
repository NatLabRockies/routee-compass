use std::{collections::HashMap, fs::File, path::Path};

use csv::QuoteStyle;
use flate2::{write::GzEncoder, Compression};
use kdam::tqdm;
use routee_compass_core::model::network::{Edge, EdgeConfig, EdgeId, Vertex};

use super::serialize_ops::get_connectors_mapping;
use crate::{
    collection::{OvertureMapsCollectionError, TransportationCollection},
    graph::{serialize_ops::get_connector_splits, vertex_serializable::VertexSerializable},
};

#[derive(Debug)]
pub struct OmfGraphVectorized {
    pub edge_list_id: usize,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    /// for each OMF ID, the vertex index
    pub vertex_lookup: HashMap<String, usize>,
}

impl OmfGraphVectorized {
    pub fn try_from_collection(
        collection: TransportationCollection,
        edge_list_id: usize,
    ) -> Result<Self, OvertureMapsCollectionError> {
        // Process initial set of connectors
        let (vertices, vertex_mapping) = get_connectors_mapping(&collection.connectors)?;

        // Initialize result
        let mut result = Self {
            edge_list_id,
            vertices,
            edges: vec![],
            vertex_lookup: vertex_mapping,
        };

        // Process segments
        for segment in collection.segments.iter() {
            // Compute all the splits
            // Here is where we would define and run additional splits
            get_connector_splits(segment)?
                .iter()
                .try_for_each(|split| split.split(&mut result, segment))?;
        }

        Ok(result)
    }

    pub fn add_vertex(&mut self, x: f32, y: f32) -> usize {
        let current_vertex_id = self.vertices.len();
        // Should we just use UUID?
        let new_name = format!("new-{current_vertex_id}");
        self.vertex_lookup.insert(new_name, current_vertex_id);
        self.vertices
            .push(Vertex::new(current_vertex_id, x, y));

        current_vertex_id
    }

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
            self.vertices.iter().enumerate(),
            total = self.vertices.len(),
            desc = "write vertex dataset"
        );
        for (_, vertex) in v_iter {
            if let Some(ref mut writer) = vertex_writer {
                let vertex_ser = VertexSerializable::from(*vertex);
                writer.serialize(vertex_ser).map_err(|e| {
                    OvertureMapsCollectionError::CsvWriteError(format!(
                        "Failed to write to vertices-compass.csv.gz: {e}"
                    ))
                })?;
            }
        }

        // Write Edges
        let e_iter = tqdm!(
            self.edges.iter().enumerate(),
            total = self.edges.len(),
            desc = "write edges dataset"
        );
        for (edge_id, row) in e_iter {
            if let Some(ref mut writer) = edge_writer {
                let edge = EdgeConfig {
                    edge_id: EdgeId(edge_id),
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
