//! W3D C API materials, combiners, and fixed-function tinting.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::leftover::*;
use super::lighting::*;
use super::textures::*;
use super::types::*;
use crate::w3d::renderer::{batch_material_params, batch_priority};
use crate::w3d::w3d_device::RenderObject;
use crate::w3d::{
    Camera, Light, Material, Mesh, Result, Texture, W3DConfig, W3DDevice, W3DError, W3DLightData,
    W3DMaterialData, W3DVertex,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::ffi::{CStr, CString, c_char, c_void};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::RwLock;

/// Set material - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry. `device` must be a live W3D_DEVICE; `material_data`
// SAFETY: must be readable for one W3DMaterialData (null clears binding). The
// SAFETY: struct is copied before any await point.
pub unsafe extern "C" fn W3DDevice_SetMaterial(
    device: W3D_DEVICE,
    material_data: *const W3DMaterialData,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if material_data.is_null() {
        if let Ok(mut current_material) = device_ref.current_material_id.lock() {
            *current_material = None;
        }
        if let Ok(mut current_material_data) = device_ref.current_material_data.lock() {
            *current_material_data = None;
        }
        return 1;
    }
    if !is_valid_ptr(material_data) {
        return 0;
    }

    let material_data = *material_data;
    let material_id = next_material_id(device_ref);
    let material = c_material_data_to_material(&material_id, material_data);
    let material_id_for_state = material_id.clone();

    match device_ref
        .runtime
        .block_on(async { set_material_internal(&device_ref.device, material).await })
    {
        Ok(_) => {
            if let Ok(mut current_material) = device_ref.current_material_id.lock() {
                *current_material = Some(material_id_for_state);
            }
            if let Ok(mut current_material_data) = device_ref.current_material_data.lock() {
                *current_material_data = Some(material_data);
            }
            1
        }
        Err(_) => 0,
    }
}

/// Get currently bound material - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry. `out_material_data` must be writable for one
// SAFETY: W3DMaterialData when non-null; `device` must be a live W3D_DEVICE.
pub unsafe extern "C" fn W3DDevice_GetMaterial(
    device: W3D_DEVICE,
    out_material_data: *mut W3DMaterialData,
) -> i32 {
    if device.is_null() || out_material_data.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(current) = device_ref.current_material_data.lock() {
        if let Some(material_data) = *current {
            *out_material_data = material_data;
            return 1;
        }
    }

    let Some(material_id) = current_material_id(device_ref) else {
        return 0;
    };
    let material = device_ref
        .runtime
        .block_on(async { get_material_internal(&device_ref.device, &material_id).await });
    if let Some(material) = material {
        *out_material_data = material_to_c_data(&material);
        return 1;
    }

    0
}
pub(super) async fn get_material_internal(
    device: &Arc<RwLock<W3DDevice>>,
    material_id: &str,
) -> Option<Material> {
    let device_lock = device.read().await;
    device_lock.get_material(material_id).await
}

pub(super) async fn set_material_internal(
    device: &Arc<RwLock<W3DDevice>>,
    material: Material,
) -> Result<()> {
    let device_lock = device.read().await;
    device_lock.add_material(material).await?;
    Ok(())
}

pub(super) async fn ensure_bound_material_internal(
    device: &Arc<RwLock<W3DDevice>>,
    base_material_id: Option<&str>,
    texture_id: Option<&str>,
    detail_texture_id: Option<&str>,
    detail_blend_mode: u8,
    bound_material_id: &str,
    tint_rgba: [f32; 4],
    lighting_state: FixedFunctionLightingState,
    surface_state: FixedFunctionSurfaceState,
) -> Result<()> {
    let device_lock = device.read().await;
    if device_lock.get_material(bound_material_id).await.is_some() {
        return Ok(());
    }
    if let Some(texture_id) = texture_id {
        if device_lock.get_texture(texture_id).await.is_none() {
            return Err(W3DError::ResourceLoadingFailed(format!(
                "Texture not found for material binding: {texture_id}"
            )));
        }
    }

    let mut material = if let Some(base_material_id) = base_material_id {
        device_lock
            .get_material(base_material_id)
            .await
            .unwrap_or_else(|| default_material(bound_material_id))
    } else {
        default_material(bound_material_id)
    };

    material.id = bound_material_id.to_string();
    material.name = bound_material_id.to_string();
    material.diffuse_texture = texture_id.map(str::to_string);
    material.detail_texture = detail_texture_id.map(str::to_string);
    material.detail_blend_mode = detail_blend_mode;
    material.properties.diffuse_color = multiply_rgba(material.properties.diffuse_color, tint_rgba);
    material.properties.transparent =
        material.properties.transparent || material.properties.diffuse_color[3] < 0.999;
    apply_fixed_function_lighting_to_material(&mut material, texture_id.is_some(), lighting_state);
    apply_fixed_function_surface_to_material(&mut material, surface_state);

    device_lock.add_material(material).await?;
    Ok(())
}

pub(super) fn arg_color_from_texture_factor(arg: u32, texture_factor: u32) -> [f32; 4] {
    let alpha = ((texture_factor >> 24) & 0xFF) as f32 / 255.0;
    let red = ((texture_factor >> 16) & 0xFF) as f32 / 255.0;
    let green = ((texture_factor >> 8) & 0xFF) as f32 / 255.0;
    let blue = (texture_factor & 0xFF) as f32 / 255.0;
    apply_arg_modifiers_to_color(arg, [red, green, blue, alpha])
}

pub(super) fn apply_arg_modifiers_to_color(arg: u32, base_color: [f32; 4]) -> [f32; 4] {
    let alpha = base_color[3];
    let mut color = if (arg & D3DTA_ALPHAREPLICATE) != 0 {
        [alpha, alpha, alpha, alpha]
    } else {
        base_color
    };

    if (arg & D3DTA_COMPLEMENT) != 0 {
        for component in &mut color {
            *component = 1.0 - *component;
        }
    }

    color
}

pub(super) fn multiply_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        lhs[0] * rhs[0],
        lhs[1] * rhs[1],
        lhs[2] * rhs[2],
        lhs[3] * rhs[3],
    ]
}

