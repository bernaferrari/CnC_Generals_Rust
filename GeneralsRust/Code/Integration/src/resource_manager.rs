//! # Resource Manager
//!
//! The Resource Manager provides global resource management and pooling for all game assets
//! including textures, audio, models, and data files.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, trace, warn};

use crate::event_system::{EventPriority, EventSystem, SystemEvent};
use crate::{IntegrationError, IntegrationResult, ResourceConfig};

const USAGE_SAMPLE_INTERVAL_FRAMES: u64 = 1;
const CLEANUP_INTERVAL_FRAMES: u64 = 30;
const PRESSURE_CHECK_INTERVAL_FRAMES: u64 = 120;
use ww3d_engine::FrameTiming;

/// Resource usage information
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub texture_memory_mb: u64,
    pub audio_memory_mb: u64,
    pub cache_memory_mb: u64,
    pub total_memory_mb: u64,
    pub loaded_assets: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Resource manager handles global resource pooling and management
#[derive(Debug)]
pub struct ResourceManager {
    config: ResourceConfig,
    event_system: Arc<EventSystem>,

    // Resource pools
    texture_pool: TexturePool,
    audio_pool: AudioPool,
    model_pool: ModelPool,
    data_pool: DataPool,

    // Usage tracking
    usage: ResourceUsage,
    last_usage_frame: Option<u64>,
    last_cleanup_frame: Option<u64>,
    last_pressure_check_frame: Option<u64>,
}

use std::collections::VecDeque;
use std::path::PathBuf;

/// Texture resource handle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureHandle {
    pub id: u32,
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

/// Audio resource handle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioHandle {
    pub id: u32,
    pub path: PathBuf,
    pub duration_ms: u32,
    pub sample_rate: u32,
    pub channels: u32,
}

/// Model resource handle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelHandle {
    pub id: u32,
    pub path: PathBuf,
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub bone_count: u32,
}

/// Data file resource handle
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DataHandle {
    pub id: u32,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub compression: String,
}

/// Texture resource pool based on C++ asset management patterns
#[derive(Debug)]
struct TexturePool {
    loaded_textures: HashMap<String, TextureHandle>,
    texture_cache: HashMap<u32, Vec<u8>>,
    lru_queue: VecDeque<u32>,
    next_id: u32,
    max_cache_size_mb: u64,
    current_cache_size_mb: u64,
}

/// Audio resource pool based on C++ Miles Audio patterns
#[derive(Debug)]
struct AudioPool {
    loaded_audio: HashMap<String, AudioHandle>,
    audio_cache: HashMap<u32, Vec<u8>>,
    streaming_sources: HashMap<u32, PathBuf>,
    next_id: u32,
    max_cache_size_mb: u64,
    current_cache_size_mb: u64,
}

/// Model resource pool based on C++ W3D model system
#[derive(Debug)]
struct ModelPool {
    loaded_models: HashMap<String, ModelHandle>,
    model_cache: HashMap<u32, Vec<u8>>,
    lod_models: HashMap<String, Vec<ModelHandle>>,
    next_id: u32,
    max_cache_size_mb: u64,
    current_cache_size_mb: u64,
}

/// Data file pool based on C++ BIG file system patterns
#[derive(Debug)]
struct DataPool {
    loaded_data: HashMap<String, DataHandle>,
    data_cache: HashMap<u32, Vec<u8>>,
    compressed_data: HashMap<u32, Vec<u8>>,
    next_id: u32,
    max_cache_size_mb: u64,
    current_cache_size_mb: u64,
}

