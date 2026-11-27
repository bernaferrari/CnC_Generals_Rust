//! Receiving world's logical height samples and derived cliff data.

use glam::Vec3;
use math_utilities::{CastResult, Vector3};

use crate::terrain::{
    CliffBitfield, MAP_HEIGHT_SCALE, MAP_XY_FACTOR, is_cliff_cell, triangle_height,
};
use crate::terrain_los::is_clear_line_of_sight;
use crate::terrain_ray::{TerrainHeightSamples, TerrainRayCast, source_height_cell_floor};

/// One accepted raw lower-write, in original playable ground-plane XY.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerrainHeightChange {
    pub playable_x: i32,
    pub playable_y: i32,
    /// Accepted canonical sample after original flattened-index bounds checking.
    pub sample_x: i32,
    pub sample_y: i32,
    pub raw_height: u8,
    /// Original Int argument before WorldHeightMap byte narrowing.
    pub requested_height: i32,
}

/// The complete bordered height grid and its per-instance cache identity.
pub struct TerrainHeightMap {
    map_data: Vec<u8>,
    height_revision: u64,
    map_dx: i32,
    map_dy: i32,
    // Authored or fully restored extrema. Raw edits retain the loaded range.
    map_min_z: f32,
    map_max_z: f32,
    border_size: i32,
    cliff_state: CliffBitfield,
}

impl Default for TerrainHeightMap {
    fn default() -> Self {
        Self {
            map_data: Vec::new(),
            height_revision: 0,
            map_dx: 0,
            map_dy: 0,
            map_min_z: 0.0,
            map_max_z: 1.0,
            border_size: 0,
            cliff_state: CliffBitfield::new(),
        }
    }
}

impl TerrainHeightMap {
    /// Resolve decoded playable or full extents to the complete sample stride.
    pub fn map_dimensions(
        width: u32,
        height: u32,
        border: i32,
        byte_count: usize,
    ) -> Result<(i32, i32), String> {
        let width = i32::try_from(width).map_err(|_| "terrain map width exceeds index domain")?;
        let height =
            i32::try_from(height).map_err(|_| "terrain map height exceeds index domain")?;
        let (width, height) = if width == 0 && height == 0 && border == 0 && byte_count == 0 {
            (0, 0)
        } else {
            let twice_border = border.checked_mul(2).ok_or("terrain map border overflow")?;
            let full = width
                .checked_add(twice_border)
                .zip(height.checked_add(twice_border));
            let full = full.filter(|&(w, h)| {
                border > 0 && Self::checked_height_sample_len(w, h, border).ok() == Some(byte_count)
            });
            if let Some(full) = full {
                full
            } else {
                let expected = Self::checked_height_sample_len(width, height, border)?;
                if expected != byte_count {
                    return Err("terrain map bytes disagree with full/sample dimensions".into());
                }
                (width, height)
            }
        };

        Ok((width, height))
    }

    fn checked_height_sample_len(width: i32, height: i32, border: i32) -> Result<usize, String> {
        if width <= 0
            || height <= 0
            || border < 0
            || width
                .checked_add(border)
                .and_then(|n| n.checked_add(border))
                .and_then(|n| n.checked_add(132))
                .is_none()
            || height
                .checked_add(border)
                .and_then(|n| n.checked_add(border))
                .and_then(|n| n.checked_add(132))
                .is_none()
        {
            return Err("invalid terrain height dimensions/border".into());
        }
        let len = (width as usize)
            .checked_mul(height as usize)
            .ok_or("terrain height sample count overflow")?;
        if len > isize::MAX as usize {
            return Err("terrain height allocation exceeds addressable slice".into());
        }
        Ok(len)
    }

    /// An admitted map is either unloaded or a complete bordered grid.
    pub fn validate(&self) -> Result<(), String> {
        if self.map_dx == 0 && self.map_dy == 0 && self.border_size == 0 && self.map_data.is_empty()
        {
            return Ok(());
        }
        let expected = Self::checked_height_sample_len(self.map_dx, self.map_dy, self.border_size)?;
        if expected != self.map_data.len() {
            return Err("terrain height sample count disagrees with dimensions".into());
        }
        Ok(())
    }

