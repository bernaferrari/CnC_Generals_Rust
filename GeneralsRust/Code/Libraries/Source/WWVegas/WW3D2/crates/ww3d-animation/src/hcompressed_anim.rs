//! HCompressedAnimClass - Compressed Animation Container
//!
//! Direct port of C++ HCompressedAnimClass from hcanim.cpp
//! This class manages compressed animation data using either TimeCodedMotionChannelClass
//! or AdaptiveDeltaMotionChannelClass for efficient storage and playback.
//!
//! Reference: hcanim.h:73-135, hcanim.cpp:161-710

use crate::hanim::{AnimationEvent, AnimationMode};
use crate::motion_channels::{
    AdaptiveDeltaMotionChannelClass, TimeCodedBitChannelClass, TimeCodedMotionChannelClass,
};
use glam::{Mat4, Quat, Vec3};

/// Animation flavor constants from w3d_file.h:1437-1440
pub const ANIM_FLAVOR_TIMECODED: u32 = 0;
pub const ANIM_FLAVOR_ADAPTIVE_DELTA: u32 = 1;

/// Node motion data structure for compressed animations
/// Reference: hcanim.cpp:63-98
#[derive(Debug, Clone)]
enum NodeCompressedMotion {
    /// Time-coded motion channels
    TimeCoded {
        x: Option<TimeCodedMotionChannelClass>,
        y: Option<TimeCodedMotionChannelClass>,
        z: Option<TimeCodedMotionChannelClass>,
        q: Option<TimeCodedMotionChannelClass>,
        vis: Option<TimeCodedBitChannelClass>,
    },
    /// Adaptive delta motion channels
    AdaptiveDelta {
        x: Option<AdaptiveDeltaMotionChannelClass>,
        y: Option<AdaptiveDeltaMotionChannelClass>,
        z: Option<AdaptiveDeltaMotionChannelClass>,
        q: Option<AdaptiveDeltaMotionChannelClass>,
        vis: Option<TimeCodedBitChannelClass>,
    },
}

impl NodeCompressedMotion {
    fn new_timecoded() -> Self {
        Self::TimeCoded {
            x: None,
            y: None,
            z: None,
            q: None,
            vis: None,
        }
    }

    fn new_adaptive_delta() -> Self {
        Self::AdaptiveDelta {
            x: None,
            y: None,
            z: None,
            q: None,
            vis: None,
        }
    }

    /// Check if this node has any motion data
    /// Reference: hcanim.cpp:666-677
    fn has_motion(&self) -> bool {
        match self {
            NodeCompressedMotion::TimeCoded { x, y, z, q, vis } => {
                x.is_some() || y.is_some() || z.is_some() || q.is_some() || vis.is_some()
            }
            NodeCompressedMotion::AdaptiveDelta { x, y, z, q, vis } => {
                x.is_some() || y.is_some() || z.is_some() || q.is_some() || vis.is_some()
            }
        }
    }
}

/// Compressed Hierarchical Animation
/// Reference: hcanim.h:73-135
#[derive(Debug, Clone)]
pub struct HCompressedAnimClass {
    name: String,
    hierarchy_name: String,
    num_frames: u32,
    num_nodes: usize,
    flavor: u32,
    frame_rate: f32,
    node_motion: Vec<NodeCompressedMotion>,
    embedded_sound_bone_index: Option<usize>,
    // Playback state
    mode: AnimationMode,
    current_frame: f32,
    speed_multiplier: f32,
    is_complete: bool,
    events: Vec<AnimationEvent>,
    last_frame: f32,
}

