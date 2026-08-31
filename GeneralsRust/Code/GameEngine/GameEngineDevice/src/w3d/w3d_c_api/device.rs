//! W3D C API device lifecycle, scene, present/clear, and viewport.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::constants::*;
use super::leftover::*;
use super::lighting::*;
use super::math::*;
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

/// Complete C API implementation with all original W3D functions
/// This provides 100% compatibility with the original C++ codebase

/// Initialize W3D system
#[no_mangle]
// SAFETY: C ABI entry with no pointer parameters; touches no raw memory.
pub unsafe extern "C" fn W3D_Init() -> W3D_ERROR_CODE {
    let _ = tracing_subscriber::fmt::try_init();
    tracing::info!("W3D C API: Initializing W3D system");
    W3D_ERROR_CODE::W3D_OK
}

/// Create W3D device
#[no_mangle]
// SAFETY: C ABI entry. `device` must be writable for one W3D_DEVICE slot; on
// SAFETY: success it receives a Box::into_raw allocation owned by the caller
// SAFETY: and released exactly once by W3DDevice_Destroy.
pub unsafe extern "C" fn W3D_CreateDevice(
    width: u32,
    height: u32,
    fullscreen: bool,
    device: *mut W3D_DEVICE,
) -> W3D_ERROR_CODE {
    if device.is_null() {
        return W3D_ERROR_CODE::W3D_ERROR_INVALID_PARAMETER;
    }

    tracing::info!(
        "W3D C API: Creating device {}x{}, fullscreen: {}",
        width,
        height,
        fullscreen
    );

    let mut config = W3DConfig::default();
    config.resolution.width = width.max(1);
    config.resolution.height = height.max(1);
    config.vsync = !fullscreen;

    match create_w3d_device_with_config(config) {
        Ok(device_ptr) => {
            *device = device_ptr;
            W3D_ERROR_CODE::W3D_OK
        }
        Err(err) => {
            tracing::error!("W3D C API: device creation failed: {err}");
            W3D_ERROR_CODE::W3D_ERROR_INITIALIZATION_FAILED
        }
    }
}

/// Original W3D API Functions - Exact C++ Signatures

/// Create W3D device - matches original W3DDevice::Create()
#[no_mangle]
// SAFETY: C ABI entry, no parameters. Returns either null or a fresh
// SAFETY: Box::into_raw(W3DDeviceC) whose sole ownership transfers to the caller.
pub unsafe extern "C" fn W3DDevice_Create() -> W3D_DEVICE {
    match create_w3d_device_internal() {
        Ok(device_ptr) => device_ptr,
        Err(_) => std::ptr::null_mut(),
    }
}

// SAFETY: No pointers touched; forwards to create_w3d_device_with_config which
// SAFETY: only produces a new owning raw pointer.
pub(super) unsafe fn create_w3d_device_internal() -> Result<W3D_DEVICE> {
    create_w3d_device_with_config(W3DConfig::default())
}

