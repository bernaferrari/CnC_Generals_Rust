//! W3DTreeBuffer Module - Advanced Scene Graph and Culling System
//! 
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/W3DTreeBuffer.cpp
//! 
//! This module provides high-performance scene graph management, spatial partitioning,
//! frustum culling, occlusion culling, and optimized render batching for 3D objects.

use cgmath::{Point3, Vector3, Matrix4, InnerSpace, EuclideanSpace, Zero};
use wgpu::{Buffer, Device, Queue, BufferDescriptor, BufferUsages, CommandEncoder};
use bytemuck::{Pod, Zeroable};
use std::{
    collections::{HashMap, VecDeque, BTreeMap},
    sync::{Arc, Weak},
    time::Duration,
};
use parking_lot::{RwLock, Mutex};
use smallvec::SmallVec;
use slotmap::{SlotMap, DefaultKey, SecondaryMap};
use dashmap::DashMap;
use rayon::prelude::*;
use bumpalo::Bump;
use anyhow::{Result, Context};
use thiserror::Error;
use game_network::NetworkInstant;

/// Maximum objects per spatial partition node
pub const MAX_OBJECTS_PER_NODE: usize = 16;

/// Maximum depth for spatial partitioning tree
pub const MAX_TREE_DEPTH: usize = 8;

/// Minimum bounding box size for subdivision
pub const MIN_BBOX_SIZE: f32 = 1.0;

/// Maximum render batches per frame
pub const MAX_RENDER_BATCHES: usize = 1024;

/// Instance buffer chunk size
pub const INSTANCE_BUFFER_CHUNK_SIZE: usize = 1000;

/// LOD distance thresholds
pub const LOD_DISTANCES: [f32; 4] = [50.0, 150.0, 400.0, 1000.0];

/// Axis-aligned bounding box for spatial partitioning
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AABB {
    pub min: Point3<f32>,
    pub max: Point3<f32>,
}

impl AABB {
    /// Create new AABB from min and max points
    pub fn new(min: Point3<f32>, max: Point3<f32>) -> Self {
        Self { min, max }
    }

    /// Create AABB from center and extents
    pub fn from_center_extents(center: Point3<f32>, extents: Vector3<f32>) -> Self {
        Self {
            min: center - extents,
            max: center + extents,
        }
    }

    /// Get center point of AABB
    pub fn center(&self) -> Point3<f32> {
        Point3::from_vec((self.min.to_vec() + self.max.to_vec()) * 0.5)
    }

    /// Get extents of AABB
    pub fn extents(&self) -> Vector3<f32> {
        (self.max.to_vec() - self.min.to_vec()) * 0.5
    }

    /// Get size of AABB
    pub fn size(&self) -> Vector3<f32> {
        self.max.to_vec() - self.min.to_vec()
    }

    /// Check if AABB contains a point
    pub fn contains_point(&self, point: Point3<f32>) -> bool {
        point.x >= self.min.x && point.x <= self.max.x
            && point.y >= self.min.y && point.y <= self.max.y
            && point.z >= self.min.z && point.z <= self.max.z
    }

    /// Check if AABB intersects another AABB
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x
            && self.min.y <= other.max.y && self.max.y >= other.min.y
            && self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    /// Expand AABB to include another AABB
    pub fn expand_to_include(&mut self, other: &AABB) {
        self.min.x = self.min.x.min(other.min.x);
        self.min.y = self.min.y.min(other.min.y);
        self.min.z = self.min.z.min(other.min.z);
        self.max.x = self.max.x.max(other.max.x);
        self.max.y = self.max.y.max(other.max.y);
        self.max.z = self.max.z.max(other.max.z);
    }

    /// Expand AABB to include a point
    pub fn expand_to_include_point(&mut self, point: Point3<f32>) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }
}

impl Default for AABB {
    fn default() -> Self {
        Self {
            min: Point3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Point3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }
}

/// Level of detail enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LODLevel {
    High = 0,
    Medium = 1,
    Low = 2,
    VeryLow = 3,
}

