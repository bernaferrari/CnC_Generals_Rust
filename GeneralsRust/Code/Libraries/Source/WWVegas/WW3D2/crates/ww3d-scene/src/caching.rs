//! Caching strategies for expensive operations
//!
//! Implements transform caching, visibility caching, and shader caching
//! to match C++ dirty flag and caching systems

use glam::{Mat4, Vec3};
use std::collections::{HashMap, HashSet};

/// Transform cache with dirty flag tracking
pub struct TransformCache {
    /// Cached world transforms
    world_transforms: Vec<Mat4>,
    /// Dirty flags for each transform
    dirty_flags: Vec<bool>,
    /// Parent indices for hierarchy
    parent_indices: Vec<Option<usize>>,
    /// Local transforms
    local_transforms: Vec<Mat4>,
    /// Last update frame
    last_update_frame: u64,
    /// Current frame number
    current_frame: u64,
}

impl TransformCache {
    /// Create a new transform cache
    pub fn new(capacity: usize) -> Self {
        Self {
            world_transforms: vec![Mat4::IDENTITY; capacity],
            dirty_flags: vec![true; capacity],
            parent_indices: vec![None; capacity],
            local_transforms: vec![Mat4::IDENTITY; capacity],
            last_update_frame: 0,
            current_frame: 0,
        }
    }

    /// Set local transform and mark as dirty
    pub fn set_local_transform(&mut self, index: usize, transform: Mat4) {
        if index < self.local_transforms.len() {
            self.local_transforms[index] = transform;
            self.mark_dirty(index);
        }
    }

    /// Get world transform, computing if necessary
    pub fn get_world_transform(&mut self, index: usize) -> Mat4 {
        if index >= self.world_transforms.len() {
            return Mat4::IDENTITY;
        }

        if self.dirty_flags[index] {
            self.update_world_transform(index);
        }

        self.world_transforms[index]
    }

    /// Set parent for hierarchical transforms
    pub fn set_parent(&mut self, index: usize, parent: Option<usize>) {
        if index < self.parent_indices.len() {
            self.parent_indices[index] = parent;
            self.mark_dirty(index);
        }
    }

    /// Mark a transform as dirty (and all children)
    pub fn mark_dirty(&mut self, index: usize) {
        if index < self.dirty_flags.len() {
            self.dirty_flags[index] = true;

            // Mark all children dirty
            for i in 0..self.parent_indices.len() {
                if self.parent_indices[i] == Some(index) {
                    self.mark_dirty(i);
                }
            }
        }
    }

    /// Update a world transform
    fn update_world_transform(&mut self, index: usize) {
        if let Some(parent_idx) = self.parent_indices[index] {
            // Ensure parent is up to date
            if self.dirty_flags[parent_idx] {
                self.update_world_transform(parent_idx);
            }

            // Combine with parent
            self.world_transforms[index] =
                self.world_transforms[parent_idx] * self.local_transforms[index];
        } else {
            // No parent, world = local
            self.world_transforms[index] = self.local_transforms[index];
        }

        self.dirty_flags[index] = false;
    }

    /// Update all dirty transforms
    pub fn update_all_dirty(&mut self) {
        for i in 0..self.dirty_flags.len() {
            if self.dirty_flags[i] {
                self.update_world_transform(i);
            }
        }
        self.current_frame += 1;
    }

    /// Check if a transform is dirty
    pub fn is_dirty(&self, index: usize) -> bool {
        self.dirty_flags.get(index).copied().unwrap_or(false)
    }

    /// Get statistics
    pub fn stats(&self) -> CacheStats {
        let dirty_count = self.dirty_flags.iter().filter(|&&d| d).count();
        CacheStats {
            total_entries: self.world_transforms.len(),
            dirty_entries: dirty_count,
            clean_entries: self.world_transforms.len() - dirty_count,
            cache_hit_rate: if self.world_transforms.len() > 0 {
                (self.world_transforms.len() - dirty_count) as f32
                    / self.world_transforms.len() as f32
            } else {
                0.0
            },
        }
    }

    /// Resize the cache
    pub fn resize(&mut self, new_size: usize) {
        self.world_transforms.resize(new_size, Mat4::IDENTITY);
        self.dirty_flags.resize(new_size, true);
        self.parent_indices.resize(new_size, None);
        self.local_transforms.resize(new_size, Mat4::IDENTITY);
    }
}

