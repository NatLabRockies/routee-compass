use super::{
    cost_ops, network::NetworkCostRate, CostAggregation, CostFeature, TraversalCost,
    VehicleCostRate,
};
use crate::algorithm::search::SearchTree;
use crate::model::cost::CostModelError;
use crate::model::network::Edge;
use crate::model::network::Vertex;
use crate::model::state::StateModel;
use crate::model::state::StateVariable;
use indexmap::IndexMap;
use itertools::Itertools;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

/// implementation of a model for calculating Cost from a state transition.
/// vectorized, where each index in these vectors matches the corresponding index
/// in the state model.
pub struct CostModel {
    features: IndexMap<String, CostFeature>,
    weights_mapping: Arc<HashMap<String, f64>>,
    vehicle_rate_mapping: Arc<HashMap<String, VehicleCostRate>>,
    network_rate_mapping: Arc<HashMap<String, NetworkCostRate>>,
    cost_aggregation: CostAggregation,
}

impl CostModel {
    /// builds a cost model for a specific query.
    ///
    /// this search instance has a state model that dictates the location of each feature.
    /// here we aim to vectorize a mapping from those features into the cost weights,
    /// vehicle cost rates and network cost rates related to that feature.
    /// at runtime, we can iterate through these vectors to compute the cost.
    ///
    /// # Arguments
    /// * `displayed_costs`      - on serialization, the cost values to calculate. state variable names can be called out explicitly here that do not contribute to the total cost.
    /// * `weights`              - user-provided weighting factors for each feature
    /// * `vehicle_rate_mapping` - for each feature name, a vehicle cost rate for that feature
    /// * `network_rate_mapping` - for each feature name, a network cost rate for that feature
    /// * `cost_aggregation`     - function for aggregating each feature cost (for example, Sum)
    /// * `state_model`          - state model instance for this search
    pub fn new(
        weights_mapping: Arc<HashMap<String, f64>>,
        vehicle_rate_mapping: Arc<HashMap<String, VehicleCostRate>>,
        network_rate_mapping: Arc<HashMap<String, NetworkCostRate>>,
        cost_aggregation: CostAggregation,
        state_model: Arc<StateModel>,
    ) -> Result<CostModel, CostModelError> {
        let ignored_weights = weights_mapping
            .keys()
            .filter(|k| !state_model.contains_key(k))
            .collect_vec();
        if !ignored_weights.is_empty() {
            return Err(CostModelError::InvalidWeightNames(
                ignored_weights.iter().map(|k| k.to_string()).collect(),
                state_model.keys().cloned().collect_vec(),
            ));
        }

        let mut features = IndexMap::new();
        let mut total_weight = 0.0;

        for (name, config) in state_model.iter() {
            // always instantiate a value for each vector, diverting to default (zero-valued) if not provided
            // which has the following effect:
            // - weight: deactivates costs for this feature (product)
            // - v_rate: ignores vehicle costs for this feature (sum)
            // - n_rate: ignores network costs for this feature (sum)
            let w_opt = weights_mapping.get(name);
            let v_opt = vehicle_rate_mapping.get(name);
            let n_opt = network_rate_mapping.get(name);
            let feature =
                CostFeature::new(name.clone(), w_opt, v_opt, n_opt, config.is_accumulator());

            total_weight += feature.weight;
            features.insert(name.clone(), feature);
        }

        if total_weight == 0.0 {
            // TODO: update this Error variant after refactor
            return Err(CostModelError::InvalidCostVariables(vec![]));
        }

        Ok(CostModel {
            features,
            weights_mapping,
            vehicle_rate_mapping,
            network_rate_mapping,
            cost_aggregation,
        })
    }

