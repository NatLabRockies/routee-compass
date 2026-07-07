use super::categorical_builder_config::CategoricalModelBuilderConfig;
use super::categorical_service::CategoricalModelService;
use crate::model::constraint::ConstraintModelBuilder;
use crate::model::constraint::ConstraintModelError;
use crate::model::constraint::ConstraintModelService;
use crate::util::fs::read_decoders;
use crate::util::fs::read_utils;
use kdam::Bar;
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CategoricalModelBuilder {}

impl ConstraintModelBuilder for CategoricalModelBuilder {
    /// Builds the CategoricalConstraintService from the configuration.
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, ConstraintModelError> {
        // TODO: should builder grab all of the categories? or just the one?
        let config: CategoricalModelBuilderConfig = serde_json::from_value(parameters.clone())
            .map_err(|e| {
                ConstraintModelError::BuildError(format!(
                    "failed to read categorical constraint configuration: {e}"
                ))
            })?;

        let key = &config.key;
        let input_file = PathBuf::from(&config.input_file);

        let category_lookup: Box<[String]> = read_utils::read_raw_file(
            &input_file,
            read_decoders::string,
            Some(Bar::builder().desc(format!("{key}"))),
            None,
        )
        .map_err(|e| {
            ConstraintModelError::BuildError(format!(
                "failed to load file at {file_path:?}: {e} for constraint: {key}",
                file_path = input_file
            ))
        })?;

        let mut mapping = HashMap::new();
        let mut encoded = Vec::with_capacity(category_lookup.len());
        let mut next_id = 0usize;

        // since we have a new hashmap, first ID will match to None, so we incrementally build out
        // the hashmap.
        for class in category_lookup.iter() {
            let id = match mapping.get(class) {
                Some(id) => *id,
                None => {
                    let id_usize = next_id;
                    if id_usize > u8::MAX as usize {
                        return Err(ConstraintModelError::BuildError(
                            "too many unique road classes, max is 256".to_string(),
                        ));
                    }
                    next_id += 1; // next new string in the mapping will be mapped to id + 1
                    let id = id_usize as u8;
                    mapping.insert(class.clone(), id);
                    id
                }
            };
            encoded.push(id); // add the id for this edge to the encoded road class vector.
        }

        // build the service
        let m: Arc<dyn ConstraintModelService> = Arc::new(CategoricalModelService {
            key: key.to_string(),
            category_by_edge: Arc::new(encoded.into_boxed_slice()),
            category_mapping: Arc::new(mapping),
        });
        Ok(m)
    }
}
