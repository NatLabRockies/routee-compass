use super::cli_args::CliArgs;
use crate::app::compass::response::response_output_format::ResponseOutputFormat;
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

    if let Some(ref output_file) = args.output_file {
        info!(
            "Overriding output file from config with CLI value: {}",
            output_file
        );
        // Override the output file in the response_output_policy
        if let Some(ref mut response_policy) = config.system.response_output_policy {
            apply_output_file_override(response_policy, output_file)?;
        } else {
            warn!("No response_output_policy in config; output_file override will have no effect");
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
        let chunk_start = Instant::now();
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

/// Recursively applies output file override to a ResponseOutputPolicy
/// Validates that the output file extension matches the expected format
fn apply_output_file_override(
    policy: &mut ResponseOutputPolicy,
    output_file: &str,
) -> Result<(), CompassAppError> {
    match policy {
        ResponseOutputPolicy::File {
            filename, format, ..
        } => {
            // Strip .gz suffix if present to check the actual format
            let file_to_check = output_file.strip_suffix(".gz").unwrap_or(output_file);

            // Validate extension matches format
            let valid = match format {
                ResponseOutputFormat::Json { .. } => file_to_check.ends_with(".json"),
                ResponseOutputFormat::Csv { .. } => file_to_check.ends_with(".csv"),
                ResponseOutputFormat::Parquet { .. } => file_to_check.ends_with(".parquet"),
            };

            if !valid {
                let expected_ext = match format {
                    ResponseOutputFormat::Json { .. } => ".json",
                    ResponseOutputFormat::Csv { .. } => ".csv",
                    ResponseOutputFormat::Parquet { .. } => ".parquet",
                };
                return Err(CompassAppError::BuildFailure(format!(
                    "Output file '{}' does not match expected format. Expected file ending with '{}' (optionally followed by '.gz')",
                    output_file, expected_ext
                )));
            }

            *filename = output_file.to_string();
            Ok(())
        }
        ResponseOutputPolicy::Combined { policies } => {
            // Count how many File policies exist
            let file_policy_count = count_file_policies(policies);

            if file_policy_count > 1 {
                return Err(CompassAppError::BuildFailure(format!(
                    "Cannot override output file with combined policy containing {} file outputs. \
                    It is ambiguous which file to override. Please adjust your configuration to use \
                    a single output file, or remove the --output-file CLI argument.",
                    file_policy_count
                )));
            }

            for sub_policy in policies.iter_mut() {
                apply_output_file_override(sub_policy, output_file)?;
            }
            Ok(())
        }
        ResponseOutputPolicy::None => {
            // No file to override
            Ok(())
        }
    }
}

/// Recursively counts the number of File policies in a policy tree
fn count_file_policies(policies: &[Box<ResponseOutputPolicy>]) -> usize {
    policies
        .iter()
        .map(|policy| match policy.as_ref() {
            ResponseOutputPolicy::File { .. } => 1,
            ResponseOutputPolicy::Combined { policies: nested } => count_file_policies(nested),
            ResponseOutputPolicy::None => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::compass::response::response_output_format::ResponseOutputFormat;
    use ordered_hash_map::OrderedHashMap;

    #[test]
    fn test_apply_output_file_override_json_valid() {
        let mut policy = ResponseOutputPolicy::File {
            filename: "old.json".to_string(),
            format: ResponseOutputFormat::Json {
                newline_delimited: false,
            },
            file_flush_rate: None,
            write_mode: None,
        };

        // Should succeed for .json file
        assert!(apply_output_file_override(&mut policy, "new.json").is_ok());

        // Should succeed for .json.gz file
        assert!(apply_output_file_override(&mut policy, "new.json.gz").is_ok());
    }

    #[test]
    fn test_apply_output_file_override_json_invalid() {
        let mut policy = ResponseOutputPolicy::File {
            filename: "old.json".to_string(),
            format: ResponseOutputFormat::Json {
                newline_delimited: false,
            },
            file_flush_rate: None,
            write_mode: None,
        };

        // Should fail for .csv file
        let result = apply_output_file_override(&mut policy, "new.csv");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not match expected format"));
    }

    #[test]
    fn test_apply_output_file_override_csv_valid() {
        let mut policy = ResponseOutputPolicy::File {
            filename: "old.csv".to_string(),
            format: ResponseOutputFormat::Csv {
                mapping: OrderedHashMap::new(),
                sorted: false,
            },
            file_flush_rate: None,
            write_mode: None,
        };

        // Should succeed for .csv file
        assert!(apply_output_file_override(&mut policy, "new.csv").is_ok());

        // Should succeed for .csv.gz file
        assert!(apply_output_file_override(&mut policy, "new.csv.gz").is_ok());
    }

    #[test]
    fn test_apply_output_file_override_csv_invalid() {
        let mut policy = ResponseOutputPolicy::File {
            filename: "old.csv".to_string(),
            format: ResponseOutputFormat::Csv {
                mapping: OrderedHashMap::new(),
                sorted: false,
            },
            file_flush_rate: None,
            write_mode: None,
        };

        // Should fail for .json file
        let result = apply_output_file_override(&mut policy, "new.json");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not match expected format"));
    }

    #[test]
    fn test_apply_output_file_override_parquet_valid() {
        let mut policy = ResponseOutputPolicy::File {
            filename: "old.parquet".to_string(),
            format: ResponseOutputFormat::Parquet { mapping: None },
            file_flush_rate: None,
            write_mode: None,
        };

        // Should succeed for .parquet file
        assert!(apply_output_file_override(&mut policy, "new.parquet").is_ok());

        // Should succeed for .parquet.gz file
        assert!(apply_output_file_override(&mut policy, "new.parquet.gz").is_ok());
    }

    #[test]
    fn test_apply_output_file_override_combined_policy() {
        // Test 1: Combined policy where all sub-policies have the same format
        let mut policy = ResponseOutputPolicy::Combined {
            policies: vec![
                Box::new(ResponseOutputPolicy::File {
                    filename: "old1.json".to_string(),
                    format: ResponseOutputFormat::Json {
                        newline_delimited: false,
                    },
                    file_flush_rate: None,
                    write_mode: None,
                }),
                Box::new(ResponseOutputPolicy::File {
                    filename: "old2.json".to_string(),
                    format: ResponseOutputFormat::Json {
                        newline_delimited: true,
                    },
                    file_flush_rate: None,
                    write_mode: None,
                }),
            ],
        };

        // Should fail because there are multiple File policies
        let result = apply_output_file_override(&mut policy, "new.json");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ambiguous"));
        assert!(err_msg.contains("2 file outputs"));

        // Test 2: Combined policy with single File and None should succeed
        let mut policy_single = ResponseOutputPolicy::Combined {
            policies: vec![
                Box::new(ResponseOutputPolicy::File {
                    filename: "old.json".to_string(),
                    format: ResponseOutputFormat::Json {
                        newline_delimited: false,
                    },
                    file_flush_rate: None,
                    write_mode: None,
                }),
                Box::new(ResponseOutputPolicy::None),
            ],
        };

        // Should succeed when only one File policy exists
        assert!(apply_output_file_override(&mut policy_single, "new.json").is_ok());
    }

    #[test]
    fn test_apply_output_file_override_none_policy() {
        let mut policy = ResponseOutputPolicy::None;

        // Should succeed with no effect on None policy
        assert!(apply_output_file_override(&mut policy, "new.json").is_ok());
        assert!(apply_output_file_override(&mut policy, "new.csv").is_ok());
        assert!(apply_output_file_override(&mut policy, "any_file.txt").is_ok());

        // Verify policy is still None
        assert!(matches!(policy, ResponseOutputPolicy::None));
    }

    #[test]
    fn test_count_file_policies_empty() {
        let policies: Vec<Box<ResponseOutputPolicy>> = vec![];
        assert_eq!(count_file_policies(&policies), 0);
    }

    #[test]
    fn test_count_file_policies_single_file() {
        let policies = vec![Box::new(ResponseOutputPolicy::File {
            filename: "test.json".to_string(),
            format: ResponseOutputFormat::Json {
                newline_delimited: false,
            },
            file_flush_rate: None,
            write_mode: None,
        })];
        assert_eq!(count_file_policies(&policies), 1);
    }

    #[test]
    fn test_count_file_policies_multiple_files() {
        let policies = vec![
            Box::new(ResponseOutputPolicy::File {
                filename: "test1.json".to_string(),
                format: ResponseOutputFormat::Json {
                    newline_delimited: false,
                },
                file_flush_rate: None,
                write_mode: None,
            }),
            Box::new(ResponseOutputPolicy::File {
                filename: "test2.csv".to_string(),
                format: ResponseOutputFormat::Csv {
                    mapping: OrderedHashMap::new(),
                    sorted: false,
                },
                file_flush_rate: None,
                write_mode: None,
            }),
        ];
        assert_eq!(count_file_policies(&policies), 2);
    }

    #[test]
    fn test_count_file_policies_none() {
        let policies = vec![Box::new(ResponseOutputPolicy::None)];
        assert_eq!(count_file_policies(&policies), 0);
    }

    #[test]
    fn test_count_file_policies_mixed() {
        let policies = vec![
            Box::new(ResponseOutputPolicy::File {
                filename: "test.json".to_string(),
                format: ResponseOutputFormat::Json {
                    newline_delimited: false,
                },
                file_flush_rate: None,
                write_mode: None,
            }),
            Box::new(ResponseOutputPolicy::None),
            Box::new(ResponseOutputPolicy::File {
                filename: "test.csv".to_string(),
                format: ResponseOutputFormat::Csv {
                    mapping: OrderedHashMap::new(),
                    sorted: false,
                },
                file_flush_rate: None,
                write_mode: None,
            }),
        ];
        assert_eq!(count_file_policies(&policies), 2);
    }

    #[test]
    fn test_count_file_policies_nested_combined() {
        let policies = vec![
            Box::new(ResponseOutputPolicy::File {
                filename: "test1.json".to_string(),
                format: ResponseOutputFormat::Json {
                    newline_delimited: false,
                },
                file_flush_rate: None,
                write_mode: None,
            }),
            Box::new(ResponseOutputPolicy::Combined {
                policies: vec![
                    Box::new(ResponseOutputPolicy::File {
                        filename: "test2.csv".to_string(),
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
        ];
        // Should count 2 files: test1.json and test2.csv
        assert_eq!(count_file_policies(&policies), 2);
    }
}