impl ResourceManager {
    #[instrument(name = "resource_mgr_new")]
    pub fn new(config: ResourceConfig, event_system: Arc<EventSystem>) -> IntegrationResult<Self> {
        info!("Creating Resource Manager");
        debug!(
            "Resource config: max_texture_cache={}MB, max_audio_cache={}MB",
            config.max_texture_cache_mb, config.max_audio_cache_mb
        );

        let texture_pool = TexturePool {
            loaded_textures: HashMap::new(),
            texture_cache: HashMap::new(),
            lru_queue: VecDeque::new(),
            next_id: 1,
            max_cache_size_mb: config.max_texture_cache_mb,
            current_cache_size_mb: 0,
        };

        let audio_pool = AudioPool {
            loaded_audio: HashMap::new(),
            audio_cache: HashMap::new(),
            streaming_sources: HashMap::new(),
            next_id: 1,
            max_cache_size_mb: config.max_audio_cache_mb,
            current_cache_size_mb: 0,
        };

        let model_pool = ModelPool {
            loaded_models: HashMap::new(),
            model_cache: HashMap::new(),
            lod_models: HashMap::new(),
            next_id: 1,
            max_cache_size_mb: config.max_model_cache_mb,
            current_cache_size_mb: 0,
        };

        let data_pool = DataPool {
            loaded_data: HashMap::new(),
            data_cache: HashMap::new(),
            compressed_data: HashMap::new(),
            next_id: 1,
            max_cache_size_mb: config.max_data_cache_mb,
            current_cache_size_mb: 0,
        };

        Ok(Self {
            config,
            event_system,
            texture_pool,
            audio_pool,
            model_pool,
            data_pool,
            usage: ResourceUsage {
                texture_memory_mb: 0,
                audio_memory_mb: 0,
                cache_memory_mb: 0,
                total_memory_mb: 0,
                loaded_assets: 0,
                cache_hits: 0,
                cache_misses: 0,
            },
            last_usage_frame: None,
            last_cleanup_frame: None,
            last_pressure_check_frame: None,
        })
    }

    #[instrument(name = "resource_mgr_init_pools", skip(self))]
    pub async fn initialize_pools(&mut self) -> IntegrationResult<()> {
        info!("Initializing resource pools");

        // Initialize texture pool based on C++ texture management
        info!(
            "Initializing texture pool with {}MB cache",
            self.texture_pool.max_cache_size_mb
        );
        self.texture_pool.loaded_textures.reserve(1000);
        self.texture_pool.texture_cache.reserve(500);

        // Initialize audio pool based on C++ Miles Audio system
        info!(
            "Initializing audio pool with {}MB cache",
            self.audio_pool.max_cache_size_mb
        );
        self.audio_pool.loaded_audio.reserve(500);
        self.audio_pool.audio_cache.reserve(100);

        // Initialize model pool based on C++ W3D system
        info!(
            "Initializing model pool with {}MB cache",
            self.model_pool.max_cache_size_mb
        );
        self.model_pool.loaded_models.reserve(200);
        self.model_pool.model_cache.reserve(100);
        self.model_pool.lod_models.reserve(200);

        // Initialize data pool based on C++ BIG file system
        info!(
            "Initializing data pool with {}MB cache",
            self.data_pool.max_cache_size_mb
        );
        self.data_pool.loaded_data.reserve(2000);
        self.data_pool.data_cache.reserve(1000);
        self.data_pool.compressed_data.reserve(1000);

        // Send initialization event
        self.event_system
            .send_system_event(SystemEvent::ResourceLoaded {
                asset_id: "resource_pools_initialized".to_string(),
            })
            .await
            .map_err(|e| IntegrationError::EventSystemError {
                message: e.to_string(),
            })?;

        info!("Resource pools initialized successfully");
        Ok(())
    }

    #[instrument(name = "resource_mgr_update", skip(self))]
    pub async fn update(&mut self, timing: &FrameTiming) -> IntegrationResult<()> {
        trace!(
            "Updating Resource Manager, frame: {}, delta: {:.6}",
            timing.frame_number,
            timing.delta_seconds()
        );

        if Self::interval_elapsed(
            self.last_usage_frame,
            timing.frame_number,
            USAGE_SAMPLE_INTERVAL_FRAMES,
        ) {
            self.update_usage_metrics().await?;
            self.last_usage_frame = Some(timing.frame_number);
        }

        if Self::interval_elapsed(
            self.last_cleanup_frame,
            timing.frame_number,
            CLEANUP_INTERVAL_FRAMES,
        ) {
            self.perform_cache_cleanup().await?;
            self.last_cleanup_frame = Some(timing.frame_number);
        }

        if Self::interval_elapsed(
            self.last_pressure_check_frame,
            timing.frame_number,
            PRESSURE_CHECK_INTERVAL_FRAMES,
        ) {
            self.check_memory_pressure().await?;
            self.last_pressure_check_frame = Some(timing.frame_number);
        }

        Ok(())
    }

