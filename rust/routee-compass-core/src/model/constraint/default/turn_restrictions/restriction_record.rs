use serde::{Deserialize, Serialize};

use crate::model::network::EdgeId;

/// a row in the turn restrictions CSV file.
#[derive(Eq, PartialEq, Hash, Deserialize, Serialize, Clone)]
pub struct RestrictionRecord {
    pub prev_edge_id: EdgeId,
    pub next_edge_id: EdgeId,
}