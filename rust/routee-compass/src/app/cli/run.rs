use super::cli_args::CliArgs;
use crate::app::compass::response::response_output_policy::ResponseOutputPolicy;
use crate::app::compass::CompassAppConfig;
use crate::app::compass::{
    CompassApp, CompassAppError, CompassBuilderInventory, CompassJsonExtensions,
};
use itertools::{Either, Itertools};
use log::{debug, error, info, warn};
use serde_json::{json, Value};
use std::io::BufRead;
use std::time::Instant;
use std::{fs::File, io::BufReader, path::Path};

/// runs CompassApp from the command line using the provided app builder and optional
/// additional CompassApp configuration overwrites.
///
/// # Arguments
/// * `args`       - command line arguments for this run
/// * `builder`    - optional builder instance to overwrite the default. see CompassBuilderInventory for explanation.
/// * `run_config` - optional CompassApp configuration overrides
///
/// # Returns
/// After executing all queries, returns nothing, or returns an un-handled application error.
/// Any user errors are logged and optionally written to an output file depending on the file io policy.
pub fn command_line_runner(
    args: &CliArgs,
    builder: Option<CompassBuilderInventory>,
    run_config: Option<&Value>,
) -> Result<(), CompassAppError> {
    args.validate()?;

    // Start timing the load phase
    let load_start = Instant::now();

    // build the app
    let builder_or_default = match builder {
        Some(b) => b,
        None => CompassBuilderInventory::new()?,
    };
    let config_path = Path::new(&args.config_file);
    let mut config = CompassAppConfig::try_from(config_path)?;

    // Apply CLI overrides to config
    if let Some(parallelism) = args.parallelism {
        info!(
            "Overriding parallelism from config with CLI value: {}",
            parallelism
        );
        config.system.parallelism = Some(parallelism);
    }

    if let Some(ref output_directory) = args.output_directory {
        info!(
            "Overriding output directory from config with CLI value: {}",
            output_directory
        );

        // Create the directory if it doesn't exist
        let output_path = Path::new(output_directory);
        if !output_path.exists() {
            info!("Creating output directory: {}", output_directory);
            std::fs::create_dir_all(output_path).map_err(|e| {
                CompassAppError::BuildFailure(format!(
                    "Failed to create output directory '{}': {}",
                    output_directory, e
                ))
            })?;
        }

        // Override the output file in the response_output_policy
        if let Some(ref mut response_policy) = config.system.response_output_policy {
            apply_output_directory_override(response_policy, output_directory)?;
        } else {
            warn!("No response_output_policy in config; output_directory override will have no effect");
        }
    }

    info!(
        "Loaded the following Compass configuration:\n{}",
        config.to_pretty_string()?
    );
    let compass_app = match CompassApp::new(&config, &builder_or_default) {
        Ok(app) => app,
        Err(e) => {
            error!("Could not build CompassApp from config file: {e}");
            return Err(e);
        }
    };

    let load_duration = load_start.elapsed();
    debug!(
        "TIMING: phase=load_app duration_ms={} duration_secs={:.3}",
        load_duration.as_millis(),
        load_duration.as_secs_f64()
    );

    // read user file containing JSON query/queries
    info!("reading queries from {}", &args.query_file);
    let query_file = File::open(args.query_file.clone()).map_err(|_e| {
        CompassAppError::BuildFailure(format!("Could not find query file {}", args.query_file))
    })?;

    // Start timing the run phase
    let run_start = Instant::now();

    // execute queries on app
    let result = match (args.chunksize, args.newline_delimited) {
        (None, false) => run_json(&query_file, &compass_app, run_config),
        (Some(_), false) => Err(CompassAppError::InternalError(String::from(
            "not yet implemented",
        ))),
        (_, true) => {
            let chunksize = args.get_chunksize_option()?;
            run_newline_json(&query_file, chunksize, &compass_app, run_config)
        }
    };

    let run_duration = run_start.elapsed();
    debug!(
        "TIMING: phase=run_queries duration_ms={} duration_secs={:.3}",
        run_duration.as_millis(),
        run_duration.as_secs_f64()
    );

    let total_duration = load_start.elapsed();
    debug!(
        "TIMING: phase=total duration_ms={} duration_secs={:.3}",
        total_duration.as_millis(),
        total_duration.as_secs_f64()
    );

    result
}

