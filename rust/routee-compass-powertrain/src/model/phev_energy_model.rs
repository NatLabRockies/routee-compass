use crate::model::fieldname;

use super::{
    energy_model_ops,
    prediction::{PredictionModelConfig, PredictionModelRecord},
};
use routee_compass_core::{
    algorithm::search::SearchTree,
    model::{
        network::{Edge, Vertex},
        state::{InputFeature, StateModel, StateVariable, StateVariableConfig},
        traversal::{TraversalModel, TraversalModelError, TraversalModelService},
        unit::{EnergyRateUnit, EnergyUnit, RatioUnit},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};
use uom::{
    si::f64::{Energy, Ratio},
    ConstZero,
};

#[derive(Clone)]
pub struct PhevEnergyModel {
    /// liquid fuel model used when battery is depleted
    pub charge_sustain_model: Arc<PredictionModelRecord>,
    /// electric fuel model used when battery is non-zero
    pub charge_depleting_model: Arc<PredictionModelRecord>,
    pub battery_capacity: Energy,
    pub starting_soc: Ratio,
    pub include_trip_energy: bool,
}

impl PhevEnergyModel {
    pub fn new(
        charge_sustain_model: Arc<PredictionModelRecord>,
        charge_depleting_model: Arc<PredictionModelRecord>,
        battery_capacity: Energy,
        starting_battery_energy: Energy,
        include_trip_energy: bool,
    ) -> Result<Self, TraversalModelError> {
        let starting_soc = energy_model_ops::soc_from_energy(
            starting_battery_energy,
            battery_capacity,
        )
        .map_err(|e| {
            TraversalModelError::BuildError(format!("Error building PHEV traversal model: {e}"))
        })?;
        Ok(Self {
            charge_sustain_model,
            charge_depleting_model,
            battery_capacity,
            starting_soc,
            include_trip_energy,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PhevEnergyModelConfig {
    pub charge_sustaining: PredictionModelConfig,
    pub charge_depleting: PredictionModelConfig,
    pub battery_capacity: f64,
    pub battery_capacity_unit: EnergyUnit,
    #[serde(default)]
    pub include_trip_energy: Option<bool>,
}

impl TryFrom<&Value> for PhevEnergyModel {
    type Error = TraversalModelError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let config: PhevEnergyModelConfig = serde_json::from_value(value.clone()).map_err(|e| {
            TraversalModelError::BuildError(format!(
                "failure reading phev energy model configuration: {e}"
            ))
        })?;
        let charge_depleting_record = PredictionModelRecord::try_from(&config.charge_depleting)?;
        let charge_sustaining_record = PredictionModelRecord::try_from(&config.charge_sustaining)?;
        let battery_capacity = config.battery_capacity_unit.to_uom(config.battery_capacity);
        let include_trip_energy = config.include_trip_energy.unwrap_or(true);
        let bev = PhevEnergyModel::new(
            Arc::new(charge_sustaining_record),
            Arc::new(charge_depleting_record),
            battery_capacity,
            battery_capacity,
            include_trip_energy,
        )?;
        Ok(bev)
    }
}

impl TraversalModelService for PhevEnergyModel {
    fn build(
        &self,
        query: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        match energy_model_ops::get_query_start_energy(query, self.battery_capacity)? {
            None => Ok(Arc::new(self.clone())),
            Some(starting_energy) => {
                let updated = Self::new(
                    self.charge_sustain_model.clone(),
                    self.charge_depleting_model.clone(),
                    self.battery_capacity,
                    starting_energy,
                    self.include_trip_energy,
                )?;
                Ok(Arc::new(updated))
            }
        }
    }
}

impl TraversalModel for PhevEnergyModel {
    fn name(&self) -> String {
        format!(
            "PHEV Energy Model: {} / {}",
            self.charge_depleting_model.name, self.charge_sustain_model.name
        )
    }
    fn input_features(&self) -> Vec<InputFeature> {
        let mut input_features = vec![InputFeature::Distance {
            name: String::from(fieldname::EDGE_DISTANCE),
            unit: None,
        }];
        input_features.extend(self.charge_depleting_model.input_features.clone());
        input_features.extend(self.charge_sustain_model.input_features.clone());

        // remove any duplicates
        let mut unique_features: HashSet<InputFeature> = HashSet::new();
        for feature in input_features {
            unique_features.insert(feature);
        }
        unique_features.into_iter().collect()
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        let mut features = vec![
            (
                String::from(fieldname::EDGE_ENERGY_LIQUID),
                StateVariableConfig::Energy {
                    initial: Energy::ZERO,
                    accumulator: false,
                    output_unit: Some(
                        self.charge_sustain_model
                            .energy_rate_unit
                            .associated_energy_unit(),
                    ),
                },
            ),
            (
                String::from(fieldname::EDGE_ENERGY_ELECTRIC),
                StateVariableConfig::Energy {
                    initial: Energy::ZERO,
                    accumulator: false,
                    output_unit: Some(
                        self.charge_depleting_model
                            .energy_rate_unit
                            .associated_energy_unit(),
                    ),
                },
            ),
            (
                String::from(fieldname::TRIP_SOC),
                StateVariableConfig::Ratio {
                    initial: self.starting_soc,
                    accumulator: false,
                    output_unit: Some(RatioUnit::Percent),
                },
            ),
            (
                String::from(fieldname::BATTERY_CAPACITY),
                StateVariableConfig::Energy {
                    initial: self.battery_capacity,
                    accumulator: false,
                    output_unit: Some(
                        self.charge_depleting_model
                            .energy_rate_unit
                            .associated_energy_unit(),
                    ),
                },
            ),
        ];

        if self.include_trip_energy {
            features.push((
                String::from(fieldname::TRIP_ENERGY_LIQUID),
                StateVariableConfig::Energy {
                    initial: Energy::ZERO,
                    accumulator: true,
                    output_unit: Some(
                        self.charge_sustain_model
                            .energy_rate_unit
                            .associated_energy_unit(),
                    ),
                },
            ));
            features.push((
                String::from(fieldname::TRIP_ENERGY_ELECTRIC),
                StateVariableConfig::Energy {
                    initial: Energy::ZERO,
                    accumulator: true,
                    output_unit: Some(
                        self.charge_depleting_model
                            .energy_rate_unit
                            .associated_energy_unit(),
                    ),
                },
            ));
        }

        features
    }

    fn traverse_edge(
        &self,
        _trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        phev_traversal(
            state,
            state_model,
            self.charge_depleting_model.clone(),
            self.charge_sustain_model.clone(),
            self.battery_capacity,
            self.include_trip_energy,
            false,
        )
    }

    /// estimates energy use based only on using the liquid fuel model
    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchTree,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        phev_traversal(
            state,
            state_model,
            self.charge_depleting_model.clone(),
            self.charge_sustain_model.clone(),
            self.battery_capacity,
            self.include_trip_energy,
            true,
        )
    }
}

/// a PHEV traversal that switches between a charge depleting and charge sustaining model in order
/// to compute energy consumption. the mechanism checks for battery state of charge, and if it is
/// greater than 0, then the charge depleting model is used. otherwise, the charge sustaining model is used.
/// if battery state is empty mid-traversal, the remaining distance will be applied to the charge
/// sustaining model.
fn phev_traversal(
    state: &mut [StateVariable],
    state_model: &StateModel,
    depleting: Arc<PredictionModelRecord>,
    sustaining: Arc<PredictionModelRecord>,
    battery_capacity: Energy,
    include_trip_energy: bool,
    estimate: bool,
) -> Result<(), TraversalModelError> {
    // collect state variables
    let distance = state_model.get_distance(state, fieldname::EDGE_DISTANCE)?;
    let start_soc = state_model.get_ratio(state, fieldname::TRIP_SOC)?;
    let trip_energy_elec = if include_trip_energy {
        state_model.get_energy(state, fieldname::TRIP_ENERGY_ELECTRIC)?
    } else {
        // if we are not tracking trip energy, we still need to calculate the current battery state
        // based on the start SOC
        (Ratio::new::<uom::si::ratio::ratio>(1.0) - start_soc) * battery_capacity
    };

    // figure out how much energy we had at the start of the edge
    let edge_start_elec = battery_capacity - trip_energy_elec;

    let est_edge_elec = if estimate {
        // TODO: This should be updated once we have a uom EnergyRateUnit
        match depleting.energy_rate_unit {
            EnergyRateUnit::KWHPM => {
                let distance_miles = distance.get::<uom::si::length::mile>();
                let energy_kwh = depleting.a_star_heuristic_energy_rate * distance_miles;
                Energy::new::<uom::si::energy::kilowatt_hour>(energy_kwh)
            }
            EnergyRateUnit::KWHPKM => {
                let distance_km = distance.get::<uom::si::length::kilometer>();
                let energy_kwh = depleting.a_star_heuristic_energy_rate * distance_km;
                Energy::new::<uom::si::energy::kilowatt_hour>(energy_kwh)
            }
            _ => {
                return Err(TraversalModelError::BuildError(format!(
                    "PHEV elec energy model does not support energy rate unit: {}",
                    depleting.energy_rate_unit
                )));
            }
        }
    } else {
        depleting.predict(state, state_model)?
    };

    let remaining_elec: Energy = edge_start_elec - est_edge_elec;

    // did we complete the edge on battery or do we need to switch to our charge sustaining model?
    let energy_overage = Energy::ZERO - remaining_elec;
    let completed_on_battery = energy_overage <= Energy::ZERO;
    if completed_on_battery {
        depleting_only_traversal(
            state,
            state_model,
            start_soc,
            est_edge_elec,
            battery_capacity,
            include_trip_energy,
        )
    } else {
        mixed_traversal(
            state,
            state_model,
            sustaining,
            edge_start_elec,
            trip_energy_elec,
            energy_overage,
            include_trip_energy,
            estimate,
        )
    }
}

/// used when edge traversal can be fully served by battery fuel
fn depleting_only_traversal(
    state: &mut [StateVariable],
    state_model: &StateModel,
    start_soc: Ratio,
    est_edge_elec: Energy,
    battery_capacity: Energy,
    include_trip_energy: bool,
) -> Result<(), TraversalModelError> {
    state_model.set_energy(state, fieldname::EDGE_ENERGY_ELECTRIC, &est_edge_elec)?;
    if include_trip_energy {
        state_model.add_energy(state, fieldname::TRIP_ENERGY_ELECTRIC, &est_edge_elec)?;
    }
    let end_soc = energy_model_ops::update_soc_percent(start_soc, est_edge_elec, battery_capacity)?;
    state_model.set_ratio(state, fieldname::TRIP_SOC, &end_soc)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn mixed_traversal(
    state: &mut [StateVariable],
    state_model: &StateModel,
    sustaining: Arc<PredictionModelRecord>,
    edge_start_elec: Energy,
    trip_energy_elec: Energy,
    energy_overage: Energy,
    include_trip_energy: bool,
    estimate: bool,
) -> Result<(), TraversalModelError> {
    let distance = state_model.get_distance(state, fieldname::EDGE_DISTANCE)?;

    // use up remaining battery first (edge start electricity, not edge consumption electricity)
    state_model.set_energy(state, fieldname::EDGE_ENERGY_ELECTRIC, &edge_start_elec)?;
    if include_trip_energy {
        state_model.add_energy(state, fieldname::TRIP_ENERGY_ELECTRIC, &edge_start_elec)?;
    }
    state_model.set_ratio(state, fieldname::TRIP_SOC, &Ratio::ZERO)?;

    // find the amount of distance remaining on this edge as a ratio of remaining energy to total energy used
    let numer = trip_energy_elec;
    let denom = trip_energy_elec + energy_overage;
    let remaining_ratio = Ratio::new::<uom::si::ratio::ratio>(1.0) - (numer / denom);
    let remaining_dist = distance * remaining_ratio;

    // estimate energy over this distance at the ideal energy rate for the charge sustaining model
    // estimate remaining energy if we travel this distance
    let remaining_energy = if estimate {
        match sustaining.energy_rate_unit {
            // TODO: This should be updated once we have a uom EnergyRateUnit
            EnergyRateUnit::GGPM => {
                let distance_miles = remaining_dist.get::<uom::si::length::mile>();
                let energy_gallons_gas = sustaining.a_star_heuristic_energy_rate * distance_miles;
                EnergyUnit::GallonsGasolineEquivalent.to_uom(energy_gallons_gas)
            }
            EnergyRateUnit::GDPM => {
                let distance_miles = remaining_dist.get::<uom::si::length::mile>();
                let energy_gallons_diesel =
                    sustaining.a_star_heuristic_energy_rate * distance_miles;
                EnergyUnit::GallonsDieselEquivalent.to_uom(energy_gallons_diesel)
            }
            _ => {
                return Err(TraversalModelError::BuildError(format!(
                    "PHEV liquid energy model does not support energy rate unit: {}",
                    sustaining.energy_rate_unit
                )));
            }
        }
    } else {
        sustaining.predict(state, state_model)?
    };

    state_model.set_energy(state, fieldname::EDGE_ENERGY_LIQUID, &remaining_energy)?;
    if include_trip_energy {
        state_model.add_energy(state, fieldname::TRIP_ENERGY_LIQUID, &remaining_energy)?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::PhevEnergyModel;
    use crate::model::{
        fieldname,
        phev_energy_model::phev_traversal,
        prediction::{
            interpolation::feature_bounds::FeatureBounds, ModelType, PredictionModelConfig,
            PredictionModelRecord,
        },
    };
    use routee_compass_core::{
        model::{
            state::{InputFeature, StateModel, StateVariable},
            traversal::TraversalModel,
            unit::{EnergyRateUnit, RatioUnit, SpeedUnit},
        },
        testing::mock::traversal_model::TestTraversalModel,
    };
    use std::{collections::HashMap, path::PathBuf, sync::Arc};
    use uom::{
        si::f64::{Energy, Length, Ratio, Velocity},
        ConstZero,
    };

    #[test]
    fn test_phev_energy_model_just_electric() {
        let bat_cap = Energy::new::<uom::si::energy::kilowatt_hour>(12.0);
        let charge_depleting = mock_prediction_model(
            "2016_CHEVROLET_Volt_Charge_Depleting",
            EnergyRateUnit::KWHPM,
        );
        let charge_sustaining = mock_prediction_model(
            "2016_CHEVROLET_Volt_Charge_Sustaining",
            EnergyRateUnit::GGPM,
        );
        let model = mock_phev(
            charge_sustaining.clone(),
            charge_depleting.clone(),
            Ratio::new::<uom::si::ratio::percent>(100.0),
            bat_cap,
        );
        let state_model = state_model(model);
        // starting at 100% SOC, we should be able to traverse 1000 meters
        // without using any liquid_fuel
        let distance = Length::new::<uom::si::length::meter>(1000.0);
        let speed = Velocity::new::<uom::si::velocity::mile_per_hour>(60.0);
        let grade = Ratio::new::<uom::si::ratio::percent>(0.0);
        let mut state = state_vector(&state_model, distance, speed, grade);

        phev_traversal(
            &mut state,
            &state_model,
            charge_depleting.clone(),
            charge_sustaining.clone(),
            bat_cap,
            true,
            false,
        )
        .expect("test invariant failed");

        let elec = state_model
            .get_energy(&state, fieldname::EDGE_ENERGY_ELECTRIC)
            .expect("test invariant failed");

        let soc = state_model
            .get_ratio(&state, fieldname::TRIP_SOC)
            .expect("test invariant failed");
        assert!(elec > Energy::ZERO, "elec energy {elec:?} should be > 0");

        assert!(
            soc < Ratio::new::<uom::si::ratio::percent>(100.0),
            "soc {soc:?} should be < 100%"
        );
    }

    #[test]
    fn test_phev_energy_model_gas_and_electric() {
        let bat_cap = Energy::new::<uom::si::energy::kilowatt_hour>(12.0);
        let charge_depleting = mock_prediction_model(
            "2016_CHEVROLET_Volt_Charge_Depleting",
            EnergyRateUnit::KWHPM,
        );
        let charge_sustaining = mock_prediction_model(
            "2016_CHEVROLET_Volt_Charge_Sustaining",
            EnergyRateUnit::GGPM,
        );
        let model = mock_phev(
            charge_sustaining.clone(),
            charge_depleting.clone(),
            Ratio::new::<uom::si::ratio::percent>(100.0),
            bat_cap,
        );
        let state_model = state_model(model);

        // now let's traverse a really long link to deplete the battery
        let distance = Length::new::<uom::si::length::mile>(100.0);
        let speed = Velocity::new::<uom::si::velocity::mile_per_hour>(60.0);
        let grade = Ratio::new::<uom::si::ratio::percent>(0.0);
        let mut state = state_vector(&state_model, distance, speed, grade);

        phev_traversal(
            &mut state,
            &state_model,
            charge_depleting.clone(),
            charge_sustaining.clone(),
            bat_cap,
            true,
            false,
        )
        .expect("test invariant failed");

        let elec = state_model
            .get_energy(&state, fieldname::EDGE_ENERGY_ELECTRIC)
            .expect("test invariant failed");
        let soc = state_model
            .get_ratio(&state, fieldname::TRIP_SOC)
            .expect("test invariant failed");

        assert!(
            elec == bat_cap,
            "elec energy {elec:?} should be == battery capacity {bat_cap:?}"
        );
        assert!(soc == Ratio::ZERO, "soc {soc:?} should be 0");

        // and then traverse the same distance but this time we should only use liquid_fuel energy
        phev_traversal(
            &mut state,
            &state_model,
            charge_depleting.clone(),
            charge_sustaining.clone(),
            bat_cap,
            true,
            false,
        )
        .expect("test invariant failed");

        let liquid_energy_2 = state_model
            .get_energy(&state, fieldname::EDGE_ENERGY_LIQUID)
            .expect("test invariant failed");

        assert!(liquid_energy_2 > Energy::ZERO);

        println!(
            "{:?}",
            serde_json::to_string_pretty(&state_model.serialize_state(&state, true).unwrap())
                .unwrap()
        );
    }

    fn mock_phev(
        charge_sustaining: Arc<PredictionModelRecord>,
        charge_depleting: Arc<PredictionModelRecord>,
        starting_soc_percent: Ratio,
        battery_capacity: Energy,
    ) -> Arc<dyn TraversalModel> {
        let starting_battery_energy = battery_capacity * starting_soc_percent;
        let bev = PhevEnergyModel::new(
            charge_sustaining,
            charge_depleting,
            battery_capacity,
            starting_battery_energy,
            true,
        )
        .expect("test invariant failed");

        // mock the upstream models via TestTraversalModel

        (TestTraversalModel::new(Arc::new(bev)).expect("test invariant failed")) as _
    }

    fn mock_prediction_model(
        model_name: &str,
        energy_rate_unit: EnergyRateUnit,
    ) -> Arc<PredictionModelRecord> {
        let model_file_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("test")
            .join(format!("{}.bin", &model_name));
        let model_filename = model_file_path.to_str().expect("test invariant failed");

        let feature_bounds = HashMap::from([
            (
                fieldname::EDGE_SPEED.to_string(),
                FeatureBounds {
                    lower_bound: 0.0,
                    upper_bound: 100.0,
                    num_bins: 101,
                },
            ),
            (
                fieldname::EDGE_GRADE.to_string(),
                FeatureBounds {
                    lower_bound: -0.2,
                    upper_bound: 0.2,
                    num_bins: 41,
                },
            ),
        ]);

        let input_features = vec![
            InputFeature::Speed {
                name: fieldname::EDGE_SPEED.to_string(),
                unit: Some(SpeedUnit::MPH),
            },
            InputFeature::Ratio {
                name: fieldname::EDGE_GRADE.to_string(),
                unit: Some(RatioUnit::Decimal),
            },
        ];

        let model_config = PredictionModelConfig::new(
            model_name.to_string(),
            model_filename.to_string(),
            ModelType::Interpolate {
                underlying_model_type: Box::new(ModelType::Smartcore),
                feature_bounds,
            },
            input_features,
            energy_rate_unit,
            3000.0,
            None,
            Some(1.3958),
        );
        let model_record =
            PredictionModelRecord::try_from(&model_config).expect("test invariant failed");
        Arc::new(model_record)
    }

    fn state_model(m: Arc<dyn TraversalModel>) -> StateModel {
        StateModel::empty()
            .register(m.input_features(), m.output_features())
            .expect("test invariant failed")
    }

    fn state_vector(
        state_model: &StateModel,
        distance: Length,
        speed: Velocity,
        grade: Ratio,
    ) -> Vec<StateVariable> {
        let mut state = state_model.initial_state(None).unwrap();
        state_model
            .set_distance(&mut state, fieldname::EDGE_DISTANCE, &distance)
            .expect("test invariant failed");
        state_model
            .set_speed(&mut state, fieldname::EDGE_SPEED, &speed)
            .expect("test invariant failed");
        state_model
            .set_ratio(&mut state, fieldname::EDGE_GRADE, &grade)
            .expect("test invariant failed");
        state
    }
}