pub(super) fn add_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        (lhs[0] + rhs[0]).clamp(0.0, 1.0),
        (lhs[1] + rhs[1]).clamp(0.0, 1.0),
        (lhs[2] + rhs[2]).clamp(0.0, 1.0),
        (lhs[3] + rhs[3]).clamp(0.0, 1.0),
    ]
}

pub(super) fn subtract_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        (lhs[0] - rhs[0]).clamp(0.0, 1.0),
        (lhs[1] - rhs[1]).clamp(0.0, 1.0),
        (lhs[2] - rhs[2]).clamp(0.0, 1.0),
        (lhs[3] - rhs[3]).clamp(0.0, 1.0),
    ]
}

pub(super) fn addsigned_rgba(lhs: [f32; 4], rhs: [f32; 4], scale: f32) -> [f32; 4] {
    [
        ((lhs[0] + rhs[0] - 0.5) * scale).clamp(0.0, 1.0),
        ((lhs[1] + rhs[1] - 0.5) * scale).clamp(0.0, 1.0),
        ((lhs[2] + rhs[2] - 0.5) * scale).clamp(0.0, 1.0),
        ((lhs[3] + rhs[3] - 0.5) * scale).clamp(0.0, 1.0),
    ]
}

pub(super) fn addsmooth_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        (lhs[0] + rhs[0] * (1.0 - lhs[0])).clamp(0.0, 1.0),
        (lhs[1] + rhs[1] * (1.0 - lhs[1])).clamp(0.0, 1.0),
        (lhs[2] + rhs[2] * (1.0 - lhs[2])).clamp(0.0, 1.0),
        (lhs[3] + rhs[3] * (1.0 - lhs[3])).clamp(0.0, 1.0),
    ]
}

pub(super) fn scale_rgb_add_rgba(base: [f32; 4], added: [f32; 4], factor: [f32; 4]) -> [f32; 4] {
    [
        (base[0] + added[0] * factor[0]).clamp(0.0, 1.0),
        (base[1] + added[1] * factor[1]).clamp(0.0, 1.0),
        (base[2] + added[2] * factor[2]).clamp(0.0, 1.0),
        base[3],
    ]
}

