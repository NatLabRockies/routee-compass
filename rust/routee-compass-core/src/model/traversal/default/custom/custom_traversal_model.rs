use super::CustomTraversalEngine;
use crate::algorithm::search::SearchTree;
use crate::model::network::Vertex;
use crate::model::state::StateModel;
use crate::model::state::StateVariable;
use crate::model::state::{InputFeature, StateVariableConfig};
use crate::model::traversal::traversal_model::TraversalModel;
use crate::model::traversal::traversal_model_error::TraversalModelError;
use std::sync::Arc;

/// looks up values to assign to a traversal based on the edge id for some
/// custom value type stored in a file.
pub struct CustomTraversalModel {
    engine: Arc<CustomTraversalEngine>,
}

impl CustomTraversalModel {
    pub fn new(engine: Arc<CustomTraversalEngine>) -> CustomTraversalModel {
        Self { engine }
    }
}

impl TraversalModel for CustomTraversalModel {
    fn name(&self) -> String {
        format!(
            "Custom Traversal Model: {}",
            self.engine.config().custom_type
        )
    }
    fn input_features(&self) -> Vec<InputFeature> {
        vec![]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        let feature = self.engine.output_feature();
        let name = self.engine.config().custom_type.clone();
        vec![(name, feature)]
    }

    /// records the value that will be assigned to this edge into the state vector.
    fn traverse_edge(
        &self,
        ctx: &crate::model::traversal::EdgeTraversalContext,
        state: &mut Vec<StateVariable>,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        self.engine
            .insert_value(&ctx.edge.edge_id, state, state_model)
    }

    /// records the value that will be assigned to this edge into the state vector.
    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        _state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        Ok(())
    }
}
