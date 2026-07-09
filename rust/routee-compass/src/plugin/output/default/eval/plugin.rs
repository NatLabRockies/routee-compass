
use crate::plugin::output::{
    default::eval::{
        ops::{self, CompiledExpression, CompiledOnFailure},
        NotANumberBehavior,
    },
    OutputPlugin,
};

use super::config::{EvalOutputPluginConfig, OnFailureBehavior};

pub struct EvalOutputPlugin {
    expressions: Vec<CompiledExpression>,
    on_failure: CompiledOnFailure,
    on_nan: NotANumberBehavior,
}

const NAME: &str = "eval";

impl EvalOutputPlugin {
    pub fn new(conf: EvalOutputPluginConfig) -> Result<Self, crate::plugin::PluginError> {
        let expressions = conf
            .expressions
            .into_iter()
            .map(ops::compile_expression)
            .collect::<Result<Vec<_>, _>>()?;

        let on_failure = match conf.on_failure {
            OnFailureBehavior::Interrupt => CompiledOnFailure::Interrupt,
            OnFailureBehavior::Ignore => CompiledOnFailure::Ignore,
            OnFailureBehavior::Record { path, limit } => {
                let segments = ops::parse_path_segments(&path).map_err(|e| {
                    crate::plugin::PluginError::BuildFailed(format!(
                        "invalid on_failure record path '{path}': {e}"
                    ))
                })?;
                CompiledOnFailure::Record { segments, limit }
            }
        };

        Ok(Self {
            expressions,
            on_failure,
            on_nan: conf.on_nan,
        })
    }
}

