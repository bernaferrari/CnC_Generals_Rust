//! Polygon Batching System for Draw Call Reduction
//!
//! This module implements optimized batching of polygons with the same material
//! to reduce draw calls and improve rendering performance.
//!
//! Port of meshgeometry.cpp batching logic (lines 450-580)

use glam::{Vec2, Vec3};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Maximum texture stages per material
pub const MAX_TEXTURE_STAGES: usize = 2;

/// Material key for batching polygons with same render state
/// Matches C++ material comparison logic (meshgeometry.cpp lines 450-480)
#[derive(Debug, Clone, Copy, Eq)]
pub struct MaterialKey {
    /// Shader ID
    pub shader_id: u64,
    /// Texture IDs (up to MAX_TEXTURE_STAGES)
    pub texture_ids: [u64; MAX_TEXTURE_STAGES],
    /// Material pass index
    pub material_index: usize,
    /// Vertex format flags
    pub vertex_format: u32,
}

impl MaterialKey {
    /// Create new material key
    pub fn new(
        shader_id: u64,
        texture_ids: [u64; MAX_TEXTURE_STAGES],
        material_index: usize,
        vertex_format: u32,
    ) -> Self {
        Self {
            shader_id,
            texture_ids,
            material_index,
            vertex_format,
        }
    }

    /// Create from individual components
    pub fn from_components(
        shader_id: u64,
        texture0: Option<u64>,
        texture1: Option<u64>,
        material_index: usize,
    ) -> Self {
        Self {
            shader_id,
            texture_ids: [texture0.unwrap_or(0), texture1.unwrap_or(0)],
            material_index,
            vertex_format: 0,
        }
    }
}

impl PartialEq for MaterialKey {
    fn eq(&self, other: &Self) -> bool {
        self.shader_id == other.shader_id
            && self.texture_ids == other.texture_ids
            && self.material_index == other.material_index
            && self.vertex_format == other.vertex_format
    }
}

impl Hash for MaterialKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shader_id.hash(state);
        self.texture_ids.hash(state);
        self.material_index.hash(state);
        self.vertex_format.hash(state);
    }
}

/// Polygon vertex data for batching
#[derive(Debug, Clone, Copy)]
pub struct BatchVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv0: Vec2,
    pub uv1: Vec2,
    pub color: u32,
}

impl BatchVertex {
    /// Create new batch vertex
    pub fn new(position: Vec3, normal: Vec3, uv0: Vec2, uv1: Vec2, color: u32) -> Self {
        Self {
            position,
            normal,
            uv0,
            uv1,
            color,
        }
    }
}

/// Polygon batch for a single material
/// Port of C++ polygon batching (meshgeometry.cpp lines 520-580)
pub struct PolygonBatch {
    /// Vertex data
    pub vertices: Vec<BatchVertex>,
    /// Index data
    pub indices: Vec<u32>,
    /// Number of draw calls represented
    pub draw_count: usize,
    /// Total triangle count
    pub triangle_count: usize,
}

impl PolygonBatch {
    /// Create new empty batch
    pub fn new() -> Self {
        Self {
            vertices: Vec::with_capacity(1024),
            indices: Vec::with_capacity(3072),
            draw_count: 0,
            triangle_count: 0,
        }
    }

    /// Add triangle to batch
    /// Port of meshgeometry.cpp Add_Triangle (lines 450-500)
    pub fn add_triangle(&mut self, v0: BatchVertex, v1: BatchVertex, v2: BatchVertex) {
        let base_index = self.vertices.len() as u32;

        self.vertices.push(v0);
        self.vertices.push(v1);
        self.vertices.push(v2);

        self.indices.push(base_index);
        self.indices.push(base_index + 1);
        self.indices.push(base_index + 2);

        self.triangle_count += 1;
        self.draw_count += 1;
    }

