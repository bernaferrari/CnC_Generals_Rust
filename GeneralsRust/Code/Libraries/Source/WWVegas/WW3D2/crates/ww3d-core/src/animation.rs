/// Animation system for WW3D
///
/// This module implements hierarchical skeletal animation with support for
/// keyframe interpolation, animation blending, and compressed animations.
use crate::errors::W3DResult;
use crate::w3d_format::*;
use glam::{Mat4, Quat, Vec3};

/// Animation playback mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationMode {
    /// Play once and stop
    Once,
    /// Loop continuously
    Loop,
    /// Loop with ping-pong (forward then backward)
    LoopPingPong,
    /// Play once backward
    OnceBackward,
    /// Loop backward
    LoopBackward,
    /// Manual control (no automatic advancement)
    Manual,
}

/// Keyframe for position animation
#[derive(Debug, Clone, Copy)]
pub struct PositionKeyframe {
    pub time: f32,
    pub position: Vec3,
}

/// Keyframe for rotation animation
#[derive(Debug, Clone, Copy)]
pub struct RotationKeyframe {
    pub time: f32,
    pub rotation: Quat,
}

/// Keyframe for scale animation
#[derive(Debug, Clone, Copy)]
pub struct ScaleKeyframe {
    pub time: f32,
    pub scale: Vec3,
}

/// Animation channel for a single bone/pivot
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    pub pivot_index: u32,
    pub position_keys: Vec<PositionKeyframe>,
    pub rotation_keys: Vec<RotationKeyframe>,
    pub scale_keys: Vec<ScaleKeyframe>,
}

impl AnimationChannel {
    pub fn new(pivot_index: u32) -> Self {
        Self {
            pivot_index,
            position_keys: Vec::new(),
            rotation_keys: Vec::new(),
            scale_keys: Vec::new(),
        }
    }

    pub fn from_w3d(w3d_channel: &W3dAnimChannelStruct) -> Self {
        Self {
            pivot_index: w3d_channel.pivot as u32,
            position_keys: Vec::new(),
            rotation_keys: Vec::new(),
            scale_keys: Vec::new(),
        }
    }

    pub fn add_position_key(&mut self, time: f32, position: Vec3) {
        self.position_keys.push(PositionKeyframe { time, position });
    }

    pub fn add_rotation_key(&mut self, time: f32, rotation: Quat) {
        self.rotation_keys.push(RotationKeyframe { time, rotation });
    }

    pub fn add_scale_key(&mut self, time: f32, scale: Vec3) {
        self.scale_keys.push(ScaleKeyframe { time, scale });
    }

    pub fn evaluate_position(&self, time: f32) -> Vec3 {
        if self.position_keys.is_empty() {
            return Vec3::ZERO;
        }

        if self.position_keys.len() == 1 {
            return self.position_keys[0].position;
        }

        // Find keyframes to interpolate between
        let mut key0_idx = 0;
        let mut key1_idx = 0;

        for (i, key) in self.position_keys.iter().enumerate() {
            if key.time <= time {
                key0_idx = i;
            }
            if key.time >= time {
                key1_idx = i;
                break;
            }
        }

        if key0_idx == key1_idx {
            return self.position_keys[key0_idx].position;
        }

        let key0 = &self.position_keys[key0_idx];
        let key1 = &self.position_keys[key1_idx];

        let dt = key1.time - key0.time;
        if dt < 0.0001 {
            return key0.position;
        }

        let t = ((time - key0.time) / dt).clamp(0.0, 1.0);
        key0.position.lerp(key1.position, t)
    }

    pub fn evaluate_rotation(&self, time: f32) -> Quat {
        if self.rotation_keys.is_empty() {
            return Quat::IDENTITY;
        }

        if self.rotation_keys.len() == 1 {
            return self.rotation_keys[0].rotation;
        }

        // Find keyframes to interpolate between
        let mut key0_idx = 0;
        let mut key1_idx = 0;

        for (i, key) in self.rotation_keys.iter().enumerate() {
            if key.time <= time {
                key0_idx = i;
            }
            if key.time >= time {
                key1_idx = i;
                break;
            }
        }

        if key0_idx == key1_idx {
            return self.rotation_keys[key0_idx].rotation;
        }

        let key0 = &self.rotation_keys[key0_idx];
        let key1 = &self.rotation_keys[key1_idx];

        let dt = key1.time - key0.time;
        if dt < 0.0001 {
            return key0.rotation;
        }

        let t = ((time - key0.time) / dt).clamp(0.0, 1.0);
        key0.rotation.slerp(key1.rotation, t)
    }

