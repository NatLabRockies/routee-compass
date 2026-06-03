use std::sync::Arc;

use routee_compass::{
    app::{
        compass::CompassAppError,
        search::{SearchApp, SearchAppResult},
    },
    plugin::{
        input::{InputPlugin, InputPluginError},
        output::{OutputPlugin, OutputPluginBuilder, OutputPluginError},
    },
};
use routee_compass_core::algorithm::search::SearchInstance;

use super::TemplateConfig;

pub struct TemplateInputPlugin {
    config: TemplateConfig,
}

impl InputPlugin for TemplateInputPlugin {
    fn name(&self) -> &str {
        "TemplateInputPlugin"
    }

    fn process(
        &self,
        input: &mut serde_json::Value,
        search_app: Arc<SearchApp>,
    ) -> Result<(), InputPluginError> {
        todo!()
    }
}

impl TemplateInputPlugin {
    pub fn new(config: TemplateConfig) -> Self {
        Self { config }
    }
}
