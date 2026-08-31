//! W3D C API textures, stage state, and texcoord generation.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::leftover::*;
use super::math::*;
use super::transforms::*;
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

/// Load texture - matches original W3DDevice::LoadTexture(filename)
#[no_mangle]
// SAFETY: C ABI entry. `device` live W3D_DEVICE; `filename` null-terminated UTF-8
// SAFETY: C string, converted to &str before any await point. Returns a new or
// SAFETY: interned W3D_TEXTURE owned by the device's texture-handle table.
pub unsafe extern "C" fn W3DDevice_LoadTexture(
    device: W3D_DEVICE,
    filename: *const c_char,
) -> W3D_TEXTURE {
    if device.is_null() || filename.is_null() {
        return std::ptr::null_mut();
    }

    let filename_cstr = CStr::from_ptr(filename);
    let filename_str = match filename_cstr.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let device_ref = &*device;
    match device_ref
        .runtime
        .block_on(async { load_texture_internal(&device_ref.device, filename_str).await })
    {
        Ok(texture_ptr) => intern_texture_handle(device_ref, texture_ptr),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Bind texture to a stage - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry. `texture` must be null or a W3D_TEXTURE previously
// SAFETY: returned by LoadTexture/GetTexture and still owned by the device's
// SAFETY: handle table; only the wrapped Texture value is cloned here.
pub unsafe extern "C" fn W3DDevice_SetTexture(
    device: W3D_DEVICE,
    stage: u32,
    texture: W3D_TEXTURE,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if texture.is_null() {
        if let Ok(mut bindings) = device_ref.bound_textures.lock() {
            bindings.remove(&stage);
        }
        return 1;
    }
    if !is_valid_ptr(texture) {
        return 0;
    }

    let texture_ref = &*texture;
    let texture_copy = texture_ref.texture.clone();
    if let Ok(mut bindings) = device_ref.bound_textures.lock() {
        bindings.insert(stage, texture_copy.id.clone());
    }
    if let Ok(mut handles) = device_ref.texture_handles.lock() {
        handles.entry(texture_copy.id.clone()).or_insert(texture);
    }

    match device_ref
        .runtime
        .block_on(async { set_texture_internal(&device_ref.device, texture_copy).await })
    {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Get texture bound to a stage - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI query. Returns either a handle already interned in the device's
// SAFETY: table or interns a fresh Box allocation; the pointer stays valid while
// SAFETY: the device lives (freed by W3DDevice_Destroy).
pub unsafe extern "C" fn W3DDevice_GetTexture(device: W3D_DEVICE, stage: u32) -> W3D_TEXTURE {
    if device.is_null() {
        return std::ptr::null_mut();
    }

    let device_ref = &*device;
    let texture_id = if let Ok(bindings) = device_ref.bound_textures.lock() {
        bindings.get(&stage).cloned()
    } else {
        None
    };
    let Some(texture_id) = texture_id else {
        return std::ptr::null_mut();
    };

    if let Ok(handles) = device_ref.texture_handles.lock() {
        if let Some(texture_handle) = handles.get(&texture_id).copied() {
            if is_valid_ptr(texture_handle) {
                return texture_handle;
            }
        }
    }

    let texture = device_ref
        .runtime
        .block_on(async { get_texture_internal(&device_ref.device, &texture_id).await });
    let Some(texture) = texture else {
        return std::ptr::null_mut();
    };

    let handle = Box::into_raw(Box::new(W3DTextureC { texture }));
    intern_texture_handle(device_ref, handle)
}

/// Set texture stage state - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry; only the device handle is dereferenced and stage-state
// SAFETY: mutation happens under its Mutex. No pointers are read.
pub unsafe extern "C" fn W3DDevice_SetTextureStageState(
    device: W3D_DEVICE,
    stage: u32,
    state: u32,
    value: u32,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut stage_states) = device_ref.texture_stage_states.lock() {
        stage_states.insert((stage, state), value);
        return 1;
    }
    0
}

/// Get texture stage state - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI query; only the device handle is dereferenced under its Mutex.
// SAFETY: Returns a by-value u32, no pointer is written.
pub unsafe extern "C" fn W3DDevice_GetTextureStageState(
    device: W3D_DEVICE,
    stage: u32,
    state: u32,
) -> u32 {
    if device.is_null() {
        return default_texture_stage_state(stage, state);
    }

    let device_ref = &*device;
    stage_texture_state_value(device_ref, stage, state)
}
pub(super) async fn load_texture_internal(
    device: &Arc<RwLock<W3DDevice>>,
    filename: &str,
) -> Result<W3D_TEXTURE> {
    let texture = match load_texture_from_disk(filename) {
        Ok(texture) => texture,
        Err(err) => {
            tracing::warn!(
                "W3D C API: failed to load texture '{filename}' ({err}); using checkerboard fallback"
            );
            checkerboard_fallback_texture(filename, 64, 64)
        }
    };

    let device_lock = device.read().await;
    device_lock.add_texture(texture.clone()).await?;

    let texture_c = Box::new(W3DTextureC { texture });
    Ok(Box::into_raw(texture_c))
}

pub(super) async fn set_texture_internal(
    device: &Arc<RwLock<W3DDevice>>,
    texture: Texture,
) -> Result<()> {
    let device_lock = device.read().await;
    device_lock.add_texture(texture).await?;
    Ok(())
}

pub(super) async fn get_texture_internal(
    device: &Arc<RwLock<W3DDevice>>,
    texture_id: &str,
) -> Option<Texture> {
    let device_lock = device.read().await;
    device_lock.get_texture(texture_id).await
}

// SAFETY: `texture_handle` must be null or an owning Box<W3DTextureC> raw
// SAFETY: pointer freshly produced by Box::into_raw in this module. Duplicate ids
// SAFETY: drop the new box so exactly one owner remains in the device table.
pub(super) unsafe fn intern_texture_handle(
    device: &W3DDeviceC,
    texture_handle: W3D_TEXTURE,
) -> W3D_TEXTURE {
    if texture_handle.is_null() {
        return std::ptr::null_mut();
    }

    let texture_id = (&*texture_handle).texture.id.clone();
    if let Ok(mut handles) = device.texture_handles.lock() {
        if let Some(existing) = handles.get(&texture_id).copied() {
            if existing != texture_handle {
                let _ = Box::from_raw(texture_handle);
            }
            return existing;
        }
        handles.insert(texture_id, texture_handle);
    }

    texture_handle
}

pub(super) fn stage_texcoord_usage_index(device: &W3DDeviceC, stage: u32) -> u8 {
    let raw = stage_texcoord_index_raw(device, stage);
    (raw & 0xFF) as u8
}

pub(super) fn active_draw_texture_stage(device: &W3DDeviceC) -> u32 {
    let Ok(bindings) = device.bound_textures.lock() else {
        return 0;
    };

    if bindings.is_empty() {
        return 0;
    }
    let mut stages = bindings.keys().copied().collect::<Vec<_>>();
    stages.sort_unstable();
    drop(bindings);

    resolve_active_draw_texture_stage(&stages, |stage, state| {
        stage_texture_state_value(device, stage, state)
    })
}

pub(super) fn resolve_active_draw_texture_stage<F>(
    bound_stages: &[u32],
    mut stage_state_lookup: F,
) -> u32
where
    F: FnMut(u32, u32) -> u32,
{
    if bound_stages.is_empty() {
        return 0;
    }

    // Prefer the first stage that is both enabled and samples texture in COLOR ops.
    // This better matches legacy fixed-function expectations where stage color output
    // is what our single-texture fallback material path approximates.
    if bound_stages.contains(&0)
        && texture_stage_enabled_with(&mut stage_state_lookup, 0)
        && texture_stage_uses_texture_color_input_with(&mut stage_state_lookup, 0)
    {
        return 0;
    }
    if let Some(stage) = bound_stages.iter().copied().find(|stage| {
        texture_stage_enabled_with(&mut stage_state_lookup, *stage)
            && texture_stage_uses_texture_color_input_with(&mut stage_state_lookup, *stage)
    }) {
        return stage;
    }

    // If no stage samples texture in color ops, fallback to alpha-sampling stages.
    if bound_stages.contains(&0)
        && texture_stage_enabled_with(&mut stage_state_lookup, 0)
        && texture_stage_uses_texture_alpha_input_with(&mut stage_state_lookup, 0)
    {
        return 0;
    }
    if let Some(stage) = bound_stages.iter().copied().find(|stage| {
        texture_stage_enabled_with(&mut stage_state_lookup, *stage)
            && texture_stage_uses_texture_alpha_input_with(&mut stage_state_lookup, *stage)
    }) {
        return stage;
    }

    // Fallback to historical behavior for unusual state combinations.
    if bound_stages.contains(&0) && texture_stage_enabled_with(&mut stage_state_lookup, 0) {
        return 0;
    }
    if let Some(stage) = bound_stages
        .iter()
        .copied()
        .find(|stage| texture_stage_enabled_with(&mut stage_state_lookup, *stage))
    {
        return stage;
    }

    bound_stages.first().copied().unwrap_or(0)
}

pub(super) fn stage_texcoord_index_raw(device: &W3DDeviceC, stage: u32) -> u32 {
    stage_texture_state_value(device, stage, D3DTSS_TEXCOORDINDEX)
}

pub(super) fn texture_stage_enabled(device: &W3DDeviceC, stage: u32) -> bool {
    let color_op = stage_texture_state_value(device, stage, D3DTSS_COLOROP);
    let alpha_op = stage_texture_state_value(device, stage, D3DTSS_ALPHAOP);
    color_op != D3DTOP_DISABLE || alpha_op != D3DTOP_DISABLE
}

pub(super) fn texture_stage_enabled_with<F>(stage_state_lookup: &mut F, stage: u32) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
    let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
    color_op != D3DTOP_DISABLE || alpha_op != D3DTOP_DISABLE
}

pub(super) fn texture_stage_uses_texture_input_with<F>(
    stage_state_lookup: &mut F,
    stage: u32,
) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    texture_stage_uses_texture_color_input_with(stage_state_lookup, stage)
        || texture_stage_uses_texture_alpha_input_with(stage_state_lookup, stage)
}

pub(super) fn texture_stage_uses_texture_color_input_with<F>(
    stage_state_lookup: &mut F,
    stage: u32,
) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
    let color_arg0 = stage_state_lookup(stage, D3DTSS_COLORARG0);
    let color_arg1 = stage_state_lookup(stage, D3DTSS_COLORARG1);
    let color_arg2 = stage_state_lookup(stage, D3DTSS_COLORARG2);
    op_uses_texture_arg(color_op, color_arg0, color_arg1, color_arg2)
}

