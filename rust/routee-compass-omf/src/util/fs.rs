use std::{
    fs::File,
    path::{Path, PathBuf},
};

use csv::QuoteStyle;
use flate2::{write::GzEncoder, Compression};
use kdam::tqdm;
use serde::Serialize;

use crate::collection::OvertureMapsCollectionError;

/// copies bambam-config-omf.toml to the directory of an OMF import.
pub fn copy_default_config(output_directory: &Path) -> Result<(), OvertureMapsCollectionError> {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("util")
        .join("bambam-config-omf.toml");
    let dst = output_directory.join("bambam.toml");
    std::fs::copy(&src, &dst).map_err(|e| OvertureMapsCollectionError::WriteError {
        path: dst,
        message: format!(
            "unable to copy default TOML from '{}': {e}",
            src.to_str().unwrap_or("?")
        ),
    })?;
    Ok(())
}

/// helper function to "mkdir -p path" - make all directories along a path
pub fn create_dirs<P>(path: P) -> Result<(), OvertureMapsCollectionError>
where
    P: AsRef<Path>,
{
    let dirspath = path.as_ref();
    if !dirspath.is_dir() {
        std::fs::create_dir_all(dirspath).map_err(|e| {
            let msg = format!(
                "error building output directory '{}': {e}",
                dirspath.to_str().unwrap_or_default()
            );
            OvertureMapsCollectionError::InvalidUserInput(msg)
        })
    } else {
        Ok(())
    }
}

pub fn serialize_into_csv<I>(
    iterable: I,
    filename: &str,
    output_directory: &Path,
    overwrite: bool,
    desc: &str,
) -> Result<(), OvertureMapsCollectionError>
where
    I: IntoIterator,
    I::IntoIter: ExactSizeIterator,
    I::Item: Serialize,
{
    let mut writer: Option<csv::Writer<GzEncoder<File>>> = create_writer(
        output_directory,
        filename,
        true,
        QuoteStyle::Necessary,
        overwrite,
    );
    let iter = iterable.into_iter();
    let total = iter.len();
    let bar_iter = tqdm!(iter, total = total, desc = desc);
    for element in bar_iter {
        if let Some(ref mut writer) = writer {
            writer.serialize(element).map_err(|e| {
                OvertureMapsCollectionError::CsvWriteError(format!(
                    "Failed to write to {filename}: {e}"
                ))
            })?;
        }
    }
    eprintln!();
    if let Some(ref mut writer) = writer {
        writer.flush().map_err(|e| {
            OvertureMapsCollectionError::CsvWriteError(format!("Failed to flush {filename}: {e}"))
        })?;
    };

    Ok(())
}

pub fn serialize_into_enumerated_txt<I>(
    iterable: I,
    filename: &str,
    output_directory: &Path,
    overwrite: bool,
    desc: &str,
) -> Result<(), OvertureMapsCollectionError>
where
    I: IntoIterator,
    I::IntoIter: ExactSizeIterator,
    I::Item: Serialize,
{
    let mut writer: Option<csv::Writer<GzEncoder<File>>> = create_writer(
        output_directory,
        filename,
        false,
        QuoteStyle::Never,
        overwrite,
    );
    let iter = iterable.into_iter();
    let total = iter.len();
    let bar_iter = tqdm!(iter, total = total, desc = desc);
    for element in bar_iter {
        if let Some(ref mut writer) = writer {
            writer.serialize(element).map_err(|e| {
                OvertureMapsCollectionError::CsvWriteError(format!(
                    "Failed to write to {filename}: {e}"
                ))
            })?;
        }
    }
    eprintln!();
    if let Some(ref mut writer) = writer {
        writer.flush().map_err(|e| {
            OvertureMapsCollectionError::CsvWriteError(format!("Failed to flush {filename}: {e}"))
        })?;
    };

    Ok(())
}

/// helper function to build a filewriter for writing either .csv.gz or
/// .txt.gz files for compass datasets while respecting the user's overwrite
/// preferences and properly formatting WKT outputs.
fn create_writer(
    directory: &Path,
    filename: &str,
    has_headers: bool,
    quote_style: QuoteStyle,
    overwrite: bool,
) -> Option<csv::Writer<GzEncoder<File>>> {
    let filepath = directory.join(filename);
    if filepath.exists() && !overwrite {
        return None;
    }
    let file = File::create(filepath).unwrap();
    let buffer = GzEncoder::new(file, Compression::default());
    let writer = csv::WriterBuilder::new()
        .has_headers(has_headers)
        .quote_style(quote_style)
        .from_writer(buffer);
    Some(writer)
}
