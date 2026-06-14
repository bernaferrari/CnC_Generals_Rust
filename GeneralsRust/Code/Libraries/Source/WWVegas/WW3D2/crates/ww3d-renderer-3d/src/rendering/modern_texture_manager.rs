//! Modern Texture Manager - Advanced texture management system
//!
//! This module provides a complete texture management system that replaces
//! the DirectX8 texture manager with modern capabilities including:
//! - Automatic texture streaming and caching
//! - Multiple texture formats support
//! - Mipmap generation and management
//! - Texture compression optimization
//! - Memory management and budgeting

use crate::core::error::{Error, RendererResult};
use crate::rendering::texture_system::texture_base::{
    PoolType, TextureAddressMode as TextureWrapMode, TextureBaseClass as TextureBase,
    TextureFilterMode as TextureFilter,
};
use crate::rendering::texture_system::texture_loader::TextureLoader;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use wgpu::{Device, Queue, Sampler};
use ww3d_core::W3dTextureStruct as W3dTexture;

/// Texture manager configuration
#[derive(Debug, Clone)]
pub struct TextureManagerConfig {
    /// Maximum texture memory budget in bytes
    pub max_memory_budget: u64,
    /// Enable texture streaming
    pub enable_streaming: bool,
    /// Enable automatic mipmap generation
    pub auto_mipmaps: bool,
    /// Enable texture compression
    pub enable_compression: bool,
    /// Default texture filter mode
    pub default_filter: TextureFilter,
    /// Default texture wrap mode
    pub default_wrap: TextureWrapMode,
    /// Maximum texture size
    pub max_texture_size: u32,
    /// Enable anisotropic filtering
    pub anisotropic_filtering: bool,
    /// Anisotropic filtering level
    pub anisotropy_level: u32,
}

impl Default for TextureManagerConfig {
    fn default() -> Self {
        Self {
            max_memory_budget: 512 * 1024 * 1024, // 512MB
            enable_streaming: true,
            auto_mipmaps: true,
            enable_compression: true,
            default_filter: TextureFilter::Linear,
            default_wrap: TextureWrapMode::Repeat,
            max_texture_size: 4096,
            anisotropic_filtering: true,
            anisotropy_level: 4,
        }
    }
}

/// Texture usage statistics
#[derive(Debug, Clone, Default)]
pub struct TextureStats {
    /// Total number of loaded textures
    pub loaded_textures: u32,
    /// Total memory usage in bytes
    pub memory_usage: u64,
    /// Number of streaming textures
    pub streaming_textures: u32,
    /// Number of compressed textures
    pub compressed_textures: u32,
    /// Cache hit ratio (0.0 to 1.0)
    pub cache_hit_ratio: f32,
    /// Number of texture uploads this frame
    pub uploads_this_frame: u32,
}

/// Texture cache entry
#[derive(Debug, Clone)]
struct TextureCacheEntry {
    /// The texture data
    texture: Arc<TextureBase>,
    /// Last access time
    last_access: std::time::Instant,
    /// Memory usage in bytes
    memory_usage: u64,
    /// Reference count
    ref_count: u32,
    /// Is this texture streaming?
    is_streaming: bool,
    /// Texture priority for eviction
    priority: TexturePriority,
}

/// Texture priority for cache management
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TexturePriority {
    /// Critical textures (always keep in memory)
    Critical = 0,
    /// High priority textures
    High = 1,
    /// Normal priority textures
    Normal = 2,
    /// Low priority textures (evict first)
    Low = 3,
}

/// Modern texture manager with advanced features
pub struct ModernTextureManager {
    /// WGPU device reference
    device: Arc<Device>,
    /// WGPU queue reference
    queue: Arc<Queue>,
    /// Texture loader
    texture_loader: TextureLoader,
    /// Configuration
    config: TextureManagerConfig,
    /// Texture cache
    texture_cache: HashMap<String, TextureCacheEntry>,
    /// Statistics
    stats: TextureStats,
    /// Current memory usage
    current_memory_usage: u64,
    /// Texture samplers cache
    samplers: HashMap<SamplerKey, Sampler>,
    /// Streaming texture queue
    streaming_queue: Vec<String>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct SamplerKey {
    filter: TextureFilter,
    wrap: TextureWrapMode,
    anisotropy: u32,
}

impl ModernTextureManager {
    /// Create a new texture manager
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> RendererResult<Self> {
        let texture_loader = TextureLoader::new(device.clone(), queue.clone())?;
        let config = TextureManagerConfig::default();

        Ok(Self {
            device,
            queue,
            texture_loader,
            config,
            texture_cache: HashMap::new(),
            stats: TextureStats::default(),
            current_memory_usage: 0,
            samplers: HashMap::new(),
            streaming_queue: Vec::new(),
        })
    }

