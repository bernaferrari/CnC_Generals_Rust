//! # W3D Texture Management System
//!
//! Advanced texture management with compression, streaming, and GPU optimization.
//! Supports all W3D texture formats with modern compression techniques.

use super::{Result, Texture, TextureFormat, TextureType, W3DError};
use image::{DynamicImage, ImageError, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use wgpu::{
    AddressMode, CompareFunction, Device, Extent3d, FilterMode, Origin3d, Queue, Sampler,
    SamplerDescriptor, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture as WgpuTexture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureUsages, TextureView,
};

/// Advanced texture manager with streaming and compression
pub struct W3DTextureManager {
    /// WGPU device reference
    device: Arc<Device>,
    /// WGPU command queue
    queue: Arc<Queue>,

    /// Loaded textures cache
    texture_cache: Arc<RwLock<HashMap<String, W3DTextureGpu>>>,

    /// Texture streaming queue
    stream_queue: Arc<RwLock<VecDeque<StreamRequest>>>,

    /// Memory management
    memory_budget: u64,
    memory_used: Arc<RwLock<u64>>,

    /// Compression settings
    compression_enabled: bool,
    compression_quality: CompressionQuality,

    /// Texture streaming settings
    streaming_enabled: bool,
    max_texture_size: u32,
    mip_levels_auto: bool,

    /// Performance statistics
    stats: Arc<RwLock<TextureManagerStats>>,
}

/// GPU texture resource with complete wgpu integration
#[derive(Debug)]
pub struct W3DTextureGpu {
    /// Original texture metadata
    pub texture_info: Texture,
    /// GPU texture resource
    pub wgpu_texture: WgpuTexture,
    /// Texture view for shaders
    pub view: TextureView,
    /// Sampler for filtering
    pub sampler: Sampler,
    /// Memory usage in bytes
    pub memory_size: u64,
    /// Last access time for LRU eviction
    pub last_access: std::time::Instant,
    /// Reference count for usage tracking
    pub reference_count: Arc<RwLock<u32>>,
}

/// Texture streaming request
#[derive(Debug, Clone)]
pub struct StreamRequest {
    /// Texture ID
    pub texture_id: String,
    /// File path or data source
    pub source: TextureSource,
    /// Priority (0 = highest)
    pub priority: u32,
    /// Target format
    pub target_format: TextureFormat,
    /// Generate mipmaps
    pub generate_mipmaps: bool,
    /// Compression settings
    pub compression: Option<CompressionSettings>,
}

/// Texture data source
#[derive(Debug, Clone)]
pub enum TextureSource {
    /// File path
    FilePath(String),
    /// Raw texture data
    RawData(Vec<u8>),
    /// Procedural generation
    Procedural(ProceduralTexture),
}

/// Procedural texture generation
#[derive(Debug, Clone)]
pub struct ProceduralTexture {
    /// Texture type (noise, checkerboard, etc.)
    pub texture_type: ProceduralType,
    /// Generation parameters
    pub parameters: HashMap<String, f32>,
    /// Output size
    pub size: (u32, u32),
}

/// Procedural texture types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProceduralType {
    /// Solid color
    SolidColor,
    /// Checkerboard pattern
    Checkerboard,
    /// Perlin noise
    PerlinNoise,
    /// White noise
    WhiteNoise,
    /// Gradient
    Gradient,
    /// Normal map from height
    NormalFromHeight,
}

/// Compression quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionQuality {
    /// Maximum compression, lower quality
    Low,
    /// Balanced compression and quality
    Medium,
    /// Minimum compression, maximum quality
    High,
    /// No compression (original quality)
    Lossless,
}

/// Compression settings for specific textures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionSettings {
    /// Compression quality
    pub quality: CompressionQuality,
    /// Force specific format
    pub force_format: Option<TextureFormat>,
    /// Allow format conversion
    pub allow_format_conversion: bool,
}

