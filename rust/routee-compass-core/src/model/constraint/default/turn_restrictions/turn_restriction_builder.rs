use super::{RestrictionRecord, TurnRestrictionFrontierService};
use crate::model::constraint::default::turn_restrictions::TurnRestrictionConstraintConfig;
use crate::{
    model::constraint::{ConstraintModelBuilder, ConstraintModelError, ConstraintModelService},
    util::fs::read_utils,
};
use kdam::Bar;
use std::{collections::HashSet, sync::Arc};

pub struct TurnRestrictionBuilder {}

impl ConstraintModelBuilder for TurnRestrictionBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, ConstraintModelError> {
        let config: TurnRestrictionConstraintConfig = serde_json::from_value(parameters.clone())
            .map_err(|e| {
                let msg = format!("failure reading turn restriction constraint model config: {e}");
                ConstraintModelError::BuildError(msg)
            })?;

        let restricted_edges: HashSet<RestrictionRecord> = read_utils::from_csv(
            &config.turn_restriction_input_file,
            true,
            Some(Bar::builder().desc("turn restrictions")),
            None,
        )
        .map_err(|e| {
            ConstraintModelError::BuildError(format!(
                "failure reading {}: {}",
                config.turn_restriction_input_file, e
            ))
        })?
        .iter()
        .cloned()
        .collect();

        log::debug!(
            "Loaded {} turn restrictions from {:?}.",
            restricted_edges.len(),
            config.turn_restriction_input_file
        );

        let m: Arc<dyn ConstraintModelService> = Arc::new(TurnRestrictionFrontierService {
            restricted_edge_pairs: Arc::new(restricted_edges),
        });
        Ok(m)
    }
}
