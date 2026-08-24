//! Metrics helpers: SVG sparkline generation for CPU/memory series.

/// Normalize a series to `[0, 1]`, preserving relative shape.
///
/// An empty series yields an empty result; a flat series maps to the midline
/// (`0.5`).
pub fn normalize(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        return values.iter().map(|_| 0.5).collect();
    }
    values.iter().map(|v| (v - min) / (max - min)).collect()
}

/// Map normalized values to SVG polyline points within a viewport.
///
/// `x` spans `0..width`; `y` is inverted so higher values sit nearer the top.
pub fn polyline_points(normalized: &[f64], width: f64, height: f64) -> Vec<(f64, f64)> {
    let n = normalized.len();
    if n == 0 {
        return Vec::new();
    }
    let step = if n == 1 {
        0.0
    } else {
        width / (n as f64 - 1.0)
    };
    normalized
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64 * step, height - v * height))
        .collect()
}

/// Render a series as an SVG sparkline (a single theme-aware polyline, no axes).
///
/// Uses `currentColor` for the stroke so the line follows the app theme's
/// foreground color with no re-render.
pub fn sparkline(values: &[f64], width: f64, height: f64) -> String {
    let points = polyline_points(&normalize(values), width, height);
    let coords = points
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" preserveAspectRatio="none"><polyline fill="none" stroke="currentColor" stroke-width="1.5" points="{p}"/></svg>"#,
        w = width,
        h = height,
        p = coords,
    )
}
