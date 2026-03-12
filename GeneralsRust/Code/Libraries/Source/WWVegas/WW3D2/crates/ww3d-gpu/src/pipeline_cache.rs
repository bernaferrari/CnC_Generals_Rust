//! Pipeline Cache
//!
//! This module provides a caching system for render pipelines to avoid redundant
//! pipeline creation. Pipelines are expensive to create, so caching based on
//! pipeline state is critical for performance.

use crate::{pipeline, GpuError};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Pipeline cache key - uniquely identifies a pipeline configuration
#[derive(Clone, Debug)]
pub struct PipelineCacheKey {
    /// Vertex shader module hash
    pub vertex_shader: u64,
    /// Fragment shader module hash
    pub fragment_shader: Option<u64>,
    /// Vertex buffer layouts
    pub vertex_layouts: Vec<u64>,
    /// Primitive topology
    pub topology: wgpu::PrimitiveTopology,
    /// Polygon mode
    pub polygon_mode: wgpu::PolygonMode,
    /// Cull mode
    pub cull_mode: Option<wgpu::Face>,
    /// Front face winding
    pub front_face: wgpu::FrontFace,
    /// Depth/stencil format
    pub depth_stencil_format: Option<wgpu::TextureFormat>,
    /// Depth compare function
    pub depth_compare: Option<wgpu::CompareFunction>,
    /// Depth write enabled
    pub depth_write_enabled: bool,
    /// Color target formats
    pub color_formats: Vec<wgpu::TextureFormat>,
    /// Blend mode
    pub blend_mode: Option<u64>,
    /// Sample count
    pub sample_count: u32,
}

impl Hash for PipelineCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.vertex_shader.hash(state);
        self.fragment_shader.hash(state);
        self.vertex_layouts.hash(state);
        (self.topology as u32).hash(state);
        (self.polygon_mode as u32).hash(state);
        self.cull_mode.map(|f| f as u32).hash(state);
        (self.front_face as u32).hash(state);
        self.depth_stencil_format
            .map(|f| format!("{:?}", f))
            .hash(state);
        self.depth_compare.map(|c| c as u32).hash(state);
        self.depth_write_enabled.hash(state);
        for format in &self.color_formats {
            format!("{:?}", format).hash(state);
        }
        self.blend_mode.hash(state);
        self.sample_count.hash(state);
    }
}

impl PartialEq for PipelineCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.vertex_shader == other.vertex_shader
            && self.fragment_shader == other.fragment_shader
            && self.vertex_layouts == other.vertex_layouts
            && self.topology == other.topology
            && self.polygon_mode == other.polygon_mode
            && self.cull_mode == other.cull_mode
            && self.front_face == other.front_face
            && self.depth_stencil_format == other.depth_stencil_format
            && self.depth_compare == other.depth_compare
            && self.depth_write_enabled == other.depth_write_enabled
            && self.color_formats == other.color_formats
            && self.blend_mode == other.blend_mode
            && self.sample_count == other.sample_count
    }
}

impl Eq for PipelineCacheKey {}

/// Pipeline cache statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineCacheStats {
    /// Number of cached pipelines
    pub pipeline_count: usize,
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    /// Total memory used (estimate)
    pub memory_used: u64,
}

impl PipelineCacheStats {
    /// Get cache hit rate
    pub fn hit_rate(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total > 0 {
            self.cache_hits as f32 / total as f32
        } else {
            0.0
        }
    }
}

/// Pipeline cache
pub struct PipelineCache {
    /// Cached pipelines
    pipelines: HashMap<PipelineCacheKey, Arc<pipeline::RenderPipeline>>,
    /// Cache statistics
    stats: PipelineCacheStats,
}

