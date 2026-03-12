//! Terrain Texture Blending System
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Include/W3DDevice/GameClient/TerrainTex.h
//! - GameEngineDevice/Source/W3DDevice/GameClient/WorldHeightMap.cpp
//! - GameEngineDevice/Include/W3DDevice/GameClient/TileData.h
//!
//! Implements multi-layer texture blending with:
//! - Base terrain textures
//! - Detail/cliff textures
//! - Alpha blend maps (splatmaps)
//! - Edge blending for transitions

use wgpu::{
    Device, Queue, Texture, TextureView, Sampler, TextureDescriptor, TextureUsages,
    TextureFormat, TextureDimension, Extent3d, SamplerDescriptor, AddressMode,
    FilterMode, TextureViewDescriptor, ImageCopyTexture, ImageDataLayout, Origin3d,
};
use std::sync::Arc;
use anyhow::{Result, Context};

// Constants from C++ WorldHeightMap.h and TerrainTex.h
pub const TILE_OFFSET: usize = 8; // TerrainTex.h line 17
pub const NUM_SOURCE_TILES: usize = 1024; // WorldHeightMap.h line 26
pub const NUM_BLEND_TILES: usize = 16192; // WorldHeightMap.h line 27
pub const NUM_TEXTURE_CLASSES: usize = 256; // WorldHeightMap.h line 58
pub const TERRAIN_TEXTURE_SIZE: u32 = 2048; // Common texture atlas size

/// Tile data for terrain textures
/// Corresponds to C++ TileData class
#[derive(Debug, Clone)]
pub struct TileData {
    /// Width and height of the tile in pixels
    pub width: u32,
    pub height: u32,
    /// Pixel data (RGBA8)
    pub pixels: Vec<u8>,
}

impl TileData {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height * 4) as usize;
        Self {
            width,
            height,
            pixels: vec![0; size],
        }
    }

    pub fn from_pixels(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        Self { width, height, pixels }
    }
}

/// Texture class information
/// Corresponds to C++ TXTextureClass from WorldHeightMap.h line 36
#[derive(Debug, Clone)]
pub struct TextureClass {
    pub global_texture_class: i32,
    pub first_tile: usize,
    pub num_tiles: usize,
    pub width: u32,
    pub is_blend_edge_tile: bool,
    pub name: String,
    pub position_in_texture: (u32, u32),
}

/// Blend tile information
/// Corresponds to C++ TBlendTileInfo
#[derive(Debug, Clone, Copy)]
pub struct BlendTileInfo {
    /// UV coordinates for the 4 corners
    pub uv: [[f32; 2]; 4],
    /// Alpha values for the 4 corners
    pub alpha: [u8; 4],
    /// Source tile index
    pub tile_index: i16,
    /// Whether this tile is flipped
    pub flip: bool,
}

/// Terrain texture manager
/// Corresponds to C++ TerrainTextureClass and WorldHeightMap texture management
pub struct TerrainTextureManager {
    device: Arc<Device>,
    queue: Arc<Queue>,

    /// Base terrain texture atlas (C++ m_terrainTex)
    base_texture: Texture,
    base_texture_view: TextureView,
    base_sampler: Sampler,

    /// Detail/cliff texture atlas (C++ m_alphaTerrainTex)
    detail_texture: Texture,
    detail_texture_view: TextureView,
    detail_sampler: Sampler,

    /// Alpha blend map (C++ m_alphaEdgeTex)
    blend_texture: Texture,
    blend_texture_view: TextureView,
    blend_sampler: Sampler,

    /// Source tiles (C++ m_sourceTiles)
    source_tiles: Vec<Option<TileData>>,

    /// Edge blend tiles (C++ m_edgeTiles)
    edge_tiles: Vec<Option<TileData>>,

    /// Texture classes (C++ m_textureClasses)
    texture_classes: Vec<TextureClass>,

    /// Blend tile information (C++ m_blendedTiles)
    blend_tiles: Vec<BlendTileInfo>,

    /// Extra blend tiles for 3-way blends (C++ m_extraBlendedTiles)
    extra_blend_tiles: Vec<BlendTileInfo>,
}

