use super::CompassAppError;
use crate::app::{
    compass::response::response_sink::ResponseSink,
    search::{SearchApp, SearchAppResult},
};
use crate::plugin::{
    input::{input_plugin_ops as in_ops, InputJsonExtensions, InputPlugin},
    output::{output_plugin_ops as out_ops, OutputPlugin},
    PluginError,
};
use itertools::Itertools;
use kdam::{Bar, BarExt};
use ordered_float::OrderedFloat;
use rayon::prelude::*;
use routee_compass_core::algorithm::search::SearchInstance;
use routee_compass_core::config::ConfigJsonExtensions;
use routee_compass_core::util::progress;
use serde_json::Value;
use std::sync::{Arc, Mutex};
/// applies the weight balancing policy set by the LoadBalancerPlugin InputPlugin.
///
/// # Arguments
///
/// * `queries` - user queries to load balance based on a query weight heuristic.
/// * `parallelism` - number of chunks to split inputs into, set by user
/// * `default` - weight value if weight heuristic fails to produce an estimate
///
/// # Returns
///
/// An index for sorting the values so that, when fed into rayon's par_chunks iterator,
/// load balances the queries across processes based on the estimates. the resulting
/// batches are not equal-sized
pub fn apply_load_balancing_policy(
    queries: Vec<serde_json::Value>,
    parallelism: usize,
    default: f64,
) -> Result<Vec<Vec<serde_json::Value>>, CompassAppError> {
    if queries.is_empty() {
        return Ok(vec![]);
    }

    let mut bin_totals = vec![0.0; parallelism];
    let mut assignments: Vec<Vec<serde_json::Value>> = vec![vec![]; parallelism];
    let n_queries = queries.len();

    let bar_builder = Bar::builder()
        .total(n_queries)
        .desc("load balancing")
        .animation("fillup");
    let mut bar_opt = progress::build_progress_bar(bar_builder);
    for q in queries.into_iter() {
        let w = q.get_query_weight_estimate()?.unwrap_or(default);
        let min_bin = min_bin(&bin_totals)?;
        bin_totals[min_bin] += w;
        assignments[min_bin].push(q);
        if let Some(ref mut bar) = bar_opt {
            let _ = bar.update(1);
        }
    }
    Ok(assignments)
}

fn min_bin(bins: &[f64]) -> Result<usize, PluginError> {
    bins.iter()
        .enumerate()
        .min_by_key(|(_i, w)| OrderedFloat(**w))
        .map(|(i, _w)| i)
        .ok_or_else(|| {
            PluginError::InternalError(String::from("cannot find min bin of empty slice"))
        })
}

/// executes the input plugins on each query, returning all
/// successful mappings (left) and mapping errors (right) as the pair
/// (left, right). errors are already serialized into JSON.
pub fn apply_input_plugins(
    queries: &mut Vec<Value>,
    input_plugins: &[Arc<dyn InputPlugin>],
    search_app: Arc<SearchApp>,
    parallelism: usize,
) -> Result<(Vec<Value>, Vec<Value>), CompassAppError> {
    // result of each iteration of plugin updates is stored here
    let mut queries_processed = queries.drain(..).collect_vec();
    let mut query_errors: Vec<Value> = vec![];

    // progress bar running for each input plugin
    let mut outer_bar = Bar::builder()
        .total(input_plugins.len())
        .position(0)
        .build()
        .map_err(CompassAppError::InternalError)?;
    outer_bar.set_description("input plugins"); // until we have named plugins

    for (idx, plugin) in input_plugins.iter().enumerate() {
        // nested progress bar running for each query
        // outer_bar.set_description(format!("{}", plugin.name));  // placeholder for named plugins
        let inner_bar = Arc::new(Mutex::new(
            Bar::builder()
                .total(queries_processed.len())
                .position(1)
                .animation("fillup")
                .desc(format!("applying input plugin {}", idx + 1))
                .build()
                .map_err(|e| {
                    CompassAppError::InternalError(format!(
                        "could not build input plugin progress bar: {e}"
                    ))
                })?,
        ));

        let tasks_per_thread = queries_processed.len() as f64 / parallelism as f64;
        let chunk_size: usize = std::cmp::max(1, tasks_per_thread.ceil() as usize);

        // apply this input plugin in parallel, assigning the result back to `queries_processed`
        // and tracking any errors along the way.
        let (good, bad): (Vec<Value>, Vec<Value>) = queries_processed
            .par_chunks_mut(chunk_size)
            .flat_map(|qs| {
                qs.iter_mut()
                    .flat_map(|q| {
                        if let Ok(mut pb_local) = inner_bar.lock() {
                            let _ = pb_local.update(1);
                        }
                        // run the input plugin and flatten the result if it is a JSON array
                        let p = plugin.clone();
                        match p.process(q, search_app.clone()) {
                            Err(e) => vec![in_ops::package_error(&mut q.clone(), e)],
                            Ok(_) => in_ops::unpack_json_array_as_vec(q),
                        }
                    })
                    .collect_vec()
            })
            .partition(|row| !matches!(row.as_object(), Some(obj) if obj.contains_key("error")));
        queries_processed = good;
        query_errors.extend(bad);
    }
    eprintln!();
    eprintln!();

    Ok((queries_processed, query_errors))
}

