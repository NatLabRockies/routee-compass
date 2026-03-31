use crate::{
    algorithm::search::SearchGraph,
    model::{
        network::{Edge, Vertex},
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{TraversalModel, TraversalModelError},
    },
};
use std::sync::Arc;

pub struct CombinedTraversalModel {
    models: Vec<Arc<dyn TraversalModel>>,
}

impl CombinedTraversalModel {
    /// combines a collection of traversal models into one combined model.
    /// it is assumed that these are provided in the correct running order,
    /// which can be set by combined_ops::topological_dependency_sort.
    pub fn new(models: Vec<Arc<dyn TraversalModel>>) -> Self {
        CombinedTraversalModel { models }
    }
}

impl TraversalModel for CombinedTraversalModel {
    fn name(&self) -> String {
        format!(
            "Combined Traversal Model: {}",
            self.models
                .iter()
                .map(|m| m.name())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
    fn input_features(&self) -> Vec<InputFeature> {
        self.models
            .iter()
            .flat_map(|m| m.input_features())
            .collect()
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        self.models
            .iter()
            .flat_map(|m| m.output_features())
            .collect()
    }

    fn traverse_edge(
        &self,
        trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        graph: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        for model in self.models.iter() {
            model.traverse_edge(trajectory, state, graph, state_model)?;
        }
        Ok(())
    }

    fn estimate_traversal(
        &self,
        od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        graph: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        for model in self.models.iter() {
            model.estimate_traversal(od, state, graph, state_model)?;
        }
        Ok(())
    }
}
