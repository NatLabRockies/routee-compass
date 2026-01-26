use super::compass_app_system::CompassAppSystemParameters;
use super::response::response_sink::ResponseSink;
use super::{compass_app_ops as ops, CompassBuilderInventory};
use crate::app::compass::compass_app_config::CompassAppConfig;
use crate::app::compass::response::response_persistence_policy::ResponsePersistencePolicy;
use crate::{
    app::{
        compass::CompassAppError,
        search::{SearchApp, SearchAppResult},
    },
    plugin::{
        input::{input_plugin_ops as in_ops, InputPlugin},
        output::{output_plugin_ops as out_ops, OutputPlugin},
    },
};
use chrono::Local;
use itertools::Itertools;
use kdam::{Bar, BarExt};
use rayon::{current_num_threads, prelude::*};
use routee_compass_core::algorithm::search::{SearchAlgorithm, SearchInstance};
use routee_compass_core::config::ConfigJsonExtensions;
use routee_compass_core::model::cost::cost_model_service::CostModelService;
use routee_compass_core::model::map::MapModel;
use routee_compass_core::model::network::Graph;
use routee_compass_core::model::state::StateModel;
use routee_compass_core::util::duration_extension::DurationExtension;
use serde_json::Value;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use crate::app::map_matching::{
    MapMatchingAppError, MapMatchingRequest, MapMatchingResponse, PointMatchResponse, TracePoint,
};
use geo::Point;
use routee_compass_core::algorithm::map_matching::{
    MapMatchingAlgorithm, MapMatchingPoint, MapMatchingResult, MapMatchingTrace,
};

/// Instance of RouteE Compass as an application.
/// When constructed, it holds
///   - the core search application which performs parallel path search
///   - the input plugins for query pre-processing
///   - the output plugins for query post-processing
///
/// A CompassApp instance provides the high-level API for building and
/// running RouteE Compass.
pub struct CompassApp {
    pub search_app: Arc<SearchApp>,
    pub input_plugins: Vec<Arc<dyn InputPlugin>>,
    pub output_plugins: Vec<Arc<dyn OutputPlugin>>,
    pub system_parameters: CompassAppSystemParameters,
    pub map_matching_algorithm: Arc<dyn MapMatchingAlgorithm>,
}

impl TryFrom<&Path> for CompassApp {
    type Error = CompassAppError;

    /// Builds a CompassApp from a configuration filepath, using the default CompassBuilderInventory.
    /// Builds all components such as the DirectedGraph, TraversalModel, and SearchAlgorithm.
    /// Also builds the input and output plugins.
    /// Returns a persistent application that can run user queries in parallel.
    ///
    /// # Arguments
    ///
    /// * `conf_file` - path to a configuration TOML file
    ///
    /// # Returns
    ///
    /// * an instance of [`CompassApp`], or an error if load failed.
    fn try_from(conf_file: &Path) -> Result<Self, Self::Error> {
        let config = CompassAppConfig::try_from(conf_file)?;
        let builder = CompassBuilderInventory::new()?;
        let compass_app = CompassApp::new(&config, &builder)?;
        Ok(compass_app)
    }
}

