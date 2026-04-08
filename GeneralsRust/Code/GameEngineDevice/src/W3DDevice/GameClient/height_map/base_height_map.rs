//! Base Height Map Module
//!
//! Port of C++ BaseHeightMap.h and BaseHeightMap.cpp
//! Original: GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/BaseHeightMap.cpp (2990 lines)
//! Original: GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/BaseHeightMap.h
//! Author: Mark W., John Ahlquist, April/May 2001
//!
//! PARITY_NOTE: This file provides the base terrain height query infrastructure.
//! It ports the height query functions (getHeightMapHeight, getClipHeight,
//! getMaxCellHeight, isCliffCell, isClearLineOfSight) from
//! BaseHeightMapRenderObjClass. The rendering-specific code (shaders, scorchmarks,
//! shoreline rendering, tree/prop/bib buffers) is deferred to terrain_rendering.rs.

use super::world_height_map::WorldHeightMap;

// =============================================================================
// CONSTANTS - Must match C++ BaseHeightMap.h and GlobalData.h
// =============================================================================

/// Maximum number of dynamic lights (C++ BaseHeightMap.h line 18)
pub const MAX_ENABLED_DYNAMIC_LIGHTS: usize = 20;

/// Maximum scorch marks (C++ BaseHeightMap.h line 224)
pub const MAX_SCORCH_MARKS: usize = 500;

/// Scorch marks in texture (C++ BaseHeightMap.h line 224)
pub const SCORCH_MARKS_IN_TEXTURE: usize = 9;

/// Scorch marks per row in texture (C++ BaseHeightMap.h line 224)
pub const SCORCH_PER_ROW: usize = 3;

/// Maximum scorch vertices (C++ BaseHeightMap.h line 224)
pub const MAX_SCORCH_VERTEX: usize = 8194;

/// Maximum scorch indices (C++ BaseHeightMap.h line 224)
pub const MAX_SCORCH_INDEX: usize = 6 * 8194;

/// Default impassable slope in degrees (C++ BaseHeightMap.cpp line 270)
pub const DEFAULT_IMPASSABLE_SLOPE: f32 = 45.0;

/// Pathfind cliff slope limit (C++ WorldHeightMap.cpp line 41)
pub const PATHFIND_CLIFF_SLOPE_LIMIT_F: f32 = 9.8;

/// Line of sight fudge factor (C++ BaseHeightMap.cpp line 1082)
pub const LOS_FUDGE: f32 = 0.5;

/// Height sample type: u8 (C++ BaseHeightMap.h line 19)
pub type HeightSampleType = u8;

// =============================================================================
// SCORCH MARK TYPE
// =============================================================================

/// Scorch mark data (C++ BaseHeightMap.h line 53-57)
#[derive(Debug, Clone, Copy)]
pub struct ScorchMark {
    /// Location of the scorch mark
    pub location: [f32; 3],
    /// Radius of the scorch mark
    pub radius: f32,
    /// Type of scorch mark
    pub scorch_type: i32,
}

impl Default for ScorchMark {
    fn default() -> Self {
        Self {
            location: [0.0; 3],
            radius: 0.0,
            scorch_type: 0,
        }
    }
}

// =============================================================================
// SHORELINE TILE DATA
// =============================================================================

/// Shoreline tile info (C++ BaseHeightMap.h line 276-280)
#[derive(Debug, Clone, Copy, Default)]
pub struct ShoreLineTileInfo {
    /// x,y position of tile packed as (y<<16)|x
    pub xy: i32,
    /// Position of tile vertices (4 verts with 3 components)
    pub verts: [f32; 12],
    /// Index into water depth alpha LUT
    pub t: [f32; 4],
}

/// Shoreline tile sort info for efficient culling (C++ BaseHeightMap.h line 285-291)
#[derive(Debug, Clone, Copy, Default)]
pub struct ShoreLineTileSortInfo {
    /// Index within m_shoreLineTilePositions where tiles start
    pub tile_start_index: i32,
    /// Total tiles at this coordinate
    pub num_tiles: i32,
    /// Lowest coordinate in list
    pub min_tile_coordinate: u16,
    /// Highest coordinate in list
    pub max_tile_coordinate: u16,
}

