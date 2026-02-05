//! Integration tests for map matching algorithms.
//!
//! These tests use the map_matching_test grid network which is a 10x10 grid:
//! - Vertices: 0-99, where vertex ID = row * 10 + col
//! - Grid origin: (-105.0, 40.0) at vertex 0, spacing 0.01 degrees
//! - Row 0 is at y=40.0, Row 1 at y=40.01, etc.
//! - Col 0 is at x=-105.0, Col 1 at x=-104.99, etc.
//!
//! Edge ID structure (per row):
//! - Each vertex has up to 2 edges: horizontal (to col+1) and vertical (to row+1)
//! - Within a row, edges are assigned: H, V, H, V, ... for each vertex left to right
//! - Row 0: vertices 0-8 have 2 edges each, vertex 9 has 1 (only vertical) = 19 edges (0-18)
//! - Row 1: starts at edge 19, etc.

use crate::app::compass::CompassApp;
use std::path::PathBuf;

// =============================================================================
// Grid Network Helper Functions
// =============================================================================

/// Grid configuration constants
const GRID_COLS: usize = 10;
const GRID_ROWS: usize = 10;
const BASE_X: f64 = -105.0;
const BASE_Y: f64 = 40.0;
const SPACING: f64 = 0.01;

/// Computes the edge ID for a horizontal edge (going right) at the given grid position.
/// Returns None if the position is at the right edge of the grid.
fn horizontal_edge_id(row: usize, col: usize) -> Option<i64> {
    if col >= GRID_COLS - 1 {
        return None; // No horizontal edge at rightmost column
    }

    // Count edges before this row
    let edges_before_row = edges_per_row() * row;

    // Within this row: each column contributes 2 edges (H + V) except the last column (V only)
    // For column c, horizontal edge is at position 2*c within the row's edges
    let edge_in_row = 2 * col;

    Some((edges_before_row + edge_in_row) as i64)
}

/// Computes the edge ID for a vertical edge (going up) at the given grid position.
/// Returns None if the position is at the top row of the grid.
fn vertical_edge_id(row: usize, col: usize) -> Option<i64> {
    if row >= GRID_ROWS - 1 {
        return None; // No vertical edge at topmost row
    }

    // Count edges before this row
    let edges_before_row = edges_per_row() * row;

    // Within this row: horizontal edges come first for columns 0..(COLS-1)
    // Then vertical edges. For column c with c < COLS-1, vertical edge is at 2*c + 1
    // For column c = COLS-1 (rightmost), only vertical edge exists at position 2*(COLS-1)
    let edge_in_row = if col < GRID_COLS - 1 {
        2 * col + 1
    } else {
        2 * (GRID_COLS - 1) // Last column only has vertical edge
    };

    Some((edges_before_row + edge_in_row) as i64)
}

/// Number of edges per row (9 horizontal + 10 vertical = 19)
fn edges_per_row() -> usize {
    // 9 horizontal edges (col 0-8 each have one) + 10 vertical edges (all cols have one)
    (GRID_COLS - 1) + GRID_COLS
}

/// Returns the x-coordinate for a column
fn col_x(col: usize) -> f64 {
    BASE_X + (col as f64 * SPACING)
}

/// Returns the y-coordinate for a row
fn row_y(row: usize) -> f64 {
    BASE_Y + (row as f64 * SPACING)
}

/// Returns the midpoint x-coordinate between two columns (for horizontal edge midpoint)
fn horizontal_edge_midpoint_x(col: usize) -> f64 {
    col_x(col) + SPACING / 2.0
}

/// Returns the midpoint y-coordinate between two rows (for vertical edge midpoint)
fn vertical_edge_midpoint_y(row: usize) -> f64 {
    row_y(row) + SPACING / 2.0
}

// =============================================================================
// Test Trace Abstraction
// =============================================================================

struct TestTrace {
    points: Vec<serde_json::Value>,
    expected_edges: Vec<i64>,
}

