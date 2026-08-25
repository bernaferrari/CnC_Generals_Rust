//! Terrain line-of-sight.
//!
//! C++ `W3DTerrainLogic::isClearLineOfSight` forwards to
//! `BaseHeightMapRenderObjClass::isClearLineOfSight` (`DO_BRESENHAM`).

use crate::common::{Coord3D, MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

/// C++ `BaseHeightMap.cpp` `const Real LOS_FUDGE = 0.5f`.
pub const LOS_FUDGE: f32 = 0.5;

/// C++ `BaseHeightMapRenderObjClass::isClearLineOfSight` (lines 979-1109).
pub fn is_clear_line_of_sight(
    pos: &Coord3D,
    pos_other: &Coord3D,
    map_data: &[u8],
    map_dx: i32,
    map_dy: i32,
    border_size: i32,
    map_max_z: f32,
) -> bool {
    // C++: `if (m_map == NULL) return false;`
    if map_data.is_empty() || map_dx <= 0 || map_dy <= 0 {
        return false;
    }

    let map_xy_factor_inv = 1.0 / MAP_XY_FACTOR;
    let border = border_size.max(0);
    let start_x = (pos.x * map_xy_factor_inv).floor() as i32 + border;
    let start_y = (pos.y * map_xy_factor_inv).floor() as i32 + border;
    let end_x = (pos_other.x * map_xy_factor_inv).floor() as i32 + border;
    let end_y = (pos_other.y * map_xy_factor_inv).floor() as i32 + border;

    let delta_x = (end_x - start_x).abs();
    let delta_y = (end_y - start_y).abs();
    let mut x = start_x;
    let mut y = start_y;

    let (mut xinc1, mut xinc2) = if end_x >= start_x { (1, 1) } else { (-1, -1) };
    let (mut yinc1, mut yinc2) = if end_y >= start_y { (1, 1) } else { (-1, -1) };

    let (den, mut num, numadd, numpixels) = if delta_x >= delta_y {
        xinc1 = 0;
        yinc2 = 0;
        (delta_x, delta_x / 2, delta_y, delta_x)
    } else {
        xinc2 = 0;
        yinc1 = 0;
        (delta_y, delta_y / 2, delta_x, delta_y)
    };

    if numpixels == 0 {
        return true;
    }

    let ns_inv = 1.0 / numpixels as f32;
    let mut z = pos.z;
    let zinc = (pos_other.z - pos.z) * ns_inv;
    let x_extent = map_dx;
    let y_extent = map_dy;

    for _ in 0..numpixels {
        if x < 0 || y < 0 || x >= x_extent - 1 || y >= y_extent - 1 {
            break;
        }

        let idx = (x + y * x_extent) as usize;
        let stride = x_extent as usize;
        if idx + stride + 1 >= map_data.len() {
            break;
        }

        let mut height = map_data[idx] as f32;
        height = height.max(map_data[idx + 1] as f32);
        height = height.max(map_data[idx + stride] as f32);
        height = height.max(map_data[idx + stride + 1] as f32);
        height *= MAP_HEIGHT_SCALE;

        if height > z + LOS_FUDGE {
            return false;
        }

        if z >= map_max_z && zinc > 0.0 {
            break;
        }

        z += zinc;

        num += numadd;
        if num >= den {
            num -= den;
            x += xinc1;
            y += yinc1;
        }
        x += xinc2;
        y += yinc2;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_map_is_blocked_like_cpp_null_heightmap() {
        let a = Coord3D::new(0.0, 0.0, 10.0);
        let b = Coord3D::new(40.0, 0.0, 10.0);
        assert!(!is_clear_line_of_sight(&a, &b, &[], 0, 0, 0, 10.0));
    }

    #[test]
    fn ridge_blocks_when_max_of_four_corners_exceeds_fudge() {
        // 3x2 cells so the walk can sample cell (0,0) and (1,0).
        let mut data = vec![0u8; 3 * 2];
        // Raise the four corners of cell (1,0) well above the LOS line.
        data[1] = 80;
        data[2] = 80;
        data[1 + 3] = 80;
        data[2 + 3] = 80;
        let a = Coord3D::new(0.0, 0.0, 1.0);
        let b = Coord3D::new(20.0, 0.0, 1.0);
        assert!(!is_clear_line_of_sight(
            &a,
            &b,
            &data,
            3,
            2,
            0,
            80.0 * MAP_HEIGHT_SCALE
        ));
    }

    #[test]
    fn small_bump_under_half_unit_does_not_block() {
        let mut data = vec![0u8; 3 * 2];
        // MAP_HEIGHT_SCALE is 0.625; a single-step of 0 is under 0.5 fudge.
        data[1] = 0;
        let a = Coord3D::new(0.0, 0.0, 1.0);
        let b = Coord3D::new(20.0, 0.0, 1.0);
        assert!(is_clear_line_of_sight(&a, &b, &data, 3, 2, 0, 10.0));
    }
}
