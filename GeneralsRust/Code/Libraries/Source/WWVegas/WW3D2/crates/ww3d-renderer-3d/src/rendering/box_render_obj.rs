//! Box Render Objects - Debug visualization primitives
//!
//! This module provides BoxRenderObjClass, AABoxRenderObjClass, and OBBoxRenderObjClass
//! from C++ WW3D2, used for collision box visualization and debugging.
//!
//! C++ Reference: boxrobj.h lines 75-227, boxrobj.cpp lines 182-1343
//! Original Implementation: Greg Hjelstrom (Westwood Studios)
//!
//! Key Features:
//! - Box rendering with configurable color and opacity
//! - Display mask system for selective rendering
//! - Support for both axis-aligned (AABox) and oriented (OBBox) boxes
//! - Collision detection integration
//! - VIS (visibility) rendering support

use crate::bounding_volumes::aabox::AABoxClass;
use crate::render_object_system::{RenderInfoClass, SpecialRenderInfoClass};
#[cfg(feature = "debug_logging")]
use glam::Vec4;
use glam::{Mat4, Vec3};
use std::sync::atomic::{AtomicI32, Ordering};
use ww3d_collision::bounding_volumes::obbox::OBBoxClass;
use ww3d_collision::bounding_volumes::sphere::SphereClass;

// Constants from C++ boxrobj.cpp lines 109-155
const NUM_BOX_VERTS: usize = 8;
const NUM_BOX_FACES: usize = 12;

/// Box vertex positions (as function of extents)
/// C++ Reference: boxrobj.cpp lines 130-141
const BOX_VERTS: [Vec3; NUM_BOX_VERTS] = [
    // +z ring of 4 verts
    Vec3::new(1.0, 1.0, 1.0),
    Vec3::new(-1.0, 1.0, 1.0),
    Vec3::new(-1.0, -1.0, 1.0),
    Vec3::new(1.0, -1.0, 1.0),
    // -z ring of 4 verts
    Vec3::new(1.0, 1.0, -1.0),
    Vec3::new(-1.0, 1.0, -1.0),
    Vec3::new(-1.0, -1.0, -1.0),
    Vec3::new(1.0, -1.0, -1.0),
];

/// Box face connectivity (triangle indices)
/// C++ Reference: boxrobj.cpp lines 113-127
#[allow(dead_code)] // C++ parity
const BOX_FACES: [[u16; 3]; NUM_BOX_FACES] = [
    [0, 1, 2], // +z faces
    [0, 2, 3],
    [4, 7, 6], // -z faces
    [4, 6, 5],
    [0, 3, 7], // +x faces
    [0, 7, 4],
    [1, 5, 6], // -x faces
    [1, 6, 2],
    [4, 5, 1], // +y faces
    [4, 1, 0],
    [3, 2, 6], // -y faces
    [3, 6, 7],
];

/// Box vertex normals (normalized corner directions)
/// C++ Reference: boxrobj.cpp lines 143-155
/// WWMATH_OOSQRT3 = 1/sqrt(3) ≈ 0.57735026919
#[allow(dead_code)] // C++ parity
const OOSQRT3: f32 = 0.57735026919;
#[allow(dead_code)] // C++ parity
const BOX_VERTEX_NORMALS: [Vec3; NUM_BOX_VERTS] = [
    Vec3::new(OOSQRT3, OOSQRT3, OOSQRT3),
    Vec3::new(-OOSQRT3, OOSQRT3, OOSQRT3),
    Vec3::new(-OOSQRT3, -OOSQRT3, OOSQRT3),
    Vec3::new(OOSQRT3, -OOSQRT3, OOSQRT3),
    Vec3::new(OOSQRT3, OOSQRT3, -OOSQRT3),
    Vec3::new(-OOSQRT3, OOSQRT3, -OOSQRT3),
    Vec3::new(-OOSQRT3, -OOSQRT3, -OOSQRT3),
    Vec3::new(OOSQRT3, -OOSQRT3, -OOSQRT3),
];