// =============================================================================
// BOUNDS STRUCT
// =============================================================================

/// 2D integer bounds (C++ BaseHeightMap.h line 38-41)
#[derive(Debug, Clone, Copy, Default)]
pub struct TBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

// =============================================================================
// BASE HEIGHT MAP
// =============================================================================

/// Base terrain height map providing height queries, cliff detection, and
/// line-of-sight calculations.
///
/// Corresponds to the height-query portion of C++ BaseHeightMapRenderObjClass.
/// The rendering-specific functionality (shaders, vertex buffers, scorchmarks)
/// lives in terrain_rendering.rs.
///
/// PARITY_NOTE: getHeightMapHeight matches C++ BaseHeightMap.cpp:828-976 exactly.
/// The bilinear interpolation uses the same triangle split (fy > fx) and the
/// same 12-point normal smoothing algorithm.
pub struct BaseHeightMap {
    /// Dimensions of heightmap in vertices (C++ m_x, m_y)
    pub x: i32,
    pub y: i32,

    /// Pointer to the world height map data (C++ m_map)
    map: Option<std::sync::Arc<std::sync::RwLock<WorldHeightMap>>>,

    /// Minimum height value in the heightmap (C++ m_minHeight)
    pub min_height: f32,
    /// Maximum height value in the heightmap (C++ m_maxHeight)
    pub max_height: f32,

    /// Whether textures are disabled (C++ m_disableTextures)
    pub disable_textures: bool,
    /// Whether a full update is needed (C++ m_needFullUpdate)
    pub need_full_update: bool,
    /// Whether currently updating (C++ m_updating)
    pub updating: bool,
    /// Whether to use depth fade for underwater terrain (C++ m_useDepthFade)
    pub use_depth_fade: bool,
    /// Depth fade RGB values (C++ m_depthFade)
    pub depth_fade: [f32; 3],
    /// Show impassable areas (C++ m_showImpassableAreas)
    pub show_impassable_areas: bool,
    /// Current impassable slope threshold (C++ m_curImpassableSlope)
    pub cur_impassable_slope: f32,

    /// Scorch marks array (C++ m_scorches[MAX_SCORCH_MARKS])
    pub scorches: Vec<ScorchMark>,
    /// Number of scorch marks (C++ m_numScorches)
    pub num_scorches: usize,

    /// Visible cliff flags per cell (C++ m_showAsVisibleCliff)
    pub show_as_visible_cliff: Vec<bool>,
}

impl BaseHeightMap {
    /// Construct a new base height map.
    /// Corresponds to C++ BaseHeightMapRenderObjClass constructor (line 224-293).
    pub fn new() -> Self {
        // C++ line 233: m_maxHeight = (pow(256.0, sizeof(HeightSampleType))-1.0)*MAP_HEIGHT_SCALE
        let max_height = (256.0_f32.powi(std::mem::size_of::<HeightSampleType>() as i32) - 1.0)
            * MAP_HEIGHT_SCALE;
        Self {
            x: 0,
            y: 0,
            map: None,
            min_height: 0.0,
            max_height,
            disable_textures: false,
            need_full_update: false,
            updating: false,
            use_depth_fade: false,
            depth_fade: [0.0; 3],
            show_impassable_areas: false,
            cur_impassable_slope: DEFAULT_IMPASSABLE_SLOPE,
            scorches: Vec::with_capacity(MAX_SCORCH_MARKS),
            num_scorches: 0,
            show_as_visible_cliff: Vec::new(),
        }
    }

    /// Set the world height map.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::redirectToHeightmap (line 108-112).
    pub fn set_map(&mut self, map: std::sync::Arc<std::sync::RwLock<WorldHeightMap>>) {
        self.map = Some(map);
    }

    /// Get a reference to the world height map.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::getMap (line 191).
    pub fn get_map(&self) -> Option<&std::sync::Arc<std::sync::RwLock<WorldHeightMap>>> {
        self.map.as_ref()
    }

