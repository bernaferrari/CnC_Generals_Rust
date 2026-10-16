//! Camera commands change the receiving view; only admitted views advance it.
//! W3DView.cpp setupWaypointPath/moveAlongWaypointPath/updateCameraMovements.
use super::*;
use crate::presentation_frame::camera::motion::MotionAdmission;
use crate::presentation_frame::scripted::CameraObjectTracking;
use crate::presentation_frame::waypoint::{Point, WaypointMotion};

const CAMERA_FRAME_MS: i32 = 33;
fn camera_milliseconds(seconds: f32) -> i32 {
    (seconds * 1000.0) as i32
}
fn movement_disabled() -> bool {
    game_engine::common::global_data::read()
        .writable
        .disable_camera_movement
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
            return;
        }
        self.pending_camera_rotate = None;
        self.pending_camera_look_toward = None;
        self.session_view.scripted.script_look_toward_object_id = None;
        self.session_view.install_motion(
            motion,
            false,
            &mut self.visual_speed_multiplier,
            movement_disabled(),
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
            return;
        }
        let final_ground = {
            let owner = self.world_services.terrain();
            let terrain = match owner.read() {
                Ok(terrain) => terrain,
                Err(_) => {
                    log::error!("receiving camera terrain poisoned");
                    return;
                }
            };
            terrain.get_ground_height(request.position.x, request.position.z, None)
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
            return;
        }
        self.pending_camera_rotate = None;
        self.pending_camera_look_toward = None;
        self.session_view.scripted.script_look_toward_object_id = None;
        self.session_view.install_motion(
            motion,
            true,
            &mut self.visual_speed_multiplier,
            movement_disabled(),
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
        camera.camera_yaw_target.is_some()
            || camera.camera_zoom_target.is_some()
            || camera.camera_pitch_target.is_some()
    }
    pub(in crate::game_logic::game_logic::world_scripts) fn clear_script_camera_orientation_remaining(
        &mut self,
    ) {
        let camera = &mut self.session_view.camera;
        camera.camera_yaw_target = None;
        camera.camera_zoom_target = None;
        camera.camera_pitch_target = None;
    }
    pub(super) fn mark_script_camera_movement_maybe_finished(&mut self) {
        self.mission_scripts
            .set_camera_movement_finished(self.session_view.movement_finished());
    }
    pub(in crate::game_logic::game_logic) fn script_camera_remaining_seconds(&self) -> f32 {
        let camera = &self.session_view.camera;
        if camera.camera_yaw_target.is_some() {
            let frames = self
                .session_view
                .scripted
                .object_tracking
                .as_ref()
                .map(|tracking| tracking.turn_frames + tracking.hold_frames)
                .unwrap_or_else(|| {
                    (camera_milliseconds(camera.camera_yaw_duration).max(1) / CAMERA_FRAME_MS)
                        .max(1) as u32
                });
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
        if camera.camera_yaw_target.is_some()
            && self.session_view.scripted.object_tracking.is_none()
        {
            camera.camera_yaw_start = camera.camera_yaw_radians;
            camera.camera_yaw_target = Some(camera.camera_yaw_radians);
        }
        self.session_view.scripted.script_look_toward_object_id = None;
        self.pending_camera_look_toward = None;
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_look_toward(
        &mut self,
        position: Vec3,
        final_look: bool,
    ) {
        self.session_view.modify_path_look(
            Point::from_world(position),
            final_look,
            &mut self.visual_speed_multiplier,
            movement_disabled(),
        );
        self.mark_script_camera_movement_maybe_finished();
    }
    pub(in crate::game_logic::game_logic) fn apply_script_camera_mod_final_speed_multiplier(
        &mut self,
        request: &CameraModFinalSpeedMultiplierRequest,
    ) {
        if self.session_view.camera.camera_yaw_target.is_some() {
            if let Some(tracking) = self.session_view.scripted.object_tracking.as_mut() {
                tracking.end_time_multiplier = request.multiplier;
                return;
            }
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
    pub(in crate::game_logic::game_logic) fn apply_visual_speed_multiplier(
        &mut self,
        request: &VisualSpeedMultiplierRequest,
    ) {
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
    pub(crate) fn headless_camera_view_offer(&mut self, frame_ms: i32) -> bool {
        self.advance_camera_view(
            MotionAdmission {
                update_views: true,
                fast_camera_only: false,
                paused: self.is_paused(),
                debug_frozen: false,
                disable_camera_movement: movement_disabled(),
            },
            frame_ms,
        )
    }
    pub(super) fn install_camera_object_tracking(
        &mut self,
        object_id: u32,
        position: Vec3,
        seconds: f32,
        hold_seconds: f32,
        ease_in: f32,
        ease_out: f32,
    ) {
        let milliseconds = camera_milliseconds(seconds).max(1);
        let turn_frames = (milliseconds / CAMERA_FRAME_MS).max(1) as u32;
        let hold_frames = (camera_milliseconds(hold_seconds).max(0) / CAMERA_FRAME_MS) as u32;
        let multiplier = self.visual_speed_multiplier as i32;
        self.session_view.scripted.object_tracking = Some(CameraObjectTracking {
            duration_ms: milliseconds,
            turn_frames,
            hold_frames,
            last_position: position,
            start_time_multiplier: multiplier,
            end_time_multiplier: multiplier,
        });
        self.session_view.scripted.script_look_toward_object_id = Some(object_id);
        self.session_view.scripted.motion.active = false;
        self.session_view.scripted.path_arrived = false;
        let camera = &mut self.session_view.camera;
        camera.camera_yaw_frame = 0;
        camera.camera_yaw_duration = milliseconds as f32 / 1000.;
        camera.camera_yaw_ease_in = ease_in;
        camera.camera_yaw_ease_out = ease_out;
        camera.camera_yaw_target = Some(camera.camera_yaw_radians);
        self.mark_script_camera_movement_maybe_finished();
    }
    fn advance_camera_object_tracking(&mut self, admission: MotionAdmission) {
        if !admission.advances() || self.session_view.camera.camera_yaw_target.is_none() {
            return;
        }
        let current = self
            .session_view
            .scripted
            .script_look_toward_object_id
            .and_then(|id| self.objects.get(&ObjectId(id)))
            .map(|object| object.get_position());
        let Some(tracking) = self.session_view.scripted.object_tracking.as_mut() else {
            return;
        };
        let camera = &mut self.session_view.camera;
        camera.camera_yaw_frame = camera.camera_yaw_frame.saturating_add(1);
        let frame = camera.camera_yaw_frame;
        let final_frame = tracking.turn_frames + tracking.hold_frames;
        if !admission.disable_camera_movement && frame <= final_frame {
            if let Some(position) = current {
                tracking.last_position = position;
            }
            let direction = tracking.last_position - self.session_view.target;
            let length =
                f64::from(direction.x * direction.x + direction.z * direction.z).sqrt() as f32;
            if length >= 0.1 {
                let mut angle = f64::from(direction.x / length).acos() as f32;
                if direction.z < 0. {
                    angle = -angle;
                }
                angle -= std::f32::consts::FRAC_PI_2;
                normalize_tracking_angle(&mut angle);
                if frame <= tracking.turn_frames {
                    let duration_ms = tracking.duration_ms as f32;
                    let factor = crate::presentation_frame::camera::motion::parabolic_ease(
                        frame as f32 / tracking.turn_frames as f32,
                        camera.camera_yaw_ease_in * 1000. / duration_ms,
                        camera.camera_yaw_ease_out * 1000. / duration_ms,
                    );
                    let mut delta = angle - camera.camera_yaw_radians;
                    normalize_tracking_angle(&mut delta);
                    delta *= factor;
                    camera.camera_yaw_radians += delta;
                    normalize_tracking_angle(&mut camera.camera_yaw_radians);
                    let time_delta = (i64::from(tracking.end_time_multiplier)
                        - i64::from(tracking.start_time_multiplier))
                        as f32
                        * factor;
                    self.visual_speed_multiplier = tracking
                        .start_time_multiplier
                        .saturating_add((0.5 + f64::from(time_delta)).floor() as i32)
                        as f32;
                } else {
                    camera.camera_yaw_radians = angle;
                }
            }
        }
        if frame >= final_frame {
            camera.camera_yaw_target = None;
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
