//! LOD Object - Integrated into 3D Renderer
//!
//! This module defines objects that support multiple levels of detail
//! integrated with the 3D rendering pipeline.

use glam::{Mat4, Vec3};
use std::any::Any;
use std::sync::Arc;

/// Individual LOD level containing rendering information
#[derive(Debug, Clone)]
pub struct LodLevel {
    /// Mesh/geometry for this LOD level
    pub mesh: Option<Arc<dyn LodGeometry>>,
    /// Distance at which this LOD level becomes active
    pub distance_threshold: f32,
    /// Screen space threshold for this LOD level
    pub screen_space_threshold: f32,
    /// Render cost of this LOD level
    pub render_cost: f32,
    /// Quality value of this LOD level
    pub render_value: f32,
}

impl LodLevel {
    /// Create a new LOD level
    pub fn new(distance_threshold: f32, screen_space_threshold: f32) -> Self {
        Self {
            mesh: None,
            distance_threshold,
            screen_space_threshold,
            render_cost: 1.0,
            render_value: 1.0,
        }
    }

    /// Set the mesh for this LOD level
    pub fn with_mesh(mut self, mesh: Arc<dyn LodGeometry>) -> Self {
        self.mesh = Some(mesh);
        self
    }

    /// Set the render cost
    pub fn with_cost(mut self, cost: f32) -> Self {
        self.render_cost = cost;
        self
    }
}

/// Trait for LOD geometry that can be rendered by the 3D renderer
pub trait LodGeometry: std::fmt::Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    /// Render object name used to instantiate geometry
    fn render_object_name(&self) -> &str;

    /// Get the bounding center
    fn bounding_center(&self) -> Vec3;

    /// Get the bounding radius
    fn bounding_radius(&self) -> f32;

    /// Get triangle count
    fn triangle_count(&self) -> usize;

    /// Get vertex count
    fn vertex_count(&self) -> usize;

    /// Render this geometry (placeholder for integration with render pipeline)
    fn render(&self) {
        // Default implementation - override in concrete types
    }

    fn transform_nodes(&self) -> &[TransformNodeInstance] {
        &[]
    }

    fn snap_points(&self) -> &[Vec3] {
        &[]
    }
}

/// LOD object integrated with the 3D renderer
#[derive(Debug)]
pub struct LodObject {
    /// Unique identifier
    pub id: u64,

    /// World position
    pub position: Vec3,

    /// Descriptive label (usually the asset name)
    pub label: String,

    /// Current LOD level
    pub current_lod_level: u32,

    /// Target LOD level
    pub target_lod_level: u32,

    /// LOD levels available
    pub lod_levels: Vec<LodLevel>,

    /// Whether this object is visible
    pub is_visible: bool,

    /// Whether LOD is enabled for this object
    pub lod_enabled: bool,
}

impl LodObject {
    /// Create a new LOD object
    pub fn new(id: u64, position: Vec3) -> Self {
        Self {
            id,
            position,
            label: String::new(),
            current_lod_level: 0,
            target_lod_level: 0,
            lod_levels: Vec::new(),
            is_visible: true,
            lod_enabled: true,
        }
    }

    /// Assign a descriptive label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    /// Add a LOD level
    pub fn add_lod_level(&mut self, lod_level: LodLevel) {
        self.lod_levels.push(lod_level);
        // Sort by distance threshold
        self.lod_levels.sort_by(|a, b| {
            a.distance_threshold
                .partial_cmp(&b.distance_threshold)
                .unwrap()
        });
    }

    /// Set the current LOD level
    pub fn set_lod_level(&mut self, level: u32) {
        let clamped_level = level.min(self.lod_levels.len().saturating_sub(1) as u32);
        self.current_lod_level = clamped_level;
        self.target_lod_level = clamped_level;
    }

    /// Get the current geometry
    pub fn current_geometry(&self) -> Option<&Arc<dyn LodGeometry>> {
        self.lod_levels
            .get(self.current_lod_level as usize)
            .and_then(|level| level.mesh.as_ref())
    }

    /// Get LOD level count
    pub fn lod_level_count(&self) -> usize {
        self.lod_levels.len()
    }

    /// Check if should render
    pub fn should_render(&self) -> bool {
        self.is_visible && self.lod_enabled && !self.lod_levels.is_empty()
    }
}

/// Mesh LOD geometry for the 3D renderer
#[derive(Debug, Clone)]
pub struct MeshLodGeometry {
    pub render_obj_name: String,
    pub transform: Mat4,
    pub center: Vec3,
    pub radius: f32,
    pub triangle_count: usize,
    pub vertex_count: usize,
    pub transform_nodes: Vec<TransformNodeInstance>,
    pub snap_points: Vec<Vec3>,
}

impl MeshLodGeometry {
    pub fn new(
        render_obj_name: String,
        transform: Mat4,
        center: Vec3,
        radius: f32,
        triangles: usize,
        vertices: usize,
        transform_nodes: Vec<TransformNodeInstance>,
        snap_points: Vec<Vec3>,
    ) -> Self {
        Self {
            render_obj_name,
            transform,
            center,
            radius,
            triangle_count: triangles,
            vertex_count: vertices,
            transform_nodes,
            snap_points,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeshInstanceTemplate {
    pub name: String,
    pub transform: Mat4,
}

#[derive(Debug, Clone)]
pub struct TransformNodeInstance {
    pub name: String,
    pub transform: Mat4,
}

#[derive(Debug, Clone)]
pub struct CompositeMeshLodGeometry {
    pub instances: Vec<MeshInstanceTemplate>,
    pub center: Vec3,
    pub radius: f32,
    pub triangle_count: usize,
    pub vertex_count: usize,
    pub transform_nodes: Vec<TransformNodeInstance>,
    pub snap_points: Vec<Vec3>,
}

impl CompositeMeshLodGeometry {
    pub fn new(
        instances: Vec<MeshInstanceTemplate>,
        center: Vec3,
        radius: f32,
        triangles: usize,
        vertices: usize,
        transform_nodes: Vec<TransformNodeInstance>,
        snap_points: Vec<Vec3>,
    ) -> Self {
        Self {
            instances,
            center,
            radius,
            triangle_count: triangles,
            vertex_count: vertices,
            transform_nodes,
            snap_points,
        }
    }
}

impl LodGeometry for MeshLodGeometry {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn render_object_name(&self) -> &str {
        &self.render_obj_name
    }

    fn bounding_center(&self) -> Vec3 {
        self.center
    }

    fn bounding_radius(&self) -> f32 {
        self.radius
    }

    fn triangle_count(&self) -> usize {
        self.triangle_count
    }

    fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    fn render(&self) {
        println!(
            "Rendering mesh LOD '{}' ({} triangles, {} vertices)",
            self.render_obj_name, self.triangle_count, self.vertex_count
        );
    }

    fn transform_nodes(&self) -> &[TransformNodeInstance] {
        &self.transform_nodes
    }

    fn snap_points(&self) -> &[Vec3] {
        &self.snap_points
    }
}

impl LodGeometry for CompositeMeshLodGeometry {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn render_object_name(&self) -> &str {
        ""
    }

    fn bounding_center(&self) -> Vec3 {
        self.center
    }

    fn bounding_radius(&self) -> f32 {
        self.radius
    }

    fn triangle_count(&self) -> usize {
        self.triangle_count
    }

    fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    fn transform_nodes(&self) -> &[TransformNodeInstance] {
        &self.transform_nodes
    }

    fn snap_points(&self) -> &[Vec3] {
        &self.snap_points
    }
}
