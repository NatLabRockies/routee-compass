use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OvertureMapsBbox {
    xmin: Option<f32>,
    xmax: Option<f32>,
    ymin: Option<f32>,
    ymax: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OvertureMapsSource {
    property: Option<String>,
    dataset: Option<String>,
    record_id: Option<String>,
    update_time: Option<String>,
    confidence: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct OvertureMapsNames {
    primary: Option<String>,
    common: Option<HashMap<String, Option<String>>>,
    rules: Option<Vec<OvertureMapsNamesRule>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct OvertureMapsNamesRule {
    variant: Option<String>,
    language: Option<String>,
    value: Option<String>,
    between: Option<Vec<f64>>,
    side: Option<String>,
}
