/// Scene graph and camera system for WW3D
///
/// This module implements the scene graph, camera system, and rendering layers.
use crate::errors::{W3DError, W3DResult};
use crate::render_object::{AABox, BoundingSphere, RenderInfo, RenderObject};
use glam::{Mat4, Quat, Vec3};

/// Camera projection type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionType {
    Perspective,
    Orthographic,
}

/// Camera class for viewing the scene
#[derive(Debug, Clone)]
pub struct Camera {
    name: String,
    position: Vec3,
    rotation: Quat,
    projection_type: ProjectionType,
    fov: f32,
    aspect_ratio: f32,
    near_plane: f32,
    far_plane: f32,
    ortho_width: f32,
    ortho_height: f32,
    view_matrix: Mat4,
    projection_matrix: Mat4,
    view_projection_matrix: Mat4,
    matrices_dirty: bool,
}

impl Camera {
    pub fn new(name: String) -> Self {
        Self {
            name,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            projection_type: ProjectionType::Perspective,
            fov: 60.0_f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            near_plane: 0.1,
            far_plane: 1000.0,
            ortho_width: 10.0,
            ortho_height: 10.0,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection_matrix: Mat4::IDENTITY,
            matrices_dirty: true,
        }
    }

    pub fn perspective(name: String, fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        let mut camera = Self::new(name);
        camera.set_perspective(fov, aspect, near, far);
        camera
    }

    pub fn orthographic(name: String, width: f32, height: f32, near: f32, far: f32) -> Self {
        let mut camera = Self::new(name);
        camera.set_orthographic(width, height, near, far);
        camera
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
        self.matrices_dirty = true;
    }

