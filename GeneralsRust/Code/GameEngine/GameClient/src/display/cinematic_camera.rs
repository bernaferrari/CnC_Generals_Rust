//! # Cinematic Camera System
//!
//! Complete camera system for C&C Generals Zero Hour with cinematic capabilities.
//! Ported from C++ TacticalView, W3DView, and CameraShakeSystem.
//!
//! ## Features
//! - Camera shake effects (explosions, earthquakes)
//! - Scripted camera paths for cutscenes
//! - Camera transitions (pan, zoom, rotate, pitch)
//! - Follow camera for units
//! - Death camera (zoom to killed unit)
//! - Camera constraints and boundaries
//! - Smooth interpolation with easing functions
//!
//! ## C++ References
//! - `/GeneralsMD/Code/GameEngine/Source/GameClient/View.cpp`
//! - `/GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DView.h`
//! - `/GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/camerashakesystem.cpp`

use glam::{Mat4, Quat, Vec2, Vec3};
use std::collections::VecDeque;
use std::f32::consts::PI;

// ================================================================================================
// CONSTANTS - Matched from C++ camerashakesystem.cpp
// ================================================================================================

/// Minimum angular frequency for camera shake (radians/second)
/// C++ Reference: camerashakesystem.cpp line 58
const MIN_OMEGA: f32 = 12.5 * 360.0 * PI / 180.0;

/// Maximum angular frequency for camera shake (radians/second)
/// C++ Reference: camerashakesystem.cpp line 59
const MAX_OMEGA: f32 = 15.0 * 360.0 * PI / 180.0;

/// End angular frequency for camera shake (radians/second)
/// C++ Reference: camerashakesystem.cpp line 60
const END_OMEGA: f32 = 360.0 * PI / 180.0;

/// Minimum phase shift for camera shake (radians)
/// C++ Reference: camerashakesystem.cpp line 61
const MIN_PHI: f32 = 0.0;

/// Maximum phase shift for camera shake (radians)
/// C++ Reference: camerashakesystem.cpp line 62
const MAX_PHI: f32 = 360.0 * PI / 180.0;

/// Axis rotation amplitudes for camera shake (pitch, yaw, roll)
/// C++ Reference: camerashakesystem.cpp line 63
/// Pitch is 2x yaw because vertical motion is more effective
const AXIS_ROTATION: Vec3 = Vec3::new(
    7.5 * PI / 180.0,  // Pitch (X-axis)
    15.0 * PI / 180.0, // Yaw (Y-axis) - doubled from pitch
    5.0 * PI / 180.0,  // Roll (Z-axis)
);

/// Maximum waypoints for camera path
/// C++ Reference: W3DView.h line 28
const MAX_WAYPOINTS: usize = 25;

/// Default field of view in radians
/// C++ Reference: View.cpp line 53
const DEFAULT_FOV: f32 = 50.0 * PI / 180.0;

/// Default pitch limit (in radians) - 36 degrees
/// C++ Reference: View.cpp line 152
const PITCH_LIMIT: f32 = PI / 5.0;

fn clamp_shake_angles(mut angles: Vec3) -> Vec3 {
    angles.x = angles.x.clamp(-AXIS_ROTATION.x, AXIS_ROTATION.x);
    angles.y = angles.y.clamp(-AXIS_ROTATION.y, AXIS_ROTATION.y);
    angles.z = angles.z.clamp(-AXIS_ROTATION.z, AXIS_ROTATION.z);
    angles
}

// ================================================================================================
// ENUMS
// ================================================================================================

/// Camera shake intensity types
/// C++ Reference: View.h lines 51-60
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraShakeType {
    /// Subtle shake (small explosions)
    Subtle = 0,
    /// Normal shake (medium explosions)
    Normal = 1,
    /// Strong shake (large explosions)
    Strong = 2,
    /// Severe shake (massive explosions)
    Severe = 3,
    /// Extreme shake (cinematics only)
    CineExtreme = 4,
    /// Insane shake (cinematics only)
    CineInsane = 5,
}

impl CameraShakeType {
    /// Get the power multiplier for this shake type
    pub fn power(&self) -> f32 {
        match self {
            CameraShakeType::Subtle => 0.5,
            CameraShakeType::Normal => 1.0,
            CameraShakeType::Strong => 2.0,
            CameraShakeType::Severe => 4.0,
            CameraShakeType::CineExtreme => 8.0,
            CameraShakeType::CineInsane => 16.0,
        }
    }
}

/// Camera lock/follow types
/// C++ Reference: View.h line 195
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraLockType {
    /// Follow the object directly
    Follow,
    /// Tether to object with maximum distance
    Tether,
}

// ================================================================================================
// CAMERA SHAKE SYSTEM
// ================================================================================================

/// Individual camera shake instance
/// C++ Reference: camerashakesystem.h lines 45-65
#[derive(Debug, Clone)]
struct CameraShaker {
    /// World position of shake epicenter
    position: Vec3,
    /// Radius of effect
    radius: f32,
    /// Duration in seconds
    duration: f32,
    /// Shake intensity
    intensity: f32,
    /// Elapsed time
    elapsed_time: f32,
    /// Angular frequencies for each axis
    omega: Vec3,
    /// Phase shifts for each axis
    phi: Vec3,
}