impl CompassApp {
    /// Builds a CompassApp from configuration and a (possibly customized) CompassBuilderInventory.
    /// Builds all modules such as the DirectedGraph, TraversalModel, and SearchAlgorithm.
    /// Also builds the input and output plugins.
    /// Returns a persistent application that can run user queries in parallel.
    ///
    /// This is the extension API for building [`CompassApp`] instances.
    /// In application, the user becomes responsible for
    ///   -
    ///
    /// # Arguments
    ///
    /// * `config` - deserialized TOML file contents
    /// * `builder` - inventory of Compass components that can be built from the config object
    ///
    /// # Returns
    ///
    /// * an instance of [`CompassApp`], or an error if load failed.
    pub fn new(
        config: &CompassAppConfig,
        builder: &CompassBuilderInventory,
    ) -> Result<Self, CompassAppError> {
        let state_model = match &config.state {
            Some(state_config) => Arc::new(StateModel::new(state_config.clone())),
            None => Arc::new(StateModel::empty()),
        };
        let cost_model_service = CostModelService::try_from(&config.cost)?;
        let label_model_service = builder.build_label_model_service(&config.label)?;
        log::info!("app termination model: {:?}", config.termination);

        // build selected components for search behaviors
        let traversal_model_services = with_timing("traversal models", || {
            config.build_traversal_model_services(builder)
        })?;
        let constraint_model_services = with_timing("constraint models", || {
            config.build_constraint_model_services(builder)
        })?;

        // build graph
        let graph = with_timing("graph", || Ok(Arc::new(Graph::try_from(&config.graph)?)))?;

        let map_model = with_timing("map model", || {
            let mm = MapModel::new(graph.clone(), &config.mapping).map_err(|e| {
                CompassAppError::BuildFailure(format!("unable to load MapModel from config: {e}"))
            })?;
            Ok(Arc::new(mm))
        })?;

        let search_algorithm = SearchAlgorithm::from(&config.algorithm);

        // build search app
        let search_app = Arc::new(SearchApp::new(
            search_algorithm,
            graph,
            map_model,
            state_model,
            traversal_model_services,
            constraint_model_services,
            cost_model_service,
            config.termination.clone(),
            label_model_service,
            config.system.default_edge_list,
        ));

        let input_plugins = with_timing("input plugins", || {
            Ok(builder.build_input_plugins(&config.plugin.input_plugins)?)
        })?;
        let output_plugins = with_timing("output plugins", || {
            Ok(builder.build_output_plugins(&config.plugin.output_plugins)?)
        })?;

        let map_matching_algorithm = with_timing("map matching algorithm", || {
            Ok(builder.build_map_matching_algorithm(&config.map_matching)?)
        })?;

        let app = CompassApp {
            search_app,
            input_plugins,
            output_plugins,
            system_parameters: config.system.clone(),
            map_matching_algorithm,
        };
        Ok(app)
    }

    /// runs a set of queries via this instance of CompassApp. this
    ///   1. processes each input query based on the InputPlugins
    ///   2. runs the search algorithm with each query via SearchApp
    ///   3. processes each output based on the OutputPlugins
    ///   4. returns the JSON response
    ///
    /// only  errors should cause CompassApp to halt. if there are
    /// errors due to the user, they should be propagated along into the output
    /// JSON in an error format along with the request.
    ///
    /// # Arguments
    ///
    /// * `queries` - list of search queries to execute
    /// * `config` - configuration for this run batch which may override default configurations
    ///
    /// # Result
    ///
    /// if
    pub fn run(
        &self,
        queries: &mut Vec<Value>,
        config: Option<&Value>,
    ) -> Result<Vec<Value>, CompassAppError> {
        let override_config_opt: Option<CompassAppSystemParameters> = match config {
            Some(c) => serde_json::from_value(c.clone())?,
            None => None,
        };
        // allow the user to overwrite global configurations for this run
        let parallelism = override_config_opt
            .as_ref()
            .and_then(|c| c.parallelism)
            .or(self.system_parameters.parallelism)
            .unwrap_or(1);

        let response_persistence_policy = override_config_opt
            .as_ref()
            .and_then(|c| c.response_persistence_policy)
            .or(self.system_parameters.response_persistence_policy)
            .unwrap_or_default();

        let response_output_policy = override_config_opt
            .as_ref()
            .and_then(|c| c.response_output_policy.clone())
            .or(self.system_parameters.response_output_policy.clone())
            .unwrap_or_default();
        let response_writer = response_output_policy.build()?;

        // INPUT PROCESSING

        let input_plugin_result = apply_input_plugins(
            queries,
            &self.input_plugins,
            self.search_app.clone(),
            parallelism,
        )?;
        let (processed_inputs, input_errors) = input_plugin_result;
        let mut load_balanced_inputs =
            ops::apply_load_balancing_policy(processed_inputs, parallelism, 1.0)?;

        log::info!(
            "creating {} parallel batches across {} threads to run queries",
            parallelism,
            current_num_threads(),
        );
        let proc_batch_sizes = load_balanced_inputs
            .iter()
            .map(|qs| qs.len())
            .collect::<Vec<_>>();
        log::info!("queries assigned per executor: {proc_batch_sizes:?}");

        // set up search progress bar
        let num_balanced_inputs = load_balanced_inputs
            .iter()
            .flatten()
            .collect::<Vec<_>>()
            .len();
        let search_pb = Bar::builder()
            .total(num_balanced_inputs)
            .animation("fillup")
            .desc("search")
            .build()
            .map_err(|e| {
                CompassAppError::InternalError(format!("could not build progress bar: {e}"))
            })?;
        let search_pb_shared = Arc::new(Mutex::new(search_pb));

        // run parallel searches as organized by the (optional) load balancing policy
        // across a thread pool managed by rayon
        let run_query_result = match response_persistence_policy {
            ResponsePersistencePolicy::PersistResponseInMemory => run_batch_with_responses(
                &mut load_balanced_inputs,
                &self.output_plugins,
                &self.search_app,
                &response_writer,
                search_pb_shared,
            )?,
            ResponsePersistencePolicy::DiscardResponseFromMemory => run_batch_without_responses(
                &mut load_balanced_inputs,
                &self.output_plugins,
                &self.search_app,
                &response_writer,
                search_pb_shared,
            )?,
        };
        eprintln!();
        response_writer.close()?;

        // combine successful runs along with any error rows for response
        let run_result = run_query_result
            // .chain(mapped_errors)
            .chain(input_errors)
            .collect();
        Ok(run_result)
    }
}

