//! Texture Loader System - Advanced texture loading and management
//!
//! This module implements the TextureLoader system from the original C++ code,
//! providing comprehensive texture loading, caching, and management capabilities.
//!
//! Converted from:
//! - textureloader.cpp/h (main texture loader)
//! - texfcach.cpp/h (texture file cache)
//! - Texture loading, caching, and management

use crate::core::error::{W3dError, RendererResult as Result};
use crate::core::ww3dformat::{FormatManager, WW3DFormat};
use crate::core::wwstring::StringClass;
use crate::rendering::texture_decode::{decode_texture_file, TextureData, TextureDataKind, TextureMipLevel};
use crate::rendering::texture_quality;
use crate::rendering::texture_system::texture_base::{PoolType, TextureClass, TextureUsagePolicy};
use log::{debug, warn};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use log::warn;

/// Texture loading priority
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureLoadPriority {
    /// Low priority - load when convenient
    Low = 0,
    /// Normal priority - load as needed
    Normal = 1,
    /// High priority - load immediately
    High = 2,
    /// Critical priority - load before anything else
    Critical = 3,
}

/// Texture load request
#[derive(Clone)]
pub struct TextureLoadRequest {
    /// Texture filename
    pub filename: StringClass,
    /// Priority
    pub priority: TextureLoadPriority,
    /// Desired format override
    pub desired_format: Option<WW3DFormat>,
    /// Whether compressed formats may be used
    pub allow_compression: bool,
    /// Whether runtime reduction is allowed
    pub allow_reduction: bool,
    /// Requested final mip count (optional)
    pub requested_mip_levels: Option<u32>,
    /// Whether to generate missing mipmaps
    pub generate_mipmaps: bool,
    /// Whether texture is persistent
    pub persistent: bool,
    /// Callback for when loading completes
    pub callback: Option<Arc<dyn Fn(Arc<TextureClass>) + Send + Sync>>,
}

impl std::fmt::Debug for TextureLoadRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureLoadRequest")
            .field("filename", &self.filename)
            .field("priority", &self.priority)
            .field("desired_format", &self.desired_format)
            .field("allow_compression", &self.allow_compression)
            .field("allow_reduction", &self.allow_reduction)
            .field("requested_mip_levels", &self.requested_mip_levels)
            .field("generate_mipmaps", &self.generate_mipmaps)
            .field("persistent", &self.persistent)
            .field(
                "callback",
                &self.callback.as_ref().map(|_| "Fn(Arc<TextureClass>)"),
            )
            .finish()
    }
}