impl TestTrace {
    fn eastward_horizontal(row: usize, count: usize) -> Self {
        let points = (0..count)
            .map(|col| {
                let x = if col == 0 {
                    col_x(col) + SPACING * 0.25
                } else if col == count - 1 {
                    col_x(col) + SPACING * 0.75
                } else {
                    horizontal_edge_midpoint_x(col)
                };
                let y = row_y(row);
                serde_json::json!({"x": x, "y": y})
            })
            .collect();
        let expected_edges = (0..count)
            .map(|col| horizontal_edge_id(row, col).unwrap())
            .collect();
        Self {
            points,
            expected_edges,
        }
    }

    fn northward_vertical(col: usize, count: usize) -> Self {
        let points = (0..count)
            .map(|row| {
                let y = if row == 0 {
                    row_y(row) + SPACING * 0.25
                } else if row == count - 1 {
                    row_y(row) + SPACING * 0.75
                } else {
                    vertical_edge_midpoint_y(row)
                };
                let x = col_x(col);
                serde_json::json!({"x": x, "y": y})
            })
            .collect();
        let expected_edges = (0..count)
            .map(|row| vertical_edge_id(row, col).unwrap())
            .collect();
        Self {
            points,
            expected_edges,
        }
    }

    fn l_shaped() -> Self {
        // East along row 0 (cols 0, 1), then North along col 2 (rows 0, 1, 2)
        let points = vec![
            serde_json::json!({"x": col_x(0) + SPACING * 0.25, "y": row_y(0)}),
            serde_json::json!({"x": col_x(1) + SPACING * 0.25, "y": row_y(0)}),
            serde_json::json!({"x": col_x(2), "y": row_y(0) + SPACING * 0.25}),
            serde_json::json!({"x": col_x(2), "y": row_y(1) + SPACING * 0.25}),
            serde_json::json!({"x": col_x(2), "y": row_y(2) + SPACING * 0.75}),
        ];
        let expected_edges = vec![
            horizontal_edge_id(0, 0).unwrap(),
            horizontal_edge_id(0, 1).unwrap(),
            vertical_edge_id(0, 2).unwrap(),
            vertical_edge_id(1, 2).unwrap(),
            vertical_edge_id(2, 2).unwrap(),
        ];
        Self {
            points,
            expected_edges,
        }
    }

    fn noisy_eastward_horizontal(row: usize, count: usize) -> Self {
        let points = (0..count)
            .map(|col| {
                let noise = if col % 2 == 0 { 0.0003 } else { -0.0003 };
                let x = if col == 0 {
                    col_x(col) + SPACING * 0.25
                } else if col == count - 1 {
                    col_x(col) + SPACING * 0.75
                } else {
                    horizontal_edge_midpoint_x(col)
                };
                let y = row_y(row) + noise;
                serde_json::json!({"x": x, "y": y})
            })
            .collect();
        let expected_edges = (0..count)
            .map(|col| horizontal_edge_id(row, col).unwrap())
            .collect();
        Self {
            points,
            expected_edges,
        }
    }
}

fn run_map_match_test(app: &CompassApp, trace: TestTrace, label: &str) {
    let query = serde_json::json!({
        "trace": trace.points
    });
    let queries = vec![query];

    let result = app.map_match(&queries).unwrap();
    assert_eq!(result.len(), 1, "{}: Expected 1 result", label);

    let point_matches = result[0]
        .get("point_matches")
        .expect("result has point_matches")
        .as_array()
        .expect("point_matches is array");

    assert_eq!(
        point_matches.len(),
        trace.expected_edges.len(),
        "{}: point_matches length mismatch",
        label
    );

    for (i, (matched, &expected)) in point_matches
        .iter()
        .zip(trace.expected_edges.iter())
        .enumerate()
    {
        let actual = matched.get("edge_id").unwrap().as_i64().unwrap();
        assert_eq!(
            actual, expected,
            "{}: mismatch at point {}, expected edge {}, got {}",
            label, i, expected, actual
        );
    }
}

// =============================================================================
// App Loading Helpers
// =============================================================================

/// Helper to load the CompassApp with the LCSS map matching config
fn load_lcss_app() -> CompassApp {
    let conf_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("app")
        .join("compass")
        .join("test")
        .join("map_matching_test")
        .join("compass_lcss.toml");
    CompassApp::try_from(conf_file.as_path()).expect("failed to load LCSS map matching config")
}

