//! W3D C API draw calls and transient mesh submission.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::decl::*;
use super::leftover::*;
use super::materials::*;
use super::math::*;
use super::streams::*;
use super::textures::*;
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

/// Draw indexed primitive - matches original W3DDevice::DrawIndexedPrimitive(type, vertices, indices)
#[unsafe(no_mangle)]
// SAFETY: C ABI entry. `device` live W3D_DEVICE; when non-null `vertex_buffer`
// SAFETY: must be readable for vertex_count W3D_VERTEXs and `index_buffer` for
// SAFETY: index_count u16s (null falls back to staged stream/index state).
pub unsafe extern "C" fn W3DDevice_DrawIndexedPrimitive(
    device: W3D_DEVICE,
    primitive_type: W3D_PRIMITIVE_TYPE,
    vertex_buffer: *const W3D_VERTEX,
    vertex_count: u32,
    index_buffer: *const u16,
    index_count: u32,
) -> i32 {
    if device.is_null() {
        return 0; // Failure
    }

    let device_ref = &*device;
    let mut staged_vertices: Option<Vec<W3D_VERTEX>> = None;
    let mut staged_indices: Option<Vec<u16>> = None;
    let mut staged_base_vertex_index = 0;

    let resolved_vertex_buffer = if vertex_buffer.is_null() {
        let Some(vertices) = staged_stream_vertices(device_ref, 0, vertex_count) else {
            return 0;
        };
        staged_vertices = Some(vertices);
        staged_vertices
            .as_ref()
            .map(|vertices| vertices.as_ptr())
            .unwrap_or(std::ptr::null())
    } else {
        vertex_buffer
    };
    let resolved_vertex_count = if let Some(vertices) = &staged_vertices {
        vertices.len() as u32
    } else {
        vertex_count
    };

    let resolved_index_buffer = if index_buffer.is_null() {
        let Some((indices, base_vertex_index)) = staged_index_buffer(device_ref, index_count)
        else {
            return 0;
        };
        staged_base_vertex_index = base_vertex_index;
        staged_indices = Some(indices);
        staged_indices
            .as_ref()
            .map(|indices| indices.as_ptr())
            .unwrap_or(std::ptr::null())
    } else {
        index_buffer
    };
    let resolved_index_count = if let Some(indices) = &staged_indices {
        indices.len() as u32
    } else {
        index_count
    };

    if resolved_vertex_buffer.is_null() || resolved_index_buffer.is_null() {
        return 0;
    }

    if let Ok(mut active) = device_ref.scene_active.lock() {
        if !*active {
            // Keep callers resilient when legacy BeginScene sequencing is omitted.
            tracing::trace!("W3D C API: implicit BeginScene on DrawIndexedPrimitive");
            *active = true;
        }
    }
    let world_matrix = current_world_transform(device_ref);
    let mesh_id = next_transient_mesh_id(device_ref);
    let draw_texture_stage = active_draw_texture_stage(device_ref);
    let material_id = resolve_draw_material_id(device_ref, draw_texture_stage);

    match draw_indexed_primitive_internal(
        device_ref,
        &device_ref.device,
        primitive_type,
        resolved_vertex_buffer,
        resolved_vertex_count,
        resolved_index_buffer,
        resolved_index_count,
        staged_base_vertex_index,
        &mesh_id,
        world_matrix,
        material_id,
    ) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Draw indexed primitive from staged stream/index state using DX8-style arguments.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry drawing from staged stream state; only the device handle
// SAFETY: is dereferenced and all staged data was copied at Set* time.
pub unsafe extern "C" fn W3DDevice_DrawIndexedPrimitiveLegacy(
    device: W3D_DEVICE,
    primitive_type: W3D_PRIMITIVE_TYPE,
    min_vertex_index: u32,
    vertex_count: u32,
    start_index: u32,
    primitive_count: u32,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let Some(index_count) = primitive_index_count(primitive_type, primitive_count) else {
        return 0;
    };
    if index_count == 0 {
        return 1;
    }

    let device_ref = &*device;
    let requested_vertices = if vertex_count == 0 {
        0
    } else {
        min_vertex_index.saturating_add(vertex_count)
    };
    let Some(vertices) = staged_stream_vertices(device_ref, 0, requested_vertices) else {
        return 0;
    };
    let Some((indices, staged_base_vertex_index)) =
        staged_index_buffer_range(device_ref, start_index as usize, index_count as usize)
    else {
        return 0;
    };

    if vertex_count != 0 {
        let range_start = min_vertex_index as usize;
        let range_end = range_start.saturating_add(vertex_count as usize);
        if indices
            .iter()
            .any(|index| (*index as usize) < range_start || (*index as usize) >= range_end)
        {
            return 0;
        }
    }

    if let Ok(mut active) = device_ref.scene_active.lock() {
        if !*active {
            tracing::trace!("W3D C API: implicit BeginScene on DrawIndexedPrimitiveLegacy");
            *active = true;
        }
    }
    let world_matrix = current_world_transform(device_ref);
    let mesh_id = next_transient_mesh_id(device_ref);
    let draw_texture_stage = active_draw_texture_stage(device_ref);
    let material_id = resolve_draw_material_id(device_ref, draw_texture_stage);

    match draw_indexed_primitive_internal(
        device_ref,
        &device_ref.device,
        primitive_type,
        vertices.as_ptr(),
        vertices.len() as u32,
        indices.as_ptr(),
        indices.len() as u32,
        staged_base_vertex_index,
        &mesh_id,
        world_matrix,
        material_id,
    ) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}
/// Draw primitive from staged stream data (non-indexed path).
#[unsafe(no_mangle)]
// SAFETY: C ABI entry drawing non-indexed primitives from staged stream state;
// SAFETY: only the device handle is dereferenced, staged data is device-owned.
pub unsafe extern "C" fn W3DDevice_DrawPrimitive(
    device: W3D_DEVICE,
    primitive_type: W3D_PRIMITIVE_TYPE,
    start_vertex: u32,
    primitive_count: u32,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let Some(vertex_count) = primitive_vertex_count(primitive_type, primitive_count) else {
        return 0;
    };
    if vertex_count == 0 {
        return 1;
    }
    let device_ref = &*device;
    let Some(mut vertices) =
        staged_stream_vertices_range(device_ref, 0, start_vertex as usize, vertex_count as usize)
    else {
        return 0;
    };
    let draw_texture_stage = active_draw_texture_stage(device_ref);
    apply_stage_texture_transform(device_ref, draw_texture_stage, &mut vertices);
    let indices = (0..(vertices.len() as u32)).collect::<Vec<u32>>();

    if let Ok(mut active) = device_ref.scene_active.lock() {
        if !*active {
            tracing::trace!("W3D C API: implicit BeginScene on DrawPrimitive");
            *active = true;
        }
    }
    let world_matrix = current_world_transform(device_ref);
    let mesh_id = next_transient_mesh_id(device_ref);
    let material_id = resolve_draw_material_id(device_ref, draw_texture_stage);
    let alpha_blend_enabled = is_alpha_blend_enabled(device_ref);

    match submit_transient_draw_internal(
        &device_ref.device,
        primitive_type,
        &vertices,
        &indices,
        &mesh_id,
        world_matrix,
        material_id,
        alpha_blend_enabled,
    ) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Draw primitive UP - legacy immediate-mode compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry. `vertex_data` must remain readable for
// SAFETY: stride * primitive_vertex_count bytes through collect_up_vertices,
// SAFETY: which copies into an owned Vec before any async work.
pub unsafe extern "C" fn W3DDevice_DrawPrimitiveUP(
    device: W3D_DEVICE,
    primitive_type: W3D_PRIMITIVE_TYPE,
    primitive_count: u32,
    vertex_data: *const c_void,
    vertex_stride: u32,
) -> i32 {
    if device.is_null() || !is_valid_ptr(vertex_data) || vertex_stride < 12 {
        return 0;
    }
    let device_ref = &*device;

    let Some(vertex_count) = primitive_vertex_count(primitive_type, primitive_count) else {
        return 0;
    };
    if vertex_count == 0 {
        return 1;
    }

    let draw_texture_stage = active_draw_texture_stage(device_ref);
    let draw_texcoord_usage_index = stage_texcoord_usage_index(device_ref, draw_texture_stage);
    let fvf = current_fvf(device_ref);
    let Some(vertices) = collect_up_vertices(
        vertex_data,
        vertex_count as usize,
        vertex_stride as usize,
        fvf,
        draw_texcoord_usage_index,
    ) else {
        return 0;
    };
    let mut vertices = vertices;
    apply_stage_texture_transform(device_ref, draw_texture_stage, &mut vertices);
    let indices = (0..vertex_count).collect::<Vec<u32>>();

    if let Ok(mut active) = device_ref.scene_active.lock() {
        if !*active {
            tracing::trace!("W3D C API: implicit BeginScene on DrawPrimitiveUP");
            *active = true;
        }
    }
    let world_matrix = current_world_transform(device_ref);
    let mesh_id = next_transient_mesh_id(device_ref);
    let material_id = resolve_draw_material_id(device_ref, draw_texture_stage);
    let alpha_blend_enabled = is_alpha_blend_enabled(device_ref);

    match submit_transient_draw_internal(
        &device_ref.device,
        primitive_type,
        &vertices,
        &indices,
        &mesh_id,
        world_matrix,
        material_id,
        alpha_blend_enabled,
    ) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Draw indexed primitive from immediate-mode UP buffers.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry. `vertex_data` readable for stride*vertex_count bytes,
// SAFETY: `index_data` for primitive_index_count u16/u32s per index_format; both
// SAFETY: are copied immediately by collect_up_vertices / collect_up_indices.
pub unsafe extern "C" fn W3DDevice_DrawIndexedPrimitiveUP(
    device: W3D_DEVICE,
    primitive_type: W3D_PRIMITIVE_TYPE,
    min_vertex_index: u32,
    vertex_count: u32,
    primitive_count: u32,
    index_data: *const c_void,
    index_format: u32,
    vertex_data: *const c_void,
    vertex_stride: u32,
) -> i32 {
    if device.is_null()
        || !is_valid_ptr(index_data)
        || !is_valid_ptr(vertex_data)
        || vertex_stride < 12
        || vertex_count == 0
    {
        return 0;
    }

    let Some(index_count) = primitive_index_count(primitive_type, primitive_count) else {
        return 0;
    };
    if index_count == 0 {
        return 1;
    }

    let device_ref = &*device;
    let draw_texture_stage = active_draw_texture_stage(device_ref);
    let draw_texcoord_usage_index = stage_texcoord_usage_index(device_ref, draw_texture_stage);
    let fvf = current_fvf(device_ref);
    let Some(vertices) = collect_up_vertices(
        vertex_data,
        vertex_count as usize,
        vertex_stride as usize,
        fvf,
        draw_texcoord_usage_index,
    ) else {
        return 0;
    };
    let mut vertices = vertices;
    apply_stage_texture_transform(device_ref, draw_texture_stage, &mut vertices);

    let Some(mut indices) = collect_up_indices(index_data, index_count as usize, index_format)
    else {
        return 0;
    };
    if indices
        .iter()
        .any(|index| (*index as usize) >= vertices.len())
    {
        let min = min_vertex_index as u32;
        let max_exclusive = min.saturating_add(vertices.len() as u32);
        if indices
            .iter()
            .all(|index| *index >= min && *index < max_exclusive)
        {
            for index in &mut indices {
                *index -= min;
            }
        } else {
            return 0;
        }
    }

    if let Ok(mut active) = device_ref.scene_active.lock() {
        if !*active {
            tracing::trace!("W3D C API: implicit BeginScene on DrawIndexedPrimitiveUP");
            *active = true;
        }
    }
    let world_matrix = current_world_transform(device_ref);
    let mesh_id = next_transient_mesh_id(device_ref);
    let material_id = resolve_draw_material_id(device_ref, draw_texture_stage);
    let alpha_blend_enabled = is_alpha_blend_enabled(device_ref);

    match submit_transient_draw_internal(
        &device_ref.device,
        primitive_type,
        &vertices,
        &indices,
        &mesh_id,
        world_matrix,
        material_id,
        alpha_blend_enabled,
    ) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}
pub(super) fn draw_indexed_primitive_internal(
    device_ref: &W3DDeviceC,
    device: &Arc<RwLock<W3DDevice>>,
    primitive_type: W3D_PRIMITIVE_TYPE,
    vertex_buffer: *const W3D_VERTEX,
    vertex_count: u32,
    index_buffer: *const u16,
    index_count: u32,
    base_vertex_index: i32,
    mesh_id: &str,
    world_matrix: W3D_MATRIX,
    material_id: Option<String>,
) -> Result<()> {
    let mut vertices =
        // SAFETY: Caller (W3DDevice_DrawIndexedPrimitive*) validated the buffer is
        // SAFETY: non-null after staging fallback and vertex_count bounds the caller's
        // SAFETY: live vertex array; contents are cloned before any await point.
        unsafe { std::slice::from_raw_parts(vertex_buffer, vertex_count as usize).to_vec() };
    let draw_texture_stage = active_draw_texture_stage(device_ref);
    apply_stage_texture_transform(device_ref, draw_texture_stage, &mut vertices);
    // SAFETY: `index_buffer` comes from a validated caller argument or device-owned
    // SAFETY: staged indices; index_count bounds the readable u16 region. The slice is
    // SAFETY: consumed within this function without outliving the call.
    let indices = unsafe { std::slice::from_raw_parts(index_buffer, index_count as usize) };
    let modern_indices: Vec<u32> = indices
        .iter()
        .map(|&i| {
            let adjusted = i as i32 + base_vertex_index;
            adjusted.max(0) as u32
        })
        .collect();

    submit_transient_draw_internal(
        device,
        primitive_type,
        &vertices,
        &modern_indices,
        mesh_id,
        world_matrix,
        material_id,
        is_alpha_blend_enabled(device_ref),
    )
}

pub(super) fn submit_transient_draw_internal(
    _device: &Arc<RwLock<W3DDevice>>,
    primitive_type: W3D_PRIMITIVE_TYPE,
    vertices: &[W3D_VERTEX],
    indices: &[u32],
    mesh_id: &str,
    world_matrix: W3D_MATRIX,
    material_id: Option<String>,
    alpha_blend_enabled: bool,
) -> Result<()> {
    let modern_vertices: Vec<W3DVertex> = vertices.iter().map(w3d_vertex_to_modern).collect();
    let mesh_data = bytemuck::cast_slice(&modern_vertices).to_vec();
    crate::w3d::renderer::stage_up_draw(crate::w3d::renderer::StagedUpDraw {
        mesh_id: mesh_id.to_string(),
        vertices: mesh_data,
        indices: indices.to_vec(),
        topology: match primitive_type {
            W3D_PRIMITIVE_TYPE::W3D_TRIANGLES => crate::w3d::PrimitiveTopology::TriangleList,
            W3D_PRIMITIVE_TYPE::W3D_TRIANGLE_STRIP => crate::w3d::PrimitiveTopology::TriangleStrip,
            W3D_PRIMITIVE_TYPE::W3D_TRIANGLE_FAN => crate::w3d::PrimitiveTopology::TriangleFan,
            W3D_PRIMITIVE_TYPE::W3D_LINES => crate::w3d::PrimitiveTopology::LineList,
            W3D_PRIMITIVE_TYPE::W3D_LINE_STRIP => crate::w3d::PrimitiveTopology::LineStrip,
            W3D_PRIMITIVE_TYPE::W3D_POINTS => crate::w3d::PrimitiveTopology::PointList,
        },
        world_matrix: world_matrix.m,
        material_id,
        alpha_blend_enabled,
        render_states: crate::w3d::renderer::deferred_render_state_snapshot(),
    });
    Ok(())
}
pub(super) fn primitive_vertex_count(
    primitive_type: W3D_PRIMITIVE_TYPE,
    primitive_count: u32,
) -> Option<u32> {
    match primitive_type {
        W3D_PRIMITIVE_TYPE::W3D_TRIANGLES => primitive_count.checked_mul(3),
        W3D_PRIMITIVE_TYPE::W3D_TRIANGLE_STRIP | W3D_PRIMITIVE_TYPE::W3D_TRIANGLE_FAN => {
            primitive_count.checked_add(2)
        }
        W3D_PRIMITIVE_TYPE::W3D_LINES => primitive_count.checked_mul(2),
        W3D_PRIMITIVE_TYPE::W3D_LINE_STRIP => primitive_count.checked_add(1),
        W3D_PRIMITIVE_TYPE::W3D_POINTS => Some(primitive_count),
    }
}

pub(super) fn primitive_index_count(
    primitive_type: W3D_PRIMITIVE_TYPE,
    primitive_count: u32,
) -> Option<u32> {
    primitive_vertex_count(primitive_type, primitive_count)
}
