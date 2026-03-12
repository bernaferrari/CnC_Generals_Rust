//! Camera Class - Core camera functionality with projection and view matrices
//!
//! This module implements the CameraClass from the original C++ code,
//! providing comprehensive camera control with WGPU integration.

use super::frustum::FrustumClass;
use super::viewport::ViewportClass;
use crate::core::error::Result;
use crate::core::wwstring::StringClass;
use crate::render_object_system::{AABoxClass, SphereClass};
use crate::render_object_system::{
    AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass, MaterialInfoClass,
    OBBoxCollisionTestClass, OBBoxIntersectionTestClass, RayCollisionTestClass, RenderInfoClass,
    RenderObjClass, SpecialRenderInfoClass,
};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use ww3d_core::RenderObjClassId;

/// Projection type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectionType {
    /// Perspective projection
    Perspective = 0,
    /// Orthographic projection
    Ortho,
}

/// Projection result type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectionResType {
    /// Point is inside frustum
    InsideFrustum,
    /// Point is outside frustum
    OutsideFrustum,
    /// Point is outside near clip plane
    OutsideNearClip,
    /// Point is outside far clip plane
    OutsideFarClip,
}

/// Camera Class - Core camera with projection and view matrices
#[derive(Debug)]
pub struct CameraClass {
    /// Camera name
    name: StringClass,
    /// Transform matrix
    transform: Mat4,
    /// Whether transform is identity
    transform_identity: bool,

    /// Projection type
    projection_type: ProjectionType,
    /// Near clip plane distance
    near_clip: f32,
    /// Far clip plane distance
    far_clip: f32,
    /// Z-buffer minimum value
    zbuffer_min: f32,
    /// Z-buffer maximum value
    zbuffer_max: f32,

    /// View plane minimum point
    view_plane_min: Vec2,
    /// View plane maximum point
    view_plane_max: Vec2,
    /// Horizontal field of view (in radians)
    hfov: f32,
    /// Vertical field of view (in radians)
    vfov: f32,
    /// Aspect ratio
    aspect_ratio: f32,

    /// Viewport
    viewport: ViewportClass,
    /// Depth range start
    depth_start: f32,
    /// Depth range end
    depth_end: f32,

    /// Cached view matrix
    view_matrix: Mat4,
    /// Cached projection matrix
    projection_matrix: Mat4,
    /// Cached view-projection matrix
    view_projection_matrix: Mat4,
    /// Whether matrices are dirty and need recalculation
    matrices_dirty: bool,

    /// Frustum for culling
    frustum: FrustumClass,

    /// Reference count
    ref_count: std::sync::atomic::AtomicU32,
}

impl CameraClass {
    /// Create new camera with default settings
    pub fn new() -> Self {
        let mut camera = Self {
            name: StringClass::empty(),
            transform: Mat4::IDENTITY,
            transform_identity: true,
            projection_type: ProjectionType::Perspective,
            near_clip: 1.0,
            far_clip: 1000.0,
            zbuffer_min: 0.0,
            zbuffer_max: 1.0,
            view_plane_min: Vec2::new(-1.0, -1.0),
            view_plane_max: Vec2::new(1.0, 1.0),
            hfov: std::f32::consts::PI / 3.0, // 60 degrees
            vfov: std::f32::consts::PI / 3.0, // 60 degrees
            aspect_ratio: 1.0,
            viewport: ViewportClass::new(),
            depth_start: 0.0,
            depth_end: 1.0,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            matrices_dirty: true,
            frustum: FrustumClass::new(),
            ref_count: std::sync::atomic::AtomicU32::new(1),
        };

        camera.update_aspect_ratio();
        camera.update_matrices();
        camera.update_frustum();
        camera
    }

    /// Create camera with custom aspect ratio
    pub fn with_aspect_ratio(aspect_ratio: f32) -> Self {
        let mut camera = Self::new();
        camera.set_aspect_ratio(aspect_ratio);
        camera
    }

    /// Set projection type
    pub fn set_projection_type(&mut self, projection_type: ProjectionType) {
        if self.projection_type != projection_type {
            self.projection_type = projection_type;
            self.matrices_dirty = true;
            self.update_frustum();
        }
    }

    /// Get projection type
    pub fn get_projection_type(&self) -> ProjectionType {
        self.projection_type
    }

    /// Set clip planes
    pub fn set_clip_planes(&mut self, near: f32, far: f32) {
        self.near_clip = near;
        self.far_clip = far;
        self.matrices_dirty = true;
        self.update_frustum();
    }

