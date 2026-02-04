use crate::plugin::output::default::traversal::TraversalOutputFormat;
use routee_compass_core::algorithm::search::EdgeTraversal;
use routee_compass_core::algorithm::search::SearchInstance;
use routee_compass_core::model::cost::TraversalCost;
use routee_compass_core::model::state::StateVariable;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
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
    pub fn default_summary_ops() -> HashMap<String, SummaryOp> {
        use routee_compass_core::model::traversal::default::fieldname::*;
        HashMap::from([
            (EDGE_DISTANCE.to_string(), SummaryOp::Sum),
            (EDGE_SPEED.to_string(), SummaryOp::Avg),
            (EDGE_TIME.to_string(), SummaryOp::Sum),
            (EDGE_GRADE.to_string(), SummaryOp::Avg),
            (EDGE_TURN_DELAY.to_string(), SummaryOp::Sum),
            (AMBIENT_TEMPERATURE.to_string(), SummaryOp::Avg),
            (TRIP_DISTANCE.to_string(), SummaryOp::Last),
            (TRIP_TIME.to_string(), SummaryOp::Last),
            (TRIP_ELEVATION_GAIN.to_string(), SummaryOp::Last),
            (TRIP_ELEVATION_LOSS.to_string(), SummaryOp::Last),
        ])
    }
}

pub struct RouteOutput;

impl RouteOutput {
    /// Generates the JSON output for a route, including the path and a summary of the state.
    pub fn generate(
        route: &Vec<EdgeTraversal>,
        si: &SearchInstance,
        output_format: &TraversalOutputFormat,
        summary_ops: &HashMap<String, SummaryOp>,
    ) -> Result<serde_json::Value, String> {
        if route.is_empty() {
            return Ok(serde_json::json!({
                "path": output_format.generate_route_output(route, si.map_model.clone(), si.state_model.clone()).map_err(|e| e.to_string())?,
                "traversal_summary": serde_json::Map::new(),
                "final_state": serde_json::Value::Null,
                "cost": serde_json::Value::Null,
            }));
        }
        let last_edge = route
            .last()
            .ok_or_else(|| String::from("cannot find result route state when route is empty"))?;
        let path_json = output_format
            .generate_route_output(route, si.map_model.clone(), si.state_model.clone())
            .map_err(|e| e.to_string())?;
        let final_state = si
            .state_model
            .serialize_state(&last_edge.result_state, true)
            .map_err(|e| format!("failed serializing final trip state: {e}"))?;

        let state_model = si.state_model.serialize_state_model();

        // Compute total route cost by summing all edge costs
        let route_cost = route
            .iter()
            .fold(TraversalCost::default(), |mut acc, edge| {
                acc.total_cost += edge.cost.total_cost;
                acc.objective_cost += edge.cost.objective_cost;
                acc
            });

        let cost = json![route_cost];
        let cost_model = si
            .cost_model
            .serialize_cost_info()
            .map_err(|e| e.to_string())?;

        let default_summary_ops = SummaryOp::default_summary_ops();
        let mut traversal_summary = serde_json::Map::new();
        for (i, (name, feature)) in si.state_model.indexed_iter() {
            let op = summary_ops
                .get(name)
                .cloned()
                .or_else(|| default_summary_ops.get(name).cloned())
                .unwrap_or_else(|| {
                    if feature.is_accumulator() {
                        SummaryOp::Last
                    } else {
                        SummaryOp::Sum
                    }
                });

            let value = match op {
                SummaryOp::Sum => route
                    .iter()
                    .map(|e| e.result_state[i])
                    .sum::<StateVariable>(),
                SummaryOp::Avg => {
                    let sum = route
                        .iter()
                        .map(|e| e.result_state[i])
                        .sum::<StateVariable>();
                    let count = route.len() as f64;
                    StateVariable(sum.0 / count)
                }
                SummaryOp::Last => last_edge.result_state[i],
                SummaryOp::First => route
                    .first()
                    .map(|e| e.result_state[i])
                    .unwrap_or(StateVariable::ZERO),
                SummaryOp::Min => route
                    .iter()
                    .map(|e| e.result_state[i])
                    .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(StateVariable::ZERO),
                SummaryOp::Max => route
                    .iter()
                    .map(|e| e.result_state[i])
                    .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(StateVariable::ZERO),
            };
            let serialized = feature
                .serialize_variable(&value)
                .map_err(|e| e.to_string())?;
            traversal_summary.insert(name.clone(), serialized);
        }

        let result = serde_json::json![{
            "final_state": final_state,
            "state_model": state_model,
            "cost_model": cost_model,
            "cost": cost,
            "path": path_json,
            "traversal_summary": traversal_summary
        }];
        Ok(result)
    }
}

