use allocative::Allocative;
use serde::{Deserialize, Serialize};
#[cfg(feature = "detailed_costs")]
use std::collections::HashMap;

use crate::model::unit::Cost;

/// the cost of an edge traversal.
#[derive(Serialize, Deserialize, Default, Clone, Debug, Allocative)]
pub struct TraversalCost {
    /// the cost components with user-defined weighting objectives applied
    pub objective_cost: Cost,
    /// the true total cost of this traversal
    pub total_cost: Cost,
    #[cfg(feature = "detailed_costs")]
    /// the cost components making up this traversal
    pub cost_component: HashMap<String, Cost>,
}

impl TraversalCost {
    /// inserts a new cost into this traversal.
    /// manages storing a separate notion of objective vs total cost
    /// by only applying the "weight" value to the objective cost.
    ///
    /// when recording a cost component, if it already exists, we append to the cost value.
    pub fn insert(&mut self, #[allow(unused_variables)] name: &str, cost: Cost, weight: f64) {
        let positive_cost = Cost::enforce_strictly_positive(cost);
        self.total_cost += positive_cost;
        self.objective_cost += positive_cost * weight;
        #[cfg(feature = "detailed_costs")]
        {
            self.cost_component
                .entry(name.to_string())
                .and_modify(|c| *c += positive_cost)
                .or_insert(positive_cost);
        }
    }
}