pub(super) fn lerp_rgba(lhs: [f32; 4], rhs: [f32; 4], factor: f32) -> [f32; 4] {
    let t = factor.clamp(0.0, 1.0);
    [
        lhs[0] * t + rhs[0] * (1.0 - t),
        lhs[1] * t + rhs[1] * (1.0 - t),
        lhs[2] * t + rhs[2] * (1.0 - t),
        lhs[3] * t + rhs[3] * (1.0 - t),
    ]
}

pub(super) fn lerp_rgba_per_channel(factor: [f32; 4], lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        lhs[0] * factor[0].clamp(0.0, 1.0) + rhs[0] * (1.0 - factor[0].clamp(0.0, 1.0)),
        lhs[1] * factor[1].clamp(0.0, 1.0) + rhs[1] * (1.0 - factor[1].clamp(0.0, 1.0)),
        lhs[2] * factor[2].clamp(0.0, 1.0) + rhs[2] * (1.0 - factor[2].clamp(0.0, 1.0)),
        lhs[3] * factor[3].clamp(0.0, 1.0) + rhs[3] * (1.0 - factor[3].clamp(0.0, 1.0)),
    ]
}

pub(super) fn dotproduct3_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    let lx = lhs[0] * 2.0 - 1.0;
    let ly = lhs[1] * 2.0 - 1.0;
    let lz = lhs[2] * 2.0 - 1.0;
    let rx = rhs[0] * 2.0 - 1.0;
    let ry = rhs[1] * 2.0 - 1.0;
    let rz = rhs[2] * 2.0 - 1.0;
    let scalar = ((lx * rx + ly * ry + lz * rz) + 1.0) * 0.5;
    let clamped = scalar.clamp(0.0, 1.0);
    [clamped, clamped, clamped, clamped]
}

pub(super) fn pack_rgba8(color: [f32; 4]) -> [u8; 4] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

pub(super) fn render_state_value(device: &W3DDeviceC, state: W3D_RENDER_STATE) -> u32 {
    if let Ok(states) = device.render_states.lock() {
        return states
            .get(&state)
            .copied()
            .unwrap_or_else(|| default_render_state_value(state));
    }

    default_render_state_value(state)
}

pub(super) fn apply_fixed_function_lighting_to_material(
    material: &mut Material,
    has_texture: bool,
    state: FixedFunctionLightingState,
) {
    material.properties.unlit = !state.lighting_enabled;

    let diffuse_rgb = [
        material.properties.diffuse_color[0],
        material.properties.diffuse_color[1],
        material.properties.diffuse_color[2],
    ];

    if !state.specular_enabled
        || !material_source_uses_material(state.specular_material_source, state.color_vertex)
    {
        material.properties.specular_color = [0.0, 0.0, 0.0];
        material.properties.shininess = 0.0;
    }

    if !material_source_uses_material(state.emissive_material_source, state.color_vertex) {
        material.properties.emissive_color = [0.0, 0.0, 0.0];
    }

    if state.ambient_argb != 0
        && material_source_uses_material(state.ambient_material_source, state.color_vertex)
    {
        let ambient = decode_argb_color(state.ambient_argb);
        material.properties.emissive_color = add_rgb(
            material.properties.emissive_color,
            multiply_rgb(diffuse_rgb, [ambient[0], ambient[1], ambient[2]]),
        );
    }

    if !state.lighting_enabled {
        material.properties.specular_color = [0.0, 0.0, 0.0];
        material.properties.shininess = 0.0;

        if !has_texture {
            material.properties.emissive_color =
                add_rgb(material.properties.emissive_color, diffuse_rgb);
        }
    }
}

pub(super) fn apply_fixed_function_surface_to_material(
    material: &mut Material,
    state: FixedFunctionSurfaceState,
) {
    material.properties.alpha_test = state.alpha_test_enabled;
    material.properties.alpha_cutoff = if state.alpha_test_enabled {
        state.alpha_ref as f32 / 255.0
    } else {
        0.0
    };
    material.properties.double_sided = state.cull_mode == D3DCULL_NONE;
    material.properties.transparent = material.properties.transparent || state.alpha_blend_enabled;
}