pub(super) fn texture_stage_uses_texture_alpha_input_with<F>(
    stage_state_lookup: &mut F,
    stage: u32,
) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
    let alpha_arg0 = stage_state_lookup(stage, D3DTSS_ALPHAARG0);
    let alpha_arg1 = stage_state_lookup(stage, D3DTSS_ALPHAARG1);
    let alpha_arg2 = stage_state_lookup(stage, D3DTSS_ALPHAARG2);
    op_uses_texture_arg(alpha_op, alpha_arg0, alpha_arg1, alpha_arg2)
}

pub(super) fn op_uses_texture_arg(op: u32, arg0: u32, arg1: u32, arg2: u32) -> bool {
    if op == D3DTOP_DISABLE {
        return false;
    }

    let uses_arg0 = op_uses_arg0(op);
    let uses_arg1 = op_uses_arg1(op);
    let uses_arg2 = op_uses_arg2(op);
    (uses_arg0 && arg_references_texture(arg0))
        || (uses_arg1 && arg_references_texture(arg1))
        || (uses_arg2 && arg_references_texture(arg2))
}

pub(super) fn op_uses_arg0(op: u32) -> bool {
    matches!(op, D3DTOP_MULTIPLYADD | D3DTOP_LERP)
}

pub(super) fn op_uses_arg1(op: u32) -> bool {
    match op {
        D3DTOP_DISABLE | D3DTOP_SELECTARG2 => false,
        D3DTOP_SELECTARG1
        | D3DTOP_MODULATE
        | D3DTOP_MODULATE2X
        | D3DTOP_MODULATE4X
        | D3DTOP_ADD
        | D3DTOP_ADDSIGNED
        | D3DTOP_ADDSIGNED2X
        | D3DTOP_SUBTRACT
        | D3DTOP_ADDSMOOTH
        | D3DTOP_BLENDDIFFUSEALPHA
        | D3DTOP_BLENDTEXTUREALPHA
        | D3DTOP_BLENDFACTORALPHA
        | D3DTOP_BLENDTEXTUREALPHAPM
        | D3DTOP_BLENDCURRENTALPHA
        | D3DTOP_PREMODULATE
        | D3DTOP_MODULATEALPHA_ADDCOLOR
        | D3DTOP_MODULATECOLOR_ADDALPHA
        | D3DTOP_MODULATEINVALPHA_ADDCOLOR
        | D3DTOP_MODULATEINVCOLOR_ADDALPHA
        | D3DTOP_BUMPENVMAP
        | D3DTOP_BUMPENVMAPLUMINANCE
        | D3DTOP_DOTPRODUCT3
        | D3DTOP_MULTIPLYADD
        | D3DTOP_LERP => true,
        _ => false,
    }
}

