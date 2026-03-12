/// Screen-space picking (ported from intersec.inl Get_Screen_Ray)
///
/// Generates rays from screen coordinates for mouse picking and selection
use glam::{Mat4, Vec3, Vec4};

/// Camera information for screen-space picking
#[derive(Debug, Clone)]
pub struct Camera {
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub viewport_width: f32,
    pub viewport_height: f32,
}

impl Camera {
    pub fn new(
        view_matrix: Mat4,
        projection_matrix: Mat4,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Self {
        Self {
            view_matrix,
            projection_matrix,
            viewport_width,
            viewport_height,
        }
    }
}

/// Screen-space ray for picking
#[derive(Debug, Clone)]
pub struct ScreenRay {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl ScreenRay {
    /// Generate ray from screen coordinates
    /// screen_x, screen_y are in pixel coordinates (0,0 = top-left)
    pub fn from_screen_coords(camera: &Camera, screen_x: f32, screen_y: f32) -> Self {
        // Convert screen coordinates to normalized device coordinates (-1 to 1)
        let ndc_x = (2.0 * screen_x) / camera.viewport_width - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y) / camera.viewport_height; // Flip Y

        // Convert NDC to clip space (z = -1 for near plane, w = 1)
        let clip_coords = Vec4::new(ndc_x, ndc_y, -1.0, 1.0);

        // Calculate inverse matrices
        let view_proj = camera.projection_matrix * camera.view_matrix;
        let inv_view_proj = view_proj.inverse();

        // Unproject to world space
        let world_coords_near = inv_view_proj * clip_coords;
        let world_coords_near = world_coords_near / world_coords_near.w;

        // Also unproject far plane
        let clip_coords_far = Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        let world_coords_far = inv_view_proj * clip_coords_far;
        let world_coords_far = world_coords_far / world_coords_far.w;

        // Calculate ray
        let origin = Vec3::new(
            world_coords_near.x,
            world_coords_near.y,
            world_coords_near.z,
        );
        let far_point = Vec3::new(world_coords_far.x, world_coords_far.y, world_coords_far.z);
        let direction = (far_point - origin).normalize();

        Self { origin, direction }
    }

    /// Alternative method: generate ray from camera position and screen coords
    /// Useful for perspective projections
    pub fn from_camera_perspective(
        camera_pos: Vec3,
        camera_forward: Vec3,
        camera_right: Vec3,
        camera_up: Vec3,
        fov_y: f32,
        aspect_ratio: f32,
        screen_x: f32,
        screen_y: f32,
        viewport_width: f32,
        viewport_height: f32,
    ) -> Self {
        // Convert screen to normalized coordinates (-1 to 1)
        let ndc_x = (2.0 * screen_x) / viewport_width - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y) / viewport_height;

        // Calculate ray direction in view space
        let tan_half_fov = (fov_y / 2.0).tan();
        let view_x = ndc_x * aspect_ratio * tan_half_fov;
        let view_y = ndc_y * tan_half_fov;

        // Transform to world space
        let direction = (camera_forward + camera_right * view_x + camera_up * view_y).normalize();

        Self {
            origin: camera_pos,
            direction,
        }
    }

    /// Get point along ray at distance t
    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Pick result for screen-space selection
#[derive(Debug, Clone)]
pub struct PickResult {
    pub hit: bool,
    pub distance: f32,
    pub position: Vec3,
    pub normal: Vec3,
    pub object_id: Option<u32>,
}

impl PickResult {
    pub fn new() -> Self {
        Self {
            hit: false,
            distance: f32::MAX,
            position: Vec3::ZERO,
            normal: Vec3::ZERO,
            object_id: None,
        }
    }

    pub fn with_hit(distance: f32, position: Vec3, normal: Vec3) -> Self {
        Self {
            hit: true,
            distance,
            position,
            normal,
            object_id: None,
        }
    }
}

impl Default for PickResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_screen_ray_generation() {
        // Create a simple camera looking down -Z
        let view = Mat4::look_at_rh(
            Vec3::new(0.0, 0.0, 10.0), // eye
            Vec3::ZERO,                // target
            Vec3::Y,                   // up
        );

        let projection = Mat4::perspective_rh(PI / 4.0, 16.0 / 9.0, 0.1, 100.0);

        let camera = Camera::new(view, projection, 1920.0, 1080.0);

        // Ray through center of screen should point at target
        let ray = ScreenRay::from_screen_coords(&camera, 960.0, 540.0);

        // Direction should be roughly -Z
        assert!(ray.direction.z < 0.0);
        assert!(ray.direction.x.abs() < 0.1);
        assert!(ray.direction.y.abs() < 0.1);
    }

    #[test]
    fn test_perspective_ray() {
        let camera_pos = Vec3::new(0.0, 0.0, 10.0);
        let forward = Vec3::NEG_Z;
        let right = Vec3::X;
        let up = Vec3::Y;

        let ray = ScreenRay::from_camera_perspective(
            camera_pos,
            forward,
            right,
            up,
            PI / 4.0,
            16.0 / 9.0,
            960.0,
            540.0,
            1920.0,
            1080.0,
        );

        // Center ray should be roughly forward
        assert!(ray.direction.z < 0.0);
        assert!(ray.origin == camera_pos);
    }

    #[test]
    fn test_point_at() {
        let ray = ScreenRay {
            origin: Vec3::ZERO,
            direction: Vec3::X,
        };

        let point = ray.point_at(5.0);
        assert_eq!(point, Vec3::new(5.0, 0.0, 0.0));
    }
}