impl CameraShaker {
    /// Create a new camera shaker
    /// C++ Reference: camerashakesystem.cpp lines 71-93
    fn new(position: Vec3, radius: f32, duration: f32, intensity: f32) -> Self {
        // Initialize random sinusoid values
        let omega = Vec3::new(
            rand::random::<f32>() * (MAX_OMEGA - MIN_OMEGA) + MIN_OMEGA,
            rand::random::<f32>() * (MAX_OMEGA - MIN_OMEGA) + MIN_OMEGA,
            rand::random::<f32>() * (MAX_OMEGA - MIN_OMEGA) + MIN_OMEGA,
        );

        let phi = Vec3::new(
            rand::random::<f32>() * (MAX_PHI - MIN_PHI) + MIN_PHI,
            rand::random::<f32>() * (MAX_PHI - MIN_PHI) + MIN_PHI,
            rand::random::<f32>() * (MAX_PHI - MIN_PHI) + MIN_PHI,
        );

        Self {
            position,
            radius,
            duration,
            intensity,
            elapsed_time: 0.0,
            omega,
            phi,
        }
    }

    /// Update elapsed time
    fn timestep(&mut self, dt: f32) {
        self.elapsed_time += dt;
    }

    /// Check if shake has expired
    fn is_expired(&self) -> bool {
        self.elapsed_time >= self.duration
    }

    /// Compute rotation angles for camera shake
    /// C++ Reference: camerashakesystem.cpp lines 100-140
    fn compute_rotations(&self, camera_position: Vec3) -> Vec3 {
        // Check if camera is within radius of effect
        let offset = camera_position - self.position;
        let distance_sq = offset.length_squared();

        if distance_sq > self.radius * self.radius {
            return Vec3::ZERO;
        }

        // Calculate intensity falloff based on distance and time remaining
        // intensity(t,pos) = intensity * (radius/distance) * time_remaining/total_time
        let distance = distance_sq.sqrt();
        let distance_factor = if distance > 0.0 {
            1.0 - (distance / self.radius)
        } else {
            1.0
        };
        let time_factor = 1.0 - (self.elapsed_time / self.duration);
        let intensity = self.intensity * distance_factor * time_factor;

        // Compute sinusoidal shake for each axis
        // f(t) = intensity(t,pos) * sin(omega(t) * t + phi)
        // omega(t) = start_omega + (end_omega - start_omega) * t
        let mut angles = Vec3::ZERO;

        for i in 0..3 {
            let omega = self.omega[i] + (END_OMEGA - self.omega[i]) * self.elapsed_time;
            angles[i] =
                AXIS_ROTATION[i] * intensity * (omega * self.elapsed_time + self.phi[i]).sin();

            // C++ creates and adds one secondary random vector inside the axis loop,
            // so the non-periodic perturbation is accumulated three times per shaker.
            let minor_intensity = intensity * 0.5;
            angles.x += (rand::random::<f32>() * 2.0 - 1.0) * minor_intensity;
            angles.y += (rand::random::<f32>() * 2.0 - 1.0) * minor_intensity;
            angles.z += (rand::random::<f32>() * 2.0 - 1.0) * minor_intensity;
        }

        angles
    }
}

/// Camera shake system managing multiple active shakes
/// C++ Reference: camerashakesystem.h lines 17-69
#[derive(Debug, Clone, Default)]
pub struct CameraShakeSystem {
    /// Active camera shakers
    shakers: Vec<CameraShaker>,
}

impl CameraShakeSystem {
    /// Create a new camera shake system
    pub fn new() -> Self {
        Self {
            shakers: Vec::new(),
        }
    }

    /// Add a camera shake effect
    /// C++ Reference: camerashakesystem.cpp lines 164-183
    ///
    /// # Arguments
    /// * `position` - World position of shake epicenter
    /// * `radius` - Radius of effect in world units
    /// * `duration` - Duration in seconds
    /// * `power` - Power in degrees of amplitude (converted to radians internally)
    pub fn add_camera_shake(&mut self, position: Vec3, radius: f32, duration: f32, power: f32) {
        // Convert power from degrees to radians
        let power_radians = power * PI / 180.0;

        let shaker = CameraShaker::new(position, radius, duration, power_radians);
        self.shakers.push(shaker);
    }

    /// Update all active shakers
    /// C++ Reference: camerashakesystem.cpp lines 201-225
    pub fn timestep(&mut self, dt: f32) {
        // Update all shakers
        for shaker in self.shakers.iter_mut() {
            shaker.timestep(dt);
        }

        // Remove expired shakers
        self.shakers.retain(|shaker| !shaker.is_expired());
    }

    /// Check if camera is currently shaking
    /// C++ Reference: camerashakesystem.cpp lines 185-198
    pub fn is_camera_shaking(&self) -> bool {
        !self.shakers.is_empty()
    }

    /// Compute accumulated shake angles for camera
    /// C++ Reference: camerashakesystem.cpp lines 227-263
    pub fn update_camera_shaker(&self, camera_position: Vec3) -> Vec3 {
        let mut angles = Vec3::ZERO;

        // Accumulate effects of all active shakers
        for shaker in &self.shakers {
            angles += shaker.compute_rotations(camera_position);
        }

        clamp_shake_angles(angles)
    }
}

// ================================================================================================
// EASING FUNCTIONS
// ================================================================================================

/// Parabolic ease function for smooth transitions
/// C++ Reference: GameClient/ParabolicEase.h
#[derive(Debug, Clone, Copy)]
pub struct ParabolicEase {
    ease_in: f32,
    ease_out: f32,
}

impl ParabolicEase {
    /// Create a new parabolic ease function
    ///
    /// # Arguments
    /// * `ease_in` - Ease in factor (0.0 = linear, 1.0 = full ease)
    /// * `ease_out` - Ease out factor (0.0 = linear, 1.0 = full ease)
    pub fn new(ease_in: f32, ease_out: f32) -> Self {
        let mut ease = Self {
            ease_in: 0.0,
            ease_out: 0.0,
        };
        ease.set_ease_times(ease_in, ease_out);
        ease
    }

