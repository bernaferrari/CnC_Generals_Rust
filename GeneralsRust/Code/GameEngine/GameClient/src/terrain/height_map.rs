//! # Height Map System
//!
//! Handles terrain height data loading, processing, and querying.
//! Supports multiple formats including .hmp, .tga, and .raw files.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use gamelogic::common::types::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use glam::Vec3;
use image::{DynamicImage, ImageBuffer, Luma};

use super::textures::{BlendTileInfo, FLIPPED_MASK, INVERTED_MASK, TileData};
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

/// Result of C++ `WorldHeightMap::getExtraAlphaUVData`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtraAlphaUvData {
    pub u: [f32; 4],
    pub v: [f32; 4],
    pub alpha: [u8; 4],
    pub need_flip: bool,
    pub cliff: bool,
}

impl Default for ExtraAlphaUvData {
    fn default() -> Self {
        Self {
            u: [0.0, 1.0, 1.0, 0.0],
            v: [0.0, 0.0, 1.0, 1.0],
            alpha: [0; 4],
            need_flip: false,
            cliff: false,
        }
    }
}

/// One GPU extra-blend vertex (Y-up: x, height, z).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtraBlendDrawVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub color: [f32; 4],
}

/// Two-triangle extra-blend overlay mesh (C++ second 3-way pass).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExtraBlendDrawMesh {
    pub vertices: Vec<ExtraBlendDrawVertex>,
    pub indices: Vec<u32>,
    pub tile_count: usize,
}

impl ExtraBlendDrawMesh {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() || self.indices.is_empty()
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn index_count(&self) -> usize {
        self.indices.len()
    }
}

/// C++ `TCliffInfo` — per-cell mutant/flip UVs for steep faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TCliffInfo {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub u2: f32,
    pub v2: f32,
    pub u3: f32,
    pub v3: f32,
    pub flip: bool,
    pub mutant: bool,
    pub tile_index: i16,
}

impl Default for TCliffInfo {
    fn default() -> Self {
        Self {
            u0: 0.0,
            v0: 0.0,
            u1: 1.0,
            v1: 0.0,
            u2: 1.0,
            v2: 1.0,
            u3: 0.0,
            v3: 1.0,
            flip: false,
            mutant: false,
            tile_index: 0,
        }
    }
}

/// Result of C++ `WorldHeightMap::getUVForTileIndex`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeightMapUvData {
    pub u: [f32; 4],
    pub v: [f32; 4],
    pub flip: bool,
    pub stretched: bool,
}

impl Default for HeightMapUvData {
    fn default() -> Self {
        Self {
            u: [0.0, 1.0, 1.0, 0.0],
            v: [1.0, 1.0, 0.0, 0.0],
            flip: false,
            stretched: false,
        }
    }
}

/// C++ `shoreLineTileInfo` — cell that straddles the water plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShoreLineTile {
    pub packed_xy: u32,
    pub verts: [[f32; 3]; 4],
    pub t: [f32; 4],
}

