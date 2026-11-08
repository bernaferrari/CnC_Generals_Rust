#![allow(clippy::new_without_default)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::single_match)]

pub mod animatable_object;
pub mod animated_sound;
pub mod animation_blending;
pub mod animation_state_machine;
pub mod combo;
pub mod hanim;
pub mod hcompressed_anim;
pub mod hraw_anim;
pub mod htree;
pub mod ik_system;
pub mod manager;
pub mod motion_channels;
pub mod skeletal_animation;
pub mod w3d_loader;
pub mod w3d_model_loader;

#[cfg(feature = "wgpu-renderer")]
pub mod wgpu_skinned_renderer;

pub use animatable_object::Animatable3DObjClass;
pub use animated_sound::{
    AnimatedSoundMgr, SoundLibraryBridge, embedded_sound_bone, has_embedded_sounds,
    initialize as initialize_animated_sound_mgr,
    initialize_from_bytes as initialize_animated_sound_mgr_from_bytes, set_sound_library,
    shutdown as shutdown_animated_sound_mgr, trigger_sound,
};
pub use animation_blending::{
    AdvancedAnimatable3DObj, AnimationController, AnimationLayer, AnimationParameter,
    AnimationState, AnimationTransition, BlendMode, TransitionCondition,
};
pub use combo::{HAnimCombo, HAnimComboData, NamedPivotMap, PivotMap, PivotWeightMap};
pub use hanim::{AnimationEvent, AnimationMode, HAnimClass};
pub use hcompressed_anim::{
    ANIM_FLAVOR_ADAPTIVE_DELTA, ANIM_FLAVOR_TIMECODED, HCompressedAnimClass,
};
pub use hraw_anim::{HRawAnimClass, RawPivotPose};
pub use htree::HTreeClass;
pub use ik_system::{BoneConstraint, ConstraintType, FABRIKSolver, IKChain, IKError, IKResult};
pub use manager::HAnimManager;
pub use motion_channels::{
    AdaptiveDeltaMotionChannelClass, AnimChannelKind, BitChannelClass, BitChannelKind,
    MotionChannelClass, TimeCodedBitChannelClass, TimeCodedMotionChannelClass,
};
pub use w3d_loader::{
    W3DAnimationData, W3DAnimationError, load_w3d_animation,
    load_w3d_animation_from_file, load_w3d_hierarchy, load_w3d_hierarchy_from_file,
    w3d_animation_to_hanim,
};

// Export new skeletal animation system
pub use animation_state_machine::{
    AnimationState as GameAnimationState, AnimationStateMachine, AnimationStateMachineBuilder,
    StateTransition, TransitionCondition as StateTransitionCondition,
};
pub use skeletal_animation::{AnimatedModel, MAX_BONES, SkeletonState};
pub use w3d_model_loader::{
    BoneInfluence, BoundingBox, HModelConnection, HModelData, LODLevel, LODModelData, W3DMeshData,
    W3DModel,
};

#[cfg(feature = "wgpu-renderer")]
pub use wgpu_skinned_renderer::{
    BoneMatricesUniform, SKINNED_MESH_SHADER, SkinnedMeshBuffer, SkinnedMeshRenderer,
    SkinnedVertex, prepare_model_for_rendering,
};