    pub fn evaluate_scale(&self, time: f32) -> Vec3 {
        if self.scale_keys.is_empty() {
            return Vec3::ONE;
        }

        if self.scale_keys.len() == 1 {
            return self.scale_keys[0].scale;
        }

        // Find keyframes to interpolate between
        let mut key0_idx = 0;
        let mut key1_idx = 0;

        for (i, key) in self.scale_keys.iter().enumerate() {
            if key.time <= time {
                key0_idx = i;
            }
            if key.time >= time {
                key1_idx = i;
                break;
            }
        }

        if key0_idx == key1_idx {
            return self.scale_keys[key0_idx].scale;
        }

        let key0 = &self.scale_keys[key0_idx];
        let key1 = &self.scale_keys[key1_idx];

        let dt = key1.time - key0.time;
        if dt < 0.0001 {
            return key0.scale;
        }

        let t = ((time - key0.time) / dt).clamp(0.0, 1.0);
        key0.scale.lerp(key1.scale, t)
    }

    pub fn evaluate(&self, time: f32) -> (Vec3, Quat, Vec3) {
        (
            self.evaluate_position(time),
            self.evaluate_rotation(time),
            self.evaluate_scale(time),
        )
    }
}

/// Hierarchy animation
#[derive(Debug, Clone)]
pub struct HierarchyAnimation {
    pub name: String,
    pub hierarchy_name: String,
    pub frame_count: u32,
    pub frame_rate: f32,
    pub channels: Vec<AnimationChannel>,
}

impl HierarchyAnimation {
    pub fn new(name: String, hierarchy_name: String) -> Self {
        Self {
            name,
            hierarchy_name,
            frame_count: 0,
            frame_rate: 30.0,
            channels: Vec::new(),
        }
    }

    pub fn from_w3d(w3d_anim: &W3dAnimation) -> W3DResult<Self> {
        let header = &w3d_anim.header;

        let name = header.name_str();
        let hierarchy_name = header.hiera_name_str();

        let mut animation = Self {
            name,
            hierarchy_name,
            frame_count: header.num_frames,
            frame_rate: header.frame_rate as f32,
            channels: Vec::new(),
        };

        for w3d_channel in &w3d_anim.channels {
            animation
                .channels
                .push(AnimationChannel::from_w3d(w3d_channel));
        }

        Ok(animation)
    }

    pub fn add_channel(&mut self, channel: AnimationChannel) {
        self.channels.push(channel);
    }

    pub fn get_channel(&self, pivot_index: u32) -> Option<&AnimationChannel> {
        self.channels.iter().find(|c| c.pivot_index == pivot_index)
    }

    pub fn get_channel_mut(&mut self, pivot_index: u32) -> Option<&mut AnimationChannel> {
        self.channels
            .iter_mut()
            .find(|c| c.pivot_index == pivot_index)
    }

    pub fn duration(&self) -> f32 {
        if self.frame_rate > 0.0 {
            self.frame_count as f32 / self.frame_rate
        } else {
            0.0
        }
    }

    pub fn evaluate_channel(&self, pivot_index: u32, time: f32) -> Option<(Vec3, Quat, Vec3)> {
        self.get_channel(pivot_index)
            .map(|channel| channel.evaluate(time))
    }
}

/// Animation instance with playback state
#[derive(Debug)]
pub struct AnimationInstance {
    animation: HierarchyAnimation,
    mode: AnimationMode,
    current_time: f32,
    speed: f32,
    playing: bool,
    reverse: bool,
    weight: f32,
}

impl AnimationInstance {
    pub fn new(animation: HierarchyAnimation) -> Self {
        Self {
            animation,
            mode: AnimationMode::Loop,
            current_time: 0.0,
            speed: 1.0,
            playing: false,
            reverse: false,
            weight: 1.0,
        }
    }

    pub fn animation(&self) -> &HierarchyAnimation {
        &self.animation
    }

    pub fn set_mode(&mut self, mode: AnimationMode) {
        self.mode = mode;
    }

