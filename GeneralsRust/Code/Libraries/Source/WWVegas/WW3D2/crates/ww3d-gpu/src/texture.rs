//! GPU Texture Resource Management
//!
//! This module provides GPU texture creation, management, and data transfer
//! functionality for 2D textures, cube maps, volume textures, and texture arrays.

use crate::*;
use std::sync::Arc;

/// GPU texture abstraction
#[derive(Debug)]
pub struct GpuTexture {
    /// Underlying WGPU texture
    texture: wgpu::Texture,
    /// Texture view for binding
    view: wgpu::TextureView,
    /// Texture sampler
    sampler: wgpu::Sampler,
    /// Texture size
    size: wgpu::Extent3d,
    /// Texture format
    format: wgpu::TextureFormat,
    /// Texture usage
    usage: wgpu::TextureUsages,
    /// Mipmap level count
    mip_levels: u32,
    /// Texture label
    label: Option<String>,
    /// Last update timestamp
    last_update: std::time::Instant,
}

impl GpuTexture {
    /// Create a new GPU texture
    pub fn new(
        device: &crate::device::GpuDevice,
        desc: &wgpu::TextureDescriptor,
    ) -> Result<Self, GpuError> {
        let wrapped_texture = device.create_texture(desc)?;

        let view = wrapped_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: desc.label,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            texture: wrapped_texture.texture,
            view,
            sampler,
            size: desc.size,
            format: desc.format,
            usage: desc.usage,
            mip_levels: desc.mip_level_count,
            label: desc.label.map(|s| s.to_string()),
            last_update: std::time::Instant::now(),
        })
    }

    /// Create a 2D texture
    pub fn create_2d(
        device: &crate::device::GpuDevice,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        };

        Self::new(device, &desc)
    }

    /// Create a cube texture
    pub fn create_cube(
        device: &crate::device::GpuDevice,
        size: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        };

        let mut texture = Self::new(device, &desc)?;

        // Create cube map view
        texture.view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        Ok(texture)
    }

    /// Create a texture array
    pub fn create_array(
        device: &crate::device::GpuDevice,
        width: u32,
        height: u32,
        layers: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        };

        let mut texture = Self::new(device, &desc)?;

        // Create array view
        texture.view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        Ok(texture)
    }

    /// Create a volume texture (3D)
    pub fn create_volume(
        device: &crate::device::GpuDevice,
        width: u32,
        height: u32,
        depth: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format,
            usage,
            view_formats: &[],
        };

        let mut texture = Self::new(device, &desc)?;

        // Create 3D view
        texture.view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D3),
            ..Default::default()
        });

        Ok(texture)
    }

    /// Write texture data
    pub fn write_data(
        &mut self,
        device: &crate::device::GpuDevice,
        data: &[u8],
        origin: wgpu::Origin3d,
        size: wgpu::Extent3d,
        mip_level: u32,
    ) {
        device.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.bytes_per_row(size.width)),
                rows_per_image: Some(size.height),
            },
            size,
        );

        self.last_update = std::time::Instant::now();
    }

    /// Write texture data for a specific layer (for arrays/cube maps)
    pub fn write_layer_data(
        &mut self,
        device: &crate::device::GpuDevice,
        data: &[u8],
        layer: u32,
        mip_level: u32,
    ) {
        let size = wgpu::Extent3d {
            width: self.size.width >> mip_level,
            height: self.size.height >> mip_level,
            depth_or_array_layers: 1,
        };

        self.write_data(
            device,
            data,
            wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            size,
            mip_level,
        );
    }

    /// Generate mipmaps
    pub fn generate_mipmaps(&self, device: &crate::device::GpuDevice) {
        if self.mip_levels <= 1 {
            return;
        }

        let mut encoder = device.create_command_encoder(Some("Generate Mipmaps"));

        for mip_level in 1..self.mip_levels {
            let _src_view = self.texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: mip_level - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });

            let _dst_view = self.texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: mip_level,
                mip_level_count: Some(1),
                ..Default::default()
            });

            // Blit from previous mip level to current
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
                wgpu::Extent3d {
                    width: (self.size.width >> mip_level).max(1),
                    height: (self.size.height >> mip_level).max(1),
                    depth_or_array_layers: self.size.depth_or_array_layers,
                },
            );
        }

        device.submit(vec![encoder.finish()]);
    }

    /// Create a texture view with custom descriptor
    pub fn create_view(&self, desc: &wgpu::TextureViewDescriptor) -> wgpu::TextureView {
        self.texture.create_view(desc)
    }

    /// Get the texture view
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the texture sampler
    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    /// Get the underlying WGPU texture
    pub fn wgpu_texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Get texture size
    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }

    /// Get texture width
    pub fn width(&self) -> u32 {
        self.size.width
    }

    /// Get texture height
    pub fn height(&self) -> u32 {
        self.size.height
    }

    /// Get texture format
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get texture usage
    pub fn usage(&self) -> wgpu::TextureUsages {
        self.usage
    }

    /// Get mipmap level count
    pub fn mip_levels(&self) -> u32 {
        self.mip_levels
    }

    /// Get texture label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Check if texture is render target
    pub fn is_render_target(&self) -> bool {
        self.usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
    }

    /// Check if texture is storage texture
    pub fn is_storage_texture(&self) -> bool {
        self.usage.contains(wgpu::TextureUsages::STORAGE_BINDING)
    }

    /// Check if texture is sampled texture
    pub fn is_sampled_texture(&self) -> bool {
        self.usage.contains(wgpu::TextureUsages::TEXTURE_BINDING)
    }

    /// Get time since last update
    pub fn time_since_update(&self) -> std::time::Duration {
        self.last_update.elapsed()
    }

    /// Calculate bytes per row for texture data layout
    fn bytes_per_row(&self, width: u32) -> u32 {
        let bytes_per_pixel = match self.format {
            wgpu::TextureFormat::R8Unorm => 1,
            wgpu::TextureFormat::Rg8Unorm => 2,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4,
            wgpu::TextureFormat::Rgba16Float => 8,
            wgpu::TextureFormat::Rgba32Float => 16,
            // Add more formats as needed
            _ => 4, // Default to 4 bytes per pixel
        };

        width * bytes_per_pixel
    }
}

