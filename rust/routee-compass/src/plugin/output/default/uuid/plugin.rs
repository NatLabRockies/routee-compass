use super::output_json_extensions::UUIDJsonExtensions;
use crate::app::compass::CompassAppError;
use crate::app::search::SearchAppResult;
use crate::plugin::output::default::uuid::output_json_extensions::UUIDJsonField;
use crate::plugin::output::{OutputPlugin, OutputPluginError};
use kdam::Bar;
use routee_compass_core::algorithm::search::SearchInstance;
use routee_compass_core::util::fs::read_utils::read_raw_file;
use std::path::Path;

pub struct UUIDOutputPlugin {
    uuids: Box<[String]>,
    o_key: String,
    d_key: String,
}

const NAME: &str = "uuid";

impl UUIDOutputPlugin {
    pub fn from_file<P: AsRef<Path>>(filename: &P) -> Result<UUIDOutputPlugin, OutputPluginError> {
        let uuids = read_raw_file(
            filename,
            |_idx, row| Ok(row),
            Some(Bar::builder().desc("uuids")),
            None,
        )
        .map_err(|e| {
            OutputPluginError::BuildFailed(format!(
                "failure reading UUID file {}: {}",
                filename.as_ref().to_str().unwrap_or_default(),
                e
            ))
        })?;
        eprintln!();

        let o_key = UUIDJsonField::OriginVertexUUID.to_string();
        let d_key = UUIDJsonField::DestinationVertexUUID.to_string();
        Ok(UUIDOutputPlugin {
            uuids,
            o_key,
            d_key,
        })
    }
}

impl OutputPlugin for UUIDOutputPlugin {
    fn process(
        &self,
        output: &mut serde_json::Value,
        search_result: &Result<(SearchAppResult, SearchInstance), CompassAppError>,
    ) -> Result<(), OutputPluginError> {
        match search_result {
            Err(_) => Ok(()),
            Ok(_) => {
                let (origin_vertex_id, destination_vertex_id) = output.get_od_vertex_ids()?;
                let origin_uuid = self.uuids.get(origin_vertex_id.0).cloned().ok_or_else(|| {
                    OutputPluginError::OutputPluginFailed(format!(
                        "UUID lookup table missing vertex index {}",
                        origin_vertex_id.0
                    ))
                })?;
                let destination_uuid = self
                    .uuids
                    .get(destination_vertex_id.0)
                    .cloned()
                    .ok_or_else(|| {
                        OutputPluginError::OutputPluginFailed(format!(
                            "UUID lookup table missing vertex index {}",
                            destination_vertex_id.0
                        ))
                    })?;

                output[&self.o_key] = serde_json::Value::String(origin_uuid);
                output[&self.d_key] = serde_json::Value::String(destination_uuid);
                Ok(())
            }
        }
    }

    fn name(&self) -> &str {
        NAME
    }
}