pub(super) fn op_uses_arg2(op: u32) -> bool {
    match op {
        D3DTOP_DISABLE | D3DTOP_SELECTARG1 => false,
        D3DTOP_SELECTARG2
        | D3DTOP_MODULATE
        | D3DTOP_MODULATE2X
        | D3DTOP_MODULATE4X
        | D3DTOP_ADD
        | D3DTOP_ADDSIGNED
        | D3DTOP_ADDSIGNED2X
        | D3DTOP_SUBTRACT
        | D3DTOP_ADDSMOOTH
        | D3DTOP_BLENDDIFFUSEALPHA
        | D3DTOP_BLENDTEXTUREALPHA
        | D3DTOP_BLENDFACTORALPHA
        | D3DTOP_BLENDTEXTUREALPHAPM
        | D3DTOP_BLENDCURRENTALPHA
        | D3DTOP_PREMODULATE
        | D3DTOP_MODULATEALPHA_ADDCOLOR
        | D3DTOP_MODULATECOLOR_ADDALPHA
        | D3DTOP_MODULATEINVALPHA_ADDCOLOR
        | D3DTOP_MODULATEINVCOLOR_ADDALPHA
        | D3DTOP_BUMPENVMAP
        | D3DTOP_BUMPENVMAPLUMINANCE
        | D3DTOP_DOTPRODUCT3
        | D3DTOP_MULTIPLYADD
        | D3DTOP_LERP => true,
        _ => false,
    }
}