pub(super) fn simple_stage_tfactor_tint_with<F>(
    stage_state_lookup: &mut F,
    stage: u32,
    texture_factor: u32,
) -> Option<[f32; 4]>
where
    F: FnMut(u32, u32) -> u32,
{
    simple_stage_tint_from_current_with(
        stage_state_lookup,
        stage,
        [1.0, 1.0, 1.0, 1.0],
        texture_factor,
    )
}

pub(super) fn simple_stage_chain_tint_with<F>(
    stage_state_lookup: &mut F,
    last_stage: u32,
    texture_factor: u32,
) -> Option<[f32; 4]>
where
    F: FnMut(u32, u32) -> u32,
{
    let mut current = [1.0, 1.0, 1.0, 1.0];
    let mut used = false;

    for stage in 0..=last_stage {
        if !texture_stage_enabled_with(stage_state_lookup, stage) {
            continue;
        }

        if let Some(next) =
            simple_stage_tint_from_current_with(stage_state_lookup, stage, current, texture_factor)
        {
            current = next;
            used = true;
        }
    }

    used.then_some(current)
}

pub(super) fn simple_stage_tint_from_current_with<F>(
    stage_state_lookup: &mut F,
    stage: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<[f32; 4]>
where
    F: FnMut(u32, u32) -> u32,
{
    let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
    let color_arg0 = stage_state_lookup(stage, D3DTSS_COLORARG0);
    let color_arg1 = stage_state_lookup(stage, D3DTSS_COLORARG1);
    let color_arg2 = stage_state_lookup(stage, D3DTSS_COLORARG2);
    let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
    let alpha_arg0 = stage_state_lookup(stage, D3DTSS_ALPHAARG0);
    let alpha_arg1 = stage_state_lookup(stage, D3DTSS_ALPHAARG1);
    let alpha_arg2 = stage_state_lookup(stage, D3DTSS_ALPHAARG2);

    let mut out = current_tint;
    let mut used = false;

    if color_op != D3DTOP_DISABLE {
        if let Some(color_tint) = simple_material_tint_color_for_op(
            color_op,
            color_arg0,
            color_arg1,
            color_arg2,
            current_tint,
            texture_factor,
        ) {
            out[0] = color_tint[0];
            out[1] = color_tint[1];
            out[2] = color_tint[2];
            used = true;
        }
    }

    if alpha_op != D3DTOP_DISABLE {
        if let Some(alpha_tint) = simple_material_tint_alpha_for_op(
            alpha_op,
            alpha_arg0,
            alpha_arg1,
            alpha_arg2,
            current_tint,
            texture_factor,
        ) {
            out[3] = alpha_tint;
            used = true;
        }
    }

    used.then_some(out)
}

pub(super) fn simple_material_tint_arg_value(
    arg: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<[f32; 4]> {
    match arg & D3DTA_SELECTMASK {
        D3DTA_TFACTOR => Some(arg_color_from_texture_factor(arg, texture_factor)),
        D3DTA_CURRENT => Some(apply_arg_modifiers_to_color(arg, current_tint)),
        D3DTA_TEXTURE | D3DTA_DIFFUSE => {
            Some(apply_arg_modifiers_to_color(arg, [1.0, 1.0, 1.0, 1.0]))
        }
        _ => None,
    }
}

pub(super) fn simple_material_tint_color_for_op(
    op: u32,
    arg0: u32,
    arg1: u32,
    arg2: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<[f32; 4]> {
    match op {
        D3DTOP_PREMODULATE | D3DTOP_BUMPENVMAP | D3DTOP_BUMPENVMAPLUMINANCE => Some(current_tint),
        D3DTOP_SELECTARG1 => simple_material_tint_arg_value(arg1, current_tint, texture_factor),
        D3DTOP_SELECTARG2 => simple_material_tint_arg_value(arg2, current_tint, texture_factor),
        D3DTOP_MODULATE | D3DTOP_MODULATE2X | D3DTOP_MODULATE4X => {
            let mut tint = multiply_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            );

            let scale = match op {
                D3DTOP_MODULATE2X => 2.0,
                D3DTOP_MODULATE4X => 4.0,
                _ => 1.0,
            };
            Some([
                (tint[0] * scale).clamp(0.0, 1.0),
                (tint[1] * scale).clamp(0.0, 1.0),
                (tint[2] * scale).clamp(0.0, 1.0),
                tint[3],
            ])
        }
        D3DTOP_ADD => Some(add_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_ADDSIGNED => Some(addsigned_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            1.0,
        )),
        D3DTOP_ADDSIGNED2X => Some(addsigned_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            2.0,
        )),
        D3DTOP_SUBTRACT => Some(subtract_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_ADDSMOOTH => Some(addsmooth_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_BLENDDIFFUSEALPHA | D3DTOP_BLENDTEXTUREALPHA | D3DTOP_BLENDTEXTUREALPHAPM => {
            // The constrained fallback evaluator treats diffuse/texture as neutral white sources,
            // so their alpha factor resolves to 1.0 in this approximation and these ops collapse to arg1.
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)
        }
        D3DTOP_MODULATEALPHA_ADDCOLOR => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            Some(scale_rgb_add_rgba(lhs, rhs, [lhs[3], lhs[3], lhs[3], 1.0]))
        }
        D3DTOP_MODULATECOLOR_ADDALPHA => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            Some([
                (lhs[0] * rhs[0] + lhs[3]).clamp(0.0, 1.0),
                (lhs[1] * rhs[1] + lhs[3]).clamp(0.0, 1.0),
                (lhs[2] * rhs[2] + lhs[3]).clamp(0.0, 1.0),
                lhs[3],
            ])
        }
        D3DTOP_MODULATEINVALPHA_ADDCOLOR => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            let inv_alpha = 1.0 - lhs[3];
            Some(scale_rgb_add_rgba(
                lhs,
                rhs,
                [inv_alpha, inv_alpha, inv_alpha, 1.0],
            ))
        }
        D3DTOP_MODULATEINVCOLOR_ADDALPHA => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            Some([
                ((1.0 - lhs[0]) * rhs[0] + lhs[3]).clamp(0.0, 1.0),
                ((1.0 - lhs[1]) * rhs[1] + lhs[3]).clamp(0.0, 1.0),
                ((1.0 - lhs[2]) * rhs[2] + lhs[3]).clamp(0.0, 1.0),
                lhs[3],
            ])
        }
        D3DTOP_BLENDFACTORALPHA => Some(lerp_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            arg_color_from_texture_factor(D3DTA_TFACTOR, texture_factor)[3],
        )),
        D3DTOP_BLENDCURRENTALPHA => Some(lerp_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            current_tint[3],
        )),
        D3DTOP_MULTIPLYADD => Some(add_rgba(
            multiply_rgba(
                simple_material_tint_arg_value(arg0, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            ),
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_LERP => Some(lerp_rgba_per_channel(
            simple_material_tint_arg_value(arg0, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_DOTPRODUCT3 => Some(dotproduct3_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        _ => None,
    }
}

pub(super) fn simple_material_tint_alpha_for_op(
    op: u32,
    arg0: u32,
    arg1: u32,
    arg2: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<f32> {
    match op {
        D3DTOP_PREMODULATE | D3DTOP_BUMPENVMAP | D3DTOP_BUMPENVMAPLUMINANCE => {
            Some(current_tint[3])
        }
        D3DTOP_SELECTARG1 => {
            Some(simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3])
        }
        D3DTOP_SELECTARG2 => {
            Some(simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
        }
        D3DTOP_MODULATE | D3DTOP_MODULATE2X | D3DTOP_MODULATE4X => {
            let mut tint = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                * simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3];

            let scale = match op {
                D3DTOP_MODULATE2X => 2.0,
                D3DTOP_MODULATE4X => 4.0,
                _ => 1.0,
            };
            Some((tint * scale).clamp(0.0, 1.0))
        }
        D3DTOP_ADD => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
                .clamp(0.0, 1.0),
        ),
        D3DTOP_ADDSIGNED => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3]
                - 0.5)
                .clamp(0.0, 1.0),
        ),
        D3DTOP_ADDSIGNED2X => Some(
            ((simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3]
                - 0.5)
                * 2.0)
                .clamp(0.0, 1.0),
        ),
        D3DTOP_SUBTRACT => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                - simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
                .clamp(0.0, 1.0),
        ),
        D3DTOP_ADDSMOOTH => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3]
                    * (1.0
                        - simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]))
                .clamp(0.0, 1.0),
        ),
        D3DTOP_BLENDDIFFUSEALPHA | D3DTOP_BLENDTEXTUREALPHA | D3DTOP_BLENDTEXTUREALPHAPM => {
            Some(simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3])
        }
        D3DTOP_MODULATEALPHA_ADDCOLOR
        | D3DTOP_MODULATECOLOR_ADDALPHA
        | D3DTOP_MODULATEINVALPHA_ADDCOLOR
        | D3DTOP_MODULATEINVCOLOR_ADDALPHA => {
            Some(simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3])
        }
        D3DTOP_BLENDFACTORALPHA => Some(
            lerp_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
                arg_color_from_texture_factor(D3DTA_TFACTOR, texture_factor)[3],
            )[3],
        ),
        D3DTOP_BLENDCURRENTALPHA => Some(
            lerp_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
                current_tint[3],
            )[3],
        ),
        D3DTOP_MULTIPLYADD => Some(
            (simple_material_tint_arg_value(arg0, current_tint, texture_factor)?[3]
                * simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
                .clamp(0.0, 1.0),
        ),
        D3DTOP_LERP => Some(
            lerp_rgba_per_channel(
                simple_material_tint_arg_value(arg0, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            )[3],
        ),
        D3DTOP_DOTPRODUCT3 => Some(
            dotproduct3_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            )[3],
        ),
        _ => None,
    }
}
pub(super) fn resolve_draw_material_id(device: &W3DDeviceC, texture_stage: u32) -> Option<String> {
    let base_material_id = current_material_id(device);
    let active_texture_id = if let Ok(bindings) = device.bound_textures.lock() {
        bindings.get(&texture_stage).cloned()
    } else {
        None
    };
    let lighting_state = current_fixed_function_lighting_state(device);
    let surface_state = current_fixed_function_surface_state(device);
    let combiner_signature = material_combiner_signature_with(
        &mut |stage, state| stage_texture_state_value(device, stage, state),
        8,
    );
    let multi_texture_chain = combiner_signature.sampling_stage_count > 1;

    // Resolve detail (Stage 1) texture and blend mode for multi-texture chains.
    let (detail_texture_id, detail_blend_mode) = if multi_texture_chain {
        let detail_id = resolve_detail_texture_id(device);
        let stage1_color_op = stage_texture_state_value(device, 1, D3DTSS_COLOROP);
        let blend = detail_blend_mode_from_color_op(stage1_color_op);
        (detail_id, blend)
    } else {
        (None, 0)
    };

    let texture_factor = render_state_value(device, W3D_RENDER_STATE::W3DRS_TEXTUREFACTOR);
    let tint_rgba = if active_texture_id.is_some() {
        if multi_texture_chain {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            simple_stage_chain_tint_with(
                &mut |stage, state| stage_texture_state_value(device, stage, state),
                texture_stage,
                texture_factor,
            )
            .unwrap_or([1.0, 1.0, 1.0, 1.0])
        }
    } else if let Some(stage) = first_enabled_texture_stage_with(
        &mut |stage, state| stage_texture_state_value(device, stage, state),
        8,
    ) {
        if multi_texture_chain {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            match simple_stage_chain_tint_with(
                &mut |lookup_stage, state| stage_texture_state_value(device, lookup_stage, state),
                stage,
                texture_factor,
            ) {
                Some(tint_rgba) => tint_rgba,
                None => {
                    if lighting_state_requires_material_variant(lighting_state)
                        || surface_state_requires_material_variant(surface_state)
                    {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        return base_material_id;
                    }
                }
            }
        }
    } else {
        if lighting_state_requires_material_variant(lighting_state)
            || surface_state_requires_material_variant(surface_state)
        {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            return base_material_id;
        }
    };

    let effective_texture_id = effective_bound_texture_id(
        base_material_id.is_some(),
        multi_texture_chain,
        active_texture_id.clone(),
    );
    let texture_cache_id = effective_texture_id.clone().unwrap_or_default();
    let detail_cache_id = detail_texture_id.clone().unwrap_or_default();
    let cache_key = MaterialBindingCacheKey {
        base_material_id: base_material_id.clone(),
        texture_id: texture_cache_id.clone(),
        tint_rgba: pack_rgba8(tint_rgba),
        combiner_signature,
        lighting_state,
        surface_state,
    };
    if let Ok(material_bindings) = device.material_texture_bindings.lock() {
        if let Some(bound_material_id) = material_bindings.get(&cache_key) {
            return Some(bound_material_id.clone());
        }
    }

    let bound_material_id = material_binding_id(
        base_material_id.as_deref(),
        &texture_cache_id,
        &detail_cache_id,
        cache_key.tint_rgba,
        combiner_signature,
        lighting_state,
        surface_state,
    );
    if device
        .runtime
        .block_on(async {
            ensure_bound_material_internal(
                &device.device,
                base_material_id.as_deref(),
                effective_texture_id.as_deref(),
                detail_texture_id.as_deref(),
                detail_blend_mode,
                &bound_material_id,
                tint_rgba,
                lighting_state,
                surface_state,
            )
            .await
        })
        .is_err()
    {
        return base_material_id;
    }

    if let Ok(mut material_bindings) = device.material_texture_bindings.lock() {
        material_bindings.insert(cache_key, bound_material_id.clone());
    }
    Some(bound_material_id)
}

