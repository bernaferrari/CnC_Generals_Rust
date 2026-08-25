//! Vertex declaration / FVF decode helpers for the W3D C API.
//!
//! Split from `w3d_c_api.rs`.

use super::constants::*;
use super::leftover::*;
use super::streams::*;
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

pub(super) fn overlay_stream_components(
    device: &W3DDeviceC,
    start_vertex: usize,
    vertices: &mut [W3D_VERTEX],
    fvf: u32,
) {
    if vertices.is_empty() {
        return;
    }

    let declaration_active = current_vertex_declaration(device) != 0;
    let draw_texture_stage = active_draw_texture_stage(device);
    let draw_texcoord_usage_index = stage_texcoord_usage_index(device, draw_texture_stage);
    let Ok(stream_sources) = device.stream_sources.lock() else {
        return;
    };
    let declaration_elements = if declaration_active {
        current_vertex_declaration_elements(device)
    } else {
        None
    };
    let applied_decl = declaration_elements
        .as_ref()
        .map(|elements| {
            overlay_stream_components_from_declaration(
                &stream_sources,
                start_vertex,
                vertices,
                elements,
                draw_texcoord_usage_index,
            )
        })
        .unwrap_or_default();

    let mut need_uv = if declaration_active {
        !applied_decl.uv
    } else {
        fvf_tex_count(fvf) == 0
    };
    let mut need_normal = if declaration_active {
        !applied_decl.normal
    } else {
        !fvf_has_normal(fvf)
    };
    let mut need_color = if declaration_active {
        !applied_decl.color
    } else {
        !fvf_has_diffuse(fvf)
    };
    if !need_uv && !need_normal && !need_color {
        return;
    }

    let preferred_uv_stream = if draw_texcoord_usage_index > 0 {
        Some(draw_texcoord_usage_index as u32)
    } else {
        None
    };
    if need_uv {
        if let Some(stream_id) = preferred_uv_stream {
            if let Some(source) = stream_sources.get(&stream_id) {
                if apply_stream_uv_overlay(source, start_vertex, vertices) {
                    need_uv = false;
                }
            }
        }
    }

    let mut stream_ids = stream_sources.keys().copied().collect::<Vec<_>>();
    stream_ids.sort_unstable();
    for stream_id in stream_ids {
        if stream_id == 0 {
            continue;
        }
        if !need_uv && !need_normal && !need_color {
            break;
        }

        let Some(source) = stream_sources.get(&stream_id) else {
            continue;
        };
        let stride = source.vertex_stride;
        if stride < 4 {
            continue;
        }
        let Some(base_offset) = staged_stream_base_byte(source) else {
            continue;
        };
        let available_count = staged_stream_available_count(source);
        if available_count <= start_vertex {
            continue;
        }
        let count = vertices.len().min(available_count - start_vertex);
        if need_uv && stride >= 8 {
            if apply_stream_uv_overlay(source, start_vertex, vertices) {
                need_uv = false;
            }
        }

        if need_normal && stride >= 12 {
            let mut applied = false;
            for (i, vertex) in vertices.iter_mut().take(count).enumerate() {
                let stream_offset = (start_vertex + i) * stride;
                let Some(base) = base_offset.checked_add(stream_offset) else {
                    break;
                };
                let end = base + stride;
                if end > source.data.len() {
                    break;
                }
                let bytes = &source.data[base..end];
                if let (Some(nx), Some(ny), Some(nz)) = (
                    read_f32_at(bytes, 0),
                    read_f32_at(bytes, 4),
                    read_f32_at(bytes, 8),
                ) {
                    if nx.is_finite() && ny.is_finite() && nz.is_finite() {
                        vertex.nx = nx;
                        vertex.ny = ny;
                        vertex.nz = nz;
                        applied = true;
                    }
                }
            }
            if applied {
                need_normal = false;
            }
        }

        if need_color && stride >= 4 {
            let mut applied = false;
            for (i, vertex) in vertices.iter_mut().take(count).enumerate() {
                let stream_offset = (start_vertex + i) * stride;
                let Some(base) = base_offset.checked_add(stream_offset) else {
                    break;
                };
                let end = base + stride;
                if end > source.data.len() {
                    break;
                }
                let bytes = &source.data[base..end];
                if let Some(color) = read_u32_at(bytes, 0) {
                    vertex.color = color;
                    applied = true;
                }
            }
            if applied {
                need_color = false;
            }
        }
    }
}