/// Texture manager statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TextureManagerStats {
    /// Total textures loaded
    pub textures_loaded: u32,
    /// Textures currently in memory
    pub textures_in_memory: u32,
    /// Total memory used (bytes)
    pub memory_used: u64,
    /// Memory budget (bytes)
    pub memory_budget: u64,
    /// Cache hit rate
    pub cache_hit_rate: f32,
    /// Texture uploads per frame
    pub uploads_per_frame: u32,
    /// Compression ratio achieved
    pub compression_ratio: f32,
    /// Streaming queue size
    pub stream_queue_size: u32,
    /// Average load time (ms)
    pub average_load_time_ms: f32,
}

impl W3DTextureManager {
    /// Create a new texture manager
    pub async fn new(device: &Device, queue: &Queue, memory_budget: u64) -> Result<Self> {
        tracing::info!(
            "Creating W3D texture manager with {}MB budget",
            memory_budget / 1024 / 1024
        );

        Ok(Self {
            device: Arc::new(device.clone()),
            queue: Arc::new(queue.clone()),
            texture_cache: Arc::new(RwLock::new(HashMap::new())),
            stream_queue: Arc::new(RwLock::new(VecDeque::new())),
            memory_budget,
            memory_used: Arc::new(RwLock::new(0)),
            compression_enabled: true,
            compression_quality: CompressionQuality::High,
            streaming_enabled: true,
            max_texture_size: 8192,
            mip_levels_auto: true,
            stats: Arc::new(RwLock::new(TextureManagerStats {
                memory_budget,
                ..Default::default()
            })),
        })
    }

    /// Load texture from file with streaming support
    pub async fn load_texture(
        &self,
        texture_id: &str,
        file_path: &str,
    ) -> Result<Arc<W3DTextureGpu>> {
        let start_time = std::time::Instant::now();

        // Check cache first
        {
            let cache = self.texture_cache.read().await;
            if let Some(texture) = cache.get(texture_id) {
                // Update access time
                let texture_clone = texture.clone();
                drop(cache);

                let mut stats = self.stats.write().await;
                stats.cache_hit_rate = (stats.cache_hit_rate * 0.9) + 0.1; // Moving average

                return Ok(Arc::new(texture_clone));
            }
        }

        // Load image from file
        let image = self.load_image_from_file(file_path).await?;
        let texture_gpu = self.create_texture_from_image(texture_id, image).await?;

        // Add to cache
        {
            let mut cache = self.texture_cache.write().await;
            cache.insert(texture_id.to_string(), texture_gpu.clone());
        }

        // Update memory usage
        {
            let mut memory_used = self.memory_used.write().await;
            *memory_used += texture_gpu.memory_size;
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.textures_loaded += 1;
            stats.textures_in_memory += 1;
            stats.memory_used += texture_gpu.memory_size;

            let load_time = start_time.elapsed().as_secs_f32() * 1000.0;
            stats.average_load_time_ms = (stats.average_load_time_ms * 0.9) + (load_time * 0.1);

            // Cache miss
            stats.cache_hit_rate = stats.cache_hit_rate * 0.9;
        }

        // Check memory budget and evict if necessary
        self.enforce_memory_budget().await?;

        tracing::debug!(
            "Loaded texture '{}' from '{}' in {:.2}ms",
            texture_id,
            file_path,
            start_time.elapsed().as_secs_f32() * 1000.0
        );

        Ok(Arc::new(texture_gpu))
    }

    /// Load image from file with format detection
    async fn load_image_from_file(&self, file_path: &str) -> Result<DynamicImage> {
        let path = Path::new(file_path);

        // Check if file exists
        if !path.exists() {
            return Err(W3DError::ResourceLoadingFailed(format!(
                "Texture file not found: {}",
                file_path
            )));
        }

        // Load and decode image
        let image = image::open(path).map_err(|e| {
            W3DError::ResourceLoadingFailed(format!("Failed to load image '{}': {}", file_path, e))
        })?;

        // Convert to RGBA if needed
        let rgba_image = match image {
            DynamicImage::ImageRgba8(_) => image,
            _ => DynamicImage::ImageRgba8(image.to_rgba8()),
        };

        Ok(rgba_image)
    }

