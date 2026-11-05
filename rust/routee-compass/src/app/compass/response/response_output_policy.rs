use super::{
    response_output_format::ResponseOutputFormat, response_sink::ResponseSink,
    write_mode::WriteMode,
};
use crate::app::compass::CompassAppError;
use serde::{Deserialize, Serialize};
use std::{num::NonZeroUsize, path::PathBuf};

/// user configuration for the file writing of compass outputs.
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ResponseOutputPolicy {
    /// when outputs are simply persisted by the app into program memory, such as
    /// when using via the Rust or Python API.
    #[default]
    None,
    /// writes all results to a single file (CSV, JSON, JSONL) or archive (Parquet).
    File {
        /// destination file. may be a standard file suffix, or, if terminates with '.gz' will be gzip-compressed.
        filepath: PathBuf,
        /// file format to target
        format: ResponseOutputFormat,
        /// optional argument to specify the frequency (in rows) to flush data to the file
        file_flush_rate: Option<u64>,
        /// optional argument to specify if we expect to open, append, or overwrite data.
        write_mode: Option<WriteMode>,
    },
    /// concurrently writes all results to a directory of flat files (CSV, JSON, JSONL)
    /// using a pool of output files and a round-robin assignment strategy that, in the default case,
    /// creates as many output files as there are threads.
    Directory {
        /// destination directory.
        path: PathBuf,
        /// optional filename prefix for each file in the directory
        prefix: Option<String>,
        /// optional number of files to write to. by default, uses the Compass system parallelism argument.
        concurrency: Option<NonZeroUsize>,
        /// gzip each output file.
        gzip: bool,
        /// file format to target
        format: ResponseOutputFormat,
        /// optional argument to specify the frequency (in rows) to flush data to the file
        file_flush_rate: Option<u64>,
        /// optional argument to specify if we expect to open, append, or overwrite data.
        write_mode: Option<WriteMode>,
    },
    /// write to multiple locations
    Combined {
        policies: Vec<Box<ResponseOutputPolicy>>,
    },
}

impl ResponseOutputPolicy {
    /// creates an instance of a writer which writes responses to some destination.
    /// the act of building this writer may include writing initial content to some sink,
    /// such as a file header.
    pub fn build(&self) -> Result<ResponseSink, CompassAppError> {
        match self {
            ResponseOutputPolicy::None => Ok(ResponseSink::None),
            ResponseOutputPolicy::File {
                filepath: path,
                format,
                file_flush_rate,
                write_mode,
            } => match format {
                ResponseOutputFormat::Parquet { mapping } => {
                    ResponseSink::new_parquet(path.clone(), *file_flush_rate, mapping.clone())
                }
                _ => ResponseSink::new_file(
                    path.clone(),
                    format.clone(),
                    *file_flush_rate,
                    write_mode.clone(),
                ),
            },
            ResponseOutputPolicy::Directory {
                path,
                prefix,
                concurrency,
                gzip,
                format,
                file_flush_rate,
                write_mode,
            } => {
                let file_prefix = match prefix {
                    Some(p) => p.clone(),
                    None => "part".to_string(),
                };
                if matches!(format, ResponseOutputFormat::Parquet { .. }) {
                    return Err(CompassAppError::BuildFailure(
                         "directory response_output_policy does not support parquet; use type=\"file\" with parquet format instead".to_string(),
                     ));
                }
                ResponseSink::new_files(
                    path.clone(),
                    file_prefix,
                    *gzip,
                    *concurrency,
                    format.clone(),
                    *file_flush_rate,
                    write_mode.clone(),
                )
            }
            ResponseOutputPolicy::Combined { policies } => {
                let policies = policies
                    .iter()
                    .map(|p| p.build().map(Box::new))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ResponseSink::Combined(policies))
            }
        }
    }
}