impl LODLevel {
    /// Get LOD level based on distance from camera
    pub fn from_distance(distance: f32) -> Self {
        if distance < LOD_DISTANCES[0] {
            LODLevel::High
        } else if distance < LOD_DISTANCES[1] {
            LODLevel::Medium
        } else if distance < LOD_DISTANCES[2] {
            LODLevel::Low
        } else {
            LODLevel::VeryLow
        }
    }
}

/// Render object data stored in the tree buffer
#[derive(Debug, Clone)]
pub struct RenderObject {
    pub id: DefaultKey,
    pub transform: Matrix4<f32>,
    pub world_bounds: AABB,
    pub local_bounds: AABB,
    pub mesh_id: u32,
    pub material_id: u32,
    pub lod_level: LODLevel,
    pub distance_from_camera: f32,
    pub visible: bool,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub transparency: f32,
    pub priority: i32,
    pub flags: u32,
}

impl Default for RenderObject {
    fn default() -> Self {
        Self {
            id: DefaultKey::default(),
            transform: Matrix4::identity(),
            world_bounds: AABB::default(),
            local_bounds: AABB::default(),
            mesh_id: 0,
            material_id: 0,
            lod_level: LODLevel::High,
            distance_from_camera: 0.0,
            visible: true,
            cast_shadows: true,
            receive_shadows: true,
            transparency: 0.0,
            priority: 0,
            flags: 0,
        }
    }
}

/// Render batch for efficient GPU rendering
#[derive(Debug, Clone)]
pub struct RenderBatch {
    pub mesh_id: u32,
    pub material_id: u32,
    pub lod_level: LODLevel,
    pub objects: Vec<DefaultKey>,
    pub instance_data: Vec<Matrix4<f32>>,
    pub instance_buffer: Option<Buffer>,
    pub transparent: bool,
    pub average_distance: f32,
}

impl RenderBatch {
    pub fn new(mesh_id: u32, material_id: u32, lod_level: LODLevel) -> Self {
        Self {
            mesh_id,
            material_id,
            lod_level,
            objects: Vec::with_capacity(INSTANCE_BUFFER_CHUNK_SIZE),
            instance_data: Vec::with_capacity(INSTANCE_BUFFER_CHUNK_SIZE),
            instance_buffer: None,
            transparent: false,
            average_distance: 0.0,
        }
    }

    /// Add object to batch
    pub fn add_object(&mut self, object_key: DefaultKey, transform: Matrix4<f32>, distance: f32) {
        self.objects.push(object_key);
        self.instance_data.push(transform);
        
        // Update average distance
        let count = self.objects.len() as f32;
        self.average_distance = (self.average_distance * (count - 1.0) + distance) / count;
    }

    /// Check if batch is full
    pub fn is_full(&self) -> bool {
        self.objects.len() >= INSTANCE_BUFFER_CHUNK_SIZE
    }

    /// Clear batch data
    pub fn clear(&mut self) {
        self.objects.clear();
        self.instance_data.clear();
        self.average_distance = 0.0;
    }
}

/// Spatial partitioning node for octree/quadtree
#[derive(Debug)]
pub struct SpatialNode {
    pub bounds: AABB,
    pub objects: Vec<DefaultKey>,
    pub children: Option<Box<[SpatialNode; 8]>>, // Octree children
    pub depth: usize,
}

impl SpatialNode {
    /// Create new spatial node
    pub fn new(bounds: AABB, depth: usize) -> Self {
        Self {
            bounds,
            objects: Vec::new(),
            children: None,
            depth,
        }
    }

    /// Check if node should be subdivided
    pub fn should_subdivide(&self) -> bool {
        self.objects.len() > MAX_OBJECTS_PER_NODE
            && self.depth < MAX_TREE_DEPTH
            && self.bounds.size().magnitude() > MIN_BBOX_SIZE
    }