    /// Create GPU texture from image
    async fn create_texture_from_image(
        &self,
        texture_id: &str,
        image: DynamicImage,
    ) -> Result<W3DTextureGpu> {
        let rgba_image = image.to_rgba8();
        let (width, height) = rgba_image.dimensions();
        let raw_data = rgba_image.into_raw();

        // Calculate mip levels
        let mip_levels = if self.mip_levels_auto {
            (width.min(height) as f32).log2().floor() as u32 + 1
        } else {
            1
        };

        // Create GPU texture
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let wgpu_texture = self.device.create_texture(&TextureDescriptor {
            label: Some(&format!("W3D Texture: {}", texture_id)),
            size: texture_size,
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload texture data
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &raw_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        // Generate mipmaps if requested
        if mip_levels > 1 {
            self.generate_mipmaps(&wgpu_texture, &raw_data, width, height, mip_levels)
                .await?;
        }

        // Create texture view
        let view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = self.device.create_sampler(&SamplerDescriptor {
            label: Some(&format!("W3D Sampler: {}", texture_id)),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            compare: None,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            border_color: None,
            anisotropy_clamp: 16,
        });

        // Calculate memory usage
        let memory_size = (raw_data.len() as u64) * mip_levels as u64;

        // Create texture info
        let texture_info = Texture {
            id: texture_id.to_string(),
            name: texture_id.to_string(),
            width,
            height,
            depth: 1,
            mip_levels,
            format: TextureFormat::Rgba8,
            texture_type: TextureType::Texture2D,
            data: raw_data,
        };

        Ok(W3DTextureGpu {
            texture_info,
            wgpu_texture,
            view,
            sampler,
            memory_size,
            last_access: std::time::Instant::now(),
            reference_count: Arc::new(RwLock::new(1)),
        })
    }

    /// Generate mipmaps for texture
    async fn generate_mipmaps(
        &self,
        texture: &WgpuTexture,
        base_level_rgba: &[u8],
        base_width: u32,
        base_height: u32,
        mip_levels: u32,
    ) -> Result<()> {
        let mut previous_data = base_level_rgba.to_vec();
        let mut previous_width = base_width;
        let mut previous_height = base_height;

        for mip_level in 1..mip_levels {
            let mip_width = (previous_width / 2).max(1);
            let mip_height = (previous_height / 2).max(1);
            let mip_data = Self::downsample_rgba8(&previous_data, previous_width, previous_height);
            let mip_size = Extent3d {
                width: mip_width,
                height: mip_height,
                depth_or_array_layers: 1,
            };

            self.queue.write_texture(
                TexelCopyTextureInfo {
                    texture,
                    mip_level,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &mip_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * mip_width),
                    rows_per_image: Some(mip_height),
                },
                mip_size,
            );

            tracing::trace!(
                "Generated mipmap level {} ({}x{})",
                mip_level,
                mip_size.width,
                mip_size.height
            );

            previous_data = mip_data;
            previous_width = mip_width;
            previous_height = mip_height;
        }

        Ok(())
    }

    fn downsample_rgba8(src: &[u8], src_width: u32, src_height: u32) -> Vec<u8> {
        let dst_width = (src_width / 2).max(1);
        let dst_height = (src_height / 2).max(1);
        let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];

        for y in 0..dst_height {
            for x in 0..dst_width {
                let src_x = x * 2;
                let src_y = y * 2;
                let mut accum = [0u32; 4];
                let mut samples = 0u32;

                for oy in 0..2 {
                    for ox in 0..2 {
                        let sample_x = (src_x + ox).min(src_width.saturating_sub(1));
                        let sample_y = (src_y + oy).min(src_height.saturating_sub(1));
                        let idx = ((sample_y * src_width + sample_x) * 4) as usize;
                        accum[0] += src[idx] as u32;
                        accum[1] += src[idx + 1] as u32;
                        accum[2] += src[idx + 2] as u32;
                        accum[3] += src[idx + 3] as u32;
                        samples += 1;
                    }
                }

                let out_idx = ((y * dst_width + x) * 4) as usize;
                dst[out_idx] = (accum[0] / samples) as u8;
                dst[out_idx + 1] = (accum[1] / samples) as u8;
                dst[out_idx + 2] = (accum[2] / samples) as u8;
                dst[out_idx + 3] = (accum[3] / samples) as u8;
            }
        }

        dst
    }

