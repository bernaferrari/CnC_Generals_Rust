//! WGPU Texture Manager
//!
//! This module provides comprehensive texture management for WGPU,
//! equivalent to the DirectX8 texture manager functionality.

use std::collections::HashMap;
use std::sync::Arc;
use wgpu::TextureDescriptor;

use super::wgpu_texture::{TextureUtils, WgpuTexture};
use crate::core::error::{Error, Result};
use crate::rendering::texture_system::texture_base::TextureClass;
// Note: WW3DFormat enum maps texture formats (DXT1, DXT3, RGBA8, etc.) to WGPU formats.
// Currently using wgpu::TextureFormat directly. C++ equivalent: WW3DFormat enum (d3dtypes.h)

/// Texture tracking information
#[derive(Debug, Clone)]
pub struct TextureTracker {
    /// Texture width
    pub width: u32,
    /// Texture height
    pub height: u32,
    /// Mip level count
    pub mip_levels: u32,
    /// Texture format
    pub format: wgpu::TextureFormat,
    /// Whether this is a render target
    pub is_render_target: bool,
    /// Whether this is a depth texture
    pub is_depth_texture: bool,
    /// Associated texture class
    pub texture_class: Option<Arc<TextureClass>>,
}

/// Main WGPU texture manager
pub struct WgpuTextureManager {
    /// WGPU device reference
    device: Option<Arc<wgpu::Device>>,
    /// WGPU queue reference
    queue: Option<Arc<wgpu::Queue>>,
    /// Managed textures
    textures: HashMap<String, Arc<WgpuTexture>>,
    /// Texture trackers for recreation
    trackers: HashMap<String, TextureTracker>,
    /// Total memory used by textures
    total_memory_used: u64,
    /// Memory budget
    memory_budget: u64,
    /// Whether to enable mipmapping
    enable_mipmaps: bool,
    /// Default texture format
    default_format: wgpu::TextureFormat,
}

