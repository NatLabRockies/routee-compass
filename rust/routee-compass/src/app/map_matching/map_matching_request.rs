use crate::app::search::SummaryOp;
use crate::plugin::output::default::traversal::TraversalOutputFormat;
use serde::Deserialize;
use std::collections::HashMap;

/// JSON-deserializable request for map matching.
#[derive(Debug, Clone, Deserialize)]
pub struct MapMatchingRequest {
    /// The GPS trace to match to the road network.
    pub trace: Vec<TracePoint>,
    /// Optional search configuration to override defaults.
    #[serde(default)]
    pub search_parameters: Option<serde_json::Value>,
    /// The format to return the matched path in.
    #[serde(default = "default_output_format")]
    pub output_format: TraversalOutputFormat,
    /// Operations to perform on the search state for the final summary.
    #[serde(default = "default_summary_ops")]
    pub summary_ops: HashMap<String, SummaryOp>,
}

fn default_output_format() -> TraversalOutputFormat {
    TraversalOutputFormat::GeoJson
}

fn default_summary_ops() -> HashMap<String, SummaryOp> {
    HashMap::new()
}

/// A single GPS point in the request trace.
#[derive(Debug, Clone, Deserialize)]
pub struct TracePoint {
    /// Longitude (x coordinate)
    pub x: f64,

    /// Latitude (y coordinate)  
    pub y: f64,
}

impl MapMatchingRequest {
    /// Validates the request and returns an error message if invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.trace.is_empty() {
            return Err("trace cannot be empty".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_request() {
        let json = r#"{
            "trace": [
                {"x": -105.0, "y": 40.0},
                {"x": -105.1, "y": 40.1}
            ]
        }"#;

        let request: MapMatchingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.trace.len(), 2);
    }

    #[test]
    fn test_empty_trace_validation() {
        let request = MapMatchingRequest {
            trace: vec![],
            search_parameters: None,
            output_format: TraversalOutputFormat::Json,
            summary_ops: HashMap::new(),
        };
        assert!(request.validate().is_err());
    }
}
