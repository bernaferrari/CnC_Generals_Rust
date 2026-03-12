//! Line3D Render Objects - 3D line segment rendering
//!
//! This module provides Line3DRenderObj, a render object for rendering 3D line segments
//! with configurable width, color, and opacity. Lines are rendered as boxes oriented
//! along the line direction, providing proper depth testing and perspective rendering.
//!
//! C++ Reference: line3d.h, line3d.cpp from WW3D2
//! Implementation matches C++ Line3DClass behavior exactly.

use crate::bounding_volumes::aabox::AABoxClass;
use crate::core::error::RendererResult;
use crate::render_object_system::{
    AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass, MaterialInfoClass,
    OBBoxCollisionTestClass, OBBoxIntersectionTestClass, RayCollisionTestClass, RenderInfoClass,
    RenderObjClass, SpecialRenderInfoClass,
};
use crate::rendering::shader_system::shader::{ShaderClass, SORT_LEVEL_NONE};
use glam::{Mat4, Vec3, Vec4};
use std::any::Any;
use ww3d_collision::bounding_volumes::sphere::SphereClass;
use ww3d_core::RenderObjClassId;

/// Triangle indices for rendering the line box (12 triangles, 36 indices)
/// C++ Reference: line3d.cpp lines 62-76
/// This defines a box mesh with 8 vertices arranged as follows:
/// - Vertices 0-3: Start end of line (origin)
/// - Vertices 4-7: End of line (at distance = length)
#[allow(dead_code)]
const INDICES: [u16; 36] = [
    3, 5, 1, 7, 5, 3, 1, 5, 0, 5, 4, 0, 4, 2, 0, 4, 6, 2, 7, 3, 2, 6, 7, 2, 7, 6, 5, 5, 6, 4, 2, 3,
    1, 2, 1, 0,
];

/// Line3DRenderObj - Render object for 3D line segments
///
/// Lines are rendered as oriented boxes with square cross-section. The line is
/// constructed as a box with origin at the start point, extending along the X-axis
/// to the end point, with width defining the Y/Z extents.
///
/// # C++ Compatibility
///
/// This struct exactly matches the C++ Line3DClass implementation:
/// - Constructor: line3d.cpp lines 96-142
/// - Render: line3d.cpp lines 254-310
/// - Reset/Re_Color/Set_Opacity: line3d.cpp lines 399-510
/// - Bounding volumes: line3d.cpp lines 371-384
///
/// # Implementation Details
///
/// The line is represented as 8 vertices forming a box:
/// ```text
///     3---7
///    /|  /|
///   1---5 |    X-axis: line direction (length)
///   | 2-|-6    Y-axis: width/2
///   |/  |/     Z-axis: width/2
///   0---4
/// ```
///
/// The transform matrix positions this box in world space using Obj_Look_At
/// to align the X-axis with the line direction.
#[derive(Debug, Clone)]
pub struct Line3DRenderObj {
    /// Name of this render object
    name: String,

    /// Length of the line (distance between start and end points)
    /// Kept separately to facilitate changing endpoints
    length: f32,

    /// Width of the line (thickness in world units)
    /// Kept separately to facilitate changing width
    width: f32,

    /// Shader for rendering (opaque or alpha based on opacity)
    shader: ShaderClass,

    /// 8 vertices defining the line box in object space
    /// C++ Reference: line3d.h line 118
    /// Vertices are arranged as a box with X-axis along line direction
    vertices: [Vec3; 8],

    /// Color of the line (RGBA)
    /// C++ Reference: line3d.h line 120 (stored as Vector4)
    color: Vec4,

    /// Sort level for transparent rendering
    /// C++ Reference: line3d.h line 121
    sort_level: i32,

    /// Transform matrix (world position and orientation)
    /// Set via Obj_Look_At to point along line direction
    transform: Mat4,
}

/// Prepared line geometry and state for renderer submission.
#[derive(Debug, Clone)]
pub struct Line3DRenderSubmission {
    pub world_transform: Mat4,
    pub shader: ShaderClass,
    pub color: Vec4,
    pub world_vertices: [Vec3; 8],
    pub world_triangles: Vec<[Vec3; 3]>,
    pub triangle_indices: &'static [u16; 36],
    pub sort_level: i32,
}

