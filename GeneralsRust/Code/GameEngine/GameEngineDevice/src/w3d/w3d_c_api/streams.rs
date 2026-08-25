//! W3D C API stream sources, staged vertices, and index buffers.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::decl::*;
use super::leftover::*;
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

/// Stage vertex stream data for legacy draw-call ordering.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetStreamSource(
    device: W3D_DEVICE,
    stream: u32,
    vertex_data: *const c_void,
    vertex_stride: u32,
    vertex_count: u32,
) -> i32 {
    W3DDevice_SetStreamSourceEx(device, stream, vertex_data, vertex_stride, 0, vertex_count)
}

/// Stage vertex stream data with explicit byte offset semantics.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetStreamSourceEx(
    device: W3D_DEVICE,
    stream: u32,
    vertex_data: *const c_void,
    vertex_stride: u32,
    vertex_offset_bytes: u32,
    vertex_count: u32,
) -> i32 {
    if device.is_null() {
        return 0;
    }
    let device_ref = &*device;

    if vertex_data.is_null() || vertex_stride == 0 || vertex_count == 0 {
        if let Ok(mut stream_sources) = device_ref.stream_sources.lock() {
            stream_sources.remove(&stream);
            return 1;
        }
        return 0;
    }
    let min_stride = if stream == 0 { 12 } else { 4 };
    if !is_valid_ptr(vertex_data) || vertex_stride < min_stride {
        return 0;
    }

    stage_stream_source(
        device_ref,
        stream,
        vertex_data,
        vertex_stride as usize,
        vertex_offset_bytes as usize,
        vertex_count as usize,
    )
}

/// Alias for callers that use explicit UP naming.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetStreamSourceUP(
    device: W3D_DEVICE,
    stream: u32,
    vertex_data: *const c_void,
    vertex_stride: u32,
    vertex_count: u32,
) -> i32 {
    W3DDevice_SetStreamSourceEx(device, stream, vertex_data, vertex_stride, 0, vertex_count)
}

/// Get staged vertex stream source for legacy compatibility/debug.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_GetStreamSource(
    device: W3D_DEVICE,
    stream: u32,
    out_vertex_data: *mut *const c_void,
    out_vertex_stride: *mut u32,
    out_vertex_count: *mut u32,
) -> i32 {
    if device.is_null()
        || out_vertex_data.is_null()
        || out_vertex_stride.is_null()
        || out_vertex_count.is_null()
    {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(sources) = device_ref.stream_sources.lock() {
        if let Some(stream_source) = sources.get(&stream) {
            let ptr = stream_source
                .data
                .get(stream_source.vertex_offset_bytes..)
                .map(|s| s.as_ptr() as *const c_void)
                .unwrap_or(std::ptr::null());
            *out_vertex_data = ptr;
            *out_vertex_stride = stream_source.vertex_stride as u32;
            *out_vertex_count = staged_stream_available_count(stream_source) as u32;
            return 1;
        }
    }

    *out_vertex_data = std::ptr::null();
    *out_vertex_stride = 0;
    *out_vertex_count = 0;
    0
}

/// Get staged vertex stream source including explicit byte offset.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_GetStreamSourceEx(
    device: W3D_DEVICE,
    stream: u32,
    out_vertex_data: *mut *const c_void,
    out_vertex_stride: *mut u32,
    out_vertex_offset_bytes: *mut u32,
    out_vertex_count: *mut u32,
) -> i32 {
    if device.is_null()
        || out_vertex_data.is_null()
        || out_vertex_stride.is_null()
        || out_vertex_offset_bytes.is_null()
        || out_vertex_count.is_null()
    {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(sources) = device_ref.stream_sources.lock() {
        if let Some(stream_source) = sources.get(&stream) {
            let ptr = stream_source
                .data
                .get(stream_source.vertex_offset_bytes..)
                .map(|s| s.as_ptr() as *const c_void)
                .unwrap_or(std::ptr::null());
            *out_vertex_data = ptr;
            *out_vertex_stride = stream_source.vertex_stride as u32;
            *out_vertex_offset_bytes = stream_source.vertex_offset_bytes as u32;
            *out_vertex_count = staged_stream_available_count(stream_source) as u32;
            return 1;
        }
    }

    *out_vertex_data = std::ptr::null();
    *out_vertex_stride = 0;
    *out_vertex_offset_bytes = 0;
    *out_vertex_count = 0;
    0
}

/// Stage index buffer data for legacy draw-call ordering.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetIndices(
    device: W3D_DEVICE,
    index_data: *const u16,
    index_count: u32,
    base_vertex_index: i32,
) -> i32 {
    if device.is_null() {
        return 0;
    }
    let device_ref = &*device;

    if index_data.is_null() || index_count == 0 {
        if let Ok(mut staged_indices) = device_ref.staged_indices.lock() {
            staged_indices.clear();
        }
        if let Ok(mut staged_base) = device_ref.staged_base_vertex_index.lock() {
            *staged_base = 0;
        }
        return 1;
    }
    if !is_valid_ptr(index_data) {
        return 0;
    }

    let source = std::slice::from_raw_parts(index_data, index_count as usize).to_vec();
    if let Ok(mut staged_indices) = device_ref.staged_indices.lock() {
        *staged_indices = source;
    } else {
        return 0;
    }
    if let Ok(mut staged_base) = device_ref.staged_base_vertex_index.lock() {
        *staged_base = base_vertex_index;
        return 1;
    }
    0
}

