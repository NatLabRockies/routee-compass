use super::parquet_writer::ParquetPartitionWriter;
use super::response_output_format::ResponseOutputFormat;
use crate::app::compass::response::internal_writer::InternalWriter;
use crate::app::compass::CompassAppError;
use std::io::prelude::*;
use std::sync::{Arc, Mutex};

pub enum ResponseSink {
    None,
    File {
        filename: String,
        file: Arc<Mutex<InternalWriter>>,
        format: ResponseOutputFormat,
        delimiter: Option<String>,
        iterations_per_flush: u64,
        iterations: Arc<Mutex<u64>>,
    },
    Parquet {
        base_filename: String,
        writers: Vec<Mutex<ParquetPartitionWriter>>,
    },
    Combined(Vec<Box<ResponseSink>>),
}

impl ResponseSink {
    /// uses a writer to write a RouteE Compass app response to some location.
    pub fn write_response(&self, response: &mut serde_json::Value) -> Result<(), CompassAppError> {
        match self {
            ResponseSink::None => Ok(()),
            ResponseSink::File {
                filename,
                file,
                format,
                delimiter,
                iterations_per_flush,
                iterations,
            } => {
                let file_ref = Arc::clone(file);
                let mut file_attained = file_ref.lock().map_err(|e| {
                    CompassAppError::ReadOnlyPoisonError(format!(
                        "Could not aquire lock on output file: {e}"
                    ))
                })?;
                let it_ref = Arc::new(iterations);
                let mut it_attained = it_ref.lock().map_err(|e| {
                    CompassAppError::ReadOnlyPoisonError(format!(
                        "Could not aquire lock on File::iterations: {e}"
                    ))
                })?;

                let output_row = format.format_response(response)?;
                match delimiter {
                    Some(delimiter) => {
                        if *it_attained > 0 {
                            write!(file_attained, "{delimiter}").map_err(|e| {
                                CompassAppError::InternalError(format!(
                                    "failure writing delimiter to {filename}: {e}"
                                ))
                            })?;
                        }
                        write!(file_attained, "{output_row}").map_err(|e| {
                            CompassAppError::InternalError(format!(
                                "failure writing to {filename}: {e}"
                            ))
                        })?;
                    }
                    None => {
                        writeln!(file_attained, "{output_row}").map_err(|e| {
                            CompassAppError::InternalError(format!(
                                "failure writing to {filename}: {e}"
                            ))
                        })?;
                    }
                }
                *it_attained += 1;
                if *it_attained % iterations_per_flush == 0 {
                    file_attained.flush().map_err(|e| {
                        CompassAppError::InternalError(format!(
                            "failure flushing output to {filename}: {e}"
                        ))
                    })?;
                }

                Ok(())
            }
            ResponseSink::Parquet {
                base_filename: _,
                writers,
            } => {
                let thread_idx = rayon::current_thread_index().unwrap_or(0);
                let writer_idx = thread_idx % writers.len();
                let writer_mutex = &writers[writer_idx];
                let mut writer = writer_mutex.lock().map_err(|e| {
                    CompassAppError::ReadOnlyPoisonError(format!(
                        "Poisoned lock on parquet writer: {e}"
                    ))
                })?;
                writer.write_record(response.clone())?;
                Ok(())
            }
            ResponseSink::Combined(policies) => {
                for policy in policies {
                    policy.write_response(response)?;
                }
                Ok(())
            }
        }
    }

    pub fn close(&self) -> Result<String, CompassAppError> {
        match self {
            ResponseSink::None => Ok(String::from("")),
            ResponseSink::File {
                filename,
                file,
                format,
                ..
            } => {
                let file_ref = Arc::clone(file);
                let mut file_attained = file_ref.lock().map_err(|e| {
                    CompassAppError::ReadOnlyPoisonError(format!(
                        "Could not aquire lock on output file: {e}"
                    ))
                })?;

                // write optional footer (depends on format type)
                if let Some(final_contents) = format.generate_footer() {
                    writeln!(file_attained, "{final_contents}").map_err(|e| {
                        CompassAppError::InternalError(format!(
                            "failure writing final contents to {filename}: {e}"
                        ))
                    })?;
                }
                file_attained.finish()?;
                Ok(filename.clone())
            }
            ResponseSink::Parquet {
                base_filename: _,
                writers,
            } => {
                let mut out_strs = vec![];
                for (i, writer_mutex) in writers.iter().enumerate() {
                    let mut writer = writer_mutex.lock().map_err(|e| {
                        CompassAppError::ReadOnlyPoisonError(format!(
                            "Poisoned lock on parquet writer {}: {e}",
                            i
                        ))
                    })?;
                    let fname = writer.close()?;
                    if !fname.is_empty() {
                        out_strs.push(fname);
                    }
                }
                Ok(out_strs.join(","))
            }
            ResponseSink::Combined(policies) => {
                let mut out_strs = vec![];
                for policy in policies {
                    let out_str = policy.close()?;
                    if !out_str.is_empty() {
                        out_strs.push(out_str);
                    }
                }

                Ok(out_strs.join(","))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::compass::response::{
        response_output_policy::ResponseOutputPolicy, write_mode::WriteMode,
    };
    use serde_json::json;

    #[test]
    fn json_array_responses_are_comma_delimited() -> Result<(), Box<dyn std::error::Error>> {
        let output_file = tempfile::NamedTempFile::new()?;
        let policy = ResponseOutputPolicy::File {
            filename: output_file.path().to_string_lossy().into_owned(),
            format: ResponseOutputFormat::Json {
                newline_delimited: false,
            },
            file_flush_rate: None,
            write_mode: Some(WriteMode::Overwrite),
        };
        let sink = policy.build()?;

        sink.write_response(&mut json!({"id": 1}))?;
        sink.write_response(&mut json!({"id": 2}))?;
        sink.close()?;

        let contents = std::fs::read_to_string(output_file.path())?;
        let responses: Vec<serde_json::Value> = serde_json::from_str(&contents)?;
        assert_eq!(responses, vec![json!({"id": 1}), json!({"id": 2})]);
        Ok(())
    }
}