/// Texture manager for handling multiple textures
#[derive(Debug)]
pub struct TextureManager {
    /// GPU device reference
    device: Arc<crate::device::GpuDevice>,
    /// Managed textures
    textures: Vec<Arc<GpuTexture>>,
    /// Texture cache for reuse
    cache: std::collections::HashMap<String, Arc<GpuTexture>>,
    /// Texture statistics
    stats: TextureStats,
}

impl TextureManager {
    /// Create a new texture manager
    pub fn new(device: Arc<crate::device::GpuDevice>) -> Self {
        Self {
            device,
            textures: Vec::new(),
            cache: std::collections::HashMap::new(),
            stats: TextureStats::default(),
        }
    }

    /// Create a 2D texture
    pub fn create_texture_2d(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<Arc<GpuTexture>, GpuError> {
        let texture = GpuTexture::create_2d(&self.device, width, height, format, usage, label)?;
        let texture_arc = Arc::new(texture);
        self.textures.push(texture_arc.clone());
        self.update_stats();
        Ok(texture_arc)
    }

    /// Create a cube texture
    pub fn create_cube_texture(
        &mut self,
        size: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<Arc<GpuTexture>, GpuError> {
        let texture = GpuTexture::create_cube(&self.device, size, format, usage, label)?;
        let texture_arc = Arc::new(texture);
        self.textures.push(texture_arc.clone());
        self.update_stats();
        Ok(texture_arc)
    }

    /// Create a texture array
    pub fn create_texture_array(
        &mut self,
        width: u32,
        height: u32,
        layers: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        label: Option<&str>,
    ) -> Result<Arc<GpuTexture>, GpuError> {
        let texture =
            GpuTexture::create_array(&self.device, width, height, layers, format, usage, label)?;
        let texture_arc = Arc::new(texture);
        self.textures.push(texture_arc.clone());
        self.update_stats();
        Ok(texture_arc)
    }

    /// Load texture from image data
    pub fn load_texture_from_data(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: Option<&str>,
    ) -> Result<Arc<GpuTexture>, GpuError> {
        let mut texture = self.create_texture_2d(
            width,
            height,
            format,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label,
        )?;

        // Upload texture data
        Arc::get_mut(&mut texture).unwrap().write_data(
            &self.device,
            data,
            wgpu::Origin3d::ZERO,
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            0,
        );

        Ok(texture)
    }

    /// Get texture from cache or create new one
    pub fn get_or_create_texture(
        &mut self,
        key: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Result<Arc<GpuTexture>, GpuError> {
        if let Some(texture) = self.cache.get(key) {
            return Ok(texture.clone());
        }

        let texture = self.create_texture_2d(width, height, format, usage, Some(key))?;
        self.cache.insert(key.to_string(), texture.clone());
        Ok(texture)
    }

    /// Get texture statistics
    pub fn stats(&self) -> &TextureStats {
        &self.stats
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.texture_count = self.textures.len();
        self.stats.total_memory = self
            .textures
            .iter()
            .map(|t| {
                let bytes_per_pixel = match t.format() {
                    wgpu::TextureFormat::R8Unorm => 1,
                    wgpu::TextureFormat::Rg8Unorm => 2,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => 4,
                    wgpu::TextureFormat::Rgba16Float => 8,
                    wgpu::TextureFormat::Rgba32Float => 16,
                    _ => 4,
                };
                (t.width() * t.height() * t.size().depth_or_array_layers) as u64
                    * bytes_per_pixel as u64
            })
            .sum();

        self.stats.render_target_count = self
            .textures
            .iter()
            .filter(|t| t.is_render_target())
            .count();
        self.stats.storage_texture_count = self
            .textures
            .iter()
            .filter(|t| t.is_storage_texture())
            .count();
    }

    /// Cleanup unused textures
    pub fn cleanup(&mut self) {
        // Remove textures that haven't been used recently
        let cutoff = std::time::Duration::from_secs(300); // 5 minutes
        self.textures
            .retain(|texture| texture.time_since_update() < cutoff);
        self.update_stats();
    }
}

/// Texture statistics
#[derive(Debug, Clone, Default)]
pub struct TextureStats {
    pub texture_count: usize,
    pub total_memory: u64,
    pub render_target_count: usize,
    pub storage_texture_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_stats() {
        let stats = TextureStats::default();
        assert_eq!(stats.texture_count, 0);
        assert_eq!(stats.total_memory, 0);
        assert_eq!(stats.render_target_count, 0);
    }
}
