//! W3DView movements operate on this view's actual fields at one admitted phase.
use super::camera::motion::MotionAdmission;
use super::{
    SessionView,
    waypoint::{Point, View, WaypointMotion},
};

impl SessionView {
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
