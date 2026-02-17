use itertools::Itertools;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
#[serde(untagged)]
pub enum FileMapping {
    Path(String),
    Sum {
        sum: Vec<Box<FileMapping>>,
    },
    Optional {
        optional: Box<FileMapping>,
        #[serde(default)]
        dtype: Option<String>,
    },
}

impl FileMapping {
    // const CSV_ERR_SEPARATOR: &'static str = "-";

    // /// applies a user-specified JSON to CSV mapping. if the mapping fails,
    // /// a helpful error message is returned.
    pub fn apply_mapping(&self, json: &serde_json::Value) -> Result<serde_json::Value, String> {
        match self {
            FileMapping::Path(p) => {
                let split_path = p.split('.').collect_vec();
                traverse(json, &split_path)
            }
            FileMapping::Sum { sum } => {
                let (ok, err): (Vec<_>, Vec<_>) =
                    sum.iter().map(|m| m.apply_mapping(json)).partition_result();
                if !err.is_empty() {
                    let valid = if ok.is_empty() {
                        String::from("")
                    } else {
                        format!(" with these valid paths: {}", ok.iter().join(", "))
                    };
                    let invalid = err.join(", ");
                    let msg = format!("unable to sum invalid paths: {invalid}{valid}");
                    Err(msg)
                } else {
                    let (nums, num_errs): (Vec<_>, Vec<_>) = ok
                        .iter()
                        .map(|v| match v {
                            serde_json::Value::Null => Ok(0.0),
                            serde_json::Value::Number(n) => {
                                n.as_f64().ok_or_else(|| format!("invalid number {v}"))
                            }
                            _ => Err(format!("expected a number, found {v}")),
                        })
                        .partition_result();
                    if !num_errs.is_empty() {
                        let valid = if nums.is_empty() {
                            String::from("")
                        } else {
                            format!(" with these valid numbers: {}", nums.iter().join(", "))
                        };
                        let invalid = num_errs.join(", ");
                        let msg = format!("unable to sum invalid numbers: {invalid}{valid}");
                        Err(msg)
                    } else {
                        Ok(json![nums.into_iter().sum::<f64>()])
                    }
                }
            }
            FileMapping::Optional { optional, .. } => match optional.apply_mapping(json).ok() {
                Some(value) => Ok(value),
                None => Ok(serde_json::Value::Null),
            },
        }
    }
}

fn traverse(value: &serde_json::Value, path: &Vec<&str>) -> Result<serde_json::Value, String> {
    let mut cursor = value;
    let mut remaining = path.as_slice();
    while let Some(next) = remaining.first() {
        match cursor.get(*next) {
            None => {
                let path_str = path.iter().join(".");
                return Err(format!("could not find object {next} in path {path_str}"));
            }
            Some(child) => {
                cursor = child;
                remaining = &remaining[1..];
            }
        }
    }
    Ok(cursor.clone())
}