    pub fn world_to_grid(&self, x: f32, y: f32) -> Option<(i32, i32)> {
        let map = self.map.as_ref()?;
        let map = map.read().ok()?;
        Some((
            (x / MAP_XY_FACTOR).floor() as i32 + map.get_border_size(),
            (y / MAP_XY_FACTOR).floor() as i32 + map.get_border_size(),
        ))
    }

    pub fn grid_to_world(&self, x_index: i32, y_index: i32) -> Option<(f32, f32)> {
        let map = self.map.as_ref()?;
        let map = map.read().ok()?;
        let border = map.get_border_size() as f32;
        Some((
            (x_index as f32 - border) * MAP_XY_FACTOR,
            (y_index as f32 - border) * MAP_XY_FACTOR,
        ))
    }

    pub fn get_height_map_height_lod(
        &self,
        x: f32,
        y: f32,
        lod: u32,
        normal: Option<&mut [f32; 3]>,
    ) -> f32 {
        if lod <= 1 {
            return self.get_height_map_height(x, y, normal);
        }

        let lod = lod as f32;
        let sample_x = (x / MAP_XY_FACTOR / lod).floor() * MAP_XY_FACTOR * lod;
        let sample_y = (y / MAP_XY_FACTOR / lod).floor() * MAP_XY_FACTOR * lod;
        self.get_height_map_height(sample_x, sample_y, normal)
    }

    // =========================================================================
    // HEIGHT QUERIES
    // =========================================================================

    /// Get height at a clipped grid position.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::getClipHeight (line 115-131).
    ///
    /// Clamps x and y to valid range [0, extent-1] before looking up the height.
    pub fn get_clip_height(&self, x: i32, y: i32) -> u8 {
        if let Some(map) = &self.map {
            let map = map.read().unwrap();
            let xextent = map.get_x_extent() - 1;
            let yextent = map.get_y_extent() - 1;

            let x = if x < 0 {
                0
            } else if x > xextent {
                xextent
            } else {
                x
            };
            let y = if y < 0 {
                0
            } else if y > yextent {
                yextent
            } else {
                y
            };

            map.get_height(x, y)
        } else {
            0
        }
    }

