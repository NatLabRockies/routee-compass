use serde::Deserialize;

/// JSON-deserializable request for map matching.
#[derive(Debug, Clone, Deserialize)]
pub struct MapMatchingRequest {
    /// The GPS trace to match to the road network.
    pub trace: Vec<TracePoint>,
}

/// A single GPS point in the request trace.
#[derive(Debug, Clone, Deserialize)]
pub struct TracePoint {
    /// Longitude (x coordinate)
    pub x: f64,

    /// Latitude (y coordinate)  
    pub y: f64,

    /// Optional timestamp as ISO 8601 string
    #[serde(default)]
    pub timestamp: Option<String>,
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
                {"x": -105.1, "y": 40.1, "timestamp": "2024-01-01T12:00:00Z"}
            ]
        }"#;

        let request: MapMatchingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.trace.len(), 2);
        assert!(request.trace[0].timestamp.is_none());
        assert!(request.trace[1].timestamp.is_some());
    }

    #[test]
    fn test_empty_trace_validation() {
        let request = MapMatchingRequest { trace: vec![] };
        assert!(request.validate().is_err());
    }
}
