//! # W3D C++ API Compatibility Layer
//!
//! This module provides 100% compatibility with the original Westwood 3D C++ API
//! while using the modern Rust/wgpu backend underneath. All function signatures
//! match the original W3D API exactly.

use super::renderer::{batch_material_params, batch_priority};
use super::w3d_device::RenderObject;
use super::{
    Camera, Light, Material, Mesh, Result, Texture, W3DConfig, W3DDevice, W3DError, W3DLightData,
    W3DMaterialData, W3DVertex,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::ffi::{c_char, c_void, CStr, CString};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::RwLock;

/// W3D Device handle for C++ compatibility
pub type W3D_DEVICE = *mut W3DDeviceC;

/// W3D Texture handle for C++ compatibility
pub type W3D_TEXTURE = *mut W3DTextureC;

/// W3D Mesh handle for C++ compatibility
pub type W3D_MESH = *mut W3DMeshC;

/// W3D Material handle for C++ compatibility
pub type W3D_MATERIAL = *mut W3DMaterialC;

/// Error codes used by legacy C API callers.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3D_ERROR_CODE {
    W3D_OK = 0,
    W3D_ERROR_INVALID_PARAMETER = -1,
    W3D_ERROR_INITIALIZATION_FAILED = -2,
    W3D_ERROR_RESOURCE_LOADING_FAILED = -3,
}

/// W3D primitive types matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum W3D_PRIMITIVE_TYPE {
    W3D_TRIANGLES = 0,
    W3D_TRIANGLE_STRIP = 1,
    W3D_TRIANGLE_FAN = 2,
    W3D_LINES = 3,
    W3D_LINE_STRIP = 4,
    W3D_POINTS = 5,
}

/// W3D render states matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum W3D_RENDER_STATE {
    W3DRS_ZENABLE = 1,
    W3DRS_FILLMODE = 2,
    W3DRS_SHADEMODE = 3,
    W3DRS_ZWRITEENABLE = 4,
    W3DRS_ALPHATESTENABLE = 5,
    W3DRS_LASTPIXEL = 6,
    W3DRS_SRCBLEND = 7,
    W3DRS_DESTBLEND = 8,
    W3DRS_CULLMODE = 9,
    W3DRS_ZFUNC = 10,
    W3DRS_ALPHAREF = 11,
    W3DRS_ALPHAFUNC = 12,
    W3DRS_DITHERENABLE = 13,
    W3DRS_ALPHABLENDENABLE = 14,
    W3DRS_FOGENABLE = 15,
    W3DRS_SPECULARENABLE = 16,
    W3DRS_FOGCOLOR = 17,
    W3DRS_FOGTABLEMODE = 18,
    W3DRS_FOGSTART = 19,
    W3DRS_FOGEND = 20,
    W3DRS_FOGDENSITY = 21,
    W3DRS_RANGEFOGENABLE = 22,
    W3DRS_STENCILENABLE = 23,
    W3DRS_STENCILFAIL = 24,
    W3DRS_STENCILZFAIL = 25,
    W3DRS_STENCILPASS = 26,
    W3DRS_STENCILFUNC = 27,
    W3DRS_STENCILREF = 28,
    W3DRS_STENCILMASK = 29,
    W3DRS_STENCILWRITEMASK = 30,
    W3DRS_TEXTUREFACTOR = 31,
    W3DRS_LIGHTING = 137,
    W3DRS_AMBIENT = 139,
    W3DRS_COLORVERTEX = 141,
    W3DRS_LOCALVIEWER = 142,
    W3DRS_NORMALIZENORMALS = 143,
    W3DRS_DIFFUSEMATERIALSOURCE = 145,
    W3DRS_SPECULARMATERIALSOURCE = 146,
    W3DRS_AMBIENTMATERIALSOURCE = 147,
    W3DRS_EMISSIVEMATERIALSOURCE = 148,
}

/// W3D transform states matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum W3D_TRANSFORM_STATE {
    W3DTS_VIEW = 1,
    W3DTS_PROJECTION = 2,
    W3DTS_TEXTURE0 = 3,
    W3DTS_TEXTURE1 = 4,
    W3DTS_TEXTURE2 = 5,
    W3DTS_TEXTURE3 = 6,
    W3DTS_WORLD = 7,
}

/// W3D matrix structure matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3D_MATRIX {
    pub m: [[f32; 4]; 4],
}

impl From<Mat4> for W3D_MATRIX {
    fn from(mat: Mat4) -> Self {
        Self {
            m: mat.to_cols_array_2d(),
        }
    }
}

impl From<W3D_MATRIX> for Mat4 {
    fn from(mat: W3D_MATRIX) -> Self {
        Mat4::from_cols_array_2d(&mat.m)
    }
}

/// W3D Vector structure matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3D_VECTOR {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Vec3> for W3D_VECTOR {
    fn from(vec: Vec3) -> Self {
        Self {
            x: vec.x,
            y: vec.y,
            z: vec.z,
        }
    }
}

impl From<W3D_VECTOR> for Vec3 {
    fn from(vec: W3D_VECTOR) -> Self {
        Self::new(vec.x, vec.y, vec.z)
    }
}

/// Viewport structure matching legacy D3D layout used by W3D callers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3D_VIEWPORT {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub min_z: f32,
    pub max_z: f32,
}

/// Complete C API implementation with all original W3D functions
/// This provides 100% compatibility with the original C++ codebase

/// Initialize W3D system
#[no_mangle]
pub unsafe extern "C" fn W3D_Init() -> W3D_ERROR_CODE {
    let _ = tracing_subscriber::fmt::try_init();
    tracing::info!("W3D C API: Initializing W3D system");
    W3D_ERROR_CODE::W3D_OK
}

/// Create W3D device
#[no_mangle]
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

/// W3D vertex structure matching original W3D API
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3D_VERTEX {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub u: f32,
    pub v: f32,
    pub color: u32,
}

/// D3D-style vertex declaration element for legacy multi-stream layouts.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3D_VERTEX_ELEMENT {
    pub stream: u16,
    pub offset: u16,
    pub decl_type: u8,
    pub method: u8,
    pub usage: u8,
    pub usage_index: u8,
}