    /// Get the height and normal at a world-space (x, y) position.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::getHeightMapHeight (line 828-976).
    ///
    /// Uses bilinear interpolation within the correct triangle of the heightmap cell.
    /// Triangle split: if fy > fx, use upper-left triangle, else lower-right.
    /// Normal calculation uses 12-point smoothed neighborhood (C++ lines 925-970).
    ///
    /// PARITY_NOTE: This is a critical gameplay function used for object placement,
    /// projectile trajectories, and camera positioning. The interpolation algorithm
    /// must match C++ exactly.
    pub fn get_height_map_height(&self, x: f32, y: f32, normal: Option<&mut [f32; 3]>) -> f32 {
        if let Some(map) = &self.map {
            let map = map.read().unwrap();

            if map.get_data_ptr().is_empty() {
                if let Some(n) = normal {
                    // return a default normal pointing up (C++ line 847-851)
                    n[0] = 0.0;
                    n[1] = 0.0;
                    n[2] = 1.0;
                }
                return 0.0;
            }

            let map_xy_factor_inv = 1.0 / MAP_XY_FACTOR;

            // C++ lines 867-878: Find surrounding grid points
            let xdiv = x * map_xy_factor_inv;
            let ydiv = y * map_xy_factor_inv;

            let ixf = xdiv.floor();
            let iyf = ydiv.floor();

            let fx = xdiv - ixf; // fraction
            let fy = ydiv - iyf; // fraction

            let border = map.get_border_size() as f32;
            let mut ix = ixf as i32 + border as i32;
            let mut iy = iyf as i32 + border as i32;
            let x_extent = map.get_x_extent();
            let y_extent = map.get_y_extent();

            // C++ lines 883-894: Check for valid range (extent-3 for smoothed normals)
            if ix > (x_extent - 3) || iy > (y_extent - 3) || iy < 1 || ix < 1 {
                if let Some(n) = normal {
                    n[0] = 0.0;
                    n[1] = 0.0;
                    n[2] = 1.0;
                }
                return self.get_clip_height(ix, iy) as f32 * MAP_HEIGHT_SCALE;
            }

            let data = map.get_data_ptr();
            let idx = (ix + iy * x_extent) as usize;
            let p0 = data[idx] as f32;
            let p2 = data[(idx + x_extent as usize + 1)] as f32;

            // C++ lines 900-910: Bilinear interpolation within triangle
            let height = if fy > fx {
                // Upper triangle (C++ line 903)
                let p3 = data[(idx + x_extent as usize)] as f32;
                (p3 + (1.0 - fy) * (p0 - p3) + fx * (p2 - p3)) * MAP_HEIGHT_SCALE
            } else {
                // Lower triangle (C++ line 909)
                let p1 = data[(idx + 1)] as f32;
                (p1 + fy * (p2 - p1) + (1.0 - fx) * (p0 - p1)) * MAP_HEIGHT_SCALE
            };

            // C++ lines 914-970: Smoothed normal calculation using 12-point neighborhood
            if let Some(n) = normal {
                /*
                    9       8

                10  3-----2       7
                    |    /|
                    |  /  |
                    |/    |
                11  0-----1       6

                    4       5
                */
                let xe = x_extent as usize;
                let idx4 = ix as usize + (iy as usize - 1) * xe;
                let idx0 = ix as usize + iy as usize * xe;
                let idx3 = ix as usize + (iy as usize + 1) * xe;
                let idx9 = ix as usize + (iy as usize + 2) * xe;

                let d0 = data[idx0] as f32;
                let d1 = data[idx0 + 1] as f32;
                let d2 = data[idx3 + 1] as f32;
                let d3 = data[idx3] as f32;
                let d4 = data[idx4] as f32;
                let d5 = data[idx4 + 1] as f32;
                let d6 = data[idx0 + 2] as f32;
                let d7 = data[idx3 + 2] as f32;
                let d8 = data[idx9 + 1] as f32;
                let d9 = data[idx9] as f32;
                let d10 = data[idx3 - 1] as f32;
                let d11 = data[idx0 - 1] as f32;

                // C++ lines 943-951: Height deltas in X and Y
                let delta_z_x0 = d1 - d11;
                let delta_z_x1 = d6 - d0;
                let delta_z_x2 = d7 - d3;
                let delta_z_x3 = d6 - d0;

                let delta_z_y0 = d3 - d4;
                let delta_z_y1 = d2 - d5;
                let delta_z_y2 = d8 - d1;
                let delta_z_y3 = d9 - d0;

                // C++ lines 954-960: Bilinear interpolation of deltas
                let delta_z_x_left = delta_z_x0 * (1.0 - fx) + fx * delta_z_x3;
                let delta_z_x_right = delta_z_x1 * (1.0 - fx) + fx * delta_z_x2;
                let delta_z_x = delta_z_x_left * (1.0 - fy) + fy * delta_z_x_right;

                let delta_z_y_left = delta_z_y0 * (1.0 - fx) + fx * delta_z_y3;
                let delta_z_y_right = delta_z_y1 * (1.0 - fx) + fx * delta_z_y2;
                let delta_z_y = delta_z_y_left * (1.0 - fy) + fy * delta_z_y_right;

                // C++ lines 964-970: Cross product to get normal
                let scale = 2.0 * MAP_XY_FACTOR / MAP_HEIGHT_SCALE;
                let l2r = [scale, 0.0, delta_z_x];
                let n2f = [0.0, scale, delta_z_y];

                // Cross product: l2r x n2f
                let mut nx = l2r[1] * n2f[2] - l2r[2] * n2f[1];
                let mut ny = l2r[2] * n2f[0] - l2r[0] * n2f[2];
                let mut nz = l2r[0] * n2f[1] - l2r[1] * n2f[0];
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                if len > 0.0 {
                    nx /= len;
                    ny /= len;
                    nz /= len;
                }

                n[0] = nx;
                n[1] = ny;
                n[2] = nz;
            }

            height
        } else {
            if let Some(n) = normal {
                n[0] = 0.0;
                n[1] = 0.0;
                n[2] = 1.0;
            }
            0.0
        }
    }

