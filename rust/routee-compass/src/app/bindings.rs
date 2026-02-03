use super::compass::{CompassApp, CompassAppError};
use crate::app::search::SearchAppGraphOps;
use itertools::Itertools;
use routee_compass_core::{
    algorithm::search::Direction,
    model::{
        network::{EdgeId, EdgeListId, VertexId},
        unit::DistanceUnit,
    },
};
use std::str::FromStr;

/// Defines the interface for exposing the application via a set of language bindings using
/// standard types for easy conversion between languages.
///
/// Most of these functions are implemented in the default implementation but can be overridden
/// to provide custom behavior.
///
/// This also provides a way to build the compass app from a custom builder with external
/// models injected at build time.
///
/// # Example
///
/// Say we want to build a custom python compass app with a custom traversal model that is not
/// provided by the default compass app. We can do this by implementing the `CompassAppBindings` trait
/// and providing a custom implementation for the `from_config_toml_string` function.
///
/// ```
/// use routee_compass::app::bindings::CompassAppBindings;
/// use routee_compass::app::compass::{CompassApp, CompassAppError, CompassAppConfig, CompassBuilderInventory};
/// use config::FileFormat;
///
/// //#[pybindings]
/// pub struct CustomAppPy {
///     app: CompassApp,
/// }
///
/// impl CompassAppBindings for CustomAppPy {
///     fn from_config_toml_string(
///         config_string: String,
///         original_file_path: String,
///     ) -> Result<Self, CompassAppError>
///     where
///         Self: Sized,
///     {
///         let mut builder = CompassBuilderInventory::new()?;
///
///         // inject custom traversal model here like:
///
///         // my_custom_traversal_model_builder = MyCustomTraversalModelBuilder::new();
///         // builder.add_traversal_model("my_custom_model", Rc::new(my_custom_traversal_model));
///
///         let config = CompassAppConfig::from_str(&config_string, &original_file_path, FileFormat::Toml)?;
///         let app = CompassApp::new(&config, &builder)?;
///         Ok(CustomAppPy { app })
///     }
///     fn app(&self) -> &CompassApp {
///         &self.app
///     }
/// }
/// ```
pub trait CompassAppBindings {
    // Functions to be implemented

    /// Build the compass app from a toml string
    ///
    /// # Arguments
    /// * `config_string` - the toml string containing the configuration
    /// * `original_file_path` - the original file path of the toml file
    ///
    /// # Returns
    /// * The compass app wrapper
    fn from_config_toml_string(
        config_string: String,
        original_file_path: String,
    ) -> Result<Self, CompassAppError>
    where
        Self: Sized;

    /// Get the compass app
    ///
    /// # Returns
    /// * The compass app internal to the binding wrapper
    fn app(&self) -> &CompassApp;

    // Default functions

    /// Get the origin vertex of an edge
    ///
    /// # Arguments
    /// * `edge_list_id` - edge list to use, defaults to edge list zero
    /// * `edge_id` - the id of the edge
    ///
    /// # Returns
    /// * the id of the origin vertex
    fn graph_edge_origin(
        &self,
        edge_list_id: Option<usize>,
        edge_id: usize,
    ) -> Result<usize, CompassAppError> {
        let edge_list_id = EdgeListId(edge_list_id.unwrap_or_default());
        let edge_id_internal = EdgeId(edge_id);
        self.app()
            .search_app
            .get_edge_origin(&edge_list_id, &edge_id_internal)
            .map(|o| o.0)
    }

    /// Get the destination vertex of an edge
    ///
    /// # Arguments
    /// * `edge_list_id` - edge list to use, defaults to edge list zero
    /// * `edge_id` - the id of the edge
    ///
    /// # Returns
    /// * the id of the destination vertex
    fn graph_edge_destination(
        &self,
        edge_list_id: Option<usize>,
        edge_id: usize,
    ) -> Result<usize, CompassAppError> {
        let edge_list_id = EdgeListId(edge_list_id.unwrap_or_default());
        let edge_id_internal = EdgeId(edge_id);
        self.app()
            .search_app
            .get_edge_destination(&edge_list_id, &edge_id_internal)
            .map(|o| o.0)
    }