const STRETCH_LIMIT: f32 = 1.5;
const TILE_LIMIT: f32 = 4.0;
const TALL_STRETCH_LIMIT: f32 = 2.0;
const DIAMOND_STRETCH_LIMIT: f32 = 2.4;

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
    /// C++ `m_extraBlendTileNdxes` — second blend overlay, parallel to `blend_tile_ndxes`.
    pub extra_blend_tile_ndxes: Vec<i16>,
    /// C++ `m_blendedTiles` used by extra-blend UV/alpha/flip.
    pub blended_tiles: Vec<BlendTileInfo>,
    /// C++ `m_extraBlendedTiles` fallback for extra-blend info.
    pub extra_blended_tiles: Vec<BlendTileInfo>,
    /// C++ `m_cliffInfo` — index 0 unused.
    pub cliff_info: Vec<TCliffInfo>,
    /// C++ `m_cliffInfoNdxes` parallel to `tile_ndxes`.
    pub cliff_info_ndxes: Vec<i16>,
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
            extra_blend_tile_ndxes: vec![0i16; sample_count],
            blended_tiles: Vec::new(),
            extra_blended_tiles: Vec::new(),
            cliff_info: vec![TCliffInfo::default()],
            cliff_info_ndxes: vec![0i16; sample_count],
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
            extra_blend_tile_ndxes: vec![0i16; sample_count],
            blended_tiles: Vec::new(),
            extra_blended_tiles: Vec::new(),
            cliff_info: vec![TCliffInfo::default()],
            cliff_info_ndxes: vec![0i16; sample_count],
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
            extra_blend_tile_ndxes: vec![0i16; sample_count],
            blended_tiles: Vec::new(),
            extra_blended_tiles: Vec::new(),
            cliff_info: vec![TCliffInfo::default()],
            cliff_info_ndxes: vec![0i16; sample_count],
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
            extra_blend_tile_ndxes: vec![0i16; sample_count],
            blended_tiles: Vec::new(),
            extra_blended_tiles: Vec::new(),
            cliff_info: vec![TCliffInfo::default()],
            cliff_info_ndxes: vec![0i16; sample_count],
            draw_origin_x: 0,
            draw_origin_y: 0,
            draw_width: dimension as i32,
            draw_height: dimension as i32,
        })
    }

    /// Get height at world coordinates using the C++ height-map triangle split.
    ///
    /// Out-of-range samples clamp to the edge cell like C++ `getClipHeight` /
    /// `getMaxCellHeight` (`BaseHeightMap.cpp`). They never return a literal 0.0
    /// just because the query is off-map; empty maps and zero scale still return 0.
    pub fn get_height_at(&self, world_x: f32, world_y: f32) -> f32 {
        if self.width == 0 || self.height == 0 || self.scale.abs() <= f32::EPSILON {
            return 0.0;
        }

        // Convert world coordinates to heightmap coordinates.
        // C++ adds getBorderSizeInline() so playable (0,0) samples (border, border).
        let mut hm_x = world_x / self.scale + self.border_size as f32;
        let mut hm_y = world_y / self.scale + self.border_size as f32;

        // C++ BaseHeightMap::getClipHeight: x<0 -> 0; x>extent-1 -> extent-1.
        let max_x = self.width.saturating_sub(1) as f32;
        let max_y = self.height.saturating_sub(1) as f32;
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

    /// Get height at heightmap index. Out-of-range samples clamp to the edge
    /// cell like C++ `getClipHeight` — they never invent a 0.0 sea-level cliff.
    pub fn get_height_at_index(&self, x: u32, y: u32) -> f32 {
        if self.width == 0 || self.height == 0 || self.heights.is_empty() {
            return 0.0;
        }
        let x = x.min(self.width - 1);
        let y = y.min(self.height - 1);
        let index = (y * self.width + x) as usize;
        self.heights.get(index).copied().unwrap_or(0.0)
    }

    /// World-space height at a grid sample (same remapping as `get_height_at`).
    pub fn world_height_at_index(&self, x: u32, y: u32) -> f32 {
        self.min_height + self.get_height_at_index(x, y) * self.height_range
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

    /// Get surface normal at world coordinates.
    /// Neighbor samples go through `get_height_at`, so off-map taps clamp to
    /// the edge cell instead of inventing a 0.0 cliff.
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

    /// C++ `getClipHeight` — clamp indices then read the 8-bit sample.
    pub fn get_raw_height(&self, x_index: i32, y_index: i32) -> u8 {
        if self.width == 0 || self.height == 0 || self.heights.is_empty() {
            return 0;
        }
        let x = x_index.clamp(0, self.width as i32 - 1);
        let y = y_index.clamp(0, self.height as i32 - 1);
        let ndx = y * (self.width as i32) + x;
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

    pub fn assign_cliff_info(&mut self, info: Vec<TCliffInfo>, ndxes: Vec<i16>) {
        self.cliff_info = info;
        if self.cliff_info.is_empty() {
            self.cliff_info.push(TCliffInfo::default());
        }
        self.cliff_info_ndxes = ndxes;
    }

    /// C++ `WorldHeightMap::getUVData` for the cell at `(x_index, y_index)`.
    pub fn get_uv_data(&self, x_index: i32, y_index: i32, full_tile: bool) -> HeightMapUvData {
        let x = x_index + self.draw_origin_x;
        let y = y_index + self.draw_origin_y;
        let ndx = y * self.width as i32 + x;
        if ndx < 0 || (ndx as usize) >= self.tile_ndxes.len() {
            return HeightMapUvData::default();
        }
        let tile_ndx = self.tile_ndxes[ndx as usize];
        self.get_uv_for_tile_index(ndx as usize, tile_ndx, full_tile)
    }

    /// C++ `WorldHeightMap::getUVForTileIndex` including `DO_OLD_UV` stretch.
    pub fn get_uv_for_tile_index(
        &self,
        ndx: usize,
        tile_ndx: i16,
        full_tile: bool,
    ) -> HeightMapUvData {
        let mut uv = HeightMapUvData::default();
        // Quarter-tile UVs when not a full tile; splat slots treat each cell as 0..1.
        if full_tile {
            uv.u = [0.0, 1.0, 1.0, 0.0];
            uv.v = [1.0, 1.0, 0.0, 0.0];
        } else {
            let sub = (tile_ndx & 3) as f32;
            let ou = (sub % 2.0) * 0.5;
            let ov = (sub / 2.0).floor() * 0.5;
            uv.u = [ou, ou + 0.5, ou + 0.5, ou];
            uv.v = [ov + 0.5, ov + 0.5, ov, ov];
        }

        if ndx < self.cliff_info_ndxes.len() {
            let cliff_ndx = self.cliff_info_ndxes[ndx] as usize;
            if cliff_ndx > 0 && cliff_ndx < self.cliff_info.len() {
                let info = self.cliff_info[cliff_ndx];
                let same_class = (tile_ndx >> 2) == (info.tile_index >> 2);
                if same_class {
                    uv.u = [info.u0, info.u1, info.u2, info.u3];
                    uv.v = [info.v0, info.v1, info.v2, info.v3];
                    uv.flip = info.flip;
                    uv.stretched = true;
                    return uv;
                }
            }
        }

        if full_tile {
            return uv;
        }

        let width = self.width as usize;
        if ndx + width + 1 >= self.heights.len() {
            return uv;
        }
        let h0 = self.get_raw_height((ndx % width) as i32, (ndx / width) as i32) as i32;
        let h1 = self.get_raw_height((ndx % width) as i32 + 1, (ndx / width) as i32) as i32;
        let h2 = self.get_raw_height((ndx % width) as i32 + 1, (ndx / width) as i32 + 1) as i32;
        let h3 = self.get_raw_height((ndx % width) as i32, (ndx / width) as i32 + 1) as i32;
        let min_h = h0.min(h1).min(h2).min(h3);
        let max_h = h0.max(h1).max(h2).max(h3);
        let delta_h = max_h - min_h;
        let height_scale = MAP_HEIGHT_SCALE / MAP_XY_FACTOR.max(f32::EPSILON);
        if (delta_h as f32) * height_scale < STRETCH_LIMIT {
            return uv;
        }

        let below_limit = min_h + (2 * delta_h + 1) / 3;
        let above_limit = min_h + (delta_h + 1) / 3;
        let below = [h0, h1, h2, h3]
            .iter()
            .filter(|h| **h < below_limit)
            .count();
        let above = [h0, h1, h2, h3]
            .iter()
            .filter(|h| **h > above_limit)
            .count();
        if above != 1
            && below != 1
            && (above != 2 || below != 2)
            && (delta_h as f32) * height_scale < DIAMOND_STRETCH_LIMIT
        {
            return uv;
        }

        let mut divisor = TILE_LIMIT / ((delta_h as f32) * height_scale).max(f32::EPSILON);
        divisor = divisor.clamp(1.0, TILE_LIMIT);
        let n_v = uv.v[2];
        let x_v = uv.v[0];
        let delta_v = x_v - n_v;

        if below == 1 || above > below {
            if h0 == min_h {
                uv.v[0] = n_v + delta_v / divisor;
            } else if h1 == min_h {
                uv.v[1] = n_v + delta_v / divisor;
            } else if h2 == min_h {
                uv.v[2] = x_v - delta_v / divisor;
            } else if h3 == min_h {
                uv.v[3] = x_v - delta_v / divisor;
            }
            uv.stretched = true;
        } else if above == 1 || below > above {
            if h0 == max_h {
                uv.v[0] = n_v + delta_v / divisor;
            } else if h1 == max_h {
                uv.v[1] = n_v + delta_v / divisor;
            } else if h2 == max_h {
                uv.v[2] = x_v - delta_v / divisor;
            } else if h3 == max_h {
                uv.v[3] = x_v - delta_v / divisor;
            }
            uv.stretched = true;
        } else if (delta_h as f32) * height_scale >= TALL_STRETCH_LIMIT {
            let n_u = uv.u[0];
            let x_u = uv.u[1];
            let mut dx = ((h3 - h2) as f32 * height_scale).hypot(1.0);
            let mut dy = ((h3 - h0) as f32 * height_scale).hypot(1.0);
            if dx < STRETCH_LIMIT {
                dx = 1.0;
            }
            if dy < STRETCH_LIMIT {
                dy = 1.0;
            }
            dx = dx.min(TILE_LIMIT) * (x_u - n_u);
            dy = dy.min(TILE_LIMIT) * (x_v - n_v);
            uv.u = [n_u, n_u + dx, n_u + dx, n_u];
            uv.v = [n_v + dy, n_v + dy, n_v, n_v];
            let mut dx1 = ((h1 - h0) as f32 * height_scale).hypot(1.0);
            let mut dy1 = ((h2 - h1) as f32 * height_scale).hypot(1.0);
            if dx1 < STRETCH_LIMIT {
                dx1 = 1.0;
            }
            if dy1 < STRETCH_LIMIT {
                dy1 = 1.0;
            }
            dx1 = dx1.min(TILE_LIMIT) * (x_u - n_u);
            dy1 = dy1.min(TILE_LIMIT) * (x_v - n_v);
            uv.u[1] = uv.u[0] + dx1;
            uv.v[1] = uv.v[3] + dy1;
            uv.stretched = true;
        }
        uv
    }

    /// World-space UV for a vertex so each MAP_XY_FACTOR cell tiles once.
    pub fn cell_uv_at_world(&self, world_x: f32, world_z: f32) -> [f32; 2] {
        let scale = if self.scale.abs() > f32::EPSILON {
            self.scale
        } else {
            MAP_XY_FACTOR
        };
        let u = world_x / scale;
        let v = world_z / scale;
        let (ix, iy) = {
            let max_x = self.width.saturating_sub(1) as i32;
            let max_y = self.height.saturating_sub(1) as i32;
            let x = ((world_x / scale).floor() as i32 + self.border_size).clamp(0, max_x);
            let y = ((world_z / scale).floor() as i32 + self.border_size).clamp(0, max_y);
            (x, y)
        };
        let uv = self.get_uv_data(ix - self.draw_origin_x, iy - self.draw_origin_y, false);
        if uv.stretched {
            let fx = (world_x / scale + self.border_size as f32 - ix as f32).clamp(0.0, 1.0);
            let fy = (world_z / scale + self.border_size as f32 - iy as f32).clamp(0.0, 1.0);
            // Bilinear the four corner UVs.
            let uu = uv.u[0] * (1.0 - fx) * (1.0 - fy)
                + uv.u[1] * fx * (1.0 - fy)
                + uv.u[2] * fx * fy
                + uv.u[3] * (1.0 - fx) * fy;
            let vv = uv.v[0] * (1.0 - fx) * (1.0 - fy)
                + uv.v[1] * fx * (1.0 - fy)
                + uv.v[2] * fx * fy
                + uv.v[3] * (1.0 - fx) * fy;
            [uu, vv]
        } else {
            [u, v]
        }
    }

    /// C++ `updateShorelineTiles` over the full map.
    pub fn rebuild_shoreline_tiles(
        &self,
        water_height: impl Fn(f32, f32) -> f32,
        transparent_depth: f32,
        show_soft_edge: bool,
    ) -> Vec<ShoreLineTile> {
        if !show_soft_edge || transparent_depth <= f32::EPSILON {
            return Vec::new();
        }
        let depth_scale = 1.0 / transparent_depth;
        let border = self.border_size;
        let scale = if self.scale.abs() > f32::EPSILON {
            self.scale
        } else {
            MAP_XY_FACTOR
        };
        let max_x = self.width.saturating_sub(1) as i32;
        let max_y = self.height.saturating_sub(1) as i32;
        let mut tiles = Vec::new();
        for j in 0..max_y {
            for i in 0..max_x {
                let x0 = (i - border) as f32 * scale;
                let y0 = (j - border) as f32 * scale;
                let x1 = (i - border + 1) as f32 * scale;
                let y1 = (j - border + 1) as f32 * scale;
                let t0 = self.world_height_at_index(i as u32, j as u32);
                let t1 = self.world_height_at_index((i + 1) as u32, j as u32);
                let t2 = self.world_height_at_index((i + 1) as u32, (j + 1) as u32);
                let t3 = self.world_height_at_index(i as u32, (j + 1) as u32);
                let w0 = water_height(x0, y0);
                let w1 = water_height(x1, y0);
                let w2 = water_height(x1, y1);
                let w3 = water_height(x0, y1);
                if w0 <= 0.0 || w1 <= 0.0 || w2 <= 0.0 || w3 <= 0.0 {
                    continue;
                }
                let mut water_side = 0u8;
                if w0 > t0 {
                    water_side |= 1;
                }
                if w1 > t1 {
                    water_side |= 2;
                }
                if w2 > t2 {
                    water_side |= 4;
                }
                if w3 > t3 {
                    water_side |= 8;
                }
                if water_side == 0 {
                    continue;
                }
                if water_side == 0xf
                    && (w0 - t0) >= transparent_depth
                    && (w1 - t1) >= transparent_depth
                    && (w2 - t2) >= transparent_depth
                    && (w3 - t3) >= transparent_depth
                {
                    continue;
                }
                tiles.push(ShoreLineTile {
                    packed_xy: (i as u32) | ((j as u32) << 16),
                    verts: [[x0, t0, y0], [x1, t1, y0], [x1, t2, y1], [x0, t3, y1]],
                    t: [
                        ((w0 - t0) * depth_scale).clamp(0.0, 1.0),
                        ((w1 - t1) * depth_scale).clamp(0.0, 1.0),
                        ((w2 - t2) * depth_scale).clamp(0.0, 1.0),
                        ((w3 - t3) * depth_scale).clamp(0.0, 1.0),
                    ],
                });
            }
        }
        tiles
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

    /// Source-tile index under a world position, mirroring the
    /// floor/clamp/border unpack in `get_terrain_color_at_world`
    /// (C++ `WorldHeightMap::getTerrainColorAt` ndx/tileNdx math,
    /// WorldHeightMap.cpp:2333-2345). `None` outside the map or where no
    /// tile is assigned.
    pub fn tile_ndx_at_world(&self, world_x: f32, world_y: f32) -> Option<usize> {
        if self.width == 0 || self.height == 0 || self.scale.abs() <= f32::EPSILON {
            return None;
        }
        let max_x = self.width.saturating_sub(1) as i32;
        let max_y = self.height.saturating_sub(1) as i32;
        let x_index = ((world_x / self.scale).floor() as i32 + self.border_size).clamp(0, max_x);
        let y_index = ((world_y / self.scale).floor() as i32 + self.border_size).clamp(0, max_y);
        let ndx = y_index * self.width as i32 + x_index;
        if ndx < 0 || (ndx as usize) >= self.heights.len() {
            return None;
        }
        let tile_ndx = self.tile_ndxes.get(ndx as usize).copied().unwrap_or(0) >> 2;
        if tile_ndx < 0 {
            return None;
        }
        Some(tile_ndx as usize)
    }

    pub fn get_blend_tile_index(&self, x_index: i32, y_index: i32) -> i16 {
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx >= 0 && (ndx as usize) < self.blend_tile_ndxes.len() {
            self.blend_tile_ndxes[ndx as usize]
        } else {
            0
        }
    }

    pub fn get_extra_blend_tile_index(&self, x_index: i32, y_index: i32) -> i16 {
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx >= 0 && (ndx as usize) < self.extra_blend_tile_ndxes.len() {
            self.extra_blend_tile_ndxes[ndx as usize]
        } else {
            0
        }
    }

    /// Assign parsed map extra-blend indices (must match sample count).
    pub fn assign_extra_blend_tile_ndxes(&mut self, ndxes: Vec<i16>) {
        self.extra_blend_tile_ndxes = ndxes;
    }

    pub fn assign_blended_tiles(&mut self, tiles: Vec<BlendTileInfo>) {
        self.blended_tiles = tiles;
    }

    pub fn assign_extra_blended_tiles(&mut self, tiles: Vec<BlendTileInfo>) {
        self.extra_blended_tiles = tiles;
    }

    /// C++ `WorldHeightMap::getExtraAlphaUVData`.
    /// Returns `Some` when the extra-blend ndx is non-zero. Missing blend-tile
    /// records still produce unit UVs so the GPU second pass can emit geometry.
    pub fn get_extra_alpha_uv_data(&self, x_index: i32, y_index: i32) -> Option<ExtraAlphaUvData> {
        let ndx = y_index * (self.width as i32) + x_index;
        if ndx < 0 || (ndx as usize) >= self.extra_blend_tile_ndxes.len() {
            return None;
        }
        let blend_ndx = self.extra_blend_tile_ndxes[ndx as usize];
        if blend_ndx == 0 {
            return None;
        }

        let mut data = ExtraAlphaUvData::default();
        let blend = if (blend_ndx as usize) < self.blended_tiles.len() {
            Some(&self.blended_tiles[blend_ndx as usize])
        } else if (blend_ndx as usize) < self.extra_blended_tiles.len() {
            Some(&self.extra_blended_tiles[blend_ndx as usize])
        } else {
            None
        };
        let Some(blend) = blend else {
            return Some(data);
        };
        let uv = self.get_uv_for_tile_index(ndx as usize, blend.blend_ndx as i16, false);
        data.u = uv.u;
        data.v = uv.v;
        data.cliff = uv.stretched;
        if uv.flip {
            data.need_flip = true;
        }

        if blend.horiz != 0 {
            data.need_flip = (blend.inverted & FLIPPED_MASK) != 0;
            if (blend.inverted & INVERTED_MASK) != 0 {
                data.alpha[0] = 255;
                data.alpha[3] = 255;
            } else {
                data.alpha[1] = 255;
                data.alpha[2] = 255;
            }
        }
        if blend.vert != 0 {
            data.need_flip = (blend.inverted & FLIPPED_MASK) != 0;
            if (blend.inverted & INVERTED_MASK) != 0 {
                data.alpha[0] = 255;
                data.alpha[1] = 255;
            } else {
                data.alpha[2] = 255;
                data.alpha[3] = 255;
            }
        }
        if blend.right_diagonal != 0 {
            if (blend.inverted & INVERTED_MASK) != 0 {
                data.alpha[1] = 255;
                if blend.long_diagonal != 0 {
                    data.alpha[0] = 255;
                    data.alpha[2] = 255;
                }
            } else {
                data.need_flip = true;
                data.alpha[2] = 255;
                if blend.long_diagonal != 0 {
                    data.alpha[1] = 255;
                    data.alpha[3] = 255;
                }
            }
        }
        if blend.left_diagonal != 0 {
            if (blend.inverted & INVERTED_MASK) != 0 {
                data.need_flip = true;
                data.alpha[0] = 255;
                if blend.long_diagonal != 0 {
                    data.alpha[1] = 255;
                    data.alpha[3] = 255;
                }
            } else {
                data.alpha[3] = 255;
                if blend.long_diagonal != 0 {
                    data.alpha[0] = 255;
                    data.alpha[2] = 255;
                }
            }
        }
        if blend.custom_blend_edge_class >= 0 {
            data.alpha = [0, 0, 0, 0];
            data.need_flip = false;
        }
        Some(data)
    }

    /// C++ `HeightMapRenderObjClass` packs extra-blend cells as `i | (j << 16)`.
    /// Scans cells (not vertices): `0..width-1` × `0..height-1`.
    pub fn collect_extra_blend_tile_positions(&self) -> Vec<u32> {
        collect_extra_blend_tile_positions(
            self.width as i32,
            self.height as i32,
            &self.extra_blend_tile_ndxes,
        )
    }

    /// Build the extra-blend overlay mesh for packed `i | (j << 16)` tiles.
    /// Two triangles per tile (6 verts), honoring `need_flip`.
    pub fn build_extra_blend_draw_mesh(&self, positions: &[u32]) -> ExtraBlendDrawMesh {
        self.build_extra_blend_draw_mesh_for_window(
            positions,
            self.draw_origin_x,
            self.draw_origin_y,
            self.draw_width,
            self.draw_height,
        )
    }

    pub fn build_extra_blend_draw_mesh_for_window(
        &self,
        positions: &[u32],
        draw_origin_x: i32,
        draw_origin_y: i32,
        draw_width: i32,
        draw_height: i32,
    ) -> ExtraBlendDrawMesh {
        let owned_positions;
        let positions = if positions.is_empty() {
            owned_positions = self.collect_extra_blend_tile_positions();
            owned_positions.as_slice()
        } else {
            positions
        };

        let x_extent = self.width as i32;
        let y_extent = self.height as i32;
        let mut draw_edge_x = draw_origin_x + draw_width - 1;
        let mut draw_edge_y = draw_origin_y + draw_height - 1;
        if draw_edge_x > x_extent - 1 {
            draw_edge_x = x_extent - 1;
        }
        if draw_edge_y > y_extent - 1 {
            draw_edge_y = y_extent - 1;
        }

        let mut mesh = ExtraBlendDrawMesh::default();
        let scale = self.scale;
        let border = self.border_size as f32;

        for packed in positions {
            let x = (packed & 0xffff) as i32;
            let y = (packed >> 16) as i32;
            if x < draw_origin_x || x >= draw_edge_x || y < draw_origin_y || y >= draw_edge_y {
                continue;
            }
            if x < 0 || y < 0 || x + 1 >= x_extent || y + 1 >= y_extent {
                continue;
            }
            let Some(uv) = self.get_extra_alpha_uv_data(x, y) else {
                continue;
            };

            let x0 = (x as f32 - border) * scale;
            let z0 = (y as f32 - border) * scale;
            let x1 = ((x + 1) as f32 - border) * scale;
            let z1 = ((y + 1) as f32 - border) * scale;
            let p0 = self.world_height_at_index(x as u32, y as u32);
            let p1 = self.world_height_at_index((x + 1) as u32, y as u32);
            let p2 = self.world_height_at_index((x + 1) as u32, (y + 1) as u32);
            let p3 = self.world_height_at_index(x as u32, (y + 1) as u32);

            let mut flip = uv.need_flip;
            if uv.cliff && (p0 - p2).abs() > (p1 - p3).abs() {
                flip = true;
            }

            let corners = [
                ExtraBlendDrawVertex {
                    position: [x0, p0, z0],
                    tex_coords: [uv.u[0], uv.v[0]],
                    color: [1.0, 1.0, 1.0, uv.alpha[0] as f32 / 255.0],
                },
                ExtraBlendDrawVertex {
                    position: [x1, p1, z0],
                    tex_coords: [uv.u[1], uv.v[1]],
                    color: [1.0, 1.0, 1.0, uv.alpha[1] as f32 / 255.0],
                },
                ExtraBlendDrawVertex {
                    position: [x1, p2, z1],
                    tex_coords: [uv.u[2], uv.v[2]],
                    color: [1.0, 1.0, 1.0, uv.alpha[2] as f32 / 255.0],
                },
                ExtraBlendDrawVertex {
                    position: [x0, p3, z1],
                    tex_coords: [uv.u[3], uv.v[3]],
                    color: [1.0, 1.0, 1.0, uv.alpha[3] as f32 / 255.0],
                },
            ];

            // C++ HeightMap.cpp extra-blend IB: flip uses 1,3,0 / 1,2,3 else 0,2,3 / 0,1,2.
            let order: [usize; 6] = if flip {
                [1, 3, 0, 1, 2, 3]
            } else {
                [0, 2, 3, 0, 1, 2]
            };
            let base = mesh.vertices.len() as u32;
            for (i, corner) in order.into_iter().enumerate() {
                mesh.vertices.push(corners[corner]);
                mesh.indices.push(base + i as u32);
            }
            mesh.tile_count += 1;
        }

        mesh
    }

    /// Matches C++ WorldHeightMap::getPointerToTileData. Given a tile data
    /// source (callback for get_raw_tile_data) and blend tiles, returns the
    /// BGRA pixel data for the tile at (x_index, y_index) blended with any
    /// overlay tiles, then the extra-blend (3-way) overlay when present.
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
            Self::apply_blend_overlay(
                &mut buffer,
                blend_ndx,
                width,
                blend_tiles,
                alpha_tiles,
                get_raw_tile_data,
            );
            // C++ 3-way extra blend: second alpha composite after first overlay.
            let extra_ndx = self
                .extra_blend_tile_ndxes
                .get(ndx as usize)
                .copied()
                .unwrap_or(0);
            Self::apply_blend_overlay(
                &mut buffer,
                extra_ndx,
                width,
                blend_tiles,
                alpha_tiles,
                get_raw_tile_data,
            );
            return Some(buffer);
        }

        None
    }

    /// Alpha-composite one blend overlay into `buffer` when `blend_ndx > 0`.
    fn apply_blend_overlay<F>(
        buffer: &mut [u8],
        blend_ndx: i16,
        width: i32,
        blend_tiles: &[super::textures::BlendTileInfo; NUM_BLEND_TILES],
        alpha_tiles: &[Option<Vec<u8>>; NUM_ALPHA_TILES],
        get_raw_tile_data: &F,
    ) where
        F: Fn(i16, i32, &mut [u8]) -> bool,
    {
        if blend_ndx <= 0 || (blend_ndx as usize) >= NUM_BLEND_TILES {
            return;
        }
        let blend = &blend_tiles[blend_ndx as usize];
        let mut blend_buffer = vec![0u8; buffer.len()];
        if !get_raw_tile_data(blend.blend_ndx as i16, width, &mut blend_buffer) {
            return;
        }
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
            buffer[base] = ((b_blend * a_i) / 255 + (buffer[base] as i32 * inv_a) / 255) as u8;
            buffer[base + 1] =
                ((g_blend * a_i) / 255 + (buffer[base + 1] as i32 * inv_a) / 255) as u8;
            buffer[base + 2] =
                ((r_blend * a_i) / 255 + (buffer[base + 2] as i32 * inv_a) / 255) as u8;
            buffer[base + 3] = 255;
        }
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

        // Generate vertices at exact grid samples (world height remapping).
        for y in (min_y..=max_y).step_by(step as usize) {
            for x in (min_x..=max_x).step_by(step as usize) {
                // C++ ADJUST_FROM_INDEX_TO_REAL(k) = (k - border) * MAP_XY_FACTOR
                let world_x = (x as f32 - self.border_size as f32) * self.scale;
                let world_y = (y as f32 - self.border_size as f32) * self.scale;
                let height = self.world_height_at_index(x, y);
                let normal = self.get_normal_at(world_x, world_y);

                vertices.push(HeightMapVertex {
                    position: [world_x, world_y, height],
                    normal: [normal.x, normal.y, normal.z],
                    tex_coords: [x as f32 / self.width as f32, y as f32 / self.height as f32],
                });
            }
        }

        // C++ cell split is the p0→p2 diagonal (same as `get_height_at`):
        //
        //  p3 ----- p2
        //   |    /  |
        //   |  /    |
        //  p0 ----- p1
        let width_in_vertices = (max_x - min_x) / step + 1;
        let height_in_vertices = (max_y - min_y) / step + 1;

        for y in 0..height_in_vertices - 1 {
            for x in 0..width_in_vertices - 1 {
                let base = y * width_in_vertices + x;
                let p0 = base;
                let p1 = base + 1;
                let p2 = base + width_in_vertices + 1;
                let p3 = base + width_in_vertices;

                // fy > fx triangle: p0, p3, p2
                indices.push(p0);
                indices.push(p3);
                indices.push(p2);

                // fy <= fx triangle: p0, p2, p1
                indices.push(p0);
                indices.push(p2);
                indices.push(p1);
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
                let height = self.world_height_at_index(x, y);
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

// SAFETY: `#[repr(C)]` position/normal/tex_coords f32 arrays pack with no hidden
// SAFETY: padding; vertex data is only read as raw bytes on upload.
unsafe impl bytemuck::Pod for HeightMapVertex {}
// SAFETY: All-zero fields are valid f32 values; no invariants imposed.
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

/// C++ `m_extraBlendTilePositions[n] = i | (j << 16)` for cells with extra blend.
pub fn collect_extra_blend_tile_positions(
    width: i32,
    height: i32,
    extra_blend_tile_ndxes: &[i16],
) -> Vec<u32> {
    if width < 2 || height < 2 {
        return Vec::new();
    }
    let mut positions = Vec::new();
    for j in 0..(height - 1) {
        for i in 0..(width - 1) {
            let ndx = (j * width + i) as usize;
            if extra_blend_tile_ndxes.get(ndx).copied().unwrap_or(0) > 0 {
                positions.push((i as u32) | ((j as u32) << 16));
            }
        }
    }
    positions
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
        assert_eq!(heightmap.extra_blend_tile_ndxes.len(), 64 * 64);
    }

    #[test]
    fn extra_blend_tile_ndxes_survive_assign_and_read_back() {
        let mut heightmap = HeightMap::new(2, 2, 100.0, 1.0);
        heightmap.assign_extra_blend_tile_ndxes(vec![0, 7, 0, 3]);
        assert_eq!(heightmap.extra_blend_tile_ndxes, vec![0, 7, 0, 3]);
        assert_eq!(heightmap.get_extra_blend_tile_index(1, 0), 7);
        assert_eq!(heightmap.get_extra_blend_tile_index(1, 1), 3);
        assert_eq!(heightmap.get_extra_blend_tile_index(0, 0), 0);
    }

    #[test]
    fn extra_blend_tile_positions_pack_i_or_j_shift_16_like_cpp() {
        let mut heightmap = HeightMap::new(3, 3, 100.0, 1.0);
        // 3x3 samples → 2x2 cells. Extra blend at (1,0) and (0,1).
        let mut ndxes = vec![0i16; 9];
        ndxes[1] = 4; // x=1,y=0
        ndxes[3] = 2; // x=0,y=1
        heightmap.assign_extra_blend_tile_ndxes(ndxes);
        let positions = heightmap.collect_extra_blend_tile_positions();
        assert_eq!(positions, vec![1 | (0 << 16), 0 | (1 << 16)]);
    }

    #[test]
    fn extra_blend_alpha_uv_sets_horiz_alphas_and_flip() {
        let mut heightmap = HeightMap::new(2, 2, 100.0, 1.0);
        heightmap.assign_extra_blend_tile_ndxes(vec![1, 0, 0, 0]);
        let mut tiles = vec![BlendTileInfo::new(); 2];
        tiles[1].horiz = 1;
        tiles[1].inverted = FLIPPED_MASK | INVERTED_MASK;
        heightmap.assign_blended_tiles(tiles);

        let data = heightmap
            .get_extra_alpha_uv_data(0, 0)
            .expect("extra blend cell");
        assert!(data.need_flip);
        assert_eq!(data.alpha, [255, 0, 0, 255]);
        assert!(heightmap.get_extra_alpha_uv_data(1, 0).is_none());
    }

    #[test]
    fn extra_blend_draw_mesh_has_two_triangles_per_tile() {
        let mut heightmap = HeightMap::new(3, 3, 100.0, 1.0);
        let mut ndxes = vec![0i16; 9];
        ndxes[0] = 1;
        heightmap.assign_extra_blend_tile_ndxes(ndxes);
        let mesh = heightmap.build_extra_blend_draw_mesh(&[]);
        assert!(
            mesh.vertex_count() >= 6,
            "one extra-blend tile must emit two triangles"
        );
        assert_eq!(mesh.index_count(), 6);
        assert_eq!(mesh.tile_count, 1);
    }

    #[test]
    fn extra_blend_draw_mesh_honors_need_flip() {
        let mut heightmap = HeightMap::new(3, 3, 100.0, 1.0);
        let mut ndxes = vec![0i16; 9];
        ndxes[0] = 1;
        heightmap.assign_extra_blend_tile_ndxes(ndxes);

        let unflipped = heightmap.build_extra_blend_draw_mesh(&[]);
        assert!(unflipped.vertex_count() >= 6);

        let mut tiles = vec![BlendTileInfo::new(); 2];
        tiles[1].right_diagonal = 1; // uninverted right diagonal forces flip
        heightmap.assign_blended_tiles(tiles);
        let flipped = heightmap.build_extra_blend_draw_mesh(&[]);
        assert!(flipped.vertex_count() >= 6);
        assert_ne!(
            unflipped.vertices[0].position, flipped.vertices[0].position,
            "need_flip must change the first triangle winding"
        );
        assert!(
            heightmap
                .get_extra_alpha_uv_data(0, 0)
                .expect("tile")
                .need_flip
        );
    }

    #[test]
    fn extra_blend_overlay_changes_composed_pixels_vs_first_blend_only() {
        let mut heightmap = HeightMap::new(1, 1, 255.0, 1.0);
        heightmap.tile_ndxes[0] = 0;
        heightmap.blend_tile_ndxes[0] = 1;
        heightmap.extra_blend_tile_ndxes[0] = 0;

        let source_tiles: Box<[Option<crate::terrain::textures::TileData>; NUM_SOURCE_TILES]> =
            vec![None; NUM_SOURCE_TILES]
                .into_boxed_slice()
                .try_into()
                .expect("source tile array size");
        let mut blend_tiles: Box<[crate::terrain::textures::BlendTileInfo; NUM_BLEND_TILES]> =
            vec![crate::terrain::textures::BlendTileInfo::new(); NUM_BLEND_TILES]
                .into_boxed_slice()
                .try_into()
                .expect("blend tile array size");
        blend_tiles[1].blend_ndx = 4;
        blend_tiles[1].horiz = 1;
        blend_tiles[2].blend_ndx = 8;
        blend_tiles[2].vert = 1;

        let alpha_tiles: [Option<Vec<u8>>; NUM_ALPHA_TILES] = std::array::from_fn(|index| {
            let mut pixel = vec![0, 0, 0, 0];
            if index == K_HORIZ {
                pixel[3] = 255;
            }
            if index == K_VERT {
                pixel[3] = 128;
            }
            Some(pixel)
        });

        let get_raw_tile_data = |tile_ndx: i16, _width: i32, buffer: &mut [u8]| match tile_ndx {
            0 => {
                buffer[..4].copy_from_slice(&[10, 20, 30, 255]);
                true
            }
            4 => {
                buffer[..4].copy_from_slice(&[110, 120, 130, 255]);
                true
            }
            8 => {
                buffer[..4].copy_from_slice(&[200, 10, 10, 255]);
                true
            }
            _ => false,
        };

        let first_only = heightmap
            .get_pointer_to_tile_data(
                0,
                0,
                1,
                &source_tiles,
                &blend_tiles,
                &alpha_tiles,
                &get_raw_tile_data,
            )
            .expect("first-blend compose");

        heightmap.extra_blend_tile_ndxes[0] = 2;
        let with_extra = heightmap
            .get_pointer_to_tile_data(
                0,
                0,
                1,
                &source_tiles,
                &blend_tiles,
                &alpha_tiles,
                &get_raw_tile_data,
            )
            .expect("extra-blend compose");

        assert_ne!(
            &first_only[..4],
            &with_extra[..4],
            "extra_blend_tile_ndxes must change composed atlas pixels"
        );
        assert_eq!(&first_only[..4], &[110, 120, 130, 255]);
        // second overlay: 128 alpha of [200,10,10] over [110,120,130]
        let expected_b = ((200 * 128) / 255 + (110 * 127) / 255) as u8;
        let expected_g = ((10 * 128) / 255 + (120 * 127) / 255) as u8;
        let expected_r = ((10 * 128) / 255 + (130 * 127) / 255) as u8;
        assert_eq!(&with_extra[..4], &[expected_b, expected_g, expected_r, 255]);
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
    fn get_height_at_adds_border_size_like_cpp() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);
        heightmap.border_size = 1;
        heightmap.set_height_at_index(1, 1, 0.5);
        let height = heightmap.get_height_at(0.0, 0.0);
        assert!(
            (height - 50.0).abs() < 0.001,
            "playable (0,0) samples index (border,border); got {height}"
        );
    }

    fn test_heightmap_sampling_includes_exact_map_edges() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);
        heightmap.set_height_at_index(3, 0, 0.25);
        heightmap.set_height_at_index(0, 3, 0.5);
        heightmap.set_height_at_index(3, 3, 0.75);

        assert!((heightmap.get_height_at(3.0, 0.0) - 25.0).abs() < 0.001);
        assert!((heightmap.get_height_at(0.0, 3.0) - 50.0).abs() < 0.001);
        assert!((heightmap.get_height_at(3.0, 3.0) - 75.0).abs() < 0.001);
        // C++ getClipHeight: OOB clamps to the edge cell, never a synthetic 0.
        assert!((heightmap.get_height_at(3.001, 3.0) - 75.0).abs() < 0.001);
        assert!(
            (heightmap.get_height_at(-1.0, -1.0) - heightmap.get_height_at(0.0, 0.0)).abs() < 0.001
        );
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

    #[test]
    fn generate_mesh_subtracts_border_size_like_cpp_adjust_from_index() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 10.0);
        heightmap.border_size = 1;
        heightmap.set_height_at_index(1, 1, 0.5);
        let (vertices, _) = heightmap.generate_mesh(0, 0, 3, 3, 0);
        // Index 0 → world (-border * scale) = -10
        assert_eq!(vertices[0].position[0], -10.0);
        assert_eq!(vertices[0].position[1], -10.0);
        // Index 1,1 → playable origin (0,0); height matches get_height_at(0,0)
        let origin = vertices
            .iter()
            .find(|v| v.position[0] == 0.0 && v.position[1] == 0.0)
            .expect("border-adjusted origin vertex");
        let sampled = heightmap.get_height_at(0.0, 0.0);
        assert!((origin.position[2] - sampled).abs() < 0.001);
    }

    #[test]
    fn generate_mesh_uses_cpp_p0_p2_split_matching_get_height_at() {
        let mut heightmap = HeightMap::new(4, 4, 100.0, 1.0);
        heightmap.set_height_at_index(1, 1, 0.0);
        heightmap.set_height_at_index(2, 1, 0.0);
        heightmap.set_height_at_index(1, 2, 1.0);
        heightmap.set_height_at_index(2, 2, 0.0);

        let (vertices, indices) = heightmap.generate_mesh(1, 1, 2, 2, 0);
        assert_eq!(vertices.len(), 4);
        assert_eq!(indices.len(), 6);
        // p0,p3,p2 then p0,p2,p1
        assert_eq!(indices, vec![0, 2, 3, 0, 3, 1]);

        let sampled = heightmap.get_height_at(1.25, 1.75);
        assert!((sampled - 50.0).abs() < 0.001);

        // Upper triangle p0-p3-p2 contains (1.25, 1.75) because fy > fx.
        let p0 = vertices[0].position;
        let p3 = vertices[2].position;
        let p2 = vertices[3].position;
        let mesh_z = interpolate_z_in_triangle(
            [p0[0], p0[1], p0[2]],
            [p3[0], p3[1], p3[2]],
            [p2[0], p2[1], p2[2]],
            1.25,
            1.75,
        );
        assert!(
            (mesh_z - sampled).abs() < 0.001,
            "mesh plane {mesh_z} must match get_height_at {sampled}"
        );
    }

    #[test]
    fn calculate_bounds_uses_world_height_remap() {
        let mut heightmap = HeightMap::new(2, 2, 80.0, 1.0);
        heightmap.min_height = 20.0;
        heightmap.height_range = 80.0;
        heightmap.set_height_at_index(0, 0, 0.0);
        heightmap.set_height_at_index(1, 1, 1.0);
        let (min, max) = heightmap.calculate_bounds(0, 0, 1, 1);
        assert!((min.z - 20.0).abs() < 0.001);
        assert!((max.z - 100.0).abs() < 0.001);
    }

    fn interpolate_z_in_triangle(a: [f32; 3], b: [f32; 3], c: [f32; 3], x: f32, y: f32) -> f32 {
        let v0x = b[0] - a[0];
        let v0y = b[1] - a[1];
        let v1x = c[0] - a[0];
        let v1y = c[1] - a[1];
        let v2x = x - a[0];
        let v2y = y - a[1];
        let den = v0x * v1y - v1x * v0y;
        let v = (v2x * v1y - v1x * v2y) / den;
        let w = (v0x * v2y - v2x * v0y) / den;
        let u = 1.0 - v - w;
        u * a[2] + v * b[2] + w * c[2]
    }
}
