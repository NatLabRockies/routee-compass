use std::path::PathBuf;

use parquet::errors::ParquetError;

#[derive(thiserror::Error, Debug)]
pub enum OvertureMapsCollectionError {
    #[error("Invalid input: {0}")]
    InvalidUserInput(String),
    #[error("Failed to connect to S3 Bucket: {0}")]
    ConnectionError(String),
    #[error("Failed to acquire Metadata: {0}")]
    MetadataError(String),
    #[error("Failed to create ArrowBuilder instance: {source}")]
    ArrowReaderError { source: ParquetError },
    #[error("Failed to create Parquet Stream instance: {source}")]
    ParquetRecordBatchStreamError { source: ParquetError },
    #[error("Failed to retrieve Record Batch from source: {source}")]
    RecordBatchRetrievalError { source: ParquetError },
    #[error("Failed to deserialize RecordBatch into native type record: {0}")]
    DeserializeError(String),
    #[error("Failed to deserialize general OvertureRecord type into specific record type: {0}")]
    DeserializeTypeError(String),
    #[error("Failed to get a valid response from URL: {0}")]
    TaxonomyLoadingError(String),
    #[error("Failed to deserialize CSV row into Taxonomy record: {0}")]
    TaxonomyDeserializingError(String),
    #[error("Failed to filter predicate column cast to correct type: {0}")]
    PredicateCastingError(String),
    #[error("Failed to find predicate column in schema: {0}")]
    PredicateColumnNotFoundError(String),
    #[error("Error creating a runtime to handle async code: {0}")]
    TokioError(String),
    #[error("Group Mapping operation Failed: {0}")]
    GroupMappingError(String),
    #[error("Processing records into opportunities failed: {0}")]
    ProcessingError(String),
    #[error("Serializing record into compass format failed: {0}")]
    SerializationError(String),
    #[error("Segment connectors vector is invalid or not specified: {0}")]
    InvalidSegmentConnectors(String),
    #[error("linear reference {0} must be in range [0, 1]")]
    InvalidLinearReference(f64),
    #[error("Invalid or empty geometry: {0}")]
    InvalidGeometry(String),
    #[error("Error writing to csv: {0}")]
    CsvWriteError(String),
    #[error("Error reading from '{path}': {message}")]
    ReadError { path: PathBuf, message: String },
    #[error("Error writing to '{path}': {message}")]
    WriteError { path: PathBuf, message: String },
    #[error("Invalid `between` vector: {0}")]
    InvalidBetweenVector(String),
    #[error("Required attribute is None: {0}")]
    MissingAttribute(String),
    #[error("{0}")]
    InternalError(String),
}