/// W3D Device C wrapper
pub struct W3DDeviceC {
    device: Arc<RwLock<W3DDevice>>,
    runtime: tokio::runtime::Runtime,
    render_states: Mutex<HashMap<W3D_RENDER_STATE, u32>>,
    transform_states: Mutex<HashMap<W3D_TRANSFORM_STATE, W3D_MATRIX>>,
    viewport: Mutex<W3D_VIEWPORT>,
    bound_textures: Mutex<HashMap<u32, String>>,
    texture_handles: Mutex<HashMap<String, W3D_TEXTURE>>,
    texture_stage_states: Mutex<HashMap<(u32, u32), u32>>,
    stream_sources: Mutex<HashMap<u32, StagedStreamSource>>,
    staged_indices: Mutex<Vec<u16>>,
    staged_base_vertex_index: Mutex<i32>,
    current_fvf: Mutex<u32>,
    current_vertex_declaration: Mutex<u32>,
    vertex_declarations: Mutex<HashMap<u32, Vec<W3D_VERTEX_ELEMENT>>>,
    current_vertex_shader: Mutex<u32>,
    current_pixel_shader: Mutex<u32>,
    material_texture_bindings: Mutex<HashMap<MaterialBindingCacheKey, String>>,
    lights: Mutex<HashMap<u32, Light>>,
    enabled_lights: Mutex<HashMap<u32, bool>>,
    material_counter: Mutex<u64>,
    current_material_id: Mutex<Option<String>>,
    current_material_data: Mutex<Option<W3DMaterialData>>,
    scene_active: Mutex<bool>,
    transient_mesh_counter: Mutex<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MaterialBindingCacheKey {
    base_material_id: Option<String>,
    texture_id: String,
    tint_rgba: [u8; 4],
    combiner_signature: MaterialCombinerSignature,
    lighting_state: FixedFunctionLightingState,
    surface_state: FixedFunctionSurfaceState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FixedFunctionLightingState {
    lighting_enabled: bool,
    specular_enabled: bool,
    color_vertex: bool,
    local_viewer: bool,
    normalize_normals: bool,
    ambient_argb: u32,
    ambient_material_source: u32,
    diffuse_material_source: u32,
    specular_material_source: u32,
    emissive_material_source: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FixedFunctionSurfaceState {
    alpha_test_enabled: bool,
    alpha_ref: u8,
    alpha_blend_enabled: bool,
    cull_mode: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MaterialCombinerSignature {
    sampling_stage_count: u8,
    force_multiply_like: bool,
}

#[derive(Debug, Clone)]
struct StagedStreamSource {
    vertex_stride: usize,
    vertex_offset_bytes: usize,
    vertex_count: usize,
    data: Vec<u8>,
}

/// W3D Texture C wrapper
pub struct W3DTextureC {
    texture: Texture,
}

/// W3D Mesh C wrapper
pub struct W3DMeshC {
    mesh: Mesh,
}

/// W3D Material C wrapper
pub struct W3DMaterialC {
    material: Material,
}

// Global device instance for C compatibility
static GLOBAL_W3D_DEVICE: std::sync::Mutex<Option<usize>> = std::sync::Mutex::new(None);
const TEMP_MESH_PREFIX: &str = "__w3d_c_api_temp_";
const TEMP_MESH_RING_SIZE: u64 = 4096;
const D3DFVF_XYZ: u32 = 0x002;
const D3DFVF_XYZRHW: u32 = 0x004;
const D3DFVF_NORMAL: u32 = 0x010;
const D3DFVF_DIFFUSE: u32 = 0x040;
const D3DFVF_SPECULAR: u32 = 0x080;
const D3DFVF_TEXCOUNT_MASK: u32 = 0xF00;
const D3DFVF_TEXCOUNT_SHIFT: u32 = 8;
const D3DFVF_TEXCOORDFORMAT_MASK: u32 = 0x3;
const D3DFVF_TEXCOORDFORMAT_SHIFT: u32 = 16;
const DEFAULT_FVF_TL1: u32 =
    D3DFVF_XYZRHW | D3DFVF_DIFFUSE | D3DFVF_SPECULAR | (1 << D3DFVF_TEXCOUNT_SHIFT);
const D3DFMT_INDEX16: u32 = 101;
const D3DFMT_INDEX32: u32 = 102;
const D3DDECLTYPE_FLOAT1: u8 = 0;
const D3DDECLTYPE_FLOAT2: u8 = 1;
const D3DDECLTYPE_FLOAT3: u8 = 2;
const D3DDECLTYPE_FLOAT4: u8 = 3;
const D3DDECLTYPE_D3DCOLOR: u8 = 4;
const D3DDECLTYPE_UBYTE4: u8 = 5;
const D3DDECLTYPE_SHORT2: u8 = 6;
const D3DDECLTYPE_SHORT4: u8 = 7;
const D3DDECLTYPE_UBYTE4N: u8 = 8;
const D3DDECLTYPE_SHORT2N: u8 = 9;
const D3DDECLTYPE_SHORT4N: u8 = 10;
const D3DDECLTYPE_USHORT2N: u8 = 11;
const D3DDECLTYPE_USHORT4N: u8 = 12;
const D3DDECLTYPE_UDEC3: u8 = 13;
const D3DDECLTYPE_DEC3N: u8 = 14;
const D3DDECLTYPE_UNUSED: u8 = 17;
const D3DDECLUSAGE_POSITION: u8 = 0;
const D3DDECLUSAGE_NORMAL: u8 = 3;
const D3DDECLUSAGE_TEXCOORD: u8 = 5;
const D3DDECLUSAGE_POSITIONT: u8 = 9;
const D3DDECLUSAGE_COLOR: u8 = 10;
const D3DTSS_COLOROP: u32 = 1;
const D3DTSS_COLORARG1: u32 = 2;
const D3DTSS_COLORARG2: u32 = 3;
const D3DTSS_ALPHAOP: u32 = 4;
const D3DTSS_ALPHAARG1: u32 = 5;
const D3DTSS_ALPHAARG2: u32 = 6;
const D3DMCS_MATERIAL: u32 = 0;
const D3DMCS_COLOR1: u32 = 1;
const D3DMCS_COLOR2: u32 = 2;
const D3DTSS_COLORARG0: u32 = 26;
const D3DTSS_ALPHAARG0: u32 = 27;
const D3DTSS_TEXCOORDINDEX: u32 = 11;
const D3DTSS_TEXTURETRANSFORMFLAGS: u32 = 24;
const D3DTSS_TCI_PASSTHRU: u32 = 0x0000_0000;
const D3DTSS_TCI_CAMERASPACENORMAL: u32 = 0x0001_0000;
const D3DTSS_TCI_CAMERASPACEPOSITION: u32 = 0x0002_0000;
const D3DTSS_TCI_CAMERASPACEREFLECTIONVECTOR: u32 = 0x0003_0000;
const D3DTSS_TCI_SPHEREMAP: u32 = 0x0004_0000;
const D3DTSS_TCI_MASK: u32 = 0xFFFF_0000;
const D3DTOP_DISABLE: u32 = 1;
const D3DTOP_SELECTARG1: u32 = 2;
const D3DTOP_SELECTARG2: u32 = 3;
const D3DTOP_MODULATE: u32 = 4;
const D3DTOP_MODULATE2X: u32 = 5;
const D3DTOP_MODULATE4X: u32 = 6;
const D3DTOP_ADD: u32 = 7;
const D3DTOP_ADDSIGNED: u32 = 8;
const D3DTOP_ADDSIGNED2X: u32 = 9;
const D3DTOP_SUBTRACT: u32 = 10;
const D3DTOP_ADDSMOOTH: u32 = 11;
const D3DTOP_BLENDDIFFUSEALPHA: u32 = 12;
const D3DTOP_BLENDTEXTUREALPHA: u32 = 13;
const D3DTOP_BLENDFACTORALPHA: u32 = 14;
const D3DTOP_BLENDTEXTUREALPHAPM: u32 = 15;
const D3DTOP_BLENDCURRENTALPHA: u32 = 16;
const D3DTOP_PREMODULATE: u32 = 17;
const D3DTOP_MODULATEALPHA_ADDCOLOR: u32 = 18;
const D3DTOP_MODULATECOLOR_ADDALPHA: u32 = 19;
const D3DTOP_MODULATEINVALPHA_ADDCOLOR: u32 = 20;
const D3DTOP_MODULATEINVCOLOR_ADDALPHA: u32 = 21;
const D3DTOP_BUMPENVMAP: u32 = 22;
const D3DTOP_BUMPENVMAPLUMINANCE: u32 = 23;
const D3DTOP_DOTPRODUCT3: u32 = 24;
const D3DTOP_MULTIPLYADD: u32 = 25;
const D3DTOP_LERP: u32 = 26;
const D3DTA_SELECTMASK: u32 = 0xF;
const D3DTA_DIFFUSE: u32 = 0x0;
const D3DTA_CURRENT: u32 = 0x1;
const D3DTA_TEXTURE: u32 = 0x2;
const D3DTA_TFACTOR: u32 = 0x3;
const D3DTA_SPECULAR: u32 = 0x4;
const D3DTA_TEMP: u32 = 0x5;
const D3DTA_COMPLEMENT: u32 = 0x10;
const D3DTA_ALPHAREPLICATE: u32 = 0x20;
const D3DCULL_NONE: u32 = 1;
const D3DTTFF_DISABLE: u32 = 0;
const D3DTTFF_COUNT_MASK: u32 = 0xF;
const D3DTTFF_COUNT1: u32 = 1;
const D3DTTFF_COUNT2: u32 = 2;
const D3DTTFF_COUNT3: u32 = 3;
const D3DTTFF_COUNT4: u32 = 4;
const D3DTTFF_PROJECTED: u32 = 256;

/// Original W3D API Functions - Exact C++ Signatures

/// Create W3D device - matches original W3DDevice::Create()
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_Create() -> W3D_DEVICE {
    match create_w3d_device_internal() {
        Ok(device_ptr) => device_ptr,
        Err(_) => std::ptr::null_mut(),
    }
}

unsafe fn create_w3d_device_internal() -> Result<W3D_DEVICE> {
    create_w3d_device_with_config(W3DConfig::default())
}

unsafe fn create_w3d_device_with_config(config: W3DConfig) -> Result<W3D_DEVICE> {
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

/// Set render state - matches original W3DDevice::SetRenderState(state, value)
#[no_mangle]
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
    match device_ref
        .runtime
        .block_on(async { set_render_state_internal(&device_ref.device, state, value).await })
    {
        Ok(_) => 1,  // Success
        Err(_) => 0, // Failure
    }
}

async fn set_render_state_internal(
    device: &Arc<RwLock<W3DDevice>>,
    state: W3D_RENDER_STATE,
    value: u32,
) -> Result<()> {
    match state {
        W3D_RENDER_STATE::W3DRS_FOGENABLE
        | W3D_RENDER_STATE::W3DRS_FOGCOLOR
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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
#[no_mangle]
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

/// Draw indexed primitive - matches original W3DDevice::DrawIndexedPrimitive(type, vertices, indices)
#[no_mangle]
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

    match device_ref.runtime.block_on(async {
        draw_indexed_primitive_internal(
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
        )
        .await
    }) {
        Ok(_) => 1,  // Success
        Err(_) => 0, // Failure
    }
}

/// Draw indexed primitive from staged stream/index state using DX8-style arguments.
#[no_mangle]
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

    match device_ref.runtime.block_on(async {
        draw_indexed_primitive_internal(
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
        )
        .await
    }) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

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

/// Draw primitive from staged stream data (non-indexed path).
#[no_mangle]
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

    match device_ref.runtime.block_on(async {
        submit_transient_draw_internal(
            &device_ref.device,
            primitive_type,
            &vertices,
            &indices,
            &mesh_id,
            world_matrix,
            material_id,
            alpha_blend_enabled,
        )
        .await
    }) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Draw primitive UP - legacy immediate-mode compatibility entry point.
#[no_mangle]
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

    match device_ref.runtime.block_on(async {
        submit_transient_draw_internal(
            &device_ref.device,
            primitive_type,
            &vertices,
            &indices,
            &mesh_id,
            world_matrix,
            material_id,
            alpha_blend_enabled,
        )
        .await
    }) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Draw indexed primitive from immediate-mode UP buffers.
#[no_mangle]
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

    match device_ref.runtime.block_on(async {
        submit_transient_draw_internal(
            &device_ref.device,
            primitive_type,
            &vertices,
            &indices,
            &mesh_id,
            world_matrix,
            material_id,
            alpha_blend_enabled,
        )
        .await
    }) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

/// Begin scene - legacy W3D compatibility entry point.
#[no_mangle]
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

async fn draw_indexed_primitive_internal(
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
        unsafe { std::slice::from_raw_parts(vertex_buffer, vertex_count as usize).to_vec() };
    let draw_texture_stage = active_draw_texture_stage(device_ref);
    apply_stage_texture_transform(device_ref, draw_texture_stage, &mut vertices);
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
    .await
}

async fn submit_transient_draw_internal(
    device: &Arc<RwLock<W3DDevice>>,
    primitive_type: W3D_PRIMITIVE_TYPE,
    vertices: &[W3D_VERTEX],
    indices: &[u32],
    mesh_id: &str,
    world_matrix: W3D_MATRIX,
    material_id: Option<String>,
    alpha_blend_enabled: bool,
) -> Result<()> {
    let render_material_id = material_id.clone();
    // Convert W3D vertices to modern W3DVertex format.
    let modern_vertices: Vec<W3DVertex> = vertices.iter().map(w3d_vertex_to_modern).collect();
    // Create temporary mesh and render it
    let mesh_data = bytemuck::cast_slice(&modern_vertices).to_vec();
    let (local_min, local_max) = compute_vertex_bounds(&modern_vertices);
    let temp_mesh = Mesh {
        id: mesh_id.to_string(),
        name: format!("Temporary Draw Call {mesh_id}"),
        vertex_format: super::VertexFormat::PositionNormalUvColor,
        vertices: mesh_data,
        indices: indices.to_vec(),
        topology: match primitive_type {
            W3D_PRIMITIVE_TYPE::W3D_TRIANGLES => super::PrimitiveTopology::TriangleList,
            W3D_PRIMITIVE_TYPE::W3D_TRIANGLE_STRIP => super::PrimitiveTopology::TriangleStrip,
            W3D_PRIMITIVE_TYPE::W3D_TRIANGLE_FAN => super::PrimitiveTopology::TriangleFan,
            W3D_PRIMITIVE_TYPE::W3D_LINES => super::PrimitiveTopology::LineList,
            W3D_PRIMITIVE_TYPE::W3D_LINE_STRIP => super::PrimitiveTopology::LineStrip,
            W3D_PRIMITIVE_TYPE::W3D_POINTS => super::PrimitiveTopology::PointList,
        },
        material_id,
        bounding_box: super::BoundingBox::new(local_min, local_max),
    };

    let world_mat4 = Mat4::from(world_matrix);
    let (world_min, world_max) = transform_bounds(local_min, local_max, world_mat4);

    // Add mesh and transient render object to the scene for this present call.
    let device_lock = device.read().await;
    device_lock.add_mesh(temp_mesh).await?;
    let transparent_by_material = if let Some(material_id) = render_material_id.as_deref() {
        device_lock
            .get_material(material_id)
            .await
            .map(|material| material.properties.transparent)
            .unwrap_or(false)
    } else {
        false
    };
    let transparent = transparent_by_material || alpha_blend_enabled;
    let (material_params, priority) = if let Some(material_id) = render_material_id.as_deref() {
        if let Some(material) = device_lock.get_material(material_id).await {
            (
                batch_material_params(Some(&material)),
                batch_priority(Some(&material)),
            )
        } else {
            (batch_material_params(None), batch_priority(None))
        }
    } else {
        (batch_material_params(None), batch_priority(None))
    };
    let mut scene = device_lock.get_scene().await;
    scene.render_objects.push(RenderObject {
        mesh_id: mesh_id.to_string(),
        material_id: render_material_id,
        transform: world_matrix.m,
        world_bounds: super::BoundingBox::new(world_min, world_max),
        lod_bias: 0.0,
        cast_shadows: false,
        receive_shadows: false,
        visible: true,
        transparent,
        material_params,
        priority,
    });
    device_lock.set_scene(scene).await?;

    tracing::trace!(
        "Submitted transient primitive: {:?}, {} vertices, {} indices",
        primitive_type,
        vertices.len(),
        indices.len()
    );

    Ok(())
}

/// Load texture - matches original W3DDevice::LoadTexture(filename)
#[no_mangle]
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

/// Set material - legacy compatibility entry point.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_SetMaterial(
    device: W3D_DEVICE,
    material_data: *const W3DMaterialData,
) -> i32 {
    if device.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if material_data.is_null() {
        if let Ok(mut current_material) = device_ref.current_material_id.lock() {
            *current_material = None;
        }
        if let Ok(mut current_material_data) = device_ref.current_material_data.lock() {
            *current_material_data = None;
        }
        return 1;
    }
    if !is_valid_ptr(material_data) {
        return 0;
    }

    let material_data = *material_data;
    let material_id = next_material_id(device_ref);
    let material = c_material_data_to_material(&material_id, material_data);
    let material_id_for_state = material_id.clone();

    match device_ref
        .runtime
        .block_on(async { set_material_internal(&device_ref.device, material).await })
    {
        Ok(_) => {
            if let Ok(mut current_material) = device_ref.current_material_id.lock() {
                *current_material = Some(material_id_for_state);
            }
            if let Ok(mut current_material_data) = device_ref.current_material_data.lock() {
                *current_material_data = Some(material_data);
            }
            1
        }
        Err(_) => 0,
    }
}

/// Get currently bound material - legacy compatibility entry point.
#[no_mangle]
pub unsafe extern "C" fn W3DDevice_GetMaterial(
    device: W3D_DEVICE,
    out_material_data: *mut W3DMaterialData,
) -> i32 {
    if device.is_null() || out_material_data.is_null() {
        return 0;
    }

    let device_ref = &*device;
    if let Ok(current) = device_ref.current_material_data.lock() {
        if let Some(material_data) = *current {
            *out_material_data = material_data;
            return 1;
        }
    }

    let Some(material_id) = current_material_id(device_ref) else {
        return 0;
    };
    let material = device_ref
        .runtime
        .block_on(async { get_material_internal(&device_ref.device, &material_id).await });
    if let Some(material) = material {
        *out_material_data = material_to_c_data(&material);
        return 1;
    }

    0
}

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

/// Set viewport - legacy compatibility entry point.
#[no_mangle]
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

async fn load_texture_internal(
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

async fn set_texture_internal(device: &Arc<RwLock<W3DDevice>>, texture: Texture) -> Result<()> {
    let device_lock = device.read().await;
    device_lock.add_texture(texture).await?;
    Ok(())
}

async fn get_texture_internal(
    device: &Arc<RwLock<W3DDevice>>,
    texture_id: &str,
) -> Option<Texture> {
    let device_lock = device.read().await;
    device_lock.get_texture(texture_id).await
}

async fn get_material_internal(
    device: &Arc<RwLock<W3DDevice>>,
    material_id: &str,
) -> Option<Material> {
    let device_lock = device.read().await;
    device_lock.get_material(material_id).await
}

async fn set_material_internal(device: &Arc<RwLock<W3DDevice>>, material: Material) -> Result<()> {
    let device_lock = device.read().await;
    device_lock.add_material(material).await?;
    Ok(())
}

async fn ensure_bound_material_internal(
    device: &Arc<RwLock<W3DDevice>>,
    base_material_id: Option<&str>,
    texture_id: Option<&str>,
    detail_texture_id: Option<&str>,
    detail_blend_mode: u8,
    bound_material_id: &str,
    tint_rgba: [f32; 4],
    lighting_state: FixedFunctionLightingState,
    surface_state: FixedFunctionSurfaceState,
) -> Result<()> {
    let device_lock = device.read().await;
    if device_lock.get_material(bound_material_id).await.is_some() {
        return Ok(());
    }
    if let Some(texture_id) = texture_id {
        if device_lock.get_texture(texture_id).await.is_none() {
            return Err(W3DError::ResourceLoadingFailed(format!(
                "Texture not found for material binding: {texture_id}"
            )));
        }
    }

    let mut material = if let Some(base_material_id) = base_material_id {
        device_lock
            .get_material(base_material_id)
            .await
            .unwrap_or_else(|| default_material(bound_material_id))
    } else {
        default_material(bound_material_id)
    };

    material.id = bound_material_id.to_string();
    material.name = bound_material_id.to_string();
    material.diffuse_texture = texture_id.map(str::to_string);
    material.detail_texture = detail_texture_id.map(str::to_string);
    material.detail_blend_mode = detail_blend_mode;
    material.properties.diffuse_color = multiply_rgba(material.properties.diffuse_color, tint_rgba);
    material.properties.transparent =
        material.properties.transparent || material.properties.diffuse_color[3] < 0.999;
    apply_fixed_function_lighting_to_material(&mut material, texture_id.is_some(), lighting_state);
    apply_fixed_function_surface_to_material(&mut material, surface_state);

    device_lock.add_material(material).await?;
    Ok(())
}

async fn set_lights_internal(device: &Arc<RwLock<W3DDevice>>, lights: Vec<Light>) -> Result<()> {
    let device_lock = device.read().await;
    let mut scene = device_lock.get_scene().await;
    scene.lights = lights;
    device_lock.set_scene(scene).await?;
    Ok(())
}

async fn set_viewport_internal(
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

async fn set_transform_internal(
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

/// Present frame - matches original W3DDevice::Present()
#[no_mangle]
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

async fn present_internal(device: &Arc<RwLock<W3DDevice>>) -> Result<()> {
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

async fn clear_internal(
    device: &Arc<RwLock<W3DDevice>>,
    flags: u32,
    color: [f32; 4],
) -> Result<()> {
    const D3DCLEAR_TARGET: u32 = 0x1;

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
pub unsafe extern "C" fn W3DDevice_GetDeviceCaps(device: W3D_DEVICE) -> u32 {
    if device.is_null() {
        return 0;
    }

    // Return capability flags (hardware T&L, vertex shaders, etc.)
    0xFFFFFFFF // All capabilities supported
}

/// Get render state - matches original API
#[no_mangle]
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

/// Helper function to check if a pointer is valid
unsafe fn is_valid_ptr<T>(ptr: *const T) -> bool {
    !ptr.is_null() && (ptr as usize) > 0x1000 // Basic sanity check
}

unsafe fn intern_texture_handle(device: &W3DDeviceC, texture_handle: W3D_TEXTURE) -> W3D_TEXTURE {
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

fn current_world_transform(device: &W3DDeviceC) -> W3D_MATRIX {
    current_transform_value(device, W3D_TRANSFORM_STATE::W3DTS_WORLD)
}

fn is_alpha_blend_enabled(device: &W3DDeviceC) -> bool {
    if let Ok(states) = device.render_states.lock() {
        return alpha_blend_enabled_from_states(&states);
    }
    default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE) != 0
}

fn alpha_blend_enabled_from_states(states: &HashMap<W3D_RENDER_STATE, u32>) -> bool {
    states
        .get(&W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE)
        .copied()
        .unwrap_or_else(|| default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE))
        != 0
}

fn current_transform_value(device: &W3DDeviceC, state: W3D_TRANSFORM_STATE) -> W3D_MATRIX {
    if let Ok(states) = device.transform_states.lock() {
        states
            .get(&state)
            .copied()
            .unwrap_or_else(|| default_transform_state_value(state))
    } else {
        default_transform_state_value(state)
    }
}

fn next_transient_mesh_id(device: &W3DDeviceC) -> String {
    if let Ok(mut counter) = device.transient_mesh_counter.lock() {
        let slot = *counter % TEMP_MESH_RING_SIZE;
        *counter = counter.wrapping_add(1);
        format!("{TEMP_MESH_PREFIX}{slot}")
    } else {
        format!("{TEMP_MESH_PREFIX}fallback")
    }
}

fn next_material_id(device: &W3DDeviceC) -> String {
    if let Ok(mut counter) = device.material_counter.lock() {
        let id = format!("__w3d_c_api_material_{}", *counter);
        *counter = counter.wrapping_add(1);
        id
    } else {
        "__w3d_c_api_material_fallback".to_string()
    }
}

fn current_material_id(device: &W3DDeviceC) -> Option<String> {
    if let Ok(current) = device.current_material_id.lock() {
        current.clone()
    } else {
        None
    }
}

fn current_fvf(device: &W3DDeviceC) -> u32 {
    if let Ok(current) = device.current_fvf.lock() {
        *current
    } else {
        0
    }
}

fn current_vertex_declaration(device: &W3DDeviceC) -> u32 {
    if let Ok(current) = device.current_vertex_declaration.lock() {
        *current
    } else {
        0
    }
}

fn current_vertex_declaration_elements(device: &W3DDeviceC) -> Option<Vec<W3D_VERTEX_ELEMENT>> {
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

fn stage_stream_source(
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

fn staged_stream_available_count(stream_source: &StagedStreamSource) -> usize {
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

fn staged_stream_base_byte(stream_source: &StagedStreamSource) -> Option<usize> {
    if stream_source.vertex_offset_bytes > stream_source.data.len() {
        return None;
    }
    Some(stream_source.vertex_offset_bytes)
}

fn staged_stream_bytes_for_vertex_range<'a>(
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

fn staged_stream_vertices(
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

fn staged_stream_vertices_range(
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

fn overlay_stream_components(
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
struct DeclOverlayApplied {
    uv: bool,
    normal: bool,
    color: bool,
}

fn overlay_stream_components_from_declaration(
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

fn collect_vertices_from_declaration_streams(
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

fn stage_texcoord_usage_index(device: &W3DDeviceC, stage: u32) -> u8 {
    let raw = stage_texcoord_index_raw(device, stage);
    (raw & 0xFF) as u8
}

fn active_draw_texture_stage(device: &W3DDeviceC) -> u32 {
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

fn resolve_active_draw_texture_stage<F>(bound_stages: &[u32], mut stage_state_lookup: F) -> u32
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

fn stage_texcoord_index_raw(device: &W3DDeviceC, stage: u32) -> u32 {
    stage_texture_state_value(device, stage, D3DTSS_TEXCOORDINDEX)
}

fn texture_stage_enabled(device: &W3DDeviceC, stage: u32) -> bool {
    let color_op = stage_texture_state_value(device, stage, D3DTSS_COLOROP);
    let alpha_op = stage_texture_state_value(device, stage, D3DTSS_ALPHAOP);
    color_op != D3DTOP_DISABLE || alpha_op != D3DTOP_DISABLE
}

fn texture_stage_enabled_with<F>(stage_state_lookup: &mut F, stage: u32) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
    let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
    color_op != D3DTOP_DISABLE || alpha_op != D3DTOP_DISABLE
}

fn texture_stage_uses_texture_input_with<F>(stage_state_lookup: &mut F, stage: u32) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    texture_stage_uses_texture_color_input_with(stage_state_lookup, stage)
        || texture_stage_uses_texture_alpha_input_with(stage_state_lookup, stage)
}

fn texture_stage_uses_texture_color_input_with<F>(stage_state_lookup: &mut F, stage: u32) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
    let color_arg0 = stage_state_lookup(stage, D3DTSS_COLORARG0);
    let color_arg1 = stage_state_lookup(stage, D3DTSS_COLORARG1);
    let color_arg2 = stage_state_lookup(stage, D3DTSS_COLORARG2);
    op_uses_texture_arg(color_op, color_arg0, color_arg1, color_arg2)
}

fn texture_stage_uses_texture_alpha_input_with<F>(stage_state_lookup: &mut F, stage: u32) -> bool
where
    F: FnMut(u32, u32) -> u32,
{
    let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
    let alpha_arg0 = stage_state_lookup(stage, D3DTSS_ALPHAARG0);
    let alpha_arg1 = stage_state_lookup(stage, D3DTSS_ALPHAARG1);
    let alpha_arg2 = stage_state_lookup(stage, D3DTSS_ALPHAARG2);
    op_uses_texture_arg(alpha_op, alpha_arg0, alpha_arg1, alpha_arg2)
}

fn op_uses_texture_arg(op: u32, arg0: u32, arg1: u32, arg2: u32) -> bool {
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

fn op_uses_arg0(op: u32) -> bool {
    matches!(op, D3DTOP_MULTIPLYADD | D3DTOP_LERP)
}

fn op_uses_arg1(op: u32) -> bool {
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

fn op_uses_arg2(op: u32) -> bool {
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

fn arg_references_texture(arg: u32) -> bool {
    (arg & D3DTA_SELECTMASK) == D3DTA_TEXTURE
}

fn arg_references_tfactor(arg: u32) -> bool {
    (arg & D3DTA_SELECTMASK) == D3DTA_TFACTOR
}

fn arg_color_from_texture_factor(arg: u32, texture_factor: u32) -> [f32; 4] {
    let alpha = ((texture_factor >> 24) & 0xFF) as f32 / 255.0;
    let red = ((texture_factor >> 16) & 0xFF) as f32 / 255.0;
    let green = ((texture_factor >> 8) & 0xFF) as f32 / 255.0;
    let blue = (texture_factor & 0xFF) as f32 / 255.0;
    apply_arg_modifiers_to_color(arg, [red, green, blue, alpha])
}

fn apply_arg_modifiers_to_color(arg: u32, base_color: [f32; 4]) -> [f32; 4] {
    let alpha = base_color[3];
    let mut color = if (arg & D3DTA_ALPHAREPLICATE) != 0 {
        [alpha, alpha, alpha, alpha]
    } else {
        base_color
    };

    if (arg & D3DTA_COMPLEMENT) != 0 {
        for component in &mut color {
            *component = 1.0 - *component;
        }
    }

    color
}

fn multiply_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        lhs[0] * rhs[0],
        lhs[1] * rhs[1],
        lhs[2] * rhs[2],
        lhs[3] * rhs[3],
    ]
}

fn add_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        (lhs[0] + rhs[0]).clamp(0.0, 1.0),
        (lhs[1] + rhs[1]).clamp(0.0, 1.0),
        (lhs[2] + rhs[2]).clamp(0.0, 1.0),
        (lhs[3] + rhs[3]).clamp(0.0, 1.0),
    ]
}

fn subtract_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        (lhs[0] - rhs[0]).clamp(0.0, 1.0),
        (lhs[1] - rhs[1]).clamp(0.0, 1.0),
        (lhs[2] - rhs[2]).clamp(0.0, 1.0),
        (lhs[3] - rhs[3]).clamp(0.0, 1.0),
    ]
}

fn addsigned_rgba(lhs: [f32; 4], rhs: [f32; 4], scale: f32) -> [f32; 4] {
    [
        ((lhs[0] + rhs[0] - 0.5) * scale).clamp(0.0, 1.0),
        ((lhs[1] + rhs[1] - 0.5) * scale).clamp(0.0, 1.0),
        ((lhs[2] + rhs[2] - 0.5) * scale).clamp(0.0, 1.0),
        ((lhs[3] + rhs[3] - 0.5) * scale).clamp(0.0, 1.0),
    ]
}

fn addsmooth_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        (lhs[0] + rhs[0] * (1.0 - lhs[0])).clamp(0.0, 1.0),
        (lhs[1] + rhs[1] * (1.0 - lhs[1])).clamp(0.0, 1.0),
        (lhs[2] + rhs[2] * (1.0 - lhs[2])).clamp(0.0, 1.0),
        (lhs[3] + rhs[3] * (1.0 - lhs[3])).clamp(0.0, 1.0),
    ]
}

fn scale_rgb_add_rgba(base: [f32; 4], added: [f32; 4], factor: [f32; 4]) -> [f32; 4] {
    [
        (base[0] + added[0] * factor[0]).clamp(0.0, 1.0),
        (base[1] + added[1] * factor[1]).clamp(0.0, 1.0),
        (base[2] + added[2] * factor[2]).clamp(0.0, 1.0),
        base[3],
    ]
}

fn lerp_rgba(lhs: [f32; 4], rhs: [f32; 4], factor: f32) -> [f32; 4] {
    let t = factor.clamp(0.0, 1.0);
    [
        lhs[0] * t + rhs[0] * (1.0 - t),
        lhs[1] * t + rhs[1] * (1.0 - t),
        lhs[2] * t + rhs[2] * (1.0 - t),
        lhs[3] * t + rhs[3] * (1.0 - t),
    ]
}

fn lerp_rgba_per_channel(factor: [f32; 4], lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    [
        lhs[0] * factor[0].clamp(0.0, 1.0) + rhs[0] * (1.0 - factor[0].clamp(0.0, 1.0)),
        lhs[1] * factor[1].clamp(0.0, 1.0) + rhs[1] * (1.0 - factor[1].clamp(0.0, 1.0)),
        lhs[2] * factor[2].clamp(0.0, 1.0) + rhs[2] * (1.0 - factor[2].clamp(0.0, 1.0)),
        lhs[3] * factor[3].clamp(0.0, 1.0) + rhs[3] * (1.0 - factor[3].clamp(0.0, 1.0)),
    ]
}

fn dotproduct3_rgba(lhs: [f32; 4], rhs: [f32; 4]) -> [f32; 4] {
    let lx = lhs[0] * 2.0 - 1.0;
    let ly = lhs[1] * 2.0 - 1.0;
    let lz = lhs[2] * 2.0 - 1.0;
    let rx = rhs[0] * 2.0 - 1.0;
    let ry = rhs[1] * 2.0 - 1.0;
    let rz = rhs[2] * 2.0 - 1.0;
    let scalar = ((lx * rx + ly * ry + lz * rz) + 1.0) * 0.5;
    let clamped = scalar.clamp(0.0, 1.0);
    [clamped, clamped, clamped, clamped]
}

fn pack_rgba8(color: [f32; 4]) -> [u8; 4] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn render_state_value(device: &W3DDeviceC, state: W3D_RENDER_STATE) -> u32 {
    if let Ok(states) = device.render_states.lock() {
        return states
            .get(&state)
            .copied()
            .unwrap_or_else(|| default_render_state_value(state));
    }

    default_render_state_value(state)
}

fn current_fixed_function_lighting_state(device: &W3DDeviceC) -> FixedFunctionLightingState {
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

fn lighting_state_requires_material_variant(state: FixedFunctionLightingState) -> bool {
    state != default_fixed_function_lighting_state()
}

fn default_fixed_function_lighting_state() -> FixedFunctionLightingState {
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

fn current_fixed_function_surface_state(device: &W3DDeviceC) -> FixedFunctionSurfaceState {
    FixedFunctionSurfaceState {
        alpha_test_enabled: render_state_value(device, W3D_RENDER_STATE::W3DRS_ALPHATESTENABLE)
            != 0,
        alpha_ref: render_state_value(device, W3D_RENDER_STATE::W3DRS_ALPHAREF) as u8,
        alpha_blend_enabled: render_state_value(device, W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE)
            != 0,
        cull_mode: render_state_value(device, W3D_RENDER_STATE::W3DRS_CULLMODE),
    }
}

fn default_fixed_function_surface_state() -> FixedFunctionSurfaceState {
    FixedFunctionSurfaceState {
        alpha_test_enabled: default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHATESTENABLE)
            != 0,
        alpha_ref: default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHAREF) as u8,
        alpha_blend_enabled: default_render_state_value(W3D_RENDER_STATE::W3DRS_ALPHABLENDENABLE)
            != 0,
        cull_mode: default_render_state_value(W3D_RENDER_STATE::W3DRS_CULLMODE),
    }
}

fn surface_state_requires_material_variant(state: FixedFunctionSurfaceState) -> bool {
    state != default_fixed_function_surface_state()
}

fn material_source_uses_material(source: u32, color_vertex: bool) -> bool {
    if !color_vertex {
        return true;
    }
    source == D3DMCS_MATERIAL
}

fn multiply_rgb(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [lhs[0] * rhs[0], lhs[1] * rhs[1], lhs[2] * rhs[2]]
}

fn add_rgb(lhs: [f32; 3], rhs: [f32; 3]) -> [f32; 3] {
    [
        (lhs[0] + rhs[0]).clamp(0.0, 1.0),
        (lhs[1] + rhs[1]).clamp(0.0, 1.0),
        (lhs[2] + rhs[2]).clamp(0.0, 1.0),
    ]
}

fn apply_fixed_function_lighting_to_material(
    material: &mut Material,
    has_texture: bool,
    state: FixedFunctionLightingState,
) {
    material.properties.unlit = !state.lighting_enabled;

    let diffuse_rgb = [
        material.properties.diffuse_color[0],
        material.properties.diffuse_color[1],
        material.properties.diffuse_color[2],
    ];

    if !state.specular_enabled
        || !material_source_uses_material(state.specular_material_source, state.color_vertex)
    {
        material.properties.specular_color = [0.0, 0.0, 0.0];
        material.properties.shininess = 0.0;
    }

    if !material_source_uses_material(state.emissive_material_source, state.color_vertex) {
        material.properties.emissive_color = [0.0, 0.0, 0.0];
    }

    if state.ambient_argb != 0
        && material_source_uses_material(state.ambient_material_source, state.color_vertex)
    {
        let ambient = decode_argb_color(state.ambient_argb);
        material.properties.emissive_color = add_rgb(
            material.properties.emissive_color,
            multiply_rgb(diffuse_rgb, [ambient[0], ambient[1], ambient[2]]),
        );
    }

    if !state.lighting_enabled {
        material.properties.specular_color = [0.0, 0.0, 0.0];
        material.properties.shininess = 0.0;

        if !has_texture {
            material.properties.emissive_color =
                add_rgb(material.properties.emissive_color, diffuse_rgb);
        }
    }
}

fn apply_fixed_function_surface_to_material(
    material: &mut Material,
    state: FixedFunctionSurfaceState,
) {
    material.properties.alpha_test = state.alpha_test_enabled;
    material.properties.alpha_cutoff = if state.alpha_test_enabled {
        state.alpha_ref as f32 / 255.0
    } else {
        0.0
    };
    material.properties.double_sided = state.cull_mode == D3DCULL_NONE;
    material.properties.transparent = material.properties.transparent || state.alpha_blend_enabled;
}

fn simple_stage_tfactor_tint_with<F>(
    stage_state_lookup: &mut F,
    stage: u32,
    texture_factor: u32,
) -> Option<[f32; 4]>
where
    F: FnMut(u32, u32) -> u32,
{
    simple_stage_tint_from_current_with(
        stage_state_lookup,
        stage,
        [1.0, 1.0, 1.0, 1.0],
        texture_factor,
    )
}

fn simple_stage_chain_tint_with<F>(
    stage_state_lookup: &mut F,
    last_stage: u32,
    texture_factor: u32,
) -> Option<[f32; 4]>
where
    F: FnMut(u32, u32) -> u32,
{
    let mut current = [1.0, 1.0, 1.0, 1.0];
    let mut used = false;

    for stage in 0..=last_stage {
        if !texture_stage_enabled_with(stage_state_lookup, stage) {
            continue;
        }

        if let Some(next) =
            simple_stage_tint_from_current_with(stage_state_lookup, stage, current, texture_factor)
        {
            current = next;
            used = true;
        }
    }

    used.then_some(current)
}

fn simple_stage_tint_from_current_with<F>(
    stage_state_lookup: &mut F,
    stage: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<[f32; 4]>
where
    F: FnMut(u32, u32) -> u32,
{
    let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
    let color_arg0 = stage_state_lookup(stage, D3DTSS_COLORARG0);
    let color_arg1 = stage_state_lookup(stage, D3DTSS_COLORARG1);
    let color_arg2 = stage_state_lookup(stage, D3DTSS_COLORARG2);
    let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
    let alpha_arg0 = stage_state_lookup(stage, D3DTSS_ALPHAARG0);
    let alpha_arg1 = stage_state_lookup(stage, D3DTSS_ALPHAARG1);
    let alpha_arg2 = stage_state_lookup(stage, D3DTSS_ALPHAARG2);

    let mut out = current_tint;
    let mut used = false;

    if color_op != D3DTOP_DISABLE {
        if let Some(color_tint) = simple_material_tint_color_for_op(
            color_op,
            color_arg0,
            color_arg1,
            color_arg2,
            current_tint,
            texture_factor,
        ) {
            out[0] = color_tint[0];
            out[1] = color_tint[1];
            out[2] = color_tint[2];
            used = true;
        }
    }

    if alpha_op != D3DTOP_DISABLE {
        if let Some(alpha_tint) = simple_material_tint_alpha_for_op(
            alpha_op,
            alpha_arg0,
            alpha_arg1,
            alpha_arg2,
            current_tint,
            texture_factor,
        ) {
            out[3] = alpha_tint;
            used = true;
        }
    }

    used.then_some(out)
}

fn simple_material_tint_arg_value(
    arg: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<[f32; 4]> {
    match arg & D3DTA_SELECTMASK {
        D3DTA_TFACTOR => Some(arg_color_from_texture_factor(arg, texture_factor)),
        D3DTA_CURRENT => Some(apply_arg_modifiers_to_color(arg, current_tint)),
        D3DTA_TEXTURE | D3DTA_DIFFUSE => {
            Some(apply_arg_modifiers_to_color(arg, [1.0, 1.0, 1.0, 1.0]))
        }
        _ => None,
    }
}

fn simple_material_tint_color_for_op(
    op: u32,
    arg0: u32,
    arg1: u32,
    arg2: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<[f32; 4]> {
    match op {
        D3DTOP_PREMODULATE | D3DTOP_BUMPENVMAP | D3DTOP_BUMPENVMAPLUMINANCE => Some(current_tint),
        D3DTOP_SELECTARG1 => simple_material_tint_arg_value(arg1, current_tint, texture_factor),
        D3DTOP_SELECTARG2 => simple_material_tint_arg_value(arg2, current_tint, texture_factor),
        D3DTOP_MODULATE | D3DTOP_MODULATE2X | D3DTOP_MODULATE4X => {
            let mut tint = multiply_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            );

            let scale = match op {
                D3DTOP_MODULATE2X => 2.0,
                D3DTOP_MODULATE4X => 4.0,
                _ => 1.0,
            };
            Some([
                (tint[0] * scale).clamp(0.0, 1.0),
                (tint[1] * scale).clamp(0.0, 1.0),
                (tint[2] * scale).clamp(0.0, 1.0),
                tint[3],
            ])
        }
        D3DTOP_ADD => Some(add_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_ADDSIGNED => Some(addsigned_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            1.0,
        )),
        D3DTOP_ADDSIGNED2X => Some(addsigned_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            2.0,
        )),
        D3DTOP_SUBTRACT => Some(subtract_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_ADDSMOOTH => Some(addsmooth_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_BLENDDIFFUSEALPHA | D3DTOP_BLENDTEXTUREALPHA | D3DTOP_BLENDTEXTUREALPHAPM => {
            // The constrained fallback evaluator treats diffuse/texture as neutral white sources,
            // so their alpha factor resolves to 1.0 in this approximation and these ops collapse to arg1.
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)
        }
        D3DTOP_MODULATEALPHA_ADDCOLOR => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            Some(scale_rgb_add_rgba(lhs, rhs, [lhs[3], lhs[3], lhs[3], 1.0]))
        }
        D3DTOP_MODULATECOLOR_ADDALPHA => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            Some([
                (lhs[0] * rhs[0] + lhs[3]).clamp(0.0, 1.0),
                (lhs[1] * rhs[1] + lhs[3]).clamp(0.0, 1.0),
                (lhs[2] * rhs[2] + lhs[3]).clamp(0.0, 1.0),
                lhs[3],
            ])
        }
        D3DTOP_MODULATEINVALPHA_ADDCOLOR => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            let inv_alpha = 1.0 - lhs[3];
            Some(scale_rgb_add_rgba(
                lhs,
                rhs,
                [inv_alpha, inv_alpha, inv_alpha, 1.0],
            ))
        }
        D3DTOP_MODULATEINVCOLOR_ADDALPHA => {
            let lhs = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?;
            let rhs = simple_material_tint_arg_value(arg2, current_tint, texture_factor)?;
            Some([
                ((1.0 - lhs[0]) * rhs[0] + lhs[3]).clamp(0.0, 1.0),
                ((1.0 - lhs[1]) * rhs[1] + lhs[3]).clamp(0.0, 1.0),
                ((1.0 - lhs[2]) * rhs[2] + lhs[3]).clamp(0.0, 1.0),
                lhs[3],
            ])
        }
        D3DTOP_BLENDFACTORALPHA => Some(lerp_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            arg_color_from_texture_factor(D3DTA_TFACTOR, texture_factor)[3],
        )),
        D3DTOP_BLENDCURRENTALPHA => Some(lerp_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            current_tint[3],
        )),
        D3DTOP_MULTIPLYADD => Some(add_rgba(
            multiply_rgba(
                simple_material_tint_arg_value(arg0, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            ),
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_LERP => Some(lerp_rgba_per_channel(
            simple_material_tint_arg_value(arg0, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        D3DTOP_DOTPRODUCT3 => Some(dotproduct3_rgba(
            simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
            simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
        )),
        _ => None,
    }
}

fn simple_material_tint_alpha_for_op(
    op: u32,
    arg0: u32,
    arg1: u32,
    arg2: u32,
    current_tint: [f32; 4],
    texture_factor: u32,
) -> Option<f32> {
    match op {
        D3DTOP_PREMODULATE | D3DTOP_BUMPENVMAP | D3DTOP_BUMPENVMAPLUMINANCE => {
            Some(current_tint[3])
        }
        D3DTOP_SELECTARG1 => {
            Some(simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3])
        }
        D3DTOP_SELECTARG2 => {
            Some(simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
        }
        D3DTOP_MODULATE | D3DTOP_MODULATE2X | D3DTOP_MODULATE4X => {
            let mut tint = simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                * simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3];

            let scale = match op {
                D3DTOP_MODULATE2X => 2.0,
                D3DTOP_MODULATE4X => 4.0,
                _ => 1.0,
            };
            Some((tint * scale).clamp(0.0, 1.0))
        }
        D3DTOP_ADD => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
                .clamp(0.0, 1.0),
        ),
        D3DTOP_ADDSIGNED => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3]
                - 0.5)
                .clamp(0.0, 1.0),
        ),
        D3DTOP_ADDSIGNED2X => Some(
            ((simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3]
                - 0.5)
                * 2.0)
                .clamp(0.0, 1.0),
        ),
        D3DTOP_SUBTRACT => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                - simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
                .clamp(0.0, 1.0),
        ),
        D3DTOP_ADDSMOOTH => Some(
            (simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3]
                    * (1.0
                        - simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]))
                .clamp(0.0, 1.0),
        ),
        D3DTOP_BLENDDIFFUSEALPHA | D3DTOP_BLENDTEXTUREALPHA | D3DTOP_BLENDTEXTUREALPHAPM => {
            Some(simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3])
        }
        D3DTOP_MODULATEALPHA_ADDCOLOR
        | D3DTOP_MODULATECOLOR_ADDALPHA
        | D3DTOP_MODULATEINVALPHA_ADDCOLOR
        | D3DTOP_MODULATEINVCOLOR_ADDALPHA => {
            Some(simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3])
        }
        D3DTOP_BLENDFACTORALPHA => Some(
            lerp_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
                arg_color_from_texture_factor(D3DTA_TFACTOR, texture_factor)[3],
            )[3],
        ),
        D3DTOP_BLENDCURRENTALPHA => Some(
            lerp_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
                current_tint[3],
            )[3],
        ),
        D3DTOP_MULTIPLYADD => Some(
            (simple_material_tint_arg_value(arg0, current_tint, texture_factor)?[3]
                * simple_material_tint_arg_value(arg1, current_tint, texture_factor)?[3]
                + simple_material_tint_arg_value(arg2, current_tint, texture_factor)?[3])
                .clamp(0.0, 1.0),
        ),
        D3DTOP_LERP => Some(
            lerp_rgba_per_channel(
                simple_material_tint_arg_value(arg0, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            )[3],
        ),
        D3DTOP_DOTPRODUCT3 => Some(
            dotproduct3_rgba(
                simple_material_tint_arg_value(arg1, current_tint, texture_factor)?,
                simple_material_tint_arg_value(arg2, current_tint, texture_factor)?,
            )[3],
        ),
        _ => None,
    }
}

