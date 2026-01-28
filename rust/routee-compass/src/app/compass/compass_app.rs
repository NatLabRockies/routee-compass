use super::compass_app_system::CompassAppSystemParameters;

use super::{
    compass_app_ops as ops, compass_map_matching as map_matching_ops, CompassBuilderInventory,
};
use crate::app::compass::compass_app_config::CompassAppConfig;
use crate::app::compass::response::response_persistence_policy::ResponsePersistencePolicy;
use crate::{
    app::{compass::CompassAppError, search::SearchApp},
    plugin::{input::InputPlugin, output::OutputPlugin},
};
use chrono::Local;

use kdam::Bar;
use rayon::current_num_threads;
use routee_compass_core::algorithm::search::SearchAlgorithm;
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

use crate::app::map_matching::{MapMatchingAppError, MapMatchingRequest};
use routee_compass_core::algorithm::map_matching::MapMatchingAlgorithm;

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

        let input_plugin_result = ops::apply_input_plugins(
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
            ResponsePersistencePolicy::PersistResponseInMemory => ops::run_batch_with_responses(
                &mut load_balanced_inputs,
                &self.output_plugins,
                &self.search_app,
                &response_writer,
                search_pb_shared,
            )?,
            ResponsePersistencePolicy::DiscardResponseFromMemory => {
                ops::run_batch_without_responses(
                    &mut load_balanced_inputs,
                    &self.output_plugins,
                    &self.search_app,
                    &response_writer,
                    search_pb_shared,
                )?
            }
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

impl CompassApp {
    /// Runs map matching on a batch of requests, returning a JSON response for each.
    ///
    /// # Arguments
    ///
    /// * `queries` - List of map matching requests as JSON values
    ///
    /// # Returns
    ///
    /// A list of `MapMatchingResponse` objects as JSON values.
    pub fn map_match(&self, queries: &Vec<Value>) -> Result<Vec<Value>, CompassAppError> {
        let mut results = Vec::with_capacity(queries.len());

        for query in queries {
            let request: MapMatchingRequest = serde_json::from_value(query.clone())?;
            // Validate the request
            request
                .validate()
                .map_err(MapMatchingAppError::InvalidRequest)?;

            // Convert request to internal trace format
            let trace = map_matching_ops::convert_request_to_trace(&request);

            // Build a search instance for this query
            let mut query_config = self.map_matching_algorithm.search_parameters();
            if let Some(search_overrides) = &request.search_parameters {
                if let Some(obj) = search_overrides.as_object() {
                    for (k, v) in obj {
                        query_config[k] = v.clone();
                    }
                }
            }
            let search_instance = self
                .search_app
                .build_search_instance(&query_config)
                .map_err(|e| MapMatchingAppError::BuildFailure(e.to_string()))?;

            // Run the algorithm
            let result = self
                .map_matching_algorithm
                .match_trace(&trace, &search_instance)
                .map_err(|e| MapMatchingAppError::AlgorithmError { source: e })?;

            // Convert result to response format
            let response = map_matching_ops::convert_result_to_response(
                result,
                &self.search_app.map_model,
                request.include_geometry,
            );
            let response_json = serde_json::to_value(response)?;
            results.push(response_json);
        }

        Ok(results)
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
            "destination_vertex": 2,
            "include_geometry": true
        });
        let mut queries = vec![query];
        let result = app.run(&mut queries, None).expect("run failed");
        assert_eq!(result.len(), 1, "expected one result");
        let route_0 = result[0].get("route").expect("result has no route");
        let path_0 = route_0.get("path").expect("result route has no path");
        let geometry = route_0
            .get("geometry")
            .expect("result route has no geometry")
            .as_array()
            .expect("geometry should be an array");
        assert!(!geometry.is_empty(), "Geometry should not be empty");
        // Verify it's a valid linestring (not empty)
        let first_linestring = geometry[0]
            .as_array()
            .expect("first linestring should be an array of points");
        assert!(
            !first_linestring.is_empty(),
            "Linestring should not be empty"
        );
        let first_point = first_linestring[0].as_object().unwrap();
        assert!(first_point.contains_key("x"));
        assert!(first_point.contains_key("y"));

        // path [1] is distance-optimal; path [0, 2] is time-optimal
        let expected_path = serde_json::json!(vec![0, 2]);
        assert_eq!(path_0, &expected_path);
    }
}
