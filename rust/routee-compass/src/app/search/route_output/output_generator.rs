use super::summary_op::SummaryOp;
use crate::plugin::output::default::traversal::TraversalOutputFormat;
use routee_compass_core::algorithm::search::EdgeTraversal;
use routee_compass_core::algorithm::search::SearchInstance;
use routee_compass_core::model::cost::TraversalCost;
use serde_json::json;
use std::collections::HashMap;

#[derive(thiserror::Error, Debug)]
pub enum RouteOutputError {
    #[error("failed to generate route output: {0}")]
    OutputGenerationFailed(String),
    #[error("cannot find result route state when route is empty")]
    EmptyRoute,
    #[error("failed serializing final trip state: {0}")]
    StateSerialization(String),
    #[error("failed serializing cost info: {0}")]
    CostSerialization(String),
    #[error("failed serializing state variable: {0}")]
    StateVariableSerialization(String),
}

pub fn generate_route_output(
    route: &Vec<EdgeTraversal>,
    si: &SearchInstance,
    output_format: &TraversalOutputFormat,
    summary_ops: &HashMap<String, SummaryOp>,
) -> Result<serde_json::Value, RouteOutputError> {
    if route.is_empty() {
        return Ok(serde_json::json!({
            "path": output_format.generate_route_output(route, si.map_model.clone(), si.state_model.clone()).map_err(|e| RouteOutputError::OutputGenerationFailed(e.to_string()))?,
            "traversal_summary": serde_json::Map::new(),
            "final_state": serde_json::Value::Null,
            "cost": serde_json::Value::Null,
        }));
    }
    let last_edge = route.last().ok_or(RouteOutputError::EmptyRoute)?;
    let path_json = output_format
        .generate_route_output(route, si.map_model.clone(), si.state_model.clone())
        .map_err(|e| RouteOutputError::OutputGenerationFailed(e.to_string()))?;
    let final_state = si
        .state_model
        .serialize_state(&last_edge.result_state, true)
        .map_err(|e| RouteOutputError::StateSerialization(e.to_string()))?;

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
        .map_err(|e| RouteOutputError::CostSerialization(e.to_string()))?;

    let mut traversal_summary = serde_json::Map::new();
    for (i, (name, feature)) in si.state_model.indexed_iter() {
        let op = summary_ops.get(name).cloned().unwrap_or_else(|| {
            if feature.is_accumulator() {
                SummaryOp::Last
            } else {
                SummaryOp::Sum
            }
        });

        let value = op.summarize_route(route, i);

        let serialized = feature
            .serialize_variable(&value)
            .map_err(|e| RouteOutputError::StateVariableSerialization(e.to_string()))?;
        let unit = feature.get_unit_name();
        let summary_entry = json!({
            "value": serialized,
            "unit": unit,
            "op": op
        });
        traversal_summary.insert(name.clone(), summary_entry);
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
