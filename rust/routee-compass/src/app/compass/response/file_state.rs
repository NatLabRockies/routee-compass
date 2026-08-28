use std::{io::Write, path::Path};

use crate::app::compass::{
    response::{
        internal_writer::InternalWriter, response_output_format::ResponseOutputFormat, WriteMode,
    },
    CompassAppError,
};

/// the state of a file write.
pub struct FileState {
    /// name of the file written to
    pub filename: String,
    /// file writer, either a raw writer or gzip writer.
    writer: InternalWriter,
    /// number of records written to the file.
    records: u64,
}

impl FileState {
    /// creates a new file state object, tracking an open file along with the number of
    /// rows written to the file. writes the header to the file.
    pub fn new(
        filepath: &Path,
        format: &ResponseOutputFormat,
        write_mode: Option<WriteMode>,
    ) -> Result<Self, CompassAppError> {
        let mut writer = InternalWriter::new(filepath, write_mode)?;
        let filename = filepath.to_string_lossy().to_string();
        writer.write_header(format)?;
        Ok(FileState {
            filename,
            writer,
            records: 0,
        })
    }

    /// write content to this file, updating the count of rows written.
    /// if the rows exceed the flush cycle, flush the output.
    pub fn write(
        &mut self,
        value: &str,
        delimiter: Option<&str>,
        iterations_per_flush: u64,
    ) -> Result<(), CompassAppError> {
        if let Some(delimiter) = delimiter {
            if self.records > 0 {
                write!(self.writer, "{delimiter}").map_err(|e| {
                    CompassAppError::InternalError(format!(
                        "failure writing delimiter to {}: {e}",
                        &self.filename
                    ))
                })?;
            }
        }
        write!(self.writer, "{value}").map_err(|e| {
            CompassAppError::InternalError(format!("failure writing to {}: {e}", &self.filename))
        })?;
        self.records += 1;
        if self.records.is_multiple_of(iterations_per_flush) {
            self.writer.flush().map_err(|e| {
                CompassAppError::InternalError(format!(
                    "failure flushing output to {}: {e}",
                    &self.filename
                ))
            })?;
        }

        Ok(())
    }

    pub fn close(&mut self) -> Result<(), CompassAppError> {
        self.writer.finish()
    }
}
