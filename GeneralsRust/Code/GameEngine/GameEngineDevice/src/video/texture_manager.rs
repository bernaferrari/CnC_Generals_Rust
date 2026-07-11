//! # Texture Management System
//!
//! Efficient texture streaming, caching, and memory management for the video device.

use super::render_device::{RenderDevice, TextureDesc, TextureUsage};
use super::{ColorFormat, Result, VideoDeviceError};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "video")]
use wgpu::{
    AddressMode, Device, FilterMode, Queue, Sampler, SamplerDescriptor, Texture, TextureView,
};

use dashmap::DashMap;
use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba};
use rayon::prelude::*;

/// Texture streaming priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StreamingPriority {
    /// Critical - must be loaded immediately
    Critical = 0,
    /// High - should be loaded soon
    High = 1,
    /// Normal - standard priority
    Normal = 2,
    /// Low - can be deferred
    Low = 3,
    /// Background - load when resources available
    Background = 4,
}

/// Texture compression format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureCompression {
    /// No compression (raw format)
    None,
    /// BC1 (DXT1) - 1 bit alpha
    BC1,
    /// BC3 (DXT5) - full alpha
    BC3,
    /// BC7 - high quality RGB/RGBA
    BC7,
    /// ETC2 RGB
    ETC2_RGB,
    /// ETC2 RGBA
    ETC2_RGBA,
    /// ASTC 4x4
    ASTC_4x4,
    /// ASTC 6x6
    ASTC_6x6,
    /// ASTC 8x8
    ASTC_8x8,
}

/// Texture filter settings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureFilter {
    /// Minification filter
    #[serde(skip)]
    pub min_filter: FilterMode,
    /// Magnification filter
    #[serde(skip)]
    pub mag_filter: FilterMode,
    /// Mipmap filter
    #[serde(skip)]
    pub mipmap_filter: FilterMode,
    /// Anisotropic filtering level (1 = disabled, up to 16)
    pub anisotropy: u8,
}

impl Default for TextureFilter {
    fn default() -> Self {
        Self {
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            anisotropy: 4,
        }
    }
}

/// Texture wrap mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureWrap {
    /// U/S coordinate wrap mode
    #[serde(skip)]
    pub u: AddressMode,
    /// V/T coordinate wrap mode  
    #[serde(skip)]
    pub v: AddressMode,
    /// W/R coordinate wrap mode
    #[serde(skip)]
    pub w: AddressMode,
}

impl Default for TextureWrap {
    fn default() -> Self {
        Self {
            u: AddressMode::Repeat,
            v: AddressMode::Repeat,
            w: AddressMode::Repeat,
        }
    }
}

/// Texture metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureMetadata {
    /// Texture ID
    pub id: String,
    /// File path (if loaded from file)
    pub file_path: Option<PathBuf>,
    /// Texture dimensions
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// Texture format
    pub format: ColorFormat,
    /// Compression format used
    pub compression: TextureCompression,
    /// Number of mip levels
    pub mip_levels: u32,
    /// Array layers
    pub array_layers: u32,
    /// File size in bytes
    pub file_size: u64,
    /// GPU memory usage in bytes
    pub gpu_memory_size: u64,
    /// Creation timestamp
    pub created_at: std::time::SystemTime,
    /// Last access timestamp
    #[serde(skip, default = "default_last_accessed")]
    pub last_accessed: Arc<Mutex<std::time::SystemTime>>,
    /// Access count
    #[serde(skip)]
    pub access_count: Arc<Mutex<u64>>,
    /// Streaming priority
    pub priority: StreamingPriority,
    /// Whether texture has alpha channel
    pub has_alpha: bool,
    /// Whether texture is sRGB
    pub is_srgb: bool,
    /// Hash of texture data for deduplication
    pub content_hash: u64,
}

/// Cached texture resource
#[derive(Debug, Clone)]
pub struct CachedTexture {
    /// Texture metadata
    pub metadata: TextureMetadata,

    /// WGPU texture
    #[cfg(feature = "video")]
    pub texture: Arc<Texture>,

    /// WGPU texture view
    #[cfg(feature = "video")]
    pub view: Arc<TextureView>,

    /// WGPU sampler
    #[cfg(feature = "video")]
    pub sampler: Arc<Sampler>,

