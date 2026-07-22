use super::TripHistoryTraversalConfig;
use crate::model::state::CustomVariableConfig;
use crate::model::state::{StateModel, StateModelError, StateVariable, StateVariableConfig};
use crate::model::traversal::{EdgeFrontierContext, TraversalModelError};
use uom::si::f64::{Energy, Length, Ratio, ThermodynamicTemperature, Time, Velocity};

pub struct TripHistoryTraversalEngine {
    pub input_state_variable_cfgs: Vec<StateVariableConfig>,
    pub depth: std::num::NonZeroUsize,
    pub output_features: Vec<(String, StateVariableConfig)>, // A tuple containing (feature_name, state_variable_cfg)
}

impl TryFrom<TripHistoryTraversalConfig> for TripHistoryTraversalEngine {
    type Error = TraversalModelError;

    fn try_from(config: TripHistoryTraversalConfig) -> Result<Self, Self::Error> {
        // below, f is "feature", d is "depth", m is number of features, n is max depth
        // output feature names are of the form: ["f1_d1", "f2_d1", ... "fm_d1", "f1_d2", "f2_d2", ..., "f1_dn",..."fm_dn"]
        let output_features = (1..=config.depth.get()) // 1 to depth_n
            .flat_map(|depth| {
                config
                    .input_state_variable_cfgs
                    .iter() // feature_1 to feature_m
                    .map(move |cfg| (format!("{}_{depth}", cfg.get_feature_type()), cfg.clone()))
            })
            .collect();

        Ok(Self {
            input_state_variable_cfgs: config.input_state_variable_cfgs,
            depth: config.depth,
            output_features,
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
        self.shift(state, state_model)?;
        self.insert_first(ctx, state, state_model)?;
        Ok(())
    }
    /// Takes all values in the state vector from depth 1..(depth) and shifts them from
    /// `format!({feature_name}_{depth_value}` to `format!("{feature_name}_{depth_value+1}")`
    /// shifting values one link into "the past".
    pub fn shift(
        &self,
        state: &mut [StateVariable],
        state_model: &StateModel,
    ) -> Result<(), StateModelError> {
        for feature in &self.output_features {
            let current_depth: u32 = feature
                        .0 // feature name
                        .chars()
                        .last()
                        .ok_or(StateModelError::RuntimeError(format!(
                            "Failed to shift trip history input feature {} because the final character of the feature name was None.", feature.0)))?
                        .to_digit(10) // base 10
                        .ok_or(StateModelError::RuntimeError(format!(
                            "Failed to shift trip history input feature {} because the final character of the feature name was not numeric." , feature.0)))?;

            // we don't want to accidentally work with something outside of the specified depth
            // TODO: double check that this is actually what we want.
            let next_depth: u32;

            if current_depth == self.depth.get() as u32 {
                next_depth = 1; // if the depth is the max depth, move the values to the first depth to be overwritten.
            } else if current_depth < self.depth.get() as u32 {
                next_depth = current_depth + 1;
            } else {
                return Err(StateModelError::RuntimeError(format!("For feature {}, depth {} exceeds the max depth of {} in TripHistoryTraversalModel", feature.0, current_depth, self.depth.get())));
            }

            // create the feature name for the next depth (which should be a u32 indicating the feature's depth in the history)
            let mut feature_name_next_depth = feature.0[0..feature.0.len() - 1].to_string();
            feature_name_next_depth.push(char::from_u32(next_depth).ok_or(
                StateModelError::RuntimeError(format!(
                    "Failed to shift trip history input feature {} because the next depth (Option<u32>) was None and couldn't convert to a char.", feature.0
                )),
            )?);

            // use StateVariableConfig variants for choosing the getter.
            // The variants are simply performing the following operation:
            // <feature_i>_<depth_j> = <feature_i>_<depth_j+1>;
            let _ = match feature.1 {
                StateVariableConfig::Distance { .. } => {
                    let distance_next_depth: Length =
                        state_model.get_distance(state, &feature_name_next_depth)?;

                    state_model.set_distance(state, &feature.0, &distance_next_depth)
                }
                StateVariableConfig::Time { .. } => {
                    let time_next_depth: Time =
                        state_model.get_time(state, &feature_name_next_depth)?;

                    state_model.set_time(state, &feature.0, &time_next_depth)
                }
                StateVariableConfig::Speed { .. } => {
                    let speed_next_depth: Velocity =
                        state_model.get_speed(state, &feature_name_next_depth)?;

                    state_model.set_speed(state, &feature.0, &speed_next_depth)
                }
                StateVariableConfig::Energy { .. } => {
                    let energy_next_depth: Energy =
                        state_model.get_energy(state, &feature_name_next_depth)?;

                    state_model.set_energy(state, &feature.0, &energy_next_depth)
                }
                StateVariableConfig::Ratio { .. } => {
                    let ratio_next_depth: Ratio =
                        state_model.get_ratio(state, &feature_name_next_depth)?;
                    state_model.set_ratio(state, &feature.0, &ratio_next_depth)
                }
                StateVariableConfig::Temperature { .. } => {
                    let temperature_next_depth: ThermodynamicTemperature =
                        state_model.get_temperature(state, &feature_name_next_depth)?;
                    state_model.set_temperature(state, &feature.0, &temperature_next_depth)
                }
                StateVariableConfig::Custom { value, .. } => match value {
                    CustomVariableConfig::FloatingPoint { .. } => {
                        let custom_next_depth: f64 =
                            state_model.get_custom_f64(state, &feature_name_next_depth)?;
                        state_model.set_custom_f64(state, &feature.0, &custom_next_depth)
                    }
                    CustomVariableConfig::SignedInteger { .. } => {
                        let custom_next_depth: i64 =
                            state_model.get_custom_i64(state, &feature_name_next_depth)?;
                        state_model.set_custom_i64(state, &feature.0, &custom_next_depth)
                    }
                    CustomVariableConfig::UnsignedInteger { .. } => {
                        let custom_next_depth: u64 =
                            state_model.get_custom_u64(state, &feature_name_next_depth)?;
                        state_model.set_custom_u64(state, &feature.0, &custom_next_depth)
                    }
                    CustomVariableConfig::Boolean { .. } => {
                        let custom_next_depth: bool =
                            state_model.get_custom_bool(state, &feature_name_next_depth)?;
                        state_model.set_custom_bool(state, &feature.0, &custom_next_depth)
                    }
                },
            };
        }
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
    ) -> Result<(), TraversalModelError> {
        let previous_edge = ctx.tree.backtrack_with_depth(ctx.src.vertex_id, 1)?;
        let previous_state: &[StateVariable] = &previous_edge[0].result_state;

        // for each of the input feature options (e.g., {distance, time, cost, etc.})
        for cfg in &self.input_state_variable_cfgs {
            let feature_name = cfg.get_feature_type(); // feature name
            let _ = match cfg {
                StateVariableConfig::Distance { .. } => {
                    let previous_distance =
                        state_model.get_distance(previous_state, &feature_name)?;
                    state_model.set_distance(state, &(feature_name + "_1"), &previous_distance)
                }
                StateVariableConfig::Time { .. } => {
                    let previous_time = state_model.get_time(previous_state, &feature_name)?;
                    state_model.set_time(state, &(feature_name + "_1"), &previous_time)
                }
                StateVariableConfig::Speed { .. } => {
                    let previous_speed = state_model.get_speed(previous_state, &feature_name)?;
                    state_model.set_speed(state, &(feature_name + "_1"), &previous_speed)
                }
                StateVariableConfig::Energy { .. } => {
                    let previous_energy = state_model.get_energy(previous_state, &feature_name)?;
                    state_model.set_energy(state, &(feature_name + "_1"), &previous_energy)
                }
                StateVariableConfig::Ratio { .. } => {
                    let previous_ratio = state_model.get_ratio(previous_state, &feature_name)?;
                    state_model.set_ratio(state, &(feature_name + "_1"), &previous_ratio)
                }
                StateVariableConfig::Temperature { .. } => {
                    let previous_temperature =
                        state_model.get_temperature(previous_state, &feature_name)?;
                    state_model.set_temperature(
                        state,
                        &(feature_name + "_1"),
                        &previous_temperature,
                    )
                }
                StateVariableConfig::Custom { value, .. } => match value {
                    CustomVariableConfig::FloatingPoint { .. } => {
                        let previous_custom_f64 =
                            state_model.get_custom_f64(previous_state, &feature_name)?;
                        state_model.set_custom_f64(
                            state,
                            &(feature_name + "_1"),
                            &previous_custom_f64,
                        )
                    }
                    CustomVariableConfig::SignedInteger { .. } => {
                        let previous_custom_i64 =
                            state_model.get_custom_i64(previous_state, &feature_name)?;
                        state_model.set_custom_i64(
                            state,
                            &(feature_name + "_1"),
                            &previous_custom_i64,
                        )
                    }
                    CustomVariableConfig::UnsignedInteger { .. } => {
                        let previous_custom_u64 =
                            state_model.get_custom_u64(previous_state, &feature_name)?;
                        state_model.set_custom_u64(
                            state,
                            &(feature_name + "_1"),
                            &previous_custom_u64,
                        )
                    }
                    CustomVariableConfig::Boolean { .. } => {
                        let previous_custom_bool =
                            state_model.get_custom_bool(previous_state, &feature_name)?;
                        state_model.set_custom_bool(
                            state,
                            &(feature_name + "_1"),
                            &previous_custom_bool,
                        )
                    }
                },
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_shift() {
        todo!();
    }

    #[test]
    fn test_insert_first() {
        todo!();
    }
}
