use super::mapping::file_mapping::FileMapping;
use crate::app::compass::CompassAppError;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::json::{reader::infer_json_schema, ReaderBuilder};
use indexmap::IndexMap;
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;
use serde_json::json;
use std::fs::File;
use std::io::{Cursor, Seek, SeekFrom};
use std::sync::Arc;

pub struct ParquetPartitionWriter {
    filename: String,
    writer: Option<ArrowWriter<File>>,
    buffer: Vec<serde_json::Value>,
    buffer_limit: usize,
    schema: Option<arrow::datatypes::SchemaRef>,
    mapping: Option<IndexMap<String, FileMapping>>,
}

impl ParquetPartitionWriter {
    pub fn new(
        filename: String,
        buffer_limit: usize,
        mapping: Option<IndexMap<String, FileMapping>>,
    ) -> Self {
        Self {
            filename,
            writer: None,
            buffer: Vec::new(),
            buffer_limit,
            schema: None,
            mapping,
        }
    }

    pub fn write_record(&mut self, record: serde_json::Value) -> Result<(), CompassAppError> {
        let record_to_write = if let Some(mapping) = &self.mapping {
            let mut mapped_record = serde_json::Map::new();
            for (key, value) in mapping {
                match value.apply_mapping(&record) {
                    Ok(val) => {
                        mapped_record.insert(key.clone(), val);
                    }
                    Err(msg) => {
                        mapped_record.insert("error".to_string(), json!(msg));
                    }
                }
            }
            serde_json::Value::Object(mapped_record)
        } else {
            record
        };

        self.buffer.push(record_to_write);
        if self.buffer.len() >= self.buffer_limit {
            self.flush_buffer()?;
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<String, CompassAppError> {
        self.flush_buffer()?;
        if let Some(writer) = self.writer.take() {
            writer.close().map_err(|e| {
                CompassAppError::InternalError(format!(
                    "failed to close parquet file {}: {}",
                    self.filename, e
                ))
            })?;
        }
        Ok(self.filename.clone())
    }

    fn flush_buffer(&mut self) -> Result<(), CompassAppError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // 1. Serialize buffer to JSON (newline delimited) in memory
        let mut json_bytes = Vec::new();
        for val in &self.buffer {
            serde_json::to_writer(&mut json_bytes, val).map_err(|e| {
                CompassAppError::InternalError(format!(
                    "Failed to serialize json for parquet: {}",
                    e
                ))
            })?;
            json_bytes.push(b'\n');
        }

        // 2. Create RecordBatch using Arrow JSON Reader
        let mut cursor = Cursor::new(json_bytes);

        // Resolve schema: use existing or infer
        let schema = if let Some(s) = &self.schema {
            s.clone()
        } else {
            let (inferred_schema, _records_read) =
                infer_json_schema(&mut cursor, Some(self.buffer.len())).map_err(|e| {
                    CompassAppError::InternalError(format!(
                        "Failed to infer schema from json: {}",
                        e
                    ))
                })?;
            // Reset cursor after inference
            cursor
                .seek(SeekFrom::Start(0))
                .map_err(|e| CompassAppError::InternalError(e.to_string()))?;

            let final_schema = if let Some(mapping) = &self.mapping {
                apply_overrides(inferred_schema, mapping)?
            } else {
                inferred_schema
            };

            let s = Arc::new(final_schema);
            self.schema = Some(s.clone());
            s
        };

        let builder = ReaderBuilder::new(schema);
        let mut reader = builder.build(cursor).map_err(|e| {
            CompassAppError::InternalError(format!("Failed to build arrow json reader: {}", e))
        })?;

        // There should be exactly one batch since we provided everything in one cursor
        let batch = reader
            .next()
            .transpose()
            .map_err(|e| {
                CompassAppError::InternalError(format!("Failed to read batch from json: {}", e))
            })?
            .ok_or_else(|| {
                CompassAppError::InternalError("No batch produced from buffer".to_string())
            })?;

        // 3. Initialize Parquet Writer if not exists
        if self.writer.is_none() {
            let file = File::create(&self.filename).map_err(|e| {
                CompassAppError::InternalError(format!(
                    "Failed to create parquet file {}: {}",
                    self.filename, e
                ))
            })?;

            // Capture the schema from the first batch
            if self.schema.is_none() {
                self.schema = Some(batch.schema());
            }

            let props = WriterProperties::builder().build();
            let writer = ArrowWriter::try_new(file, batch.schema(), Some(props)).map_err(|e| {
                CompassAppError::InternalError(format!("Failed to create parquet writer: {}", e))
            })?;
            self.writer = Some(writer);
        }

        // 4. Write batch
        if let Some(writer) = &mut self.writer {
            writer.write(&batch).map_err(|e| {
                CompassAppError::InternalError(format!("Failed to write batch to parquet: {}", e))
            })?;
        }

        self.buffer.clear();
        Ok(())
    }
}

fn apply_overrides(
    schema: Schema,
    mapping: &IndexMap<String, FileMapping>,
) -> Result<Schema, CompassAppError> {
    let mut new_fields = Vec::new();
    for field in schema.fields() {
        let field_name = field.name();
        if let Some(FileMapping::Optional {
            dtype: Some(dtype), ..
        }) = mapping.get(field_name)
        {
            let new_dtype = match dtype.as_str() {
                "string" => DataType::Utf8,
                "float" | "float64" => DataType::Float64,
                "int" | "int64" => DataType::Int64,
                "int32" => DataType::Int32,
                "bool" | "boolean" => DataType::Boolean,
                _ => {
                    return Err(CompassAppError::InternalError(format!(
                        "Unsupported dtype: {}",
                        dtype
                    )))
                }
            };

            // If current type is Null, or we just want to force it
            if field.data_type() == &DataType::Null || field.data_type() != &new_dtype {
                new_fields.push(Arc::new(Field::new(field_name, new_dtype, true)));
                // Always nullable for Optional
                continue;
            }
        }
        new_fields.push(field.clone());
    }
    Ok(Schema::new(new_fields))
}