impl TextureLoadRequest {
    /// Create new load request
    pub fn new(filename: &str) -> Self {
        Self {
            filename: StringClass::from(filename),
            priority: TextureLoadPriority::Normal,
            desired_format: None,
            allow_compression: true,
            allow_reduction: true,
            requested_mip_levels: None,
            generate_mipmaps: true,
            persistent: true,
            callback: None,
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: TextureLoadPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set desired format
    pub fn with_format(mut self, format: WW3DFormat) -> Self {
        self.desired_format = Some(format);
        self
    }

    /// Allow or disallow GPU block-compressed formats
    pub fn with_compression_allowed(mut self, allow: bool) -> Self {
        self.allow_compression = allow;
        self
    }

    /// Allow or disallow runtime reduction
    pub fn with_reduction_allowed(mut self, allow: bool) -> Self {
        self.allow_reduction = allow;
        self
    }

    /// Request a specific mip count
    pub fn with_mip_levels(mut self, levels: u32) -> Self {
        self.requested_mip_levels = Some(levels.max(1));
        self
    }

    /// Set mipmap generation
    pub fn with_mipmaps(mut self, generate: bool) -> Self {
        self.generate_mipmaps = generate;
        if generate {
            if self.requested_mip_levels == Some(1) {
                self.requested_mip_levels = None;
            }
        } else {
            self.requested_mip_levels = Some(1);
        }
        self
    }

    /// Set persistence
    pub fn with_persistence(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }

    /// Set callback
    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(Arc<TextureClass>) + Send + Sync + 'static,
    {
        self.callback = Some(Arc::new(callback));
        self
    }
}

/// Texture cache entry
#[derive(Debug)]
pub struct TextureCacheEntry {
    /// Texture data
    pub texture: Arc<TextureClass>,
    /// Last access time
    pub last_access: std::time::Instant,
    /// Access count
    pub access_count: u64,
    /// Memory usage
    pub memory_usage: usize,
    /// Whether entry is persistent
    pub persistent: bool,
}

impl TextureCacheEntry {
    /// Create new cache entry
    pub fn new(texture: Arc<TextureClass>, persistent: bool) -> Self {
        let memory_usage = texture.get_memory_usage();
        Self {
            texture,
            last_access: std::time::Instant::now(),
            access_count: 0,
            memory_usage,
            persistent,
        }
    }

    /// Access the entry (updates access time and count)
    pub fn access(&mut self) {
        self.last_access = std::time::Instant::now();
        self.access_count += 1;
    }

    /// Check if entry is stale
    pub fn is_stale(&self, max_age: std::time::Duration) -> bool {
        !self.persistent && self.last_access.elapsed() > max_age
    }
}

/// Texture loader class
#[derive(Debug)]
pub struct TextureLoaderClass {
    /// Texture cache
    pub texture_cache: HashMap<String, TextureCacheEntry>,
    /// Loading queue
    pub load_queue: Vec<TextureLoadRequest>,
    /// Currently loading textures
    pub loading_textures: HashMap<String, Arc<TextureLoadRequest>>,
    /// Cache size limit
    pub cache_size_limit: usize,
    /// Current cache size
    pub current_cache_size: usize,
    /// Maximum cache age for non-persistent textures
    pub max_cache_age: std::time::Duration,
    /// Texture search paths
    pub search_paths: Vec<StringClass>,
    /// Format manager
    pub format_manager: Arc<FormatManager>,
    /// Loader ID
    pub loader_id: u32,
}

impl TextureLoaderClass {
    /// Create new texture loader
    pub fn new() -> Self {
        let loader_id = LOADER_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;

        let format_manager = Arc::new(FormatManager::default_cpu());

        Self {
            texture_cache: HashMap::new(),
            load_queue: Vec::new(),
            loading_textures: HashMap::new(),
            cache_size_limit: 100 * 1024 * 1024, // 100MB default
            current_cache_size: 0,
            max_cache_age: std::time::Duration::from_secs(300), // 5 minutes
            search_paths: vec![StringClass::from("./"), StringClass::from("textures/")],
            format_manager,
            loader_id,
        }
    }

    /// Set format manager
    pub fn set_format_manager(&mut self, manager: Arc<FormatManager>) {
        self.format_manager = manager;
    }

    fn take_cached_texture(
        &mut self,
        filename: &str,
        request: Option<&TextureLoadRequest>,
    ) -> Option<Arc<TextureClass>> {
        if let Some(entry) = self.texture_cache.get_mut(filename) {
            if request.map_or(true, |req| Self::texture_meets_request(&entry.texture, req)) {
                entry.access();
                return Some(Arc::clone(&entry.texture));
            }
        }
        if self.texture_cache.contains_key(filename) {
            self.evict_cache_entry(filename);
        }
        None
    }

    fn evict_cache_entry(&mut self, filename: &str) {
        if let Some(entry) = self.texture_cache.remove(filename) {
            self.current_cache_size = self.current_cache_size.saturating_sub(entry.memory_usage);
        }
    }

    fn usage_policy_from_request(request: Option<&TextureLoadRequest>) -> TextureUsagePolicy {
        request
            .map(|req| TextureUsagePolicy::new(req.allow_compression, req.allow_reduction, req.requested_mip_levels))
            .unwrap_or_default()
    }

    fn texture_meets_request(texture: &TextureClass, request: &TextureLoadRequest) -> bool {
        let policy = texture.usage_policy();
        if policy.allow_compression != request.allow_compression {
            return false;
        }
        if policy.allow_reduction != request.allow_reduction {
            return false;
        }
        if policy.requested_mip_levels != request.requested_mip_levels {
            return false;
        }
        if !request.generate_mipmaps && texture.mip_level_count() > 1 {
            return false;
        }
        if let Some(target) = request.requested_mip_levels {
            if texture.mip_level_count() != target {
                return false;
            }
        }
        if !request.allow_compression {
            if let Some(decision) = texture.format_history() {
                if decision.format.is_block_compressed() {
                    return false;
                }
            }
        }
        true
    }

    /// Load texture synchronously
    pub fn load_texture(&mut self, filename: &str) -> Result<Arc<TextureClass>> {
        self.load_texture_with_request(filename, None)
    }

    pub fn load_texture_with_request(
        &mut self,
        filename: &str,
        request: Option<&TextureLoadRequest>,
    ) -> Result<Arc<TextureClass>> {
        if let Some(texture) = self.take_cached_texture(filename, request) {
            return Ok(texture);
        }

        let file_path = self.find_texture_file(filename)?;
        let texture_data = self.load_texture_data(&file_path, request)?;

        let mut texture = TextureClass::new(
            texture_data.width,
            texture_data.height,
            texture_data.mip_levels,
            PoolType::Managed,
        );
        texture.set_name(filename);
        texture.set_full_path(&file_path);
        texture.apply_texture_data(&texture_data);

        if let Some(req) = request {
            if let Some(target) = req.requested_mip_levels {
                if texture_data.mip_levels != target {
                    warn!(
                        "Texture {} truncated to {} mip levels instead of requested {}",
                        filename,
                        texture_data.mip_levels,
                        target
                    );
                }
            }
        }

        let policy = Self::usage_policy_from_request(request);
        texture.set_usage_policy(policy);

        if let Some(decision) = texture.format_history() {
            debug!(
                "Texture {} resolved {:?} -> {:?} (decompress={})",
                filename,
                decision.source_format,
                decision.format,
                decision.requires_decompression
            );
        }

        let texture = Arc::new(texture);
        let persistent = request.map_or(true, |req| req.persistent);
        self.add_to_cache(filename, Arc::clone(&texture), persistent);
        Ok(texture)
    }

    /// Load texture asynchronously
    pub fn load_texture_async(&mut self, request: TextureLoadRequest) -> Result<()> {
        let filename = request.filename.as_str();

        if let Some(texture) = self.take_cached_texture(filename, Some(&request)) {
            if let Some(callback) = request.callback.as_ref() {
                callback(Arc::clone(&texture));
            }
            return Ok(());
        }

        // Check if already loading
        if self.loading_textures.contains_key(filename) {
            return Ok(());
        }

        // Add to loading queue
        let arc_request = Arc::new(request.clone());
        self.loading_textures
            .insert(filename.to_string(), arc_request);
        self.load_queue.push(request);

        Ok(())
    }

    /// Update loader (process loading queue)
    pub fn update(&mut self) -> Result<()> {
        // Clean up stale cache entries
        self.cleanup_cache();

        let mut completed = Vec::new();
        let mut remaining = Vec::new();

        for request in self.load_queue.drain(..) {
            let filename = request.filename.to_string();
            match self.load_texture_with_request(&filename, Some(&request)) {
                Ok(texture) => completed.push((request, texture)),
                Err(_) => remaining.push(request),
            }
        }

        self.load_queue = remaining;

        for (request, texture) in completed {
            let filename = request.filename.to_string();
            if let Some(loading_req) = self.loading_textures.remove(&filename) {
                if let Some(callback) = &loading_req.callback {
                    callback(texture);
                }
            } else if let Some(callback) = request.callback {
                callback(texture);
            }
        }

        Ok(())
    }

    /// Get cached texture
    pub fn get_cached_texture(&mut self, filename: &str) -> Option<Arc<TextureClass>> {
        if let Some(entry) = self.texture_cache.get_mut(filename) {
            entry.access();
            Some(Arc::clone(&entry.texture))
        } else {
            None
        }
    }

    /// Check if texture is cached
    pub fn is_texture_cached(&self, filename: &str) -> bool {
        self.texture_cache.contains_key(filename)
    }

    /// Check if texture is loading
    pub fn is_texture_loading(&self, filename: &str) -> bool {
        self.loading_textures.contains_key(filename)
    }

    /// Preload texture
    pub fn preload_texture(&mut self, filename: &str) -> Result<()> {
        if !self.is_texture_cached(filename) && !self.is_texture_loading(filename) {
            let request = TextureLoadRequest::new(filename).with_priority(TextureLoadPriority::Low);
            self.load_texture_async(request)?;
        }
        Ok(())
    }

    /// Unload texture
    pub fn unload_texture(&mut self, filename: &str) -> Result<()> {
        if let Some(entry) = self.texture_cache.remove(filename) {
            self.current_cache_size -= entry.memory_usage;
        }
        Ok(())
    }

    /// Clear cache
    pub fn clear_cache(&mut self) {
        self.texture_cache.clear();
        self.current_cache_size = 0;
    }

    /// Set cache size limit
    pub fn set_cache_size_limit(&mut self, limit: usize) {
        self.cache_size_limit = limit;
        self.enforce_cache_limit();
    }

    /// Set max cache age
    pub fn set_max_cache_age(&mut self, age: std::time::Duration) {
        self.max_cache_age = age;
    }

    /// Add search path
    pub fn add_search_path(&mut self, path: &str) {
        self.search_paths.push(StringClass::from(path));
    }

    /// Get cache statistics
    pub fn get_cache_statistics(&self) -> TextureCacheStatistics {
        let total_textures = self.texture_cache.len();
        let persistent_textures = self
            .texture_cache
            .values()
            .filter(|entry| entry.persistent)
            .count();
        let loading_textures = self.loading_textures.len();

        TextureCacheStatistics {
            total_textures,
            persistent_textures,
            loading_textures,
            cache_size_bytes: self.current_cache_size,
            cache_size_mb: self.current_cache_size as f32 / (1024.0 * 1024.0),
            cache_hit_ratio: 0.0, // Would need to track hits/misses
        }
    }

    /// Find texture file
    fn find_texture_file(&self, filename: &str) -> Result<String> {
        // Check if file exists as-is
        if Path::new(filename).exists() {
            return Ok(filename.to_string());
        }

        // Check search paths
        for search_path in &self.search_paths {
            let full_path = format!("{}{}", search_path.as_str(), filename);
            if Path::new(&full_path).exists() {
                return Ok(full_path);
            }
        }

        Err(W3dError::InvalidParameter(format!(
            "Texture file not found: {}",
            filename
        )))
    }

    /// Load texture data from file
    fn load_texture_data(
        &self,
        filename: &str,
        request: Option<&TextureLoadRequest>,
    ) -> Result<TextureData> {
        let mut data = decode_texture_file(filename)?;
        if data.kind != TextureDataKind::Texture2D {
            return Err(W3dError::InvalidParameter(format!(
                "Unsupported texture kind for {}",
                filename
            )));
        }

        let desired_override = request.and_then(|req| req.desired_format);
        let allow_compression = request.map_or(true, |req| req.allow_compression);
        let decision = self
            .format_manager
            .decide(data.format, desired_override, allow_compression);
        data = data.convert_to_format(&decision)?;

        if let Some(req) = request {
            if req.generate_mipmaps {
                if let Some(target) = req.requested_mip_levels {
                    if target > data.mip_levels {
                        if let Err(err) = data.ensure_mip_levels(target) {
                            warn!(
                                "Failed to generate mipmaps for {}: {}",
                                filename, err
                            );
                        }
                    }
                } else {
                    let desired = data.max_possible_mip_levels();
                    if desired > data.mip_levels {
                        if let Err(err) = data.ensure_mip_levels(desired) {
                            warn!(
                                "Failed to generate full mip chain for {}: {}",
                                filename, err
                            );
                        }
                    }
                }
            }
        }

        let allow_reduction = request.map_or(true, |req| req.allow_reduction);
        if allow_reduction {
            let mut reduction = texture_quality::compute_effective_reduction(
                data.width,
                data.height,
                data.mip_levels,
            );

            if let Some(req) = request {
                if let Some(target) = req.requested_mip_levels {
                    let min_target = target.max(1);
                    if min_target <= data.mip_levels {
                        let max_drop = data.mip_levels.saturating_sub(min_target);
                        reduction = reduction.min(max_drop);
                    } else if req.generate_mipmaps {
                        if let Err(err) = data.ensure_mip_levels(min_target) {
                            warn!(
                                "Failed to backfill mip levels for {}: {}",
                                filename, err
                            );
                        }
                        let max_drop = data.mip_levels.saturating_sub(min_target);
                        reduction = reduction.min(max_drop);
                    } else {
                        reduction = 0;
                    }
                }
            }

            if reduction > 0 {
                data = data.drop_mip_levels(reduction);
            }
        }

        if let Some(req) = request {
            if let Some(target) = req.requested_mip_levels {
                data = data.truncate_to_mip_count(target.max(1));
            } else if !req.generate_mipmaps {
                data = data.truncate_to_mip_count(1);
            }
        }

        Ok(data)
    }

    /// Add texture to cache
    fn add_to_cache(&mut self, filename: &str, texture: Arc<TextureClass>, persistent: bool) {
        let entry = TextureCacheEntry::new(Arc::clone(&texture), persistent);
        self.current_cache_size += entry.memory_usage;

        self.texture_cache.insert(filename.to_string(), entry);
        self.enforce_cache_limit();
    }

    /// Enforce cache size limit
    fn enforce_cache_limit(&mut self) {
        if self.current_cache_size <= self.cache_size_limit {
            return;
        }

        // Remove least recently used non-persistent textures
        let mut entries: Vec<(String, TextureCacheEntry)> = self.texture_cache.drain().collect();

        // Sort by access time (oldest first)
        entries.sort_by(|a, b| a.1.last_access.cmp(&b.1.last_access));

        self.texture_cache.clear();
        self.current_cache_size = 0;

        for (filename, entry) in entries {
            if self.current_cache_size + entry.memory_usage <= self.cache_size_limit {
                self.current_cache_size += entry.memory_usage;
                self.texture_cache.insert(filename, entry);
            } else {
                // Stop adding when we would exceed the limit
                break;
            }
        }
    }

    /// Clean up stale cache entries
    fn cleanup_cache(&mut self) {
        let mut to_remove = Vec::new();

        for (filename, entry) in &self.texture_cache {
            if entry.is_stale(self.max_cache_age) {
                to_remove.push(filename.clone());
            }
        }

        for filename in to_remove {
            if let Some(entry) = self.texture_cache.remove(&filename) {
                self.current_cache_size -= entry.memory_usage;
            }
        }
    }
}

/// Texture cache statistics
#[derive(Debug, Clone)]
pub struct TextureCacheStatistics {
    /// Total number of cached textures
    pub total_textures: usize,
    /// Number of persistent textures
    pub persistent_textures: usize,
    /// Number of textures currently loading
    pub loading_textures: usize,
    /// Cache size in bytes
    pub cache_size_bytes: usize,
    /// Cache size in MB
    pub cache_size_mb: f32,
    /// Cache hit ratio
    pub cache_hit_ratio: f32,
}

/// Texture file cache system
#[derive(Debug)]
pub struct TextureFileCache {
    /// Cache directory
    pub cache_directory: StringClass,
    /// Maximum cache size
    pub max_cache_size: usize,
    /// Current cache size
    pub current_cache_size: usize,
    /// Cached files
    pub cached_files: HashMap<String, CachedFileInfo>,
}

impl TextureFileCache {
    /// Create new file cache
    pub fn new(cache_directory: &str) -> Self {
        Self {
            cache_directory: StringClass::from(cache_directory),
            max_cache_size: 500 * 1024 * 1024, // 500MB
            current_cache_size: 0,
            cached_files: HashMap::new(),
        }
    }

    /// Cache texture file
    pub fn cache_file(&mut self, filename: &str, data: &[u8]) -> Result<()> {
        let file_size = data.len();

        // Check if we have space
        if self.current_cache_size + file_size > self.max_cache_size {
            self.cleanup_old_files(file_size)?;
        }

        // Write to cache
        let cache_path = format!("{}/{}", self.cache_directory.as_str(), filename);
        std::fs::write(&cache_path, data)?;

        let info = CachedFileInfo {
            filename: filename.to_string(),
            size: file_size,
            last_access: std::time::Instant::now(),
            access_count: 1,
        };

        self.cached_files.insert(filename.to_string(), info);
        self.current_cache_size += file_size;

        Ok(())
    }

    /// Get cached file
    pub fn get_cached_file(&mut self, filename: &str) -> Option<Vec<u8>> {
        if let Some(info) = self.cached_files.get_mut(filename) {
            let cache_path = format!("{}/{}", self.cache_directory.as_str(), filename);

            if let Ok(data) = std::fs::read(&cache_path) {
                info.last_access = std::time::Instant::now();
                info.access_count += 1;
                return Some(data);
            } else {
                // File corrupted or missing, remove from cache
                self.cached_files.remove(filename);
            }
        }
        None
    }

    /// Check if file is cached
    pub fn is_file_cached(&self, filename: &str) -> bool {
        self.cached_files.contains_key(filename)
    }

    /// Cleanup old files to make space
    fn cleanup_old_files(&mut self, required_space: usize) -> Result<()> {
        let mut files: Vec<(String, CachedFileInfo)> = self.cached_files.drain().collect();

        // Sort by access time (oldest first)
        files.sort_by(|a, b| a.1.last_access.cmp(&b.1.last_access));

        let mut freed_space = 0;
        let mut kept_files = Vec::new();

        for (filename, info) in files {
            if freed_space >= required_space {
                kept_files.push((filename, info));
            } else {
                // Remove file from disk
                let cache_path = format!("{}/{}", self.cache_directory.as_str(), filename);
                let _ = std::fs::remove_file(&cache_path);

                freed_space += info.size;
                self.current_cache_size -= info.size;
            }
        }

        // Put back files we kept
        for (filename, info) in kept_files {
            self.cached_files.insert(filename, info);
        }

        Ok(())
    }
}

/// Cached file information
#[derive(Debug, Clone)]
pub struct CachedFileInfo {
    /// Filename
    pub filename: String,
    /// File size
    pub size: usize,
    /// Last access time
    pub last_access: std::time::Instant,
    /// Access count
    pub access_count: u64,
}

static LOADER_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

fn texture_loader_slot() -> &'static Mutex<Option<TextureLoaderClass>> {
    static SLOT: OnceLock<Mutex<Option<TextureLoaderClass>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn lock_texture_loader_slot() -> MutexGuard<'static, Option<TextureLoaderClass>> {
    match texture_loader_slot().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Scoped handle for the global texture loader.
pub struct TextureLoaderHandle<'a> {
    guard: MutexGuard<'a, Option<TextureLoaderClass>>,
}

impl<'a> Deref for TextureLoaderHandle<'a> {
    type Target = TextureLoaderClass;

