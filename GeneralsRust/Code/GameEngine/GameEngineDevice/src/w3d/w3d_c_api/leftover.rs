//! Shared leftover helpers used across the W3D C API split.
//!
//! Split from `w3d_c_api.rs`.

use super::constants::*;
use super::math::*;
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

/// Helper function to check if a pointer is valid
pub(super) unsafe fn is_valid_ptr<T>(ptr: *const T) -> bool {
    !ptr.is_null() && (ptr as usize) > 0x1000 // Basic sanity check
}
pub(super) fn is_alpha_blend_enabled(device: &W3DDeviceC) -> bool {
    if let Ok(states) = device.render_states.lock() {
        return alpha_blend_enabled_from_states(&states);
    }
    default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE) != 0
}

pub(super) fn alpha_blend_enabled_from_states(states: &HashMap<W3D_RENDER_STATE, u32>) -> bool {
    states
        .get(&W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE)
        .copied()
        .unwrap_or_else(|| default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE))
        != 0
}
pub(super) fn next_transient_mesh_id(device: &W3DDeviceC) -> String {
    if let Ok(mut counter) = device.transient_mesh_counter.lock() {
        let slot = *counter % TEMP_MESH_RING_SIZE;
        *counter = counter.wrapping_add(1);
        format!("{TEMP_MESH_PREFIX}{slot}")
    } else {
        format!("{TEMP_MESH_PREFIX}fallback")
    }
}

pub(super) fn next_material_id(device: &W3DDeviceC) -> String {
    if let Ok(mut counter) = device.material_counter.lock() {
        let id = format!("__w3d_c_api_material_{}", *counter);
        *counter = counter.wrapping_add(1);
        id
    } else {
        "__w3d_c_api_material_fallback".to_string()
    }
}

pub(super) fn current_material_id(device: &W3DDeviceC) -> Option<String> {
    if let Ok(current) = device.current_material_id.lock() {
        current.clone()
    } else {
        None
    }
}

pub(super) fn current_fvf(device: &W3DDeviceC) -> u32 {
    if let Ok(current) = device.current_fvf.lock() {
        *current
    } else {
        0
    }
}

pub(super) fn current_vertex_declaration(device: &W3DDeviceC) -> u32 {
    if let Ok(current) = device.current_vertex_declaration.lock() {
        *current
    } else {
        0
    }
}

pub(super) fn current_vertex_declaration_elements(
    device: &W3DDeviceC,
) -> Option<Vec<W3D_VERTEX_ELEMENT>> {
    let declaration = current_vertex_declaration(device);
    if declaration == 0 {
        return None;
    }
    device
        .vertex_declarations
        .lock()
        .ok()
        .and_then(|declarations| declarations.get(&declaration).cloned())
}
pub(super) fn read_f32(data: &[u8], offset: &mut usize) -> Option<f32> {
    let end = (*offset).checked_add(4)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(f32::from_le_bytes(bytes.try_into().ok()?))
}

pub(super) fn read_f32_at(data: &[u8], offset: usize) -> Option<f32> {
    let end = offset.checked_add(4)?;
    let bytes = data.get(offset..end)?;
    Some(f32::from_le_bytes(bytes.try_into().ok()?))
}