fn default_texture_stage_state(stage: u32, state: u32) -> u32 {
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

fn stage_texture_state_value(device: &W3DDeviceC, stage: u32, state: u32) -> u32 {
    let Ok(stage_states) = device.texture_stage_states.lock() else {
        return default_texture_stage_state(stage, state);
    };
    stage_states
        .get(&(stage, state))
        .copied()
        .unwrap_or_else(|| default_texture_stage_state(stage, state))
}

fn texture_transform_state(stage: u32) -> Option<W3D_TRANSFORM_STATE> {
    match stage {
        0 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE0),
        1 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE1),
        2 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE2),
        3 => Some(W3D_TRANSFORM_STATE::W3DTS_TEXTURE3),
        _ => None,
    }
}

fn stage_texture_transform_flags(device: &W3DDeviceC, stage: u32) -> u32 {
    stage_texture_state_value(device, stage, D3DTSS_TEXTURETRANSFORMFLAGS)
}

fn current_texture_transform_matrix(device: &W3DDeviceC, stage: u32) -> Option<W3D_MATRIX> {
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

fn apply_stage_texture_transform(device: &W3DDeviceC, stage: u32, vertices: &mut [W3D_VERTEX]) {
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

fn apply_generated_stage_texcoords(
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

fn texture_transform_input_for_vertex(
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

fn mul_row_vec4_matrix(v: [f32; 4], m: &W3D_MATRIX) -> [f32; 4] {
    [
        v[0] * m.m[0][0] + v[1] * m.m[1][0] + v[2] * m.m[2][0] + v[3] * m.m[3][0],
        v[0] * m.m[0][1] + v[1] * m.m[1][1] + v[2] * m.m[2][1] + v[3] * m.m[3][1],
        v[0] * m.m[0][2] + v[1] * m.m[1][2] + v[2] * m.m[2][2] + v[3] * m.m[3][2],
        v[0] * m.m[0][3] + v[1] * m.m[1][3] + v[2] * m.m[2][3] + v[3] * m.m[3][3],
    ]
}

fn apply_stream_uv_overlay(
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

fn declaration_element_for_usage(
    elements: &[W3D_VERTEX_ELEMENT],
    usage: u8,
    usage_index: u8,
) -> Option<W3D_VERTEX_ELEMENT> {
    elements
        .iter()
        .copied()
        .find(|element| element.usage == usage && element.usage_index == usage_index)
}

fn apply_declared_uv(
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

fn apply_declared_normal(
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

fn apply_declared_color(
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

fn stream_vertex_bytes(source: &StagedStreamSource, vertex_index: usize) -> Option<&[u8]> {
    let base_offset = staged_stream_base_byte(source)?;
    let stream_offset = vertex_index.checked_mul(source.vertex_stride)?;
    let base = base_offset.checked_add(stream_offset)?;
    let end = base.checked_add(source.vertex_stride)?;
    if end > source.data.len() {
        return None;
    }
    Some(&source.data[base..end])
}

fn read_uv_from_decl(bytes: &[u8], offset: usize, decl_type: u8) -> Option<(f32, f32)> {
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

fn read_position_from_decl(bytes: &[u8], offset: usize, decl_type: u8) -> Option<(f32, f32, f32)> {
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

fn read_normal_from_decl(bytes: &[u8], offset: usize, decl_type: u8) -> Option<(f32, f32, f32)> {
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

fn read_color_from_decl(bytes: &[u8], offset: usize, decl_type: u8) -> Option<u32> {
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

fn normalize_i16(value: i16) -> f32 {
    (value as f32 / 32767.0).clamp(-1.0, 1.0)
}

fn normalize_u16(value: u16) -> f32 {
    value as f32 / 65535.0
}

fn normalize_u8(value: u8) -> f32 {
    value as f32 / 255.0
}

fn unpack_udec3(packed: u32) -> (f32, f32, f32) {
    let x = (packed & 0x3FF) as f32;
    let y = ((packed >> 10) & 0x3FF) as f32;
    let z = ((packed >> 20) & 0x3FF) as f32;
    (x, y, z)
}

fn unpack_dec3n(packed: u32) -> (f32, f32, f32) {
    let sx = sign_extend_10((packed & 0x3FF) as i32);
    let sy = sign_extend_10(((packed >> 10) & 0x3FF) as i32);
    let sz = sign_extend_10(((packed >> 20) & 0x3FF) as i32);
    (
        (sx as f32 / 511.0).clamp(-1.0, 1.0),
        (sy as f32 / 511.0).clamp(-1.0, 1.0),
        (sz as f32 / 511.0).clamp(-1.0, 1.0),
    )
}

fn sign_extend_10(value: i32) -> i32 {
    if (value & 0x200) != 0 {
        value | !0x3FF
    } else {
        value
    }
}

fn pack_color_f32(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let to_u8 = |value: f32| -> u8 { (value.clamp(0.0, 1.0) * 255.0).round() as u8 };
    pack_argb(to_u8(a), to_u8(r), to_u8(g), to_u8(b))
}

fn pack_argb(a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

fn fvf_has_normal(fvf: u32) -> bool {
    (fvf & D3DFVF_NORMAL) != 0
}

fn fvf_has_diffuse(fvf: u32) -> bool {
    (fvf & D3DFVF_DIFFUSE) != 0
}

fn fvf_tex_count(fvf: u32) -> usize {
    ((fvf & D3DFVF_TEXCOUNT_MASK) >> D3DFVF_TEXCOUNT_SHIFT) as usize
}

fn fvf_texcoord_dimension(fvf: u32, texcoord_set_index: usize) -> usize {
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

fn staged_index_buffer(device: &W3DDeviceC, requested_count: u32) -> Option<(Vec<u16>, i32)> {
    let requested = requested_count as usize;
    let (indices, base_vertex_index) = staged_index_buffer_range(device, 0, requested)?;
    Some((indices, base_vertex_index))
}

fn staged_index_buffer_range(
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

fn resolve_draw_material_id(device: &W3DDeviceC, texture_stage: u32) -> Option<String> {
    let base_material_id = current_material_id(device);
    let active_texture_id = if let Ok(bindings) = device.bound_textures.lock() {
        bindings.get(&texture_stage).cloned()
    } else {
        None
    };
    let lighting_state = current_fixed_function_lighting_state(device);
    let surface_state = current_fixed_function_surface_state(device);
    let combiner_signature = material_combiner_signature_with(
        &mut |stage, state| stage_texture_state_value(device, stage, state),
        8,
    );
    let multi_texture_chain = combiner_signature.sampling_stage_count > 1;

    // Resolve detail (Stage 1) texture and blend mode for multi-texture chains.
    let (detail_texture_id, detail_blend_mode) = if multi_texture_chain {
        let detail_id = resolve_detail_texture_id(device);
        let stage1_color_op = stage_texture_state_value(device, 1, D3DTSS_COLOROP);
        let blend = detail_blend_mode_from_color_op(stage1_color_op);
        (detail_id, blend)
    } else {
        (None, 0)
    };

    let texture_factor = render_state_value(device, W3D_RENDER_STATE::W3DRS_TEXTUREFACTOR);
    let tint_rgba = if active_texture_id.is_some() {
        if multi_texture_chain {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            simple_stage_chain_tint_with(
                &mut |stage, state| stage_texture_state_value(device, stage, state),
                texture_stage,
                texture_factor,
            )
            .unwrap_or([1.0, 1.0, 1.0, 1.0])
        }
    } else if let Some(stage) = first_enabled_texture_stage_with(
        &mut |stage, state| stage_texture_state_value(device, stage, state),
        8,
    ) {
        if multi_texture_chain {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            match simple_stage_chain_tint_with(
                &mut |lookup_stage, state| stage_texture_state_value(device, lookup_stage, state),
                stage,
                texture_factor,
            ) {
                Some(tint_rgba) => tint_rgba,
                None => {
                    if lighting_state_requires_material_variant(lighting_state)
                        || surface_state_requires_material_variant(surface_state)
                    {
                        [1.0, 1.0, 1.0, 1.0]
                    } else {
                        return base_material_id;
                    }
                }
            }
        }
    } else {
        if lighting_state_requires_material_variant(lighting_state)
            || surface_state_requires_material_variant(surface_state)
        {
            [1.0, 1.0, 1.0, 1.0]
        } else {
            return base_material_id;
        }
    };

    let effective_texture_id = effective_bound_texture_id(
        base_material_id.is_some(),
        multi_texture_chain,
        active_texture_id.clone(),
    );
    let texture_cache_id = effective_texture_id.clone().unwrap_or_default();
    let detail_cache_id = detail_texture_id.clone().unwrap_or_default();
    let cache_key = MaterialBindingCacheKey {
        base_material_id: base_material_id.clone(),
        texture_id: texture_cache_id.clone(),
        tint_rgba: pack_rgba8(tint_rgba),
        combiner_signature,
        lighting_state,
        surface_state,
    };
    if let Ok(material_bindings) = device.material_texture_bindings.lock() {
        if let Some(bound_material_id) = material_bindings.get(&cache_key) {
            return Some(bound_material_id.clone());
        }
    }

    let bound_material_id = material_binding_id(
        base_material_id.as_deref(),
        &texture_cache_id,
        &detail_cache_id,
        cache_key.tint_rgba,
        combiner_signature,
        lighting_state,
        surface_state,
    );
    if device
        .runtime
        .block_on(async {
            ensure_bound_material_internal(
                &device.device,
                base_material_id.as_deref(),
                effective_texture_id.as_deref(),
                detail_texture_id.as_deref(),
                detail_blend_mode,
                &bound_material_id,
                tint_rgba,
                lighting_state,
                surface_state,
            )
            .await
        })
        .is_err()
    {
        return base_material_id;
    }

    if let Ok(mut material_bindings) = device.material_texture_bindings.lock() {
        material_bindings.insert(cache_key, bound_material_id.clone());
    }
    Some(bound_material_id)
}

fn effective_bound_texture_id(
    _has_base_material: bool,
    _multi_texture_chain: bool,
    active_texture_id: Option<String>,
) -> Option<String> {
    // Always return the primary (Stage 0) texture ID.
    // The secondary (Stage 1) texture is resolved separately for multi-texture chains.
    active_texture_id
}

/// Resolve the Stage 1 (detail) texture bound to a multi-texture chain.
fn resolve_detail_texture_id(device: &W3DDeviceC) -> Option<String> {
    if let Ok(bindings) = device.bound_textures.lock() {
        bindings.get(&1u32).cloned()
    } else {
        None
    }
}

/// Map a D3DTOP color operation to our detail blend mode enum.
/// Returns: 0=off, 1=MODULATE, 2=ADDSIGNED, 3=BLENDCURRENTALPHA.
fn detail_blend_mode_from_color_op(color_op: u32) -> u8 {
    match color_op {
        D3DTOP_MODULATE | D3DTOP_MODULATE2X | D3DTOP_MODULATE4X => 1,
        D3DTOP_ADDSIGNED | D3DTOP_ADDSIGNED2X => 2,
        D3DTOP_BLENDCURRENTALPHA => 3,
        D3DTOP_ADD | D3DTOP_ADDSMOOTH => 2, // Approximate ADD as ADDSIGNED
        _ => 1,                             // Default to MODULATE for unknown ops
    }
}

fn material_binding_id(
    base_material_id: Option<&str>,
    texture_id: &str,
    detail_texture_id: &str,
    tint_rgba: [u8; 4],
    combiner_signature: MaterialCombinerSignature,
    lighting_state: FixedFunctionLightingState,
    surface_state: FixedFunctionSurfaceState,
) -> String {
    let mut hasher = DefaultHasher::new();
    base_material_id.hash(&mut hasher);
    texture_id.hash(&mut hasher);
    detail_texture_id.hash(&mut hasher);
    tint_rgba.hash(&mut hasher);
    combiner_signature.hash(&mut hasher);
    lighting_state.hash(&mut hasher);
    surface_state.hash(&mut hasher);
    format!("__w3d_c_api_bound_material_{:016x}", hasher.finish())
}

fn material_combiner_signature_with<F>(
    stage_state_lookup: &mut F,
    max_stages: u32,
) -> MaterialCombinerSignature
where
    F: FnMut(u32, u32) -> u32,
{
    let mut sampling_stage_count = 0u8;
    let mut force_multiply_like = false;

    for stage in 0..max_stages {
        if !texture_stage_enabled_with(stage_state_lookup, stage) {
            continue;
        }

        if !texture_stage_uses_texture_input_with(stage_state_lookup, stage) {
            continue;
        }

        sampling_stage_count = sampling_stage_count.saturating_add(1);

        let color_op = stage_state_lookup(stage, D3DTSS_COLOROP);
        let alpha_op = stage_state_lookup(stage, D3DTSS_ALPHAOP);
        if combiner_op_is_force_multiply_like(color_op)
            || combiner_op_is_force_multiply_like(alpha_op)
        {
            force_multiply_like = true;
        }
    }

    MaterialCombinerSignature {
        sampling_stage_count,
        force_multiply_like,
    }
}

fn combiner_op_is_force_multiply_like(op: u32) -> bool {
    matches!(
        op,
        D3DTOP_MODULATE
            | D3DTOP_MODULATE2X
            | D3DTOP_MODULATE4X
            | D3DTOP_MULTIPLYADD
            | D3DTOP_MODULATEALPHA_ADDCOLOR
            | D3DTOP_MODULATECOLOR_ADDALPHA
            | D3DTOP_MODULATEINVALPHA_ADDCOLOR
            | D3DTOP_MODULATEINVCOLOR_ADDALPHA
            | D3DTOP_PREMODULATE
            | D3DTOP_BUMPENVMAP
            | D3DTOP_BUMPENVMAPLUMINANCE
    )
}

fn first_enabled_texture_stage_with<F>(stage_state_lookup: &mut F, max_stages: u32) -> Option<u32>
where
    F: FnMut(u32, u32) -> u32,
{
    (0..max_stages).find(|stage| texture_stage_enabled_with(stage_state_lookup, *stage))
}

fn enabled_texture_sampling_stage_count_with<F>(
    stage_state_lookup: &mut F,
    max_stages: u32,
) -> usize
where
    F: FnMut(u32, u32) -> u32,
{
    (0..max_stages)
        .filter(|stage| {
            texture_stage_enabled_with(stage_state_lookup, *stage)
                && texture_stage_uses_texture_input_with(stage_state_lookup, *stage)
        })
        .count()
}

fn current_scene_lights(device: &W3DDeviceC) -> Vec<Light> {
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

fn c_material_data_to_material(id: &str, data: W3DMaterialData) -> Material {
    Material {
        id: id.to_string(),
        name: id.to_string(),
        shader_id: "default".to_string(),
        diffuse_texture: None,
        normal_texture: None,
        specular_texture: None,
        emissive_texture: None,
        detail_texture: None,
        detail_blend_mode: 0,
        properties: super::MaterialProperties {
            diffuse_color: data.albedo,
            specular_color: [data.metallic.clamp(0.0, 1.0); 3],
            emissive_color: [
                data.emission[0].max(0.0),
                data.emission[1].max(0.0),
                data.emission[2].max(0.0),
            ],
            shininess: (1.0 - data.roughness.clamp(0.0, 1.0)) * 128.0,
            alpha_cutoff: 0.5,
            alpha_test: false,
            transparent: data.albedo[3] < 0.999,
            double_sided: false,
            unlit: false,
        },
    }
}

fn material_to_c_data(material: &Material) -> W3DMaterialData {
    let metallic = material
        .properties
        .specular_color
        .iter()
        .copied()
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0);
    let roughness = (1.0 - (material.properties.shininess / 128.0)).clamp(0.0, 1.0);
    W3DMaterialData {
        albedo: material.properties.diffuse_color,
        metallic,
        roughness,
        emission: material.properties.emissive_color,
    }
}

fn default_material(id: &str) -> Material {
    Material {
        id: id.to_string(),
        name: id.to_string(),
        shader_id: "default".to_string(),
        diffuse_texture: None,
        normal_texture: None,
        specular_texture: None,
        emissive_texture: None,
        detail_texture: None,
        detail_blend_mode: 0,
        properties: super::MaterialProperties {
            diffuse_color: [1.0, 1.0, 1.0, 1.0],
            specular_color: [0.0, 0.0, 0.0],
            emissive_color: [0.0, 0.0, 0.0],
            shininess: 1.0,
            alpha_cutoff: 0.0,
            alpha_test: false,
            transparent: false,
            double_sided: false,
            unlit: true,
        },
    }
}

fn c_light_data_to_light(index: u32, data: W3DLightData) -> Light {
    let light_type = match data.light_type {
        0 => super::LightType::Directional,
        1 => super::LightType::Point,
        2 => super::LightType::Spot,
        3 => super::LightType::Area,
        _ => super::LightType::Directional,
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
        spot_params: if light_type == super::LightType::Spot {
            Some([0.9, 0.75])
        } else {
            None
        },
    }
}

fn light_to_c_data(light: &Light) -> W3DLightData {
    let light_type = match light.light_type {
        super::LightType::Directional => 0,
        super::LightType::Point => 1,
        super::LightType::Spot => 2,
        super::LightType::Area => 3,
    };
    W3DLightData {
        position: light.position,
        direction: light.direction,
        color: light.color,
        intensity: light.intensity,
        light_type,
    }
}

fn primitive_vertex_count(primitive_type: W3D_PRIMITIVE_TYPE, primitive_count: u32) -> Option<u32> {
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

fn primitive_index_count(primitive_type: W3D_PRIMITIVE_TYPE, primitive_count: u32) -> Option<u32> {
    primitive_vertex_count(primitive_type, primitive_count)
}

fn collect_up_vertices(
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

fn collect_up_indices(
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

fn collect_vertices_from_bytes(
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

fn decode_fvf_vertex(
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            (material.properties.emissive_color[0] - (0.5 * (0x80 as f32 / 255.0))).abs() < 1e-6
        );
        assert!(
            (material.properties.emissive_color[1] - (0.25 * (0x40 as f32 / 255.0))).abs() < 1e-6
        );
        assert!(
            (material.properties.emissive_color[2] - (0.75 * (0x20 as f32 / 255.0))).abs() < 1e-6
        );
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

        apply_generated_stage_texcoords(
            &mut vertices,
            D3DTSS_TCI_CAMERASPACEPOSITION,
            &world,
            &view,
        );

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
        let (nx, ny, nz) =
            read_normal_from_decl(&bytes, 0, D3DDECLTYPE_DEC3N).expect("dec3n normal");
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
}

fn read_f32(data: &[u8], offset: &mut usize) -> Option<f32> {
    let end = (*offset).checked_add(4)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(f32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_f32_at(data: &[u8], offset: usize) -> Option<f32> {
    let end = offset.checked_add(4)?;
    let bytes = data.get(offset..end)?;
    Some(f32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u32(data: &[u8], offset: &mut usize) -> Option<u32> {
    let end = (*offset).checked_add(4)?;
    let bytes = data.get(*offset..end)?;
    *offset = end;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u32_at(data: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes = data.get(offset..end)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn read_i16_at(data: &[u8], offset: usize) -> Option<i16> {
    let end = offset.checked_add(2)?;
    let bytes = data.get(offset..end)?;
    Some(i16::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u16_at(data: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let bytes = data.get(offset..end)?;
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u8_at(data: &[u8], offset: usize) -> Option<u8> {
    data.get(offset).copied()
}

fn w3d_vertex_to_modern(v: &W3D_VERTEX) -> W3DVertex {
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

fn load_texture_from_disk(filename: &str) -> Result<Texture> {
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
        format: super::TextureFormat::Rgba8,
        texture_type: super::TextureType::Texture2D,
        data: rgba.into_raw(),
    })
}

fn decode_texture_from_bytes(
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

fn resolve_texture_path(filename: &str) -> Option<PathBuf> {
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

fn checkerboard_fallback_texture(filename: &str, width: u32, height: u32) -> Texture {
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
        format: super::TextureFormat::Rgba8,
        texture_type: super::TextureType::Texture2D,
        data,
    }
}

fn decode_argb_color(argb: u32) -> [f32; 4] {
    [
        ((argb >> 16) & 0xFF) as f32 / 255.0,
        ((argb >> 8) & 0xFF) as f32 / 255.0,
        (argb & 0xFF) as f32 / 255.0,
        ((argb >> 24) & 0xFF) as f32 / 255.0,
    ]
}

fn compute_vertex_bounds(vertices: &[W3DVertex]) -> ([f32; 3], [f32; 3]) {
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

fn transform_bounds(min: [f32; 3], max: [f32; 3], matrix: Mat4) -> ([f32; 3], [f32; 3]) {
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

fn sync_camera_from_view_matrix(camera: &mut Camera) {
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

fn sync_camera_from_projection_matrix(camera: &mut Camera) {
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

fn default_render_state_value(state: W3D_RENDER_STATE) -> u32 {
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

fn default_render_states() -> HashMap<W3D_RENDER_STATE, u32> {
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

fn default_transform_state_value(state: W3D_TRANSFORM_STATE) -> W3D_MATRIX {
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

fn default_transform_states() -> HashMap<W3D_TRANSFORM_STATE, W3D_MATRIX> {
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

fn default_viewport(width: u32, height: u32) -> W3D_VIEWPORT {
    W3D_VIEWPORT {
        x: 0,
        y: 0,
        width,
        height,
        min_z: 0.0,
        max_z: 1.0,
    }
}