pub(super) fn effective_bound_texture_id(
    _has_base_material: bool,
    _multi_texture_chain: bool,
    active_texture_id: Option<String>,
) -> Option<String> {
    // Always return the primary (Stage 0) texture ID.
    // The secondary (Stage 1) texture is resolved separately for multi-texture chains.
    active_texture_id
}

/// Resolve the Stage 1 (detail) texture bound to a multi-texture chain.
pub(super) fn resolve_detail_texture_id(device: &W3DDeviceC) -> Option<String> {
    if let Ok(bindings) = device.bound_textures.lock() {
        bindings.get(&1u32).cloned()
    } else {
        None
    }
}

/// Map a D3DTOP color operation to our detail blend mode enum.
/// Returns: 0=off, 1=MODULATE, 2=ADDSIGNED, 3=BLENDCURRENTALPHA.
pub(super) fn detail_blend_mode_from_color_op(color_op: u32) -> u8 {
    match color_op {
        D3DTOP_MODULATE | D3DTOP_MODULATE2X | D3DTOP_MODULATE4X => 1,
        D3DTOP_ADDSIGNED | D3DTOP_ADDSIGNED2X => 2,
        D3DTOP_BLENDCURRENTALPHA => 3,
        D3DTOP_ADD | D3DTOP_ADDSMOOTH => 2, // Approximate ADD as ADDSIGNED
        _ => 1,                             // Default to MODULATE for unknown ops
    }
}

