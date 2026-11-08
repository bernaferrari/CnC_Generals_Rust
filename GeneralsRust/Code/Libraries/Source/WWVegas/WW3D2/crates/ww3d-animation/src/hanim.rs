//! HAnim (Hierarchy Animation) base class implementation
//!
//! Playback wrapper over the canonical [`HRawAnimClass`] channel owner; all
//! sampling is delegated to it.

use super::htree::HTreeClass;
use crate::hraw_anim::HRawAnimClass;
use glam::{Mat4, Quat, Vec3};

/// Animation playback modes matching C++ RenderObjClass::AnimMode
/// Reference: rendobj.h:331-339
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationMode {
    /// ANIM_MODE_MANUAL - Application controls frame manually
    Manual,
    /// ANIM_MODE_LOOP - Loop continuously
    #[default]
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

/// Playable classic animation: the canonical [`HRawAnimClass`] channels plus
/// the caller-owned playback cursor of this port's `HAnimClass` users.
#[derive(Debug, Clone)]
pub struct HAnimClass {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub total_time: f32,
    raw: HRawAnimClass,
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
        let mut anim = Self::from_raw(HRawAnimClass::new(name, hierarchy_name, num_frames, 0));
        anim.frame_rate = frame_rate;
        anim.total_time = total_time(num_frames, frame_rate);
        anim
    }

    /// Play a loaded canonical raw animation.
    pub fn from_raw(raw: HRawAnimClass) -> Self {
        // `HAnimClass::Get_Frame_Rate` returns the authored integer as float.
        #[allow(clippy::cast_precision_loss)]
        let frame_rate = raw.get_frame_rate() as f32;
        Self {
            name: raw.get_name().to_string(),
            hierarchy_name: raw.get_hierarchy_name().to_string(),
            num_frames: raw.get_num_frames(),
            frame_rate,
            total_time: total_time(raw.get_num_frames(), frame_rate),
            raw,
            embedded_sound_bone_index: None,
            mode: AnimationMode::Loop,
            current_frame: 0.0,
            speed_multiplier: 1.0,
            is_complete: false,
            events: Vec::new(),
            last_frame: 0.0,
        }
    }

    /// The canonical channels.
    pub fn raw(&self) -> &HRawAnimClass {
        &self.raw
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
        self.raw.get_num_pivots()
    }

    pub fn has_translation_x(&self, pivot_idx: usize) -> bool {
        self.raw.has_x_translation(pivot_idx)
    }

    pub fn has_translation_y(&self, pivot_idx: usize) -> bool {
        self.raw.has_y_translation(pivot_idx)
    }

    pub fn has_translation_z(&self, pivot_idx: usize) -> bool {
        self.raw.has_z_translation(pivot_idx)
    }

    pub fn has_translation(&self, pivot_idx: usize) -> bool {
        self.has_translation_x(pivot_idx)
            || self.has_translation_y(pivot_idx)
            || self.has_translation_z(pivot_idx)
    }

    /// Whether the pivot has a `Q` channel (the only orientation source).
    pub fn has_rotation(&self, pivot_idx: usize) -> bool {
        self.raw.has_rotation(pivot_idx)
    }

    pub fn has_visibility(&self, pivot_idx: usize) -> bool {
        self.raw.has_visibility(pivot_idx)
    }

    /// Local `Build_Matrix3D(q)` with the translation column set.
    pub fn get_transform(&self, pivot_idx: usize, frame: f32) -> Mat4 {
        Mat4::from_rotation_translation(
            self.get_orientation(pivot_idx, frame),
            self.get_translation(pivot_idx, frame),
        )
    }

    /// `HRawAnimClass::Get_Translation`.
    pub fn get_translation(&self, pivot_idx: usize, frame: f32) -> Vec3 {
        self.raw.get_translation(pivot_idx, frame)
    }

    /// `HRawAnimClass::Get_Orientation`.
    pub fn get_orientation(&self, pivot_idx: usize, frame: f32) -> Quat {
        self.raw.get_orientation(pivot_idx, frame)
    }

    /// `HRawAnimClass::Get_Visibility`.
    pub fn get_visibility(&self, pivot_idx: usize, frame: f32) -> bool {
        self.raw.get_visibility(pivot_idx, frame)
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
                        self.current_frame %= max_frame;
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
        htree.anim_update_sampled(root_transform, |index| {
            (index < self.num_pivots()).then(|| {
                (
                    self.get_translation(index, frame),
                    self.get_orientation(index, frame),
                    self.get_visibility(index, frame),
                )
            })
        });
    }
}

fn total_time(num_frames: u32, frame_rate: f32) -> f32 {
    if frame_rate > 0.0 {
        // Display-only duration; frame counts are bounded by the file format.
        #[allow(clippy::cast_precision_loss)]
        let frames = num_frames as f32;
        frames / frame_rate
    } else {
        0.0
    }
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
            (0.0..=9.0).contains(&frame),
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
            !events.is_empty(),
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
