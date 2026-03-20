use std::str::FromStr;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

/// arithmetic operation to apply during evaluation.
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
            (Operation::Sqrt, [x]) => Ok(x.sqrt()),
            (Operation::Abs, [x]) => Ok(x.abs()),
            (Operation::Floor, [x]) => Ok(x.floor()),
            (Operation::Ceil, [x]) => Ok(x.ceil()),
            (Operation::Round, [x]) => Ok(x.round()),
            (Operation::Exp, [x]) => Ok(x.exp()),
            (Operation::Ln, [x]) => Ok(x.ln()),
            (Operation::Log2, [x]) => Ok(x.log2()),
            (Operation::Log10, [x]) => Ok(x.log10()),
            (Operation::Log, [x, b]) => Ok(x.log(*b)),
            (Operation::Sin, [x]) => Ok(x.sin()),
            (Operation::Cos, [x]) => Ok(x.cos()),
            (Operation::Tan, [x]) => Ok(x.tan()),
            (Operation::Asin, [x]) => Ok(x.asin()),
            (Operation::Acos, [x]) => Ok(x.acos()),
            (Operation::Atan, [x]) => Ok(x.atan()),
            (Operation::Atan2, [y, x]) => Ok(y.atan2(*x)),
            (Operation::Min, [a, b]) => Ok(a.min(*b)),
            (Operation::Max, [a, b]) => Ok(a.max(*b)),
            (Operation::Pow, [b, e]) => Ok(b.powf(*e)),
            _ => Err(format!(
                "wrong number of arguments for '{}' found {:?}",
                self.describe(),
                args
            )),
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
            _ => "(x)",
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