// =============================================================================
// Grid Helper Tests (to verify our edge ID calculation is correct)
// =============================================================================

#[test]
fn test_grid_helper_horizontal_edges_row0() {
    // Row 0 horizontal edges should be: 0, 2, 4, 6, 8, 10, 12, 14, 16
    assert_eq!(horizontal_edge_id(0, 0), Some(0));
    assert_eq!(horizontal_edge_id(0, 1), Some(2));
    assert_eq!(horizontal_edge_id(0, 2), Some(4));
    assert_eq!(horizontal_edge_id(0, 3), Some(6));
    assert_eq!(horizontal_edge_id(0, 4), Some(8));
    assert_eq!(horizontal_edge_id(0, 5), Some(10));
    assert_eq!(horizontal_edge_id(0, 6), Some(12));
    assert_eq!(horizontal_edge_id(0, 7), Some(14));
    assert_eq!(horizontal_edge_id(0, 8), Some(16));
    assert_eq!(horizontal_edge_id(0, 9), None); // No horizontal edge at rightmost column
}

#[test]
fn test_grid_helper_vertical_edges_row0() {
    // Row 0 vertical edges should be: 1, 3, 5, 7, 9, 11, 13, 15, 17, 18
    assert_eq!(vertical_edge_id(0, 0), Some(1));
    assert_eq!(vertical_edge_id(0, 1), Some(3));
    assert_eq!(vertical_edge_id(0, 2), Some(5));
    assert_eq!(vertical_edge_id(0, 3), Some(7));
    assert_eq!(vertical_edge_id(0, 4), Some(9));
    assert_eq!(vertical_edge_id(0, 5), Some(11));
    assert_eq!(vertical_edge_id(0, 6), Some(13));
    assert_eq!(vertical_edge_id(0, 7), Some(15));
    assert_eq!(vertical_edge_id(0, 8), Some(17));
    assert_eq!(vertical_edge_id(0, 9), Some(18)); // Rightmost column
}

#[test]
fn test_grid_helper_row1_edges() {
    // Row 1 starts at edge 19
    // Horizontal edges: 19, 21, 23, 25, 27, 29, 31, 33, 35
    assert_eq!(horizontal_edge_id(1, 0), Some(19));
    assert_eq!(horizontal_edge_id(1, 1), Some(21));
    // Vertical edges: 20, 22, 24, 26, 28, 30, 32, 34, 36, 37
    assert_eq!(vertical_edge_id(1, 0), Some(20));
    assert_eq!(vertical_edge_id(1, 9), Some(37));
}

#[test]
fn test_map_match_json() {
    let conf_file_test = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("app")
        .join("compass")
        .join("test")
        .join("speeds_test")
        .join("speeds_test.toml");

    let conf_str = std::fs::read_to_string(&conf_file_test).unwrap();
    let conf_str_with_mm = format!(
        "{}\n[map_matching]\ntype = \"lcss\"\n[mapping]\nspatial_index_type = \"edge\"",
        conf_str
    );

    let config = crate::app::compass::CompassAppConfig::from_str(
        &conf_str_with_mm,
        conf_file_test.to_str().unwrap(),
        config::FileFormat::Toml,
    )
    .unwrap();
    let builder = crate::app::compass::CompassBuilderInventory::new().unwrap();
    let app = CompassApp::new(&config, &builder).unwrap();

    // Construct a simple trace within range of the test graph (Denver area)
    // Vertex 0: -105.1683038, 39.7379033
    // Vertex 2: -111.9095014, 40.7607176
    // Let's use points very close to Vertex 0
    let query = serde_json::json!({
        "trace": [
            {"x": -105.1683, "y": 39.7379},
            {"x": -105.1683, "y": 39.7379}
        ]
    });
    let queries = vec![query];

    // Execute map match
    let result = app.map_match(&queries).unwrap();

    // Verify result structure
    assert_eq!(result.len(), 1);
    let first_result = &result[0];
    assert!(first_result.get("point_matches").is_some());
    assert!(first_result.get("matched_path").is_some());
}

