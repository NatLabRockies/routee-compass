use crate::model::traversal::{TraversalModel, TraversalModelError};
use itertools::Itertools;
use std::sync::Arc;
use topological_sort;

/// sorts the traversal models such that the inter-model dependencies are sorted.
/// in other words: a time model depends on a distance calculation <-> the distance model
/// must appear earlier in the list than the time model.
///
/// only confirms that the names match, ignores confirming the feature types match.
///
/// # Arguments
///
/// * `models` - the traversal models to sort
///
/// # Returns
///
/// Sorted list of models, or an error if dependencies are missing
pub fn topological_dependency_sort(
    models: &[Arc<dyn TraversalModel>],
) -> Result<Vec<Arc<dyn TraversalModel>>, TraversalModelError> {
    let output_features_lookup = models
        .iter()
        .enumerate()
        .flat_map(|(idx, m)| m.output_features().into_iter().map(move |(n, _)| (n, idx)))
        .into_group_map();

    // find relationships between model input and output features
    let mut missing_parents: Vec<String> = vec![];
    let mut sort = topological_sort::TopologicalSort::<usize>::new();
    for (idx, m) in models.iter().enumerate() {
        let input_features = m.input_features();
        if input_features.is_empty() {
            sort.insert(idx);
        } else {
            for feature in input_features.iter() {
                match &output_features_lookup.get(&feature.name()) {
                    None => {
                        missing_parents.push(feature.name());
                    }
                    Some(ref_idxs) => {
                        // look at all dependencies but ignore self-looping dependencies
                        let refs_iter = ref_idxs.iter().filter(|i| &idx != *i);
                        for ref_idx in refs_iter {
                            sort.add_dependency(idx, *ref_idx);
                        }
                    }
                }
            }
        }
    }

    if !missing_parents.is_empty() {
        let joined = missing_parents.iter().join(",");
        let msg = format!(
            "the following state variables are required by traversal models but missing: {{{joined}}}"
        );
        return Err(TraversalModelError::BuildError(msg));
    }

    // apply topological sort to the models.
    // the correct order we want is the opposite of the popped result
    let mut result = vec![];
    while let Some(m_idx) = sort.pop() {
        let model = models.get(m_idx).ok_or_else(|| {
            TraversalModelError::BuildError(format!("internal error: sort has model index {m_idx} which is not found in the model collection"))
        })?;
        result.push(model.clone());
    }
    result.reverse();

    log::debug!(
        "topological sort of traversal models: {}",
        result.iter().map(|m| m.name()).join(", ")
    );

    // topological_sort crate's pop() method stops when collection is empty, unless there is a cyclical
    // dependency, in which case the collection will return None and remain non-empty.
    if !sort.is_empty() {
        let msg = format!("cyclical dependency in traversal model features: {sort:?}");
        return Err(TraversalModelError::BuildError(msg));
    }

    Ok(result)
}

#[cfg(test)]
mod test {

    use super::topological_dependency_sort;
    use crate::{
        algorithm::search::SearchTree,
        model::{
            network::{Vertex},
            state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
            traversal::{TraversalModel, TraversalModelError},
        },
    };
    use itertools::Itertools;
    use std::sync::Arc;
    use uom::{si::f64::Length, ConstZero};

    /// tests dependency sort on the typical setup for modeling distance, speed,
    /// time, grade, elevation, and energy.
    #[test]
    fn test_default_model_setup() {
        init_test_logger();

        let distance_feature = InputFeature::Distance {
            name: "distance".to_string(),
            unit: None,
        };
        let speed_feature = InputFeature::Speed {
            name: "speed".to_string(),
            unit: None,
        };
        let grade_feature = InputFeature::Ratio {
            name: "grade".to_string(),
            unit: None,
        };

        // mock up models for 6 dimensions, 3 of which have upstream dependencies
        let distance = MockModel::new(vec![], vec!["distance"]);
        let speed = MockModel::new(vec![], vec!["speed"]);
        let time = MockModel::new(
            vec![distance_feature.clone(), speed_feature.clone()],
            vec!["time"],
        );
        let grade = MockModel::new(vec![], vec!["grade"]);
        let elevation = MockModel::new(vec![grade_feature.clone()], vec!["elevation"]);
        let energy = MockModel::new(
            vec![distance_feature, speed_feature, grade_feature],
            vec!["energy"],
        );
        let models: Vec<Arc<dyn TraversalModel>> =
            vec![energy, elevation, time, grade, speed, distance]
                .into_iter()
                .map(|m| {
                    let am: Arc<dyn TraversalModel> = Arc::new(m);
                    am
                })
                .collect_vec();

        // apply sort and then reconstruct descriptions for each model on the sorted values
        let sorted = topological_dependency_sort(&models).expect("failure during sort function");
        let sorted_descriptions = sorted
            .iter()
            .map(|m| {
                let input_features = m.input_features();
                let in_names = if input_features.is_empty() {
                    String::from("*")
                } else {
                    m.input_features()
                        .iter()
                        .map(|feature| feature.name())
                        .join("+")
                };
                let out_name = m.output_features().iter().map(|(n, _)| n).join("");
                format!("{in_names}->{out_name}")
            })
            .collect_vec();

        // validate that the dependencies are honored. the algorithm makes no guarantees about
        // tiebreaking usize values so we cannot expect a deterministic output here.
        let mut distance_seen = false;
        let mut speed_seen = false;
        let mut grade_seen = false;
        for description in sorted_descriptions.iter() {
            match description.as_str() {
                "*->grade" => grade_seen = true,
                "*->speed" => speed_seen = true,
                "*->distance" => distance_seen = true,
                "distance+speed+grade->energy" => {
                    assert!(distance_seen && speed_seen && grade_seen)
                }
                "distance+speed->time" => assert!(distance_seen && speed_seen),
                "grade->elevation" => assert!(grade_seen),
                other => panic!("unexpected key: '{other}'"),
            }
        }
    }

