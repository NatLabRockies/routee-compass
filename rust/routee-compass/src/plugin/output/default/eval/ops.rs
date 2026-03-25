use std::{collections::HashMap, str::FromStr};

use fasteval::{Compiler, Evaler};
use itertools::Itertools;
use serde_json_path::JsonPath;

use crate::plugin::output::{
    default::eval::{config::ExpressionConfig, Operation},
    OutputPluginError,
};

/// A pre-parsed path segment used for writing results back into the JSON tree.
pub enum PathSegment {
    Key(String),
    Index(usize),
}

/// One expression with its inputs pre-parsed into [`JsonPath`]s and its output
/// path pre-parsed into [`PathSegment`]s so the per-query hot path is as cheap
/// as possible.
pub struct CompiledExpression {
    /// `(variable_name, compiled JSONPath)` pairs for each input binding.
    pub inputs: Vec<(String, JsonPath)>,
    /// Raw expression string retained for error messages.
    pub expr: String,
    /// Pre-parsed output path segments.
    pub output_segments: Vec<PathSegment>,
    /// Memory arena that backs the pre-compiled bytecode.
    pub slab: fasteval::Slab,
    /// Pre-compiled bytecode produced by `fasteval`'s compiler.
    pub compiled: fasteval::Instruction,
}

/// Compiled form of [`OnFailureBehavior`] — the `Record` variant's path is
/// pre-parsed once at plugin construction time.
pub enum CompiledOnFailure {
    Interrupt,
    Record { segments: Vec<PathSegment> },
    Ignore,
}

// ---------------------------------------------------------------------------
// Config → CompiledExpression
// ---------------------------------------------------------------------------

pub fn compile_expression(
    conf: ExpressionConfig,
) -> Result<CompiledExpression, crate::plugin::PluginError> {
    let inputs = conf
        .inputs
        .into_iter()
        .map(|(name, path_str)| {
            let path = JsonPath::parse(&path_str).map_err(|e| {
                crate::plugin::PluginError::BuildFailed(format!(
                    "invalid JSONPath '{path_str}' for input '{name}': {e}"
                ))
            })?;
            Ok((name, path))
        })
        .collect::<Result<Vec<_>, crate::plugin::PluginError>>()?;

    let output_segments = parse_path_segments(&conf.output).map_err(|e| {
        crate::plugin::PluginError::BuildFailed(format!(
            "invalid output path '{}': {e}",
            conf.output
        ))
    })?;

    // Parse and compile the fasteval expression once so that eval_and_write can
    // skip the parse step on every row.
    let mut slab = fasteval::Slab::new();
    let parsed = fasteval::Parser::new()
        .parse(&conf.expr, &mut slab.ps)
        .map_err(|e| {
            crate::plugin::PluginError::BuildFailed(format!(
                "failed to parse expression '{}': {e}",
                conf.expr
            ))
        })?;
    let compiled = parsed.from(&slab.ps).compile(&slab.ps, &mut slab.cs);

    Ok(CompiledExpression {
        inputs,
        expr: conf.expr,
        output_segments,
        slab,
        compiled,
    })
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Parse a JSONPath-style string into segments for writing.
///
/// Supports dot notation (`$.a.b`) and bracket array indices (`$.a[0].b`).
/// The leading `$.` or `$` prefix is stripped before parsing.
pub fn parse_path_segments(path: &str) -> Result<Vec<PathSegment>, OutputPluginError> {
    let stripped = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(path);

    if stripped.is_empty() {
        return Err(OutputPluginError::OutputPluginFailed(
            "output path must not be empty after '$'".to_string(),
        ));
    }

    let mut segments = Vec::new();
    let mut current_key = String::new();
    let mut chars = stripped.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '.' => {
                if !current_key.is_empty() {
                    segments.push(PathSegment::Key(std::mem::take(&mut current_key)));
                }
            }
            '[' => {
                if !current_key.is_empty() {
                    segments.push(PathSegment::Key(std::mem::take(&mut current_key)));
                }
                let mut bracket_content = String::new();
                for ic in chars.by_ref() {
                    if ic == ']' {
                        break;
                    }
                    bracket_content.push(ic);
                }
                // Quoted string key: ['foo'] or ["foo"]
                let segment = if (bracket_content.starts_with('\'')
                    && bracket_content.ends_with('\''))
                    || (bracket_content.starts_with('"') && bracket_content.ends_with('"'))
                {
                    let key = bracket_content[1..bracket_content.len() - 1].to_string();
                    PathSegment::Key(key)
                } else {
                    // Numeric array index: [0]
                    let idx: usize = bracket_content.parse().map_err(|_| {
                        OutputPluginError::OutputPluginFailed(format!(
                            "invalid bracket segment '[{bracket_content}]' in path '{path}': \
                            expected an integer index or a quoted string key"
                        ))
                    })?;
                    PathSegment::Index(idx)
                };
                segments.push(segment);
            }
            other => current_key.push(other),
        }
    }

    if !current_key.is_empty() {
        segments.push(PathSegment::Key(current_key));
    }

    Ok(segments)
}

