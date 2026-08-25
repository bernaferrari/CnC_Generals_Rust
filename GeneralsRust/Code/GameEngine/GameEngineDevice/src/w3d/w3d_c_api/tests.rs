//! Unit tests for the W3D C API compatibility layer.
//!
//! Split from `w3d_c_api.rs`. Observable behavior is unchanged.

use super::constants::*;
use super::decl::*;
use super::device::*;
use super::draw::*;
use super::leftover::*;
use super::lighting::*;
use super::materials::*;
use super::math::*;
use super::render_state::*;
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

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[test]
fn decode_fvf_vertex_uses_selected_texcoord_set() {
    let fvf = D3DFVF_XYZ | D3DFVF_DIFFUSE | (2 << D3DFVF_TEXCOUNT_SHIFT);
    let mut bytes = Vec::new();
    // XYZ
    push_f32(&mut bytes, 1.0);
    push_f32(&mut bytes, 2.0);
    push_f32(&mut bytes, 3.0);
    // Diffuse
    push_u32(&mut bytes, 0xFF112233);
    // UV set 0
    push_f32(&mut bytes, 0.1);
    push_f32(&mut bytes, 0.2);
    // UV set 1
    push_f32(&mut bytes, 0.7);
    push_f32(&mut bytes, 0.8);

    let vertex = decode_fvf_vertex(&bytes, fvf, 1).expect("decode_fvf_vertex");
    assert!((vertex.u - 0.7).abs() < 1e-6);
    assert!((vertex.v - 0.8).abs() < 1e-6);
}

#[test]
fn decode_fvf_vertex_honors_texcoord_dimension_one() {
    // One texcoord set with dimension 1 (D3DFVF_TEXTUREFORMAT1 => format code 3).
    let fvf = D3DFVF_XYZ
        | D3DFVF_DIFFUSE
        | (1 << D3DFVF_TEXCOUNT_SHIFT)
        | (3 << D3DFVF_TEXCOORDFORMAT_SHIFT);
    let mut bytes = Vec::new();
    // XYZ
    push_f32(&mut bytes, -1.0);
    push_f32(&mut bytes, -2.0);
    push_f32(&mut bytes, -3.0);
    // Diffuse
    push_u32(&mut bytes, 0xFF445566);
    // Single U component only.
    push_f32(&mut bytes, 0.42);

    let vertex = decode_fvf_vertex(&bytes, fvf, 0).expect("decode_fvf_vertex");
    assert!((vertex.u - 0.42).abs() < 1e-6);
    assert!(vertex.v.abs() < 1e-6);
}

#[test]
fn declaration_stream_decode_uses_nonzero_position_stream() {
    let mut streams = HashMap::new();

    // Stream 0: UV only.
    let mut uv_bytes = Vec::new();
    push_f32(&mut uv_bytes, 0.25);
    push_f32(&mut uv_bytes, 0.75);
    streams.insert(
        0,
        StagedStreamSource {
            vertex_stride: 8,
            vertex_offset_bytes: 0,
            vertex_count: 1,
            data: uv_bytes,
        },
    );

    // Stream 1: Position only.
    let mut pos_bytes = Vec::new();
    push_f32(&mut pos_bytes, 10.0);
    push_f32(&mut pos_bytes, 20.0);
    push_f32(&mut pos_bytes, 30.0);
    streams.insert(
        1,
        StagedStreamSource {
            vertex_stride: 12,
            vertex_offset_bytes: 0,
            vertex_count: 1,
            data: pos_bytes,
        },
    );

    let elements = vec![
        W3D_VERTEX_ELEMENT {
            stream: 1,
            offset: 0,
            decl_type: D3DDECLTYPE_FLOAT3,
            method: 0,
            usage: D3DDECLUSAGE_POSITION,
            usage_index: 0,
        },
        W3D_VERTEX_ELEMENT {
            stream: 0,
            offset: 0,
            decl_type: D3DDECLTYPE_FLOAT2,
            method: 0,
            usage: D3DDECLUSAGE_TEXCOORD,
            usage_index: 0,
        },
    ];

    let vertices = collect_vertices_from_declaration_streams(&streams, 0, 1, &elements, 0)
        .expect("declaration vertices");
    assert_eq!(vertices.len(), 1);
    let vertex = vertices[0];
    assert!((vertex.x - 10.0).abs() < 1e-6);
    assert!((vertex.y - 20.0).abs() < 1e-6);
    assert!((vertex.z - 30.0).abs() < 1e-6);
    assert!((vertex.u - 0.25).abs() < 1e-6);
    assert!((vertex.v - 0.75).abs() < 1e-6);
}

#[test]
fn resolve_active_texture_stage_skips_non_sampling_stage_zero() {
    let mut states = HashMap::new();

    // Stage 0 is enabled but does not sample texture (SELECTARG2 CURRENT).
    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG2);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    // Stage 1 performs texture sampling.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 1);
}

#[test]
fn resolve_active_texture_stage_prefers_stage_zero_when_sampling() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 0);
}