    pub fn admit(
        &mut self,
        width: u32,
        height: u32,
        border: i32,
        samples: Vec<u8>,
    ) -> Result<(), String> {
        let (width, height) = Self::map_dimensions(width, height, border, samples.len())?;
        self.replace_samples(width, height, border, samples);
        Ok(())
    }

    fn replace_samples(&mut self, width: i32, height: i32, border: i32, samples: Vec<u8>) {
        self.bump_height_revision();
        self.map_data = samples;
        self.map_dx = width;
        self.map_dy = height;
        self.border_size = border;
        if let Some((&min_height, &max_height)) =
            self.map_data.iter().min().zip(self.map_data.iter().max())
        {
            self.map_min_z = min_height as f32 * MAP_HEIGHT_SCALE;
            self.map_max_z = max_height as f32 * MAP_HEIGHT_SCALE;
        } else {
            self.map_min_z = 0.0;
            self.map_max_z = 1.0;
        }
        self.cliff_state
            .rebuild(&self.map_data, self.map_dx, self.map_dy);
    }

    pub fn clear(&mut self) {
        self.bump_height_revision();
        self.map_data.clear();
        self.map_dx = 0;
        self.map_dy = 0;
        self.map_min_z = 0.0;
        self.map_max_z = 1.0;
        self.border_size = 0;
        self.cliff_state.clear();
    }

    /// Receiving cache identity. It is never serialized.
    pub fn revision(&self) -> u64 {
        self.height_revision
    }

    pub fn border_size(&self) -> i32 {
        self.border_size
    }

    pub fn extents(&self) -> (i32, i32) {
        (self.map_dx, self.map_dy)
    }

    pub fn vertical_range(&self) -> (f32, f32) {
        (self.map_min_z, self.map_max_z)
    }

    pub fn samples(&self) -> &[u8] {
        &self.map_data
    }

    fn bump_height_revision(&mut self) {
        self.height_revision = self.height_revision.wrapping_add(1);
    }

    pub fn has_height_map(&self) -> bool {
        !self.map_data.is_empty() && self.map_dx > 0 && self.map_dy > 0
    }

    /// Apply a W3DTerrainVisual Xfer payload to the receiving grid.
    /// Returns whether there is a loaded grid to notify about the payload.
    pub fn apply_snapshot_prefix(&mut self, data: &[u8]) -> Result<bool, String> {
        self.validate()?;
        if self.map_data.is_empty() {
            if data.is_empty() {
                return Ok(false);
            }
            return Err("cannot apply height bytes without an admitted receiving grid".into());
        }
        let len = self.map_data.len().min(data.len());
        if self.map_data[..len] != data[..len] {
            self.bump_height_revision();
        }
        self.map_data[..len].copy_from_slice(&data[..len]);
        self.cliff_state
            .rebuild(&self.map_data, self.map_dx, self.map_dy);
        Ok(true)
    }

    /// Restore a complete grid with the receiving border and zero-fill any short payload.
    pub fn restore_snapshot(&mut self, width: i32, height: i32, data: &[u8]) -> Result<(), String> {
        self.validate()?;
        let expected = Self::checked_height_sample_len(width, height, self.border_size)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(expected)
            .map_err(|_| "terrain height allocation failed")?;
        replacement.resize(expected, 0);
        let n = expected.min(data.len());
        replacement[..n].copy_from_slice(&data[..n]);
        self.replace_samples(width, height, self.border_size, replacement);
        Ok(())
    }

