//! Terrain line-of-sight.
//!
//! C++ `W3DTerrainLogic::isClearLineOfSight` forwards to
//! `BaseHeightMapRenderObjClass::isClearLineOfSight` (`DO_BRESENHAM`).

use glam::Vec3;

use crate::terrain::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

/// C++ `BaseHeightMap.cpp` `const Real LOS_FUDGE = 0.5f`.
pub const LOS_FUDGE: f32 = 0.5;

/// C++ `BaseHeightMapRenderObjClass::isClearLineOfSight` (lines 979-1109).
pub fn is_clear_line_of_sight(
    pos: &Vec3,
    pos_other: &Vec3,
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