#[test]
fn op_uses_texture_arg_respects_selectarg2_current() {
    assert!(!op_uses_texture_arg(
        D3DTOP_SELECTARG2,
        D3DTA_CURRENT,
        D3DTA_TEXTURE,
        D3DTA_CURRENT
    ));
    assert!(op_uses_texture_arg(
        D3DTOP_SELECTARG2,
        D3DTA_CURRENT,
        D3DTA_CURRENT,
        D3DTA_TEXTURE
    ));
}

#[test]
fn op_uses_texture_arg_detects_lerp_arg0_texture() {
    assert!(op_uses_texture_arg(
        D3DTOP_LERP,
        D3DTA_TEXTURE,
        D3DTA_CURRENT,
        D3DTA_CURRENT
    ));
    assert!(!op_uses_texture_arg(
        D3DTOP_LERP,
        D3DTA_CURRENT,
        D3DTA_CURRENT,
        D3DTA_CURRENT
    ));
}

#[test]
fn resolve_active_texture_stage_considers_arg0_sampling_ops() {
    let mut states = HashMap::new();

    // Stage 0 enabled with LERP, but no texture sampling in any used arg.
    states.insert((0, D3DTSS_COLOROP), D3DTOP_LERP);
    states.insert((0, D3DTSS_COLORARG0), D3DTA_CURRENT);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    // Stage 1 uses texture in LERP arg0.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_LERP);
    states.insert((1, D3DTSS_COLORARG0), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 1);
}

#[test]
fn resolve_active_texture_stage_prefers_color_sampling_over_alpha_only_stage_zero() {
    let mut states = HashMap::new();

    // Stage 0: color path does not sample texture, alpha path does.
    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG2);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TEXTURE);

    // Stage 1: color path samples texture and should be preferred.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_BLENDCURRENTALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 1);
}

#[test]
fn resolve_active_texture_stage_falls_back_to_alpha_sampling_when_no_color_sampling_exists() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG2);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TEXTURE);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_SELECTARG2);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 0);
}

#[test]
fn op_uses_texture_arg_detects_add_and_dotproduct3_texture_args() {
    assert!(op_uses_texture_arg(
        D3DTOP_ADD,
        D3DTA_CURRENT,
        D3DTA_TEXTURE,
        D3DTA_CURRENT
    ));
    assert!(op_uses_texture_arg(
        D3DTOP_DOTPRODUCT3,
        D3DTA_CURRENT,
        D3DTA_CURRENT,
        D3DTA_TEXTURE
    ));
    assert!(!op_uses_texture_arg(
        D3DTOP_ADD,
        D3DTA_CURRENT,
        D3DTA_CURRENT,
        D3DTA_CURRENT
    ));
}

#[test]
fn op_uses_texture_arg_detects_extended_fixed_function_ops() {
    assert!(op_uses_texture_arg(
        D3DTOP_MODULATE2X,
        D3DTA_CURRENT,
        D3DTA_TEXTURE,
        D3DTA_CURRENT
    ));
    assert!(op_uses_texture_arg(
        D3DTOP_BLENDTEXTUREALPHA,
        D3DTA_CURRENT,
        D3DTA_CURRENT,
        D3DTA_TEXTURE
    ));
    assert!(op_uses_texture_arg(
        D3DTOP_MODULATEINVCOLOR_ADDALPHA,
        D3DTA_CURRENT,
        D3DTA_CURRENT,
        D3DTA_TEXTURE
    ));
    assert!(!op_uses_texture_arg(
        D3DTOP_ADDSMOOTH,
        D3DTA_CURRENT,
        D3DTA_CURRENT,
        D3DTA_CURRENT
    ));
}

#[test]
fn arg_references_texture_ignores_tfactor_selector() {
    assert!(!arg_references_texture(D3DTA_TFACTOR));
    assert!(!arg_references_texture(D3DTA_TFACTOR | D3DTA_COMPLEMENT));
    assert!(arg_references_texture(D3DTA_TEXTURE | D3DTA_ALPHAREPLICATE));
}

#[test]
fn arg_color_from_texture_factor_respects_alpha_replicate_and_complement() {
    let color = arg_color_from_texture_factor(D3DTA_TFACTOR, 0x80402010);
    assert!((color[0] - (0x40 as f32 / 255.0)).abs() < 1e-6);
    assert!((color[1] - (0x20 as f32 / 255.0)).abs() < 1e-6);
    assert!((color[2] - (0x10 as f32 / 255.0)).abs() < 1e-6);
    assert!((color[3] - (0x80 as f32 / 255.0)).abs() < 1e-6);

    let replicated = arg_color_from_texture_factor(
        D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE | D3DTA_COMPLEMENT,
        0x80402010,
    );
    let expected = 1.0 - (0x80 as f32 / 255.0);
    assert!((replicated[0] - expected).abs() < 1e-6);
    assert!((replicated[1] - expected).abs() < 1e-6);
    assert!((replicated[2] - expected).abs() < 1e-6);
    assert!((replicated[3] - expected).abs() < 1e-6);
}