    /// Get clip planes
    pub fn get_clip_planes(&self) -> (f32, f32) {
        (self.near_clip, self.far_clip)
    }

    /// Get near clip plane
    pub fn get_near_clip(&self) -> f32 {
        self.near_clip
    }

    /// Get far clip plane
    pub fn get_far_clip(&self) -> f32 {
        self.far_clip
    }

    /// Get near plane distance (alias for get_near_clip)
    pub fn get_near_plane(&self) -> f32 {
        self.near_clip
    }

    /// Get far plane distance (alias for get_far_clip)
    pub fn get_far_plane(&self) -> f32 {
        self.far_clip
    }

    /// Set z-buffer range
    pub fn set_zbuffer_range(&mut self, min: f32, max: f32) {
        self.zbuffer_min = min;
        self.zbuffer_max = max;
        self.matrices_dirty = true;
    }

    /// Get z-buffer range
    pub fn get_zbuffer_range(&self) -> (f32, f32) {
        (self.zbuffer_min, self.zbuffer_max)
    }

    /// Set view plane by min/max points
    pub fn set_view_plane(&mut self, min: Vec2, max: Vec2) {
        self.view_plane_min = min;
        self.view_plane_max = max;
        self.update_fov_from_view_plane();
        self.update_aspect_ratio();
        self.matrices_dirty = true;
        self.update_frustum();
    }

    /// Set view plane by FOV
    pub fn set_view_plane_fov(&mut self, hfov: f32, vfov: f32) {
        self.hfov = hfov;
        self.vfov = if vfov > 0.0 {
            vfov
        } else {
            hfov / self.aspect_ratio
        };
        self.update_view_plane_from_fov();
        self.matrices_dirty = true;
        self.update_frustum();
    }

