use std::num::ParseIntError;

#[derive(thiserror::Error, Debug)]
pub enum ConversionError {
    #[error("could not decode {0} as {1}")]
    DecoderError(String, String),
    #[error("JSON serialization/deserialization error during value conversion: {source}")]
    SerdeJsonError {
        #[from]
        source: serde_json::Error,
    },
    #[error("regex compilation error during conversion: {0}")]
    RegexError(#[from] regex::Error),
    #[error("failed to parse integer during conversion: {0}")]
    ParseIntError(#[from] ParseIntError),
}
