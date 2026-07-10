use fasteval::Compiler;
use serde_json_path::JsonPath;

use crate::plugin::{
    output::default::eval::{
        ops::{self, parse_path_segments, PathSegment},
        ExpressionConfig, OnFailureBehavior,
    },
    PluginError,
};

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
    Record {
        segments: Vec<PathSegment>,
        limit: Option<usize>,
    },
    Ignore,
}

impl TryFrom<&ExpressionConfig> for CompiledExpression {
    type Error = PluginError;

    fn try_from(conf: &ExpressionConfig) -> Result<CompiledExpression, PluginError> {
        let inputs = conf
            .inputs
            .iter()
            .map(|(name, path_str)| {
                let path = JsonPath::parse(&path_str).map_err(|e| {
                    crate::plugin::PluginError::BuildFailed(format!(
                        "invalid JSONPath '{path_str}' for input '{name}': {e}"
                    ))
                })?;
                Ok((name.clone(), path))
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
            expr: conf.expr.clone(),
            output_segments,
            slab,
            compiled,
        })
    }
}

impl TryFrom<OnFailureBehavior> for CompiledOnFailure {
    type Error = PluginError;

    fn try_from(value: OnFailureBehavior) -> Result<Self, Self::Error> {
        match value {
            OnFailureBehavior::Interrupt => Ok(CompiledOnFailure::Interrupt),
            OnFailureBehavior::Ignore => Ok(CompiledOnFailure::Ignore),
            OnFailureBehavior::Record { path, limit } => {
                let segments = ops::parse_path_segments(&path).map_err(|e| {
                    crate::plugin::PluginError::BuildFailed(format!(
                        "invalid on_failure record path '{path}': {e}"
                    ))
                })?;
                Ok(CompiledOnFailure::Record { segments, limit })
            }
        }
    }
}
