use itertools::Itertools;

use crate::algorithm::search::SearchGraph;
use crate::model::network::{Edge, Vertex, VertexId};
use crate::model::state::{StateModel, StateVariable};
use crate::model::unit::Cost;
use crate::model::{cost::CostModelError, network::EdgeId};
use std::collections::HashMap;

/// a mapping for how to transform network state values into a Cost.
/// mappings come via lookup functions.
///
/// when multiple mappings are specified they are applied sequentially (in user-defined order)
/// to the state value.
#[derive(Clone, Default, Debug)]
pub enum NetworkCostRate {
    #[default]
    Zero,
    EdgeLookup {
        lookup: HashMap<EdgeId, Cost>,
    },
    VertexLookup {
        lookup: HashMap<VertexId, Cost>,
    },
    Combined(Vec<NetworkCostRate>),
}

impl NetworkCostRate {
    pub fn rate_type(&self) -> String {
        match self {
            NetworkCostRate::Zero => "zero".to_string(),
            NetworkCostRate::EdgeLookup { .. } => "edge".to_string(),
            NetworkCostRate::VertexLookup { .. } => "vertex".to_string(),
            NetworkCostRate::Combined(rates) => {
                let names = rates.iter().map(|r| r.rate_type()).join(", ");
                format!("[{names}]")
            }
        }
    }

    pub fn traversal_cost(
        &self,
        _prev_state_var: StateVariable,
        _next_state_var: StateVariable,
        edge: &Edge,
    ) -> Result<Cost, CostModelError> {
        match self {
            NetworkCostRate::Zero => Ok(Cost::ZERO),
            NetworkCostRate::VertexLookup { lookup: _ } => Ok(Cost::ZERO),
            NetworkCostRate::EdgeLookup { lookup } => {
                let cost = lookup.get(&edge.edge_id).unwrap_or(&Cost::ZERO).to_owned();
                Ok(cost)
            }
            NetworkCostRate::Combined(mappings) => {
                let mapped = mappings
                    .iter()
                    .map(|f| f.traversal_cost(_prev_state_var, _next_state_var, edge))
                    .collect::<Result<Vec<Cost>, CostModelError>>()?;
                let cost = mapped.iter().fold(Cost::ZERO, |a, b| a + *b);

                Ok(cost)
            }
        }
    }

    /// computes the cost for accessing this part of the network.
    pub fn network_cost(
        &self,
        trajectory: (&Vertex, &Edge, &Vertex),
        _state: &[StateVariable],
        _tree: &SearchGraph,
        _state_model: &StateModel,
    ) -> Result<Cost, CostModelError> {
        match self {
            NetworkCostRate::Zero => Ok(Cost::ZERO),
            NetworkCostRate::EdgeLookup { lookup } => {
                let (_, edge, _) = trajectory;
                let cost = lookup.get(&edge.edge_id).copied().unwrap_or_default();
                Ok(cost)
            }
            NetworkCostRate::VertexLookup { lookup } => {
                let (src, _, _) = trajectory;
                let cost = lookup.get(&src.vertex_id).copied().unwrap_or_default();
                Ok(cost)
            }
            NetworkCostRate::Combined(rates) => {
                let mapped = rates
                    .iter()
                    .map(|f| f.network_cost(trajectory, _state, _tree, _state_model))
                    .collect::<Result<Vec<Cost>, CostModelError>>()?;
                let cost = mapped.iter().fold(Cost::ZERO, |a, b| a + *b);

                Ok(cost)
            }
        }
    }
}
