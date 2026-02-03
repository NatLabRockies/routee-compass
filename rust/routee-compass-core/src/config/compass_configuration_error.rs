use crate::{
    algorithm::map_matching::MapMatchingError,
    model::{
        constraint::ConstraintModelError, label::label_model_error::LabelModelError,
        network::NetworkError, traversal::TraversalModelError,
    },
    util::conversion::conversion_error::ConversionError,
};

#[derive(thiserror::Error, Debug)]
pub enum CompassConfigurationError {
    #[error("{0}")]
    UserConfigurationError(String),
    #[error("expected field {0} for {1} not found")]
    ExpectedFieldForComponent(String, String),
    #[error("expected field {0} with type {1} was unable to deserialize")]
    ExpectedFieldWithType(String, String),
    #[error("expected field {0} for component {1} had unrecognized value {2}")]
    ExpectedFieldWithTypeUnrecognized(String, String, String),
    #[error(
        "unknown module '{0}' for component '{1}' provided by configuration, must be one of {2}"
    )]
    UnknownModelNameForComponent(String, String, String),
    #[error(
        r#"
        File '{0}' was not found.
        This file came from field '{1}' for component '{2}'.

        First, make sure this file path is either relative to your config file, 
        or, is provided as an absolute path. 

        Second, make sure the file exists.

        Third, make sure the config key ends with '_input_file(s)' which is a schema requirement
        for the CompassApp config.
        "#
    )]
    FileNotFoundForComponent(String, String, String),
    #[error("could not normalize incoming file {0}")]
    FileNormalizationError(String),
    #[error(
        r#"
        Could not find incoming configuration file '{1}' for key '{0}'

        Tried: 
         - '{1}'
         - '{2}'
        "#
    )]
    FileNormalizationNotFound(String, String, String),
    #[error("{0}")]
    InsertError(String),
    #[error(transparent)]
    GraphError(#[from] NetworkError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    SerdeDeserializationError(#[from] serde_json::Error),
    #[error(transparent)]
    ConversionError(#[from] ConversionError),
    #[error(transparent)]
    TraversalModelError(#[from] TraversalModelError),
    #[error(transparent)]
    ConstraintModelError(#[from] ConstraintModelError),
    #[error(transparent)]
    LabelModelError(#[from] LabelModelError),
    #[error(transparent)]
    MapMatchingError(#[from] MapMatchingError),
}