/// Recursively walk `root` along `segments` and write `value` at the final location.
/// Intermediate objects that do not exist are created automatically.
pub fn set_path(
    root: &mut serde_json::Value,
    segments: &[PathSegment],
    value: serde_json::Value,
) -> Result<(), OutputPluginError> {
    match segments {
        [] => Err(OutputPluginError::OutputPluginFailed(
            "empty output path".to_string(),
        )),

        [PathSegment::Key(k)] => {
            root.as_object_mut()
                .ok_or_else(|| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "cannot write key '{k}' into a non-object JSON value"
                    ))
                })?
                .insert(k.clone(), value);
            Ok(())
        }

        [PathSegment::Index(i)] => {
            let arr = root.as_array_mut().ok_or_else(|| {
                OutputPluginError::OutputPluginFailed(format!(
                    "cannot index a non-array JSON value with [{i}]"
                ))
            })?;
            if *i < arr.len() {
                arr[*i] = value;
                Ok(())
            } else {
                Err(OutputPluginError::OutputPluginFailed(format!(
                    "array index {i} is out of bounds (length {})",
                    arr.len()
                )))
            }
        }

        [PathSegment::Key(k), rest @ ..] => {
            if !root.is_object() {
                *root = serde_json::Value::Object(serde_json::Map::new());
            }
            let child = root
                .as_object_mut()
                .unwrap()
                .entry(k.clone())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            set_path(child, rest, value)
        }

        [PathSegment::Index(i), rest @ ..] => {
            let arr = root.as_array_mut().ok_or_else(|| {
                OutputPluginError::OutputPluginFailed(format!(
                    "cannot index a non-array JSON value with [{i}]"
                ))
            })?;
            let child = arr.get_mut(*i).ok_or_else(|| {
                OutputPluginError::OutputPluginFailed(format!("array index {i} is out of bounds"))
            })?;
            set_path(child, rest, value)
        }
    }
}

/// Append an error entry `{ "expr": ..., "error": ... }` to the array located
/// at `segments` within `root`. If the target location does not yet hold an
/// array it is replaced with a single-element array.
pub fn record_error(
    root: &mut serde_json::Value,
    segments: &[PathSegment],
    expr_str: &str,
    message: &str,
) -> Result<(), OutputPluginError> {
    let entry = serde_json::json!({ "expr": expr_str, "error": message });

    // Navigate to the parent, then handle the final segment manually so we can
    // push into (or create) an array rather than blindly overwriting.
    let (last, parent_segments) = match segments.split_last() {
        Some(pair) => pair,
        None => {
            return Err(OutputPluginError::OutputPluginFailed(
                "on_failure record path must not be empty".to_string(),
            ))
        }
    };

    // Walk to the parent node, creating intermediate objects as needed.
    let parent = navigate_mut(root, parent_segments)?;

    match last {
        PathSegment::Key(k) => {
            let obj = parent.as_object_mut().ok_or_else(|| {
                OutputPluginError::OutputPluginFailed(format!(
                    "cannot record error under key '{k}' in a non-object value"
                ))
            })?;
            let slot = obj
                .entry(k.clone())
                .or_insert_with(|| serde_json::Value::Array(vec![]));
            if let Some(arr) = slot.as_array_mut() {
                arr.push(entry);
            } else {
                *slot = serde_json::Value::Array(vec![entry]);
            }
        }
        PathSegment::Index(i) => {
            let arr = parent.as_array_mut().ok_or_else(|| {
                OutputPluginError::OutputPluginFailed(format!(
                    "cannot index a non-array JSON value with [{i}] while recording error"
                ))
            })?;
            if *i < arr.len() {
                if let Some(slot_arr) = arr[*i].as_array_mut() {
                    slot_arr.push(entry);
                } else {
                    arr[*i] = serde_json::Value::Array(vec![entry]);
                }
            } else {
                return Err(OutputPluginError::OutputPluginFailed(format!(
                    "array index {i} is out of bounds while recording error"
                )));
            }
        }
    }

    Ok(())
}