    pub fn position(&self) -> Vec3 {
        self.position
    }

    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation;
        self.matrices_dirty = true;
    }

    pub fn rotation(&self) -> Quat {
        self.rotation
    }

    pub fn look_at(&mut self, target: Vec3, up: Vec3) {
        let forward = (target - self.position).normalize();
        let right = forward.cross(up).normalize();
        let new_up = right.cross(forward);

        self.rotation = Quat::from_mat3(&glam::Mat3::from_cols(right, new_up, -forward));
        self.matrices_dirty = true;
    }

    pub fn set_perspective(&mut self, fov: f32, aspect: f32, near: f32, far: f32) {
        self.projection_type = ProjectionType::Perspective;
        self.fov = fov;
        self.aspect_ratio = aspect;
        self.near_plane = near;
        self.far_plane = far;
        self.matrices_dirty = true;
    }

    pub fn set_orthographic(&mut self, width: f32, height: f32, near: f32, far: f32) {
        self.projection_type = ProjectionType::Orthographic;
        self.ortho_width = width;
        self.ortho_height = height;
        self.near_plane = near;
        self.far_plane = far;
        self.matrices_dirty = true;
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }

    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }

    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    pub fn update_matrices(&mut self) {
        if !self.matrices_dirty {
            return;
        }

        // Build view matrix
        let transform = Mat4::from_rotation_translation(self.rotation, self.position);
        self.view_matrix = transform.inverse();

        // Build projection matrix
        self.projection_matrix = match self.projection_type {
            ProjectionType::Perspective => {
                Mat4::perspective_rh(self.fov, self.aspect_ratio, self.near_plane, self.far_plane)
            }
            ProjectionType::Orthographic => {
                let half_width = self.ortho_width * 0.5;
                let half_height = self.ortho_height * 0.5;
                Mat4::orthographic_rh(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    self.near_plane,
                    self.far_plane,
                )
            }
        };

        self.view_projection_matrix = self.projection_matrix * self.view_matrix;
        self.matrices_dirty = false;
    }

    pub fn view_matrix(&mut self) -> Mat4 {
        self.update_matrices();
        self.view_matrix
    }

    pub fn projection_matrix(&mut self) -> Mat4 {
        self.update_matrices();
        self.projection_matrix
    }

    pub fn view_projection_matrix(&mut self) -> Mat4 {
        self.update_matrices();
        self.view_projection_matrix
    }

    pub fn is_sphere_visible(&mut self, sphere: &BoundingSphere) -> bool {
        self.update_matrices();
        // Proper frustum culling using 6 frustum planes
        // Extract planes from view-projection matrix (Gribb-Hartmann method)
        let vp = self.view_projection_matrix;

        // Test sphere against each plane
        // Plane equation: normal.dot(point) + d = 0
        // For each frustum plane, check if sphere is outside

        // Left plane: row3 + row0
        let left = Vec3::new(
            vp.w_axis.x + vp.x_axis.x,
            vp.w_axis.y + vp.x_axis.y,
            vp.w_axis.z + vp.x_axis.z,
        );
        let left_d = vp.w_axis.w + vp.x_axis.w;
        if left.dot(sphere.center) + left_d < -sphere.radius {
            return false;
        }

        // Right plane: row3 - row0
        let right = Vec3::new(
            vp.w_axis.x - vp.x_axis.x,
            vp.w_axis.y - vp.x_axis.y,
            vp.w_axis.z - vp.x_axis.z,
        );
        let right_d = vp.w_axis.w - vp.x_axis.w;
        if right.dot(sphere.center) + right_d < -sphere.radius {
            return false;
        }

        // Bottom plane: row3 + row1
        let bottom = Vec3::new(
            vp.w_axis.x + vp.y_axis.x,
            vp.w_axis.y + vp.y_axis.y,
            vp.w_axis.z + vp.y_axis.z,
        );
        let bottom_d = vp.w_axis.w + vp.y_axis.w;
        if bottom.dot(sphere.center) + bottom_d < -sphere.radius {
            return false;
        }

        // Top plane: row3 - row1
        let top = Vec3::new(
            vp.w_axis.x - vp.y_axis.x,
            vp.w_axis.y - vp.y_axis.y,
            vp.w_axis.z - vp.y_axis.z,
        );
        let top_d = vp.w_axis.w - vp.y_axis.w;
        if top.dot(sphere.center) + top_d < -sphere.radius {
            return false;
        }

        // Near plane: row3 + row2
        let near = Vec3::new(
            vp.w_axis.x + vp.z_axis.x,
            vp.w_axis.y + vp.z_axis.y,
            vp.w_axis.z + vp.z_axis.z,
        );
        let near_d = vp.w_axis.w + vp.z_axis.w;
        if near.dot(sphere.center) + near_d < -sphere.radius {
            return false;
        }

        // Far plane: row3 - row2
        let far = Vec3::new(
            vp.w_axis.x - vp.z_axis.x,
            vp.w_axis.y - vp.z_axis.y,
            vp.w_axis.z - vp.z_axis.z,
        );
        let far_d = vp.w_axis.w - vp.z_axis.w;
        if far.dot(sphere.center) + far_d < -sphere.radius {
            return false;
        }

        true
    }

    pub fn is_box_visible(&mut self, bbox: &AABox) -> bool {
        self.update_matrices();
        // Proper frustum culling for AABB
        // Extract planes from view-projection matrix
        let vp = self.view_projection_matrix;
        let min = bbox.min;
        let max = bbox.max;

        // Helper to test all 8 corners against a plane
        let test_plane = |normal: Vec3, d: f32| -> bool {
            // Find the positive vertex (furthest along plane normal)
            let p_vertex = Vec3::new(
                if normal.x >= 0.0 { max.x } else { min.x },
                if normal.y >= 0.0 { max.y } else { min.y },
                if normal.z >= 0.0 { max.z } else { min.z },
            );

            // If positive vertex is outside, box is outside
            normal.dot(p_vertex) + d >= 0.0
        };

        // Test against all 6 frustum planes
        // Left plane
        let left = Vec3::new(
            vp.w_axis.x + vp.x_axis.x,
            vp.w_axis.y + vp.x_axis.y,
            vp.w_axis.z + vp.x_axis.z,
        );
        let left_d = vp.w_axis.w + vp.x_axis.w;
        if !test_plane(left, left_d) {
            return false;
        }

        // Right plane
        let right = Vec3::new(
            vp.w_axis.x - vp.x_axis.x,
            vp.w_axis.y - vp.x_axis.y,
            vp.w_axis.z - vp.x_axis.z,
        );
        let right_d = vp.w_axis.w - vp.x_axis.w;
        if !test_plane(right, right_d) {
            return false;
        }

        // Bottom plane
        let bottom = Vec3::new(
            vp.w_axis.x + vp.y_axis.x,
            vp.w_axis.y + vp.y_axis.y,
            vp.w_axis.z + vp.y_axis.z,
        );
        let bottom_d = vp.w_axis.w + vp.y_axis.w;
        if !test_plane(bottom, bottom_d) {
            return false;
        }

        // Top plane
        let top = Vec3::new(
            vp.w_axis.x - vp.y_axis.x,
            vp.w_axis.y - vp.y_axis.y,
            vp.w_axis.z - vp.y_axis.z,
        );
        let top_d = vp.w_axis.w - vp.y_axis.w;
        if !test_plane(top, top_d) {
            return false;
        }

        // Near plane
        let near = Vec3::new(
            vp.w_axis.x + vp.z_axis.x,
            vp.w_axis.y + vp.z_axis.y,
            vp.w_axis.z + vp.z_axis.z,
        );
        let near_d = vp.w_axis.w + vp.z_axis.w;
        if !test_plane(near, near_d) {
            return false;
        }

        // Far plane
        let far = Vec3::new(
            vp.w_axis.x - vp.z_axis.x,
            vp.w_axis.y - vp.z_axis.y,
            vp.w_axis.z - vp.z_axis.z,
        );
        let far_d = vp.w_axis.w - vp.z_axis.w;
        if !test_plane(far, far_d) {
            return false;
        }

        true
    }
}

