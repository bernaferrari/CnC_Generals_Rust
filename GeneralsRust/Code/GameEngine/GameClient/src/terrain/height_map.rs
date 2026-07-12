//! # Height Map System
//!
//! Handles terrain height data loading, processing, and querying.
//! Supports multiple formats including .hmp, .tga, and .raw files.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use gamelogic::common::types::MAP_HEIGHT_SCALE;
use glam::Vec3;
use image::{DynamicImage, ImageBuffer, Luma};

use super::textures::TileData;
use super::utils::calculate_normal;
use super::{TerrainError, TerrainResult};

pub const K_MIN_HEIGHT: u8 = 0;
pub const K_MAX_HEIGHT: u8 = 255;
pub const NUM_SOURCE_TILES: usize = 1024;
pub const NUM_BLEND_TILES: usize = 16192;

const K_HORIZ: usize = 0;
const K_VERT: usize = 1;
const K_LDIAG: usize = 2;
const K_RDIAG: usize = 3;
const K_LLDIAG: usize = 4;
const K_LRDIAG: usize = 5;
const K_DIR_MOD: u8 = 0x05;
const K_INV: usize = 6;
const NUM_ALPHA_TILES: usize = 12;

/// Height map data structure
#[derive(Debug, Clone)]
pub struct HeightMap {
    /// Width of the heightmap in samples
    pub width: u32,

    /// Height of the heightmap in samples
    pub height: u32,

    /// Height data as normalized floats (0.0 to 1.0)
    pub heights: Vec<f32>,

    /// Maximum height value in world units
    pub max_height: f32,

    /// Scale factor for converting heightmap coordinates to world coordinates
    pub scale: f32,

    /// Minimum height value
    pub min_height: f32,

    /// Height range (max - min)
    pub height_range: f32,

    pub border_size: i32,

    pub tile_ndxes: Vec<i16>,
    pub blend_tile_ndxes: Vec<i16>,
    pub draw_origin_x: i32,
    pub draw_origin_y: i32,
    pub draw_width: i32,
    pub draw_height: i32,
}

impl HeightMap {
    /// Create a new heightmap
    pub fn new(width: u32, height: u32, max_height: f32, scale: f32) -> Self {
        let sample_count = (width * height) as usize;
        Self {
            width,
            height,
            heights: vec![0.0; sample_count],
            max_height,
            scale,
            min_height: 0.0,
            height_range: max_height,
            border_size: 0,
            tile_ndxes: vec![0i16; sample_count],
            blend_tile_ndxes: vec![0i16; sample_count],
            draw_origin_x: 0,
            draw_origin_y: 0,
            draw_width: width as i32,
            draw_height: height as i32,
        }
    }

    /// Load heightmap from .hmp file (Generals format)
    pub fn load_hmp(path: &str) -> TerrainResult<Self> {
        log::info!("Loading .hmp heightmap: {}", path);

        let file = File::open(path).map_err(|e| {
            TerrainError::HeightmapError(format!("Failed to open .hmp file: {}", e))
        })?;

        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).map_err(|e| {
            TerrainError::HeightmapError(format!("Failed to read .hmp file: {}", e))
        })?;

        // Parse HMP header
        if buffer.len() < 8 {
            return Err(TerrainError::HeightmapError(
                "Invalid .hmp file: too small".to_string(),
            ));
        }

