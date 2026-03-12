//! WGPU Pipeline Integration for N-Patch Tessellation
//!
//! This module provides integration between N-Patch tessellation and WGPU rendering pipelines.
//! Since modern WGPU doesn't have the same tessellation shader support as DirectX 8's
//! D3DRS_PATCHSEGMENTS, we use CPU-based subdivision as the primary approach.
//!
//! ## Integration Strategy
//!
//! 1. **CPU Subdivision (Default)**: Pre-compute subdivided meshes on CPU
//!    - Always available, works on all platforms
//!    - Good performance for static meshes with caching
//!    - Memory overhead: ~O(level^2) per mesh
//!
//! 2. **GPU Tessellation (Future)**: Use compute shaders for subdivision
//!    - Not yet implemented, but architecture allows for it
//!    - Would use WGPU compute pipeline
//!    - Better for dynamic meshes
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use ww3d_scene::npatch::{NPatchTessellator, TessellationLevel};
//! use ww3d_scene::npatch_pipeline::NPatchPipeline;
//!
//! // Create pipeline with medium tessellation
//! let pipeline = NPatchPipeline::new(TessellationLevel::MEDIUM);
//!
//! // Process mesh for rendering
//! // let subdivided = pipeline.process_mesh(&original_mesh);
//! ```