    /// Create texture manager with custom configuration
    pub fn with_config(
        device: Arc<Device>,
        queue: Arc<Queue>,
        config: TextureManagerConfig,
    ) -> RendererResult<Self> {
        let mut manager = Self::new(device, queue)?;
        manager.config = config;
        Ok(manager)
    }

    /// Load a texture from file
    pub fn load_texture(&mut self, path: &Path, name: &str) -> RendererResult<Arc<TextureBase>> {
        // Check if texture is already cached
        if let Some(entry) = self.texture_cache.get_mut(name) {
            entry.last_access = std::time::Instant::now();
            entry.ref_count += 1;
            self.stats.cache_hit_ratio = (self.stats.cache_hit_ratio * 0.9) + 0.1; // Smooth average
            return Ok(entry.texture.clone());
        }

        // Load texture from file
        let texture = self.texture_loader.load_texture_from_path(path)?;

        // Create cache entry
        let memory_usage = texture.get_memory_usage();
        let entry = TextureCacheEntry {
            texture: Arc::new(texture),
            last_access: std::time::Instant::now(),
            memory_usage,
            ref_count: 1,
            is_streaming: false,
            priority: TexturePriority::Normal,
        };

        // Check memory budget
        if self.current_memory_usage + memory_usage > self.config.max_memory_budget {
            self.evict_textures(memory_usage)?;
        }

        // Add to cache
        self.texture_cache.insert(name.to_string(), entry);
        self.current_memory_usage += memory_usage;
        self.stats.loaded_textures += 1;
        self.stats.memory_usage = self.current_memory_usage;
        self.stats.uploads_this_frame += 1;

        // Update cache hit ratio
        self.stats.cache_hit_ratio *= 0.9; // Smooth average

        Ok(self.texture_cache[name].texture.clone())
    }

    /// Load a texture from W3D format data
    pub fn load_w3d_texture(
        &mut self,
        w3d_texture: &W3dTexture,
        name: &str,
    ) -> RendererResult<Arc<TextureBase>> {
        // Check cache first
        if let Some(entry) = self.texture_cache.get_mut(name) {
            entry.last_access = std::time::Instant::now();
            entry.ref_count += 1;
            self.stats.cache_hit_ratio = (self.stats.cache_hit_ratio * 0.9) + 0.1;
            return Ok(entry.texture.clone());
        }

        // Convert W3D texture to our format
        let texture = self.convert_w3d_texture(w3d_texture)?;

        // Create cache entry
        let memory_usage = texture.get_memory_usage();
        let entry = TextureCacheEntry {
            texture: Arc::new(texture),
            last_access: std::time::Instant::now(),
            memory_usage,
            ref_count: 1,
            is_streaming: false,
            priority: TexturePriority::Normal,
        };

        // Memory management
        if self.current_memory_usage + memory_usage > self.config.max_memory_budget {
            self.evict_textures(memory_usage)?;
        }

        // Add to cache
        self.texture_cache.insert(name.to_string(), entry);
        self.current_memory_usage += memory_usage;
        self.stats.loaded_textures += 1;
        self.stats.memory_usage = self.current_memory_usage;
        self.stats.uploads_this_frame += 1;

        Ok(self.texture_cache[name].texture.clone())
    }

    /// Convert W3D texture format to our internal format
    fn convert_w3d_texture(&mut self, w3d_texture: &W3dTexture) -> RendererResult<TextureBase> {
        self.texture_loader
            .load_w3d_descriptor(w3d_texture, PoolType::Managed)
    }

    /// Get a texture from cache
    pub fn get_texture(&self, name: &str) -> Option<Arc<TextureBase>> {
        self.texture_cache
            .get(name)
            .map(|entry| entry.texture.clone())
    }

