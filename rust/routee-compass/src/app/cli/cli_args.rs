use crate::app::compass::CompassAppError;
use clap::Parser;
use routee_compass_core::config::CompassConfigurationError;
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct CliArgs {
    /// RouteE Compass service configuration TOML file
    #[arg(short, long, value_name = "*.toml")]
    pub config_file: String,

    /// JSON file containing queries. Should be newline-delimited if chunksize is set
    #[arg(short, long, value_name = "*.json")]
    pub query_file: String,

    /// Size of batches to load into memory at a time
    #[arg(long)]
    pub chunksize: Option<i64>,

    /// Format of JSON queries file, if regular JSON or newline-delimited JSON
    #[arg(short, long)]
    pub newline_delimited: bool,

    /// Override parallelism setting from config file
    #[arg(short, long)]
    pub parallelism: Option<usize>,

    /// Override output filename from config file
    #[arg(short, long, value_name = "FILE")]
    pub output_file: Option<String>,
}

impl CliArgs {
    pub fn validate(&self) -> Result<(), CompassAppError> {
        match (self.chunksize, self.newline_delimited) {
            (Some(_), false) => Err(CompassAppError::CompassConfigurationError(
                CompassConfigurationError::UserConfigurationError(String::from(
                    "chunksize must be set if newline_delimited_queries is true",
                )),
            )),
            (Some(chunksize), _) if chunksize < 1 => {
                Err(CompassAppError::CompassConfigurationError(
                    CompassConfigurationError::UserConfigurationError(format!(
                        "chunksize must be positive, found {chunksize}"
                    )),
                ))
            }
            _ => Ok(()),
        }
    }

    pub fn get_chunksize_option(&self) -> Result<Option<usize>, CompassAppError> {
        match self.chunksize {
            None => Ok(None),
            Some(c) => {
                if c > 0 {
                    Ok(Some(c as usize))
                } else {
                    Err(CompassAppError::CompassFailure(format!(
                        "chunksize must be positive, found {c}"
                    )))
                }
            }
        }
    }
}