pub(super) fn arg_references_texture(arg: u32) -> bool {
    (arg & D3DTA_SELECTMASK) == D3DTA_TEXTURE
}

pub(super) fn arg_references_tfactor(arg: u32) -> bool {
    (arg & D3DTA_SELECTMASK) == D3DTA_TFACTOR
}
pub(super) fn default_texture_stage_state(stage: u32, state: u32) -> u32 {
    match state {
        D3DTSS_COLOROP => {
            if stage == 0 {
                D3DTOP_MODULATE
            } else {
                D3DTOP_DISABLE
            }
        }
        D3DTSS_COLORARG0 | D3DTSS_ALPHAARG0 => D3DTA_CURRENT,
        D3DTSS_COLORARG1 | D3DTSS_ALPHAARG1 => D3DTA_TEXTURE,
        D3DTSS_COLORARG2 | D3DTSS_ALPHAARG2 => D3DTA_CURRENT,
        D3DTSS_ALPHAOP => {
            if stage == 0 {
                D3DTOP_SELECTARG1
            } else {
                D3DTOP_DISABLE
            }
        }
        D3DTSS_TEXCOORDINDEX => stage,
        D3DTSS_TEXTURETRANSFORMFLAGS => D3DTTFF_DISABLE,
        _ => 0,
    }
}

pub(super) fn stage_texture_state_value(device: &W3DDeviceC, stage: u32, state: u32) -> u32 {
    let Ok(stage_states) = device.texture_stage_states.lock() else {
        return default_texture_stage_state(stage, state);
    };
    stage_states
        .get(&(stage, state))
        .copied()
        .unwrap_or_else(|| default_texture_stage_state(stage, state))
}

