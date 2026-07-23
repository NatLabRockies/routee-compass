use super::TripHistoryTraversalConfig;
use crate::model::state::CustomVariableConfig;
use crate::model::state::{StateModel, StateModelError, StateVariable, StateVariableConfig};
use crate::model::traversal::default::trip_history::trip_history_traversal_config::HistoryFeature;
use crate::model::traversal::{EdgeFrontierContext, TraversalModelError};

/// Alias for feature name strings inside of `ShiftFeatureNameMappings`.
///
/// A feature name in the context of `TripHistoryTraversalEngine` is `<feature_i>_<depth_j>`
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
    let depths = (depth - 1)..1;
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
    pub fn update_history(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        for (src, dst, conf) in self.shift_feature_name_mappings.iter() {
            self.shift(state, state_model, src, dst, conf)?;
        }
        for feature in self.history_features.iter() {
            self.insert_first(ctx, state, state_model, &feature.state_variable_config)?;
        }
        Ok(())
    }
    /// Takes feature with name `{feature_i}_{depth_j}` and shifts its value to the value of feature with name `{feature_i}_{depth_(j+1)}`
    /// shifting values one link into "the past".
    pub fn shift(
        &self,
        state: &mut [StateVariable],
        state_model: &StateModel,
        src: &FeatureName,
        dst: &FeatureName,
        conf: &StateVariableConfig,
    ) -> Result<(), StateModelError> {
        // <feature_i>_<depth_j> = <feature_i>_<depth_j+1>;
        copy_state_variable(conf, state_model, state, None, src, dst)?;
        Ok(())
    }

    /// Traverse one step into the history via `ctx.tree.backtrack_with_depth(state_variable, depth)`
    /// and record the value at `format!({feature_name}_1")`. This must be run after `shift` to avoid
    /// overwriting the first history value before shifting. if the backtrack result is empty,
    /// do nothing.
    fn insert_first(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
        state_model: &StateModel,
        conf: &StateVariableConfig,
    ) -> Result<(), TraversalModelError> {
        let previous_edge = ctx.tree.backtrack_with_depth(ctx.src.vertex_id, 1)?;
        let previous_state: &[StateVariable] = &previous_edge[0].result_state;
        let feature_name = conf.get_feature_type();
        copy_state_variable(
            conf,
            state_model,
            state,
            Some(previous_state),
            &(feature_name.clone() + "_1"),
            &feature_name,
        )?;
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
    use crate::{
        config::CompassConfigurationError,
        model::traversal::{
            default::{
                combined::CombinedTraversalBuilder,
                distance::{self, DistanceTraversalBuilder},
                speed::SpeedTraversalBuilder,
                time::{TimeTraversalBuilder, TimeTraversalModel},
                trip_history::TripHistoryTraversalBuilder,
            },
            TraversalModelBuilder,
        },
    };
    use std::{collections::HashMap, error::Error, path::PathBuf, rc::Rc};
    #[test]
    fn test_shift() -> Result<(), String> {
        // The current plan for shift is to take mock up distance, speed, time traversals
        // and run TripHistoryTraversal model for a depth of 2.
        let combined_builder_parameters = r#"{"models": [
            {
            "type": "distance",
            "distance_unit": "miles"
            },
            {
            "type": "speed",
            "speed_table_input_file": "src/model/traversal/default/trip_history/test/test_edge_speeds.csv",
            "speed_unit": "kph"
            },
            {
            "type": "time",
            "time_unit": "minutes"
            },
            {
            "type": "trip_history",
            "depth": 2,
            "history_features": [
                { "name": "edge_distance", "state_variable_config": { "type": "distance", "output_unit": "miles" }},
                { "name": "edge_speed", "state_variable_config": { "type": "speed", "output_unit": "kph" }},
                { "name": "edge_time", "state_variable_config": { "type": "time", "output_unit": "minutes" }}
            ]
            }]
            }"#;

        let mut builder_hashmap: HashMap<String, Rc<dyn TraversalModelBuilder>> = HashMap::new();
        builder_hashmap.insert("distance".to_string(), Rc::new(DistanceTraversalBuilder {}));
        builder_hashmap.insert("time".to_string(), Rc::new(TimeTraversalBuilder {}));
        builder_hashmap.insert("speed".to_string(), Rc::new(SpeedTraversalBuilder {}));
        builder_hashmap.insert(
            "trip_history".to_string(),
            Rc::new(TripHistoryTraversalBuilder {}),
        );

        let combined_builder = CombinedTraversalBuilder::new(builder_hashmap);
        let combined_service = combined_builder
            .build(&serde_json::from_str(combined_builder_parameters).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

        let query = serde_json::json!({
            "origin_vertex": 0,
            "destination_vertex": 2
        });

        let combined_model = combined_service.build(&query).map_err(|e| e.to_string())?;

        // combined_model.traverse_edge(ctx, state, state_model)
        Ok(())
    }

    #[test]
    fn test_insert_first() {
        todo!();
    }
}
