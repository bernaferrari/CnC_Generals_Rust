//! Terrain Texture Module
//!
//! Port of C++ TerrainTex.cpp and TerrainTex.h
//! Original: GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/TerrainTex.cpp (1,111 lines)
//! Original: GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/TerrainTex.h
//! Author: John Ahlquist, April 2001
//!
//! This module provides texture classes for terrain rendering, including:
//! - TerrainTextureClass: Base terrain texture with tiling and mip-mapping
//! - AlphaTerrainTextureClass: Alpha-blended terrain overlay
//! - AlphaEdgeTextureClass: Edge blending between tiles
//! - LightMapTerrainTextureClass: Terrain lighting map
//! - CloudMapTerrainTextureClass: Animated cloud shadows
//! - ScorchTextureClass: Scorch marks and damage

use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use wgpu::{Device, Queue, Sampler, Texture, TextureView};

// =============================================================================
// CONSTANTS - Must match C++ TerrainTex.cpp exactly
// =============================================================================

/// Texture width in pixels (C++ line 39)
pub const TEXTURE_WIDTH: u32 = 2048;

/// Tile offset for border duplication (C++ TerrainTex.h line 17)
pub const TILE_OFFSET: u32 = 8;

/// Tile pixel extent (64x64 pixels per tile) (C++ line 79)
pub const TILE_PIXEL_EXTENT: u32 = 64;

/// Bytes per pixel for tile data (BGRA format) (C++ line 108)
pub const TILE_BYTES_PER_PIXEL: u32 = 4;

/// Stretch factor for light/cloud maps (C++ line 626)
/// Covers 63/2 tiles = 31.5 tiles
pub const STRETCH_FACTOR: f32 = 1.0 / (63.0 * MAP_XY_FACTOR / 2.0);

/// Map coordinate scaling factor
pub const MAP_XY_FACTOR: f32 = 2.0;

// =============================================================================
// MIP LEVEL CONSTANTS - Match WW3D2 MipCountType
// =============================================================================

/// All mip levels
pub const MIP_LEVELS_ALL: u32 = 0;

/// 3 mip levels
pub const MIP_LEVELS_3: u32 = 3;

/// 1 mip level (no mipmapping)
pub const MIP_LEVELS_1: u32 = 1;

// =============================================================================
// TEXTURE FORMATS - Match D3DFORMAT from C++
// =============================================================================

/// 16-bit texture format: A1R5G5B5 (C++ line 40, 51)
/// 1-bit alpha, 5-bit red, 5-bit green, 5-bit blue
pub const WW3D_FORMAT_A1R5G5B5: u32 = 0;

/// 32-bit texture format: A8R8G8B8 (C++ line 727)
/// 8-bit alpha, 8-bit red, 8-bit green, 8-bit blue
pub const WW3D_FORMAT_A8R8G8B8: u32 = 1;

/// Unknown/default format
pub const WW3D_FORMAT_UNKNOWN: u32 = 0xFFFFFFFF;

// =============================================================================
// HELPER STRUCTURES
// =============================================================================

/// 2D integer coordinate (matches ICoord2D from C++)
#[derive(Debug, Clone, Copy, Default)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Tile data structure (simplified from C++ TileData)
#[derive(Debug, Clone)]
pub struct TileData {
    /// Location of this tile in the texture atlas
    pub tile_location_in_texture: ICoord2D,
    /// RGB bitmap data for this tile
    rgb_data: Vec<u8>,
    /// Tile width in pixels
    width: u32,
}

impl TileData {
    /// Construct tile data from raw bytes.
    pub fn new(width: u32, tile_location_in_texture: ICoord2D, rgb_data: Vec<u8>) -> Self {
        Self {
            tile_location_in_texture,
            rgb_data,
            width,
        }
    }

    /// Get pointer to RGB data for a specific width
    /// Matches C++ TileData::getRGBDataForWidth()
    pub fn get_rgb_data_for_width(&self, _width: u32) -> &[u8] {
        &self.rgb_data
    }

    /// Get mutable pointer to RGB data
    pub fn get_rgb_data_mut(&mut self) -> &mut [u8] {
        &mut self.rgb_data
    }
}

/// Texture class data (simplified from C++ TextureClass)
#[derive(Debug, Clone, Copy)]
pub struct TextureClassData {
    /// Width in tiles
    pub width: u32,
    /// Position in texture atlas
    pub position_in_texture: ICoord2D,
}

impl TextureClassData {
    pub fn new(width: u32, position_in_texture: ICoord2D) -> Self {
        Self {
            width,
            position_in_texture,
        }
    }
}

/// World height map (simplified from C++ WorldHeightMap)
pub struct WorldHeightMap {
    /// Number of bitmap tiles
    pub num_bitmap_tiles: usize,
    /// Number of edge tiles
    pub num_edge_tiles: usize,
    /// Number of texture classes
    pub num_texture_classes: usize,
    /// Source tiles
    source_tiles: Vec<Option<TileData>>,
    /// Edge tiles
    edge_tiles: Vec<Option<TileData>>,
    /// Texture class data
    pub texture_classes: Vec<TextureClassData>,
}

impl WorldHeightMap {
    /// Create a height map container with pre-sized tile arrays.
    pub fn new(num_bitmap_tiles: usize, num_edge_tiles: usize, num_texture_classes: usize) -> Self {
        Self {
            num_bitmap_tiles,
            num_edge_tiles,
            num_texture_classes,
            source_tiles: vec![None; num_bitmap_tiles],
            edge_tiles: vec![None; num_edge_tiles],
            texture_classes: Vec::with_capacity(num_texture_classes),
        }
    }

    /// Store a source tile at a fixed index.
    pub fn set_source_tile(&mut self, index: usize, tile: TileData) {
        if index >= self.source_tiles.len() {
            self.source_tiles.resize_with(index + 1, || None);
        }
        self.source_tiles[index] = Some(tile);
        self.num_bitmap_tiles = self.source_tiles.len();
    }

