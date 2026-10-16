//! Existing scripted camera path implementation, owned by SessionView.
//! Algorithms and positional serde records are unchanged in the state move.
use crate::game_logic::mission_scripts::{CameraMoveToRequest, CameraPathRequest};
use game_engine::common::system::game_common::LOGIC_FRAMES_PER_SECONDS_REAL as LOGIC_FRAMES_PER_SECOND;
use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub(crate) struct ParabolicEase {
    pub(crate) in_t: f32,
    pub(crate) out_t: f32,
}

impl ParabolicEase {
    pub(crate) fn new(ease_in_time: f32, ease_out_time: f32) -> Self {
        let mut in_t = ease_in_time.clamp(0.0, 1.0);
        let out_t = 1.0 - ease_out_time.clamp(0.0, 1.0);
        if in_t > out_t {
            in_t = out_t;
        }
        Self { in_t, out_t }
    }

    pub(crate) fn eval(self, param: f32) -> f32 {
        let param = param.clamp(0.0, 1.0);
        let v0 = 1.0 + self.out_t - self.in_t;
        if param < self.in_t {
            if self.in_t <= 0.0 {
                0.0
            } else {
                param * param / (v0 * self.in_t)
            }
        } else if param <= self.out_t {
            (self.in_t + 2.0 * (param - self.in_t)) / v0
        } else {
            let denom = (1.0 - self.out_t).max(f32::EPSILON);
            (self.in_t
                + 2.0 * (self.out_t - self.in_t)
                + (2.0 * (param - self.out_t) + self.out_t * self.out_t - param * param) / denom)
                / v0
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScriptCameraMoveTo {
    pub(crate) start: Vec3,
    pub(crate) target: Vec3,
    pub(crate) ease: ParabolicEase,
    pub(crate) total_time_seconds: f32,
    pub(crate) elapsed_seconds: f32,
    pub(crate) shutter_frames: u32,
    pub(crate) cur_shutter: u32,
    pub(crate) last_ease: f32,
    pub(crate) freeze_time: bool,
    pub(crate) freeze_angle: bool,
    pub(crate) look_toward: Option<Vec3>,
    pub(crate) suppress_travel_look: bool,
    pub(crate) speed_ramp_start_t: f32,
    pub(crate) speed_ramp_start_multiplier: f32,
    pub(crate) speed_ramp_final_multiplier: f32,
}

impl ScriptCameraMoveTo {
    pub(crate) fn new(start: Vec3, request: &CameraMoveToRequest) -> Self {
        let total_time_seconds = request.seconds.max(0.001);
        let ease_in = (request.ease_in_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease_out = (request.ease_out_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease = ParabolicEase::new(ease_in, ease_out);
        let shutter_frames =
            (request.camera_stutter_seconds * LOGIC_FRAMES_PER_SECOND).round() as u32;
        let shutter_frames = shutter_frames.max(1);
        Self {
            start,
            // C++ TacticalView::lookAt stores XY and samples height later.
            // Script Coord3D z=0 (ground plane) becomes Y-up look-at 0 and
            // slams the camera into the void (hq-n7sk). Keep start height.
            target: if request.position.y.abs() <= f32::EPSILON {
                Vec3::new(request.position.x, start.y, request.position.z)
            } else {
                request.position
            },
            ease,
            total_time_seconds,
            elapsed_seconds: 0.0,
            shutter_frames,
            cur_shutter: shutter_frames,
            last_ease: 0.0,
            freeze_time: false,
            freeze_angle: false,
            look_toward: None,
            suppress_travel_look: false,
            speed_ramp_start_t: 0.0,
            speed_ramp_start_multiplier: 1.0,
            speed_ramp_final_multiplier: 1.0,
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.elapsed_seconds >= self.total_time_seconds
    }

    pub(crate) fn final_focus(&self) -> Vec3 {
        self.target
    }

    pub(crate) fn remaining_time_seconds(&self) -> f32 {
        (self.total_time_seconds - self.elapsed_seconds).max(0.0)
    }

    pub(crate) fn set_freeze_time(&mut self, freeze: bool) {
        self.freeze_time = freeze;
    }

    pub(crate) fn freeze_time(&self) -> bool {
        self.freeze_time
    }

    pub(crate) fn set_freeze_angle(&mut self, freeze: bool) {
        self.freeze_angle = freeze;
    }

    pub(crate) fn freeze_angle(&self) -> bool {
        self.freeze_angle
    }

    pub(crate) fn set_look_toward(&mut self, position: Vec3) {
        self.look_toward = Some(position);
        self.freeze_angle = false;
        self.suppress_travel_look = false;
    }

    pub(crate) fn look_toward(&self) -> Option<Vec3> {
        self.look_toward
    }

    pub(crate) fn set_suppress_travel_look(&mut self, suppress: bool) {
        self.suppress_travel_look = suppress;
        if suppress {
            self.look_toward = None;
        }
    }

    pub(crate) fn suppress_travel_look(&self) -> bool {
        self.suppress_travel_look
    }

    pub(crate) fn current_speed_multiplier(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= self.speed_ramp_start_t {
            return self.speed_ramp_start_multiplier;
        }
        let span = (1.0 - self.speed_ramp_start_t).max(f32::EPSILON);
        let t = ((progress - self.speed_ramp_start_t) / span).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier
            + (self.speed_ramp_final_multiplier - self.speed_ramp_start_multiplier) * t
    }

    pub(crate) fn set_final_speed_multiplier(&mut self, multiplier: f32) {
        if !multiplier.is_finite() {
            return;
        }
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier = self.current_speed_multiplier(progress);
        self.speed_ramp_start_t = progress;
        self.speed_ramp_final_multiplier = multiplier.max(0.0);
    }

    /// C++ `cameraModFinalMoveTo` on a `moveCameraTo` 2-waypoint path: retarget dest.
    pub(crate) fn camera_mod_final_move_to(&mut self, target: Vec3) {
        self.target = if target.y.abs() <= f32::EPSILON {
            Vec3::new(target.x, self.target.y, target.z)
        } else {
            target
        };
    }

    pub(crate) fn advance(&mut self, dt: f32) -> Option<Vec3> {
        let prev_ease = self.last_ease;
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let speed_multiplier = self.current_speed_multiplier(progress).max(0.0);
        self.elapsed_seconds =
            (self.elapsed_seconds + dt.max(0.0) * speed_multiplier).min(self.total_time_seconds);
        let t = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let next_ease = self.ease.eval(t);
        self.last_ease = next_ease;

        self.cur_shutter = self.cur_shutter.saturating_sub(1);
        if self.cur_shutter > 0 && next_ease > prev_ease {
            return None;
        }
        self.cur_shutter = self.shutter_frames;

        Some(self.start.lerp(self.target, next_ease))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct ScriptCameraPathMove {
    pub(crate) points: Vec<Vec3>,
    pub(crate) segment_length: Vec<f32>,
    pub(crate) total_distance: f32,
    pub(crate) ease: ParabolicEase,
    pub(crate) total_time_seconds: f32,
    pub(crate) elapsed_seconds: f32,
    pub(crate) cur_segment: usize,
    pub(crate) cur_seg_distance: f32,
    pub(crate) shutter_frames: u32,
    pub(crate) cur_shutter: u32,
    pub(crate) last_ease: f32,
    pub(crate) freeze_time: bool,
    pub(crate) freeze_angle: bool,
    pub(crate) look_toward: Option<Vec3>,
    pub(crate) look_toward_is_final: bool,
    pub(crate) suppress_travel_look: bool,
    pub(crate) rolling_average_frames: i32,
    pub(crate) smoothed_focus: Option<Vec3>,
    pub(crate) speed_ramp_start_t: f32,
    pub(crate) speed_ramp_start_multiplier: f32,
    pub(crate) speed_ramp_final_multiplier: f32,
    pub(crate) start_angle: f32,
    pub(crate) frozen_to_start_angle: bool,
}

impl ScriptCameraPathMove {
    pub(crate) fn new(start_focus: Vec3, request: &CameraPathRequest) -> Option<Self> {
        let waypoint_name = gamelogic::common::AsciiString::from(&request.waypoint);
        let terrain_owner_handle = gamelogic::terrain::get_terrain_logic();
        let chain: Vec<Vec3> = terrain_owner_handle
            .read()
            .ok()
            .and_then(|terrain| {
                let start = terrain.get_waypoint_by_name(&waypoint_name)?;
                // C++ W3DView::moveCameraAlongWaypointPath: numWaypoints < MAX_WAYPOINTS.
                // Visited-set also breaks 1-link rings C++ would still spin on (same loc).
                let points = terrain
                    .walk_link0_chain(start, gamelogic::terrain::CAMERA_WAYPOINT_PATH_LIMIT)
                    .into_iter()
                    .map(|wp| {
                        let loc = wp.get_location();
                        Vec3::new(loc.x, 0.0, loc.y)
                    })
                    .collect();
                Some(points)
            })
            .unwrap_or_default();

        if chain.is_empty() {
            return None;
        }

        let min_delta = gamelogic::common::MAP_XY_FACTOR;
        let mut points: Vec<Vec3> = Vec::with_capacity(chain.len() + 4);
        points.push(start_focus);
        points.push(start_focus);

        for p in chain {
            if let Some(last) = points.last().copied() {
                if Vec2::new(p.x - last.x, p.z - last.z).length() < min_delta {
                    continue;
                }
            }
            points.push(p);
        }

        if points.len() < 3 {
            return None;
        }

        // Pad start to allow spline interpolation like the original W3D view.
        let first = points[1];
        let second = points[2];
        points[0] = Vec3::new(
            first.x - (second.x - first.x),
            0.0,
            first.z - (second.z - first.z),
        );

        // Pad end one segment beyond last to keep interpolation stable.
        let last = *points.last().unwrap();
        let prev = points[points.len() - 2];
        points.push(Vec3::new(
            last.x + (last.x - prev.x),
            0.0,
            last.z + (last.z - prev.z),
        ));

        let last_meaningful = points.len() - 2;
        let mut segment_length = vec![0.0f32; points.len()];
        let mut total_distance = 0.0f32;

        for i in 1..last_meaningful {
            let a = points[i];
            let b = points[i + 1];
            let len = Vec2::new(b.x - a.x, b.z - a.z).length();
            segment_length[i] = len;
            total_distance += len;
        }

        if total_distance < 1.0 && last_meaningful >= 2 {
            let idx = last_meaningful - 1;
            segment_length[idx] += 1.0 - total_distance;
            total_distance = 1.0;
        }

        if last_meaningful >= 2 {
            segment_length[last_meaningful] = segment_length[last_meaningful - 1];
        }

        let total_time_seconds = request.seconds.max(0.001);
        let ease_in = (request.ease_in_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease_out = (request.ease_out_seconds / total_time_seconds).clamp(0.0, 1.0);
        let ease = ParabolicEase::new(ease_in, ease_out);

        let shutter_frames =
            (request.camera_stutter_seconds * LOGIC_FRAMES_PER_SECOND).round() as u32;
        let shutter_frames = shutter_frames.max(1);

        Some(Self {
            points,
            segment_length,
            total_distance,
            ease,
            total_time_seconds,
            elapsed_seconds: 0.0,
            cur_segment: 1,
            cur_seg_distance: 0.0,
            shutter_frames,
            cur_shutter: shutter_frames,
            last_ease: 0.0,
            freeze_time: false,
            freeze_angle: false,
            look_toward: None,
            look_toward_is_final: false,
            suppress_travel_look: false,
            rolling_average_frames: 1,
            smoothed_focus: None,
            speed_ramp_start_t: 0.0,
            speed_ramp_start_multiplier: 1.0,
            speed_ramp_final_multiplier: 1.0,
            start_angle: leftover_tactical_view_angle(),
            frozen_to_start_angle: false,
        })
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.elapsed_seconds >= self.total_time_seconds
    }

    pub(crate) fn final_focus(&self) -> Vec3 {
        let idx = self.points.len().saturating_sub(2);
        self.points.get(idx).copied().unwrap_or(Vec3::ZERO)
    }

    pub(crate) fn remaining_time_seconds(&self) -> f32 {
        (self.total_time_seconds - self.elapsed_seconds).max(0.0)
    }

    pub(crate) fn set_freeze_time(&mut self, freeze: bool) {
        self.freeze_time = freeze;
    }

    pub(crate) fn freeze_time(&self) -> bool {
        self.freeze_time
    }

    pub(crate) fn set_freeze_angle(&mut self, freeze: bool) {
        self.freeze_angle = freeze;
    }

    pub(crate) fn freeze_angle(&self) -> bool {
        self.freeze_angle
    }

    /// C++ `W3DView::cameraModFreezeAngle` on a waypoint path: remaining
    /// `cameraAngle[i+1] = cameraAngle[0]` (start yaw, not current facing).
    pub(crate) fn freeze_angles_to_start(&mut self) {
        self.freeze_angle = true;
        self.frozen_to_start_angle = true;
        self.look_toward = None;
        self.look_toward_is_final = false;
        self.suppress_travel_look = false;
    }

    pub(crate) fn frozen_start_look_toward(&self, focus: Vec3) -> Option<Vec3> {
        if !self.frozen_to_start_angle {
            return None;
        }
        Some(look_point_from_angle_xz(focus, self.start_angle))
    }

    pub(crate) fn set_look_toward(&mut self, position: Vec3) {
        self.look_toward = Some(position);
        self.look_toward_is_final = false;
        self.freeze_angle = false;
        self.frozen_to_start_angle = false;
        self.suppress_travel_look = false;
    }

    /// C++ `W3DView::cameraModLookToward` — whole remaining path faces `target`.
    pub(crate) fn camera_mod_look_toward(&mut self, target: Vec3) {
        self.set_look_toward(target);
    }

    /// C++ `W3DView::cameraModFinalLookToward` — last one or two segments only.
    pub(crate) fn camera_mod_final_look_toward(&mut self, target: Vec3) {
        self.set_look_toward(target);
        self.look_toward_is_final = true;
    }

    /// Look used for the current path segment. Final-look stays travel-facing
    /// until `cur_segment >= max(last-1, 2)`; the last waypoint is full look,
    /// the previous one half-lerps the yaw (C++ `W3DView.cpp:2667-2708`).
    pub(crate) fn look_toward_for_current_segment(&self) -> Option<Vec3> {
        let target = self.look_toward?;
        if !self.look_toward_is_final {
            return Some(target);
        }
        let last_meaningful = self.points.len().saturating_sub(2);
        let min = last_meaningful.saturating_sub(1).max(2);
        if self.cur_segment < min {
            return None;
        }
        if self.cur_segment >= last_meaningful {
            return Some(target);
        }
        let focus = self
            .smoothed_focus
            .or_else(|| self.points.get(self.cur_segment).copied())?;
        let travel = self.travel_look_toward()?;
        let current_angle = look_angle_xz(focus, travel);
        let target_angle = look_angle_xz(focus, target);
        let delta = normalize_camera_angle(target_angle - current_angle);
        let half = normalize_camera_angle(current_angle + delta * 0.5);
        Some(look_point_from_angle_xz(focus, half))
    }

    pub(crate) fn look_toward(&self) -> Option<Vec3> {
        self.look_toward
    }

    pub(crate) fn set_suppress_travel_look(&mut self, suppress: bool) {
        self.suppress_travel_look = suppress;
        if suppress {
            self.look_toward = None;
        }
    }

    pub(crate) fn suppress_travel_look(&self) -> bool {
        self.suppress_travel_look
    }

    pub(crate) fn travel_look_toward(&self) -> Option<Vec3> {
        let i = self.cur_segment.max(1);
        let a = self.points.get(i)?;
        let b = self.points.get(i + 1)?;
        let dir = *b - *a;
        if dir.length_squared() <= f32::EPSILON {
            return None;
        }
        Some(*a + dir)
    }

    pub(crate) fn set_rolling_average_frames(&mut self, frames: i32) {
        self.rolling_average_frames = frames.max(1);
    }

    pub(crate) fn current_speed_multiplier(&self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        if progress <= self.speed_ramp_start_t {
            return self.speed_ramp_start_multiplier;
        }
        let span = (1.0 - self.speed_ramp_start_t).max(f32::EPSILON);
        let t = ((progress - self.speed_ramp_start_t) / span).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier
            + (self.speed_ramp_final_multiplier - self.speed_ramp_start_multiplier) * t
    }

    pub(crate) fn set_final_speed_multiplier(&mut self, multiplier: f32) {
        if !multiplier.is_finite() {
            return;
        }
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        self.speed_ramp_start_multiplier = self.current_speed_multiplier(progress);
        self.speed_ramp_start_t = progress;
        self.speed_ramp_final_multiplier = multiplier.max(0.0);
    }

    fn rebuild_segments(&mut self) {
        let last_meaningful = self.points.len().saturating_sub(2);
        self.segment_length = vec![0.0f32; self.points.len()];
        self.total_distance = 0.0;
        for i in 1..last_meaningful {
            let a = self.points[i];
            let b = self.points[i + 1];
            let len = Vec2::new(b.x - a.x, b.z - a.z).length();
            self.segment_length[i] = len;
            self.total_distance += len;
        }
        if self.total_distance < 1.0 && last_meaningful >= 2 {
            let idx = last_meaningful - 1;
            self.segment_length[idx] += 1.0 - self.total_distance;
            self.total_distance = 1.0;
        }
        if last_meaningful >= 2 {
            self.segment_length[last_meaningful] = self.segment_length[last_meaningful - 1];
        }
    }

    /// C++ `W3DView::cameraModFinalMoveTo` — shift waypoints `[2..num]`.
    pub(crate) fn camera_mod_final_move_to(&mut self, target: Vec3) {
        let last_meaningful = self.points.len().saturating_sub(2);
        if last_meaningful < 2 {
            return;
        }
        let start = self.points[last_meaningful];
        let dx = target.x - start.x;
        let dz = target.z - start.z;
        for i in 2..=last_meaningful {
            self.points[i].x += dx;
            self.points[i].z += dz;
        }
        let last = self.points[last_meaningful];
        let prev = self.points[last_meaningful - 1];
        if self.points.len() > last_meaningful + 1 {
            self.points[last_meaningful + 1] =
                Vec3::new(last.x + (last.x - prev.x), 0.0, last.z + (last.z - prev.z));
        }
        self.rebuild_segments();
    }

    pub(crate) fn advance(&mut self, dt: f32) -> Option<Vec3> {
        let last_meaningful = self.points.len().saturating_sub(2);
        if last_meaningful <= 1 {
            return None;
        }

        let prev_ease = self.last_ease;
        let progress = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let speed_multiplier = self.current_speed_multiplier(progress).max(0.0);
        self.elapsed_seconds =
            (self.elapsed_seconds + dt.max(0.0) * speed_multiplier).min(self.total_time_seconds);
        let t = (self.elapsed_seconds / self.total_time_seconds).clamp(0.0, 1.0);
        let next_ease = self.ease.eval(t);
        self.last_ease = next_ease;

        let delta = next_ease - prev_ease;
        self.cur_seg_distance += delta * self.total_distance;

        while self.cur_segment < last_meaningful
            && self.cur_seg_distance >= self.segment_length[self.cur_segment]
        {
            self.cur_seg_distance -= self.segment_length[self.cur_segment];
            self.cur_segment += 1;
            if self.cur_segment >= last_meaningful {
                return None;
            }
        }

        self.cur_shutter = self.cur_shutter.saturating_sub(1);
        if self.cur_shutter > 0 {
            return None;
        }
        self.cur_shutter = self.shutter_frames;

        let seg_len = self.segment_length[self.cur_segment].max(f32::EPSILON);
        let mut factor = (self.cur_seg_distance / seg_len).clamp(0.0, 1.0);

        let (start, mid, end) = if factor < 0.5 {
            let start = (self.points[self.cur_segment - 1] + self.points[self.cur_segment]) * 0.5;
            let mid = self.points[self.cur_segment];
            let end = (self.points[self.cur_segment] + self.points[self.cur_segment + 1]) * 0.5;
            factor += 0.5;
            (start, mid, end)
        } else {
            let start = (self.points[self.cur_segment] + self.points[self.cur_segment + 1]) * 0.5;
            let mid = self.points[self.cur_segment + 1];
            let end = (self.points[self.cur_segment + 1] + self.points[self.cur_segment + 2]) * 0.5;
            factor -= 0.5;
            (start, mid, end)
        };

        let p =
            start + (end - start) * factor + (mid - end + mid - start) * (1.0 - factor) * factor;
        let focus = Vec3::new(p.x, 0.0, p.z);
        let average_factor = 1.0 / self.rolling_average_frames.max(1) as f32;
        let smoothed = if let Some(previous) = self.smoothed_focus {
            previous + (focus - previous) * average_factor
        } else {
            focus
        };
        self.smoothed_focus = Some(smoothed);
        Some(smoothed)
    }
}

fn look_angle_xz(from: Vec3, to: Vec3) -> f32 {
    let dir = Vec2::new(to.x - from.x, to.z - from.z);
    if dir.length() < 0.1 {
        return 0.0;
    }
    normalize_camera_angle(dir.y.atan2(dir.x) - std::f32::consts::PI * 0.5)
}

fn look_point_from_angle_xz(from: Vec3, angle: f32) -> Vec3 {
    Vec3::new(
        from.x - angle.sin() * 100.0,
        from.y,
        from.z + angle.cos() * 100.0,
    )
}

fn normalize_camera_angle(mut angle: f32) -> f32 {
    if !(-10.0 * std::f32::consts::PI..=10.0 * std::f32::consts::PI).contains(&angle) {
        angle = 0.0;
    }
    while angle > std::f32::consts::PI {
        angle -= 2.0 * std::f32::consts::PI;
    }
    while angle < -std::f32::consts::PI {
        angle += 2.0 * std::f32::consts::PI;
    }
    angle
}

fn leftover_tactical_view_angle() -> f32 {
    #[cfg(feature = "game_client")]
    {
        game_client::display::view::with_tactical_view_ref(|view| view.angle())
    }
    #[cfg(not(feature = "game_client"))]
    {
        0.0
    }
}

#[cfg(test)]
impl ScriptCameraPathMove {
    pub(crate) fn from_points_for_test(points: Vec<Vec3>, seconds: f32) -> Self {
        let n = points.len();
        Self {
            points,
            segment_length: vec![1.0; n],
            total_distance: 1.0,
            ease: ParabolicEase::new(0.0, 0.0),
            total_time_seconds: seconds.max(0.001),
            elapsed_seconds: 0.0,
            cur_segment: 1,
            cur_seg_distance: 0.0,
            shutter_frames: 1,
            cur_shutter: 1,
            last_ease: 0.0,
            freeze_time: false,
            freeze_angle: false,
            look_toward: None,
            look_toward_is_final: false,
            suppress_travel_look: false,
            rolling_average_frames: 1,
            smoothed_focus: None,
            speed_ramp_start_t: 0.0,
            speed_ramp_start_multiplier: 1.0,
            speed_ramp_final_multiplier: 1.0,
            start_angle: 0.0,
            frozen_to_start_angle: false,
        }
    }
}

#[cfg(test)]
mod camera_mod_look_tests {
    use super::*;

    fn padded_path() -> ScriptCameraPathMove {
        // [pad, start, mid, last, pad] — last_meaningful = 3, min final = 2
        ScriptCameraPathMove::from_points_for_test(
            vec![
                Vec3::new(-10.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(100.0, 0.0, 0.0),
                Vec3::new(200.0, 0.0, 0.0),
                Vec3::new(300.0, 0.0, 0.0),
            ],
            4.0,
        )
    }

    #[test]
    fn final_look_toward_skips_early_path_segments() {
        let mut path = padded_path();
        path.camera_mod_final_look_toward(Vec3::new(200.0, 0.0, 200.0));
        path.cur_segment = 1;
        assert!(
            path.look_toward_for_current_segment().is_none(),
            "CAMERA_MOD_FINAL_LOOK_TOWARD must not yaw mid-path corners"
        );
        path.cur_segment = 3;
        let look = path
            .look_toward_for_current_segment()
            .expect("last segment takes the full look");
        assert!((look.x - 200.0).abs() < 0.01 && (look.z - 200.0).abs() < 0.01);
    }

    #[test]
    fn look_toward_rewrites_every_remaining_segment() {
        let mut path = padded_path();
        path.camera_mod_look_toward(Vec3::new(50.0, 0.0, 80.0));
        path.cur_segment = 1;
        let look = path
            .look_toward_for_current_segment()
            .expect("LOOK_TOWARD faces the target on every remaining segment");
        assert!((look.x - 50.0).abs() < 0.01 && (look.z - 80.0).abs() < 0.01);
    }

    #[test]
    fn freeze_angles_to_start_uses_start_yaw() {
        let mut path = padded_path();
        path.start_angle = 0.5;
        path.cur_segment = 2;
        path.freeze_angles_to_start();
        let focus = Vec3::new(100.0, 0.0, 0.0);
        let look = path
            .frozen_start_look_toward(focus)
            .expect("FREEZE_ANGLE must rewrite remaining path to start yaw");
        let expected = super::look_point_from_angle_xz(focus, 0.5);
        assert!((look - expected).length() < 0.01);
        let travel = path.travel_look_toward().expect("travel look exists");
        assert!(
            (look - travel).length() > 1.0,
            "start yaw must differ from current travel facing"
        );
    }
}
