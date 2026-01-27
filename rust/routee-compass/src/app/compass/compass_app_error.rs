use super::CompassComponentError;
use crate::app::map_matching::MapMatchingAppError;
use crate::plugin::{input::InputPluginError, output::OutputPluginError, PluginError};
use config::ConfigError;
use routee_compass_core::config::CompassConfigurationError;
use routee_compass_core::{
    algorithm::search::SearchError,
    model::{
        constraint::ConstraintModelError, cost::CostModelError, map::MapError,
        network::NetworkError, state::StateModelError, termination::TerminationModelError,
        traversal::TraversalModelError,
    },
};

#[derive(thiserror::Error, Debug)]
pub enum CompassAppError {
    // TOP-LEVEL FAILURES
    //   errors observed during build and run of compass that are not due to the search modules
    #[error("failure building compass app: {0}")]
    BuildFailure(String),
    #[error("failure while running app: {0}")]
    CompassFailure(String),
    #[error("internal error: {0}")]
    InternalError(String),
    #[error("error accessing shared read-only dataset: {0}")]
    ReadOnlyPoisonError(String),

    // TRANSPARENT MODULE FAILURES
    //   failures from these modules are detailed enough to get surfaced directly to the user
    #[error(transparent)]
    ConfigFailure(#[from] ConfigError),
    #[error(transparent)]
    CompassConfigurationError(#[from] CompassConfigurationError),
    #[error(transparent)]
    CompassComponentError(#[from] CompassComponentError),
    #[error(transparent)]
    SearchFailure(#[from] SearchError),
    #[error(transparent)]
    PluginError(#[from] PluginError),
    #[error(transparent)]
    InputPluginFailure(#[from] InputPluginError),
    #[error(transparent)]
    OutputPluginFailure(#[from] OutputPluginError),
    #[error(transparent)]
    MapMatchingFailure(#[from] MapMatchingAppError),

    // CONTEXTUALIZED MODULE FAILURES
    //   failures from these modules are happening outside of the context of running the search,
    //   which is clarified for the user and may help direct where to look to solve the problem.
    #[error("While interacting with the map model outside of the context of search, an error occurred. Source: {source}")]
    MappingFailure {
        #[from]
        source: MapError,
    },
    #[error("While interacting with the state model outside of the context of search, an error occurred. Source: {source}")]
    StateFailure {
        #[from]
        source: StateModelError,
    },
    #[error("While interacting with the network model outside of the context of search, an error occurred. Source: {source}")]
    NetworkFailure {
        #[from]
        source: NetworkError,
    },
    #[error("While interacting with the termination model outside of the context of search, an error occurred. Source: {source}")]
    TerminationModelFailure {
        #[from]
        source: TerminationModelError,
    },
    #[error("While interacting with the traversal model outside of the context of search, an error occurred. Source: {source}")]
    TraversalModelFailure {
        #[from]
        source: TraversalModelError,
    },
    #[error("While interacting with the constraint model outside of the context of search, an error occurred. Source: {source}")]
    ConstraintModelFailure {
        #[from]
        source: ConstraintModelError,
    },
    #[error("While interacting with the cost model outside of the context of search, an error occurred. Source: {source}")]
    CostFailure {
        #[from]
        source: CostModelError,
    },
    #[error("failure due to JSON: {source}")]
    JsonError {
        #[from]
        source: serde_json::Error,
    },
}