#[derive(Default)]
pub(super) struct DeclOverlayApplied {
    pub(super) uv: bool,
    pub(super) normal: bool,
    pub(super) color: bool,
}

pub(super) fn overlay_stream_components_from_declaration(
    stream_sources: &HashMap<u32, StagedStreamSource>,
    start_vertex: usize,
    vertices: &mut [W3D_VERTEX],
    elements: &[W3D_VERTEX_ELEMENT],
    uv_usage_index: u8,
) -> DeclOverlayApplied {
    let mut applied = DeclOverlayApplied::default();

    if let Some(element) =
        declaration_element_for_usage(elements, D3DDECLUSAGE_TEXCOORD, uv_usage_index)
            .or_else(|| declaration_element_for_usage(elements, D3DDECLUSAGE_TEXCOORD, 0))
    {
        applied.uv = apply_declared_uv(stream_sources, start_vertex, vertices, element);
    }
    if let Some(element) = declaration_element_for_usage(elements, D3DDECLUSAGE_NORMAL, 0) {
        applied.normal = apply_declared_normal(stream_sources, start_vertex, vertices, element);
    }
    if let Some(element) = declaration_element_for_usage(elements, D3DDECLUSAGE_COLOR, 0) {
        applied.color = apply_declared_color(stream_sources, start_vertex, vertices, element);
    }

    applied
}

