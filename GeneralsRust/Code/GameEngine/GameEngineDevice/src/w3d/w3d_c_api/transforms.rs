//! W3D C API transform state and camera matrix sync.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::leftover::*;
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

/// Set transform - matches original W3DDevice::SetTransform(matrix)
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetTransform(
    device: W3D_DEVICE,
    state: W3D_TRANSFORM_STATE,
    matrix: *const W3D_MATRIX,
) -> i32 {
    if device.is_null() || matrix.is_null() {
        return 0; // Failure
    }

    let matrix_ref = &*matrix;
    let device_ref = &*device;
    if let Ok(mut states) = device_ref.transform_states.lock() {
        states.insert(state, *matrix_ref);
    }

    match device_ref
        .runtime
        .block_on(async { set_transform_internal(&device_ref.device, state, *matrix_ref).await })
    {
        Ok(_) => 1,  // Success
        Err(_) => 0, // Failure
    }
}

/// Get transform - matches original W3DDevice::GetTransform(state, matrix)
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_GetTransform(
    device: W3D_DEVICE,
    state: W3D_TRANSFORM_STATE,
    matrix: *mut W3D_MATRIX,
) -> i32 {
    if device.is_null() || matrix.is_null() {
        return 0;
    }

    let device_ref = &*device;
    let value = if let Ok(states) = device_ref.transform_states.lock() {
        states
            .get(&state)
            .copied()
            .unwrap_or_else(|| default_transform_state_value(state))
    } else {
        default_transform_state_value(state)
    };

    *matrix = value;
    1
}

pub(super) async fn set_transform_internal(
    device: &Arc<RwLock<W3DDevice>>,
    state: W3D_TRANSFORM_STATE,
    matrix: W3D_MATRIX,
) -> Result<()> {
    match state {
        W3D_TRANSFORM_STATE::W3DTS_WORLD => {
            tracing::debug!("Setting world matrix");
        }
        W3D_TRANSFORM_STATE::W3DTS_VIEW => {
            let device_lock = device.read().await;
            let mut scene = device_lock.get_scene().await;
            scene.camera.view_matrix = matrix.m;
            sync_camera_from_view_matrix(&mut scene.camera);
            device_lock.set_scene(scene).await?;
        }
        W3D_TRANSFORM_STATE::W3DTS_PROJECTION => {
            let device_lock = device.read().await;
            let mut scene = device_lock.get_scene().await;
            scene.camera.projection_matrix = matrix.m;
            sync_camera_from_projection_matrix(&mut scene.camera);
            device_lock.set_scene(scene).await?;
        }
        W3D_TRANSFORM_STATE::W3DTS_TEXTURE0
        | W3D_TRANSFORM_STATE::W3DTS_TEXTURE1
        | W3D_TRANSFORM_STATE::W3DTS_TEXTURE2
        | W3D_TRANSFORM_STATE::W3DTS_TEXTURE3 => {
            tracing::trace!("Set texture transform state {:?}", state);
        }
    }

    Ok(())
}

pub(super) fn current_world_transform(device: &W3DDeviceC) -> W3D_MATRIX {
    current_transform_value(device, W3D_TRANSFORM_STATE::W3DTS_WORLD)
}
pub(super) fn current_transform_value(
    device: &W3DDeviceC,
    state: W3D_TRANSFORM_STATE,
) -> W3D_MATRIX {
    if let Ok(states) = device.transform_states.lock() {
        states
            .get(&state)
            .copied()
            .unwrap_or_else(|| default_transform_state_value(state))
    } else {
        default_transform_state_value(state)
    }
}
pub(super) fn sync_camera_from_view_matrix(camera: &mut Camera) {
    let view = Mat4::from_cols_array_2d(&camera.view_matrix);
    let inverse = view.inverse();
    let position = inverse.transform_point3(Vec3::ZERO);
    if position.is_finite() {
        camera.position = position.to_array();
    }

    let forward = inverse.transform_vector3(Vec3::new(0.0, 0.0, -1.0));
    if forward.length_squared() > f32::EPSILON {
        let target = position + forward.normalize();
        if target.is_finite() {
            camera.target = target.to_array();
        }
    }

    let up = inverse.transform_vector3(Vec3::Y);
    if up.length_squared() > f32::EPSILON {
        let normalized = up.normalize();
        if normalized.is_finite() {
            camera.up = normalized.to_array();
        }
    }
}

pub(super) fn sync_camera_from_projection_matrix(camera: &mut Camera) {
    let projection = Mat4::from_cols_array_2d(&camera.projection_matrix).to_cols_array_2d();
    let m00 = projection[0][0];
    let m11 = projection[1][1];
    let m22 = projection[2][2];
    let m23 = projection[2][3];

    if m11.is_finite() && m11.abs() > f32::EPSILON {
        let fov = 2.0 * (1.0 / m11.abs()).atan();
        if fov.is_finite() && fov > 0.0 {
            camera.fov = fov;
        }
    }

    if m00.is_finite() && m00.abs() > f32::EPSILON && m11.is_finite() {
        let aspect = (m11 / m00).abs();
        if aspect.is_finite() && aspect > 0.0 {
            camera.aspect_ratio = aspect;
        }
    }

    if m22.is_finite() && m23.is_finite() {
        let near_denom = m22 - 1.0;
        let far_denom = m22 + 1.0;
        if near_denom.abs() > 1.0e-6 && far_denom.abs() > 1.0e-6 {
            let near_plane = m23 / near_denom;
            let far_plane = m23 / far_denom;
            if near_plane.is_finite() && far_plane.is_finite() {
                let near_plane = near_plane.abs();
                let far_plane = far_plane.abs();
                if near_plane > 0.0 && far_plane > near_plane {
                    camera.near_plane = near_plane;
                    camera.far_plane = far_plane;
                }
            }
        }
    }
}
pub(super) fn default_transform_state_value(state: W3D_TRANSFORM_STATE) -> W3D_MATRIX {
    match state {
        W3D_TRANSFORM_STATE::W3DTS_WORLD
        | W3D_TRANSFORM_STATE::W3DTS_VIEW
        | W3D_TRANSFORM_STATE::W3DTS_PROJECTION
        | W3D_TRANSFORM_STATE::W3DTS_TEXTURE0
        | W3D_TRANSFORM_STATE::W3DTS_TEXTURE1
        | W3D_TRANSFORM_STATE::W3DTS_TEXTURE2
        | W3D_TRANSFORM_STATE::W3DTS_TEXTURE3 => W3D_MATRIX::from(Mat4::IDENTITY),
    }
}

pub(super) fn default_transform_states() -> HashMap<W3D_TRANSFORM_STATE, W3D_MATRIX> {
    let mut states = HashMap::new();
    for state in [
        W3D_TRANSFORM_STATE::W3DTS_WORLD,
        W3D_TRANSFORM_STATE::W3DTS_VIEW,
        W3D_TRANSFORM_STATE::W3DTS_PROJECTION,
        W3D_TRANSFORM_STATE::W3DTS_TEXTURE0,
        W3D_TRANSFORM_STATE::W3DTS_TEXTURE1,
        W3D_TRANSFORM_STATE::W3DTS_TEXTURE2,
        W3D_TRANSFORM_STATE::W3DTS_TEXTURE3,
    ] {
        states.insert(state, default_transform_state_value(state));
    }
    states
}