    /// C++ BaseHeightMap.cpp ground sample and optional smoothed Z-up normal.
    pub fn get_ground_height(&self, x: f32, y: f32, normal: Option<&mut Vec3>) -> f32 {
        if self.map_data.is_empty() || self.map_dx <= 0 || self.map_dy <= 0 {
            if let Some(n) = normal {
                *n = Vec3::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        }

        // BaseHeightMap.cpp multiplies by the stored reciprocal, then uses
        // FAST_REAL_FLOOR. Negative exact integers deliberately differ from
        // mathematical floor after the source's almost-one subtraction.
        const MAP_XY_FACTOR_INV: f32 = 1.0 / MAP_XY_FACTOR;
        let map_x = x * MAP_XY_FACTOR_INV;
        let map_y = y * MAP_XY_FACTOR_INV;
        let (Ok(ixf), Ok(iyf)) = (
            source_height_cell_floor(map_x),
            source_height_cell_floor(map_y),
        ) else {
            // Original conversion is undefined outside its finite Int domain.
            // Reject it without indexing or manufacturing a fallback cell.
            if let Some(n) = normal {
                *n = Vec3::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        };
        let fx = map_x - ixf as f32;
        let fy = map_y - iyf as f32;

        // Add the authored border only after the source conversion.
        let Some((ix, iy)) = ixf
            .checked_add(self.border_size)
            .zip(iyf.checked_add(self.border_size))
        else {
            if let Some(n) = normal {
                *n = Vec3::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        };

        let x_extent = self.map_dx;
        let y_extent = self.map_dy;

        let get_height_sample = |gx: i32, gy: i32| -> f32 {
            let cx = gx.clamp(0, x_extent.saturating_sub(1).max(0));
            let cy = gy.clamp(0, y_extent.saturating_sub(1).max(0));
            let idx = (cy * x_extent + cx) as usize;
            if idx < self.map_data.len() {
                self.map_data[idx] as f32
            } else {
                0.0
            }
        };

        // C++ rejects ix/iy < 1 or > extent-3 (needs neighborhood for normals).
        if x_extent >= 3
            && y_extent >= 3
            && (ix > x_extent - 3 || iy > y_extent - 3 || iy < 1 || ix < 1)
        {
            if let Some(n) = normal {
                *n = Vec3::new(0.0, 0.0, 1.0);
            }
            return get_height_sample(ix, iy) * MAP_HEIGHT_SCALE;
        }

        let p0 = get_height_sample(ix, iy);
        let p1 = get_height_sample(ix + 1, iy);
        let p2 = get_height_sample(ix + 1, iy + 1);
        let p3 = get_height_sample(ix, iy + 1);

        let height = triangle_height(p0, p1, p3, p2, fx, fy) * MAP_HEIGHT_SCALE;

        if let Some(n) = normal {
            // C++ BaseHeightMapRenderObjClass::getHeightMapHeight (BaseHeightMap.cpp:914-970)
            // bilinearly smooths deltaZ_X/deltaZ_Y over 4 samples each (12 neighbors),
            // then builds l2r/n2f and a normalized cross product.
            let d0 = p0;
            let d1 = p1;
            let d2 = p2;
            let d3 = p3;
            let d4 = get_height_sample(ix, iy - 1);
            let d5 = get_height_sample(ix + 1, iy - 1);
            let d6 = get_height_sample(ix + 2, iy);
            let d7 = get_height_sample(ix + 2, iy + 1);
            let d8 = get_height_sample(ix + 1, iy + 2);
            let d9 = get_height_sample(ix, iy + 2);
            let _d10 = get_height_sample(ix - 1, iy + 1);
            let d11 = get_height_sample(ix - 1, iy);

            let delta_z_x0 = d1 - d11;
            let delta_z_x1 = d6 - d0;
            let delta_z_x2 = d7 - d3;
            let delta_z_x3 = d6 - d0;
            let delta_z_y0 = d3 - d4;
            let delta_z_y1 = d2 - d5;
            let delta_z_y2 = d8 - d1;
            let delta_z_y3 = d9 - d0;

            let delta_z_x_left = delta_z_x0 * (1.0 - fx) + fx * delta_z_x3;
            let delta_z_x_right = delta_z_x1 * (1.0 - fx) + fx * delta_z_x2;
            let delta_z_x = delta_z_x_left * (1.0 - fy) + fy * delta_z_x_right;
            let delta_z_y_left = delta_z_y0 * (1.0 - fx) + fx * delta_z_y3;
            let delta_z_y_right = delta_z_y1 * (1.0 - fx) + fx * delta_z_y2;
            let delta_z_y = delta_z_y_left * (1.0 - fy) + fy * delta_z_y_right;

            let l2r_x = 2.0 * MAP_XY_FACTOR / MAP_HEIGHT_SCALE;
            let n2f_y = 2.0 * MAP_XY_FACTOR / MAP_HEIGHT_SCALE;
            let mut nx = 0.0 * delta_z_y - n2f_y * delta_z_x;
            let mut ny = delta_z_x * 0.0 - l2r_x * delta_z_y;
            let mut nz = l2r_x * n2f_y - 0.0 * 0.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > f32::EPSILON {
                nx /= len;
                ny /= len;
                nz /= len;
            } else {
                nx = 0.0;
                ny = 0.0;
                nz = 1.0;
            }
            *n = Vec3::new(nx, ny, nz);
        }

        height
    }

    pub fn is_clear_line_of_sight(&self, pos1: &Vec3, pos2: &Vec3) -> bool {
        is_clear_line_of_sight(
            pos1,
            pos2,
            &self.map_data,
            self.map_dx,
            self.map_dy,
            self.border_size,
            self.map_max_z,
        )
    }

    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        is_cliff_cell(
            x,
            y,
            &self.cliff_state,
            self.map_dx,
            self.map_dy,
            self.border_size,
        )
    }

    /// Cast against current logical terrain in original XYZ space. An unloaded map misses.
    pub fn cast_terrain_ray(&self, start: Vector3, end: Vector3) -> Result<TerrainRayCast, String> {
        if self.map_dx == 0 && self.map_dy == 0 && self.map_data.is_empty() {
            return Ok(TerrainRayCast {
                hit: false,
                result: CastResult::default(),
            });
        }
        TerrainHeightSamples::new(&self.map_data, self.map_dx, self.map_dy, self.border_size)?
            .cast_ray(start, end)
    }

    /// C++ TerrainLogic::flattenTerrain rotated box, in playable XY.
    pub fn flatten_terrain_box_at_with_effects(
        &mut self,
        x: f32,
        y: f32,
        angle: f32,
        halfsize_x: f32,
        halfsize_y: f32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) -> Result<(), String> {
        if halfsize_x <= 0.0 && halfsize_y <= 0.0 {
            return Ok(());
        }
        let c = angle.cos();
        let s = angle.sin();

        let top_left_x = x - halfsize_x * c - halfsize_y * s;
        let top_left_y = y + halfsize_y * c - halfsize_x * s;
        let top_right_x = x + halfsize_x * c - halfsize_y * s;
        let top_right_y = y + halfsize_y * c + halfsize_x * s;
        let bottom_right_x = x + halfsize_x * c + halfsize_y * s;
        let bottom_right_y = y - halfsize_y * c + halfsize_x * s;
        let bottom_left_x = x - halfsize_x * c + halfsize_y * s;
        let bottom_left_y = y - halfsize_y * c - halfsize_x * s;

        let min_x = top_left_x
            .min(top_right_x)
            .min(bottom_right_x)
            .min(bottom_left_x);
        let max_x = top_left_x
            .max(top_right_x)
            .max(bottom_right_x)
            .max(bottom_left_x);
        let min_y = top_left_y
            .min(top_right_y)
            .min(bottom_right_y)
            .min(bottom_left_y);
        let max_y = top_left_y
            .max(top_right_y)
            .max(bottom_right_y)
            .max(bottom_left_y);

        let i_min_x = source_height_cell_floor((min_x / MAP_XY_FACTOR))?;
        let _i_min_y = source_height_cell_floor((min_y / MAP_XY_FACTOR))?;
        let i_max_x = source_height_cell_floor((max_x / MAP_XY_FACTOR))?;
        let i_max_y = source_height_cell_floor((max_y / MAP_XY_FACTOR))?;

        let mut total_height: f32 = 0.0;
        let mut num_samples: i32 = 0;
        for i in i_min_x..=i_max_x {
            // C++ bug: j starts at 0, not iMin.y — we match C++ exactly
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let match_tri = Self::point_in_triangle_2d(
                    top_left_x,
                    top_left_y,
                    top_right_x,
                    top_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                ) || Self::point_in_triangle_2d(
                    top_right_x,
                    top_right_y,
                    bottom_right_x,
                    bottom_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                );
                if match_tri {
                    total_height += self.get_ground_height(test_pt_x, test_pt_y, None);
                    num_samples += 1;
                }
            }
        }
        if num_samples == 0 {
            return Ok(());
        }
        let avg_height = total_height / num_samples as f32;
        let mut raw_data_height = source_height_cell_floor((0.5 + avg_height / MAP_HEIGHT_SCALE))?;

        let center_height =
            source_height_cell_floor((self.get_ground_height(x, y, None) / MAP_HEIGHT_SCALE))?;
        if raw_data_height > center_height {
            raw_data_height = center_height;
        }

        for i in i_min_x..=i_max_x {
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let match_tri = Self::point_in_triangle_2d(
                    top_left_x,
                    top_left_y,
                    top_right_x,
                    top_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                ) || Self::point_in_triangle_2d(
                    top_right_x,
                    top_right_y,
                    bottom_right_x,
                    bottom_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                );
                if match_tri {
                    self.lower_footprint_cell_with_effects(i, j, raw_data_height, effect);
                }
            }
        }

        Ok(())
    }

