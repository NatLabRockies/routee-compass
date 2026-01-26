use std::{path::Path, sync::Arc};

use config::Config;
use routee_compass_core::{
    algorithm::search::SearchAlgorithmConfig,
    config::{ConfigJsonExtensions, OneOrMany},
    model::{
        constraint::ConstraintModelService, cost::CostModelConfig, map::MapModelConfig,
        network::GraphConfig, state::StateVariableConfig, termination::TerminationModel,
        traversal::TraversalModelService,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    app::compass::{CompassAppError, CompassAppSystemParameters, CompassBuilderInventory},
    plugin::PluginConfig,
};

/// high-level application configuration that orchestrates together
/// configuration requirements for the various components making up a
/// [`CompassApp`].
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CompassAppConfig {
    pub algorithm: SearchAlgorithmConfig,
    pub state: Option<Vec<(String, StateVariableConfig)>>,
    pub cost: CostModelConfig,
    pub label: Value,
    pub mapping: MapModelConfig,
    pub graph: GraphConfig,
    /// section containing a single search config or an array of search configs (OneOrMany).
    pub search: OneOrMany<SearchConfig>,
    pub plugin: PluginConfig,
    pub termination: TerminationModel,
    pub system: CompassAppSystemParameters,
    pub map_matching: Value,
}

/// sub-section of [`CompassAppConfig`] where the [`TraversalModelService`], [`AccessModelService`], and [`ConstraintModelService`] components
/// for an [`EdgeList`] are specified.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SearchConfig {
    pub traversal: Value,
    pub constraint: Value,
}

impl CompassAppConfig {
    /// reads a stringified configuration file with provided format and constructs a [`CompassAppConfig`]
    pub fn from_str(
        config: &str,
        config_path: &str,
        format: config::FileFormat,
    ) -> Result<CompassAppConfig, CompassAppError> {
        let default_config = config::File::from_str(
            include_str!("config.default.toml"),
            config::FileFormat::Toml,
        );

        let user_config = config::File::from_str(config, format);

        let config = Config::builder()
            .add_source(default_config)
            .add_source(user_config)
            .build()?;

        let config_json = config
            .clone()
            .try_deserialize::<serde_json::Value>()?
            .normalize_file_paths(Path::new(config_path), None)?;
        let compass_config: CompassAppConfig = serde_json::from_value(config_json)?;

        Ok(compass_config)
    }
}

impl TryFrom<&Path> for CompassAppConfig {
    type Error = CompassAppError;

    fn try_from(config_path: &Path) -> Result<Self, Self::Error> {
        let default_config = config::File::from_str(
            include_str!("config.default.toml"),
            config::FileFormat::Toml,
        );

        let config = Config::builder()
            .add_source(default_config)
            .add_source(config::File::from(config_path))
            .build()?;

        let config_json = config
            .clone()
            .try_deserialize::<serde_json::Value>()?
            .normalize_file_paths(config_path, None)?;
        let compass_config: CompassAppConfig =
            serde_json::from_value(config_json).map_err(|e| {
                let filename = config_path.to_str().unwrap_or("<config path>");
                CompassAppError::BuildFailure(format!("while reading {filename}: {e}"))
            })?;

        Ok(compass_config)
    }
}

impl CompassAppConfig {
    /// Returns a pretty-printed JSON representation of this config.
    /// Useful for debugging and logging the exact configuration being used.
    pub fn to_pretty_string(&self) -> Result<String, CompassAppError> {
        serde_json::to_string_pretty(self).map_err(|e| {
            CompassAppError::BuildFailure(format!("Failed to serialize config: {}", e))
        })
    }

    pub fn build_traversal_model_services(
        &self,
        builders: &CompassBuilderInventory,
    ) -> Result<Vec<Arc<dyn TraversalModelService>>, CompassAppError> {
        let result = self
            .search
            .iter()
            .map(|el| builders.build_traversal_model_service(&el.traversal))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    }

    pub fn build_constraint_model_services(
        &self,
        builders: &CompassBuilderInventory,
    ) -> Result<Vec<Arc<dyn ConstraintModelService>>, CompassAppError> {
        let result = self
            .search
            .iter()
            .map(|el| builders.build_constraint_model_service(&el.constraint))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    }
}