    /// Initialize the ease-in/ease-out function (C++ ParabolicEase::setEaseTimes).
    pub fn set_ease_times(&mut self, ease_in_time: f32, ease_out_time: f32) {
        let mut ease_in = ease_in_time.clamp(0.0, 1.0);
        let mut ease_out = 1.0 - ease_out_time;

        if !(0.0..=1.0).contains(&ease_out) {
            ease_out = ease_out.clamp(0.0, 1.0);
        }

        if ease_in > ease_out {
            ease_in = ease_out;
        }

        self.ease_in = ease_in;
        self.ease_out = ease_out;
    }

    /// Apply easing to a normalized time value [0, 1]
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let v0 = 1.0 + self.ease_out - self.ease_in;

        if t < self.ease_in {
            t * t / (v0 * self.ease_in)
        } else if t <= self.ease_out {
            (self.ease_in + 2.0 * (t - self.ease_in)) / v0
        } else {
            (self.ease_in
                + 2.0 * (self.ease_out - self.ease_in)
                + (2.0 * (t - self.ease_out) + self.ease_out * self.ease_out - t * t)
                    / (1.0 - self.ease_out))
                / v0
        }
    }
}

impl Default for ParabolicEase {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

// ================================================================================================
// CAMERA PATH SYSTEM
// ================================================================================================

/// Waypoint for camera path
#[derive(Debug, Clone, Copy)]
pub struct CameraWaypoint {
    /// World position
    pub position: Vec3,
    /// Camera angle at this waypoint
    pub angle: f32,
    /// Time multiplier at this waypoint
    pub time_multiplier: i32,
}

/// Camera path for scripted movements
/// C++ Reference: W3DView.h lines 32-49
#[derive(Debug, Clone)]
pub struct CameraPath {
    /// Waypoints along the path
    waypoints: Vec<CameraWaypoint>,
    /// Segment lengths
    segment_lengths: Vec<f32>,
    /// Total path length
    total_distance: f32,
    /// Total time in milliseconds
    total_time_ms: i32,
    /// Elapsed time in milliseconds
    elapsed_time_ms: i32,
    /// Current segment index
    current_segment: usize,
    /// Distance along current segment
    current_segment_distance: f32,
    /// Shutter setting
    shutter: i32,
    /// Current shutter frame
    current_shutter: i32,
    /// Rolling average frames
    rolling_average_frames: i32,
    /// Easing function
    ease: ParabolicEase,
    /// Whether to orient camera along path
    orient: bool,
}

impl CameraPath {
    /// Create a new camera path
    pub fn new(
        waypoints: Vec<CameraWaypoint>,
        total_time_ms: i32,
        shutter: i32,
        orient: bool,
        ease_in: f32,
        ease_out: f32,
    ) -> Self {
        let mut path = Self {
            waypoints: waypoints.clone(),
            segment_lengths: Vec::new(),
            total_distance: 0.0,
            total_time_ms,
            elapsed_time_ms: 0,
            current_segment: 0,
            current_segment_distance: 0.0,
            shutter,
            current_shutter: 0,
            rolling_average_frames: 1,
            ease: ParabolicEase::new(ease_in, ease_out),
            orient,
        };

        // Calculate segment lengths
        path.calculate_segments();
        path
    }

    /// Calculate segment lengths and total distance
    fn calculate_segments(&mut self) {
        self.segment_lengths.clear();
        self.total_distance = 0.0;

        for i in 0..self.waypoints.len().saturating_sub(1) {
            let start = self.waypoints[i].position;
            let end = self.waypoints[i + 1].position;
            let length = (end - start).length();
            self.segment_lengths.push(length);
            self.total_distance += length;
        }
    }

    /// Update path with delta time in milliseconds
    /// Returns true if path is complete
    pub fn update(&mut self, delta_ms: i32) -> bool {
        let scaled_delta = ((delta_ms.max(0) as f32) * self.current_time_multiplier()) as i32;
        self.elapsed_time_ms += scaled_delta;

        if self.elapsed_time_ms >= self.total_time_ms {
            self.elapsed_time_ms = self.total_time_ms.max(0);
            return true;
        }

        // Update shutter
        if self.shutter > 0 {
            self.current_shutter = (self.elapsed_time_ms / self.shutter) % self.shutter;
        } else {
            self.current_shutter = 0;
        }

        false
    }

    fn segment_index_and_t(&self, eased_t: f32) -> (usize, f32) {
        let segment_index = ((self.waypoints.len() - 1) as f32 * eased_t) as usize;
        let segment_index = segment_index.min(self.waypoints.len() - 2);
        let segment_t = eased_t * (self.waypoints.len() - 1) as f32 - segment_index as f32;
        (segment_index, segment_t.clamp(0.0, 1.0))
    }

    fn position_at_eased_time(&self, eased_t: f32) -> Vec3 {
        if self.waypoints.is_empty() {
            return Vec3::ZERO;
        }

        if self.waypoints.len() == 1 {
            return self.waypoints[0].position;
        }

        // Find target distance along path
        let target_distance = eased_t * self.total_distance;

        // Find which segment we're in
        let mut accumulated_distance = 0.0;
        for (i, &length) in self.segment_lengths.iter().enumerate() {
            if accumulated_distance + length >= target_distance {
                // Interpolate within this segment
                let segment_t = (target_distance - accumulated_distance) / length;
                let start = self.waypoints[i].position;
                let end = self.waypoints[i + 1].position;
                return start.lerp(end, segment_t);
            }
            accumulated_distance += length;
        }

        // Return last waypoint if we've gone past the end
        self.waypoints.last().unwrap().position
    }

