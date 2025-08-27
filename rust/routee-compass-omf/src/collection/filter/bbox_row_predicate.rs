use super::Bbox;
use arrow::{
    array::{Array, BooleanArray, Float32Array, StructArray},
    error::ArrowError,
};
use parquet::arrow::arrow_reader::ArrowPredicate;

pub struct BboxRowPredicate {
    bbox: Bbox,
    projection_mask: parquet::arrow::ProjectionMask,
}

impl BboxRowPredicate {
    pub fn new(bbox: Bbox, projection_mask: parquet::arrow::ProjectionMask) -> Self {
        Self {
            bbox,
            projection_mask,
        }
    }
}

impl ArrowPredicate for BboxRowPredicate {
    fn projection(&self) -> &parquet::arrow::ProjectionMask {
        &self.projection_mask
    }

    fn evaluate(
        &mut self,
        batch: arrow::array::RecordBatch,
    ) -> Result<arrow::array::BooleanArray, arrow::error::ArrowError> {
        let struct_array = batch
            .column_by_name("bbox")
            .ok_or(ArrowError::ParquetError(String::from(
                "`bbox` column not found",
            )))?
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or(ArrowError::ParquetError(String::from(
                "Cannot cast column `bbox` to StructArray type",
            )))?;

        let x_min_col = struct_array
            .column_by_name("xmin")
            .ok_or(ArrowError::ParquetError(String::from(
                "`bbox.xmin` column not found",
            )))?
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or(ArrowError::ParquetError(String::from(
                "Cannot cast column `bbox.xmin` to Float32Array type",
            )))?;

        let y_min_col = struct_array
            .column_by_name("ymin")
            .ok_or(ArrowError::ParquetError(String::from(
                "`bbox.ymin` column not found",
            )))?
            .as_any()
            .downcast_ref::<Float32Array>()
            .ok_or(ArrowError::ParquetError(String::from(
                "Cannot cast column `bbox.ymin` to Float32Array type",
            )))?;

        let boolean_values: Vec<bool> = (0..struct_array.len())
            .map(|i| {
                self.bbox.xmin < x_min_col.value(i)
                    && x_min_col.value(i) < self.bbox.xmax
                    && self.bbox.ymin < y_min_col.value(i)
                    && y_min_col.value(i) < self.bbox.ymax
            })
            .collect();
        Ok(BooleanArray::from(boolean_values))
    }
}
