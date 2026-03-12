/// Animation playback system
///
/// This module provides animation playback, keyframe interpolation, blending,
/// and skeleton pose calculation matching the C++ animation system (hanim.h, hanimmgr.h)
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;
use std::sync::Arc;

/// Animation channel type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelType {
    TranslationX,
    TranslationY,
    TranslationZ,
    Rotation,
    Visibility,
}

/// Animation keyframe for different data types
#[derive(Debug, Clone)]
pub enum Keyframe {
    Scalar { time: f32, value: f32 },
    Vector { time: f32, value: Vec3 },
    Quaternion { time: f32, value: Quat },
    Bool { time: f32, value: bool },
}

impl Keyframe {
    pub fn time(&self) -> f32 {
        match self {
            Keyframe::Scalar { time, .. } => *time,
            Keyframe::Vector { time, .. } => *time,
            Keyframe::Quaternion { time, .. } => *time,
            Keyframe::Bool { time, .. } => *time,
        }
    }
}

/// Animation channel containing keyframes for a single property
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    pub pivot_index: u32,
    pub channel_type: ChannelType,
    pub keyframes: Vec<Keyframe>,
}

impl AnimationChannel {
    pub fn new(pivot_index: u32, channel_type: ChannelType) -> Self {
        Self {
            pivot_index,
            channel_type,
            keyframes: Vec::new(),
        }
    }

