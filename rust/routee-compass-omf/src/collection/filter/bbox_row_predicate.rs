use super::Bbox;
use arrow::{
    array::{Array, BooleanArray, Float32Array, StructArray},
    error::ArrowError,
};
use parquet::arrow::arrow_reader::ArrowPredicate;

/// tests if a row is contained within a bounding box.
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

    /// tests the bounding box of each row in the record batch, filtering entries
    /// that are not fully-contained.
    fn evaluate(
        &mut self,
        batch: arrow::array::RecordBatch,
    ) -> Result<arrow::array::BooleanArray, arrow::error::ArrowError> {
        let bbox_struct = batch
            .column_by_name("bbox")
            .ok_or(ArrowError::ParquetError(String::from(
                "`bbox` column not found",
            )))?
            .as_any()
            .downcast_ref::<StructArray>()
            .ok_or(ArrowError::ParquetError(String::from(
                "Cannot cast column `bbox` to StructArray type",
            )))?;

        let xmins = get_column::<Float32Array>("xmin", bbox_struct)?;
        let ymins = get_column::<Float32Array>("ymin", bbox_struct)?;
        let xmaxs = get_column::<Float32Array>("xmax", bbox_struct)?;
        let ymaxs = get_column::<Float32Array>("ymax", bbox_struct)?;

        let boolean_values: Vec<bool> = (0..bbox_struct.len())
            .map(|i| within_box(i, xmins, ymins, xmaxs, ymaxs, &self.bbox))
            .collect();
        Ok(BooleanArray::from(boolean_values))
    }
}

/// helper function to get a column by name from a struct array and return it as
/// the expected type.
fn get_column<'a, 'b, T>(col: &'a str, struct_array: &'b StructArray) -> Result<&'b T, ArrowError>
where
    T: 'static,
{
    struct_array
        .column_by_name(col)
        .ok_or(ArrowError::ParquetError(format!(
            "'bbox.{col}' column not found"
        )))?
        .as_any()
        .downcast_ref::<T>()
        .ok_or(ArrowError::ParquetError(format!(
            "Cannot cast column 'bbox.{col}' to expected type"
        )))
}

/// helper function to test whether a given row's values are contained within the bounding box.
fn within_box(
    index: usize,
    xmins: &Float32Array,
    ymins: &Float32Array,
    xmaxs: &Float32Array,
    ymaxs: &Float32Array,
    bbox: &Bbox,
) -> bool {
    bbox.xmin < xmins.value(index)
        && xmaxs.value(index) < bbox.xmax
        && bbox.ymin < ymins.value(index)
        && ymaxs.value(index) < bbox.ymax
}