    /// Texture data (for CPU access)
    pub data: Option<Arc<Vec<u8>>>,

    /// Loading state
    pub loading_state: Arc<Mutex<TextureLoadingState>>,
}

/// Texture loading state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextureLoadingState {
    /// Not loaded
    NotLoaded,
    /// Currently loading
    Loading,
    /// Loaded successfully
    Loaded,
    /// Failed to load
    Failed(String),
}

/// Texture streaming request
#[derive(Clone)]
pub struct StreamingRequest {
    /// Texture ID
    pub texture_id: String,
    /// File path to load from
    pub file_path: Option<PathBuf>,
    /// Priority level
    pub priority: StreamingPriority,
    /// Request timestamp
    pub requested_at: std::time::Instant,
    /// Callback for when loading completes
    pub callback: Option<Arc<dyn Fn(Result<Arc<CachedTexture>>) + Send + Sync>>,
}

fn default_last_accessed() -> Arc<Mutex<std::time::SystemTime>> {
    Arc::new(Mutex::new(std::time::SystemTime::now()))
}

/// Texture manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureManagerConfig {
    /// Maximum GPU memory budget in bytes
    pub max_gpu_memory: u64,
    /// Maximum number of cached textures
    pub max_cached_textures: u32,
    /// Maximum streaming threads
    pub max_streaming_threads: u32,
    /// Enable texture compression
    pub enable_compression: bool,
    /// Preferred compression format
    pub preferred_compression: TextureCompression,
    /// Generate mipmaps automatically
    pub generate_mipmaps: bool,
    /// Enable anisotropic filtering
    pub enable_anisotropic: bool,
    /// Maximum anisotropy level
    pub max_anisotropy: u8,
    /// Texture cache directory
    pub cache_directory: Option<PathBuf>,
    /// Enable texture streaming
    pub enable_streaming: bool,
    /// Background streaming enabled
    pub background_streaming: bool,
}

impl Default for TextureManagerConfig {
    fn default() -> Self {
        Self {
            max_gpu_memory: 1024 * 1024 * 1024, // 1GB
            max_cached_textures: 1000,
            max_streaming_threads: 4,
            enable_compression: true,
            preferred_compression: TextureCompression::BC7,
            generate_mipmaps: true,
            enable_anisotropic: true,
            max_anisotropy: 16,
            cache_directory: None,
            enable_streaming: true,
            background_streaming: true,
        }
    }
}

/// Texture manager statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TextureManagerStats {
    /// Total textures cached
    pub total_cached: u32,
    /// Currently loading textures
    pub currently_loading: u32,
    /// Failed loads
    pub failed_loads: u32,
    /// Total GPU memory used
    pub gpu_memory_used: u64,
    /// Total system memory used
    pub system_memory_used: u64,
    /// Cache hit rate (0.0 to 1.0)
    pub cache_hit_rate: f32,
    /// Average load time in milliseconds
    pub average_load_time: f32,
    /// Textures evicted from cache
    pub textures_evicted: u32,
    /// Compression ratio achieved
    pub compression_ratio: f32,
}

/// High-performance texture manager with streaming support
pub struct TextureManager {
    /// Configuration
    config: TextureManagerConfig,

    /// Render device
    render_device: Arc<RenderDevice>,

    /// Texture cache (ID -> Texture)
    texture_cache: Arc<DashMap<String, Arc<CachedTexture>>>,

    /// Metadata cache (ID -> Metadata)
    metadata_cache: Arc<DashMap<String, TextureMetadata>>,

    /// LRU cache for eviction (access_time -> texture_id)
    lru_cache: Arc<RwLock<BTreeMap<std::time::SystemTime, String>>>,

    /// Streaming request queue
    streaming_queue: Arc<RwLock<VecDeque<StreamingRequest>>>,

    /// Currently loading textures
    loading_textures: Arc<DashMap<String, Arc<Mutex<TextureLoadingState>>>>,

    /// Statistics
    statistics: Arc<RwLock<TextureManagerStats>>,

    /// Thread pool for background loading
    thread_pool: Arc<rayon::ThreadPool>,

    /// GPU memory tracker
    gpu_memory_used: Arc<Mutex<u64>>,

    /// Content hash to texture ID mapping (for deduplication)
    content_hash_map: Arc<DashMap<u64, String>>,
}

