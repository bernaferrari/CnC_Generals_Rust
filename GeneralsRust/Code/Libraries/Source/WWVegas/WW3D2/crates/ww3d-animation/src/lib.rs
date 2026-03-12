use glam::{Mat4, Quat, Vec3};
use ww3d_assets::prototypes::{AnimationChannelData, AnimationPrototype, HierarchyPrototype};

pub mod animated_sound;
pub mod animation_blending;
pub mod animation_state_machine;
pub mod animatable_object;
pub mod combo;
pub mod hanim;
pub mod hcompressed_anim;
pub mod htree;
pub mod ik_system;
pub mod manager;
pub mod motion_channels;
pub mod skeletal_animation;
pub mod w3d_loader;
pub mod w3d_model_loader;

#[cfg(feature = "wgpu-renderer")]
pub mod wgpu_skinned_renderer;

pub use animated_sound::{
    embedded_sound_bone, has_embedded_sounds, initialize as initialize_animated_sound_mgr,
    initialize_from_bytes as initialize_animated_sound_mgr_from_bytes, set_sound_library,
    shutdown as shutdown_animated_sound_mgr, trigger_sound, SoundLibraryBridge,
};
pub use animatable_object::Animatable3DObjClass;
pub use animation_blending::{
    AdvancedAnimatable3DObj, AnimationController, AnimationLayer, AnimationParameter,
    AnimationState, AnimationTransition, BlendMode, TransitionCondition,
};
pub use combo::{HAnimCombo, HAnimComboData, NamedPivotMap, PivotMap, PivotWeightMap};
pub use hanim::{
    AnimationEvent, AnimationMode, Axis, BitChannel, HAnimClass, MotionChannel, MotionChannelType,
};
pub use hcompressed_anim::{
    HCompressedAnimClass, ANIM_FLAVOR_ADAPTIVE_DELTA, ANIM_FLAVOR_TIMECODED,
};
pub use htree::HTreeClass;
pub use ik_system::{BoneConstraint, ConstraintType, FABRIKSolver, IKChain, IKError, IKResult};
pub use manager::HAnimManager;
pub use motion_channels::{
    AdaptiveDeltaMotionChannelClass, TimeCodedBitChannelClass, TimeCodedMotionChannelClass,
};
pub use w3d_loader::{
    load_w3d_animation, load_w3d_animation_from_file, load_w3d_hierarchy,
    load_w3d_hierarchy_from_file, w3d_animation_to_hanim, W3DAnimationChannel, W3DAnimationData,
    W3DAnimationError,
};

// Export new skeletal animation system
pub use skeletal_animation::{AnimatedModel, SkeletonState, MAX_BONES};
pub use w3d_model_loader::{
    BoneInfluence, BoundingBox, HModelConnection, HModelData, LODLevel, LODModelData, W3DMeshData,
    W3DModel,
};
pub use animation_state_machine::{
    AnimationState as GameAnimationState, AnimationStateMachine, AnimationStateMachineBuilder,
    StateTransition, TransitionCondition as StateTransitionCondition,
};

#[cfg(feature = "wgpu-renderer")]
pub use wgpu_skinned_renderer::{
    BoneMatricesUniform, SkinnedMeshBuffer, SkinnedMeshRenderer, SkinnedVertex,
    prepare_model_for_rendering, SKINNED_MESH_SHADER,
};

/// Build a [`HAnimClass`] from an [`AnimationPrototype`] exported by the asset loader.
pub fn hanim_from_prototype(proto: &AnimationPrototype) -> HAnimClass {
    let channels = proto
        .channels
        .iter()
        .filter_map(|channel| motion_channel_from_data(channel))
        .collect::<Vec<_>>();

    HAnimClass::with_channels(
        &proto.name,
        &proto.hierarchy_name,
        proto.num_frames,
        proto.frame_rate as f32,
        channels,
        Vec::new(),
    )
}