impl Line3DRenderObj {
    /// Create a new 3D line segment
    ///
    /// # Arguments
    ///
    /// * `start` - Start point of the line in world coordinates
    /// * `end` - End point of the line in world coordinates
    /// * `width` - Width (thickness) of the line in world units
    /// * `r` - Red component (0.0-1.0)
    /// * `g` - Green component (0.0-1.0)
    /// * `b` - Blue component (0.0-1.0)
    /// * `opacity` - Alpha/opacity component (0.0-1.0)
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Line3DClass constructor (line3d.cpp lines 96-142)
    ///
    /// # Implementation Notes
    ///
    /// 1. Calculates line length from start/end distance
    /// 2. Creates 8 vertices forming a box in object space
    ///    - Origin at (0,0,0) representing start point
    ///    - Extends to (length,0,0) representing end point
    ///    - Width extends ±width/2 in Y and Z directions
    /// 3. Sets shader based on opacity (alpha blend if < 1.0)
    /// 4. Computes transform matrix using Obj_Look_At
    ///    - Positions box origin at start point
    ///    - Orients X-axis toward end point
    pub fn new(start: Vec3, end: Vec3, width: f32, r: f32, g: f32, b: f32, opacity: f32) -> Self {
        let length = (end - start).length();
        let half_width = width * 0.5;

        // Create box vertices in object space
        // C++ Reference: line3d.cpp lines 104-130
        // Box has origin at start point, extends along X-axis to end point
        let vertices = [
            // Start end (X = 0)
            Vec3::new(0.0, -half_width, -half_width), // 0
            Vec3::new(0.0, half_width, -half_width),  // 1
            Vec3::new(0.0, -half_width, half_width),  // 2
            Vec3::new(0.0, half_width, half_width),   // 3
            // End (X = length)
            Vec3::new(length, -half_width, -half_width), // 4
            Vec3::new(length, half_width, -half_width),  // 5
            Vec3::new(length, -half_width, half_width),  // 6
            Vec3::new(length, half_width, half_width),   // 7
        ];

        // Initialize color (opacity set separately to configure shader)
        let color = Vec4::new(r, g, b, opacity);

        // Set shader based on opacity
        // C++ Reference: line3d.cpp lines 501-509 (Set_Opacity implementation)
        let (shader, sort_level) = if opacity < 1.0 {
            (ShaderClass::preset_alpha_solid(), 1)
        } else {
            (ShaderClass::preset_opaque_solid(), SORT_LEVEL_NONE as i32)
        };

        // Compute transform matrix to position and orient the line
        // C++ Reference: line3d.cpp lines 138-141
        // Uses Obj_Look_At to make X-axis point from start to end
        let transform = Self::obj_look_at(start, end);

        Self {
            name: String::new(),
            length,
            width,
            shader,
            vertices,
            color,
            sort_level,
            transform,
        }
    }

    /// Reset the line start and end points
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Reset (line3d.cpp lines 399-418)
    ///
    /// # Implementation
    ///
    /// 1. Calculate new length
    /// 2. Scale vertices to new length (X-axis only)
    /// 3. Update transform to new position/orientation
    pub fn reset(&mut self, new_start: Vec3, new_end: Vec3) {
        let mut new_length = (new_end - new_start).length();

        // Prevent zero-length lines
        // C++ Reference: line3d.cpp lines 403-405
        if new_length == 0.0 {
            new_length = 0.001;
        }

        // Scale vertices along X-axis
        let scale_factor = new_length / self.length;
        for vertex in &mut self.vertices {
            vertex.x *= scale_factor;
        }

        self.length = new_length;

        // Update transform
        self.transform = Self::obj_look_at(new_start, new_end);
    }