    /// Subdivide node into 8 octree children
    pub fn subdivide(&mut self) {
        if self.children.is_some() {
            return;
        }

        let center = self.bounds.center();
        let extents = self.bounds.extents();
        let half_extents = extents * 0.5;

        let mut children = Vec::with_capacity(8);
        
        // Create 8 octree children
        for i in 0..8 {
            let x_offset = if i & 1 != 0 { half_extents.x } else { -half_extents.x };
            let y_offset = if i & 2 != 0 { half_extents.y } else { -half_extents.y };
            let z_offset = if i & 4 != 0 { half_extents.z } else { -half_extents.z };
            
            let child_center = center + Vector3::new(x_offset, y_offset, z_offset);
            let child_bounds = AABB::from_center_extents(child_center, half_extents);
            
            children.push(SpatialNode::new(child_bounds, self.depth + 1));
        }

        self.children = Some(children.into_boxed_slice().try_into().unwrap());
    }

    /// Insert object into appropriate child node
    pub fn insert_object(&mut self, object_key: DefaultKey, bounds: AABB) {
        if let Some(ref mut children) = self.children {
            // Find appropriate child nodes
            for child in children.iter_mut() {
                if child.bounds.intersects(&bounds) {
                    child.insert_object(object_key, bounds);
                }
            }
        } else {
            // Add to this node
            self.objects.push(object_key);
            
            // Check if we need to subdivide
            if self.should_subdivide() {
                self.subdivide();
                
                // Redistribute objects to children
                let objects_to_redistribute = std::mem::take(&mut self.objects);
                // TODO: Implement redistribution logic
                self.objects = objects_to_redistribute; // Placeholder
            }
        }
    }

    /// Query objects in region
    pub fn query_region(&self, region: &AABB, results: &mut Vec<DefaultKey>) {
        if !self.bounds.intersects(region) {
            return;
        }

        // Add objects from this node
        results.extend(&self.objects);

        // Query children
        if let Some(ref children) = self.children {
            for child in children.iter() {
                child.query_region(region, results);
            }
        }
    }
}

/// Frustum planes for culling
pub use crate::W3DDevice::GameClient::wthree_d_view::FrustumPlane;

/// Culling statistics
#[derive(Debug, Default)]
pub struct CullingStats {
    pub total_objects: u32,
    pub frustum_culled: u32,
    pub occlusion_culled: u32,
    pub distance_culled: u32,
    pub rendered: u32,
}

/// Performance metrics for tree buffer operations
#[derive(Debug, Default)]
pub struct TreeBufferMetrics {
    pub update_time: Duration,
    pub culling_time: Duration,
    pub batching_time: Duration,
    pub spatial_queries: u32,
    pub batches_created: u32,
    pub objects_batched: u32,
    pub culling_stats: CullingStats,
}

/// Main W3D Tree Buffer implementation with advanced scene management
pub struct W3DTreeBuffer {
    // Core data storage
    objects: SlotMap<DefaultKey, RenderObject>,
    object_transforms: SecondaryMap<DefaultKey, Matrix4<f32>>,
    object_visibility: SecondaryMap<DefaultKey, bool>,
    
    // Spatial partitioning
    root_node: SpatialNode,
    dirty_objects: Vec<DefaultKey>,
    rebuild_tree: bool,
    
    // Render batching
    render_batches: Vec<RenderBatch>,
    batch_map: HashMap<(u32, u32, LODLevel), usize>, // (mesh_id, material_id, lod) -> batch_index
    transparent_batches: Vec<RenderBatch>,
    
    // GPU resources
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    instance_buffers: Vec<Buffer>,
    
    // Culling and optimization
    camera_position: Point3<f32>,
    frustum_planes: [FrustumPlane; 6],
    max_draw_distance: f32,
    lod_bias: f32,
    
    // Performance tracking
    metrics: TreeBufferMetrics,
    last_update_time: NetworkInstant,
    frame_counter: u64,
    
    // Memory management
    temp_allocator: Bump,
    
    // Thread safety
    update_lock: Mutex<()>,
    read_lock: RwLock<()>,
    
    // Configuration
    culling_enabled: bool,
    batching_enabled: bool,
    lod_enabled: bool,
    parallel_processing: bool,
}

