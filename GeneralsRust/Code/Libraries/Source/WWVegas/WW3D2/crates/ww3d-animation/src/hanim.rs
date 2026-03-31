//! HAnim (Hierarchy Animation) base class implementation
//!
//! This module mirrors the legacy `HRawAnimClass` container and sampling logic. Channels are
//! stored per pivot and evaluated with the same frame wrapping and interpolation semantics used
//! by the original WW3D engine.

use super::htree::HTreeClass;
use glam::{Mat4, Quat, Vec3};
use std::array;

const FRAME_EPS: f32 = 1e-5;

/// Animation playback modes matching C++ RenderObjClass::AnimMode
/// Reference: rendobj.h:331-339
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationMode {
    /// ANIM_MODE_MANUAL - Application controls frame manually
    Manual,
    /// ANIM_MODE_LOOP - Loop continuously
    Loop,
    /// ANIM_MODE_ONCE - Play once and stop at last frame
    Once,
    /// ANIM_MODE_LOOP_PINGPONG - Play forward then backward continuously
    PingPong,
    /// ANIM_MODE_LOOP_BACKWARDS - Loop backwards continuously
    LoopBackwards,
    /// ANIM_MODE_ONCE_BACKWARDS - Play once backwards and stop
    OnceBackwards,
}

impl Default for AnimationMode {
    fn default() -> Self {
        AnimationMode::Loop
    }
}

/// Animation event for triggering game logic at specific frames
#[derive(Debug, Clone)]
pub struct AnimationEvent {
    pub frame: f32,
    pub event_type: String,
    pub data: String,
}

impl AnimationEvent {
    pub fn new(frame: f32, event_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            frame,
            event_type: event_type.into(),
            data: data.into(),
        }
    }
}

/// Axis helper used by translation and rotation channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Motion channel variants supported by the classic animation system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionChannelType {
    Translation(Axis),
    RotationAxis(Axis),
    Quaternion,
    Visibility,
    Unknown(u16),
}

impl MotionChannelType {
    pub fn from_flags(flags: u16) -> Self {
        match flags {
            0 => MotionChannelType::Translation(Axis::X),
            1 => MotionChannelType::Translation(Axis::Y),
            2 => MotionChannelType::Translation(Axis::Z),
            3 => MotionChannelType::RotationAxis(Axis::X),
            4 => MotionChannelType::RotationAxis(Axis::Y),
            5 => MotionChannelType::RotationAxis(Axis::Z),
            6 => MotionChannelType::Quaternion,
            7 => MotionChannelType::Translation(Axis::X), // time coded variants; fall back for now
            8 => MotionChannelType::Translation(Axis::Y),
            9 => MotionChannelType::Translation(Axis::Z),
            10 => MotionChannelType::Quaternion,
            11 => MotionChannelType::Translation(Axis::X), // adaptive delta; handled later
            12 => MotionChannelType::Translation(Axis::Y),
            13 => MotionChannelType::Translation(Axis::Z),
            14 => MotionChannelType::Quaternion,
            15 => MotionChannelType::Visibility,
            other => MotionChannelType::Unknown(other),
        }
    }
}

/// Motion channel with decoded keyframe payload.
#[derive(Debug, Clone)]
pub struct MotionChannel {
    channel_type: MotionChannelType,
    pivot_idx: usize,
    vector_len: usize,
    first_frame: u16,
    last_frame: u16,
    data: Vec<f32>,
}

