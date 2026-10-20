//! W3DView movements operate on this view's actual fields at one admitted phase.
use super::camera::motion::MotionAdmission;
use super::{
    SessionView,
    waypoint::{Point, View, WaypointMotion},
};

impl SessionView {
    /// Original rotateCamera admission captures the receiving View time before
    /// any later modifier. A path and object-tracking rotation are replaced.
    pub(crate) fn install_rotate(
        &mut self,
        rotations: f32,
        seconds: f32,
        ease_in: f32,
        ease_out: f32,
        speed: f32,
    ) {
        self.camera.rotation.time = super::camera::rotation::RotationTime::constant(speed as i32);
        self.scripted.object_tracking = None;
        self.scripted.script_look_toward_object_id = None;
        self.scripted.motion.active = false;
        self.scripted.path_arrived = false;
        self.camera
            .install_rotate(rotations, seconds, ease_in, ease_out);
    }
    pub(crate) fn install_look_toward(
        &mut self,
        from: glam::Vec3,
        target: glam::Vec3,
        seconds: f32,
        ease_in: f32,
        ease_out: f32,
        reverse: bool,
        speed: f32,
    ) -> bool {
        // Original prefix clears rcInfo.trackObject and hold count before its
        // direction check. Model the initialized Target/Angle union explicitly.
        // Rust's cache is initialized at admission; exact source union parity
        // is defined after an actual tracking offer populates the C++ cache.
        if let Some(tracking) = self.scripted.object_tracking.take() {
            let id = self.scripted.script_look_toward_object_id.unwrap_or(0);
            self.camera.camera_yaw_start = f32::from_bits(id);
            if self.camera.camera_yaw_target.is_some() {
                self.camera.camera_yaw_target = Some(tracking.last_position.x);
            }
        }
        self.scripted.script_look_toward_object_id = None;
        if !self
            .camera
            .install_look_toward(from, target, seconds, ease_in, ease_out, reverse)
        {
            return false;
        }
        self.camera.rotation.time = super::camera::rotation::RotationTime::constant(speed as i32);
        self.scripted.object_tracking = None;
        self.scripted.script_look_toward_object_id = None;
        self.scripted.motion.active = false;
        self.scripted.path_arrived = false;
        true
    }

    fn motion_parts<'a>(&'a mut self, speed: &'a mut f32) -> (&'a mut WaypointMotion, View<'a>) {
        let (ground, offset, pitch, yaw) = self.camera.native_camera.path_parts();
        (
            &mut self.scripted.motion,
            View {
                position_x: &mut self.target.x,
                position_y: &mut self.target.z,
                position_z: &mut self.target.y,
                angle: &mut self.camera.camera_yaw_radians,
                ground,
                camera_offset: offset,
                basis_pitch_degrees: pitch,
                basis_yaw_degrees: yaw,
                time_multiplier: speed,
                frozen: &mut self.scripted.script_camera_freeze_time,
                arrived: &mut self.scripted.path_arrived,
                constraint: &mut self.camera.scripted_camera_constraint_widen,
            },
        )
    }
    pub(crate) fn install_motion(
        &mut self,
        motion: WaypointMotion,
        instant: bool,
        speed: &mut f32,
        disabled: bool,
    ) {
        self.camera.camera_yaw_target = None;
        self.scripted.path_arrived = false;
        self.scripted.motion = motion;
        if instant {
            let (motion, mut view) = self.motion_parts(speed);
            motion.instant_admission(&mut view, disabled);
        }
    }
    pub(crate) fn modify_path_look(
        &mut self,
        point: Point,
        final_look: bool,
        speed: &mut f32,
        disabled: bool,
    ) {
        if self.camera.camera_yaw_target.is_some() {
            return;
        }
        let (motion, mut view) = self.motion_parts(speed);
        if final_look {
            motion.final_look_toward(point);
        } else {
            motion.look_toward(point, &mut view, disabled);
        }
    }
    pub(crate) fn advance_camera_movements(
        &mut self,
        admission: MotionAdmission,
        frame_ms: i32,
        speed: &mut f32,
    ) -> bool {
        // Capture rotation before stepping it: source uses rotate ELSE path,
        // including the offer on which rotation reaches its final frame.
        let rotating = self.camera.camera_yaw_target.is_some();
        let travelling = self.scripted.motion.active;
        let active = self.camera.advance_scalar_movements(
            admission,
            self.target,
            self.scripted.object_tracking.is_some(),
            speed,
        );
        if !admission.advances() {
            return active || travelling;
        }
        if rotating {
            if self.camera.camera_yaw_target.is_none() {
                self.scripted.script_camera_freeze_time = false;
            }
        } else if travelling {
            #[cfg(feature = "game_client")]
            {
                self.camera.native_filter_previous_target = self.target;
            }
            let (motion, mut view) = self.motion_parts(speed);
            motion.step(frame_ms, &mut view, admission.disable_camera_movement);
        }
        active || travelling
    }
    pub(crate) fn movement_finished(&self) -> bool {
        !self.scripted.motion.active
            && self.camera.camera_yaw_target.is_none()
            && self.camera.camera_zoom_target.is_none()
            && self.camera.camera_pitch_target.is_none()
    }
}

#[cfg(test)]
mod scalar_time_tests {
    use super::*;
    #[test]
    fn scalar_time_obeys_admitted_phase_and_disabled_expiry_without_late_path_step() {
        let mut view = SessionView::default();
        let mut speed = 2.;
        view.install_rotate(0.25, 0.132, 0., 0., speed);
        view.camera.rotation.time.end = 6;
        let normal = MotionAdmission {
            update_views: true,
            fast_camera_only: false,
            paused: false,
            debug_frozen: false,
            disable_camera_movement: false,
        };
        for blocked in [
            MotionAdmission {
                paused: true,
                ..normal
            },
            MotionAdmission {
                debug_frozen: true,
                ..normal
            },
            MotionAdmission {
                update_views: false,
                ..normal
            },
        ] {
            view.advance_camera_movements(blocked, 33, &mut speed);
            assert_eq!(view.camera.camera_yaw_frame, 0);
            assert_eq!(speed, 2.);
        }
        view.advance_camera_movements(normal, 33, &mut speed);
        assert_eq!(speed, 3.);
        view.advance_camera_movements(
            MotionAdmission {
                disable_camera_movement: true,
                ..normal
            },
            33,
            &mut speed,
        );
        assert_eq!(view.camera.camera_yaw_frame, 2);
        assert_eq!(speed, 3.);
        view.advance_camera_movements(normal, 33, &mut speed);
        assert_eq!(speed, 5.);
        view.advance_camera_movements(
            MotionAdmission {
                disable_camera_movement: true,
                ..normal
            },
            33,
            &mut speed,
        );
        assert_eq!(speed, 5.);
        assert!(view.camera.camera_yaw_target.is_none());
        view.advance_camera_movements(normal, 33, &mut speed);
        assert_eq!(speed, 5., "inactive endpoint cannot publish a late speed");
        view.install_rotate(0.25, 0.033, 0., 0., speed);
        assert_eq!(view.camera.rotation.time.start, 5);
        assert_eq!(view.camera.rotation.time.end, 5);
        // Source near-zero look preserves endpoints/current frame/time while
        // changing the turn frame count and switching away from tracking.
        assert!(!view.install_look_toward(view.target, view.target, 1., 0., 0., false, 9.));
        assert_eq!(view.camera.rotation.time.start, 5);
    }
}
