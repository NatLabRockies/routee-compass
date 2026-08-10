use super::TripHistoryTraversalConfig;
use crate::model::state::CustomVariableConfig;
use crate::model::state::{StateModel, StateModelError, StateVariable, StateVariableConfig};
use crate::model::traversal::default::trip_history::trip_history_traversal_config::HistoryFeature;
use crate::model::traversal::{EdgeFrontierContext, TraversalModelError};

/// Alias for feature name strings inside of `ShiftFeatureNameMappings`.
///
/// A feature name derived from the `TripHistoryTraversalModel` is `<feature_i>_<depth_j>`
/// Where `<feature_i>` is an alphabetic string representing the physcial feature name (e.g. "distance" or "time")
/// and `<depth_j>` is a numeric string that represents the depth backwards that the historical feature was pulled from in the tree.
type FeatureName = String;

/// The alias `ShiftFeatureNameMappings` is a vector map that maps all output feature names `<feature_i>_<depth_j>` to `<feature_i>_<depth_(j+1)>` for
/// use in the `shift()` algorithm in the `TripHistoryTraversalEngine`.
///
/// The first column of the vector is the original feature name, the next column is the target feature name for the shift algorithm, the third column
/// is the StateVariable type's StateVariableConfig
type ShiftFeatureNameMappings = Vec<(FeatureName, FeatureName, StateVariableConfig)>;

/// Given a set of `StateVariableConfig`s and a desired backtracking `depth`,
/// builds the vector mapping from feature name `"<variable_i>_<depth_j>"` to `"<variable_i>_<depth_(j+1)"`
/// and includes the feature's `StateVariableConfig`.
fn build_shift_feature_name_mappings(
    history_features: Vec<HistoryFeature>,
    depth: usize,
) -> Result<ShiftFeatureNameMappings, String> {
    // include d=0 so the carried-forward `_0` (previous edge's value) shifts into `_1`
    let depths = (0..=depth - 1).rev();
    let mapping = depths
        .flat_map(|d| {
            history_features.iter().map(move |feature| {
                (
                    format!("{}_{}", feature.name, d),
                    format!("{}_{}", feature.name, d + 1),
                    feature.state_variable_config.clone(),
                )
            })
        })
        .collect::<Vec<(FeatureName, FeatureName, StateVariableConfig)>>();
    Ok(mapping)
}
/// The `TripHistoryTraversalEngine` is responsible for managing and updating historical trip features
/// within the state, using the set of input history features (e.g., "edge_distance", "edge_time", etc.) and the desired depth backwards in the history of the tree.
pub struct TripHistoryTraversalEngine {
    pub history_features: Vec<HistoryFeature>,
    pub depth: std::num::NonZeroUsize,
    pub shift_feature_name_mappings: ShiftFeatureNameMappings,
}

impl TryFrom<TripHistoryTraversalConfig> for TripHistoryTraversalEngine {
    type Error = TraversalModelError;

    fn try_from(config: TripHistoryTraversalConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            history_features: config.history_features.clone(),
            depth: config.depth,
            shift_feature_name_mappings: build_shift_feature_name_mappings(
                config.history_features,
                config.depth.get(),
            )
            .map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Failed to build ShiftFeatureNameMappings because of {e}"
                ))
            })?,
        })
    }
}

impl TripHistoryTraversalEngine {
    /// Shifts the accumulated history one slot deeper, then refreshes `_0` with the
    /// current edge's value for the next traversal. History slots `_0.._n` are accumulators,
    /// so `_0` carried forward holds the previous edge's value (replacing the tree lookup).
    pub fn update_history(
        &self,
        state: &mut [StateVariable],
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        for (src, dst, conf) in self.shift_feature_name_mappings.iter() {
            copy_state_variable(conf, state_model, state, None, dst, src)?;
        }
        // record the current edge's source value into `_0`
        for feature in self.history_features.iter() {
            copy_state_variable(
                &feature.state_variable_config,
                state_model,
                state,
                None,
                &format!("{}_0", feature.name),
                &feature.name,
            )?;
        }
        Ok(())
    }
}

