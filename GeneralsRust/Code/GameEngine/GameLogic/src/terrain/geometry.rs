//! Small geometry and path-resolution helpers shared by terrain behavior modules.

use super::*;

pub(super) fn point_in_rotated_rect(
    px: f32,
    py: f32,
    cx: f32,
    cy: f32,
    half_w: f32,
    half_h: f32,
    cos_a: f32,
    sin_a: f32,
) -> bool {
    let dx = px - cx;
    let dy = py - cy;
    let local_x = dx * cos_a + dy * sin_a;
    let local_y = -dx * sin_a + dy * cos_a;
    local_x.abs() <= half_w && local_y.abs() <= half_h
}

pub(super) fn path_with_map_variants(input: &Path) -> Vec<PathBuf> {
    let mut variants = vec![input.to_path_buf()];
    if input.extension().is_none() {
        variants.push(input.with_extension("map"));
        variants.push(input.with_extension("MAP"));
    }
    variants
}