    /// Store an edge tile at a fixed index.
    pub fn set_edge_tile(&mut self, index: usize, tile: TileData) {
        if index >= self.edge_tiles.len() {
            self.edge_tiles.resize_with(index + 1, || None);
        }
        self.edge_tiles[index] = Some(tile);
        self.num_edge_tiles = self.edge_tiles.len();
    }

    /// Add texture-class metadata.
    pub fn add_texture_class(&mut self, class: TextureClassData) {
        self.texture_classes.push(class);
        self.num_texture_classes = self.texture_classes.len();
    }

    /// Get source tile by index (C++ line 100)
    pub fn get_source_tile(&self, index: usize) -> Option<&TileData> {
        self.source_tiles.get(index).and_then(|t| t.as_ref())
    }

    /// Get edge tile by index (C++ line 771)
    pub fn get_edge_tile(&self, index: usize) -> Option<&TileData> {
        self.edge_tiles.get(index).and_then(|t| t.as_ref())
    }

    fn find_tile_data_for_cell(
        &self,
        x_cell: i32,
        y_cell: i32,
        pixels_per_cell: u32,
    ) -> Option<&TileData> {
        if x_cell < 0 || y_cell < 0 || pixels_per_cell == 0 {
            return None;
        }

        let pixels_per_cell = pixels_per_cell as i32;
        let requested_pixel_x = x_cell.saturating_mul(pixels_per_cell);
        let requested_pixel_y = y_cell.saturating_mul(pixels_per_cell);

        let mut fallback: Option<&TileData> = None;
        let mut seen_tiles = 0usize;

        for tile in self.source_tiles.iter().flatten() {
            seen_tiles += 1;
            let pos = tile.tile_location_in_texture;
            let tile_pixel_extent = tile.width as i32;
            if tile_pixel_extent <= 0 {
                continue;
            }

            let tile_cell_extent = (tile_pixel_extent / pixels_per_cell).max(1);
            let tile_cell_x = pos.x / pixels_per_cell;
            let tile_cell_y = pos.y / pixels_per_cell;

            if (pos.x == requested_pixel_x && pos.y == requested_pixel_y)
                || (x_cell >= tile_cell_x
                    && x_cell < tile_cell_x + tile_cell_extent
                    && y_cell >= tile_cell_y
                    && y_cell < tile_cell_y + tile_cell_extent)
            {
                return Some(tile);
            }

            if fallback.is_none() {
                fallback = Some(tile);
            }
        }

        if seen_tiles == 1 {
            fallback
        } else {
            None
        }
    }

    fn tile_data_for_width(tile: &TileData, width: u32) -> Vec<u8> {
        let src_width = tile.width.max(1);
        let src_len = (src_width * src_width * TILE_BYTES_PER_PIXEL) as usize;
        let src_data = if tile.rgb_data.len() >= src_len {
            &tile.rgb_data[..src_len]
        } else {
            tile.rgb_data.as_slice()
        };

        if width == 0 {
            return Vec::new();
        }

        if src_width == width {
            return src_data.to_vec();
        }

        let mut out = vec![0u8; (width * width * TILE_BYTES_PER_PIXEL) as usize];
        for y in 0..width {
            let src_y = ((y as u64 * src_width as u64) / width as u64) as u32;
            for x in 0..width {
                let src_x = ((x as u64 * src_width as u64) / width as u64) as u32;
                let src_idx = ((src_y * src_width + src_x) * TILE_BYTES_PER_PIXEL) as usize;
                let dst_idx = ((y * width + x) * TILE_BYTES_PER_PIXEL) as usize;
                if src_idx + 3 < src_data.len() {
                    out[dst_idx..dst_idx + 4].copy_from_slice(&src_data[src_idx..src_idx + 4]);
                }
            }
        }

        out
    }

    /// Get pointer to tile data at cell coordinates (C++ line 377)
    pub fn get_pointer_to_tile_data(
        &self,
        x_cell: i32,
        y_cell: i32,
        pixels_per_cell: u32,
    ) -> Option<&[u8]> {
        self.find_tile_data_for_cell(x_cell, y_cell, pixels_per_cell)
            .map(|tile| tile.get_rgb_data_for_width(pixels_per_cell))
    }
}

/// Global data (simplified from C++ GlobalData)
pub struct GlobalData {
    /// Texture reduction factor (LOD offset)
    pub texture_reduction_factor: u32,
    /// Enable bilinear filtering on terrain
    pub bilinear_terrain_tex: bool,
    /// Enable trilinear filtering on terrain
    pub trilinear_terrain_tex: bool,
    /// Use multi-pass terrain rendering
    pub multi_pass_terrain: bool,
}

// Global instance (would be properly initialized in real code)
static THE_GLOBAL_DATA: OnceLock<RwLock<GlobalData>> = OnceLock::new();

pub fn get_global_data() -> Option<&'static RwLock<GlobalData>> {
    THE_GLOBAL_DATA.get()
}

// =============================================================================
// BASE TEXTURE CLASS
// =============================================================================

/// Base texture class (port of C++ TextureClass)
pub struct TextureClass {
    /// WGPU texture handle
    d3d_texture: Option<Arc<Texture>>,
    /// Texture view
    texture_view: Option<Arc<TextureView>>,
    /// Sampler
    sampler: Option<Arc<Sampler>>,
    /// Texture width
    width: u32,
    /// Texture height
    height: u32,
    /// Texture format
    format: u32,
    /// Mip level count
    mip_levels: u32,
    /// Current LOD level
    current_lod: u32,
    /// Texture name
    name: String,
    /// Last state requested by `apply()`.
    apply_state: Mutex<TextureApplyState>,
}

