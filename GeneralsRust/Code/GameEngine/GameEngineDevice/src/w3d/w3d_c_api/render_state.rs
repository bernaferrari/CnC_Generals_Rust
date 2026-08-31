//! W3D C API render-state, FVF, vertex declaration, and shader handles.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::leftover::*;
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

/// Set render state - matches original W3DDevice::SetRenderState(state, value)
#[unsafe(no_mangle)]
// SAFETY: C ABI entry; only the device handle is dereferenced and state mutation
// SAFETY: happens under its Mutex. All parameters are by-value scalars.
pub unsafe extern "C" fn W3DDevice_SetRenderState(
    device: W3D_DEVICE,
    state: W3D_RENDER_STATE,
    value: u32,
) -> i32 {
    if device.is_null() {
        return 0; // Failure
    }

    let device_ref = &*device;
    if let Ok(mut states) = device_ref.render_states.lock() {
        states.insert(state, value);
    }
    crate::w3d::renderer::record_deferred_render_state(state as u32, value);
    1
}

#[allow(dead_code)]
pub(super) async fn set_render_state_internal(
    device: &Arc<RwLock<W3DDevice>>,
    state: W3D_RENDER_STATE,
    value: u32,
) -> Result<()> {
    match state {
        W3D_RENDER_STATE::W3DRS_FOGENABLE
        | W3D_RENDER_STATE::W3DRS_FOGCOLOR
        | W3D_RENDER_STATE::W3DRS_FOGTABLEMODE
        | W3D_RENDER_STATE::W3DRS_FOGSTART
        | W3D_RENDER_STATE::W3DRS_FOGEND
        | W3D_RENDER_STATE::W3DRS_FOGDENSITY
        | W3D_RENDER_STATE::W3DRS_AMBIENT => {
            let device_lock = device.read().await;
            let mut scene = device_lock.get_scene().await;
            match state {
                W3D_RENDER_STATE::W3DRS_FOGENABLE => {
                    scene.fog_enabled = value != 0;
                }
                W3D_RENDER_STATE::W3DRS_FOGCOLOR => {
                    scene.fog_color = decode_argb_color(value);
                }
                W3D_RENDER_STATE::W3DRS_FOGSTART => {
                    let fog_start = f32::from_bits(value);
                    if fog_start.is_finite() {
                        scene.fog_params[0] = fog_start;
                    }
                }
                W3D_RENDER_STATE::W3DRS_FOGEND => {
                    let fog_end = f32::from_bits(value);
                    if fog_end.is_finite() {
                        scene.fog_params[1] = fog_end;
                    }
                }
                W3D_RENDER_STATE::W3DRS_FOGDENSITY => {
                    let fog_density = f32::from_bits(value);
                    if fog_density.is_finite() {
                        scene.fog_params[2] = fog_density;
                    }
                }
                W3D_RENDER_STATE::W3DRS_FOGTABLEMODE => {
                    tracing::debug!("Set fog table mode: {}", value);
                }
                W3D_RENDER_STATE::W3DRS_AMBIENT => {
                    let ambient = decode_argb_color(value);
                    scene.ambient_light = [ambient[0], ambient[1], ambient[2]];
                }
                _ => {}
            }
            device_lock.set_scene(scene).await?;
        }
        W3D_RENDER_STATE::W3DRS_ZENABLE => {
            // PARITY_NOTE: D3DRS_ZENABLE maps to wgpu depth_stencil state.
            // Value: TRUE(1)=enable depth test, FALSE(0)=disable.
            tracing::debug!("Set depth test enabled: {}", value != 0);
        }
        W3D_RENDER_STATE::W3DRS_ZWRITEENABLE => {
            // PARITY_NOTE: D3DRS_ZWRITEENABLE controls depth buffer writes.
            tracing::debug!("Set depth write enabled: {}", value != 0);
        }
        W3D_RENDER_STATE::W3DRS_ZFUNC => {
            // PARITY_NOTE: D3DRS_ZFUNC sets depth comparison function.
            // D3DCMP_NEVER=1..D3DCMP_ALWAYS=8. Default D3DCMP_LESSEQUAL=4.
            tracing::debug!("Set depth comparison func: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_FILLMODE => {
            // PARITY_NOTE: D3DRS_FILLMODE maps to wgpu PolygonMode.
            // D3DFILL_POINT=1, D3DFILL_WIREFRAME=2, D3DFILL_SOLID=3.
            tracing::debug!("Set fill mode: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_SHADEMODE => {
            // PARITY_NOTE: No direct wgpu equivalent; always smooth interpolation.
            tracing::debug!("Set shade mode (no-op in wgpu, always smooth): {}", value);
        }
        W3D_RENDER_STATE::W3DRS_CULLMODE => {
            // PARITY_NOTE: D3DRS_CULLMODE maps to wgpu face culling.
            // D3DCULL_NONE=1, D3DCULL_CW=2, D3DCULL_CCW=3.
            tracing::debug!("Set cull mode: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_ALPHATESTENABLE => {
            // PARITY_NOTE: Enables alpha test (discard). Tracked in FixedFunctionSurfaceState.
            tracing::debug!("Set alpha test enabled: {}", value != 0);
        }
        W3D_RENDER_STATE::W3DRS_ALPHAREF => {
            // PARITY_NOTE: Reference value for alpha test.
            tracing::debug!("Set alpha reference: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_ALPHAFUNC => {
            // PARITY_NOTE: Comparison for alpha test. Default D3DCMP_ALWAYS=8.
            tracing::debug!("Set alpha func: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE => {
            // PARITY_NOTE: Maps to wgpu BlendState. Tracked in FixedFunctionSurfaceState.
            tracing::debug!("Set alpha blend enabled: {}", value != 0);
        }
        W3D_RENDER_STATE::W3DRS_SRCBLEND => {
            // PARITY_NOTE: D3DBLEND_ZERO=1..D3DBLEND_BLENDFACTOR=19. Maps to wgpu BlendFactor.
            tracing::debug!("Set source blend factor: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_DESTBLEND => {
            // PARITY_NOTE: Maps to wgpu BlendFactor.
            tracing::debug!("Set dest blend factor: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_STENCILENABLE => {
            // PARITY_NOTE: Maps to wgpu StencilFaceState.
            tracing::debug!("Set stencil enabled: {}", value != 0);
        }
        W3D_RENDER_STATE::W3DRS_STENCILFAIL => {
            // PARITY_NOTE: D3DSTENCILOP_KEEP=1..D3DSTENCILOP_DECRSAT=8.
            tracing::debug!("Set stencil fail op: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_STENCILZFAIL => {
            tracing::debug!("Set stencil zfail op: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_STENCILPASS => {
            tracing::debug!("Set stencil pass op: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_STENCILFUNC => {
            // PARITY_NOTE: Maps to wgpu CompareFunction for StencilFaceState.
            tracing::debug!("Set stencil func: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_STENCILREF => {
            // PARITY_NOTE: Applied via wgpu render pass set_stencil_reference().
            tracing::debug!("Set stencil ref: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_STENCILMASK => {
            tracing::debug!("Set stencil mask: {}", value);
        }
        W3D_RENDER_STATE::W3DRS_STENCILWRITEMASK => {
            tracing::debug!("Set stencil write mask: 0x{:08X}", value);
        }
        W3D_RENDER_STATE::W3DRS_DITHERENABLE => {
            // PARITY_NOTE: Dithering always enabled in wgpu for supported formats.
            tracing::debug!("Set dither enable (no-op in wgpu): {}", value != 0);
        }
        W3D_RENDER_STATE::W3DRS_LASTPIXEL => {
            // PARITY_NOTE: No wgpu equivalent; always draws all pixels.
            tracing::debug!("Set last pixel (no wgpu equivalent): {}", value);
        }
        W3D_RENDER_STATE::W3DRS_TEXTUREFACTOR => {
            // PARITY_NOTE: ARGB color used by D3DTA_TFACTOR in texture stage states.
            tracing::debug!("Set texture factor: 0x{:08X}", value);
        }
        W3D_RENDER_STATE::W3DRS_RANGEFOGENABLE => {
            // PARITY_NOTE: Range-based fog; wgpu fog is in fragment shader.
            tracing::debug!("Set range fog enabled: {}", value != 0);
        }
        W3D_RENDER_STATE::W3DRS_LIGHTING
        | W3D_RENDER_STATE::W3DRS_SPECULARENABLE
        | W3D_RENDER_STATE::W3DRS_COLORVERTEX
        | W3D_RENDER_STATE::W3DRS_LOCALVIEWER
        | W3D_RENDER_STATE::W3DRS_NORMALIZENORMALS
        | W3D_RENDER_STATE::W3DRS_DIFFUSEMATERIALSOURCE
        | W3D_RENDER_STATE::W3DRS_SPECULARMATERIALSOURCE
        | W3D_RENDER_STATE::W3DRS_AMBIENTMATERIALSOURCE
        | W3D_RENDER_STATE::W3DRS_EMISSIVEMATERIALSOURCE => {
            // Fixed-function lighting states tracked in FixedFunctionLightingState
            // for material hash computation. Actual lighting computed in shaders.
            tracing::debug!(
                "Tracking fixed-function lighting state {:?}: {}",
                state,
                value
            );
        }
    }

    Ok(())
}

/// Set fixed-function vertex format - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry; only the device handle is dereferenced under its Mutex.
// SAFETY: `fvf` is an opaque by-value handle word, never a pointer dereference.
pub unsafe extern "C" fn W3DDevice_SetFVF(device: W3D_DEVICE, fvf: u32) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut current_fvf) = device_ref.current_fvf.lock() {
        *current_fvf = fvf;
        return 1;
    }
    0
}

/// Get fixed-function vertex format - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI query; only the device handle is dereferenced under its Mutex.
// SAFETY: By-value return, no pointers written.
pub unsafe extern "C" fn W3DDevice_GetFVF(device: W3D_DEVICE) -> u32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(current_fvf) = device_ref.current_fvf.lock() {
        return *current_fvf;
    }
    0
}

/// Set current vertex declaration handle - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry; `declaration` is an opaque by-value handle id, not a
// SAFETY: pointer. Only the device handle is dereferenced under its Mutex.
pub unsafe extern "C" fn W3DDevice_SetVertexDeclaration(
    device: W3D_DEVICE,
    declaration: u32,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut current_decl) = device_ref.current_vertex_declaration.lock() {
        *current_decl = declaration;
        return 1;
    }
    0
}

/// Get current vertex declaration handle - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI query; opaque declaration-id lookup under the device Mutex.
// SAFETY: By-value return, no pointer dereference beyond the live device handle.
pub unsafe extern "C" fn W3DDevice_GetVertexDeclaration(device: W3D_DEVICE) -> u32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(current_decl) = device_ref.current_vertex_declaration.lock() {
        return *current_decl;
    }
    0
}

/// Define or replace declaration metadata for a legacy declaration handle.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry. `elements` must be readable for element_count
// SAFETY: W3D_VERTEX_ELEMENTs through this call; contents are cloned into the
// SAFETY: device's declaration table immediately.
pub unsafe extern "C" fn W3DDevice_DefineVertexDeclaration(
    device: W3D_DEVICE,
    declaration: u32,
    elements: *const W3D_VERTEX_ELEMENT,
    element_count: u32,
) -> i32 {
    if device.is_null() || declaration == 0 {
        return 0;
    }
    let device_ref = &*device;

    if elements.is_null() || element_count == 0 {
        if let Ok(mut declarations) = device_ref.vertex_declarations.lock() {
            declarations.remove(&declaration);
            return 1;
        }
        return 0;
    }
    if !is_valid_ptr(elements) {
        return 0;
    }

    let mut defined = std::slice::from_raw_parts(elements, element_count as usize).to_vec();
    if let Some(unused_idx) = defined
        .iter()
        .position(|entry| entry.decl_type == D3DDECLTYPE_UNUSED)
    {
        defined.truncate(unused_idx);
    }
    if defined.is_empty() {
        return 0;
    }

    if let Ok(mut declarations) = device_ref.vertex_declarations.lock() {
        declarations.insert(declaration, defined);
        return 1;
    }
    0
}

/// Clear declaration metadata for a legacy declaration handle.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry; removes a table entry keyed by the opaque declaration id.
// SAFETY: No pointer parameters besides the validated device handle.
pub unsafe extern "C" fn W3DDevice_ClearVertexDeclaration(
    device: W3D_DEVICE,
    declaration: u32,
) -> i32 {
    if device.is_null() || declaration == 0 {
        return 0;
    }
    let device_ref = &*device;
    if let Ok(mut declarations) = device_ref.vertex_declarations.lock() {
        declarations.remove(&declaration);
        return 1;
    }
    0
}

/// Set current vertex shader handle - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry; `shader` is an opaque by-value handle word, not a
// SAFETY: pointer. Only the device handle is dereferenced under its Mutex.
pub unsafe extern "C" fn W3DDevice_SetVertexShader(device: W3D_DEVICE, shader: u32) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut current_shader) = device_ref.current_vertex_shader.lock() {
        *current_shader = shader;
        return 1;
    }
    0
}

/// Get current vertex shader handle - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI query; opaque shader-word lookup under the device Mutex.
// SAFETY: By-value return, no pointer dereference beyond the live device handle.
pub unsafe extern "C" fn W3DDevice_GetVertexShader(device: W3D_DEVICE) -> u32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(current_shader) = device_ref.current_vertex_shader.lock() {
        return *current_shader;
    }
    0
}

/// Set current pixel shader handle - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI entry; `shader` is an opaque by-value handle word, not a
// SAFETY: pointer. Only the device handle is dereferenced under its Mutex.
pub unsafe extern "C" fn W3DDevice_SetPixelShader(device: W3D_DEVICE, shader: u32) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut current_shader) = device_ref.current_pixel_shader.lock() {
        *current_shader = shader;
        return 1;
    }
    0
}

/// Get current pixel shader handle - legacy compatibility entry point.
#[unsafe(no_mangle)]
// SAFETY: C ABI query; opaque shader-word lookup under the device Mutex.
// SAFETY: By-value return, no pointer dereference beyond the live device handle.
pub unsafe extern "C" fn W3DDevice_GetPixelShader(device: W3D_DEVICE) -> u32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(current_shader) = device_ref.current_pixel_shader.lock() {
        return *current_shader;
    }
    0
}
/// Get render state - matches original API
#[unsafe(no_mangle)]
// SAFETY: C ABI query; reads the state map under the device Mutex. Only the
// SAFETY: validated device handle is dereferenced; returns a by-value u32.
pub unsafe extern "C" fn W3DDevice_GetRenderState(
    device: W3D_DEVICE,
    state: W3D_RENDER_STATE,
) -> u32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(states) = device_ref.render_states.lock() {
        return states
            .get(&state)
            .copied()
            .unwrap_or_else(|| default_render_state_value(state));
    }

    default_render_state_value(state)
}