#[test]
fn simple_stage_tfactor_tint_detects_modulate_stage() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);

    let tint = simple_stage_tfactor_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        0,
        0x80402010,
    )
    .expect("tint");

    assert!((tint[0] - (0x40 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[1] - (0x20 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[2] - (0x10 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[3] - (0x80 as f32 / 255.0)).abs() < 1e-6);
}

#[test]
fn simple_stage_tfactor_tint_detects_selectarg_stage_without_texture() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);

    let tint = simple_stage_tfactor_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        0,
        0x80402010,
    )
    .expect("tint");

    assert!((tint[0] - (0x40 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[1] - (0x20 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[2] - (0x10 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[3] - (0x80 as f32 / 255.0)).abs() < 1e-6);
}

#[test]
fn simple_stage_tfactor_tint_ignores_additive_stage() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_ADD);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let tint = simple_stage_tfactor_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        0,
        0x80402010,
    );

    assert!(tint.is_none());
}

#[test]
fn simple_stage_tfactor_tint_detects_additive_tfactor_alpha_stage() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_DISABLE);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_ADD);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((0, D3DTSS_ALPHAARG2), D3DTA_TFACTOR);

    let tint = simple_stage_tfactor_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        0,
        0x80402010,
    )
    .expect("tint");

    assert!((tint[0] - 1.0).abs() < 1e-6);
    assert!((tint[1] - 1.0).abs() < 1e-6);
    assert!((tint[2] - 1.0).abs() < 1e-6);
    assert!((tint[3] - (0x80 as f32 / 255.0)).abs() < 1e-6);
}

