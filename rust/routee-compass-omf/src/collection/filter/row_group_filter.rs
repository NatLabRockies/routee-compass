use parquet::file::metadata::RowGroupMetaData;

pub trait RowGroupFilter {
    fn prune_row_group(&self, row_groups: &[RowGroupMetaData]) -> Vec<usize>;
}
