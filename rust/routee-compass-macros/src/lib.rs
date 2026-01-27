extern crate proc_macro;
extern crate proc_macro_error;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// A procedural macro that generates python bindings for a struct that implements the `CompassAppBindings` trait.
/// This allows custom applications to be exposed to python in a similar manner as the original python compass app.
#[proc_macro_error]
#[proc_macro_attribute]
pub fn pybindings(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let name = &input.ident;

    let expanded = quote! {
        use pyo3::{exceptions::PyException, prelude::*, types::PyType, PyResult};

        #[pyclass]
        #input

        #[pymethods]
        impl #name {
            #[pyo3(signature = (edge_id, edge_list_id=None))]
            fn graph_edge_origin(&self, edge_id: usize, edge_list_id: Option<usize>) -> PyResult<usize> {
                CompassAppBindings::graph_edge_origin(self, edge_list_id, edge_id).map_err(|e| {
                    PyException::new_err(format!(
                        "error retrieving edge origin for edge_id {}: {}",
                        edge_id, e
                    ))
                })
            }
            #[pyo3(signature = (edge_id, edge_list_id=None))]
            fn graph_edge_destination(&self, edge_id: usize, edge_list_id: Option<usize>) -> PyResult<usize> {
                CompassAppBindings::graph_edge_destination(self, edge_list_id, edge_id).map_err(|e| {
                    PyException::new_err(format!(
                        "error retrieving edge destination for edge_id {}: {}",
                        edge_id, e
                    ))
                })
            }
            #[pyo3(signature = (edge_id, edge_list_id=None, distance_unit=None))]
            fn graph_edge_distance(&self, edge_id: usize, edge_list_id: Option<usize>, distance_unit: Option<String>) -> PyResult<f64> {
                CompassAppBindings::graph_edge_distance(self, edge_list_id, edge_id, distance_unit).map_err(|e| {
                    PyException::new_err(format!(
                        "error retrieving edge distance for edge_id {}: {}",
                        edge_id, e
                    ))
                })
            }
            fn graph_get_out_edge_ids(&self, vertex_id: usize) -> Vec<(usize, usize)> {
                CompassAppBindings::graph_get_out_edge_ids(self, vertex_id)
            }
            fn graph_get_in_edge_ids(&self, vertex_id: usize) -> Vec<(usize, usize)> {
                CompassAppBindings::graph_get_in_edge_ids(self, vertex_id)
            }
            #[staticmethod]
            pub fn _from_config_toml_string(
                config_string: String,
                original_file_path: String,
            ) -> PyResult<#name> {
                CompassAppBindings::from_config_toml_string(config_string, original_file_path).map_err(
                    |e| {
                        PyException::new_err(format!(
                            "Error while creating CompassApp from config toml string: {}",
                            e
                        ))
                    },
                )
            }

            #[pyo3(signature = (queries, config=None))]
            pub fn _run_queries(
                &self,
                queries: Vec<String>,
                config: Option<String>,
            ) -> PyResult<Vec<String>> {
                CompassAppBindings::run_queries(self, queries, config)
                    .map_err(|e| PyException::new_err(format!("Error while running queries: {}", e)))
            }
            #[pyo3(signature = (queries))]
            pub fn _map_match(
                &self,
                queries: Vec<String>,
            ) -> PyResult<Vec<String>> {
                CompassAppBindings::map_match(self, queries)
                    .map_err(|e| PyException::new_err(format!("Error while running map matching: {}", e)))
            }
        }
    };

    TokenStream::from(expanded)
}
