use std::sync::Arc;

use super::{TripHistoryEngine, TripHistoryParams};

use crate::{
    algorithm::search::SearchTree,
    model::{
        network::Vertex,
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{EdgeFrontierContext, TraversalModel, TraversalModelError},
    },
};

pub struct TripHistoryModel {
    pub engine: Arc<TripHistoryEngine>,
    pub params: TripHistoryParams,
}

impl TripHistoryModel {
    pub fn new(engine: Arc<TripHistoryEngine>, params: TripHistoryParams) -> Self {
        // modify this and the struct definition if additional pre-processing
        // is required during model instantiation from query parameters.
        Self { engine, params }
    }
}

impl TraversalModel for TripHistoryModel {
    fn name(&self) -> String {
        "TripHistoryModel".to_string()
    }

    fn input_features(&self) -> Vec<InputFeature> {
        todo!()
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        todo!()
    }

    fn traverse_edge(
        &self,
        _ctx: &EdgeFrontierContext,
        _state: &mut Vec<StateVariable>,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        todo!()
    }

    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        _state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        todo!()
    }
}