#[test]
fn test_map_matching_simple_single_point() {
    let app = load_lcss_app();

    // Query point near edge 0
    // Edge 0: (-105.0, 40.0) -> (-104.99, 40.0)
    // Midpoint: (-104.995, 40.0)
    let query = serde_json::json!({
        "trace": [
            {"x": -104.995, "y": 40.0}
        ]
    });
    let queries = vec![query];

    // Execute map match
    let result = app.map_match(&queries).unwrap();

    // Verify result matches Edge 0
    assert_eq!(result.len(), 1);
    let first_result = &result[0];
    let point_matches = first_result
        .get("point_matches")
        .expect("result has point_matches");
    let first_match = &point_matches[0];
    let edge_id = first_match
        .get("edge_id")
        .expect("match has edge_id")
        .as_i64()
        .expect("edge_id is i64");
    assert_eq!(edge_id, 0);

    // Vertex-oriented LCSS algorithm will return an empty FeatureCollection for a single point
    let matched_path = first_result.get("matched_path").unwrap();
    assert_eq!(
        matched_path.get("type").unwrap().as_str().unwrap(),
        "FeatureCollection"
    );
    assert!(matched_path
        .get("features")
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn test_map_matching_simple_long_trace() {
    let app = load_lcss_app();

    // Construct a trace moving East along the top row of the grid
    // Path: 0 -> 1 -> ... -> 9
    // Edges: 0, 2, 4, 6, 8, 10, 12, 14, 16 (horizontal edges)
    // Points: Midpoints of these edges
    let trace_points: Vec<serde_json::Value> = (0..9)
        .map(|i| {
            let x = if i == 0 {
                -105.0 + 0.0025
            } else if i == 8 {
                -105.0 + (i as f64 * 0.01) + 0.0075
            } else {
                -105.0 + (i as f64 * 0.01) + 0.005
            };
            serde_json::json!({"x": x, "y": 40.0})
        })
        .collect();

    let query = serde_json::json!({
        "trace": trace_points
    });
    let queries = vec![query];

    let result = app.map_match(&queries).unwrap();
    assert_eq!(result.len(), 1);

    let point_matches = result[0]
        .get("point_matches")
        .expect("result has point_matches")
        .as_array()
        .expect("point_matches is array");

    assert_eq!(point_matches.len(), 9);

    // Expected edge IDs: 0, 2, 4, 6, 8, 10, 12, 14, 16 (stride 2 for horizontal edges)
    for (i, matched) in point_matches.iter().enumerate() {
        let edge_id = matched.get("edge_id").unwrap().as_i64().unwrap();
        assert_eq!(edge_id, (i * 2) as i64, "Mismatch at index {}", i);
    }
}

// =============================================================================
// LCSS Map Matching Tests
// =============================================================================

#[test]
fn test_lcss_eastward_horizontal_trace() {
    let app = load_lcss_app();
    let trace = TestTrace::eastward_horizontal(0, 5);
    run_map_match_test(&app, trace, "LCSS eastward horizontal");
}

#[test]
fn test_lcss_northward_vertical_trace() {
    let app = load_lcss_app();
    let trace = TestTrace::northward_vertical(0, 5);
    run_map_match_test(&app, trace, "LCSS northward vertical");
}

#[test]
fn test_lcss_l_shaped_path() {
    let app = load_lcss_app();
    let trace = TestTrace::l_shaped();
    run_map_match_test(&app, trace, "LCSS L-shaped");
}

#[test]
fn test_lcss_noisy_trace() {
    let app = load_lcss_app();
    let trace = TestTrace::noisy_eastward_horizontal(0, 5);
    run_map_match_test(&app, trace, "LCSS noisy horizontal");
}
#[test]
fn test_map_matching_with_geometry() {
    let app = load_lcss_app();

    // Query points near edges 0 and 2
    let query = serde_json::json!({
        "trace": [
            {"x": col_x(0) + SPACING * 0.25, "y": row_y(0)},
            {"x": col_x(1) + SPACING * 0.75, "y": row_y(0)}
        ],
        "output_format": "json"
    });
    let queries = vec![query];

    // Execute map match
    let result = app.map_match(&queries).unwrap();

    // Verify result has geometry
    assert_eq!(result.len(), 1);
    let first_result = &result[0];

    // Check point matches
    let point_matches = first_result
        .get("point_matches")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(point_matches.len(), 2);
    assert_eq!(
        point_matches[0].get("edge_id").unwrap().as_i64().unwrap(),
        0
    );
    assert_eq!(
        point_matches[1].get("edge_id").unwrap().as_i64().unwrap(),
        2
    );

    // Check matched path
    let matched_path = first_result
        .get("matched_path")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(matched_path.len(), 2);
    let matched_edge = &matched_path[0];
    assert_eq!(matched_edge.get("edge_id").unwrap().as_i64().unwrap(), 0);

    // Check geometry inside matched_path objects
    for edge in matched_path {
        assert!(
            edge.get("geometry").is_some(),
            "geometry should be present by default"
        );
        let geometry = edge
            .get("geometry")
            .unwrap()
            .as_array()
            .expect("geometry should be an array");
        assert!(!geometry.is_empty(), "geometry should not be empty");
    }

    // Verify it can be disabled
    let query_no_geom = serde_json::json!({
        "trace": [
            {"x": col_x(0) + SPACING * 0.25, "y": row_y(0)},
            {"x": col_x(1) + SPACING * 0.75, "y": row_y(0)}
        ],
        "output_format": "edge_id"
    });
    let result_no_geom = app.map_match(&vec![query_no_geom]).unwrap();
    let matched_path_no_geom = result_no_geom[0]
        .get("matched_path")
        .unwrap()
        .as_array()
        .unwrap();
    // edge_id format returns a list of edge IDs (i64)
    assert_eq!(matched_path_no_geom[0].as_i64().unwrap(), 0);
}

#[test]
fn test_map_matching_formats_and_summaries() {
    let app = load_lcss_app();

    // Query points near edges 0 and 2
    let query = serde_json::json!({
        "trace": [
            {"x": col_x(0) + SPACING * 0.25, "y": row_y(0)},
            {"x": col_x(1) + SPACING * 0.75, "y": row_y(0)}
        ],
        "output_format": "wkt",
        "summary_ops": {
            "trip_distance": "sum"
        }
    });
    let queries = vec![query];

    // Execute map match
    let result = app.map_match(&queries).unwrap();
    assert_eq!(result.len(), 1);
    let first_result = &result[0];

    // Check matched path is WKT (string)
    let matched_path = first_result
        .get("matched_path")
        .expect("should have matched_path")
        .as_str()
        .expect("matched_path should be a string (WKT)");
    assert!(matched_path.starts_with("LINESTRING"));

    // Check traversal summary exists and has trip_distance with new structured format
    let summary = first_result
        .get("traversal_summary")
        .expect("should have traversal_summary")
        .as_object()
        .expect("traversal_summary should be an object");
    let trip_distance = summary
        .get("trip_distance")
        .expect("should have trip_distance in summary")
        .as_object()
        .expect("trip_distance summary should be an object");

    assert!(trip_distance.contains_key("value"));
    assert!(trip_distance.contains_key("unit"));
    assert_eq!(trip_distance.get("op").unwrap().as_str().unwrap(), "sum");

    // Verify GeoJSON format
    let query_geojson = serde_json::json!({
        "trace": [
            {"x": col_x(0) + SPACING * 0.25, "y": row_y(0)},
            {"x": col_x(1) + SPACING * 0.75, "y": row_y(0)}
        ],
        "output_format": "geo_json"
    });
    let result_geojson = app.map_match(&vec![query_geojson]).unwrap();
    let matched_path_geojson = result_geojson[0]
        .get("matched_path")
        .expect("should have matched_path")
        .as_object()
        .expect("matched_path should be a GeoJSON object");
    assert_eq!(
        matched_path_geojson.get("type").unwrap().as_str().unwrap(),
        "FeatureCollection"
    );
}