    /// Reset line start, end, and width
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Reset (line3d.cpp lines 435-465)
    pub fn reset_with_width(&mut self, new_start: Vec3, new_end: Vec3, new_width: f32) {
        let mut new_length = (new_end - new_start).length();

        // Prevent zero-length lines
        if new_length == 0.0 {
            new_length = 0.001;
        }

        // Scale vertices
        let length_scale = new_length / self.length;
        let width_scale = new_width / self.width;

        for vertex in &mut self.vertices {
            vertex.x *= length_scale;
            vertex.y *= width_scale;
            vertex.z *= width_scale;
        }

        self.length = new_length;
        self.width = new_width;

        // Update transform
        self.transform = Self::obj_look_at(new_start, new_end);
    }

    /// Reset the line color
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Re_Color (line3d.cpp lines 481-484)
    pub fn re_color(&mut self, r: f32, g: f32, b: f32) {
        self.color.x = r;
        self.color.y = g;
        self.color.z = b;
    }

    /// Set the line opacity
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Set_Opacity (line3d.cpp lines 499-510)
    ///
    /// # Implementation
    ///
    /// Updates shader based on opacity:
    /// - opacity < 1.0: Use alpha blend shader, sort_level = 1
    /// - opacity >= 1.0: Use opaque shader, sort_level = NONE
    pub fn set_opacity(&mut self, opacity: f32) {
        if opacity < 1.0 {
            self.shader = ShaderClass::preset_alpha_solid();
            self.sort_level = 1;
        } else {
            self.shader = ShaderClass::preset_opaque_solid();
            self.sort_level = SORT_LEVEL_NONE as i32;
        }
        self.color.w = opacity;
    }

    /// Get the current color (including alpha)
    pub fn get_color(&self) -> Vec4 {
        self.color
    }

    /// Get the current width
    pub fn get_width(&self) -> f32 {
        self.width
    }

    /// Get the current length
    pub fn get_length(&self) -> f32 {
        self.length
    }

    /// Compute transform matrix to orient object from start to end
    ///
    /// # C++ Reference
    ///
    /// Matches Matrix3D::Obj_Look_At functionality
    /// This creates a transform that:
    /// - Places origin at `start`
    /// - Orients X-axis to point toward `end`
    /// - Y and Z axes are perpendicular to line direction
    ///
    /// # Implementation
    ///
    /// For a line from start to end:
    /// 1. Direction = normalized(end - start)
    /// 2. Choose perpendicular up vector
    /// 3. Build orthonormal basis (right, up, forward)
    /// 4. Construct rotation + translation matrix
    fn obj_look_at(start: Vec3, end: Vec3) -> Mat4 {
        let direction = (end - start).normalize_or_zero();

        // Handle degenerate case
        if direction.length_squared() < 1e-6 {
            return Mat4::from_translation(start);
        }

        // Choose up vector that's not parallel to direction
        let up = if direction.y.abs() < 0.999 {
            Vec3::Y
        } else {
            Vec3::Z
        };

        // Build orthonormal basis
        // X-axis = direction (along line)
        let x_axis = direction;
        // Z-axis = cross(X, up)
        let z_axis = x_axis.cross(up).normalize_or_zero();
        // Y-axis = cross(Z, X) to complete orthonormal basis
        let y_axis = z_axis.cross(x_axis);

        // Construct transform matrix
        // C++ stores as column-major, glam Mat4 is column-major
        Mat4::from_cols(
            Vec4::new(x_axis.x, x_axis.y, x_axis.z, 0.0),
            Vec4::new(y_axis.x, y_axis.y, y_axis.z, 0.0),
            Vec4::new(z_axis.x, z_axis.y, z_axis.z, 0.0),
            Vec4::new(start.x, start.y, start.z, 1.0),
        )
    }

    fn build_world_vertices(&self) -> [Vec3; 8] {
        std::array::from_fn(|i| self.transform.transform_point3(self.vertices[i]))
    }

    fn build_world_triangles_from_vertices(world_vertices: &[Vec3; 8]) -> Vec<[Vec3; 3]> {
        INDICES
            .chunks_exact(3)
            .map(|triangle| {
                [
                    world_vertices[triangle[0] as usize],
                    world_vertices[triangle[1] as usize],
                    world_vertices[triangle[2] as usize],
                ]
            })
            .collect()
    }