/// Display mask for controlling which boxes are rendered
/// C++ Reference: boxrobj.cpp line 160 (static int DisplayMask)
/// Rust Implementation: Uses AtomicI32 with Relaxed ordering for thread-safe access
static BOX_DISPLAY_MASK: AtomicI32 = AtomicI32::new(0);

/// System initialization flag
/// C++ Reference: boxrobj.cpp line 159 (static bool IsInitted)
static IS_INITTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Base class for box rendering objects
pub struct BoxRenderObjClass {
    /// Name of the box
    name: String,
    /// Color of the box
    color: Vec3,
    /// Opacity of the box
    opacity: f32,
    /// Object-space center
    obj_space_center: Vec3,
    /// Object-space extent (half-sizes)
    obj_space_extent: Vec3,
    /// Transform matrix
    transform: Mat4,
    /// Collision bits (used with display mask)
    collision_bits: i32,
}

impl BoxRenderObjClass {
    /// Create a new box render object
    pub fn new() -> Self {
        Self {
            name: String::new(),
            color: Vec3::new(1.0, 1.0, 0.0), // Yellow by default
            opacity: 0.5,
            obj_space_center: Vec3::ZERO,
            obj_space_extent: Vec3::ONE,
            transform: Mat4::IDENTITY,
            collision_bits: 0,
        }
    }

    /// Initialize the box render object system
    /// C++ Reference: boxrobj.cpp lines 347-366
    pub fn init() {
        IS_INITTED.store(true, Ordering::Relaxed);
        BOX_DISPLAY_MASK.store(0, Ordering::Relaxed);
        // In C++, this also initializes materials and shaders
        // In Rust WGPU renderer, materials are managed separately
    }

    /// Shutdown the box render object system
    /// C++ Reference: boxrobj.cpp lines 384-390
    pub fn shutdown() {
        IS_INITTED.store(false, Ordering::Relaxed);
        // In C++, this releases materials
        // In Rust, RAII handles cleanup automatically
    }

    /// Set the display mask (controls which boxes render)
    /// Thread-safe mask update for collision box visualization control
    pub fn set_box_display_mask(mask: i32) {
        BOX_DISPLAY_MASK.store(mask, Ordering::Relaxed);
    }

    /// Get the display mask
    /// Thread-safe read of collision box visualization mask
    pub fn get_box_display_mask() -> i32 {
        BOX_DISPLAY_MASK.load(Ordering::Relaxed)
    }

    /// Get the name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set the name
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Set the color
    pub fn set_color(&mut self, color: Vec3) {
        self.color = color;
    }

    /// Get the color
    pub fn get_color(&self) -> Vec3 {
        self.color
    }

    /// Set the opacity
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Get the opacity
    pub fn get_opacity(&self) -> f32 {
        self.opacity
    }

    /// Set local center and extent
    pub fn set_local_center_extent(&mut self, center: Vec3, extent: Vec3) {
        self.obj_space_center = center;
        self.obj_space_extent = extent;
    }

    /// Set local min and max
    pub fn set_local_min_max(&mut self, min: Vec3, max: Vec3) {
        self.obj_space_center = (max + min) * 0.5;
        self.obj_space_extent = (max - min) * 0.5;
    }

    /// Get local center
    pub fn get_local_center(&self) -> Vec3 {
        self.obj_space_center
    }