impl TerrainTextureManager {
    /// Create a new terrain texture manager
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Result<Self> {
        // Create base terrain texture atlas
        let base_texture = Self::create_texture(
            &device,
            "Terrain Base Texture",
            TERRAIN_TEXTURE_SIZE,
            TERRAIN_TEXTURE_SIZE,
        )?;
        let base_texture_view = base_texture.create_view(&TextureViewDescriptor::default());
        let base_sampler = Self::create_sampler(&device, "Base Sampler");

        // Create detail texture atlas
        let detail_texture = Self::create_texture(
            &device,
            "Terrain Detail Texture",
            TERRAIN_TEXTURE_SIZE,
            TERRAIN_TEXTURE_SIZE,
        )?;
        let detail_texture_view = detail_texture.create_view(&TextureViewDescriptor::default());
        let detail_sampler = Self::create_sampler(&device, "Detail Sampler");

        // Create blend map texture
        let blend_texture = Self::create_texture(
            &device,
            "Terrain Blend Texture",
            TERRAIN_TEXTURE_SIZE,
            TERRAIN_TEXTURE_SIZE,
        )?;
        let blend_texture_view = blend_texture.create_view(&TextureViewDescriptor::default());
        let blend_sampler = Self::create_sampler(&device, "Blend Sampler");

        Ok(Self {
            device,
            queue,
            base_texture,
            base_texture_view,
            base_sampler,
            detail_texture,
            detail_texture_view,
            detail_sampler,
            blend_texture,
            blend_texture_view,
            blend_sampler,
            source_tiles: vec![None; NUM_SOURCE_TILES],
            edge_tiles: vec![None; NUM_SOURCE_TILES],
            texture_classes: Vec::new(),
            blend_tiles: Vec::with_capacity(NUM_BLEND_TILES),
            extra_blend_tiles: Vec::with_capacity(NUM_BLEND_TILES),
        })
    }

    /// Create a texture
    fn create_texture(device: &Device, label: &str, width: u32, height: u32) -> Result<Texture> {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        Ok(texture)
    }

    /// Create a sampler with wrapping and filtering
    fn create_sampler(device: &Device, label: &str) -> Sampler {
        device.create_sampler(&SamplerDescriptor {
            label: Some(label),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        })
    }

    /// Load a tile into the source tiles array
    /// Corresponds to C++ WorldHeightMap::readTiles
    pub fn load_source_tile(&mut self, index: usize, tile: TileData) -> Result<()> {
        if index >= NUM_SOURCE_TILES {
            anyhow::bail!("Tile index out of range: {}", index);
        }

        self.source_tiles[index] = Some(tile);
        Ok(())
    }

    /// Load an edge blend tile
    pub fn load_edge_tile(&mut self, index: usize, tile: TileData) -> Result<()> {
        if index >= NUM_SOURCE_TILES {
            anyhow::bail!("Edge tile index out of range: {}", index);
        }

        self.edge_tiles[index] = Some(tile);
        Ok(())
    }

    /// Add a texture class
    /// Corresponds to C++ WorldHeightMap::m_textureClasses
    pub fn add_texture_class(&mut self, class: TextureClass) {
        self.texture_classes.push(class);
    }