impl TextureManager {
    /// Create a new texture manager
    pub fn new(render_device: Arc<RenderDevice>, config: TextureManagerConfig) -> Result<Self> {
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(config.max_streaming_threads as usize)
            .build()
            .map_err(|e| {
                VideoDeviceError::InitializationFailed(format!(
                    "Failed to create thread pool: {}",
                    e
                ))
            })?;

        Ok(Self {
            config,
            render_device,
            texture_cache: Arc::new(DashMap::new()),
            metadata_cache: Arc::new(DashMap::new()),
            lru_cache: Arc::new(RwLock::new(BTreeMap::new())),
            streaming_queue: Arc::new(RwLock::new(VecDeque::new())),
            loading_textures: Arc::new(DashMap::new()),
            statistics: Arc::new(RwLock::new(TextureManagerStats::default())),
            thread_pool: Arc::new(thread_pool),
            gpu_memory_used: Arc::new(Mutex::new(0)),
            content_hash_map: Arc::new(DashMap::new()),
        })
    }

    /// Load texture from file with streaming support
    pub async fn load_texture_from_file<P: AsRef<Path>>(
        &self,
        id: String,
        path: P,
        priority: StreamingPriority,
    ) -> Result<Arc<CachedTexture>> {
        let path = path.as_ref().to_path_buf();

        // Check if already cached
        if let Some(cached) = self.texture_cache.get(&id) {
            self.update_access_time(&id).await;
            return Ok(cached.clone());
        }

        // Check if already loading
        if let Some(loading_state) = self.loading_textures.get(&id) {
            match *loading_state.lock() {
                TextureLoadingState::Loading => {
                    // Wait for completion
                    return self.wait_for_texture_load(&id).await;
                }
                TextureLoadingState::Loaded => {
                    if let Some(cached) = self.texture_cache.get(&id) {
                        return Ok(cached.clone());
                    }
                }
                TextureLoadingState::Failed(ref err) => {
                    return Err(VideoDeviceError::ResourceError(err.clone()));
                }
                _ => {}
            }
        }

        // Start loading
        self.start_texture_loading(id.clone(), path, priority).await
    }

    /// Load texture from memory data
    pub async fn load_texture_from_memory(
        &self,
        id: String,
        data: &[u8],
        width: u32,
        height: u32,
        format: ColorFormat,
    ) -> Result<Arc<CachedTexture>> {
        // Calculate content hash for deduplication
        let content_hash = self.calculate_hash(data);

        // Check for existing texture with same content
        if let Some(existing_id) = self.content_hash_map.get(&content_hash) {
            if let Some(cached) = self.texture_cache.get(existing_id.as_str()) {
                // Clone reference to existing texture
                let cloned = cached.clone();
                self.texture_cache.insert(id, cloned.clone());
                return Ok(cloned);
            }
        }

        // Create texture descriptor
        let texture_desc = TextureDesc {
            width,
            height,
            depth: 1,
            format,
            mip_levels: if self.config.generate_mipmaps {
                ((width.max(height) as f32).log2().floor() as u32) + 1
            } else {
                1
            },
            array_layers: 1,
            sample_count: 1,
            usage: TextureUsage {
                shader_resource: true,
                render_target: false,
                storage: false,
                copy_src: true,
                copy_dst: true,
            },
        };

        // Create WGPU texture
        let texture = self
            .render_device
            .create_texture(&texture_desc, Some(data))?;

        #[cfg(feature = "video")]
        let (view, sampler) = {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = self
                .render_device
                .get_wgpu_device()
                .create_sampler(&SamplerDescriptor {
                    label: Some(&format!("{}_sampler", id)),
                    address_mode_u: AddressMode::Repeat,
                    address_mode_v: AddressMode::Repeat,
                    address_mode_w: AddressMode::Repeat,
                    mag_filter: FilterMode::Linear,
                    min_filter: FilterMode::Linear,
                    mipmap_filter: FilterMode::Linear,
                    lod_min_clamp: 0.0,
                    lod_max_clamp: 100.0,
                    compare: None,
                    anisotropy_clamp: if self.config.enable_anisotropic {
                        self.config.max_anisotropy as u16
                    } else {
                        1
                    },
                    border_color: None,
                });
            (Arc::new(view), Arc::new(sampler))
        };

        // Calculate memory usage
        let bytes_per_pixel = Self::get_bytes_per_pixel(format);
        let gpu_memory_size = (width * height * bytes_per_pixel * texture_desc.mip_levels) as u64;

        // Create metadata
        let metadata = TextureMetadata {
            id: id.clone(),
            file_path: None,
            width,
            height,
            depth: 1,
            format,
            compression: TextureCompression::None,
            mip_levels: texture_desc.mip_levels,
            array_layers: 1,
            file_size: data.len() as u64,
            gpu_memory_size,
            created_at: std::time::SystemTime::now(),
            last_accessed: Arc::new(Mutex::new(std::time::SystemTime::now())),
            access_count: Arc::new(Mutex::new(1)),
            priority: StreamingPriority::Normal,
            has_alpha: Self::format_has_alpha(format),
            is_srgb: Self::format_is_srgb(format),
            content_hash,
        };

        // Create cached texture
        let cached_texture = Arc::new(CachedTexture {
            metadata: metadata.clone(),
            #[cfg(feature = "video")]
            texture,
            #[cfg(feature = "video")]
            view,
            #[cfg(feature = "video")]
            sampler,
            data: Some(Arc::new(data.to_vec())),
            loading_state: Arc::new(Mutex::new(TextureLoadingState::Loaded)),
        });

        // Update caches
        self.texture_cache
            .insert(id.clone(), cached_texture.clone());
        self.metadata_cache.insert(id.clone(), metadata);
        self.content_hash_map.insert(content_hash, id);

        // Update memory usage
        *self.gpu_memory_used.lock() += gpu_memory_size;

        // Update statistics
        let mut stats = self.statistics.write();
        stats.total_cached += 1;
        stats.gpu_memory_used += gpu_memory_size;
        stats.system_memory_used += data.len() as u64;

        // Check if we need to evict textures
        if stats.gpu_memory_used > self.config.max_gpu_memory {
            drop(stats);
            self.evict_textures().await?;
        }

        Ok(cached_texture)
    }