    /// Get current position along path
    pub fn get_current_position(&self) -> Vec3 {
        if self.waypoints.is_empty() {
            return Vec3::ZERO;
        }

        let total_time = self.total_time_ms.max(1) as f32;
        let t = (self.elapsed_time_ms as f32 / total_time).clamp(0.0, 1.0);
        if self.rolling_average_frames <= 1 {
            return self.position_at_eased_time(self.ease.apply(t));
        }

        let sample_count = self.rolling_average_frames.max(1) as usize;
        let mut accum = Vec3::ZERO;
        for i in 0..sample_count {
            let sample_t = (t - (i as f32 / total_time)).clamp(0.0, 1.0);
            accum += self.position_at_eased_time(self.ease.apply(sample_t));
        }
        accum / sample_count as f32
    }

    /// Get current camera angle
    pub fn get_current_angle(&self) -> f32 {
        if self.waypoints.is_empty() {
            return 0.0;
        }

        if self.waypoints.len() == 1 {
            return self.waypoints[0].angle;
        }

        let t = (self.elapsed_time_ms as f32 / self.total_time_ms.max(1) as f32).clamp(0.0, 1.0);
        let eased_t = self.ease.apply(t);
        let (segment_index, segment_t) = self.segment_index_and_t(eased_t);

        let start_angle = self.waypoints[segment_index].angle;
        let end_angle = self.waypoints[segment_index + 1].angle;

        start_angle + (end_angle - start_angle) * segment_t
    }

    /// Check if path is complete
    pub fn is_complete(&self) -> bool {
        self.elapsed_time_ms >= self.total_time_ms
    }

    fn current_time_multiplier(&self) -> f32 {
        if self.waypoints.is_empty() {
            return 1.0;
        }
        if self.waypoints.len() == 1 {
            return self.waypoints[0].time_multiplier.max(1) as f32;
        }

        let t = (self.elapsed_time_ms as f32 / self.total_time_ms.max(1) as f32).clamp(0.0, 1.0);
        let eased_t = self.ease.apply(t);
        let (segment_index, segment_t) = self.segment_index_and_t(eased_t);

        let start_multiplier = self.waypoints[segment_index].time_multiplier.max(1) as f32;
        let end_multiplier = self.waypoints[segment_index + 1].time_multiplier.max(1) as f32;
        (start_multiplier + (end_multiplier - start_multiplier) * segment_t).max(1.0)
    }

    pub fn set_final_time_multiplier(&mut self, multiplier: i32) {
        if let Some(last) = self.waypoints.last_mut() {
            last.time_multiplier = multiplier.max(1);
        }
    }

    pub fn set_rolling_average_frames(&mut self, frames: i32) {
        self.rolling_average_frames = frames.max(1);
    }

    fn extended_waypoint_position(&self, index: isize) -> Vec3 {
        let len = self.waypoints.len() as isize;
        if len <= 0 {
            return Vec3::ZERO;
        }
        if len == 1 {
            return self.waypoints[0].position;
        }

        if index < 0 {
            let first = self.waypoints[0].position;
            let second = self.waypoints[1].position;
            return first - (second - first);
        }
        if index >= len {
            let last = self.waypoints[(len - 1) as usize].position;
            let prev = self.waypoints[(len - 2) as usize].position;
            return last + (last - prev);
        }

        self.waypoints[index as usize].position
    }

    fn sample_spline_position(&self, index: usize) -> Vec3 {
        // Matches the quadratic midpoint blend used by W3DView::moveAlongWaypointPath.
        let mid = self.extended_waypoint_position(index as isize);
        let prev = self.extended_waypoint_position(index as isize - 1);
        let next = self.extended_waypoint_position(index as isize + 1);

        let start = (prev + mid) * 0.5;
        let end = (mid + next) * 0.5;
        let factor = 0.5_f32;

        let mut result = start;
        result += factor * (end - start);
        result += (1.0 - factor) * factor * ((mid - end) + (mid - start));
        result
    }

    fn look_toward_angle_at(&self, index: usize, target: Vec3) -> Option<f32> {
        let result = self.sample_spline_position(index);
        let dir = Vec2::new(target.x - result.x, target.y - result.y);
        if dir.length() < 0.1 {
            return None;
        }

        let angle = dir.y.atan2(dir.x) - PI * 0.5;
        Some(normalize_camera_angle(angle))
    }

    /// C++ parity for `W3DView::cameraModLookToward`.
    pub fn camera_mod_look_toward(&mut self, target: Vec3) {
        for index in 0..self.waypoints.len() {
            if let Some(angle) = self.look_toward_angle_at(index, target) {
                self.waypoints[index].angle = angle;
            }
        }
    }

    /// C++ parity for `W3DView::cameraModFinalLookToward`.
    pub fn camera_mod_final_look_toward(&mut self, target: Vec3) {
        let len = self.waypoints.len();
        if len == 0 {
            return;
        }

        let start = len.saturating_sub(2);
        for index in start..len {
            let Some(mut angle) = self.look_toward_angle_at(index, target) else {
                continue;
            };

            if index + 1 != len {
                let current = self.waypoints[index].angle;
                let delta = normalize_camera_angle(angle - current);
                angle = normalize_camera_angle(current + delta * 0.5);
            }

            self.waypoints[index].angle = angle;
        }
    }