impl HCompressedAnimClass {
    /// Create a new compressed animation
    /// Reference: hcanim.cpp:173-182
    pub fn new(
        name: String,
        hierarchy_name: String,
        num_frames: u32,
        num_nodes: usize,
        flavor: u32,
        frame_rate: f32,
    ) -> Self {
        // Initialize node motion based on flavor (hcanim.cpp:289-291)
        let node_motion = match flavor {
            ANIM_FLAVOR_TIMECODED => (0..num_nodes)
                .map(|_| NodeCompressedMotion::new_timecoded())
                .collect(),
            ANIM_FLAVOR_ADAPTIVE_DELTA => (0..num_nodes)
                .map(|_| NodeCompressedMotion::new_adaptive_delta())
                .collect(),
            _ => panic!("Unsupported animation flavor: {}", flavor),
        };

        Self {
            name,
            hierarchy_name,
            num_frames,
            num_nodes,
            flavor,
            frame_rate,
            node_motion,
            embedded_sound_bone_index: None,
            mode: AnimationMode::Loop,
            current_frame: 0.0,
            speed_multiplier: 1.0,
            is_complete: false,
            events: Vec::new(),
            last_frame: 0.0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_hname(&self) -> &str {
        &self.hierarchy_name
    }

    pub fn get_num_frames(&self) -> u32 {
        self.num_frames
    }

    pub fn get_frame_rate(&self) -> f32 {
        self.frame_rate
    }

    pub fn get_total_time(&self) -> f32 {
        if self.frame_rate > 0.0 {
            self.num_frames as f32 / self.frame_rate
        } else {
            0.0
        }
    }

    pub fn get_flavor(&self) -> u32 {
        self.flavor
    }

    pub fn get_num_pivots(&self) -> usize {
        self.num_nodes
    }

    /// Check if a node has motion data
    /// Reference: hcanim.cpp:666-677
    pub fn is_node_motion_present(&self, pividx: usize) -> bool {
        if pividx >= self.num_nodes {
            return false;
        }
        self.node_motion[pividx].has_motion()
    }

    /// Add a timecoded motion channel to the animation
    /// Reference: hcanim.cpp:419-442
    pub fn add_timecoded_channel(&mut self, channel: TimeCodedMotionChannelClass) {
        let pivot_idx = channel.get_pivot() as usize;
        if pivot_idx >= self.num_nodes {
            eprintln!(
                "ERROR! animation {} indexes a bone not present in the model",
                self.name
            );
            return;
        }

        if let NodeCompressedMotion::TimeCoded { x, y, z, q, .. } = &mut self.node_motion[pivot_idx]
        {
            match channel.get_type() {
                0 => *x = Some(channel), // ANIM_CHANNEL_X
                1 => *y = Some(channel), // ANIM_CHANNEL_Y
                2 => *z = Some(channel), // ANIM_CHANNEL_Z
                6 => *q = Some(channel), // ANIM_CHANNEL_Q
                _ => {}
            }
        }
    }

    /// Add an adaptive delta motion channel to the animation
    /// Reference: hcanim.cpp:444-467
    pub fn add_adaptive_delta_channel(&mut self, channel: AdaptiveDeltaMotionChannelClass) {
        let pivot_idx = channel.get_pivot() as usize;
        if pivot_idx >= self.num_nodes {
            eprintln!(
                "ERROR! animation {} indexes a bone not present in the model",
                self.name
            );
            return;
        }

        if let NodeCompressedMotion::AdaptiveDelta { x, y, z, q, .. } =
            &mut self.node_motion[pivot_idx]
        {
            match channel.get_type() {
                0 => *x = Some(channel),  // ANIM_CHANNEL_X
                1 => *y = Some(channel),  // ANIM_CHANNEL_Y
                2 => *z = Some(channel),  // ANIM_CHANNEL_Z
                6 => *q = Some(channel),  // ANIM_CHANNEL_Q (14 for adaptive delta variant)
                14 => *q = Some(channel), // ANIM_CHANNEL_Q adaptive delta
                _ => {}
            }
        }
    }

    /// Add a bit channel (visibility) to the animation
    /// Reference: hcanim.cpp:506-516
    pub fn add_bit_channel(&mut self, channel: TimeCodedBitChannelClass) {
        let pivot_idx = channel.get_pivot() as usize;
        if pivot_idx >= self.num_nodes {
            eprintln!(
                "ERROR! animation {} indexes a bone not present in the model",
                self.name
            );
            return;
        }

        match &mut self.node_motion[pivot_idx] {
            NodeCompressedMotion::TimeCoded { vis, .. } => {
                *vis = Some(channel);
            }
            NodeCompressedMotion::AdaptiveDelta { vis, .. } => {
                *vis = Some(channel);
            }
        }
    }

    /// Get translation for a pivot at a specific frame
    /// Reference: hcanim.cpp:530-551
    pub fn get_translation(&mut self, pividx: usize, frame: f32) -> Vec3 {
        if pividx >= self.num_nodes {
            return Vec3::ZERO;
        }

        let mut trans = Vec3::ZERO;

        match &mut self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { x, y, z, .. } => {
                let mut buffer = [0.0f32; 1];
                if let Some(ch) = x {
                    ch.get_vector(frame, &mut buffer);
                    trans.x = buffer[0];
                }
                if let Some(ch) = y {
                    ch.get_vector(frame, &mut buffer);
                    trans.y = buffer[0];
                }
                if let Some(ch) = z {
                    ch.get_vector(frame, &mut buffer);
                    trans.z = buffer[0];
                }
            }
            NodeCompressedMotion::AdaptiveDelta { x, y, z, .. } => {
                let mut buffer = [0.0f32; 1];
                if let Some(ch) = x {
                    ch.get_vector(frame, &mut buffer);
                    trans.x = buffer[0];
                }
                if let Some(ch) = y {
                    ch.get_vector(frame, &mut buffer);
                    trans.y = buffer[0];
                }
                if let Some(ch) = z {
                    ch.get_vector(frame, &mut buffer);
                    trans.z = buffer[0];
                }
            }
        }

        trans
    }

    /// Get orientation (quaternion) for a pivot at a specific frame
    /// Reference: hcanim.cpp:565-580
    pub fn get_orientation(&mut self, pividx: usize, frame: f32) -> Quat {
        if pividx >= self.num_nodes {
            return Quat::IDENTITY;
        }

        match &mut self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { q, .. } => {
                if let Some(ch) = q {
                    ch.get_quat_vector(frame)
                } else {
                    Quat::IDENTITY
                }
            }
            NodeCompressedMotion::AdaptiveDelta { q, .. } => {
                if let Some(ch) = q {
                    ch.get_quat_vector(frame)
                } else {
                    Quat::IDENTITY
                }
            }
        }
    }