impl PipelineCache {
    /// Create a new pipeline cache
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
            stats: PipelineCacheStats::default(),
        }
    }

    /// Get or create a pipeline
    pub fn get_or_create<F>(
        &mut self,
        key: PipelineCacheKey,
        create_fn: F,
    ) -> Result<Arc<pipeline::RenderPipeline>, GpuError>
    where
        F: FnOnce() -> Result<pipeline::RenderPipeline, GpuError>,
    {
        // Check cache first
        if let Some(pipeline) = self.pipelines.get(&key) {
            self.stats.cache_hits += 1;
            return Ok(pipeline.clone());
        }

        // Cache miss - create new pipeline
        self.stats.cache_misses += 1;
        let pipeline = create_fn()?;
        let pipeline_arc = Arc::new(pipeline);

        // Store in cache
        self.pipelines.insert(key, pipeline_arc.clone());
        self.stats.pipeline_count = self.pipelines.len();

        // Update memory estimate (rough approximation)
        self.stats.memory_used = (self.stats.pipeline_count as u64) * 4096; // ~4KB per pipeline

        Ok(pipeline_arc)
    }

    /// Get pipeline if exists in cache
    pub fn get(&mut self, key: &PipelineCacheKey) -> Option<Arc<pipeline::RenderPipeline>> {
        if let Some(pipeline) = self.pipelines.get(key) {
            self.stats.cache_hits += 1;
            Some(pipeline.clone())
        } else {
            self.stats.cache_misses += 1;
            None
        }
    }

    /// Insert pipeline into cache
    pub fn insert(&mut self, key: PipelineCacheKey, pipeline: Arc<pipeline::RenderPipeline>) {
        self.pipelines.insert(key, pipeline);
        self.stats.pipeline_count = self.pipelines.len();
        self.stats.memory_used = (self.stats.pipeline_count as u64) * 4096;
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.pipelines.clear();
        self.stats.pipeline_count = 0;
        self.stats.memory_used = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> PipelineCacheStats {
        self.stats
    }

    /// Get number of cached pipelines
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// Remove least recently used pipelines (if cache grows too large)
    pub fn trim(&mut self, max_pipelines: usize) {
        if self.pipelines.len() > max_pipelines {
            // Simple approach: clear entire cache if too large
            // A more sophisticated LRU implementation could be added
            self.clear();
        }
    }
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

// Global pipeline cache
lazy_static::lazy_static! {
    pub static ref PIPELINE_CACHE: Mutex<PipelineCache> = Mutex::new(PipelineCache::new());
}

/// Get or create a pipeline from global cache
pub fn get_or_create_pipeline<F>(
    key: PipelineCacheKey,
    create_fn: F,
) -> Result<Arc<pipeline::RenderPipeline>, GpuError>
where
    F: FnOnce() -> Result<pipeline::RenderPipeline, GpuError>,
{
    PIPELINE_CACHE.lock().get_or_create(key, create_fn)
}

/// Get pipeline from global cache
pub fn get_cached_pipeline(key: &PipelineCacheKey) -> Option<Arc<pipeline::RenderPipeline>> {
    PIPELINE_CACHE.lock().get(key)
}

/// Insert pipeline into global cache
pub fn cache_pipeline(key: PipelineCacheKey, pipeline: Arc<pipeline::RenderPipeline>) {
    PIPELINE_CACHE.lock().insert(key, pipeline);
}

/// Clear global pipeline cache
pub fn clear_pipeline_cache() {
    PIPELINE_CACHE.lock().clear();
}

/// Get global pipeline cache statistics
pub fn pipeline_cache_stats() -> PipelineCacheStats {
    PIPELINE_CACHE.lock().stats()
}

/// Trim global pipeline cache
pub fn trim_pipeline_cache(max_pipelines: usize) {
    PIPELINE_CACHE.lock().trim(max_pipelines);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_key() -> PipelineCacheKey {
        PipelineCacheKey {
            vertex_shader: 123,
            fragment_shader: Some(456),
            vertex_layouts: vec![789],
            topology: wgpu::PrimitiveTopology::TriangleList,
            polygon_mode: wgpu::PolygonMode::Fill,
            cull_mode: Some(wgpu::Face::Back),
            front_face: wgpu::FrontFace::Ccw,
            depth_stencil_format: Some(wgpu::TextureFormat::Depth24Plus),
            depth_compare: Some(wgpu::CompareFunction::Less),
            depth_write_enabled: true,
            color_formats: vec![wgpu::TextureFormat::Rgba8UnormSrgb],
            blend_mode: None,
            sample_count: 1,
        }
    }

    #[test]
    fn test_cache_key_equality() {
        let key1 = create_test_key();
        let key2 = create_test_key();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_stats() {
        let cache = PipelineCache::new();
        let stats = cache.stats();
        assert_eq!(stats.pipeline_count, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_operations() {
        let mut cache = PipelineCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_trim() {
        let mut cache = PipelineCache::new();
        cache.trim(100); // Should not panic on empty cache
        assert!(cache.is_empty());
    }

    #[test]
    fn test_hit_rate_calculation() {
        let stats = PipelineCacheStats {
            pipeline_count: 10,
            cache_hits: 80,
            cache_misses: 20,
            memory_used: 40960,
        };
        assert_eq!(stats.hit_rate(), 0.8);
    }
}