impl TextureClass {
    /// Create new texture with dimensions and format
    pub fn new(width: u32, height: u32, format: u32, mip_levels: u32) -> Self {
        Self {
            d3d_texture: None,
            texture_view: None,
            sampler: None,
            width,
            height,
            format,
            mip_levels,
            current_lod: 0,
            name: String::new(),
            apply_state: Mutex::new(TextureApplyState::default()),
        }
    }

    /// Create texture from file
    pub fn from_file(name: &str, path: &str, mip_levels: u32) -> Self {
        Self {
            d3d_texture: None,
            texture_view: None,
            sampler: None,
            width: 0,
            height: 0,
            format: WW3D_FORMAT_UNKNOWN,
            mip_levels,
            current_lod: 0,
            name: name.to_string(),
            apply_state: Mutex::new(TextureApplyState::default()),
        }
    }

    /// Get D3D texture (C++ Peek_D3D_Texture())
    pub fn peek_d3d_texture(&self) -> Option<&Arc<Texture>> {
        self.d3d_texture.as_ref()
    }

    /// Set D3D texture (C++ Set_D3D_Base_Texture())
    pub fn set_d3d_base_texture(&mut self, texture: Arc<Texture>) {
        self.d3d_texture = Some(texture);
    }

    /// Apply texture to rendering pipeline (virtual function in C++)
    /// To be overridden by subclasses
    pub fn apply(&self, stage: u32) {
        let mut state = self
            .apply_state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        state.stage = stage;
    }

    fn reset_apply_state(&self) {
        let mut state = self
            .apply_state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        state.stage = 0;
    }

    /// Set LOD level (C++ line 332)
    pub fn set_lod(&mut self, lod: u32) {
        self.current_lod = lod;
        // Would call actual texture LOD API here
    }

    /// Get filter settings (simplified)
    pub fn get_filter(&self) -> TextureFilter {
        self.apply_state
            .lock()
            .map(|state| state.filter)
            .unwrap_or_default()
    }

    fn set_apply_state(&self, stage: u32, filter: TextureFilter) {
        let mut state = self
            .apply_state
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        state.stage = stage;
        state.filter = filter;
    }
}

/// Texture filter settings (simplified from C++ TextureFilterClass)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextureFilter {
    min_filter: FilterType,
    mag_filter: FilterType,
    mip_filter: FilterType,
    u_addr_mode: AddressMode,
    v_addr_mode: AddressMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    Point,
    Fast,
    Linear,
    Best,
}