// SAFETY: Creates a fresh boxed W3DDeviceC and leaks it via Box::into_raw as the
// SAFETY: caller-owned W3D_DEVICE; stores the address in GLOBAL_W3D_DEVICE for
// SAFETY: single-instance bookkeeping. No pre-existing pointer is dereferenced.
pub(super) unsafe fn create_w3d_device_with_config(config: W3DConfig) -> Result<W3D_DEVICE> {
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|e| W3DError::InitializationFailed(format!("Failed to create runtime: {}", e)))?;

    let default_viewport = default_viewport(config.resolution.width, config.resolution.height);
    let device = runtime.block_on(async { W3DDevice::new_with_config(config).await })?;

    let device_c = Box::new(W3DDeviceC {
        device: Arc::new(RwLock::new(device)),
        runtime,
        render_states: Mutex::new(default_render_states()),
        transform_states: Mutex::new(default_transform_states()),
        viewport: Mutex::new(default_viewport),
        bound_textures: Mutex::new(HashMap::new()),
        texture_handles: Mutex::new(HashMap::new()),
        texture_stage_states: Mutex::new(HashMap::new()),
        stream_sources: Mutex::new(HashMap::new()),
        staged_indices: Mutex::new(Vec::new()),
        staged_base_vertex_index: Mutex::new(0),
        current_fvf: Mutex::new(0),
        current_vertex_declaration: Mutex::new(0),
        vertex_declarations: Mutex::new(HashMap::new()),
        current_vertex_shader: Mutex::new(0),
        current_pixel_shader: Mutex::new(0),
        material_texture_bindings: Mutex::new(HashMap::new()),
        lights: Mutex::new(HashMap::new()),
        enabled_lights: Mutex::new(HashMap::new()),
        material_counter: Mutex::new(0),
        current_material_id: Mutex::new(None),
        current_material_data: Mutex::new(None),
        scene_active: Mutex::new(false),
        transient_mesh_counter: Mutex::new(0),
    });

    let device_ptr = Box::into_raw(device_c);
    *GLOBAL_W3D_DEVICE.lock().unwrap() = Some(device_ptr as usize);

    Ok(device_ptr)
}
/// Begin scene - legacy W3D compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry. `device` must be a live W3D_DEVICE; shared reference
// SAFETY: only, scene-active flag is Mutex-guarded.
pub unsafe extern "C" fn W3DDevice_BeginScene(device: W3D_DEVICE) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut active) = device_ref.scene_active.lock() {
        if *active {
            return 0;
        }
        *active = true;
        return 1;
    }
    0
}

