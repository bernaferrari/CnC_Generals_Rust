//! Scratch `HTreeClass::Anim_Update` evaluation for HLOD ghost freeze.
//!
//! C++ `W3DGhostObject.cpp:113-120` Peek_Animation_And_Info then
//! `Set_Animation(hanim, frame)` MANUAL on the clone. The clone later
//! renders via `HTreeClass::Anim_Update` (`htree.cpp:509-554`) and
//! `HLodClass::Update_Sub_Object_Transforms` (`hlod.cpp:3242`).
//!
//! This module evaluates that pose without mutating prototypes or the
//! cached `HlodInstance`. Empty channels / missing hierarchy fail closed.

use crate::assets::AssetManager;
use crate::prototypes::{
    AnimationChannelData, AnimationPrototype, HierarchyPrototype, HlodInstance,
};
use glam::{Mat4, Quat, Vec3};

const ANIM_CHANNEL_X: u16 = 0;
const ANIM_CHANNEL_Y: u16 = 1;
const ANIM_CHANNEL_Z: u16 = 2;
const ANIM_CHANNEL_Q: u16 = 6;

/// Convert a 0..=1 animation fraction to a C++ HAnim frame number.
///
/// Live collect uses `fraction * (num_frames - 1)` (`pipeline_collect`).
pub fn fraction_to_hanim_frame(fraction: f32, num_frames: u32) -> Option<f32> {
    if !fraction.is_finite() || num_frames == 0 {
        return None;
    }
    let fraction = fraction.clamp(0.0, 1.0);
    if num_frames == 1 {
        return Some(0.0);
    }
    Some(fraction * (num_frames - 1) as f32)
}

/// World-space bone transforms after `HTreeClass::Anim_Update`.
///
/// Pivot 0 is `root`. Pivots `1..` are `parent * Base * T * Q` at the
/// raw integer frame (`htree.cpp:558-632`). Returns `None` when the
/// animation has no usable channel payload.
pub fn evaluate_htree_anim_worlds(
    hierarchy: &HierarchyPrototype,
    animation: &AnimationPrototype,
    frame: f32,
    root: Mat4,
) -> Option<Vec<Mat4>> {
    if hierarchy.pivots.is_empty() || animation.channels.is_empty() || !root.is_finite() {
        return None;
    }
    let raw_frame = raw_frame_index(frame, animation.num_frames)?;
    let count = hierarchy.pivots.len();
    let mut worlds = vec![Mat4::IDENTITY; count];
    worlds[0] = root;

    for index in 1..count {
        let pivot = &hierarchy.pivots[index];
        let parent_idx = pivot.parent_idx;
        if parent_idx < 0 || (parent_idx as usize) >= count {
            return None;
        }
        let parent = worlds[parent_idx as usize];
        let translation = sample_translation(animation, index, raw_frame)?;
        let rotation = sample_rotation(animation, index, raw_frame)?;
        let world = parent
            * pivot.base_transform()
            * Mat4::from_translation(translation)
            * Mat4::from_quat(rotation);
        if !world.is_finite() {
            return None;
        }
        worlds[index] = world;
    }
    Some(worlds)
}

/// Resolve the named clip on `assets` and evaluate bones for `hlod`.
///
/// Uses the instance root so a cached identity-root HLOD stays in
/// hierarchy space (same space as bind-pose `object.get_transform()`).
pub fn resolve_hlod_anim_applied_bones(
    assets: &AssetManager,
    hlod: &HlodInstance,
    animation_name: &str,
    animation_fraction: f32,
) -> Option<Vec<Mat4>> {
    if animation_name.trim().is_empty() {
        return None;
    }
    let hierarchy = assets.get_hierarchy_prototype(hlod.hierarchy_name())?;
    let animation = assets.get_prototype_as::<AnimationPrototype>(animation_name)?;
    let frame = fraction_to_hanim_frame(animation_fraction, animation.num_frames)?;
    evaluate_htree_anim_worlds(hierarchy, animation, frame, *hlod.transform())
}

/// Child world C++ would write with `HTree->Get_Transform(bone)`.
pub fn child_world_from_anim_bones(bone_index: i32, bone_worlds: &[Mat4]) -> Option<Mat4> {
    let index = usize::try_from(bone_index).ok()?;
    bone_worlds.get(index).copied().filter(Mat4::is_finite)
}

fn raw_frame_index(frame: f32, num_frames: u32) -> Option<i32> {
    if !frame.is_finite() || num_frames == 0 {
        return None;
    }
    let rounded = frame.round_ties_even();
    if rounded < i32::MIN as f32 || rounded > i32::MAX as f32 {
        return None;
    }
    let frame = rounded as i32;
    let num_frames = i32::try_from(num_frames).ok()?;
    if frame >= num_frames {
        Some(0)
    } else {
        Some(frame)
    }
}

