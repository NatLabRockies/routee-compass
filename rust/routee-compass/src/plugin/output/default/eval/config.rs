use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::plugin::output::OutputPluginError;

/// Configure the Eval plugin to perform a set of expressions on output rows.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvalOutputPluginConfig {
    /// expressions to run for each row
    pub expressions: Vec<ExpressionConfig>,
    /// behavior when expression fails
    pub on_failure: OnFailureBehavior,
    /// behavior when a NaN value is produced by an expression. by default,
    /// NaN values are allowed.
    #[serde(default)]
    pub on_nan: NotANumberBehavior,
}

/// Configuration for a single arithmetic expression to evaluate over the output JSON.
///
/// Each expression resolves named inputs from the JSON using JSONPath queries,
/// evaluates a `fasteval` arithmetic expression over those inputs, and writes
/// the resulting `f64` to an output JSONPath location.
///
/// # Example (TOML)
///
/// ```toml
/// {
///     inputs = {
///         time_delay = "$.metric.time_delay",
///         per_hour = "$.cost.per_hour"
///     },
///     expr = "time_delay * per_hour",
///     output = "$.cost.delay_cost"
/// }
/// ```
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExpressionConfig {
    /// Map of variable name → JSONPath. Each JSONPath must resolve to a single
    /// numeric value in the output JSON.
    pub inputs: HashMap<String, String>,
    /// A `fasteval` arithmetic expression using the variable names defined in `inputs`.
    /// Supports `+`, `-`, `*`, `/`, `^`, parentheses, and built-in functions such as
    /// `sqrt`, `log`, `log2`, `log10`, `exp`, `abs`, `min`, `max`, `sin`, `cos`, etc.
    /// for a complete list, see the [`super::Operation`] type.
    pub expr: String,
    /// JSONPath location to write the computed `f64` result. Intermediate objects are
    /// created automatically if they do not already exist.
    pub output: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum OnFailureBehavior {
    /// interrupt the plugin on failure, returning an Err from the plugin.
    Interrupt,
    /// record the error to the output row at some JSONpath
    Record {
        /// JSONPath declaring where to write eval errors from this plugin instance
        path: String,
        /// limit to the depth of errors to show, starting from the first.
        /// if not provided, shows the full collection of errors, but if multiple
        /// expressions are inter-dependent, limiting to 1 or few errors is usually
        /// sufficient to identify the source of a chain of errors.
        limit: Option<usize>,
        /// write the list of errors to the output row as a serde_json::Value::String
        /// instead of as an array. used when writing eval errors to CSV rows.
        #[serde(default)]
        as_string: bool,
    },
    /// ignore the error
    Ignore,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum NotANumberBehavior {
    /// allow NaNs to get written
    #[default]
    Allow,
    /// zero out NaN values when they are the result of an expression
    Zero,
    /// error out if we return a NaN
    Error,
}

impl ExpressionConfig {
    /// create a new ExpressionConfig programmatically.
    ///
    /// # Arguments
    /// * `inputs` - a pair of (variable_name, JSONPath) for each variable we want to bind from the output row.
    /// * `expression` - mathematical expression
    pub fn new(inputs: &[(&str, &str)], expression: &str, output: &str) -> Self {
        Self {
            inputs: inputs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>(),
            expr: expression.to_string(),
            output: output.to_string(),
        }
    }
}

impl NotANumberBehavior {
    pub fn apply(&self, value: f64) -> Result<f64, OutputPluginError> {
        if f64::is_nan(value) {
            match self {
                NotANumberBehavior::Allow => Ok(value),
                NotANumberBehavior::Zero => Ok(0.0),
                NotANumberBehavior::Error => {
                    let msg = "encountered NaN value when NotANumberBehavior is Error".to_string();
                    Err(OutputPluginError::OutputPluginFailed(msg))
                }
            }
        } else {
            Ok(value)
        }
    }
}