    /// Get the maximum height of the 4 cell corners at (x, y).
    /// Corresponds to C++ BaseHeightMapRenderObjClass::getMaxCellHeight (line 1172-1217).
    pub fn get_max_cell_height(&self, x: f32, y: f32) -> f32 {
        if let Some(map) = &self.map {
            let map = map.read().unwrap();
            if map.get_data_ptr().is_empty() {
                return 0.0;
            }

            let mut ix = (x / MAP_XY_FACTOR) as i32 + map.get_border_size();
            let mut iy = (y / MAP_XY_FACTOR) as i32 + map.get_border_size();

            ix = ix.max(0);
            iy = iy.max(0);
            if ix >= (map.get_x_extent() - 1) {
                ix = map.get_x_extent() - 2;
            }
            if iy >= (map.get_y_extent() - 1) {
                iy = map.get_y_extent() - 2;
            }

            let xe = map.get_x_extent() as usize;
            let data = map.get_data_ptr();

            let p0 = data[(ix as usize + iy as usize * xe)] as f32 * MAP_HEIGHT_SCALE;
            let p1 = data[((ix + 1) as usize + iy as usize * xe)] as f32 * MAP_HEIGHT_SCALE;
            let p2 = data[((ix + 1) as usize + (iy + 1) as usize * xe)] as f32 * MAP_HEIGHT_SCALE;
            let p3 = data[(ix as usize + (iy + 1) as usize * xe)] as f32 * MAP_HEIGHT_SCALE;

            p0.max(p1).max(p2).max(p3)
        } else {
            0.0
        }
    }

