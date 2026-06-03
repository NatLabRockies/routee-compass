use super::TemplateConfig;

use routee_compass_core::model::constraint::ConstraintModelError;

pub struct TemplateEngine {
    config: TemplateConfig,
}

impl TryFrom<TemplateConfig> for TemplateEngine {
    type Error = ConstraintModelError;

    fn try_from(config: TemplateConfig) -> Result<Self, Self::Error> {
        Ok(Self { config })
    }
}