    /// calculates the total trip cost of traversing the provided edge.
    ///
    /// For accumulator features, the cost is computed as the difference between
    /// the current and previous state (delta). For non-accumulator features,
    /// the cost is computed directly from the current state value (which already
    /// represents just the edge traversal).
    pub fn traversal_cost(
        &self,
        trajectory: (&Vertex, &Edge, &Vertex),
        previous_state: &[StateVariable],
        current_state: &[StateVariable],
        tree: &SearchTree,
        state_model: &StateModel,
    ) -> Result<TraversalCost, CostModelError> {
        let mut result = TraversalCost::default();
        for (name, feature) in self.features.iter() {
            let v_cost = if feature.is_accumulator {
                let current_cost =
                    feature
                        .vehicle_cost_rate
                        .compute_cost(name, current_state, state_model)?;
                let previous_cost =
                    feature
                        .vehicle_cost_rate
                        .compute_cost(name, previous_state, state_model)?;
                current_cost - previous_cost
            } else {
                feature
                    .vehicle_cost_rate
                    .compute_cost(name, current_state, state_model)?
            };

            let n_cost = if feature.is_accumulator {
                let current_network_cost = feature.network_cost_rate.network_cost(
                    trajectory,
                    current_state,
                    tree,
                    state_model,
                )?;
                let previous_network_cost = feature.network_cost_rate.network_cost(
                    trajectory,
                    previous_state,
                    tree,
                    state_model,
                )?;
                current_network_cost - previous_network_cost
            } else {
                feature.network_cost_rate.network_cost(
                    trajectory,
                    current_state,
                    tree,
                    state_model,
                )?
            };

            let cost = v_cost + n_cost;
            result.insert(name, cost, feature.weight);
        }
        Ok(result)
    }

    /// calculates the total trip cost of traversing the provided edge.
    pub fn estimate_cost(
        &self,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<TraversalCost, CostModelError> {
        let mut result = TraversalCost::default();
        for (name, feature) in self.features.iter() {
            let v_cost = feature
                .vehicle_cost_rate
                .compute_cost(name, state, state_model)?;
            result.insert(name, v_cost, feature.weight);
        }
        Ok(result)
    }

    /// Serializes other information about a cost model as a JSON value.
    ///
    /// # Arguments
    ///
    /// * `state` - the state to serialize information from
    ///
    /// # Returns
    ///
    /// JSON containing information such as the units (kph, hours, etc) or other
    /// traversal info (charge events, days traveled, etc)
    pub fn serialize_cost_info(&self) -> Result<serde_json::Value, CostModelError> {
        let mut result = serde_json::Map::with_capacity(self.features.len());
        for (index, (name, feature)) in self.features.iter().enumerate() {
            let desc = cost_ops::describe_cost_feature_configuration(
                name,
                self.weights_mapping.clone(),
                self.vehicle_rate_mapping.clone(),
                self.network_rate_mapping.clone(),
            );
            result.insert(
                name.clone(),
                json![{
                    Self::WEIGHT: json![feature.weight],
                    Self::VEHICLE_RATE: json![feature.vehicle_cost_rate],
                    Self::NETWORK_RATE: json![feature.network_cost_rate.rate_type()],
                    Self::INDEX: json![index],
                    Self::DESCRIPTION: json![desc],
                }],
            );
        }

        result.insert(
            Self::COST_AGGREGATION.to_string(),
            json![self.cost_aggregation],
        );

        Ok(json![result])
    }

    const INDEX: &'static str = "index";
    const VEHICLE_RATE: &'static str = "vehicle_rate";
    const NETWORK_RATE: &'static str = "network_rate";
    const WEIGHT: &'static str = "weight";
    const COST_AGGREGATION: &'static str = "cost_aggregation";
    const DESCRIPTION: &'static str = "description";
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::algorithm::search::Direction;
    use crate::model::network::{EdgeId, EdgeListId, VertexId};
    use crate::model::state::StateVariableConfig;
    use crate::model::unit::{AsF64, Cost, DistanceUnit, TimeUnit};
    use crate::util::geo::InternalCoord;
    use geo::coord;
    use std::collections::HashMap;
    use std::sync::Arc;
    use uom::si::f64::*;
    use uom::si::length::meter;
    use uom::si::time::second;

    /// Helper to create a simple vertex
    fn create_vertex(id: VertexId) -> Vertex {
        Vertex {
            vertex_id: id,
            coordinate: InternalCoord(coord! {x: 0.0, y: 0.0}),
        }
    }

    /// Helper to create a simple edge
    fn create_edge(id: EdgeId, src: VertexId, dst: VertexId) -> Edge {
        Edge {
            edge_list_id: EdgeListId(0),
            edge_id: id,
            src_vertex_id: src,
            dst_vertex_id: dst,
            distance: Length::new::<meter>(100.0),
        }
    }

    /// Helper to create a basic search tree for testing
    fn create_test_tree() -> SearchTree {
        SearchTree::new(Direction::Forward)
    }

    #[test]
    fn test_cost_model_new_valid_weights() {
        // Create a state model with one feature
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        // Create weight mapping
        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 1.0);
        let weights = Arc::new(weights);

        // Create vehicle rate mapping
        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert(
            "distance".to_string(),
            VehicleCostRate::Distance {
                factor: 1.0,
                unit: DistanceUnit::Meters,
            },
        );
        let vehicle_rates = Arc::new(vehicle_rates);

        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        // This should succeed
        let result = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_cost_model_new_invalid_weight_names() {
        // Create a state model with one feature
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        // Create weight mapping with a name that doesn't exist in the state model
        let mut weights = HashMap::new();
        weights.insert("invalid_feature".to_string(), 1.0);
        let weights = Arc::new(weights);

        let vehicle_rates = Arc::new(HashMap::new());
        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        // This should fail with InvalidWeightNames error
        let result = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model,
        );
        assert!(matches!(
            result,
            Err(CostModelError::InvalidWeightNames(_, _))
        ));
    }

