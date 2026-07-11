//! Batch rendering system for optimized draw call reduction
//!
//! Implements static and dynamic batching to match C++ batching performance

use glam::{Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Vertex data for batching
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BatchVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
    pub color: Vec4,
}

impl Default for BatchVertex {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            normal: Vec3::Y,
            uv: Vec2::ZERO,
            color: Vec4::ONE,
        }
    }
}

/// Material key for batching objects with same material
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterialKey {
    pub shader_id: u64,
    pub texture_id: u64,
    pub blend_mode: u8,
    pub cull_mode: u8,
}

impl Hash for MaterialKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shader_id.hash(state);
        self.texture_id.hash(state);
        self.blend_mode.hash(state);
        self.cull_mode.hash(state);
    }
}

/// Static batch for geometry that doesn't move
pub struct StaticBatch {
    /// Material identifier
    pub material: MaterialKey,
    /// Combined vertex buffer
    pub vertices: Vec<BatchVertex>,
    /// Combined index buffer
    pub indices: Vec<u32>,
    /// Bounding box for culling
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    /// Number of objects in this batch
    pub object_count: usize,
}

impl StaticBatch {
    /// Create a new empty batch
    pub fn new(material: MaterialKey) -> Self {
        Self {
            material,
            vertices: Vec::new(),
            indices: Vec::new(),
            bounds_min: Vec3::splat(f32::INFINITY),
            bounds_max: Vec3::splat(f32::NEG_INFINITY),
            object_count: 0,
        }
    }

    /// Add an object to this batch
    pub fn add_object(&mut self, vertices: &[BatchVertex], indices: &[u32], transform: Mat4) {
        let vertex_offset = self.vertices.len() as u32;

        // Transform and add vertices
        for vertex in vertices {
            let transformed_pos = transform.transform_point3(vertex.position);
            let transformed_normal = transform.transform_vector3(vertex.normal).normalize();

            self.vertices.push(BatchVertex {
                position: transformed_pos,
                normal: transformed_normal,
                uv: vertex.uv,
                color: vertex.color,
            });

            // Update bounds
            self.bounds_min = self.bounds_min.min(transformed_pos);
            self.bounds_max = self.bounds_max.max(transformed_pos);
        }

        // Add indices with offset
        for &index in indices {
            self.indices.push(index + vertex_offset);
        }

        self.object_count += 1;
    }

    /// Get memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<BatchVertex>()
            + self.indices.len() * std::mem::size_of::<u32>()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// Dynamic batch for objects that can move
pub struct DynamicBatch {
    /// Material identifier
    pub material: MaterialKey,
    /// Per-object vertex data (not combined)
    pub object_vertices: Vec<Vec<BatchVertex>>,
    /// Per-object index data
    pub object_indices: Vec<Vec<u32>>,
    /// Per-object transforms
    pub object_transforms: Vec<Mat4>,
    /// Visibility flags
    pub object_visible: Vec<bool>,
}

impl DynamicBatch {
    /// Create a new dynamic batch
    pub fn new(material: MaterialKey) -> Self {
        Self {
            material,
            object_vertices: Vec::new(),
            object_indices: Vec::new(),
            object_transforms: Vec::new(),
            object_visible: Vec::new(),
        }
    }

    /// Add an object to this batch
    pub fn add_object(
        &mut self,
        vertices: Vec<BatchVertex>,
        indices: Vec<u32>,
        transform: Mat4,
    ) -> usize {
        let object_id = self.object_vertices.len();
        self.object_vertices.push(vertices);
        self.object_indices.push(indices);
        self.object_transforms.push(transform);
        self.object_visible.push(true);
        object_id
    }

    /// Update object transform
    pub fn update_transform(&mut self, object_id: usize, transform: Mat4) {
        if object_id < self.object_transforms.len() {
            self.object_transforms[object_id] = transform;
        }
    }

    /// Set object visibility
    pub fn set_visible(&mut self, object_id: usize, visible: bool) {
        if object_id < self.object_visible.len() {
            self.object_visible[object_id] = visible;
        }
    }

    /// Build combined buffers for visible objects
    pub fn build_combined_buffers(&self) -> (Vec<BatchVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..self.object_vertices.len() {
            if !self.object_visible[i] {
                continue;
            }

            let vertex_offset = vertices.len() as u32;
            let transform = self.object_transforms[i];

            // Transform vertices
            for vertex in &self.object_vertices[i] {
                let transformed_pos = transform.transform_point3(vertex.position);
                let transformed_normal = transform.transform_vector3(vertex.normal).normalize();

                vertices.push(BatchVertex {
                    position: transformed_pos,
                    normal: transformed_normal,
                    uv: vertex.uv,
                    color: vertex.color,
                });
            }

            // Add indices with offset
            for &index in &self.object_indices[i] {
                indices.push(index + vertex_offset);
            }
        }

        (vertices, indices)
    }