    /// Get cached texture by ID
    pub async fn get_texture(&self, id: &str) -> Option<Arc<CachedTexture>> {
        if let Some(texture) = self.texture_cache.get(id) {
            self.update_access_time(id).await;
            Some(texture.clone())
        } else {
            None
        }
    }

    /// Remove texture from cache
    pub async fn remove_texture(&self, id: &str) -> Result<()> {
        if let Some((_, texture)) = self.texture_cache.remove(id) {
            // Update memory tracking
            let gpu_memory = texture.metadata.gpu_memory_size;
            let system_memory = texture.data.as_ref().map(|d| d.len() as u64).unwrap_or(0);

            *self.gpu_memory_used.lock() -= gpu_memory;

            // Remove from other caches
            self.metadata_cache.remove(id);
            self.content_hash_map.remove(&texture.metadata.content_hash);

            // Update statistics
            let mut stats = self.statistics.write();
            stats.total_cached = stats.total_cached.saturating_sub(1);
            stats.gpu_memory_used = stats.gpu_memory_used.saturating_sub(gpu_memory);
            stats.system_memory_used = stats.system_memory_used.saturating_sub(system_memory);

            tracing::debug!(
                "Removed texture: {} (freed {} bytes GPU memory)",
                id,
                gpu_memory
            );
        }

        Ok(())
    }

    /// Request texture streaming (async loading)
    pub async fn request_texture_streaming<P: AsRef<Path>>(
        &self,
        id: String,
        path: P,
        priority: StreamingPriority,
    ) -> Result<()> {
        let request = StreamingRequest {
            texture_id: id,
            file_path: Some(path.as_ref().to_path_buf()),
            priority,
            requested_at: std::time::Instant::now(),
            callback: None,
        };

        // Add to streaming queue
        let mut queue = self.streaming_queue.write();

        // Insert based on priority
        let insert_pos = queue
            .iter()
            .position(|r| r.priority > priority)
            .unwrap_or(queue.len());

        queue.insert(insert_pos, request);

        // Start background processing if enabled
        if self.config.background_streaming {
            self.process_streaming_queue().await;
        }

        Ok(())
    }

    /// Get texture manager statistics
    pub fn get_statistics(&self) -> TextureManagerStats {
        let stats = self.statistics.read().clone();

        // Update cache hit rate
        let total_requests = stats.total_cached + stats.failed_loads;
        let cache_hit_rate = if total_requests > 0 {
            stats.total_cached as f32 / total_requests as f32
        } else {
            0.0
        };

        TextureManagerStats {
            cache_hit_rate,
            ..stats
        }
    }

