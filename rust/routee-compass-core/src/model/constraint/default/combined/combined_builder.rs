use crate::config::ops::strip_type_from_config;
use crate::config::{CompassConfigurationError, CompassConfigurationField, ConfigJsonExtensions};
use crate::model::constraint::{
    ConstraintModelBuilder, ConstraintModelError, ConstraintModelService,
};
use itertools::Itertools;
use std::{collections::HashMap, rc::Rc, sync::Arc};

use super::combined_service::CombinedFrontierService;

pub struct CombinedConstraintModelBuilder {
    pub builders: HashMap<String, Rc<dyn ConstraintModelBuilder>>,
}

impl CombinedConstraintModelBuilder {
    pub fn new(builders: HashMap<String, Rc<dyn ConstraintModelBuilder>>) -> Self {
        Self { builders }
    }

    fn build_service(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, CompassConfigurationError> {
        let (conf_stripped, fm_type) = strip_type_from_config(config)?;
        self.builders
            .get(&fm_type)
            .ok_or_else(|| {
                CompassConfigurationError::UnknownModelNameForComponent(
                    fm_type.clone(),
                    String::from("constraint"),
                    self.builders.keys().join(", "),
                )
            })
            .and_then(|b| {
                b.build(&conf_stripped)
                    .map_err(CompassConfigurationError::ConstraintModelError)
            })
    }
}

impl ConstraintModelBuilder for CombinedConstraintModelBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, ConstraintModelError> {
        let constraint_key = CompassConfigurationField::Constraint;
        let params = parameters
            .get_config_array(&"models", &constraint_key)
            .map_err(|e| ConstraintModelError::BuildError(e.to_string()))?;

        let inner_services = params
            .iter()
            .map(|p| self.build_service(p))
            .collect::<Result<Vec<Arc<dyn ConstraintModelService>>, CompassConfigurationError>>()
            .map_err(|e| ConstraintModelError::BuildError(e.to_string()))?;

        let service = CombinedFrontierService { inner_services };

        Ok(Arc::new(service))
    }
}