    /// Set aspect ratio
    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
        self.update_view_plane_from_fov();
        self.matrices_dirty = true;
        self.update_frustum();
    }

    /// Get aspect ratio
    pub fn get_aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    /// Get horizontal FOV
    pub fn get_horizontal_fov(&self) -> f32 {
        self.hfov
    }

    /// Get vertical FOV
    pub fn get_vertical_fov(&self) -> f32 {
        self.vfov
    }

    /// Get view plane
    pub fn get_view_plane(&self) -> (Vec2, Vec2) {
        (self.view_plane_min, self.view_plane_max)
    }

    /// Set viewport
    pub fn set_viewport(&mut self, min: Vec2, max: Vec2) {
        self.viewport = ViewportClass::from_min_max(min, max);
    }

    /// Get viewport
    pub fn get_viewport(&self) -> &ViewportClass {
        &self.viewport
    }

    /// Set depth range
    pub fn set_depth_range(&mut self, start: f32, end: f32) {
        self.depth_start = start;
        self.depth_end = end;
    }

    /// Get depth range
    pub fn get_depth_range(&self) -> (f32, f32) {
        (self.depth_start, self.depth_end)
    }

    /// Get projection matrix
    pub fn get_projection_matrix(&mut self) -> Mat4 {
        if self.matrices_dirty {
            self.update_matrices();
        }
        self.projection_matrix
    }

    /// Get cached projection matrix (assumes matrices are up to date)
    pub fn get_cached_projection_matrix(&self) -> Mat4 {
        self.projection_matrix
    }

    /// Get view matrix
    pub fn get_view_matrix(&mut self) -> Mat4 {
        if self.matrices_dirty {
            self.update_matrices();
        }
        self.view_matrix
    }

    /// Get cached view matrix (assumes matrices are up to date)
    pub fn get_cached_view_matrix(&self) -> Mat4 {
        self.view_matrix
    }

    /// Get view-projection matrix
    pub fn get_view_projection_matrix(&mut self) -> Mat4 {
        if self.matrices_dirty {
            self.update_matrices();
        }
        self.view_projection_matrix
    }

    /// Get cached view-projection matrix (assumes matrices are up to date)
    pub fn get_cached_view_projection_matrix(&self) -> Mat4 {
        self.view_projection_matrix
    }

    /// Get view matrix (convenience method that updates if needed)
    pub fn view_matrix(&mut self) -> Mat4 {
        self.get_view_matrix()
    }

    /// Get projection matrix (convenience method that updates if needed)
    pub fn projection_matrix(&mut self) -> Mat4 {
        self.get_projection_matrix()
    }

    /// Get view-projection matrix (convenience method that updates if needed)
    pub fn view_projection_matrix(&mut self) -> Mat4 {
        self.get_view_projection_matrix()
    }

    /// Get camera position (convenience alias for get_position)
    pub fn position(&self) -> Vec3 {
        self.get_position()
    }

    /// Project world space point to screen space
    pub fn project(&mut self, world_point: Vec3) -> Result<(Vec3, ProjectionResType)> {
        let view_proj = self.get_view_projection_matrix();

        // Transform to clip space
        let clip_point = view_proj * Vec4::new(world_point.x, world_point.y, world_point.z, 1.0);

        // Check if point is behind camera
        if clip_point.w < 0.0 {
            return Ok((Vec3::ZERO, ProjectionResType::OutsideNearClip));
        }

        // Perspective divide
        let ndc_point = Vec3::new(clip_point.x, clip_point.y, clip_point.z) / clip_point.w;

        // Check if point is outside frustum
        if ndc_point.x < -1.0
            || ndc_point.x > 1.0
            || ndc_point.y < -1.0
            || ndc_point.y > 1.0
            || ndc_point.z < 0.0
            || ndc_point.z > 1.0
        {
            return Ok((ndc_point, ProjectionResType::OutsideFrustum));
        }

        // Convert to screen space
        let screen_x = (ndc_point.x + 1.0) * 0.5 * self.viewport.width();
        let screen_y = (1.0 - ndc_point.y) * 0.5 * self.viewport.height();
        let screen_point = Vec3::new(screen_x, screen_y, ndc_point.z);

        Ok((screen_point, ProjectionResType::InsideFrustum))
    }

    /// Unproject screen space point to world space
    pub fn unproject(&mut self, screen_point: Vec2) -> Vec3 {
        let view_proj_inv = self.get_view_projection_matrix().inverse();

        // Convert to NDC
        let ndc_x = (screen_point.x / self.viewport.width()) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_point.y / self.viewport.height()) * 2.0;

        // Create clip space point at near plane
        let clip_point = Vec4::new(ndc_x, ndc_y, 0.0, 1.0);

        // Transform back to world space
        let world_point = view_proj_inv * clip_point;

        Vec3::new(world_point.x, world_point.y, world_point.z) / world_point.w
    }

    /// Transform point to view space
    pub fn transform_to_view_space(&mut self, world_point: Vec3) -> Vec3 {
        let view_matrix = self.get_view_matrix();
        (view_matrix * Vec4::new(world_point.x, world_point.y, world_point.z, 1.0)).truncate()
    }

    /// Rotate vector to view space
    pub fn rotate_to_view_space(&mut self, world_vector: Vec3) -> Vec3 {
        let view_matrix = self.get_view_matrix();
        (view_matrix * Vec4::new(world_vector.x, world_vector.y, world_vector.z, 0.0)).truncate()
    }

    /// Get frustum
    pub fn get_frustum(&self) -> &FrustumClass {
        &self.frustum
    }

    /// Get camera position
    pub fn get_position(&self) -> Vec3 {
        self.transform.row(3).truncate()
    }

    /// Get camera forward direction
    pub fn get_forward(&self) -> Vec3 {
        -(self.transform.row(2).truncate()).normalize()
    }

    /// Get camera up direction
    pub fn get_up(&self) -> Vec3 {
        self.transform.row(1).truncate().normalize()
    }

    /// Get camera right direction
    pub fn get_right(&self) -> Vec3 {
        self.transform.row(0).truncate().normalize()
    }

    /// Look at target
    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let position = self.get_position();
        let forward = (target - position).normalize();
        let right = up.cross(forward).normalize();
        let up_corrected = forward.cross(right).normalize();

        // Reconstruct the matrix with new orientation
        let position = self.get_position();
        self.transform = Mat4::from_cols(
            Vec4::new(right.x, up_corrected.x, -forward.x, 0.0),
            Vec4::new(right.y, up_corrected.y, -forward.y, 0.0),
            Vec4::new(right.z, up_corrected.z, -forward.z, 0.0),
            Vec4::new(position.x, position.y, position.z, 1.0),
        );
        self.transform_identity = false;
        self.matrices_dirty = true;
        self.update_frustum();
    }

    /// Set camera position
    pub fn set_position(&mut self, position: Vec3) {
        self.transform.w_axis.x = position.x;
        self.transform.w_axis.y = position.y;
        self.transform.w_axis.z = position.z;
        self.transform_identity = false;
        self.matrices_dirty = true;
        self.update_frustum();
    }

    /// Set view matrix directly
    pub fn set_view_matrix(&mut self, view_matrix: Mat4) {
        self.view_matrix = view_matrix;
        // Derive transform from inverse of view matrix
        self.transform = view_matrix.inverse();
        self.transform_identity = self.transform == Mat4::IDENTITY;
        self.matrices_dirty = false; // We just set the view matrix directly
        self.update_frustum();
    }

    /// Set projection matrix directly
    pub fn set_projection_matrix(&mut self, projection_matrix: Mat4) {
        self.projection_matrix = projection_matrix;
        // Update view-projection matrix
        self.view_projection_matrix = self.projection_matrix * self.view_matrix;
        self.update_frustum();
    }

    /// Get depth (distance from near to far clip)
    pub fn get_depth(&self) -> f32 {
        self.far_clip - self.near_clip
    }

    // Private helper methods

    /// Update FOV from view plane
    fn update_fov_from_view_plane(&mut self) {
        let width = self.view_plane_max.x - self.view_plane_min.x;
        let height = self.view_plane_max.y - self.view_plane_min.y;

        // Assuming view plane is at distance 1.0
        self.hfov = 2.0 * (width * 0.5).atan();
        self.vfov = 2.0 * (height * 0.5).atan();
    }

    /// Update view plane from FOV
    fn update_view_plane_from_fov(&mut self) {
        // View plane at distance 1.0
        let half_width = (self.hfov * 0.5).tan();
        let half_height = (self.vfov * 0.5).tan();

        self.view_plane_min = Vec2::new(-half_width, -half_height);
        self.view_plane_max = Vec2::new(half_width, half_height);
    }

    /// Update aspect ratio
    fn update_aspect_ratio(&mut self) {
        let width = self.view_plane_max.x - self.view_plane_min.x;
        let height = self.view_plane_max.y - self.view_plane_min.y;
        self.aspect_ratio = width / height;
    }

    /// Update matrices
    fn update_matrices(&mut self) {
        // Update view matrix
        self.view_matrix = self.transform.inverse();

        // Update projection matrix
        match self.projection_type {
            ProjectionType::Perspective => {
                self.projection_matrix = Mat4::perspective_rh(
                    self.vfov,
                    self.aspect_ratio,
                    self.near_clip,
                    self.far_clip,
                );
            }
            ProjectionType::Ortho => {
                self.projection_matrix = Mat4::orthographic_rh(
                    self.view_plane_min.x,
                    self.view_plane_max.x,
                    self.view_plane_min.y,
                    self.view_plane_max.y,
                    self.near_clip,
                    self.far_clip,
                );
            }
        }

        // Update view-projection matrix
        self.view_projection_matrix = self.projection_matrix * self.view_matrix;
        self.matrices_dirty = false;
    }

    /// Update frustum
    fn update_frustum(&mut self) {
        let view_proj = self.get_view_projection_matrix();
        self.frustum.update_from_matrix(view_proj);
    }
}

