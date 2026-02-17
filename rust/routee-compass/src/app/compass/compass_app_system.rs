use super::response::{
    response_output_policy::ResponseOutputPolicy,
    response_persistence_policy::ResponsePersistencePolicy,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct CompassAppSystemParameters {
    pub parallelism: Option<usize>,
    pub default_edge_list: Option<usize>,
    pub response_persistence_policy: Option<ResponsePersistencePolicy>,
    pub response_output_policy: Option<ResponseOutputPolicy>,
}