/// Build an [`HTreeClass`] from a hierarchy prototype exported by the asset loader.
pub fn htree_from_hierarchy_prototype(proto: &HierarchyPrototype) -> HTreeClass {
    let mut tree = HTreeClass::new();
    tree.name = proto.name.clone();
    tree.pivots.clear();
    tree.pivot_name_to_index.clear();

    for pivot in &proto.pivots {
        let base_transform = pivot.base_transform();
        tree.add_pivot_from_base(&pivot.name_str(), pivot.parent_idx, base_transform);
    }

    if !proto.pivots.is_empty() {
        tree.base_update(Mat4::IDENTITY);
    }

    if !proto.bind_transforms.is_empty() && proto.bind_transforms.len() == tree.pivots.len() {
        for (pivot, bind) in tree.pivots.iter_mut().zip(proto.bind_transforms.iter()) {
            pivot.transform = *bind;
        }
    }

    tree
}

fn motion_channel_from_data(channel: &AnimationChannelData) -> Option<MotionChannel> {
    let channel_type = MotionChannelType::from_flags(channel.flags);
    if matches!(channel_type, MotionChannelType::Unknown(_)) {
        return None;
    }

    let vector_len = usize::max(channel.vector_len as usize, 1);
    Some(MotionChannel::new(
        channel_type,
        channel.pivot as usize,
        channel.first_frame,
        channel.last_frame,
        vector_len,
        channel.data.clone(),
    ))
}

/// Represents a pivot in a hierarchy tree (skeleton bone)
#[derive(Debug, Clone)]
pub struct Pivot {
    pub name: String,
    pub parent_idx: Option<usize>,
    pub base_transform: Mat4,
    pub base_translate: Vec3,
    pub base_rotate: Quat,
}

impl Pivot {
    pub fn new(name: String, parent_idx: Option<usize>) -> Self {
        Self {
            name,
            parent_idx,
            base_transform: Mat4::IDENTITY,
            base_translate: Vec3::ZERO,
            base_rotate: Quat::IDENTITY,
        }
    }
}

/// Hierarchy tree representing a skeleton
#[derive(Debug)]
pub struct HTree {
    pub pivots: Vec<Pivot>,
}

impl HTree {
    pub fn new() -> Self {
        Self { pivots: Vec::new() }
    }

    pub fn add_pivot(&mut self, pivot: Pivot) {
        self.pivots.push(pivot);
    }

    pub fn get_pivot_count(&self) -> usize {
        self.pivots.len()
    }

    pub fn get_pivot(&self, index: usize) -> Option<&Pivot> {
        self.pivots.get(index)
    }

    pub fn find_pivot_index(&self, name: &str) -> Option<usize> {
        self.pivots
            .iter()
            .position(|pivot| pivot.name.eq_ignore_ascii_case(name))
    }
}

/// Animation channel types
#[derive(Debug, Clone, Copy)]
pub enum ChannelType {
    Translation,
    Rotation,
    Visibility,
}

/// Animation channel data
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    pub pivot_idx: usize,
    pub channel_type: ChannelType,
    pub data: Vec<f32>,  // Keyframe data
    pub times: Vec<f32>, // Keyframe times
}

impl AnimationChannel {
    pub fn new(pivot_idx: usize, channel_type: ChannelType) -> Self {
        Self {
            pivot_idx,
            channel_type,
            data: Vec::new(),
            times: Vec::new(),
        }
    }

    pub fn add_keyframe(&mut self, time: f32, value: f32) {
        self.times.push(time);
        self.data.push(value);
    }