        let width = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        let height = u32::from_le_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]);

        let expected_size = 8 + (width * height * 2) as usize; // 16-bit heights
        if buffer.len() != expected_size {
            return Err(TerrainError::HeightmapError(format!(
                "Invalid .hmp file size: expected {}, got {}",
                expected_size,
                buffer.len()
            )));
        }

        // Parse height data (16-bit unsigned integers)
        let mut heights = Vec::with_capacity((width * height) as usize);
        let mut min_height = f32::MAX;
        let mut max_height = f32::MIN;

        for i in 0..(width * height) as usize {
            let offset = 8 + i * 2;
            let height_value = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]) as f32;

            min_height = min_height.min(height_value);
            max_height = max_height.max(height_value);
            heights.push(height_value);
        }

        // Normalize heights to 0.0 - 1.0 range
        let height_range = max_height - min_height;
        if height_range > 0.0 {
            for height in &mut heights {
                *height = (*height - min_height) / height_range;
            }
        }

        let world_min_height = min_height * MAP_HEIGHT_SCALE;
        let world_height_range = height_range * MAP_HEIGHT_SCALE;
        let world_max_height = max_height * MAP_HEIGHT_SCALE;
        let sample_count = (width * height) as usize;

        Ok(Self {
            width,
            height,
            heights,
            max_height: world_max_height,
            scale: 1.0,
            min_height: world_min_height,
            height_range: world_height_range,
            border_size: 0,
            tile_ndxes: vec![0i16; sample_count],
            blend_tile_ndxes: vec![0i16; sample_count],
            draw_origin_x: 0,
            draw_origin_y: 0,
            draw_width: width as i32,
            draw_height: height as i32,
        })
    }

    /// Load heightmap from .tga file
    pub fn load_tga(path: &str) -> TerrainResult<Self> {
        log::info!("Loading .tga heightmap: {}", path);

        let img = image::open(path).map_err(|e| {
            TerrainError::HeightmapError(format!("Failed to load .tga file: {}", e))
        })?;

        let gray_img = img.to_luma8();
        let (width, height) = gray_img.dimensions();

        let mut heights = Vec::with_capacity((width * height) as usize);

        // C++ terrain uses raw 8-bit samples scaled by MAP_HEIGHT_SCALE.
        let mut min_sample = f32::MAX;
        let mut max_sample = f32::MIN;

        // Convert 8-bit grayscale values to normalized heights
        for pixel in gray_img.pixels() {
            let sample = pixel[0] as f32;
            min_sample = min_sample.min(sample);
            max_sample = max_sample.max(sample);
            heights.push(sample / 255.0);
        }

        let world_min_height = min_sample * MAP_HEIGHT_SCALE;
        let world_height_range = (max_sample - min_sample) * MAP_HEIGHT_SCALE;
        let world_max_height = max_sample * MAP_HEIGHT_SCALE;
        let sample_count = (width * height) as usize;

        Ok(Self {
            width,
            height,
            heights,
            max_height: world_max_height,
            scale: 1.0,
            min_height: world_min_height,
            height_range: world_height_range,
            border_size: 0,
            tile_ndxes: vec![0i16; sample_count],
            blend_tile_ndxes: vec![0i16; sample_count],
            draw_origin_x: 0,
            draw_origin_y: 0,
            draw_width: width as i32,
            draw_height: height as i32,
        })
    }

    /// Load heightmap from .raw file (16-bit unsigned)
    pub fn load_raw(path: &str) -> TerrainResult<Self> {
        log::info!("Loading .raw heightmap: {}", path);

        let file = File::open(path).map_err(|e| {
            TerrainError::HeightmapError(format!("Failed to open .raw file: {}", e))
        })?;

        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).map_err(|e| {
            TerrainError::HeightmapError(format!("Failed to read .raw file: {}", e))
        })?;

        // Assume square heightmap
        let total_samples = buffer.len() / 2; // 16-bit samples
        let dimension = (total_samples as f32).sqrt() as u32;

        if dimension * dimension != total_samples as u32 {
            return Err(TerrainError::HeightmapError(
                "Raw heightmap must be square".to_string(),
            ));
        }

        let mut heights = Vec::with_capacity(total_samples);
        let mut min_height = f32::MAX;
        let mut max_height = f32::MIN;

        // Parse 16-bit height data
        for i in 0..total_samples {
            let offset = i * 2;
            let height_value = u16::from_le_bytes([buffer[offset], buffer[offset + 1]]) as f32;

            min_height = min_height.min(height_value);
            max_height = max_height.max(height_value);
            heights.push(height_value);
        }

        // Normalize heights
        let height_range = max_height - min_height;
        if height_range > 0.0 {
            for height in &mut heights {
                *height = (*height - min_height) / height_range;
            }
        }

        let world_min_height = min_height * MAP_HEIGHT_SCALE;
        let world_height_range = height_range * MAP_HEIGHT_SCALE;
        let world_max_height = max_height * MAP_HEIGHT_SCALE;
        let sample_count = (dimension * dimension) as usize;

        Ok(Self {
            width: dimension,
            height: dimension,
            heights,
            max_height: world_max_height,
            scale: 1.0,
            min_height: world_min_height,
            height_range: world_height_range,
            border_size: 0,
            tile_ndxes: vec![0i16; sample_count],
            blend_tile_ndxes: vec![0i16; sample_count],
            draw_origin_x: 0,
            draw_origin_y: 0,
            draw_width: dimension as i32,
            draw_height: dimension as i32,
        })
    }

    /// Get height at world coordinates using the C++ height-map triangle split.
    pub fn get_height_at(&self, world_x: f32, world_y: f32) -> f32 {
        if self.width == 0 || self.height == 0 || self.scale.abs() <= f32::EPSILON {
            return 0.0;
        }

        // Convert world coordinates to heightmap coordinates
        let mut hm_x = world_x / self.scale;
        let mut hm_y = world_y / self.scale;

        // Clamp to heightmap bounds
        let max_x = self.width.saturating_sub(1) as f32;
        let max_y = self.height.saturating_sub(1) as f32;
        if hm_x < 0.0 || hm_y < 0.0 || hm_x > max_x || hm_y > max_y {
            return 0.0;
        }
        hm_x = hm_x.clamp(0.0, max_x);
        hm_y = hm_y.clamp(0.0, max_y);

        // Get integer coordinates and fractional parts
        let x0 = hm_x.floor() as u32;
        let y0 = hm_y.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);

        let fx = hm_x - x0 as f32;
        let fy = hm_y - y0 as f32;

        // Sample four corner heights
        let h00 = self.get_height_at_index(x0, y0);
        let h10 = self.get_height_at_index(x1, y0);
        let h01 = self.get_height_at_index(x0, y1);
        let h11 = self.get_height_at_index(x1, y1);

        // C++ samples the actual triangle plane in the cell, split from p0 to p2:
        //
        //  p3 ----- p2
        //   |    /  |
        //   |  /    |
        //  p0 ----- p1
        let normalized_height = if fy > fx {
            h01 + (1.0 - fy) * (h00 - h01) + fx * (h11 - h01)
        } else {
            h10 + fy * (h11 - h10) + (1.0 - fx) * (h00 - h10)
        };

        self.min_height + normalized_height * self.height_range
    }

    /// Get height at heightmap index
    pub fn get_height_at_index(&self, x: u32, y: u32) -> f32 {
        if x >= self.width || y >= self.height {
            return 0.0;
        }

        let index = (y * self.width + x) as usize;
        if index < self.heights.len() {
            self.heights[index]
        } else {
            0.0
        }
    }

    /// Set height at heightmap index
    pub fn set_height_at_index(&mut self, x: u32, y: u32, height: f32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let index = (y * self.width + x) as usize;
        if index < self.heights.len() {
            self.heights[index] = height.clamp(0.0, 1.0);
        }
    }

    /// Get surface normal at world coordinates
    pub fn get_normal_at(&self, world_x: f32, world_y: f32) -> Vec3 {
        let step = self.scale;

        // Sample heights at neighboring points
        let center = self.get_height_at(world_x, world_y);
        let left = self.get_height_at(world_x - step, world_y);
        let right = self.get_height_at(world_x + step, world_y);
        let up = self.get_height_at(world_x, world_y - step);
        let down = self.get_height_at(world_x, world_y + step);

        calculate_normal(center, left, right, up, down, step)
    }

    /// Intersect ray with terrain heightmap
    pub fn intersect_ray(&self, ray_start: Vec3, ray_end: Vec3) -> Option<Vec3> {
        let direction = ray_end - ray_start;
        let length = direction.length();

        if length == 0.0 {
            return None;
        }

        let normalized_dir = direction / length;
        let step_size = self.scale * 0.5; // Half a texel for accuracy
        let max_steps = (length / step_size).ceil() as u32;

        // March along the ray
        for i in 0..max_steps {
            let t = i as f32 * step_size;
            let current_pos = ray_start + normalized_dir * t;

            // Check if we're within terrain bounds
            if !self.is_valid_position(current_pos.x, current_pos.y) {
                continue;
            }

            let terrain_height = self.get_height_at(current_pos.x, current_pos.y);

            // Check intersection
            if current_pos.z <= terrain_height {
                // Found intersection, refine it
                return Some(Vec3::new(current_pos.x, current_pos.y, terrain_height));
            }
        }

        None
    }

    /// Check if world position is within heightmap bounds
    pub fn is_valid_position(&self, world_x: f32, world_y: f32) -> bool {
        let hm_x = world_x / self.scale;
        let hm_y = world_y / self.scale;

        hm_x >= 0.0 && hm_y >= 0.0 && hm_x < self.width as f32 && hm_y < self.height as f32
    }

    pub fn get_display_height(&self, x: i32, y: i32) -> u8 {
        let ndx = (x + self.draw_origin_x) + (self.width as i32) * (y + self.draw_origin_y);
        if ndx >= 0 && (ndx as usize) < self.heights.len() {
            (self.heights[ndx as usize] * (K_MAX_HEIGHT as f32)).round() as u8
        } else {
            0
        }
    }

    pub fn get_raw_height(&self, x_index: i32, y_index: i32) -> u8 {
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx >= 0 && (ndx as usize) < self.heights.len() {
            (self.heights[ndx as usize] * (K_MAX_HEIGHT as f32)).round() as u8
        } else {
            0
        }
    }

    pub fn set_raw_height(&mut self, x_index: i32, y_index: i32, height: u8) {
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx >= 0 && (ndx as usize) < self.heights.len() {
            self.heights[ndx as usize] = height as f32 / K_MAX_HEIGHT as f32;
        }
    }

    pub fn get_tile_index(&self, x_index: i32, y_index: i32) -> i16 {
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx >= 0 && (ndx as usize) < self.tile_ndxes.len() {
            self.tile_ndxes[ndx as usize]
        } else {
            0
        }
    }

    /// C++ `WorldHeightMap::getTerrainNameAt` indexes the logic map by
    /// floor(world / MAP_XY_FACTOR), clamps to map bounds, then shifts the
    /// packed tile index right by two because four grids share one tile.
    pub fn get_packed_terrain_tile_at_world(&self, world_x: f32, world_y: f32) -> u32 {
        if self.width == 0 || self.height == 0 || self.scale.abs() <= f32::EPSILON {
            return 0;
        }

        let max_x = self.width.saturating_sub(1) as i32;
        let max_y = self.height.saturating_sub(1) as i32;
        let x_index = ((world_x / self.scale).floor() as i32 + self.border_size).clamp(0, max_x);
        let y_index = ((world_y / self.scale).floor() as i32 + self.border_size).clamp(0, max_y);
        let packed_tile = self.get_tile_index(x_index, y_index) as i32;
        (packed_tile >> 2).max(0) as u32
    }

    /// Match C++ `WorldHeightMap::getTerrainColorAt`: floor/clamp the world
    /// position, unpack the 4-grid terrain tile index, sample the source tile
    /// mipped down to one BGRA pixel, and return RGB floats.
    pub fn get_terrain_color_at_world(
        &self,
        world_x: f32,
        world_y: f32,
        source_tiles: &[Option<TileData>],
    ) -> [f32; 3] {
        if self.width == 0 || self.height == 0 || self.scale.abs() <= f32::EPSILON {
            return [0.0, 0.0, 0.0];
        }

        let max_x = self.width.saturating_sub(1) as i32;
        let max_y = self.height.saturating_sub(1) as i32;
        let x_index = ((world_x / self.scale).floor() as i32 + self.border_size).clamp(0, max_x);
        let y_index = ((world_y / self.scale).floor() as i32 + self.border_size).clamp(0, max_y);
        let ndx = y_index * self.width as i32 + x_index;
        if ndx < 0 || (ndx as usize) >= self.heights.len() {
            return [0.0, 0.0, 0.0];
        }

        let tile_ndx = self.tile_ndxes.get(ndx as usize).copied().unwrap_or(0) >> 2;
        if tile_ndx < 0 {
            return [0.0, 0.0, 0.0];
        }

        let Some(Some(tile)) = source_tiles.get(tile_ndx as usize) else {
            return [0.0, 0.0, 0.0];
        };
        let pixel = tile.get_rgb_data_for_width(1);
        if pixel.len() < 3 {
            return [0.0, 0.0, 0.0];
        }

        [
            pixel[2] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[0] as f32 / 255.0,
        ]
    }

    pub fn get_blend_tile_index(&self, x_index: i32, y_index: i32) -> i16 {
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx >= 0 && (ndx as usize) < self.blend_tile_ndxes.len() {
            self.blend_tile_ndxes[ndx as usize]
        } else {
            0
        }
    }

    /// Matches C++ WorldHeightMap::getPointerToTileData. Given a tile data
    /// source (callback for get_raw_tile_data) and blend tiles, returns the
    /// BGRA pixel data for the tile at (x_index, y_index) blended with any
    /// overlay tiles.
    pub fn get_pointer_to_tile_data<F>(
        &self,
        x_index: i32,
        y_index: i32,
        width: i32,
        source_tiles: &[Option<super::textures::TileData>; NUM_SOURCE_TILES],
        blend_tiles: &[super::textures::BlendTileInfo; NUM_BLEND_TILES],
        alpha_tiles: &[Option<Vec<u8>>; NUM_ALPHA_TILES],
        get_raw_tile_data: &F,
    ) -> Option<Vec<u8>>
    where
        F: Fn(i16, i32, &mut [u8]) -> bool,
    {
        if y_index < 0
            || x_index < 0
            || x_index >= self.width as i32
            || y_index >= self.height as i32
        {
            return None;
        }
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx < 0 || (ndx as usize) >= self.heights.len() {
            return None;
        }

        let tile_ndx = self.tile_ndxes.get(ndx as usize).copied().unwrap_or(0);
        let data_len = (width * width * 4) as usize;
        let mut buffer = vec![0u8; data_len];

        if get_raw_tile_data(tile_ndx, width, &mut buffer) {
            let blend_ndx = self
                .blend_tile_ndxes
                .get(ndx as usize)
                .copied()
                .unwrap_or(0);
            if blend_ndx > 0 && (blend_ndx as usize) < NUM_BLEND_TILES {
                let blend = &blend_tiles[blend_ndx as usize];
                let mut blend_buffer = vec![0u8; data_len];
                if get_raw_tile_data(blend.blend_ndx as i16, width, &mut blend_buffer) {
                    let alpha_data = Self::get_rgb_alpha_data_for_width(width, blend, alpha_tiles);
                    let pixel_count = (width * width) as usize;
                    for i in 0..pixel_count {
                        let base = i * 4;
                        let a = alpha_data.get(base + 3).copied().unwrap_or(0);
                        let b_blend = blend_buffer[base] as i32;
                        let g_blend = blend_buffer[base + 1] as i32;
                        let r_blend = blend_buffer[base + 2] as i32;
                        let a_i = a as i32;
                        let inv_a = 255 - a_i;
                        buffer[base] =
                            ((b_blend * a_i) / 255 + (buffer[base] as i32 * inv_a) / 255) as u8;
                        buffer[base + 1] =
                            ((g_blend * a_i) / 255 + (buffer[base + 1] as i32 * inv_a) / 255) as u8;
                        buffer[base + 2] =
                            ((r_blend * a_i) / 255 + (buffer[base + 2] as i32 * inv_a) / 255) as u8;
                        buffer[base + 3] = 255;
                    }
                }
            }
            return Some(buffer);
        }

        None
    }

    /// Matches C++ WorldHeightMap::getRGBAlphaDataForWidth.
    /// Returns the alpha tile data for the given blend direction.
    fn get_rgb_alpha_data_for_width(
        width: i32,
        blend: &super::textures::BlendTileInfo,
        alpha_tiles: &[Option<Vec<u8>>; NUM_ALPHA_TILES],
    ) -> Vec<u8> {
        let mut alpha_ndx = 0usize;
        if blend.horiz != 0 {
            alpha_ndx = K_HORIZ;
        } else if blend.vert != 0 {
            alpha_ndx = K_VERT;
        } else if blend.right_diagonal != 0 {
            alpha_ndx = K_RDIAG;
            if blend.long_diagonal != 0 {
                alpha_ndx = K_LRDIAG;
            }
        } else if blend.left_diagonal != 0 {
            alpha_ndx = K_LDIAG;
            if blend.long_diagonal != 0 {
                alpha_ndx = K_LLDIAG;
            }
        }
        if blend.inverted != 0 {
            alpha_ndx += K_INV;
        }

        let pixels_per_side = width as usize;
        let data_len = pixels_per_side * pixels_per_side * 4;
        if let Some(Some(alpha)) = alpha_tiles.get(alpha_ndx) {
            if alpha.len() >= data_len {
                return alpha.clone();
            }
        }

        vec![0u8; data_len]
    }

    /// Apply terrain modification
    pub fn apply_modification(
        &mut self,
        center: Vec3,
        radius: f32,
        strength: f32,
        operation: HeightModOperation,
    ) {
        let hm_center_x = center.x / self.scale;
        let hm_center_y = center.y / self.scale;
        let hm_radius = radius / self.scale;

        // Calculate affected region
        let min_x = ((hm_center_x - hm_radius).floor() as i32).max(0) as u32;
        let max_x = ((hm_center_x + hm_radius).ceil() as i32).min(self.width as i32 - 1) as u32;
        let min_y = ((hm_center_y - hm_radius).floor() as i32).max(0) as u32;
        let max_y = ((hm_center_y + hm_radius).ceil() as i32).min(self.height as i32 - 1) as u32;

        // Apply modification to each affected height sample
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 - hm_center_x;
                let dy = y as f32 - hm_center_y;
                let distance = (dx * dx + dy * dy).sqrt();

                if distance <= hm_radius {
                    let falloff = 1.0 - (distance / hm_radius);
                    let effect_strength = strength * falloff;

                    let current_height = self.get_height_at_index(x, y);
                    let new_height = match operation {
                        HeightModOperation::Raise => current_height + effect_strength,
                        HeightModOperation::Lower => current_height - effect_strength,
                        HeightModOperation::Flatten(target) => {
                            let target_normalized = target / self.max_height;
                            current_height + (target_normalized - current_height) * effect_strength
                        }
                        HeightModOperation::Smooth => {
                            // Sample neighboring heights for smoothing
                            let mut sum = current_height;
                            let mut count = 1;

                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    if dx == 0 && dy == 0 {
                                        continue;
                                    }

                                    let nx = (x as i32 + dx) as u32;
                                    let ny = (y as i32 + dy) as u32;

                                    if nx < self.width && ny < self.height {
                                        sum += self.get_height_at_index(nx, ny);
                                        count += 1;
                                    }
                                }
                            }

                            let average = sum / count as f32;
                            current_height + (average - current_height) * effect_strength
                        }
                    };

                    self.set_height_at_index(x, y, new_height);
                }
            }
        }
    }

    /// Generate mesh vertices for a region of the heightmap
    pub fn generate_mesh(
        &self,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
        lod_level: u8,
    ) -> (Vec<HeightMapVertex>, Vec<u32>) {
        let step = 1u32 << lod_level; // LOD step size
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Generate vertices
        for y in (min_y..=max_y).step_by(step as usize) {
            for x in (min_x..=max_x).step_by(step as usize) {
                let world_x = x as f32 * self.scale;
                let world_y = y as f32 * self.scale;
                let height = self.get_height_at(world_x, world_y);
                let normal = self.get_normal_at(world_x, world_y);

                vertices.push(HeightMapVertex {
                    position: [world_x, world_y, height],
                    normal: [normal.x, normal.y, normal.z],
                    tex_coords: [x as f32 / self.width as f32, y as f32 / self.height as f32],
                });
            }
        }

        // Generate indices for triangle strips
        let width_in_vertices = (max_x - min_x) / step + 1;
        let height_in_vertices = (max_y - min_y) / step + 1;

        for y in 0..height_in_vertices - 1 {
            for x in 0..width_in_vertices - 1 {
                let base = y * width_in_vertices + x;

                // First triangle
                indices.push(base);
                indices.push(base + width_in_vertices);
                indices.push(base + 1);

                // Second triangle
                indices.push(base + 1);
                indices.push(base + width_in_vertices);
                indices.push(base + width_in_vertices + 1);
            }
        }

        (vertices, indices)
    }

    /// Calculate bounding box for heightmap region
    pub fn calculate_bounds(&self, min_x: u32, min_y: u32, max_x: u32, max_y: u32) -> (Vec3, Vec3) {
        let mut min_height = f32::MAX;
        let mut max_height = f32::MIN;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let height = self.get_height_at_index(x, y) * self.max_height;
                min_height = min_height.min(height);
                max_height = max_height.max(height);
            }
        }

        let world_min_x = min_x as f32 * self.scale;
        let world_min_y = min_y as f32 * self.scale;
        let world_max_x = max_x as f32 * self.scale;
        let world_max_y = max_y as f32 * self.scale;

        (
            Vec3::new(world_min_x, world_min_y, min_height),
            Vec3::new(world_max_x, world_max_y, max_height),
        )
    }

    /// Get heightmap statistics
    pub fn get_statistics(&self) -> HeightMapStats {
        let mut min_height = f32::MAX;
        let mut max_height = f32::MIN;
        let mut sum = 0.0;

        for &height in &self.heights {
            let world_height = height * self.max_height;
            min_height = min_height.min(world_height);
            max_height = max_height.max(world_height);
            sum += world_height;
        }

        let average_height = sum / self.heights.len() as f32;

        HeightMapStats {
            width: self.width,
            height: self.height,
            min_height,
            max_height,
            average_height,
            scale: self.scale,
            memory_usage: self.heights.len() * std::mem::size_of::<f32>(),
        }
    }
}