/// Walk `root` along `segments`, creating intermediate objects where missing,
/// and return a mutable reference to the node at the end of the path.
fn navigate_mut<'a>(
    root: &'a mut serde_json::Value,
    segments: &[PathSegment],
) -> Result<&'a mut serde_json::Value, OutputPluginError> {
    let mut current = root;
    for seg in segments {
        match seg {
            PathSegment::Key(k) => {
                if !current.is_object() {
                    *current = serde_json::Value::Object(serde_json::Map::new());
                }
                current = current
                    .as_object_mut()
                    .unwrap()
                    .entry(k.clone())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            }
            PathSegment::Index(i) => {
                let arr = current.as_array_mut().ok_or_else(|| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "cannot index a non-array JSON value with [{i}]"
                    ))
                })?;
                current = arr.get_mut(*i).ok_or_else(|| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "array index {i} is out of bounds"
                    ))
                })?;
            }
        }
    }
    Ok(current)
}

// ---------------------------------------------------------------------------
// Per-expression evaluation
// ---------------------------------------------------------------------------

/// Resolve inputs, evaluate the expression, and write the result to `output`.
/// Returns `Err` for any failure (bad JSONPath query, non-numeric input,
/// expression error, non-finite result, or write error).
pub fn eval_and_write(
    expr: &CompiledExpression,
    output: &mut serde_json::Value,
) -> Result<(), OutputPluginError> {
    // 1. Resolve each input binding to an f64 via JSONPath.
    let mut variables: HashMap<String, f64> = HashMap::new();
    for (name, path) in &expr.inputs {
        let node = path.query(output).exactly_one().map_err(|e| {
            OutputPluginError::OutputPluginFailed(format!(
                "JSONPath query for input '{name}' did not return exactly one result: {e}"
            ))
        })?;
        let f = node.as_f64().ok_or_else(|| {
            OutputPluginError::OutputPluginFailed(format!(
                "input '{name}' is not a number (value: {node})"
            ))
        })?;
        variables.insert(name.clone(), f);
    }

    // 2. Evaluate the pre-compiled fasteval bytecode, providing both user
    //    variables and common math functions via the callback.
    let mut callback_errors: Vec<String> = vec![];
    let eval_result = expr
        .compiled
        .eval(&expr.slab, &mut |name: &str, args: Vec<f64>| {
            // Zero-arg identifiers are variable lookups.
            if args.is_empty() {
                return variables.get(name).copied();
            }

            let op = match Operation::from_str(name) {
                Ok(operation) => Some(operation),
                Err(e) => {
                    callback_errors.push(e);
                    None
                }
            }?;

            let result = match op.apply(args.as_slice()) {
                Ok(result) => Some(result),
                Err(e) => {
                    callback_errors.push(e);
                    None
                }
            }?;

            Some(result)
        })
        .map_err(|e| {
            OutputPluginError::OutputPluginFailed(format!(
                "error evaluating expression '{}': {e}",
                expr.expr
            ))
        });

    // propagate the callback errors in priority over fasteval errors which would be less descriptive
    if !callback_errors.is_empty() {
        let msg = callback_errors.into_iter().join("\n");
        return Err(OutputPluginError::OutputPluginFailed(format!(
            "failure evaluating expression operations: {msg}"
        )));
    }
    let result = eval_result?;

    // 3. Write the result back into the output JSON.
    let number = serde_json::Number::from_f64(result).ok_or_else(|| {
        OutputPluginError::OutputPluginFailed(format!(
            "expression '{}' produced a non-finite value: {result}",
            expr.expr
        ))
    })?;
    set_path(
        output,
        &expr.output_segments,
        serde_json::Value::Number(number),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::plugin::output::default::eval::config::ExpressionConfig;

    fn make_compiled_expr(inputs: &[(&str, &str)], expr: &str, output: &str) -> CompiledExpression {
        compile_expression(ExpressionConfig {
            inputs: inputs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            expr: expr.to_string(),
            output: output.to_string(),
        })
        .expect("expression should compile")
    }

    // -----------------------------------------------------------------------
    // parse_path_segments
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_single_key() {
        let segs = parse_path_segments("$.foo").unwrap();
        assert_eq!(segs.len(), 1);
        assert!(matches!(&segs[0], PathSegment::Key(k) if k == "foo"));
    }

    #[test]
    fn test_parse_nested_keys() {
        let segs = parse_path_segments("$.a.b.c").unwrap();
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[0], PathSegment::Key(k) if k == "a"));
        assert!(matches!(&segs[1], PathSegment::Key(k) if k == "b"));
        assert!(matches!(&segs[2], PathSegment::Key(k) if k == "c"));
    }

    #[test]
    fn test_parse_bracket_single_quoted_string_key() {
        // $.a['10'].b  →  [Key("a"), Key("10"), Key("b")]
        let segs = parse_path_segments("$.a['10'].b").unwrap();
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[0], PathSegment::Key(k) if k == "a"));
        assert!(matches!(&segs[1], PathSegment::Key(k) if k == "10"));
        assert!(matches!(&segs[2], PathSegment::Key(k) if k == "b"));
    }

    #[test]
    fn test_parse_bracket_double_quoted_string_key() {
        // $.a["spaced key"].b  →  [Key("a"), Key("spaced key"), Key("b")]
        let segs = parse_path_segments("$.a[\"spaced key\"].b").unwrap();
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[0], PathSegment::Key(k) if k == "a"));
        assert!(matches!(&segs[1], PathSegment::Key(k) if k == "spaced key"));
        assert!(matches!(&segs[2], PathSegment::Key(k) if k == "b"));
    }

    #[test]
    fn test_parse_bracket_array_index() {
        let segs = parse_path_segments("$.arr[2].val").unwrap();
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[0], PathSegment::Key(k) if k == "arr"));
        assert!(matches!(&segs[1], PathSegment::Index(2)));
        assert!(matches!(&segs[2], PathSegment::Key(k) if k == "val"));
    }

    #[test]
    fn test_parse_bare_dollar_fails() {
        assert!(parse_path_segments("$").is_err());
    }

    // -----------------------------------------------------------------------
    // set_path
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_path_shallow_key() {
        let mut root = json!({});
        let segs = parse_path_segments("$.result").unwrap();
        set_path(&mut root, &segs, json!(42.0)).unwrap();
        assert_eq!(root["result"], json!(42.0));
    }

    #[test]
    fn test_set_path_creates_intermediate_objects() {
        let mut root = json!({});
        let segs = parse_path_segments("$.a.b.c").unwrap();
        set_path(&mut root, &segs, json!(99.0)).unwrap();
        assert_eq!(root["a"]["b"]["c"], json!(99.0));
    }

    #[test]
    fn test_set_path_overwrites_existing_key() {
        let mut root = json!({ "x": 1.0 });
        let segs = parse_path_segments("$.x").unwrap();
        set_path(&mut root, &segs, json!(2.0)).unwrap();
        assert_eq!(root["x"], json!(2.0));
    }

    #[test]
    fn test_set_path_array_index() {
        let mut root = json!({ "arr": [10.0, 20.0, 30.0] });
        let segs = parse_path_segments("$.arr[1]").unwrap();
        set_path(&mut root, &segs, json!(99.0)).unwrap();
        assert_eq!(root["arr"][1], json!(99.0));
    }
}