    /// Update the base texture atlas
    /// Corresponds to C++ TerrainTextureClass::update
    pub fn update_base_atlas(&self) -> Result<()> {
        // Pack tiles into texture atlas
        let mut atlas_data = vec![0u8; (TERRAIN_TEXTURE_SIZE * TERRAIN_TEXTURE_SIZE * 4) as usize];

        let mut current_y = 0;
        let mut current_x = 0;
        let mut row_height = 0;

        for (idx, tile_opt) in self.source_tiles.iter().enumerate() {
            if let Some(tile) = tile_opt {
                // Check if we need to start a new row
                if current_x + tile.width > TERRAIN_TEXTURE_SIZE {
                    current_x = 0;
                    current_y += row_height + TILE_OFFSET as u32;
                    row_height = 0;
                }

                // Check if we have space
                if current_y + tile.height > TERRAIN_TEXTURE_SIZE {
                    break; // Atlas is full
                }

                // Copy tile pixels into atlas
                for y in 0..tile.height {
                    for x in 0..tile.width {
                        let src_idx = ((y * tile.width + x) * 4) as usize;
                        let dst_x = current_x + x;
                        let dst_y = current_y + y;
                        let dst_idx = ((dst_y * TERRAIN_TEXTURE_SIZE + dst_x) * 4) as usize;

                        if dst_idx + 3 < atlas_data.len() && src_idx + 3 < tile.pixels.len() {
                            atlas_data[dst_idx..dst_idx + 4].copy_from_slice(&tile.pixels[src_idx..src_idx + 4]);
                        }
                    }
                }

                current_x += tile.width + TILE_OFFSET as u32;
                row_height = row_height.max(tile.height);
            }
        }

        // Upload to GPU
        self.queue.write_texture(
            ImageCopyTexture {
                texture: &self.base_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(TERRAIN_TEXTURE_SIZE * 4),
                rows_per_image: Some(TERRAIN_TEXTURE_SIZE),
            },
            Extent3d {
                width: TERRAIN_TEXTURE_SIZE,
                height: TERRAIN_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Update the detail texture atlas
    /// Corresponds to C++ AlphaTerrainTextureClass::update
    pub fn update_detail_atlas(&self) -> Result<()> {
        let mut atlas_data = vec![0u8; (TERRAIN_TEXTURE_SIZE * TERRAIN_TEXTURE_SIZE * 4) as usize];

        let mut current_y = 0;
        let mut current_x = 0;
        let mut row_height = 0;

        for (idx, tile_opt) in self.edge_tiles.iter().enumerate() {
            if let Some(tile) = tile_opt {
                if current_x + tile.width > TERRAIN_TEXTURE_SIZE {
                    current_x = 0;
                    current_y += row_height + TILE_OFFSET as u32;
                    row_height = 0;
                }

                if current_y + tile.height > TERRAIN_TEXTURE_SIZE {
                    break;
                }

                for y in 0..tile.height {
                    for x in 0..tile.width {
                        let src_idx = ((y * tile.width + x) * 4) as usize;
                        let dst_x = current_x + x;
                        let dst_y = current_y + y;
                        let dst_idx = ((dst_y * TERRAIN_TEXTURE_SIZE + dst_x) * 4) as usize;

                        if dst_idx + 3 < atlas_data.len() && src_idx + 3 < tile.pixels.len() {
                            atlas_data[dst_idx..dst_idx + 4].copy_from_slice(&tile.pixels[src_idx..src_idx + 4]);
                        }
                    }
                }

                current_x += tile.width + TILE_OFFSET as u32;
                row_height = row_height.max(tile.height);
            }
        }

        self.queue.write_texture(
            ImageCopyTexture {
                texture: &self.detail_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(TERRAIN_TEXTURE_SIZE * 4),
                rows_per_image: Some(TERRAIN_TEXTURE_SIZE),
            },
            Extent3d {
                width: TERRAIN_TEXTURE_SIZE,
                height: TERRAIN_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Get UV coordinates for a tile
    /// Corresponds to C++ WorldHeightMap::getUVData
    pub fn get_uv_for_tile(&self, tile_index: i16) -> [[f32; 2]; 4] {
        // Find the tile in the texture classes
        for class in &self.texture_classes {
            if tile_index >= class.first_tile as i16 && tile_index < (class.first_tile + class.num_tiles) as i16 {
                let tile_offset = tile_index - class.first_tile as i16;

                // Calculate UV coordinates based on tile position in atlas
                let u_offset = (class.position_in_texture.0 as f32) / TERRAIN_TEXTURE_SIZE as f32;
                let v_offset = (class.position_in_texture.1 as f32) / TERRAIN_TEXTURE_SIZE as f32;
                let u_size = (class.width as f32) / TERRAIN_TEXTURE_SIZE as f32;
                let v_size = (class.width as f32) / TERRAIN_TEXTURE_SIZE as f32;

                return [
                    [u_offset, v_offset],                       // Top-left
                    [u_offset + u_size, v_offset],              // Top-right
                    [u_offset + u_size, v_offset + v_size],     // Bottom-right
                    [u_offset, v_offset + v_size],              // Bottom-left
                ];
            }
        }

        // Default UVs if tile not found
        [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
    }

    /// Create blend tile information
    /// Corresponds to C++ WorldHeightMap blend tile generation
    pub fn create_blend_tile(
        &mut self,
        tile_index: i16,
        alpha: [u8; 4],
        flip: bool,
    ) -> usize {
        let uv = self.get_uv_for_tile(tile_index);

        let blend_tile = BlendTileInfo {
            uv,
            alpha,
            tile_index,
            flip,
        };

        self.blend_tiles.push(blend_tile);
        self.blend_tiles.len() - 1
    }

    /// Generate blend map texture
    /// Corresponds to C++ AlphaEdgeTextureClass::update
    pub fn update_blend_map(&self, map_width: usize, map_height: usize, blend_indices: &[i16]) -> Result<()> {
        // Create blend map data (single channel alpha)
        let mut blend_data = vec![0u8; (TERRAIN_TEXTURE_SIZE * TERRAIN_TEXTURE_SIZE * 4) as usize];

        // Scale map coordinates to texture coordinates
        let scale_x = TERRAIN_TEXTURE_SIZE as f32 / map_width as f32;
        let scale_y = TERRAIN_TEXTURE_SIZE as f32 / map_height as f32;

        for y in 0..map_height {
            for x in 0..map_width {
                let idx = y * map_width + x;
                if idx < blend_indices.len() {
                    let blend_idx = blend_indices[idx];

                    if blend_idx >= 0 && (blend_idx as usize) < self.blend_tiles.len() {
                        let blend_info = &self.blend_tiles[blend_idx as usize];

                        // Get alpha value for this tile (simplified - just use first corner)
                        let alpha = blend_info.alpha[0];

                        // Write to blend texture
                        let tex_x = (x as f32 * scale_x) as u32;
                        let tex_y = (y as f32 * scale_y) as u32;
                        let tex_idx = ((tex_y * TERRAIN_TEXTURE_SIZE + tex_x) * 4) as usize;

                        if tex_idx + 3 < blend_data.len() {
                            blend_data[tex_idx] = alpha;     // R
                            blend_data[tex_idx + 1] = alpha; // G
                            blend_data[tex_idx + 2] = alpha; // B
                            blend_data[tex_idx + 3] = 255;   // A
                        }
                    }
                }
            }
        }

        // Upload blend map to GPU
        self.queue.write_texture(
            ImageCopyTexture {
                texture: &self.blend_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &blend_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(TERRAIN_TEXTURE_SIZE * 4),
                rows_per_image: Some(TERRAIN_TEXTURE_SIZE),
            },
            Extent3d {
                width: TERRAIN_TEXTURE_SIZE,
                height: TERRAIN_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Get texture views for binding
    pub fn get_base_view(&self) -> &TextureView {
        &self.base_texture_view
    }

    pub fn get_detail_view(&self) -> &TextureView {
        &self.detail_texture_view
    }

    pub fn get_blend_view(&self) -> &TextureView {
        &self.blend_texture_view
    }

    pub fn get_base_sampler(&self) -> &Sampler {
        &self.base_sampler
    }

    pub fn get_detail_sampler(&self) -> &Sampler {
        &self.detail_sampler
    }

    pub fn get_blend_sampler(&self) -> &Sampler {
        &self.blend_sampler
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_creation() {
        let tile = TileData::new(64, 64);
        assert_eq!(tile.pixels.len(), 64 * 64 * 4);
    }

    #[test]
    fn test_texture_class() {
        let class = TextureClass {
            global_texture_class: 0,
            first_tile: 0,
            num_tiles: 4,
            width: 64,
            is_blend_edge_tile: false,
            name: "grass".to_string(),
            position_in_texture: (0, 0),
        };

        assert_eq!(class.num_tiles, 4);
    }

    #[test]
    fn test_blend_tile_info() {
        let blend = BlendTileInfo {
            uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            alpha: [255, 255, 255, 255],
            tile_index: 0,
            flip: false,
        };

        assert_eq!(blend.alpha[0], 255);
    }
}
