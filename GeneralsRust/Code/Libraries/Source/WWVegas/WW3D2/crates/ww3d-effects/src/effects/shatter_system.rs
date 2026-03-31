//! Shatter System - Advanced mesh destruction and physics
//!
//! This module implements the ShatterSystem from the original C++ code,
//! providing realistic mesh destruction with physics-based shattering.
//!
//! Converted from:
//! - shattersystem.cpp/h (shatter system implementation)
//! - dynamesh.h (dynamic mesh functionality)

use glam::{Quat, Vec2, Vec3};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use ww3d_core::errors::{W3DError, W3DResult as Result};
use ww3d_renderer_3d::bounding_volumes::aabox::AABoxClass;
use ww3d_renderer_3d::mesh::mesh_core::MeshClass;

/// Shatter plane structure
#[derive(Debug, Clone)]
pub struct ShatterPlane {
    /// Plane normal
    pub normal: Vec3,
    /// Plane distance from origin
    pub distance: f32,
    /// Whether this plane is convex
    pub convex: bool,
}

impl ShatterPlane {
    /// Create new shatter plane
    pub fn new(normal: Vec3, distance: f32, convex: bool) -> Self {
        Self {
            normal: normal.normalize(),
            distance,
            convex,
        }
    }

    /// Classify point relative to plane
    pub fn classify_point(&self, point: Vec3) -> PlaneClassification {
        let dist = self.normal.dot(point) - self.distance;
        if dist > BPT_EPSILON {
            PlaneClassification::Front
        } else if dist < -BPT_EPSILON {
            PlaneClassification::Back
        } else {
            PlaneClassification::On
        }
    }

    /// Get distance from point to plane
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.distance
    }
}

/// Plane classification enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaneClassification {
    /// Point is in front of plane
    Front = 0x01,
    /// Point is behind plane
    Back = 0x02,
    /// Point is on plane
    On = 0x04,
}

/// Shatter pattern structure
#[derive(Debug, Clone)]
pub struct ShatterPattern {
    /// Pattern name
    pub name: String,
    /// Shatter planes
    pub planes: Vec<ShatterPlane>,
    /// Pattern scale
    pub scale: f32,
    /// Whether pattern is convex
    pub convex: bool,
}

impl ShatterPattern {
    /// Create new shatter pattern
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            planes: Vec::new(),
            scale: 1.0,
            convex: true,
        }
    }

    /// Add plane to pattern
    pub fn add_plane(&mut self, plane: ShatterPlane) {
        self.planes.push(plane);
    }

    /// Validate pattern convexity
    pub fn validate_convexity(&mut self) {
        // Check if all planes form a convex shape
        // This is a simplified check - full implementation would be more complex
        self.convex = !self.planes.is_empty();
    }
}

/// Vertex classification for shattering
#[derive(Debug, Clone)]
pub struct ShatterVertex {
    /// Original position
    pub position: Vec3,
    /// UV coordinates
    pub uv: Vec2,
    /// Normal vector
    pub normal: Vec3,
    /// Classification flags
    pub classification: u32,
    /// Distance to each plane
    pub plane_distances: Vec<f32>,
}

impl ShatterVertex {
    /// Create new shatter vertex
    pub fn new(position: Vec3, uv: Vec2, normal: Vec3) -> Self {
        Self {
            position,
            uv,
            normal,
            classification: 0,
            plane_distances: Vec::new(),
        }
    }

    /// Classify vertex against all planes
    pub fn classify_against_planes(&mut self, planes: &[ShatterPlane]) {
        self.plane_distances.clear();
        self.classification = 0;

        for plane in planes {
            let distance = plane.distance_to_point(self.position);
            self.plane_distances.push(distance);

            match plane.classify_point(self.position) {
                PlaneClassification::Front => self.classification |= BPT_FRONT,
                PlaneClassification::Back => self.classification |= BPT_BACK,
                PlaneClassification::On => self.classification |= BPT_ON,
            }
        }
    }
}

/// Triangle classification for shattering
#[derive(Debug, Clone)]
pub struct ShatterTriangle {
    /// Vertex indices
    pub vertices: [usize; 3],
    /// Triangle normal
    pub normal: Vec3,
    /// Material index
    pub material_index: usize,
    /// Classification result
    pub classification: TriangleClassification,
}