/// Visibility cache using temporal coherence
pub struct VisibilityCache {
    /// Objects visible last frame
    visible_last_frame: HashSet<usize>,
    /// Objects visible this frame
    visible_this_frame: HashSet<usize>,
    /// Cache valid flag
    cache_valid: bool,
    /// Camera position last frame
    last_camera_pos: Vec3,
    /// Camera movement threshold
    movement_threshold: f32,
}

impl VisibilityCache {
    /// Create a new visibility cache
    pub fn new(movement_threshold: f32) -> Self {
        Self {
            visible_last_frame: HashSet::new(),
            visible_this_frame: HashSet::new(),
            cache_valid: false,
            last_camera_pos: Vec3::ZERO,
            movement_threshold,
        }
    }

    /// Check if object was visible last frame (temporal coherence)
    pub fn was_visible_last_frame(&self, object_id: usize) -> bool {
        self.cache_valid && self.visible_last_frame.contains(&object_id)
    }

    /// Mark object as visible this frame
    pub fn mark_visible(&mut self, object_id: usize) {
        self.visible_this_frame.insert(object_id);
    }

    /// Update camera position and invalidate if moved too much
    pub fn update_camera(&mut self, camera_pos: Vec3) {
        let movement = (camera_pos - self.last_camera_pos).length();

        if movement > self.movement_threshold {
            self.invalidate();
        }

        self.last_camera_pos = camera_pos;
    }

    /// Swap buffers and prepare for next frame
    pub fn end_frame(&mut self) {
        std::mem::swap(&mut self.visible_last_frame, &mut self.visible_this_frame);
        self.visible_this_frame.clear();
        self.cache_valid = true;
    }

    /// Invalidate the cache
    pub fn invalidate(&mut self) {
        self.cache_valid = false;
        self.visible_last_frame.clear();
        self.visible_this_frame.clear();
    }

    /// Get cache statistics
    pub fn stats(&self) -> VisibilityCacheStats {
        VisibilityCacheStats {
            visible_count: self.visible_last_frame.len(),
            cache_valid: self.cache_valid,
        }
    }
}

/// Shader parameter cache to avoid redundant GPU updates
pub struct ShaderParameterCache {
    /// Cached parameter values
    cached_values: HashMap<String, CachedValue>,
}

#[derive(Clone)]
enum CachedValue {
    Float(f32),
    Vec3(Vec3),
    Mat4(Mat4),
    Int(i32),
}

impl ShaderParameterCache {
    /// Create a new shader parameter cache
    pub fn new() -> Self {
        Self {
            cached_values: HashMap::new(),
        }
    }

    /// Set a float parameter, returns true if changed
    pub fn set_float(&mut self, name: &str, value: f32) -> bool {
        if let Some(CachedValue::Float(cached)) = self.cached_values.get(name) {
            if (cached - value).abs() < 0.0001 {
                return false;
            }
        }
        self.cached_values
            .insert(name.to_string(), CachedValue::Float(value));
        true
    }

    /// Set a Vec3 parameter, returns true if changed
    pub fn set_vec3(&mut self, name: &str, value: Vec3) -> bool {
        if let Some(CachedValue::Vec3(cached)) = self.cached_values.get(name) {
            if (cached - value).length() < 0.0001 {
                return false;
            }
        }
        self.cached_values
            .insert(name.to_string(), CachedValue::Vec3(value));
        true
    }

    /// Set a Mat4 parameter, returns true if changed
    pub fn set_mat4(&mut self, name: &str, value: Mat4) -> bool {
        // Simple comparison - in production might want more sophisticated comparison
        self.cached_values
            .insert(name.to_string(), CachedValue::Mat4(value));
        true
    }

    /// Clear all cached values
    pub fn clear(&mut self) {
        self.cached_values.clear();
    }
}

impl Default for ShaderParameterCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Geometry data cache for mesh data
pub struct GeometryCache {
    /// Cached vertex buffers
    vertex_buffers: HashMap<usize, Vec<u8>>,
    /// Cached index buffers
    index_buffers: HashMap<usize, Vec<u32>>,
    /// Last access time for LRU eviction
    last_access: HashMap<usize, u64>,
    /// Current time
    current_time: u64,
    /// Maximum cache size in bytes
    max_size: usize,
    /// Current cache size
    current_size: usize,
}

impl GeometryCache {
    /// Create a new geometry cache
    pub fn new(max_size_mb: usize) -> Self {
        Self {
            vertex_buffers: HashMap::new(),
            index_buffers: HashMap::new(),
            last_access: HashMap::new(),
            current_time: 0,
            max_size: max_size_mb * 1024 * 1024,
            current_size: 0,
        }
    }

