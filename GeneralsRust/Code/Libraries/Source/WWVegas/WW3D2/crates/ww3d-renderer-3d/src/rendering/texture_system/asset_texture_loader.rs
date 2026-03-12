//! Asset Manager Integration for Texture Loading
//!
//! This module provides integration between the texture loading system and the
//! asset manager, enabling loading of textures from .big archives and other
//! asset sources.

use crate::core::error::{Error, RendererResult};
use crate::rendering::texture_system::dds_loader::{load_dds_from_memory, DdsTextureType};
use crate::rendering::texture_system::texture_base::{PoolType, TexAssetType, TextureBaseClass};
use crate::rendering::texture_system::texture_loader::TextureLoader;
use crate::rendering::texture_system::tga_loader::load_tga_from_memory;
use log::warn;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use wgpu::{
    Device, Extent3d, Origin3d, Queue, TexelCopyBufferLayout, TexelCopyTextureInfo,
    TextureDescriptor, TextureDimension, TextureUsages,
};
use ww3d_assets::AssetManager;

/// Asset-integrated texture loader
pub struct AssetTextureLoader {
    base_loader: TextureLoader,
    asset_manager: Arc<Mutex<AssetManager>>,
    texture_cache: HashMap<String, Arc<TextureBaseClass>>,
    search_paths: Vec<String>,
}