/// Get staged index buffer for legacy compatibility/debug.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_GetIndices(
    device: W3D_DEVICE,
    out_index_data: *mut *const u16,
    out_index_count: *mut u32,
    out_base_vertex_index: *mut i32,
) -> i32 {
    if device.is_null()
        || out_index_data.is_null()
        || out_index_count.is_null()
        || out_base_vertex_index.is_null()
    {
        return 0;
    }

    let device_ref = &*device;
    let base_vertex_index = if let Ok(base) = device_ref.staged_base_vertex_index.lock() {
        *base
    } else {
        0
    };

    if let Ok(indices) = device_ref.staged_indices.lock() {
        if !indices.is_empty() {
            *out_index_data = indices.as_ptr();
            *out_index_count = indices.len() as u32;
            *out_base_vertex_index = base_vertex_index;
            return 1;
        }
    }

    *out_index_data = std::ptr::null();
    *out_index_count = 0;
    *out_base_vertex_index = base_vertex_index;
    0
}
pub(super) fn stage_stream_source(
    device: &W3DDeviceC,
    stream: u32,
    vertex_data: *const c_void,
    vertex_stride: usize,
    vertex_offset_bytes: usize,
    vertex_count: usize,
) -> i32 {
    let range_bytes = vertex_stride.checked_mul(vertex_count);
    let Some(range_bytes) = range_bytes else {
        return 0;
    };
    let total_bytes = vertex_offset_bytes.checked_add(range_bytes);
    let Some(total_bytes) = total_bytes else {
        return 0;
    };
    if total_bytes == 0 {
        return 0;
    }

    let source =
        unsafe { std::slice::from_raw_parts(vertex_data as *const u8, total_bytes) }.to_vec();
    if let Ok(mut stream_sources) = device.stream_sources.lock() {
        stream_sources.insert(
            stream,
            StagedStreamSource {
                vertex_stride,
                vertex_offset_bytes,
                vertex_count,
                data: source,
            },
        );
        return 1;
    }
    0
}

pub(super) fn staged_stream_available_count(stream_source: &StagedStreamSource) -> usize {
    if stream_source.vertex_stride == 0 {
        return 0;
    }
    let available_bytes = stream_source
        .data
        .len()
        .saturating_sub(stream_source.vertex_offset_bytes);
    let available_by_bytes = available_bytes / stream_source.vertex_stride;
    available_by_bytes.min(stream_source.vertex_count)
}

pub(super) fn staged_stream_base_byte(stream_source: &StagedStreamSource) -> Option<usize> {
    if stream_source.vertex_offset_bytes > stream_source.data.len() {
        return None;
    }
    Some(stream_source.vertex_offset_bytes)
}

pub(super) fn staged_stream_bytes_for_vertex_range<'a>(
    stream_source: &'a StagedStreamSource,
    start_vertex: usize,
    requested_count: usize,
) -> Option<(&'a [u8], usize)> {
    let base = staged_stream_base_byte(stream_source)?;
    let available_count = staged_stream_available_count(stream_source);
    if available_count == 0 || start_vertex >= available_count {
        return None;
    }

    let count = if requested_count == 0 {
        available_count - start_vertex
    } else {
        requested_count.min(available_count - start_vertex)
    };
    if count == 0 {
        return None;
    }

    let start_byte = start_vertex.checked_mul(stream_source.vertex_stride)?;
    let range_offset = base.checked_add(start_byte)?;
    let range_len = count.checked_mul(stream_source.vertex_stride)?;
    let range_end = range_offset.checked_add(range_len)?;
    if range_end > stream_source.data.len() {
        return None;
    }

    Some((&stream_source.data[range_offset..range_end], count))
}

