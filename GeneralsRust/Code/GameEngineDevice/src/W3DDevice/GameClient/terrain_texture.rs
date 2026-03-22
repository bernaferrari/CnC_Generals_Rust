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
use anyhow::Result;

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

#[derive(Debug, Clone, Copy, Default)]
struct AtlasPlacement {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
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

    /// Cached source tile placements after atlas updates
    source_tile_positions: Vec<Option<AtlasPlacement>>,

    /// Cached edge tile placements after atlas updates
    edge_tile_positions: Vec<Option<AtlasPlacement>>,

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
            edge_tiles: vec![None; NUM_BLEND_TILES],
            texture_classes: Vec::new(),
            source_tile_positions: vec![None; NUM_SOURCE_TILES],
            edge_tile_positions: vec![None; NUM_BLEND_TILES],
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
            // C++ terrain textures clamp at the atlas edge and default to point sampling.
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: FilterMode::Nearest,
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
        if index >= NUM_BLEND_TILES {
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
    pub fn update_base_atlas(&mut self) -> Result<()> {
        let mut atlas_data = vec![0u8; (TERRAIN_TEXTURE_SIZE * TERRAIN_TEXTURE_SIZE * 4) as usize];
        let source_tiles = &self.source_tiles;
        let source_tile_positions = &mut self.source_tile_positions;
        let texture_classes = &mut self.texture_classes;
        source_tile_positions.fill(None);
        Self::pack_tiles_into_atlas(source_tiles, source_tile_positions, texture_classes, &mut atlas_data);

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
    pub fn update_detail_atlas(&mut self) -> Result<()> {
        let mut atlas_data = vec![0u8; (TERRAIN_TEXTURE_SIZE * TERRAIN_TEXTURE_SIZE * 4) as usize];
        let edge_tiles = &self.edge_tiles;
        let edge_tile_positions = &mut self.edge_tile_positions;
        let texture_classes = &mut self.texture_classes;
        edge_tile_positions.fill(None);
        Self::pack_tiles_into_atlas(edge_tiles, edge_tile_positions, texture_classes, &mut atlas_data);

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
        if tile_index < 0 {
            return [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        }

        if let Some(Some(placement)) = self.source_tile_positions.get(tile_index as usize) {
            let u_offset = placement.x as f32 / TERRAIN_TEXTURE_SIZE as f32;
            let v_offset = placement.y as f32 / TERRAIN_TEXTURE_SIZE as f32;
            let u_size = placement.width as f32 / TERRAIN_TEXTURE_SIZE as f32;
            let v_size = placement.height as f32 / TERRAIN_TEXTURE_SIZE as f32;

            return [
                [u_offset, v_offset],
                [u_offset + u_size, v_offset],
                [u_offset + u_size, v_offset + v_size],
                [u_offset, v_offset + v_size],
            ];
        }

        // Find the tile in the texture classes
        for class in &self.texture_classes {
            if tile_index >= class.first_tile as i16 && tile_index < (class.first_tile + class.num_tiles) as i16 {
                let tile_offset = tile_index - class.first_tile as i16;
                let tile_width = class.width.max(1);
                let tile_height = class.width.max(1);
                let stride = tile_width + Self::atlas_border();
                let tile_x = class.position_in_texture.0 + tile_offset.max(0) as u32 * stride;
                let tile_y = class.position_in_texture.1;
                let u_offset = tile_x as f32 / TERRAIN_TEXTURE_SIZE as f32;
                let v_offset = tile_y as f32 / TERRAIN_TEXTURE_SIZE as f32;
                let u_size = tile_width as f32 / TERRAIN_TEXTURE_SIZE as f32;
                let v_size = tile_height as f32 / TERRAIN_TEXTURE_SIZE as f32;

                return [
                    [u_offset, v_offset],
                    [u_offset + u_size, v_offset],
                    [u_offset + u_size, v_offset + v_size],
                    [u_offset, v_offset + v_size],
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

    fn atlas_border() -> u32 {
        (TILE_OFFSET as u32) / 2
    }

    fn tile_outer_extent(tile: &TileData) -> (u32, u32) {
        let border = Self::atlas_border();
        (
            tile.width.saturating_add(border * 2),
            tile.height.saturating_add(border * 2),
        )
    }

    fn find_texture_class_for_tile(texture_classes: &[TextureClass], tile_index: usize) -> Option<usize> {
        texture_classes.iter().enumerate().find_map(|(idx, class)| {
            let start = class.first_tile;
            let end = class.first_tile.saturating_add(class.num_tiles);
            if tile_index >= start && tile_index < end {
                Some(idx)
            } else {
                None
            }
        })
    }

    fn write_tile_with_border(
        atlas_data: &mut [u8],
        atlas_size: u32,
        outer_x: u32,
        outer_y: u32,
        tile: &TileData,
    ) -> Option<AtlasPlacement> {
        let border = Self::atlas_border();
        let (outer_width, outer_height) = Self::tile_outer_extent(tile);

        if outer_x.checked_add(outer_width)? > atlas_size || outer_y.checked_add(outer_height)? > atlas_size {
            return None;
        }

        if tile.width == 0 || tile.height == 0 {
            return None;
        }

        let tile_width = tile.width as usize;
        let inner_x = outer_x + border;
        let inner_y = outer_y + border;

        for oy in 0..outer_height {
            let src_y = oy.saturating_sub(border).min(tile.height - 1) as usize;
            for ox in 0..outer_width {
                let src_x = ox.saturating_sub(border).min(tile.width - 1) as usize;
                let src_idx = ((src_y * tile_width + src_x) * 4) as usize;
                let dst_x = outer_x + ox;
                let dst_y = outer_y + oy;
                let dst_idx = ((dst_y * atlas_size + dst_x) * 4) as usize;

                if src_idx + 3 < tile.pixels.len() && dst_idx + 3 < atlas_data.len() {
                    atlas_data[dst_idx..dst_idx + 4].copy_from_slice(&tile.pixels[src_idx..src_idx + 4]);
                }
            }
        }

        Some(AtlasPlacement {
            x: inner_x,
            y: inner_y,
            width: tile.width,
            height: tile.height,
        })
    }

    fn pack_tiles_into_atlas(
        tiles: &[Option<TileData>],
        placements: &mut [Option<AtlasPlacement>],
        texture_classes: &mut [TextureClass],
        atlas_data: &mut [u8],
    ) {
        placements.fill(None);

        let mut used = vec![false; tiles.len()];
        let mut tile_order = Vec::with_capacity(tiles.len());

        let mut class_order: Vec<usize> = (0..texture_classes.len()).collect();
        class_order.sort_by(|&lhs, &rhs| {
            let a = &texture_classes[lhs];
            let b = &texture_classes[rhs];
            (
                a.global_texture_class,
                a.first_tile,
                a.num_tiles,
                a.is_blend_edge_tile,
                a.name.as_str(),
            )
                .cmp(&(
                    b.global_texture_class,
                    b.first_tile,
                    b.num_tiles,
                    b.is_blend_edge_tile,
                    b.name.as_str(),
                ))
        });

        for class_idx in class_order {
            let class = &texture_classes[class_idx];
            let start = class.first_tile;
            let end = class.first_tile.saturating_add(class.num_tiles);
            for tile_idx in start..end {
                if tile_idx < tiles.len() && tiles[tile_idx].is_some() && !used[tile_idx] {
                    tile_order.push(tile_idx);
                    used[tile_idx] = true;
                }
            }
        }

        for tile_idx in 0..tiles.len() {
            if tiles[tile_idx].is_some() && !used[tile_idx] {
                tile_order.push(tile_idx);
                used[tile_idx] = true;
            }
        }

        let mut cursor_x = 0u32;
        let mut cursor_y = 0u32;
        let mut row_height = 0u32;
        let mut current_class: Option<usize> = None;

        for tile_idx in tile_order {
            let Some(tile) = tiles[tile_idx].as_ref() else {
                continue;
            };

            if let Some(class_idx) = Self::find_texture_class_for_tile(texture_classes, tile_idx) {
                if current_class != Some(class_idx) {
                    let class = &mut texture_classes[class_idx];
                    if class.position_in_texture == (0, 0) && (cursor_x != 0 || cursor_y != 0) {
                        class.position_in_texture = (cursor_x, cursor_y);
                    }

                    cursor_x = class.position_in_texture.0;
                    cursor_y = class.position_in_texture.1;
                    current_class = Some(class_idx);
                }
            } else {
                current_class = None;
            }

            let (outer_width, outer_height) = Self::tile_outer_extent(tile);
            if cursor_x + outer_width > TERRAIN_TEXTURE_SIZE {
                cursor_x = 0;
                cursor_y = cursor_y.saturating_add(row_height);
                row_height = 0;
            }

            if let Some(placement) = Self::write_tile_with_border(
                atlas_data,
                TERRAIN_TEXTURE_SIZE,
                cursor_x,
                cursor_y,
                tile,
            ) {
                placements[tile_idx] = Some(placement);
            }

            cursor_x = cursor_x.saturating_add(outer_width);
            row_height = row_height.max(outer_height);
        }
    }

    /// Resolve a blend tile's four corner alpha values into a single mask value.
    /// This keeps the output stable while avoiding the top-left bias from alpha[0].
    fn resolve_blend_alpha(alpha: [u8; 4]) -> u8 {
        let top = u16::from(alpha[0]) + u16::from(alpha[1]);
        let bottom = u16::from(alpha[3]) + u16::from(alpha[2]);
        ((top + bottom + 2) / 4) as u8
    }

    #[inline]
    fn clamp_texel(coord: f32) -> u32 {
        coord
            .floor()
            .max(0.0)
            .min((TERRAIN_TEXTURE_SIZE - 1) as f32) as u32
    }

    /// Generate blend map texture
    /// Corresponds to C++ AlphaEdgeTextureClass::update
    pub fn update_blend_map(&self, map_width: usize, map_height: usize, blend_indices: &[i16]) -> Result<()> {
        if map_width == 0 || map_height == 0 {
            return Ok(());
        }

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

                        // Resolve the four corner alphas into one representative mask value.
                        let alpha = Self::resolve_blend_alpha(blend_info.alpha);

                        // Write to blend texture
                        let tex_x = Self::clamp_texel(x as f32 * scale_x);
                        let tex_y = Self::clamp_texel(y as f32 * scale_y);
                        let tex_idx = ((tex_y * TERRAIN_TEXTURE_SIZE + tex_x) * 4) as usize;

                        if tex_idx + 3 < blend_data.len() {
                            blend_data[tex_idx] = alpha;     // R
                            blend_data[tex_idx + 1] = alpha; // G
                            blend_data[tex_idx + 2] = alpha; // B
                            blend_data[tex_idx + 3] = alpha;  // A
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

    #[test]
    fn test_resolve_blend_alpha_uses_all_corners() {
        assert_eq!(TerrainTextureManager::resolve_blend_alpha([0, 255, 255, 255]), 191);
        assert_eq!(TerrainTextureManager::resolve_blend_alpha([255, 0, 0, 0]), 64);
    }

    #[test]
    fn test_write_tile_with_border_clones_edge_texels() {
        let tile = TileData::from_pixels(1, 1, vec![10, 20, 30, 40]);
        let mut atlas = vec![0u8; (32 * 32 * 4) as usize];

        let placement = TerrainTextureManager::write_tile_with_border(&mut atlas, 32, 0, 0, &tile)
            .expect("tile should fit");

        let outer_pixel = &atlas[0..4];
        let inner_idx = ((placement.y * 32 + placement.x) * 4) as usize;
        let inner_pixel = &atlas[inner_idx..inner_idx + 4];

        assert_eq!(placement.x, (TILE_OFFSET as u32) / 2);
        assert_eq!(placement.y, (TILE_OFFSET as u32) / 2);
        assert_eq!(outer_pixel, &[10, 20, 30, 40]);
        assert_eq!(inner_pixel, &[10, 20, 30, 40]);
    }
}