pub(super) fn material_binding_id(
    base_material_id: Option<&str>,
    texture_id: &str,
    detail_texture_id: &str,
    tint_rgba: [u8; 4],
    combiner_signature: MaterialCombinerSignature,
    lighting_state: FixedFunctionLightingState,
    surface_state: FixedFunctionSurfaceState,
) -> String {
    let mut hasher = DefaultHasher::new();
    base_material_id.hash(&mut hasher);
    texture_id.hash(&mut hasher);
    detail_texture_id.hash(&mut hasher);
    tint_rgba.hash(&mut hasher);
    combiner_signature.hash(&mut hasher);
    lighting_state.hash(&mut hasher);
    surface_state.hash(&mut hasher);
    format!("__w3d_c_api_bound_material_{:016x}", hasher.finish())
}

pub(super) fn material_combiner_signature_with<F>(
    stage_state_lookup: &mut F,
    max_stages: u32,
) -> MaterialCombinerSignature
where
    F: FnMut(u32, u32) -> u32,
{
    let mut sampling_stage_count = 0u8;
    let mut force_multiply_like = false;

    for stage in 0..max_stages {
        if !texture_stage_enabled_with(stage_state_lookup, stage) {
            continue;
        }

        if !texture_stage_uses_texture_input_with(stage_state_lookup, stage) {
            continue;
        }

        sampling_stage_count = sampling_stage_count.saturating_add(1);

        let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
        let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
        if combiner_op_is_force_multiply_like(color_op)
            || combiner_op_is_force_multiply_like(alpha_op)
        {
            force_multiply_like = true;
        }
    }

    MaterialCombinerSignature {
        sampling_stage_count,
        force_multiply_like,
    }
}