    /// Clear all cached textures
    pub async fn clear_cache(&self) -> Result<()> {
        self.texture_cache.clear();
        self.metadata_cache.clear();
        self.content_hash_map.clear();

        *self.gpu_memory_used.lock() = 0;

        let mut stats = self.statistics.write();
        *stats = TextureManagerStats::default();

        tracing::info!("Texture cache cleared");
        Ok(())
    }

    // Private helper methods

    async fn start_texture_loading<P: AsRef<Path>>(
        &self,
        id: String,
        path: P,
        priority: StreamingPriority,
    ) -> Result<Arc<CachedTexture>> {
        let path = path.as_ref().to_path_buf();

        // Mark as loading
        let loading_state = Arc::new(Mutex::new(TextureLoadingState::Loading));
        self.loading_textures
            .insert(id.clone(), loading_state.clone());

        // Load in background thread
        let render_device = self.render_device.clone();
        let config = self.config.clone();
        let texture_cache = self.texture_cache.clone();
        let metadata_cache = self.metadata_cache.clone();
        let gpu_memory_used = self.gpu_memory_used.clone();
        let statistics = self.statistics.clone();
        let content_hash_map = self.content_hash_map.clone();
        let loading_id = id.clone();

        let result = tokio::task::spawn_blocking(move || {
            Self::load_texture_from_file_sync(
                &render_device,
                &config,
                loading_id,
                path,
                priority,
                &texture_cache,
                &metadata_cache,
                &gpu_memory_used,
                &statistics,
                &content_hash_map,
            )
        })
        .await;

        match result {
            Ok(Ok(cached_texture)) => {
                *loading_state.lock() = TextureLoadingState::Loaded;
                self.loading_textures.remove(&id);
                Ok(cached_texture)
            }
            Ok(Err(err)) => {
                *loading_state.lock() = TextureLoadingState::Failed(err.to_string());
                self.loading_textures.remove(&id);
                Err(err)
            }
            Err(join_err) => {
                let err_msg = format!("Task join error: {}", join_err);
                *loading_state.lock() = TextureLoadingState::Failed(err_msg.clone());
                self.loading_textures.remove(&id);
                Err(VideoDeviceError::InitializationFailed(err_msg))
            }
        }
    }

    fn load_texture_from_file_sync(
        render_device: &RenderDevice,
        config: &TextureManagerConfig,
        id: String,
        path: PathBuf,
        priority: StreamingPriority,
        texture_cache: &DashMap<String, Arc<CachedTexture>>,
        metadata_cache: &DashMap<String, TextureMetadata>,
        gpu_memory_used: &Mutex<u64>,
        statistics: &RwLock<TextureManagerStats>,
        content_hash_map: &DashMap<u64, String>,
    ) -> Result<Arc<CachedTexture>> {
        // Load image file
        let img = image::open(&path)
            .map_err(|e| VideoDeviceError::ResourceError(format!("Failed to load image: {}", e)))?;

        let (width, height) = img.dimensions();
        let rgba_img = img.to_rgba8();
        let data = rgba_img.as_raw();

        // Calculate content hash
        let content_hash = Self::calculate_hash_static(data);

        // Check for duplicates
        if let Some(existing_id) = content_hash_map.get(&content_hash) {
            if let Some(existing) = texture_cache.get(existing_id.as_str()) {
                // Reuse existing texture
                texture_cache.insert(id, existing.clone());
                return Ok(existing.clone());
            }
        }

        // Create texture
        let format = ColorFormat::Rgba8;
        let texture_desc = TextureDesc {
            width,
            height,
            depth: 1,
            format,
            mip_levels: if config.generate_mipmaps {
                ((width.max(height) as f32).log2().floor() as u32) + 1
            } else {
                1
            },
            array_layers: 1,
            sample_count: 1,
            usage: TextureUsage::default(),
        };

        let texture = render_device.create_texture(&texture_desc, Some(data))?;

        #[cfg(feature = "video")]
        let (view, sampler) = {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let sampler = render_device
                .get_wgpu_device()
                .create_sampler(&SamplerDescriptor::default());
            (Arc::new(view), Arc::new(sampler))
        };

        // Calculate sizes
        let bytes_per_pixel = Self::get_bytes_per_pixel(format);
        let gpu_memory_size = (width * height * bytes_per_pixel * texture_desc.mip_levels) as u64;
        let file_size = std::fs::metadata(&path)
            .map(|m| m.len())
            .unwrap_or(data.len() as u64);

        // Create metadata
        let metadata = TextureMetadata {
            id: id.clone(),
            file_path: Some(path),
            width,
            height,
            depth: 1,
            format,
            compression: TextureCompression::None,
            mip_levels: texture_desc.mip_levels,
            array_layers: 1,
            file_size,
            gpu_memory_size,
            created_at: std::time::SystemTime::now(),
            last_accessed: Arc::new(Mutex::new(std::time::SystemTime::now())),
            access_count: Arc::new(Mutex::new(1)),
            priority,
            has_alpha: true,
            is_srgb: false,
            content_hash,
        };

        let cached_texture = Arc::new(CachedTexture {
            metadata: metadata.clone(),
            #[cfg(feature = "video")]
            texture,
            #[cfg(feature = "video")]
            view,
            #[cfg(feature = "video")]
            sampler,
            data: Some(Arc::new(data.to_vec())),
            loading_state: Arc::new(Mutex::new(TextureLoadingState::Loaded)),
        });

        // Update caches
        texture_cache.insert(id.clone(), cached_texture.clone());
        metadata_cache.insert(id.clone(), metadata);
        content_hash_map.insert(content_hash, id);

        // Update memory usage
        *gpu_memory_used.lock() += gpu_memory_size;

        // Update statistics
        let mut stats = statistics.write();
        stats.total_cached += 1;
        stats.gpu_memory_used += gpu_memory_size;

        Ok(cached_texture)
    }