/// parses a file as a valid JSON object and executes it as queries against
/// the CompassApp.run command.
fn run_json(
    query_file: &File,
    compass_app: &CompassApp,
    run_config: Option<&Value>,
) -> Result<(), CompassAppError> {
    let reader = BufReader::new(query_file);
    let user_json: serde_json::Value = serde_json::from_reader(reader)?;
    let mut user_queries = user_json.get_queries()?;
    let results = compass_app.run(&mut user_queries, run_config)?;
    for result in results.iter() {
        log_error(result);
    }
    Ok(())
}

/// parses a file as newline-delimited JSON which can be optionally chunked into sub-batches
/// and each sub-batch run as queries against the CompassApp.run command.
/// chunksize should be >> the configured CompassApp parallelism (from TOML file) for best
/// performance.
fn run_newline_json(
    query_file: &File,
    chunksize_option: Option<usize>,
    compass_app: &CompassApp,
    run_config: Option<&Value>,
) -> Result<(), CompassAppError> {
    let reader = BufReader::new(query_file);
    let iterator = reader.lines();
    let chunksize = chunksize_option.unwrap_or(usize::MAX);
    let chunks = iterator.chunks(chunksize);
    info!("reading {chunksize} queries at-a-time from newline-delimited JSON file");

    for (iteration, chunk) in chunks.into_iter().enumerate() {
        debug!("executing batch {}", iteration + 1);

        // parse JSON output
        let (mut chunk_queries, errors): (Vec<Value>, Vec<CompassAppError>) =
            chunk.enumerate().partition_map(|(idx, row)| match row {
                Ok(string) => match serde_json::from_str(&string) {
                    Ok(query) => Either::Left(query),
                    Err(e) => Either::Right(CompassAppError::CompassFailure(format!(
                        "while reading chunk {iteration} row {idx}, failed to read JSON: {e}"
                    ))),
                },
                Err(e) => Either::Right(CompassAppError::CompassFailure(format!(
                    "failed to parse query row due to: {e}"
                ))),
            });
        // run Compass on this chunk of queries
        for result in compass_app.run(&mut chunk_queries, run_config)?.iter() {
            log_error(result)
        }

        // report JSON parsing errors
        for error in errors {
            let error_json = json!({
                "request": "failed to parse",
                "error": error.to_string()
            });
            log_error(&error_json)
        }
    }

    Ok(())
}

fn log_error(result: &Value) {
    if let Some(error) = result.get("error") {
        let error_string = error.to_string().replace("\\n", "\n");
        error!("Error: {error_string}");
    }
}