    /// Create procedural texture
    pub async fn create_procedural_texture(
        &self,
        texture_id: &str,
        procedural: &ProceduralTexture,
    ) -> Result<Arc<W3DTextureGpu>> {
        let image = self.generate_procedural_image(procedural).await?;
        let texture_gpu = self.create_texture_from_image(texture_id, image).await?;

        // Add to cache
        {
            let mut cache = self.texture_cache.write().await;
            cache.insert(texture_id.to_string(), texture_gpu.clone());
        }

        tracing::debug!(
            "Generated procedural texture '{}' ({:?})",
            texture_id,
            procedural.texture_type
        );

        Ok(Arc::new(texture_gpu))
    }

    /// Generate procedural image
    async fn generate_procedural_image(
        &self,
        procedural: &ProceduralTexture,
    ) -> Result<DynamicImage> {
        let (width, height) = procedural.size;
        let mut image = RgbaImage::new(width, height);

        match procedural.texture_type {
            ProceduralType::SolidColor => {
                let r = (procedural.parameters.get("r").unwrap_or(&1.0) * 255.0) as u8;
                let g = (procedural.parameters.get("g").unwrap_or(&1.0) * 255.0) as u8;
                let b = (procedural.parameters.get("b").unwrap_or(&1.0) * 255.0) as u8;
                let a = (procedural.parameters.get("a").unwrap_or(&1.0) * 255.0) as u8;

                for pixel in image.pixels_mut() {
                    *pixel = image::Rgba([r, g, b, a]);
                }
            }
            ProceduralType::Checkerboard => {
                let size = (*procedural.parameters.get("size").unwrap_or(&8.0)) as u32;

                for (x, y, pixel) in image.enumerate_pixels_mut() {
                    let checker = ((x / size) + (y / size)) % 2 == 0;
                    let color = if checker { 255 } else { 0 };
                    *pixel = image::Rgba([color, color, color, 255]);
                }
            }
            ProceduralType::WhiteNoise => {
                // Use fastrand for 2025 performance optimization
                let mut rng = fastrand::Rng::new();

                for pixel in image.pixels_mut() {
                    let value = rng.u8(..);
                    *pixel = image::Rgba([value, value, value, 255]);
                }
            }
            _ => {
                // Default to solid white for unimplemented types
                for pixel in image.pixels_mut() {
                    *pixel = image::Rgba([255, 255, 255, 255]);
                }
            }
        }

        Ok(DynamicImage::ImageRgba8(image))
    }