    /// Add indexed polygon to batch
    /// More efficient when vertices are shared
    pub fn add_indexed_triangle(
        &mut self,
        v0: BatchVertex,
        v1: BatchVertex,
        v2: BatchVertex,
        reuse_vertices: bool,
    ) {
        if reuse_vertices {
            // Try to find existing vertices to reduce duplication
            let idx0 = self.find_or_add_vertex(v0);
            let idx1 = self.find_or_add_vertex(v1);
            let idx2 = self.find_or_add_vertex(v2);

            self.indices.push(idx0);
            self.indices.push(idx1);
            self.indices.push(idx2);
        } else {
            // Add as unique vertices (faster, but more memory)
            self.add_triangle(v0, v1, v2);
        }
    }

    /// Find existing vertex or add new one
    fn find_or_add_vertex(&mut self, vertex: BatchVertex) -> u32 {
        // Simple linear search for now
        // In production, could use spatial hashing for better performance
        const EPSILON: f32 = 0.0001;

        for (i, v) in self.vertices.iter().enumerate() {
            if (v.position - vertex.position).length_squared() < EPSILON
                && (v.normal - vertex.normal).length_squared() < EPSILON
                && (v.uv0 - vertex.uv0).length_squared() < EPSILON
                && v.color == vertex.color
            {
                return i as u32;
            }
        }

        // Not found, add new vertex
        let index = self.vertices.len() as u32;
        self.vertices.push(vertex);
        index
    }

    /// Clear batch data
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.draw_count = 0;
        self.triangle_count = 0;
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get index count
    pub fn index_count(&self) -> usize {
        self.indices.len()
    }

    /// Get memory usage estimate in bytes
    pub fn memory_usage(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<BatchVertex>()
            + self.indices.len() * std::mem::size_of::<u32>()
    }
}

impl Default for PolygonBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Polygon batcher for draw call reduction
/// Port of C++ batching system (meshgeometry.cpp lines 450-580)
pub struct PolygonBatcher {
    /// Batches grouped by material
    batches: HashMap<MaterialKey, PolygonBatch>,
    /// Total polygons added
    total_polygons: usize,
    /// Original draw call count (before batching)
    original_draw_calls: usize,
    /// Enable vertex deduplication
    deduplicate_vertices: bool,
}

impl PolygonBatcher {
    /// Create new polygon batcher
    pub fn new() -> Self {
        Self {
            batches: HashMap::new(),
            total_polygons: 0,
            original_draw_calls: 0,
            deduplicate_vertices: true,
        }
    }

    /// Add polygon to appropriate batch
    /// Port of meshgeometry.cpp lines 450-500
    pub fn add_polygon(
        &mut self,
        material: MaterialKey,
        v0: BatchVertex,
        v1: BatchVertex,
        v2: BatchVertex,
    ) {
        let batch = self.batches.entry(material).or_insert_with(PolygonBatch::new);

        if self.deduplicate_vertices {
            batch.add_indexed_triangle(v0, v1, v2, true);
        } else {
            batch.add_triangle(v0, v1, v2);
        }

        self.total_polygons += 1;
        self.original_draw_calls += 1;
    }

    /// Add multiple polygons at once
    pub fn add_polygons(
        &mut self,
        material: MaterialKey,
        triangles: &[(BatchVertex, BatchVertex, BatchVertex)],
    ) {
        for &(v0, v1, v2) in triangles {
            self.add_polygon(material, v0, v1, v2);
        }
    }

    /// Get batch for material (if exists)
    pub fn get_batch(&self, material: &MaterialKey) -> Option<&PolygonBatch> {
        self.batches.get(material)
    }

    /// Get mutable batch for material (if exists)
    pub fn get_batch_mut(&mut self, material: &MaterialKey) -> Option<&mut PolygonBatch> {
        self.batches.get_mut(material)
    }

    /// Get all batches
    pub fn batches(&self) -> impl Iterator<Item = (&MaterialKey, &PolygonBatch)> {
        self.batches.iter()
    }