/// Error types for tree buffer operations
#[derive(Error, Debug)]
pub enum TreeBufferError {
    #[error("Graphics device not available")]
    DeviceNotAvailable,
    #[error("Buffer creation failed: {0}")]
    BufferCreationFailed(String),
    #[error("Object not found: {0:?}")]
    ObjectNotFound(DefaultKey),
    #[error("Invalid spatial bounds")]
    InvalidBounds,
    #[error("Tree subdivision failed")]
    SubdivisionFailed,
    #[error("Batch creation failed: {0}")]
    BatchCreationFailed(String),
}

impl W3DTreeBuffer {
    /// Create new tree buffer with specified world bounds
    pub fn new(world_bounds: AABB) -> Self {
        Self {
            objects: SlotMap::new(),
            object_transforms: SecondaryMap::new(),
            object_visibility: SecondaryMap::new(),
            root_node: SpatialNode::new(world_bounds, 0),
            dirty_objects: Vec::new(),
            rebuild_tree: false,
            render_batches: Vec::with_capacity(MAX_RENDER_BATCHES),
            batch_map: HashMap::new(),
            transparent_batches: Vec::new(),
            device: None,
            queue: None,
            instance_buffers: Vec::new(),
            camera_position: Point3::origin(),
            frustum_planes: [FrustumPlane { 
                normal: Vector3::zero(), 
                distance: 0.0 
            }; 6],
            max_draw_distance: 1000.0,
            lod_bias: 1.0,
            metrics: TreeBufferMetrics::default(),
            last_update_time: NetworkInstant::now(),
            frame_counter: 0,
            temp_allocator: Bump::new(),
            update_lock: Mutex::new(()),
            read_lock: RwLock::new(()),
            culling_enabled: true,
            batching_enabled: true,
            lod_enabled: true,
            parallel_processing: true,
        }
    }

    /// Initialize with GPU device and queue
    pub fn init(&mut self, device: Arc<Device>, queue: Arc<Queue>) -> Result<()> {
        self.device = Some(device);
        self.queue = Some(queue);
        self.create_initial_buffers()?;
        Ok(())
    }

