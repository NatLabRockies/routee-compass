use crate::app::compass::CompassAppError;
use crate::app::{
    compass::response::response_sink::ResponseSink,
    search::{SearchApp, SearchAppResult},
};
use crate::plugin::{
    input::{input_plugin_ops as in_ops, InputJsonExtensions, InputPlugin},
    output::{output_plugin_ops as out_ops, OutputPlugin},
    PluginError,
};
use chrono::Local;
use itertools::Itertools;
use kdam::{Bar, BarExt};
use ordered_float::OrderedFloat;
use rayon::prelude::*;
use routee_compass_core::algorithm::search::SearchInstance;
use routee_compass_core::config::ConfigJsonExtensions;
use routee_compass_core::model::network::{EdgeId, EdgeListId};
use routee_compass_core::util::duration_extension::DurationExtension;
use routee_compass_core::util::progress;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Creates a shared progress bar wrapped in Arc<Mutex<>> for parallel processing.
///
/// # Arguments
///
/// * `total` - Total number of items to process
/// * `desc` - Description to show on the progress bar
///
/// # Returns
///
/// A progress bar wrapped in Arc<Mutex<>> for thread-safe updates
pub fn create_progress_bar(total: usize, desc: &str) -> Result<Arc<Mutex<Bar>>, CompassAppError> {
    let pb = Bar::builder()
        .total(total)
        .animation("fillup")
        .desc(desc)
        .build()
        .map_err(|e| {
            CompassAppError::InternalError(format!("could not build progress bar: {e}"))
        })?;
    Ok(Arc::new(Mutex::new(pb)))
}
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
            queries.iter_mut().try_for_each(|q| {
                let mut response = run_single_query(q, output_plugins, search_app)?;
                if let Ok(mut pb_local) = pb.lock() {
                    let _ = pb_local.update(1);
                }
                response_writer.write_response(&mut response)?;
                Ok(())
            })
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

/// Runs a batch of queries in parallel, updating a progress bar.
///
/// # Arguments
///
/// * `queries` - List of queries to process
/// * `parallelism` - Number of parallel threads to use
/// * `pb_desc` - Description for the progress bar
/// * `f` - Function to apply to each query
///
/// # Returns
///
/// A list of results from the function application
pub fn run_batch<F>(
    queries: &[Value],
    parallelism: usize,
    pb_desc: &str,
    f: F,
) -> Result<Vec<Value>, CompassAppError>
where
    F: Fn(&Value) -> Value + Sync + Send,
{
    if queries.is_empty() {
        return Ok(vec![]);
    }

    let pb = create_progress_bar(queries.len(), pb_desc)?;

    let tasks_per_thread = queries.len() as f64 / parallelism as f64;
    let chunk_size = std::cmp::max(1, tasks_per_thread.ceil() as usize);

    let results: Vec<Value> = queries
        .par_chunks(chunk_size)
        .flat_map(|chunk| {
            chunk
                .iter()
                .map(|query| {
                    let result = f(query);
                    if let Ok(mut pb_local) = pb.lock() {
                        let _ = kdam::BarExt::update(&mut *pb_local, 1);
                    }
                    result
                })
                .collect::<Vec<_>>()
        })
        .collect();

    eprintln!();
    Ok(results)
}

/// helper function to wrap some lambda with runtime logging
pub fn with_timing<T>(
    name: &str,
    thunk: impl Fn() -> Result<T, CompassAppError>,
) -> Result<T, CompassAppError> {
    let start = Local::now();
    let result = thunk();
    let duration = (Local::now() - start)
        .to_std()
        .map_err(|e| CompassAppError::InternalError(e.to_string()))?;
    log::info!(
        "finished reading {name} with duration {}",
        duration.hhmmss()
    );
    result
}

/// Inner implementation of single path evaluation that returns Result for easier error handling
pub fn run_single_calculate_path(
    query: &Value,
    search_app: &SearchApp,
    output_plugins: &[Arc<dyn OutputPlugin>],
) -> Result<Value, CompassAppError> {
    let edges = query
        .get("path")
        .ok_or_else(|| CompassAppError::InternalError("query missing 'path'".to_string()))?
        .as_array()
        .ok_or_else(|| CompassAppError::InternalError("'path' must be an array".to_string()))?;

    let path = edges
        .iter()
        .map(|v| {
            let edge_id_val = v.get("edge_id").ok_or_else(|| {
                CompassAppError::InternalError("edge object missing 'edge_id'".to_string())
            })?;
            let edge_id = edge_id_val.as_u64().ok_or_else(|| {
                CompassAppError::InternalError("edge_id must be a number".to_string())
            })?;

            let edge_list_id = match v.get("edge_list_id") {
                Some(id_val) => {
                    let id = id_val.as_u64().ok_or_else(|| {
                        CompassAppError::InternalError("edge_list_id must be a number".to_string())
                    })?;
                    EdgeListId(id as usize)
                }
                None => EdgeListId::default(),
            };

            Ok((edge_list_id, EdgeId(edge_id as usize)))
        })
        .collect::<Result<Vec<_>, CompassAppError>>()?;

    let si = search_app.build_search_instance(query)?;
    let start_time = Local::now();

    let edge_traversals = si
        .compute_path(&path)
        .map_err(CompassAppError::SearchFailure)?;

    let end_time = Local::now();
    let runtime = (end_time - start_time)
        .to_std()
        .unwrap_or(std::time::Duration::ZERO);

    let search_app_result = crate::app::search::SearchAppResult {
        routes: vec![edge_traversals],
        trees: vec![],
        search_executed_time: start_time.to_rfc3339(),
        search_runtime: runtime,
        iterations: 0,
        terminated: None,
    };

    let response = apply_output_processing(
        query,
        Ok((search_app_result, si)),
        search_app,
        output_plugins,
    );
    Ok(response)
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