pub(super) fn texture_transform_state(stage: u32) -> Option<W3D_TRANSFORM_STATE> {
    match stage {
        0 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE0),
        1 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE1),
        2 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE2),
        3 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE3),
        _ => None,
    }
}

pub(super) fn stage_texture_transform_flags(device: &W3DDeviceC, stage: u32) -> u32 {
    stage_texture_state_value(device, stage, D3DTSS_TEXTURETRANSFORMFLAGS)
}

pub(super) fn current_texture_transform_matrix(
    device: &W3DDeviceC,
    stage: u32,
) -> Option<W3D_MATRIX> {
    let state = texture_transform_state(stage)?;
    let Ok(states) = device.transform_states.lock() else {
        return Some(default_transform_state_value(state));
    };
    Some(
        states
            .get(&state)
            .copied()
            .unwrap_or_else(|| default_transform_state_value(state)),
    )
}

pub(super) fn apply_stage_texture_transform(
    device: &W3DDeviceC,
    stage: u32,
    vertices: &mut [W3D_VERTEX],
) {
    if vertices.is_empty() {
        return;
    }

    let texcoord_source = stage_texcoord_index_raw(device, stage) & D3DTSS_TCI_MASK;
    let world_matrix = current_world_transform(device);
    let view_matrix = current_transform_value(device, W3D_TRANSFORM_STATE::W3DTS_VIEW);

    let flags = stage_texture_transform_flags(device, stage);
    let coord_count = flags & D3DTTFF_COUNT_MASK;
    if coord_count == D3DTTFF_DISABLE {
        // D3D fixed-function still applies texcoord generation when TCI_* is set,
        // even if no texture transform matrix is active for the stage.
        if texcoord_source != 0 {
            apply_generated_stage_texcoords(vertices, texcoord_source, &world_matrix, &view_matrix);
        }
        return;
    }
    if !(D3DTTFF_COUNT1..=D3DTTFF_COUNT4).contains(&coord_count) {
        return;
    }

    let Some(matrix) = current_texture_transform_matrix(device, stage) else {
        return;
    };
    let projected = (flags & D3DTTFF_PROJECTED) != 0;
    let count = coord_count as usize;

    for vertex in vertices {
        let src = texture_transform_input_for_vertex(
            vertex,
            texcoord_source,
            &world_matrix,
            &view_matrix,
        );
        let transformed = mul_row_vec4_matrix(src, &matrix);

        let (mut u, mut v) = (transformed[0], transformed[1]);
        if projected && count >= 2 {
            let w = transformed[count - 1];
            if w.is_finite() && w.abs() > 1.0e-6 {
                u /= w;
                v /= w;
            }
        }

        if u.is_finite() {
            vertex.u = u;
        }
        if count >= 2 && v.is_finite() {
            vertex.v = v;
        }
    }
}

pub(super) fn apply_generated_stage_texcoords(
    vertices: &mut [W3D_VERTEX],
    texcoord_source: u32,
    world_matrix: &W3D_MATRIX,
    view_matrix: &W3D_MATRIX,
) {
    if texcoord_source == 0 {
        return;
    }

    for vertex in vertices {
        let generated =
            texture_transform_input_for_vertex(vertex, texcoord_source, world_matrix, view_matrix);
        if generated[0].is_finite() {
            vertex.u = generated[0];
        }
        if generated[1].is_finite() {
            vertex.v = generated[1];
        }
    }
}