pub(super) fn collect_vertices_from_declaration_streams(
    stream_sources: &HashMap<u32, StagedStreamSource>,
    start_vertex: usize,
    requested_count: usize,
    elements: &[W3D_VERTEX_ELEMENT],
    uv_usage_index: u8,
) -> Option<Vec<W3D_VERTEX>> {
    let position_element = declaration_element_for_usage(elements, D3DDECLUSAGE_POSITION, 0)
        .or_else(|| declaration_element_for_usage(elements, D3DDECLUSAGE_POSITIONT, 0))?;
    let position_stream = stream_sources.get(&(position_element.stream as u32))?;
    let available_count = staged_stream_available_count(position_stream);
    if available_count <= start_vertex {
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

    let normal_element = declaration_element_for_usage(elements, D3DDECLUSAGE_NORMAL, 0);
    let color_element = declaration_element_for_usage(elements, D3DDECLUSAGE_COLOR, 0);
    let uv_element = declaration_element_for_usage(elements, D3DDECLUSAGE_TEXCOORD, uv_usage_index)
        .or_else(|| declaration_element_for_usage(elements, D3DDECLUSAGE_TEXCOORD, 0));
    let mut vertices = Vec::with_capacity(count);

    for i in 0..count {
        let vertex_index = start_vertex + i;
        let position_bytes = stream_vertex_bytes(position_stream, vertex_index)?;
        let (x, y, z) = read_position_from_decl(
            position_bytes,
            position_element.offset as usize,
            position_element.decl_type,
        )?;
        let mut vertex = W3D_VERTEX {
            x,
            y,
            z,
            nx: 0.0,
            ny: 0.0,
            nz: 1.0,
            u: 0.0,
            v: 0.0,
            color: 0xFFFF_FFFF,
        };

        if let Some(element) = normal_element {
            if let Some(source) = stream_sources.get(&(element.stream as u32)) {
                if let Some(bytes) = stream_vertex_bytes(source, vertex_index) {
                    if let Some((nx, ny, nz)) =
                        read_normal_from_decl(bytes, element.offset as usize, element.decl_type)
                    {
                        if nx.is_finite() && ny.is_finite() && nz.is_finite() {
                            vertex.nx = nx;
                            vertex.ny = ny;
                            vertex.nz = nz;
                        }
                    }
                }
            }
        }

        if let Some(element) = color_element {
            if let Some(source) = stream_sources.get(&(element.stream as u32)) {
                if let Some(bytes) = stream_vertex_bytes(source, vertex_index) {
                    if let Some(color) =
                        read_color_from_decl(bytes, element.offset as usize, element.decl_type)
                    {
                        vertex.color = color;
                    }
                }
            }
        }

        if let Some(element) = uv_element {
            if let Some(source) = stream_sources.get(&(element.stream as u32)) {
                if let Some(bytes) = stream_vertex_bytes(source, vertex_index) {
                    if let Some((u, v)) =
                        read_uv_from_decl(bytes, element.offset as usize, element.decl_type)
                    {
                        if u.is_finite() && v.is_finite() {
                            vertex.u = u;
                            vertex.v = v;
                        }
                    }
                }
            }
        }

        vertices.push(vertex);
    }

    Some(vertices)
}
pub(super) fn apply_stream_uv_overlay(
    source: &StagedStreamSource,
    start_vertex: usize,
    vertices: &mut [W3D_VERTEX],
) -> bool {
    if source.vertex_stride < 8 {
        return false;
    }
    let Some(base_offset) = staged_stream_base_byte(source) else {
        return false;
    };
    let available_count = staged_stream_available_count(source);
    if available_count <= start_vertex {
        return false;
    }
    let count = vertices.len().min(available_count - start_vertex);
    let mut applied = false;
    for (i, vertex) in vertices.iter_mut().take(count).enumerate() {
        let stream_offset = (start_vertex + i) * source.vertex_stride;
        let Some(base) = base_offset.checked_add(stream_offset) else {
            break;
        };
        let end = base + source.vertex_stride;
        if end > source.data.len() {
            break;
        }
        let bytes = &source.data[base..end];
        if let (Some(u), Some(v)) = (read_f32_at(bytes, 0), read_f32_at(bytes, 4)) {
            if u.is_finite() && v.is_finite() {
                vertex.u = u;
                vertex.v = v;
                applied = true;
            }
        }
    }
    applied
}

pub(super) fn declaration_element_for_usage(
    elements: &[W3D_VERTEX_ELEMENT],
    usage: u8,
    usage_index: u8,
) -> Option<W3D_VERTEX_ELEMENT> {
    elements
        .iter()
        .copied()
        .find(|element| element.usage == usage && element.usage_index == usage_index)
}

pub(super) fn apply_declared_uv(
    stream_sources: &HashMap<u32, StagedStreamSource>,
    start_vertex: usize,
    vertices: &mut [W3D_VERTEX],
    element: W3D_VERTEX_ELEMENT,
) -> bool {
    let Some(source) = stream_sources.get(&(element.stream as u32)) else {
        return false;
    };

    let available_count = staged_stream_available_count(source);
    if available_count <= start_vertex {
        return false;
    }
    let count = vertices.len().min(available_count - start_vertex);
    let mut applied = false;
    for (i, vertex) in vertices.iter_mut().take(count).enumerate() {
        let Some(bytes) = stream_vertex_bytes(source, start_vertex + i) else {
            break;
        };
        if let Some((u, v)) = read_uv_from_decl(bytes, element.offset as usize, element.decl_type) {
            if u.is_finite() && v.is_finite() {
                vertex.u = u;
                vertex.v = v;
                applied = true;
            }
        }
    }
    applied
}

pub(super) fn apply_declared_normal(
    stream_sources: &HashMap<u32, StagedStreamSource>,
    start_vertex: usize,
    vertices: &mut [W3D_VERTEX],
    element: W3D_VERTEX_ELEMENT,
) -> bool {
    let Some(source) = stream_sources.get(&(element.stream as u32)) else {
        return false;
    };

    let available_count = staged_stream_available_count(source);
    if available_count <= start_vertex {
        return false;
    }
    let count = vertices.len().min(available_count - start_vertex);
    let mut applied = false;
    for (i, vertex) in vertices.iter_mut().take(count).enumerate() {
        let Some(bytes) = stream_vertex_bytes(source, start_vertex + i) else {
            break;
        };
        if let Some((nx, ny, nz)) =
            read_normal_from_decl(bytes, element.offset as usize, element.decl_type)
        {
            if nx.is_finite() && ny.is_finite() && nz.is_finite() {
                vertex.nx = nx;
                vertex.ny = ny;
                vertex.nz = nz;
                applied = true;
            }
        }
    }
    applied
}

pub(super) fn apply_declared_color(
    stream_sources: &HashMap<u32, StagedStreamSource>,
    start_vertex: usize,
    vertices: &mut [W3D_VERTEX],
    element: W3D_VERTEX_ELEMENT,
) -> bool {
    let Some(source) = stream_sources.get(&(element.stream as u32)) else {
        return false;
    };

    let available_count = staged_stream_available_count(source);
    if available_count <= start_vertex {
        return false;
    }
    let count = vertices.len().min(available_count - start_vertex);
    let mut applied = false;
    for (i, vertex) in vertices.iter_mut().take(count).enumerate() {
        let Some(bytes) = stream_vertex_bytes(source, start_vertex + i) else {
            break;
        };
        if let Some(color) = read_color_from_decl(bytes, element.offset as usize, element.decl_type)
        {
            vertex.color = color;
            applied = true;
        }
    }
    applied
}

pub(super) fn stream_vertex_bytes(
    source: &StagedStreamSource,
    vertex_index: usize,
) -> Option<&[u8]> {
    let base_offset = staged_stream_base_byte(source)?;
    let stream_offset = vertex_index.checked_mul(source.vertex_stride)?;
    let base = base_offset.checked_add(stream_offset)?;
    let end = base.checked_add(source.vertex_stride)?;
    if end > source.data.len() {
        return None;
    }
    Some(&source.data[base..end])
}

pub(super) fn read_uv_from_decl(bytes: &[u8], offset: usize, decl_type: u8) -> Option<(f32, f32)> {
    match decl_type {
        D3DDECLTYPE_FLOAT1 => Some((read_f32_at(bytes, offset)?, 0.0)),
        D3DDECLTYPE_FLOAT2 | D3DDECLTYPE_FLOAT3 | D3DDECLTYPE_FLOAT4 => {
            Some((read_f32_at(bytes, offset)?, read_f32_at(bytes, offset + 4)?))
        }
        D3DDECLTYPE_SHORT2 => Some((
            read_i16_at(bytes, offset)? as f32,
            read_i16_at(bytes, offset + 2)? as f32,
        )),
        D3DDECLTYPE_SHORT4 => Some((
            read_i16_at(bytes, offset)? as f32,
            read_i16_at(bytes, offset + 2)? as f32,
        )),
        D3DDECLTYPE_SHORT2N => Some((
            normalize_i16(read_i16_at(bytes, offset)?),
            normalize_i16(read_i16_at(bytes, offset + 2)?),
        )),
        D3DDECLTYPE_SHORT4N => Some((
            normalize_i16(read_i16_at(bytes, offset)?),
            normalize_i16(read_i16_at(bytes, offset + 2)?),
        )),
        D3DDECLTYPE_USHORT2N => Some((
            normalize_u16(read_u16_at(bytes, offset)?),
            normalize_u16(read_u16_at(bytes, offset + 2)?),
        )),
        D3DDECLTYPE_USHORT4N => Some((
            normalize_u16(read_u16_at(bytes, offset)?),
            normalize_u16(read_u16_at(bytes, offset + 2)?),
        )),
        D3DDECLTYPE_UDEC3 => {
            let packed = read_u32_at(bytes, offset)?;
            let (x, y, _) = unpack_udec3(packed);
            Some((x, y))
        }
        D3DDECLTYPE_DEC3N => {
            let packed = read_u32_at(bytes, offset)?;
            let (x, y, _) = unpack_dec3n(packed);
            Some((x, y))
        }
        _ => None,
    }
}

pub(super) fn read_position_from_decl(
    bytes: &[u8],
    offset: usize,
    decl_type: u8,
) -> Option<(f32, f32, f32)> {
    match decl_type {
        D3DDECLTYPE_FLOAT1 => Some((read_f32_at(bytes, offset)?, 0.0, 0.0)),
        D3DDECLTYPE_FLOAT2 => Some((
            read_f32_at(bytes, offset)?,
            read_f32_at(bytes, offset + 4)?,
            0.0,
        )),
        D3DDECLTYPE_FLOAT3 | D3DDECLTYPE_FLOAT4 => Some((
            read_f32_at(bytes, offset)?,
            read_f32_at(bytes, offset + 4)?,
            read_f32_at(bytes, offset + 8)?,
        )),
        D3DDECLTYPE_SHORT2 => Some((
            read_i16_at(bytes, offset)? as f32,
            read_i16_at(bytes, offset + 2)? as f32,
            0.0,
        )),
        D3DDECLTYPE_SHORT2N => Some((
            normalize_i16(read_i16_at(bytes, offset)?),
            normalize_i16(read_i16_at(bytes, offset + 2)?),
            0.0,
        )),
        D3DDECLTYPE_USHORT2N => Some((
            normalize_u16(read_u16_at(bytes, offset)?),
            normalize_u16(read_u16_at(bytes, offset + 2)?),
            0.0,
        )),
        D3DDECLTYPE_SHORT4 => Some((
            read_i16_at(bytes, offset)? as f32,
            read_i16_at(bytes, offset + 2)? as f32,
            read_i16_at(bytes, offset + 4)? as f32,
        )),
        D3DDECLTYPE_SHORT4N => Some((
            normalize_i16(read_i16_at(bytes, offset)?),
            normalize_i16(read_i16_at(bytes, offset + 2)?),
            normalize_i16(read_i16_at(bytes, offset + 4)?),
        )),
        D3DDECLTYPE_USHORT4N => Some((
            normalize_u16(read_u16_at(bytes, offset)?),
            normalize_u16(read_u16_at(bytes, offset + 2)?),
            normalize_u16(read_u16_at(bytes, offset + 4)?),
        )),
        D3DDECLTYPE_UBYTE4 => Some((
            read_u8_at(bytes, offset)? as f32,
            read_u8_at(bytes, offset + 1)? as f32,
            read_u8_at(bytes, offset + 2)? as f32,
        )),
        D3DDECLTYPE_UBYTE4N => Some((
            normalize_u8(read_u8_at(bytes, offset)?),
            normalize_u8(read_u8_at(bytes, offset + 1)?),
            normalize_u8(read_u8_at(bytes, offset + 2)?),
        )),
        D3DDECLTYPE_UDEC3 => {
            let packed = read_u32_at(bytes, offset)?;
            Some(unpack_udec3(packed))
        }
        D3DDECLTYPE_DEC3N => {
            let packed = read_u32_at(bytes, offset)?;
            Some(unpack_dec3n(packed))
        }
        _ => None,
    }
}

pub(super) fn read_normal_from_decl(
    bytes: &[u8],
    offset: usize,
    decl_type: u8,
) -> Option<(f32, f32, f32)> {
    match decl_type {
        D3DDECLTYPE_FLOAT2 => Some((
            read_f32_at(bytes, offset)?,
            read_f32_at(bytes, offset + 4)?,
            0.0,
        )),
        D3DDECLTYPE_FLOAT3 | D3DDECLTYPE_FLOAT4 => Some((
            read_f32_at(bytes, offset)?,
            read_f32_at(bytes, offset + 4)?,
            read_f32_at(bytes, offset + 8)?,
        )),
        D3DDECLTYPE_SHORT4N => Some((
            normalize_i16(read_i16_at(bytes, offset)?),
            normalize_i16(read_i16_at(bytes, offset + 2)?),
            normalize_i16(read_i16_at(bytes, offset + 4)?),
        )),
        D3DDECLTYPE_UBYTE4N => Some((
            normalize_u8(read_u8_at(bytes, offset)?),
            normalize_u8(read_u8_at(bytes, offset + 1)?),
            normalize_u8(read_u8_at(bytes, offset + 2)?),
        )),
        D3DDECLTYPE_SHORT4 => Some((
            read_i16_at(bytes, offset)? as f32,
            read_i16_at(bytes, offset + 2)? as f32,
            read_i16_at(bytes, offset + 4)? as f32,
        )),
        D3DDECLTYPE_USHORT4N => Some((
            normalize_u16(read_u16_at(bytes, offset)?),
            normalize_u16(read_u16_at(bytes, offset + 2)?),
            normalize_u16(read_u16_at(bytes, offset + 4)?),
        )),
        D3DDECLTYPE_UDEC3 => {
            let packed = read_u32_at(bytes, offset)?;
            Some(unpack_udec3(packed))
        }
        D3DDECLTYPE_DEC3N => {
            let packed = read_u32_at(bytes, offset)?;
            Some(unpack_dec3n(packed))
        }
        _ => None,
    }
}

pub(super) fn read_color_from_decl(bytes: &[u8], offset: usize, decl_type: u8) -> Option<u32> {
    match decl_type {
        D3DDECLTYPE_D3DCOLOR => read_u32_at(bytes, offset),
        D3DDECLTYPE_UBYTE4 | D3DDECLTYPE_UBYTE4N => {
            let r = read_u8_at(bytes, offset)?;
            let g = read_u8_at(bytes, offset + 1)?;
            let b = read_u8_at(bytes, offset + 2)?;
            let a = read_u8_at(bytes, offset + 3)?;
            Some(pack_argb(a, r, g, b))
        }
        D3DDECLTYPE_FLOAT4 => Some(pack_color_f32(
            read_f32_at(bytes, offset)?,
            read_f32_at(bytes, offset + 4)?,
            read_f32_at(bytes, offset + 8)?,
            read_f32_at(bytes, offset + 12)?,
        )),
        D3DDECLTYPE_FLOAT3 => Some(pack_color_f32(
            read_f32_at(bytes, offset)?,
            read_f32_at(bytes, offset + 4)?,
            read_f32_at(bytes, offset + 8)?,
            1.0,
        )),
        _ => None,
    }
}

pub(super) fn normalize_i16(value: i16) -> f32 {
    (value as f32 / 32767.0).clamp(-1.0, 1.0)
}

pub(super) fn normalize_u16(value: u16) -> f32 {
    value as f32 / 65535.0
}

pub(super) fn normalize_u8(value: u8) -> f32 {
    value as f32 / 255.0
}

pub(super) fn unpack_udec3(packed: u32) -> (f32, f32, f32) {
    let x = (packed & 0x3FF) as f32;
    let y = ((packed >> 10) & 0x3FF) as f32;
    let z = ((packed >> 20) & 0x3FF) as f32;
    (x, y, z)
}

pub(super) fn unpack_dec3n(packed: u32) -> (f32, f32, f32) {
    let sx = sign_extend_10((packed & 0x3FF) as i32);
    let sy = sign_extend_10(((packed >> 10) & 0x3FF) as i32);
    let sz = sign_extend_10(((packed >> 20) & 0x3FF) as i32);
    (
        (sx as f32 / 511.0).clamp(-1.0, 1.0),
        (sy as f32 / 511.0).clamp(-1.0, 1.0),
        (sz as f32 / 511.0).clamp(-1.0, 1.0),
    )
}

pub(super) fn sign_extend_10(value: i32) -> i32 {
    if (value & 0x200) != 0 {
        value | !0x3FF
    } else {
        value
    }
}

pub(super) fn pack_color_f32(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let to_u8 = |value: f32| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
    pack_argb(to_u8(a), to_u8(r), to_u8(g), to_u8(b))
}

pub(super) fn pack_argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub(super) fn fvf_has_normal(fvf: u32) -> bool {
    (fvf & D3DFVF_NORMAL) != 0
}

pub(super) fn fvf_has_diffuse(fvf: u32) -> bool {
    (fvf & D3DFVF_DIFFUSE) != 0
}

pub(super) fn fvf_tex_count(fvf: u32) -> usize {
    ((fvf & D3DFVF_TEXCOUNT_MASK) >> D3DFVF_TEXCOUNT_SHIFT) as usize
}

pub(super) fn fvf_texcoord_dimension(fvf: u32, texcoord_set_index: usize) -> usize {
    if texcoord_set_index >= 8 {
        return 2;
    }
    let shift = D3DFVF_TEXCOORDFORMAT_SHIFT + (texcoord_set_index as u32 * 2);
    let format_code = (fvf >> shift) & D3DFVF_TEXCOORDFORMAT_MASK;
    match format_code {
        // D3DFVF_TEXTUREFORMAT2 (default)
        0 => 2,
        // D3DFVF_TEXTUREFORMAT3
        1 => 3,
        // D3DFVF_TEXTUREFORMAT4
        2 => 4,
        // D3DFVF_TEXTUREFORMAT1
        3 => 1,
        _ => 2,
    }
}
pub(super) fn collect_up_vertices(
    vertex_data: *const c_void,
    vertex_count: usize,
    vertex_stride: usize,
    fvf: u32,
    texcoord_usage_index: u8,
) -> Option<Vec<W3D_VERTEX>> {
    let total_bytes = vertex_count.checked_mul(vertex_stride)?;
    if total_bytes == 0 || vertex_stride < 12 {
        return None;
    }

    let bytes = unsafe { std::slice::from_raw_parts(vertex_data as *const u8, total_bytes) };
    collect_vertices_from_bytes(
        bytes,
        vertex_count,
        vertex_stride,
        fvf,
        texcoord_usage_index,
    )
}

pub(super) fn collect_up_indices(
    index_data: *const c_void,
    index_count: usize,
    index_format: u32,
) -> Option<Vec<u32>> {
    if index_count == 0 {
        return Some(Vec::new());
    }

    match index_format {
        0 | D3DFMT_INDEX16 => {
            let indices =
                unsafe { std::slice::from_raw_parts(index_data as *const u16, index_count) };
            Some(indices.iter().map(|&v| v as u32).collect())
        }
        D3DFMT_INDEX32 => {
            let indices =
                unsafe { std::slice::from_raw_parts(index_data as *const u32, index_count) };
            Some(indices.to_vec())
        }
        _ => None,
    }
}

pub(super) fn collect_vertices_from_bytes(
    vertex_data: &[u8],
    vertex_count: usize,
    vertex_stride: usize,
    fvf: u32,
    texcoord_usage_index: u8,
) -> Option<Vec<W3D_VERTEX>> {
    let vertex_size = std::mem::size_of::<W3D_VERTEX>();
    let total_bytes = vertex_count.checked_mul(vertex_stride)?;
    if total_bytes == 0 || vertex_stride < 12 || total_bytes > vertex_data.len() {
        return None;
    }

    if fvf == 0 && vertex_stride >= vertex_size {
        let mut vertices = Vec::with_capacity(vertex_count);
        for i in 0..vertex_count {
            let offset = i.checked_mul(vertex_stride)?;
            let end = offset.checked_add(vertex_size)?;
            let bytes = vertex_data.get(offset..end)?;
            let ptr = bytes.as_ptr() as *const W3D_VERTEX;
            let vertex = unsafe { std::ptr::read_unaligned(ptr) };
            vertices.push(vertex);
        }
        return Some(vertices);
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let offset = i.checked_mul(vertex_stride)?;
        let end = offset.checked_add(vertex_stride)?;
        let bytes = vertex_data.get(offset..end)?;
        let vertex = decode_fvf_vertex(bytes, fvf, texcoord_usage_index)?;
        vertices.push(vertex);
    }
    Some(vertices)
}

pub(super) fn decode_fvf_vertex(
    vertex_bytes: &[u8],
    fvf: u32,
    texcoord_usage_index: u8,
) -> Option<W3D_VERTEX> {
    let mut offset = 0usize;
    let effective_fvf = if fvf != 0 {
        fvf
    } else if vertex_bytes.len() == 32 {
        DEFAULT_FVF_TL1
    } else {
        D3DFVF_XYZ | D3DFVF_DIFFUSE | (1 << D3DFVF_TEXCOUNT_SHIFT)
    };

    let x = read_f32(vertex_bytes, &mut offset)?;
    let y = read_f32(vertex_bytes, &mut offset)?;
    let z = read_f32(vertex_bytes, &mut offset)?;
    if (effective_fvf & D3DFVF_XYZRHW) != 0 {
        let _ = read_f32(vertex_bytes, &mut offset)?;
    } else if (effective_fvf & D3DFVF_XYZ) == 0 {
        return None;
    }

    let (nx, ny, nz) = if (effective_fvf & D3DFVF_NORMAL) != 0 {
        (
            read_f32(vertex_bytes, &mut offset)?,
            read_f32(vertex_bytes, &mut offset)?,
            read_f32(vertex_bytes, &mut offset)?,
        )
    } else {
        (0.0, 0.0, 1.0)
    };

    let color = if (effective_fvf & D3DFVF_DIFFUSE) != 0 {
        read_u32(vertex_bytes, &mut offset)?
    } else {
        0xFFFF_FFFF
    };

    if (effective_fvf & D3DFVF_SPECULAR) != 0 {
        let _ = read_u32(vertex_bytes, &mut offset)?;
    }

    let tex_count = ((effective_fvf & D3DFVF_TEXCOUNT_MASK) >> D3DFVF_TEXCOUNT_SHIFT) as usize;
    let (u, v) = if tex_count > 0 {
        let selected_set = (texcoord_usage_index as usize).min(tex_count.saturating_sub(1));
        let mut selected_uv = None;
        for set_index in 0..tex_count {
            let texcoord_dimension = fvf_texcoord_dimension(effective_fvf, set_index);
            if texcoord_dimension == 0 {
                return None;
            }
            let tu = read_f32(vertex_bytes, &mut offset)?;
            let tv = if texcoord_dimension >= 2 {
                read_f32(vertex_bytes, &mut offset)?
            } else {
                0.0
            };
            for _ in 2..texcoord_dimension {
                let _ = read_f32(vertex_bytes, &mut offset)?;
            }
            if set_index == selected_set {
                selected_uv = Some((tu, tv));
            }
        }
        selected_uv.unwrap_or((0.0, 0.0))
    } else {
        (0.0, 0.0)
    };

    Some(W3D_VERTEX {
        x,
        y,
        z,
        nx,
        ny,
        nz,
        u,
        v,
        color,
    })
}
