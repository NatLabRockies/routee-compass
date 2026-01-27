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

/// Computes the vertex ID for a grid position
fn vertex_id(row: usize, col: usize) -> usize {
    row * GRID_COLS + col
}

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
// App Loading Helpers
// =============================================================================

/// Helper to load the CompassApp with the simple map matching config
fn load_simple_app() -> CompassApp {
    let conf_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("app")
        .join("compass")
        .join("test")
        .join("map_matching_test")
        .join("compass.toml");
    CompassApp::try_from(conf_file.as_path()).expect("failed to load simple map matching config")
}

/// Helper to load the CompassApp with the HMM map matching config
fn load_hmm_app() -> CompassApp {
    let conf_file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("app")
        .join("compass")
        .join("test")
        .join("map_matching_test")
        .join("compass_hmm.toml");
    CompassApp::try_from(conf_file.as_path()).expect("failed to load HMM map matching config")
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
        "{}\n[map_matching]\ntype = \"simple\"\n[mapping]\nspatial_index_type = \"edge\"",
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
    let app = load_simple_app();

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
}

#[test]
fn test_map_matching_simple_long_trace() {
    let app = load_simple_app();

    // Construct a trace moving East along the top row of the grid
    // Path: 0 -> 1 -> ... -> 9
    // Edges: 0, 2, 4, 6, 8, 10, 12, 14, 16 (horizontal edges)
    // Points: Midpoints of these edges
    let trace_points: Vec<serde_json::Value> = (0..9)
        .map(|i| {
            let x = -105.0 + (i as f64 * 0.01) + 0.005;
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
// HMM Map Matching Tests
// =============================================================================
//
// NOTE: The HMM algorithm finds k-nearest candidate edges for each point and
// uses transition probabilities to find a globally optimal path. The spatial
// index may return candidates from multiple rows. These tests verify:
// 1. All matched edges exist in the network
// 2. Edge IDs form a coherent horizontal or vertical progression
// 3. The matched path is consistent across all points

/// Extracts the row number from an edge ID
/// Row 0: edges 0-18, Row 1: edges 19-37, Row 2: edges 38-56, etc.
fn edge_row(edge_id: i64) -> usize {
    (edge_id as usize) / edges_per_row()
}

/// Extracts whether an edge is horizontal (vs vertical) within its row
fn is_horizontal_edge(edge_id: i64) -> bool {
    let within_row = (edge_id as usize) % edges_per_row();
    // In each row: 0, 2, 4, ..., 16 are horizontal (even positions for cols 0-8)
    within_row < 2 * (GRID_COLS - 1) && within_row % 2 == 0
}

/// Returns the column of a horizontal edge within its row
fn horizontal_edge_col(edge_id: i64) -> usize {
    let within_row = (edge_id as usize) % edges_per_row();
    within_row / 2
}

#[test]
fn test_hmm_basic_trace() {
    let app = load_hmm_app();

    // 5 points along a horizontal path
    let trace_points: Vec<serde_json::Value> = (0..5)
        .map(|col| {
            let x = horizontal_edge_midpoint_x(col);
            let y = row_y(0);
            serde_json::json!({"x": x, "y": y})
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

    assert_eq!(point_matches.len(), 5);

    // Extract edge IDs
    let edge_ids: Vec<i64> = point_matches
        .iter()
        .map(|m| m.get("edge_id").unwrap().as_i64().unwrap())
        .collect();

    // All edges should be on the same row
    let first_row = edge_row(edge_ids[0]);
    for (i, &edge_id) in edge_ids.iter().enumerate() {
        let row = edge_row(edge_id);
        assert_eq!(
            row, first_row,
            "HMM basic trace: edge {} at point {} is on row {}, expected row {}",
            edge_id, i, row, first_row
        );
    }

    // All edges should be horizontal
    for (i, &edge_id) in edge_ids.iter().enumerate() {
        assert!(
            is_horizontal_edge(edge_id),
            "HMM basic trace: edge {} at point {} is not horizontal",
            edge_id,
            i
        );
    }

    // Edges should progress eastward (increasing column)
    for i in 1..edge_ids.len() {
        let col_prev = horizontal_edge_col(edge_ids[i - 1]);
        let col_curr = horizontal_edge_col(edge_ids[i]);
        assert!(
            col_curr >= col_prev,
            "HMM basic trace: column regression from {} to {} at point {}",
            col_prev,
            col_curr,
            i
        );
    }
}

#[test]
fn test_hmm_eastward_horizontal_trace() {
    let app = load_hmm_app();

    // Trace moving East along row 0
    let trace_points: Vec<serde_json::Value> = (0..5)
        .map(|col| {
            let x = horizontal_edge_midpoint_x(col);
            let y = row_y(0);
            serde_json::json!({"x": x, "y": y})
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

    assert_eq!(point_matches.len(), 5);

    let edge_ids: Vec<i64> = point_matches
        .iter()
        .map(|m| m.get("edge_id").unwrap().as_i64().unwrap())
        .collect();

    // All edges should be on the same row (may not be row 0 due to HMM optimization)
    let matched_row = edge_row(edge_ids[0]);
    for (i, &edge_id) in edge_ids.iter().enumerate() {
        assert_eq!(
            edge_row(edge_id),
            matched_row,
            "HMM eastward trace: row inconsistency at point {}, edge {}",
            i,
            edge_id
        );
    }

    // All should be horizontal edges with correct column progression
    for i in 0..edge_ids.len() {
        assert!(
            is_horizontal_edge(edge_ids[i]),
            "HMM eastward trace: edge {} at point {} is not horizontal",
            edge_ids[i],
            i
        );
        let expected_col = i;
        let actual_col = horizontal_edge_col(edge_ids[i]);
        assert_eq!(
            actual_col, expected_col,
            "HMM eastward trace: column mismatch at point {}, expected col {}, got {}",
            i, expected_col, actual_col
        );
    }

    // Verify matched_path contains the same edges
    let matched_path = result[0]
        .get("matched_path")
        .expect("result has matched_path")
        .as_array()
        .expect("matched_path is array");

    assert_eq!(matched_path.len(), 5, "Expected 5 edges in matched path");
}

/// Returns whether an edge is vertical and its row number
fn is_vertical_edge(edge_id: i64) -> bool {
    let within_row = (edge_id as usize) % edges_per_row();
    // Vertical edges are at odd positions (1, 3, 5, ..., 17) plus the last one (18)
    within_row % 2 == 1 || within_row == 2 * (GRID_COLS - 1)
}

/// Returns the row of a vertical edge (the row it originates from)
fn vertical_edge_origin_row(edge_id: i64) -> usize {
    (edge_id as usize) / edges_per_row()
}

#[test]
fn test_hmm_northward_vertical_trace() {
    let app = load_hmm_app();

    // Trace moving North along column 0
    let trace_points: Vec<serde_json::Value> = (0..5)
        .map(|row| {
            let x = col_x(0);
            let y = vertical_edge_midpoint_y(row);
            serde_json::json!({"x": x, "y": y})
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

    assert_eq!(point_matches.len(), 5);

    let edge_ids: Vec<i64> = point_matches
        .iter()
        .map(|m| m.get("edge_id").unwrap().as_i64().unwrap())
        .collect();

    // All should be vertical edges with progressive rows
    for (i, &edge_id) in edge_ids.iter().enumerate() {
        assert!(
            is_vertical_edge(edge_id),
            "HMM northward trace: edge {} at point {} is not vertical",
            edge_id,
            i
        );
    }

    // Rows should be increasing
    for i in 1..edge_ids.len() {
        let row_prev = vertical_edge_origin_row(edge_ids[i - 1]);
        let row_curr = vertical_edge_origin_row(edge_ids[i]);
        assert!(
            row_curr >= row_prev,
            "HMM northward trace: row regression from {} to {} at point {}",
            row_prev,
            row_curr,
            i
        );
    }

    // Verify matched_path
    let matched_path = result[0]
        .get("matched_path")
        .expect("result has matched_path")
        .as_array()
        .expect("matched_path is array");

    assert_eq!(matched_path.len(), 5, "Expected 5 edges in matched path");
}

#[test]
fn test_hmm_l_shaped_path() {
    let app = load_hmm_app();

    // L-turn: East along row, then North along column
    let trace_points = vec![
        // Horizontal edges
        serde_json::json!({"x": horizontal_edge_midpoint_x(0), "y": row_y(0)}),
        serde_json::json!({"x": horizontal_edge_midpoint_x(1), "y": row_y(0)}),
        // Vertical edges at column 2
        serde_json::json!({"x": col_x(2), "y": vertical_edge_midpoint_y(0)}),
        serde_json::json!({"x": col_x(2), "y": vertical_edge_midpoint_y(1)}),
        serde_json::json!({"x": col_x(2), "y": vertical_edge_midpoint_y(2)}),
    ];

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

    assert_eq!(point_matches.len(), 5);

    let edge_ids: Vec<i64> = point_matches
        .iter()
        .map(|m| m.get("edge_id").unwrap().as_i64().unwrap())
        .collect();

    // First two edges should be horizontal (eastward movement)
    for i in 0..2 {
        assert!(
            is_horizontal_edge(edge_ids[i]),
            "HMM L-shaped: edge {} at point {} should be horizontal",
            edge_ids[i],
            i
        );
    }

    // Last three edges should be vertical (northward movement)
    for i in 2..5 {
        assert!(
            is_vertical_edge(edge_ids[i]),
            "HMM L-shaped: edge {} at point {} should be vertical",
            edge_ids[i],
            i
        );
    }

    // Verify matched_path shows the turn
    let matched_path = result[0]
        .get("matched_path")
        .expect("result has matched_path")
        .as_array()
        .expect("matched_path is array");

    assert!(
        matched_path.len() >= 5,
        "L-shaped path should have at least 5 edges, got {}",
        matched_path.len()
    );
}

#[test]
fn test_hmm_noisy_trace() {
    let app = load_hmm_app();

    // Trace with GPS noise - points perturbed north/south of row 0
    let trace_points = vec![
        serde_json::json!({"x": horizontal_edge_midpoint_x(0), "y": row_y(0) + 0.0003}),
        serde_json::json!({"x": horizontal_edge_midpoint_x(1), "y": row_y(0) - 0.0003}),
        serde_json::json!({"x": horizontal_edge_midpoint_x(2), "y": row_y(0) + 0.0005}),
        serde_json::json!({"x": horizontal_edge_midpoint_x(3), "y": row_y(0) - 0.0002}),
        serde_json::json!({"x": horizontal_edge_midpoint_x(4), "y": row_y(0) + 0.0002}),
    ];

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

    assert_eq!(point_matches.len(), 5);

    let edge_ids: Vec<i64> = point_matches
        .iter()
        .map(|m| m.get("edge_id").unwrap().as_i64().unwrap())
        .collect();

    // Despite noise, all edges should be on the same row
    let matched_row = edge_row(edge_ids[0]);
    for (i, &edge_id) in edge_ids.iter().enumerate() {
        assert_eq!(
            edge_row(edge_id),
            matched_row,
            "HMM noisy trace: row inconsistency at point {}, edge {}",
            i,
            edge_id
        );
    }

    // All should be horizontal edges with correct column progression
    for i in 0..edge_ids.len() {
        assert!(
            is_horizontal_edge(edge_ids[i]),
            "HMM noisy trace: edge {} at point {} is not horizontal",
            edge_ids[i],
            i
        );
    }

    // Columns should progress correctly
    for i in 0..edge_ids.len() {
        let actual_col = horizontal_edge_col(edge_ids[i]);
        assert_eq!(
            actual_col, i,
            "HMM noisy trace: expected column {} at point {}, got {}",
            i, i, actual_col
        );
    }

    // Verify matched_path
    let matched_path = result[0]
        .get("matched_path")
        .expect("result has matched_path")
        .as_array()
        .expect("matched_path is array");

    assert_eq!(matched_path.len(), 5, "Expected 5 edges in matched path");
}
