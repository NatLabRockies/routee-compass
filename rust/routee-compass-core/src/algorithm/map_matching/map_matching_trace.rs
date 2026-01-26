use geo::Point;
use serde::{Deserialize, Serialize};

/// A GPS trace consisting of a sequence of points to be matched to the road network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMatchingTrace {
    /// Ordered sequence of GPS points in the trace
    pub points: Vec<MapMatchingPoint>,
}

impl MapMatchingTrace {
    /// Creates a new trace from a vector of points.
    pub fn new(points: Vec<MapMatchingPoint>) -> Self {
        Self { points }
    }

    /// Returns the number of points in the trace.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Returns true if the trace has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// A single GPS point in a trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapMatchingPoint {
    /// Geographic coordinate of the GPS observation
    pub coord: Point<f32>,

    /// Optional timestamp of the observation as an ISO 8601 string.
    /// Using String to avoid chrono serde feature dependencies.
    #[serde(default)]
    pub timestamp: Option<String>,
}

impl MapMatchingPoint {
    /// Creates a new point with just coordinates.
    pub fn new(coord: Point<f32>) -> Self {
        Self {
            coord,
            timestamp: None,
        }
    }

    /// Creates a new point with coordinates and timestamp.
    pub fn with_timestamp(coord: Point<f32>, timestamp: String) -> Self {
        Self {
            coord,
            timestamp: Some(timestamp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::point;

    #[test]
    fn test_trace_creation() {
        let points = vec![
            MapMatchingPoint::new(point!(x: -105.0, y: 40.0)),
            MapMatchingPoint::new(point!(x: -105.1, y: 40.1)),
        ];
        let trace = MapMatchingTrace::new(points);
        assert_eq!(trace.len(), 2);
        assert!(!trace.is_empty());
    }

    #[test]
    fn test_empty_trace() {
        let trace = MapMatchingTrace::new(vec![]);
        assert!(trace.is_empty());
        assert_eq!(trace.len(), 0);
    }
}