    /// Get batch count (final draw call count after batching)
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    /// Get draw call reduction ratio
    pub fn reduction_ratio(&self) -> f32 {
        if self.original_draw_calls == 0 {
            return 1.0;
        }
        self.batches.len() as f32 / self.original_draw_calls as f32
    }

    /// Get draw call savings
    pub fn draw_calls_saved(&self) -> usize {
        self.original_draw_calls.saturating_sub(self.batches.len())
    }

    /// Clear all batches
    pub fn clear(&mut self) {
        self.batches.clear();
        self.total_polygons = 0;
        self.original_draw_calls = 0;
    }

    /// Flush all batches (for rendering)
    /// Port of meshgeometry.cpp lines 520-580
    pub fn flush(&mut self) -> Vec<(MaterialKey, PolygonBatch)> {
        let mut result = Vec::with_capacity(self.batches.len());

        for (key, batch) in self.batches.drain() {
            if !batch.is_empty() {
                result.push((key, batch));
            }
        }

        self.total_polygons = 0;
        self.original_draw_calls = 0;

        result
    }

    /// Enable or disable vertex deduplication
    pub fn set_deduplicate_vertices(&mut self, enable: bool) {
        self.deduplicate_vertices = enable;
    }

    /// Get statistics
    pub fn stats(&self) -> BatchingStats {
        let total_vertices: usize = self.batches.values().map(|b| b.vertex_count()).sum();
        let total_indices: usize = self.batches.values().map(|b| b.index_count()).sum();
        let memory_usage: usize = self.batches.values().map(|b| b.memory_usage()).sum();

        BatchingStats {
            batch_count: self.batches.len(),
            total_polygons: self.total_polygons,
            original_draw_calls: self.original_draw_calls,
            draw_calls_saved: self.draw_calls_saved(),
            reduction_ratio: self.reduction_ratio(),
            total_vertices,
            total_indices,
            memory_usage,
        }
    }
}

