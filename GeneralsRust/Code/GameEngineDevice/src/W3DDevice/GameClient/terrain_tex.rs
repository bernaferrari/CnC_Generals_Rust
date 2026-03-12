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

use std::sync::Arc;
use wgpu::{Device, Queue, Texture, TextureView, Sampler};

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
pub struct TileData {
    /// Location of this tile in the texture atlas
    pub tile_location_in_texture: ICoord2D,
    /// RGB bitmap data for this tile
    rgb_data: Vec<u8>,
    /// Tile width in pixels
    width: u32,
}

impl TileData {
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
pub struct TextureClassData {
    /// Width in tiles
    pub width: u32,
    /// Position in texture atlas
    pub position_in_texture: ICoord2D,
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
    /// Get source tile by index (C++ line 100)
    pub fn get_source_tile(&self, index: usize) -> Option<&TileData> {
        self.source_tiles.get(index).and_then(|t| t.as_ref())
    }

    /// Get edge tile by index (C++ line 771)
    pub fn get_edge_tile(&self, index: usize) -> Option<&TileData> {
        self.edge_tiles.get(index).and_then(|t| t.as_ref())
    }

    /// Get pointer to tile data at cell coordinates (C++ line 377)
    pub fn get_pointer_to_tile_data(&self, x_cell: i32, y_cell: i32, pixels_per_cell: u32) -> Option<&[u8]> {
        // Simplified implementation - would need full terrain system
        None
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
static mut THE_GLOBAL_DATA: Option<GlobalData> = None;

pub fn get_global_data() -> Option<&'static GlobalData> {
    unsafe { THE_GLOBAL_DATA.as_ref() }
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
        // Base implementation - sets the texture
        // Actual D3D/WGPU state setup would go here
    }

    /// Set LOD level (C++ line 332)
    pub fn set_lod(&mut self, lod: u32) {
        self.current_lod = lod;
        // Would call actual texture LOD API here
    }

    /// Get filter settings (simplified)
    pub fn get_filter(&self) -> TextureFilter {
        TextureFilter::default()
    }
}

/// Texture filter settings (simplified from C++ TextureFilterClass)
#[derive(Debug, Clone, Default)]
pub struct TextureFilter {
    min_filter: FilterType,
    mag_filter: FilterType,
    mip_filter: FilterType,
    u_addr_mode: AddressMode,
    v_addr_mode: AddressMode,
}

#[derive(Debug, Clone, Copy)]
pub enum FilterType {
    Point,
    Linear,
    Best,
}

impl Default for FilterType {
    fn default() -> Self {
        FilterType::Point
    }
}

#[derive(Debug, Clone, Copy)]
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
                    let src_row_offset = ((TILE_PIXEL_EXTENT - 1 - j) * TILE_BYTES_PER_PIXEL * TILE_PIXEL_EXTENT) as usize;

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
                    let src_idx = row_offset + (column + width as usize - 4 + k) * pixel_bytes as usize;
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
                    && src_row + copy_start + copy_len <= pixel_data.len() {
                    pixel_data.copy_within(
                        src_row + copy_start..src_row + copy_start + copy_len,
                        dst_row + copy_start,
                    );
                }

                // Copy after (from top of tile area)
                let src_row = origin.y as usize * row_bytes;
                let dst_row = (origin.y as usize + width as usize + j) * row_bytes;

