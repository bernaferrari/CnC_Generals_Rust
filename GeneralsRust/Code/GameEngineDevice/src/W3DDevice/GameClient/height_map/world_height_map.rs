//! World Height Map Module
//!
//! Port of C++ WorldHeightMap.h and WorldHeightMap.cpp
//! Original: GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp (2554 lines)
//! Original: GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/WorldHeightMap.h
//! Author: John Ahlquist, April 2001
//!
//! PARITY_NOTE: This file provides the actual terrain height data storage and queries.
//! It ports the core WorldHeightMap class including height data storage, flip/cliff
//! state bitfields, seismic velocity data, and boundary information.

use super::base_height_map::MAP_HEIGHT_SCALE;

// =============================================================================
// CONSTANTS - Must match C++ WorldHeightMap.h
// =============================================================================

/// Minimum height value (C++ WorldHeightMap.h line 23)
pub const K_MIN_HEIGHT: u8 = 0;

/// Maximum height value (C++ WorldHeightMap.h line 24)
pub const K_MAX_HEIGHT: u8 = 255;

/// Number of source tiles (C++ WorldHeightMap.h line 26)
pub const NUM_SOURCE_TILES: usize = 1024;

/// Number of blend tiles (C++ WorldHeightMap.h line 27)
pub const NUM_BLEND_TILES: usize = 16192;

/// Number of cliff info entries (C++ WorldHeightMap.h line 28)
pub const NUM_CLIFF_INFO: usize = 32384;

/// Number of alpha tiles (C++ WorldHeightMap.h line 69)
pub const NUM_ALPHA_TILES: usize = 12;

/// Number of texture classes (C++ WorldHeightMap.h line 58)
pub const NUM_TEXTURE_CLASSES: usize = 256;

/// Normal draw width (C++ WorldHeightMap.h line 86)
pub const NORMAL_DRAW_WIDTH: i32 = 129;

/// Normal draw height (C++ WorldHeightMap.h line 87)
pub const NORMAL_DRAW_HEIGHT: i32 = 129;

/// Stretch draw width (C++ WorldHeightMap.h line 88)
pub const STRETCH_DRAW_WIDTH: i32 = 65;

/// Stretch draw height (C++ WorldHeightMap.h line 89)
pub const STRETCH_DRAW_HEIGHT: i32 = 65;

/// Flag validation value (C++ WorldHeightMap.h line 29)
pub const FLAG_VAL: u32 = 0x7ADA0000;

/// Tile pixel extent (C++ TileData.h line 29)
pub const TILE_PIXEL_EXTENT: i32 = 64;

/// Texture width in pixels (C++ TileData.h line 39)
pub const TEXTURE_WIDTH: i32 = 2048;

// =============================================================================
// BLEND TILE INFO
// =============================================================================

/// Blend tile information (C++ WorldHeightMap.h TBlendTileInfo / TileData.h line 16-25)
#[derive(Debug, Clone, Copy, Default)]
pub struct TBlendTileInfo {
    pub blend_ndx: i32,
    pub horiz: u8,
    pub vert: u8,
    pub right_diagonal: u8,
    pub left_diagonal: u8,
    pub inverted: u8,
    pub long_diagonal: u8,
    pub custom_blend_edge_class: i32,
}

// =============================================================================
// CLIFF INFO
// =============================================================================

/// Cliff tile UV mapping info (C++ WorldHeightMap.h line 48-56)
#[derive(Debug, Clone, Copy, Default)]
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

// =============================================================================
// TEXTURE CLASS
// =============================================================================

/// Texture class data (C++ WorldHeightMap.h line 36-44)
#[derive(Debug, Clone, Default)]
pub struct TXTextureClass {
    pub global_texture_class: i32,
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub is_blend_edge_tile: bool,
    pub name: String,
    pub position_in_texture: (i32, i32),
}

// =============================================================================
// 2D INTEGER COORDINATE
// =============================================================================

/// 2D integer coordinate (C++ ICoord2D)
#[derive(Debug, Clone, Copy, Default)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

// =============================================================================
// WORLD HEIGHT MAP
// =============================================================================

