//! W3D C API lights and fixed-function lighting state.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::leftover::*;
use super::materials::*;
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

/// Set light - legacy compatibility entry point.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetLight(
    device: W3D_DEVICE,
    index: u32,
    light_data: *const W3DLightData,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut lights) = device_ref.lights.lock() {
        if light_data.is_null() {
            lights.remove(&index);
            if let Ok(mut enabled) = device_ref.enabled_lights.lock() {
                enabled.remove(&index);
            }
        } else {
            if !is_valid_ptr(light_data) {
                return 0;
            }
            lights.insert(index, c_light_data_to_light(index, *light_data));
            if let Ok(mut enabled) = device_ref.enabled_lights.lock() {
                enabled.entry(index).or_insert(true);
            }
        }
    } else {
        return 0;
    }

    let current_lights = current_scene_lights(device_ref);
    match device_ref
        .runtime
        .block_on(async { set_lights_internal(&device_ref.device, current_lights).await })
    {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Get light - legacy compatibility entry point.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_GetLight(
    device: W3D_DEVICE,
    index: u32,
    out_light_data: *mut W3DLightData,
) -> i32 {
    if device.is_null() || out_light_data.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(lights) = device_ref.lights.lock() {
        if let Some(light) = lights.get(&index) {
            *out_light_data = light_to_c_data(light);
            return 1;
        }
    }
    0
}

/// Enable/disable light index - legacy D3D-style compatibility entry point.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_LightEnable(device: W3D_DEVICE, index: u32, enable: i32) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut enabled_lights) = device_ref.enabled_lights.lock() {
        enabled_lights.insert(index, enable != 0);
    } else {
        return 0;
    }

    let current_lights = current_scene_lights(device_ref);
    match device_ref
        .runtime
        .block_on(async { set_lights_internal(&device_ref.device, current_lights).await })
    {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Alias for legacy callers expecting `SetLightEnable`.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetLightEnable(
    device: W3D_DEVICE,
    index: u32,
    enable: i32,
) -> i32 {
    W3DDevice_LightEnable(device, index, enable)
}

/// Query whether a light index is enabled.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_GetLightEnable(device: W3D_DEVICE, index: u32) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(enabled_lights) = device_ref.enabled_lights.lock() {
        if let Some(enabled) = enabled_lights.get(&index) {
            return if *enabled { 1 } else { 0 };
        }
    }
    if let Ok(lights) = device_ref.lights.lock() {
        if lights.contains_key(&index) {
            return 1;
        }
    }
    0
}
pub(super) async fn set_lights_internal(
    device: &Arc<RwLock<W3DDevice>>,
    lights: Vec<Light>,
) -> Result<()> {
    let device_lock = device.read().await;
    let mut scene = device_lock.get_scene().await;
    scene.lights = lights;
    device_lock.set_scene(scene).await?;
    Ok(())
}
pub(super) fn current_fixed_function_lighting_state(
    device: &W3DDeviceC,
) -> FixedFunctionLightingState {
    FixedFunctionLightingState {
        lighting_enabled: render_state_value(device, W3D_RENDER_STATE::W3DRS_LIGHTING) != 0,
        specular_enabled: render_state_value(device, W3D_RENDER_STATE::W3DRS_SPECULARENABLE) != 0,
        color_vertex: render_state_value(device, W3D_RENDER_STATE::W3DRS_COLORVERTEX) != 0,
        local_viewer: render_state_value(device, W3D_RENDER_STATE::W3DRS_LOCALVIEWER) != 0,
        normalize_normals: render_state_value(device, W3D_RENDER_STATE::W3DRS_NORMALIZENORMALS)
            != 0,
        ambient_argb: render_state_value(device, W3D_RENDER_STATE::W3DRS_AMBIENT),
        ambient_material_source: render_state_value(
            device,
            W3D_RENDER_STATE::W3DRS_AMBIENTMATERIALSOURCE,
        ),
        diffuse_material_source: render_state_value(
            device,
            W3D_RENDER_STATE::W3DRS_DIFFUSEMATERIALSOURCE,
        ),
        specular_material_source: render_state_value(
            device,
            W3D_RENDER_STATE::W3DRS_SPECULARMATERIALSOURCE,
        ),
        emissive_material_source: render_state_value(
            device,
            W3D_RENDER_STATE::W3DRS_EMISSIVEMATERIALSOURCE,
        ),
    }
}

pub(super) fn lighting_state_requires_material_variant(state: FixedFunctionLightingState) -> bool {
    state != default_fixed_function_lighting_state()
}