#[test]
fn first_enabled_texture_stage_with_finds_later_enabled_stage() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_DISABLE);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);
    states.insert((3, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((3, D3DTSS_COLORARG1), D3DTA_TFACTOR);

    let stage = first_enabled_texture_stage_with(
        &mut |lookup_stage, state| {
            states
                .get(&(lookup_stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(lookup_stage, state))
        },
        8,
    );

    assert_eq!(stage, Some(3));
}

#[test]
fn simple_stage_chain_tint_propagates_current_between_stages() {
    let mut states = HashMap::new();

    // Stage 0 establishes tint from TFACTOR.
    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    // Stage 1 uses CURRENT, which should carry stage 0 tint forward.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_SELECTARG2);
    states.insert((1, D3DTSS_ALPHAARG2), D3DTA_CURRENT);

    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        0x80402010,
    )
    .expect("tint");

    assert!((tint[0] - (0x40 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[1] - (0x20 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[2] - (0x10 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[3] - (0x80 as f32 / 255.0)).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_blendcurrentalpha_with_current() {
    let mut states = HashMap::new();

    // Stage 0 establishes current tint and alpha from texture factor.
    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    // Stage 1 blends neutral texture color with CURRENT using CURRENT alpha.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_BLENDCURRENTALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        0x80402010,
    )
    .expect("tint");

    let current = [
        0x40 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x10 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let expected = [
        1.0 * current[3] + current[0] * (1.0 - current[3]),
        1.0 * current[3] + current[1] * (1.0 - current[3]),
        1.0 * current[3] + current[2] * (1.0 - current[3]),
        1.0 * current[3] + current[3] * (1.0 - current[3]),
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - current[3]).abs() < 1e-6);
}

#[test]
fn simple_material_tint_arg_value_accepts_modified_neutral_diffuse() {
    let white = simple_material_tint_arg_value(
        D3DTA_DIFFUSE | D3DTA_ALPHAREPLICATE,
        [0.25, 0.5, 0.75, 0.5],
        0x80402010,
    )
    .expect("white");
    assert_eq!(white, [1.0, 1.0, 1.0, 1.0]);

    let black = simple_material_tint_arg_value(
        D3DTA_DIFFUSE | D3DTA_COMPLEMENT | D3DTA_ALPHAREPLICATE,
        [0.25, 0.5, 0.75, 0.5],
        0x80402010,
    )
    .expect("black");
    assert_eq!(black, [0.0, 0.0, 0.0, 0.0]);
}

#[test]
fn simple_stage_chain_tint_handles_blendfactoralpha() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_BLENDFACTORALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        0x80402010,
    )
    .expect("tint");

    let current = [
        0x40 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x10 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let factor = current[3];
    let expected = [
        1.0 * factor + current[0] * (1.0 - factor),
        1.0 * factor + current[1] * (1.0 - factor),
        1.0 * factor + current[2] * (1.0 - factor),
        1.0 * factor + current[3] * (1.0 - factor),
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - current[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_multiplyadd_arg0() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_MULTIPLYADD);
    states.insert((0, D3DTSS_COLORARG0), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x40102030;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        0,
        texture_factor,
    )
    .expect("tint");

    let alpha = 0x40 as f32 / 255.0;
    let expected = (alpha * 1.0 + alpha).clamp(0.0, 1.0);
    assert!((tint[0] - expected).abs() < 1e-6);
    assert!((tint[1] - expected).abs() < 1e-6);
    assert!((tint[2] - expected).abs() < 1e-6);
    assert!((tint[3] - 1.0).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_dotproduct3_after_multiplyadd() {
    let mut states = HashMap::new();

    // Stage 0 mirrors the shader-manager grayscale setup.
    states.insert((0, D3DTSS_COLOROP), D3DTOP_MULTIPLYADD);
    states.insert((0, D3DTSS_COLORARG0), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    // Stage 1 consumes CURRENT via DOTPRODUCT3.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_DOTPRODUCT3);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x80A5CA8E;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let stage0 = {
        let alpha = 0x80 as f32 / 255.0;
        let value = (alpha * 1.0 + alpha).clamp(0.0, 1.0);
        [value, value, value, 1.0]
    };
    let tfactor = [
        0xA5 as f32 / 255.0,
        0xCA as f32 / 255.0,
        0x8E as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let expected = dotproduct3_rgba(stage0, tfactor);

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - stage0[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_addsigned2x_after_current() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_ADDSIGNED2X);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_ADDSIGNED2X);
    states.insert((1, D3DTSS_ALPHAARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAARG2), D3DTA_CURRENT);

    let texture_factor = 0x80406020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let tfactor = [
        0x40 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let expected = addsigned_rgba(tfactor, tfactor, 2.0);

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_subtract_against_tfactor() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_SUBTRACT);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0xFF204080;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let tfactor = [
        0x20 as f32 / 255.0,
        0x40 as f32 / 255.0,
        0x80 as f32 / 255.0,
        1.0,
    ];
    let expected = subtract_rgba(
        tfactor,
        [1.0 - tfactor[0], 1.0 - tfactor[1], 1.0 - tfactor[2], 0.0],
    );

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - tfactor[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_addsmooth_with_current() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_ADDSMOOTH);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x60408020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let tfactor = [
        0x40 as f32 / 255.0,
        0x80 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x60 as f32 / 255.0,
    ];
    let alpha_replicated = [tfactor[3], tfactor[3], tfactor[3], tfactor[3]];
    let expected = addsmooth_rgba(tfactor, alpha_replicated);

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - 1.0).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_blenddiffusealpha_as_arg1_in_neutral_domain() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_BLENDDIFFUSEALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_BLENDDIFFUSEALPHA);
    states.insert((1, D3DTSS_ALPHAARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);

    let texture_factor = 0x80406020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let expected = [
        0x40 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_blendtexturealpha_as_arg1_in_neutral_domain() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR | D3DTA_ALPHAREPLICATE);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_BLENDTEXTUREALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x7FA0C040;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let alpha = 0x7F as f32 / 255.0;
    let expected = [alpha, alpha, alpha, 1.0];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_blendtexturealphapm_as_arg1_in_neutral_domain() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_BLENDTEXTUREALPHAPM);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_BLENDTEXTUREALPHAPM);
    states.insert((1, D3DTSS_ALPHAARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);

    let texture_factor = 0x90C08040;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let expected = [
        0xC0 as f32 / 255.0,
        0x80 as f32 / 255.0,
        0x40 as f32 / 255.0,
        0x90 as f32 / 255.0,
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_modulatealpha_addcolor() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATEALPHA_ADDCOLOR);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x80406020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let lhs = [
        0x40 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let rhs = [1.0 - lhs[0], 1.0 - lhs[1], 1.0 - lhs[2], 1.0 - lhs[3]];
    let expected = scale_rgb_add_rgba(lhs, rhs, [lhs[3], lhs[3], lhs[3], 1.0]);

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - lhs[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_modulatecolor_addalpha() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATECOLOR_ADDALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x80406020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let lhs = [
        0x40 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let rhs = [1.0 - lhs[0], 1.0 - lhs[1], 1.0 - lhs[2], 1.0 - lhs[3]];
    let expected = [
        (lhs[0] * rhs[0] + lhs[3]).clamp(0.0, 1.0),
        (lhs[1] * rhs[1] + lhs[3]).clamp(0.0, 1.0),
        (lhs[2] * rhs[2] + lhs[3]).clamp(0.0, 1.0),
        lhs[3],
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - lhs[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_modulateinvalpha_addcolor() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATEINVALPHA_ADDCOLOR);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x80406020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let lhs = [
        0x40 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let rhs = [1.0 - lhs[0], 1.0 - lhs[1], 1.0 - lhs[2], 1.0 - lhs[3]];
    let inv_alpha = 1.0 - lhs[3];
    let expected = scale_rgb_add_rgba(lhs, rhs, [inv_alpha, inv_alpha, inv_alpha, 1.0]);

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - lhs[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_modulateinvcolor_addalpha() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATEINVCOLOR_ADDALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let texture_factor = 0x80406020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let lhs = [
        0x40 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
    ];
    let rhs = [1.0 - lhs[0], 1.0 - lhs[1], 1.0 - lhs[2], 1.0 - lhs[3]];
    let expected = [
        ((1.0 - lhs[0]) * rhs[0] + lhs[3]).clamp(0.0, 1.0),
        ((1.0 - lhs[1]) * rhs[1] + lhs[3]).clamp(0.0, 1.0),
        ((1.0 - lhs[2]) * rhs[2] + lhs[3]).clamp(0.0, 1.0),
        lhs[3],
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - lhs[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_preserves_current_through_premodulate_stage() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_PREMODULATE);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_PREMODULATE);

    states.insert((2, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((2, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((2, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((2, D3DTSS_ALPHAARG1), D3DTA_CURRENT);

    let texture_factor = 0x90402080;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        2,
        texture_factor,
    )
    .expect("tint");

    let expected = [
        0x40 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
        0x90 as f32 / 255.0,
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_preserves_current_through_bumpenvmap_stage() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_BUMPENVMAP);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_BUMPENVMAP);

    states.insert((2, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((2, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((2, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((2, D3DTSS_ALPHAARG1), D3DTA_CURRENT);

    let texture_factor = 0x90506030;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        2,
        texture_factor,
    )
    .expect("tint");

    let expected = [
        0x50 as f32 / 255.0,
        0x60 as f32 / 255.0,
        0x30 as f32 / 255.0,
        0x90 as f32 / 255.0,
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_preserves_current_through_bumpenvmapluminance_stage() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_BUMPENVMAPLUMINANCE);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_BUMPENVMAPLUMINANCE);

    states.insert((2, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((2, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((2, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((2, D3DTSS_ALPHAARG1), D3DTA_CURRENT);

    let texture_factor = 0xA0402080;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        2,
        texture_factor,
    )
    .expect("tint");

    let expected = [
        0x40 as f32 / 255.0,
        0x20 as f32 / 255.0,
        0x80 as f32 / 255.0,
        0xA0 as f32 / 255.0,
    ];

    assert!((tint[0] - expected[0]).abs() < 1e-6);
    assert!((tint[1] - expected[1]).abs() < 1e-6);
    assert!((tint[2] - expected[2]).abs() < 1e-6);
    assert!((tint[3] - expected[3]).abs() < 1e-6);
}

#[test]
fn simple_stage_chain_tint_handles_blendcurrentalpha_in_alpha_lane() {
    let mut states = HashMap::new();

    states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_SELECTARG1);
    states.insert((0, D3DTSS_ALPHAARG1), D3DTA_TFACTOR);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_BLENDCURRENTALPHA);
    states.insert((1, D3DTSS_ALPHAARG1), D3DTA_TFACTOR | D3DTA_COMPLEMENT);
    states.insert((1, D3DTSS_ALPHAARG2), D3DTA_TFACTOR);

    let texture_factor = 0x80406020;
    let tint = simple_stage_chain_tint_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        1,
        texture_factor,
    )
    .expect("tint");

    let alpha = 0x80 as f32 / 255.0;
    let expected_alpha = alpha * (1.0 - alpha) + alpha * (1.0 - alpha);

    assert!((tint[0] - (0x40 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[1] - (0x60 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[2] - (0x20 as f32 / 255.0)).abs() < 1e-6);
    assert!((tint[3] - expected_alpha).abs() < 1e-6);
}

#[test]
fn resolve_active_texture_stage_detects_extended_op_texture_usage() {
    let mut states = HashMap::new();

    // Stage 0 enabled but color path has no texture usage.
    states.insert((0, D3DTSS_COLOROP), D3DTOP_ADDSMOOTH);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    // Stage 1 uses texture via a blend op.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_BLENDTEXTUREALPHA);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_CURRENT);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 1);
}

#[test]
fn resolve_active_texture_stage_ignores_tfactor_only_stage() {
    let mut states = HashMap::new();

    // Stage 0 reads TFACTOR only and must not count as texture sampling.
    states.insert((0, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TFACTOR);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    // Stage 1 is the first actual texture-sampling stage.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 1);
}

#[test]
fn default_render_state_value_defaults_texture_factor_to_white() {
    assert_eq!(
        default_render_state_value(W3D_RENDER_STATE::W3DRS_TEXTUREFACTOR),
        0xFFFF_FFFF
    );
}

#[test]
fn default_render_state_value_defaults_fixed_function_lighting_states() {
    assert_eq!(
        default_render_state_value(W3D_RENDER_STATE::W3DRS_LIGHTING),
        1
    );
    assert_eq!(
        default_render_state_value(W3D_RENDER_STATE::W3DRS_SPECULARENABLE),
        0
    );
    assert_eq!(
        default_render_state_value(W3D_RENDER_STATE::W3DRS_COLORVERTEX),
        1
    );
    assert_eq!(
        default_render_state_value(W3D_RENDER_STATE::W3DRS_AMBIENTMATERIALSOURCE),
        D3DMCS_MATERIAL
    );
    assert_eq!(
        default_render_state_value(W3D_RENDER_STATE::W3DRS_DIFFUSEMATERIALSOURCE),
        D3DMCS_COLOR1
    );
}

#[test]
fn apply_fixed_function_lighting_to_material_applies_ambient_and_disables_specular() {
    let mut material = default_material("ambient");
    material.properties.diffuse_color = [0.5, 0.25, 0.75, 1.0];
    material.properties.specular_color = [1.0, 0.5, 0.25];
    material.properties.shininess = 48.0;

    apply_fixed_function_lighting_to_material(
        &mut material,
        true,
        FixedFunctionLightingState {
            ambient_argb: 0xFF804020,
            specular_enabled: false,
            ..default_fixed_function_lighting_state()
        },
    );

    assert_eq!(material.properties.specular_color, [0.0, 0.0, 0.0]);
    assert!(material.properties.shininess.abs() < 1e-6);
    assert!((material.properties.emissive_color[0] - (0.5 * (0x80 as f32 / 255.0))).abs() < 1e-6);
    assert!((material.properties.emissive_color[1] - (0.25 * (0x40 as f32 / 255.0))).abs() < 1e-6);
    assert!((material.properties.emissive_color[2] - (0.75 * (0x20 as f32 / 255.0))).abs() < 1e-6);
}

#[test]
fn apply_fixed_function_lighting_to_material_respects_vertex_sourced_channels() {
    let mut material = default_material("vertex_sourced");
    material.properties.diffuse_color = [0.6, 0.4, 0.2, 1.0];
    material.properties.specular_color = [0.9, 0.8, 0.7];
    material.properties.emissive_color = [0.3, 0.2, 0.1];

    apply_fixed_function_lighting_to_material(
        &mut material,
        true,
        FixedFunctionLightingState {
            color_vertex: true,
            ambient_argb: 0xFFFFFFFF,
            ambient_material_source: D3DMCS_COLOR1,
            specular_material_source: D3DMCS_COLOR2,
            emissive_material_source: D3DMCS_COLOR1,
            ..default_fixed_function_lighting_state()
        },
    );

    assert_eq!(material.properties.specular_color, [0.0, 0.0, 0.0]);
    assert_eq!(material.properties.emissive_color, [0.0, 0.0, 0.0]);
}

#[test]
fn apply_fixed_function_lighting_to_material_makes_unlit_solid_material_visible() {
    let mut material = default_material("unlit_solid");
    material.properties.diffuse_color = [0.2, 0.4, 0.6, 1.0];

    apply_fixed_function_lighting_to_material(
        &mut material,
        false,
        FixedFunctionLightingState {
            lighting_enabled: false,
            ..default_fixed_function_lighting_state()
        },
    );

    assert!((material.properties.emissive_color[0] - 0.2).abs() < 1e-6);
    assert!((material.properties.emissive_color[1] - 0.4).abs() < 1e-6);
    assert!((material.properties.emissive_color[2] - 0.6).abs() < 1e-6);
    assert_eq!(material.properties.specular_color, [0.0, 0.0, 0.0]);
    assert!(material.properties.unlit);
}

#[test]
fn apply_fixed_function_lighting_to_material_marks_textured_unlit_materials() {
    let mut material = default_material("unlit_textured");
    material.properties.diffuse_color = [0.8, 0.6, 0.4, 1.0];
    material.properties.unlit = false;

    apply_fixed_function_lighting_to_material(
        &mut material,
        true,
        FixedFunctionLightingState {
            lighting_enabled: false,
            ..default_fixed_function_lighting_state()
        },
    );

    assert!(material.properties.unlit);
    assert_eq!(material.properties.specular_color, [0.0, 0.0, 0.0]);
    assert_eq!(material.properties.emissive_color, [0.0, 0.0, 0.0]);
}

#[test]
fn apply_fixed_function_surface_to_material_applies_alpha_test_and_cull_mode() {
    let mut material = default_material("surface");

    apply_fixed_function_surface_to_material(
        &mut material,
        FixedFunctionSurfaceState {
            alpha_test_enabled: true,
            alpha_ref: 0x80,
            alpha_blend_enabled: true,
            cull_mode: D3DCULL_NONE,
        },
    );

    assert!(material.properties.alpha_test);
    assert!((material.properties.alpha_cutoff - (0x80 as f32 / 255.0)).abs() < 1.0e-6);
    assert!(material.properties.transparent);
    assert!(material.properties.double_sided);
}

#[test]
fn default_material_matches_cpp_apply_null_defaults() {
    let material = default_material("null");

    assert_eq!(material.properties.diffuse_color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(material.properties.specular_color, [0.0, 0.0, 0.0]);
    assert_eq!(material.properties.emissive_color, [0.0, 0.0, 0.0]);
    assert!((material.properties.shininess - 1.0).abs() < 1.0e-6);
    assert!(!material.properties.alpha_test);
    assert!(!material.properties.transparent);
    assert!(!material.properties.double_sided);
    assert!(material.properties.unlit);
}

#[test]
fn enabled_texture_sampling_stage_count_detects_multitexture_chains() {
    let mut states = HashMap::new();
    states.insert((0, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_DIFFUSE);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let count = enabled_texture_sampling_stage_count_with(
        &mut |stage, state| {
            states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        8,
    );

    assert_eq!(count, 2);
}

#[test]
fn material_binding_id_includes_fixed_function_state() {
    let base = Some("base");
    let texture = "tex";
    let tint = [255, 255, 255, 255];
    let lighting_default = default_fixed_function_lighting_state();
    let surface_default = default_fixed_function_surface_state();
    let identity_signature = MaterialCombinerSignature {
        sampling_stage_count: 1,
        force_multiply_like: false,
    };
    let lit_id = material_binding_id(
        base,
        texture,
        "",
        tint,
        identity_signature,
        lighting_default,
        surface_default,
    );
    let alpha_test_id = material_binding_id(
        base,
        texture,
        "",
        tint,
        identity_signature,
        lighting_default,
        FixedFunctionSurfaceState {
            alpha_test_enabled: true,
            alpha_ref: 0x80,
            ..surface_default
        },
    );

    assert_ne!(lit_id, alpha_test_id);
}

#[test]
fn material_binding_id_distinguishes_force_multiply_like_combiner_paths() {
    let base = Some("base");
    let texture = "tex";
    let tint = [255, 255, 255, 255];
    let lighting_default = default_fixed_function_lighting_state();
    let surface_default = default_fixed_function_surface_state();

    let select_arg1 = MaterialCombinerSignature {
        sampling_stage_count: 1,
        force_multiply_like: false,
    };
    let modulate = MaterialCombinerSignature {
        sampling_stage_count: 1,
        force_multiply_like: true,
    };

    let select_id = material_binding_id(
        base,
        texture,
        "",
        tint,
        select_arg1,
        lighting_default,
        surface_default,
    );
    let modulate_id = material_binding_id(
        base,
        texture,
        "",
        tint,
        modulate,
        lighting_default,
        surface_default,
    );

    assert_ne!(select_id, modulate_id);
}

#[test]
fn material_combiner_signature_detects_force_multiply_like_paths() {
    let mut select_arg1_states = HashMap::new();
    select_arg1_states.insert((0, D3DTSS_COLOROP), D3DTOP_SELECTARG1);
    select_arg1_states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    select_arg1_states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let mut modulate_states = HashMap::new();
    modulate_states.insert((0, D3DTSS_COLOROP), D3DTOP_MODULATE);
    modulate_states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    modulate_states.insert((0, D3DTSS_COLORARG2), D3DTA_CURRENT);
    modulate_states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let select_signature = material_combiner_signature_with(
        &mut |stage, state| {
            select_arg1_states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        8,
    );
    let modulate_signature = material_combiner_signature_with(
        &mut |stage, state| {
            modulate_states
                .get(&(stage, state))
                .copied()
                .unwrap_or_else(|| default_texture_stage_state(stage, state))
        },
        8,
    );

    assert_eq!(select_signature.sampling_stage_count, 1);
    assert_eq!(modulate_signature.sampling_stage_count, 1);
    assert!(!select_signature.force_multiply_like);
    assert!(modulate_signature.force_multiply_like);
}

#[test]
fn effective_bound_texture_id_avoids_single_texture_override_for_multitexture_base_materials() {
    assert_eq!(
        effective_bound_texture_id(true, true, Some("stage_tex".to_string())),
        None
    );
    assert_eq!(
        effective_bound_texture_id(false, true, Some("stage_tex".to_string())),
        Some("stage_tex".to_string())
    );
    assert_eq!(
        effective_bound_texture_id(true, false, Some("stage_tex".to_string())),
        Some("stage_tex".to_string())
    );
}

#[test]
fn op_uses_texture_arg_treats_unknown_op_as_non_sampling() {
    assert!(!op_uses_texture_arg(
        0xDEAD_BEEF,
        D3DTA_CURRENT,
        D3DTA_TEXTURE,
        D3DTA_TEXTURE
    ));
}

#[test]
fn resolve_active_texture_stage_ignores_unknown_color_op_stage() {
    let mut states = HashMap::new();

    // Stage 0: enabled by raw state value, but op code is unknown and should not sample.
    states.insert((0, D3DTSS_COLOROP), 0xDEAD_BEEF);
    states.insert((0, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_COLORARG2), D3DTA_TEXTURE);
    states.insert((0, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    // Stage 1: known op with texture sampling.
    states.insert((1, D3DTSS_COLOROP), D3DTOP_MODULATE);
    states.insert((1, D3DTSS_COLORARG1), D3DTA_TEXTURE);
    states.insert((1, D3DTSS_COLORARG2), D3DTA_CURRENT);
    states.insert((1, D3DTSS_ALPHAOP), D3DTOP_DISABLE);

    let bound_stages = vec![0, 1];
    let active = resolve_active_draw_texture_stage(&bound_stages, |stage, state| {
        states
            .get(&(stage, state))
            .copied()
            .unwrap_or_else(|| default_texture_stage_state(stage, state))
    });

    assert_eq!(active, 1);
}

#[test]
fn generated_texcoords_apply_without_texture_transform_for_camera_position() {
    let world = W3D_MATRIX::from(Mat4::IDENTITY);
    let view = W3D_MATRIX::from(Mat4::IDENTITY);
    let mut vertices = vec![W3D_VERTEX {
        x: 2.0,
        y: 3.0,
        z: 4.0,
        nx: 0.0,
        ny: 0.0,
        nz: 1.0,
        u: 0.0,
        v: 0.0,
        color: 0xFFFF_FFFF,
    }];

    apply_generated_stage_texcoords(&mut vertices, D3DTSS_TCI_CAMERASPACEPOSITION, &world, &view);

    assert!((vertices[0].u - 2.0).abs() < 1e-6);
    assert!((vertices[0].v - 3.0).abs() < 1e-6);
}

#[test]
fn generated_texcoords_apply_without_texture_transform_for_camera_normal() {
    let world = W3D_MATRIX::from(Mat4::IDENTITY);
    let view = W3D_MATRIX::from(Mat4::IDENTITY);
    let mut vertices = vec![W3D_VERTEX {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        nx: 0.0,
        ny: 1.0,
        nz: 0.0,
        u: 0.25,
        v: 0.5,
        color: 0xFFFF_FFFF,
    }];

    apply_generated_stage_texcoords(&mut vertices, D3DTSS_TCI_CAMERASPACENORMAL, &world, &view);

    assert!(vertices[0].u.abs() < 1e-6);
    assert!((vertices[0].v - 1.0).abs() < 1e-6);
}

#[test]
fn generated_texcoords_apply_without_texture_transform_for_spheremap() {
    let world = W3D_MATRIX::from(Mat4::IDENTITY);
    let view = W3D_MATRIX::from(Mat4::IDENTITY);
    let mut vertices = vec![W3D_VERTEX {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        nx: 1.0,
        ny: 0.0,
        nz: 0.0,
        u: 0.0,
        v: 0.0,
        color: 0xFFFF_FFFF,
    }];

    apply_generated_stage_texcoords(&mut vertices, D3DTSS_TCI_SPHEREMAP, &world, &view);

    assert!((vertices[0].u - 1.0).abs() < 1e-6);
    assert!((vertices[0].v - 0.5).abs() < 1e-6);
}

#[test]
fn declaration_stream_decode_supports_float1_texcoord() {
    let mut streams = HashMap::new();

    let mut uv_bytes = Vec::new();
    push_f32(&mut uv_bytes, 0.625);
    streams.insert(
        0,
        StagedStreamSource {
            vertex_stride: 4,
            vertex_offset_bytes: 0,
            vertex_count: 1,
            data: uv_bytes,
        },
    );

    let mut pos_bytes = Vec::new();
    push_f32(&mut pos_bytes, 1.0);
    push_f32(&mut pos_bytes, 2.0);
    push_f32(&mut pos_bytes, 3.0);
    streams.insert(
        1,
        StagedStreamSource {
            vertex_stride: 12,
            vertex_offset_bytes: 0,
            vertex_count: 1,
            data: pos_bytes,
        },
    );

    let elements = vec![
        W3D_VERTEX_ELEMENT {
            stream: 1,
            offset: 0,
            decl_type: D3DDECLTYPE_FLOAT3,
            method: 0,
            usage: D3DDECLUSAGE_POSITION,
            usage_index: 0,
        },
        W3D_VERTEX_ELEMENT {
            stream: 0,
            offset: 0,
            decl_type: D3DDECLTYPE_FLOAT1,
            method: 0,
            usage: D3DDECLUSAGE_TEXCOORD,
            usage_index: 0,
        },
    ];

    let vertices = collect_vertices_from_declaration_streams(&streams, 0, 1, &elements, 0)
        .expect("declaration vertices");
    assert_eq!(vertices.len(), 1);
    assert!((vertices[0].u - 0.625).abs() < 1e-6);
    assert!(vertices[0].v.abs() < 1e-6);
}

#[test]
fn read_normal_from_decl_supports_dec3n() {
    // x=-1, y=0, z=+1 in signed 10-bit normalized format.
    let packed = (0x201_u32) | (0x000_u32 << 10) | (0x1FF_u32 << 20);
    let bytes = packed.to_le_bytes();
    let (nx, ny, nz) = read_normal_from_decl(&bytes, 0, D3DDECLTYPE_DEC3N).expect("dec3n normal");
    assert!((nx + 1.0).abs() < 0.01);
    assert!(ny.abs() < 0.01);
    assert!((nz - 1.0).abs() < 0.01);
}

#[test]
fn alpha_blend_enabled_from_states_defaults_to_disabled() {
    let states = HashMap::new();
    assert!(!alpha_blend_enabled_from_states(&states));
}

#[test]
fn alpha_blend_enabled_from_states_honors_nonzero_value() {
    let mut states = HashMap::new();
    states.insert(W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE, 1);
    assert!(alpha_blend_enabled_from_states(&states));
}
