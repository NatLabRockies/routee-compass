use crate::app::{compass::CompassAppError, search::SearchAppResult};
use crate::plugin::output::default::summary::SummaryConfig;
use crate::plugin::output::output_plugin::OutputPlugin;
use crate::plugin::output::OutputPluginError;
use routee_compass_core::algorithm::search::SearchInstance;
use routee_compass_core::util::duration_extension::DurationExtension;
use serde_json::json;

/// provides metrics for the performance of the search algorithm.
pub struct SummaryOutputPlugin {
    pub estimate_memory_consumption: bool,
}

impl OutputPlugin for SummaryOutputPlugin {
    /// append "Cost" value to the output JSON
    fn process(
        &self,
        output: &mut serde_json::Value,
        search_result: &Result<(SearchAppResult, SearchInstance), CompassAppError>,
    ) -> Result<(), OutputPluginError> {
        match search_result {
            Err(_e) => Ok(()),
            Ok((result, _)) => {
                let route_edges = result.routes.iter().map(|r| r.len()).sum::<usize>();
                let tree_edges = result.graphs.iter().map(|t| t.len()).sum::<usize>();
                let terminated = result
                    .terminated
                    .clone()
                    .unwrap_or_else(|| "false".to_string());

                output["search_executed_time"] = json![result.search_executed_time.clone()];
                output["search_runtime"] = json![result.search_runtime.hhmmss()];
                output["route_edges"] = json![route_edges];
                output["tree_size_count"] = json![tree_edges];
                output["iterations"] = json![result.iterations];
                output["terminated"] = json![terminated];

                if self.estimate_memory_consumption {
                    let memory_bytes = allocative::size_of_unique(result) as f64;
                    let memory_mib = memory_bytes / 1_048_576.0;
                    output["search_result_size_mib"] = json![memory_mib];
                }

                Ok(())
            }
        }
    }
}

impl SummaryOutputPlugin {
    pub fn new(conf: SummaryConfig) -> SummaryOutputPlugin {
        SummaryOutputPlugin {
            estimate_memory_consumption: conf.estimate_memory_consumption.unwrap_or_default(),
        }
    }
}