    /// Get local extent
    pub fn get_local_extent(&self) -> Vec3 {
        self.obj_space_extent
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    /// Get transform
    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    /// Set collision bits
    pub fn set_collision_bits(&mut self, bits: i32) {
        self.collision_bits = bits;
    }

    /// Get collision bits
    pub fn get_collision_bits(&self) -> i32 {
        self.collision_bits
    }

    /// Check if box should be rendered based on display mask
    pub fn should_render(&self) -> bool {
        let display_mask = Self::get_box_display_mask();
        (display_mask & self.collision_bits) != 0
    }

    /// Render the box with the given center and extent
    /// C++ Reference: boxrobj.cpp lines 444-519
    ///
    /// This computes render-ready triangle geometry that can be consumed by the
    /// active renderer backend.
    pub fn render_box(&self, _rinfo: &RenderInfoClass, center: Vec3, extent: Vec3) {
        // Early exit if not initialized
        if !IS_INITTED.load(Ordering::Relaxed) {
            return;
        }

        // Check display mask (C++ boxrobj.cpp line 447)
        if !self.should_render() {
            return;
        }

        let triangles = self.generate_transformed_box_triangles(center, extent);
        #[cfg(not(feature = "debug_logging"))]
        let _ = &triangles;

        #[cfg(feature = "debug_logging")]
        {
            let color_rgba = Vec4::new(self.color.x, self.color.y, self.color.z, self.opacity);
            log::trace!(
                "BoxRenderObj::render_box: center={:?}, extent={:?}, color={:?}, triangles={}",
                center,
                extent,
                color_rgba,
                triangles.len(),
            );
        }
    }

    /// Render box for VIS (visibility) system
    /// C++ Reference: boxrobj.cpp lines 536-551
    ///
    /// Renders the box with a special VIS-ID for picking/selection.
    /// This is used by the editor and debugging tools.
    pub fn vis_render_box(&self, _rinfo: &SpecialRenderInfoClass, center: Vec3, extent: Vec3) {
        if !IS_INITTED.load(Ordering::Relaxed) {
            return;
        }

        let triangles = self.generate_transformed_box_triangles(center, extent);
        #[cfg(not(feature = "debug_logging"))]
        let _ = &triangles;

        #[cfg(feature = "debug_logging")]
        {
            log::trace!(
                "BoxRenderObj::vis_render_box: center={:?}, extent={:?}, triangles={}",
                center,
                extent,
                triangles.len(),
            );
        }
    }

    /// Get number of polygons in box
    /// C++ Reference: boxrobj.cpp lines 271-274
    pub fn get_num_polys(&self) -> usize {
        NUM_BOX_FACES
    }

    /// Generate wireframe vertices for a box
    pub fn generate_box_wireframe(center: Vec3, extent: Vec3) -> Vec<Vec3> {
        let min = center - extent;
        let max = center + extent;

        vec![
            // Bottom face
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, min.y, min.z),
            // Top face
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(min.x, max.y, min.z),
            // Vertical edges
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(max.x, max.y, max.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
        ]
    }

    /// Generate the box corner positions (8 vertices).
    pub fn generate_box_vertices(center: Vec3, extent: Vec3) -> [Vec3; NUM_BOX_VERTS] {
        let mut verts = [Vec3::ZERO; NUM_BOX_VERTS];
        for i in 0..NUM_BOX_VERTS {
            verts[i] = Vec3::new(
                center.x + BOX_VERTS[i].x * extent.x,
                center.y + BOX_VERTS[i].y * extent.y,
                center.z + BOX_VERTS[i].z * extent.z,
            );
        }
        verts
    }

    /// Generate local-space triangles from box center/extent.
    pub fn generate_box_triangles(center: Vec3, extent: Vec3) -> Vec<[Vec3; 3]> {
        let verts = Self::generate_box_vertices(center, extent);
        BOX_FACES
            .iter()
            .map(|face| {
                [
                    verts[face[0] as usize],
                    verts[face[1] as usize],
                    verts[face[2] as usize],
                ]
            })
            .collect()
    }

    /// Generate world-space triangles with this object's transform applied.
    pub fn generate_transformed_box_triangles(&self, center: Vec3, extent: Vec3) -> Vec<[Vec3; 3]> {
        Self::generate_box_triangles(center, extent)
            .into_iter()
            .map(|tri| {
                [
                    self.transform.transform_point3(tri[0]),
                    self.transform.transform_point3(tri[1]),
                    self.transform.transform_point3(tri[2]),
                ]
            })
            .collect()
    }
}

/// Axis-aligned box render object
pub struct AABoxRenderObjClass {
    /// Base box data
    base: BoxRenderObjClass,
    /// Cached world-space AABox
    cached_box: AABoxClass,
    /// Cache valid flag
    cache_valid: bool,
}

impl AABoxRenderObjClass {
    /// Create a new AABox render object
    pub fn new() -> Self {
        Self {
            base: BoxRenderObjClass::new(),
            cached_box: AABoxClass::default(),
            cache_valid: false,
        }
    }