    fn deref(&self) -> &Self::Target {
        self.guard
            .as_ref()
            .expect("texture loader must be initialized before use")
    }
}

impl<'a> DerefMut for TextureLoaderHandle<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard
            .as_mut()
            .expect("texture loader must be initialized before use")
    }
}

/// Initialize global texture loader
pub fn init_texture_loader() -> Result<()> {
    let mut guard = lock_texture_loader_slot();
    *guard = Some(TextureLoaderClass::new());
    Ok(())
}

/// Get global texture loader
pub fn get_texture_loader() -> Option<TextureLoaderHandle<'static>> {
    let guard = lock_texture_loader_slot();
    if guard.is_none() {
        None
    } else {
        Some(TextureLoaderHandle { guard })
    }
}

/// Shutdown global texture loader
pub fn shutdown_texture_loader() {
    let mut guard = lock_texture_loader_slot();
    *guard = None;
}

/// Quick texture loading functions
pub fn load_texture(filename: &str) -> Result<Arc<TextureClass>> {
    if let Some(mut loader) = get_texture_loader() {
        loader.load_texture(filename)
    } else {
        Err(W3dError::NotInitialized(
            "Texture loader not initialized".to_string(),
        ))
    }
}

pub fn load_texture_async(request: TextureLoadRequest) -> Result<()> {
    if let Some(mut loader) = get_texture_loader() {
        loader.load_texture_async(request)
    } else {
        Err(W3dError::NotInitialized(
            "Texture loader not initialized".to_string(),
        ))
    }
}

