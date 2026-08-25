//! Renderer-owned HLOD live-child capture for fog ghosts.
//!
//! C++ `W3DGhostObject.cpp:64-128`:
//! - `update` Clone()s the live RenderObj; child vis/transforms are whatever
//!   the live hierarchy currently shows after this frame's anim/bone apply.
//! - HLOD anim is frozen via Peek_Animation_And_Info / Set_Animation(frame)
//!   (mode/mult discarded → MANUAL).
//! - `disableUVAnimations` walks live HLOD children: MESH +
//!   `MAPPER_ID_LINEAR_OFFSET` → UV pin; name contains `"MUZZLEFX"` → hidden.
//!
//! This adapter walks a live `HlodInstance` read-only. UV-disable is recorded
//! on the captured state only — shared prototypes are never mutated.

use crate::render_bridge::{BoneOverride, DrawSubmission, SubObjectVisibility};
use gamelogic::object::draw::w3d_model_draw::{HlodGhostChildCapturePath, HlodLiveChildState};
use gamelogic::object::w3d_ghost_object::{Matrix3x4, RenderSubObjectSnapshot};
use ww3d_assets::AssetManager;
use ww3d_assets::hlod_anim_pose::{child_world_from_anim_bones, resolve_hlod_anim_applied_bones};
use ww3d_assets::prototypes::{HlodInstance, MeshPrototype};

/// C++ `TextureMapperClass::MAPPER_ID_LINEAR_OFFSET` (`mapper.h:29`).
pub const MAPPER_ID_LINEAR_OFFSET: u32 = 1;

pub fn child_name_is_muzzle_fx(name: &str) -> bool {
    name.contains("MUZZLEFX")
}

pub fn mesh_has_linear_offset_mapper(assets: &AssetManager, child_name: &str) -> bool {
    let Some(mesh) = assets.get_prototype_as::<MeshPrototype>(child_name) else {
        return false;
    };
    mesh.vertex_mapper_configs.iter().any(|config| {
        mapper_is_linear_offset(config.stage0.as_ref())
            || mapper_is_linear_offset(config.stage1.as_ref())
    })
}

fn mapper_is_linear_offset(mapper: Option<&ww3d_assets::prototypes::MapperDefinition>) -> bool {
    mapper.is_some_and(|definition| definition.mapper_type == MAPPER_ID_LINEAR_OFFSET)
}

/// Snapshot live per-child name / visibility / transform / UV-disable.
///
/// Bone overrides stay a read-time overlay (C++ `Control_Bone` after
/// `Anim_Update`). Children missing from `bone_overrides` use the scratch
/// HAnim pose at `animation_time` when that clip can be resolved; otherwise
/// the instance current transform (bind pose after propagate). Shared
/// prototypes are never mutated.
pub fn capture_hlod_live_child_states(
    hlod: &HlodInstance,
    assets: &AssetManager,
    bone_overrides: &[BoneOverride],
    visibility: &[SubObjectVisibility],
    animation_name: Option<&str>,
    animation_time: f32,
) -> Option<Vec<HlodLiveChildState>> {
    let animation_requested = animation_name.is_some() || animation_time != 0.0;
    let anim_bones = animation_name
        .and_then(|name| resolve_hlod_anim_applied_bones(assets, hlod, name, animation_time));
    if animation_requested && anim_bones.is_none() && bone_overrides.is_empty() {
        return None;
    }
    let lod = hlod.current_lod()?;
    let extra = hlod
        .aggregates()
        .iter()
        .flat_map(|aggregate| aggregate.models().iter());
    let mut states = Vec::new();
    for model in lod.models().iter().chain(extra) {
        if model.name.trim().is_empty() {
            return None;
        }
        let object = model.object.as_deref()?;
        let mut transform = *object.get_transform();
        if let Some(bone) = bone_overrides
            .iter()
            .find(|bone| bone.bone_index == model.bone_index)
        {
            if !bone.transform.is_finite() {
                return None;
            }
            transform = bone.transform;
        } else if let Some(bones) = anim_bones.as_deref() {
            transform = child_world_from_anim_bones(model.bone_index, bones)?;
        } else if animation_requested {
            return None;
        }
        if !transform.is_finite() {
            return None;
        }
        let hidden_by_submission = visibility
            .iter()
            .any(|entry| entry.hidden && entry.sub_object_name.eq_ignore_ascii_case(&model.name));
        states.push(HlodLiveChildState {
            name: model.name.clone(),
            hidden: hidden_by_submission || child_name_is_muzzle_fx(&model.name),
            local_transform: transform,
            uv_animations_disabled: mesh_has_linear_offset_mapper(assets, &model.name),
        });
    }
    Some(states)
}