    /// Get the distance of an edge
    ///
    /// # Arguments
    /// * `edge_list_id` - edge list to use, defaults to edge list zero
    /// * `edge_id` - the id of the edge
    /// * `distance_unit` - the distance unit to use. If not provided, the default distance unit is meters
    ///
    /// # Returns
    /// * the distance of the edge in the specified distance unit
    fn graph_edge_distance(
        &self,
        edge_list_id: Option<usize>,
        edge_id: usize,
        distance_unit: Option<String>,
    ) -> Result<f64, CompassAppError> {
        let edge_list_id = EdgeListId(edge_list_id.unwrap_or_default());
        let du_internal: Option<DistanceUnit> = match distance_unit {
            Some(du_str) => {
                let du = DistanceUnit::from_str(du_str.as_str()).map_err(|_| {
                    CompassAppError::InternalError(format!(
                        "could not deserialize distance unit '{du_str}'"
                    ))
                })?;

                Some(du)
            }

            None => None,
        };
        let edge_id_internal = EdgeId(edge_id);
        let edge_distance_internal = self
            .app()
            .search_app
            .get_edge_distance(&edge_list_id, &edge_id_internal)?;
        match du_internal {
            Some(du) => Ok(du.from_uom(edge_distance_internal)),
            None => Ok(DistanceUnit::Meters.from_uom(edge_distance_internal)),
        }
    }

    /// Get the ids of the edges incident to a vertex in the forward direction
    ///
    /// # Arguments
    /// * `vertex_id` - the id of the vertex
    ///
    /// # Returns
    /// * the edge list ids and edge ids of the edges incident to the vertex in the forward direction
    fn graph_get_out_edge_ids(&self, vertex_id: usize) -> Vec<(usize, usize)> {
        let vertex_id_internal = VertexId(vertex_id);
        self.app()
            .search_app
            .get_incident_edge_ids(&vertex_id_internal, &Direction::Forward)
            .into_iter()
            .map(|e| (e.0 .0, e.1 .0))
            .collect_vec()
    }

    /// Get the ids of the edges incident to a vertex in the reverse direction
    ///
    /// # Arguments
    /// * `vertex_id` - the id of the vertex
    ///
    /// # Returns
    /// * the edge list ids and edge ids of the edges incident to the vertex in the reverse direction
    fn graph_get_in_edge_ids(&self, vertex_id: usize) -> Vec<(usize, usize)> {
        let vertex_id_internal = VertexId(vertex_id);
        self.app()
            .search_app
            .get_incident_edge_ids(&vertex_id_internal, &Direction::Reverse)
            // .map(|es| es.iter().map(|e| e.0).collect())
            .into_iter()
            .map(|e| (e.0 .0, e.1 .0))
            .collect_vec()
    }

    /// Runs a set of queries and returns the results
    ///
    /// # Arguments
    /// * `queries` - a list of queries to run as json strings
    ///
    /// # Returns
    /// * a list of json strings containing the results of the queries
    fn run_queries(
        &self,
        queries: Vec<String>,
        config: Option<String>,
    ) -> Result<Vec<String>, CompassAppError> {
        let config_inner: Option<serde_json::Value> = match config {
            Some(c) => {
                let c_serde: serde_json::Value = serde_json::from_str(&c)?;
                Some(c_serde)
            }
            None => None,
        };

        let mut json_queries = queries
            .iter()
            .map(|q| serde_json::from_str(q))
            .collect::<Result<Vec<serde_json::Value>, serde_json::Error>>()?;

        let results = self.app().run(&mut json_queries, config_inner.as_ref())?;

        let string_results: Vec<String> = results.iter().map(|r| r.to_string()).collect();
        Ok(string_results)
    }

    /// Runs map matching on a batch of requests
    ///
    /// # Arguments
    /// * `queries` - a list of queries to run as json strings
    ///
    /// # Returns
    /// * a list of json strings containing the results of the queries
    fn map_match(&self, queries: Vec<String>) -> Result<Vec<String>, CompassAppError> {
        let json_queries = queries
            .iter()
            .map(|q| serde_json::from_str(q))
            .collect::<Result<Vec<serde_json::Value>, serde_json::Error>>()?;

        let results = self.app().map_match(&json_queries)?;

        let string_results: Vec<String> = results.iter().map(|r| r.to_string()).collect();
        Ok(string_results)
    }
}