pub(super) fn combiner_op_is_force_multiply_like(op: u32) -> bool {
    matches!(
        op,
        D3DTOP_MODULATE
            | D3DTOP_MODULATE2X
            | D3DTOP_MODULATE4X
            | D3DTOP_MULTIPLYADD
            | D3DTOP_MODULATEALPHA_ADDCOLOR
            | D3DTOP_MODULATECOLOR_ADDALPHA
            | D3DTOP_MODULATEINVALPHA_ADDCOLOR
            | D3DTOP_MODULATEINVCOLOR_ADDALPHA
            | D3DTOP_PREMODULATE
            | D3DTOP_BUMPENVMAP
            | D3DTOP_BUMPENVMAPLUMINANCE
    )
}

pub(super) fn first_enabled_texture_stage_with<F>(
    stage_state_lookup: &mut F,
    max_stages: u32,
) -> Option<u32>
where
    F: FnMut(u32, u32) -> u32,
{
    (0..max_stages).find(|stage| texture_stage_enabled_with(stage_state_lookup, *stage))
}

pub(super) fn enabled_texture_sampling_stage_count_with<F>(
    stage_state_lookup: &mut F,
    max_stages: u32,
) -> usize
where
    F: FnMut(u32, u32) -> u32,
{
    (0..max_stages)
        .filter(|stage| {
            texture_stage_enabled_with(stage_state_lookup, *stage)
                && texture_stage_uses_texture_input_with(stage_state_lookup, *stage)
        })
        .count()
}

