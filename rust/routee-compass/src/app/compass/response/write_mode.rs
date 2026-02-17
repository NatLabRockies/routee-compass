use crate::app::compass::CompassAppError;
use routee_compass_core::config::CompassConfigurationError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    /// default write mode which accommodates the Compass chunking API. expects we can continue to append to a file.
    #[default]
    Append,
    /// if a file already exists, overwrite it. should NOT be used in chunking mode.
    Overwrite,
    /// if a file already exists, produce an error. should NOT be used in chunking mode.
    Error,
}

impl WriteMode {
    pub fn open_file(&self, path: &Path) -> Result<File, CompassAppError> {
        match self {
            WriteMode::Append => {
                if !path.exists() {
                    create_file(path)?;
                }
                open_append(path)
            }
            WriteMode::Overwrite => {
                remove_if_exists(path)?;
                create_file(path)?;
                open_append(path)
            }
            WriteMode::Error => {
                if path.exists() {
                    Err(CompassAppError::CompassConfigurationError(
                        CompassConfigurationError::UserConfigurationError(format!(
                            "file exists but write mode is 'error' {}",
                            path.to_str().unwrap_or_default()
                        )),
                    ))?
                }
                create_file(path)?;
                open_append(path)
            }
        }
    }
}

fn remove_if_exists(path: &Path) -> Result<(), CompassAppError> {
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| {
            CompassAppError::BuildFailure(format!(
                "attempting to remove existing file {} in overwrite mode, {e}",
                path.to_string_lossy()
            ))
        })
    } else {
        Ok(())
    }
}

fn open_append(path: &Path) -> Result<File, CompassAppError> {
    OpenOptions::new().append(true).open(path).map_err(|e| {
        CompassAppError::InternalError(format!(
            "failure opening file {} in append mode: {}",
            path.to_str().unwrap_or_default(),
            e
        ))
    })
}

fn create_file(path: &Path) -> Result<File, CompassConfigurationError> {
    File::create(path).map_err(|e| {
        CompassConfigurationError::UserConfigurationError(format!("Could not create file: {e}"))
    })
}