impl Default for FilterType {
    fn default() -> Self {
        FilterType::Point
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressMode {
    Wrap,
    Clamp,
    Repeat,
}

impl Default for AddressMode {
    fn default() -> Self {
        AddressMode::Wrap
    }
}

impl TextureFilter {
    pub fn set_min_filter(&mut self, filter: FilterType) {
        self.min_filter = filter;
    }

    pub fn set_mag_filter(&mut self, filter: FilterType) {
        self.mag_filter = filter;
    }

    pub fn set_mip_mapping(&mut self, filter: FilterType) {
        self.mip_filter = filter;
    }

    pub fn set_u_addr_mode(&mut self, mode: AddressMode) {
        self.u_addr_mode = mode;
    }

    pub fn set_v_addr_mode(&mut self, mode: AddressMode) {
        self.v_addr_mode = mode;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TextureApplyState {
    stage: u32,
    filter: TextureFilter,
}

// =============================================================================
// TERRAINTEXTURECLASS - Main terrain texture
// C++ lines 28-170
// =============================================================================

/// Terrain texture class
/// Port of C++ TerrainTextureClass (lines 28-170)
pub struct TerrainTextureClass {
    /// Base texture functionality
    base: TextureClass,
}

impl TerrainTextureClass {
    /// Constructor - creates 16-bit A1R5G5B5 texture
    /// C++ line 38-42
    pub fn new(height: u32) -> Self {
        Self {
            base: TextureClass::new(TEXTURE_WIDTH, height, WW3D_FORMAT_A1R5G5B5, MIP_LEVELS_3),
        }
    }

    /// Constructor with custom width
    /// C++ line 50-54
    pub fn new_with_width(height: u32, width: u32) -> Self {
        Self {
            base: TextureClass::new(width, height, WW3D_FORMAT_A1R5G5B5, MIP_LEVELS_ALL),
        }
    }

    /// Update texture from height map
    /// C++ lines 64-170
    ///
    /// Sets tile bitmap data with 4-pixel borders to prevent seams during
    /// bilinear interpolation. Returns actual texture height.
    pub fn update(&mut self, ht_map: &WorldHeightMap, device: &Device, queue: &Queue) -> u32 {
        // In real implementation, would:
        // 1. Lock texture surface (D3D) or get staging buffer (WGPU)
        // 2. Write tile data with pixel format conversion
        // 3. Add 4-pixel borders by duplicating edges
        // 4. Generate mipmaps
        // 5. Set LOD if texture reduction enabled

        // Simplified for now - would need actual WGPU texture writing
        let surface_width = self.base.width;
        let surface_height = self.base.height;

        if surface_width < TEXTURE_WIDTH {
            return 0;
        }

        // Calculate tiles per row (C++ line 80)
        let tiles_per_row = surface_width / (2 * TILE_PIXEL_EXTENT + TILE_OFFSET);
        let tiles_per_row = tiles_per_row * 2;

        // Allocate staging buffer for texture data
        let pixel_bytes = 2; // A1R5G5B5 format
        let buffer_size = (surface_width * surface_height * pixel_bytes) as usize;
        let mut pixel_data = vec![0u8; buffer_size];

        // Copy tile data with BGR24 -> A1R5G5B5 conversion (C++ lines 97-121)
        for tile_ndx in 0..ht_map.num_bitmap_tiles {
            if let Some(tile) = ht_map.get_source_tile(tile_ndx) {
                let position = tile.tile_location_in_texture;
                if position.x <= 0 {
                    continue; // All real tile offsets start at 2
                }

                // Copy pixels with format conversion
                for j in 0..TILE_PIXEL_EXTENT {
                    let bgr_data = tile.get_rgb_data_for_width(TILE_PIXEL_EXTENT);
                    let src_row_offset = ((TILE_PIXEL_EXTENT - 1 - j)
                        * TILE_BYTES_PER_PIXEL
                        * TILE_PIXEL_EXTENT) as usize;

                    let row = (position.y as u32 + j) as usize;
                    let dst_row_offset = row * surface_width as usize * pixel_bytes as usize;

                    let column = position.x as usize;
                    let dst_offset = dst_row_offset + column * pixel_bytes as usize;

                    for i in 0..TILE_PIXEL_EXTENT as usize {
                        let src_idx = src_row_offset + i * TILE_BYTES_PER_PIXEL as usize;
                        let dst_idx = dst_offset + i * pixel_bytes as usize;

                        // BGR24 to A1R5G5B5 conversion (C++ line 116)
                        // Format: 0x8000 + ((b>>3)<<10) + ((g>>3)<<5) + (r>>3)
                        let b = bgr_data[src_idx + 2];
                        let g = bgr_data[src_idx + 1];
                        let r = bgr_data[src_idx + 0];

                        let pixel_16bit: u16 = 0x8000
                            + (((b >> 3) as u16) << 10)
                            + (((g >> 3) as u16) << 5)
                            + ((r >> 3) as u16);

                        // Write as little-endian u16
                        pixel_data[dst_idx] = (pixel_16bit & 0xFF) as u8;
                        pixel_data[dst_idx + 1] = ((pixel_16bit >> 8) & 0xFF) as u8;
                    }
                }
            }
        }

        // Draw 4-pixel borders around each texture class (C++ lines 122-160)
        for tex_class_idx in 0..ht_map.num_texture_classes {
            let tex_class = &ht_map.texture_classes[tex_class_idx];
            let width = tex_class.width * TILE_PIXEL_EXTENT;
            let origin = tex_class.position_in_texture;

            if origin.x <= 0 {
                continue;
            }

            // Duplicate 4 columns of pixels before and after (C++ lines 129-142)
            for j in 0..width {
                let row = (origin.y as u32 + j) as usize;
                let row_offset = row * surface_width as usize * pixel_bytes as usize;
                let column = origin.x as usize;

                // Copy before (wrap from end)
                for k in 0..4 {
                    let src_idx =
                        row_offset + (column + width as usize - 4 + k) * pixel_bytes as usize;
                    let dst_idx = row_offset + (column - 4 + k) * pixel_bytes as usize;
                    if dst_idx < pixel_data.len() && src_idx < pixel_data.len() {
                        pixel_data[dst_idx] = pixel_data[src_idx];
                        pixel_data[dst_idx + 1] = pixel_data[src_idx + 1];
                    }
                }

                // Copy after (wrap from start)
                for k in 0..4 {
                    let src_idx = row_offset + (column + k) * pixel_bytes as usize;
                    let dst_idx = row_offset + (column + width as usize + k) * pixel_bytes as usize;
                    if dst_idx < pixel_data.len() && src_idx < pixel_data.len() {
                        pixel_data[dst_idx] = pixel_data[src_idx];
                        pixel_data[dst_idx + 1] = pixel_data[src_idx + 1];
                    }
                }
            }

            // Duplicate 4 rows of pixels before and after (C++ lines 144-158)
            let row_bytes = surface_width as usize * pixel_bytes as usize;
            for j in 0..4 {
                // Copy before (from bottom of tile area)
                let src_row = (origin.y as usize + width as usize - 1) * row_bytes;
                let dst_row = ((origin.y as i32 - j as i32 - 1) as usize) * row_bytes;
                let copy_start = (origin.x as usize - 4) * pixel_bytes as usize;
                let copy_len = (width as usize + 8) * pixel_bytes as usize;

                if dst_row + copy_start + copy_len <= pixel_data.len()
                    && src_row + copy_start + copy_len <= pixel_data.len()
                {
                    pixel_data.copy_within(
                        src_row + copy_start..src_row + copy_start + copy_len,
                        dst_row + copy_start,
                    );
                }

                // Copy after (from top of tile area)
                let src_row = origin.y as usize * row_bytes;
                let dst_row = (origin.y as usize + width as usize + j) * row_bytes;

                if dst_row + copy_start + copy_len <= pixel_data.len()
                    && src_row + copy_start + copy_len <= pixel_data.len()
                {
                    pixel_data.copy_within(
                        src_row + copy_start..src_row + copy_start + copy_len,
                        dst_row + copy_start,
                    );
                }
            }
        }

        // Write to WGPU texture
        if let Some(texture) = self.base.peek_d3d_texture() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: texture.as_ref(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixel_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(surface_width * pixel_bytes),
                    rows_per_image: Some(surface_height),
                },
                wgpu::Extent3d {
                    width: surface_width,
                    height: surface_height,
                    depth_or_array_layers: 1,
                },
            );

            // Generate mipmaps (equivalent to D3DXFilterTexture with D3DX_FILTER_BOX)
            // In WGPU, this would be done via compute shader or manual generation
            // C++ line 165
        }

        // Set LOD if texture reduction enabled (C++ lines 166-168)
        if let Some(global_data) = get_global_data() {
            if global_data.texture_reduction_factor > 0 {
                self.base.set_lod(global_data.texture_reduction_factor);
            }
        }

        surface_height
    }

    fn terrain_filter() -> TextureFilter {
        let mut filter = TextureFilter::default();
        let use_linear = get_global_data()
            .map(|data| data.bilinear_terrain_tex || data.trilinear_terrain_tex)
            .unwrap_or(false);
        let use_trilinear = get_global_data()
            .map(|data| data.trilinear_terrain_tex)
            .unwrap_or(false);

        filter.set_min_filter(if use_linear {
            FilterType::Linear
        } else {
            FilterType::Point
        });
        filter.set_mag_filter(if use_linear {
            FilterType::Linear
        } else {
            FilterType::Point
        });
        filter.set_mip_mapping(if use_trilinear {
            FilterType::Linear
        } else {
            FilterType::Point
        });
        filter.set_u_addr_mode(AddressMode::Clamp);
        filter.set_v_addr_mode(AddressMode::Clamp);
        filter
    }

    /// Update flat terrain texture
    /// C++ lines 343-397
    pub fn update_flat(
        &mut self,
        ht_map: &WorldHeightMap,
        x_cell: i32,
        y_cell: i32,
        cell_width: i32,
        pixels_per_cell: u32,
        device: &Device,
        queue: &Queue,
    ) -> bool {
        let surface_width = self.base.width;
        let surface_height = self.base.height;
        let expected_extent = cell_width.max(0) as u32 * pixels_per_cell;

        if surface_width != expected_extent || surface_height != expected_extent {
            return false;
        }

        if self.base.format != WW3D_FORMAT_A1R5G5B5 || cell_width <= 0 || pixels_per_cell == 0 {
            return false;
        }

        let pixel_bytes = 2; // A1R5G5B5
        let buffer_size = (surface_width * surface_height * pixel_bytes) as usize;
        let mut pixel_data = vec![0u8; buffer_size];

        // Copy cell data (C++ lines 374-389)
        for cell_x in 0..cell_width {
            for cell_y in 0..cell_width {
                if let Some(tile) = ht_map.find_tile_data_for_cell(
                    x_cell + cell_x,
                    y_cell + cell_y,
                    pixels_per_cell,
                ) {
                    let bgr_data = WorldHeightMap::tile_data_for_width(tile, pixels_per_cell);
                    if bgr_data.len()
                        < (pixels_per_cell * pixels_per_cell * TILE_BYTES_PER_PIXEL) as usize
                    {
                        continue;
                    }

                    // Convert and copy pixels
                    for k in (0..pixels_per_cell as i32).rev() {
                        let dst_row =
                            (pixels_per_cell as i32 * (cell_width - cell_y - 1) + k) as usize;
                        let dst_row_offset =
                            dst_row * surface_width as usize * pixel_bytes as usize;
                        let dst_col_offset =
                            (cell_x as u32 * pixels_per_cell) as usize * pixel_bytes as usize;

                        for l in 0..pixels_per_cell as usize {
                            let src_idx = ((pixels_per_cell as usize - 1 - k as usize)
                                * pixels_per_cell as usize
                                + l)
                                * TILE_BYTES_PER_PIXEL as usize;
                            let dst_idx =
                                dst_row_offset + dst_col_offset + l * pixel_bytes as usize;

                            if src_idx + 2 < bgr_data.len() && dst_idx + 1 < pixel_data.len() {
                                let b = bgr_data[src_idx + 2];
                                let g = bgr_data[src_idx + 1];
                                let r = bgr_data[src_idx + 0];

                                // BGR24 to A1R5G5B5 (C++ line 384)
                                let pixel_16bit: u16 = 0x8000
                                    + (((b >> 3) as u16) << 10)
                                    + (((g >> 3) as u16) << 5)
                                    + ((r >> 3) as u16);

                                pixel_data[dst_idx] = (pixel_16bit & 0xFF) as u8;
                                pixel_data[dst_idx + 1] = ((pixel_16bit >> 8) & 0xFF) as u8;
                            }
                        }
                    }
                }
            }
        }

        // Write to texture and generate mipmaps (C++ lines 393-396)
        if let Some(texture) = self.base.peek_d3d_texture() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: texture.as_ref(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixel_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(surface_width * pixel_bytes),
                    rows_per_image: Some(surface_height),
                },
                wgpu::Extent3d {
                    width: surface_width,
                    height: surface_height,
                    depth_or_array_layers: 1,
                },
            );
        }

        true
    }