pub(super) fn default_fixed_function_lighting_state() -> FixedFunctionLightingState {
    FixedFunctionLightingState {
        lighting_enabled: default_render_state_value(W3D_RENDER_STATE::W3DRS_LIGHTING) != 0,
        specular_enabled: default_render_state_value(W3D_RENDER_STATE::W3DRS_SPECULARENABLE) != 0,
        color_vertex: default_render_state_value(W3D_RENDER_STATE::W3DRS_COLORVERTEX) != 0,
        local_viewer: default_render_state_value(W3D_RENDER_STATE::W3DRS_LOCALVIEWER) != 0,
        normalize_normals: default_render_state_value(W3D_RENDER_STATE::W3DRS_NORMALIZENORMALS)
            != 0,
        ambient_argb: default_render_state_value(W3D_RENDER_STATE::W3DRS_AMBIENT),
        ambient_material_source: default_render_state_value(
            W3D_RENDER_STATE::W3DRS_AMBIENTMATERIALSOURCE,
        ),
        diffuse_material_source: default_render_state_value(
            W3D_RENDER_STATE::W3DRS_DIFFUSEMATERIALSOURCE,
        ),
        specular_material_source: default_render_state_value(
            W3D_RENDER_STATE::W3DRS_SPECULARMATERIALSOURCE,
        ),
        emissive_material_source: default_render_state_value(
            W3D_RENDER_STATE::W3DRS_EMISSIVEMATERIALSOURCE,
        ),
    }
}

pub(super) fn current_fixed_function_surface_state(
    device: &W3DDeviceC,
) -> FixedFunctionSurfaceState {
    FixedFunctionSurfaceState {
        alpha_test_enabled: render_state_value(device, W3D_RENDER_STATE::W3DRS_ALPHATESTENABLE)
            != 0,
        alpha_ref: render_state_value(device, W3D_RENDER_STATE::W3DRS_ALPHAREF) as u8,
        alpha_blend_enabled: render_state_value(device, W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE)
            != 0,
        cull_mode: render_state_value(device, W3D_RENDER_STATE::W3DRS_CULLMODE),
    }
}

pub(super) fn default_fixed_function_surface_state() -> FixedFunctionSurfaceState {
    FixedFunctionSurfaceState {
        alpha_test_enabled: default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHATESTENABLE)
            != 0,
        alpha_ref: default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHAREF) as u8,
        alpha_blend_enabled: default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE)
            != 0,
        cull_mode: default_render_state_value(W3D_RENDER_STATE::W3DRS_CULLMODE),
    }
}

pub(super) fn surface_state_requires_material_variant(state: FixedFunctionSurfaceState) -> bool {
    state != default_fixed_function_surface_state()
}

pub(super) fn material_source_uses_material(source: u32, color_vertex: bool) -> bool {
    if !color_vertex {
        return true;
    }
    source == D3DMCS_MATERIAL
}

pub(super) fn multiply_rgb(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [lhs[0] * rhs[0], lhs[1] * rhs[1], lhs[2] * rhs[2]]
}

pub(super) fn add_rgb(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [
        (lhs[0] + rhs[0]).clamp(0.0, 1.0),
        (lhs[1] + rhs[1]).clamp(0.0, 1.0),
        (lhs[2] + rhs[2]).clamp(0.0, 1.0),
    ]
}
pub(super) fn current_scene_lights(device: &W3DDeviceC) -> Vec<Light> {
    if let Ok(lights) = device.lights.lock() {
        let enabled_lights = device
            .enabled_lights
            .lock()
            .ok()
            .map(|flags| flags.clone())
            .unwrap_or_default();
        let mut entries: Vec<(u32, Light)> = lights
            .iter()
            .filter_map(|(k, v)| {
                if enabled_lights.get(k).copied().unwrap_or(true) {
                    Some((*k, v.clone()))
                } else {
                    None
                }
            })
            .collect();
        entries.sort_by_key(|(k, _)| *k);
        entries.into_iter().map(|(_, light)| light).collect()
    } else {
        Vec::new()
    }
}
pub(super) fn c_light_data_to_light(index: u32, data: W3DLightData) -> Light {
    let light_type = match data.light_type {
        0 => crate::w3d::LightType::Directional,
        1 => crate::w3d::LightType::Point,
        2 => crate::w3d::LightType::Spot,
        3 => crate::w3d::LightType::Area,
        _ => crate::w3d::LightType::Directional,
    };
    Light {
        id: format!("__w3d_c_api_light_{index}"),
        name: format!("W3D C API Light {index}"),
        light_type,
        position: data.position,
        direction: data.direction,
        color: data.color,
        intensity: if data.intensity.is_finite() {
            data.intensity.max(0.0)
        } else {
            1.0
        },
        attenuation: [1.0, 0.0, 0.0],
        spot_params: if light_type == crate::w3d::LightType::Spot {
            Some([0.9, 0.75])
        } else {
            None
        },
    }
}

pub(super) fn light_to_c_data(light: &Light) -> W3DLightData {
    let light_type = match light.light_type {
        crate::w3d::LightType::Directional => 0,
        crate::w3d::LightType::Point => 1,
        crate::w3d::LightType::Spot => 2,
        crate::w3d::LightType::Area => 3,
    };
    W3DLightData {
        position: light.position,
        direction: light.direction,
        color: light.color,
        intensity: light.intensity,
        light_type,
    }
}