    /// Add a keyframe to the channel
    pub fn add_keyframe(&mut self, keyframe: Keyframe) {
        self.keyframes.push(keyframe);
        // Keep keyframes sorted by time
        self.keyframes.sort_by(|a, b| {
            a.time()
                .partial_cmp(&b.time())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Evaluate channel at a specific time with interpolation
    pub fn evaluate_scalar(&self, time: f32) -> Option<f32> {
        if self.keyframes.is_empty() {
            return None;
        }

        // Find surrounding keyframes
        let mut before_idx = None;
        let mut after_idx = None;

        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.time() <= time {
                before_idx = Some(i);
            }
            if kf.time() >= time && after_idx.is_none() {
                after_idx = Some(i);
                break;
            }
        }

        match (before_idx, after_idx) {
            (Some(b), Some(a)) if b == a => {
                // Exact keyframe match
                if let Keyframe::Scalar { value, .. } = self.keyframes[b] {
                    Some(value)
                } else {
                    None
                }
            }
            (Some(b), Some(a)) => {
                // Interpolate between keyframes
                if let (
                    Keyframe::Scalar {
                        value: v1,
                        time: t1,
                    },
                    Keyframe::Scalar {
                        value: v2,
                        time: t2,
                    },
                ) = (&self.keyframes[b], &self.keyframes[a])
                {
                    let t = (time - t1) / (t2 - t1);
                    Some(v1 + (v2 - v1) * t)
                } else {
                    None
                }
            }
            (Some(b), None) => {
                // Use last keyframe
                if let Keyframe::Scalar { value, .. } = self.keyframes[b] {
                    Some(value)
                } else {
                    None
                }
            }
            (None, Some(a)) => {
                // Use first keyframe
                if let Keyframe::Scalar { value, .. } = self.keyframes[a] {
                    Some(value)
                } else {
                    None
                }
            }
            (None, None) => None,
        }
    }

    /// Evaluate vector channel
    pub fn evaluate_vector(&self, time: f32) -> Option<Vec3> {
        if self.keyframes.is_empty() {
            return None;
        }

        let mut before_idx = None;
        let mut after_idx = None;

        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.time() <= time {
                before_idx = Some(i);
            }
            if kf.time() >= time && after_idx.is_none() {
                after_idx = Some(i);
                break;
            }
        }

        match (before_idx, after_idx) {
            (Some(b), Some(a)) if b == a => {
                if let Keyframe::Vector { value, .. } = self.keyframes[b] {
                    Some(value)
                } else {
                    None
                }
            }
            (Some(b), Some(a)) => {
                if let (
                    Keyframe::Vector {
                        value: v1,
                        time: t1,
                    },
                    Keyframe::Vector {
                        value: v2,
                        time: t2,
                    },
                ) = (&self.keyframes[b], &self.keyframes[a])
                {
                    let t = (time - t1) / (t2 - t1);
                    Some(v1.lerp(*v2, t))
                } else {
                    None
                }
            }
            (Some(b), None) => {
                if let Keyframe::Vector { value, .. } = self.keyframes[b] {
                    Some(value)
                } else {
                    None
                }
            }
            (None, Some(a)) => {
                if let Keyframe::Vector { value, .. } = self.keyframes[a] {
                    Some(value)
                } else {
                    None
                }
            }
            (None, None) => None,
        }
    }

    /// Evaluate quaternion channel with SLERP
    pub fn evaluate_quaternion(&self, time: f32) -> Option<Quat> {
        if self.keyframes.is_empty() {
            return None;
        }

        let mut before_idx = None;
        let mut after_idx = None;

        for (i, kf) in self.keyframes.iter().enumerate() {
            if kf.time() <= time {
                before_idx = Some(i);
            }
            if kf.time() >= time && after_idx.is_none() {
                after_idx = Some(i);
                break;
            }
        }

        match (before_idx, after_idx) {
            (Some(b), Some(a)) if b == a => {
                if let Keyframe::Quaternion { value, .. } = self.keyframes[b] {
                    Some(value)
                } else {
                    None
                }
            }
            (Some(b), Some(a)) => {
                if let (
                    Keyframe::Quaternion {
                        value: q1,
                        time: t1,
                    },
                    Keyframe::Quaternion {
                        value: q2,
                        time: t2,
                    },
                ) = (&self.keyframes[b], &self.keyframes[a])
                {
                    let t = (time - t1) / (t2 - t1);
                    Some(q1.slerp(*q2, t))
                } else {
                    None
                }
            }
            (Some(b), None) => {
                if let Keyframe::Quaternion { value, .. } = self.keyframes[b] {
                    Some(value)
                } else {
                    None
                }
            }
            (None, Some(a)) => {
                if let Keyframe::Quaternion { value, .. } = self.keyframes[a] {
                    Some(value)
                } else {
                    None
                }
            }
            (None, None) => None,
        }
    }

    /// Evaluate boolean channel
    pub fn evaluate_bool(&self, time: f32) -> Option<bool> {
        if self.keyframes.is_empty() {
            return None;
        }

        // Find the most recent keyframe before or at the time
        for kf in self.keyframes.iter().rev() {
            if kf.time() <= time {
                if let Keyframe::Bool { value, .. } = kf {
                    return Some(*value);
                }
            }
        }

        // If no keyframe found before time, use first keyframe
        if let Some(Keyframe::Bool { value, .. }) = self.keyframes.first() {
            Some(*value)
        } else {
            None
        }
    }
}

/// Animation data representing a single animation
#[derive(Debug, Clone)]
pub struct Animation {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub channels: Vec<AnimationChannel>,
}

impl Animation {
    pub fn new(name: String, hierarchy_name: String, num_frames: u32, frame_rate: f32) -> Self {
        Self {
            name,
            hierarchy_name,
            num_frames,
            frame_rate,
            channels: Vec::new(),
        }
    }

    /// Get animation duration in seconds
    pub fn duration(&self) -> f32 {
        if self.frame_rate > 0.0 {
            self.num_frames as f32 / self.frame_rate
        } else {
            0.0
        }
    }

    /// Add an animation channel
    pub fn add_channel(&mut self, channel: AnimationChannel) {
        self.channels.push(channel);
    }

    /// Get channels for a specific pivot
    pub fn channels_for_pivot(&self, pivot_index: u32) -> Vec<&AnimationChannel> {
        self.channels
            .iter()
            .filter(|ch| ch.pivot_index == pivot_index)
            .collect()
    }

