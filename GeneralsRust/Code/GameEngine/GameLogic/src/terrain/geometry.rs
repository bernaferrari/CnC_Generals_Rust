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

pub(super) fn point_in_convex_quad(point: &Coord2D, quad: &[Coord2D; 4]) -> bool {
    let mut has_positive = false;
    let mut has_negative = false;

    for edge in 0..4 {
        let a = quad[edge];
        let b = quad[(edge + 1) % 4];
        let cross = cross_2d(&a, &b, point);
        if cross > 1.0e-5 {
            has_positive = true;
        } else if cross < -1.0e-5 {
            has_negative = true;
        }

        if has_positive && has_negative {
            return false;
        }
    }

    true
}

pub(super) fn cross_2d(a: &Coord2D, b: &Coord2D, p: &Coord2D) -> f32 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ap_x = p.x - a.x;
    let ap_y = p.y - a.y;
    ab_x * ap_y - ab_y * ap_x
}

pub(super) fn path_with_map_variants(input: &Path) -> Vec<PathBuf> {
    let mut variants = vec![input.to_path_buf()];
    if input.extension().is_none() {
        variants.push(input.with_extension("map"));
        variants.push(input.with_extension("MAP"));
    }
    variants
}

pub(super) fn line_in_region(line1: &Coord2D, line2: &Coord2D, region: &Region2D) -> bool {
    // Liang-Barsky clipping for axis-aligned rectangle.
    let x0 = line1.x;
    let y0 = line1.y;
    let x1 = line2.x;
    let y1 = line2.y;
    let dx = x1 - x0;
    let dy = y1 - y0;
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;

    let clip = |p: f32, q: f32, t0: &mut f32, t1: &mut f32| -> bool {
        if p.abs() <= f32::EPSILON {
            return q >= 0.0;
        }
        let r = q / p;
        if p < 0.0 {
            if r > *t1 {
                return false;
            }
            if r > *t0 {
                *t0 = r;
            }
        } else {
            if r < *t0 {
                return false;
            }
            if r < *t1 {
                *t1 = r;
            }
        }
        true
    };

    let left = region.lo.x;
    let right = region.hi.x;
    let top = region.lo.y;
    let bottom = region.hi.y;

    if !clip(-dx, x0 - left, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dx, right - x0, &mut t0, &mut t1) {
        return false;
    }
    if !clip(-dy, y0 - top, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dy, bottom - y0, &mut t0, &mut t1) {
        return false;
    }

    t0 <= t1
}