    #[instrument(name = "resource_mgr_shutdown", skip(self))]
    pub async fn shutdown(&mut self) -> IntegrationResult<()> {
        info!("Shutting down Resource Manager");

        // Cleanup all resource pools based on C++ cleanup patterns

        // Clear texture pool
        info!(
            "Cleaning up texture pool: {} textures, {}MB cache",
            self.texture_pool.loaded_textures.len(),
            self.texture_pool.current_cache_size_mb
        );
        self.texture_pool.loaded_textures.clear();
        self.texture_pool.texture_cache.clear();
        self.texture_pool.lru_queue.clear();

        // Clear audio pool
        info!(
            "Cleaning up audio pool: {} audio files, {}MB cache",
            self.audio_pool.loaded_audio.len(),
            self.audio_pool.current_cache_size_mb
        );
        self.audio_pool.loaded_audio.clear();
        self.audio_pool.audio_cache.clear();
        self.audio_pool.streaming_sources.clear();

        // Clear model pool
        info!(
            "Cleaning up model pool: {} models, {}MB cache",
            self.model_pool.loaded_models.len(),
            self.model_pool.current_cache_size_mb
        );
        self.model_pool.loaded_models.clear();
        self.model_pool.model_cache.clear();
        self.model_pool.lod_models.clear();

        // Clear data pool
        info!(
            "Cleaning up data pool: {} data files, {}MB cache",
            self.data_pool.loaded_data.len(),
            self.data_pool.current_cache_size_mb
        );
        self.data_pool.loaded_data.clear();
        self.data_pool.data_cache.clear();
        self.data_pool.compressed_data.clear();

        // Reset usage metrics
        self.usage = ResourceUsage {
            texture_memory_mb: 0,
            audio_memory_mb: 0,
            cache_memory_mb: 0,
            total_memory_mb: 0,
            loaded_assets: 0,
            cache_hits: 0,
            cache_misses: 0,
        };

        info!("Resource Manager shutdown complete");
        Ok(())
    }

    pub fn get_usage(&self) -> ResourceUsage {
        self.usage.clone()
    }

    #[instrument(name = "resource_mgr_update_config", skip(self))]
    pub async fn update_config(&mut self, config: ResourceConfig) -> IntegrationResult<()> {
        info!("Updating Resource Manager configuration");

        // Update cache limits if they changed
        if config.max_texture_cache_mb != self.config.max_texture_cache_mb {
            info!(
                "Updating texture cache limit: {}MB -> {}MB",
                self.config.max_texture_cache_mb, config.max_texture_cache_mb
            );
            self.texture_pool.max_cache_size_mb = config.max_texture_cache_mb;
        }

        if config.max_audio_cache_mb != self.config.max_audio_cache_mb {
            info!(
                "Updating audio cache limit: {}MB -> {}MB",
                self.config.max_audio_cache_mb, config.max_audio_cache_mb
            );
            self.audio_pool.max_cache_size_mb = config.max_audio_cache_mb;
        }

        self.config = config;

        // Perform immediate cleanup if new limits are smaller
        self.perform_cache_cleanup().await?;

        Ok(())
    }

    // Private implementation methods based on C++ resource management patterns

    async fn update_usage_metrics(&mut self) -> IntegrationResult<()> {
        trace!("Updating resource usage metrics");

        // Update texture memory usage
        self.usage.texture_memory_mb = self.texture_pool.current_cache_size_mb;

        // Update audio memory usage
        self.usage.audio_memory_mb = self.audio_pool.current_cache_size_mb;

        // Update cache memory usage (total)
        self.usage.cache_memory_mb = self.texture_pool.current_cache_size_mb
            + self.audio_pool.current_cache_size_mb
            + self.model_pool.current_cache_size_mb
            + self.data_pool.current_cache_size_mb;

        // Update total memory usage
        self.usage.total_memory_mb = self.usage.cache_memory_mb;

        // Update loaded asset count
        self.usage.loaded_assets = (self.texture_pool.loaded_textures.len()
            + self.audio_pool.loaded_audio.len()
            + self.model_pool.loaded_models.len()
            + self.data_pool.loaded_data.len()) as u64;

        self.event_system.send_system_event_lockfree(
            SystemEvent::ResourceUsageSample {
                usage: self.usage.clone(),
            },
            EventPriority::Low,
        );

        Ok(())
    }

    fn interval_elapsed(last_frame: Option<u64>, current_frame: u64, interval: u64) -> bool {
        match last_frame {
            None => true,
            Some(frame) => current_frame.wrapping_sub(frame) >= interval,
        }
    }

    async fn perform_cache_cleanup(&mut self) -> IntegrationResult<()> {
        trace!("Performing cache cleanup");

        // Cleanup texture cache if over limit
        self.cleanup_texture_cache().await?;

        // Cleanup audio cache if over limit
        self.cleanup_audio_cache().await?;

        // Cleanup model cache if over limit
        self.cleanup_model_cache().await?;

        // Cleanup data cache if over limit
        self.cleanup_data_cache().await?;

        Ok(())
    }