    /// C++ parity for `W3DView::cameraModFinalMoveTo`.
    pub fn camera_mod_final_move_to(&mut self, target: Vec3) {
        let Some(last) = self.waypoints.last().map(|waypoint| waypoint.position) else {
            return;
        };
        let delta = target - last;
        for waypoint in &mut self.waypoints {
            waypoint.position += delta;
        }
    }

    pub fn freeze_angles_to_start(&mut self) {
        let Some(start_angle) = self.waypoints.first().map(|waypoint| waypoint.angle) else {
            return;
        };
        for waypoint in self.waypoints.iter_mut().skip(1) {
            waypoint.angle = start_angle;
        }
    }

    pub fn is_oriented(&self) -> bool {
        self.orient
    }
}

fn normalize_camera_angle(mut angle: f32) -> f32 {
    if !(-10.0 * PI..=10.0 * PI).contains(&angle) {
        angle = 0.0;
    }
    while angle < -PI {
        angle += 2.0 * PI;
    }
    while angle > PI {
        angle -= 2.0 * PI;
    }
    angle
}

// ================================================================================================
// CAMERA TRANSITION SYSTEM
// ================================================================================================

/// Camera rotation transition
/// C++ Reference: W3DView.h lines 53-74
#[derive(Debug, Clone)]
pub struct CameraRotateTransition {
    /// Number of frames
    num_frames: i32,
    /// Current frame
    current_frame: i32,
    /// Start angle
    start_angle: f32,
    /// End angle
    end_angle: f32,
    /// Easing function
    ease: ParabolicEase,
}

impl CameraRotateTransition {
    /// Create a new rotation transition
    pub fn new(
        rotations: f32,
        frames: i32,
        ease_in: f32,
        ease_out: f32,
        current_angle: f32,
    ) -> Self {
        let target_angle = current_angle + rotations * 2.0 * PI;

        Self {
            num_frames: frames,
            current_frame: 0,
            start_angle: current_angle,
            end_angle: target_angle,
            ease: ParabolicEase::new(ease_in, ease_out),
        }
    }

    /// Update transition
    /// Returns true if complete
    pub fn update(&mut self) -> bool {
        self.current_frame += 1;
        self.current_frame >= self.num_frames
    }

    /// Get current angle
    pub fn get_current_angle(&self) -> f32 {
        if self.num_frames <= 0 {
            return self.end_angle;
        }

        let t = (self.current_frame as f32) / (self.num_frames as f32);
        let eased_t = self.ease.apply(t);

        self.start_angle + (self.end_angle - self.start_angle) * eased_t
    }

    /// Remaining frames before this transition completes.
    pub fn remaining_frames(&self) -> i32 {
        (self.num_frames - self.current_frame).max(0)
    }

    pub fn freeze_current_angle(&mut self) {
        let angle = self.get_current_angle();
        self.start_angle = angle;
        self.end_angle = angle;
    }
}

/// Camera zoom transition
/// C++ Reference: W3DView.h lines 92-101
#[derive(Debug, Clone)]
pub struct CameraZoomTransition {
    /// Number of frames
    num_frames: i32,
    /// Current frame
    current_frame: i32,
    /// Start zoom
    start_zoom: f32,
    /// End zoom
    end_zoom: f32,
    /// Easing function
    ease: ParabolicEase,
}

impl CameraZoomTransition {
    /// Create a new zoom transition
    pub fn new(
        target_zoom: f32,
        frames: i32,
        ease_in: f32,
        ease_out: f32,
        current_zoom: f32,
    ) -> Self {
        Self {
            num_frames: frames,
            current_frame: 0,
            start_zoom: current_zoom,
            end_zoom: target_zoom,
            ease: ParabolicEase::new(ease_in, ease_out),
        }
    }

    /// Update transition
    /// Returns true if complete
    pub fn update(&mut self) -> bool {
        self.current_frame += 1;
        self.current_frame >= self.num_frames
    }

    /// Get current zoom
    pub fn get_current_zoom(&self) -> f32 {
        if self.num_frames <= 0 {
            return self.end_zoom;
        }

        let t = (self.current_frame as f32) / (self.num_frames as f32);
        let eased_t = self.ease.apply(t);

        self.start_zoom + (self.end_zoom - self.start_zoom) * eased_t
    }

    /// Remaining frames before this transition completes.
    pub fn remaining_frames(&self) -> i32 {
        (self.num_frames - self.current_frame).max(0)
    }
}

/// Camera pitch transition
/// C++ Reference: W3DView.h lines 78-88
#[derive(Debug, Clone)]
pub struct CameraPitchTransition {
    /// Number of frames
    num_frames: i32,
    /// Current frame
    current_frame: i32,
    /// Start pitch
    start_pitch: f32,
    /// End pitch
    end_pitch: f32,
    /// Easing function
    ease: ParabolicEase,
}

impl CameraPitchTransition {
    /// Create a new pitch transition
    pub fn new(
        target_pitch: f32,
        frames: i32,
        ease_in: f32,
        ease_out: f32,
        current_pitch: f32,
    ) -> Self {
        Self {
            num_frames: frames,
            current_frame: 0,
            start_pitch: current_pitch,
            end_pitch: target_pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT),
            ease: ParabolicEase::new(ease_in, ease_out),
        }
    }

    /// Update transition
    /// Returns true if complete
    pub fn update(&mut self) -> bool {
        self.current_frame += 1;
        self.current_frame >= self.num_frames
    }

