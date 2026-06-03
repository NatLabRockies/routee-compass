use routee_compass::{
    app::{compass::CompassAppError, search::SearchAppResult},
    plugin::output::{OutputPlugin, OutputPluginBuilder, OutputPluginError},
};
use routee_compass_core::algorithm::search::SearchInstance;

use super::TemplateConfig;

pub struct TemplateOutputPlugin {
    config: TemplateConfig,
}

impl OutputPlugin for TemplateOutputPlugin {
    fn name(&self) -> &str {
        "TemplateOutputPlugin"
    }

    fn process(
        &self,
        output: &mut serde_json::Value,
        result: &Result<(SearchAppResult, SearchInstance), CompassAppError>,
    ) -> Result<(), OutputPluginError> {
        todo!()
    }
}

impl TemplateOutputPlugin {
    pub fn new(config: TemplateConfig) -> Self {
        Self { config }
    }
}
