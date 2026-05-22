use crate::collection::TaxonomyModel;
use arrow::{
    array::{Array, BooleanArray, ListArray, StringArray, StructArray},
    error::ArrowError,
};
use parquet::arrow::arrow_reader::ArrowPredicate;
use std::sync::Arc;

pub struct TaxonomyRowPredicate {
    category_model: Arc<TaxonomyModel>,
    projection_mask: parquet::arrow::ProjectionMask,
}

impl TaxonomyRowPredicate {
    pub fn new(
        category_model: Arc<TaxonomyModel>,
        projection_mask: parquet::arrow::ProjectionMask,
    ) -> Self {
        Self {
            category_model,
            projection_mask,
        }
    }
}

impl ArrowPredicate for TaxonomyRowPredicate {
    fn projection(&self) -> &parquet::arrow::ProjectionMask {
        &self.projection_mask
    }

    fn evaluate(
        &mut self,
        batch: arrow::array::RecordBatch,
    ) -> Result<arrow::array::BooleanArray, arrow::error::ArrowError> {
        let struct_array = batch
            .column_by_name("categories")
            .ok_or(ArrowError::ParquetError(String::from(
                "`categories` column not found",
            )))?
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or(ArrowError::ParquetError(String::from(
                "Cannot cast column `categories` to StructArray type",
            )))?;

        let primary_col = struct_array
            .column_by_name("primary")
            .ok_or(ArrowError::ParquetError(String::from(
                "`categories.primary` column not found",
            )))?
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(ArrowError::ParquetError(String::from(
                "Cannot cast column `categories.primary` to StringArray type",
            )))?;

        let alternate_col = struct_array
            .column_by_name("alternate")
            .ok_or(ArrowError::ParquetError(String::from(
                "`categories.alternate` column not found",
            )))?
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or(ArrowError::ParquetError(String::from(
                "Cannot cast column `categories.alternate` to StringArray type",
            )))?;

        let relevant_categories = self.category_model.get_unique_categories();

        let boolean_values: Vec<bool> = (0..struct_array.len())
            .map(|i| {
                let mut possible_categories = vec![primary_col.value(i)];

                let alternate_col_value = alternate_col.value(i);
                if let Some(alternate_categories) =
                    alternate_col_value.as_any().downcast_ref::<StringArray>()
                {
                    for category in alternate_categories.into_iter().flatten() {
                        possible_categories.push(category);
                    }
                }

                possible_categories
                    .into_iter()
                    .any(|str| relevant_categories.contains(str))
            })
            .collect();
        Ok(BooleanArray::from(boolean_values))
    }
}
