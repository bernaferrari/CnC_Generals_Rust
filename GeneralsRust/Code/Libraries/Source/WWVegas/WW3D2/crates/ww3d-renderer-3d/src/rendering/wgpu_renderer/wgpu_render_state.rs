//! WGPU Render State Structures
//!
//! This module contains the render state structures used by the WGPU wrapper,
//! equivalent to the original RenderStateStruct from DX8Wrapper.

use crate::material_system::{TextureStageSettings, VertexMaterialClass};
use crate::rendering::camera_system::ViewportClass as Viewport;
use crate::rendering::shader_system::shader::ShaderClass;
use crate::rendering::texture_system::texture_base::TextureBaseClass;
use bitflags::bitflags;
use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;

use super::wgpu_buffer::{WgpuIndexBuffer, WgpuVertexBuffer};
use super::wgpu_vertex_format::VertexFormatFlags;

bitflags! {
    /// Render state change flags (equivalent to ChangedStates enum)
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ChangedStates: u32 {
        const WORLD_CHANGED = 1 << 0;
        const VIEW_CHANGED = 1 << 1;
        const LIGHT0_CHANGED = 1 << 2;
        const LIGHT1_CHANGED = 1 << 3;
        const LIGHT2_CHANGED = 1 << 4;
        const LIGHT3_CHANGED = 1 << 5;
        const TEXTURE0_CHANGED = 1 << 6;
        const TEXTURE1_CHANGED = 1 << 7;
        const TEXTURE2_CHANGED = 1 << 8;
        const TEXTURE3_CHANGED = 1 << 9;
        const MATERIAL_CHANGED = 1 << 14;
        const SHADER_CHANGED = 1 << 15;
        const VERTEX_BUFFER_CHANGED = 1 << 16;
        const INDEX_BUFFER_CHANGED = 1 << 17;
        const WORLD_IDENTITY = 1 << 18;
        const VIEW_IDENTITY = 1 << 19;

        const TEXTURES_CHANGED = Self::TEXTURE0_CHANGED.bits() |
                                Self::TEXTURE1_CHANGED.bits() |
                                Self::TEXTURE2_CHANGED.bits() |
                                Self::TEXTURE3_CHANGED.bits();
        const LIGHTS_CHANGED = Self::LIGHT0_CHANGED.bits() |
                              Self::LIGHT1_CHANGED.bits() |
                              Self::LIGHT2_CHANGED.bits() |
                              Self::LIGHT3_CHANGED.bits();
    }
}

/// Main render state structure (equivalent to RenderStateStruct)
#[derive(Debug, Clone)]
pub struct RenderStateStruct {
    /// Current shader
    pub shader: Option<Arc<ShaderClass>>,
    /// Current vertex material
    pub material: Option<Arc<VertexMaterialClass>>,
    /// Current textures (up to MAX_TEXTURE_STAGES)
    pub textures: Vec<Option<Arc<TextureBaseClass>>>,
    /// Current lights (up to 4)
    pub lights: [D3DLight8; 4],
    /// Light enable states
    pub light_enable: [bool; 4],
    /// World transformation matrix
    pub world: Mat4,
    /// View transformation matrix
    pub view: Mat4,
    /// World transformation matrix (alias for compatibility)
    pub world_matrix: Mat4,
    /// View transformation matrix (alias for compatibility)
    pub view_matrix: Mat4,
    /// Projection transformation matrix
    pub projection_matrix: Mat4,
    /// Vertex buffer types for each stream
    pub vertex_buffer_types: Vec<u32>,
    /// Index buffer type
    pub index_buffer_type: u32,
    /// Vertex buffer offset
    pub vba_offset: u16,
    /// Vertex buffer count
    pub vba_count: u16,
    /// Index buffer offset
    pub iba_offset: u16,
    /// Vertex buffers for each stream
    pub vertex_buffers: Vec<Option<Arc<WgpuVertexBuffer>>>,
    /// Index buffer
    pub index_buffer: Option<Arc<WgpuIndexBuffer>>,
    /// Index base offset
    pub index_base_offset: u16,
    /// Current viewport (WGPU specific)
    pub viewport: Option<Viewport>,
    /// Cached vertex format flag (DX8 FVF equivalent)
    pub vertex_format: Option<VertexFormatFlags>,
    /// Per-stage sampler/settings information
    pub texture_stage_settings: Vec<TextureStageSettings>,
}

impl RenderStateStruct {
    /// Create a new render state structure
    pub fn new() -> Self {
        Self {
            shader: None,
            material: None,
            textures: vec![None; super::wgpu_wrapper::MAX_TEXTURE_STAGES],
            lights: [D3DLight8::default(); 4],
            light_enable: [false; 4],
            world: Mat4::IDENTITY,
            view: Mat4::IDENTITY,
            world_matrix: Mat4::IDENTITY,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            vertex_buffer_types: vec![0; super::wgpu_wrapper::MAX_VERTEX_STREAMS],
            index_buffer_type: 0,
            vba_offset: 0,
            vba_count: 0,
            iba_offset: 0,
            vertex_buffers: vec![None; super::wgpu_wrapper::MAX_VERTEX_STREAMS],
            index_buffer: None,
            index_base_offset: 0,
            viewport: None,
            vertex_format: None,
            texture_stage_settings: vec![
                TextureStageSettings::default();
                super::wgpu_wrapper::MAX_TEXTURE_STAGES
            ],
        }
    }