    /// Get visible object count
    pub fn visible_count(&self) -> usize {
        self.object_visible.iter().filter(|&&v| v).count()
    }
}

/// Instance data for instanced rendering
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InstanceData {
    pub transform: Mat4,
    pub color: Vec4,
}

/// Instanced batch for many instances of same mesh
pub struct InstancedBatch {
    /// Material identifier
    pub material: MaterialKey,
    /// Shared vertex buffer (one copy)
    pub vertices: Vec<BatchVertex>,
    /// Shared index buffer
    pub indices: Vec<u32>,
    /// Per-instance data
    pub instances: Vec<InstanceData>,
    /// Maximum instances per batch
    pub max_instances: usize,
}

impl InstancedBatch {
    /// Create a new instanced batch
    pub fn new(
        material: MaterialKey,
        vertices: Vec<BatchVertex>,
        indices: Vec<u32>,
        max_instances: usize,
    ) -> Self {
        Self {
            material,
            vertices,
            indices,
            instances: Vec::new(),
            max_instances,
        }
    }

    /// Add an instance
    pub fn add_instance(&mut self, transform: Mat4, color: Vec4) -> Option<usize> {
        if self.instances.len() >= self.max_instances {
            return None;
        }

        let instance_id = self.instances.len();
        self.instances.push(InstanceData { transform, color });
        Some(instance_id)
    }

    /// Update instance transform
    pub fn update_instance(&mut self, instance_id: usize, transform: Mat4, color: Vec4) {
        if instance_id < self.instances.len() {
            self.instances[instance_id] = InstanceData { transform, color };
        }
    }

    /// Check if batch is full
    pub fn is_full(&self) -> bool {
        self.instances.len() >= self.max_instances
    }

    /// Clear all instances
    pub fn clear_instances(&mut self) {
        self.instances.clear();
    }
}

/// Batch renderer manager
pub struct BatchRenderer {
    /// Static batches by material
    static_batches: HashMap<MaterialKey, StaticBatch>,
    /// Dynamic batches by material
    dynamic_batches: HashMap<MaterialKey, DynamicBatch>,
    /// Instanced batches by material
    instanced_batches: HashMap<MaterialKey, Vec<InstancedBatch>>,
    /// Maximum vertices per batch
    max_vertices_per_batch: usize,
    /// Maximum instances per batch
    max_instances_per_batch: usize,
}

impl BatchRenderer {
    /// Create a new batch renderer
    pub fn new() -> Self {
        Self {
            static_batches: HashMap::new(),
            dynamic_batches: HashMap::new(),
            instanced_batches: HashMap::new(),
            max_vertices_per_batch: 65536,
            max_instances_per_batch: 1024,
        }
    }

    /// Add a static object (baked into batch)
    pub fn add_static_object(
        &mut self,
        material: MaterialKey,
        vertices: &[BatchVertex],
        indices: &[u32],
        transform: Mat4,
    ) {
        let batch = self
            .static_batches
            .entry(material.clone())
            .or_insert_with(|| StaticBatch::new(material));

        // Check if batch would exceed limit
        if batch.vertices.len() + vertices.len() > self.max_vertices_per_batch {
            // Create a new batch (in production, would need better key)
            return;
        }

        batch.add_object(vertices, indices, transform);
    }

    /// Add a dynamic object (can be updated)
    pub fn add_dynamic_object(
        &mut self,
        material: MaterialKey,
        vertices: Vec<BatchVertex>,
        indices: Vec<u32>,
        transform: Mat4,
    ) -> (MaterialKey, usize) {
        let batch = self
            .dynamic_batches
            .entry(material.clone())
            .or_insert_with(|| DynamicBatch::new(material.clone()));

        let object_id = batch.add_object(vertices, indices, transform);
        (material, object_id)
    }

    /// Add an instanced object
    pub fn add_instanced_object(
        &mut self,
        material: MaterialKey,
        vertices: Vec<BatchVertex>,
        indices: Vec<u32>,
        transform: Mat4,
        color: Vec4,
    ) -> Option<(usize, usize)> {
        // Find or create instanced batch
        let batches = self
            .instanced_batches
            .entry(material.clone())
            .or_default();

        // Try to find a batch that matches this mesh
        // In production, would use mesh hash to find matching batches
        for (batch_id, batch) in batches.iter_mut().enumerate() {
            if !batch.is_full() {
                if let Some(instance_id) = batch.add_instance(transform, color) {
                    return Some((batch_id, instance_id));
                }
            }
        }

        // Create new batch
        let mut batch =
            InstancedBatch::new(material, vertices, indices, self.max_instances_per_batch);
        batch.add_instance(transform, color);
        batches.push(batch);
        Some((batches.len() - 1, 0))
    }

    /// Get all static batches
    pub fn get_static_batches(&self) -> impl Iterator<Item = &StaticBatch> {
        self.static_batches.values()
    }