    async fn cleanup_texture_cache(&mut self) -> IntegrationResult<()> {
        if self.texture_pool.current_cache_size_mb <= self.texture_pool.max_cache_size_mb {
            return Ok(());
        }

        debug!(
            "Texture cache over limit: {}MB / {}MB, performing cleanup",
            self.texture_pool.current_cache_size_mb, self.texture_pool.max_cache_size_mb
        );

        // Use LRU eviction based on C++ cache patterns
        let target_size = (self.texture_pool.max_cache_size_mb as f64 * 0.8) as u64;

        while self.texture_pool.current_cache_size_mb > target_size
            && !self.texture_pool.lru_queue.is_empty()
        {
            if let Some(texture_id) = self.texture_pool.lru_queue.pop_front() {
                if let Some(texture_data) = self.texture_pool.texture_cache.remove(&texture_id) {
                    let freed_mb = texture_data.len() as u64 / 1024 / 1024;
                    self.texture_pool.current_cache_size_mb -= freed_mb;
                    debug!("Evicted texture ID {}, freed {}MB", texture_id, freed_mb);
                }
            }
        }

        Ok(())
    }

    async fn cleanup_audio_cache(&mut self) -> IntegrationResult<()> {
        if self.audio_pool.current_cache_size_mb <= self.audio_pool.max_cache_size_mb {
            return Ok(());
        }

        debug!(
            "Audio cache over limit: {}MB / {}MB, performing cleanup",
            self.audio_pool.current_cache_size_mb, self.audio_pool.max_cache_size_mb
        );

        // Simple LRU cleanup for audio cache
        let target_size = (self.audio_pool.max_cache_size_mb as f64 * 0.8) as u64;
        let mut audio_ids_to_remove = Vec::new();

        for (audio_id, _) in &self.audio_pool.audio_cache {
            if self.audio_pool.current_cache_size_mb <= target_size {
                break;
            }
            audio_ids_to_remove.push(*audio_id);
        }

        for audio_id in audio_ids_to_remove {
            if let Some(audio_data) = self.audio_pool.audio_cache.remove(&audio_id) {
                let freed_mb = audio_data.len() as u64 / 1024 / 1024;
                self.audio_pool.current_cache_size_mb -= freed_mb;
                debug!("Evicted audio ID {}, freed {}MB", audio_id, freed_mb);
            }
        }

        Ok(())
    }

    async fn cleanup_model_cache(&mut self) -> IntegrationResult<()> {
        if self.model_pool.current_cache_size_mb <= self.model_pool.max_cache_size_mb {
            return Ok(());
        }

        debug!(
            "Model cache over limit: {}MB / {}MB, performing cleanup",
            self.model_pool.current_cache_size_mb, self.model_pool.max_cache_size_mb
        );

        // Clean up least recently used models
        let target_size = (self.model_pool.max_cache_size_mb as f64 * 0.8) as u64;
        let mut model_ids_to_remove = Vec::new();

        for (model_id, _) in &self.model_pool.model_cache {
            if self.model_pool.current_cache_size_mb <= target_size {
                break;
            }
            model_ids_to_remove.push(*model_id);
        }

        for model_id in model_ids_to_remove {
            if let Some(model_data) = self.model_pool.model_cache.remove(&model_id) {
                let freed_mb = model_data.len() as u64 / 1024 / 1024;
                self.model_pool.current_cache_size_mb -= freed_mb;
                debug!("Evicted model ID {}, freed {}MB", model_id, freed_mb);
            }
        }

        Ok(())
    }

    async fn cleanup_data_cache(&mut self) -> IntegrationResult<()> {
        if self.data_pool.current_cache_size_mb <= self.data_pool.max_cache_size_mb {
            return Ok(());
        }

        debug!(
            "Data cache over limit: {}MB / {}MB, performing cleanup",
            self.data_pool.current_cache_size_mb, self.data_pool.max_cache_size_mb
        );

        // Clean up data cache using LRU strategy
        let target_size = (self.data_pool.max_cache_size_mb as f64 * 0.8) as u64;
        let mut data_ids_to_remove = Vec::new();

        for (data_id, _) in &self.data_pool.data_cache {
            if self.data_pool.current_cache_size_mb <= target_size {
                break;
            }
            data_ids_to_remove.push(*data_id);
        }

        for data_id in data_ids_to_remove {
            if let Some(data) = self.data_pool.data_cache.remove(&data_id) {
                let freed_mb = data.len() as u64 / 1024 / 1024;
                self.data_pool.current_cache_size_mb -= freed_mb;
                debug!("Evicted data ID {}, freed {}MB", data_id, freed_mb);
            }
        }

        Ok(())
    }

