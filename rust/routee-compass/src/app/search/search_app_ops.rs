use indexmap::IndexMap;
use itertools::Itertools;
use routee_compass_core::config::ConfigJsonExtensions;
use routee_compass_core::model::{
    state::{StateModelError, StateVariableConfig},
    traversal::TraversalModel,
};
use std::sync::Arc;

/// collects the state features to use in this search. the features are collected in
/// the following order:
///   1. from the traversal model
///   2. from the access model
///   3. optionally from the query itself
///
/// using the order above, each new source optionally overwrites any existing feature
/// by name (tuple index 0) as long as they match in StateFeature::get_feature_name and
/// StateFeature::get_feature_unit_name.
///
/// features are returned in the order in which the traversal models declare them, so that
/// the resulting state model, and any output derived from it, is deterministic.
pub fn collect_features(
    query: &serde_json::Value,
    traversal_models: &[Arc<dyn TraversalModel>],
) -> Result<Vec<(String, StateVariableConfig)>, StateModelError> {
    // prepare the set of features for this state model
    let model_features = traversal_models
        .iter()
        .flat_map(|m| m.output_features().into_iter())
        .collect::<IndexMap<_, _>>();

    // build the state model. inject state features from the traversal and access models
    // and then allow the user to optionally override any initial conditions for those
    // state features.
    let user_features_option: Option<IndexMap<String, StateVariableConfig>> = query
        .get_config_serde_optional(&"state_features", &"query")
        .map_err(|e| StateModelError::BuildError(e.to_string()))?;
    let user_features = user_features_option
        .unwrap_or_default()
        .into_iter()
        .map(|(name, feature)| match model_features.get(&name) {
            None => {
                let fnames = model_features.keys().join(",");
                Err(StateModelError::UnknownStateVariableName(name, fnames))
            }
            Some(existing) if existing.get_feature_type() != feature.get_feature_type() => {
                Err(StateModelError::UnexpectedFeatureType(
                    existing.get_feature_type(),
                    feature.get_feature_type(),
                ))
            }
            Some(_) => Ok((name, feature)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut added_features: Vec<(String, StateVariableConfig)> =
        model_features.into_iter().collect_vec();
    added_features.extend(user_features);
    Ok(added_features)
}

#[cfg(test)]
mod test {
    use super::collect_features;
    use itertools::Itertools;
    use routee_compass_core::algorithm::search::SearchTree;
    use routee_compass_core::model::network::Vertex;
    use routee_compass_core::model::state::{
        InputFeature, StateModel, StateVariable, StateVariableConfig,
    };
    use routee_compass_core::model::traversal::{
        EdgeFrontierContext, TraversalModel, TraversalModelError,
    };
    use serde_json::json;
    use std::sync::Arc;
    use uom::{si::f64::Length, ConstZero};

    struct MockModel {
        out_features: Vec<String>,
    }

    impl MockModel {
        fn new_arc(out_features: &[&str]) -> Arc<dyn TraversalModel> {
            Arc::new(MockModel {
                out_features: out_features.iter().map(|n| n.to_string()).collect_vec(),
            })
        }
    }

    impl TraversalModel for MockModel {
        fn name(&self) -> String {
            format!("Mock Traversal Model: {}", self.out_features.join("+"))
        }

        fn input_features(&self) -> Vec<InputFeature> {
            vec![]
        }

        fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
            self.out_features
                .iter()
                .map(|n| {
                    (
                        n.clone(),
                        StateVariableConfig::Distance {
                            initial: Length::ZERO,
                            accumulator: true,
                            output_unit: None,
                        },
                    )
                })
                .collect_vec()
        }

        fn traverse_edge(
            &self,
            _ctx: &EdgeFrontierContext,
            _state: &mut Vec<StateVariable>,
            _state_model: &StateModel,
        ) -> Result<(), TraversalModelError> {
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

    #[test]
    fn test_collect_features_preserves_model_order() {
        let first = [
            "trip_time",
            "edge_speed",
            "trip_distance",
            "edge_time",
            "trip_energy",
        ];
        let second = ["edge_distance", "trip_soc", "edge_grade", "zeta", "alpha"];
        let models = vec![MockModel::new_arc(&first), MockModel::new_arc(&second)];
        let query = json!({});

        let features = collect_features(&query, &models).expect("should collect features");
        let names = features.iter().map(|(n, _)| n.as_str()).collect_vec();
        let expected = first.iter().chain(second.iter()).copied().collect_vec();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_collect_features_duplicate_keeps_first_position() {
        let models = vec![
            MockModel::new_arc(&["trip_distance", "trip_time"]),
            MockModel::new_arc(&["edge_speed", "trip_distance"]),
        ];
        let query = json!({});

        let features = collect_features(&query, &models).expect("should collect features");
        let names = features.iter().map(|(n, _)| n.as_str()).collect_vec();
        assert_eq!(names, vec!["trip_distance", "trip_time", "edge_speed"]);
    }
}
