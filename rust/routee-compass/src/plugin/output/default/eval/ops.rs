use std::{collections::HashMap, str::FromStr};

use itertools::Itertools;
use serde_json_path::JsonPath;

use crate::plugin::output::{OutputPluginError, default::eval::config::{ExpressionConfig, Operation}};

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
    /// Raw `fasteval` expression string (re-used each invocation).
    pub expr: String,
    /// Pre-parsed output path segments.
    pub output_segments: Vec<PathSegment>,
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

    Ok(CompiledExpression {
        inputs,
        expr: conf.expr,
        output_segments,
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
                let mut idx_str = String::new();
                for ic in chars.by_ref() {
                    if ic == ']' {
                        break;
                    }
                    idx_str.push(ic);
                }
                let idx: usize = idx_str.parse().map_err(|_| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "invalid array index '{idx_str}' in path '{path}'"
                    ))
                })?;
                segments.push(PathSegment::Index(idx));
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
                OutputPluginError::OutputPluginFailed(format!(
                    "array index {i} is out of bounds"
                ))
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

    // 2. Evaluate the fasteval expression, providing both user variables and
    //    common math functions via the callback (ez_eval routes all identifiers —
    //    including function calls — through the user-supplied closure).
    let mut callback_errors: Vec<String> = vec![];
    let eval_result = fasteval::ez_eval(&expr.expr, &mut |name: &str, args: Vec<f64>| {
        // Zero-arg identifiers are variable lookups.
        if args.is_empty() {
            return variables.get(name).copied();
        }
        
            let op = match Operation::from_str(name) {
                Ok(operation) => Some(operation),
                Err(e) => {
                    callback_errors.push(e);
                    None
                },
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
        return Err(OutputPluginError::OutputPluginFailed(format!("failure evaluating expression operations: {msg}")));
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

    // -----------------------------------------------------------------------
    // eval_and_write
    // -----------------------------------------------------------------------

    #[test]
    fn test_eval_simple_multiplication() {
        let expr = make_compiled_expr(&[("a", "$.a"), ("b", "$.b")], "a * b", "$.result");
        let mut output = json!({ "a": 3.0, "b": 4.0 });
        eval_and_write(&expr, &mut output).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 12.0);
    }

    #[test]
    fn test_eval_sqrt_function() {
        let expr = make_compiled_expr(&[("x", "$.x")], "sqrt(x)", "$.result");
        let mut output = json!({ "x": 9.0 });
        eval_and_write(&expr, &mut output).unwrap();
        let result = output["result"].as_f64().unwrap();
        assert!((result - 3.0).abs() < 1e-10, "expected 3.0, got {result}");
    }

    #[test]
    fn test_eval_complex_expression_with_parens() {
        let expr = make_compiled_expr(
            &[("base", "$.base"), ("rate", "$.rate"), ("hours", "$.hours")],
            "(base + rate) * hours",
            "$.total",
        );
        let mut output = json!({ "base": 10.0, "rate": 5.0, "hours": 8.0 });
        eval_and_write(&expr, &mut output).unwrap();
        assert_eq!(output["total"].as_f64().unwrap(), 120.0);
    }

    #[test]
    fn test_eval_writes_to_nested_output_path() {
        let expr = make_compiled_expr(&[("v", "$.v")], "v * 2", "$.out.nested.value");
        let mut output = json!({ "v": 5.0 });
        eval_and_write(&expr, &mut output).unwrap();
        assert_eq!(output["out"]["nested"]["value"].as_f64().unwrap(), 10.0);
    }

    #[test]
    fn test_eval_missing_input_path_fails() {
        let expr = make_compiled_expr(&[("x", "$.missing")], "x * 2", "$.result");
        let mut output = json!({});
        assert!(eval_and_write(&expr, &mut output).is_err());
    }

    #[test]
    fn test_eval_non_numeric_input_fails() {
        let expr = make_compiled_expr(&[("x", "$.x")], "x * 2", "$.result");
        let mut output = json!({ "x": "not_a_number" });
        assert!(eval_and_write(&expr, &mut output).is_err());
    }
}