use crate::npatch::{
    NPatchConfig, NPatchTessellator, NPatchVertex, SubdividedMesh, TessellationLevel,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Cache key for subdivided meshes
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MeshCacheKey {
    mesh_id: u64,
    level: u32,
}

/// N-Patch rendering pipeline for WGPU
///
/// Manages N-Patch tessellation for rendering, including:
/// - CPU-based mesh subdivision
/// - Caching of subdivided meshes
/// - Integration with shader system
pub struct NPatchPipeline {
    config: NPatchConfig,
    tessellator: NPatchTessellator,
    cache: Arc<RwLock<HashMap<MeshCacheKey, Arc<SubdividedMesh>>>>,
    stats: Arc<RwLock<PipelineStats>>,
}

impl NPatchPipeline {
    /// Create a new N-Patch pipeline with default configuration
    pub fn new(level: TessellationLevel) -> Self {
        let config = NPatchConfig::with_level(level);
        let tessellator = NPatchTessellator::new(level);

        Self {
            config,
            tessellator,
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PipelineStats::default())),
        }
    }

    /// Create a disabled N-Patch pipeline (passthrough mode)
    pub fn disabled() -> Self {
        let config = NPatchConfig::disabled();
        let tessellator = NPatchTessellator::new(TessellationLevel::NONE);

        Self {
            config,
            tessellator,
            cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PipelineStats::default())),
        }
    }

    /// Process a single triangle with N-Patch subdivision
    ///
    /// If caching is enabled, this will check the cache first.
    pub fn process_triangle(
        &self,
        mesh_id: u64,
        v0: &NPatchVertex,
        v1: &NPatchVertex,
        v2: &NPatchVertex,
    ) -> Arc<SubdividedMesh> {
        if !self.config.enabled {
            // Passthrough mode - return original triangle
            return Arc::new(SubdividedMesh {
                vertices: vec![*v0, *v1, *v2],
                indices: vec![0, 1, 2],
            });
        }

        let key = MeshCacheKey {
            mesh_id,
            level: self.config.level.as_raw(),
        };

        // Try to get from cache
        if self.config.cache_subdivisions {
            if let Ok(cache) = self.cache.read() {
                if let Some(cached) = cache.get(&key) {
                    self.record_cache_hit();
                    return cached.clone();
                }
            }
        }

        // Cache miss - subdivide
        self.record_cache_miss();
        let subdivided = self.tessellator.subdivide_triangle(v0, v1, v2);
        let result = Arc::new(subdivided);

        // Store in cache
        if self.config.cache_subdivisions {
            if let Ok(mut cache) = self.cache.write() {
                cache.insert(key, result.clone());
            }
        }

        result
    }

    /// Process an entire mesh with N-Patch subdivision
    pub fn process_mesh(
        &self,
        mesh_id: u64,
        triangles: &[(NPatchVertex, NPatchVertex, NPatchVertex)],
    ) -> Arc<SubdividedMesh> {
        if !self.config.enabled {
            // Passthrough mode - flatten original triangles
            let mut vertices = Vec::with_capacity(triangles.len() * 3);
            let mut indices = Vec::with_capacity(triangles.len() * 3);

            for (i, (v0, v1, v2)) in triangles.iter().enumerate() {
                let base_idx = (i * 3) as u32;
                vertices.push(*v0);
                vertices.push(*v1);
                vertices.push(*v2);
                indices.push(base_idx);
                indices.push(base_idx + 1);
                indices.push(base_idx + 2);
            }

            return Arc::new(SubdividedMesh { vertices, indices });
        }

        let key = MeshCacheKey {
            mesh_id,
            level: self.config.level.as_raw(),
        };

        // Try to get from cache
        if self.config.cache_subdivisions {
            if let Ok(cache) = self.cache.read() {
                if let Some(cached) = cache.get(&key) {
                    self.record_cache_hit();
                    return cached.clone();
                }
            }
        }

        // Cache miss - subdivide
        self.record_cache_miss();
        let subdivided = self.tessellator.subdivide_mesh(triangles);
        let result = Arc::new(subdivided);

        // Store in cache
        if self.config.cache_subdivisions {
            if let Ok(mut cache) = self.cache.write() {
                cache.insert(key, result.clone());
            }
        }

        result
    }

    /// Set tessellation level
    pub fn set_level(&mut self, level: TessellationLevel) {
        self.config.level = level;
        self.tessellator.set_level(level);

        // Clear cache when level changes
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }

    /// Enable or disable N-Patch tessellation
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// Check if N-Patch is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get current tessellation level
    pub fn level(&self) -> TessellationLevel {
        self.config.level
    }

    /// Clear the subdivision cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
        if let Ok(mut stats) = self.stats.write() {
            *stats = PipelineStats::default();
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> PipelineStats {
        if let Ok(stats) = self.stats.read() {
            *stats
        } else {
            PipelineStats::default()
        }
    }

    /// Get cache size (number of cached meshes)
    pub fn cache_size(&self) -> usize {
        if let Ok(cache) = self.cache.read() {
            cache.len()
        } else {
            0
        }
    }

    /// Get estimated cache memory usage in bytes
    pub fn cache_memory_usage(&self) -> usize {
        if let Ok(cache) = self.cache.read() {
            cache.values().map(|mesh| mesh.memory_size()).sum()
        } else {
            0
        }
    }

    fn record_cache_hit(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.cache_hits += 1;
        }
    }

    fn record_cache_miss(&self) {
        if let Ok(mut stats) = self.stats.write() {
            stats.cache_misses += 1;
        }
    }
}

/// Statistics for N-Patch pipeline performance
#[derive(Debug, Clone, Copy, Default)]
pub struct PipelineStats {
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl PipelineStats {
    /// Calculate cache hit rate (0.0 - 1.0)
    pub fn hit_rate(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f32 / total as f32
        }
    }

    /// Get total number of cache accesses
    pub fn total_accesses(&self) -> u64 {
        self.cache_hits + self.cache_misses
    }
}

/// Shader integration for N-Patch
///
/// Provides helpers for integrating N-Patch with the WW3D shader system.
pub struct NPatchShaderIntegration;

