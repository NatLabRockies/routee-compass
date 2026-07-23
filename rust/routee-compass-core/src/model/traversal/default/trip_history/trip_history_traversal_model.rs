use std::sync::Arc;

use super::TripHistoryTraversalEngine;

use crate::{
    algorithm::search::SearchTree,
    model::{
        network::Vertex,
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{EdgeFrontierContext, TraversalModel, TraversalModelError},
    },
};

pub struct TripHistoryTraversalModel {
    pub engine: Arc<TripHistoryTraversalEngine>,
}

impl TripHistoryTraversalModel {
    pub fn new(engine: Arc<TripHistoryTraversalEngine>) -> Self {
        // modify this and the struct definition if additional pre-processing
        // is required during model instantiation from query parameters.
        Self { engine }
    }
}

impl TraversalModel for TripHistoryTraversalModel {
    fn name(&self) -> String {
        "TripHistoryModel".to_string()
    }

    fn input_features(&self) -> Vec<InputFeature> {
        // the set of input features from the trip history configuration
        self.engine
            .input_state_variable_configs
            .iter()
            .map(InputFeature::from)
            .collect()
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        // below, f is "feature", d is "depth", m is number of features, n is max depth
        // output is: ["f1_d1", "f2_d1", ... "fm_d1", "f1_d2", "f2_d2", ..., "f1_dn",..."fm_dn"]
        (1..=self.engine.depth.get()) // depth_n
            .flat_map(|depth| {
                self.engine
                    .input_state_variable_configs
                    .iter() // feature_m
                    .map(move |cfg| (format!("{}_{depth}", cfg.get_feature_type()), cfg.clone()))
            })
            .collect()
    }

    fn traverse_edge(
        &self,
        _ctx: &EdgeFrontierContext,
        _state: &mut Vec<StateVariable>,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        self.engine.update_history(_ctx, _state, _state_model)?;
        Ok(())
    }

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
