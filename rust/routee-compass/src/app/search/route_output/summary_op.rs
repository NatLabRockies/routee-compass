use routee_compass_core::algorithm::search::EdgeTraversal;
use routee_compass_core::model::state::StateVariable;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryOp {
    Sum,
    Avg,
    Last,
    First,
    Min,
    Max,
}

impl SummaryOp {
    pub fn summarize_route(
        &self,
        route: &[EdgeTraversal],
        state_variable_index: usize,
    ) -> StateVariable {
        match self {
            SummaryOp::Sum => route
                .iter()
                .map(|e| e.result_state[state_variable_index])
                .sum(),
            SummaryOp::Avg => {
                let sum = route
                    .iter()
                    .map(|e| e.result_state[state_variable_index])
                    .sum::<StateVariable>();
                let count = route.len() as f64;
                StateVariable(sum.0 / count)
            }
            SummaryOp::Last => route
                .last()
                .map(|e| e.result_state[state_variable_index])
                .unwrap_or(StateVariable::ZERO),
            SummaryOp::First => route
                .first()
                .map(|e| e.result_state[state_variable_index])
                .unwrap_or(StateVariable::ZERO),
            SummaryOp::Min => route
                .iter()
                .map(|e| e.result_state[state_variable_index])
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(StateVariable::ZERO),
            SummaryOp::Max => route
                .iter()
                .map(|e| e.result_state[state_variable_index])
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(StateVariable::ZERO),
        }
    }
}
