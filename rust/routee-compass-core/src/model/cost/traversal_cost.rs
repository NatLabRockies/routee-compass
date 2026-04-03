use allocative::Allocative;
use serde::{Deserialize, Serialize};
#[cfg(feature = "detailed_costs")]
use std::collections::HashMap;

use crate::model::{cost::CostConstraint, unit::Cost};

/// the cost of an edge traversal.
#[derive(Serialize, Deserialize, Clone, Debug, Default, Allocative)]
pub struct TraversalCost {
    /// the cost components with user-defined weighting objectives applied
    pub objective_cost: Cost,
    /// the true total cost of this traversal
    pub edge_cost: Cost,
    #[cfg(feature = "detailed_costs")]
    /// the cost components making up this traversal
    pub cost_component: HashMap<String, Cost>,
}

impl TraversalCost {
    /// helper for building one-off [TraversalCost] values that can be
    /// used in prescribed scenarios such as testing.
    pub fn new(edge_cost: Cost, objective_cost: Cost) -> TraversalCost {
        TraversalCost {
            edge_cost,
            objective_cost,
            #[cfg(feature = "detailed_costs")]
            cost_component: std::collections::HashMap::new(),
        }
    }

    /// creates a TraversalCost where the cost value is the true zero. 
    /// 
    /// IMPORTANT! zero is _not_ a valid cost value to assign to an edge in a search algorithm
    /// that requires values are monotonic to avoid cycles (such as Dijkstra's). in that case,
    /// use [TraversalCost::min].
    pub fn zero() -> TraversalCost {
        TraversalCost::new(Cost::ZERO, Cost::ZERO)
    }

    /// creates a TraversalCost where the cost value is a low value > zero, used when assigning
    /// edge costs in breadth-first tree building search algorithms such as Dijkstra's.
    pub fn min() -> TraversalCost {
        TraversalCost::new(Cost::MIN_COST, Cost::MIN_COST)
    }

    /// inserts a new cost into this traversal.
    /// manages storing a separate notion of objective vs total cost
    /// by only applying the "weight" value to the objective cost.
    ///
    /// when recording a cost component, if it already exists, we append to the cost value.
    pub fn insert(&mut self, #[allow(unused_variables)] name: &str, cost: Cost, weight: f64, constraint: CostConstraint) {
        let insert_cost = match constraint {
            CostConstraint::StrictlyPositive => Cost::enforce_strictly_positive(cost),
            CostConstraint::Unconstrained => cost,
        };
        self.edge_cost += insert_cost;
        self.objective_cost += insert_cost * weight;
        #[cfg(feature = "detailed_costs")]
        {
            self.cost_component
                .entry(name.to_string())
                .and_modify(|c| *c += insert_cost)
                .or_insert(insert_cost);
        }
    }
}