/// executes the input plugins on each query, returning all
/// successful mappings (left) and mapping errors (right) as the pair
/// (left, right). errors are already serialized into JSON.
fn apply_input_plugins(
    queries: &mut Vec<Value>,
    input_plugins: &Vec<Arc<dyn InputPlugin>>,
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
impl CompassApp {
    /// Runs map matching on a request, returning a JSON response.
    ///
    /// # Arguments
    ///
    /// * `request` - The map matching request containing the GPS trace
    ///
    /// # Returns
    ///
    /// A `MapMatchingResponse` with matched edges and the inferred path.
    pub fn map_match(
        &self,
        request: MapMatchingRequest,
    ) -> Result<MapMatchingResponse, MapMatchingAppError> {
        // Validate the request
        request
            .validate()
            .map_err(MapMatchingAppError::InvalidRequest)?;

        // Convert request to internal trace format
        let trace = self.convert_request_to_trace(&request);

        // Build a search instance for this query
        // Using an empty JSON object as we don't need query-specific configuration
        let query = serde_json::json!({});
        let search_instance = self
            .search_app
            .build_search_instance(&query)
            .map_err(|e| MapMatchingAppError::BuildFailure(e.to_string()))?;

        // Run the algorithm
        let result = self
            .map_matching_algorithm
            .match_trace(&trace, &search_instance)?;

        // Convert result to response format
        Ok(self.convert_result_to_response(result))
    }

    /// Converts a JSON request to the internal trace format.
    fn convert_request_to_trace(&self, request: &MapMatchingRequest) -> MapMatchingTrace {
        let points: Vec<MapMatchingPoint> = request
            .trace
            .iter()
            .map(|tp| self.convert_trace_point(tp))
            .collect();
        MapMatchingTrace::new(points)
    }

    /// Converts a single trace point from the request format.
    fn convert_trace_point(&self, point: &TracePoint) -> MapMatchingPoint {
        let coord = Point::new(point.x as f32, point.y as f32);
        match &point.timestamp {
            Some(ts) => MapMatchingPoint::with_timestamp(coord, ts.clone()),
            None => MapMatchingPoint::new(coord),
        }
    }

    /// Converts the internal result to the response format.
    fn convert_result_to_response(&self, result: MapMatchingResult) -> MapMatchingResponse {
        let point_matches: Vec<PointMatchResponse> = result
            .point_matches
            .into_iter()
            .map(|pm| {
                PointMatchResponse::new(pm.edge_list_id.0, pm.edge_id.0 as u64, pm.distance_to_edge)
            })
            .collect();

        let matched_path: Vec<(usize, u64)> = result
            .matched_path
            .into_iter()
            .map(|(list_id, edge_id)| (list_id.0, edge_id.0 as u64))
            .collect();

        MapMatchingResponse::new(point_matches, matched_path)
    }
}

/// helper function to wrap some lambda with runtime logging
fn with_timing<T>(
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

#[cfg(test)]
mod tests {
    use super::CompassApp;
    use crate::app::compass::CompassAppError;
    use routee_compass_core::config::CompassConfigurationError;
    use std::path::PathBuf;

    #[test]
    fn test_e2e_dist_speed_time_traversal() {
        // let cwd_str = match std::env::current_dir() {
        //     Ok(cwd_path) => String::from(cwd_path.to_str().unwrap_or("<unknown>")),
        //     _ => String::from("<unknown>"),
        // };
        // eprintln!("cwd           : {}", cwd_str);
        // eprintln!("Cargo.toml dir: {}", env!("CARGO_MANIFEST_DIR"));

        // rust runs test and debug at different locations, which breaks the URLs
        // written in the referenced TOML files. here's a quick fix
        // turnaround that doesn't leak into anyone's VS Code settings.json files
        // see https://github.com/rust-lang/rust-analyzer/issues/4705 for discussion
        let conf_file_test = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("app")
            .join("compass")
            .join("test")
            .join("speeds_test")
            .join("speeds_test.toml");

        let conf_file_debug = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("app")
            .join("compass")
            .join("test")
            .join("speeds_test")
            .join("speeds_debug.toml");

        println!(
            "attempting to load '{}'",
            conf_file_test.to_str().unwrap_or_default()
        );
        let app = match CompassApp::try_from(conf_file_test.as_path()) {
            Ok(a) => Ok(a),
            Err(CompassAppError::CompassConfigurationError(
                CompassConfigurationError::FileNormalizationNotFound(..),
            )) => {
                // could just be the run location, depending on the environment/runner/IDE
                // try the alternative configuration that runs from the root directory
                println!(
                    "attempting to load '{}'",
                    conf_file_debug.to_str().unwrap_or_default()
                );
                CompassApp::try_from(conf_file_debug.as_path())
            }
            Err(other) => panic!("{}", other),
        }
        .unwrap();
        let query = serde_json::json!({
            "origin_vertex": 0,
            "destination_vertex": 2
        });
        let mut queries = vec![query];
        let result = app.run(&mut queries, None).expect("run failed");
        assert_eq!(result.len(), 1, "expected one result");
        eprintln!("{}", serde_json::to_string_pretty(&result).unwrap());
        let route_0 = result[0].get("route").expect("result has no route");
        let path_0 = route_0.get("path").expect("result route has no path");

        // path [1] is distance-optimal; path [0, 2] is time-optimal
        let expected_path = serde_json::json!(vec![0, 2]);
        assert_eq!(path_0, &expected_path);
    }

    // #[test]
    // fn test_energy() {
    //     // rust runs test and debug at different locations, which breaks the URLs
    //     // written in the referenced TOML files. here's a quick fix
    //     // turnaround that doesn't leak into anyone's VS Code settings.json files
    //     // see https://github.com/rust-lang/rust-analyzer/issues/4705 for discussion
    //     let conf_file_test = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    //         .join("src")
    //         .join("app")
    //         .join("compass")
    //         .join("test")
    //         .join("energy_test")
    //         .join("energy_test.toml");

    //     let conf_file_debug = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    //         .join("src")
    //         .join("app")
    //         .join("compass")
    //         .join("test")
    //         .join("energy_test")
    //         .join("energy_debug.toml");

    //     let app = CompassApp::try_from(conf_file_test)
    //         .or(CompassApp::try_from(conf_file_debug))
    //         .unwrap();
    //     let query = serde_json::json!({
    //         "origin_vertex": 0,
    //         "destination_vertex": 2
    //     });
    //     let result = app.run(vec![query]).unwrap();
    //     let edge_ids = result[0].get("edge_id_list").unwrap();
    //     // path [1] is distance-optimal; path [0, 2] is time-optimal
    //     let expected = serde_json::json!(vec![0, 2]);
    //     assert_eq!(edge_ids, &expected);
    // }
}
