use std::sync::Arc;

use routee_compass::{
    app::{
        compass::{CompassAppError, CompassComponentError},
        search::SearchAppResult,
    },
    plugin::{
        input::{InputPlugin, InputPluginBuilder, InputPluginError},
        output::{OutputPlugin, OutputPluginBuilder, OutputPluginError},
        PluginError,
    },
};
use routee_compass_core::{algorithm::search::SearchInstance, config::CompassConfigurationError};

use super::{TemplateConfig, TemplateInputPlugin};

pub struct TemplatePluginBuilder {}

impl InputPluginBuilder for TemplatePluginBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn InputPlugin>, CompassConfigurationError> {
        let config: TemplateConfig = serde_json::from_value(parameters.clone()).map_err(|e| {
            let msg = format!("failure reading config for Template builder: {e}");
            CompassConfigurationError::UserConfigurationError(msg)
        })?;
        let plugin = TemplateInputPlugin::new(config);
        Ok(Arc::new(plugin))
    }
}