#[allow(unused)]
pub fn get_optional_run_config<'a, K, T>(
    key: &K,
    parent_key: &K,
    config: Option<&serde_json::Value>,
) -> Result<Option<T>, CompassAppError>
where
    K: AsRef<str>,
    T: serde::de::DeserializeOwned + 'a,
{
    match config {
        Some(c) => {
            let value = c.get_config_serde_optional::<T>(key, parent_key)?;
            Ok(value)
        }
        None => Ok(None),
    }
}

/// Helper function that runs CompassApp on a single query.
/// It is assumed that all pre-processing from InputPlugins have been applied.
/// This function runs a vertex-oriented search and feeds the result into the
/// OutputPlugins for post-processing, returning the result as JSON.
///
/// # Arguments
///
/// * `query` - a single search query that has been processed by InputPlugins
///
/// # Returns
///
/// * The result of the search and post-processing as a JSON object, or, an error
pub fn run_single_query(
    query: &mut serde_json::Value,
    output_plugins: &[Arc<dyn OutputPlugin>],
    search_app: &SearchApp,
) -> Result<serde_json::Value, CompassAppError> {
    let search_result = search_app.run(query);
    let output = apply_output_processing(query, search_result, search_app, output_plugins);
    Ok(output)
}

/// runs a query batch which has been sorted into parallel chunks
/// and retains the responses from each search in memory.
pub fn run_batch_with_responses(
    load_balanced_inputs: &mut Vec<Vec<Value>>,
    output_plugins: &[Arc<dyn OutputPlugin>],
    search_app: &SearchApp,
    response_writer: &ResponseSink,
    pb: Arc<Mutex<Bar>>,
) -> Result<Box<dyn Iterator<Item = Value>>, CompassAppError> {
    let run_query_result = load_balanced_inputs
        .par_iter_mut()
        .map(|queries| {
            queries
                .iter_mut()
                .map(|q| {
                    let mut response = run_single_query(q, output_plugins, search_app)?;
                    if let Ok(mut pb_local) = pb.lock() {
                        let _ = pb_local.update(1);
                    }
                    response_writer.write_response(&mut response)?;
                    Ok(response)
                })
                .collect::<Result<Vec<serde_json::Value>, CompassAppError>>()
        })
        .collect::<Result<Vec<Vec<serde_json::Value>>, CompassAppError>>()?;

    let run_result = run_query_result.into_iter().flatten();

    Ok(Box::new(run_result))
}

/// runs a query batch which has been sorted into parallel chunks.
/// the search result is not persisted in memory.
pub fn run_batch_without_responses(
    load_balanced_inputs: &mut Vec<Vec<Value>>,
    output_plugins: &[Arc<dyn OutputPlugin>],
    search_app: &SearchApp,
    response_writer: &ResponseSink,
    pb: Arc<Mutex<Bar>>,
) -> Result<Box<dyn Iterator<Item = Value>>, CompassAppError> {
    // run the computations, discard values that do not trigger an error
    let _ = load_balanced_inputs
        .par_iter_mut()
        .map(|queries| {
            // fold over query iterator allows us to propagate failures up while still using constant
            // memory to hold the state of the result object. we can't similarly return error values from
            // within a for loop or for_each call, and map creates more allocations. open to other ideas!
            let initial: Result<(), CompassAppError> = Ok(());
            let _ = queries.iter_mut().fold(initial, |_, q| {
                let mut response = run_single_query(q, output_plugins, search_app)?;
                if let Ok(mut pb_local) = pb.lock() {
                    let _ = pb_local.update(1);
                }
                response_writer.write_response(&mut response)?;
                Ok(())
            });
            Ok(())
        })
        .collect::<Result<Vec<_>, CompassAppError>>()?;

    Ok(Box::new(std::iter::empty::<Value>()))
}