fn sample_translation(animation: &AnimationPrototype, pivot: usize, frame: i32) -> Option<Vec3> {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;
    for channel in &animation.channels {
        if channel.pivot as usize != pivot {
            continue;
        }
        match channel.flags {
            ANIM_CHANNEL_X => x = scalar_at(channel, frame)?,
            ANIM_CHANNEL_Y => y = scalar_at(channel, frame)?,
            ANIM_CHANNEL_Z => z = scalar_at(channel, frame)?,
            _ => {}
        }
    }
    Some(Vec3::new(x, y, z))
}

fn sample_rotation(animation: &AnimationPrototype, pivot: usize, frame: i32) -> Option<Quat> {
    let mut rotation = Quat::IDENTITY;
    for channel in &animation.channels {
        if channel.pivot as usize == pivot && channel.flags == ANIM_CHANNEL_Q {
            rotation = quat_at(channel, frame)?;
        }
    }
    Some(rotation)
}

fn scalar_at(channel: &AnimationChannelData, frame: i32) -> Option<f32> {
    if channel.vector_len != 1 || channel.last_frame < channel.first_frame {
        return None;
    }
    let first = i32::from(channel.first_frame);
    let last = i32::from(channel.last_frame);
    if frame < first || frame > last {
        return Some(0.0);
    }
    let index = usize::try_from(frame - first).ok()?;
    channel.data.get(index).copied()
}

fn quat_at(channel: &AnimationChannelData, frame: i32) -> Option<Quat> {
    if channel.vector_len != 4 || channel.last_frame < channel.first_frame {
        return None;
    }
    let first = i32::from(channel.first_frame);
    let last = i32::from(channel.last_frame);
    if frame < first || frame > last {
        return Some(Quat::IDENTITY);
    }
    let index = usize::try_from(frame - first).ok()?.checked_mul(4)?;
    Some(Quat::from_xyzw(
        *channel.data.get(index)?,
        *channel.data.get(index + 1)?,
        *channel.data.get(index + 2)?,
        *channel.data.get(index + 3)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prototypes::AnimationChannelData;
    use ww3d_core::{W3dPivotStruct, W3dVectorStruct};

    fn pivot(name: &str, parent_idx: i32, translation: [f32; 3]) -> W3dPivotStruct {
        let mut name_bytes = [0u8; 16];
        let raw = name.as_bytes();
        let len = raw.len().min(16);
        name_bytes[..len].copy_from_slice(&raw[..len]);
        W3dPivotStruct {
            name: name_bytes,
            parent_idx,
            translation: W3dVectorStruct {
                x: translation[0],
                y: translation[1],
                z: translation[2],
            },
            euler_angles: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        }
    }

    fn hierarchy() -> HierarchyPrototype {
        let mut hierarchy = HierarchyPrototype::new("PoseHier".into());
        hierarchy.pivots = vec![
            pivot("ROOT", -1, [0.0, 0.0, 0.0]),
            pivot("BONE", 0, [2.0, 0.0, 0.0]),
        ];
        hierarchy.num_pivots = 2;
        hierarchy.recompute_bind_transforms();
        hierarchy
    }

    fn translation_anim(x0: f32, x1: f32) -> AnimationPrototype {
        let mut animation = AnimationPrototype::new("Idle".into(), "PoseHier".into());
        animation.num_frames = 3;
        animation.frame_rate = 30;
        animation.channels.push(AnimationChannelData {
            first_frame: 0,
            last_frame: 2,
            vector_len: 1,
            flags: ANIM_CHANNEL_X,
            pivot: 1,
            data: vec![x0, x1, x1 + 1.0],
        });
        animation
    }

    #[test]
    fn anim_applied_pose_differs_from_bind_and_freezes_at_frame() {
        let hierarchy = hierarchy();
        let animation = translation_anim(0.0, 4.0);
        let bind = hierarchy.bind_transforms[1];
        let frame_mid = fraction_to_hanim_frame(0.5, animation.num_frames).expect("frame");
        let first = evaluate_htree_anim_worlds(&hierarchy, &animation, frame_mid, Mat4::IDENTITY)
            .expect("pose");
        let again = evaluate_htree_anim_worlds(&hierarchy, &animation, frame_mid, Mat4::IDENTITY)
            .expect("frozen");
        assert_ne!(first[1], bind);
        assert_eq!(first[1], again[1]);
        let later =
            evaluate_htree_anim_worlds(&hierarchy, &animation, 2.0, Mat4::IDENTITY).expect("later");
        assert_ne!(first[1], later[1]);
        assert_eq!(hierarchy.bind_transforms[1], bind);
        assert_eq!(animation.channels[0].data[1], 4.0);
    }

    #[test]
    fn empty_channels_fail_closed() {
        let hierarchy = hierarchy();
        let animation = AnimationPrototype::new("Empty".into(), "PoseHier".into());
        assert!(evaluate_htree_anim_worlds(&hierarchy, &animation, 0.0, Mat4::IDENTITY).is_none());
    }
}