    pub fn mode(&self) -> AnimationMode {
        self.mode
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.current_time = 0.0;
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn set_time(&mut self, time: f32) {
        self.current_time = time.clamp(0.0, self.animation.duration());
    }

    pub fn current_time(&self) -> f32 {
        self.current_time
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_weight(&mut self, weight: f32) {
        self.weight = weight.clamp(0.0, 1.0);
    }

    pub fn weight(&self) -> f32 {
        self.weight
    }

    pub fn update(&mut self, delta_time: f32) {
        if !self.playing {
            return;
        }

        let duration = self.animation.duration();
        if duration <= 0.0 {
            return;
        }

        let time_delta = delta_time * self.speed * if self.reverse { -1.0 } else { 1.0 };
        self.current_time += time_delta;

        match self.mode {
            AnimationMode::Once => {
                if self.current_time >= duration {
                    self.current_time = duration;
                    self.playing = false;
                } else if self.current_time < 0.0 {
                    self.current_time = 0.0;
                    self.playing = false;
                }
            }
            AnimationMode::Loop => {
                self.current_time = self.current_time.rem_euclid(duration);
            }
            AnimationMode::LoopPingPong => {
                if self.current_time >= duration {
                    self.current_time = duration;
                    self.reverse = !self.reverse;
                } else if self.current_time < 0.0 {
                    self.current_time = 0.0;
                    self.reverse = !self.reverse;
                }
            }
            AnimationMode::OnceBackward => {
                self.reverse = true;
                if self.current_time < 0.0 {
                    self.current_time = 0.0;
                    self.playing = false;
                }
            }
            AnimationMode::LoopBackward => {
                self.reverse = true;
                self.current_time = self.current_time.rem_euclid(duration);
            }
            AnimationMode::Manual => {
                // No automatic update
            }
        }
    }

    pub fn evaluate_channel(&self, pivot_index: u32) -> Option<(Vec3, Quat, Vec3)> {
        self.animation
            .evaluate_channel(pivot_index, self.current_time)
    }
}

/// Hierarchy (skeleton) definition
#[derive(Debug, Clone)]
pub struct Hierarchy {
    pub name: String,
    pub pivots: Vec<Pivot>,
}

impl Hierarchy {
    pub fn new(name: String) -> Self {
        Self {
            name,
            pivots: Vec::new(),
        }
    }

    pub fn from_w3d(w3d_hier: &W3dHierarchy) -> W3DResult<Self> {
        let header = &w3d_hier.header;

        let name = header.name_str();

        let mut hierarchy = Self {
            name,
            pivots: Vec::new(),
        };

        for w3d_pivot in &w3d_hier.pivots {
            hierarchy.pivots.push(Pivot::from_w3d(w3d_pivot));
        }

        Ok(hierarchy)
    }

    pub fn add_pivot(&mut self, pivot: Pivot) {
        self.pivots.push(pivot);
    }

    pub fn get_pivot(&self, index: usize) -> Option<&Pivot> {
        self.pivots.get(index)
    }

    pub fn get_pivot_mut(&mut self, index: usize) -> Option<&mut Pivot> {
        self.pivots.get_mut(index)
    }

    pub fn pivot_count(&self) -> usize {
        self.pivots.len()
    }

    pub fn find_pivot_by_name(&self, name: &str) -> Option<usize> {
        self.pivots.iter().position(|p| p.name == name)
    }
}

/// Pivot (bone) in a hierarchy
#[derive(Debug, Clone)]
pub struct Pivot {
    pub name: String,
    pub parent_index: i32,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Pivot {
    pub fn new(name: String) -> Self {
        Self {
            name,
            parent_index: -1,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn from_w3d(w3d_pivot: &W3dPivotStruct) -> Self {
        let position: Vec3 = w3d_pivot.translation.into();
        // Convert euler angles to quaternion
        let euler: Vec3 = w3d_pivot.euler_angles.into();
        let rotation = Quat::from_euler(glam::EulerRot::XYZ, euler.x, euler.y, euler.z);

        Self {
            name: w3d_pivot.name_str(),
            parent_index: w3d_pivot.parent_idx,
            position,
            rotation,
            scale: Vec3::ONE,
        }
    }

    pub fn local_transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    pub fn has_parent(&self) -> bool {
        self.parent_index >= 0
    }
}

/// Animation controller for managing multiple animations and blending
#[derive(Debug)]
pub struct AnimationController {
    hierarchy: Hierarchy,
    instances: Vec<AnimationInstance>,
    bone_transforms: Vec<Mat4>,
}

impl AnimationController {
    pub fn new(hierarchy: Hierarchy) -> Self {
        let pivot_count = hierarchy.pivot_count();
        Self {
            hierarchy,
            instances: Vec::new(),
            bone_transforms: vec![Mat4::IDENTITY; pivot_count],
        }
    }

    pub fn hierarchy(&self) -> &Hierarchy {
        &self.hierarchy
    }

    pub fn add_animation(&mut self, animation: HierarchyAnimation) -> usize {
        let index = self.instances.len();
        self.instances.push(AnimationInstance::new(animation));
        index
    }

    pub fn get_instance(&self, index: usize) -> Option<&AnimationInstance> {
        self.instances.get(index)
    }

    pub fn get_instance_mut(&mut self, index: usize) -> Option<&mut AnimationInstance> {
        self.instances.get_mut(index)
    }

    pub fn update(&mut self, delta_time: f32) {
        // Update all animation instances
        for instance in &mut self.instances {
            instance.update(delta_time);
        }

        // Rebuild bone transforms
        self.rebuild_bone_transforms();
    }

    pub fn bone_transforms(&self) -> &[Mat4] {
        &self.bone_transforms
    }

    fn rebuild_bone_transforms(&mut self) {
        // Initialize with base poses
        for (i, pivot) in self.hierarchy.pivots.iter().enumerate() {
            self.bone_transforms[i] = pivot.local_transform();
        }

        // Apply animations with blending
        for instance in &self.instances {
            if !instance.is_playing() || instance.weight() <= 0.0 {
                continue;
            }

            for (i, _pivot) in self.hierarchy.pivots.iter().enumerate() {
                if let Some((pos, rot, scale)) = instance.evaluate_channel(i as u32) {
                    let anim_transform = Mat4::from_scale_rotation_translation(scale, rot, pos);

                    if instance.weight() >= 1.0 {
                        self.bone_transforms[i] = anim_transform;
                    } else {
                        // Blend with existing transform
                        self.bone_transforms[i] = self.blend_transforms(
                            &self.bone_transforms[i],
                            &anim_transform,
                            instance.weight(),
                        );
                    }
                }
            }
        }

        // Convert to world space (apply parent transforms)
        for i in 0..self.hierarchy.pivot_count() {
            let parent_idx = self.hierarchy.pivots[i].parent_index;
            if parent_idx >= 0 {
                let parent_transform = self.bone_transforms[parent_idx as usize];
                self.bone_transforms[i] = parent_transform * self.bone_transforms[i];
            }
        }
    }

    fn blend_transforms(&self, t1: &Mat4, t2: &Mat4, weight: f32) -> Mat4 {
        // Extract components
        let (scale1, rot1, pos1) = t1.to_scale_rotation_translation();
        let (scale2, rot2, pos2) = t2.to_scale_rotation_translation();

        // Blend components
        let pos = pos1.lerp(pos2, weight);
        let rot = rot1.slerp(rot2, weight);
        let scale = scale1.lerp(scale2, weight);

        Mat4::from_scale_rotation_translation(scale, rot, pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_channel() {
        let mut channel = AnimationChannel::new(0);

        channel.add_position_key(0.0, Vec3::ZERO);
        channel.add_position_key(1.0, Vec3::new(10.0, 0.0, 0.0));

        let pos_mid = channel.evaluate_position(0.5);
        assert!((pos_mid.x - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_animation_instance() {
        let mut anim = HierarchyAnimation::new("test".to_string(), "hier".to_string());
        anim.frame_count = 30;
        anim.frame_rate = 30.0;

        let mut instance = AnimationInstance::new(anim);
        instance.set_mode(AnimationMode::Loop);
        instance.play();

        instance.update(0.5);
        assert!((instance.current_time() - 0.5).abs() < 0.001);

        instance.update(0.6); // Should loop
        assert!((instance.current_time() - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_pivot_transform() {
        let pivot = Pivot {
            name: "test".to_string(),
            parent_index: -1,
            position: Vec3::new(1.0, 2.0, 3.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };

        let transform = pivot.local_transform();
        let pos = transform.transform_point3(Vec3::ZERO);

        assert!((pos - pivot.position).length() < 0.001);
    }

    #[test]
    fn test_hierarchy() {
        let mut hierarchy = Hierarchy::new("test_hier".to_string());

        hierarchy.add_pivot(Pivot::new("root".to_string()));
        hierarchy.add_pivot(Pivot::new("child".to_string()));

        assert_eq!(hierarchy.pivot_count(), 2);
        assert_eq!(hierarchy.find_pivot_by_name("root"), Some(0));
        assert_eq!(hierarchy.find_pivot_by_name("child"), Some(1));
    }

    #[test]
    fn test_animation_controller() {
        let mut hierarchy = Hierarchy::new("test".to_string());
        hierarchy.add_pivot(Pivot::new("root".to_string()));

        let mut controller = AnimationController::new(hierarchy);

        let anim = HierarchyAnimation::new("walk".to_string(), "test".to_string());
        let anim_idx = controller.add_animation(anim);

        assert!(controller.get_instance(anim_idx).is_some());
    }
}