    pub fn sample(&self, time: f32) -> f32 {
        if self.times.is_empty() {
            return 0.0;
        }

        // Find the appropriate keyframes
        let mut left = 0;
        let mut right = self.times.len() - 1;

        while left < right {
            let mid = (left + right) / 2;
            if self.times[mid] < time {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        if left == 0 {
            return self.data[0];
        }

        if left >= self.times.len() {
            return *self.data.last().unwrap();
        }

        let t1 = self.times[left - 1];
        let t2 = self.times[left];
        let v1 = self.data[left - 1];
        let v2 = self.data[left];

        if (t2 - t1).abs() < f32::EPSILON {
            return v1;
        }

        let factor = (time - t1) / (t2 - t1);
        v1 + (v2 - v1) * factor
    }
}

/// Bone hierarchy for skeletal animation
#[derive(Debug, Clone)]
pub struct BoneHierarchy {
    pub bones: Vec<Bone>,
}

/// Individual bone in the hierarchy
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub transform: glam::Mat4,
    pub parent_index: Option<usize>,
}

impl BoneHierarchy {
    /// Sample bone matrices at a given time
    pub fn sample_bone_matrices(&self, _time: f32) -> Vec<glam::Mat4> {
        self.bones.iter().map(|bone| bone.transform).collect()
    }
}

#[derive(Debug)]
pub struct HAnim {
    pub name: String,
    pub num_frames: u32,
    pub frame_rate: f32,
    pub channels: Vec<AnimationChannel>,
}

impl HAnim {
    pub fn new(name: String) -> Self {
        Self {
            name,
            num_frames: 0,
            frame_rate: 30.0,
            channels: Vec::new(),
        }
    }

    pub fn add_channel(&mut self, channel: AnimationChannel) {
        self.channels.push(channel);
    }

    pub fn get_channel_count(&self) -> usize {
        self.channels.len()
    }
}

/// Animated object that combines skeleton and animation
#[derive(Debug)]
pub struct Animatable3DObj {
    pub htree: HTree,
    pub anim: Option<HAnim>,
    pub current_frame: f32,
    pub bone_matrices: Vec<Mat4>,
}

impl Animatable3DObj {
    pub fn new(htree: HTree) -> Self {
        let bone_count = htree.get_pivot_count();
        Self {
            htree,
            anim: None,
            current_frame: 0.0,
            bone_matrices: vec![Mat4::IDENTITY; bone_count],
        }
    }

    pub fn set_animation(&mut self, anim: HAnim) {
        self.anim = Some(anim);
    }

    pub fn update_animation(&mut self, delta_time: f32) {
        if let Some(ref anim) = self.anim {
            self.current_frame += delta_time * anim.frame_rate;

            if self.current_frame >= anim.num_frames as f32 {
                self.current_frame = 0.0;
            }

            self.update_bone_matrices();
        }
    }

    fn update_bone_matrices(&mut self) {
        let Some(anim) = self.anim.as_ref() else {
            return;
        };

        let mut translations = vec![Vec3::ZERO; self.htree.pivots.len()];
        let mut rotations = vec![Quat::IDENTITY; self.htree.pivots.len()];

        for channel in &anim.channels {
            let pivot_idx = channel.pivot_idx;
            if pivot_idx >= self.htree.pivots.len() {
                continue;
            }

            let value = channel.sample(self.current_frame);

            match channel.channel_type {
                ChannelType::Translation => {
                    translations[pivot_idx] = Vec3::new(value, 0.0, 0.0);
                }
                ChannelType::Rotation => {
                    rotations[pivot_idx] = Quat::from_rotation_y(value);
                }
                ChannelType::Visibility => {}
            }
        }

        for i in 0..self.htree.pivots.len() {
            let pivot = &self.htree.pivots[i];

            let mut transform = pivot.base_transform;

            let anim_translate = Mat4::from_translation(translations[i]);
            let anim_rotate = Mat4::from_quat(rotations[i]);

            transform = transform * anim_translate * anim_rotate;

            if let Some(parent_idx) = pivot.parent_idx {
                if parent_idx < self.bone_matrices.len() {
                    transform = self.bone_matrices[parent_idx] * transform;
                }
            }

            self.bone_matrices[i] = transform;
        }
    }

    pub fn get_bone_transform(&self, bone_idx: usize) -> Option<&Mat4> {
        self.bone_matrices.get(bone_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skeleton_sampling() {
        let skel = BoneHierarchy { bones: vec![] };
        let matrices = skel.sample_bone_matrices(0.0);
        assert!(matrices.is_empty());
    }
}
