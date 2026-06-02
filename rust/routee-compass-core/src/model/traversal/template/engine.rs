use super::TemplateConfig;

use crate::model::traversal::TraversalModelError;

pub struct TemplateEngine {}

impl TryFrom<TemplateConfig> for TemplateEngine {
    type Error = TraversalModelError;

    fn try_from(_config: TemplateConfig) -> Result<Self, Self::Error> {
        todo!()
    }
}
