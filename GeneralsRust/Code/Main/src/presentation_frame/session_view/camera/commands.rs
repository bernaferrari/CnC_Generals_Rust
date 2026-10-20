//! Synchronous command admission into the receiving camera's scalar fields.
use super::SessionCamera;
use glam::Vec3;

fn milliseconds_duration(seconds: f32) -> f32 {
    (((seconds * 1000.) as i32).max(1) as f32) / 1000.
}

impl SessionCamera {
    pub(crate) fn install_zoom(&mut self, zoom: f32, seconds: f32, ease_in: f32, ease_out: f32) {
        self.camera_zoom_start = self.camera_zoom;
        self.camera_zoom_target = Some(zoom);
        self.camera_zoom_duration = milliseconds_duration(seconds);
        self.camera_zoom_frame = 0;
        self.camera_zoom_ease_in = ease_in.max(0.);
        self.camera_zoom_ease_out = ease_out.max(0.);
    }
    pub(crate) fn install_pitch(&mut self, pitch: f32, seconds: f32, ease_in: f32, ease_out: f32) {
        self.camera_pitch_start = self.camera_fx_pitch;
        self.camera_pitch_target = Some(if pitch.is_finite() { pitch } else { 1. });
        self.camera_pitch_duration = milliseconds_duration(seconds);
        self.camera_pitch_frame = 0;
        self.camera_pitch_ease_in = ease_in.max(0.);
        self.camera_pitch_ease_out = ease_out.max(0.);
    }
    pub(crate) fn install_rotate(
        &mut self,
        rotations: f32,
        seconds: f32,
        ease_in: f32,
        ease_out: f32,
    ) {
        let target = self.camera_yaw_radians + rotations * std::f32::consts::TAU;
        self.install_heading(target, seconds, ease_in, ease_out);
    }
    fn install_heading(&mut self, target: f32, seconds: f32, ease_in: f32, ease_out: f32) {
        self.camera_yaw_start = self.camera_yaw_radians;
        self.camera_yaw_target = Some(target);
        self.rotation = super::rotation::RotationTiming::authored(
            seconds,
            ease_in,
            ease_out,
            self.rotation.time,
        );
        self.camera_yaw_frame = 0;
    }
    /// W3DView::rotateCameraTowardPosition returns before cancelling a path
    /// when the horizontal direction is shorter than 0.1 source units.
    pub(crate) fn install_look_toward(
        &mut self,
        from: Vec3,
        target: Vec3,
        seconds: f32,
        ease_in: f32,
        ease_out: f32,
        reverse: bool,
    ) -> bool {
        // Source rcInfo.numFrames is updated before the near-zero return.
        // Do not reset curFrame, old angle/ease, or active flag on that path.
        self.rotation.update_count(seconds);
        let dx = target.x - from.x;
        let dy = target.z - from.z;
        let length = f64::from(dx * dx + dy * dy).sqrt() as f32;
        if length < 0.1 || !length.is_finite() {
            return false;
        }
        let mut angle = f64::from(dx / length).acos() as f32;
        if dy < 0. {
            angle = -angle;
        }
        angle -= std::f32::consts::PI / 2.;
        while angle < -std::f32::consts::PI {
            angle += std::f32::consts::TAU;
        }
        while angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        if reverse {
            if self.camera_yaw_radians < angle {
                angle -= std::f32::consts::TAU;
            } else {
                angle += std::f32::consts::TAU;
            }
        }
        self.install_heading(angle, seconds, ease_in, ease_out);
        true
    }
}
