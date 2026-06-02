use super::TemplateConfig;

use crate::model::traversal::TraversalModelError;

pub struct TemplateEngine {
    #[allow(unused)]
    config: TemplateConfig,
}

impl TryFrom<TemplateConfig> for TemplateEngine {
    type Error = TraversalModelError;

    fn try_from(config: TemplateConfig) -> Result<Self, Self::Error> {
        Ok(Self { config })
    }
}
