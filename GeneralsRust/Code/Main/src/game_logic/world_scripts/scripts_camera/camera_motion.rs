//! Camera commands change the receiving view; only admitted views advance it.
//! W3DView.cpp setupWaypointPath/moveAlongWaypointPath/updateCameraMovements.
use super::*;
use crate::presentation_frame::camera::motion::MotionAdmission;
use crate::presentation_frame::waypoint::{Point, WaypointMotion};

const CAMERA_FRAME_MS: i32 = 33;
fn camera_milliseconds(seconds: f32) -> i32 {
    (seconds * 1000.0) as i32
}

impl GameLogic {
    pub(in crate::game_logic::game_logic) fn start_camera_path_move(
        &mut self,
        request: CameraPathRequest,
    ) {
        let chain = match super::path_admission::admit_camera_chain(
            self,
            &request.waypoint,
            self.session_view.target,
        ) {
            Ok(Some(chain)) => chain,
            Ok(None) => {
                self.mark_script_camera_movement_maybe_finished();
                return;
            }
            Err(error) => {
                log::error!("receiving camera path admission: {error}");
                self.mark_script_camera_movement_maybe_finished();
                return;
            }
        };
        let motion = WaypointMotion::along_chain(
            &chain.points,
            Point::from_world(self.session_view.target),
            self.session_view.camera.camera_yaw_radians,
            self.session_view
                .camera
                .native_camera
                .basis()
                .ground_level(),
            chain.final_ground,
            self.visual_speed_multiplier as i32,
            camera_milliseconds(request.seconds),
            camera_milliseconds(request.camera_stutter_seconds),
            CAMERA_FRAME_MS,
            true,
            request.ease_in_seconds * 1000.0,
            request.ease_out_seconds * 1000.0,
            gamelogic::common::MAP_XY_FACTOR,
        );
        if let Err(error) = motion.validate() {
            log::error!("receiving camera path invalid: {error}");
            self.mark_script_camera_movement_maybe_finished();
            return;
        }
        self.pending_camera_rotate = None;
        self.pending_camera_look_toward = None;
        let disable_camera_movement = self.camera_movement_disabled();
        self.session_view.install_motion(
            motion,
            false,
            &mut self.visual_speed_multiplier,
            disable_camera_movement,
        );
        self.mark_script_camera_movement_maybe_finished();
    }
    pub(in crate::game_logic::game_logic) fn start_camera_move_to(
        &mut self,
        request: CameraMoveToRequest,
    ) {
        self.start_camera_move_to_oriented(request, true);
    }
    pub(in crate::game_logic::game_logic) fn start_camera_move_to_oriented(
        &mut self,
        request: CameraMoveToRequest,
        orient: bool,
    ) {
        if !request.position.is_finite() {
            log::error!("non-finite camera destination");
            self.mark_script_camera_movement_maybe_finished();
            return;
        }
        let final_ground = {
            let owner = self.world_services.terrain();
            owner.read().ok().map(|terrain| {
                terrain.get_ground_height(request.position.x, request.position.z, None)
            })
        };
        let Some(final_ground) = final_ground else {
            log::error!("receiving camera terrain poisoned");
            // Release the terrain loan before refreshing the derived hook.
            self.mark_script_camera_movement_maybe_finished();
            return;
        };
        let motion = WaypointMotion::move_to(
            Point::from_world(request.position),
            Point::from_world(self.session_view.target),
            self.session_view.camera.camera_yaw_radians,
            self.session_view
                .camera
                .native_camera
                .basis()
                .ground_level(),
            final_ground,
            self.visual_speed_multiplier as i32,
            camera_milliseconds(request.seconds),
            orient,
            request.ease_in_seconds * 1000.0,
            request.ease_out_seconds * 1000.0,
        );
        if let Err(error) = motion.validate() {
            log::error!("receiving camera destination invalid: {error}");
            self.mark_script_camera_movement_maybe_finished();
            return;
        }
        self.pending_camera_rotate = None;
        self.pending_camera_look_toward = None;
        let disable_camera_movement = self.camera_movement_disabled();
        self.session_view.install_motion(
            motion,
            true,
            &mut self.visual_speed_multiplier,
            disable_camera_movement,
        );
        self.mark_script_camera_movement_maybe_finished();
    }
    pub(in crate::game_logic::game_logic) fn is_script_camera_movement_finished_now(&self) -> bool {
        self.session_view.movement_finished()
    }
    /// Motion-blur query exceptions never delete live motion state.
    pub(crate) fn camera_movement_finished(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            use game_client::display::view::{FilterMode, FilterType};
            if self.session_view.filter.filter() == FilterType::MotionBlur
                && matches!(
                    self.session_view.filter.mode(),
                    FilterMode::MBInAndOutAlpha
                        | FilterMode::MBInAndOutSaturate
                        | FilterMode::MBInAlpha
                        | FilterMode::MBOutAlpha
                        | FilterMode::MBInSaturate
                        | FilterMode::MBOutSaturate
                )
            {
                return true;
            }
        }
        self.session_view.movement_finished()
    }
    pub(super) fn script_camera_has_orientation_motion(&self) -> bool {
        let camera = &self.session_view.camera;
        camera.rotation_registers.active()
            || camera.camera_zoom_target.is_some()
            || camera.camera_pitch_target.is_some()
    }
    pub(in crate::game_logic::game_logic::world_scripts) fn clear_script_camera_orientation_remaining(
        &mut self,
    ) {
        let camera = &mut self.session_view.camera;
        camera.rotation_registers.finish();
        camera.camera_zoom_target = None;
        camera.camera_pitch_target = None;
    }
    pub(super) fn mark_script_camera_movement_maybe_finished(&mut self) {
        self.mission_scripts
            .set_camera_movement_finished(self.session_view.movement_finished());
    }
    pub(in crate::game_logic::game_logic) fn script_camera_remaining_seconds(&self) -> f32 {
        let camera = &self.session_view.camera;
        if camera.rotation_registers.active() {
            let frames = camera.rotation.turn_frames
                + if camera.rotation_registers.is_tracking() {
                    camera.rotation_registers.hold_frames()
                } else {
                    0
                };
            return frames.saturating_sub(camera.camera_yaw_frame) as f32 * CAMERA_FRAME_MS as f32
                / 1000.0;
        }
        let motion = &self.session_view.scripted.motion;
        if motion.active {
            (motion.total_time_ms - motion.elapsed_ms) as f32 / 1000.0
        } else {
            0.0
        }
    }
    pub(in crate::game_logic::game_logic) fn is_script_camera_angle_frozen(&self) -> bool {
        let motion = &self.session_view.scripted.motion;
        motion.active
            && motion.angles[1..=motion.num_waypoints]
                .iter()
                .all(|angle| *angle == motion.angles[0])
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_default(
        &mut self,
        request: CameraSetDefaultRequest,
    ) -> Result<(), String> {
        if !request.pitch.is_finite() {
            return Err("nonfinite camera default pitch".into());
        }
        let mut settings = self.session_view.camera.settings;
        settings.capture_default(request.max_height)?;
        self.session_view.scripted.script_default_camera_pitch = request.pitch;
        self.session_view.scripted.script_default_camera_angle = 0.;
        self.session_view.scripted.script_default_camera_max_height = request.max_height;
        self.session_view.camera.settings = settings;
        Ok(())
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_freeze_time(&mut self) {
        self.session_view.scripted.script_camera_freeze_time = true;
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_freeze_angle(&mut self) {
        // Original rewrites the stored path table and/or current rotation;
        // there is no armed request applied to a later unrelated movement.
        self.session_view.scripted.motion.freeze_angle();
        let camera = &mut self.session_view.camera;
        camera
            .rotation_registers
            .freeze_angle(camera.camera_yaw_radians);
        self.pending_camera_look_toward = None;
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_look_toward(
        &mut self,
        position: Vec3,
        final_look: bool,
    ) {
        let disable_camera_movement = self.camera_movement_disabled();
        self.session_view.modify_path_look(
            Point::from_world(position),
            final_look,
            &mut self.visual_speed_multiplier,
            disable_camera_movement,
        );
        self.mark_script_camera_movement_maybe_finished();
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_final_speed_multiplier(
        &mut self,
        request: &CameraModFinalSpeedMultiplierRequest,
    ) {
        if self.session_view.camera.rotation_registers.active() {
            self.session_view.camera.rotation.time.end = request.multiplier;
            return;
        }
        self.session_view
            .scripted
            .motion
            .final_time_multiplier(request.multiplier, &mut self.visual_speed_multiplier);
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_rolling_average(
        &mut self,
        request: &CameraModRollingAverageRequest,
    ) {
        self.session_view
            .scripted
            .motion
            .rolling_average(request.frames);
    }
    pub(crate) fn apply_visual_speed_multiplier(&mut self, request: &VisualSpeedMultiplierRequest) {
        self.visual_speed_multiplier = request.multiplier.max(1) as f32;
    }
    pub(in crate::game_logic::game_logic) fn apply_set_fps_limit(
        &mut self,
        request: &SetFpsLimitRequest,
    ) {
        self.pending_script_fps_limit = Some(request.fps);
    }
    /// Explicit native/headless view admission. The logic evaluator never
    /// advances this clock, and an offer always advances scalar axes once.
    pub(crate) fn advance_camera_view(
        &mut self,
        admission: MotionAdmission,
        frame_ms: i32,
    ) -> bool {
        let changed = self.session_view.advance_camera_movements(
            admission,
            frame_ms,
            &mut self.visual_speed_multiplier,
        );
        self.advance_camera_object_tracking(admission);
        self.mark_script_camera_movement_maybe_finished();
        changed
    }
    pub(crate) fn headless_camera_view_offer(
        &mut self,
        frame_ms: i32,
    ) -> gamelogic::GameLogicResult<bool> {
        self.headless_camera_view_offer_with_script_engine(frame_ms, None)
    }

    pub(crate) fn headless_camera_view_offer_with_script_engine(
        &mut self,
        frame_ms: i32,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.advance_receiving_camera_view(
            true,
            false,
            self.is_paused(),
            self.camera_movement_disabled(),
            frame_ms,
            interpreter,
        )
    }
    pub(super) fn install_camera_object_tracking(
        &mut self,
        object_id: u32,
        seconds: f32,
        hold_seconds: f32,
        ease_in: f32,
        ease_out: f32,
    ) {
        let milliseconds = camera_milliseconds(seconds).max(1);
        let hold_frames = (camera_milliseconds(hold_seconds).max(0) / CAMERA_FRAME_MS) as u32;
        let multiplier = self.visual_speed_multiplier as i32;
        self.session_view.camera.rotation.time =
            crate::presentation_frame::camera::rotation::RotationTime::constant(multiplier);
        self.session_view
            .camera
            .rotation_registers
            .begin_tracking(object_id, hold_frames);
        self.session_view.scripted.motion.active = false;
        self.session_view.scripted.path_arrived = false;
        let camera = &mut self.session_view.camera;
        camera.camera_yaw_frame = 0;
        camera.rotation = crate::presentation_frame::camera::rotation::RotationTiming::milliseconds(
            milliseconds,
            ease_in,
            ease_out,
            camera.rotation.time,
        );
        self.mark_script_camera_movement_maybe_finished();
    }
    fn advance_camera_object_tracking(&mut self, admission: MotionAdmission) {
        if !admission.advances()
            || !self.session_view.camera.rotation_registers.active()
            || !self.session_view.camera.rotation_registers.is_tracking()
        {
            return;
        }
        let camera = &mut self.session_view.camera;
        camera.camera_yaw_frame = camera.camera_yaw_frame.saturating_add(1);
        let frame = camera.camera_yaw_frame;
        let final_frame = camera.rotation.turn_frames + camera.rotation_registers.hold_frames();
        if !admission.disable_camera_movement && frame <= final_frame {
            let current = camera
                .rotation_registers
                .tracking_id()
                .filter(|id| *id != 0)
                .and_then(|id| self.objects.get(&ObjectId(id)))
                .map(|object| object.get_position());
            camera.rotation_registers.observe_tracking(current);
            let Some(cached) = camera.rotation_registers.cached_target() else {
                // No C++ initialized target position exists yet. Do not invent
                // admission/zero/current-pose coordinates. Expire normally.
                if frame >= final_frame {
                    camera.rotation_registers.finish();
                    self.session_view.scripted.script_camera_freeze_time = false;
                }
                return;
            };
            let direction = cached - self.session_view.target;
            let length =
                f64::from(direction.x * direction.x + direction.z * direction.z).sqrt() as f32;
            if length >= 0.1 {
                let mut angle = f64::from(direction.x / length).acos() as f32;
                if direction.z < 0. {
                    angle = -angle;
                }
                angle -= std::f32::consts::FRAC_PI_2;
                normalize_tracking_angle(&mut angle);
                if frame <= camera.rotation.turn_frames {
                    let factor = camera.rotation.factor(frame);
                    let mut delta = angle - camera.camera_yaw_radians;
                    normalize_tracking_angle(&mut delta);
                    delta *= factor;
                    camera.camera_yaw_radians += delta;
                    normalize_tracking_angle(&mut camera.camera_yaw_radians);
                    self.visual_speed_multiplier = camera.rotation.time.sample(factor);
                } else {
                    camera.camera_yaw_radians = angle;
                }
            }
        }
        if frame >= final_frame {
            camera.rotation_registers.finish();
            self.session_view.scripted.script_camera_freeze_time = false;
        }
    }
    #[cfg(test)]
    pub fn script_camera_path_active(&self) -> bool {
        self.session_view.scripted.motion.active
    }
    #[cfg(test)]
    pub fn install_script_camera_path_for_test(&mut self) {
        let motion = WaypointMotion::along_chain(
            &[
                Point {
                    x: 100.,
                    y: 0.,
                    z: 0.,
                },
                Point {
                    x: 200.,
                    y: 0.,
                    z: 0.,
                },
            ],
            Point::from_world(self.session_view.target),
            self.session_view.camera.camera_yaw_radians,
            10.,
            10.,
            1,
            4000,
            0,
            CAMERA_FRAME_MS,
            true,
            0.,
            0.,
            10.,
        );
        self.session_view
            .install_motion(motion, false, &mut self.visual_speed_multiplier, false);
    }
    #[cfg(test)]
    pub fn script_camera_path_rolling_average_frames(&self) -> Option<i32> {
        self.session_view
            .scripted
            .motion
            .active
            .then_some(self.session_view.scripted.motion.rolling_average_frames)
    }
    #[cfg(test)]
    pub fn script_camera_move_to_target(&self) -> Option<Vec3> {
        let motion = &self.session_view.scripted.motion;
        motion
            .active
            .then(|| motion.points[motion.num_waypoints].world())
    }
}

fn normalize_tracking_angle(angle: &mut f32) {
    let pi = std::f32::consts::PI;
    if *angle < -10. * pi {
        *angle = 0.;
    }
    if *angle > 10. * pi {
        *angle = 0.;
    }
    while *angle < -pi {
        *angle += 2. * pi;
    }
    while *angle > pi {
        *angle -= 2. * pi;
    }
}

#[cfg(test)]
#[path = "headless_debug_tests.rs"]
mod headless_debug_tests;

#[cfg(test)]
#[path = "rejected_camera_completion_tests.rs"]
mod rejected_camera_completion_tests;