    /// tests dependency sort where the energy model depends on and generates SOC values
    #[test]
    fn test_self_dependency() {
        init_test_logger();

        let distance_feature = InputFeature::Distance {
            name: "distance".to_string(),
            unit: None,
        };
        let speed_feature = InputFeature::Speed {
            name: "speed".to_string(),
            unit: None,
        };
        let grade_feature = InputFeature::Ratio {
            name: "grade".to_string(),
            unit: None,
        };
        let soc_feature = InputFeature::Ratio {
            name: "soc".to_string(),
            unit: None,
        };

        // mock up models for 6 dimensions, 3 of which have upstream dependencies
        let distance = MockModel::new(vec![], vec!["distance"]);
        let speed = MockModel::new(vec![], vec!["speed"]);
        let time = MockModel::new(
            vec![distance_feature.clone(), speed_feature.clone()],
            vec!["time"],
        );
        let grade = MockModel::new(vec![], vec!["grade"]);
        let elevation = MockModel::new(vec![grade_feature.clone()], vec!["elevation"]);
        let energy = MockModel::new(
            vec![distance_feature, speed_feature, grade_feature, soc_feature],
            vec!["energy", "soc"],
        );
        let models: Vec<Arc<dyn TraversalModel>> =
            vec![energy, elevation, time, grade, speed, distance]
                .into_iter()
                .map(|m| {
                    let am: Arc<dyn TraversalModel> = Arc::new(m);
                    am
                })
                .collect_vec();

        // apply sort and then reconstruct descriptions for each model on the sorted values
        let sorted = topological_dependency_sort(&models).expect("failure during sort function");
        let sorted_descriptions = sorted
            .iter()
            .map(|m| {
                let input_features = m.input_features();
                let in_names = if input_features.is_empty() {
                    String::from("*")
                } else {
                    m.input_features()
                        .iter()
                        .map(|feature| feature.name())
                        .join("+")
                };
                let out_name = m.output_features().iter().map(|(n, _)| n).join("+");
                format!("{in_names}->{out_name}")
            })
            .collect_vec();

        // validate that the dependencies are honored. the algorithm makes no guarantees about
        // tiebreaking usize values so we cannot expect a deterministic output here.
        let mut distance_seen = false;
        let mut speed_seen = false;
        let mut grade_seen = false;
        for description in sorted_descriptions.iter() {
            match description.as_str() {
                "*->grade" => grade_seen = true,
                "*->speed" => speed_seen = true,
                "*->distance" => distance_seen = true,
                "distance+speed+grade+soc->energy+soc" => {
                    assert!(distance_seen && speed_seen && grade_seen)
                }
                "distance+speed->time" => assert!(distance_seen && speed_seen),
                "grade->elevation" => assert!(grade_seen),
                other => panic!("unexpected key: '{other}'"),
            }
        }
    }

    fn init_test_logger() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();
    }

    struct MockModel {
        in_features: Vec<InputFeature>,
        out_features: Vec<String>,
    }

    impl MockModel {
        pub fn new(in_features: Vec<InputFeature>, out_features: Vec<&str>) -> MockModel {
            MockModel {
                in_features,
                out_features: out_features.into_iter().map(String::from).collect_vec(),
            }
        }
    }

    impl TraversalModel for MockModel {
        fn name(&self) -> String {
            format!(
                "Mock Traversal Model: {} -> {}",
                self.in_features.iter().map(|f| f.name()).join("+"),
                self.out_features.join("+")
            )
        }
        fn input_features(&self) -> Vec<InputFeature> {
            self.in_features.clone()
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



        fn estimate_traversal(
            &self,
            _od: (&Vertex, &Vertex),
            _state: &mut Vec<StateVariable>,
            _tree: &SearchTree,
            _state_model: &StateModel,
        ) -> Result<(), TraversalModelError> {
            todo!()
        }
        
        fn traverse_edge(
            &self,
            _ctx: &crate::model::traversal::EdgeTraversalContext,
            _state: &mut Vec<StateVariable>,
            _state_model: &StateModel,
        ) -> Result<(), TraversalModelError> {
            todo!()
        }
    }
}