impl OutputPlugin for EvalOutputPlugin {
    fn process(
        &self,
        output: &mut serde_json::Value,
        _result: &Result<
            (
                crate::app::search::SearchAppResult,
                routee_compass_core::algorithm::search::SearchInstance,
            ),
            crate::app::compass::CompassAppError,
        >,
    ) -> Result<(), crate::plugin::output::OutputPluginError> {
        for expr in &self.expressions {
            match ops::eval_and_write(expr, output, &self.on_nan) {
                Ok(()) => {}
                Err(e) => match &self.on_failure {
                    CompiledOnFailure::Interrupt => return Err(e),
                    CompiledOnFailure::Ignore => {}
                    CompiledOnFailure::Record { segments, limit } => {
                        let segments_recorded = match limit {
                            Some(lim) => segments.iter().take(*lim).cloned().collect::<Vec<_>>(),
                            None => segments.clone(),
                        };
                        ops::record_error(output, &segments_recorded, &expr.expr, &e.to_string())?;
                    }
                },
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        NAME
    }
}

#[cfg(test)]
mod tests {

    use serde_json::json;

    use crate::{
        app::compass::CompassAppError,
        plugin::output::{
            default::eval::config::{EvalOutputPluginConfig, ExpressionConfig, OnFailureBehavior},
            OutputPlugin,
        },
    };

    use super::EvalOutputPlugin;

    /// mocks a dummy search result for tests wrapped by OutputPlugin.
    /// the plugin ignores it, as it only interacts with the output JSON row, so an Err here is fine.
    type DummyResult = Result<
        (
            crate::app::search::SearchAppResult,
            routee_compass_core::algorithm::search::SearchInstance,
        ),
        CompassAppError,
    >;
    fn dummy_result() -> DummyResult {
        Err(CompassAppError::InternalError("test".to_string()))
    }

    /// creates a plugin instance for testing.
    fn build_plugin(
        expressions: Vec<ExpressionConfig>,
        on_failure: OnFailureBehavior,
    ) -> EvalOutputPlugin {
        EvalOutputPlugin::new(EvalOutputPluginConfig {
            expressions,
            on_failure,
            on_nan: Default::default(),
        })
        .expect("plugin should build")
    }

    // -----------------------------------------------------------------------
    // Success cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_success_simple_arithmetic() {
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[
                    ("delay", "$.metric.time_delay"),
                    ("rate", "$.cost.per_hour"),
                ],
                "delay * rate",
                "$.cost.delay_cost",
            )],
            OnFailureBehavior::Interrupt,
        );

        let mut output = json!({
            "metric": { "time_delay": 2.5 },
            "cost":   { "per_hour": 4.0 }
        });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["cost"]["delay_cost"].as_f64().unwrap(), 10.0);
    }

    #[test]
    fn test_success_multiple_expressions_run_in_order() {
        // Second expression reads a value written by the first.
        let plugin = build_plugin(
            vec![
                ExpressionConfig::new(&[("x", "$.x")], "x * 2", "$.doubled"),
                ExpressionConfig::new(&[("d", "$.doubled")], "d + 1", "$.result"),
            ],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 3.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["doubled"].as_f64().unwrap(), 6.0);
        assert_eq!(output["result"].as_f64().unwrap(), 7.0);
    }

    #[test]
    fn test_success_creates_intermediate_objects() {
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("v", "$.v")],
                "v ^ 2",
                "$.stats.squared.value",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "v": 5.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["stats"]["squared"]["value"].as_f64().unwrap(), 25.0);
    }

    #[test]
    fn test_success_subtraction() {
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("a", "$.a"), ("b", "$.b")],
                "a - b",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "a": 10.0, "b": 3.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 7.0);
    }

    #[test]
    fn test_success_division() {
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("a", "$.a"), ("b", "$.b")],
                "a / b",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "a": 10.0, "b": 4.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 2.5);
    }

    #[test]
    fn test_success_overwrites_existing_output() {
        // Writing to a path that already holds a value should replace it.
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "x * 2", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 5.0, "result": 999.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 10.0);
    }

    // -----------------------------------------------------------------------
    // Interrupt behavior
    // -----------------------------------------------------------------------

    #[test]
    fn test_interrupt_returns_err_on_missing_input() {
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.missing")],
                "x * 2",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({});
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_interrupt_stops_after_first_failure() {
        // Two expressions: first fails, second would succeed and write "$.sentinel".
        let plugin = build_plugin(
            vec![
                ExpressionConfig::new(&[("x", "$.missing")], "x * 2", "$.result"),
                ExpressionConfig::new(&[("y", "$.y")], "y + 1", "$.sentinel"),
            ],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "y": 10.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
        // The second expression must not have run.
        assert!(output.get("sentinel").is_none());
    }

    // -----------------------------------------------------------------------
    // Ignore behavior
    // -----------------------------------------------------------------------

    #[test]
    fn test_ignore_continues_after_failure() {
        // First expression fails; second must still run.
        let plugin = build_plugin(
            vec![
                ExpressionConfig::new(&[("x", "$.missing")], "x * 2", "$.result"),
                ExpressionConfig::new(&[("y", "$.y")], "y + 1", "$.sentinel"),
            ],
            OnFailureBehavior::Ignore,
        );
        let mut output = json!({ "y": 10.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["sentinel"].as_f64().unwrap(), 11.0);
    }

    // -----------------------------------------------------------------------
    // Record behavior
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_appends_error_and_returns_ok() {
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.missing")],
                "x * 2",
                "$.result",
            )],
            OnFailureBehavior::Record {
                path: "$.eval_errors".to_string(),
                limit: None,
            },
        );
        let mut output = json!({});
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["eval_errors"]
            .as_array()
            .expect("should be an array");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["expr"], json!("x * 2"));
        assert!(errors[0]["error"].is_string());
    }

    #[test]
    fn test_record_continues_after_failure() {
        let plugin = build_plugin(
            vec![
                ExpressionConfig::new(&[("x", "$.missing")], "x * 2", "$.result"),
                ExpressionConfig::new(&[("y", "$.y")], "y + 1", "$.sentinel"),
            ],
            OnFailureBehavior::Record {
                path: "$.eval_errors".to_string(),
                limit: None,
            },
        );
        let mut output = json!({ "y": 5.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["eval_errors"]
            .as_array()
            .expect("should be an array");
        assert_eq!(errors.len(), 1);
        // The second expression must still have run.
        assert_eq!(output["sentinel"].as_f64().unwrap(), 6.0);
    }

    #[test]
    fn test_record_accumulates_multiple_failures() {
        let plugin = build_plugin(
            vec![
                ExpressionConfig::new(&[("x", "$.missing_x")], "x * 2", "$.r1"),
                ExpressionConfig::new(&[("y", "$.missing_y")], "y + 1", "$.r2"),
            ],
            OnFailureBehavior::Record {
                path: "$.eval_errors".to_string(),
                limit: None,
            },
        );
        let mut output = json!({});
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["eval_errors"]
            .as_array()
            .expect("should be an array");
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0]["expr"], json!("x * 2"));
        assert_eq!(errors[1]["expr"], json!("y + 1"));
    }

    #[test]
    fn test_record_nested_error_path() {
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.missing")],
                "x",
                "$.result",
            )],
            OnFailureBehavior::Record {
                path: "$.diagnostics.errors".to_string(),
                limit: None,
            },
        );
        let mut output = json!({});
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["diagnostics"]["errors"]
            .as_array()
            .expect("should be an array");
        assert_eq!(errors.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Operations — arithmetic functions (positive cases)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ops_pow() {
        // pow(2, 10) = 1024
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("b", "$.b"), ("e", "$.e")],
                "pow(b, e)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "b": 2.0, "e": 10.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 1024.0);
    }

    #[test]
    fn test_ops_log2() {
        // log2(8) = 3
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "log2(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 8.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        let result = output["result"].as_f64().unwrap();
        assert!((result - 3.0).abs() < 1e-10, "expected 3.0, got {result}");
    }

    #[test]
    fn test_ops_log10() {
        // log10(100) = 2
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "log10(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 100.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        let result = output["result"].as_f64().unwrap();
        assert!((result - 2.0).abs() < 1e-10, "expected 2.0, got {result}");
    }

    #[test]
    fn test_ops_ln() {
        // ln(1) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "ln(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 1.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_ops_abs() {
        // abs(-7.5) = 7.5
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "abs(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": -7.5 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 7.5);
    }

    #[test]
    fn test_ops_floor() {
        // floor(3.7) = 3
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "floor(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 3.7 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 3.0);
    }

    #[test]
    fn test_ops_ceil() {
        // ceil(3.2) = 4
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "ceil(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 3.2 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 4.0);
    }

    #[test]
    fn test_ops_round() {
        // round(2.5) = 3 (half-away-from-zero)
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "round(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 2.5 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 3.0);
    }

    #[test]
    fn test_ops_min() {
        // min(3, 7) = 3
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("a", "$.a"), ("b", "$.b")],
                "min(a, b)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "a": 3.0, "b": 7.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 3.0);
    }

    #[test]
    fn test_ops_max() {
        // max(3, 7) = 7
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("a", "$.a"), ("b", "$.b")],
                "max(a, b)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "a": 3.0, "b": 7.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 7.0);
    }

    #[test]
    fn test_ops_exp() {
        // exp(0) = 1
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "exp(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 0.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 1.0);
    }

    #[test]
    fn test_ops_sin() {
        // sin(0) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "sin(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 0.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_ops_cos() {
        // cos(0) = 1
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "cos(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 0.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 1.0);
    }

    #[test]
    fn test_ops_asin() {
        // asin(0) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "asin(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 0.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_ops_tan() {
        // tan(0) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "tan(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 0.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_ops_acos() {
        // acos(1) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "acos(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 1.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_ops_atan() {
        // atan(0) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "atan(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 0.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_ops_atan2() {
        // atan2(0, 1) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("y", "$.y"), ("x", "$.x")],
                "atan2(y, x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "y": 0.0, "x": 1.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["result"].as_f64().unwrap(), 0.0);
    }

    #[test]
    fn test_ops_log() {
        // log(base, value): first argument is the base, second is the value.
        // log(10, 100) = log_10(100) = 2.
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("base", "$.base"), ("val", "$.val")],
                "log(base, val)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "base": 10.0, "val": 100.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        let result = output["result"].as_f64().unwrap();
        assert!((result - 2.0).abs() < 1e-10, "expected 2.0, got {result}");
    }

    // -----------------------------------------------------------------------
    // Operations — nesting (operation wrapping arithmetic / other operations)
    // -----------------------------------------------------------------------

    #[test]
    fn test_nesting_sqrt_of_sum() {
        // sqrt(x + y) where x=9, y=16 → sqrt(25) = 5
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x"), ("y", "$.y")],
                "sqrt(x + y)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 9.0, "y": 16.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        let result = output["result"].as_f64().unwrap();
        assert!((result - 5.0).abs() < 1e-10, "expected 5.0, got {result}");
    }

    #[test]
    fn test_nesting_min_of_two_functions() {
        // min(log2(x), sin(x)) where x=1 → min(log2(1), sin(1)) = min(0, ~0.841) = 0
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "min(log2(x), sin(x))",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 1.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        let result = output["result"].as_f64().unwrap();
        assert!(
            (result - 0.0).abs() < 1e-10,
            "expected 0.0 (log2(1)), got {result}"
        );
    }

    #[test]
    fn test_nesting_pow_of_abs() {
        // pow(abs(x), e) where x=-3, e=2 → pow(3, 2) = 9
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x"), ("e", "$.e")],
                "pow(abs(x), e)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": -3.0, "e": 2.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        let result = output["result"].as_f64().unwrap();
        assert!((result - 9.0).abs() < 1e-10, "expected 9.0, got {result}");
    }

    // -----------------------------------------------------------------------
    // Operations — domain errors (negative cases)
    // -----------------------------------------------------------------------

    #[test]
    fn test_ops_sqrt_of_negative_fails() {
        // sqrt(-4) → NaN → non-finite result → error
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "sqrt(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": -4.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_ops_ln_of_negative_fails() {
        // ln(-1) → NaN → non-finite result → error
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "ln(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": -1.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_ops_log2_of_zero_fails() {
        // log2(0) → -Infinity → non-finite result → error
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "log2(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 0.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_ops_asin_out_of_domain_fails() {
        // asin(2) → NaN (domain is [-1, 1]) → non-finite result → error
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "asin(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 2.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_ops_acos_out_of_domain_fails() {
        // acos(2) → NaN (domain is [-1, 1]) → non-finite result → error
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x")],
                "acos(x)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 2.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_ops_division_by_zero_fails() {
        // 1.0 / 0.0 → f64::INFINITY → non-finite → error
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x"), ("y", "$.y")],
                "x / y",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 1.0, "y": 0.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    // -----------------------------------------------------------------------
    // Error propagation — expression errors
    // -----------------------------------------------------------------------

    #[test]
    fn test_unknown_function_fails() {
        // An unrecognized function name should return Err.
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "foo(x)", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 1.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_wrong_argument_count_fails() {
        // sqrt expects one argument; passing two should return Err.
        let plugin = build_plugin(
            vec![ExpressionConfig::new(
                &[("x", "$.x"), ("y", "$.y")],
                "sqrt(x, y)",
                "$.result",
            )],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": 4.0, "y": 9.0 });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_non_numeric_input_fails() {
        // A string value at the input JSONPath is not coercible to f64 → Err.
        let plugin = build_plugin(
            vec![ExpressionConfig::new(&[("x", "$.x")], "x * 2", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "x": "not_a_number" });
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    // -----------------------------------------------------------------------
    // Build-time failures
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_fails_for_invalid_input_jsonpath() {
        // An input path that is not a valid JSONPath expression should fail at build time.
        let result = EvalOutputPlugin::new(EvalOutputPluginConfig {
            expressions: vec![ExpressionConfig::new(
                &[("x", "not_a_valid_jsonpath!!!")],
                "x",
                "$.result",
            )],
            on_failure: OnFailureBehavior::Interrupt,
            on_nan: Default::default(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_build_fails_for_empty_output_path() {
        // An output path of bare "$" (no key after the root) should fail at build time.
        let result = EvalOutputPlugin::new(EvalOutputPluginConfig {
            expressions: vec![ExpressionConfig::new(&[], "1 + 1", "$")],
            on_failure: OnFailureBehavior::Interrupt,
            on_nan: Default::default(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_build_fails_for_invalid_record_path() {
        // A Record on_failure path of bare "$" should fail at build time.
        let result = EvalOutputPlugin::new(EvalOutputPluginConfig {
            expressions: vec![],
            on_failure: OnFailureBehavior::Record {
                path: "$".to_string(),
                limit: None,
            },
            on_nan: Default::default(),
        });
        assert!(result.is_err());
    }

    // a more complicated test case based on calculating a MEP Metric score
    // from a BAMBAM run. leverages the sequential ordering of expressions to
    // build up the terms of multiple MEP calculations.
    #[test]
    fn test_example_mep() {
        // MEP =
        let plugin = EvalOutputPlugin::new(EvalOutputPluginConfig {
            expressions: vec![
                // assign the t=0 opportunity term
                ExpressionConfig::new(&[], "0.0", "$.mep.opp_0"),
                // assign the opportunity normalization term
                ExpressionConfig::new(
                    &[
                        ("N_f", "$.info.opportunity_totals.food"),
                        ("N_e", "$.info.opportunity_totals.entertainment"),
                        ("N_h", "$.info.opportunity_totals.healthcare"),
                        ("N_j", "$.info.opportunity_totals.jobs"),
                        ("N_r", "$.info.opportunity_totals.retail"),
                        ("N_s", "$.info.opportunity_totals.services"),
                        ("F_f", "$.info.activity_frequencies.food"),
                        ("F_e", "$.info.activity_frequencies.entertainment"),
                        ("F_h", "$.info.activity_frequencies.healthcare"),
                        ("F_j", "$.info.activity_frequencies.jobs"),
                        ("F_r", "$.info.activity_frequencies.retail"),
                        ("F_s", "$.info.activity_frequencies.services"),
                    ],
                    "(N_f / N_j) * (F_j / (F_f + F_e + F_h + F_j + F_r + F_s))",
                    "$.mep.norm",
                ),
                // assign the t=10 opportunity term
                ExpressionConfig::new(
                    &[
                        ("norm", "$.mep.norm"),
                        ("o_10", "$.aggregate_opportunities.opportunities['10'].jobs"),
                    ],
                    "o_10 * norm",
                    "$.mep.opp_10",
                ),
                // assign the t=20 opportunity term
                ExpressionConfig::new(
                    &[
                        ("norm", "$.mep.norm"),
                        ("o_20", "$.aggregate_opportunities.opportunities['20'].jobs"),
                    ],
                    "o_20 * norm",
                    "$.mep.opp_20",
                ),
                // assign the modal weighting factor for drive, 10m
                ExpressionConfig::new(
                    &[
                        ("e_k", "$.info.intensities.drive.energy"),
                        ("c_k", "$.info.intensities.drive.cost"),
                    ],
                    "(-0.5 * e_k) + (-0.08 * 10.0) + (-0.5 * c_k)",
                    "$.mep.m_i_drive_10",
                ),
                // assign the modal weighting factor for drive, 10m
                ExpressionConfig::new(
                    &[
                        ("e_k", "$.info.intensities.drive.energy"),
                        ("c_k", "$.info.intensities.drive.cost"),
                    ],
                    "(-0.5 * e_k) + (-0.08 * 20.0) + (-0.5 * c_k)",
                    "$.mep.m_i_drive_20",
                ),
                ExpressionConfig::new(
                    &[
                        ("o_prev", "$.mep.opp_0"),
                        ("o", "$.mep.opp_10"),
                        ("mikt", "$.mep.m_i_drive_10"),
                    ],
                    "(o - o_prev) * exp(mikt)",
                    "$.mep.mep_10",
                ),
                ExpressionConfig::new(
                    &[
                        ("o_prev", "$.mep.opp_10"),
                        ("o", "$.mep.opp_20"),
                        ("mikt", "$.mep.m_i_drive_20"),
                    ],
                    "(o - o_prev) * exp(mikt)",
                    "$.mep.mep_20",
                ),
            ],
            on_failure: OnFailureBehavior::Record {
                path: "$.diagnostics.errors".to_string(),
                limit: None,
            },
            on_nan: Default::default(),
        })
        .unwrap();
        let mut output = json!({
            "info": {
                "activity_frequencies": {
                    "entertainment": 8.4,
                    "food": 6.7,
                    "healthcare": 1.5,
                    "jobs": 17.0,
                    "retail": 20.0,
                    "services": 3.1
                },
                "opportunity_totals": {
                    "entertainment": 100_000,
                    "food": 10_000,
                    "healthcare": 100_000,
                    "jobs": 100_000_000,
                    "retail": 10_000,
                    "services": 1_000_000
                },
                "normalizing_activity": "food",
                "intensities": {
                    "walk": { "energy": 0, "cost": 0 },
                    "bike": { "energy": 0, "cost": 0 },
                    "drive": { "energy": 0.48, "cost": 0.9 },
                    "transit": { "energy": 0.855, "cost": 0.65 }
                }
            },
            "aggregate_opportunities": {
                "opportunities": {
                    "10": {
                        "jobs": 10_000.0
                    },
                    "20": {
                        "jobs": 20_000.0
                    }
                }
            }
        });
        plugin.process(&mut output, &dummy_result()).unwrap();

        assert!(output.get("diagnostics").is_none());
    }
}