/// Height modification operation
#[derive(Debug, Clone, Copy)]
pub enum HeightModOperation {
    Raise,
    Lower,
    Flatten(f32), // Target height
    Smooth,
}

/// Vertex data for heightmap mesh generation
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct HeightMapVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
}

unsafe impl bytemuck::Pod for HeightMapVertex {}
unsafe impl bytemuck::Zeroable for HeightMapVertex {}

/// Heightmap statistics
#[derive(Debug, Clone)]
pub struct HeightMapStats {
    pub width: u32,
    pub height: u32,
    pub min_height: f32,
    pub max_height: f32,
    pub average_height: f32,
    pub scale: f32,
    pub memory_usage: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heightmap_creation() {
        let heightmap = HeightMap::new(64, 64, 100.0, 1.0);

        assert_eq!(heightmap.width, 64);
        assert_eq!(heightmap.height, 64);
        assert_eq!(heightmap.max_height, 100.0);
        assert_eq!(heightmap.heights.len(), 64 * 64);
    }

    #[test]
    fn test_heightmap_sampling() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);

        // Set some test heights
        heightmap.set_height_at_index(1, 1, 0.5); // 50% of max height
        heightmap.set_height_at_index(2, 1, 1.0); // 100% of max height
        heightmap.set_height_at_index(1, 2, 0.25); // 25% of max height
        heightmap.set_height_at_index(2, 2, 0.75); // 75% of max height

        // Test height sampling
        let height = heightmap.get_height_at(1.5, 1.5); // Sample center of 2x2 region
        let expected = (0.5 + 1.0 + 0.25 + 0.75) / 4.0 * 100.0; // Average * max_height

        assert!((height - expected).abs() < 0.001);
    }

    #[test]
    fn heightmap_sampling_uses_cpp_triangle_split_not_bilinear() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);

        heightmap.set_height_at_index(1, 1, 0.0);
        heightmap.set_height_at_index(2, 1, 0.0);
        heightmap.set_height_at_index(1, 2, 1.0);
        heightmap.set_height_at_index(2, 2, 0.0);

        let height = heightmap.get_height_at(1.25, 1.75);

        assert!((height - 50.0).abs() < 0.001);
    }

    #[test]
    fn packed_terrain_tile_query_matches_cpp_floor_clamp_and_shift() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);
        heightmap.border_size = 1;
        heightmap.tile_ndxes[(2 * 4 + 2) as usize] = 44;
        heightmap.tile_ndxes[0] = 28;

        assert_eq!(heightmap.get_packed_terrain_tile_at_world(1.25, 1.75), 11);
        assert_eq!(heightmap.get_packed_terrain_tile_at_world(-99.0, -99.0), 7);
    }

    #[test]
    fn alpha_tile_selection_treats_any_nonzero_inverted_as_cpp_true() {
        let alpha_tiles: [Option<Vec<u8>>; NUM_ALPHA_TILES] = std::array::from_fn(|index| {
            let mut data = vec![0u8; 4];
            data[3] = index as u8;
            Some(data)
        });
        let mut blend = crate::terrain::textures::BlendTileInfo::new();
        blend.horiz = 1;
        blend.inverted = crate::terrain::textures::FLIPPED_MASK;

        let alpha = HeightMap::get_rgb_alpha_data_for_width(1, &blend, &alpha_tiles);

        assert_eq!(alpha[3], (K_INV + K_HORIZ) as u8);
    }

    #[test]
    fn test_heightmap_sampling_includes_exact_map_edges() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);
        heightmap.set_height_at_index(3, 0, 0.25);
        heightmap.set_height_at_index(0, 3, 0.5);
        heightmap.set_height_at_index(3, 3, 0.75);

        assert!((heightmap.get_height_at(3.0, 0.0) - 25.0).abs() < 0.001);
        assert!((heightmap.get_height_at(0.0, 3.0) - 50.0).abs() < 0.001);
        assert!((heightmap.get_height_at(3.0, 3.0) - 75.0).abs() < 0.001);
        assert_eq!(heightmap.get_height_at(3.001, 3.0), 0.0);
    }

    #[test]
    fn test_normal_calculation() {
        let mut heightmap = HeightMap::new(5, 5, 100.0, 1.0);

        // Create a slope
        for y in 0..5 {
            for x in 0..5 {
                heightmap.set_height_at_index(x, y, x as f32 / 4.0);
            }
        }

        let normal = heightmap.get_normal_at(2.0, 2.0);

        // Should point generally upward and to the left (negative X slope)
        assert!(normal.z > 0.0);
        assert!(normal.x < 0.0);

        // Should be normalized
        assert!((normal.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_ray_intersection() {
        let mut heightmap = HeightMap::new(10, 10, 100.0, 1.0);

        // Create a raised area
        heightmap.set_height_at_index(5, 5, 1.0);

        // Ray from above should intersect
        let ray_start = Vec3::new(5.0, 5.0, 200.0);
        let ray_end = Vec3::new(5.0, 5.0, 0.0);

        let intersection = heightmap.intersect_ray(ray_start, ray_end);
        assert!(intersection.is_some());

        let hit_point = intersection.unwrap();
        assert_eq!(hit_point.x, 5.0);
        assert_eq!(hit_point.y, 5.0);
        assert!(hit_point.z > 90.0); // Should be near max height
    }

    #[test]
    fn test_terrain_modification() {
        let mut heightmap = HeightMap::new(10, 10, 100.0, 1.0);

        // Raise terrain at center
        let center = Vec3::new(5.0, 5.0, 0.0);
        heightmap.apply_modification(center, 2.0, 0.5, HeightModOperation::Raise);

        // Check that center was raised
        let center_height = heightmap.get_height_at_index(5, 5);
        assert!(center_height > 0.0);

        // Check that effect diminishes with distance
        let edge_height = heightmap.get_height_at_index(7, 5);
        assert!(edge_height < center_height);
    }

    #[test]
    fn test_mesh_generation() {
        let heightmap = HeightMap::new(5, 5, 100.0, 1.0);
        let (vertices, indices) = heightmap.generate_mesh(0, 0, 4, 4, 0);

        // Should generate 5x5 = 25 vertices
        assert_eq!(vertices.len(), 25);

        // Should generate 4x4 quads = 32 triangles = 96 indices
        assert_eq!(indices.len(), 96);

        // Check first vertex
        assert_eq!(vertices[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(vertices[0].tex_coords, [0.0, 0.0]);
    }
}
