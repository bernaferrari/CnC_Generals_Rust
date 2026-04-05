//! Complete Texture Management System
//!
//! This module provides a high-level interface that integrates all texture
//! system components: loading, caching, sampling, and asset management.

use crate::core::error::RendererResult;
use crate::rendering::texture_system::{
    asset_texture_loader::ArchiveFileReader, AssetTextureLoader, TextureBaseClass,
    TextureCacheConfig, TextureFileCache, TextureFilteringUtils, TextureSamplerManager,
    TextureSamplingConfig, TextureUsage,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use wgpu::{Device, Queue, Sampler};
use ww3d_assets::AssetManager;

/// Complete texture management system
pub struct TextureManager {
    /// Asset-integrated texture loader
    asset_loader: AssetTextureLoader,
    /// File-based texture cache
    file_cache: TextureFileCache,
    /// Sampler manager for filtering and addressing
    sampler_manager: TextureSamplerManager,
    /// Active texture bindings
    active_textures: HashMap<u32, Arc<TextureBaseClass>>,
    /// Texture usage statistics
    stats: TextureManagerStats,
}

/// Texture manager statistics
#[derive(Debug, Clone, Default)]
pub struct TextureManagerStats {
    pub textures_loaded: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub memory_usage_mb: u64,
    pub active_bindings: usize,
}

impl TextureManager {
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        asset_manager: Arc<Mutex<AssetManager>>,
    ) -> RendererResult<Self> {
        Self::with_archive_reader(device, queue, asset_manager, None)
    }

    pub fn with_archive_reader(
        device: Arc<Device>,
        queue: Arc<Queue>,
        asset_manager: Arc<Mutex<AssetManager>>,
        archive_reader: Option<Arc<dyn ArchiveFileReader>>,
    ) -> RendererResult<Self> {
        let asset_loader = AssetTextureLoader::with_archive_reader(
            device.clone(),
            queue,
            asset_manager,
            archive_reader,
        )?;
        let cache_config = TextureCacheConfig::default();
        let file_cache = TextureFileCache::new_with_config("textures", cache_config);
        let sampler_manager = TextureSamplerManager::new(device);

        Ok(Self {
            asset_loader,
            file_cache,
            sampler_manager,
            active_textures: HashMap::new(),
            stats: TextureManagerStats::default(),
        })
    }

    pub fn set_archive_reader(&mut self, reader: Arc<dyn ArchiveFileReader>) {
        self.asset_loader.set_archive_reader(reader);
    }

    /// Load a texture with automatic format detection and caching
    pub fn load_texture(
        &mut self,
        filename: &str,
        _usage: TextureUsage,
    ) -> RendererResult<Arc<TextureBaseClass>> {
        // Check if already loaded
        if let Some(texture) = self.file_cache.get_texture(filename) {
            self.stats.cache_hits += 1;
            return Ok(texture);
        }

        self.stats.cache_misses += 1;

        // Load texture using asset loader
        let texture = self.asset_loader.load_texture(filename)?;

        // Add to cache
        self.file_cache.add_texture(filename, (*texture).clone())?;

        self.stats.textures_loaded += 1;
        Ok(texture)
    }

    /// Get appropriate sampler for texture usage
    pub fn get_sampler_for_usage(&mut self, usage: TextureUsage) -> Arc<Sampler> {
        let filter_quality = TextureFilteringUtils::get_recommended_filter_quality(usage);

        let config = match usage {
            TextureUsage::UI | TextureUsage::HUD | TextureUsage::Font => {
                TextureSamplingConfig::ui_optimized()
            }
            TextureUsage::Terrain => TextureSamplingConfig::terrain_optimized(),
            TextureUsage::Shadow => TextureSamplingConfig::shadow_mapping(),
            _ => {
                let mut config = TextureSamplingConfig::default();
                config.filter_quality = filter_quality;
                config
            }
        };

        self.sampler_manager.get_or_create_sampler(&config)
    }

    /// Bind texture to a specific binding slot
    pub fn bind_texture(&mut self, slot: u32, texture: Arc<TextureBaseClass>) {
        self.active_textures.insert(slot, texture);
        self.update_binding_stats();
    }

    /// Unbind texture from a specific slot
    pub fn unbind_texture(&mut self, slot: u32) {
        self.active_textures.remove(&slot);
        self.update_binding_stats();
    }

    /// Get bound texture at slot
    pub fn get_bound_texture(&self, slot: u32) -> Option<&Arc<TextureBaseClass>> {
        self.active_textures.get(&slot)
    }

    /// Preload common textures for better performance
    pub fn preload_common_textures(&mut self) -> RendererResult<()> {
        self.asset_loader.preload_common_textures()?;

        // Load some commonly used texture patterns
        let common_patterns = [
            ("white", TextureUsage::Diffuse),
            ("black", TextureUsage::Diffuse),
            ("normal", TextureUsage::Normal),
            ("default", TextureUsage::Diffuse),
        ];

        for (name, usage) in &common_patterns {
            let _ = self.load_texture(&format!("{}.dds", name), *usage);
        }

        Ok(())
    }

    /// Perform cache maintenance
    pub fn cleanup_cache(&mut self) {
        self.file_cache.cleanup_unused_textures();
        self.asset_loader.clear_cache();
        self.sampler_manager.cleanup_unused_samplers();
    }

    /// Force garbage collection of all caches
    pub fn garbage_collect(&mut self) {
        self.file_cache.garbage_collect();
        self.cleanup_cache();
    }

    /// Get comprehensive statistics
    pub fn get_stats(&self) -> TextureManagerStats {
        let cache_stats = self.file_cache.get_cache_stats();

        TextureManagerStats {
            textures_loaded: self.stats.textures_loaded,
            cache_hits: self.stats.cache_hits,
            cache_misses: self.stats.cache_misses,
            memory_usage_mb: cache_stats.total_memory_mb,
            active_bindings: self.active_textures.len(),
        }
    }

    /// Set texture quality settings globally
    pub fn set_quality_settings(&mut self, settings: TextureQualitySettings) {
        // Update cache configuration
        let mut cache_config = TextureCacheConfig::default();
        cache_config.max_memory_mb = settings.max_cache_memory_mb;

        // In a full implementation, we would recreate the cache with new settings
        // For now, keep the configuration value accessible for debugging.
        let _cache_config = cache_config;
    }

    /// Create optimized sampler configuration for specific scenarios
    pub fn create_custom_sampler(&mut self, config: TextureSamplingConfig) -> Arc<Sampler> {
        self.sampler_manager.get_or_create_sampler(&config)
    }

    /// Hot-reload texture (useful for development)
    pub fn hot_reload_texture(&mut self, filename: &str) -> RendererResult<Arc<TextureBaseClass>> {
        // Remove from cache to force reload
        self.file_cache.release_texture(filename);

        // Reload the texture
        self.load_texture(filename, TextureUsage::Diffuse)
    }

    /// Update internal statistics
    fn update_binding_stats(&mut self) {
        self.stats.active_bindings = self.active_textures.len();
    }

    /// Check if texture manager is healthy
    pub fn health_check(&self) -> TextureManagerHealth {
        let stats = self.get_stats();
        let cache_stats = self.file_cache.get_cache_stats();

        let cache_hit_ratio = if stats.cache_hits + stats.cache_misses > 0 {
            stats.cache_hits as f32 / (stats.cache_hits + stats.cache_misses) as f32
        } else {
            0.0
        };

        let memory_critical = cache_stats.is_memory_critical();
        let low_cache_efficiency = cache_hit_ratio < 0.7;
        let too_many_bindings = stats.active_bindings > 100;

        TextureManagerHealth {
            overall_healthy: !memory_critical && !low_cache_efficiency && !too_many_bindings,
            cache_hit_ratio,
            memory_usage_percent: cache_stats.memory_usage_percent(),
            issues: {
                let mut issues = Vec::new();
                if memory_critical {
                    issues.push("Memory usage critical".to_string());
                }
                if low_cache_efficiency {
                    issues.push("Low cache hit ratio".to_string());
                }
                if too_many_bindings {
                    issues.push("Too many active texture bindings".to_string());
                }
                issues
            },
        }
    }
}

/// Texture quality settings
#[derive(Debug, Clone)]
pub struct TextureQualitySettings {
    pub max_cache_memory_mb: u64,
    pub default_anisotropy: u16,
    pub mipmap_bias: f32,
    pub enable_compression: bool,
    pub max_texture_size: u32,
}

impl Default for TextureQualitySettings {
    fn default() -> Self {
        Self {
            max_cache_memory_mb: 512,
            default_anisotropy: 16,
            mipmap_bias: 0.0,
            enable_compression: true,
            max_texture_size: 4096,
        }
    }
}

/// Health check results for texture manager
#[derive(Debug, Clone)]
pub struct TextureManagerHealth {
    pub overall_healthy: bool,
    pub cache_hit_ratio: f32,
    pub memory_usage_percent: f32,
    pub issues: Vec<String>,
}
