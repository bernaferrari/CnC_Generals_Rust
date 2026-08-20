//! Sibling helpers for Device `W3DModelDraw` — keep the god-file from growing.
//!
//! C++ `W3DModelDraw.cpp`: projectile hide-show, muzzle one-frame flash,
//! one-shot setAnimationFrame, AnimationSpeedFactor, and shadow allocate/hide.

use crate::W3DDevice::GameClient::shadow::{
    the_w3d_shadow_manager, RenderObject, ShadowHandle, ShadowType, ShadowTypeInfo,
};
use crate::W3DDevice::GameClient::wthree_d_display::W3DDisplay;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;


/// C++ `doHideShowProjectileObjects`: numbered launch bones only when the
/// authored hide-show name is empty; otherwise a single mesh + hideCount.
pub fn projectile_clip_entries(
    hide_show_name: &str,
    launch_bone_name: &str,
    shots_remaining: u32,
    max_shots: u32,
) -> Vec<(String, bool)> {
    let hide_count = max_shots.saturating_sub(shots_remaining);
    if hide_show_name.is_empty() {
        if launch_bone_name.is_empty() {
            return Vec::new();
        }
        (0..max_shots)
            .map(|projectile_index| {
                (
                    format!("{}{:02}", launch_bone_name, projectile_index + 1),
                    (projectile_index + 1) <= hide_count,
                )
            })
            .collect()
    } else {
        vec![(hide_show_name.to_string(), hide_count > 0)]
    }
}

/// Apply state's HideShowVec then re-apply runtime overrides.
pub fn compose_hide_show(
    state_list: &[(String, bool)],
    overrides: &[(String, bool)],
) -> Vec<(String, bool)> {
    let mut composed = state_list.to_vec();
    for (name, hide) in overrides {
        if name.is_empty() {
            continue;
        }
        if let Some(existing) = composed
            .iter_mut()
            .find(|(entry_name, _)| entry_name.eq_ignore_ascii_case(name))
        {
            existing.1 = *hide;
        } else {
            composed.push((name.clone(), *hide));
        }
    }
    composed
}

/// Recoil states match Device `RecoilState` discriminants: Idle, RecoilStart, Recoil, Settle.
pub const RECOIL_IDLE: u8 = 0;
pub const RECOIL_START: u8 = 1;
pub const RECOIL_ACTIVE: u8 = 2;
pub const RECOIL_SETTLE: u8 = 3;

pub fn muzzle_should_hide(state: u8) -> bool {
    state != RECOIL_START
}

/// Advance one recoil barrel. Returns whether the muzzle should be hidden.
pub fn tick_recoil_barrel(
    state: &mut u8,
    shift: &mut f32,
    recoil_rate: &mut f32,
    has_recoil_bone: bool,
    max_recoil: f32,
    damping: f32,
    settle: f32,
) -> bool {
    let hide_muzzle = muzzle_should_hide(*state);
    if !has_recoil_bone {
        *state = RECOIL_IDLE;
        return hide_muzzle;
    }
    const TINY_RECOIL: f32 = 0.01;
    match *state {
        RECOIL_IDLE => {}
        RECOIL_START | RECOIL_ACTIVE => {
            *shift += *recoil_rate;
            *recoil_rate *= damping;
            if *shift >= max_recoil {
                *shift = max_recoil;
                *state = RECOIL_SETTLE;
            } else if recoil_rate.abs() < TINY_RECOIL {
                *state = RECOIL_SETTLE;
            } else {
                *state = RECOIL_ACTIVE;
            }
        }
        RECOIL_SETTLE => {
            *shift -= settle;
            if *shift <= 0.0 {
                *shift = 0.0;
                *state = RECOIL_IDLE;
            }
        }
        _ => {}
    }
    hide_muzzle
}

pub fn duration_frame_rate_multiplier(natural_ms: f32, desired_ms: f32) -> f32 {
    if natural_ms > 0.0 && desired_ms > 0.0 {
        natural_ms / desired_ms
    } else {
        1.0
    }
}

pub fn pick_anim_speed_factor(min_factor: f32, max_factor: f32) -> f32 {
    if min_factor <= max_factor && min_factor.is_finite() && max_factor.is_finite() {
        // Deterministic mid-point until GameClientRandomValueReal is wired here.
        ((min_factor + max_factor) * 0.5).max(0.0)
    } else {
        1.0
    }
}

pub fn shadow_should_render(hidden: bool, shadow_enabled: bool) -> bool {
    !hidden && shadow_enabled
}

pub fn allocate_projected_shadow(
    existing: &Option<ShadowHandle>,
    render_object_id: Option<RenderObjectId>,
    hidden: bool,
    shadow_enabled: bool,
    fully_obscured: bool,
    size_x: f32,
    size_y: f32,
    texture: &str,
) -> Option<ShadowHandle> {
    if existing.is_some() || render_object_id.is_none() {
        return existing.clone();
    }
    let info = ShadowTypeInfo {
        shadow_type: ShadowType::DECAL,
        allow_updates: false,
        allow_world_align: true,
        shadow_name: if texture.is_empty() {
            "shadow".to_string()
        } else {
            texture.to_string()
        },
        size_x: size_x.max(8.0),
        size_y: size_y.max(8.0),
        offset_x: 0.0,
        offset_y: 0.0,
    };
    let robj = RenderObject::default();
    let mut handle = the_w3d_shadow_manager()
        .write()
        .add_shadow(&robj, Some(&info))?;
    handle.is_enabled = shadow_should_render(hidden, shadow_enabled);
    handle.is_invisible_enabled = fully_obscured;
    Some(handle)
}

pub fn release_projected_shadow(handle: Option<ShadowHandle>, scene_id: Option<RenderObjectId>) {
    if let Some(handle) = handle {
        the_w3d_shadow_manager().write().remove_shadow(&handle);
    }
    if let Some(id) = scene_id {
        let scene = W3DDisplay::global_scene();
        scene.write().remove_render_object(id);
    }
}

pub fn apply_shadow_render_enabled(handle: &mut Option<ShadowHandle>, enabled: bool) {
    if let Some(handle) = handle.as_mut() {
        handle.is_enabled = enabled;
    }
}
