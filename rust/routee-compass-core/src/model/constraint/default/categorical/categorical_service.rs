use super::categorical_model::CategoricalConstraintModel;
use crate::model::{
    constraint::{ConstraintModel, ConstraintModelError, ConstraintModelService},
    state::StateModel,
};
use serde_json;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Long-lived service that constructs a `CategoricalModel` for each query.
///
/// Built once by the `CategoricalModelBuilder` at application startup, the service
/// persists for the lifetime of the Compass process, so the loaded category data
/// can be shared across many queries.
#[derive(Clone)]
pub struct CategoricalModelService {
    pub key: String,                                // the category name
    pub category_by_edge: Arc<Box<[u8]>>,           // categorizes each edge with the encoding
    pub category_mapping: Arc<HashMap<String, u8>>, // maps the categories (String) to the encoding (u8)
}

impl ConstraintModelService for CategoricalModelService {
    /// Builds the `CategoricalModel`
    fn build(
        &self,
        query: &serde_json::Value,
        _state_model: Arc<StateModel>,
    ) -> Result<Arc<dyn ConstraintModel>, ConstraintModelError> {
        let query_categories = match query
            .get(&self.key)
            .map(|val| read_categories_from_query(val, &self.key))
        {
            Some(Err(e)) => Err(e),
            Some(Ok(categories)) => {
                let mapped: Result<HashSet<u8>, ConstraintModelError> = categories
                    .iter()
                    .map(|c| {
                        self.category_mapping.get(c).copied().ok_or_else(|| {
                            ConstraintModelError::BuildError(format!(
                                "{} category '{}' not found in mapping",
                                &self.key,
                                c,
                            ))
                        })
                    })
                    .collect();
                mapped.map(Some)
            }
            None => Ok(None),
        }?;

        let service: Arc<CategoricalModelService> = Arc::new(self.clone());
        let model = CategoricalConstraintModel {
            service,
            query_categories,
        };
        Ok(Arc::new(model))
    }
}

/// Grabs all categories from the query.
fn read_categories_from_query(
    value: &Value,
    category_key: &String,
) -> Result<HashSet<String>, ConstraintModelError> {
    let arr = value.as_array().ok_or_else(|| {
        ConstraintModelError::BuildError(format!(
            "query's {category_key} value must be an array, found '{value}'"
        ))
    })?;
    // if the value is a string (or number or bool), store it as a valid road class
    let arr_str = arr
        .iter()
        .enumerate()
        .map(|(idx, c)| match c {
            Value::Bool(b) => Ok(b.to_string()),
            Value::Number(number) => Ok(number.to_string()),
            Value::String(string) => Ok(string.clone()),
            _ => Err(ConstraintModelError::BuildError(format!(
                "query's '{category_key}[{idx}]' value must be a string, found '{c}'"
            ))),
        })
        .collect::<Result<HashSet<_>, _>>()?;

    Ok(arr_str)
}
