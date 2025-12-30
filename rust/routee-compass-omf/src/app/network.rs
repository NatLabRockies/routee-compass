use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    collection::{
        filter::TravelModeFilter, ObjectStoreSource, OvertureMapsCollectionError,
        OvertureMapsCollectorConfig, ReleaseVersion, RowFilterConfig, TransportationCollection,
    },
    graph::OmfGraphVectorized,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkEdgeListConfiguration {
    pub mode: String,
    pub filter: Vec<TravelModeFilter>,
}

/// runs an OMF network import using the provided configuration.
pub fn run(
    configuration: &[NetworkEdgeListConfiguration],
    output_directory: &Path,
    local_source: Option<&Path>,
    write_json: bool,
) -> Result<(), OvertureMapsCollectionError> {
    let collection: TransportationCollection = match local_source {
        Some(src_path) => read_local(src_path),
        None => {
            let collector =
                OvertureMapsCollectorConfig::new(ObjectStoreSource::AmazonS3, 128).build()?;
            let release = ReleaseVersion::Latest;
            let row_filter_config = RowFilterConfig::Bbox {
                xmin: -105.254,
                xmax: -105.197,
                ymin: 39.733,
                ymax: 39.784,
            };

            TransportationCollection::try_from_collector(
                collector,
                release,
                Some(row_filter_config),
            )
        }
    }?;

    if write_json {
        collection.to_json(output_directory)?;
    }

    let vectorized_graph = OmfGraphVectorized::new(&collection, configuration)?;
    vectorized_graph.write_compass(output_directory, true)?;

    Ok(())
}

fn read_local(path: &Path) -> Result<TransportationCollection, OvertureMapsCollectionError> {
    let contents = std::fs::read(path).map_err(|e| OvertureMapsCollectionError::ReadError {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    let collection =
        serde_json::from_slice::<TransportationCollection>(&contents).map_err(|e| {
            OvertureMapsCollectionError::ReadError {
                path: path.to_owned(),
                message: format!("failed to deserialize from JSON: {e}"),
            }
        })?;
    Ok(collection)
}
