use ordered_hash_map::OrderedHashMap;

use super::parquet_writer::ParquetPartitionWriter;
use super::response_output_format::ResponseOutputFormat;
use super::write_mode::WriteMode;
use crate::app::compass::response::mapping::file_mapping::FileMapping;
use crate::app::compass::response::FileState;
use crate::app::compass::CompassAppError;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// implements the output policy for a given output location.
pub enum ResponseSink {
    None,
    File {
        filename: String,
        state: Arc<Mutex<FileState>>,
        format: ResponseOutputFormat,
        delimiter: Option<String>,
        iterations_per_flush: u64,
    },
    Files {
        directory: String,
        state: Vec<Mutex<FileState>>,
        format: ResponseOutputFormat,
        delimiter: Option<String>,
        iterations_per_flush: u64,
    },
    Parquet {
        base_filename: String,
        writers: Vec<Mutex<ParquetPartitionWriter>>,
    },
    Combined(Vec<Box<ResponseSink>>),
}

impl ResponseSink {
    /// creates a response sink for a single file.
    pub fn new_file(
        filename: PathBuf,
        format: ResponseOutputFormat,
        file_flush_rate: Option<u64>,
        write_mode: Option<WriteMode>,
    ) -> Result<Self, CompassAppError> {
        let state = FileState::new(&filename, &format, write_mode)?;
        let delimiter = format.delimiter();
        let iterations_per_flush = file_flush_rate.unwrap_or(1);
        let filename = filename.to_string_lossy().to_string();
        Ok(Self::File {
            filename,
            state: Arc::new(Mutex::new(state)),
            format,
            delimiter,
            iterations_per_flush,
        })
    }

    /// create a response sink backed by a directory of files.
    pub fn new_files(
        directory: PathBuf,
        file_prefix: String,
        gzip: bool,
        n_writers: Option<NonZeroUsize>,
        format: ResponseOutputFormat,
        file_flush_rate: Option<u64>,
        write_mode: Option<WriteMode>,
    ) -> Result<Self, CompassAppError> {
        std::fs::create_dir_all(&directory).map_err(|e| {
            CompassAppError::InternalError(format!(
                "failed to create output directory {:?}: {}",
                directory, e
            ))
        })?;
        let n_writers = match n_writers {
            Some(n) => n.into(),
            None => rayon::current_num_threads(),
        };
        let suffix = format.file_suffix();
        let gzip = match gzip {
            true => ".gz",
            false => "",
        };

        let mut state = Vec::with_capacity(n_writers);
        for i in 0..n_writers {
            let filename = format!("{file_prefix}-{i}.{suffix}{gzip}");
            let filepath = directory.join(filename);
            let file_state = FileState::new(&filepath, &format, write_mode.clone())?;
            state.push(Mutex::new(file_state));
        }

        let delimiter = format.delimiter();
        let iterations_per_flush = file_flush_rate.unwrap_or(1);
        let directory = directory.to_string_lossy().to_string();

        let result = Self::Files {
            directory,
            state,
            format,
            delimiter,
            iterations_per_flush,
        };
        Ok(result)
    }

    /// creates a response sink that targets a parquet archive.
    ///
    /// note: does not take advantage of parquet partitionings.
    pub fn new_parquet(
        path: PathBuf,
        file_flush_rate: Option<u64>,
        mapping: Option<OrderedHashMap<String, FileMapping>>,
    ) -> Result<Self, CompassAppError> {
        let num_threads = rayon::current_num_threads();
        let buffer_size = file_flush_rate.unwrap_or(100) as usize;
        let base_filename = path.to_string_lossy().to_string();

        // create the parent directory if it doesn't exist
        std::fs::create_dir_all(&path).map_err(|e| {
            CompassAppError::InternalError(format!(
                "failed to create parquet base file {:?}: {}",
                path, e
            ))
        })?;

        let writers = (0..num_threads)
            .map(|i| {
                let partname = format!("part_{i}.parquet");
                let filepath = path.join(partname);
                let fname = filepath.to_string_lossy().to_string();
                let writer = ParquetPartitionWriter::new(fname, buffer_size, mapping.clone());
                Mutex::new(writer)
            })
            .collect();
        Ok(ResponseSink::Parquet {
            base_filename,
            writers,
        })
    }

