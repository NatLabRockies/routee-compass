use uom::ConstZero;

use crate::algorithm::search::SearchTree;
use crate::model::network::Vertex;
use crate::model::state::{CustomVariableConfig, InputFeature, StateModel, StateVariable};
use crate::model::traversal::{EdgeTraversalContext, TraversalModel};
use crate::model::{
    state::StateVariableConfig,
    traversal::{default::combined::CombinedTraversalModel, TraversalModelError},
};
use std::sync::Arc;

/// a traversal model that can be used in tests which will "plug in" a mock model that
/// registers all of the input feature requirements for the provided model.
pub struct TestTraversalModel {}

impl TestTraversalModel {
    /// build (wrap) the model for testing.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        model: Arc<dyn TraversalModel>,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let upstream: Box<dyn TraversalModel> =
            Box::new(MockUpstreamModel::new_upstream_from(model.clone()));
        let wrapped: Arc<dyn TraversalModel> = Arc::new(CombinedTraversalModel::new(vec![
            Arc::from(upstream),
            model.clone(),
        ]));
        Ok(wrapped)
    }
}

struct MockUpstreamModel {
    input_features: Vec<InputFeature>,
    output_features: Vec<(String, StateVariableConfig)>,
}

impl MockUpstreamModel {
    /// builds a new mock upstream TraversalModel that registers all of the input feature
    /// requirements for the provided model.
    pub fn new_upstream_from(model: Arc<dyn TraversalModel>) -> MockUpstreamModel {
        let input_features = vec![];
        let output_features = model
            .input_features()
            .iter()
            .map(|feature| match feature {
                InputFeature::Distance { name, unit: _ } => (
                    name.clone(),
                    StateVariableConfig::Distance {
                        initial: uom::si::f64::Length::ZERO,
                        accumulator: false,
                        output_unit: None,
                    },
                ),
                InputFeature::Ratio { name, unit: _ } => (
                    name.clone(),
                    StateVariableConfig::Ratio {
                        initial: uom::si::f64::Ratio::ZERO,
                        accumulator: false,
                        output_unit: None,
                    },
                ),
                InputFeature::Speed { name, unit: _ } => (
                    name.clone(),
                    StateVariableConfig::Speed {
                        initial: uom::si::f64::Velocity::ZERO,
                        accumulator: false,
                        output_unit: None,
                    },
                ),
                InputFeature::Time { name, unit: _ } => (
                    name.clone(),
                    StateVariableConfig::Time {
                        initial: uom::si::f64::Time::ZERO,
                        accumulator: false,
                        output_unit: None,
                    },
                ),
                InputFeature::Energy { name, unit: _ } => (
                    name.clone(),
                    StateVariableConfig::Energy {
                        initial: uom::si::f64::Energy::ZERO,
                        accumulator: false,
                        output_unit: None,
                    },
                ),
                InputFeature::Temperature { name, unit: _ } => (
                    name.clone(),
                    StateVariableConfig::Temperature {
                        initial: uom::si::f64::ThermodynamicTemperature::ZERO,
                        accumulator: false,
                        output_unit: None,
                    },
                ),
                InputFeature::Custom { name, unit } => {
                    // only current way to hook in custom unit type
                    use CustomVariableConfig as C;
                    let var_config = match unit.as_str() {
                        "floating_point" => C::FloatingPoint {
                            initial: ordered_float::OrderedFloat(0.0),
                        },
                        "signed_integer" => C::SignedInteger { initial: 0 },
                        "unsigned_integer" => C::UnsignedInteger { initial: 0 },
                        "boolean" => C::Boolean { initial: false },
                        _ => C::FloatingPoint {
                            initial: ordered_float::OrderedFloat(0.0),
                        },
                    };
                    (
                        name.clone(),
                        StateVariableConfig::Custom {
                            custom_type: name.clone(),
                            accumulator: false,
                            value: var_config,
                        },
                    )
                }
            })
            .collect();
        Self {
            input_features,
            output_features,
        }
    }
}

impl TraversalModel for MockUpstreamModel {
    fn name(&self) -> String {
        String::from("Mock Upstream Traversal Model")
    }
    fn input_features(&self) -> Vec<InputFeature> {
        self.input_features.clone()
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        self.output_features.clone()
    }

    fn traverse_edge(
        &self,
        _ctx: &EdgeTraversalContext,
        _state: &mut Vec<StateVariable>,
        _state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        Ok(())
    }

    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        _state: &mut Vec<crate::model::state::StateVariable>,
        _tree: &SearchTree,
        _state_model: &crate::model::state::StateModel,
    ) -> Result<(), TraversalModelError> {
        Ok(())
    }
}
