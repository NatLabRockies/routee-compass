use parquet::errors::ParquetError;

#[derive(thiserror::Error, Debug)]
pub enum OvertureMapsCollectionError {
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
}
