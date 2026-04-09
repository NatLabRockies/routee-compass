use allocative::Allocative;
use serde::{Deserialize, Serialize};
#[cfg(feature = "detailed_costs")]
use std::collections::HashMap;

use crate::{
    algorithm::search::EdgeTraversal,
    model::{cost::CostConstraint, unit::Cost},
};

/// the cost of an edge traversal.
#[derive(Serialize, Deserialize, Clone, Debug, Allocative)]
pub struct TraversalCost {
    /// the cost components with user-defined weighting objectives applied
    pub objective_cost: Cost,
    /// the cost of traversing this edge
    pub edge_cost: Cost,
    #[cfg(feature = "detailed_costs")]
    /// the cost components making up this traversal
    pub cost_component: HashMap<String, Cost>,
}

/// an accumulation of [TraversalCost] values over the course of a path.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AccumulatedTraversalCost {
    /// the cost components with user-defined weighting objectives applied
    pub objective_cost: Cost,
    /// the cost of traversing this edge
    pub total_cost: Cost,
}

impl TraversalCost {
    /// creates a TraversalCost with no values.
    ///
    /// IMPORTANT! zero is _not_ a valid cost value to assign to an edge in a search algorithm
    /// that requires values are monotonic to avoid cycles (such as Dijkstra's). in that case,
    /// use [TraversalCost::min]. see [CostConstraint].
    pub fn empty() -> TraversalCost {
        TraversalCost {
            edge_cost: Cost::ZERO,
            objective_cost: Cost::ZERO,
            #[cfg(feature = "detailed_costs")]
            cost_component: std::collections::HashMap::new(),
        }
    }

    /// inserts a new cost into this traversal.
    /// manages storing a separate notion of objective vs total cost
    /// by only applying the "weight" value to the objective cost.
    ///
    /// when recording a cost component, if it already exists, we append to the cost value.
    ///
    /// # Arguments
    /// * `name` - name of the state variable that creates this cost observation
    /// * `cost` - the cost of the state variable using the current cost model
    /// * `weight` - weighting factor to apply to this cost value when calculating the objective cost
    /// * `constraint` - constraints on Cost to apply based on the type of search algorithm that is running
    pub fn insert(
        &mut self,
        #[allow(unused_variables)] name: &str,
        cost: Cost,
        weight: f64,
        constraint: CostConstraint,
    ) {
        // handle non-positive costs in the case of monotonic search algorithms
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

    /// helper for building one-off [TraversalCost] values that can be
    /// used in prescribed scenarios such as testing.
    #[cfg(test)]
    pub fn mock(edge_cost: Cost, objective_cost: Cost) -> TraversalCost {
        TraversalCost {
            edge_cost,
            objective_cost,
            #[cfg(feature = "detailed_costs")]
            cost_component: std::collections::HashMap::new(),
        }
    }
}

impl AccumulatedTraversalCost {
    /// creates an [AccumulatedTraversalCost] instance by summing the values
    /// stored in each EdgeTraversal.
    pub fn new(values: &[EdgeTraversal]) -> Self {
        // Compute total route cost by summing all edge costs
        values.iter().fold(Self::default(), |mut acc, edge| {
            acc.total_cost += edge.cost.edge_cost;
            acc.objective_cost += edge.cost.objective_cost;
            acc
        })
    }
}
