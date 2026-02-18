use std::path::Path;

use routee_compass_core::model::unit::DistanceUnit;
use serde::{Deserialize, Serialize};

use crate::{
    app::CliBoundingBox,
    collection::{
        filter::TravelModeFilter, ObjectStoreSource, OvertureMapsCollectionError,
        OvertureMapsCollectorConfig, ReleaseVersion, SegmentAccessRestrictionWhen,
        TransportationCollection,
    },
    graph::OmfGraphVectorized,
    util,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NetworkEdgeListConfiguration {
    pub mode: String,
    pub filter: Vec<TravelModeFilter>,
    pub island_algorithm_config: Option<IslandDetectionAlgorithmConfiguration>,
}

impl From<&NetworkEdgeListConfiguration> for SegmentAccessRestrictionWhen {
    fn from(value: &NetworkEdgeListConfiguration) -> Self {
        let user_modes_opt = value.filter.iter().find_map(|f| match f {
            TravelModeFilter::MatchesModeAccess { modes } => Some(modes.clone()),
            _ => None,
        });
        let mut result = SegmentAccessRestrictionWhen::default();
        if let Some(modes) = user_modes_opt {
            result.mode = Some(modes);
        }
        result
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IslandDetectionAlgorithmConfiguration {
    pub min_distance: f64,
    pub distance_unit: DistanceUnit,
    pub parallel_execution: bool,
}

/// runs an OMF network import using the provided configuration.
pub fn run(
    bbox: Option<&CliBoundingBox>,
    modes: &[NetworkEdgeListConfiguration],
    output_directory: &Path,
    local_source: Option<&Path>,
    write_json: bool,
    island_detection_configuration: Option<IslandDetectionAlgorithmConfiguration>,
    export_omf_ids: bool,
) -> Result<(), OvertureMapsCollectionError> {
    let collection: TransportationCollection = match local_source {
        Some(src_path) => read_local(src_path),
        None => run_collector(bbox),
    }?;

    if write_json {
        util::fs::create_dirs(output_directory)?;
        collection.to_json(output_directory)?;
    }

    let vectorized_graph =
        OmfGraphVectorized::new(&collection, modes, island_detection_configuration)?;
    vectorized_graph.write_compass(output_directory, true, export_omf_ids)?;

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
    let rg_chunk_size = 4;
    let file_concurrency_limit = 64;
    let collector = OvertureMapsCollectorConfig::new(
        object_store,
        Some(rg_chunk_size),
        Some(file_concurrency_limit),
    )
    .build()?;
    let release = ReleaseVersion::Latest;
    let bbox = bbox_arg.ok_or_else(|| {
        let msg = String::from("must provide bbox argument for download");
        OvertureMapsCollectionError::InvalidUserInput(msg)
    })?;
    log::info!(
        "running OMF import with
        object store {object_store:?}
        rg_chunk_size {rg_chunk_size}
        file_concurrency_limit {file_concurrency_limit}
        release {release}
        (xmin,xmax,ymin,ymax): {bbox}"
    );

    TransportationCollection::try_from_collector(collector, release, Some(bbox.into()))
}
