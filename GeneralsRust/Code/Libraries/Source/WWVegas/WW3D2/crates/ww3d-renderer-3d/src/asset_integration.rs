//! Asset Integration Module
//!
//! This module provides integration between ww3d-assets and ww3d-renderer-3d,
//! enabling loaded textures and meshes to be uploaded to GPU and used in rendering.

use std::sync::{Arc, Mutex};
use ww3d_assets::{Material, TextureBase, TextureFormat as AssetTextureFormat};
use ww3d_core::errors::{W3DError, W3DResult};
use ww3d_gpu::device::GpuDevice;
use ww3d_gpu::texture::GpuTexture;

/// Converts asset texture format to WGPU texture format
pub fn asset_format_to_wgpu(format: AssetTextureFormat) -> wgpu::TextureFormat {
    match format {
        AssetTextureFormat::A8R8G8B8 | AssetTextureFormat::X8R8G8B8 => {
            wgpu::TextureFormat::Rgba8UnormSrgb
        }
        AssetTextureFormat::R5G6B5 => wgpu::TextureFormat::Rgba8UnormSrgb, // Expand to RGBA8
        AssetTextureFormat::A1R5G5B5 | AssetTextureFormat::A4R4G4B4 => {
            wgpu::TextureFormat::Rgba8UnormSrgb // Expand to RGBA8
        }
        AssetTextureFormat::R8G8B8 => wgpu::TextureFormat::Rgba8UnormSrgb, // Add alpha channel
        AssetTextureFormat::L8 => wgpu::TextureFormat::R8Unorm,
        AssetTextureFormat::A8 => wgpu::TextureFormat::R8Unorm,
        AssetTextureFormat::A8L8 => wgpu::TextureFormat::Rg8Unorm,
        AssetTextureFormat::DXT1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
        AssetTextureFormat::DXT3 => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
        AssetTextureFormat::DXT5 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
        AssetTextureFormat::DXT2 | AssetTextureFormat::DXT4 => {
            wgpu::TextureFormat::Bc3RgbaUnormSrgb // DXT2/4 are premultiplied alpha variants
        }
        AssetTextureFormat::Unknown => wgpu::TextureFormat::Rgba8UnormSrgb,
    }
}

/// Converts texture data from asset format to RGBA8 if needed
fn convert_texture_data(
    data: &[u8],
    width: u32,
    height: u32,
    format: AssetTextureFormat,
) -> Vec<u8> {
    match format {
        // Already in a compatible format (compressed textures)
        AssetTextureFormat::DXT1
        | AssetTextureFormat::DXT2
        | AssetTextureFormat::DXT3
        | AssetTextureFormat::DXT4
        | AssetTextureFormat::DXT5 => data.to_vec(),

        // Direct copy formats (already RGBA8 or compatible)
        AssetTextureFormat::A8R8G8B8 => {
            // Convert ARGB to RGBA
            let mut rgba = Vec::with_capacity(data.len());
            for pixel in data.chunks_exact(4) {
                rgba.push(pixel[2]); // R
                rgba.push(pixel[1]); // G
                rgba.push(pixel[0]); // B
                rgba.push(pixel[3]); // A
            }
            rgba
        }

        AssetTextureFormat::X8R8G8B8 => {
            // Convert XRGB to RGBA (opaque)
            let mut rgba = Vec::with_capacity(data.len());
            for pixel in data.chunks_exact(4) {
                rgba.push(pixel[2]); // R
                rgba.push(pixel[1]); // G
                rgba.push(pixel[0]); // B
                rgba.push(255); // A (opaque)
            }
            rgba
        }

        AssetTextureFormat::R8G8B8 => {
            // Convert RGB to RGBA
            let mut rgba = Vec::with_capacity(((width * height * 4) as usize).max(data.len()));
            for pixel in data.chunks_exact(3) {
                rgba.push(pixel[0]); // R
                rgba.push(pixel[1]); // G
                rgba.push(pixel[2]); // B
                rgba.push(255); // A (opaque)
            }
            rgba
        }

        AssetTextureFormat::R5G6B5 => {
            // Convert R5G6B5 to RGBA8
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for pixel in data.chunks_exact(2) {
                let value = u16::from_le_bytes([pixel[0], pixel[1]]);
                let r = ((value >> 11) & 0x1F) as u8;
                let g = ((value >> 5) & 0x3F) as u8;
                let b = (value & 0x1F) as u8;

                rgba.push((r << 3) | (r >> 2)); // R (5 bits to 8 bits)
                rgba.push((g << 2) | (g >> 4)); // G (6 bits to 8 bits)
                rgba.push((b << 3) | (b >> 2)); // B (5 bits to 8 bits)
                rgba.push(255); // A (opaque)
            }
            rgba
        }

        AssetTextureFormat::A1R5G5B5 => {
            // Convert A1R5G5B5 to RGBA8
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for pixel in data.chunks_exact(2) {
                let value = u16::from_le_bytes([pixel[0], pixel[1]]);
                let a = ((value >> 15) & 0x01) as u8;
                let r = ((value >> 10) & 0x1F) as u8;
                let g = ((value >> 5) & 0x1F) as u8;
                let b = (value & 0x1F) as u8;

                rgba.push((r << 3) | (r >> 2)); // R
                rgba.push((g << 3) | (g >> 2)); // G
                rgba.push((b << 3) | (b >> 2)); // B
                rgba.push(a * 255); // A (1 bit to 8 bits)
            }
            rgba
        }

        AssetTextureFormat::A4R4G4B4 => {
            // Convert A4R4G4B4 to RGBA8
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for pixel in data.chunks_exact(2) {
                let value = u16::from_le_bytes([pixel[0], pixel[1]]);
                let a = ((value >> 12) & 0x0F) as u8;
                let r = ((value >> 8) & 0x0F) as u8;
                let g = ((value >> 4) & 0x0F) as u8;
                let b = (value & 0x0F) as u8;

                rgba.push((r << 4) | r); // R (4 bits to 8 bits)
                rgba.push((g << 4) | g); // G
                rgba.push((b << 4) | b); // B
                rgba.push((a << 4) | a); // A
            }
            rgba
        }

        AssetTextureFormat::L8 | AssetTextureFormat::A8 => {
            // Single channel format - keep as-is
            data.to_vec()
        }

        AssetTextureFormat::A8L8 => {
            // Two channel format - keep as-is
            data.to_vec()
        }

        AssetTextureFormat::Unknown => {
            // Fallback: assume RGBA8
            data.to_vec()
        }
    }
}