    /// C++ TerrainLogic::flattenTerrain circular footprint, in playable XY.
    pub fn flatten_terrain_at_with_effects(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) -> Result<(), String> {
        if radius <= 0.0 {
            return Ok(());
        }
        let radius_sqr = radius * radius;
        let i_min_x = source_height_cell_floor(((x - radius) / MAP_XY_FACTOR))?;
        let i_max_x = source_height_cell_floor(((x + radius) / MAP_XY_FACTOR))?;
        let i_max_y = source_height_cell_floor(((y + radius) / MAP_XY_FACTOR))?;
        let mut total_height: f32 = 0.0;
        let mut num_samples: i32 = 0;
        for i in i_min_x..=i_max_x {
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let dx = test_pt_x - x;
                let dy = test_pt_y - y;
                if dx * dx + dy * dy < radius_sqr {
                    total_height += self.get_ground_height(test_pt_x, test_pt_y, None);
                    num_samples += 1;
                }
            }
        }
        if num_samples == 0 {
            return Ok(());
        }
        let avg_height = total_height / num_samples as f32;
        let raw_data_height = source_height_cell_floor((0.5 + avg_height / MAP_HEIGHT_SCALE))?;
        for i in i_min_x..=i_max_x {
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let dx = test_pt_x - x;
                let dy = test_pt_y - y;
                if dx * dx + dy * dy < radius_sqr {
                    self.lower_footprint_cell_with_effects(i, j, raw_data_height, effect);
                }
            }
        }

        Ok(())
    }

    /// C++ TerrainLogic::createCraterInTerrain, in playable XY.
    pub fn create_crater(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) {
        if radius <= 0.0 {
            return;
        }

        let i_min_x = ((x - radius) / MAP_XY_FACTOR).floor() as i32;
        let _i_min_y = ((y - radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_x = ((x + radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_y = ((y + radius) / MAP_XY_FACTOR).floor() as i32;

        for i in i_min_x..=i_max_x {
            // C++ bug: j starts at 0, not iMin.y — we match C++ exactly
            for j in 0..=i_max_y {
                let delta_x = i as f32 * MAP_XY_FACTOR - x;
                let delta_y = j as f32 * MAP_XY_FACTOR - y;
                let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();

                if distance < radius {
                    let displacement_amount = radius * (1.0 - distance / radius);
                    let current_height = self.get_raw_map_height(i, j);
                    let target_height =
                        Self::crater_raw_target_height(current_height, displacement_amount);
                    self.set_raw_map_height_with_effects(i, j, target_height, effect);
                }
            }
        }
    }

    /// C++ promotes the old Int to Real and truncates after subtraction.
    fn crater_raw_target_height(current_height: i32, displacement_amount: f32) -> i32 {
        1i32.max((current_height as f32 - displacement_amount) as i32)
    }

    /// C++ W3DTerrainVisual lower-write. Notify after the grid and cliff data change.
    pub fn set_raw_map_height_with_effects(
        &mut self,
        x: i32,
        y: i32,
        height: i32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) {
        let Some(idx) = self.raw_map_height_index(x, y) else {
            return;
        };
        // Original compares Int before narrowing its accepted value to u8.
        if self.map_data[idx] as i32 > height {
            let accepted = height as u8;
            let changed = self.map_data[idx] != accepted;
            self.map_data[idx] = accepted;
            if changed {
                self.bump_height_revision();
            }
            let grid_x = (idx % self.map_dx as usize) as i32;
            let grid_y = (idx / self.map_dx as usize) as i32;
            self.cliff_state.refresh_vertex(
                &self.map_data,
                self.map_dx,
                self.map_dy,
                grid_x,
                grid_y,
            );
            effect(TerrainHeightChange {
                playable_x: x,
                playable_y: y,
                sample_x: grid_x,
                sample_y: grid_y,
                raw_height: accepted,
                requested_height: height,
            });
        }
    }

    // WorldHeightMap::getHeight/setRawHeight validate the flattened index,
    // not X and Y independently; edge aliases are intentional source behavior.
    fn raw_map_height_index(&self, x: i32, y: i32) -> Option<usize> {
        if self.map_dx <= 0 {
            return None;
        }
        let x = i64::from(x) + i64::from(self.border_size);
        let y = i64::from(y) + i64::from(self.border_size);
        let index = y.checked_mul(i64::from(self.map_dx))?.checked_add(x)?;
        usize::try_from(index)
            .ok()
            .filter(|&index| index < self.map_data.len())
    }

    fn lower_footprint_cell_with_effects(
        &mut self,
        x: i32,
        y: i32,
        height: i32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) {
        for (dx, dy) in [
            (0, 0),
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
        ] {
            if let Some((x, y)) = x.checked_add(dx).zip(y.checked_add(dy)) {
                self.set_raw_map_height_with_effects(x, y, height, effect);
            }
        }
    }

    pub fn get_raw_map_height(&self, x: i32, y: i32) -> i32 {
        self.raw_map_height_index(x, y)
            .map_or(0, |index| self.map_data[index] as i32)
    }

    fn point_in_triangle_2d(
        v0x: f32,
        v0y: f32,
        v1x: f32,
        v1y: f32,
        v2x: f32,
        v2y: f32,
        px: f32,
        py: f32,
    ) -> bool {
        let d1 = (px - v1x) * (v0y - v1y) - (v0x - v1x) * (py - v1y);
        let d2 = (px - v2x) * (v1y - v2y) - (v1x - v2x) * (py - v2y);
        let d3 = (px - v0x) * (v2y - v0y) - (v2x - v0x) * (py - v0y);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

        !(has_neg && has_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::TerrainHeightMap;

    #[test]
    fn crater_subtracts_in_float_then_truncates_like_cpp() {
        // C++ TerrainLogic.cpp:2875: MAX(1, rawHeight - displacementAmount)
        // raw 10, displacement 2.9 → 7.1 truncates to 7, not 10-2=8.
        assert_eq!(TerrainHeightMap::crater_raw_target_height(10, 2.9), 7);
        assert_eq!(TerrainHeightMap::crater_raw_target_height(10, 2.0), 8);
        assert_eq!(TerrainHeightMap::crater_raw_target_height(1, 2.9), 1);
    }
}
