use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectorReference {
    pub connector_id: String,
    pub at: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SegmentRoute {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub network: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none", default)]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wikidata: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub between: Option<Vec<f64>>,
}
