use uom::{si::f64::Ratio, ConstZero};

use super::GradeTraversalEngine;
use crate::{
    algorithm::search::SearchGraph,
    model::{
        network::{Edge, Vertex},
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{default::fieldname, TraversalModel, TraversalModelError},
        unit::RatioUnit,
    },
};
use std::sync::Arc;

pub struct GradeTraversalModel {
    pub engine: Arc<GradeTraversalEngine>,
}

impl GradeTraversalModel {
    pub fn new(engine: Arc<GradeTraversalEngine>) -> GradeTraversalModel {
        GradeTraversalModel { engine }
    }
}

impl TraversalModel for GradeTraversalModel {
    fn name(&self) -> String {
        String::from("Grade Traversal Model")
    }
    /// no upstream state dependencies
    fn input_features(&self) -> Vec<InputFeature> {
        vec![]
    }

    //
    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        vec![(
            String::from(fieldname::EDGE_GRADE),
            StateVariableConfig::Ratio {
                initial: Ratio::ZERO,
                accumulator: false,
                output_unit: Some(RatioUnit::default()),
            },
        )]
    }

    fn traverse_edge(
        &self,
        trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let (_, edge, _) = trajectory;
        let grade = self.engine.get_grade(edge.edge_id)?;
        state_model.set_ratio(state, fieldname::EDGE_GRADE, &grade)?;
        Ok(())
    }

    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        _state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        // would be nice if we could use vertex elevation to estimate overall grade change..
        Ok(())
    }
}