pub fn get_cached_texture(filename: &str) -> Option<Arc<TextureClass>> {
    get_texture_loader().and_then(|mut loader| loader.get_cached_texture(filename))
}

pub fn update_texture_loader() -> Result<()> {
    if let Some(mut loader) = get_texture_loader() {
        loader.update()
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_texture_load_request() {
        let request = TextureLoadRequest::new("test.tga")
            .with_priority(TextureLoadPriority::High)
            .with_format(WW3DFormat::R8G8B8A8)
            .with_mipmaps(true)
            .with_persistence(true);

        assert_eq!(request.filename.as_str(), "test.tga");
        assert_eq!(request.priority, TextureLoadPriority::High);
        assert_eq!(request.desired_format, Some(WW3DFormat::R8G8B8A8));
        assert!(request.generate_mipmaps);
        assert!(request.persistent);
    }

    #[test]
    fn test_texture_cache_entry() {
        let texture = Arc::new(crate::texture_system::TextureClass::new(
            "loader_texture",
            1,
            1,
        ));
        let mut entry = TextureCacheEntry::new(Arc::clone(&texture), true);

        assert!(entry.persistent);
        assert_eq!(entry.access_count, 0);

        entry.access();
        assert_eq!(entry.access_count, 1);

        // Test staleness (should not be stale if persistent)
        assert!(!entry.is_stale(std::time::Duration::from_secs(3600)));
    }

    #[test]
    fn test_texture_loader_creation() {
        let loader = TextureLoaderClass::new();
        assert_eq!(loader.cache_size_limit, 100 * 1024 * 1024); // 100MB
        assert_eq!(loader.current_cache_size, 0);
        assert!(loader.texture_cache.is_empty());
        assert!(loader.load_queue.is_empty());
    }

    #[test]
    fn test_texture_loader_cache() {
        let mut loader = TextureLoaderClass::new();

        // Create a mock texture
        let texture = Arc::new(crate::texture_system::TextureClass::new(
            "loader_async_texture",
            1,
            1,
        ));

        // Add to cache manually
        loader.add_to_cache("test.tga", Arc::clone(&texture), true);

        assert!(loader.is_texture_cached("test.tga"));
        assert_eq!(loader.texture_cache.len(), 1);

        // Get from cache
        let cached = loader.get_cached_texture("test.tga");
        assert!(cached.is_some());

        // Remove from cache
        loader.unload_texture("test.tga").unwrap();
        assert!(!loader.is_texture_cached("test.tga"));
    }

    #[test]
    fn test_texture_data() {
        let mip_size = 256 * 256 * 4;
        let data = TextureData {
            width: 256,
            height: 256,
            depth: 1,
            mip_levels: 1,
            format: WW3DFormat::A8R8G8B8,
            kind: TextureDataKind::Texture2D,
            data: vec![0; mip_size],
            mip_layout: vec![TextureMipLevel {
                offset: 0,
                size: mip_size,
                width: 256,
                height: 256,
            }],
        };

        assert_eq!(data.data_size(), mip_size);
        assert!(!data.is_compressed());
    }

    #[test]
    fn test_texture_file_cache() {
        let mut cache = TextureFileCache::new("./test_cache");

        // This test would need actual file I/O
        // For now, just test basic functionality
        assert!(!cache.is_file_cached("test.tga"));
        assert_eq!(cache.current_cache_size, 0);
    }

    #[test]
    fn test_cache_statistics() {
        let loader = TextureLoaderClass::new();
        let stats = loader.get_cache_statistics();

        assert_eq!(stats.total_textures, 0);
        assert_eq!(stats.persistent_textures, 0);
        assert_eq!(stats.loading_textures, 0);
        assert_eq!(stats.cache_size_bytes, 0);
        assert_eq!(stats.cache_size_mb, 0.0);
    }

    #[test]
    fn test_texture_loader_search_paths() {
        let mut loader = TextureLoaderClass::new();

        assert!(loader.search_paths.len() >= 1); // Should have at least "./"

        loader.add_search_path("assets/textures/");
        assert!(loader.search_paths.len() >= 2);
    }
}
