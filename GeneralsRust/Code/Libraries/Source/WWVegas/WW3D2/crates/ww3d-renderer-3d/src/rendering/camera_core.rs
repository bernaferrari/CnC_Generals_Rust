//! Camera system - equivalent to C++ CameraClass

use crate::core::{Result, RendererResult};
use glam::{Vec3, Mat4};
use ww3d_collision::AABoxClass;
use crate::rendering::frustum::FrustumClass;

/// Camera class - equivalent to C++ CameraClass
#[derive(Debug, Clone)]
pub struct CameraClass {
    pub transform: Mat4,
    pub projection: Mat4,
    pub view_matrix: Mat4,
    pub view_projection_matrix: Mat4,
    pub frustum: FrustumClass,
    pub near_clip: f32,
    pub far_clip: f32,
    pub fov: f32,
    pub aspect_ratio: f32,
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
}

impl CameraClass {
    /// Create a new camera
    pub fn new() -> Self {
        let mut camera = Self {
            transform: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
            view_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            frustum: FrustumClass::new(),
            near_clip: 1.0,
            far_clip: 1000.0,
            fov: 60.0f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            position: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
        };
        camera.update_projection();
        camera.update_view();
        camera
    }

    /// Set camera position
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.update_view();
    }

    /// Get the view-projection matrix
    pub fn get_view_projection_matrix(&self) -> Mat4 {
        self.view_projection_matrix
    }

    /// Set camera target (look at point)
    pub fn set_target(&mut self, target: Vec3) {
        self.target = target;
        self.update_view();
    }

    /// Set camera up vector
    pub fn set_up(&mut self, up: Vec3) {
        self.up = up;
        self.update_view();
    }

    /// Set field of view in radians
    pub fn set_fov(&mut self, fov: f32) {
        self.fov = fov;
        self.update_projection();
    }

    /// Set aspect ratio
    pub fn set_aspect_ratio(&mut self, aspect: f32) {
        self.aspect_ratio = aspect;
        self.update_projection();
    }

    /// Set near and far clip planes
    pub fn set_clip_planes(&mut self, near: f32, far: f32) {
        self.near_clip = near;
        self.far_clip = far;
        self.update_projection();
    }

    /// Update the view matrix
    fn update_view(&mut self) {
        // Create look-at matrix
        let forward = (self.target - self.position).normalize();
        let right = forward.cross(self.up).normalize();
        let up = right.cross(forward).normalize();

        self.view_matrix = Mat4::from_cols(
            Vec3::new(right.x, right.y, right.z).extend(-right.dot(self.position)),
            Vec3::new(up.x, up.y, up.z).extend(-up.dot(self.position)),
            Vec3::new(-forward.x, -forward.y, -forward.z).extend(forward.dot(self.position)),
            Vec3::new(0.0, 0.0, 0.0).extend(1.0),
        );

        self.update_view_projection();
    }

    /// Update the projection matrix
    fn update_projection(&mut self) {
        let f = 1.0 / (self.fov / 2.0).tan();
        let range = self.far_clip - self.near_clip;

        self.projection = Mat4::perspective_rh(self.fov, self.aspect_ratio, self.near_clip, self.far_clip);

        self.update_view_projection();
    }

    /// Update the combined view-projection matrix
    fn update_view_projection(&mut self) {
        self.view_projection_matrix = self.projection * self.view_matrix;
        self.frustum.update_from_matrix(&self.view_projection_matrix);
    }

    /// Get the view matrix
    pub fn get_view_matrix(&self) -> &Mat4 {
        &self.view_matrix
    }

    /// Get the projection matrix
    pub fn get_projection_matrix(&self) -> &Mat4 {
        &self.projection
    }

    /// Get the combined view-projection matrix
    pub fn get_view_projection_matrix(&self) -> &Mat4 {
        &self.view_projection_matrix
    }

    /// Get the camera frustum
    pub fn get_frustum(&self) -> &FrustumClass {
        &self.frustum
    }

    /// Get camera position
    pub fn get_position(&self) -> &Vec3 {
        &self.position
    }

    /// Get camera target
    pub fn get_target(&self) -> &Vec3 {
        &self.target
    }

    /// Get camera up vector
    pub fn get_up(&self) -> &Vec3 {
        &self.up
    }

    /// Check if an AABox intersects with the camera frustum
    pub fn intersects_aabox(&self, aabox: &AABoxClass) -> bool {
        self.frustum.intersects_aabox(aabox)
    }
}

impl Default for CameraClass {
    fn default() -> Self {
        Self::new()
    }
}