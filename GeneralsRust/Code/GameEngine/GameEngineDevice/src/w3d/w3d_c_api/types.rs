//! W3D C API types, enums, and device/resource wrappers.
//!
//! Split from `w3d_c_api.rs`. Public names stay identical for C ABI / parity.

use super::math::W3D_MATRIX;
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
    pub(super) device: Arc<RwLock<W3DDevice>>,
    pub(super) runtime: tokio::runtime::Runtime,
    pub(super) render_states: Mutex<HashMap<W3D_RENDER_STATE, u32>>,
    pub(super) transform_states: Mutex<HashMap<W3D_TRANSFORM_STATE, W3D_MATRIX>>,
    pub(super) viewport: Mutex<W3D_VIEWPORT>,
    pub(super) bound_textures: Mutex<HashMap<u32, String>>,
    pub(super) texture_handles: Mutex<HashMap<String, W3D_TEXTURE>>,
    pub(super) texture_stage_states: Mutex<HashMap<(u32, u32), u32>>,
    pub(super) stream_sources: Mutex<HashMap<u32, StagedStreamSource>>,
    pub(super) staged_indices: Mutex<Vec<u16>>,
    pub(super) staged_base_vertex_index: Mutex<i32>,
    pub(super) current_fvf: Mutex<u32>,
    pub(super) current_vertex_declaration: Mutex<u32>,
    pub(super) vertex_declarations: Mutex<HashMap<u32, Vec<W3D_VERTEX_ELEMENT>>>,
    pub(super) current_vertex_shader: Mutex<u32>,
    pub(super) current_pixel_shader: Mutex<u32>,
    pub(super) material_texture_bindings: Mutex<HashMap<MaterialBindingCacheKey, String>>,
    pub(super) lights: Mutex<HashMap<u32, Light>>,
    pub(super) enabled_lights: Mutex<HashMap<u32, bool>>,
    pub(super) material_counter: Mutex<u64>,
    pub(super) current_material_id: Mutex<Option<String>>,
    pub(super) current_material_data: Mutex<Option<W3DMaterialData>>,
    pub(super) scene_active: Mutex<bool>,
    pub(super) transient_mesh_counter: Mutex<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct MaterialBindingCacheKey {
    pub(super) base_material_id: Option<String>,
    pub(super) texture_id: String,
    pub(super) tint_rgba: [u8; 4],
    pub(super) combiner_signature: MaterialCombinerSignature,
    pub(super) lighting_state: FixedFunctionLightingState,
    pub(super) surface_state: FixedFunctionSurfaceState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct FixedFunctionLightingState {
    pub(super) lighting_enabled: bool,
    pub(super) specular_enabled: bool,
    pub(super) color_vertex: bool,
    pub(super) local_viewer: bool,
    pub(super) normalize_normals: bool,
    pub(super) ambient_argb: u32,
    pub(super) ambient_material_source: u32,
    pub(super) diffuse_material_source: u32,
    pub(super) specular_material_source: u32,
    pub(super) emissive_material_source: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct FixedFunctionSurfaceState {
    pub(super) alpha_test_enabled: bool,
    pub(super) alpha_ref: u8,
    pub(super) alpha_blend_enabled: bool,
    pub(super) cull_mode: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct MaterialCombinerSignature {
    pub(super) sampling_stage_count: u8,
    pub(super) force_multiply_like: bool,
}

#[derive(Debug, Clone)]
pub(super) struct StagedStreamSource {
    pub(super) vertex_stride: usize,
    pub(super) vertex_offset_bytes: usize,
    pub(super) vertex_count: usize,
    pub(super) data: Vec<u8>,
}

/// W3D Texture C wrapper
pub struct W3DTextureC {
    pub(super) texture: Texture,
}

/// W3D Mesh C wrapper
pub struct W3DMeshC {
    pub(super) mesh: Mesh,
}

/// W3D Material C wrapper
pub struct W3DMaterialC {
    pub(super) material: Material,
}