impl NPatchShaderIntegration {
    /// Get the WGSL shader code for N-Patch vertex processing
    ///
    /// This generates vertex shader code that handles tessellated vertices.
    /// The actual tessellation happens on CPU, but the shader needs to handle
    /// the increased vertex count.
    pub fn get_vertex_shader_extensions() -> &'static str {
        r#"
// N-Patch tessellation vertex shader extensions
// Vertices are pre-tessellated on CPU, this just passes them through
// with proper normal and UV interpolation

struct NPatchVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

struct NPatchVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}
"#
    }

    /// Check if a shader should use N-Patch tessellation
    ///
    /// This checks the shader's NPatchEnable flag from the shader bits.
    pub fn should_use_npatch(shader_bits: u32) -> bool {
        const NPATCH_SHIFT: u32 = 17;
        const NPATCH_MASK: u32 = 1 << NPATCH_SHIFT;
        (shader_bits & NPATCH_MASK) != 0
    }

    /// Get recommended tessellation level based on mesh properties
    ///
    /// This is a heuristic that suggests tessellation level based on:
    /// - Mesh triangle count
    /// - Mesh bounding box size
    /// - Target quality level
    pub fn recommend_level(
        triangle_count: usize,
        _bounding_box_size: f32,
        quality: QualityLevel,
    ) -> TessellationLevel {
        // For small meshes, use higher tessellation
        // For large meshes, use lower tessellation to avoid explosion
        match quality {
            QualityLevel::Low => {
                if triangle_count < 100 {
                    TessellationLevel::LOW
                } else {
                    TessellationLevel::NONE
                }
            }
            QualityLevel::Medium => {
                if triangle_count < 50 {
                    TessellationLevel::MEDIUM
                } else if triangle_count < 200 {
                    TessellationLevel::LOW
                } else {
                    TessellationLevel::NONE
                }
            }
            QualityLevel::High => {
                if triangle_count < 30 {
                    TessellationLevel::HIGH
                } else if triangle_count < 100 {
                    TessellationLevel::MEDIUM
                } else if triangle_count < 300 {
                    TessellationLevel::LOW
                } else {
                    TessellationLevel::NONE
                }
            }
            QualityLevel::VeryHigh => {
                if triangle_count < 20 {
                    TessellationLevel::VERY_HIGH
                } else if triangle_count < 50 {
                    TessellationLevel::HIGH
                } else if triangle_count < 150 {
                    TessellationLevel::MEDIUM
                } else if triangle_count < 400 {
                    TessellationLevel::LOW
                } else {
                    TessellationLevel::NONE
                }
            }
        }
    }
}