    /// Enforce memory budget by evicting least recently used textures
    async fn enforce_memory_budget(&self) -> Result<()> {
        let memory_used = *self.memory_used.read().await;

        if memory_used <= self.memory_budget {
            return Ok(());
        }

        let mut textures: Vec<_> = {
            let cache = self.texture_cache.read().await;
            cache
                .iter()
                .map(|(texture_id, texture)| {
                    (
                        texture_id.clone(),
                        texture.last_access,
                        texture.memory_size,
                        texture.reference_count.clone(),
                    )
                })
                .collect()
        };

        // Sort by last access time (oldest first)
        textures.sort_by(|a, b| a.1.cmp(&b.1));

        let mut memory_to_free = memory_used.saturating_sub(self.memory_budget);
        let mut memory_freed = 0_u64;
        let mut evicted_count = 0;
        let mut evicted_ids = Vec::new();

        for (texture_id, _last_access, texture_size, reference_count) in textures {
            let ref_count = *reference_count.read().await;

            // Only evict if not currently referenced
            if ref_count == 0 {
                memory_to_free = memory_to_free.saturating_sub(texture_size);
                memory_freed = memory_freed.saturating_add(texture_size);
                evicted_ids.push(texture_id);
                evicted_count += 1;

                if memory_to_free == 0 {
                    break;
                }
            }
        }

        if !evicted_ids.is_empty() {
            let mut cache = self.texture_cache.write().await;
            for texture_id in &evicted_ids {
                cache.remove(texture_id);
            }
        }

        // Update memory usage
        {
            let mut memory_used = self.memory_used.write().await;
            *memory_used = memory_used.saturating_sub(memory_freed);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.textures_in_memory = self.texture_cache.read().await.len() as u32;
            stats.memory_used = *self.memory_used.read().await;
        }

        if evicted_count > 0 {
            tracing::debug!(
                "Evicted {} textures to enforce memory budget",
                evicted_count
            );
        }

        Ok(())
    }

    /// Get texture manager statistics
    pub async fn get_stats(&self) -> TextureManagerStats {
        self.stats.read().await.clone()
    }

    /// Clear texture cache
    pub async fn clear_cache(&self) -> Result<()> {
        let mut cache = self.texture_cache.write().await;
        cache.clear();

        let mut memory_used = self.memory_used.write().await;
        *memory_used = 0;

        let mut stats = self.stats.write().await;
        stats.textures_in_memory = 0;
        stats.memory_used = 0;

        tracing::info!("Cleared texture cache");
        Ok(())
    }

    /// Set compression settings
    pub fn set_compression(&mut self, enabled: bool, quality: CompressionQuality) {
        self.compression_enabled = enabled;
        self.compression_quality = quality;
        tracing::debug!(
            "Set texture compression: enabled={}, quality={:?}",
            enabled,
            quality
        );
    }

    /// Set streaming settings
    pub fn set_streaming(&mut self, enabled: bool, max_texture_size: u32) {
        self.streaming_enabled = enabled;
        self.max_texture_size = max_texture_size;
        tracing::debug!(
            "Set texture streaming: enabled={}, max_size={}x{}",
            enabled,
            max_texture_size,
            max_texture_size
        );
    }
}

impl Clone for W3DTextureGpu {
    fn clone(&self) -> Self {
        // Note: This is a simplified clone that shares the GPU resources
        // In a real implementation, you'd need proper reference counting
        Self {
            texture_info: self.texture_info.clone(),
            wgpu_texture: self.wgpu_texture.clone(),
            view: self.view.clone(),
            sampler: self.sampler.clone(),
            memory_size: self.memory_size,
            last_access: std::time::Instant::now(),
            reference_count: self.reference_count.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::W3DTextureManager;

    #[test]
    fn downsample_rgba8_averages_2x2_block() {
        let src_width = 2;
        let src_height = 2;
        let src = vec![
            0, 0, 0, 255, // top-left
            100, 0, 0, 255, // top-right
            0, 100, 0, 255, // bottom-left
            0, 0, 100, 255, // bottom-right
        ];

        let mip = W3DTextureManager::downsample_rgba8(&src, src_width, src_height);
        assert_eq!(mip, vec![25, 25, 25, 255]);
    }

    #[test]
    fn downsample_rgba8_handles_odd_dimensions_with_edge_clamp() {
        let src_width = 3;
        let src_height = 1;
        let src = vec![
            10, 20, 30, 40, // x=0
            50, 60, 70, 80, // x=1
            90, 100, 110, 120, // x=2
        ];

        let mip = W3DTextureManager::downsample_rgba8(&src, src_width, src_height);
        // 1x1 output, samples are x=0,x=1 twice each due Y clamp.
        assert_eq!(mip, vec![30, 40, 50, 60]);
    }
}