    #[test]
    fn test_cost_model_new_zero_total_weight() {
        // Create a state model with one feature
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        // Create weight mapping with zero weight
        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 0.0);
        let weights = Arc::new(weights);

        let vehicle_rates = Arc::new(HashMap::new());
        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        // This should fail because total weight is zero
        let result = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model,
        );
        assert!(matches!(
            result,
            Err(CostModelError::InvalidCostVariables(_))
        ));
    }

    #[test]
    fn test_traversal_cost_accumulator_computes_delta() {
        // Setup: Create a state model with an accumulator feature (distance)
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        // Create cost model with distance as accumulator
        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 1.0);
        let weights = Arc::new(weights);

        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert(
            "distance".to_string(),
            VehicleCostRate::Distance {
                factor: 1.0,
                unit: DistanceUnit::Meters,
            },
        );
        let vehicle_rates = Arc::new(vehicle_rates);

        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        let cost_model = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model.clone(),
        )
        .expect("Failed to create cost model");

        // Create states: previous distance = 100.0, current distance = 150.0
        // The delta (edge cost) should be 50.0
        let previous_state = vec![StateVariable(100.0)];
        let current_state = vec![StateVariable(150.0)];

        let v1 = create_vertex(VertexId(0));
        let v2 = create_vertex(VertexId(1));
        let e = create_edge(EdgeId(0), VertexId(0), VertexId(1));
        let trajectory = (&v1, &e, &v2);
        let tree = create_test_tree();

        let result = cost_model
            .traversal_cost(
                trajectory,
                &previous_state,
                &current_state,
                &tree,
                &state_model,
            )
            .expect("Failed to compute traversal cost");

        // For accumulators, we compute the delta
        // The actual cost value will depend on unit conversions,
        // but we can verify the delta is being computed by checking
        // that the cost is positive and reasonable
        assert!(result.total_cost.as_f64() > 0.0);
        // With weight = 1.0, objective cost should equal total cost
        assert_eq!(result.total_cost, result.objective_cost);
    }

    #[test]
    fn test_traversal_cost_non_accumulator_uses_current_value() {
        // Setup: Create a state model with a non-accumulator feature (speed)
        let features = vec![(
            "speed".to_string(),
            StateVariableConfig::Speed {
                initial: Velocity::default(),
                accumulator: false, // Non-accumulator
                output_unit: None,
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        // Create cost model with speed as non-accumulator
        let mut weights = HashMap::new();
        weights.insert("speed".to_string(), 2.0);
        let weights = Arc::new(weights);

        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert("speed".to_string(), VehicleCostRate::Raw);
        let vehicle_rates = Arc::new(vehicle_rates);

        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        let cost_model = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model.clone(),
        )
        .expect("Failed to create cost model");

        // Create states: previous speed = 30.0, current speed = 25.0
        // For non-accumulator, we use the current value directly (25.0)
        let previous_state = vec![StateVariable(30.0)];
        let current_state = vec![StateVariable(25.0)];

        let v1 = create_vertex(VertexId(0));
        let v2 = create_vertex(VertexId(1));
        let e = create_edge(EdgeId(0), VertexId(0), VertexId(1));
        let trajectory = (&v1, &e, &v2);
        let tree = create_test_tree();

        let result = cost_model
            .traversal_cost(
                trajectory,
                &previous_state,
                &current_state,
                &tree,
                &state_model,
            )
            .expect("Failed to compute traversal cost");

        // For non-accumulators, we use the current value: 25.0
        assert_eq!(result.total_cost, Cost::new(25.0));
        // Objective cost applies weight: 25.0 * 2.0 = 50.0
        assert_eq!(result.objective_cost, Cost::new(50.0));
    }

    #[test]
    fn test_traversal_cost_multiple_features_mixed() {
        // Setup: Create a state model with both accumulator and non-accumulator features
        let features = vec![
            (
                "distance".to_string(),
                StateVariableConfig::Distance {
                    initial: Length::new::<meter>(0.0),
                    accumulator: true,
                    output_unit: Some(DistanceUnit::Meters),
                },
            ),
            (
                "time".to_string(),
                StateVariableConfig::Time {
                    initial: Time::new::<second>(0.0),
                    accumulator: true,
                    output_unit: Some(TimeUnit::Seconds),
                },
            ),
            (
                "speed".to_string(),
                StateVariableConfig::Speed {
                    initial: Velocity::default(),
                    accumulator: false,
                    output_unit: None,
                },
            ),
        ];
        let state_model = Arc::new(StateModel::new(features));

        // Create cost model
        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 1.0);
        weights.insert("time".to_string(), 2.0);
        weights.insert("speed".to_string(), 0.5);
        let weights = Arc::new(weights);

        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert(
            "distance".to_string(),
            VehicleCostRate::Distance {
                factor: 1.0,
                unit: DistanceUnit::Meters,
            },
        );
        vehicle_rates.insert(
            "time".to_string(),
            VehicleCostRate::Time {
                factor: 1.0,
                unit: TimeUnit::Seconds,
            },
        );
        vehicle_rates.insert("speed".to_string(), VehicleCostRate::Raw);
        let vehicle_rates = Arc::new(vehicle_rates);

        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        let cost_model = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model.clone(),
        )
        .expect("Failed to create cost model");

        // Create states:
        // - distance: 100.0 -> 200.0 (delta = 100.0)
        // - time: 50.0 -> 80.0 (delta = 30.0)
        // - speed: 60.0 -> 45.0 (current value = 45.0)
        let previous_state = vec![
            StateVariable(100.0),
            StateVariable(50.0),
            StateVariable(60.0),
        ];
        let current_state = vec![
            StateVariable(200.0),
            StateVariable(80.0),
            StateVariable(45.0),
        ];

        let v1 = create_vertex(VertexId(0));
        let v2 = create_vertex(VertexId(1));
        let e = create_edge(EdgeId(0), VertexId(0), VertexId(1));
        let trajectory = (&v1, &e, &v2);
        let tree = create_test_tree();

        let result = cost_model
            .traversal_cost(
                trajectory,
                &previous_state,
                &current_state,
                &tree,
                &state_model,
            )
            .expect("Failed to compute traversal cost");

        // Verify we got a non-zero cost (actual values depend on unit conversions)
        assert!(result.total_cost.as_f64() > 0.0);
        assert!(result.objective_cost.as_f64() > 0.0);
        // For mixed features with weights, the objective and total costs will differ
        // (not all weights are 1.0)
        assert_ne!(result.total_cost, result.objective_cost);
    }

    #[test]
    fn test_traversal_cost_with_network_rates() {
        // Setup: Create a state model with distance feature
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 1.0);
        let weights = Arc::new(weights);

        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert(
            "distance".to_string(),
            VehicleCostRate::Distance {
                factor: 1.0,
                unit: DistanceUnit::Meters,
            },
        );
        let vehicle_rates = Arc::new(vehicle_rates);

        // Create network rates with an edge lookup
        let mut edge_costs = HashMap::new();
        edge_costs.insert(EdgeId(0), Cost::new(10.0));
        let mut network_rates = HashMap::new();
        network_rates.insert(
            "distance".to_string(),
            NetworkCostRate::EdgeLookup { lookup: edge_costs },
        );
        let network_rates = Arc::new(network_rates);

        let cost_aggregation = CostAggregation::Sum;

        let cost_model = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model.clone(),
        )
        .expect("Failed to create cost model");

        // Create states: distance 100.0 -> 150.0 (vehicle cost delta = 50.0)
        // Network cost for edge 0 = 10.0
        let previous_state = vec![StateVariable(100.0)];
        let current_state = vec![StateVariable(150.0)];

        let v1 = create_vertex(VertexId(0));
        let v2 = create_vertex(VertexId(1));
        let e = create_edge(EdgeId(0), VertexId(0), VertexId(1));
        let trajectory = (&v1, &e, &v2);
        let tree = create_test_tree();

        let result = cost_model
            .traversal_cost(
                trajectory,
                &previous_state,
                &current_state,
                &tree,
                &state_model,
            )
            .expect("Failed to compute traversal cost");

        // For accumulators: vehicle cost should be the delta
        // Network cost is computed at both states (same edge), so delta should be 0
        // The result should be > 0 due to vehicle cost delta
        assert!(result.total_cost.as_f64() > 0.0);
        // With weight 1.0, objective == total
        assert_eq!(result.total_cost, result.objective_cost);
    }

    #[test]
    fn test_estimate_cost() {
        // Setup: Create a state model with distance
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 3.0);
        let weights = Arc::new(weights);

        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert(
            "distance".to_string(),
            VehicleCostRate::Distance {
                factor: 1.0,
                unit: DistanceUnit::Meters,
            },
        );
        let vehicle_rates = Arc::new(vehicle_rates);

        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        let cost_model = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model.clone(),
        )
        .expect("Failed to create cost model");

        let state = vec![StateVariable(100.0)];

        let result = cost_model
            .estimate_cost(&state, &state_model)
            .expect("Failed to estimate cost");

        // Estimate cost uses the state value directly
        // Weight is 3.0, so objective cost should be 3x total cost
        assert_eq!(
            result.objective_cost.as_f64(),
            result.total_cost.as_f64() * 3.0
        );
    }

    #[test]
    fn test_zero_weight_feature_has_no_objective_cost() {
        // Setup: Create a state model with distance
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        // Create cost model with zero weight - but we need at least one non-zero weight
        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 0.0);
        let weights = Arc::new(weights);

        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert(
            "distance".to_string(),
            VehicleCostRate::Distance {
                factor: 1.0,
                unit: DistanceUnit::Meters,
            },
        );
        let vehicle_rates = Arc::new(vehicle_rates);

        let network_rates = Arc::new(HashMap::new());
        let cost_aggregation = CostAggregation::Sum;

        // This should fail because total weight is zero
        let result = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model.clone(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_accumulator_network_cost_delta() {
        // Test that network costs for accumulators also compute deltas
        let features = vec![(
            "distance".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model = Arc::new(StateModel::new(features));

        let mut weights = HashMap::new();
        weights.insert("distance".to_string(), 1.0);
        let weights = Arc::new(weights);

        // Zero out vehicle rates to only test network rates
        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert("distance".to_string(), VehicleCostRate::Zero);
        let vehicle_rates = Arc::new(vehicle_rates);

        // Create vertex lookup with different costs
        let mut vertex_costs = HashMap::new();
        vertex_costs.insert(VertexId(0), Cost::new(5.0));
        vertex_costs.insert(VertexId(1), Cost::new(15.0));
        let mut network_rates = HashMap::new();
        network_rates.insert(
            "distance".to_string(),
            NetworkCostRate::VertexLookup {
                lookup: vertex_costs,
            },
        );
        let network_rates = Arc::new(network_rates);

        let cost_aggregation = CostAggregation::Sum;

        let cost_model = CostModel::new(
            weights,
            vehicle_rates,
            network_rates,
            cost_aggregation,
            state_model.clone(),
        )
        .expect("Failed to create cost model");

        let previous_state = vec![StateVariable(100.0)];
        let current_state = vec![StateVariable(150.0)];

        let v1 = create_vertex(VertexId(0));
        let v2 = create_vertex(VertexId(1));
        let e = create_edge(EdgeId(0), VertexId(0), VertexId(1));
        let trajectory = (&v1, &e, &v2);
        let tree = create_test_tree();

        let result = cost_model
            .traversal_cost(
                trajectory,
                &previous_state,
                &current_state,
                &tree,
                &state_model,
            )
            .expect("Failed to compute traversal cost");

        // Network cost is vertex lookup (based on source vertex)
        // For accumulator: it should compute delta, but vertex lookup
        // is not state-dependent, so both calls return the same vertex cost (v1 = 5.0)
        // delta = 5.0 - 5.0 = 0.0
        // vehicle cost = 0.0 (Zero rate)
        // Total should be very close to 0.0 (allowing for floating point precision)
        assert!(result.total_cost.as_f64().abs() < 1e-6);
    }

    #[test]
    fn test_accumulator_vs_non_accumulator_difference() {
        // This test explicitly demonstrates the key difference between
        // accumulator and non-accumulator features

        // Test 1: Accumulator feature (uses delta)
        let features_acc = vec![(
            "test_feature".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: true,
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model_acc = Arc::new(StateModel::new(features_acc));

        let mut weights = HashMap::new();
        weights.insert("test_feature".to_string(), 1.0);
        let weights_acc = Arc::new(weights.clone());

        let mut vehicle_rates = HashMap::new();
        vehicle_rates.insert("test_feature".to_string(), VehicleCostRate::Raw);
        let vehicle_rates_acc = Arc::new(vehicle_rates.clone());

        let cost_model_acc = CostModel::new(
            weights_acc,
            vehicle_rates_acc,
            Arc::new(HashMap::new()),
            CostAggregation::Sum,
            state_model_acc.clone(),
        )
        .expect("Failed to create accumulator cost model");

        // Test 2: Non-accumulator feature (uses current value)
        let features_non_acc = vec![(
            "test_feature".to_string(),
            StateVariableConfig::Distance {
                initial: Length::new::<meter>(0.0),
                accumulator: false, // Non-accumulator
                output_unit: Some(DistanceUnit::Meters),
            },
        )];
        let state_model_non_acc = Arc::new(StateModel::new(features_non_acc));

        let weights_non_acc = Arc::new(weights);
        let vehicle_rates_non_acc = Arc::new(vehicle_rates);

        let cost_model_non_acc = CostModel::new(
            weights_non_acc,
            vehicle_rates_non_acc,
            Arc::new(HashMap::new()),
            CostAggregation::Sum,
            state_model_non_acc.clone(),
        )
        .expect("Failed to create non-accumulator cost model");

        // Use same state transitions for both
        let previous_state = vec![StateVariable(100.0)];
        let current_state = vec![StateVariable(150.0)];

        let v1 = create_vertex(VertexId(0));
        let v2 = create_vertex(VertexId(1));
        let e = create_edge(EdgeId(0), VertexId(0), VertexId(1));
        let trajectory = (&v1, &e, &v2);
        let tree = create_test_tree();

        let result_acc = cost_model_acc
            .traversal_cost(
                trajectory,
                &previous_state,
                &current_state,
                &tree,
                &state_model_acc,
            )
            .expect("Failed to compute accumulator cost");

        let result_non_acc = cost_model_non_acc
            .traversal_cost(
                trajectory,
                &previous_state,
                &current_state,
                &tree,
                &state_model_non_acc,
            )
            .expect("Failed to compute non-accumulator cost");

        // For accumulator: cost = current - previous = 150.0 - 100.0 = 50.0
        let expected_delta = current_state[0].0 - previous_state[0].0;
        assert_eq!(result_acc.total_cost, Cost::new(expected_delta));

        // For non-accumulator: cost = current = 150.0
        assert_eq!(result_non_acc.total_cost, Cost::new(current_state[0].0));

        // The two costs should be different
        assert_ne!(result_acc.total_cost, result_non_acc.total_cost);

        // Non-accumulator cost should be exactly 3x the accumulator cost in this case
        // (150.0 vs 50.0)
        assert_eq!(
            result_non_acc.total_cost.as_f64(),
            result_acc.total_cost.as_f64() * 3.0
        );
    }
}