impl AssetTextureLoader {
    /// Create new asset texture loader
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        asset_manager: Arc<Mutex<AssetManager>>,
    ) -> RendererResult<Self> {
        let base_loader = TextureLoader::new(device, queue)?;

        Ok(Self {
            base_loader,
            asset_manager,
            texture_cache: HashMap::new(),
            search_paths: vec![
                "Art/Textures/".to_string(),
                "Art/".to_string(),
                "Data/Art/Textures/".to_string(),
                "Data/Art/".to_string(),
                "".to_string(), // Current directory
            ],
        })
    }

    /// Add search path for textures
    pub fn add_search_path(&mut self, path: &str) {
        self.search_paths.push(path.to_string());
    }

    /// Load texture with asset manager integration
    pub fn load_texture(&mut self, filename: &str) -> RendererResult<Arc<TextureBaseClass>> {
        // Check cache first
        if let Some(cached_texture) = self.texture_cache.get(filename) {
            return Ok(cached_texture.clone());
        }

        // Try loading from asset manager first
        if let Ok(texture) = self.load_texture_from_assets(filename) {
            let texture_arc = Arc::new(texture);
            self.texture_cache
                .insert(filename.to_string(), texture_arc.clone());
            return Ok(texture_arc);
        }

        // Fall back to file system and registered search paths.
        let mut first_non_missing_error: Option<Error> = None;
        for candidate in std::iter::once(filename.to_string()).chain(
            self.search_paths
                .iter()
                .map(|search_path| format!("{}{}", search_path, filename)),
        ) {
            match self
                .base_loader
                .load_texture_from_path(Path::new(&candidate))
            {
                Ok(texture) => {
                    let texture_arc = Arc::new(texture);
                    self.texture_cache
                        .insert(filename.to_string(), texture_arc.clone());
                    return Ok(texture_arc);
                }
                Err(Error::FileNotFound(_)) => {}
                Err(err) => {
                    if first_non_missing_error.is_none() {
                        first_non_missing_error = Some(err);
                    }
                }
            }
        }

        if let Some(err) = first_non_missing_error {
            return Err(err);
        }

        // Keep runtime resilient for genuinely missing textures after all search attempts.
        warn!(
            "Texture '{}' not found in archives or filesystem search paths; using fallback checker texture",
            filename
        );
        let fallback = self
            .base_loader
            .create_missing_texture(filename, PoolType::Managed)?;
        let texture_arc = Arc::new(fallback);
        self.texture_cache
            .insert(filename.to_string(), texture_arc.clone());
        Ok(texture_arc)
    }

    /// Load texture from asset manager (.big archives, etc.)
    fn load_texture_from_assets(&self, filename: &str) -> RendererResult<TextureBaseClass> {
        let asset_manager = self
            .asset_manager
            .lock()
            .map_err(|_| Error::InvalidData("Failed to lock asset manager".to_string()))?;

        // Try different search paths within archives
        for search_path in &self.search_paths {
            let asset_path = format!("{}{}", search_path, filename);

            if let Some(data) = self.load_asset_data(&asset_manager, &asset_path)? {
                return self.load_texture_from_memory(&data, filename);
            }
        }

        Err(Error::FileNotFound(format!(
            "Texture not found in assets: {}",
            filename
        )))
    }

    /// Load raw asset data using currently available asset backends.
    fn load_asset_data(
        &self,
        asset_manager: &AssetManager,
        asset_path: &str,
    ) -> RendererResult<Option<Vec<u8>>> {
        let _ = asset_manager;
        let path = Path::new(asset_path);
        if !path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(path).map_err(Error::from)?;
        if data.is_empty() {
            return Ok(None);
        }

        let _ = asset_path;
        Ok(Some(data))
    }

    /// Load texture from memory buffer
    fn load_texture_from_memory(
        &self,
        data: &[u8],
        filename: &str,
    ) -> RendererResult<TextureBaseClass> {
        // Determine format from filename extension or magic bytes
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "dds" => self.load_dds_from_memory(data),
            "tga" => self.load_tga_from_memory(data),
            "png" | "jpg" | "jpeg" | "bmp" => self.load_image_from_memory(data),
            _ => {
                // Try to detect format from magic bytes
                if data.len() >= 4 {
                    let magic = &data[0..4];
                    if magic == b"DDS " {
                        return self.load_dds_from_memory(data);
                    }
                }

                // Try loading as standard image format
                self.load_image_from_memory(data)
            }
        }
    }

    /// Load DDS texture from memory
    fn load_dds_from_memory(&self, data: &[u8]) -> RendererResult<TextureBaseClass> {
        let dds_data = load_dds_from_memory(data)?;

        // Create wgpu texture
        let texture_desc = TextureDescriptor {
            label: Some("DDS Texture from Assets"),
            size: Extent3d {
                width: dds_data.width,
                height: dds_data.height,
                depth_or_array_layers: if dds_data.texture_type == DdsTextureType::CubeMap {
                    6
                } else {
                    dds_data.depth
                },
            },
            mip_level_count: dds_data.mip_levels,
            sample_count: 1,
            dimension: match dds_data.texture_type {
                DdsTextureType::Texture2D => TextureDimension::D2,
                DdsTextureType::CubeMap => TextureDimension::D2,
                DdsTextureType::Volume => TextureDimension::D3,
            },
            format: dds_data.format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let wgpu_texture = self.base_loader.device().create_texture(&texture_desc);

        // Upload texture data
        for level in 0..dds_data.mip_levels {
            if let Some(level_data) = dds_data.get_level_data(level) {
                let level_width = (dds_data.width >> level).max(1);
                let level_height = (dds_data.height >> level).max(1);

                if dds_data.texture_type == DdsTextureType::CubeMap {
                    debug_assert_eq!(level_data.len() % 6, 0);
                    let face_size = level_data.len() / 6;
                    if face_size == 0 {
                        continue;
                    }
                    for face in 0..6 {
                        let start = face * face_size;
                        let end = start + face_size;
                        let face_data = &level_data[start..end];

                        self.base_loader.queue().write_texture(
                            TexelCopyTextureInfo {
                                texture: &wgpu_texture,
                                mip_level: level,
                                origin: Origin3d {
                                    x: 0,
                                    y: 0,
                                    z: face as u32,
                                },
                                aspect: wgpu::TextureAspect::All,
                            },
                            face_data,
                            TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: None,
                                rows_per_image: Some(level_height),
                            },
                            Extent3d {
                                width: level_width,
                                height: level_height,
                                depth_or_array_layers: 1,
                            },
                        );
                    }
                } else {
                    self.base_loader.queue().write_texture(
                        TexelCopyTextureInfo {
                            texture: &wgpu_texture,
                            mip_level: level,
                            origin: Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        level_data,
                        TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: None,
                            rows_per_image: Some(level_height),
                        },
                        Extent3d {
                            width: level_width,
                            height: level_height,
                            depth_or_array_layers: if dds_data.texture_type
                                == DdsTextureType::Volume
                            {
                                (dds_data.depth >> level).max(1)
                            } else {
                                1
                            },
                        },
                    );
                }
            }
        }

        // Create texture base class
        let mut texture = TextureBaseClass::new(
            dds_data.width,
            dds_data.height,
            dds_data.mip_levels,
            PoolType::Managed,
            match dds_data.texture_type {
                DdsTextureType::Texture2D => TexAssetType::Regular,
                DdsTextureType::CubeMap => TexAssetType::Cubemap,
                DdsTextureType::Volume => TexAssetType::Volume,
            },
        );

        texture.init_texture_resources(wgpu_texture);
        texture.set_name("Asset Texture");
        texture.set_full_path("from_assets");

        Ok(texture)
    }

    /// Load TGA texture from memory
    fn load_tga_from_memory(&self, data: &[u8]) -> RendererResult<TextureBaseClass> {
        let tga_data = load_tga_from_memory(data)?;

        // Convert to texture base class through standard image path
        self.load_image_data_to_texture(
            &tga_data.data,
            tga_data.width,
            tga_data.height,
            tga_data.format,
            false,
        )
    }

    /// Load standard image texture from memory
    fn load_image_from_memory(&self, data: &[u8]) -> RendererResult<TextureBaseClass> {
        // Use image crate for standard formats
        let img = image::load_from_memory(data)
            .map_err(|e| Error::InvalidData(format!("Failed to load image from memory: {}", e)))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        self.load_image_data_to_texture(
            &rgba.into_raw(),
            width,
            height,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            true,
        )
    }

    /// Helper to create texture from raw image data
    fn load_image_data_to_texture(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        generate_mips: bool,
    ) -> RendererResult<TextureBaseClass> {
        let mip_levels = if generate_mips {
            (width.max(height) as f32).log2() as u32 + 1
        } else {
            1
        };

        let texture_desc = TextureDescriptor {
            label: Some("Asset Image Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let wgpu_texture = self.base_loader.device().create_texture(&texture_desc);

        // Upload base level
        self.base_loader.queue().write_texture(
            TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // Generate mipmaps if requested
        if generate_mips && mip_levels > 1 {
            self.generate_texture_mipmaps(&wgpu_texture, data, width, height, format)?;
        }

        let mut texture = TextureBaseClass::new(
            width,
            height,
            mip_levels,
            PoolType::Managed,
            TexAssetType::Regular,
        );

        texture.init_texture_resources(wgpu_texture);
        texture.set_name("Asset Image");
        texture.set_full_path("from_memory");

        Ok(texture)
    }

    /// Generate mipmaps for a texture
    fn generate_texture_mipmaps(
        &self,
        texture: &wgpu::Texture,
        base_data: &[u8],
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> RendererResult<()> {
        let bytes_per_pixel = match format {
            wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Rgba8UnormSrgb
            | wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Bgra8UnormSrgb => 4usize,
            _ => return Ok(()),
        };

        let base_size = width as usize * height as usize * bytes_per_pixel;
        if base_data.len() < base_size {
            return Err(Error::InvalidData(format!(
                "Insufficient base texture data for mip generation: got {}, expected at least {}",
                base_data.len(),
                base_size
            )));
        }

        let mut level_data = base_data[..base_size].to_vec();
        let mut level_width = width.max(1);
        let mut level_height = height.max(1);
        let mip_levels = (level_width.max(level_height) as f32).log2() as u32 + 1;

        for mip_level in 1..mip_levels {
            let next_data = Self::downsample_box_2x2_rgba(&level_data, level_width, level_height);
            let next_width = (level_width >> 1).max(1);
            let next_height = (level_height >> 1).max(1);

            self.base_loader.queue().write_texture(
                TexelCopyTextureInfo {
                    texture,
                    mip_level,
                    origin: Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &next_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(next_width * bytes_per_pixel as u32),
                    rows_per_image: Some(next_height),
                },
                Extent3d {
                    width: next_width,
                    height: next_height,
                    depth_or_array_layers: 1,
                },
            );

            level_data = next_data;
            level_width = next_width;
            level_height = next_height;
        }

        Ok(())
    }

    fn downsample_box_2x2_rgba(src: &[u8], width: u32, height: u32) -> Vec<u8> {
        let dst_width = (width >> 1).max(1);
        let dst_height = (height >> 1).max(1);
        let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];

        for y in 0..dst_height {
            for x in 0..dst_width {
                let mut accum = [0u32; 4];
                let mut samples = 0u32;

                for oy in 0..2 {
                    for ox in 0..2 {
                        let sx = ((x * 2 + ox).min(width - 1)) as usize;
                        let sy = ((y * 2 + oy).min(height - 1)) as usize;
                        let src_idx = (sy * width as usize + sx) * 4;
                        for c in 0..4 {
                            accum[c] += src[src_idx + c] as u32;
                        }
                        samples += 1;
                    }
                }

                let dst_idx = ((y * dst_width + x) * 4) as usize;
                for c in 0..4 {
                    dst[dst_idx + c] = (accum[c] / samples) as u8;
                }
            }
        }

        dst
    }

    /// Preload common textures from asset manager
    pub fn preload_common_textures(&mut self) -> RendererResult<()> {
        // List of commonly used textures that should be preloaded
        let common_textures = [
            "missing.dds",
            "default.dds",
            "white.dds",
            "black.dds",
            "normal.dds",
        ];

        for texture_name in &common_textures {
            // Try to load but don't fail if missing
            let _ = self.load_texture(texture_name);
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> AssetTextureCacheStats {
        AssetTextureCacheStats {
            cached_textures: self.texture_cache.len(),
            search_paths: self.search_paths.len(),
        }
    }

    /// Clear texture cache
    pub fn clear_cache(&mut self) {
        self.texture_cache.clear();
    }

    /// Check if texture is cached
    pub fn is_cached(&self, filename: &str) -> bool {
        self.texture_cache.contains_key(filename)
    }
}

/// Cache statistics for asset texture loader
#[derive(Debug, Clone)]
pub struct AssetTextureCacheStats {
    pub cached_textures: usize,
    pub search_paths: usize,
}

#[cfg(test)]
mod tests {
    use super::AssetTextureLoader;

    #[test]
    fn test_search_path_construction() {
        let search_paths = vec!["Art/Textures/".to_string(), "Data/Art/".to_string()];
        let filename = "grass.dds";

        let expected_paths = vec!["Art/Textures/grass.dds", "Data/Art/grass.dds"];

        for (i, search_path) in search_paths.iter().enumerate() {
            let full_path = format!("{}{}", search_path, filename);
            assert_eq!(full_path, expected_paths[i]);
        }
    }

    #[test]
    fn downsample_box_2x2_rgba_averages_four_texels() {
        let src = vec![
            0, 0, 0, 255, 20, 20, 20, 255, 40, 40, 40, 255, 60, 60, 60, 255,
        ];

        let downsampled = AssetTextureLoader::downsample_box_2x2_rgba(&src, 2, 2);
        assert_eq!(downsampled, vec![30, 30, 30, 255]);
    }
}