pub(super) fn staged_stream_vertices(
    device: &W3DDeviceC,
    stream: u32,
    requested_count: u32,
) -> Option<Vec<W3D_VERTEX>> {
    let stream_source = device
        .stream_sources
        .lock()
        .ok()
        .and_then(|streams| streams.get(&stream).cloned())?;
    if stream_source.vertex_stride < 12 || stream_source.data.is_empty() {
        return None;
    }

    let requested = requested_count as usize;
    let (source_bytes, decoded_count) =
        staged_stream_bytes_for_vertex_range(&stream_source, 0, requested)?;

    let fvf = current_fvf(device);
    let draw_texture_stage = active_draw_texture_stage(device);
    let draw_texcoord_usage_index = stage_texcoord_usage_index(device, draw_texture_stage);
    let mut vertices = collect_vertices_from_bytes(
        source_bytes,
        decoded_count,
        stream_source.vertex_stride,
        fvf,
        draw_texcoord_usage_index,
    )?;
    overlay_stream_components(device, 0, &mut vertices, fvf);
    Some(vertices)
}

pub(super) fn staged_stream_vertices_range(
    device: &W3DDeviceC,
    stream: u32,
    start_vertex: usize,
    requested_count: usize,
) -> Option<Vec<W3D_VERTEX>> {
    if let Some(declaration_elements) = current_vertex_declaration_elements(device) {
        let draw_texture_stage = active_draw_texture_stage(device);
        let draw_texcoord_usage_index = stage_texcoord_usage_index(device, draw_texture_stage);
        if let Ok(stream_sources) = device.stream_sources.lock() {
            if let Some(vertices) = collect_vertices_from_declaration_streams(
                &stream_sources,
                start_vertex,
                requested_count,
                &declaration_elements,
                draw_texcoord_usage_index,
            ) {
                return Some(vertices);
            }
        }
    }

    let stream_source = device
        .stream_sources
        .lock()
        .ok()
        .and_then(|streams| streams.get(&stream).cloned())?;
    if stream_source.vertex_stride < 12 || stream_source.data.is_empty() {
        return None;
    }

    let (sub_data, decoded_count) =
        staged_stream_bytes_for_vertex_range(&stream_source, start_vertex, requested_count)?;
    let fvf = current_fvf(device);
    let draw_texture_stage = active_draw_texture_stage(device);
    let draw_texcoord_usage_index = stage_texcoord_usage_index(device, draw_texture_stage);
    let mut vertices = collect_vertices_from_bytes(
        sub_data,
        decoded_count,
        stream_source.vertex_stride,
        fvf,
        draw_texcoord_usage_index,
    )?;
    overlay_stream_components(device, start_vertex, &mut vertices, fvf);
    Some(vertices)
}
pub(super) fn staged_index_buffer(
    device: &W3DDeviceC,
    requested_count: u32,
) -> Option<(Vec<u16>, i32)> {
    let requested = requested_count as usize;
    let (indices, base_vertex_index) = staged_index_buffer_range(device, 0, requested)?;
    Some((indices, base_vertex_index))
}

pub(super) fn staged_index_buffer_range(
    device: &W3DDeviceC,
    start_index: usize,
    requested_count: usize,
) -> Option<(Vec<u16>, i32)> {
    let indices = device.staged_indices.lock().ok()?.clone();
    if indices.is_empty() || start_index >= indices.len() {
        return None;
    }

    let requested = if requested_count == 0 {
        indices.len() - start_index
    } else {
        requested_count.min(indices.len() - start_index)
    };
    if requested == 0 {
        return None;
    }
    let end = start_index.checked_add(requested)?;
    let indices = indices.get(start_index..end)?.to_vec();
    let base_vertex_index = device
        .staged_base_vertex_index
        .lock()
        .ok()
        .map(|v| *v)
        .unwrap_or(0);
    Some((indices, base_vertex_index))
}
