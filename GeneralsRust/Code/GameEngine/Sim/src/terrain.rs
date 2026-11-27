//! Logical height samples, deformation, terrain queries, and cliff bits.
//!
//! C++ `W3DTerrainLogic::isCliffCell` → `BaseHeightMapRenderObjClass::isCliffCell`
//! → `WorldHeightMap::getCliffState`. Bits are packed 8 cells/byte and initialized
//! from the four-corner world-Z range vs `PATHFIND_CLIFF_SLOPE_LIMIT_F` (9.8).

pub use crate::terrain_boundaries::TerrainBoundaries;
pub use crate::terrain_bridge::{
    BRIDGE_MAX_TOWERS, Bridge, BridgeAttackInfo, BridgeInfo, BridgeList, BridgeTowerType, Region2D,
};
pub use crate::terrain_height_map::{TerrainHeightChange, TerrainHeightMap};
pub use crate::terrain_los::{LOS_FUDGE, is_clear_line_of_sight};
pub use crate::terrain_ray::{TerrainHeightSamples, TerrainRayCast, source_height_cell_floor};
pub use crate::terrain_water::{
    WaterGridMeshMotion, WaterGridSettings, WaterGridState, WaterSurface, select_water_surface,
    water_grid_setting_index,
};
pub use crate::terrain_waypoints::{
    CAMERA_WAYPOINT_PATH_LIMIT, CameraWaypointError, CameraWaypointPath, WAYPOINT_PATH_LIMIT,
    WaveGuide1Bind, Waypoint, WaypointGraph, WaypointID, WaypointIdentity, WaypointRoute,
};
pub use crate::water_transitions::{
    MAX_DYNAMIC_WATER_ENTRIES, WATER_GRID_NAME_CPP, WaterTransition, WaterTransitionStep,
    WaterTransitions, is_grid_water_name,
};
pub use generals_game_domain::terrain::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

/// C++ BaseHeightMap sample interpolation along the (0,0) to (1,1) diagonal.
#[inline]
pub fn triangle_height(h00: f32, h10: f32, h01: f32, h11: f32, fx: f32, fy: f32) -> f32 {
    if fy > fx {
        h01 + (1.0 - fy) * (h00 - h01) + fx * (h11 - h01)
    } else {
        h10 + fy * (h11 - h10) + (1.0 - fx) * (h00 - h10)
    }
}

/// C++ `WorldHeightMap.cpp` `PATHFIND_CLIFF_SLOPE_LIMIT_F`.
pub const PATHFIND_CLIFF_SLOPE_LIMIT_F: f32 = 9.8;

/// Packed `m_cellCliffState` for the logic height map.
#[derive(Debug, Clone, Default)]
pub struct CliffBitfield {
    bits: Vec<u8>,
    flip_state_width: i32,
    width: i32,
    height: i32,
}

impl CliffBitfield {
    pub fn new() -> Self {
        Self::default()
    }

    /// C++ `WorldHeightMap::initCliffFlagsFromHeights`.
    pub fn from_heights(map_data: &[u8], map_dx: i32, map_dy: i32) -> Self {
        let mut bits = Self {
            bits: Vec::new(),
            flip_state_width: 0,
            width: 0,
            height: 0,
        };
        bits.rebuild(map_data, map_dx, map_dy);
        bits
    }

    pub fn clear(&mut self) {
        self.bits.clear();
        self.flip_state_width = 0;
        self.width = 0;
        self.height = 0;
    }

    pub fn rebuild(&mut self, map_data: &[u8], map_dx: i32, map_dy: i32) {
        if map_dx <= 0 || map_dy <= 0 || map_data.is_empty() {
            self.clear();
            return;
        }
        // C++ `numBytesX = (m_width+7)/8`.
        let flip_state_width = (map_dx + 7) / 8;
        let len = (flip_state_width as usize).saturating_mul(map_dy.max(0) as usize);
        self.bits = vec![0u8; len];
        self.flip_state_width = flip_state_width;
        self.width = map_dx;
        self.height = map_dy;
        if map_dx < 2 || map_dy < 2 {
            return;
        }
        for y in 0..map_dy - 1 {
            for x in 0..map_dx - 1 {
                self.set_cell_from_heights(map_data, map_dx, x, y);
            }
        }
    }

    /// Recompute the four cells that share vertex `(gx, gy)`.
    pub fn refresh_vertex(&mut self, map_data: &[u8], map_dx: i32, map_dy: i32, gx: i32, gy: i32) {
        if self.bits.is_empty() {
            self.rebuild(map_data, map_dx, map_dy);
            return;
        }
        for dy in -1..=0 {
            for dx in -1..=0 {
                let cx = gx + dx;
                let cy = gy + dy;
                if cx >= 0 && cy >= 0 && cx < map_dx - 1 && cy < map_dy - 1 {
                    self.set_cell_from_heights(map_data, map_dx, cx, cy);
                }
            }
        }
    }