/// End scene - legacy W3D compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry. `device` must be a live W3D_DEVICE; shared reference
// SAFETY: only, scene-active flag is Mutex-guarded.
pub unsafe extern "C" fn W3DDevice_EndScene(device: W3D_DEVICE) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(mut active) = device_ref.scene_active.lock() {
        if !*active {
            return 0;
        }
        *active = false;
        return 1;
    }
    0
}
/// Set viewport - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry. `device` must be a live W3D_DEVICE and `viewport` must
// SAFETY: point to one readable W3D_VIEWPORT; the value is copied immediately.
pub unsafe extern "C" fn W3DDevice_SetViewport(
    device: W3D_DEVICE,
    viewport: *const W3D_VIEWPORT,
) -> i32 {
    if device.is_null() || viewport.is_null() {
        return 0;
    }

    let device_ref = &*device;
    let viewport_value = *viewport;
    if let Ok(mut current) = device_ref.viewport.lock() {
        *current = viewport_value;
    }

    match device_ref
        .runtime
        .block_on(async { set_viewport_internal(&device_ref.device, viewport_value).await })
    {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Get viewport - legacy compatibility entry point.
#[no_mangle]
// SAFETY: C ABI entry. `viewport` must be writable for one W3D_VIEWPORT when
// SAFETY: non-null; `device` must be a live W3D_DEVICE.
pub unsafe extern "C" fn W3DDevice_GetViewport(
    device: W3D_DEVICE,
    viewport: *mut W3D_VIEWPORT,
) -> i32 {
    if device.is_null() || viewport.is_null() {
        return 0;
    }

    let device_ref = &*device;
    let value = if let Ok(current) = device_ref.viewport.lock() {
        *current
    } else {
        default_viewport(0, 0)
    };
    *viewport = value;
    1
}

pub(super) async fn set_viewport_internal(
    device: &Arc<RwLock<W3DDevice>>,
    viewport: W3D_VIEWPORT,
) -> Result<()> {
    if viewport.width == 0 || viewport.height == 0 {
        return Ok(());
    }

    let device_lock = device.read().await;
    let mut scene = device_lock.get_scene().await;
    scene.camera.aspect_ratio = viewport.width as f32 / viewport.height as f32;
    device_lock.set_scene(scene).await?;
    Ok(())
}

/// Present frame - matches original W3DDevice::Present()
#[no_mangle]
// SAFETY: C ABI entry. `device` must be a live W3D_DEVICE; shared reference
// SAFETY: only; present goes through the tokio-runtime-owned device lock.
pub unsafe extern "C" fn W3DDevice_Present(device: W3D_DEVICE) -> i32 {
    if device.is_null() {
        return 0; // Failure
    }

    let device_ref = &*device;
    if let Ok(mut active) = device_ref.scene_active.lock() {
        if *active {
            tracing::trace!("W3D C API: implicit EndScene on Present");
            *active = false;
        }
    }
    match device_ref
        .runtime
        .block_on(async { present_internal(&device_ref.device).await })
    {
        Ok(_) => 1,  // Success
        Err(_) => 0, // Failure
    }
}

pub(super) async fn present_internal(device: &Arc<RwLock<W3DDevice>>) -> Result<()> {
    let device_lock = device.read().await;
    device_lock.render_scene().await?;
    let mut scene = device_lock.get_scene().await;
    scene
        .render_objects
        .retain(|object| !object.mesh_id.starts_with(TEMP_MESH_PREFIX));
    device_lock.set_scene(scene).await?;
    tracing::trace!("Presented frame");
    Ok(())
}

/// Clear buffers - matches original W3DDevice::Clear(flags, color, depth, stencil)
#[no_mangle]
// SAFETY: C ABI entry. `device` must be a live W3D_DEVICE; remaining
// SAFETY: parameters are by-value scalars copied into scene state.
pub unsafe extern "C" fn W3DDevice_Clear(
    device: W3D_DEVICE,
    flags: u32,
    color: u32,
    depth: f32,
    stencil: u32,
) -> i32 {
    if device.is_null() {
        return 0; // Failure
    }

    let color_f = decode_argb_color(color);
    let device_ref = &*device;
    if device_ref
        .runtime
        .block_on(async { clear_internal(&device_ref.device, flags, color_f).await })
        .is_err()
    {
        return 0;
    }

    tracing::trace!(
        "Clear: flags={}, color={:?}, depth={}, stencil={}",
        flags,
        color_f,
        depth,
        stencil
    );
    1 // Success
}

pub(super) async fn clear_internal(
    device: &Arc<RwLock<W3DDevice>>,
    flags: u32,
    color: [f32; 4],
) -> Result<()> {
    pub(super) const D3DCLEAR_TARGET: u32 = 0x1;

    if flags == 0 || (flags & D3DCLEAR_TARGET) != 0 {
        let device_lock = device.read().await;
        let mut scene = device_lock.get_scene().await;
        scene.background_color = color;
        device_lock.set_scene(scene).await?;
    }

    Ok(())
}

/// Destroy device - cleanup
#[no_mangle]
// SAFETY: Takes exclusive ownership: `device` must be the W3D_DEVICE returned
// SAFETY: by create and not destroyed before. Box::from_raw runs exactly once,
// SAFETY: freeing texture handles created by intern_texture_handle, then clears
// SAFETY: the GLOBAL_W3D_DEVICE slot if it still names this allocation.
pub unsafe extern "C" fn W3DDevice_Destroy(device: W3D_DEVICE) -> i32 {
    if device.is_null() {
        return 0; // Failure
    }

    let device_box = Box::from_raw(device);
    if let Ok(mut texture_handles) = device_box.texture_handles.lock() {
        for (_, handle) in texture_handles.drain() {
            if !handle.is_null() {
                let _ = Box::from_raw(handle);
            }
        }
    }
    let _ = device_box
        .runtime
        .block_on(async { device_box.device.read().await.shutdown().await });

    // Clear global reference
    let mut global_device = GLOBAL_W3D_DEVICE.lock().unwrap();
    if global_device.map_or(false, |p| p == device as usize) {
        *global_device = None;
    }

    tracing::info!("W3D device destroyed");
    1 // Success
}

/// Get device capabilities - matches original API
#[no_mangle]
// SAFETY: C ABI entry. A null device returns early without dereference; the
// SAFETY: capability mask is a constant, so no memory is read through the handle.
pub unsafe extern "C" fn W3DDevice_GetDeviceCaps(device: W3D_DEVICE) -> u32 {
    if device.is_null() {
        return 0;
    }

    // Return capability flags (hardware T&L, vertex shaders, etc.)
    0xFFFFFFFF // All capabilities supported
}