impl MotionChannel {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        channel_type: MotionChannelType,
        pivot_idx: usize,
        first_frame: u16,
        last_frame: u16,
        vector_len: usize,
        data: Vec<f32>,
    ) -> Self {
        Self {
            channel_type,
            pivot_idx,
            vector_len,
            first_frame,
            last_frame,
            data,
        }
    }

    pub fn channel_type(&self) -> MotionChannelType {
        self.channel_type
    }

    pub fn pivot(&self) -> usize {
        self.pivot_idx
    }

    fn frame_in_range(&self, frame_index: usize) -> bool {
        frame_index >= self.first_frame as usize && frame_index <= self.last_frame as usize
    }

    fn sample_scalar(&self, frame_index: usize) -> f32 {
        if self.vector_len == 0 || !self.frame_in_range(frame_index) {
            return 0.0;
        }

        let local = (frame_index - self.first_frame as usize) * self.vector_len;
        self.data
            .get(local)
            .copied()
            .unwrap_or_else(|| *self.data.last().unwrap_or(&0.0))
    }

    fn sample_quaternion(&self, frame_index: usize) -> Quat {
        let mut components = [0.0f32; 4];
        if self.vector_len < 4 || !self.sample_vector_into(frame_index, &mut components) {
            return Quat::IDENTITY;
        }

        Quat::from_xyzw(components[0], components[1], components[2], components[3]).normalize()
    }

    fn sample_vector_into(&self, frame_index: usize, dst: &mut [f32]) -> bool {
        if self.vector_len == 0 || !self.frame_in_range(frame_index) {
            return false;
        }

        let vector_len = self.vector_len.min(dst.len());
        let local = (frame_index - self.first_frame as usize) * self.vector_len;
        let end = local + vector_len;
        if end > self.data.len() {
            return false;
        }

        dst[..vector_len].copy_from_slice(&self.data[local..end]);
        true
    }
}

/// Bit channel for compressed animation data.
#[derive(Debug, Clone)]
pub struct BitChannel {
    pub pivot_idx: usize,
    pub channel_type: MotionChannelType,
    pub data: Vec<u8>,
    pub first_frame: u16,
    pub last_frame: u16,
    pub default_value: u8,
}