    /// Set LOD (C++ lines 332-335)
    pub fn set_lod(&mut self, lod: u32) {
        self.base.set_lod(lod);
    }

    /// Apply texture (C++ lines 405-438)
    /// Sets up D3D texture stage states for terrain rendering
    pub fn apply(&self, stage: u32) {
        self.base.apply(stage);
        self.base.set_apply_state(stage, Self::terrain_filter());
    }
}

// =============================================================================
// ALPHATERRAINTEXTURECLASS - Alpha blended terrain overlay
// C++ lines 440-602
// =============================================================================

/// Alpha terrain texture - shares base texture, renders with alpha blending
/// C++ lines 448-462
pub struct AlphaTerrainTextureClass {
    /// Base texture (uses 8x8 dummy, then attaches real texture)
    base: TextureClass,
}

impl AlphaTerrainTextureClass {
    /// Constructor - creates dummy texture then attaches base texture
    /// C++ lines 455-462
    pub fn new(base_tex: Arc<Texture>) -> Self {
        let mut texture_class = TextureClass::new(8, 8, WW3D_FORMAT_A1R5G5B5, MIP_LEVELS_1);
        texture_class.set_d3d_base_texture(base_tex);

        Self {
            base: texture_class,
        }
    }

    /// Apply with alpha blending (C++ lines 475-602)
    pub fn apply(&self, stage: u32) {
        self.base.apply(stage);
        self.base
            .set_apply_state(stage, TerrainTextureClass::terrain_filter());
    }
}

// =============================================================================
// ALPHAEDGETEXTURECLASS - Edge blending between tiles
// C++ lines 715-866
// =============================================================================

