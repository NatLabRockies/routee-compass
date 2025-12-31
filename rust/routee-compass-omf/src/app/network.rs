use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    app::CliBoundingBox,
    collection::{
        filter::TravelModeFilter, ObjectStoreSource, OvertureMapsCollectionError,
        OvertureMapsCollectorConfig, ReleaseVersion, TransportationCollection,
    },
    graph::OmfGraphVectorized,
    util,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkEdgeListConfiguration {
    pub mode: String,
    pub filter: Vec<TravelModeFilter>,
}

/// runs an OMF network import using the provided configuration.
pub fn run(
    bbox: Option<&CliBoundingBox>,
    modes: &[NetworkEdgeListConfiguration],
    output_directory: &Path,
    local_source: Option<&Path>,
    write_json: bool,
) -> Result<(), OvertureMapsCollectionError> {
    let collection: TransportationCollection = match local_source {
        Some(src_path) => read_local(src_path),
        None => run_collector(bbox),
    }?;

    if write_json {
        util::fs::create_dirs(output_directory)?;
        collection.to_json(output_directory)?;
    }

    let vectorized_graph = OmfGraphVectorized::new(&collection, modes)?;
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

/// retrieve a TransportationCollection from a URL.
fn run_collector(
    bbox_arg: Option<&CliBoundingBox>,
) -> Result<TransportationCollection, OvertureMapsCollectionError> {
    let object_store = ObjectStoreSource::AmazonS3;
    let batch_size = 128;
    let collector = OvertureMapsCollectorConfig::new(object_store, batch_size).build()?;
    let release = ReleaseVersion::Latest;
    let bbox = bbox_arg.ok_or_else(|| {
        let msg = String::from("must provide bbox argument for download");
        OvertureMapsCollectionError::InvalidUserInput(msg)
    })?;
    log::info!(
        "running OMF import with
        object store {object_store:?}
        batch size {batch_size}
        release {release}
        (xmin,xmax,ymin,ymax): {bbox}"
    );

    TransportationCollection::try_from_collector(collector, release, Some(bbox.into()))
}