impl RenderObjClass for CameraClass {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::Camera
    }

    fn clone_render_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn render(&self, _rinfo: &RenderInfoClass) -> Result<()> {
        // Cameras don't render geometry
        Ok(())
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.transform_identity = self.transform == Mat4::IDENTITY;
        self.matrices_dirty = true;
        self.update_frustum();
    }

    fn set_position(&mut self, position: Vec3) {
        // Update position in the matrix (4th column)
        self.transform.w_axis = Vec4::new(position.x, position.y, position.z, 1.0);
        self.transform_identity = false;
        self.matrices_dirty = true;
        self.update_frustum();
    }

    fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    fn get_bounding_sphere(&self) -> SphereClass {
        // Camera has no meaningful bounding volume
        SphereClass::new(self.position(), 1.0)
    }

    fn get_bounding_box(&self) -> AABoxClass {
        // Camera has no meaningful bounding volume
        let pos = self.position();
        AABoxClass::from_center_and_extent(pos, Vec3::new(1.0, 1.0, 1.0))
    }

    fn get_name(&self) -> &str {
        self.name.as_str()
    }

    fn set_name(&mut self, name: &str) {
        self.name = StringClass::from(name);
    }

    fn add_engine_ref(&self) {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn release_engine_ref(&self) {
        let old_count = self
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        if old_count == 1 {
            // Camera will be dropped when this reference goes out of scope
        }
    }

    fn clone_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn get_num_polys(&self) -> usize {
        0 // Cameras don't have polygons
    }

    fn special_render(&self, _rinfo: &SpecialRenderInfoClass) -> Result<()> {
        // Cameras don't have special rendering
        Ok(())
    }

    fn cast_ray(&self, _raytest: &mut RayCollisionTestClass) -> bool {
        false // Cameras don't intersect rays
    }

    fn cast_aabox(&self, _boxtest: &mut AABoxCollisionTestClass) -> bool {
        false // Cameras don't intersect AABox
    }

    fn cast_obbox(&self, _boxtest: &mut OBBoxCollisionTestClass) -> bool {
        false // Cameras don't intersect OBBox
    }

    fn intersect_aabox(&self, _boxtest: &AABoxIntersectionTestClass) -> bool {
        false // Cameras don't intersect AABox
    }

    fn intersect_obbox(&self, _boxtest: &OBBoxIntersectionTestClass) -> bool {
        false // Cameras don't intersect OBBox
    }

    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        SphereClass::new(Vec3::ZERO, 1.0) // Default small sphere
    }

    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0))
        // Default small box
    }

    fn scale(&mut self, scale: f32) {
        // Scale the camera transform
        let scale_matrix = Mat4::from_scale(Vec3::splat(scale));
        self.transform = scale_matrix * self.transform;
        self.matrices_dirty = true;
    }

    fn scale_xyz(&mut self, scale_x: f32, scale_y: f32, scale_z: f32) {
        // Non-uniform scale the camera transform
        let scale_matrix = Mat4::from_scale(Vec3::new(scale_x, scale_y, scale_z));
        self.transform = scale_matrix * self.transform;
        self.matrices_dirty = true;
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        None // Cameras don't have materials
    }

    fn get_sort_level(&self) -> i32 {
        0 // Default sort level
    }

    fn set_sort_level(&mut self, _level: i32) {
        // Cameras don't use sort levels
    }

    fn create_decal(&mut self, _generator: &mut DecalGeneratorClass) {
        // Cameras don't support decals
    }

    fn delete_decal(&mut self, _decal_id: u32) {
        // Cameras don't support decals
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn engine_refs(&self) -> usize {
        self.ref_count.load(std::sync::atomic::Ordering::Relaxed) as usize
    }
}