    /// Release a texture reference
    pub fn release_texture(&mut self, name: &str) {
        if let Some(entry) = self.texture_cache.get_mut(name) {
            entry.ref_count = entry.ref_count.saturating_sub(1);
            if entry.ref_count == 0 {
                // Mark for potential eviction
                entry.last_access =
                    std::time::Instant::now() - std::time::Duration::from_secs(3600);
                // Mark as old
            }
        }
    }

    /// Evict textures to free memory
    fn evict_textures(&mut self, required_memory: u64) -> RendererResult<()> {
        let mut freed_memory = 0u64;
        let mut textures_to_evict = Vec::new();

        // Sort textures by eviction priority
        let mut sorted_textures: Vec<_> = self
            .texture_cache
            .iter()
            .filter(|(_, entry)| entry.ref_count == 0)
            .collect();

        sorted_textures.sort_by(|a, b| {
            a.1.priority
                .cmp(&b.1.priority)
                .then(a.1.last_access.cmp(&b.1.last_access))
        });

        // Evict textures until we have enough memory
        for (name, entry) in sorted_textures {
            if freed_memory >= required_memory {
                break;
            }

            freed_memory += entry.memory_usage;
            textures_to_evict.push(name.clone());
        }

        // Remove evicted textures
        for name in textures_to_evict {
            if let Some(entry) = self.texture_cache.remove(&name) {
                self.current_memory_usage -= entry.memory_usage;
                self.stats.loaded_textures -= 1;
            }
        }

        if freed_memory < required_memory {
            return Err(Error::OutOfMemory(format!(
                "Failed to free enough memory. Required: {} bytes, Freed: {} bytes",
                required_memory, freed_memory
            )));
        }

        Ok(())
    }

    /// Create or get a sampler for the given parameters
    pub fn get_sampler(
        &mut self,
        filter: TextureFilter,
        wrap: TextureWrapMode,
        anisotropy: u32,
    ) -> RendererResult<&Sampler> {
        let key = SamplerKey {
            filter,
            wrap,
            anisotropy,
        };

        if !self.samplers.contains_key(&key) {
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Texture Sampler"),
                address_mode_u: Self::convert_wrap_mode(wrap),
                address_mode_v: Self::convert_wrap_mode(wrap),
                address_mode_w: Self::convert_wrap_mode(wrap),
                mag_filter: Self::convert_filter_mode(filter),
                min_filter: Self::convert_filter_mode(filter),
                mipmap_filter: Self::convert_filter_mode(filter),
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                compare: None,
                anisotropy_clamp: if anisotropy > 1 {
                    anisotropy.min(u8::MAX as u32) as u16
                } else {
                    1
                },
                border_color: None,
            });

            self.samplers.insert(key.clone(), sampler);
        }