    /// Get current pitch
    pub fn get_current_pitch(&self) -> f32 {
        if self.num_frames <= 0 {
            return self.end_pitch;
        }

        let t = (self.current_frame as f32) / (self.num_frames as f32);
        let eased_t = self.ease.apply(t);

        self.start_pitch + (self.end_pitch - self.start_pitch) * eased_t
    }

    /// Remaining frames before this transition completes.
    pub fn remaining_frames(&self) -> i32 {
        (self.num_frames - self.current_frame).max(0)
    }
}

/// Camera position transition (pan/move)
#[derive(Debug, Clone)]
pub struct CameraPositionTransition {
    /// Number of frames
    num_frames: i32,
    /// Current frame
    current_frame: i32,
    /// Start position
    start_position: Vec3,
    /// End position
    end_position: Vec3,
    /// Easing function
    ease: ParabolicEase,
}

impl CameraPositionTransition {
    /// Create a new position transition
    pub fn new(
        target_position: Vec3,
        frames: i32,
        ease_in: f32,
        ease_out: f32,
        current_position: Vec3,
    ) -> Self {
        Self {
            num_frames: frames,
            current_frame: 0,
            start_position: current_position,
            end_position: target_position,
            ease: ParabolicEase::new(ease_in, ease_out),
        }
    }

    /// Update transition
    /// Returns true if complete
    pub fn update(&mut self) -> bool {
        self.current_frame += 1;
        self.current_frame >= self.num_frames
    }

    /// Get current position
    pub fn get_current_position(&self) -> Vec3 {
        if self.num_frames <= 0 {
            return self.end_position;
        }

        let t = (self.current_frame as f32) / (self.num_frames as f32);
        let eased_t = self.ease.apply(t);

        self.start_position.lerp(self.end_position, eased_t)
    }

    /// Remaining frames before this transition completes.
    pub fn remaining_frames(&self) -> i32 {
        (self.num_frames - self.current_frame).max(0)
    }
}

// ================================================================================================
// CAMERA FOLLOW SYSTEM
// ================================================================================================

/// Camera follow/lock settings
/// C++ Reference: View.h lines 192-199
#[derive(Debug, Clone)]
pub struct CameraFollowSettings {
    /// Lock type (follow or tether)
    pub lock_type: CameraLockType,
    /// Maximum tether distance
    pub lock_distance: f32,
    /// Whether to snap immediately to target
    pub snap_immediate: bool,
}

impl Default for CameraFollowSettings {
    fn default() -> Self {
        Self {
            lock_type: CameraLockType::Follow,
            lock_distance: 0.0,
            snap_immediate: false,
        }
    }
}

/// Camera follow system for tracking objects/units
#[derive(Debug, Clone)]
pub struct CameraFollowSystem {
    /// Target object ID (if any)
    target_id: Option<u32>,
    /// Last known target position
    target_position: Vec3,
    /// Follow settings
    settings: CameraFollowSettings,
}

impl CameraFollowSystem {
    /// Create a new follow system
    pub fn new() -> Self {
        Self {
            target_id: None,
            target_position: Vec3::ZERO,
            settings: CameraFollowSettings::default(),
        }
    }

    /// Set target to follow
    pub fn set_target(&mut self, target_id: Option<u32>, target_position: Vec3) {
        self.target_id = target_id;
        self.target_position = target_position;
        self.settings.snap_immediate = false;
    }

    /// Set follow settings
    pub fn set_settings(&mut self, settings: CameraFollowSettings) {
        self.settings = settings;
    }

    /// Snap camera immediately to target
    pub fn snap_to_target(&mut self) {
        self.settings.snap_immediate = true;
    }

    /// Update camera position based on follow settings
    /// Returns the desired camera look-at position
    pub fn update(&mut self, current_position: Vec3, target_position: Vec3) -> Vec3 {
        self.target_position = target_position;

        if self.target_id.is_none() {
            return current_position;
        }

        match self.settings.lock_type {
            CameraLockType::Follow => {
                // Directly follow the target
                if self.settings.snap_immediate {
                    self.settings.snap_immediate = false;
                    target_position
                } else {
                    // Smooth follow with interpolation
                    current_position.lerp(target_position, 0.1)
                }
            }
            CameraLockType::Tether => {
                // Follow only if outside tether distance
                let offset = target_position - current_position;
                let distance = offset.length();

                if distance > self.settings.lock_distance {
                    // Move camera to stay within tether distance
                    let direction = offset / distance;
                    current_position + direction * (distance - self.settings.lock_distance)
                } else {
                    current_position
                }
            }
        }
    }

    /// Check if currently following a target
    pub fn is_following(&self) -> bool {
        self.target_id.is_some()
    }
}

impl Default for CameraFollowSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// CAMERA CONSTRAINTS
// ================================================================================================

/// Camera boundary constraints
/// C++ Reference: W3DView.h lines 257-258
#[derive(Debug, Clone)]
pub struct CameraConstraints {
    /// Minimum position
    pub min: Vec2,
    /// Maximum position
    pub max: Vec2,
    /// Whether constraints are valid
    pub valid: bool,
}

impl CameraConstraints {
    /// Create new unconstrained camera
    pub fn new() -> Self {
        Self {
            min: Vec2::new(f32::MIN, f32::MIN),
            max: Vec2::new(f32::MAX, f32::MAX),
            valid: false,
        }
    }

    /// Set constraint boundaries
    pub fn set_bounds(&mut self, min: Vec2, max: Vec2) {
        self.min = min;
        self.max = max;
        self.valid = true;
    }

