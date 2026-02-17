use super::network_cost_rate::NetworkCostRate;
use crate::model::cost::network::NetworkVertexCostRow;
use crate::model::cost::CostModelError;
use crate::{
    model::cost::network::network_edge_cost_row::NetworkEdgeCostRow, util::fs::read_utils,
};
use kdam::Bar;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
#[serde(tag = "type")]
pub enum NetworkCostRateBuilder {
    #[serde(rename = "edge_id")]
    EdgeLookupBuilder { cost_input_file: String },
    #[serde(rename = "vertex_id")]
    VertexLookupBuilder { cost_input_file: String },
    #[serde(rename = "combined")]
    Combined(Vec<NetworkCostRateBuilder>),
}

impl NetworkCostRateBuilder {
    pub fn build(&self) -> Result<NetworkCostRate, CostModelError> {
        use NetworkCostRate as NCM;
        use NetworkCostRateBuilder as Builder;
        match self {
            Builder::EdgeLookupBuilder { cost_input_file } => {
                let lookup = read_utils::from_csv::<NetworkEdgeCostRow>(
                    cost_input_file,
                    true,
                    Some(Bar::builder().desc("network edge cost lookup")),
                    None,
                )
                .map_err(|source| {
                    CostModelError::BuildError(format!(
                        "failure reading file {cost_input_file}: {source}"
                    ))
                })?
                .iter()
                .map(|row| (row.edge_id, row.cost))
                .collect::<HashMap<_, _>>();
                Ok(NCM::EdgeLookup { lookup })
            }
            Builder::VertexLookupBuilder { cost_input_file } => {
                let lookup = read_utils::from_csv::<NetworkVertexCostRow>(
                    cost_input_file,
                    true,
                    Some(Bar::builder().desc("network edge->edge cost lookup")),
                    None,
                )
                .map_err(|source| {
                    CostModelError::BuildError(format!(
                        "failure reading file {cost_input_file}: {source}"
                    ))
                })?
                .iter()
                .map(|row| (row.vertex_id, row.cost))
                .collect::<HashMap<_, _>>();

                Ok(NCM::VertexLookup { lookup })
            }
            Builder::Combined(builders) => {
                let mappings = builders
                    .iter()
                    .map(|b| b.build())
                    .collect::<Result<Vec<_>, CostModelError>>()?;
                Ok(NCM::Combined(mappings))
            }
        }
    }
}