    /// Evaluate animation at a specific time to get bone transforms
    pub fn evaluate(&self, time: f32, bone_count: usize) -> Vec<BoneTransform> {
        let mut transforms = vec![BoneTransform::default(); bone_count];

        for i in 0..bone_count {
            let channels = self.channels_for_pivot(i as u32);

            let mut translation = Vec3::ZERO;
            let mut rotation = Quat::IDENTITY;
            let mut visibility = true;

            for channel in channels {
                match channel.channel_type {
                    ChannelType::TranslationX => {
                        if let Some(val) = channel.evaluate_scalar(time) {
                            translation.x = val;
                        }
                    }
                    ChannelType::TranslationY => {
                        if let Some(val) = channel.evaluate_scalar(time) {
                            translation.y = val;
                        }
                    }
                    ChannelType::TranslationZ => {
                        if let Some(val) = channel.evaluate_scalar(time) {
                            translation.z = val;
                        }
                    }
                    ChannelType::Rotation => {
                        if let Some(quat) = channel.evaluate_quaternion(time) {
                            rotation = quat;
                        }
                    }
                    ChannelType::Visibility => {
                        if let Some(vis) = channel.evaluate_bool(time) {
                            visibility = vis;
                        }
                    }
                }
            }

            transforms[i] = BoneTransform {
                translation,
                rotation,
                scale: Vec3::ONE,
                visibility,
            };
        }

        transforms
    }
}

/// Bone transform for a single frame
#[derive(Debug, Clone, Copy)]
pub struct BoneTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub visibility: bool,
}

impl BoneTransform {
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    /// Blend between two bone transforms
    pub fn blend(&self, other: &BoneTransform, weight: f32) -> BoneTransform {
        BoneTransform {
            translation: self.translation.lerp(other.translation, weight),
            rotation: self.rotation.slerp(other.rotation, weight),
            scale: self.scale.lerp(other.scale, weight),
            visibility: if weight < 0.5 {
                self.visibility
            } else {
                other.visibility
            },
        }
    }
}

impl Default for BoneTransform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            visibility: true,
        }
    }
}

/// Animation state for playback
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationState {
    Stopped,
    Playing,
    Paused,
}

/// Animation player for a single animation
#[derive(Debug)]
pub struct AnimationPlayer {
    pub animation: Arc<Animation>,
    pub current_time: f32,
    pub playback_speed: f32,
    pub state: AnimationState,
    pub looping: bool,
}

impl AnimationPlayer {
    pub fn new(animation: Arc<Animation>) -> Self {
        Self {
            animation,
            current_time: 0.0,
            playback_speed: 1.0,
            state: AnimationState::Stopped,
            looping: true,
        }
    }

    /// Play the animation
    pub fn play(&mut self) {
        self.state = AnimationState::Playing;
    }

    /// Pause the animation
    pub fn pause(&mut self) {
        self.state = AnimationState::Paused;
    }

    /// Stop the animation and reset to beginning
    pub fn stop(&mut self) {
        self.state = AnimationState::Stopped;
        self.current_time = 0.0;
    }

    /// Update animation time
    pub fn update(&mut self, delta_time: f32) {
        if self.state != AnimationState::Playing {
            return;
        }

        self.current_time += delta_time * self.playback_speed;

        let duration = self.animation.duration();
        if duration > 0.0 {
            if self.current_time >= duration {
                if self.looping {
                    self.current_time = self.current_time % duration;
                } else {
                    self.current_time = duration;
                    self.state = AnimationState::Stopped;
                }
            } else if self.current_time < 0.0 {
                if self.looping {
                    self.current_time = duration + (self.current_time % duration);
                } else {
                    self.current_time = 0.0;
                    self.state = AnimationState::Stopped;
                }
            }
        }
    }

    /// Evaluate current animation frame
    pub fn evaluate(&self, bone_count: usize) -> Vec<BoneTransform> {
        self.animation.evaluate(self.current_time, bone_count)
    }