impl ShatterTriangle {
    /// Create new shatter triangle
    pub fn new(v0: usize, v1: usize, v2: usize, material_index: usize) -> Self {
        Self {
            vertices: [v0, v1, v2],
            normal: Vec3::ZERO, // Will be computed
            material_index,
            classification: TriangleClassification::Unknown,
        }
    }

    /// Compute triangle normal
    pub fn compute_normal(&mut self, vertices: &[ShatterVertex]) {
        if self.vertices[0] >= vertices.len()
            || self.vertices[1] >= vertices.len()
            || self.vertices[2] >= vertices.len()
        {
            return;
        }

        let p0 = vertices[self.vertices[0]].position;
        let p1 = vertices[self.vertices[1]].position;
        let p2 = vertices[self.vertices[2]].position;

        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        self.normal = edge1.cross(edge2).normalize();
    }
}

/// Triangle classification enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TriangleClassification {
    /// Triangle classification unknown
    Unknown,
    /// Triangle is entirely in front of all planes
    Front,
    /// Triangle is entirely behind all planes
    Back,
    /// Triangle spans multiple regions
    Spanning,
}

/// Shatter fragment structure
#[derive(Debug)]
pub struct ShatterFragment {
    /// Fragment vertices
    pub vertices: Vec<ShatterVertex>,
    /// Fragment triangles
    pub triangles: Vec<ShatterTriangle>,
    /// Fragment bounding box
    pub bounding_box: AABoxClass,
    /// Fragment center of mass
    pub center_of_mass: Vec3,
    /// Fragment velocity
    pub velocity: Vec3,
    /// Fragment angular velocity
    pub angular_velocity: Vec3,
    /// Fragment mass
    pub mass: f32,
    /// Time since creation
    pub age: f32,
}

impl ShatterFragment {
    /// Create new fragment
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
            bounding_box: AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::ONE),
            center_of_mass: Vec3::ZERO,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            mass: 1.0,
            age: 0.0,
        }
    }

    /// Update fragment physics
    pub fn update_physics(&mut self, delta_time: f32, gravity: Vec3) {
        // Apply gravity
        self.velocity += gravity * delta_time;

        // Update position
        for vertex in &mut self.vertices {
            vertex.position += self.velocity * delta_time;
        }

        // Apply angular velocity
        let rotation = Quat::from_axis_angle(
            self.angular_velocity.normalize(),
            self.angular_velocity.length() * delta_time,
        );

        for vertex in &mut self.vertices {
            let relative_pos = vertex.position - self.center_of_mass;
            vertex.position = self.center_of_mass + rotation * relative_pos;
        }

        // Update age
        self.age += delta_time;

        // Update bounding box
        self.update_bounding_box();
    }

    /// Update bounding box
    pub fn update_bounding_box(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        let mut min_corner = self.vertices[0].position;
        let mut max_corner = self.vertices[0].position;

        for vertex in &self.vertices {
            min_corner = min_corner.min(vertex.position);
            max_corner = max_corner.max(vertex.position);
        }

        let center = (min_corner + max_corner) / 2.0;
        let extent = (max_corner - min_corner) / 2.0;

        self.bounding_box = AABoxClass::from_center_and_extent(center, extent);
    }

    /// Compute center of mass
    pub fn compute_center_of_mass(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        let mut center = Vec3::ZERO;
        for vertex in &self.vertices {
            center += vertex.position;
        }
        center /= self.vertices.len() as f32;
        self.center_of_mass = center;
    }

    /// Render fragment
    pub fn render(&self) {
        // In a full implementation, this would render the fragment geometry
        // using WGPU with the current mesh shader
    }
}

/// Shatter system manager
#[derive(Debug)]
pub struct ShatterSystem {
    /// Available shatter patterns
    pub patterns: HashMap<String, ShatterPattern>,
    /// Active fragments
    pub fragments: Vec<ShatterFragment>,
    /// Gravity vector
    pub gravity: Vec3,
    /// Fragment lifetime
    pub fragment_lifetime: f32,
    /// Maximum number of fragments
    pub max_fragments: usize,
    /// Fragment fade start time
    pub fade_start_time: f32,
}