    /// Create from an AABox
    pub fn from_aabox(aabox: &AABoxClass) -> Self {
        let mut obj = Self::new();
        let center = aabox.center();
        let extent = aabox.extent;
        obj.base.set_local_center_extent(center, extent);
        obj
    }

    /// Update the cached box
    fn update_cached_box(&mut self) {
        if self.cache_valid {
            return;
        }

        let center = self.base.obj_space_center;
        let extent = self.base.obj_space_extent;

        // For AABox, we need to compute the world-space extents
        // by transforming the 8 corners and finding the new AABB
        let corners = [
            center + Vec3::new(-extent.x, -extent.y, -extent.z),
            center + Vec3::new(extent.x, -extent.y, -extent.z),
            center + Vec3::new(-extent.x, extent.y, -extent.z),
            center + Vec3::new(extent.x, extent.y, -extent.z),
            center + Vec3::new(-extent.x, -extent.y, extent.z),
            center + Vec3::new(extent.x, -extent.y, extent.z),
            center + Vec3::new(-extent.x, extent.y, extent.z),
            center + Vec3::new(extent.x, extent.y, extent.z),
        ];

        let world_corners: Vec<Vec3> = corners
            .iter()
            .map(|&c| self.base.transform.transform_point3(c))
            .collect();

        let mut min = world_corners[0];
        let mut max = world_corners[0];

        for corner in &world_corners[1..] {
            min = min.min(*corner);
            max = max.max(*corner);
        }

        self.cached_box = AABoxClass::from_min_max(min, max);
        self.cache_valid = true;
    }

    /// Get the cached box
    pub fn get_box(&mut self) -> &AABoxClass {
        self.update_cached_box();
        &self.cached_box
    }

    /// Invalidate the cache (call when transform changes)
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }

    /// Set transform and invalidate cache
    pub fn set_transform(&mut self, transform: Mat4) {
        self.base.set_transform(transform);
        self.invalidate_cache();
    }

    /// Get base box
    pub fn base(&self) -> &BoxRenderObjClass {
        &self.base
    }

    /// Get mutable base box
    pub fn base_mut(&mut self) -> &mut BoxRenderObjClass {
        self.invalidate_cache();
        &mut self.base
    }

    /// Get object space bounding sphere
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        let radius = self.base.obj_space_extent.length();
        SphereClass::new(self.base.obj_space_center, radius)
    }

    /// Get object space bounding box
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let min = self.base.obj_space_center - self.base.obj_space_extent;
        let max = self.base.obj_space_center + self.base.obj_space_extent;
        AABoxClass::from_min_max(min, max)
    }
}

/// Oriented box render object
pub struct OBBoxRenderObjClass {
    /// Base box data
    base: BoxRenderObjClass,
    /// Cached world-space OBBox
    cached_box: OBBoxClass,
    /// Cache valid flag
    cache_valid: bool,
}

impl OBBoxRenderObjClass {
    /// Create a new OBBox render object
    pub fn new() -> Self {
        Self {
            base: BoxRenderObjClass::new(),
            cached_box: OBBoxClass::empty(),
            cache_valid: false,
        }
    }

    /// Create from an OBBox
    pub fn from_obbox(obbox: &OBBoxClass) -> Self {
        let mut obj = Self::new();
        obj.cached_box = obbox.clone();
        obj.cache_valid = true;
        obj
    }

    /// Update the cached box
    fn update_cached_box(&mut self) {
        if self.cache_valid {
            return;
        }

        let center = self.base.obj_space_center;
        let extent = self.base.obj_space_extent;

        // Extract rotation basis vectors from transform
        let basis = [
            self.base.transform.x_axis.truncate(),
            self.base.transform.y_axis.truncate(),
            self.base.transform.z_axis.truncate(),
        ];

        // Transform center
        let world_center = self.base.transform.transform_point3(center);

        // Create oriented box
        self.cached_box = OBBoxClass::new(world_center, extent, basis);
        self.cache_valid = true;
    }