    /// Get transform matrix for a pivot at a specific frame
    /// Reference: hcanim.cpp:594-626
    pub fn get_transform(&mut self, pividx: usize, frame: f32) -> Mat4 {
        if pividx >= self.num_nodes {
            return Mat4::IDENTITY;
        }

        let rotation = self.get_orientation(pividx, frame);
        let mut mtx = Mat4::from_quat(rotation);

        // Set translation components
        match &mut self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { x, y, z, .. } => {
                let mut buffer = [0.0f32; 1];
                if let Some(ch) = x {
                    ch.get_vector(frame, &mut buffer);
                    mtx.w_axis.x = buffer[0];
                }
                if let Some(ch) = y {
                    ch.get_vector(frame, &mut buffer);
                    mtx.w_axis.y = buffer[0];
                }
                if let Some(ch) = z {
                    ch.get_vector(frame, &mut buffer);
                    mtx.w_axis.z = buffer[0];
                }
            }
            NodeCompressedMotion::AdaptiveDelta { x, y, z, .. } => {
                let mut buffer = [0.0f32; 1];
                if let Some(ch) = x {
                    ch.get_vector(frame, &mut buffer);
                    mtx.w_axis.x = buffer[0];
                }
                if let Some(ch) = y {
                    ch.get_vector(frame, &mut buffer);
                    mtx.w_axis.y = buffer[0];
                }
                if let Some(ch) = z {
                    ch.get_vector(frame, &mut buffer);
                    mtx.w_axis.z = buffer[0];
                }
            }
        }

