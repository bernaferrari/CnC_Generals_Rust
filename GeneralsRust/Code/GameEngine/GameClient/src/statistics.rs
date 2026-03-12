//! Statistical helper functions (ported from GameClient/Statistics.cpp).

/// Apply mu-law companding.
pub fn mu_law(value: f32, max_value: f32, mu: f32) -> f32 {
    let test_val = (value - max_value / 2.0) / (max_value / 2.0);
    test_val.signum() * (1.0 + mu * test_val.abs()).ln() / (1.0 + mu).ln()
}

/// Normalize a value from min/max to 0..1.
pub fn normalize(value: f32, min_range: f32, max_range: f32) -> f32 {
    (value - min_range) / (max_range - min_range)
}

/// Normalize to an arbitrary output range.
pub fn normalize_to_range(
    value: f32,
    min_range: f32,
    max_range: f32,
    out_min: f32,
    out_max: f32,
) -> f32 {
    normalize(value, min_range, max_range) * (out_max - out_min) + out_min
}