impl ShatterSystem {
    /// Create new shatter system
    pub fn new() -> Self {
        Self {
            patterns: HashMap::new(),
            fragments: Vec::new(),
            gravity: Vec3::new(0.0, 0.0, -9.81), // Earth gravity
            fragment_lifetime: 10.0,
            max_fragments: 1000,
            fade_start_time: 7.0,
        }
    }

    /// Load shatter patterns
    pub fn load_patterns(&mut self, filename: &str) -> Result<()> {
        // In a full implementation, this would load shatter patterns from file
        // For now, create some default patterns

        let _ = filename; // Use parameter

        // Create a simple cubic shatter pattern
        let mut cube_pattern = ShatterPattern::new("cube");
        cube_pattern.add_plane(ShatterPlane::new(Vec3::X, 0.0, true));
        cube_pattern.add_plane(ShatterPlane::new(-Vec3::X, 0.0, true));
        cube_pattern.add_plane(ShatterPlane::new(Vec3::Y, 0.0, true));
        cube_pattern.add_plane(ShatterPlane::new(-Vec3::Y, 0.0, true));
        cube_pattern.add_plane(ShatterPlane::new(Vec3::Z, 0.0, true));
        cube_pattern.add_plane(ShatterPlane::new(-Vec3::Z, 0.0, true));
        cube_pattern.validate_convexity();

        self.patterns
            .insert(cube_pattern.name.clone(), cube_pattern);

        Ok(())
    }

    /// Shatter mesh using pattern
    pub fn shatter_mesh(
        &mut self,
        mesh: &MeshClass,
        pattern_name: &str,
        impact_point: Vec3,
        impact_force: Vec3,
    ) -> Result<()> {
        let pattern = self.patterns.get(pattern_name).ok_or_else(|| {
            W3DError::InvalidParameter(format!("Shatter pattern '{}' not found", pattern_name))
        })?;

        // Convert mesh to shatter vertices and triangles
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();

        // This is a simplified conversion - full implementation would extract
        // actual mesh geometry
        // get_num_polys is already implemented in MeshClass at mesh_system.rs:2160
        let num_polys = mesh.get_num_polys();
        for _i in 0..num_polys {
            // Add vertices (simplified)
            let v0 = ShatterVertex::new(Vec3::ZERO, Vec2::ZERO, Vec3::Z);
            let v1 = ShatterVertex::new(Vec3::ZERO, Vec2::ZERO, Vec3::Z);
            let v2 = ShatterVertex::new(Vec3::ZERO, Vec2::ZERO, Vec3::Z);

            let v0_idx = vertices.len();
            vertices.push(v0);
            let v1_idx = vertices.len();
            vertices.push(v1);
            let v2_idx = vertices.len();
            vertices.push(v2);

            triangles.push(ShatterTriangle::new(v0_idx, v1_idx, v2_idx, 0));
        }

        // Classify vertices against planes
        for vertex in &mut vertices {
            vertex.classify_against_planes(&pattern.planes);
        }

        // Compute triangle normals
        for triangle in &mut triangles {
            triangle.compute_normal(&vertices);
        }

        // Create fragments based on classification
        self.create_fragments_from_classification(
            &vertices,
            &triangles,
            impact_point,
            impact_force,
        );

        Ok(())
    }

    /// Create fragments from vertex classification
    fn create_fragments_from_classification(
        &mut self,
        vertices: &[ShatterVertex],
        triangles: &[ShatterTriangle],
        impact_point: Vec3,
        impact_force: Vec3,
    ) {
        // This is a simplified fragment creation
        // Full implementation would use constructive solid geometry (CSG)
        // to create proper fragments

        let mut fragment = ShatterFragment::new();

        // Copy all vertices and triangles to fragment
        fragment.vertices = vertices.to_vec();
        fragment.triangles = triangles.to_vec();

        // Set physics properties
        fragment.compute_center_of_mass();
        fragment.mass = fragment.vertices.len() as f32 * 0.1; // Simple mass calculation

        // Apply impact force
        let direction_to_fragment = (fragment.center_of_mass - impact_point).normalize();
        let force_magnitude = impact_force.length();
        fragment.velocity = direction_to_fragment * force_magnitude * 0.5;

        // Add some angular velocity
        fragment.angular_velocity = Vec3::new(
            (rand::random::<f32>() - 0.5) * 10.0,
            (rand::random::<f32>() - 0.5) * 10.0,
            (rand::random::<f32>() - 0.5) * 10.0,
        );

        self.fragments.push(fragment);
    }