/// Prefer a live hierarchy walk (HAnim freeze + bone overlay); fall back to
/// bone-override reconstruction when the scratch pose cannot be resolved.
pub fn materialize_hlod_ghost_children(
    hlod: &HlodInstance,
    submission: &DrawSubmission,
    assets: &AssetManager,
) -> Option<(Vec<RenderSubObjectSnapshot>, HlodGhostChildCapturePath)> {
    if let Some(live) = capture_hlod_live_child_states(
        hlod,
        assets,
        &submission.bone_overrides,
        &submission.sub_object_visibility,
        submission.animation_name.as_deref(),
        submission.animation_time,
    ) {
        let mut sub_objects = Vec::with_capacity(live.len());
        for child in live {
            if child.name.trim().is_empty() || !child.local_transform.is_finite() {
                return None;
            }
            sub_objects.push(RenderSubObjectSnapshot {
                name: child.name.clone(),
                visible: !child.hidden,
                transform: Matrix3x4::from_logic_matrix(child.local_transform),
            });
        }
        log::debug!(
            "hlod ghost child capture path=LiveHierarchy model={} children={}",
            submission.model_name,
            sub_objects.len()
        );
        return Some((sub_objects, HlodGhostChildCapturePath::LiveHierarchy));
    }

    let reconstructed = reconstruct_hlod_children_from_submission(hlod, submission)?;
    log::debug!(
        "hlod ghost child capture path=BoneOverridesFallback model={} children={}",
        submission.model_name,
        reconstructed.len()
    );
    Some((
        reconstructed,
        HlodGhostChildCapturePath::BoneOverridesFallback,
    ))
}

fn reconstruct_hlod_children_from_submission(
    hlod: &HlodInstance,
    submission: &DrawSubmission,
) -> Option<Vec<RenderSubObjectSnapshot>> {
    let animation_requested =
        submission.animation_name.is_some() || submission.animation_time != 0.0;
    if animation_requested && submission.bone_overrides.is_empty() {
        return None;
    }

    let lod = hlod.current_lod()?;
    let extra = hlod
        .aggregates()
        .iter()
        .flat_map(|aggregate| aggregate.models().iter());
    let mut sub_objects = Vec::new();
    for model in lod.models().iter().chain(extra) {
        if model.name.trim().is_empty() {
            return None;
        }
        let object = model.object.as_deref()?;
        let mut transform = *object.get_transform();
        if let Some(bone) = submission
            .bone_overrides
            .iter()
            .find(|bone| bone.bone_index == model.bone_index)
        {
            if !bone.transform.is_finite() {
                return None;
            }
            transform = bone.transform;
        } else if animation_requested {
            return None;
        }
        if !transform.is_finite() {
            return None;
        }
        let hidden = submission.sub_object_visibility.iter().any(|visibility| {
            visibility.hidden && visibility.sub_object_name.eq_ignore_ascii_case(&model.name)
        });
        sub_objects.push(RenderSubObjectSnapshot {
            name: model.name.clone(),
            visible: !hidden,
            transform: Matrix3x4::from_logic_matrix(transform),
        });
    }
    Some(sub_objects)
}