pub(super) fn c_material_data_to_material(id: &str, data: W3DMaterialData) -> Material {
    Material {
        id: id.to_string(),
        name: id.to_string(),
        shader_id: "default".to_string(),
        diffuse_texture: None,
        normal_texture: None,
        specular_texture: None,
        emissive_texture: None,
        detail_texture: None,
        detail_blend_mode: 0,
        properties: crate::w3d::MaterialProperties {
            diffuse_color: data.albedo,
            specular_color: [data.metallic.clamp(0.0, 1.0); 3],
            emissive_color: [
                data.emission[0].max(0.0),
                data.emission[1].max(0.0),
                data.emission[2].max(0.0),
            ],
            shininess: (1.0 - data.roughness.clamp(0.0, 1.0)) * 128.0,
            alpha_cutoff: 0.5,
            alpha_test: false,
            transparent: data.albedo[3] < 0.999,
            double_sided: false,
            unlit: false,
        },
    }
}

pub(super) fn material_to_c_data(material: &Material) -> W3DMaterialData {
    let metallic = material
        .properties
        .specular_color
        .iter()
        .copied()
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0);
    let roughness = (1.0 - (material.properties.shininess / 128.0)).clamp(0.0, 1.0);
    W3DMaterialData {
        albedo: material.properties.diffuse_color,
        metallic,
        roughness,
        emission: material.properties.emissive_color,
    }
}

pub(super) fn default_material(id: &str) -> Material {
    Material {
        id: id.to_string(),
        name: id.to_string(),
        shader_id: "default".to_string(),
        diffuse_texture: None,
        normal_texture: None,
        specular_texture: None,
        emissive_texture: None,
        detail_texture: None,
        detail_blend_mode: 0,
        properties: crate::w3d::MaterialProperties {
            diffuse_color: [1.0, 1.0, 1.0, 1.0],
            specular_color: [0.0, 0.0, 0.0],
            emissive_color: [0.0, 0.0, 0.0],
            shininess: 1.0,
            alpha_cutoff: 0.0,
            alpha_test: false,
            transparent: false,
            double_sided: false,
            unlit: true,
        },
    }
}