    async fn wait_for_texture_load(&self, id: &str) -> Result<Arc<CachedTexture>> {
        // Simple polling wait - in production you'd use proper async coordination
        for _ in 0..1000 {
            // Max 10 seconds
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            if let Some(texture) = self.texture_cache.get(id) {
                return Ok(texture.clone());
            }

            if let Some(loading_state) = self.loading_textures.get(id) {
                match &*loading_state.lock() {
                    TextureLoadingState::Failed(err) => {
                        return Err(VideoDeviceError::ResourceError(err.clone()));
                    }
                    TextureLoadingState::NotLoaded => break,
                    _ => continue,
                }
            }
        }

        Err(VideoDeviceError::ResourceError(
            "Timeout waiting for texture load".to_string(),
        ))
    }

    async fn update_access_time(&self, id: &str) {
        if let Some(texture) = self.texture_cache.get(id) {
            let now = std::time::SystemTime::now();
            *texture.metadata.last_accessed.lock() = now;
            *texture.metadata.access_count.lock() += 1;

            // Update LRU cache
            let mut lru = self.lru_cache.write();
            lru.insert(now, id.to_string());
        }
    }

    async fn evict_textures(&self) -> Result<()> {
        let mut stats = self.statistics.write();
        let target_memory = self.config.max_gpu_memory * 80 / 100; // Evict to 80% capacity

        while stats.gpu_memory_used > target_memory && stats.total_cached > 0 {
            // Find oldest texture to evict
            let texture_to_evict = {
                let lru = self.lru_cache.read();
                lru.iter().next().map(|(_, id)| id.clone())
            };

            if let Some(id) = texture_to_evict {
                drop(stats); // Release lock before async call
                self.remove_texture(&id).await?;
                stats = self.statistics.write();
                stats.textures_evicted += 1;
            } else {
                break;
            }
        }

        tracing::info!(
            "Texture eviction completed. GPU memory: {} MB",
            stats.gpu_memory_used / (1024 * 1024)
        );

        Ok(())
    }