/// Upload asset texture to GPU
pub fn upload_texture_to_gpu(
    texture: &TextureBase,
    device: &Arc<GpuDevice>,
) -> W3DResult<Arc<GpuTexture>> {
    let wgpu_format = asset_format_to_wgpu(texture.format);

    // Create GPU texture descriptor
    let desc = wgpu::TextureDescriptor {
        label: Some(&texture.name),
        size: wgpu::Extent3d {
            width: texture.width,
            height: texture.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: texture.mip_level_count(),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu_format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    };

    let mut gpu_texture = GpuTexture::new(device, &desc)
        .map_err(|e| W3DError::RenderError(format!("Failed to create GPU texture: {:?}", e)))?;

    // Upload each mip level
    for (level_index, mip_level) in texture.mip_levels.iter().enumerate() {
        if mip_level.data.is_empty() {
            continue;
        }

        // Convert texture data if needed
        let converted_data = convert_texture_data(
            &mip_level.data,
            mip_level.width,
            mip_level.height,
            texture.format,
        );

        // Upload to GPU
        gpu_texture.write_data(
            device,
            &converted_data,
            wgpu::Origin3d::ZERO,
            wgpu::Extent3d {
                width: mip_level.width,
                height: mip_level.height,
                depth_or_array_layers: 1,
            },
            level_index as u32,
        );
    }

    Ok(Arc::new(gpu_texture))
}

/// Texture upload manager that caches GPU textures
pub struct TextureUploadManager {
    device: Arc<GpuDevice>,
    cache: Mutex<std::collections::HashMap<String, Arc<GpuTexture>>>,
}

impl TextureUploadManager {
    /// Create a new texture upload manager
    pub fn new(device: Arc<GpuDevice>) -> Self {
        Self {
            device,
            cache: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Get or upload a texture to GPU
    pub fn get_or_upload(&self, texture: &Arc<TextureBase>) -> W3DResult<Arc<GpuTexture>> {
        let key = texture.name.clone();

        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(gpu_texture) = cache.get(&key) {
                return Ok(Arc::clone(gpu_texture));
            }
        }

        // Upload texture
        let gpu_texture = upload_texture_to_gpu(texture, &self.device)?;

        // Cache it
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(key, Arc::clone(&gpu_texture));
        }

        Ok(gpu_texture)
    }

    /// Clear the texture cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, u64) {
        let cache = self.cache.lock().unwrap();
        let count = cache.len();
        let total_memory: u64 = cache
            .values()
            .map(|t| (t.width() * t.height() * 4) as u64) // Assume 4 bytes per pixel
            .sum();
        (count, total_memory)
    }
}

/// Material binding helper for rendering
pub struct MaterialBinding {
    pub material: Material,
    pub textures: Vec<Arc<GpuTexture>>,
}

impl MaterialBinding {
    /// Create material bindings from asset material
    pub fn from_asset_material(
        material: &Material,
        texture_manager: &TextureUploadManager,
        asset_textures: &[Arc<TextureBase>],
    ) -> W3DResult<Self> {
        let mut textures = Vec::new();

        // Upload textures referenced by this material
        // This is simplified - real implementation would look up textures by name
        for texture in asset_textures {
            textures.push(texture_manager.get_or_upload(texture)?);
        }

        Ok(Self {
            material: material.clone(),
            textures,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_conversion() {
        assert_eq!(
            asset_format_to_wgpu(AssetTextureFormat::A8R8G8B8),
            wgpu::TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            asset_format_to_wgpu(AssetTextureFormat::DXT1),
            wgpu::TextureFormat::Bc1RgbaUnormSrgb
        );
        assert_eq!(
            asset_format_to_wgpu(AssetTextureFormat::DXT5),
            wgpu::TextureFormat::Bc3RgbaUnormSrgb
        );
    }

    #[test]
    fn test_argb_to_rgba_conversion() {
        let argb_data = vec![
            0xAA, 0xBB, 0xCC, 0xDD, // ARGB pixel (A=DD, R=AA, G=BB, B=CC)
        ];
        let rgba = convert_texture_data(&argb_data, 1, 1, AssetTextureFormat::A8R8G8B8);
        assert_eq!(rgba, vec![0xCC, 0xBB, 0xAA, 0xDD]); // RGBA
    }

    #[test]
    fn test_rgb_to_rgba_conversion() {
        let rgb_data = vec![0xFF, 0x00, 0x80]; // RGB pixel
        let rgba = convert_texture_data(&rgb_data, 1, 1, AssetTextureFormat::R8G8B8);
        assert_eq!(rgba, vec![0xFF, 0x00, 0x80, 0xFF]); // RGBA with opaque alpha
    }
}
