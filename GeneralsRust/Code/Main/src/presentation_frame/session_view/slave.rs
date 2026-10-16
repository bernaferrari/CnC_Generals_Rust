//! The final source W3DView bone-matrix slam, after ordinary view/HAG work.
use super::*;
use glam::Mat4;
impl SessionView {
    pub(crate) fn commit_native_slave_world(&mut self, camera_world: Mat4) -> bool {
        self.camera
            .commit_native_slave_world(&mut self.target, camera_world)
    }
}
impl camera::SessionCamera {
    pub(crate) fn commit_native_slave_world(
        &mut self,
        target: &mut Vec3,
        camera_world: Mat4,
    ) -> bool {
        let Ok(view) = camera::pose::inverse_camera_transform(camera_world) else {
            return false;
        };
        let eye = camera_world.w_axis.truncate();
        self.view_matrix = view;
        self.camera_position = eye;
        *target = eye;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_slave_commit_preserves_reflection_roll_and_canonical_position_without_hag_step() {
        let mut view = SessionView::default();
        view.camera.camera_yaw_radians = 0.7;
        let hag = serde_json::to_vec(&view.camera.native_camera.capture_continuation()).unwrap();
        let world = Mat4::from_cols(
            glam::Vec4::new(0., 1., 0., 0.),
            glam::Vec4::new(1., 0., 0., 0.),
            glam::Vec4::new(0., 0., 1., 0.),
            glam::Vec4::new(3., 5., 7., 1.),
        );
        assert!(world.determinant() < 0.);
        assert!(view.commit_native_slave_world(world));
        assert_eq!(view.target, Vec3::new(3., 5., 7.));
        assert_eq!(view.camera.camera_position, view.target);
        assert_eq!(view.camera.view_matrix, world.inverse());
        assert_eq!(view.camera.camera_yaw_radians, 0.7);
        assert_eq!(
            serde_json::to_vec(&view.camera.native_camera.capture_continuation()).unwrap(),
            hag
        );
        let before = view.camera.view_matrix;
        assert!(!view.commit_native_slave_world(Mat4::ZERO));
        assert_eq!(view.camera.view_matrix, before);
        let mut projective = Mat4::IDENTITY;
        projective.x_axis.w = 0.25;
        assert!(!view.commit_native_slave_world(projective));
        assert_eq!(view.camera.view_matrix, before);
    }
}