/// Alpha edge texture for tile blending
/// C++ lines 725-730
pub struct AlphaEdgeTextureClass {
    /// Base texture (A8R8G8B8 32-bit format)
    base: TextureClass,
}

impl AlphaEdgeTextureClass {
    /// Constructor - creates A8R8G8B8 texture
    /// C++ lines 725-730
    pub fn new(height: u32, mip_level_count: u32) -> Self {
        Self {
            base: TextureClass::new(TEXTURE_WIDTH, height, WW3D_FORMAT_A8R8G8B8, mip_level_count),
        }
    }

    /// Update from height map (C++ lines 737-809)
    pub fn update(&mut self, ht_map: &WorldHeightMap, device: &Device, queue: &Queue) -> u32 {
        let surface_width = self.base.width;
        let surface_height = self.base.height;

        let pixel_bytes = 4; // A8R8G8B8 format
        let buffer_size = (surface_width * surface_height * pixel_bytes) as usize;
        let mut pixel_data = vec![0u8; buffer_size];

        // Debug fill pattern (C++ lines 756-765)
        // Would normally be disabled in release
        #[cfg(debug_assertions)]
        {
            for cell_x in 0..surface_width {
                for cell_y in 0..surface_height {
                    let idx = ((cell_y * surface_width + cell_x) * 4) as usize;
                    pixel_data[idx + 0] = (cell_x / 2) as u8; // R
                    pixel_data[idx + 1] = 0; // G
                    pixel_data[idx + 2] = (255 - cell_y / 2) as u8; // B
                    pixel_data[idx + 3] = 128; // A
                }
            }
        }

        // Copy edge tiles (C++ lines 768-801)
        for tile_ndx in 0..ht_map.num_edge_tiles {
            if let Some(tile) = ht_map.get_edge_tile(tile_ndx) {
                let position = tile.tile_location_in_texture;
                if position.x <= 0 {
                    continue; // All real edge offsets start at 4
                }

                let column = position.x as usize;

                for j in 0..TILE_PIXEL_EXTENT {
                    let row = (position.y as u32 + j) as usize;
                    let bgr_data = tile.get_rgb_data_for_width(TILE_PIXEL_EXTENT);
                    let src_row_offset = ((TILE_PIXEL_EXTENT - 1 - j)
                        * TILE_BYTES_PER_PIXEL
                        * TILE_PIXEL_EXTENT) as usize;
                    let dst_row_offset = row * surface_width as usize * pixel_bytes as usize;
                    let dst_offset = dst_row_offset + column * pixel_bytes as usize;

                    for i in 0..TILE_PIXEL_EXTENT as usize {
                        let src_idx = src_row_offset + i * TILE_BYTES_PER_PIXEL as usize;
                        let dst_idx = dst_offset + i * pixel_bytes as usize;

                        if src_idx + 2 < bgr_data.len() && dst_idx + 3 < pixel_data.len() {
                            let r = bgr_data[src_idx + 0];
                            let g = bgr_data[src_idx + 1];
                            let b = bgr_data[src_idx + 2];

                            pixel_data[dst_idx + 0] = r; // R (C++ line 786)
                            pixel_data[dst_idx + 1] = g; // G (C++ line 787)
                            pixel_data[dst_idx + 2] = b; // B (C++ line 788)

                            // Alpha channel based on color (C++ lines 789-795)
                            if r == 0 && g == 0 && b == 0 {
                                pixel_data[dst_idx + 3] = 0x80; // Black -> 50% alpha
                            } else if r == 0xFF && g == 0xFF && b == 0xFF {
                                pixel_data[dst_idx + 3] = 0x00; // White -> transparent
                            } else {
                                pixel_data[dst_idx + 3] = 0xFF; // Other -> opaque
                            }
                        }
                    }
                }
            }
        }

        // Write to texture and generate mipmaps (C++ lines 805-808)
        if let Some(texture) = self.base.peek_d3d_texture() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: texture.as_ref(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixel_data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(surface_width * pixel_bytes),
                    rows_per_image: Some(surface_height),
                },
                wgpu::Extent3d {
                    width: surface_width,
                    height: surface_height,
                    depth_or_array_layers: 1,
                },
            );
        }

        surface_height
    }

    /// Update 256x256 variant (C++ lines 732-735)
    pub fn update_256(&mut self, _ht_map: &WorldHeightMap) -> u32 {
        // Placeholder - not used in current implementation
        1
    }

    /// Apply edge texture (C++ lines 811-866)
    pub fn apply(&self, stage: u32) {
        self.base.apply(stage);
        self.base
            .set_apply_state(stage, TerrainTextureClass::terrain_filter());
    }
}

// =============================================================================
// LIGHTMAPTERRAINTEXTURECLASS - Terrain lighting
// C++ lines 606-704
// =============================================================================

/// Light map terrain texture
/// C++ lines 617-624
pub struct LightMapTerrainTextureClass {
    /// Base texture (loaded from TGA file)
    base: TextureClass,
}

impl LightMapTerrainTextureClass {
    /// Constructor - loads texture from file
    /// C++ lines 617-624
    pub fn new(name: String, mip_level_count: u32) -> Self {
        let texture_name = if name.is_empty() {
            "TSNoiseUrb.tga"
        } else {
            &name
        };

        let mut base = TextureClass::from_file(texture_name, texture_name, mip_level_count);
        let mut filter = TextureFilter::default();
        filter.set_min_filter(FilterType::Best);
        filter.set_mag_filter(FilterType::Best);
        filter.set_u_addr_mode(AddressMode::Repeat);
        filter.set_v_addr_mode(AddressMode::Repeat);
        base.set_apply_state(0, filter);

        // Setup filter modes (C++ lines 620-623)
        // MIN/MAG filter: BEST (linear)
        // U/V addressing: REPEAT

        Self { base }
    }