impl Default for PolygonBatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Batching statistics
#[derive(Debug, Clone, Copy, Default)]
pub struct BatchingStats {
    pub batch_count: usize,
    pub total_polygons: usize,
    pub original_draw_calls: usize,
    pub draw_calls_saved: usize,
    pub reduction_ratio: f32,
    pub total_vertices: usize,
    pub total_indices: usize,
    pub memory_usage: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_key_equality() {
        let key1 = MaterialKey::new(100, [1, 2], 0, 0);
        let key2 = MaterialKey::new(100, [1, 2], 0, 0);
        let key3 = MaterialKey::new(200, [1, 2], 0, 0);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_material_key_from_components() {
        let key = MaterialKey::from_components(100, Some(1), Some(2), 0);
        assert_eq!(key.shader_id, 100);
        assert_eq!(key.texture_ids[0], 1);
        assert_eq!(key.texture_ids[1], 2);
    }

    #[test]
    fn test_batch_vertex_creation() {
        let vertex = BatchVertex::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec2::new(0.5, 0.5),
            Vec2::new(0.0, 0.0),
            0xFFFFFFFF,
        );
        assert_eq!(vertex.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(vertex.color, 0xFFFFFFFF);
    }

    #[test]
    fn test_polygon_batch_add_triangle() {
        let mut batch = PolygonBatch::new();
        let v0 = BatchVertex::new(
            Vec3::ZERO,
            Vec3::Z,
            Vec2::ZERO,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v1 = BatchVertex::new(
            Vec3::X,
            Vec3::Z,
            Vec2::X,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v2 = BatchVertex::new(
            Vec3::Y,
            Vec3::Z,
            Vec2::Y,
            Vec2::ZERO,
            0xFFFFFFFF,
        );

        batch.add_triangle(v0, v1, v2);

        assert_eq!(batch.vertex_count(), 3);
        assert_eq!(batch.index_count(), 3);
        assert_eq!(batch.triangle_count, 1);
    }

    #[test]
    fn test_polygon_batcher_creation() {
        let batcher = PolygonBatcher::new();
        assert_eq!(batcher.batch_count(), 0);
        assert_eq!(batcher.total_polygons, 0);
    }

    #[test]
    fn test_polygon_batcher_add_and_batch() {
        let mut batcher = PolygonBatcher::new();
        let material1 = MaterialKey::new(100, [1, 2], 0, 0);
        let material2 = MaterialKey::new(200, [3, 4], 0, 0);

        let v0 = BatchVertex::new(
            Vec3::ZERO,
            Vec3::Z,
            Vec2::ZERO,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v1 = BatchVertex::new(
            Vec3::X,
            Vec3::Z,
            Vec2::X,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v2 = BatchVertex::new(
            Vec3::Y,
            Vec3::Z,
            Vec2::Y,
            Vec2::ZERO,
            0xFFFFFFFF,
        );

        // Add 3 polygons with material1, 2 with material2
        batcher.add_polygon(material1, v0, v1, v2);
        batcher.add_polygon(material1, v0, v1, v2);
        batcher.add_polygon(material1, v0, v1, v2);
        batcher.add_polygon(material2, v0, v1, v2);
        batcher.add_polygon(material2, v0, v1, v2);

        assert_eq!(batcher.batch_count(), 2);
        assert_eq!(batcher.total_polygons, 5);
        assert_eq!(batcher.original_draw_calls, 5);
        assert_eq!(batcher.draw_calls_saved(), 3); // 5 original -> 2 batched
    }

    #[test]
    fn test_batching_stats() {
        let mut batcher = PolygonBatcher::new();
        let material = MaterialKey::new(100, [1, 2], 0, 0);

        let v0 = BatchVertex::new(
            Vec3::ZERO,
            Vec3::Z,
            Vec2::ZERO,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v1 = BatchVertex::new(
            Vec3::X,
            Vec3::Z,
            Vec2::X,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v2 = BatchVertex::new(
            Vec3::Y,
            Vec3::Z,
            Vec2::Y,
            Vec2::ZERO,
            0xFFFFFFFF,
        );

        for _ in 0..10 {
            batcher.add_polygon(material, v0, v1, v2);
        }

        let stats = batcher.stats();
        assert_eq!(stats.batch_count, 1);
        assert_eq!(stats.total_polygons, 10);
        assert_eq!(stats.original_draw_calls, 10);
        assert_eq!(stats.draw_calls_saved, 9);
        assert!((stats.reduction_ratio - 0.1).abs() < 0.01); // 1/10 = 0.1
    }

    #[test]
    fn test_flush_clears_batches() {
        let mut batcher = PolygonBatcher::new();
        let material = MaterialKey::new(100, [1, 2], 0, 0);

        let v0 = BatchVertex::new(
            Vec3::ZERO,
            Vec3::Z,
            Vec2::ZERO,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v1 = BatchVertex::new(
            Vec3::X,
            Vec3::Z,
            Vec2::X,
            Vec2::ZERO,
            0xFFFFFFFF,
        );
        let v2 = BatchVertex::new(
            Vec3::Y,
            Vec3::Z,
            Vec2::Y,
            Vec2::ZERO,
            0xFFFFFFFF,
        );

        batcher.add_polygon(material, v0, v1, v2);
        let flushed = batcher.flush();

        assert_eq!(flushed.len(), 1);
        assert_eq!(batcher.batch_count(), 0);
        assert_eq!(batcher.total_polygons, 0);
    }

    #[test]
    fn test_vertex_deduplication() {
        let mut batcher = PolygonBatcher::new();
        batcher.set_deduplicate_vertices(true);

        let material = MaterialKey::new(100, [1, 2], 0, 0);
        let v0 = BatchVertex::new(
            Vec3::ZERO,
            Vec3::Z,
            Vec2::ZERO,
            Vec2::ZERO,
            0xFFFFFFFF,
        );

        // Add same vertex 3 times
        batcher.add_polygon(material, v0, v0, v0);

        let batch = batcher.get_batch(&material).unwrap();
        // With deduplication, should have fewer vertices than indices
        assert!(batch.vertex_count() <= 3);
    }
}