    async fn check_memory_pressure(&mut self) -> IntegrationResult<()> {
        let total_usage = self.usage.total_memory_mb;
        let total_limit = self.texture_pool.max_cache_size_mb
            + self.audio_pool.max_cache_size_mb
            + self.model_pool.max_cache_size_mb
            + self.data_pool.max_cache_size_mb;

        let usage_percent = if total_limit > 0 {
            (total_usage as f64 / total_limit as f64) * 100.0
        } else {
            0.0
        };

        if usage_percent > 85.0 {
            warn!(
                "High memory pressure: {:.1}% ({}/{}MB)",
                usage_percent, total_usage, total_limit
            );

            self.event_system
                .send_system_event(SystemEvent::ResourceExhausted {
                    resource_type: "memory".to_string(),
                })
                .await
                .map_err(|e| IntegrationError::EventSystemError {
                    message: e.to_string(),
                })?;
        } else if usage_percent > 75.0 {
            info!(
                "Moderate memory pressure: {:.1}% ({}/{}MB)",
                usage_percent, total_usage, total_limit
            );
        }

        Ok(())
    }

    // Public resource loading methods based on C++ asset loading patterns

    pub async fn load_texture(&mut self, path: &str) -> IntegrationResult<TextureHandle> {
        if let Some(handle) = self.texture_pool.loaded_textures.get(path) {
            self.usage.cache_hits += 1;
            return Ok(handle.clone());
        }

        self.usage.cache_misses += 1;

        // Create new texture handle (placeholder implementation)
        let handle = TextureHandle {
            id: self.texture_pool.next_id,
            path: PathBuf::from(path),
            width: 512, // Would be read from actual texture file
            height: 512,
            format: "DXT1".to_string(),
        };

        self.texture_pool.next_id += 1;
        self.texture_pool
            .loaded_textures
            .insert(path.to_string(), handle.clone());

        info!("Loaded texture: {}", path);

        Ok(handle)
    }

    pub async fn load_audio(&mut self, path: &str) -> IntegrationResult<AudioHandle> {
        if let Some(handle) = self.audio_pool.loaded_audio.get(path) {
            self.usage.cache_hits += 1;
            return Ok(handle.clone());
        }

        self.usage.cache_misses += 1;

        // Create new audio handle (placeholder implementation)
        let handle = AudioHandle {
            id: self.audio_pool.next_id,
            path: PathBuf::from(path),
            duration_ms: 5000, // Would be read from actual audio file
            sample_rate: 44100,
            channels: 2,
        };

        self.audio_pool.next_id += 1;
        self.audio_pool
            .loaded_audio
            .insert(path.to_string(), handle.clone());

        info!("Loaded audio: {}", path);

        Ok(handle)
    }

    pub async fn load_model(&mut self, path: &str) -> IntegrationResult<ModelHandle> {
        if let Some(handle) = self.model_pool.loaded_models.get(path) {
            self.usage.cache_hits += 1;
            return Ok(handle.clone());
        }

        self.usage.cache_misses += 1;

        // Create new model handle (placeholder implementation)
        let handle = ModelHandle {
            id: self.model_pool.next_id,
            path: PathBuf::from(path),
            vertex_count: 1000, // Would be read from actual model file
            triangle_count: 500,
            bone_count: 50,
        };

        self.model_pool.next_id += 1;
        self.model_pool
            .loaded_models
            .insert(path.to_string(), handle.clone());

        info!("Loaded model: {}", path);

        Ok(handle)
    }

    pub async fn load_data(&mut self, path: &str) -> IntegrationResult<DataHandle> {
        if let Some(handle) = self.data_pool.loaded_data.get(path) {
            self.usage.cache_hits += 1;
            return Ok(handle.clone());
        }

        self.usage.cache_misses += 1;

        // Create new data handle (placeholder implementation)
        let handle = DataHandle {
            id: self.data_pool.next_id,
            path: PathBuf::from(path),
            size_bytes: 1024 * 1024, // Would be read from actual data file
            compression: "zlib".to_string(),
        };

        self.data_pool.next_id += 1;
        self.data_pool
            .loaded_data
            .insert(path.to_string(), handle.clone());

        info!("Loaded data: {}", path);

        Ok(handle)
    }
}