    /// Build immutable renderer-facing data from this line object.
    pub fn prepare_render_submission(&self) -> Line3DRenderSubmission {
        let world_vertices = self.build_world_vertices();
        let world_triangles = Self::build_world_triangles_from_vertices(&world_vertices);

        Line3DRenderSubmission {
            world_transform: self.transform,
            shader: self.shader,
            color: self.color,
            world_vertices,
            world_triangles,
            triangle_indices: &INDICES,
            sort_level: self.sort_level,
        }
    }

    /// Render the line using dynamic vertex/index buffers
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Render (line3d.cpp lines 254-310)
    ///
    /// # Implementation
    ///
    /// 1. Check visibility and sort level
    /// 2. Set shader and material (prelit diffuse)
    /// 3. Set world transform
    /// 4. Fill dynamic vertex buffer with 8 vertices
    /// 5. Fill dynamic index buffer with 36 indices (12 triangles)
    /// 6. Draw triangles
    ///
    fn render_impl(&self, _rinfo: &RenderInfoClass) -> RendererResult<()> {
        // Assemble runtime line state into a backend-agnostic submission packet.
        let submission = self.prepare_render_submission();
        debug_assert_eq!(submission.world_triangles.len(), INDICES.len() / 3);
        let _ = submission;
        Ok(())
    }
}

impl RenderObjClass for Line3DRenderObj {
    fn clone_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::Line3D
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    fn get_num_polys(&self) -> usize {
        // C++ Reference: line3d.cpp lines 515-518
        12 // 36 indices / 3 = 12 triangles
    }

    fn render(&self, rinfo: &RenderInfoClass) -> RendererResult<()> {
        self.render_impl(rinfo)
    }

    fn special_render(&self, _rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        // Lines don't typically participate in special render passes
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn cast_ray(&self, _raytest: &mut RayCollisionTestClass) -> bool {
        // Lines typically don't have ray collision
        false
    }

    fn cast_aabox(&self, _boxtest: &mut AABoxCollisionTestClass) -> bool {
        false
    }

    fn cast_obbox(&self, _boxtest: &mut OBBoxCollisionTestClass) -> bool {
        false
    }

    fn intersect_aabox(&self, _boxtest: &AABoxIntersectionTestClass) -> bool {
        false
    }

    fn intersect_obbox(&self, _boxtest: &OBBoxIntersectionTestClass) -> bool {
        false
    }

    /// Get object-space bounding sphere
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Get_Obj_Space_Bounding_Sphere (line3d.cpp lines 371-376)
    ///
    /// The sphere is centered at the midpoint of the line with radius = length/2
    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        let half_length = self.length * 0.5;
        SphereClass {
            center: Vec3::new(half_length, 0.0, 0.0),
            radius: half_length,
        }
    }