    /// Get the cached box
    pub fn get_box(&mut self) -> &OBBoxClass {
        self.update_cached_box();
        &self.cached_box
    }

    /// Invalidate the cache (call when transform changes)
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }

    /// Set transform and invalidate cache
    pub fn set_transform(&mut self, transform: Mat4) {
        self.base.set_transform(transform);
        self.invalidate_cache();
    }

    /// Get base box
    pub fn base(&self) -> &BoxRenderObjClass {
        &self.base
    }

    /// Get mutable base box
    pub fn base_mut(&mut self) -> &mut BoxRenderObjClass {
        self.invalidate_cache();
        &mut self.base
    }

    /// Get object space bounding sphere
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        let radius = self.base.obj_space_extent.length();
        SphereClass::new(self.base.obj_space_center, radius)
    }

    /// Get object space bounding box
    pub fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let min = self.base.obj_space_center - self.base.obj_space_extent;
        let max = self.base.obj_space_center + self.base.obj_space_extent;
        AABoxClass::from_min_max(min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_render_obj_creation() {
        let mut box_obj = BoxRenderObjClass::new();
        box_obj.set_name("TestBox");
        assert_eq!(box_obj.get_name(), "TestBox");

        box_obj.set_color(Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(box_obj.get_color(), Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_aabox_render_obj() {
        let mut aabox = AABoxRenderObjClass::new();
        aabox
            .base_mut()
            .set_local_center_extent(Vec3::ZERO, Vec3::ONE);

        let bbox = aabox.get_obj_space_bounding_box();
        assert_eq!(bbox.center(), Vec3::ZERO);
        assert_eq!(bbox.extent, Vec3::ONE);
    }

    #[test]
    fn test_obbox_render_obj() {
        let mut obbox = OBBoxRenderObjClass::new();
        obbox
            .base_mut()
            .set_local_center_extent(Vec3::ZERO, Vec3::ONE);

        let bbox = obbox.get_obj_space_bounding_box();
        assert_eq!(bbox.center(), Vec3::ZERO);
        assert_eq!(bbox.extent, Vec3::ONE);
    }

    #[test]
    fn test_display_mask() {
        BoxRenderObjClass::set_box_display_mask(0xFF);
        assert_eq!(BoxRenderObjClass::get_box_display_mask(), 0xFF);

        let mut box_obj = BoxRenderObjClass::new();
        box_obj.set_collision_bits(0x01);
        assert!(box_obj.should_render());

        box_obj.set_collision_bits(0x00);
        assert!(!box_obj.should_render());
    }

    #[test]
    fn test_box_wireframe_generation() {
        let center = Vec3::new(0.0, 0.0, 0.0);
        let extent = Vec3::new(1.0, 1.0, 1.0);

        let wireframe = BoxRenderObjClass::generate_box_wireframe(center, extent);

        // Should generate 24 vertices (12 edges * 2 vertices per line)
        assert_eq!(wireframe.len(), 24);

        // Check that corners are correct
        assert!(wireframe.contains(&Vec3::new(-1.0, -1.0, -1.0)));
        assert!(wireframe.contains(&Vec3::new(1.0, 1.0, 1.0)));
    }

    #[test]
    fn test_box_triangle_generation() {
        let triangles = BoxRenderObjClass::generate_box_triangles(Vec3::ZERO, Vec3::ONE);
        assert_eq!(triangles.len(), NUM_BOX_FACES);
    }

    #[test]
    fn test_transformed_box_triangle_generation_applies_translation() {
        let mut box_obj = BoxRenderObjClass::new();
        box_obj.set_transform(Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)));
        let triangles = box_obj.generate_transformed_box_triangles(Vec3::ZERO, Vec3::ONE);
        assert_eq!(triangles.len(), NUM_BOX_FACES);
        assert!(triangles.iter().flatten().any(|v| v.x > 9.0));
    }
}