    async fn process_streaming_queue(&self) {
        let mut queue = self.streaming_queue.write();

        while let Some(request) = queue.pop_front() {
            let texture_id = request.texture_id.clone();
            let callback = request.callback.clone();
            let file_path = request.file_path.clone();

            tracing::debug!("Processing streaming request for texture: {}", texture_id);

            if self.texture_cache.contains_key(&texture_id) {
                tracing::debug!("Texture '{}' already cached, skipping load", texture_id);
                if let Some(cb) = callback {
                    if let Some(cached) = self.texture_cache.get(&texture_id) {
                        cb(Ok(cached.clone()));
                    }
                }
                continue;
            }

            let loading_state = Arc::new(Mutex::new(TextureLoadingState::Loading));
            self.loading_textures
                .insert(texture_id.clone(), loading_state.clone());

            drop(queue);

            let render_device = self.render_device.clone();
            let config = self.config.clone();
            let texture_cache = self.texture_cache.clone();
            let metadata_cache = self.metadata_cache.clone();
            let gpu_memory_used = self.gpu_memory_used.clone();
            let statistics = self.statistics.clone();
            let content_hash_map = self.content_hash_map.clone();
            let load_id = texture_id.clone();
            let loading_state_clone = loading_state.clone();

            let result = tokio::task::spawn_blocking(move || match file_path {
                Some(path) => Self::load_texture_from_file_sync(
                    &render_device,
                    &config,
                    load_id.clone(),
                    path,
                    StreamingPriority::Normal,
                    &texture_cache,
                    &metadata_cache,
                    &gpu_memory_used,
                    &statistics,
                    &content_hash_map,
                ),
                None => {
                    let path = metadata_cache
                        .get(&load_id)
                        .and_then(|m| m.file_path.clone());
                    match path {
                        Some(path) => Self::load_texture_from_file_sync(
                            &render_device,
                            &config,
                            load_id.clone(),
                            path,
                            StreamingPriority::Normal,
                            &texture_cache,
                            &metadata_cache,
                            &gpu_memory_used,
                            &statistics,
                            &content_hash_map,
                        ),
                        None => Err(VideoDeviceError::ResourceError(format!(
                            "No file path for streaming texture '{}'",
                            load_id
                        ))),
                    }
                }
            })
            .await;

            match result {
                Ok(Ok(cached_texture)) => {
                    *loading_state_clone.lock() = TextureLoadingState::Loaded;
                    self.loading_textures.remove(&texture_id);
                    if let Some(cb) = callback {
                        cb(Ok(cached_texture));
                    }
                }
                Ok(Err(err)) => {
                    let err_msg = err.to_string();
                    *loading_state_clone.lock() = TextureLoadingState::Failed(err_msg.clone());
                    self.loading_textures.remove(&texture_id);
                    tracing::warn!(
                        "Failed to load streaming texture '{}': {}",
                        texture_id,
                        err_msg
                    );
                    if let Some(cb) = callback {
                        cb(Err(err));
                    }
                }
                Err(join_err) => {
                    let err_msg = format!("Task join error: {}", join_err);
                    *loading_state_clone.lock() = TextureLoadingState::Failed(err_msg.clone());
                    self.loading_textures.remove(&texture_id);
                    tracing::error!("Streaming texture task panicked: {}", err_msg);
                    if let Some(cb) = callback {
                        cb(Err(VideoDeviceError::InitializationFailed(err_msg)));
                    }
                }
            }

            queue = self.streaming_queue.write();
        }
    }

    fn calculate_hash(&self, data: &[u8]) -> u64 {
        Self::calculate_hash_static(data)
    }

    fn calculate_hash_static(data: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    fn get_bytes_per_pixel(format: ColorFormat) -> u32 {
        match format {
            ColorFormat::Rgba8 | ColorFormat::Bgra8 => 4,
            ColorFormat::Rgba16 => 8,
            ColorFormat::Rgba32Float => 16,
            ColorFormat::Rgb10A2 | ColorFormat::Hdr10 => 4,
            ColorFormat::Depth24Stencil8 => 4,
            ColorFormat::Depth32Float => 4,
        }
    }

    fn format_has_alpha(format: ColorFormat) -> bool {
        match format {
            ColorFormat::Rgba8
            | ColorFormat::Bgra8
            | ColorFormat::Rgba16
            | ColorFormat::Rgba32Float
            | ColorFormat::Rgb10A2
            | ColorFormat::Hdr10 => true,
            _ => false,
        }
    }

    fn format_is_srgb(format: ColorFormat) -> bool {
        match format {
            ColorFormat::Rgba8 | ColorFormat::Bgra8 => true,
            _ => false,
        }
    }
}

impl Drop for TextureManager {
    fn drop(&mut self) {
        tracing::debug!("Texture manager dropped");
    }
}