    /// Create a new render state structure with specified capacity
    pub fn with_capacity(texture_stages: usize, vertex_streams: usize) -> Self {
        Self {
            shader: None,
            material: None,
            textures: vec![None; texture_stages],
            lights: [D3DLight8::default(); 4],
            light_enable: [false; 4],
            world: Mat4::IDENTITY,
            view: Mat4::IDENTITY,
            world_matrix: Mat4::IDENTITY,
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            vertex_buffer_types: vec![0; vertex_streams],
            index_buffer_type: 0,
            vba_offset: 0,
            vba_count: 0,
            iba_offset: 0,
            vertex_buffers: vec![None; vertex_streams],
            index_buffer: None,
            index_base_offset: 0,
            viewport: None,
            vertex_format: None,
            texture_stage_settings: vec![TextureStageSettings::default(); texture_stages],
        }
    }
}

impl Default for RenderStateStruct {
    fn default() -> Self {
        Self::new()
    }
}

/// Light structure (equivalent to D3DLIGHT8)
#[derive(Debug, Clone, Copy)]
pub struct D3DLight8 {
    pub light_type: LightType,
    pub diffuse: Vec4,
    pub specular: Vec4,
    pub ambient: Vec4,
    pub position: glam::Vec3,
    pub direction: glam::Vec3,
    pub range: f32,
    pub falloff: f32,
    pub attenuation0: f32,
    pub attenuation1: f32,
    pub attenuation2: f32,
    pub theta: f32,
    pub phi: f32,
}

impl Default for D3DLight8 {
    fn default() -> Self {
        Self {
            light_type: LightType::Point,
            diffuse: Vec4::ONE,
            specular: Vec4::ZERO,
            ambient: Vec4::ZERO,
            position: Vec3::ZERO,
            direction: Vec3::new(0.0, 0.0, -1.0),
            range: 1000.0,
            falloff: 1.0,
            attenuation0: 1.0,
            attenuation1: 0.0,
            attenuation2: 0.0,
            theta: 0.0,
            phi: std::f32::consts::PI,
        }
    }
}

/// Light type enumeration (equivalent to D3DLIGHTTYPE)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightType {
    Point = 1,
    Spot = 2,
    Directional = 3,
}

/// Material structure (equivalent to D3DMATERIAL8)
#[derive(Debug, Clone, Copy)]
pub struct D3DMaterial8 {
    pub diffuse: Vec4,
    pub ambient: Vec4,
    pub specular: Vec4,
    pub emissive: Vec4,
    pub power: f32,
}

impl Default for D3DMaterial8 {
    fn default() -> Self {
        Self {
            diffuse: Vec4::new(1.0, 1.0, 1.0, 1.0),
            ambient: Vec4::new(0.2, 0.2, 0.2, 1.0),
            specular: Vec4::ZERO,
            emissive: Vec4::ZERO,
            power: 0.0,
        }
    }
}

/// Viewport structure (equivalent to D3DVIEWPORT8)
#[derive(Debug, Clone, Copy)]
pub struct D3DViewport8 {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub min_z: f32,
    pub max_z: f32,
}

impl Default for D3DViewport8 {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
            min_z: 0.0,
            max_z: 1.0,
        }
    }
}

/// Surface description structure (equivalent to D3DSURFACE_DESC)
#[derive(Debug, Clone)]
pub struct D3DSurfaceDesc {
    pub format: wgpu::TextureFormat,
    pub r#type: SurfaceType,
    pub usage: wgpu::TextureUsages,
    pub pool: MemoryPool,
    pub size: wgpu::Extent3d,
    pub multi_sample_type: wgpu::MultisampleState,
    pub width: u32,
    pub height: u32,
}

impl Default for D3DSurfaceDesc {
    fn default() -> Self {
        Self {
            format: wgpu::TextureFormat::Rgba8Unorm,
            r#type: SurfaceType::Texture2D,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            pool: MemoryPool::Default,
            size: wgpu::Extent3d {
                width: 640,
                height: 480,
                depth_or_array_layers: 1,
            },
            multi_sample_type: wgpu::MultisampleState::default(),
            width: 640,
            height: 480,
        }
    }
}

/// Surface type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceType {
    Texture2D,
    CubeTexture,
    VolumeTexture,
}

/// Memory pool enumeration (equivalent to D3DPOOL)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryPool {
    Default,
    Managed,
    SystemMem,
    Scratch,
}

/// Texture description structure (equivalent to D3DTEXTURE_DESC)
#[derive(Debug, Clone)]
pub struct D3DTextureDesc {
    pub format: wgpu::TextureFormat,
    pub r#type: TextureType,
    pub usage: wgpu::TextureUsages,
    pub pool: MemoryPool,
    pub size: wgpu::Extent3d,
    pub mip_levels: u32,
}

impl Default for D3DTextureDesc {
    fn default() -> Self {
        Self {
            format: wgpu::TextureFormat::Rgba8Unorm,
            r#type: TextureType::Texture2D,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            pool: MemoryPool::Managed,
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_levels: 1,
        }
    }
}

/// Texture type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureType {
    Texture2D,
    CubeTexture,
    VolumeTexture,
}
