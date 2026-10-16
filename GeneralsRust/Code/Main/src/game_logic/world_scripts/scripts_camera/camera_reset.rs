//! W3DView::resetCamera is motion admission, not three immediate player setters.
use super::*;
impl GameLogic {
    pub(crate) fn admit_camera_reset_motion(&mut self, last: CameraResetRequest) {
        self.pending_camera_rotate = None;
        self.pending_camera_zoom_reset = true;
        self.pending_camera_zoom_reset_duration = last.duration_seconds.max(0.0);
        self.pending_camera_zoom_reset_ease_in = last.ease_in_seconds.max(0.0);
        self.pending_camera_zoom_reset_ease_out = last.ease_out_seconds.max(0.0);
        self.start_camera_move_to_oriented(
            CameraMoveToRequest {
                position: last.position,
                seconds: last.duration_seconds,
                camera_stutter_seconds: 0.,
                ease_in_seconds: last.ease_in_seconds,
                ease_out_seconds: last.ease_out_seconds,
            },
            false,
        );
        self.session_view.scripted.motion.angles[2] = 0.;
        self.session_view.camera.camera_yaw_radians = self.session_view.scripted.motion.angles[0];
        if let Some(zoom) = self.owned_script_camera_max_zoom_at(last.position) {
            self.session_view.camera.install_zoom(
                zoom,
                last.duration_seconds,
                last.ease_in_seconds,
                last.ease_out_seconds,
            );
        }
        self.session_view.camera.install_pitch(
            1.,
            last.duration_seconds,
            last.ease_in_seconds,
            last.ease_out_seconds,
        );
    }
    pub(in crate::game_logic::game_logic) fn owned_script_camera_max_zoom_at(
        &self,
        target: Vec3,
    ) -> Option<f32> {
        let owner = self.world_services.terrain();
        let terrain = match owner.read() {
            Ok(terrain) => terrain,
            Err(_) => {
                log::error!("receiving camera zoom terrain poisoned");
                return None;
            }
        };
        let height = crate::presentation_frame::camera::height::height_around(target, |x, z| {
            Some(terrain.get_ground_height(x, z, None))
        })?;
        let max_height = self.receiving_camera_max_hag();
        Some(
            self.session_view
                .camera
                .native_camera
                .default_zoom(height, max_height),
        )
    }
}
#[cfg(test)]
#[path = "camera_reset_owner_tests.rs"]
mod tests;