        mtx
    }

    /// Get visibility for a pivot at a specific frame
    /// Reference: hcanim.cpp:640-650
    pub fn get_visibility(&mut self, pividx: usize, frame: f32) -> bool {
        if pividx >= self.num_nodes {
            return true; // Default to visible
        }

        let frame_int = frame as i32;

        match &mut self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { vis, .. }
            | NodeCompressedMotion::AdaptiveDelta { vis, .. } => {
                if let Some(ch) = vis {
                    ch.get_bit(frame_int) == 1
                } else {
                    true // Default to visible
                }
            }
        }
    }

    /// Check if a pivot has X translation channel
    /// Reference: hcanim.cpp:679-683
    pub fn has_x_translation(&self, pividx: usize) -> bool {
        if pividx >= self.num_nodes {
            return false;
        }
        match &self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { x, .. } => x.is_some(),
            NodeCompressedMotion::AdaptiveDelta { x, .. } => x.is_some(),
        }
    }

    /// Check if a pivot has Y translation channel
    /// Reference: hcanim.cpp:685-689
    pub fn has_y_translation(&self, pividx: usize) -> bool {
        if pividx >= self.num_nodes {
            return false;
        }
        match &self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { y, .. } => y.is_some(),
            NodeCompressedMotion::AdaptiveDelta { y, .. } => y.is_some(),
        }
    }

    /// Check if a pivot has Z translation channel
    /// Reference: hcanim.cpp:691-695
    pub fn has_z_translation(&self, pividx: usize) -> bool {
        if pividx >= self.num_nodes {
            return false;
        }
        match &self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { z, .. } => z.is_some(),
            NodeCompressedMotion::AdaptiveDelta { z, .. } => z.is_some(),
        }
    }

    /// Check if a pivot has rotation channel
    /// Reference: hcanim.cpp:697-701
    pub fn has_rotation(&self, pividx: usize) -> bool {
        if pividx >= self.num_nodes {
            return false;
        }
        match &self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { q, .. } => q.is_some(),
            NodeCompressedMotion::AdaptiveDelta { q, .. } => q.is_some(),
        }
    }

    /// Check if a pivot has visibility channel
    /// Reference: hcanim.cpp:703-707
    pub fn has_visibility(&self, pividx: usize) -> bool {
        if pividx >= self.num_nodes {
            return false;
        }
        match &self.node_motion[pividx] {
            NodeCompressedMotion::TimeCoded { vis, .. } => vis.is_some(),
            NodeCompressedMotion::AdaptiveDelta { vis, .. } => vis.is_some(),
        }
    }

    pub fn set_embedded_sound_bone_index(&mut self, index: Option<usize>) {
        self.embedded_sound_bone_index = index;
    }

    pub fn get_embedded_sound_bone_index(&self) -> Option<usize> {
        self.embedded_sound_bone_index
    }

    pub fn has_embedded_sounds(&self) -> bool {
        self.embedded_sound_bone_index.is_some()
    }

    /// Set animation playback mode
    pub fn set_mode(&mut self, mode: AnimationMode) {
        self.mode = mode;
        self.is_complete = false;
    }

    /// Get current animation mode
    pub fn get_mode(&self) -> AnimationMode {
        self.mode
    }

    /// Set animation playback speed multiplier
    pub fn set_speed(&mut self, speed: f32) {
        self.speed_multiplier = speed.max(0.0);
    }

    /// Get current animation speed
    pub fn get_speed(&self) -> f32 {
        self.speed_multiplier
    }

    /// Set current frame manually
    pub fn set_current_frame(&mut self, frame: f32) {
        self.last_frame = self.current_frame;
        self.current_frame = frame.clamp(0.0, (self.num_frames.saturating_sub(1)) as f32);
    }

    /// Get current playback frame
    pub fn get_current_frame(&self) -> f32 {
        self.current_frame
    }

    /// Check if animation is complete
    pub fn is_animation_complete(&self) -> bool {
        self.is_complete
    }

    /// Reset animation to beginning
    pub fn reset_animation(&mut self) {
        self.current_frame = 0.0;
        self.last_frame = 0.0;
        self.is_complete = false;
    }

    /// Update animation frame based on delta time and current mode
    pub fn update(&mut self, delta_time: f32) {
        if self.mode == AnimationMode::Manual || self.is_complete {
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
                        self.current_frame = wrapped;
                    } else {
                        self.current_frame = cycle_length - wrapped;
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
            AnimationMode::Manual => {}
        }
    }

    /// Add an animation event
    pub fn add_event(&mut self, event: AnimationEvent) {
        self.events.push(event);
        self.events.sort_by(|a, b| {
            a.frame
                .partial_cmp(&b.frame)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Get events triggered since last frame
    pub fn get_events_since_last_frame(&self) -> Vec<&AnimationEvent> {
        let mut triggered_events = Vec::new();
        let start = self.last_frame.min(self.current_frame);
        let end = self.last_frame.max(self.current_frame);
        let wrapped = self.current_frame < self.last_frame;

        for event in &self.events {
            if wrapped {
                if event.frame >= self.last_frame || event.frame < self.current_frame {
                    triggered_events.push(event);
                }
            } else {
                if event.frame >= start && event.frame < end {
                    triggered_events.push(event);
                }
            }
        }

        triggered_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressed_anim_creation() {
        let anim = HCompressedAnimClass::new(
            "TestAnim".to_string(),
            "TestSkeleton".to_string(),
            30,
            10,
            ANIM_FLAVOR_TIMECODED,
            30.0,
        );

        assert_eq!(anim.get_name(), "TestAnim");
        assert_eq!(anim.get_hname(), "TestSkeleton");
        assert_eq!(anim.get_num_frames(), 30);
        assert_eq!(anim.get_frame_rate(), 30.0);
        assert_eq!(anim.get_num_pivots(), 10);
        assert_eq!(anim.get_flavor(), ANIM_FLAVOR_TIMECODED);
    }

    #[test]
    fn test_adaptive_delta_anim_creation() {
        let anim = HCompressedAnimClass::new(
            "TestAnim".to_string(),
            "TestSkeleton".to_string(),
            60,
            5,
            ANIM_FLAVOR_ADAPTIVE_DELTA,
            30.0,
        );

        assert_eq!(anim.get_flavor(), ANIM_FLAVOR_ADAPTIVE_DELTA);
    }

    #[test]
    fn test_compressed_anim_mode_loop() {
        let mut anim = HCompressedAnimClass::new(
            "TestAnim".to_string(),
            "TestSkeleton".to_string(),
            10,
            5,
            ANIM_FLAVOR_TIMECODED,
            30.0,
        );
        anim.set_mode(AnimationMode::Loop);

        // Advance past the end
        anim.update(0.5); // 15 frames
        assert!((anim.get_current_frame() - 5.0).abs() < 0.01);
        assert!(!anim.is_animation_complete());
    }

    #[test]
    fn test_compressed_anim_mode_once() {
        let mut anim = HCompressedAnimClass::new(
            "TestAnim".to_string(),
            "TestSkeleton".to_string(),
            10,
            5,
            ANIM_FLAVOR_TIMECODED,
            30.0,
        );
        anim.set_mode(AnimationMode::Once);

        // Advance to end
        anim.update(0.5);
        assert_eq!(anim.get_current_frame(), 9.0);
        assert!(anim.is_animation_complete());
    }

    #[test]
    fn test_compressed_anim_speed_control() {
        let mut anim = HCompressedAnimClass::new(
            "TestAnim".to_string(),
            "TestSkeleton".to_string(),
            30,
            5,
            ANIM_FLAVOR_TIMECODED,
            30.0,
        );
        anim.set_mode(AnimationMode::Loop);

        // Double speed
        anim.set_speed(2.0);
        anim.update(0.5); // Should advance 30 frames at 2x speed
        assert!((anim.get_current_frame() - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_compressed_anim_events() {
        let mut anim = HCompressedAnimClass::new(
            "TestAnim".to_string(),
            "TestSkeleton".to_string(),
            30,
            5,
            ANIM_FLAVOR_TIMECODED,
            30.0,
        );

        anim.add_event(AnimationEvent::new(5.0, "test", "data"));
        anim.add_event(AnimationEvent::new(15.0, "test2", "data2"));

        anim.set_current_frame(10.0);
        let events = anim.get_events_since_last_frame();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].frame, 5.0);
    }
}