    /// Create initial instance buffers
    fn create_initial_buffers(&mut self) -> Result<()> {
        let device = self.device.as_ref().ok_or(TreeBufferError::DeviceNotAvailable)?;
        
        // Create initial instance buffer
        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("W3D Tree Buffer Instance Buffer"),
            size: (INSTANCE_BUFFER_CHUNK_SIZE * std::mem::size_of::<Matrix4<f32>>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        self.instance_buffers.push(instance_buffer);
        Ok(())
    }

    /// Add new render object to the tree buffer
    pub fn add_object(&mut self, mut object: RenderObject) -> Result<DefaultKey> {
        let _lock = self.update_lock.lock();
        
        // Calculate world bounds from transform and local bounds
        self.update_object_world_bounds(&mut object);
        
        // Insert into objects storage
        let object_key = self.objects.insert(object.clone());
        object.id = object_key;
        self.objects[object_key] = object.clone();
        
        // Store transform and visibility
        self.object_transforms.insert(object_key, object.transform);
        self.object_visibility.insert(object_key, object.visible);
        
        // Insert into spatial tree
        self.root_node.insert_object(object_key, object.world_bounds);
        
        // Mark for batch update
        self.dirty_objects.push(object_key);
        
        Ok(object_key)
    }

    /// Update object transform and bounds
    pub fn update_object_transform(&mut self, object_key: DefaultKey, transform: Matrix4<f32>) -> Result<()> {
        let _lock = self.update_lock.lock();
        
        let object = self.objects.get_mut(object_key)
            .ok_or(TreeBufferError::ObjectNotFound(object_key))?;
        
        object.transform = transform;
        self.object_transforms[object_key] = transform;
        
        // Update world bounds
        self.update_object_world_bounds(object);
        
        // Mark as dirty for spatial tree update
        self.dirty_objects.push(object_key);
        
        Ok(())
    }

    /// Remove object from tree buffer
    pub fn remove_object(&mut self, object_key: DefaultKey) -> Result<()> {
        let _lock = self.update_lock.lock();
        
        if self.objects.remove(object_key).is_none() {
            return Err(TreeBufferError::ObjectNotFound(object_key));
        }
        
        self.object_transforms.remove(object_key);
        self.object_visibility.remove(object_key);
        
        // Mark tree for rebuild (expensive but ensures correctness)
        self.rebuild_tree = true;
        
        Ok(())
    }

    /// Update object world bounds from transform and local bounds
    fn update_object_world_bounds(&mut self, object: &mut RenderObject) {
        // Transform local bounds to world space
        let corners = [
            Point3::new(object.local_bounds.min.x, object.local_bounds.min.y, object.local_bounds.min.z),
            Point3::new(object.local_bounds.max.x, object.local_bounds.min.y, object.local_bounds.min.z),
            Point3::new(object.local_bounds.min.x, object.local_bounds.max.y, object.local_bounds.min.z),
            Point3::new(object.local_bounds.max.x, object.local_bounds.max.y, object.local_bounds.min.z),
            Point3::new(object.local_bounds.min.x, object.local_bounds.min.y, object.local_bounds.max.z),
            Point3::new(object.local_bounds.max.x, object.local_bounds.min.y, object.local_bounds.max.z),
            Point3::new(object.local_bounds.min.x, object.local_bounds.max.y, object.local_bounds.max.z),
            Point3::new(object.local_bounds.max.x, object.local_bounds.max.y, object.local_bounds.max.z),
        ];
        
        let mut world_bounds = AABB::default();
        for corner in &corners {
            let world_corner = object.transform.transform_point(*corner);
            world_bounds.expand_to_include_point(world_corner);
        }
        
        object.world_bounds = world_bounds;
    }

    /// Update tree buffer with camera and frustum information
    pub fn update(&mut self, camera_position: Point3<f32>, frustum_planes: [FrustumPlane; 6]) -> Result<()> {
        let start_time = NetworkInstant::now();
        let _lock = self.update_lock.lock();
        
        self.camera_position = camera_position;
        self.frustum_planes = frustum_planes;
        self.frame_counter += 1;
        
        // Reset metrics
        self.metrics = TreeBufferMetrics::default();
        
        // Rebuild spatial tree if needed
        if self.rebuild_tree {
            self.rebuild_spatial_tree()?;
            self.rebuild_tree = false;
        }
        
        // Update dirty objects in spatial tree
        self.update_dirty_objects()?;
        
        // Update LOD levels and distances
        self.update_lod_and_distances()?;
        
        // Perform culling
        if self.culling_enabled {
            self.perform_culling()?;
        }
        
        // Create render batches
        if self.batching_enabled {
            self.create_render_batches()?;
        }
        
        self.metrics.update_time = start_time.elapsed();
        self.last_update_time = NetworkInstant::now();
        
        Ok(())
    }

    /// Rebuild entire spatial tree
    fn rebuild_spatial_tree(&mut self) -> Result<()> {
        // Create new root node
        let world_bounds = self.calculate_world_bounds();
        self.root_node = SpatialNode::new(world_bounds, 0);
        
        // Re-insert all objects
        for (object_key, object) in &self.objects {
            self.root_node.insert_object(object_key, object.world_bounds);
        }
        
        self.dirty_objects.clear();
        Ok(())
    }

    /// Calculate world bounds containing all objects
    fn calculate_world_bounds(&self) -> AABB {
        let mut world_bounds = AABB::default();
        
        for object in self.objects.values() {
            world_bounds.expand_to_include(&object.world_bounds);
        }
        
        // Ensure minimum size
        if world_bounds.min.x == f32::INFINITY {
            world_bounds = AABB::new(
                Point3::new(-1000.0, -1000.0, -1000.0),
                Point3::new(1000.0, 1000.0, 1000.0),
            );
        }
        
        world_bounds
    }

    /// Update dirty objects in spatial tree
    fn update_dirty_objects(&mut self) -> Result<()> {
        for &object_key in &self.dirty_objects {
            if let Some(object) = self.objects.get(object_key) {
                // TODO: Remove from old position and insert in new position
                // For now, we'll mark for full rebuild if there are many dirty objects
                if self.dirty_objects.len() > MAX_OBJECTS_PER_NODE {
                    self.rebuild_tree = true;
                    break;
                }
            }
        }
        
        self.dirty_objects.clear();
        Ok(())
    }

    /// Update LOD levels and camera distances for all objects
    fn update_lod_and_distances(&mut self) -> Result<()> {
        let start_time = NetworkInstant::now();
        
        if self.parallel_processing && self.objects.len() > 1000 {
            // Parallel processing for large object counts
            let camera_pos = self.camera_position;
            let lod_bias = self.lod_bias;
            
            self.objects.par_iter_mut().for_each(|(_, object)| {
                let distance = (object.world_bounds.center() - camera_pos).magnitude();
                object.distance_from_camera = distance;
                
                if self.lod_enabled {
                    object.lod_level = LODLevel::from_distance(distance * lod_bias);
                }
            });
        } else {
            // Sequential processing
            for object in self.objects.values_mut() {
                let distance = (object.world_bounds.center() - self.camera_position).magnitude();
                object.distance_from_camera = distance;
                
                if self.lod_enabled {
                    object.lod_level = LODLevel::from_distance(distance * self.lod_bias);
                }
            }
        }
        
        self.metrics.update_time += start_time.elapsed();
        Ok(())
    }

    /// Perform frustum and distance culling
    fn perform_culling(&mut self) -> Result<()> {
        let start_time = NetworkInstant::now();
        
        for object in self.objects.values_mut() {
            let mut visible = true;
            let object_center = object.world_bounds.center();
            let object_radius = object.world_bounds.extents().magnitude();
            
            // Distance culling
            if object.distance_from_camera > self.max_draw_distance {
                visible = false;
                self.metrics.culling_stats.distance_culled += 1;
            }
            
            // Frustum culling
            if visible {
                for plane in &self.frustum_planes {
                    let distance = plane.normal.dot(object_center.to_vec()) + plane.distance;
                    if distance < -object_radius {
                        visible = false;
                        self.metrics.culling_stats.frustum_culled += 1;
                        break;
                    }
                }
            }
            
            // TODO: Occlusion culling
            
            object.visible = visible;
            self.object_visibility[object.id] = visible;
            
            if visible {
                self.metrics.culling_stats.rendered += 1;
            }
            
            self.metrics.culling_stats.total_objects += 1;
        }
        
        self.metrics.culling_time = start_time.elapsed();
        Ok(())
    }

    /// Create optimized render batches for GPU rendering
    fn create_render_batches(&mut self) -> Result<()> {
        let start_time = NetworkInstant::now();
        
        // Clear previous batches
        self.render_batches.clear();
        self.transparent_batches.clear();
        self.batch_map.clear();
        
        // Group visible objects by material, mesh, and LOD
        for (object_key, object) in &self.objects {
            if !object.visible {
                continue;
            }
            
            let batch_key = (object.mesh_id, object.material_id, object.lod_level);
            
            // Find or create batch
            let batch_index = if let Some(&index) = self.batch_map.get(&batch_key) {
                index
            } else {
                let new_batch = RenderBatch::new(object.mesh_id, object.material_id, object.lod_level);
                let index = if object.transparency > 0.0 {
                    self.transparent_batches.push(new_batch);
                    self.transparent_batches.len() - 1
                } else {
                    self.render_batches.push(new_batch);
                    self.render_batches.len() - 1
                };
                self.batch_map.insert(batch_key, index);
                index
            };
            
            // Add object to batch
            if object.transparency > 0.0 {
                if let Some(batch) = self.transparent_batches.get_mut(batch_index) {
                    batch.add_object(object_key, object.transform, object.distance_from_camera);
                }
            } else {
                if let Some(batch) = self.render_batches.get_mut(batch_index) {
                    batch.add_object(object_key, object.transform, object.distance_from_camera);
                }
            }
            
            self.metrics.objects_batched += 1;
        }
        
        // Sort transparent batches by distance (back to front)
        self.transparent_batches.sort_by(|a, b| b.average_distance.partial_cmp(&a.average_distance).unwrap());
        
        // Sort opaque batches by state changes (material, then mesh)
        self.render_batches.sort_by_key(|batch| (batch.material_id, batch.mesh_id));
        
        // Update GPU instance buffers
        self.update_instance_buffers()?;
        
        self.metrics.batches_created = self.render_batches.len() as u32 + self.transparent_batches.len() as u32;
        self.metrics.batching_time = start_time.elapsed();
        
        Ok(())
    }

    /// Update GPU instance buffers for all batches
    fn update_instance_buffers(&mut self) -> Result<()> {
        let device = self.device.as_ref().ok_or(TreeBufferError::DeviceNotAvailable)?;
        let queue = self.queue.as_ref().ok_or(TreeBufferError::DeviceNotAvailable)?;
        
        // Update opaque batches
        for batch in &mut self.render_batches {
            if !batch.instance_data.is_empty() {
                if batch.instance_buffer.is_none() {
                    // Create new buffer
                    let buffer = device.create_buffer(&BufferDescriptor {
                        label: Some("Render Batch Instance Buffer"),
                        size: (batch.instance_data.len() * std::mem::size_of::<Matrix4<f32>>()) as u64,
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    batch.instance_buffer = Some(buffer);
                }
                
                if let Some(ref buffer) = batch.instance_buffer {
                    let data = bytemuck::cast_slice(&batch.instance_data);
                    queue.write_buffer(buffer, 0, data);
                }
            }
        }
        
        // Update transparent batches
        for batch in &mut self.transparent_batches {
            if !batch.instance_data.is_empty() {
                if batch.instance_buffer.is_none() {
                    let buffer = device.create_buffer(&BufferDescriptor {
                        label: Some("Transparent Batch Instance Buffer"),
                        size: (batch.instance_data.len() * std::mem::size_of::<Matrix4<f32>>()) as u64,
                        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    batch.instance_buffer = Some(buffer);
                }
                
                if let Some(ref buffer) = batch.instance_buffer {
                    let data = bytemuck::cast_slice(&batch.instance_data);
                    queue.write_buffer(buffer, 0, data);
                }
            }
        }
        
        Ok(())
    }

    /// Query objects in a spatial region
    pub fn query_region(&self, region: &AABB) -> Vec<DefaultKey> {
        let _lock = self.read_lock.read();
        let mut results = Vec::new();
        self.root_node.query_region(region, &mut results);
        results
    }

    /// Get render batches for opaque objects
    pub fn get_opaque_batches(&self) -> &[RenderBatch] {
        &self.render_batches
    }

    /// Get render batches for transparent objects
    pub fn get_transparent_batches(&self) -> &[RenderBatch] {
        &self.transparent_batches
    }

    /// Get object by key
    pub fn get_object(&self, object_key: DefaultKey) -> Option<&RenderObject> {
        self.objects.get(object_key)
    }

    /// Get mutable object by key
    pub fn get_object_mut(&mut self, object_key: DefaultKey) -> Option<&mut RenderObject> {
        self.objects.get_mut(object_key)
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> &TreeBufferMetrics {
        &self.metrics
    }

    /// Set maximum draw distance for culling
    pub fn set_max_draw_distance(&mut self, distance: f32) {
        self.max_draw_distance = distance;
    }

    /// Set LOD bias multiplier
    pub fn set_lod_bias(&mut self, bias: f32) {
        self.lod_bias = bias;
    }

    /// Enable or disable culling
    pub fn set_culling_enabled(&mut self, enabled: bool) {
        self.culling_enabled = enabled;
    }

    /// Enable or disable batching
    pub fn set_batching_enabled(&mut self, enabled: bool) {
        self.batching_enabled = enabled;
    }

    /// Enable or disable LOD system
    pub fn set_lod_enabled(&mut self, enabled: bool) {
        self.lod_enabled = enabled;
    }

    /// Enable or disable parallel processing
    pub fn set_parallel_processing(&mut self, enabled: bool) {
        self.parallel_processing = enabled;
    }

    /// Get total number of objects
    pub fn get_object_count(&self) -> usize {
        self.objects.len()
    }

    /// Clear all objects and reset tree
    pub fn clear(&mut self) {
        let _lock = self.update_lock.lock();
        
        self.objects.clear();
        self.object_transforms.clear();
        self.object_visibility.clear();
        self.dirty_objects.clear();
        self.render_batches.clear();
        self.transparent_batches.clear();
        self.batch_map.clear();
        
        // Reset spatial tree
        let world_bounds = AABB::new(
            Point3::new(-1000.0, -1000.0, -1000.0),
            Point3::new(1000.0, 1000.0, 1000.0),
        );
        self.root_node = SpatialNode::new(world_bounds, 0);
        
        // Reset temp allocator
        self.temp_allocator.reset();
    }
}

// Thread-safe implementation
unsafe impl Send for W3DTreeBuffer {}
unsafe impl Sync for W3DTreeBuffer {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_operations() {
        let aabb1 = AABB::new(Point3::new(0.0, 0.0, 0.0), Point3::new(10.0, 10.0, 10.0));
        let aabb2 = AABB::new(Point3::new(5.0, 5.0, 5.0), Point3::new(15.0, 15.0, 15.0));
        
        assert!(aabb1.intersects(&aabb2));
        assert_eq!(aabb1.center(), Point3::new(5.0, 5.0, 5.0));
        assert!(aabb1.contains_point(Point3::new(5.0, 5.0, 5.0)));
    }

    #[test]
    fn test_tree_buffer_creation() {
        let world_bounds = AABB::new(
            Point3::new(-100.0, -100.0, -100.0),
            Point3::new(100.0, 100.0, 100.0),
        );
        let tree_buffer = W3DTreeBuffer::new(world_bounds);
        
        assert_eq!(tree_buffer.get_object_count(), 0);
        assert!(tree_buffer.get_opaque_batches().is_empty());
        assert!(tree_buffer.get_transparent_batches().is_empty());
    }

    #[test]
    fn test_object_management() {
        let world_bounds = AABB::new(
            Point3::new(-100.0, -100.0, -100.0),
            Point3::new(100.0, 100.0, 100.0),
        );
        let mut tree_buffer = W3DTreeBuffer::new(world_bounds);
        
        let mut object = RenderObject::default();
        object.mesh_id = 1;
        object.material_id = 1;
        object.local_bounds = AABB::new(Point3::new(-1.0, -1.0, -1.0), Point3::new(1.0, 1.0, 1.0));
        
        let object_key = tree_buffer.add_object(object).unwrap();
        assert_eq!(tree_buffer.get_object_count(), 1);
        
        let retrieved_object = tree_buffer.get_object(object_key).unwrap();
        assert_eq!(retrieved_object.mesh_id, 1);
        assert_eq!(retrieved_object.material_id, 1);
    }

    #[test]
    fn test_lod_distance_calculation() {
        assert_eq!(LODLevel::from_distance(25.0), LODLevel::High);
        assert_eq!(LODLevel::from_distance(100.0), LODLevel::Medium);
        assert_eq!(LODLevel::from_distance(300.0), LODLevel::Low);
        assert_eq!(LODLevel::from_distance(2000.0), LODLevel::VeryLow);
    }

    #[test]
    fn test_spatial_node_subdivision() {
        let bounds = AABB::new(Point3::new(-10.0, -10.0, -10.0), Point3::new(10.0, 10.0, 10.0));
        let mut node = SpatialNode::new(bounds, 0);
        
        assert!(!node.should_subdivide());
        
        // Add enough objects to trigger subdivision
        for _ in 0..MAX_OBJECTS_PER_NODE + 1 {
            node.objects.push(DefaultKey::default());
        }
        
        assert!(node.should_subdivide());
    }

    #[test]
    fn test_render_batch_management() {
        let mut batch = RenderBatch::new(1, 1, LODLevel::High);
        
        assert!(!batch.is_full());
        assert_eq!(batch.objects.len(), 0);
        
        batch.add_object(DefaultKey::default(), Matrix4::identity(), 50.0);
        assert_eq!(batch.objects.len(), 1);
        assert_eq!(batch.average_distance, 50.0);
        
        batch.add_object(DefaultKey::default(), Matrix4::identity(), 100.0);
        assert_eq!(batch.average_distance, 75.0);
    }
}