/// Quality level for automatic tessellation level selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Vec2, Vec3};

    fn create_test_vertex(pos: Vec3) -> NPatchVertex {
        NPatchVertex::new(pos, Vec3::Z, Vec2::ZERO)
    }

    #[test]
    fn test_pipeline_disabled() {
        let pipeline = NPatchPipeline::disabled();
        assert!(!pipeline.is_enabled());

        let v0 = create_test_vertex(Vec3::ZERO);
        let v1 = create_test_vertex(Vec3::X);
        let v2 = create_test_vertex(Vec3::Y);

        let result = pipeline.process_triangle(1, &v0, &v1, &v2);
        assert_eq!(result.vertices.len(), 3);
        assert_eq!(result.triangle_count(), 1);
    }

    #[test]
    fn test_pipeline_enabled() {
        let pipeline = NPatchPipeline::new(TessellationLevel::MEDIUM);
        assert!(pipeline.is_enabled());

        let v0 = create_test_vertex(Vec3::ZERO);
        let v1 = create_test_vertex(Vec3::X);
        let v2 = create_test_vertex(Vec3::Y);

        let result = pipeline.process_triangle(1, &v0, &v1, &v2);
        assert_eq!(result.vertices.len(), 10);
        assert_eq!(result.triangle_count(), 9);
    }

    #[test]
    fn test_cache_hit() {
        let pipeline = NPatchPipeline::new(TessellationLevel::LOW);

        let v0 = create_test_vertex(Vec3::ZERO);
        let v1 = create_test_vertex(Vec3::X);
        let v2 = create_test_vertex(Vec3::Y);

        // First access - cache miss
        let _result1 = pipeline.process_triangle(1, &v0, &v1, &v2);
        let stats1 = pipeline.get_stats();
        assert_eq!(stats1.cache_misses, 1);
        assert_eq!(stats1.cache_hits, 0);

        // Second access - cache hit
        let _result2 = pipeline.process_triangle(1, &v0, &v1, &v2);
        let stats2 = pipeline.get_stats();
        assert_eq!(stats2.cache_misses, 1);
        assert_eq!(stats2.cache_hits, 1);
    }

    #[test]
    fn test_cache_clear() {
        let pipeline = NPatchPipeline::new(TessellationLevel::LOW);

        let v0 = create_test_vertex(Vec3::ZERO);
        let v1 = create_test_vertex(Vec3::X);
        let v2 = create_test_vertex(Vec3::Y);

        // Add to cache
        let _result = pipeline.process_triangle(1, &v0, &v1, &v2);
        assert_eq!(pipeline.cache_size(), 1);

        // Clear cache
        pipeline.clear_cache();
        assert_eq!(pipeline.cache_size(), 0);
        assert_eq!(pipeline.get_stats().total_accesses(), 0);
    }

    #[test]
    fn test_level_change_clears_cache() {
        let mut pipeline = NPatchPipeline::new(TessellationLevel::LOW);

        let v0 = create_test_vertex(Vec3::ZERO);
        let v1 = create_test_vertex(Vec3::X);
        let v2 = create_test_vertex(Vec3::Y);

        // Add to cache at LOW level
        let _result = pipeline.process_triangle(1, &v0, &v1, &v2);
        assert_eq!(pipeline.cache_size(), 1);

        // Change level
        pipeline.set_level(TessellationLevel::MEDIUM);
        assert_eq!(pipeline.cache_size(), 0);
    }

    #[test]
    fn test_mesh_processing() {
        let pipeline = NPatchPipeline::new(TessellationLevel::LOW);

        let tri1 = (
            create_test_vertex(Vec3::ZERO),
            create_test_vertex(Vec3::X),
            create_test_vertex(Vec3::Y),
        );
        let tri2 = (
            create_test_vertex(Vec3::X),
            create_test_vertex(Vec3::Y),
            create_test_vertex(Vec3::Z),
        );

        let result = pipeline.process_mesh(1, &[tri1, tri2]);

        // Two triangles at level LOW (2) should produce 12 vertices and 8 triangles
        assert_eq!(result.vertices.len(), 12);
        assert_eq!(result.triangle_count(), 8);
    }

    #[test]
    fn test_stats_hit_rate() {
        let stats = PipelineStats {
            cache_hits: 80,
            cache_misses: 20,
        };

        assert_eq!(stats.total_accesses(), 100);
        assert!((stats.hit_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_shader_integration_npatch_detection() {
        // NPatchEnable bit is at position 17
        let shader_with_npatch = 1u32 << 17;
        let shader_without_npatch = 0u32;

        assert!(NPatchShaderIntegration::should_use_npatch(
            shader_with_npatch
        ));
        assert!(!NPatchShaderIntegration::should_use_npatch(
            shader_without_npatch
        ));
    }

    #[test]
    fn test_level_recommendation_low_quality() {
        let level = NPatchShaderIntegration::recommend_level(50, 10.0, QualityLevel::Low);
        assert_eq!(level.as_raw(), TessellationLevel::LOW.as_raw());

        let level = NPatchShaderIntegration::recommend_level(500, 10.0, QualityLevel::Low);
        assert_eq!(level.as_raw(), TessellationLevel::NONE.as_raw());
    }

    #[test]
    fn test_level_recommendation_high_quality() {
        let level = NPatchShaderIntegration::recommend_level(20, 10.0, QualityLevel::High);
        assert_eq!(level.as_raw(), TessellationLevel::HIGH.as_raw());

        let level = NPatchShaderIntegration::recommend_level(200, 10.0, QualityLevel::High);
        assert_eq!(level.as_raw(), TessellationLevel::LOW.as_raw());
    }

    #[test]
    fn test_cache_memory_tracking() {
        let pipeline = NPatchPipeline::new(TessellationLevel::MEDIUM);

        let v0 = create_test_vertex(Vec3::ZERO);
        let v1 = create_test_vertex(Vec3::X);
        let v2 = create_test_vertex(Vec3::Y);

        // Process triangle to populate cache
        let _result = pipeline.process_triangle(1, &v0, &v1, &v2);

        let memory_usage = pipeline.cache_memory_usage();
        assert!(memory_usage > 0, "Cache should track memory usage");
    }
}
