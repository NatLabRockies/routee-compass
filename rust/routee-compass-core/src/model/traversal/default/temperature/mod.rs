//! Reads a table of temperature values per edge id. These are recorded
//! directly to the traversal state vector as "leg_temperature" values. If
//! Injects a constant ambient temperature value throughout the trip.
//! This module does not perform per-edge temperature lookups; instead,
//! the same temperature is applied to all edges in the traversal.

mod ambient_temperature_config;
mod temperature_traversal_builder;
mod temperature_traversal_model;
mod temperature_traversal_service;

pub use ambient_temperature_config::AmbientTemperatureConfig;
pub use temperature_traversal_builder::TemperatureTraversalBuilder;
pub use temperature_traversal_model::TemperatureTraversalModel;
pub use temperature_traversal_service::TemperatureTraversalService;