pub(super) fn texture_transform_input_for_vertex(
    vertex: &W3D_VERTEX,
    texcoord_source: u32,
    world_matrix: &W3D_MATRIX,
    view_matrix: &W3D_MATRIX,
) -> [f32; 4] {
    match texcoord_source {
        D3DTSS_TCI_CAMERASPACENORMAL => {
            let normal_world =
                mul_row_vec4_matrix([vertex.nx, vertex.ny, vertex.nz, 0.0], world_matrix);
            let normal_view = mul_row_vec4_matrix(normal_world, view_matrix);
            let normal =
                Vec3::new(normal_view[0], normal_view[1], normal_view[2]).normalize_or_zero();
            [normal.x, normal.y, normal.z, 1.0]
        }
        D3DTSS_TCI_CAMERASPACEPOSITION => {
            let pos_world = mul_row_vec4_matrix([vertex.x, vertex.y, vertex.z, 1.0], world_matrix);
            let pos_view = mul_row_vec4_matrix(pos_world, view_matrix);
            [pos_view[0], pos_view[1], pos_view[2], 1.0]
        }
        D3DTSS_TCI_CAMERASPACEREFLECTIONVECTOR => {
            let pos_world = mul_row_vec4_matrix([vertex.x, vertex.y, vertex.z, 1.0], world_matrix);
            let pos_view = mul_row_vec4_matrix(pos_world, view_matrix);
            let normal_world =
                mul_row_vec4_matrix([vertex.nx, vertex.ny, vertex.nz, 0.0], world_matrix);
            let normal_view = mul_row_vec4_matrix(normal_world, view_matrix);
            let eye_dir = Vec3::new(-pos_view[0], -pos_view[1], -pos_view[2]).normalize_or_zero();
            let normal =
                Vec3::new(normal_view[0], normal_view[1], normal_view[2]).normalize_or_zero();
            let reflection = eye_dir - (2.0 * eye_dir.dot(normal)) * normal;
            [reflection.x, reflection.y, reflection.z, 1.0]
        }
        D3DTSS_TCI_SPHEREMAP => {
            let normal_world =
                mul_row_vec4_matrix([vertex.nx, vertex.ny, vertex.nz, 0.0], world_matrix);
            let normal_view = mul_row_vec4_matrix(normal_world, view_matrix);
            let normal =
                Vec3::new(normal_view[0], normal_view[1], normal_view[2]).normalize_or_zero();
            // Legacy sphere-map approximation from camera-space normal.
            // Match D3D fixed-function expectations where generated UVs are in [0,1].
            let u = (normal.x * 0.5 + 0.5).clamp(0.0, 1.0);
            let v = (-normal.y * 0.5 + 0.5).clamp(0.0, 1.0);
            [u, v, 0.0, 1.0]
        }
        _ => [vertex.u, vertex.v, 0.0, 1.0],
    }
}

pub(super) fn mul_row_vec4_matrix(v: [f32; 4], m: &W3D_MATRIX) -> [f32; 4] {
    [
        v[0] * m.m[0][0] + v[1] * m.m[1][0] + v[2] * m.m[2][0] + v[3] * m.m[3][0],
        v[0] * m.m[0][1] + v[1] * m.m[1][1] + v[2] * m.m[2][1] + v[3] * m.m[3][1],
        v[0] * m.m[0][2] + v[1] * m.m[1][2] + v[2] * m.m[2][2] + v[3] * m.m[3][2],
        v[0] * m.m[0][3] + v[1] * m.m[1][3] + v[2] * m.m[2][3] + v[3] * m.m[3][3],
    ]
}
pub(super) fn load_texture_from_disk(filename: &str) -> Result<Texture> {
    let path = resolve_texture_path(filename).ok_or_else(|| {
        W3DError::ResourceLoadingFailed(format!("Texture path not found: {filename}"))
    })?;

    let bytes = std::fs::read(&path).map_err(|e| {
        W3DError::ResourceLoadingFailed(format!("Failed to read texture '{}': {e}", path.display()))
    })?;

    let image = decode_texture_from_bytes(&path, &bytes).map_err(|e| {
        W3DError::ResourceLoadingFailed(format!(
            "Failed to decode texture '{}': {e}",
            path.display()
        ))
    })?;

    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(Texture {
        id: format!("texture_{}", path.to_string_lossy()),
        name: filename.to_string(),
        width,
        height,
        depth: 1,
        mip_levels: 1,
        format: crate::w3d::TextureFormat::Rgba8,
        texture_type: crate::w3d::TextureType::Texture2D,
        data: rgba.into_raw(),
    })
}

