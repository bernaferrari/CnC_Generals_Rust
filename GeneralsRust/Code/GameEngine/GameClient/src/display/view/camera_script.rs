use super::*;

impl View {
    /// Move camera to a position over multiple frames.
    pub fn move_camera_to(
        &mut self,
        target: &Point3,
        milliseconds: i32,
        shutter: i32,
        orient: bool,
        ease_in: f32,
        ease_out: f32,
    ) {
        // C++ W3DView::moveCameraTo always builds a 2-node waypoint path.
        if milliseconds <= 0 {
            self.look_at(target);
            self.camera_move = None;
            self.camera_path = None;
            return;
        }
        if orient {
            let start = self.position;
            self.move_camera_along_waypoint_path(
                &[start, *target],
                milliseconds,
                shutter,
                true,
                ease_in,
                ease_out,
            );
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        let end_position = Vec3::new(target.x, target.y, 0.0);

        self.camera_move = Some(CameraPositionTransition::new(
            end_position,
            frames,
            ease_in,
            ease_out,
            Vec3::new(self.position.x, self.position.y, self.position.z),
        ));
        self.camera_path = None;
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Move camera along an explicit waypoint chain.
    pub fn move_camera_along_waypoint_path(
        &mut self,
        waypoints: &[Point3],
        milliseconds: i32,
        shutter: i32,
        orient: bool,
        ease_in: f32,
        ease_out: f32,
    ) {
        if waypoints.is_empty() {
            return;
        }

        if milliseconds <= 0 || waypoints.len() == 1 {
            self.look_at(waypoints.last().unwrap_or(&waypoints[0]));
            self.camera_path = None;
            return;
        }

        let mut path = Vec::with_capacity(waypoints.len());
        let mut angle = self.angle;
        for index in 0..waypoints.len() {
            let point = waypoints[index];
            if orient && index + 1 < waypoints.len() {
                let next = waypoints[index + 1];
                angle = travel_camera_angle(next.x - point.x, next.y - point.y, angle);
            }
            path.push(CameraWaypoint {
                position: Vec3::new(point.x, point.y, 0.0),
                angle,
                time_multiplier: 1,
            });
        }
        if orient && path.len() >= 2 {
            path[0].angle = self.angle;
            let last = path.len() - 1;
            path[last].angle = path[last - 1].angle;
            for i in (2..last).rev() {
                path[i].angle = (path[i].angle + path[i - 1].angle) * 0.5;
            }
        }

        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_move = None;
        self.camera_rotate = None;
        self.camera_path = Some(CameraPath::new(
            path,
            milliseconds.max(1),
            shutter.max(1),
            orient,
            ease_in,
            ease_out,
        ));
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Check if camera movement animation is finished
    pub fn is_camera_movement_finished(&self) -> bool {
        self.camera_move.is_none()
            && self.camera_path.is_none()
            && self.camera_rotate.is_none()
            && self.camera_zoom.is_none()
            && self.camera_pitch.is_none()
            && self.rotate_camera_toward.is_none()
    }

    pub fn reset_camera(
        &mut self,
        location: &Point3,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        if milliseconds <= 0 {
            self.look_at(location);
            self.set_angle_and_pitch_to_default();
            self.fx_pitch = 1.0;
            self.set_zoom(self.max_zoom);
            self.camera_rotate = None;
            self.camera_zoom = None;
            self.camera_pitch = None;
            return;
        }

        self.move_camera_to(location, milliseconds, 0, false, ease_in, ease_out);
        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        let angle_delta = self.default_angle - self.angle;
        self.camera_rotate = Some(CameraRotateTransition::new(
            angle_delta / (2.0 * PI),
            frames,
            ease_in,
            ease_out,
            self.angle,
        ));
        self.camera_zoom = Some(CameraZoomTransition::new(
            self.max_zoom,
            frames,
            ease_in,
            ease_out,
            self.zoom,
        ));
        self.camera_pitch = Some(CameraPitchTransition::new_fx(
            // C++ W3D resetCamera drives FXPitch endpoint to 1.0f.
            1.0,
            frames,
            ease_in,
            ease_out,
            self.fx_pitch,
        ));
    }

    /// Rotate camera by a number of full rotations.
    pub fn rotate_camera(
        &mut self,
        rotations: f32,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        if milliseconds <= 0 {
            self.set_angle(self.angle + rotations * 2.0 * PI);
            self.camera_rotate = None;
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_rotate = Some(CameraRotateTransition::new(
            rotations, frames, ease_in, ease_out, self.angle,
        ));
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Zoom camera to a specific level.
    pub fn zoom_camera(&mut self, final_zoom: f32, milliseconds: i32, ease_in: f32, ease_out: f32) {
        if milliseconds <= 0 {
            self.set_zoom(final_zoom);
            self.camera_zoom = None;
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_zoom = Some(CameraZoomTransition::new(
            final_zoom, frames, ease_in, ease_out, self.zoom,
        ));
    }

    /// C++ `W3DView::pitchCamera` — animates `m_FXPitch`, not orbit pitch.
    pub fn pitch_camera(
        &mut self,
        final_pitch: f32,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        if milliseconds <= 0 {
            self.fx_pitch = final_pitch;
            self.camera_pitch = None;
            return;
        }

        let frames = ms_to_frames(milliseconds);
        let (ease_in, ease_out) = ease_ratios(milliseconds, ease_in, ease_out);
        self.camera_pitch = Some(CameraPitchTransition::new_fx(
            final_pitch,
            frames,
            ease_in,
            ease_out,
            self.fx_pitch,
        ));
    }

    pub(super) fn rotate_camera_toward_one_frame(&mut self) {
        let disable_camera = get_global_data()
            .map(|g| g.read().disable_camera_movement)
            .unwrap_or(false);

        let finished = {
            let info = match &mut self.rotate_camera_toward {
                Some(info) => info,
                None => return,
            };

            info.cur_frame += 1;

            if disable_camera {
                info.cur_frame >= info.total_frames()
            } else if info.track_object {
                if info.cur_frame <= info.total_frames() {
                    if let Some(obj_id) = info.target_object_id {
                        if let Some(object) = TheGameLogic::find_object_by_id(obj_id) {
                            if let Ok(guard) = object.read() {
                                let pos = guard.get_position();
                                info.target_position = Point3::new(pos.x, pos.y, pos.z);
                            }
                        }
                    }

                    let center = self.position;
                    let dir_x = info.target_position.x - center.x;
                    let dir_y = info.target_position.y - center.y;
                    let dir_length = (dir_x * dir_x + dir_y * dir_y).sqrt();

                    if dir_length >= 0.1 {
                        let mut angle = (dir_x / dir_length).acos();
                        if dir_y < 0.0 {
                            angle = -angle;
                        }
                        angle -= PI / 2.0;
                        angle = normalize_angle(angle);

                        if info.cur_frame <= info.num_frames {
                            let factor =
                                parabolic_ease(info.cur_frame as f32 / info.num_frames as f32);
                            let mut angle_diff = angle - self.angle;
                            angle_diff = normalize_angle(angle_diff);
                            angle_diff *= factor;
                            self.angle += angle_diff;
                            self.angle = normalize_angle(self.angle);
                        } else {
                            self.angle = angle;
                        }
                    }
                }
                info.cur_frame >= info.total_frames()
            } else if info.cur_frame <= info.num_frames {
                let factor = parabolic_ease(info.cur_frame as f32 / info.num_frames as f32);
                self.angle = info.start_angle + (info.end_angle - info.start_angle) * factor;
                self.angle = normalize_angle(self.angle);
                info.cur_frame >= info.total_frames()
            } else {
                true
            }
        };

        if finished {
            let track_object = self
                .rotate_camera_toward
                .as_ref()
                .is_some_and(|i| i.track_object);
            let end_angle = self
                .rotate_camera_toward
                .as_ref()
                .map_or(0.0, |i| i.end_angle);
            self.rotate_camera_toward = None;
            self.freeze_time_for_camera_movement = false;
            if !track_object {
                self.angle = end_angle;
            }
        }
    }

    /// Set final zoom for an active movement (C++ `W3DView::cameraModFinalZoom`).
    pub fn camera_mod_final_zoom(&mut self, final_zoom: f32, ease_in: f32, ease_out: f32) {
        if let Some(rotate_transition) = &self.camera_rotate {
            let time_ms = frames_to_ms(rotate_transition.remaining_frames());
            self.zoom_camera(
                final_zoom,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
        if let Some(move_transition) = &self.camera_move {
            let time_ms = frames_to_ms(move_transition.remaining_frames());
            self.zoom_camera(
                final_zoom,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
    }

    /// Set final pitch for an active movement (C++ `W3DView::cameraModFinalPitch`).
    pub fn camera_mod_final_pitch(&mut self, final_pitch: f32, ease_in: f32, ease_out: f32) {
        if let Some(rotate_transition) = &self.camera_rotate {
            let time_ms = frames_to_ms(rotate_transition.remaining_frames());
            self.pitch_camera(
                final_pitch,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
        if let Some(move_transition) = &self.camera_move {
            let time_ms = frames_to_ms(move_transition.remaining_frames());
            self.pitch_camera(
                final_pitch,
                time_ms,
                (time_ms as f32) * ease_in,
                (time_ms as f32) * ease_out,
            );
        }
    }

    pub fn camera_mod_final_time_multiplier(&mut self, final_multiplier: i32) {
        if let Some(path) = &mut self.camera_path {
            path.set_final_time_multiplier(final_multiplier);
        }
    }

    pub fn camera_mod_rolling_average(&mut self, frames_to_average: i32) {
        if let Some(path) = &mut self.camera_path {
            path.set_rolling_average_frames(frames_to_average);
        }
    }

    /// C++ parity for `W3DView::cameraModLookToward`.
    pub fn camera_mod_look_toward(&mut self, target: &Point3) {
        if self.camera_rotate.is_some() {
            return;
        }
        if let Some(path) = &mut self.camera_path {
            path.camera_mod_look_toward(Vec3::new(target.x, target.y, target.z));
            return;
        }

        if let Some(move_transition) = &self.camera_move {
            let center = self.position;
            let dir_x = target.x - center.x;
            let dir_y = target.y - center.y;
            if (dir_x * dir_x + dir_y * dir_y).sqrt() < 0.1 {
                return;
            }

            let desired = normalize_angle(dir_y.atan2(dir_x) - PI * 0.5);
            let delta = normalize_angle(desired - self.angle);
            let remaining_ms = frames_to_ms(move_transition.remaining_frames());
            if remaining_ms <= 0 {
                self.angle = desired;
                return;
            }
            self.rotate_camera(delta / (2.0 * PI), remaining_ms, 0.0, 0.0);
        }
    }

    /// C++ parity for `W3DView::cameraModFinalLookToward`.
    pub fn camera_mod_final_look_toward(&mut self, target: &Point3) {
        if self.camera_rotate.is_some() {
            return;
        }
        if let Some(path) = &mut self.camera_path {
            path.camera_mod_final_look_toward(Vec3::new(target.x, target.y, target.z));
            return;
        }

        // `moveCameraTo` in C++ also uses the waypoint camera path code, so final-look modifiers
        // should still affect active simple move transitions.
        self.camera_mod_look_toward(target);
    }

    /// C++ parity for `W3DView::cameraModFinalMoveTo`.
    pub fn camera_mod_final_move_to(&mut self, target: &Point3) {
        if self.camera_rotate.is_some() {
            return;
        }
        if let Some(path) = &mut self.camera_path {
            path.camera_mod_final_move_to(Vec3::new(target.x, target.y, target.z));
            return;
        }

        if let Some(move_transition) = self.camera_move.take() {
            let remaining_frames = move_transition.remaining_frames().max(1);
            let current = move_transition.get_current_position();
            let end_position = Vec3::new(target.x, target.y, 0.0);
            self.camera_move = Some(CameraPositionTransition::new(
                end_position,
                remaining_frames,
                0.0,
                0.0,
                current,
            ));
        }
    }

    /// C++ parity for `W3DView::rotateCameraTowardObject`.
    pub fn rotate_camera_toward_object(
        &mut self,
        object_id: u32,
        milliseconds: i32,
        hold_milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
    ) {
        let hold_ms = if hold_milliseconds < 1 {
            0
        } else {
            hold_milliseconds
        };
        let num_hold_frames = (hold_ms as f32 / FRAME_LENGTH_MS) as i32;
        let num_hold_frames = num_hold_frames.max(0);

        let ms = if milliseconds < 1 { 1 } else { milliseconds };
        let num_frames = (ms as f32 / FRAME_LENGTH_MS) as i32;
        let num_frames = num_frames.max(1);

        let (ease_in, ease_out) = ease_ratios(ms, ease_in, ease_out);

        self.rotate_camera_toward = Some(RotateCameraToward {
            num_frames,
            cur_frame: 0,
            num_hold_frames,
            ease_in,
            ease_out,
            track_object: true,
            target_object_id: Some(object_id),
            target_position: Point3::ZERO,
            start_angle: 0.0,
            end_angle: 0.0,
        });
        self.camera_path = None;
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// C++ parity for `W3DView::rotateCameraTowardPosition`.
    pub fn rotate_camera_toward_position(
        &mut self,
        pos: &Point3,
        milliseconds: i32,
        ease_in: f32,
        ease_out: f32,
        reverse_rotation: bool,
    ) {
        let ms = if milliseconds < 1 { 1 } else { milliseconds };
        let num_frames = (ms as f32 / FRAME_LENGTH_MS) as i32;
        let num_frames = num_frames.max(1);

        let center = self.position;
        let dir_x = pos.x - center.x;
        let dir_y = pos.y - center.y;
        let dir_length = (dir_x * dir_x + dir_y * dir_y).sqrt();
        if dir_length < 0.1 {
            return;
        }

        let mut angle = (dir_x / dir_length).acos();
        if dir_y < 0.0 {
            angle = -angle;
        }
        angle -= PI / 2.0;
        angle = normalize_angle(angle);

        if reverse_rotation {
            if self.angle < angle {
                angle -= 2.0 * PI;
            } else {
                angle += 2.0 * PI;
            }
        }

        let (ease_in, ease_out) = ease_ratios(ms, ease_in, ease_out);

        self.rotate_camera_toward = Some(RotateCameraToward {
            num_frames,
            cur_frame: 0,
            num_hold_frames: 0,
            ease_in,
            ease_out,
            track_object: false,
            target_object_id: None,
            target_position: *pos,
            start_angle: self.angle,
            end_angle: angle,
        });
        self.camera_path = None;
        if self.freeze_time_for_camera_movement {
            self.freeze_time_for_camera_movement_active = true;
        }
    }

    /// Apply camera shake impulse using the legacy damped-oscillation model.
    pub fn shake(&mut self, _epicenter: &Point3, _shake_type: CameraShakeType) {
        let data = game_engine::common::global_data::read();
        self.shake_with_services(
            _epicenter,
            _shake_type,
            &data,
            &mut game_engine::common::random_value::get_game_client_random_value_real,
        );
    }

    /// The W3DView impulse calculation with this view's immutable rules and
    /// presentation stream. Ordinary FX never has to select an active view.
    pub fn shake_with_services(
        &mut self,
        _epicenter: &Point3,
        _shake_type: CameraShakeType,
        data: &game_engine::common::global_data::GlobalData,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) {
        game_engine::common::view_impulse::ViewImpulse {
            intensity: &mut self.shake_intensity,
            cosine: &mut self.shake_angle_cos,
            sine: &mut self.shake_angle_sin,
        }
        .enqueue(
            [_epicenter.x, _epicenter.y],
            [self.position.x, self.position.y],
            _shake_type.into(),
            data,
            random,
        );
    }
}
