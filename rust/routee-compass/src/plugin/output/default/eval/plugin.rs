use crate::plugin::output::{OutputPlugin, default::eval::ops::{self, CompiledExpression, CompiledOnFailure}};

use super::config::{EvalOutputPluginConfig, OnFailureBehavior};

pub struct EvalOutputPlugin {
    expressions: Vec<CompiledExpression>,
    on_failure: CompiledOnFailure,
}

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
            OnFailureBehavior::Record { path } => {
                let segments = ops::parse_path_segments(&path).map_err(|e| {
                    crate::plugin::PluginError::BuildFailed(format!(
                        "invalid on_failure record path '{path}': {e}"
                    ))
                })?;
                CompiledOnFailure::Record { segments }
            }
        };

        Ok(Self {
            expressions,
            on_failure,
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
            match ops::eval_and_write(expr, output) {
                Ok(()) => {}
                Err(e) => match &self.on_failure {
                    CompiledOnFailure::Interrupt => return Err(e),
                    CompiledOnFailure::Ignore => {}
                    CompiledOnFailure::Record { segments } => {
                        ops::record_error(output, segments, &expr.expr, &e.to_string())?;
                    }
                },
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use crate::{
        app::compass::CompassAppError,
        plugin::output::{
            OutputPlugin,
            default::eval::config::{EvalOutputPluginConfig, ExpressionConfig, OnFailureBehavior},
        },
    };

    use super::EvalOutputPlugin;

    // A dummy search result — the plugin ignores it, so an Err is fine.
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

    fn expr(inputs: &[(&str, &str)], expression: &str, output: &str) -> ExpressionConfig {
        ExpressionConfig {
            inputs: inputs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<HashMap<_, _>>(),
            expr: expression.to_string(),
            output: output.to_string(),
        }
    }

    fn build(expressions: Vec<ExpressionConfig>, on_failure: OnFailureBehavior) -> EvalOutputPlugin {
        EvalOutputPlugin::new(EvalOutputPluginConfig { expressions, on_failure })
            .expect("plugin should build")
    }

    // -----------------------------------------------------------------------
    // Success cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_success_simple_arithmetic() {
        let plugin = build(
            vec![expr(
                &[("delay", "$.metric.time_delay"), ("rate", "$.cost.per_hour")],
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
        let plugin = build(
            vec![
                expr(&[("x", "$.x")], "x * 2", "$.doubled"),
                expr(&[("d", "$.doubled")], "d + 1", "$.result"),
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
        let plugin = build(
            vec![expr(&[("v", "$.v")], "v ^ 2", "$.stats.squared.value")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({ "v": 5.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();
        assert_eq!(output["stats"]["squared"]["value"].as_f64().unwrap(), 25.0);
    }

    // -----------------------------------------------------------------------
    // Interrupt behavior
    // -----------------------------------------------------------------------

    #[test]
    fn test_interrupt_returns_err_on_missing_input() {
        let plugin = build(
            vec![expr(&[("x", "$.missing")], "x * 2", "$.result")],
            OnFailureBehavior::Interrupt,
        );
        let mut output = json!({});
        assert!(plugin.process(&mut output, &dummy_result()).is_err());
    }

    #[test]
    fn test_interrupt_stops_after_first_failure() {
        // Two expressions: first fails, second would succeed and write "$.sentinel".
        let plugin = build(
            vec![
                expr(&[("x", "$.missing")], "x * 2", "$.result"),
                expr(&[("y", "$.y")], "y + 1", "$.sentinel"),
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
    fn test_ignore_returns_ok_on_failure() {
        let plugin = build(
            vec![expr(&[("x", "$.missing")], "x * 2", "$.result")],
            OnFailureBehavior::Ignore,
        );
        let mut output = json!({});
        assert!(plugin.process(&mut output, &dummy_result()).is_ok());
    }

    #[test]
    fn test_ignore_continues_after_failure() {
        // First expression fails; second must still run.
        let plugin = build(
            vec![
                expr(&[("x", "$.missing")], "x * 2", "$.result"),
                expr(&[("y", "$.y")], "y + 1", "$.sentinel"),
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
        let plugin = build(
            vec![expr(&[("x", "$.missing")], "x * 2", "$.result")],
            OnFailureBehavior::Record { path: "$.eval_errors".to_string() },
        );
        let mut output = json!({});
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["eval_errors"].as_array().expect("should be an array");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["expr"], json!("x * 2"));
        assert!(errors[0]["error"].is_string());
    }

    #[test]
    fn test_record_continues_after_failure() {
        let plugin = build(
            vec![
                expr(&[("x", "$.missing")], "x * 2", "$.result"),
                expr(&[("y", "$.y")], "y + 1", "$.sentinel"),
            ],
            OnFailureBehavior::Record { path: "$.eval_errors".to_string() },
        );
        let mut output = json!({ "y": 5.0 });
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["eval_errors"].as_array().expect("should be an array");
        assert_eq!(errors.len(), 1);
        // The second expression must still have run.
        assert_eq!(output["sentinel"].as_f64().unwrap(), 6.0);
    }

    #[test]
    fn test_record_accumulates_multiple_failures() {
        let plugin = build(
            vec![
                expr(&[("x", "$.missing_x")], "x * 2", "$.r1"),
                expr(&[("y", "$.missing_y")], "y + 1", "$.r2"),
            ],
            OnFailureBehavior::Record { path: "$.eval_errors".to_string() },
        );
        let mut output = json!({});
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["eval_errors"].as_array().expect("should be an array");
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0]["expr"], json!("x * 2"));
        assert_eq!(errors[1]["expr"], json!("y + 1"));
    }

    #[test]
    fn test_record_nested_error_path() {
        let plugin = build(
            vec![expr(&[("x", "$.missing")], "x", "$.result")],
            OnFailureBehavior::Record { path: "$.diagnostics.errors".to_string() },
        );
        let mut output = json!({});
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["diagnostics"]["errors"]
            .as_array()
            .expect("should be an array");
        assert_eq!(errors.len(), 1);
    }

     #[test]
    fn test_example_mep() {
        let plugin = build(
            vec![expr(&[
                ("x", "$.missing")
                ], 
                "x", 
                "$.mep")
            ],
            OnFailureBehavior::Record { path: "$.diagnostics.errors".to_string() },
        );
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
                        "jobs": 12_345.0
                    }
                }
            }
        });
        plugin.process(&mut output, &dummy_result()).unwrap();

        let errors = output["diagnostics"]["errors"]
            .as_array()
            .expect("should be an array");
        assert_eq!(errors.len(), 1);
    }
}