    /// C++ `WorldHeightMap::getCliffState`.
    pub fn get_cliff_state(&self, x_index: i32, y_index: i32) -> bool {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return false;
        }
        if self.bits.is_empty() || self.flip_state_width <= 0 {
            return false;
        }
        let byte_idx = (y_index * self.flip_state_width + (x_index >> 3)) as usize;
        if byte_idx >= self.bits.len() {
            return false;
        }
        (self.bits[byte_idx] & (1 << (x_index & 0x7))) != 0
    }

    fn set_cliff_state(&mut self, x_index: i32, y_index: i32, state: bool) {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return;
        }
        if self.bits.is_empty() || self.flip_state_width <= 0 {
            return;
        }
        let byte_idx = (y_index * self.flip_state_width + (x_index >> 3)) as usize;
        if byte_idx >= self.bits.len() {
            return;
        }
        let mask = 1 << (x_index & 0x7);
        if state {
            self.bits[byte_idx] |= mask;
        } else {
            self.bits[byte_idx] &= !mask;
        }
    }

    fn set_cell_from_heights(&mut self, map_data: &[u8], map_dx: i32, x: i32, y: i32) {
        let h00 = sample(map_data, map_dx, x, y);
        let h10 = sample(map_data, map_dx, x + 1, y);
        let h01 = sample(map_data, map_dx, x, y + 1);
        let h11 = sample(map_data, map_dx, x + 1, y + 1);
        self.set_cliff_state(x, y, is_cliff_from_raw_heights(h00, h10, h01, h11));
    }
}

fn sample(map_data: &[u8], map_dx: i32, x: i32, y: i32) -> u8 {
    if map_dx <= 0 || x < 0 || y < 0 {
        return 0;
    }
    let idx = (y * map_dx + x) as usize;
    if idx >= map_data.len() {
        return 0;
    }
    map_data[idx]
}

/// C++ `WorldHeightMap::setCellCliffFlagFromHeights` four-corner test.
/// Heights are raw samples; compared as `raw * MAP_HEIGHT_SCALE` vs 9.8 world Z.
#[inline]
pub fn is_cliff_from_raw_heights(h0: u8, h1: u8, h2: u8, h3: u8) -> bool {
    let min_z = (h0.min(h1).min(h2).min(h3) as f32) * MAP_HEIGHT_SCALE;
    let max_z = (h0.max(h1).max(h2).max(h3) as f32) * MAP_HEIGHT_SCALE;
    max_z - min_z > PATHFIND_CLIFF_SLOPE_LIMIT_F
}

/// C++ `BaseHeightMapRenderObjClass::isCliffCell` (lines 1224-1246).
pub fn is_cliff_cell(
    x: f32,
    y: f32,
    bits: &CliffBitfield,
    map_dx: i32,
    map_dy: i32,
    border_size: i32,
) -> bool {
    if map_dx < 2 || map_dy < 2 || bits.bits.is_empty() {
        return false;
    }

    // C++: `Int iX = x/MAP_XY_FACTOR;` then `iX += getBorderSizeInline()`.
    let mut ix = (x / MAP_XY_FACTOR) as i32 + border_size.max(0);
    let mut iy = (y / MAP_XY_FACTOR) as i32 + border_size.max(0);
    if ix < 0 {
        ix = 0;
    }
    if iy < 0 {
        iy = 0;
    }
    if ix >= map_dx - 1 {
        ix = map_dx - 2;
    }
    if iy >= map_dy - 1 {
        iy = map_dy - 2;
    }
    bits.get_cliff_state(ix, iy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_corner_range_uses_9_8_not_5_0() {
        // MAP_HEIGHT_SCALE = 0.625. Delta of 8 raw steps = 5.0 — not a cliff.
        // Delta of 16 raw steps = 10.0 — is a cliff.
        let mut data = vec![0u8; 2 * 2];
        data[1] = 8;
        let bits = CliffBitfield::from_heights(&data, 2, 2);
        assert!(!bits.get_cliff_state(0, 0));

        data[1] = 16;
        let bits = CliffBitfield::from_heights(&data, 2, 2);
        assert!(bits.get_cliff_state(0, 0));
    }

    #[test]
    fn is_cliff_from_raw_heights_uses_world_z_9_8() {
        // C++ PATHFIND_CLIFF_SLOPE_LIMIT_F is 9.8 world Z, not 9.8 raw samples.
        assert!(!is_cliff_from_raw_heights(0, 10, 0, 10)); // 6.25 world
        assert!(!is_cliff_from_raw_heights(0, 15, 0, 15)); // 9.375 world
        assert!(is_cliff_from_raw_heights(0, 16, 0, 16)); // 10.0 world
    }

    #[test]
    fn world_origin_adds_border() {
        let mut data = vec![0u8; 4 * 4];
        data[2 + 1 * 4] = 20;
        data[2 + 2 * 4] = 20;
        let bits = CliffBitfield::from_heights(&data, 4, 4);
        assert!(is_cliff_cell(0.0, 0.0, &bits, 4, 4, 1));
        // Off-map clamps to [0, extent-2] instead of rejecting.
        assert!(!is_cliff_cell(-1000.0, -1000.0, &bits, 4, 4, 1));
    }
}