    /// Get current frame number
    pub fn current_frame(&self) -> f32 {
        self.current_time * self.animation.frame_rate
    }

    /// Set current frame
    pub fn set_frame(&mut self, frame: f32) {
        self.current_time = frame / self.animation.frame_rate;
    }

    /// Check if animation is finished
    pub fn is_finished(&self) -> bool {
        !self.looping && self.current_time >= self.animation.duration()
    }
}

/// Animation blend entry
#[derive(Debug)]
pub struct AnimationBlendEntry {
    pub player: AnimationPlayer,
    pub weight: f32,
    pub fade_speed: f32, // Weight change per second
}

impl AnimationBlendEntry {
    pub fn new(animation: Arc<Animation>, weight: f32) -> Self {
        Self {
            player: AnimationPlayer::new(animation),
            weight,
            fade_speed: 0.0,
        }
    }

    /// Fade in
    pub fn fade_in(&mut self, duration: f32) {
        if duration > 0.0 {
            self.fade_speed = 1.0 / duration;
        } else {
            self.weight = 1.0;
        }
    }

    /// Fade out
    pub fn fade_out(&mut self, duration: f32) {
        if duration > 0.0 {
            self.fade_speed = -1.0 / duration;
        } else {
            self.weight = 0.0;
        }
    }

    /// Update blend weight
    pub fn update_weight(&mut self, delta_time: f32) {
        if self.fade_speed != 0.0 {
            self.weight = (self.weight + self.fade_speed * delta_time).clamp(0.0, 1.0);

            // Stop fading when reached target
            if self.weight <= 0.0 || self.weight >= 1.0 {
                self.fade_speed = 0.0;
            }
        }
    }
}

/// Animation blender for mixing multiple animations
#[derive(Debug)]
pub struct AnimationBlender {
    entries: Vec<AnimationBlendEntry>,
    bone_count: usize,
}

impl AnimationBlender {
    pub fn new(bone_count: usize) -> Self {
        Self {
            entries: Vec::new(),
            bone_count,
        }
    }

    /// Add animation with weight
    pub fn add_animation(&mut self, animation: Arc<Animation>, weight: f32) {
        let mut entry = AnimationBlendEntry::new(animation, weight);
        entry.player.play();
        self.entries.push(entry);
    }

    /// Remove animation by index
    pub fn remove_animation(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }

    /// Update all animations
    pub fn update(&mut self, delta_time: f32) {
        for entry in &mut self.entries {
            entry.player.update(delta_time);
            entry.update_weight(delta_time);
        }

        // Remove animations with zero weight
        self.entries.retain(|entry| entry.weight > 0.0);
    }

    /// Evaluate blended animation
    pub fn evaluate(&self) -> Vec<BoneTransform> {
        if self.entries.is_empty() {
            return vec![BoneTransform::default(); self.bone_count];
        }

        // Normalize weights
        let total_weight: f32 = self.entries.iter().map(|e| e.weight).sum();
        let normalized_weights: Vec<f32> = if total_weight > 0.0 {
            self.entries
                .iter()
                .map(|e| e.weight / total_weight)
                .collect()
        } else {
            vec![1.0 / self.entries.len() as f32; self.entries.len()]
        };

        // Evaluate all animations
        let mut result = vec![BoneTransform::default(); self.bone_count];

        for (entry, weight) in self.entries.iter().zip(normalized_weights.iter()) {
            let transforms = entry.player.evaluate(self.bone_count);

            for (i, transform) in transforms.iter().enumerate() {
                if i < self.bone_count {
                    if *weight >= 1.0 {
                        result[i] = *transform;
                    } else {
                        result[i] = result[i].blend(transform, *weight);
                    }
                }
            }
        }

        result
    }

    /// Transition to a new animation
    pub fn transition_to(&mut self, animation: Arc<Animation>, fade_duration: f32) {
        // Fade out all existing animations
        for entry in &mut self.entries {
            entry.fade_out(fade_duration);
        }

        // Add and fade in new animation
        let mut new_entry = AnimationBlendEntry::new(animation, 0.0);
        new_entry.player.play();
        new_entry.fade_in(fade_duration);
        self.entries.push(new_entry);
    }

