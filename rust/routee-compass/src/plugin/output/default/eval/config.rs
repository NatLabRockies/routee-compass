use std::{collections::HashMap, str::FromStr};

use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::plugin::output::OutputPluginError;

/// Configure the Eval plugin to perform a set of expressions on output rows.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EvalOutputPluginConfig {
    /// expressions to run for each row
    pub expressions: Vec<ExpressionConfig>,
    /// behavior when expression fails
    pub on_failure: OnFailureBehavior
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
    pub expr: String,
    /// JSONPath location to write the computed `f64` result. Intermediate objects are
    /// created automatically if they do not already exist.
    pub output: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Sqrt,
    Abs,
    Floor,
    Ceil,
    Round,
    Exp,
    Ln,
    Log2,
    Log10,
    Log,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Min,
    Max,
    Pow,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OnFailureBehavior {
    /// interrupt the plugin on failure, returning an Err from the plugin.
    Interrupt,
    /// record the error to the output row at some JSONpath
    Record {
        path: String
    },
    /// ignore the error
    Ignore
}

impl Operation {
    pub const ALL: [Self; 20] = [
        Self::Sqrt,
        Self::Abs,
        Self::Floor,
        Self::Ceil,
        Self::Round,
        Self::Exp,
        Self::Ln,
        Self::Log2,
        Self::Log10,
        Self::Log,
        Self::Sin,
        Self::Cos,
        Self::Tan,
        Self::Asin,
        Self::Acos,
        Self::Atan,
        Self::Atan2,
        Self::Min,
        Self::Max,
        Self::Pow,
    ];
    
    /// apply this operation to a list of variables provided in the expected order
    pub fn apply(&self, args: &[f64]) -> Result<f64, String> {
        match (self, args) {
            (    Operation::Sqrt,  [x])    => Ok(x.sqrt()),
            (    Operation::Abs,   [x])    => Ok(x.abs()),
            (    Operation::Floor, [x])    => Ok(x.floor()),
            (    Operation::Ceil,  [x])    => Ok(x.ceil()),
            (    Operation::Round, [x])    => Ok(x.round()),
            (    Operation::Exp,   [x])    => Ok(x.exp()),
            (    Operation::Ln,    [x])    => Ok(x.ln()),
            (    Operation::Log2,  [x])    => Ok(x.log2()),
            (    Operation::Log10, [x])    => Ok(x.log10()),
            (    Operation::Log,   [x, b]) => Ok(x.log(*b)),
            (    Operation::Sin,   [x])    => Ok(x.sin()),
            (    Operation::Cos,   [x])    => Ok(x.cos()),
            (    Operation::Tan,   [x])    => Ok(x.tan()),
            (    Operation::Asin,  [x])    => Ok(x.asin()),
            (    Operation::Acos,  [x])    => Ok(x.acos()),
            (    Operation::Atan,  [x])    => Ok(x.atan()),
            (    Operation::Atan2, [y, x]) => Ok(y.atan2(*x)),
            (    Operation::Min,   [a, b]) => Ok(a.min(*b)),
            (    Operation::Max,   [a, b]) => Ok(a.max(*b)),
            (    Operation::Pow,   [b, e]) => Ok(b.powf(*e)),     
            _ => Err(format!("wrong number of arguments for '{}' found {:?}", self.describe(), args))
        }
    }

    /// pretty print the math function and its expected argument list
    pub fn describe(&self) -> String {
        let op = self.to_string();
        let args = match self {
            Self::Log => "(x, b)",
            Self::Atan2 => "(y, x)",
            Self::Min | Self::Max => "(a, b)",
            Self::Pow => "(b, e)",
            _ => "(x)"
        };
        format!("{op}{args}")
    }
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Sqrt => "sqrt",
            Self::Abs => "abs",
            Self::Floor => "floor",
            Self::Ceil => "ceil",
            Self::Round => "round",
            Self::Exp => "exp",
            Self::Ln => "ln",
            Self::Log2 => "log2",
            Self::Log10 => "log10",
            Self::Log => "log",
            Self::Sin => "sin",
            Self::Cos => "cos",
            Self::Tan => "tan",
            Self::Asin => "asin",
            Self::Acos => "acos",
            Self::Atan => "atan",
            Self::Atan2 => "atan2",
            Self::Min => "min",
            Self::Max => "max",
            Self::Pow => "pow",
        };
        write!(f, "{s}")
    }
}

impl FromStr for Operation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sqrt" => Ok(Self::Sqrt),
            "abs" => Ok(Self::Abs),
            "floor" => Ok(Self::Floor),
            "ceil" => Ok(Self::Ceil),
            "round" => Ok(Self::Round),
            "exp" => Ok(Self::Exp),
            "ln" => Ok(Self::Ln),
            "log2" => Ok(Self::Log2),
            "log10" => Ok(Self::Log10),
            "log" => Ok(Self::Log),
            "sin" => Ok(Self::Sin),
            "cos" => Ok(Self::Cos),
            "tan" => Ok(Self::Tan),
            "asin" => Ok(Self::Asin),
            "acos" => Ok(Self::Acos),
            "atan" => Ok(Self::Atan),
            "atan2" => Ok(Self::Atan2),
            "min" => Ok(Self::Min),
            "max" => Ok(Self::Max),
            "pow" => Ok(Self::Pow),
            _ => {
                let all_str = Self::ALL.iter().map(|s| s.to_string()).join(",");
                Err(format!("unknown operation {s}; supported ops: [{all_str}]"))
            }
        }
    }
}

impl ExpressionConfig {
    /// create a new ExpressionConfig programatically.
    /// 
    /// # Arguments
    /// * `inputs` - a pair of (variable_name, JSONPath) for each variable we 
    /// want to bind from the output row.
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