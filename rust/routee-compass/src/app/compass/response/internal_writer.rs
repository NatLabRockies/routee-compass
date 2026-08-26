use flate2::{write::GzEncoder, Compression};
use std::{fs::File, io::Write, path::Path};

use crate::app::compass::{
    response::{response_output_format::ResponseOutputFormat, write_mode::OpenResult, WriteMode},
    CompassAppError,
};

pub enum InternalWriter {
    /// writes to a file without compression.
    File { file: File, appending: bool },
    /// writes to a gzipped file
    GzippedFile {
        encoder: Box<GzEncoder<File>>,
        appending: bool,
    },
}

impl InternalWriter {
    /// creates a new InternalWriter which may target a raw or gzip'd file depending on the
    /// filename (ends with .gz).
    pub fn new(filepath: &Path, write_mode: Option<WriteMode>) -> Result<Self, CompassAppError> {
        let wm = write_mode.unwrap_or_default();
        let wrapped_file = get_or_create_file_writer(&filepath, &wm)?;
        Ok(wrapped_file)
    }

    /// writes the header to the file. checks if the file is empty; in the case that this
    /// was invoked in a chunking run of Compass, the file could already contain rows and
    /// we therefore want to skip writing the header.
    pub fn write_header(
        &mut self,
        format: &ResponseOutputFormat,
    ) -> core::result::Result<(), CompassAppError> {
        let file_is_empty = self.is_empty()?;
        if file_is_empty {
            let header = format.generate_header().unwrap_or_else(|| String::from(""));

            self.write(header.as_bytes()).map(|_| {}).map_err(|e| {
                CompassAppError::InternalError(format!("Failure writing header to file: {e}"))
            })?;
        }
        Ok(())
    }

    /// test if file has not yet received any data using the File::metadata() inspector and
    /// metadata.len().
    pub fn is_empty(&self) -> Result<bool, CompassAppError> {
        match self {
            InternalWriter::File { file, .. } => is_empty(file),
            InternalWriter::GzippedFile { encoder, .. } => is_empty(encoder.get_ref()),
        }
    }

    pub fn finish(&mut self) -> core::result::Result<(), CompassAppError> {
        match self {
            InternalWriter::File { ref mut file, .. } => {
                file.flush().map_err(|e| {
                    CompassAppError::InternalError(format!("failure flushing output {e}"))
                })?;
                Ok(())
            }
            InternalWriter::GzippedFile {
                ref mut encoder, ..
            } => {
                // NOTE: Because GzEncoder::finish requires ownership, we use try_finish instead.
                // Subsequent attempts to write to this file will panic!
                let _ = encoder.flush();
                encoder.try_finish().map_err(|e| {
                    CompassAppError::InternalError(format!("failure finishing encoded output {e}"))
                })?;
                Ok(())
            }
        }
    }
}

impl Write for InternalWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            InternalWriter::File { file, .. } => file.write(buf),
            InternalWriter::GzippedFile { encoder, .. } => encoder.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            InternalWriter::File { file, .. } => file.flush(),
            InternalWriter::GzippedFile { encoder, .. } => encoder.flush(),
        }
    }
}

fn is_empty(file: &File) -> Result<bool, CompassAppError> {
    let m = file.metadata().map_err(|e| {
        CompassAppError::InternalError(format!("failure inspecting output file metadata: {e}"))
    })?;
    Ok(m.len() == 0)
}

/// helper function to handle the various file type + write mode options
fn get_or_create_file_writer(
    filepath: &Path,
    write_mode: &WriteMode,
) -> Result<InternalWriter, CompassAppError> {
    let ext_option = filepath.extension();
    match ext_option {
        Some(ext) if ext == "gz" => {
            let OpenResult { file, appending } = write_mode.open_file(filepath)?;
            let encoder = Box::new(GzEncoder::new(file, Compression::default()));
            Ok(InternalWriter::GzippedFile { encoder, appending })
        }
        _ => {
            let OpenResult { file, appending } = write_mode.open_file(filepath)?;
            Ok(InternalWriter::File { file, appending })
        }
    }
}