/// Rendering layer for organizing objects
#[derive(Debug)]
pub struct Layer {
    name: String,
    objects: Vec<Box<dyn RenderObject>>,
    visible: bool,
    sort_level: i32,
}

impl Layer {
    pub fn new(name: String) -> Self {
        Self {
            name,
            objects: Vec::new(),
            visible: true,
            sort_level: 0,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_object(&mut self, object: Box<dyn RenderObject>) {
        self.objects.push(object);
    }

    pub fn remove_object(&mut self, index: usize) -> Option<Box<dyn RenderObject>> {
        if index < self.objects.len() {
            Some(self.objects.remove(index))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn sort_level(&self) -> i32 {
        self.sort_level
    }

    pub fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level;
    }

    pub fn render(&mut self, info: &RenderInfo) -> W3DResult<()> {
        if !self.visible {
            return Ok(());
        }

        for object in &mut self.objects {
            object.render(info)?;
        }

        Ok(())
    }

    pub fn objects_slice(&self) -> &[Box<dyn RenderObject>] {
        &self.objects
    }

    pub fn objects_slice_mut(&mut self) -> &mut [Box<dyn RenderObject>] {
        &mut self.objects
    }
}

/// Scene class for managing the scene graph
#[derive(Debug)]
pub struct Scene {
    name: String,
    layers: Vec<Layer>,
    cameras: Vec<Camera>,
    active_camera: Option<usize>,
    ambient_light: Vec3,
}

impl Scene {
    pub fn new(name: String) -> Self {
        Self {
            name,
            layers: Vec::new(),
            cameras: Vec::new(),
            active_camera: None,
            ambient_light: Vec3::splat(0.2),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_layer(&mut self, layer: Layer) -> usize {
        let index = self.layers.len();
        self.layers.push(layer);
        index
    }

    pub fn get_layer(&self, index: usize) -> Option<&Layer> {
        self.layers.get(index)
    }

    pub fn get_layer_mut(&mut self, index: usize) -> Option<&mut Layer> {
        self.layers.get_mut(index)
    }

    pub fn find_layer(&self, name: &str) -> Option<usize> {
        self.layers.iter().position(|l| l.name == name)
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn add_camera(&mut self, camera: Camera) -> usize {
        let index = self.cameras.len();
        self.cameras.push(camera);
        if self.active_camera.is_none() {
            self.active_camera = Some(index);
        }
        index
    }

    pub fn get_camera(&self, index: usize) -> Option<&Camera> {
        self.cameras.get(index)
    }

    pub fn get_camera_mut(&mut self, index: usize) -> Option<&mut Camera> {
        self.cameras.get_mut(index)
    }

    pub fn find_camera(&self, name: &str) -> Option<usize> {
        self.cameras.iter().position(|c| c.name == name)
    }

    pub fn camera_count(&self) -> usize {
        self.cameras.len()
    }

    pub fn set_active_camera(&mut self, index: usize) {
        if index < self.cameras.len() {
            self.active_camera = Some(index);
        }
    }

    pub fn active_camera(&self) -> Option<&Camera> {
        self.active_camera.and_then(|idx| self.cameras.get(idx))
    }

    pub fn active_camera_mut(&mut self) -> Option<&mut Camera> {
        self.active_camera.and_then(|idx| self.cameras.get_mut(idx))
    }

    pub fn set_ambient_light(&mut self, color: Vec3) {
        self.ambient_light = color;
    }

    pub fn ambient_light(&self) -> Vec3 {
        self.ambient_light
    }

    pub fn render(&mut self, delta_time: f32, elapsed_time: f32) -> W3DResult<()> {
        let camera = self
            .active_camera_mut()
            .ok_or(W3DError::NotInitialized("No active camera".to_string()))?;

        camera.update_matrices();

        let render_info = RenderInfo {
            view_projection: camera.view_projection_matrix,
            view: camera.view_matrix,
            projection: camera.projection_matrix,
            camera_position: camera.position,
            delta_time,
            elapsed_time,
        };

        // Sort layers by sort level
        self.layers.sort_by_key(|layer| layer.sort_level());

        // Render each layer
        for layer in &mut self.layers {
            layer.render(&render_info)?;
        }

        Ok(())
    }

    pub fn clear(&mut self) {
        self.layers.clear();
        self.cameras.clear();
        self.active_camera = None;
    }

    pub fn total_object_count(&self) -> usize {
        self.layers.iter().map(|l| l.object_count()).sum()
    }
}

/// Scene builder for constructing scenes
#[derive(Debug)]
pub struct SceneBuilder {
    scene: Scene,
}

impl SceneBuilder {
    pub fn new(name: String) -> Self {
        Self {
            scene: Scene::new(name),
        }
    }

    pub fn with_camera(mut self, camera: Camera) -> Self {
        self.scene.add_camera(camera);
        self
    }

    pub fn with_layer(mut self, layer: Layer) -> Self {
        self.scene.add_layer(layer);
        self
    }

    pub fn with_ambient_light(mut self, color: Vec3) -> Self {
        self.scene.set_ambient_light(color);
        self
    }

    pub fn build(self) -> Scene {
        self.scene
    }
}

/// Frustum for culling
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [FrustumPlane; 6],
}

#[derive(Debug, Clone, Copy)]
struct FrustumPlane {
    normal: Vec3,
    distance: f32,
}

impl Frustum {
    pub fn from_view_projection(vp: Mat4) -> Self {
        let mut planes = [FrustumPlane {
            normal: Vec3::ZERO,
            distance: 0.0,
        }; 6];

        // Extract frustum planes from view-projection matrix
        // Left plane
        planes[0] = Self::normalize_plane(
            vp.w_axis.x + vp.x_axis.x,
            vp.w_axis.y + vp.x_axis.y,
            vp.w_axis.z + vp.x_axis.z,
            vp.w_axis.w + vp.x_axis.w,
        );

        // Right plane
        planes[1] = Self::normalize_plane(
            vp.w_axis.x - vp.x_axis.x,
            vp.w_axis.y - vp.x_axis.y,
            vp.w_axis.z - vp.x_axis.z,
            vp.w_axis.w - vp.x_axis.w,
        );

        // Bottom plane
        planes[2] = Self::normalize_plane(
            vp.w_axis.x + vp.y_axis.x,
            vp.w_axis.y + vp.y_axis.y,
            vp.w_axis.z + vp.y_axis.z,
            vp.w_axis.w + vp.y_axis.w,
        );

        // Top plane
        planes[3] = Self::normalize_plane(
            vp.w_axis.x - vp.y_axis.x,
            vp.w_axis.y - vp.y_axis.y,
            vp.w_axis.z - vp.y_axis.z,
            vp.w_axis.w - vp.y_axis.w,
        );

        // Near plane
        planes[4] = Self::normalize_plane(
            vp.w_axis.x + vp.z_axis.x,
            vp.w_axis.y + vp.z_axis.y,
            vp.w_axis.z + vp.z_axis.z,
            vp.w_axis.w + vp.z_axis.w,
        );

        // Far plane
        planes[5] = Self::normalize_plane(
            vp.w_axis.x - vp.z_axis.x,
            vp.w_axis.y - vp.z_axis.y,
            vp.w_axis.z - vp.z_axis.z,
            vp.w_axis.w - vp.z_axis.w,
        );

        Self { planes }
    }

    fn normalize_plane(x: f32, y: f32, z: f32, w: f32) -> FrustumPlane {
        let length = (x * x + y * y + z * z).sqrt();
        FrustumPlane {
            normal: Vec3::new(x / length, y / length, z / length),
            distance: w / length,
        }
    }

    pub fn is_sphere_visible(&self, sphere: &BoundingSphere) -> bool {
        for plane in &self.planes {
            let dist = plane.normal.dot(sphere.center) + plane.distance;
            if dist < -sphere.radius {
                return false;
            }
        }
        true
    }

    pub fn is_box_visible(&self, bbox: &AABox) -> bool {
        for plane in &self.planes {
            let mut p = bbox.min;
            if plane.normal.x >= 0.0 {
                p.x = bbox.max.x;
            }
            if plane.normal.y >= 0.0 {
                p.y = bbox.max.y;
            }
            if plane.normal.z >= 0.0 {
                p.z = bbox.max.z;
            }

            if plane.normal.dot(p) + plane.distance < 0.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_creation() {
        let camera = Camera::perspective(
            "main".to_string(),
            60.0_f32.to_radians(),
            16.0 / 9.0,
            0.1,
            1000.0,
        );
        assert_eq!(camera.name(), "main");
        assert_eq!(camera.position(), Vec3::ZERO);
    }

    #[test]
    fn test_camera_transform() {
        let mut camera = Camera::new("test".to_string());
        camera.set_position(Vec3::new(0.0, 5.0, 10.0));
        camera.look_at(Vec3::ZERO, Vec3::Y);

        let forward = camera.forward();
        let expected_forward = (Vec3::ZERO - camera.position()).normalize();

        assert!((forward - expected_forward).length() < 0.1);
    }

    #[test]
    fn test_layer() {
        let layer = Layer::new("test_layer".to_string());

        assert_eq!(layer.name(), "test_layer");
        assert_eq!(layer.object_count(), 0);
        assert!(layer.is_visible());
    }

    #[test]
    fn test_scene() {
        let mut scene = Scene::new("test_scene".to_string());

        let camera = Camera::new("camera1".to_string());
        scene.add_camera(camera);

        let layer = Layer::new("layer1".to_string());
        scene.add_layer(layer);

        assert_eq!(scene.camera_count(), 1);
        assert_eq!(scene.layer_count(), 1);
        assert!(scene.active_camera().is_some());
    }

    #[test]
    fn test_scene_builder() {
        let scene = SceneBuilder::new("built_scene".to_string())
            .with_camera(Camera::new("cam".to_string()))
            .with_layer(Layer::new("layer".to_string()))
            .with_ambient_light(Vec3::splat(0.5))
            .build();

        assert_eq!(scene.name(), "built_scene");
        assert_eq!(scene.camera_count(), 1);
        assert_eq!(scene.layer_count(), 1);
    }

    #[test]
    fn test_frustum_sphere_culling() {
        let vp = Mat4::IDENTITY;
        let frustum = Frustum::from_view_projection(vp);

        let sphere = BoundingSphere::new(Vec3::ZERO, 1.0);
        assert!(frustum.is_sphere_visible(&sphere));
    }
}