pub(super) fn decode_texture_from_bytes(
    path: &Path,
    bytes: &[u8],
) -> std::result::Result<image::DynamicImage, image::ImageError> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    let decode_by_extension = match extension.as_deref() {
        Some("tga") => image::load_from_memory_with_format(bytes, image::ImageFormat::Tga),
        Some("dds") => image::load_from_memory_with_format(bytes, image::ImageFormat::Dds),
        Some("png") => image::load_from_memory_with_format(bytes, image::ImageFormat::Png),
        Some("jpg") | Some("jpeg") => {
            image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)
        }
        Some("bmp") => image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp),
        _ => image::load_from_memory(bytes),
    };

    if decode_by_extension.is_ok() {
        return decode_by_extension;
    }

    for format in [
        image::ImageFormat::Dds,
        image::ImageFormat::Tga,
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::Bmp,
    ] {
        if let Ok(decoded) = image::load_from_memory_with_format(bytes, format) {
            return Ok(decoded);
        }
    }

    image::load_from_memory(bytes)
}

pub(super) fn resolve_texture_path(filename: &str) -> Option<PathBuf> {
    let requested = Path::new(filename);
    if requested.is_file() {
        return Some(requested.to_path_buf());
    }

    let normalized = filename.replace('\\', "/");
    let bare = normalized.trim_start_matches("./").to_string();
    let has_extension = Path::new(&bare).extension().is_some();

    let mut resource_candidates = Vec::<String>::new();
    let mut push_resource_candidate = |list: &mut Vec<String>, candidate: String| {
        if !list.iter().any(|existing| existing == &candidate) {
            list.push(candidate);
        }
    };

    if !bare.is_empty() {
        push_resource_candidate(&mut resource_candidates, bare.clone());
    }

    if !bare.contains('/') {
        push_resource_candidate(&mut resource_candidates, format!("Art/Textures/{bare}"));
        push_resource_candidate(&mut resource_candidates, format!("Art/Terrain/{bare}"));
        push_resource_candidate(
            &mut resource_candidates,
            format!("Data/Art/Textures/{bare}"),
        );
        push_resource_candidate(&mut resource_candidates, format!("Data/Art/Terrain/{bare}"));
    }

    if !bare.starts_with("Data/") {
        push_resource_candidate(&mut resource_candidates, format!("Data/{bare}"));
    }
    if !bare.starts_with("assets/") {
        push_resource_candidate(&mut resource_candidates, format!("assets/{bare}"));
    }

    if !has_extension {
        let bases = resource_candidates.clone();
        for base in &bases {
            for ext in ["tga", "dds", "png", "jpg", "jpeg", "bmp"] {
                push_resource_candidate(&mut resource_candidates, format!("{base}.{ext}"));
            }
        }
    }

    let mut candidates = Vec::<PathBuf>::new();
    let mut push_path_candidate = |list: &mut Vec<PathBuf>, candidate: PathBuf| {
        if !list.iter().any(|existing| existing == &candidate) {
            list.push(candidate);
        }
    };

    for resource_name in &resource_candidates {
        push_path_candidate(&mut candidates, PathBuf::from(resource_name));
    }

    if let Ok(cwd) = std::env::current_dir() {
        for resource_name in &resource_candidates {
            push_path_candidate(&mut candidates, cwd.join(resource_name));
        }
    }

    if let Ok(root) = std::env::var("GENERALS_ASSETS_DIR") {
        let root = PathBuf::from(root);
        for resource_name in &resource_candidates {
            push_path_candidate(&mut candidates, root.join(resource_name));
        }
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

pub(super) fn checkerboard_fallback_texture(filename: &str, width: u32, height: u32) -> Texture {
    let mut data = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let checker = ((x / 8) + (y / 8)) % 2 == 0;
            let (r, g, b) = if checker { (255, 0, 255) } else { (24, 24, 24) };
            data[i] = r;
            data[i + 1] = g;
            data[i + 2] = b;
            data[i + 3] = 255;
        }
    }

    Texture {
        id: format!("fallback_{}", filename),
        name: filename.to_string(),
        width,
        height,
        depth: 1,
        mip_levels: 1,
        format: crate::w3d::TextureFormat::Rgba8,
        texture_type: crate::w3d::TextureType::Texture2D,
        data,
    }
}