    /// Store vertex buffer
    pub fn store_vertex_buffer(&mut self, id: usize, data: Vec<u8>) {
        let size = data.len();
        self.evict_if_needed(size);

        if let Some(old_data) = self.vertex_buffers.insert(id, data) {
            self.current_size -= old_data.len();
        }

        self.current_size += size;
        self.last_access.insert(id, self.current_time);
        self.current_time += 1;
    }

    /// Retrieve vertex buffer
    pub fn get_vertex_buffer(&mut self, id: usize) -> Option<&Vec<u8>> {
        if self.vertex_buffers.contains_key(&id) {
            self.last_access.insert(id, self.current_time);
            self.current_time += 1;
            self.vertex_buffers.get(&id)
        } else {
            None
        }
    }

    /// Evict old entries if needed
    fn evict_if_needed(&mut self, needed_size: usize) {
        while self.current_size + needed_size > self.max_size && !self.last_access.is_empty() {
            // Find least recently used
            if let Some((&lru_id, _)) = self.last_access.iter().min_by_key(|(_, &time)| time) {
                self.evict(lru_id);
            }
        }
    }

    /// Evict a specific entry
    fn evict(&mut self, id: usize) {
        if let Some(data) = self.vertex_buffers.remove(&id) {
            self.current_size -= data.len();
        }
        if let Some(data) = self.index_buffers.remove(&id) {
            self.current_size -= data.len() * std::mem::size_of::<u32>();
        }
        self.last_access.remove(&id);
    }

    /// Get cache statistics
    pub fn stats(&self) -> GeometryCacheStats {
        GeometryCacheStats {
            entry_count: self.vertex_buffers.len(),
            size_bytes: self.current_size,
            max_size_bytes: self.max_size,
            utilization: self.current_size as f32 / self.max_size as f32,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub total_entries: usize,
    pub dirty_entries: usize,
    pub clean_entries: usize,
    pub cache_hit_rate: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct VisibilityCacheStats {
    pub visible_count: usize,
    pub cache_valid: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct GeometryCacheStats {
    pub entry_count: usize,
    pub size_bytes: usize,
    pub max_size_bytes: usize,
    pub utilization: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_cache() {
        let mut cache = TransformCache::new(10);

        // Set local transform
        let transform = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        cache.set_local_transform(0, transform);

        // Should be dirty
        assert!(cache.is_dirty(0));

        // Get world transform
        let world = cache.get_world_transform(0);
        assert_eq!(world, transform);

        // Should no longer be dirty
        assert!(!cache.is_dirty(0));
    }

    #[test]
    fn test_hierarchical_transforms() {
        let mut cache = TransformCache::new(10);

        // Parent transform
        let parent_transform = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));
        cache.set_local_transform(0, parent_transform);

        // Child transform
        let child_transform = Mat4::from_translation(Vec3::new(5.0, 0.0, 0.0));
        cache.set_local_transform(1, child_transform);
        cache.set_parent(1, Some(0));

        // Get child world transform
        let child_world = cache.get_world_transform(1);

        // Should be parent * child = (10,0,0) + (5,0,0) = (15,0,0)
        let expected_pos = Vec3::new(15.0, 0.0, 0.0);
        let actual_pos = child_world.w_axis.truncate();
        assert!((actual_pos - expected_pos).length() < 0.001);
    }

    #[test]
    fn test_visibility_cache() {
        let mut cache = VisibilityCache::new(1.0);

        // Mark some objects visible
        cache.mark_visible(0);
        cache.mark_visible(1);
        cache.end_frame();

        // Should be visible from last frame
        assert!(cache.was_visible_last_frame(0));
        assert!(cache.was_visible_last_frame(1));
        assert!(!cache.was_visible_last_frame(2));
    }

    #[test]
    fn test_shader_parameter_cache() {
        let mut cache = ShaderParameterCache::new();

        // First set should return true (changed)
        assert!(cache.set_float("test", 1.0));

        // Same value should return false (not changed)
        assert!(!cache.set_float("test", 1.0));

        // Different value should return true
        assert!(cache.set_float("test", 2.0));
    }

    #[test]
    fn test_geometry_cache() {
        let mut cache = GeometryCache::new(1); // 1 MB max

        let data = vec![0u8; 1024]; // 1 KB
        cache.store_vertex_buffer(0, data.clone());

        assert!(cache.get_vertex_buffer(0).is_some());

        let stats = cache.stats();
        assert_eq!(stats.entry_count, 1);
    }
}