    /// Get all dynamic batches
    pub fn get_dynamic_batches(&self) -> impl Iterator<Item = &DynamicBatch> {
        self.dynamic_batches.values()
    }

    /// Get batch statistics
    pub fn stats(&self) -> BatchStats {
        let static_batch_count = self.static_batches.len();
        let dynamic_batch_count = self.dynamic_batches.len();
        let instanced_batch_count: usize = self.instanced_batches.values().map(|v| v.len()).sum();

        let total_static_objects: usize =
            self.static_batches.values().map(|b| b.object_count).sum();
        let total_dynamic_objects: usize = self
            .dynamic_batches
            .values()
            .map(|b| b.object_vertices.len())
            .sum();
        let total_instances: usize = self
            .instanced_batches
            .values()
            .flat_map(|batches| batches.iter().map(|b| b.instances.len()))
            .sum();

        BatchStats {
            static_batch_count,
            dynamic_batch_count,
            instanced_batch_count,
            total_static_objects,
            total_dynamic_objects,
            total_instances,
            draw_calls_saved: total_static_objects.saturating_sub(static_batch_count)
                + total_dynamic_objects.saturating_sub(dynamic_batch_count),
        }
    }

    /// Clear all batches
    pub fn clear(&mut self) {
        self.static_batches.clear();
        self.dynamic_batches.clear();
        self.instanced_batches.clear();
    }
}

impl Default for BatchRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch rendering statistics
#[derive(Debug, Clone, Copy)]
pub struct BatchStats {
    pub static_batch_count: usize,
    pub dynamic_batch_count: usize,
    pub instanced_batch_count: usize,
    pub total_static_objects: usize,
    pub total_dynamic_objects: usize,
    pub total_instances: usize,
    pub draw_calls_saved: usize,
}

impl std::fmt::Display for BatchStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Batch Rendering Statistics:")?;
        writeln!(f, "  Static batches:  {}", self.static_batch_count)?;
        writeln!(f, "  Dynamic batches: {}", self.dynamic_batch_count)?;
        writeln!(f, "  Instanced batches: {}", self.instanced_batch_count)?;
        writeln!(f, "  Static objects:  {}", self.total_static_objects)?;
        writeln!(f, "  Dynamic objects: {}", self.total_dynamic_objects)?;
        writeln!(f, "  Instances:       {}", self.total_instances)?;
        writeln!(f, "  Draw calls saved: {}", self.draw_calls_saved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_material() -> MaterialKey {
        MaterialKey {
            shader_id: 1,
            texture_id: 1,
            blend_mode: 0,
            cull_mode: 0,
        }
    }

    fn create_test_cube() -> (Vec<BatchVertex>, Vec<u32>) {
        let vertices = vec![BatchVertex::default(); 8];
        let indices = vec![
            0, 1, 2, 2, 3, 0, // Front
            4, 5, 6, 6, 7, 4, // Back
        ];
        (vertices, indices)
    }

    #[test]
    fn test_static_batch() {
        let mut batch = StaticBatch::new(create_test_material());
        let (vertices, indices) = create_test_cube();

        batch.add_object(&vertices, &indices, Mat4::IDENTITY);
        assert_eq!(batch.object_count, 1);
        assert_eq!(batch.vertices.len(), 8);
        assert_eq!(batch.indices.len(), 12);
    }

    #[test]
    fn test_dynamic_batch() {
        let mut batch = DynamicBatch::new(create_test_material());
        let (vertices, indices) = create_test_cube();

        let id = batch.add_object(vertices, indices, Mat4::IDENTITY);
        assert_eq!(id, 0);

        batch.set_visible(id, false);
        assert_eq!(batch.visible_count(), 0);
    }

    #[test]
    fn test_instanced_batch() {
        let (vertices, indices) = create_test_cube();
        let mut batch = InstancedBatch::new(create_test_material(), vertices, indices, 10);

        let id = batch.add_instance(Mat4::IDENTITY, Vec4::ONE);
        assert!(id.is_some());

        // Fill to capacity
        for _ in 0..9 {
            batch.add_instance(Mat4::IDENTITY, Vec4::ONE);
        }

        assert!(batch.is_full());
        assert!(batch.add_instance(Mat4::IDENTITY, Vec4::ONE).is_none());
    }

    #[test]
    fn test_batch_renderer() {
        let mut renderer = BatchRenderer::new();
        let (vertices, indices) = create_test_cube();
        let material = create_test_material();

        // Add static objects
        renderer.add_static_object(material.clone(), &vertices, &indices, Mat4::IDENTITY);
        renderer.add_static_object(material.clone(), &vertices, &indices, Mat4::IDENTITY);

        let stats = renderer.stats();
        assert_eq!(stats.static_batch_count, 1);
        assert_eq!(stats.total_static_objects, 2);
        assert_eq!(stats.draw_calls_saved, 1); // 2 objects in 1 batch = 1 saved draw call
    }
}