pub(super) fn read_u32(data: &[u8], offset: &mut usize) -> Option<u32> {
    let end = (*offset).checked_add(4)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

pub(super) fn read_u32_at(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes = data.get(offset..end)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

pub(super) fn read_i16_at(data: &[u8], offset: usize) -> Option<i16> {
    let end = offset.checked_add(2)?;
    let bytes = data.get(offset..end)?;
    Some(i16::from_le_bytes(bytes.try_into().ok()?))
}

pub(super) fn read_u16_at(data: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let bytes = data.get(offset..end)?;
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

pub(super) fn read_u8_at(data: &[u8], offset: usize) -> Option<u8> {
    data.get(offset).copied()
}

pub(super) fn w3d_vertex_to_modern(v: &W3D_VERTEX) -> W3DVertex {
    W3DVertex {
        position: [v.x, v.y, v.z],
        normal: [v.nx, v.ny, v.nz],
        uv: [v.u, v.v],
        color: [
            ((v.color >> 16) & 0xFF) as f32 / 255.0,
            ((v.color >> 8) & 0xFF) as f32 / 255.0,
            (v.color & 0xFF) as f32 / 255.0,
            ((v.color >> 24) & 0xFF) as f32 / 255.0,
        ],
    }
}
pub(super) fn decode_argb_color(argb: u32) -> [f32; 4] {
    [
        ((argb >> 16) & 0xFF) as f32 / 255.0,
        ((argb >> 8) & 0xFF) as f32 / 255.0,
        (argb & 0xFF) as f32 / 255.0,
        ((argb >> 24) & 0xFF) as f32 / 255.0,
    ]
}

pub(super) fn compute_vertex_bounds(vertices: &[W3DVertex]) -> ([f32; 3], [f32; 3]) {
    if vertices.is_empty() {
        return ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
    }

    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for vertex in vertices {
        let position = Vec3::from_array(vertex.position);
        min = min.min(position);
        max = max.max(position);
    }

    if min.is_finite() && max.is_finite() {
        (min.to_array(), max.to_array())
    } else {
        ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0])
    }
}

pub(super) fn transform_bounds(min: [f32; 3], max: [f32; 3], matrix: Mat4) -> ([f32; 3], [f32; 3]) {
    let corners = [
        Vec3::new(min[0], min[1], min[2]),
        Vec3::new(min[0], min[1], max[2]),
        Vec3::new(min[0], max[1], min[2]),
        Vec3::new(min[0], max[1], max[2]),
        Vec3::new(max[0], min[1], min[2]),
        Vec3::new(max[0], min[1], max[2]),
        Vec3::new(max[0], max[1], min[2]),
        Vec3::new(max[0], max[1], max[2]),
    ];

    let mut world_min = Vec3::splat(f32::INFINITY);
    let mut world_max = Vec3::splat(f32::NEG_INFINITY);

    for corner in corners {
        let transformed = matrix.transform_point3(corner);
        world_min = world_min.min(transformed);
        world_max = world_max.max(transformed);
    }

    if world_min.is_finite() && world_max.is_finite() {
        (world_min.to_array(), world_max.to_array())
    } else {
        (min, max)
    }
}
pub(super) fn default_render_state_value(state: W3D_RENDER_STATE) -> u32 {
    match state {
        W3D_RENDER_STATE::W3DRS_ZENABLE => 1,
        W3D_RENDER_STATE::W3DRS_CULLMODE => 2, // Back-face culling
        W3D_RENDER_STATE::W3DRS_ALPHATESTENABLE => 0,
        W3D_RENDER_STATE::W3DRS_ALPHAREF => 0,
        W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE => 0,
        W3D_RENDER_STATE::W3DRS_LIGHTING => 1,
        W3D_RENDER_STATE::W3DRS_SPECULARENABLE => 0,
        W3D_RENDER_STATE::W3DRS_TEXTUREFACTOR => 0xFFFF_FFFF,
        W3D_RENDER_STATE::W3DRS_AMBIENT => 0,
        W3D_RENDER_STATE::W3DRS_COLORVERTEX => 1,
        W3D_RENDER_STATE::W3DRS_LOCALVIEWER => 1,
        W3D_RENDER_STATE::W3DRS_NORMALIZENORMALS => 0,
        W3D_RENDER_STATE::W3DRS_DIFFUSEMATERIALSOURCE => D3DMCS_COLOR1,
        W3D_RENDER_STATE::W3DRS_SPECULARMATERIALSOURCE => D3DMCS_COLOR2,
        W3D_RENDER_STATE::W3DRS_AMBIENTMATERIALSOURCE => D3DMCS_MATERIAL,
        W3D_RENDER_STATE::W3DRS_EMISSIVEMATERIALSOURCE => D3DMCS_MATERIAL,
        _ => 0,
    }
}

pub(super) fn default_render_states() -> HashMap<W3D_RENDER_STATE, u32> {
    let mut states = HashMap::new();
    for state in [
        W3D_RENDER_STATE::W3DRS_ZENABLE,
        W3D_RENDER_STATE::W3DRS_CULLMODE,
        W3D_RENDER_STATE::W3DRS_ALPHATESTENABLE,
        W3D_RENDER_STATE::W3DRS_ALPHAREF,
        W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE,
        W3D_RENDER_STATE::W3DRS_TEXTUREFACTOR,
        W3D_RENDER_STATE::W3DRS_LIGHTING,
        W3D_RENDER_STATE::W3DRS_SPECULARENABLE,
        W3D_RENDER_STATE::W3DRS_AMBIENT,
        W3D_RENDER_STATE::W3DRS_COLORVERTEX,
        W3D_RENDER_STATE::W3DRS_LOCALVIEWER,
        W3D_RENDER_STATE::W3DRS_NORMALIZENORMALS,
        W3D_RENDER_STATE::W3DRS_DIFFUSEMATERIALSOURCE,
        W3D_RENDER_STATE::W3DRS_SPECULARMATERIALSOURCE,
        W3D_RENDER_STATE::W3DRS_AMBIENTMATERIALSOURCE,
        W3D_RENDER_STATE::W3DRS_EMISSIVEMATERIALSOURCE,
    ] {
        states.insert(state, default_render_state_value(state));
    }
    states
}
pub(super) fn default_viewport(width: u32, height: u32) -> W3D_VIEWPORT {
    W3D_VIEWPORT {
        x: 0,
        y: 0,
        width,
        height,
        min_z: 0.0,
        max_z: 1.0,
    }
}
