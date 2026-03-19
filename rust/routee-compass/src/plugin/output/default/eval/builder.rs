use std::sync::Arc;

use crate::{
    app::compass::CompassComponentError,
    plugin::{
        PluginError,
        output::{OutputPluginBuilder, default::eval::{EvalOutputPlugin, config::EvalOutputPluginConfig}},
    },
};

pub struct EvalOutputPluginBuilder {}

impl OutputPluginBuilder for EvalOutputPluginBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<std::sync::Arc<dyn crate::plugin::output::OutputPlugin>, CompassComponentError> {
        let conf: EvalOutputPluginConfig = serde_json::from_value(parameters.clone())
            .map_err(|e| CompassComponentError::PluginError(PluginError::BuildFailed(format!("while building eval plugin: {e}"))))?;
        Ok(Arc::new(EvalOutputPlugin::new(conf)?))
    }
}