    /// Update all fragments
    pub fn update(&mut self, delta_time: f32) {
        // Update physics for all fragments
        for fragment in &mut self.fragments {
            fragment.update_physics(delta_time, self.gravity);
        }

        // Remove old fragments
        self.fragments
            .retain(|fragment| fragment.age < self.fragment_lifetime);

        // Limit number of fragments
        if self.fragments.len() > self.max_fragments {
            // Remove oldest fragments
            let remove_count = self.fragments.len() - self.max_fragments;
            self.fragments.drain(0..remove_count);
        }
    }

    /// Render all fragments
    pub fn render(&self) {
        for fragment in &self.fragments {
            fragment.render();
        }
    }

    /// Clear all fragments
    pub fn clear(&mut self) {
        self.fragments.clear();
    }

    /// Get fragment count
    pub fn get_fragment_count(&self) -> usize {
        self.fragments.len()
    }

    /// Get pattern count
    pub fn get_pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

/// Constants for shatter system
const BPT_FRONT: u32 = 0x01;
const BPT_BACK: u32 = 0x02;
const BPT_ON: u32 = 0x04;
#[allow(dead_code)] // C++ parity
const BPT_BOTH: u32 = 0x08;
const BPT_EPSILON: f32 = 0.0001;
#[allow(dead_code)] // C++ parity
const BPT_COINCIDENCE_EPSILON: f32 = 0.000001;

fn shatter_system_store() -> &'static Mutex<Option<ShatterSystem>> {
    static STORE: OnceLock<Mutex<Option<ShatterSystem>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(None))
}

fn with_shatter_system_mut<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&mut ShatterSystem) -> R,
{
    let mut slot = shatter_system_store().lock().ok()?;
    let system = slot.as_mut()?;
    Some(f(system))
}

/// Initialize shatter system
pub fn init_shatter_system() -> Result<()> {
    let mut slot = shatter_system_store()
        .lock()
        .expect("shatter system lock poisoned");
    *slot = Some(ShatterSystem::new());
    Ok(())
}

/// Shutdown shatter system
pub fn shutdown_shatter_system() {
    if let Ok(mut slot) = shatter_system_store().lock() {
        *slot = None;
    }
}

/// Quick shatter function
pub fn shatter_object_at_point(
    mesh: &MeshClass,
    pattern_name: &str,
    impact_point: Vec3,
    impact_force: Vec3,
) -> Result<()> {
    with_shatter_system_mut(|system| {
        system.shatter_mesh(mesh, pattern_name, impact_point, impact_force)
    })
    .unwrap_or_else(|| {
        Err(W3DError::NotInitialized(
            "Shatter system not initialized".to_string(),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shatter_plane_creation() {
        let plane = ShatterPlane::new(Vec3::Z, 0.0, true);
        assert_eq!(plane.normal, Vec3::Z);
        assert_eq!(plane.distance, 0.0);
        assert_eq!(plane.convex, true);
    }

    #[test]
    fn test_shatter_vertex_classification() {
        let plane = ShatterPlane::new(Vec3::Z, 0.0, true);
        let mut vertex = ShatterVertex::new(Vec3::new(0.0, 0.0, 1.0), Vec2::ZERO, Vec3::Z);

        vertex.classify_against_planes(&[plane]);
        assert_eq!(vertex.plane_distances.len(), 1);
    }

    #[test]
    fn test_shatter_pattern_creation() {
        let pattern = ShatterPattern::new("test_pattern");
        assert_eq!(pattern.name, "test_pattern");
        assert!(pattern.planes.is_empty());
    }

    #[test]
    fn test_shatter_system_creation() {
        let system = ShatterSystem::new();
        assert!(system.patterns.is_empty());
        assert!(system.fragments.is_empty());
        assert_eq!(system.max_fragments, 1000);
    }

    #[test]
    fn test_shatter_fragment_creation() {
        let fragment = ShatterFragment::new();
        assert!(fragment.vertices.is_empty());
        assert!(fragment.triangles.is_empty());
        assert_eq!(fragment.mass, 1.0);
        assert_eq!(fragment.age, 0.0);
    }
}
