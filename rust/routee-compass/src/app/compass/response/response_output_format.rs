use super::{mapping::file_mapping::FileMapping, response_output_format_json as json_ops};
use crate::app::compass::CompassAppError;
use indexmap::IndexMap;
use itertools::Itertools;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ResponseOutputFormat {
    /// writes outputs to a JSON file as either ECMA-404 JSON or as newline-delimited
    /// JSON.
    Json { newline_delimited: bool },
    /// writes outputs to a CSV file given the specified mapping. by default, column
    /// order matches the order of keys in the map, unless "sorted" is true, in which
    /// case the fields are sorted lexicagraphically.
    Csv {
        mapping: IndexMap<String, FileMapping>,
        sorted: bool,
    },
    Parquet {
        mapping: Option<IndexMap<String, FileMapping>>,
    },
}

impl ResponseOutputFormat {
    /// generates the output header content, if it exists for this format.
    pub fn generate_header(&self) -> Option<String> {
        match self {
            ResponseOutputFormat::Json { newline_delimited } => {
                json_ops::initial_file_contents(*newline_delimited)
            }
            ResponseOutputFormat::Csv { mapping, sorted } => {
                let header = if *sorted {
                    mapping.keys().sorted().join(",")
                } else {
                    mapping.keys().rev().join(",")
                };
                Some(format!("{header}\n"))
            }
            ResponseOutputFormat::Parquet { .. } => None,
        }
    }

    /// generates the output footer content, if it exists for this format.
    pub fn generate_footer(&self) -> Option<String> {
        match self {
            ResponseOutputFormat::Json { newline_delimited } => {
                json_ops::final_file_contents(*newline_delimited)
            }
            ResponseOutputFormat::Csv { .. } => None,
            ResponseOutputFormat::Parquet { .. } => None,
        }
    }

    pub fn format_response(
        &self,
        response: &mut serde_json::Value,
    ) -> Result<String, CompassAppError> {
        match self {
            ResponseOutputFormat::Json { newline_delimited } => {
                json_ops::format_response(response, *newline_delimited)
            }
            ResponseOutputFormat::Parquet { .. } => Ok(String::new()),
            ResponseOutputFormat::Csv { mapping, sorted } => {
                let mut errors: HashMap<String, String> = HashMap::new();
                let row = if *sorted {
                    mapping
                        .iter()
                        .sorted_by_key(|(k, _)| *k)
                        .map(|(k, v)| match v.apply_mapping(response) {
                            Ok(cell) => cell.to_string(),
                            Err(msg) => {
                                errors.insert(k.clone(), msg);
                                String::from("")
                            }
                        })
                        .join(",")
                } else {
                    mapping
                        .iter()
                        .rev()
                        .map(|(k, v)| match v.apply_mapping(response) {
                            Ok(cell) => cell.to_string(),
                            Err(msg) => {
                                errors.insert(k.clone(), msg);
                                String::from("")
                            }
                        })
                        .join(",")
                };

                if !errors.is_empty() {
                    response["error"] = json![{"csv": json![errors]}];
                }
                Ok(row)
            }
        }
    }

    pub fn delimiter(&self) -> Option<String> {
        match self {
            ResponseOutputFormat::Json { newline_delimited } => {
                json_ops::delimiter(*newline_delimited)
            }
            ResponseOutputFormat::Csv {
                mapping: _,
                sorted: _,
            } => Some(String::from("\n")),
            ResponseOutputFormat::Parquet { .. } => None,
        }
    }
}