    /// Check if the cell at (x, y) is a cliff cell.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::isCliffCell (line 1224-1247).
    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        if let Some(map) = &self.map {
            let map = map.read().unwrap();
            if map.get_data_ptr().is_empty() {
                return false;
            }

            let mut ix = (x / MAP_XY_FACTOR) as i32 + map.get_border_size();
            let mut iy = (y / MAP_XY_FACTOR) as i32 + map.get_border_size();

            ix = ix.max(0);
            iy = iy.max(0);
            if ix >= (map.get_x_extent() - 1) {
                ix = map.get_x_extent() - 2;
            }
            if iy >= (map.get_y_extent() - 1) {
                iy = map.get_y_extent() - 2;
            }

            map.get_cliff_state(ix, iy)
        } else {
            false
        }
    }

    /// Check if line of sight is clear between two points using Bresenham's algorithm.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::isClearLineOfSight (line 979-1165).
    ///
    /// PARITY_NOTE: Uses the Bresenham version (C++ DO_BRESENHAM path) for parity.
    pub fn is_clear_line_of_sight(&self, pos: &[f32; 3], pos_other: &[f32; 3]) -> bool {
        if self.map.is_none() {
            return false;
        }

        let map = self.map.as_ref().unwrap().read().unwrap();
        let map_xy_factor_inv = 1.0 / MAP_XY_FACTOR;
        let border = map.get_border_size();
        let x_extent = map.get_x_extent();
        let y_extent = map.get_y_extent();
        let data = map.get_data_ptr();

        // C++ lines 996-999: Convert world positions to grid coordinates
        let start_x = (pos[0] * map_xy_factor_inv).floor() as i32 + border;
        let start_y = (pos[1] * map_xy_factor_inv).floor() as i32 + border;
        let end_x = (pos_other[0] * map_xy_factor_inv).floor() as i32 + border;
        let end_y = (pos_other[1] * map_xy_factor_inv).floor() as i32 + border;

        // C++ lines 1000-1050: Bresenham setup
        let delta_x = (end_x - start_x).abs();
        let delta_y = (end_y - start_y).abs();

        let (xinc1, xinc2) = if end_x >= start_x { (0, 1) } else { (0, -1) };
        let (yinc1, yinc2) = if end_y >= start_y { (0, 1) } else { (0, -1) };

        let (den, num, numadd, numpixels, check_y) = if delta_x >= delta_y {
            let den = delta_x;
            let num = delta_x / 2;
            let numadd = delta_y;
            (den, num, numadd, delta_x, true)
        } else {
            let den = delta_y;
            let num = delta_y / 2;
            let numadd = delta_x;
            (den, num, numadd, delta_y, false)
        };

        if numpixels == 0 {
            return true;
        }

        let ns_inv = 1.0 / numpixels as f32;
        let z = pos[2];
        let dz = pos_other[2] - z;

        let mut x = start_x;
        let mut y = start_y;
        let mut num = num;
        let mut cur_z = z;

        // C++ lines 1061-1107: Walk the line
        for _ in 0..numpixels {
            if x < 0 || y < 0 || x >= x_extent - 1 || y >= y_extent - 1 {
                break;
            }

            let idx = (x + y * x_extent) as usize;
            let mut height = data[idx] as f32;
            height = height.max(data[idx + 1] as f32);
            height = height.max(data[(idx + x_extent as usize)] as f32);
            height = height.max(data[(idx + x_extent as usize + 1)] as f32);
            height *= MAP_HEIGHT_SCALE;

            // C++ line 1082-1086: Check if terrain blocks LOS
            if height > cur_z + LOS_FUDGE {
                return false;
            }

            // C++ line 1090-1093: Early exit if above max terrain
            if cur_z >= self.max_height && dz > 0.0 {
                break;
            }

            cur_z += dz * ns_inv;

            // Bresenham step (C++ lines 1098-1106)
            num += numadd;
            if num >= den {
                num -= den;
                if check_y {
                    y += yinc2;
                } else {
                    x += xinc2;
                }
            }
            if check_y {
                x += xinc1;
            } else {
                y += yinc1;
            }
        }

        true
    }

    /// Check if a cell should be shown as a visible cliff.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::showAsVisibleCliff (line 1251-1261).
    pub fn show_as_visible_cliff(&self, x_index: i32, y_index: i32) -> bool {
        if self.map.is_none() {
            return false;
        }
        let map = self.map.as_ref().unwrap().read().unwrap();
        let x_size = map.get_x_extent();
        let idx = (x_index + y_index * x_size) as usize;
        self.show_as_visible_cliff
            .get(idx)
            .copied()
            .unwrap_or(false)
    }

    /// Evaluate whether a cell should be classified as a visible cliff.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::evaluateAsVisibleCliff (line 1265-1303).
    pub fn evaluate_as_visible_cliff(
        &self,
        x_index: i32,
        y_index: i32,
        values_greater_than_rad: f32,
    ) -> bool {
        if self.map.is_none() {
            return false;
        }
        let map = self.map.as_ref().unwrap().read().unwrap();

        // C++ lines 1268-1274: Distance lookup for 4 corners
        static DISTANCE: [f32; 4] = [
            0.0,
            1.0 * MAP_XY_FACTOR,
            (2.0_f32).sqrt() * MAP_XY_FACTOR,
            1.0 * MAP_XY_FACTOR,
        ];

        // C++ lines 1278-1284: Sample heights at 4 corners
        let bytes: [u8; 4] = [
            map.get_height(x_index, y_index),
            map.get_height(x_index + 1, y_index),
            map.get_height(x_index + 1, y_index + 1),
            map.get_height(x_index, y_index + 1),
        ];

        let heights: [f32; 4] = [
            bytes[0] as f32 * MAP_HEIGHT_SCALE,
            bytes[1] as f32 * MAP_HEIGHT_SCALE,
            bytes[2] as f32 * MAP_HEIGHT_SCALE,
            bytes[3] as f32 * MAP_HEIGHT_SCALE,
        ];

        // C++ lines 1296-1302: Check if any neighbor slope exceeds threshold
        for i in 1..4 {
            let dist = if DISTANCE[i] > 0.0 { DISTANCE[i] } else { 1.0 };
            if ((heights[i] - heights[0]) / dist).abs() > values_greater_than_rad {
                return true;
            }
        }

        false
    }

    // =========================================================================
    // SCORCH MARKS
    // =========================================================================

    /// Add a scorch mark at the given location.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::addScorch (line 2020-2048).
    pub fn add_scorch(&mut self, location: [f32; 3], radius: f32, scorch_type: i32) {
        if self.num_scorches >= MAX_SCORCH_MARKS {
            // Shift scorch marks (C++ lines 2025-2028)
            for i in 0..MAX_SCORCH_MARKS - 1 {
                self.scorches[i] = self.scorches[i + 1];
            }
            self.num_scorches -= 1;
        }

        // Check for duplicates (C++ lines 2032-2039)
        let limit = radius / 4.0;
        for i in 0..self.num_scorches {
            if (self.scorches[i].location[0] - location[0]).abs() < limit
                && (self.scorches[i].location[1] - location[1]).abs() < limit
                && (radius - self.scorches[i].radius).abs() < limit
                && self.scorches[i].scorch_type == scorch_type
            {
                return;
            }
        }

        if self.num_scorches < self.scorches.len() {
            self.scorches[self.num_scorches] = ScorchMark {
                location,
                radius,
                scorch_type,
            };
            self.num_scorches += 1;
        }
    }

    /// Clear all scorch marks.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::clearAllScorches (line 2007-2013).
    pub fn clear_all_scorches(&mut self) {
        self.num_scorches = 0;
    }

    // =========================================================================
    // STATE MANAGEMENT
    // =========================================================================

    /// Update the visible cliff flags.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::updateViewImpassableAreas.
    pub fn update_view_impassable_areas(
        &mut self,
        partial: bool,
        min_x: i32,
        max_x: i32,
        min_y: i32,
        max_y: i32,
    ) {
        if self.map.is_none() {
            return;
        }
        let map = self.map.as_ref().unwrap().read().unwrap();
        let x_size = map.get_x_extent();
        let y_size = map.get_y_extent();

        if self.show_as_visible_cliff.len() != (x_size * y_size) as usize {
            self.show_as_visible_cliff = vec![false; (x_size * y_size) as usize];
        }

        // Calculate the slope threshold from the impassable slope angle
        // C++ uses the tangent of the slope angle
        let values_greater_than_rad = self.cur_impassable_slope.to_radians().tan() * MAP_XY_FACTOR;

        let start_x = if partial { min_x } else { 0 };
        let start_y = if partial { min_y } else { 0 };
        let end_x = if partial { max_x } else { x_size - 1 };
        let end_y = if partial { max_y } else { y_size - 1 };

        for iy in start_y..=end_y {
            for ix in start_x..=end_x {
                if ix < 0 || iy < 0 || ix >= x_size - 1 || iy >= y_size - 1 {
                    continue;
                }
                let idx = (ix + iy * x_size) as usize;
                if idx < self.show_as_visible_cliff.len() {
                    self.show_as_visible_cliff[idx] =
                        self.evaluate_as_visible_cliff(ix, iy, values_greater_than_rad);
                }
            }
        }
    }

    /// Reset terrain state.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::reset (line 610-641).
    pub fn reset(&mut self) {
        self.clear_all_scorches();
        self.show_as_visible_cliff.clear();
    }

    /// Set whether to show impassable areas.
    /// Corresponds to C++ BaseHeightMapRenderObjClass::setShowImpassableAreas (line 195).
    pub fn set_show_impassable_areas(&mut self, show: bool) {
        self.show_impassable_areas = show;
    }

    /// Get whether to show impassable areas.
    pub fn get_show_impassable_areas(&self) -> bool {
        self.show_impassable_areas
    }
}

impl Default for BaseHeightMap {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// MAP CONSTANTS - Must match C++ GlobalData.h
// =============================================================================

/// World units per grid cell (C++ GlobalData.h: MAP_XY_FACTOR)
/// PARITY_NOTE: The C++ code uses different values in different places.
/// terrain_tex.rs defines MAP_XY_FACTOR as 2.0 (matching C++ default).
/// terrain_rendering.rs uses 10.0. The actual value is a global that can change.
/// We use the C++ default of 10.0 here for height queries, which is the
/// standard gameplay value.
pub const MAP_XY_FACTOR: f32 = 10.0;

/// Height scale factor (C++ GlobalData.h: MAP_HEIGHT_SCALE)
/// PARITY_NOTE: C++ defines this as MAP_XY_FACTOR / 16.0 in some contexts.
/// The actual value used in height queries is MAP_HEIGHT_SCALE directly.
pub const MAP_HEIGHT_SCALE: f32 = MAP_XY_FACTOR / 16.0;