    /// Apply constraints to position
    pub fn constrain(&self, position: Vec3) -> Vec3 {
        if !self.valid {
            return position;
        }

        Vec3::new(
            position.x.clamp(self.min.x, self.max.x),
            position.y.clamp(self.min.y, self.max.y),
            position.z,
        )
    }

    /// Invalidate constraints (remove them)
    pub fn invalidate(&mut self) {
        self.valid = false;
    }
}

impl Default for CameraConstraints {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// DEATH CAMERA
// ================================================================================================

/// Death camera for zooming to killed unit
#[derive(Debug, Clone)]
pub struct DeathCamera {
    /// Active death camera
    active: bool,
    /// Target position
    target_position: Vec3,
    /// Start position
    start_position: Vec3,
    /// Start zoom
    start_zoom: f32,
    /// Target zoom (closer)
    target_zoom: f32,
    /// Duration in frames
    duration_frames: i32,
    /// Current frame
    current_frame: i32,
    /// Easing function
    ease: ParabolicEase,
}

impl DeathCamera {
    /// Create a new death camera
    pub fn new() -> Self {
        Self {
            active: false,
            target_position: Vec3::ZERO,
            start_position: Vec3::ZERO,
            start_zoom: 1.0,
            target_zoom: 0.3,
            duration_frames: 60,
            current_frame: 0,
            ease: ParabolicEase::new(0.3, 0.7),
        }
    }

    /// Activate death camera for a unit at position
    pub fn activate(
        &mut self,
        death_position: Vec3,
        current_position: Vec3,
        current_zoom: f32,
        duration_frames: i32,
    ) {
        self.active = true;
        self.target_position = death_position;
        self.start_position = current_position;
        self.start_zoom = current_zoom;
        self.target_zoom = current_zoom * 0.3; // Zoom in closer
        self.duration_frames = duration_frames;
        self.current_frame = 0;
    }

    /// Update death camera
    /// Returns (position, zoom) if active, None otherwise
    pub fn update(&mut self) -> Option<(Vec3, f32)> {
        if !self.active {
            return None;
        }

        self.current_frame += 1;

        if self.current_frame >= self.duration_frames {
            self.active = false;
            return None;
        }

        let t = (self.current_frame as f32) / (self.duration_frames as f32);
        let eased_t = self.ease.apply(t);

        let position = self.start_position.lerp(self.target_position, eased_t);
        let zoom = self.start_zoom + (self.target_zoom - self.start_zoom) * eased_t;

        Some((position, zoom))
    }

    /// Check if death camera is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Deactivate death camera
    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

impl Default for DeathCamera {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// CINEMATIC CAMERA SYSTEM
// ================================================================================================

/// Complete cinematic camera system
/// Integrates all camera features for C&C Generals
pub struct CinematicCameraSystem {
    /// Camera shake system
    pub shake_system: CameraShakeSystem,
    /// Active camera path (if any)
    pub camera_path: Option<CameraPath>,
    /// Active rotate transition
    pub rotate_transition: Option<CameraRotateTransition>,
    /// Active zoom transition
    pub zoom_transition: Option<CameraZoomTransition>,
    /// Active pitch transition
    pub pitch_transition: Option<CameraPitchTransition>,
    /// Active position transition
    pub position_transition: Option<CameraPositionTransition>,
    /// Camera follow system
    pub follow_system: CameraFollowSystem,
    /// Camera constraints
    pub constraints: CameraConstraints,
    /// Death camera
    pub death_camera: DeathCamera,
}

impl CinematicCameraSystem {
    /// Create a new cinematic camera system
    pub fn new() -> Self {
        Self {
            shake_system: CameraShakeSystem::new(),
            camera_path: None,
            rotate_transition: None,
            zoom_transition: None,
            pitch_transition: None,
            position_transition: None,
            follow_system: CameraFollowSystem::new(),
            constraints: CameraConstraints::new(),
            death_camera: DeathCamera::new(),
        }
    }

    /// Update all camera systems
    /// Returns updated camera parameters
    pub fn update(&mut self, dt: f32, current_state: CameraState) -> CameraState {
        let mut state = current_state;

        // Update shake system
        self.shake_system.timestep(dt);
        let shake_angles = self.shake_system.update_camera_shaker(state.position);

        // Apply shake to rotation
        let shake_rotation = Quat::from_euler(
            glam::EulerRot::XYZ,
            shake_angles.x,
            shake_angles.y,
            shake_angles.z,
        );
        state.rotation *= shake_rotation;

        // Update death camera (takes priority)
        if let Some((death_pos, death_zoom)) = self.death_camera.update() {
            state.position = death_pos;
            state.zoom = death_zoom;
            return state;
        }

        // Update camera path
        if let Some(path) = &mut self.camera_path {
            if path.update((dt * 1000.0) as i32) {
                self.camera_path = None;
            } else {
                state.position = path.get_current_position();
                state.angle = path.get_current_angle();
            }
        }

        // Update transitions
        if let Some(transition) = &mut self.rotate_transition {
            if transition.update() {
                self.rotate_transition = None;
            } else {
                state.angle = transition.get_current_angle();
            }
        }

        if let Some(transition) = &mut self.zoom_transition {
            if transition.update() {
                self.zoom_transition = None;
            } else {
                state.zoom = transition.get_current_zoom();
            }
        }

        if let Some(transition) = &mut self.pitch_transition {
            if transition.update() {
                self.pitch_transition = None;
            } else {
                state.pitch = transition.get_current_pitch();
            }
        }

        if let Some(transition) = &mut self.position_transition {
            if transition.update() {
                self.position_transition = None;
            } else {
                state.position = transition.get_current_position();
            }
        }

        // Update follow system
        if self.follow_system.is_following() {
            // In a real implementation, this would query the game world for object position
            // For now, we just use the stored target position
            let target_pos = self.follow_system.target_position;
            state.position = self.follow_system.update(state.position, target_pos);
        }

        // Apply constraints
        state.position = self.constraints.constrain(state.position);

        state
    }
}

impl Default for CinematicCameraSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ================================================================================================
// CAMERA STATE
// ================================================================================================

/// Current camera state
#[derive(Debug, Clone, Copy)]
pub struct CameraState {
    /// Camera position (look-at point)
    pub position: Vec3,
    /// Camera rotation
    pub rotation: Quat,
    /// Camera angle around Z axis
    pub angle: f32,
    /// Camera pitch
    pub pitch: f32,
    /// Zoom level
    pub zoom: f32,
    /// Height above ground
    pub height: f32,
    /// Field of view
    pub fov: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angle: 0.0,
            pitch: 0.0,
            zoom: 1.0,
            height: 100.0,
            fov: DEFAULT_FOV,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_shake_basic() {
        let mut system = CameraShakeSystem::new();

        // Add a shake
        system.add_camera_shake(Vec3::ZERO, 50.0, 1.5, 1.0);

        assert!(system.is_camera_shaking());

        // Should produce shake angles at epicenter
        let angles = system.update_camera_shaker(Vec3::ZERO);
        assert!(angles.length() > 0.0);

        // Should produce no shake far away
        let far_angles = system.update_camera_shaker(Vec3::new(1000.0, 1000.0, 0.0));
        assert_eq!(far_angles, Vec3::ZERO);
    }