// helper that applies the output processing. this includes
// 1. summarizing from the TraversalModel
// 2. applying the output plugins
pub fn apply_output_processing(
    request_json: &serde_json::Value,
    result: Result<(SearchAppResult, SearchInstance), CompassAppError>,
    search_app: &SearchApp,
    output_plugins: &[Arc<dyn OutputPlugin>],
) -> serde_json::Value {
    let mut initial: Value = match out_ops::create_initial_output(request_json, &result, search_app)
    {
        Ok(value) => value,
        Err(error_value) => return error_value,
    };
    for output_plugin in output_plugins.iter() {
        match output_plugin.process(&mut initial, &result) {
            Ok(()) => {}
            Err(e) => return out_ops::package_error(request_json, e),
        }
    }

    initial
}

#[cfg(test)]
mod test {
    use super::apply_load_balancing_policy;
    use crate::plugin::input::InputField;
    use serde_json::json;

    fn test_run_policy(queries: Vec<serde_json::Value>, parallelism: usize) -> Vec<Vec<i64>> {
        apply_load_balancing_policy(queries, parallelism, 1.0)
            .unwrap()
            .iter()
            .map(|qs| {
                let is: Vec<i64> = qs
                    .iter()
                    .map(|q| q.get("index").unwrap().as_i64().unwrap())
                    .collect();
                is
            })
            .collect::<Vec<_>>()
    }

    #[test]
    fn test_uniform_input() {
        // striped
        let queries: Vec<serde_json::Value> = (0..12)
            .map(|i| {
                json!({
                    "index": i,
                    InputField::QueryWeightEstimate.to_str(): 1
                })
            })
            .collect();
        let parallelism = 4;
        let result = test_run_policy(queries, parallelism);
        let expected: Vec<Vec<i64>> =
            vec![vec![0, 4, 8], vec![1, 5, 9], vec![2, 6, 10], vec![3, 7, 11]];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_incremental_input() {
        // this produces the same layout as the uniform input
        let queries: Vec<serde_json::Value> = (0..12)
            .map(|i| {
                json!({
                    "index": i,
                    InputField::QueryWeightEstimate.to_str(): i + 1
                })
            })
            .collect();
        let parallelism = 4;
        let result = test_run_policy(queries, parallelism);
        let expected: Vec<Vec<i64>> =
            vec![vec![0, 4, 8], vec![1, 5, 9], vec![2, 6, 10], vec![3, 7, 11]];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_cycling_input() {
        // an input one can verify via debugging that produces the expected output below
        let queries: Vec<serde_json::Value> = [1, 4, 1, 2, 1, 4, 1, 2, 1, 4, 1, 2]
            .iter()
            .enumerate()
            .map(|(i, estimate)| {
                json!({
                    "index": i,
                    InputField::QueryWeightEstimate.to_str(): estimate
                })
            })
            .collect();
        let parallelism = 4;
        let result = test_run_policy(queries, parallelism);
        let expected = vec![vec![0, 4, 6, 8, 9], vec![1, 10], vec![2, 5], vec![3, 7, 11]];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_big_outlier() {
        let queries: Vec<serde_json::Value> = [4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
            .iter()
            .enumerate()
            .map(|(idx, est)| {
                json!({
                    "index": idx,
                    InputField::QueryWeightEstimate.to_str(): est
                })
            })
            .collect();
        let parallelism = 4;
        let result = test_run_policy(queries, parallelism);
        let expected = vec![vec![0], vec![1, 4, 7, 10], vec![2, 5, 8, 11], vec![3, 6, 9]];
        assert_eq!(result, expected);
    }
}