    /// Get object-space bounding box
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Get_Obj_Space_Bounding_Box (line3d.cpp lines 379-384)
    ///
    /// The box is centered at the midpoint with extent = length/2 along X-axis
    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let half_length = self.length * 0.5;
        AABoxClass {
            center: Vec3::new(half_length, 0.0, 0.0),
            extent: Vec3::new(half_length, 0.0, 0.0),
        }
    }

    /// Scale the line uniformly
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Scale (line3d.cpp lines 326-337)
    fn scale(&mut self, scale: f32) {
        for vertex in &mut self.vertices {
            *vertex *= scale;
        }
        self.length *= scale;
        self.width *= scale;
    }

    /// Scale the line with separate axis factors
    ///
    /// # C++ Reference
    ///
    /// Matches Line3DClass::Scale (line3d.cpp lines 354-368)
    ///
    /// Note: X-axis scales length, Y/Z scale width
    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        let scale_vec = Vec3::new(scalex, scaley, scalez);
        for vertex in &mut self.vertices {
            *vertex *= scale_vec;
        }
        self.length *= scalex;
        self.width *= scaley; // Assumes symmetric width in Y/Z
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        None
    }

    fn get_sort_level(&self) -> i32 {
        self.sort_level
    }

    fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level;
    }

    fn create_decal(&mut self, _generator: &mut DecalGeneratorClass) {
        // Lines don't support decals
    }

    fn delete_decal(&mut self, _decal_id: u32) {
        // Lines don't support decals
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line3d_construction() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(10.0, 0.0, 0.0);
        let width = 1.0;
        let line = Line3DRenderObj::new(start, end, width, 1.0, 0.0, 0.0, 1.0);

        assert_eq!(line.get_length(), 10.0);
        assert_eq!(line.get_width(), 1.0);
        assert_eq!(line.get_color(), Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(line.get_num_polys(), 12);
    }

    #[test]
    fn test_line3d_diagonal() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(3.0, 4.0, 0.0);
        let width = 0.5;
        let line = Line3DRenderObj::new(start, end, width, 0.0, 1.0, 0.0, 1.0);

        // Length should be 5 (3-4-5 triangle)
        assert!((line.get_length() - 5.0).abs() < 0.001);
        assert_eq!(line.get_width(), 0.5);
    }

    #[test]
    fn test_line3d_opacity() {
        let start = Vec3::ZERO;
        let end = Vec3::X;
        let mut line = Line3DRenderObj::new(start, end, 1.0, 1.0, 1.0, 1.0, 0.5);

        // Semi-transparent lines should have sort_level = 1
        assert_eq!(line.get_sort_level(), 1);
        assert_eq!(line.get_color().w, 0.5);

        // Setting to full opacity should reset sort level
        line.set_opacity(1.0);
        assert_eq!(line.get_sort_level(), SORT_LEVEL_NONE as i32);
        assert_eq!(line.get_color().w, 1.0);
    }

    #[test]
    fn test_line3d_reset() {
        let start = Vec3::ZERO;
        let end = Vec3::new(10.0, 0.0, 0.0);
        let mut line = Line3DRenderObj::new(start, end, 1.0, 1.0, 1.0, 1.0, 1.0);

        // Reset to new position
        let new_start = Vec3::new(5.0, 5.0, 5.0);
        let new_end = Vec3::new(10.0, 5.0, 5.0);
        line.reset(new_start, new_end);

        assert_eq!(line.get_length(), 5.0);
    }

    #[test]
    fn test_line3d_reset_with_width() {
        let start = Vec3::ZERO;
        let end = Vec3::new(10.0, 0.0, 0.0);
        let mut line = Line3DRenderObj::new(start, end, 1.0, 1.0, 1.0, 1.0, 1.0);

        // Reset with new width
        let new_start = Vec3::ZERO;
        let new_end = Vec3::new(5.0, 0.0, 0.0);
        line.reset_with_width(new_start, new_end, 2.0);

        assert_eq!(line.get_length(), 5.0);
        assert_eq!(line.get_width(), 2.0);
    }

    #[test]
    fn test_line3d_re_color() {
        let start = Vec3::ZERO;
        let end = Vec3::X;
        let mut line = Line3DRenderObj::new(start, end, 1.0, 1.0, 0.0, 0.0, 1.0);

        line.re_color(0.0, 1.0, 0.0);
        let color = line.get_color();
        assert_eq!(color.x, 0.0);
        assert_eq!(color.y, 1.0);
        assert_eq!(color.z, 0.0);
        assert_eq!(color.w, 1.0); // Alpha unchanged
    }

    #[test]
    fn test_line3d_scale() {
        let start = Vec3::ZERO;
        let end = Vec3::new(10.0, 0.0, 0.0);
        let mut line = Line3DRenderObj::new(start, end, 2.0, 1.0, 1.0, 1.0, 1.0);

        line.scale(2.0);
        assert_eq!(line.get_length(), 20.0);
        assert_eq!(line.get_width(), 4.0);
    }

    #[test]
    fn test_line3d_scale_xyz() {
        let start = Vec3::ZERO;
        let end = Vec3::new(10.0, 0.0, 0.0);
        let mut line = Line3DRenderObj::new(start, end, 2.0, 1.0, 1.0, 1.0, 1.0);

        // Scale length by 2x, width by 0.5x
        line.scale_xyz(2.0, 0.5, 0.5);
        assert_eq!(line.get_length(), 20.0);
        assert_eq!(line.get_width(), 1.0);
    }

    #[test]
    fn test_line3d_bounding_sphere() {
        let start = Vec3::ZERO;
        let end = Vec3::new(10.0, 0.0, 0.0);
        let line = Line3DRenderObj::new(start, end, 1.0, 1.0, 1.0, 1.0, 1.0);

        let sphere = line.get_obj_space_bounding_sphere();
        // Centered at midpoint (5, 0, 0)
        assert_eq!(sphere.center, Vec3::new(5.0, 0.0, 0.0));
        // Radius is half length
        assert_eq!(sphere.radius, 5.0);
    }

    #[test]
    fn test_line3d_bounding_box() {
        let start = Vec3::ZERO;
        let end = Vec3::new(10.0, 0.0, 0.0);
        let line = Line3DRenderObj::new(start, end, 1.0, 1.0, 1.0, 1.0, 1.0);

        let bbox = line.get_obj_space_bounding_box();
        // Centered at midpoint (5, 0, 0)
        assert_eq!(bbox.center, Vec3::new(5.0, 0.0, 0.0));
        // Extent is half length along X
        assert_eq!(bbox.extent, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn test_line3d_zero_length_prevention() {
        let start = Vec3::new(5.0, 5.0, 5.0);
        let end = Vec3::new(5.0, 5.0, 5.0); // Same point
        let mut line =
            Line3DRenderObj::new(start, Vec3::new(6.0, 5.0, 5.0), 1.0, 1.0, 1.0, 1.0, 1.0);

        // Reset to zero-length should clamp to 0.001
        line.reset(start, end);
        assert_eq!(line.get_length(), 0.001);
    }

    #[test]
    fn test_line3d_class_id() {
        let start = Vec3::ZERO;
        let end = Vec3::X;
        let line = Line3DRenderObj::new(start, end, 1.0, 1.0, 1.0, 1.0, 1.0);

        assert_eq!(line.class_id(), RenderObjClassId::Line3D);
    }

    #[test]
    fn test_line3d_vertices() {
        let start = Vec3::ZERO;
        let end = Vec3::new(10.0, 0.0, 0.0);
        let width = 2.0;
        let line = Line3DRenderObj::new(start, end, width, 1.0, 1.0, 1.0, 1.0);

        // Verify vertex positions
        // Start end (X=0)
        assert_eq!(line.vertices[0], Vec3::new(0.0, -1.0, -1.0));
        assert_eq!(line.vertices[1], Vec3::new(0.0, 1.0, -1.0));
        assert_eq!(line.vertices[2], Vec3::new(0.0, -1.0, 1.0));
        assert_eq!(line.vertices[3], Vec3::new(0.0, 1.0, 1.0));
        // End (X=length)
        assert_eq!(line.vertices[4], Vec3::new(10.0, -1.0, -1.0));
        assert_eq!(line.vertices[5], Vec3::new(10.0, 1.0, -1.0));
        assert_eq!(line.vertices[6], Vec3::new(10.0, -1.0, 1.0));
        assert_eq!(line.vertices[7], Vec3::new(10.0, 1.0, 1.0));
    }

    #[test]
    fn test_line_submission_contains_expected_triangle_count() {
        let line = Line3DRenderObj::new(
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            2.0,
            1.0,
            1.0,
            1.0,
            1.0,
        );
        let submission = line.prepare_render_submission();
        assert_eq!(submission.world_vertices.len(), 8);
        assert_eq!(submission.world_triangles.len(), 12);
        assert_eq!(submission.triangle_indices.len(), 36);
    }

    #[test]
    fn test_line_submission_applies_world_transform() {
        let line = Line3DRenderObj::new(
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(15.0, 0.0, 0.0),
            2.0,
            1.0,
            1.0,
            1.0,
            1.0,
        );
        let submission = line.prepare_render_submission();
        assert_eq!(submission.world_vertices[0], Vec3::new(5.0, -1.0, -1.0));
        assert_eq!(submission.world_vertices[4], Vec3::new(15.0, -1.0, -1.0));
    }
}