/// Copies a state variable from state `state` with feature name `read_name` into a state variable from state `state` with feature name `write_name`.
///
/// If `read_state` is specified, we can choose a different `&[StateVariable]` to read from. In this case, variables are still
/// written to `state`.
fn copy_state_variable(
    conf: &StateVariableConfig,
    state_model: &StateModel,
    state: &mut [StateVariable], // The state vector write to. Also the state vector to read from if `read_state` not specified.
    read_state: Option<&[StateVariable]>, // Optionally, the state vector to read from
    write_name: &str,
    read_name: &str,
) -> Result<(), StateModelError> {
    match conf {
        StateVariableConfig::Distance { .. } => {
            let v = state_model.get_distance(read_state.unwrap_or(state), read_name)?;
            state_model.set_distance(state, write_name, &v)
        }
        StateVariableConfig::Time { .. } => {
            let v = state_model.get_time(read_state.unwrap_or(state), read_name)?;
            state_model.set_time(state, write_name, &v)
        }
        StateVariableConfig::Speed { .. } => {
            let v = state_model.get_speed(read_state.unwrap_or(state), read_name)?;
            state_model.set_speed(state, write_name, &v)
        }
        StateVariableConfig::Energy { .. } => {
            let v = state_model.get_energy(read_state.unwrap_or(state), read_name)?;
            state_model.set_energy(state, write_name, &v)
        }
        StateVariableConfig::Ratio { .. } => {
            let v = state_model.get_ratio(read_state.unwrap_or(state), read_name)?;
            state_model.set_ratio(state, write_name, &v)
        }
        StateVariableConfig::Temperature { .. } => {
            let v = state_model.get_temperature(read_state.unwrap_or(state), read_name)?;
            state_model.set_temperature(state, write_name, &v)
        }
        StateVariableConfig::Custom { value, .. } => match value {
            CustomVariableConfig::FloatingPoint { .. } => {
                let v = state_model.get_custom_f64(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_f64(state, write_name, &v)
            }
            CustomVariableConfig::SignedInteger { .. } => {
                let v = state_model.get_custom_i64(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_i64(state, write_name, &v)
            }
            CustomVariableConfig::UnsignedInteger { .. } => {
                let v = state_model.get_custom_u64(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_u64(state, write_name, &v)
            }
            CustomVariableConfig::Boolean { .. } => {
                let v = state_model.get_custom_bool(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_bool(state, write_name, &v)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        state::{StateModel, StateVariable, StateVariableConfig},
        traversal::default::trip_history::{
            trip_history_traversal_config::HistoryFeature, TripHistoryTraversalConfig,
        },
        unit::DistanceUnit,
    };
    use std::{num::NonZeroUsize, sync::Arc};
    use uom::si::f64::Length;

    fn mock_engine() -> Arc<TripHistoryTraversalEngine> {
        Arc::new(
            TripHistoryTraversalEngine::try_from(TripHistoryTraversalConfig {
                history_features: vec![HistoryFeature {
                    name: "edge_distance".to_string(),
                    state_variable_config: StateVariableConfig::Distance {
                        output_unit: Some(DistanceUnit::Miles),
                        initial: Length::default(),
                        accumulator: true,
                    },
                }],
                depth: NonZeroUsize::new(3).unwrap(),
            })
            .unwrap(),
        )
    }

    fn mock_state_model() -> StateModel {
        let conf = StateVariableConfig::Distance {
            initial: Length::default(),
            accumulator: true,
            output_unit: Some(DistanceUnit::Miles),
        };

        let features = vec![
            ("edge_distance".to_string(), conf.clone()),
            ("edge_distance_0".to_string(), conf.clone()),
            ("edge_distance_1".to_string(), conf.clone()),
            ("edge_distance_2".to_string(), conf.clone()),
            ("edge_distance_3".to_string(), conf.clone()),
        ];
        StateModel::new(features)
    }

    /// **Tests `update_history`: verifies that `_0` shifts to `_1`, `_1` to `_2`, `_2` to `_3`,
    /// and `_0` is refreshed with the current edge's `edge_distance`.**
    #[test]
    fn test_update_history() {
        let engine = mock_engine();
        let state_model = mock_state_model();

        let mut state = vec![StateVariable(0.0); 5];

        // Current edge value set by distance traversal model
        state_model
            .set_distance(
                &mut state,
                "edge_distance",
                &Length::new::<uom::si::length::mile>(1.0),
            )
            .unwrap();

        // Carried-forward history slots (_0 is previous edge e_{k-1})
        state_model
            .set_distance(
                &mut state,
                "edge_distance_0",
                &Length::new::<uom::si::length::mile>(2.0),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_1",
                &Length::new::<uom::si::length::mile>(3.0),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_2",
                &Length::new::<uom::si::length::mile>(4.0),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_3",
                &Length::new::<uom::si::length::mile>(5.0),
            )
            .unwrap();

        engine.update_history(&mut state, &state_model).unwrap();

        // _0 refreshed with current edge distance (1.0)
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_0").unwrap(),
            Length::new::<uom::si::length::mile>(1.0)
        );
        // _1 received previous _0 (2.0)
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_1").unwrap(),
            Length::new::<uom::si::length::mile>(2.0)
        );
        // _2 received previous _1 (3.0)
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_2").unwrap(),
            Length::new::<uom::si::length::mile>(3.0)
        );
        // _3 received previous _2 (4.0)
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_3").unwrap(),
            Length::new::<uom::si::length::mile>(4.0)
        );
    }

    /// **Tests first-traversal case: traversing out of the origin where history slots start as NAN.**
    #[test]
    fn test_first_traversal_no_previous_edge() {
        let engine = mock_engine();
        let state_model = mock_state_model();

        let mut state = vec![StateVariable(0.0); 5];

        // Origin initial state: history slots are sentinel NANs
        for name in [
            "edge_distance_0",
            "edge_distance_1",
            "edge_distance_2",
            "edge_distance_3",
        ] {
            state_model
                .set_distance(
                    &mut state,
                    name,
                    &Length::new::<uom::si::length::mile>(f64::NAN),
                )
                .unwrap();
        }

        // Distance model sets current (first) edge distance
        state_model
            .set_distance(
                &mut state,
                "edge_distance",
                &Length::new::<uom::si::length::mile>(2.0),
            )
            .unwrap();

        engine.update_history(&mut state, &state_model).unwrap();

        // _0 holds current edge (2.0) for the next traversal
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_0").unwrap(),
            Length::new::<uom::si::length::mile>(2.0)
        );

        // _1.._3 remain NAN because there are no previous edges
        for name in ["edge_distance_1", "edge_distance_2", "edge_distance_3"] {
            let d = state_model.get_distance(&state, name).unwrap();
            assert!(d.value.is_nan(), "{name} should remain NAN");
        }
    }

    /// **Tests multi-step sentinel shifting across sequential edge traversals.**
    #[test]
    fn test_sentinel_shifting() {
        let engine = mock_engine();
        let state_model = mock_state_model();

        let mut state = vec![StateVariable(0.0); 5];

        // Start at origin with NAN sentinels
        for name in [
            "edge_distance_0",
            "edge_distance_1",
            "edge_distance_2",
            "edge_distance_3",
        ] {
            state_model
                .set_distance(
                    &mut state,
                    name,
                    &Length::new::<uom::si::length::mile>(f64::NAN),
                )
                .unwrap();
        }

        // Step 1: Traverse edge 1 (distance = 10.0)
        state_model
            .set_distance(
                &mut state,
                "edge_distance",
                &Length::new::<uom::si::length::mile>(10.0),
            )
            .unwrap();
        engine.update_history(&mut state, &state_model).unwrap();

        assert_eq!(
            state_model.get_distance(&state, "edge_distance_0").unwrap(),
            Length::new::<uom::si::length::mile>(10.0)
        );
        assert!(state_model
            .get_distance(&state, "edge_distance_1")
            .unwrap()
            .value
            .is_nan());

        // Step 2: Traverse edge 2 (distance = 20.0)
        state_model
            .set_distance(
                &mut state,
                "edge_distance",
                &Length::new::<uom::si::length::mile>(20.0),
            )
            .unwrap();
        engine.update_history(&mut state, &state_model).unwrap();

        assert_eq!(
            state_model.get_distance(&state, "edge_distance_0").unwrap(),
            Length::new::<uom::si::length::mile>(20.0)
        );
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_1").unwrap(),
            Length::new::<uom::si::length::mile>(10.0)
        );
        assert!(state_model
            .get_distance(&state, "edge_distance_2")
            .unwrap()
            .value
            .is_nan());

        // Step 3: Traverse edge 3 (distance = 30.0)
        state_model
            .set_distance(
                &mut state,
                "edge_distance",
                &Length::new::<uom::si::length::mile>(30.0),
            )
            .unwrap();
        engine.update_history(&mut state, &state_model).unwrap();

        assert_eq!(
            state_model.get_distance(&state, "edge_distance_0").unwrap(),
            Length::new::<uom::si::length::mile>(30.0)
        );
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_1").unwrap(),
            Length::new::<uom::si::length::mile>(20.0)
        );
        assert_eq!(
            state_model.get_distance(&state, "edge_distance_2").unwrap(),
            Length::new::<uom::si::length::mile>(10.0)
        );
        assert!(state_model
            .get_distance(&state, "edge_distance_3")
            .unwrap()
            .value
            .is_nan());
    }
}
