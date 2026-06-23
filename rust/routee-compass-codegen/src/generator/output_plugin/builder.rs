use std::sync::Arc;

use routee_compass::{
    app::{
        compass::{CompassAppError, CompassComponentError},
        search::SearchAppResult,
    },
    plugin::{
        output::{OutputPlugin, OutputPluginBuilder, OutputPluginError},
        PluginError,
    },
};
use routee_compass_core::algorithm::search::SearchInstance;

use super::{TemplateConfig, TemplateOutputPlugin};

pub struct TemplatePluginBuilder {}

impl OutputPluginBuilder for TemplatePluginBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn OutputPlugin>, CompassComponentError> {
        let config: TemplateConfig = serde_json::from_value(parameters.clone()).map_err(|e| {
            let msg = format!("failure reading config for Template builder: {e}");
            PluginError::OutputPluginFailed {
                source: OutputPluginError::BuildFailed(msg),
            }
        })?;
        let plugin = TemplateOutputPlugin::new(config);
        Ok(Arc::new(plugin))
    }
}