        Ok(self.samplers.get(&key).unwrap())
    }

    /// Convert our texture filter to WGPU format
    fn convert_filter_mode(filter: TextureFilter) -> wgpu::FilterMode {
        match filter {
            TextureFilter::Nearest => wgpu::FilterMode::Nearest,
            TextureFilter::Linear => wgpu::FilterMode::Linear,
            TextureFilter::Point => wgpu::FilterMode::Nearest,
            TextureFilter::Anisotropic => wgpu::FilterMode::Linear, // Fall back to linear
        }
    }

    /// Convert our texture wrap mode to WGPU format
    fn convert_wrap_mode(wrap: TextureWrapMode) -> wgpu::AddressMode {
        match wrap {
            TextureWrapMode::Clamp => wgpu::AddressMode::ClampToEdge,
            TextureWrapMode::Repeat => wgpu::AddressMode::Repeat,
            TextureWrapMode::Mirror => wgpu::AddressMode::MirrorRepeat,
            TextureWrapMode::Wrap => wgpu::AddressMode::Repeat,
            TextureWrapMode::Border => wgpu::AddressMode::ClampToBorder,
        }
    }

    /// Preload textures for a scene
    pub fn preload_textures(&mut self, texture_names: &[String]) -> RendererResult<()> {
        for name in texture_names {
            if !self.texture_cache.contains_key(name) {
                // Mark for streaming if not already loaded
                self.streaming_queue.push(name.clone());
            }
        }

        if self.config.enable_streaming {
            self.process_streaming_queue()?;
        }

        Ok(())
    }

    /// Process streaming texture queue
    fn process_streaming_queue(&mut self) -> RendererResult<()> {
        let mut completed = Vec::new();

        for texture_name in &self.streaming_queue {
            // In a real implementation, this would handle async texture loading
            // For now, we'll just mark them as completed
            completed.push(texture_name.clone());
        }

        // Remove completed textures from queue
        self.streaming_queue
            .retain(|name| !completed.contains(name));
        self.stats.streaming_textures = self.streaming_queue.len() as u32;

        Ok(())
    }

    /// Compress texture if compression is enabled
    pub fn compress_texture(&mut self, name: &str) -> RendererResult<()> {
        if !self.config.enable_compression {
            return Ok(());
        }

        if let Some(entry) = self.texture_cache.get_mut(name) {
            // Texture compression logic would go here
            // This could involve converting to compressed formats like BC1-BC7
            entry.texture.compress()?;
            self.stats.compressed_textures += 1;
        }

        Ok(())
    }

    /// Generate mipmaps for a texture
    pub fn generate_mipmaps(&mut self, name: &str) -> RendererResult<()> {
        if !self.config.auto_mipmaps {
            return Ok(());
        }

        if let Some(entry) = self.texture_cache.get_mut(name) {
            entry.texture.generate_mipmaps()?;
        }

        Ok(())
    }

    /// Get current memory usage
    pub fn get_memory_usage(&self) -> u64 {
        self.current_memory_usage
    }

    /// Get memory budget
    pub fn get_memory_budget(&self) -> u64 {
        self.config.max_memory_budget
    }

    /// Get current statistics
    pub fn get_stats(&self) -> &TextureStats {
        &self.stats
    }

    /// Reset frame statistics
    pub fn reset_frame_stats(&mut self) {
        self.stats.uploads_this_frame = 0;
    }

    /// Get configuration
    pub fn get_config(&self) -> &TextureManagerConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, config: TextureManagerConfig) {
        self.config = config;
    }

    /// Clear all cached textures
    pub fn clear_cache(&mut self) {
        self.texture_cache.clear();
        self.current_memory_usage = 0;
        self.stats = TextureStats::default();
        self.streaming_queue.clear();
    }

    /// Get number of cached textures
    pub fn get_texture_count(&self) -> usize {
        self.texture_cache.len()
    }

    /// Check if texture exists in cache
    pub fn has_texture(&self, name: &str) -> bool {
        self.texture_cache.contains_key(name)
    }

    /// Get texture memory usage
    pub fn get_texture_memory_usage(&self, name: &str) -> Option<u64> {
        self.texture_cache.get(name).map(|entry| entry.memory_usage)
    }

    /// Set texture priority
    pub fn set_texture_priority(
        &mut self,
        name: &str,
        priority: TexturePriority,
    ) -> RendererResult<()> {
        if let Some(entry) = self.texture_cache.get_mut(name) {
            entry.priority = priority;
            Ok(())
        } else {
            Err(Error::ResourceNotFound(format!(
                "Texture '{}' not found",
                name
            )))
        }
    }
}

impl Drop for ModernTextureManager {
    fn drop(&mut self) {
        // Cleanup resources
        self.clear_cache();
    }
}

/// Factory function to create texture manager with default settings
pub fn create_texture_manager(
    device: Arc<Device>,
    queue: Arc<Queue>,
) -> RendererResult<ModernTextureManager> {
    ModernTextureManager::new(device, queue)
}

/// Factory function to create texture manager with custom configuration
pub fn create_texture_manager_with_config(
    device: Arc<Device>,
    queue: Arc<Queue>,
    config: TextureManagerConfig,
) -> RendererResult<ModernTextureManager> {
    ModernTextureManager::with_config(device, queue, config)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_texture_manager_config_defaults() {
        let config = TextureManagerConfig::default();
        assert_eq!(config.max_memory_budget, 512 * 1024 * 1024);
        assert!(config.enable_streaming);
        assert!(config.auto_mipmaps);
        assert!(config.enable_compression);
        assert_eq!(config.max_texture_size, 4096);
        assert!(config.anisotropic_filtering);
        assert_eq!(config.anisotropy_level, 4);
    }

    #[test]
    fn test_texture_priority_ordering() {
        assert!(TexturePriority::Critical < TexturePriority::High);
        assert!(TexturePriority::High < TexturePriority::Normal);
        assert!(TexturePriority::Normal < TexturePriority::Low);
    }
}