    /// Get animation count
    pub fn animation_count(&self) -> usize {
        self.entries.len()
    }
}

/// Animation manager for global animation state
pub struct AnimationManager {
    animations: HashMap<String, Arc<Animation>>,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
        }
    }

    /// Register an animation
    pub fn register(&mut self, animation: Animation) -> Arc<Animation> {
        let name = animation.name.clone();
        let arc = Arc::new(animation);
        self.animations.insert(name, Arc::clone(&arc));
        arc
    }

    /// Get animation by name
    pub fn get(&self, name: &str) -> Option<Arc<Animation>> {
        self.animations.get(name).map(Arc::clone)
    }

    /// Remove animation
    pub fn remove(&mut self, name: &str) -> bool {
        self.animations.remove(name).is_some()
    }

    /// Clear all animations
    pub fn clear(&mut self) {
        self.animations.clear();
    }

    /// Get animation count
    pub fn count(&self) -> usize {
        self.animations.len()
    }

    /// Get all animation names
    pub fn animation_names(&self) -> Vec<String> {
        self.animations.keys().cloned().collect()
    }
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_creation() {
        let anim = Animation::new("test".to_string(), "skeleton".to_string(), 30, 30.0);
        assert_eq!(anim.duration(), 1.0); // 30 frames at 30 fps = 1 second
    }

    #[test]
    fn test_animation_channel_scalar() {
        let mut channel = AnimationChannel::new(0, ChannelType::TranslationX);
        channel.add_keyframe(Keyframe::Scalar {
            time: 0.0,
            value: 0.0,
        });
        channel.add_keyframe(Keyframe::Scalar {
            time: 1.0,
            value: 10.0,
        });

        assert_eq!(channel.evaluate_scalar(0.0), Some(0.0));
        assert_eq!(channel.evaluate_scalar(0.5), Some(5.0));
        assert_eq!(channel.evaluate_scalar(1.0), Some(10.0));
    }

    #[test]
    fn test_animation_player() {
        let anim = Arc::new(Animation::new(
            "test".to_string(),
            "skeleton".to_string(),
            30,
            30.0,
        ));
        let mut player = AnimationPlayer::new(anim);

        player.play();
        assert_eq!(player.state, AnimationState::Playing);

        player.update(0.5);
        assert!(player.current_time > 0.0);
    }

    #[test]
    fn test_bone_transform_blend() {
        let t1 = BoneTransform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            visibility: true,
        };

        let t2 = BoneTransform {
            translation: Vec3::new(10.0, 10.0, 10.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            visibility: true,
        };

        let blended = t1.blend(&t2, 0.5);
        assert!((blended.translation - Vec3::new(5.0, 5.0, 5.0)).length() < 0.001);
    }

    #[test]
    fn test_animation_blender() {
        let anim1 = Arc::new(Animation::new(
            "anim1".to_string(),
            "skeleton".to_string(),
            30,
            30.0,
        ));
        let anim2 = Arc::new(Animation::new(
            "anim2".to_string(),
            "skeleton".to_string(),
            30,
            30.0,
        ));

        let mut blender = AnimationBlender::new(10);
        blender.add_animation(anim1, 0.5);
        blender.add_animation(anim2, 0.5);

        assert_eq!(blender.animation_count(), 2);

        let transforms = blender.evaluate();
        assert_eq!(transforms.len(), 10);
    }

    #[test]
    fn test_animation_manager() {
        let mut mgr = AnimationManager::new();
        let anim = Animation::new("test".to_string(), "skeleton".to_string(), 30, 30.0);

        let arc = mgr.register(anim);
        assert_eq!(mgr.count(), 1);

        let retrieved = mgr.get("test");
        assert!(retrieved.is_some());
        assert!(Arc::ptr_eq(&arc, &retrieved.unwrap()));
    }
}
