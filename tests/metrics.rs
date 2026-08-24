//! Integration tests for metrics sparkline generation.

use openkite::metrics::{normalize, polyline_points, sparkline};

#[test]
fn normalize_maps_range_to_unit() {
    let n = normalize(&[0.0, 50.0, 100.0]);
    assert_eq!(n, vec![0.0, 0.5, 1.0]);
}

#[test]
fn normalize_flat_series_is_midline() {
    let n = normalize(&[7.0, 7.0, 7.0]);
    assert!(n.iter().all(|&v| (v - 0.5).abs() < 1e-9));
}

#[test]
fn normalize_empty_is_empty() {
    assert!(normalize(&[]).is_empty());
}

#[test]
fn points_invert_y_so_high_values_are_high() {
    let pts = polyline_points(&[0.0, 1.0], 100.0, 50.0);
    assert_eq!(pts, vec![(0.0, 50.0), (100.0, 0.0)]);
}

#[test]
fn single_point_is_centered_horizontally() {
    let pts = polyline_points(&[0.5], 100.0, 50.0);
    assert_eq!(pts.len(), 1);
    assert_eq!(pts[0], (0.0, 25.0));
}

#[test]
fn sparkline_emits_polyline() {
    let svg = sparkline(&[0.0, 1.0, 0.5], 100.0, 30.0);
    assert!(svg.contains("<polyline"));
    assert!(svg.contains("points=\""));
    assert!(svg.contains("currentColor"));
}