    /// Apply light map (C++ lines 642-704)
    pub fn apply(&self, stage: u32) {
        self.base.apply(stage);
    }
}

// =============================================================================
// CLOUDMAPTERRAINTEXTURECLASS - Animated cloud shadows
// C++ lines 870-1044
// =============================================================================

/// Cloud map terrain texture - animated sliding clouds
/// C++ lines 883-893
pub struct CloudMapTerrainTextureClass {
    /// Base texture (loaded from TGA)
    base: TextureClass,
    /// X slide speed (units per second) - C++ line 108
    x_slide_per_second: f32,
    /// Y slide speed (units per second) - C++ line 109
    y_slide_per_second: f32,
    /// Current tick count - C++ line 110
    cur_tick: u32,
    /// Current X offset - C++ line 111
    x_offset: f32,
    /// Current Y offset - C++ line 112
    y_offset: f32,
}

impl CloudMapTerrainTextureClass {
    /// Constructor - loads cloud texture
    /// C++ lines 883-893
    pub fn new(mip_level_count: u32) -> Self {
        let mut base = TextureClass::from_file("TSCloudMed.tga", "TSCloudMed.tga", mip_level_count);
        let mut filter = TextureFilter::default();
        filter.set_mip_mapping(FilterType::Fast);
        base.set_apply_state(0, filter);

        // Setup mipmap filter (C++ line 886)
        // FILTER_TYPE_FAST

        Self {
            base,
            x_slide_per_second: -0.02,        // C++ line 887
            y_slide_per_second: -0.02 * 1.50, // C++ line 888
            cur_tick: 0,                      // C++ line 889
            x_offset: 0.0,                    // C++ line 890
            y_offset: 0.0,                    // C++ line 891
        }
    }

    /// Apply cloud map with animation (C++ lines 909-992)
    pub fn apply(&mut self, stage: u32) {
        self.base.apply(stage);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u32;
        let delta_ms = now_ms.wrapping_sub(self.cur_tick);
        self.cur_tick = now_ms;

        let delta_seconds = delta_ms as f32 / 1000.0;
        self.x_offset += self.x_slide_per_second * delta_seconds;
        self.y_offset += self.y_slide_per_second * delta_seconds;

        if self.x_offset > 1.0 {
            self.x_offset -= 1.0;
        } else if self.x_offset < -1.0 {
            self.x_offset += 1.0;
        }
        if self.y_offset > 1.0 {
            self.y_offset -= 1.0;
        } else if self.y_offset < -1.0 {
            self.y_offset += 1.0;
        }
    }

    /// Restore default texture states (C++ lines 1000-1044)
    pub fn restore(&self) {
        self.base.reset_apply_state();
    }
}

// =============================================================================
// SCORCHTEXTURECLASS - Scorch marks and damage
// C++ lines 1046-1109
// =============================================================================

/// Scorch texture for ground damage marks
/// C++ lines 1059-1064
pub struct ScorchTextureClass {
    /// Base texture (loaded from TGA)
    base: TextureClass,
}

impl ScorchTextureClass {
    /// Constructor - loads scorch texture
    /// C++ lines 1059-1064
    pub fn new(mip_level_count: u32) -> Self {
        Self {
            base: TextureClass::from_file("EXScorch01.tga", "EXScorch01.tga", mip_level_count),
        }
    }

    /// Apply scorch texture (C++ lines 1074-1108)
    pub fn apply(&self, stage: u32) {
        self.base.apply(stage);
        self.base
            .set_apply_state(stage, TerrainTextureClass::terrain_filter());
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        // Verify critical constants match C++
        assert_eq!(TEXTURE_WIDTH, 2048);
        assert_eq!(TILE_OFFSET, 8);
        assert_eq!(TILE_PIXEL_EXTENT, 64);
        assert_eq!(TILE_BYTES_PER_PIXEL, 4);
    }

    #[test]
    fn test_bgr24_to_a1r5g5b5_conversion() {
        // Test pixel format conversion (C++ line 116)
        let r: u8 = 255;
        let g: u8 = 128;
        let b: u8 = 64;

        let pixel_16bit: u16 =
            0x8000 + (((b >> 3) as u16) << 10) + (((g >> 3) as u16) << 5) + ((r >> 3) as u16);

        // Verify format: A1R5G5B5
        // Alpha = 1 (bit 15)
        assert_eq!(pixel_16bit & 0x8000, 0x8000);

        // Blue = 64 >> 3 = 2 (bits 14-10)
        let b_extracted = ((pixel_16bit >> 10) & 0x1F) as u8;
        assert_eq!(b_extracted, 2);

        // Green = 128 >> 3 = 16 (bits 9-5)
        let g_extracted = ((pixel_16bit >> 5) & 0x1F) as u8;
        assert_eq!(g_extracted, 16);

        // Red = 255 >> 3 = 31 (bits 4-0)
        let r_extracted = (pixel_16bit & 0x1F) as u8;
        assert_eq!(r_extracted, 31);
    }

    #[test]
    fn test_icoord2d() {
        let coord = ICoord2D::new(100, 200);
        assert_eq!(coord.x, 100);
        assert_eq!(coord.y, 200);
    }

    #[test]
    fn test_terrain_texture_creation() {
        let texture = TerrainTextureClass::new(1024);
        assert_eq!(texture.base.width, TEXTURE_WIDTH);
        assert_eq!(texture.base.height, 1024);
        assert_eq!(texture.base.format, WW3D_FORMAT_A1R5G5B5);
        assert_eq!(texture.base.mip_levels, MIP_LEVELS_3);
    }