    #[test]
    fn test_camera_shake_expiration() {
        let mut system = CameraShakeSystem::new();

        system.add_camera_shake(Vec3::ZERO, 50.0, 0.1, 1.0);
        assert!(system.is_camera_shaking());

        // Update past duration
        system.timestep(0.2);
        assert!(!system.is_camera_shaking());
    }

    #[test]
    fn camera_shake_clamps_accumulated_angles_to_cpp_axis_limits() {
        let angles = clamp_shake_angles(AXIS_ROTATION * 4.0);
        assert_eq!(angles, AXIS_ROTATION);

        let angles = clamp_shake_angles(AXIS_ROTATION * -4.0);
        assert_eq!(angles, -AXIS_ROTATION);
    }

    #[test]
    fn test_parabolic_ease() {
        let ease = ParabolicEase::new(0.5, 0.5);

        // Start should be 0
        assert_eq!(ease.apply(0.0), 0.0);

        // End should be 1
        assert_eq!(ease.apply(1.0), 1.0);

        // Middle should be 0.5
        let mid = ease.apply(0.5);
        assert!((mid - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_camera_path() {
        let waypoints = vec![
            CameraWaypoint {
                position: Vec3::ZERO,
                angle: 0.0,
                time_multiplier: 1,
            },
            CameraWaypoint {
                position: Vec3::new(100.0, 0.0, 0.0),
                angle: PI / 2.0,
                time_multiplier: 1,
            },
        ];

        let mut path = CameraPath::new(waypoints, 1000, 1, false, 0.0, 0.0);

        // Start position
        let start_pos = path.get_current_position();
        assert!((start_pos - Vec3::ZERO).length() < 0.01);

        // Halfway
        path.update(500);
        let mid_pos = path.get_current_position();
        assert!((mid_pos.x - 50.0).abs() < 5.0);

        // End
        path.update(500);
        assert!(path.is_complete());
    }

    #[test]
    fn test_camera_constraints() {
        let mut constraints = CameraConstraints::new();
        constraints.set_bounds(Vec2::new(-100.0, -100.0), Vec2::new(100.0, 100.0));

        // Inside bounds - no change
        let pos = Vec3::new(50.0, 50.0, 0.0);
        let constrained = constraints.constrain(pos);
        assert_eq!(pos, constrained);

        // Outside bounds - clamped
        let pos = Vec3::new(200.0, -200.0, 0.0);
        let constrained = constraints.constrain(pos);
        assert_eq!(constrained.x, 100.0);
        assert_eq!(constrained.y, -100.0);
    }

    #[test]
    fn test_camera_follow_system() {
        let mut follow = CameraFollowSystem::new();

        // Set target
        follow.set_target(Some(1), Vec3::new(100.0, 100.0, 0.0));
        assert!(follow.is_following());

        // Should move towards target
        let new_pos = follow.update(Vec3::ZERO, Vec3::new(100.0, 100.0, 0.0));
        assert!(new_pos.length() > 0.0);
        assert!(new_pos.length() < 100.0); // Smooth follow
    }

    #[test]
    fn test_death_camera() {
        let mut death = DeathCamera::new();

        death.activate(Vec3::new(100.0, 100.0, 0.0), Vec3::ZERO, 1.0, 60);

        assert!(death.is_active());

        // Should interpolate towards target
        if let Some((pos, zoom)) = death.update() {
            assert!(pos.length() > 0.0);
            assert!(zoom < 1.0);
        }
    }

    #[test]
    fn test_cinematic_camera_integration() {
        let mut system = CinematicCameraSystem::new();
        let state = CameraState::default();

        // Add shake
        system
            .shake_system
            .add_camera_shake(Vec3::ZERO, 50.0, 1.0, 1.0);

        // Update system
        let new_state = system.update(0.016, state);

        // Rotation should be affected by shake
        assert_ne!(new_state.rotation, state.rotation);
    }
}