/// Recursively applies output directory override to a ResponseOutputPolicy
/// Any existing filename in the config is treated as a filename only, and re-rooted
/// to the provided output directory.
fn apply_output_directory_override(
    policy: &mut ResponseOutputPolicy,
    output_directory: &str,
) -> Result<(), CompassAppError> {
    match policy {
        ResponseOutputPolicy::File { filename, .. } => {
            let file_path = Path::new(&filename);
            let file_name = file_path.file_name().ok_or_else(|| {
                CompassAppError::BuildFailure(format!(
                    "Could not extract filename from path '{}'",
                    filename
                ))
            })?;
            let new_path = Path::new(output_directory).join(file_name);
            *filename = new_path.to_string_lossy().to_string();
            Ok(())
        }
        ResponseOutputPolicy::Combined { policies } => {
            for sub_policy in policies.iter_mut() {
                apply_output_directory_override(sub_policy, output_directory)?;
            }
            Ok(())
        }
        ResponseOutputPolicy::None => {
            // No file to override
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::compass::response::response_output_format::ResponseOutputFormat;
    use ordered_hash_map::OrderedHashMap;

    #[test]
    fn test_apply_output_directory_override_json() {
        let mut policy = ResponseOutputPolicy::File {
            filename: "old.json".to_string(),
            format: ResponseOutputFormat::Json {
                newline_delimited: false,
            },
            file_flush_rate: None,
            write_mode: None,
        };

        // Should join directory with filename
        assert!(apply_output_directory_override(&mut policy, "new_dir").is_ok());
        if let ResponseOutputPolicy::File { filename, .. } = policy {
            assert_eq!(
                filename,
                Path::new("new_dir")
                    .join("old.json")
                    .to_string_lossy()
                    .to_string()
            );
        } else {
            panic!("Policy changed type");
        }
    }

    #[test]
    fn test_apply_output_directory_override_nested_source() {
        let mut policy = ResponseOutputPolicy::File {
            filename: "some/nested/path/old.json".to_string(),
            format: ResponseOutputFormat::Json {
                newline_delimited: false,
            },
            file_flush_rate: None,
            write_mode: None,
        };

        // Should strip source path and use only filename
        assert!(apply_output_directory_override(&mut policy, "new_dir").is_ok());
        if let ResponseOutputPolicy::File { filename, .. } = policy {
            assert_eq!(
                filename,
                Path::new("new_dir")
                    .join("old.json")
                    .to_string_lossy()
                    .to_string()
            );
        } else {
            panic!("Policy changed type");
        }
    }

    #[test]
    fn test_apply_output_directory_override_combined_top_level() {
        let mut policy = ResponseOutputPolicy::Combined {
            policies: vec![
                Box::new(ResponseOutputPolicy::File {
                    filename: "file1.json".to_string(),
                    format: ResponseOutputFormat::Json {
                        newline_delimited: false,
                    },
                    file_flush_rate: None,
                    write_mode: None,
                }),
                Box::new(ResponseOutputPolicy::File {
                    filename: "file2.csv".to_string(),
                    format: ResponseOutputFormat::Csv {
                        mapping: OrderedHashMap::new(),
                        sorted: false,
                    },
                    file_flush_rate: None,
                    write_mode: None,
                }),
            ],
        };

        assert!(apply_output_directory_override(&mut policy, "out").is_ok());

        if let ResponseOutputPolicy::Combined { policies } = policy {
            // Check first file
            if let ResponseOutputPolicy::File { filename, .. } = policies[0].as_ref() {
                assert_eq!(
                    filename,
                    &Path::new("out")
                        .join("file1.json")
                        .to_string_lossy()
                        .to_string()
                );
            } else {
                panic!("First policy changed type");
            }

            // Check second file
            if let ResponseOutputPolicy::File { filename, .. } = policies[1].as_ref() {
                assert_eq!(
                    filename,
                    &Path::new("out")
                        .join("file2.csv")
                        .to_string_lossy()
                        .to_string()
                );
            } else {
                panic!("Second policy changed type");
            }
        } else {
            panic!("Policy changed type");
        }
    }

    #[test]
    fn test_apply_output_directory_override_nested_combined() {
        let mut policy = ResponseOutputPolicy::Combined {
            policies: vec![
                Box::new(ResponseOutputPolicy::File {
                    filename: "file1.json".to_string(),
                    format: ResponseOutputFormat::Json {
                        newline_delimited: false,
                    },
                    file_flush_rate: None,
                    write_mode: None,
                }),
                Box::new(ResponseOutputPolicy::Combined {
                    policies: vec![
                        Box::new(ResponseOutputPolicy::File {
                            filename: "file2.csv".to_string(),
                            format: ResponseOutputFormat::Csv {
                                mapping: OrderedHashMap::new(),
                                sorted: false,
                            },
                            file_flush_rate: None,
                            write_mode: None,
                        }),
                        Box::new(ResponseOutputPolicy::None),
                    ],
                }),
            ],
        };

        assert!(apply_output_directory_override(&mut policy, "out").is_ok());

        if let ResponseOutputPolicy::Combined { policies } = policy {
            // Check first file
            if let ResponseOutputPolicy::File { filename, .. } = policies[0].as_ref() {
                assert_eq!(
                    filename,
                    &Path::new("out")
                        .join("file1.json")
                        .to_string_lossy()
                        .to_string()
                );
            }

            // Check nested file
            if let ResponseOutputPolicy::Combined { policies: nested } = policies[1].as_ref() {
                if let ResponseOutputPolicy::File { filename, .. } = nested[0].as_ref() {
                    assert_eq!(
                        filename,
                        &Path::new("out")
                            .join("file2.csv")
                            .to_string_lossy()
                            .to_string()
                    );
                }
            }
        }
    }

    #[test]
    fn test_apply_output_directory_override_none() {
        let mut policy = ResponseOutputPolicy::None;
        assert!(apply_output_directory_override(&mut policy, "new_dir").is_ok());
        assert!(matches!(policy, ResponseOutputPolicy::None));
    }
}
