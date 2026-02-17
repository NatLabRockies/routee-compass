use super::road_class_builder_config::RoadClassBuilderConfig;
use super::road_class_service::RoadClassFrontierService;
use crate::{
    model::constraint::{ConstraintModelBuilder, ConstraintModelError, ConstraintModelService},
    util::fs::{read_decoders, read_utils},
};
use kdam::Bar;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub struct RoadClassBuilder {}

impl ConstraintModelBuilder for RoadClassBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, ConstraintModelError> {
        let config: RoadClassBuilderConfig =
            serde_json::from_value(parameters.clone()).map_err(|e| {
                ConstraintModelError::BuildError(format!(
                    "failed to read road class configuration: {e}"
                ))
            })?;

        let road_class_file = PathBuf::from(&config.road_class_input_file);

        let road_class_lookup: Box<[String]> = read_utils::read_raw_file(
            &road_class_file,
            read_decoders::string,
            Some(Bar::builder().desc("road class")),
            None,
        )
        .map_err(|e| {
            ConstraintModelError::BuildError(format!(
                "failed to load file at {file_path:?}: {e}",
                file_path = road_class_file
            ))
        })?;

        let mut mapping = HashMap::new();
        let mut encoded = Vec::with_capacity(road_class_lookup.len());
        let mut next_id = 0usize;

        for class in road_class_lookup.iter() {
            let id = match mapping.get(class) {
                Some(id) => *id,
                None => {
                    let id_usize = next_id;
                    if id_usize > u8::MAX as usize {
                        return Err(ConstraintModelError::BuildError(
                            "too many unique road classes, max is 256".to_string(),
                        ));
                    }
                    next_id += 1;
                    let id = id_usize as u8;
                    mapping.insert(class.clone(), id);
                    id
                }
            };
            encoded.push(id);
        }

        let m: Arc<dyn ConstraintModelService> = Arc::new(RoadClassFrontierService {
            road_class_by_edge: Arc::new(encoded.into_boxed_slice()),
            road_class_mapping: Arc::new(mapping),
        });
        Ok(m)
    }
}
