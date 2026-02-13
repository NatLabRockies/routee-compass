use super::json_extensions::TraversalJsonField;
use super::traversal_output_format::TraversalOutputFormat;
use crate::app::compass::CompassAppError;
use crate::app::search::{generate_route_output, RouteOutputError, SearchAppResult, SummaryOp};
use crate::plugin::output::output_plugin::OutputPlugin;
use crate::plugin::output::OutputPluginError;
use routee_compass_core::algorithm::search::SearchInstance;
use serde_json::json;
use std::collections::HashMap;

pub struct TraversalPlugin {
    route: Option<TraversalOutputFormat>,
    tree: Option<TraversalOutputFormat>,
    summary_ops: HashMap<String, SummaryOp>,
    route_key: String,
    tree_key: String,
}

impl TraversalPlugin {
    pub fn new(
        route: Option<TraversalOutputFormat>,
        tree: Option<TraversalOutputFormat>,
        summary_ops: HashMap<String, SummaryOp>,
    ) -> Result<TraversalPlugin, OutputPluginError> {
        let route_key = TraversalJsonField::RouteOutput.to_string();
        let tree_key = TraversalJsonField::TreeOutput.to_string();
        Ok(TraversalPlugin {
            route,
            tree,
            summary_ops,
            route_key,
            tree_key,
        })
    }
}

impl OutputPlugin for TraversalPlugin {
    fn process(
        &self,
        output: &mut serde_json::Value,
        search_result: &Result<(SearchAppResult, SearchInstance), CompassAppError>,
    ) -> Result<(), OutputPluginError> {
        let (result, si) = match search_result {
            Err(_) => return Ok(()),
            Ok((result, si)) => (result, si),
        };

        // output route if configured
        if let Some(route_args) = self.route {
            let mut summary_ops = self.summary_ops.clone();
            let query_summary_ops: Option<HashMap<String, SummaryOp>> = output
                .get("request")
                .and_then(|r| r.get("summary_ops"))
                .and_then(|s| serde_json::from_value(s.clone()).ok());

            if let Some(query_ops) = query_summary_ops {
                summary_ops.extend(query_ops);
            }

            let routes_serialized = result
                .routes
                .iter()
                .map(|route| generate_route_output(route, si, &route_args, &summary_ops))
                .collect::<Result<Vec<_>, RouteOutputError>>()
                .map_err(|e| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "failed to generate route output: {}",
                        e
                    ))
                })?;

            // vary the type of value stored at the route key. if there is
            // no route, store 'null'. if one, store an output object. if
            // more, store an array of objects.
            let routes_json = match routes_serialized.as_slice() {
                [] => serde_json::Value::Null,
                [route] => route.to_owned(),
                _ => json![routes_serialized],
            };
            output[&self.route_key] = routes_json;
        }

        // output tree(s) if configured
        if let Some(tree_args) = self.tree {
            let trees_serialized = result
                .trees
                .iter()
                .map(|tree| {
                    // tree_args.generate_tree_output(tree, &self.geoms)
                    tree_args.generate_tree_output(
                        tree,
                        si.map_model.clone(),
                        si.state_model.clone(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let trees_json = match trees_serialized.as_slice() {
                [] => serde_json::Value::Null,
                [tree] => tree.to_owned(),
                _ => json![trees_serialized],
            };
            output[&self.tree_key] = json![trees_json];
        }

        Ok(())
    }
}