/// World height map containing terrain height data and metadata.
///
/// Corresponds to C++ WorldHeightMap class (WorldHeightMap.h line 71-297).
/// Stores the raw height data array, flip state, cliff state, seismic data,
/// texture indices, and blend tile information.
///
/// PARITY_NOTE: Height data is stored as Vec<u8> matching C++ UnsignedByte *m_data.
/// Bitfield arrays (flip, cliff, seismic) use the same packed-bit layout as C++.
#[derive(Clone)]
pub struct WorldHeightMap {
    /// Height map width in vertices (C++ m_width)
    width: i32,
    /// Height map height in vertices (C++ m_height)
    height: i32,
    /// Non-playable border area (C++ m_borderSize)
    border_size: i32,
    /// In-game boundaries (C++ m_boundaries)
    boundaries: Vec<ICoord2D>,
    /// Size of m_data array (C++ m_dataSize)
    data_size: i32,
    /// Height values array: z values indexed [y*width+x] (C++ m_data)
    data: Vec<u8>,

    /// Seismic update flags - packed bits (C++ m_seismicUpdateFlag)
    seismic_update_flag: Vec<u8>,
    /// Width of seismic flag array in bytes (C++ m_seismicUpdateWidth)
    seismic_update_width: u32,
    /// Seismic Z velocities per cell (C++ m_seismicZVelocities)
    seismic_z_velocities: Vec<f32>,

    /// Cell flip state bits (C++ m_cellFlipState)
    cell_flip_state: Vec<u8>,
    /// Width of flip state array in bytes (C++ m_flipStateWidth)
    flip_state_width: u32,
    /// Cell cliff state bits (C++ m_cellCliffState)
    cell_cliff_state: Vec<u8>,

    /// Tile indices for each cell (C++ m_tileNdxes)
    tile_ndxes: Vec<i16>,
    /// Blend tile indices for each cell (C++ m_blendTileNdxes)
    blend_tile_ndxes: Vec<i16>,
    /// Cliff info indices for each cell (C++ m_cliffInfoNdxes)
    cliff_info_ndxes: Vec<i16>,
    /// Extra blend tile indices (C++ m_extraBlendTileNdxes)
    extra_blend_tile_ndxes: Vec<i16>,

    /// Number of bitmap tiles (C++ m_numBitmapTiles)
    num_bitmap_tiles: i32,
    /// Number of edge tiles (C++ m_numEdgeTiles)
    num_edge_tiles: i32,
    /// Number of blended tiles (C++ m_numBlendedTiles)
    num_blended_tiles: i32,

    /// Blend tile info array (C++ m_blendedTiles)
    blended_tiles: Vec<TBlendTileInfo>,
    /// Extra blend tile info array (C++ m_extraBlendedTiles)
    extra_blended_tiles: Vec<TBlendTileInfo>,
    /// Cliff info array (C++ m_cliffInfo)
    cliff_info: Vec<TCliffInfo>,
    /// Number of cliff info entries used (C++ m_numCliffInfo)
    num_cliff_info: i32,

    /// Texture classes (C++ m_textureClasses)
    texture_classes: Vec<TXTextureClass>,
    /// Number of texture classes (C++ m_numTextureClasses)
    num_texture_classes: i32,

    /// Drawing info - origin X (C++ m_drawOriginX)
    draw_origin_x: i32,
    /// Drawing info - origin Y (C++ m_drawOriginY)
    draw_origin_y: i32,
    /// Drawing info - width (C++ m_drawWidthX)
    draw_width_x: i32,
    /// Drawing info - height (C++ m_drawHeightY)
    draw_height_y: i32,
}