    /// uses a writer to write a RouteE Compass app response to some location.
    pub fn write_response(&self, response: &mut serde_json::Value) -> Result<(), CompassAppError> {
        match self {
            ResponseSink::None => Ok(()),
            ResponseSink::File {
                filename,
                state,
                format,
                delimiter,
                iterations_per_flush,
            } => {
                let state_clone = state.clone();
                let mut state_attained = state_clone.lock().map_err(|e| {
                    CompassAppError::ReadOnlyPoisonError(format!(
                        "Could not aquire lock on output file {filename}: {e}",
                    ))
                })?;
                let value = format.format_response(response)?;
                state_attained.write(&value, delimiter.as_deref(), *iterations_per_flush)
            }
            ResponseSink::Files {
                state,
                format,
                delimiter,
                iterations_per_flush,
                directory,
            } => {
                // map the current thread to the set of writers, wrapping around the max writer index using modulo
                // to ensure we are always selecting a valid index, though if the user set max writers as fewer than
                // max threads, then write operations may be blocked here as they are assigned thread-affine.
                let thread_idx = rayon::current_thread_index().unwrap_or(0);
                let writer_idx = thread_idx % state.len();
                let writer_mutex = match state.get(writer_idx) {
                    None => {
                        let msg = format!("during write_response, writer_idx > state.len() where thread_idx={thread_idx}, writer_idx={writer_idx}, state.len()={}", state.len());
                        return Err(CompassAppError::InternalError(msg));
                    }
                    Some(file_state) => file_state,
                };
                let mut writer = writer_mutex.lock().map_err(|e| {
                    CompassAppError::ReadOnlyPoisonError(format!(
                        "Could not aquire lock on {writer_idx}th output file in '{directory}': {e}"
                    ))
                })?;
                let value = format.format_response(response)?;
                writer.write(&value, delimiter.as_deref(), *iterations_per_flush)
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

    /// closes the writer. first writing the footer, and then implicitly calling [Drop] on this
    /// object, dropping the inner file writer in the process. returns the output location where
    /// the data was written.
    pub fn close(self) -> Result<String, CompassAppError> {
        match self {
            ResponseSink::None => Ok(String::from("")),
            ResponseSink::File {
                state,
                format,
                iterations_per_flush,
                filename: _,
                delimiter: _,
            } => {
                let state_mut: Mutex<FileState> = Arc::try_unwrap(state)
                    .map_err(|_| {
                        let msg = "calling close on response File writer but it still has more than 1 reference to its smart pointer".to_string();
                        CompassAppError::InternalError(msg)
                    })?;
                close_file(state_mut, &format, iterations_per_flush)
            }
            ResponseSink::Files {
                directory,
                state,
                format,
                iterations_per_flush,
                delimiter: _,
            } => {
                for s in state.into_iter() {
                    close_file(s, &format, iterations_per_flush)?;
                }
                Ok(directory)
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

/// writes the footer and returns the filename.
fn close_file(
    state: Mutex<FileState>,
    format: &ResponseOutputFormat,
    iterations_per_flush: u64,
) -> Result<String, CompassAppError> {
    let mut state_attained = state.lock().map_err(|e| {
        CompassAppError::ReadOnlyPoisonError(format!("Could not aquire lock on output file: {e}",))
    })?;
    if let Some(final_contents) = format.generate_footer() {
        state_attained.write(&final_contents, None, iterations_per_flush)?;
    }
    state_attained.close()?;
    Ok(state_attained.filename.clone())
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
            path: output_file.path().to_path_buf(),
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