impl Clone for CameraClass {
    fn clone(&self) -> Self {
        let mut cloned = Self {
            name: self.name.clone(),
            transform: self.transform,
            transform_identity: self.transform_identity,
            projection_type: self.projection_type,
            near_clip: self.near_clip,
            far_clip: self.far_clip,
            zbuffer_min: self.zbuffer_min,
            zbuffer_max: self.zbuffer_max,
            view_plane_min: self.view_plane_min,
            view_plane_max: self.view_plane_max,
            hfov: self.hfov,
            vfov: self.vfov,
            aspect_ratio: self.aspect_ratio,
            viewport: self.viewport.clone(),
            depth_start: self.depth_start,
            depth_end: self.depth_end,
            view_matrix: self.view_matrix,
            projection_matrix: self.projection_matrix,
            view_projection_matrix: self.view_projection_matrix,
            matrices_dirty: true, // Force recalculation
            frustum: self.frustum.clone(),
            ref_count: std::sync::atomic::AtomicU32::new(1),
        };

        cloned.update_matrices();
        cloned.update_frustum();
        cloned
    }
}

impl Default for CameraClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Camera utilities
pub struct CameraUtils;

impl CameraUtils {
    /// Create perspective camera
    pub fn create_perspective(fov: f32, aspect_ratio: f32, near: f32, far: f32) -> CameraClass {
        let mut camera = CameraClass::new();
        camera.set_projection_type(ProjectionType::Perspective);
        camera.set_view_plane_fov(fov, fov / aspect_ratio);
        camera.set_clip_planes(near, far);
        camera
    }

    /// Create orthographic camera
    pub fn create_orthographic(width: f32, height: f32, near: f32, far: f32) -> CameraClass {
        let mut camera = CameraClass::new();
        camera.set_projection_type(ProjectionType::Ortho);
        camera.set_view_plane(
            Vec2::new(-width * 0.5, -height * 0.5),
            Vec2::new(width * 0.5, height * 0.5),
        );
        camera.set_clip_planes(near, far);
        camera
    }

    /// Create first-person camera
    pub fn create_first_person(position: Vec3, yaw: f32, pitch: f32) -> CameraClass {
        let mut camera = CameraClass::new();
        camera.set_position(position);

        let yaw_quat = Quat::from_rotation_y(yaw);
        let pitch_quat = Quat::from_rotation_x(pitch);
        let orientation = yaw_quat * pitch_quat;

        let transform = Mat4::from_rotation_translation(orientation, position);
        camera.set_transform(transform);
        camera
    }

    /// Create third-person camera
    pub fn create_third_person(target: Vec3, distance: f32, height: f32, yaw: f32) -> CameraClass {
        let position = target + Vec3::new(yaw.cos() * distance, height, yaw.sin() * distance);

        let mut camera = CameraClass::new();
        camera.set_position(position);
        camera.look_at(target, Vec3::Y);
        camera
    }
}
