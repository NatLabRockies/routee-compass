use super::{
    EdgeHeading, TurnDelayTraversalConfig, TurnDelayTraversalModelEngine,
    TurnDelayTraversalModelService,
};
use crate::{
    model::traversal::{TraversalModelBuilder, TraversalModelError, TraversalModelService},
    util::fs::read_utils,
};
use kdam::Bar;
use std::path::PathBuf;
use std::sync::Arc;

pub struct TurnDelayTraversalModelBuilder {}

impl TraversalModelBuilder for TurnDelayTraversalModelBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {
        let config: TurnDelayTraversalConfig =
            serde_json::from_value(parameters.clone()).map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "failure reading turn delay traversal configuration: {e}"
                ))
            })?;

        let file_path = PathBuf::from(&config.edge_heading_input_file);
        let edge_headings = read_utils::from_csv::<EdgeHeading>(
            &file_path.as_path(),
            true,
            Some(Bar::builder().desc("edge headings")),
            None,
        )
        .map_err(|e| {
            TraversalModelError::BuildError(format!(
                "error reading headings from file {file_path:?}: {e}"
            ))
        })?;

        let engine = TurnDelayTraversalModelEngine {
            edge_headings,
            turn_delay_model: config.turn_delay_model.into(),
        };
        let service = TurnDelayTraversalModelService {
            engine: Arc::new(engine),
            include_trip_time: config.include_trip_time.unwrap_or(true),
        };
        Ok(Arc::new(service))
    }
}