impl WorldHeightMap {
    /// Create a new empty world height map.
    /// Corresponds to C++ WorldHeightMap::WorldHeightMap() default constructor (line 423-444).
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            border_size: 0,
            boundaries: Vec::new(),
            data_size: 0,
            data: Vec::new(),
            seismic_update_flag: Vec::new(),
            seismic_update_width: 0,
            seismic_z_velocities: Vec::new(),
            cell_flip_state: Vec::new(),
            flip_state_width: 0,
            cell_cliff_state: Vec::new(),
            tile_ndxes: Vec::new(),
            blend_tile_ndxes: Vec::new(),
            cliff_info_ndxes: Vec::new(),
            extra_blend_tile_ndxes: Vec::new(),
            num_bitmap_tiles: 0,
            num_edge_tiles: 0,
            num_blended_tiles: 1,
            blended_tiles: vec![TBlendTileInfo::default(); NUM_BLEND_TILES],
            extra_blended_tiles: vec![TBlendTileInfo::default(); NUM_BLEND_TILES],
            cliff_info: vec![TCliffInfo::default(); NUM_CLIFF_INFO],
            num_cliff_info: 1,
            texture_classes: Vec::with_capacity(NUM_TEXTURE_CLASSES),
            num_texture_classes: 0,
            draw_origin_x: 0,
            draw_origin_y: 0,
            draw_width_x: NORMAL_DRAW_WIDTH,
            draw_height_y: NORMAL_DRAW_HEIGHT,
        }
    }

    /// Create a world height map with specified dimensions.
    /// Corresponds to C++ WorldHeightMap constructor after ParseHeightMapData.
    pub fn with_dimensions(width: i32, height: i32, border_size: i32) -> Self {
        let data_size = width * height;
        let num_bytes_x = ((width as u32) + 7) / 8;
        let num_bytes_y = height as u32;

        let data = vec![0u8; data_size as usize];
        let seismic_update_flag = vec![0u8; num_bytes_x as usize * num_bytes_y as usize];
        let seismic_z_velocities = vec![0.0f32; data_size as usize];
        let cell_flip_state = vec![0u8; num_bytes_x as usize * num_bytes_y as usize];
        let cell_cliff_state = vec![0u8; num_bytes_x as usize * num_bytes_y as usize];
        let tile_ndxes = vec![0i16; data_size as usize];
        let blend_tile_ndxes = vec![0i16; data_size as usize];
        let cliff_info_ndxes = vec![0i16; data_size as usize];
        let extra_blend_tile_ndxes = vec![0i16; data_size as usize];

        let mut boundaries = Vec::new();
        boundaries.push(ICoord2D {
            x: width - 2 * border_size,
            y: height - 2 * border_size,
        });

        Self {
            width,
            height,
            border_size,
            boundaries,
            data_size,
            data,
            seismic_update_flag,
            seismic_update_width: num_bytes_x,
            seismic_z_velocities,
            cell_flip_state,
            flip_state_width: num_bytes_x,
            cell_cliff_state,
            tile_ndxes,
            blend_tile_ndxes,
            cliff_info_ndxes,
            extra_blend_tile_ndxes,
            num_bitmap_tiles: 0,
            num_edge_tiles: 0,
            num_blended_tiles: 1,
            blended_tiles: vec![TBlendTileInfo::default(); NUM_BLEND_TILES],
            extra_blended_tiles: vec![TBlendTileInfo::default(); NUM_BLEND_TILES],
            cliff_info: vec![TCliffInfo::default(); NUM_CLIFF_INFO],
            num_cliff_info: 1,
            texture_classes: Vec::with_capacity(NUM_TEXTURE_CLASSES),
            num_texture_classes: 0,
            draw_origin_x: 0,
            draw_origin_y: 0,
            draw_width_x: NORMAL_DRAW_WIDTH,
            draw_height_y: NORMAL_DRAW_HEIGHT,
        }
    }

    // =========================================================================
    // DIMENSION QUERIES
    // =========================================================================

    pub fn get_x_extent(&self) -> i32 {
        self.width
    }
    pub fn get_y_extent(&self) -> i32 {
        self.height
    }
    pub fn get_border_size(&self) -> i32 {
        self.border_size
    }
    pub fn get_data_size(&self) -> i32 {
        self.data_size
    }
    pub fn get_draw_org_x(&self) -> i32 {
        self.draw_origin_x
    }
    pub fn get_draw_org_y(&self) -> i32 {
        self.draw_origin_y
    }
    pub fn get_draw_width(&self) -> i32 {
        self.draw_width_x
    }
    pub fn get_draw_height(&self) -> i32 {
        self.draw_height_y
    }
    pub fn get_boundaries(&self) -> &[ICoord2D] {
        &self.boundaries
    }

    pub fn set_draw_width(&mut self, width: i32) {
        self.draw_width_x = width.min(self.width);
    }
    pub fn set_draw_height(&mut self, height: i32) {
        self.draw_height_y = height.min(self.height);
    }
    pub fn set_draw_org(&mut self, x_org: i32, y_org: i32) -> bool {
        if x_org >= 0
            && y_org >= 0
            && x_org + self.draw_width_x <= self.width
            && y_org + self.draw_height_y <= self.height
        {
            self.draw_origin_x = x_org;
            self.draw_origin_y = y_org;
            return true;
        }
        false
    }

    pub fn get_num_bitmap_tiles(&self) -> i32 {
        self.num_bitmap_tiles
    }
    pub fn get_num_blended_tiles(&self) -> i32 {
        self.num_blended_tiles
    }
    pub fn get_num_cliff_info(&self) -> i32 {
        self.num_cliff_info
    }

    // =========================================================================
    // STATIC HELPERS
    // =========================================================================

    pub fn get_min_height_value() -> u8 {
        K_MIN_HEIGHT
    }
    pub fn get_max_height_value() -> u8 {
        K_MAX_HEIGHT
    }

    // =========================================================================
    // HEIGHT DATA ACCESS
    // =========================================================================

    /// Get raw height data pointer.
    /// Corresponds to C++ WorldHeightMap::getDataPtr (line 204).
    pub fn get_data_ptr(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable height data pointer.
    pub fn get_data_ptr_mut(&mut self) -> &mut Vec<u8> {
        &mut self.data
    }

    pub fn snapshot_height_data(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn restore_height_data(&mut self, data: &[u8]) -> bool {
        if data.len() != self.data.len() {
            return false;
        }
        self.data.copy_from_slice(data);
        self.init_cliff_flags_from_heights();
        true
    }

    /// Get height at grid position (xIndex, yIndex).
    /// Corresponds to C++ WorldHeightMap::getHeight (line 223-230).
    ///
    /// Bounds-checked: returns 0 for out-of-range indices.
    pub fn get_height(&self, x_index: i32, y_index: i32) -> u8 {
        let ndx = (y_index * self.width + x_index) as i32;
        if ndx >= 0 && ndx < self.data_size && !self.data.is_empty() {
            self.data[ndx as usize]
        } else {
            0
        }
    }

    pub fn get_height_lod(&self, x_index: i32, y_index: i32, lod: u32) -> u8 {
        let lod = lod.max(1) as i32;
        let sample_x = ((x_index / lod) * lod).clamp(0, self.width.saturating_sub(1));
        let sample_y = ((y_index / lod) * lod).clamp(0, self.height.saturating_sub(1));
        self.get_height(sample_x, sample_y)
    }

    /// Get display height at grid position with draw offset applied.
    /// Corresponds to C++ WorldHeightMap::getDisplayHeight (line 220).
    pub fn get_display_height(&self, x: i32, y: i32) -> u8 {
        let idx = (x + self.draw_origin_x + self.width * (y + self.draw_origin_y)) as usize;
        if idx < self.data.len() {
            self.data[idx]
        } else {
            0
        }
    }

    /// Set raw height at grid position.
    /// Corresponds to C++ WorldHeightMap::setRawHeight (line 286-289).
    pub fn set_raw_height(&mut self, x_index: i32, y_index: i32, height: u8) {
        let ndx = (y_index * self.width + x_index) as i32;
        if ndx >= 0 && ndx < self.data_size {
            self.data[ndx as usize] = height;
        }
    }

    // =========================================================================
    // FLIP STATE
    // =========================================================================

    /// Get the triangle flip state for a cell.
    /// Corresponds to C++ WorldHeightMap::getFlipState (line 541-548).
    pub fn get_flip_state(&self, x_index: i32, y_index: i32) -> bool {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return false;
        }
        if self.cell_flip_state.is_empty() {
            return false;
        }
        let byte_idx = (y_index * self.flip_state_width as i32 + (x_index >> 3)) as usize;
        if byte_idx >= self.cell_flip_state.len() {
            return false;
        }
        (self.cell_flip_state[byte_idx] & (1 << (x_index & 0x7))) != 0
    }

    /// Optimized flip state query without bounds checks.
    /// Corresponds to C++ WorldHeightMap::getQuickFlipState (line 251-254).
    pub fn get_quick_flip_state(&self, x_index: i32, y_index: i32) -> bool {
        let byte_idx =
            (y_index as usize) * (self.flip_state_width as usize) + ((x_index >> 3) as usize);
        (self.cell_flip_state[byte_idx] & (1 << (x_index & 0x7))) != 0
    }

    /// Set the flip state for a cell.
    /// Corresponds to C++ WorldHeightMap::setFlipState (line 552-564).
    pub fn set_flip_state(&mut self, x_index: i32, y_index: i32, value: bool) {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return;
        }
        if self.cell_flip_state.is_empty() {
            return;
        }
        let byte_idx = (y_index * self.flip_state_width as i32 + (x_index >> 3)) as usize;
        if byte_idx >= self.cell_flip_state.len() {
            return;
        }
        if value {
            self.cell_flip_state[byte_idx] |= 1 << (x_index & 0x7);
        } else {
            self.cell_flip_state[byte_idx] &= !(1 << (x_index & 0x7));
        }
    }

    /// Clear all flip state bits.
    /// Corresponds to C++ WorldHeightMap::clearFlipStates (line 568-572).
    pub fn clear_flip_states(&mut self) {
        for b in self.cell_flip_state.iter_mut() {
            *b = 0;
        }
    }

    // =========================================================================
    // CLIFF STATE
    // =========================================================================

    /// Get whether a cell is a cliff cell.
    /// Corresponds to C++ WorldHeightMap::getCliffState (line 709-716).
    pub fn get_cliff_state(&self, x_index: i32, y_index: i32) -> bool {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return false;
        }
        if self.cell_cliff_state.is_empty() {
            return false;
        }
        let byte_idx = (y_index * self.flip_state_width as i32 + (x_index >> 3)) as usize;
        if byte_idx >= self.cell_cliff_state.len() {
            return false;
        }
        (self.cell_cliff_state[byte_idx] & (1 << (x_index & 0x7))) != 0
    }

    /// Set the cliff state for a cell.
    /// Corresponds to C++ WorldHeightMap::setCliffState (line 723-737).
    pub fn set_cliff_state(&mut self, x_index: i32, y_index: i32, state: bool) {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return;
        }
        if self.cell_cliff_state.is_empty() {
            return;
        }
        let byte_idx = (y_index * self.flip_state_width as i32 + (x_index >> 3)) as usize;
        if byte_idx >= self.cell_cliff_state.len() {
            return;
        }
        if state {
            self.cell_cliff_state[byte_idx] |= 1 << (x_index & 0x7);
        } else {
            self.cell_cliff_state[byte_idx] &= !(1 << (x_index & 0x7));
        }
    }

    // =========================================================================
    // SEISMIC DATA
    // =========================================================================

    /// Get seismic update flag.
    /// Corresponds to C++ WorldHeightMap::getSeismicUpdateFlag (line 578-585).
    pub fn get_seismic_update_flag(&self, x_index: i32, y_index: i32) -> bool {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return false;
        }
        if self.seismic_update_flag.is_empty() {
            return false;
        }
        let byte_idx = (y_index * self.seismic_update_width as i32 + (x_index >> 3)) as usize;
        if byte_idx >= self.seismic_update_flag.len() {
            return false;
        }
        (self.seismic_update_flag[byte_idx] & (1 << (x_index & 0x7))) != 0
    }

    /// Set seismic update flag.
    /// Corresponds to C++ WorldHeightMap::setSeismicUpdateFlag (line 586-598).
    pub fn set_seismic_update_flag(&mut self, x_index: i32, y_index: i32, value: bool) {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return;
        }
        if self.seismic_update_flag.is_empty() {
            return;
        }
        let byte_idx = (y_index * self.seismic_update_width as i32 + (x_index >> 3)) as usize;
        if byte_idx >= self.seismic_update_flag.len() {
            return;
        }
        if value {
            self.seismic_update_flag[byte_idx] |= 1 << (x_index & 0x7);
        } else {
            self.seismic_update_flag[byte_idx] &= !(1 << (x_index & 0x7));
        }
    }

    /// Clear all seismic update flags.
    /// Corresponds to C++ WorldHeightMap::clearSeismicUpdateFlags (line 599-604).
    pub fn clear_seismic_update_flags(&mut self) {
        for b in self.seismic_update_flag.iter_mut() {
            *b = 0;
        }
    }

    /// Get seismic Z velocity at a cell.
    /// Corresponds to C++ WorldHeightMap::getSeismicZVelocity (line 607-614).
    pub fn get_seismic_z_velocity(&self, x_index: i32, y_index: i32) -> f32 {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return 0.0;
        }
        if self.seismic_z_velocities.is_empty() {
            return 0.0;
        }
        let idx = (y_index * self.width + x_index) as usize;
        if idx < self.seismic_z_velocities.len() {
            self.seismic_z_velocities[idx]
        } else {
            0.0
        }
    }

    /// Set seismic Z velocity at a cell.
    /// Corresponds to C++ WorldHeightMap::setSeismicZVelocity (line 615-622).
    pub fn set_seismic_z_velocity(&mut self, x_index: i32, y_index: i32, value: f32) {
        if x_index < 0 || y_index < 0 || y_index >= self.height || x_index >= self.width {
            return;
        }
        if self.seismic_z_velocities.is_empty() {
            return;
        }
        let idx = (y_index * self.width + x_index) as usize;
        if idx < self.seismic_z_velocities.len() {
            self.seismic_z_velocities[idx] = value;
        }
    }

    /// Fill all seismic Z velocities with a value.
    /// Corresponds to C++ WorldHeightMap::fillSeismicZVelocities (line 623-628).
    pub fn fill_seismic_z_velocities(&mut self, value: f32) {
        for v in self.seismic_z_velocities.iter_mut() {
            *v = value;
        }
    }

    /// Get bilinear-sampled seismic Z velocity.
    /// Corresponds to C++ WorldHeightMap::getBilinearSampleSeismicZVelocity (line 630-690).
    ///
    /// Samples the 3x3 neighborhood around (x, y) and averages them.
    pub fn get_bilinear_sample_seismic_z_velocity(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || y >= self.height || x >= self.width {
            return 0.0;
        }
        if self.seismic_z_velocities.is_empty() {
            return 0.0;
        }

        let w = self.width as i32;
        let mut collector = 0.0f32;
        let mut divisor = 0.0f32;

        let sample = |xi: i32, yi: i32, v: &mut f32, d: &mut f32| {
            if xi >= 0 && xi < self.width && yi >= 0 && yi < self.height {
                let idx = (yi * w + xi) as usize;
                *v += self.seismic_z_velocities[idx];
                *d += 1.0;
            }
        };

        sample(x, y, &mut collector, &mut divisor);

        if y > 0 {
            sample(x, y - 1, &mut collector, &mut divisor);
            sample(x - 1, y - 1, &mut collector, &mut divisor);
            sample(x + 1, y - 1, &mut collector, &mut divisor);
        }
        if y < self.height - 1 {
            sample(x, y + 1, &mut collector, &mut divisor);
            sample(x - 1, y + 1, &mut collector, &mut divisor);
            sample(x + 1, y + 1, &mut collector, &mut divisor);
        }
        sample(x - 1, y, &mut collector, &mut divisor);
        sample(x + 1, y, &mut collector, &mut divisor);

        if divisor > 0.0 {
            collector / divisor
        } else {
            0.0
        }
    }

    // =========================================================================
    // TILE INDEX ACCESS
    // =========================================================================

    pub fn get_tile_ndx(&self, x_index: i32, y_index: i32) -> i16 {
        let idx = (y_index * self.width + x_index) as usize;
        if idx < self.tile_ndxes.len() {
            self.tile_ndxes[idx]
        } else {
            0
        }
    }
    pub fn get_blend_tile_ndx(&self, x_index: i32, y_index: i32) -> i16 {
        let idx = (y_index * self.width + x_index) as usize;
        if idx < self.blend_tile_ndxes.len() {
            self.blend_tile_ndxes[idx]
        } else {
            0
        }
    }
    pub fn get_cliff_info_ndx(&self, x_index: i32, y_index: i32) -> i16 {
        let idx = (y_index * self.width + x_index) as usize;
        if idx < self.cliff_info_ndxes.len() {
            self.cliff_info_ndxes[idx]
        } else {
            0
        }
    }

    // =========================================================================
    // CRATER / FLATTEN MODIFICATION
    // =========================================================================

    /// Create a crater by modifying height values within a radius.
    /// This modifies the height data in a circular area centered at (cx, cy)
    /// with the given radius, creating a depression.
    pub fn create_crater(&mut self, cx: f32, cy: f32, radius: f32, depth: f32) {
        let border = self.border_size as f32;
        let grid_cx = (cx / super::base_height_map::MAP_XY_FACTOR) + border;
        let grid_cy = (cy / super::base_height_map::MAP_XY_FACTOR) + border;
        let grid_radius = (radius / super::base_height_map::MAP_XY_FACTOR).ceil() as i32;

        let min_x = (grid_cx as i32 - grid_radius).max(0);
        let max_x = (grid_cx as i32 + grid_radius).min(self.width - 1);
        let min_y = (grid_cy as i32 - grid_radius).max(0);
        let max_y = (grid_cy as i32 + grid_radius).min(self.height - 1);

        for gy in min_y..=max_y {
            for gx in min_x..=max_x {
                let dx = gx as f32 - grid_cx;
                let dy = gy as f32 - grid_cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= grid_radius as f32 {
                    let falloff = 1.0 - (dist / grid_radius as f32);
                    let current = self.get_height(gx, gy) as f32;
                    let new_height = (current - depth * falloff).max(0.0).min(255.0) as u8;
                    self.set_raw_height(gx, gy, new_height);
                }
            }
        }
    }

    /// Flatten terrain within a rectangular area for building placement.
    /// Sets all heights in the area to the average height.
    pub fn flatten_area(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        let min_x = x0.max(0).min(self.width - 1);
        let max_x = x1.max(0).min(self.width - 1);
        let min_y = y0.max(0).min(self.height - 1);
        let max_y = y1.max(0).min(self.height - 1);

        let mut sum: u32 = 0;
        let mut count: u32 = 0;
        for gy in min_y..=max_y {
            for gx in min_x..=max_x {
                sum += self.get_height(gx, gy) as u32;
                count += 1;
            }
        }
        let avg = if count > 0 { (sum / count) as u8 } else { 0 };

        for gy in min_y..=max_y {
            for gx in min_x..=max_x {
                self.set_raw_height(gx, gy, avg);
            }
        }
    }

    // =========================================================================
    // CLIFF DETECTION FROM HEIGHTS
    // =========================================================================

    /// Initialize cliff flags from height data.
    /// Corresponds to C++ WorldHeightMap::initCliffFlagsFromHeights.
    /// Uses PATHFIND_CLIFF_SLOPE_LIMIT_F to determine if a cell is a cliff.
    pub fn init_cliff_flags_from_heights(&mut self) {
        let slope_limit = super::base_height_map::PATHFIND_CLIFF_SLOPE_LIMIT_F;
        for y in 0..self.height - 1 {
            for x in 0..self.width - 1 {
                self.set_cell_cliff_flag_from_heights(x, y, slope_limit);
            }
        }
    }

    /// Set cliff flag for a single cell based on neighboring heights.
    /// Corresponds to C++ WorldHeightMap::setCellCliffFlagFromHeights.
    fn set_cell_cliff_flag_from_heights(&mut self, x_index: i32, y_index: i32, slope_limit: f32) {
        let h00 = self.get_height(x_index, y_index) as f32 * MAP_HEIGHT_SCALE;
        let h10 = self.get_height(x_index + 1, y_index) as f32 * MAP_HEIGHT_SCALE;
        let h01 = self.get_height(x_index, y_index + 1) as f32 * MAP_HEIGHT_SCALE;
        let h11 = self.get_height(x_index + 1, y_index + 1) as f32 * MAP_HEIGHT_SCALE;

        let min_h = h00.min(h10).min(h01).min(h11);
        let max_h = h00.max(h10).max(h01).max(h11);
        let is_cliff = max_h - min_h > slope_limit;

        self.set_cliff_state(x_index, y_index, is_cliff);
    }

    // =========================================================================
    // TEXTURE CLASS ACCESS
    // =========================================================================

    pub fn get_texture_classes(&self) -> &[TXTextureClass] {
        &self.texture_classes
    }
    pub fn get_num_texture_classes(&self) -> i32 {
        self.num_texture_classes
    }

    pub fn add_texture_class(&mut self, tex_class: TXTextureClass) {
        if (self.num_texture_classes as usize) < NUM_TEXTURE_CLASSES {
            self.texture_classes.push(tex_class);
            self.num_texture_classes += 1;
        }
    }

    pub fn to_height_data(&self) -> Vec<u8> {
        self.data.clone()
    }
}

impl Default for WorldHeightMap {
    fn default() -> Self {
        Self::new()
    }
}