impl BitChannel {
    pub fn get_bit(&self, frame_index: usize) -> u8 {
        if frame_index < self.first_frame as usize || frame_index > self.last_frame as usize {
            return self.default_value;
        }

        let offset = frame_index - self.first_frame as usize;
        let byte = offset / 8;
        let bit = offset % 8;
        self.data
            .get(byte)
            .map(|value| (value >> bit) & 1)
            .unwrap_or(self.default_value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisibilityBinding {
    Scalar(usize),
    Bit(usize),
}

#[derive(Debug, Clone)]
struct PivotMotion {
    translation: [Option<usize>; 3],
    rotation: Option<usize>,
    visibility: Option<VisibilityBinding>,
    #[allow(dead_code)] // C++ parity
    rotation_axes: [Option<usize>; 3],
}

impl Default for PivotMotion {
    fn default() -> Self {
        Self {
            translation: array::from_fn(|_| None),
            rotation: None,
            visibility: None,
            rotation_axes: array::from_fn(|_| None),
        }
    }
}

/// Base animation class for all W3D animation formats.
#[derive(Debug, Clone)]
pub struct HAnimClass {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub total_time: f32,
    channels: Vec<MotionChannel>,
    pivot_motions: Vec<PivotMotion>,
    bit_channels: Vec<BitChannel>,
    embedded_sound_bone_index: Option<usize>,
    // Playback state
    mode: AnimationMode,
    current_frame: f32,
    speed_multiplier: f32,
    is_complete: bool,
    events: Vec<AnimationEvent>,
    last_frame: f32,
}

impl HAnimClass {
    /// Create a new animation container without any channels.
    pub fn new(name: &str, hierarchy_name: &str, num_frames: u32, frame_rate: f32) -> Self {
        let total_time = if frame_rate > 0.0 {
            num_frames as f32 / frame_rate
        } else {
            0.0
        };

        Self {
            name: name.to_string(),
            hierarchy_name: hierarchy_name.to_string(),
            num_frames,
            frame_rate,
            total_time,
            channels: Vec::new(),
            pivot_motions: Vec::new(),
            bit_channels: Vec::new(),
            embedded_sound_bone_index: None,
            mode: AnimationMode::Loop,
            current_frame: 0.0,
            speed_multiplier: 1.0,
            is_complete: false,
            events: Vec::new(),
            last_frame: 0.0,
        }
    }

    /// Construct an animation from decoded motion channels.
    pub fn with_channels(
        name: &str,
        hierarchy_name: &str,
        num_frames: u32,
        frame_rate: f32,
        channels: Vec<MotionChannel>,
        bit_channels: Vec<BitChannel>,
    ) -> Self {
        let mut anim = Self::new(name, hierarchy_name, num_frames, frame_rate);
        for channel in channels {
            anim.install_channel(channel);
        }
        for bit_channel in bit_channels {
            anim.install_bit_channel(bit_channel);
        }
        anim
    }

    fn install_channel(&mut self, channel: MotionChannel) {
        let pivot = channel.pivot();
        if pivot >= self.pivot_motions.len() {
            self.pivot_motions
                .resize_with(pivot + 1, PivotMotion::default);
        }

        self.channels.push(channel);
        let index = self.channels.len() - 1;
        match self.channels[index].channel_type() {
            MotionChannelType::Translation(axis) => {
                self.pivot_motions[pivot].translation[axis_index(axis)] = Some(index);
            }
            MotionChannelType::Quaternion => {
                self.pivot_motions[pivot].rotation = Some(index);
            }
            MotionChannelType::RotationAxis(axis) => {
                self.pivot_motions[pivot].rotation_axes[axis_index(axis)] = Some(index);
            }
            MotionChannelType::Visibility => {
                self.pivot_motions[pivot].visibility = Some(VisibilityBinding::Scalar(index));
            }
            MotionChannelType::Unknown(_) => {}
        }
    }

    fn install_bit_channel(&mut self, channel: BitChannel) {
        if channel.pivot_idx >= self.pivot_motions.len() {
            self.pivot_motions
                .resize_with(channel.pivot_idx + 1, PivotMotion::default);
        }
        let pivot = channel.pivot_idx;
        if matches!(channel.channel_type, MotionChannelType::Visibility) {
            let index = self.bit_channels.len();
            self.bit_channels.push(channel);
            self.pivot_motions[pivot].visibility = Some(VisibilityBinding::Bit(index));
        } else {
            self.bit_channels.push(channel);
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_hierarchy_name(&self) -> &str {
        &self.hierarchy_name
    }

    pub fn get_num_frames(&self) -> u32 {
        self.num_frames
    }

    pub fn get_frame_rate(&self) -> f32 {
        self.frame_rate
    }

    pub fn get_total_time(&self) -> f32 {
        self.total_time
    }

    pub fn set_embedded_sound_bone_index(&mut self, index: Option<usize>) {
        self.embedded_sound_bone_index = index;
    }

    pub fn embedded_sound_bone_index(&self) -> Option<usize> {
        self.embedded_sound_bone_index
    }

    pub fn num_pivots(&self) -> usize {
        self.pivot_motions.len()
    }

    pub fn has_translation_x(&self, pivot_idx: usize) -> bool {
        self.has_translation_axis(pivot_idx, Axis::X)
    }

    pub fn has_translation_y(&self, pivot_idx: usize) -> bool {
        self.has_translation_axis(pivot_idx, Axis::Y)
    }

    pub fn has_translation_z(&self, pivot_idx: usize) -> bool {
        self.has_translation_axis(pivot_idx, Axis::Z)
    }

    pub fn has_translation(&self, pivot_idx: usize) -> bool {
        self.has_translation_x(pivot_idx)
            || self.has_translation_y(pivot_idx)
            || self.has_translation_z(pivot_idx)
    }

    /// Determine whether the specified translation axis is present.
    pub fn has_translation_axis(&self, pivot_idx: usize, axis: Axis) -> bool {
        self.pivot_motions
            .get(pivot_idx)
            .and_then(|motion| motion.translation[axis_index(axis)])
            .is_some()
    }

    /// Determine whether a quaternion rotation channel exists for the pivot.
    pub fn has_quaternion_rotation(&self, pivot_idx: usize) -> bool {
        self.pivot_motions
            .get(pivot_idx)
            .and_then(|motion| motion.rotation)
            .is_some()
    }

    /// Determine whether any rotation information exists for the pivot.
    pub fn has_rotation(&self, pivot_idx: usize) -> bool {
        self.has_quaternion_rotation(pivot_idx)
            || self
                .pivot_motions
                .get(pivot_idx)
                .map(|motion| motion.rotation_axes.iter().any(|entry| entry.is_some()))
                .unwrap_or(false)
    }

    /// Determine whether a visibility channel exists.
    pub fn has_visibility(&self, pivot_idx: usize) -> bool {
        self.pivot_motions
            .get(pivot_idx)
            .and_then(|motion| motion.visibility)
            .is_some()
    }

    /// Retrieve a local transform matrix for the pivot at `frame`.
    pub fn get_transform(&self, pivot_idx: usize, frame: f32) -> Mat4 {
        let translation = self.get_translation(pivot_idx, frame);
        let rotation = self.get_orientation(pivot_idx, frame);
        Mat4::from_rotation_translation(rotation, translation)
    }

    fn resolve_frames(&self, frame: f32) -> (usize, usize, f32) {
        let frame_count = self.num_frames.max(1) as usize;
        if frame_count == 1 {
            return (0, 0, 0.0);
        }

        let mut wrapped = frame % frame_count as f32;
        if wrapped < 0.0 {
            wrapped += frame_count as f32;
        }

        let frame0 = wrapped.floor() as usize;
        let mut ratio = wrapped - frame0 as f32;
        if ratio < 0.0 {
            ratio += 1.0;
        }
        let frame1 = if frame0 + 1 >= frame_count {
            0
        } else {
            frame0 + 1
        };

        (frame0, frame1, ratio.clamp(0.0, 1.0))
    }

    /// Get translation for a pivot at a specific frame.
    pub fn get_translation(&self, pivot_idx: usize, frame: f32) -> Vec3 {
        if pivot_idx >= self.pivot_motions.len() {
            return Vec3::ZERO;
        }

        let pivot_motion = &self.pivot_motions[pivot_idx];
        let (frame0, frame1, ratio) = self.resolve_frames(frame);

        let mut t0 = Vec3::ZERO;
        if let Some(idx) = pivot_motion.translation[0] {
            t0.x = self.channels[idx].sample_scalar(frame0);
        }
        if let Some(idx) = pivot_motion.translation[1] {
            t0.y = self.channels[idx].sample_scalar(frame0);
        }
        if let Some(idx) = pivot_motion.translation[2] {
            t0.z = self.channels[idx].sample_scalar(frame0);
        }

        if ratio <= FRAME_EPS {
            return t0;
        }

        let mut t1 = Vec3::ZERO;
        if let Some(idx) = pivot_motion.translation[0] {
            t1.x = self.channels[idx].sample_scalar(frame1);
        }
        if let Some(idx) = pivot_motion.translation[1] {
            t1.y = self.channels[idx].sample_scalar(frame1);
        }
        if let Some(idx) = pivot_motion.translation[2] {
            t1.z = self.channels[idx].sample_scalar(frame1);
        }

        t0.lerp(t1, ratio)
    }

    /// Get orientation for a pivot at a specific frame.
    pub fn get_orientation(&self, pivot_idx: usize, frame: f32) -> Quat {
        if pivot_idx >= self.pivot_motions.len() {
            return Quat::IDENTITY;
        }

        let pivot_motion = &self.pivot_motions[pivot_idx];
        let (frame0, frame1, ratio) = self.resolve_frames(frame);

        if let Some(channel_index) = pivot_motion.rotation {
            let q0 = self.channels[channel_index].sample_quaternion(frame0);
            if ratio <= FRAME_EPS {
                return q0;
            }

            let q1 = self.channels[channel_index].sample_quaternion(frame1);
            return q0.slerp(q1, ratio);
        }

        self.sample_axis_rotation(pivot_motion, frame0, frame1, ratio)
    }

    fn sample_axis_rotation(
        &self,
        motion: &PivotMotion,
        frame0: usize,
        frame1: usize,
        ratio: f32,
    ) -> Quat {
        let mut euler0 = Vec3::ZERO;
        let mut euler1 = Vec3::ZERO;

        if let Some(idx) = motion.rotation_axes[0] {
            euler0.x = self.channels[idx].sample_scalar(frame0);
            euler1.x = self.channels[idx].sample_scalar(frame1);
        }
        if let Some(idx) = motion.rotation_axes[1] {
            euler0.y = self.channels[idx].sample_scalar(frame0);
            euler1.y = self.channels[idx].sample_scalar(frame1);
        }
        if let Some(idx) = motion.rotation_axes[2] {
            euler0.z = self.channels[idx].sample_scalar(frame0);
            euler1.z = self.channels[idx].sample_scalar(frame1);
        }

        if ratio <= FRAME_EPS {
            return euler_to_quat(euler0);
        }

        let blended = euler0.lerp(euler1, ratio);
        euler_to_quat(blended)
    }

    /// Get visibility flag for a pivot at the given frame.
    pub fn get_visibility(&self, pivot_idx: usize, frame: f32) -> bool {
        if pivot_idx >= self.pivot_motions.len() {
            return true;
        }

        let motion = &self.pivot_motions[pivot_idx];
        let Some(binding) = motion.visibility else {
            return true;
        };

        let (frame0, _, _) = self.resolve_frames(frame);
        match binding {
            VisibilityBinding::Scalar(index) => self
                .channels
                .get(index)
                .map(|channel| channel.sample_scalar(frame0) > 0.5)
                .unwrap_or(true),
            VisibilityBinding::Bit(index) => self
                .bit_channels
                .get(index)
                .map(|ch| ch.get_bit(frame0) != 0)
                .unwrap_or(true),
        }
    }

    /// Set animation playback mode
    pub fn set_animation_mode(&mut self, mode: AnimationMode) {
        self.mode = mode;
        self.is_complete = false;
    }

    /// Get current animation mode
    pub fn get_animation_mode(&self) -> AnimationMode {
        self.mode
    }

    /// Set animation playback speed multiplier
    /// 1.0 = normal speed, 2.0 = double speed, 0.5 = half speed
    pub fn set_animation_speed(&mut self, speed: f32) {
        self.speed_multiplier = speed.max(0.0);
    }

    /// Get current animation speed multiplier
    pub fn get_animation_speed(&self) -> f32 {
        self.speed_multiplier
    }

    /// Set current frame manually (useful for Manual mode)
    pub fn set_frame(&mut self, frame: f32) {
        self.last_frame = self.current_frame;
        self.current_frame = frame.clamp(0.0, (self.num_frames.saturating_sub(1)) as f32);
    }

    /// Get current frame
    pub fn get_current_frame(&self) -> f32 {
        self.current_frame
    }

    /// Check if animation has completed (only true for Once/OnceBackwards modes)
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    /// Reset animation to beginning
    pub fn reset(&mut self) {
        self.current_frame = 0.0;
        self.last_frame = 0.0;
        self.is_complete = false;
    }

    /// Update animation frame based on delta time and current mode
    pub fn update(&mut self, delta_time: f32) {
        if self.mode == AnimationMode::Manual {
            return; // Don't auto-update in manual mode
        }

        if self.is_complete {
            return; // Don't update if animation is complete
        }

        // Handle edge case: zero-frame animation should not update
        if self.num_frames == 0 {
            self.current_frame = 0.0;
            return;
        }

        self.last_frame = self.current_frame;
        let effective_rate = self.frame_rate * self.speed_multiplier;
        let frame_delta = delta_time * effective_rate;

        match self.mode {
            AnimationMode::Loop => {
                self.current_frame += frame_delta;
                if self.num_frames > 0 {
                    let max_frame = self.num_frames as f32;
                    if self.current_frame >= max_frame {
                        self.current_frame = self.current_frame % max_frame;
                    }
                }
            }
            AnimationMode::Once => {
                self.current_frame += frame_delta;
                let last_frame = (self.num_frames.saturating_sub(1)) as f32;
                if self.current_frame >= last_frame {
                    self.current_frame = last_frame;
                    self.is_complete = true;
                }
            }
            AnimationMode::PingPong => {
                self.current_frame += frame_delta;
                if self.num_frames > 1 {
                    let cycle_length = ((self.num_frames - 1) as f32) * 2.0;
                    let mut wrapped = self.current_frame % cycle_length;
                    if wrapped < 0.0 {
                        wrapped += cycle_length;
                    }

                    let forward_end = (self.num_frames - 1) as f32;
                    if wrapped <= forward_end {
                        self.current_frame = wrapped; // Forward
                    } else {
                        self.current_frame = cycle_length - wrapped; // Backward
                    }
                }
            }
            AnimationMode::LoopBackwards => {
                self.current_frame -= frame_delta;
                if self.current_frame < 0.0 && self.num_frames > 0 {
                    let max_frame = self.num_frames as f32;
                    self.current_frame = max_frame + (self.current_frame % max_frame);
                }
            }
            AnimationMode::OnceBackwards => {
                self.current_frame -= frame_delta;
                if self.current_frame <= 0.0 {
                    self.current_frame = 0.0;
                    self.is_complete = true;
                }
            }
            AnimationMode::Manual => {
                // Already handled above
            }
        }
    }

    /// Add an animation event at a specific frame
    pub fn add_event(&mut self, event: AnimationEvent) {
        self.events.push(event);
        // Sort events by frame for efficient lookup
        self.events.sort_by(|a, b| {
            a.frame
                .partial_cmp(&b.frame)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Get events that occurred between last frame and current frame
    /// Handles wrapping for looping animations
    pub fn get_events_since_last_frame(&self) -> Vec<&AnimationEvent> {
        let mut triggered_events = Vec::new();

        let start = self.last_frame.min(self.current_frame);
        let end = self.last_frame.max(self.current_frame);

        // Check if we wrapped around (for looping animations)
        let wrapped = self.current_frame < self.last_frame;

        for event in &self.events {
            if wrapped {
                // Handle wrap-around case
                if event.frame >= self.last_frame || event.frame < self.current_frame {
                    triggered_events.push(event);
                }
            } else {
                // Normal case
                if event.frame >= start && event.frame < end {
                    triggered_events.push(event);
                }
            }
        }

        triggered_events
    }

    /// Apply animation to a hierarchy tree.
    pub fn apply_animation(&self, htree: &mut HTreeClass, frame: f32, root_transform: Mat4) {
        let pivot_count = htree.num_pivots();
        let mut translations = Vec::with_capacity(pivot_count);
        let mut rotations = Vec::with_capacity(pivot_count);
        let mut visibility = Vec::with_capacity(pivot_count);

        for pivot in 0..pivot_count {
            translations.push(self.get_translation(pivot, frame));
            rotations.push(self.get_orientation(pivot, frame));
            visibility.push(self.get_visibility(pivot, frame));
        }

        htree.anim_update(root_transform, &translations, &rotations);
        htree.update_visibility(&visibility);
    }
}

fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::Z => 2,
    }
}

fn euler_to_quat(euler: Vec3) -> Quat {
    let (sx, cx) = (euler.x * 0.5).sin_cos();
    let (sy, cy) = (euler.y * 0.5).sin_cos();
    let (sz, cz) = (euler.z * 0.5).sin_cos();

    Quat::from_xyzw(
        sx * cy * cz + cx * sy * sz,
        cx * sy * cz - sx * cy * sz,
        cx * cy * sz + sx * sy * cz,
        cx * cy * cz - sx * sy * sz,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_animation(num_frames: u32, frame_rate: f32) -> HAnimClass {
        HAnimClass::new("test_anim", "test_hierarchy", num_frames, frame_rate)
    }

    #[test]
    fn test_animation_mode_loop() {
        let mut anim = create_test_animation(10, 30.0);
        anim.set_animation_mode(AnimationMode::Loop);

        // Advance past the end
        anim.update(0.5); // 15 frames advanced
        assert!(
            (anim.get_current_frame() - 5.0).abs() < 0.01,
            "Frame should wrap around to 5.0, got {}",
            anim.get_current_frame()
        );
        assert!(!anim.is_complete(), "Loop mode should never complete");
    }

    #[test]
    fn test_animation_mode_once() {
        let mut anim = create_test_animation(10, 30.0);
        anim.set_animation_mode(AnimationMode::Once);

        // Advance to end
        anim.update(0.5); // 15 frames advanced
        assert_eq!(anim.get_current_frame(), 9.0, "Should clamp to last frame");
        assert!(anim.is_complete(), "Once mode should complete at end");

        // Further updates should not change frame
        let frame_before = anim.get_current_frame();
        anim.update(0.1);
        assert_eq!(
            anim.get_current_frame(),
            frame_before,
            "Completed animation should not advance"
        );
    }

    #[test]
    fn test_animation_mode_pingpong() {
        let mut anim = create_test_animation(10, 30.0);
        anim.set_animation_mode(AnimationMode::PingPong);

        // Advance forward
        anim.update(0.15); // 4.5 frames
        assert!(
            (anim.get_current_frame() - 4.5).abs() < 0.01,
            "Should advance forward, got {}",
            anim.get_current_frame()
        );

        // Advance past forward end (should bounce back)
        anim.reset();
        anim.update(0.4); // 12 frames - should be at frame 6 going backward (9 - 3)
        let frame = anim.get_current_frame();
        assert!(
            frame >= 0.0 && frame <= 9.0,
            "Frame should be within valid range during pingpong, got {}",
            frame
        );
    }

    #[test]
    fn test_animation_mode_loop_backwards() {
        let mut anim = create_test_animation(10, 30.0);
        anim.set_animation_mode(AnimationMode::LoopBackwards);
        anim.set_frame(5.0);

        // Advance backwards
        anim.update(0.1); // 3 frames backwards
        assert!(
            (anim.get_current_frame() - 2.0).abs() < 0.01,
            "Should move backwards, got {}",
            anim.get_current_frame()
        );

        // Wrap around
        anim.set_frame(1.0);
        anim.update(0.1); // 3 frames backwards, should wrap to 8
        assert!(
            anim.get_current_frame() > 5.0,
            "Should wrap around to end, got {}",
            anim.get_current_frame()
        );
    }

    #[test]
    fn test_animation_mode_once_backwards() {
        let mut anim = create_test_animation(10, 30.0);
        anim.set_animation_mode(AnimationMode::OnceBackwards);
        anim.set_frame(5.0);

        // Advance backwards to beginning
        anim.update(0.2); // 6 frames backwards
        assert_eq!(anim.get_current_frame(), 0.0, "Should clamp to first frame");
        assert!(anim.is_complete(), "OnceBackwards should complete at start");
    }

    #[test]
    fn test_animation_mode_manual() {
        let mut anim = create_test_animation(10, 30.0);
        anim.set_animation_mode(AnimationMode::Manual);

        // Manual update should not change frame
        anim.update(0.5);
        assert_eq!(
            anim.get_current_frame(),
            0.0,
            "Manual mode should not auto-update"
        );

        // Set frame manually
        anim.set_frame(5.0);
        assert_eq!(
            anim.get_current_frame(),
            5.0,
            "Should allow manual frame setting"
        );
    }

    #[test]
    fn test_animation_speed_control() {
        let mut anim = create_test_animation(30, 30.0);
        anim.set_animation_mode(AnimationMode::Loop);

        // Normal speed
        anim.set_animation_speed(1.0);
        anim.update(1.0); // Should advance 30 frames (wrap to 0)
        assert!(
            (anim.get_current_frame() - 0.0).abs() < 0.01,
            "Normal speed for 1 second should complete cycle"
        );

        // Double speed
        anim.reset();
        anim.set_animation_speed(2.0);
        anim.update(0.5); // Should advance 30 frames at 2x speed
        assert!(
            (anim.get_current_frame() - 0.0).abs() < 0.01,
            "Double speed should advance twice as fast"
        );

        // Half speed
        anim.reset();
        anim.set_animation_speed(0.5);
        anim.update(1.0); // Should advance 15 frames at 0.5x speed
        assert!(
            (anim.get_current_frame() - 15.0).abs() < 0.01,
            "Half speed should advance half as fast"
        );
    }

    #[test]
    fn test_animation_events() {
        let mut anim = create_test_animation(30, 30.0);

        // Add some events
        anim.add_event(AnimationEvent::new(5.0, "footstep", "left"));
        anim.add_event(AnimationEvent::new(15.0, "footstep", "right"));
        anim.add_event(AnimationEvent::new(25.0, "jump", ""));

        // Advance to frame 10
        anim.set_frame(10.0);
        let events = anim.get_events_since_last_frame();
        assert_eq!(events.len(), 1, "Should trigger one event between 0 and 10");
        assert_eq!(events[0].frame, 5.0);

        // Advance to frame 20
        anim.set_frame(20.0);
        let events = anim.get_events_since_last_frame();
        assert_eq!(
            events.len(),
            1,
            "Should trigger one event between 10 and 20"
        );
        assert_eq!(events[0].frame, 15.0);
    }

    #[test]
    fn test_animation_events_with_wrapping() {
        let mut anim = create_test_animation(30, 30.0);
        anim.set_animation_mode(AnimationMode::Loop);

        // Add events
        anim.add_event(AnimationEvent::new(5.0, "early", ""));
        anim.add_event(AnimationEvent::new(25.0, "late", ""));

        // Set frame near end
        anim.set_frame(28.0);
        anim.update(0.3); // Advance 9 frames, wrapping to ~7.0

        let events = anim.get_events_since_last_frame();
        // Should trigger the late event (25) and early event (5) due to wrap
        assert!(
            events.len() >= 1,
            "Should trigger events across wrap boundary"
        );
    }

    #[test]
    fn test_animation_reset() {
        let mut anim = create_test_animation(10, 30.0);
        anim.set_animation_mode(AnimationMode::Once);

        // Advance and complete
        anim.update(0.5);
        assert!(anim.is_complete());

        // Reset
        anim.reset();
        assert_eq!(
            anim.get_current_frame(),
            0.0,
            "Reset should return to frame 0"
        );
        assert!(!anim.is_complete(), "Reset should clear completion flag");
    }

    #[test]
    fn test_zero_frame_animation() {
        let mut anim = create_test_animation(0, 30.0);
        anim.set_animation_mode(AnimationMode::Loop);

        // Should not crash
        anim.update(0.1);
        assert_eq!(anim.get_current_frame(), 0.0);
    }

    #[test]
    fn test_single_frame_animation() {
        let mut anim = create_test_animation(1, 30.0);
        anim.set_animation_mode(AnimationMode::Loop);

        // Should stay at frame 0
        anim.update(0.1);
        assert_eq!(anim.get_current_frame(), 0.0);
    }

    #[test]
    fn test_negative_speed_clamping() {
        let mut anim = create_test_animation(10, 30.0);

        // Negative speeds should be clamped to 0
        anim.set_animation_speed(-1.0);
        assert_eq!(
            anim.get_animation_speed(),
            0.0,
            "Negative speed should be clamped to 0"
        );
    }

    #[test]
    fn test_animation_mode_switching() {
        let mut anim = create_test_animation(10, 30.0);

        // Start in loop mode
        anim.set_animation_mode(AnimationMode::Loop);
        assert_eq!(anim.get_animation_mode(), AnimationMode::Loop);

        // Switch to once mode
        anim.set_animation_mode(AnimationMode::Once);
        assert_eq!(anim.get_animation_mode(), AnimationMode::Once);
        assert!(
            !anim.is_complete(),
            "Changing mode should reset completion flag"
        );
    }
}