    #[test]
    fn test_terrain_texture_with_width() {
        let texture = TerrainTextureClass::new_with_width(512, 1024);
        assert_eq!(texture.base.width, 1024);
        assert_eq!(texture.base.height, 512);
        assert_eq!(texture.base.format, WW3D_FORMAT_A1R5G5B5);
        assert_eq!(texture.base.mip_levels, MIP_LEVELS_ALL);
    }

    #[test]
    fn test_alpha_edge_texture_creation() {
        let texture = AlphaEdgeTextureClass::new(1024, MIP_LEVELS_3);
        assert_eq!(texture.base.width, TEXTURE_WIDTH);
        assert_eq!(texture.base.height, 1024);
        assert_eq!(texture.base.format, WW3D_FORMAT_A8R8G8B8);
    }

    #[test]
    fn test_alpha_channel_logic() {
        // Test alpha channel assignment (C++ lines 789-795)
        let test_cases = [
            ((0u8, 0u8, 0u8), 0x80u8), // Black -> 50% alpha
            ((255, 255, 255), 0x00),   // White -> transparent
            ((128, 64, 32), 0xFF),     // Other -> opaque
        ];

        for ((r, g, b), expected_alpha) in test_cases.iter() {
            let alpha = if *r == 0 && *g == 0 && *b == 0 {
                0x80
            } else if *r == 0xFF && *g == 0xFF && *b == 0xFF {
                0x00
            } else {
                0xFF
            };

            assert_eq!(alpha, *expected_alpha);
        }
    }

    #[test]
    fn test_cloud_animation_speed() {
        let clouds = CloudMapTerrainTextureClass::new(MIP_LEVELS_ALL);
        assert_eq!(clouds.x_slide_per_second, -0.02);
        assert_eq!(clouds.y_slide_per_second, -0.02 * 1.50);
    }

    #[test]
    fn test_light_map_constructor_seeds_repeat_filter() {
        let texture = LightMapTerrainTextureClass::new(String::new(), MIP_LEVELS_ALL);
        let filter = texture.base.get_filter();

        assert_eq!(filter.min_filter, FilterType::Best);
        assert_eq!(filter.mag_filter, FilterType::Best);
        assert_eq!(filter.mip_filter, FilterType::Point);
        assert_eq!(filter.u_addr_mode, AddressMode::Repeat);
        assert_eq!(filter.v_addr_mode, AddressMode::Repeat);
    }

    #[test]
    fn test_light_map_apply_preserves_constructor_filter() {
        let texture = LightMapTerrainTextureClass::new(String::new(), MIP_LEVELS_ALL);
        texture.apply(1);

        let filter = texture.base.get_filter();
        assert_eq!(filter.min_filter, FilterType::Best);
        assert_eq!(filter.mag_filter, FilterType::Best);
        assert_eq!(filter.mip_filter, FilterType::Point);
        assert_eq!(filter.u_addr_mode, AddressMode::Repeat);
        assert_eq!(filter.v_addr_mode, AddressMode::Repeat);
    }

    #[test]
    fn test_cloud_map_constructor_seeds_fast_mip_filter() {
        let texture = CloudMapTerrainTextureClass::new(MIP_LEVELS_ALL);
        let filter = texture.base.get_filter();

        assert_eq!(filter.min_filter, FilterType::Point);
        assert_eq!(filter.mag_filter, FilterType::Point);
        assert_eq!(filter.mip_filter, FilterType::Fast);
        assert_eq!(filter.u_addr_mode, AddressMode::Wrap);
        assert_eq!(filter.v_addr_mode, AddressMode::Wrap);
    }

    #[test]
    fn test_cloud_map_restore_preserves_fast_mip_filter() {
        let mut clouds = CloudMapTerrainTextureClass::new(MIP_LEVELS_ALL);
        clouds.apply(0);
        clouds.restore();

        let filter = clouds.base.get_filter();
        assert_eq!(filter.min_filter, FilterType::Point);
        assert_eq!(filter.mag_filter, FilterType::Point);
        assert_eq!(filter.mip_filter, FilterType::Fast);
        assert_eq!(filter.u_addr_mode, AddressMode::Wrap);
        assert_eq!(filter.v_addr_mode, AddressMode::Wrap);
    }

    #[test]
    fn test_stretch_factor() {
        // Verify stretch factor calculation (C++ line 626)
        let expected = 1.0 / (63.0 * 2.0 / 2.0);
        assert!((STRETCH_FACTOR - expected).abs() < 0.0001);
    }

    #[test]
    fn test_tiles_per_row_calculation() {
        // Test calculation from C++ line 80
        let surface_width = 2048u32;
        let tiles_per_row = surface_width / (2 * TILE_PIXEL_EXTENT + TILE_OFFSET);
        let tiles_per_row = tiles_per_row * 2;

        // 2048 / (2*64 + 8) = 2048 / 136 = 15
        // 15 * 2 = 30
        assert_eq!(tiles_per_row, 30);
    }

    #[test]
    fn test_terrain_texture_apply_tracks_clamp_filtering() {
        let texture = TerrainTextureClass::new(1024);
        texture.apply(0);

        let filter = texture.base.get_filter();
        assert_eq!(filter.min_filter, FilterType::Point);
        assert_eq!(filter.mag_filter, FilterType::Point);
        assert_eq!(filter.mip_filter, FilterType::Point);
        assert_eq!(filter.u_addr_mode, AddressMode::Clamp);
        assert_eq!(filter.v_addr_mode, AddressMode::Clamp);
    }

    #[test]
    fn test_cloud_map_apply_advances_animation_clock() {
        let mut clouds = CloudMapTerrainTextureClass::new(MIP_LEVELS_ALL);
        clouds.apply(0);

        assert_ne!(clouds.cur_tick, 0);
        assert!(clouds.x_offset >= -1.0 && clouds.x_offset <= 1.0);
        assert!(clouds.y_offset >= -1.0 && clouds.y_offset <= 1.0);
    }
}