impl WgpuTextureManager {
    /// Create a new texture manager
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            textures: HashMap::new(),
            trackers: HashMap::new(),
            total_memory_used: 0,
            memory_budget: 512 * 1024 * 1024, // 512MB default budget
            enable_mipmaps: true,
            default_format: wgpu::TextureFormat::Bgra8Unorm,
        }
    }

    /// Initialize the texture manager
    pub fn initialize(&mut self, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) {
        self.device = Some(device);
        self.queue = Some(queue);
    }

    /// Load texture from file
    pub fn load_texture(&mut self, name: &str, path: &str) -> Result<Arc<WgpuTexture>> {
        let device = self.device.as_ref().ok_or(Error::InitializationFailed)?;

        // Load image
        let img = image::open(path)
            .map_err(|e| Error::Generic(format!("Failed to load image {}: {}", path, e)))?;
        let rgba_img = img.to_rgba8();

        let width = rgba_img.width();
        let height = rgba_img.height();
        let mip_levels = if self.enable_mipmaps {
            Self::calculate_mip_levels(width, height)
        } else {
            1
        };

        // Create WGPU texture
        let texture = WgpuTexture::from_image_rgba8(
            device,
            self.queue.as_ref().unwrap(),
            &rgba_img,
            mip_levels,
            Some(name),
        )?;

        let texture_arc = Arc::new(texture);

        // Calculate memory usage
        let memory_usage = Self::calculate_texture_memory_usage(width, height, 4, mip_levels);

        // Track the texture
        let tracker = TextureTracker {
            width,
            height,
            mip_levels,
            format: wgpu::TextureFormat::Bgra8Unorm,
            is_render_target: false,
            is_depth_texture: false,
            texture_class: None,
        };

        self.textures.insert(name.to_string(), texture_arc.clone());
        self.trackers.insert(name.to_string(), tracker);
        self.total_memory_used += memory_usage;

        Ok(texture_arc)
    }

    /// Create render target texture
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

        // Calculate memory usage
        let memory_usage = Self::calculate_texture_memory_usage(
            width,
            height,
            TextureUtils::bytes_per_pixel(format) as u32,
            1,
        );

        // Track the texture
        let tracker = TextureTracker {
            width,
            height,
            mip_levels: 1,
            format,
            is_render_target: true,
            is_depth_texture: false,
            texture_class: None,
        };

        self.textures.insert(name.to_string(), texture_arc.clone());
        self.trackers.insert(name.to_string(), tracker);
        self.total_memory_used += memory_usage;

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

        // Calculate memory usage
        let memory_usage = Self::calculate_texture_memory_usage(
            width,
            height,
            TextureUtils::bytes_per_pixel(format) as u32,
            1,
        );

        // Track the texture
        let tracker = TextureTracker {
            width,
            height,
            mip_levels: 1,
            format,
            is_render_target: false,
            is_depth_texture: true,
            texture_class: None,
        };

        self.textures.insert(name.to_string(), texture_arc.clone());
        self.trackers.insert(name.to_string(), tracker);
        self.total_memory_used += memory_usage;

        Ok(texture_arc)
    }

    /// Create texture from raw data
    pub fn create_texture_from_data(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        data: &[u8],
        format: wgpu::TextureFormat,
        mip_levels: u32,
    ) -> Result<Arc<WgpuTexture>> {
        let device = self.device.as_ref().ok_or(Error::InitializationFailed)?;
        let queue = self.queue.as_ref().ok_or(Error::InitializationFailed)?;

        let descriptor = TextureDescriptor {
            label: Some(name),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = WgpuTexture::new(device, &descriptor)?;

        // Upload data
        let bytes_per_pixel = TextureUtils::bytes_per_pixel(format) as usize;
        let expected_size = (width * height) as usize * bytes_per_pixel;

        if data.len() != expected_size {
            return Err(Error::InvalidTextureData(
                "Invalid texture data provided".to_string(),
            ));
        }

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: texture.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((width * bytes_per_pixel as u32) as u32),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let texture_arc = Arc::new(texture);

        // Calculate memory usage
        let memory_usage =
            Self::calculate_texture_memory_usage(width, height, bytes_per_pixel as u32, mip_levels);

        // Track the texture
        let tracker = TextureTracker {
            width,
            height,
            mip_levels,
            format,
            is_render_target: false,
            is_depth_texture: false,
            texture_class: None,
        };

        self.textures.insert(name.to_string(), texture_arc.clone());
        self.trackers.insert(name.to_string(), tracker);
        self.total_memory_used += memory_usage;

        Ok(texture_arc)
    }

    /// Get texture by name
    pub fn get_texture(&self, name: &str) -> Option<&Arc<WgpuTexture>> {
        self.textures.get(name)
    }

    /// Remove texture
    pub fn remove_texture(&mut self, name: &str) -> bool {
        if self.textures.remove(name).is_some() {
            if let Some(tracker) = self.trackers.remove(name) {
                let memory_usage = Self::calculate_texture_memory_usage(
                    tracker.width,
                    tracker.height,
                    TextureUtils::bytes_per_pixel(tracker.format) as u32,
                    tracker.mip_levels,
                );
                self.total_memory_used = self.total_memory_used.saturating_sub(memory_usage);
            }
            true
        } else {
            false
        }
    }

    /// Recreate all tracked textures (for device lost scenarios)
    pub fn recreate_textures(&mut self) -> Result<()> {
        let device = self.device.as_ref().ok_or(Error::InitializationFailed)?;

        for (name, tracker) in &self.trackers {
            if !self.textures.contains_key(name) {
                continue;
            }

            // Recreate the texture
            let descriptor = TextureDescriptor {
                label: Some(name),
                size: wgpu::Extent3d {
                    width: tracker.width,
                    height: tracker.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: tracker.mip_levels,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: tracker.format,
                usage: if tracker.is_render_target {
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
                } else if tracker.is_depth_texture {
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
                } else {
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
                },
                view_formats: &[],
            };

            let new_texture = WgpuTexture::new(device, &descriptor)?;
            // Note: In a real implementation, we'd need to copy the old texture data
            // For now, we just replace the texture
            self.textures.insert(name.clone(), Arc::new(new_texture));
        }

        Ok(())
    }

    /// Get total memory used by textures
    pub fn total_memory_used(&self) -> u64 {
        self.total_memory_used
    }

    /// Get memory budget
    pub fn memory_budget(&self) -> u64 {
        self.memory_budget
    }

    /// Set memory budget
    pub fn set_memory_budget(&mut self, budget: u64) {
        self.memory_budget = budget;
    }

    /// Check if we're over memory budget
    pub fn is_over_budget(&self) -> bool {
        self.total_memory_used > self.memory_budget
    }

    /// Get texture count
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Enable/disable mipmapping
    pub fn set_mipmapping_enabled(&mut self, enabled: bool) {
        self.enable_mipmaps = enabled;
    }

    /// Set default texture format
    pub fn set_default_format(&mut self, format: wgpu::TextureFormat) {
        self.default_format = format;
    }

    /// Clear all textures
    pub fn clear(&mut self) {
        self.textures.clear();
        self.trackers.clear();
        self.total_memory_used = 0;
    }

    /// Cleanup resources
    pub fn cleanup(&mut self) {
        self.clear();
        self.device = None;
        self.queue = None;
    }

    /// Calculate mip levels for a texture
    fn calculate_mip_levels(width: u32, height: u32) -> u32 {
        let max_dimension = width.max(height);
        (max_dimension as f32).log2().floor() as u32 + 1
    }

    /// Calculate memory usage for a texture
    fn calculate_texture_memory_usage(
        width: u32,
        height: u32,
        bytes_per_pixel: u32,
        mip_levels: u32,
    ) -> u64 {
        let mut total_size = 0u64;
        let mut current_width = width;
        let mut current_height = height;

        for _ in 0..mip_levels {
            total_size += (current_width * current_height) as u64 * bytes_per_pixel as u64;
            current_width = current_width.max(1) / 2;
            current_height = current_height.max(1) / 2;
        }

        total_size
    }
}

impl Default for WgpuTextureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Texture cache for frequently used textures
pub struct TextureCache {
    /// Cached textures
    cache: HashMap<String, Arc<WgpuTexture>>,
    /// Access timestamps for LRU eviction
    access_times: HashMap<String, std::time::Instant>,
    /// Maximum cache size
    max_size: usize,
    /// Current cache size
    current_size: usize,
}

impl TextureCache {
    /// Create a new texture cache
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            access_times: HashMap::new(),
            max_size,
            current_size: 0,
        }
    }

    /// Get texture from cache
    pub fn get(&mut self, name: &str) -> Option<&Arc<WgpuTexture>> {
        if let Some(texture) = self.cache.get(name) {
            self.access_times
                .insert(name.to_string(), std::time::Instant::now());
            Some(texture)
        } else {
            None
        }
    }

    /// Insert texture into cache
    pub fn insert(&mut self, name: String, texture: Arc<WgpuTexture>) -> bool {
        if self.current_size >= self.max_size {
            self.evict_lru();
        }

        if !self.cache.contains_key(&name) {
            self.current_size += 1;
        }

        self.cache.insert(name.clone(), texture);
        self.access_times.insert(name, std::time::Instant::now());
        true
    }

    /// Remove texture from cache
    pub fn remove(&mut self, name: &str) -> bool {
        if self.cache.remove(name).is_some() {
            self.access_times.remove(name);
            self.current_size = self.current_size.saturating_sub(1);
            true
        } else {
            false
        }
    }

    /// Clear cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_times.clear();
        self.current_size = 0;
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        self.current_size
    }

    /// Evict least recently used texture
    fn evict_lru(&mut self) {
        if let Some((oldest_name, _)) = self.access_times.iter().min_by_key(|(_, time)| *time) {
            let name = oldest_name.clone();
            self.cache.remove(&name);
            self.access_times.remove(&name);
            self.current_size = self.current_size.saturating_sub(1);
        }
    }
}

impl Default for TextureCache {
    fn default() -> Self {
        Self::new(100) // Default cache size of 100 textures
    }
}
