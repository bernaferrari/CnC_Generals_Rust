//! WGPU Texture Management
//!
//! This module handles texture creation, management, and operations for WGPU,
//! equivalent to the DirectX8 texture functionality.

use crate::core::error::{Error, Result};
use crate::core::WW3DFormat;
use image::{ImageBuffer, Rgba};
use std::sync::Arc;
use wgpu::{
    Device, Queue, Sampler, Texture, TextureDescriptor, TextureView, TextureViewDescriptor,
};
/// WGPU Texture wrapper
#[derive(Debug)]
pub struct WgpuTexture {
    /// WGPU texture handle
    texture: Arc<Texture>,
    /// Texture view
    view: Arc<TextureView>,
    /// Texture sampler
    sampler: Arc<Sampler>,
    /// Texture format
    format: wgpu::TextureFormat,
    /// Texture dimensions
    size: wgpu::Extent3d,
    /// Mip level count
    mip_levels: u32,
    /// Reference count
    ref_count: std::sync::atomic::AtomicU32,
}

impl WgpuTexture {
    /// Create a new texture
    pub fn new(device: &Device, descriptor: &TextureDescriptor) -> Result<Self> {
        let texture = device.create_texture(descriptor);
        let view = texture.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            texture: Arc::new(texture),
            view: Arc::new(view),
            sampler: Arc::new(sampler),
            format: descriptor.format,
            size: descriptor.size,
            mip_levels: descriptor.mip_level_count,
            ref_count: std::sync::atomic::AtomicU32::new(1),
        })
    }

    /// Create texture from image data
    pub fn from_image_rgba8(
        device: &Device,
        queue: &Queue,
        image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
        mip_levels: u32,
        label: Option<&str>,
    ) -> Result<Self> {
        let dimensions = image.dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let descriptor = TextureDescriptor {
            label,
            size,
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = Self::new(device, &descriptor)?;

        // Upload image data
        let data = image.as_raw();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );

        Ok(texture)
    }

    /// Create render target texture
    pub fn render_target(
        device: &Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: Option<&str>,
    ) -> Result<Self> {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let descriptor = TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        Self::new(device, &descriptor)
    }

    /// Create depth texture
    pub fn depth_texture(
        device: &Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: Option<&str>,
    ) -> Result<Self> {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let descriptor = TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };

        Self::new(device, &descriptor)
    }

    /// Get the WGPU texture handle
    pub fn texture(&self) -> &Arc<Texture> {
        &self.texture
    }

    /// Get the texture view
    pub fn view(&self) -> &Arc<TextureView> {
        &self.view
    }

    /// Get the texture sampler
    pub fn sampler(&self) -> &Arc<Sampler> {
        &self.sampler
    }

    /// Get texture format
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get texture size
    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }

    /// Get width
    pub fn width(&self) -> u32 {
        self.size.width
    }

    /// Get height
    pub fn height(&self) -> u32 {
        self.size.height
    }

    /// Get mip levels
    pub fn mip_levels(&self) -> u32 {
        self.mip_levels
    }

    /// Update texture data
    pub fn update_data(
        &self,
        queue: &Queue,
        data: &[u8],
        mip_level: u32,
        origin: wgpu::Origin3d,
        size: wgpu::Extent3d,
    ) -> Result<()> {
        if mip_level >= self.mip_levels {
            return Err(Error::InvalidMipLevel(format!(
                "Mip level {} >= max levels {}",
                mip_level, self.mip_levels
            )));
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.bytes_per_row_for_mip_level(mip_level)),
                rows_per_image: Some(size.height),
            },
            size,
        );

        Ok(())
    }

    /// Copy texture to texture
    pub fn copy_to_texture(
        &self,
        device: &Device,
        queue: &Queue,
        destination: &Self,
        source_origin: wgpu::Origin3d,
        destination_origin: wgpu::Origin3d,
        copy_size: wgpu::Extent3d,
    ) -> Result<()> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Texture Copy Encoder"),
        });

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: source_origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &destination.texture,
                mip_level: 0,
                origin: destination_origin,
                aspect: wgpu::TextureAspect::All,
            },
            copy_size,
        );

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Generate mipmaps
    pub fn generate_mipmaps(&self, device: &Device, queue: &Queue) -> Result<()> {
        if self.mip_levels <= 1 {
            return Ok(());
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Mipmap Generation Encoder"),
        });

        for mip_level in 1..self.mip_levels {
            let previous_mip_size = self.mip_size(mip_level - 1);

            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: mip_level - 1,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                previous_mip_size,
            );
        }

        queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Get size for specific mip level
    fn mip_size(&self, mip_level: u32) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: (self.size.width >> mip_level).max(1),
            height: (self.size.height >> mip_level).max(1),
            depth_or_array_layers: self.size.depth_or_array_layers,
        }
    }

    /// Get bytes per row for specific mip level
    fn bytes_per_row_for_mip_level(&self, mip_level: u32) -> u32 {
        let mip_width = (self.size.width >> mip_level).max(1);
        let bytes_per_pixel = Self::get_bytes_per_pixel(self.format);
        mip_width * bytes_per_pixel
    }

    /// Get bytes per pixel for a texture format
    fn get_bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
        match format {
            wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Rgba8UnormSrgb
            | wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Bgra8UnormSrgb => 4,
            wgpu::TextureFormat::R8Unorm => 1,
            wgpu::TextureFormat::Rg8Unorm => 2,
            // Add more formats as needed
            _ => 4, // Default to 4 bytes per pixel
        }
    }

    /// Add engine reference
    pub fn add_engine_ref(&self) {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Release engine reference
    pub fn release_engine_ref(&self) {
        let old_count = self
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        if old_count == 1 {
            // Texture will be dropped when this Arc goes out of scope
        }
    }

    /// Get reference count
    pub fn engine_ref_count(&self) -> u32 {
        self.ref_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Basic texture manager for handling multiple textures
pub struct BasicTextureManager {
    /// Collection of managed textures
    textures: std::collections::HashMap<String, Arc<WgpuTexture>>,
    /// Device reference
    device: Option<Arc<wgpu::Device>>,
    /// Queue reference
    queue: Option<Arc<wgpu::Queue>>,
}

impl BasicTextureManager {
    /// Create new texture manager
    pub fn new() -> Self {
        Self {
            textures: std::collections::HashMap::new(),
            device: None,
            queue: None,
        }
    }

    /// Set device and queue
    pub fn set_device_queue(&mut self, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) {
        self.device = Some(device);
        self.queue = Some(queue);
    }

    /// Load texture from file
    pub fn load_texture(&mut self, path: &str, name: &str) -> Result<Arc<WgpuTexture>> {
        let device = self.device.as_ref().ok_or(Error::InitializationFailed)?;
        let queue = self.queue.as_ref().ok_or(Error::InitializationFailed)?;

        let img = image::open(path)
            .map_err(|e| Error::Generic(format!("Failed to load image {}: {}", path, e)))?;
        let rgba_img = img.to_rgba8();

        let texture = WgpuTexture::from_image_rgba8(
            device,
            queue,
            &rgba_img,
            1, // mip levels
            Some(name),
        )?;

        let texture_arc = Arc::new(texture);
        self.textures.insert(name.to_string(), texture_arc.clone());

        Ok(texture_arc)
    }

    /// Create render target
    pub fn create_render_target(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Result<Arc<WgpuTexture>> {
        let device = self.device.as_ref().ok_or(Error::InitializationFailed)?;

        let texture = WgpuTexture::render_target(device, width, height, format, Some(name))?;
        let texture_arc = Arc::new(texture);
        self.textures.insert(name.to_string(), texture_arc.clone());

        Ok(texture_arc)
    }

    /// Create depth texture
    pub fn create_depth_texture(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Result<Arc<WgpuTexture>> {
        let device = self.device.as_ref().ok_or(Error::InitializationFailed)?;

        let texture = WgpuTexture::depth_texture(device, width, height, format, Some(name))?;
        let texture_arc = Arc::new(texture);
        self.textures.insert(name.to_string(), texture_arc.clone());

        Ok(texture_arc)
    }

    /// Get texture by name
    pub fn get_texture(&self, name: &str) -> Option<&Arc<WgpuTexture>> {
        self.textures.get(name)
    }

    /// Remove texture
    pub fn remove_texture(&mut self, name: &str) -> bool {
        self.textures.remove(name).is_some()
    }

    /// Clear all textures
    pub fn clear(&mut self) {
        self.textures.clear();
    }

    /// Get texture count
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Cleanup resources
    pub fn cleanup(&mut self) {
        self.clear();
        self.device = None;
        self.queue = None;
    }
}

impl Default for BasicTextureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Texture utilities
pub struct TextureUtils;

impl TextureUtils {
    /// Convert WW3DFormat to WGPU format
    pub fn ww3d_format_to_wgpu(format: WW3DFormat) -> Option<wgpu::TextureFormat> {
        format.to_wgpu_format()
    }

    /// Convert WGPU format to WW3DFormat
    pub fn wgpu_format_to_ww3d(wgpu_format: wgpu::TextureFormat) -> Option<WW3DFormat> {
        WW3DFormat::from_wgpu_format(wgpu_format)
    }

    /// Get bytes per pixel for format
    pub fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
        // Direct calculation for WGPU formats
        match format {
            wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Rgba8UnormSrgb
            | wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Bgra8UnormSrgb => 4,
            wgpu::TextureFormat::R8Unorm => 1,
            wgpu::TextureFormat::Rg8Unorm => 2,
            // Add more formats as needed
            _ => 4, // Default to 4 bytes per pixel
        }
    }

    /// Check if format is compressed
    pub fn is_compressed(format: wgpu::TextureFormat) -> bool {
        matches!(
            format,
            wgpu::TextureFormat::Bc1RgbaUnorm
                | wgpu::TextureFormat::Bc2RgbaUnorm
                | wgpu::TextureFormat::Bc3RgbaUnorm
                | wgpu::TextureFormat::Bc4RUnorm
                | wgpu::TextureFormat::Bc5RgUnorm
                | wgpu::TextureFormat::Bc6hRgbUfloat
                | wgpu::TextureFormat::Bc7RgbaUnorm
        )
    }
}