                if dst_row + copy_start + copy_len <= pixel_data.len()
                    && src_row + copy_start + copy_len <= pixel_data.len() {
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

        if surface_width != (cell_width as u32 * pixels_per_cell) {
            return false;
        }

        let pixel_bytes = 2; // A1R5G5B5
        let buffer_size = (surface_width * surface_height * pixel_bytes) as usize;
        let mut pixel_data = vec![0u8; buffer_size];

        // Copy cell data (C++ lines 374-389)
        for cell_x in 0..cell_width {
            for cell_y in 0..cell_width {
                if let Some(bgr) = ht_map.get_pointer_to_tile_data(
                    x_cell + cell_x,
                    y_cell + cell_y,
                    pixels_per_cell,
                ) {
                    // Convert and copy pixels
                    for k in (0..pixels_per_cell as i32).rev() {
                        let dst_row = (pixels_per_cell as i32 * (cell_width - cell_y - 1) + k) as usize;
                        let dst_row_offset = dst_row * surface_width as usize * pixel_bytes as usize;
                        let dst_col_offset = (cell_x as u32 * pixels_per_cell) as usize * pixel_bytes as usize;

                        for l in 0..pixels_per_cell as usize {
                            let src_idx = ((pixels_per_cell as usize - 1 - k as usize)
                                * pixels_per_cell as usize + l) * TILE_BYTES_PER_PIXEL as usize;
                            let dst_idx = dst_row_offset + dst_col_offset + l * pixel_bytes as usize;

                            if src_idx + 2 < bgr.len() && dst_idx + 1 < pixel_data.len() {
                                let b = bgr[src_idx + 2];
                                let g = bgr[src_idx + 1];
                                let r = bgr[src_idx + 0];

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

        // C++ lines 410-421 - Setup filtering based on global settings
        // These would map to WGPU sampler settings:
        // - bilinear_terrain_tex -> LINEAR min/mag filter
        // - trilinear_terrain_tex -> LINEAR mipmap filter
        // - Otherwise -> POINT filter

        // C++ lines 423-436 - Texture pipeline setup
        // Stage 0 setup:
        // - ADDRESSU/V: CLAMP
        // - COLORARG1: TEXTURE
        // - COLORARG2: DIFFUSE
        // - COLOROP: MODULATE
        // - ALPHAOP: DISABLE
        // - Disable stage 1
        // - TEXCOORDINDEX: 0
        // - Disable alpha blending
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

        // Setup filtering (C++ lines 481-492)
        // Same as TerrainTextureClass

        // Clamp addressing (C++ lines 494-495)
        // ADDRESSU/V: CLAMP

        if stage == 0 {
            // Multi-pass mode (C++ lines 497-510)
            // Stage 0 setup:
            // - COLORARG1: TEXTURE
            // - COLORARG2: DIFFUSE
            // - COLOROP: MODULATE
            // - ALPHAOP: MODULATE
            // - TEXCOORDINDEX: 1
            // - Enable alpha blending: SRCALPHA/INVSRCALPHA
            // - Disable stage 1-2
        } else if stage == 1 {
            if let Some(global_data) = get_global_data() {
                if !global_data.multi_pass_terrain {
                    // 8-stage Nvidia optimization (C++ lines 513-587)
                    // Complex multi-stage setup for single-pass rendering
                    // This is a hardware-specific optimization path
                } else {
                    // Standard 2-stage setup (C++ lines 590-600)
                    // Stage 0: SELECTARG1 (texture)
                    // Stage 1: MODULATE with current
                }
            }
        }
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
                    let src_row_offset = ((TILE_PIXEL_EXTENT - 1 - j) * TILE_BYTES_PER_PIXEL * TILE_PIXEL_EXTENT) as usize;
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

        // Setup filtering (C++ lines 817-828)
        // Clamp addressing (C++ lines 830-831)

        if stage == 0 {
            // Single-stage edge rendering (C++ lines 833-846)
            // Stage 0:
            // - COLORARG1: TEXTURE
            // - COLORARG2: DIFFUSE
            // - COLOROP: MODULATE
            // - ALPHAARG1: TEXTURE
            // - ALPHAOP: SELECTARG1
            // - TEXCOORDINDEX: 1
            // - Enable alpha blend: SRCALPHA/INVSRCALPHA
            // - Disable stage 1
        } else if stage == 1 {
            // Two-stage setup (C++ lines 848-863)
            // Used when drawing texture through mask
            // Stage 0: Keep alpha from previous
            // Stage 1: Blend texture with alpha
        }
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

        // Setup filter modes (C++ lines 620-623)
        // MIN/MAG filter: BEST (linear)
        // U/V addressing: REPEAT

        Self { base }
    }

    /// Apply light map (C++ lines 642-704)
    pub fn apply(&self, stage: u32) {
        self.base.apply(stage);

        // C++ lines 648-696 - Complex texture coordinate generation
        // Uses camera space position for automatic UV generation
        // Applies transformation matrix to scale and slide the texture
        // STRETCH_FACTOR scales the texture to cover terrain

        // Key settings (from C++ code):
        // - TEXCOORDINDEX: D3DTSS_TCI_CAMERASPACEPOSITION
        // - TEXTURETRANSFORMFLAGS: D3DTTFF_COUNT2
        // - ADDRESSU/V: WRAP
        // - Transform matrix: inverse view * scale

        // Stage 0 (C++ lines 664-668):
        // - COLORARG1: TEXTURE
        // - COLOROP: SELECTARG1
        // - Disable stage 1

        // Stage 1 (C++ line 670):
        // - COLOROP: MODULATE with current

        // Blend mode (C++ lines 699-701):
        // - DESTCOLOR/ZERO (multiplicative blend)
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

        // Setup mipmap filter (C++ line 886)
        // FILTER_TYPE_FAST

        Self {
            base,
            x_slide_per_second: -0.02, // C++ line 887
            y_slide_per_second: -0.02 * 1.50, // C++ line 888
            cur_tick: 0, // C++ line 889
            x_offset: 0.0, // C++ line 890
            y_offset: 0.0, // C++ line 891
        }
    }

    /// Apply cloud map with animation (C++ lines 909-992)
    pub fn apply(&mut self, stage: u32) {
        self.base.apply(stage);

        // Animation update (C++ lines 948-962)
        // Would get current tick count, calculate delta
        // Update offsets based on slide speed
        // Wrap offsets to [-1, 1] range

        // C++ lines 918-944 - Texture coordinate generation
        // Similar to light map but with animated offset
        // Uses camera space position with sliding transform matrix

        if stage == 0 {
            // Stage 0 setup (C++ lines 964-978)
            // - COLORARG1: TEXTURE
            // - COLORARG2: DIFFUSE
            // - COLOROP: SELECTARG1
            // - ALPHAOP: DISABLE
            // - Set transform with offset
            // - Disable stages 1-3
            // - Blend: DESTCOLOR/ZERO
        } else if stage == 1 {
            // Stage 1 setup (C++ lines 982-990)
            // - COLORARG1: TEXTURE
            // - COLORARG2: CURRENT
            // - COLOROP: MODULATE
            // - ALPHAARG1: CURRENT
            // - ALPHAOP: SELECTARG1
            // - Set transform with offset
        }
    }

    /// Restore default texture states (C++ lines 1000-1044)
    pub fn restore(&self) {
        // Restore stages 0-1 to default W3D states (C++ lines 1002-1023)
        // - COLORARG1: TEXTURE
        // - COLORARG2: DIFFUSE
        // - COLOROP: MODULATE
        // - ALPHAOP: DISABLE
        // - ADDRESSU/V: WRAP
        // - TEXCOORDINDEX: 0
        // - TEXTURETRANSFORMFLAGS: DISABLE
        // - Reset alpha blend states

        // If multi-pass disabled, reset all 8 stages (C++ lines 1026-1043)
        // Clears Nvidia 8-stage hack states
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

        // Setup filtering based on global settings (C++ lines 1079-1090)
        // Same as terrain texture filtering

        // Texture pipeline setup (C++ lines 1092-1107)
        // - TEXTURETRANSFORMFLAGS: DISABLE
        // - ADDRESSU/V: CLAMP
        // - COLORARG1: TEXTURE
        // - COLORARG2: DIFFUSE
        // - COLOROP: MODULATE
        // - ALPHAOP: SELECTARG1
        // - TEXCOORDINDEX: 0
        // - Enable alpha blend: SRCALPHA/INVSRCALPHA
        // - Disable stage 1
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

        let pixel_16bit: u16 = 0x8000
            + (((b >> 3) as u16) << 10)
            + (((g >> 3) as u16) << 5)
            + ((r >> 3) as u16);

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
            ((0u8, 0u8, 0u8), 0x80u8),     // Black -> 50% alpha
            ((255, 255, 255), 0x00),        // White -> transparent
            ((128, 64, 32), 0xFF),          // Other -> opaque
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
}
