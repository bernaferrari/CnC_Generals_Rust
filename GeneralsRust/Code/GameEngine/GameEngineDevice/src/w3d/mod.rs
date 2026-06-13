//! # W3D Device Layer
//!
//! This module provides the Westwood 3D graphics system integration, converting the original
//! C++ W3D device layer to modern Rust with enhanced performance and cross-platform support.

pub mod graphics_context;
pub mod material_system;
pub mod model_loader;
pub mod performance_optimizer;
pub mod renderer;
pub mod shadow_system;
pub mod texture_manager;
pub mod volumetric_shadow;
pub mod w3d_c_api;
pub mod w3d_device;

// Re-exports
pub use graphics_context::{ContextState, GraphicsContext};
pub use renderer::{RenderBatch, RenderState, W3DRenderer};
pub use w3d_device::{W3DConfig, W3DDevice};

// Texture manager re-exports for parity with C++ W3DAssetManager
pub use texture_manager::{
    generate_team_color_palette_16bit, generate_team_color_palette_32bit, hsv_to_rgb,
    recolor_texture_16bit_hue_shift, recolor_texture_32bit_hue_shift, rgb_to_hsv,
    CompressionQuality, CompressionSettings, MipCountType, StreamRequest, TextureInactivationState,
    TextureManagerStats, TextureSource, W3DTextureGpu, W3DTextureManager, WW3DFormat,
    DEFAULT_INACTIVATION_TIME_MS, HOUSE_COLOR_SCALE, TEAM_COLOR_PALETTE_SIZE,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "w3d")]
use bytemuck::{Pod, Zeroable};
#[cfg(feature = "w3d")]
use wgpu;

/// W3D device error types
#[derive(Error, Debug)]
pub enum W3DError {
    /// Device initialization failed
    #[error("W3D device initialization failed: {0}")]
    InitializationFailed(String),

    /// Renderer creation failed
    #[error("W3D renderer creation failed: {0}")]
    RendererCreationFailed(String),

    /// Context creation failed
    #[error("W3D context creation failed: {0}")]
    ContextCreationFailed(String),

    /// Shader compilation failed
    #[error("W3D shader compilation failed: {0}")]
    ShaderCompilationFailed(String),

    /// Resource loading failed
    #[error("W3D resource loading failed: {0}")]
    ResourceLoadingFailed(String),

    /// Rendering error
    #[error("W3D rendering error: {0}")]
    RenderingError(String),

    /// Graphics API error
    #[error("W3D graphics API error: {0}")]
    GraphicsApiError(String),

    /// General resource error
    #[error("W3D resource error: {0}")]
    ResourceError(String),

    /// Model loading failed
    #[error("W3D model loading failed: {0}")]
    ModelLoadingFailed(String),
}

/// Result type for W3D operations
pub type Result<T> = std::result::Result<T, W3DError>;

/// W3D vertex format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VertexFormat {
    /// Position only (3 floats)
    Position,
    /// Position + Normal (6 floats)
    PositionNormal,
    /// Position + UV (5 floats)
    PositionUv,
    /// Position + Normal + UV (8 floats)
    PositionNormalUv,
    /// Position + Normal + UV + Color (12 floats)
    PositionNormalUvColor,
    /// Skinned vertex with bone weights
    Skinned,
}

/// W3D primitive topology
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimitiveTopology {
    /// Triangle list
    TriangleList,
    /// Triangle strip
    TriangleStrip,
    /// Triangle fan
    TriangleFan,
    /// Line list
    LineList,
    /// Line strip
    LineStrip,
    /// Point list
    PointList,
}

/// W3D render target description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderTarget {
    /// Target ID
    pub id: String,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Color format
    pub format: super::video::ColorFormat,
    /// Multi-sampling settings
    pub msaa: super::video::MsaaSettings,
    /// Has depth buffer
    pub has_depth: bool,
    /// Has stencil buffer
    pub has_stencil: bool,
}

/// W3D shader description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shader {
    /// Shader ID
    pub id: String,
    /// Shader name
    pub name: String,
    /// Vertex shader source
    pub vertex_source: String,
    /// Fragment shader source
    pub fragment_source: String,
    /// Geometry shader source (optional)
    pub geometry_source: Option<String>,
    /// Shader uniforms
    pub uniforms: Vec<ShaderUniform>,
}

/// Shader uniform description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderUniform {
    /// Uniform name
    pub name: String,
    /// Uniform type
    pub uniform_type: ShaderUniformType,
    /// Array size (1 for non-arrays)
    pub array_size: u32,
}

/// Shader uniform types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderUniformType {
    /// Float
    Float,
    /// 2D float vector
    Vec2,
    /// 3D float vector
    Vec3,
    /// 4D float vector
    Vec4,
    /// Integer
    Int,
    /// 2D integer vector
    IVec2,
    /// 3D integer vector
    IVec3,
    /// 4D integer vector
    IVec4,
    /// Boolean
    Bool,
    /// 2x2 matrix
    Mat2,
    /// 3x3 matrix
    Mat3,
    /// 4x4 matrix
    Mat4,
    /// 2D texture sampler
    Sampler2D,
    /// Cube texture sampler
    SamplerCube,
}

/// W3D texture description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Texture {
    /// Texture ID
    pub id: String,
    /// Texture name
    pub name: String,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Depth (for 3D textures)
    pub depth: u32,
    /// Mip levels
    pub mip_levels: u32,
    /// Texture format
    pub format: TextureFormat,
    /// Texture type
    pub texture_type: TextureType,
    /// Texture data
    pub data: Vec<u8>,
}

/// Texture formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureFormat {
    /// 8-bit RGBA
    Rgba8,
    /// 8-bit RGB  
    Rgb8,
    /// 16-bit RGBA
    Rgba16,
    /// 32-bit RGBA floating point
    Rgba32Float,
    /// DXT1 compression
    Dxt1,
    /// DXT3 compression
    Dxt3,
    /// DXT5 compression
    Dxt5,
    /// BC7 compression
    Bc7,
}

/// Texture types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextureType {
    /// 2D texture
    Texture2D,
    /// 3D texture
    Texture3D,
    /// Cube map texture
    TextureCube,
    /// 2D array texture
    Texture2DArray,
}

/// W3D mesh description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    /// Mesh ID
    pub id: String,
    /// Mesh name
    pub name: String,
    /// Vertex format
    pub vertex_format: VertexFormat,
    /// Vertex data
    pub vertices: Vec<u8>,
    /// Index data
    pub indices: Vec<u32>,
    /// Primitive topology
    pub topology: PrimitiveTopology,
    /// Material ID
    pub material_id: Option<String>,
    /// Bounding box
    pub bounding_box: BoundingBox,
}

/// 3D bounding box
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Minimum point
    pub min: [f32; 3],
    /// Maximum point
    pub max: [f32; 3],
}

impl BoundingBox {
    /// Create a new bounding box
    pub fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }

    /// Get center point
    pub fn center(self) -> [f32; 3] {
        [
            (self.min[0] + self.max[0]) * 0.5,
            (self.min[1] + self.max[1]) * 0.5,
            (self.min[2] + self.max[2]) * 0.5,
        ]
    }

    /// Get size
    pub fn size(self) -> [f32; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    /// Check if point is inside
    pub fn contains_point(self, point: [f32; 3]) -> bool {
        point[0] >= self.min[0]
            && point[0] <= self.max[0]
            && point[1] >= self.min[1]
            && point[1] <= self.max[1]
            && point[2] >= self.min[2]
            && point[2] <= self.max[2]
    }

    /// Get radius (half diagonal)
    pub fn radius(self) -> f32 {
        let s = self.size();
        ((s[0] * s[0] + s[1] * s[1] + s[2] * s[2]) as f64).sqrt() as f32 * 0.5
    }
}

/// W3D material description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    /// Material ID
    pub id: String,
    /// Material name
    pub name: String,
    /// Shader ID
    pub shader_id: String,
    /// Diffuse texture
    pub diffuse_texture: Option<String>,
    /// Normal texture
    pub normal_texture: Option<String>,
    /// Specular texture
    pub specular_texture: Option<String>,
    /// Emissive texture
    pub emissive_texture: Option<String>,
    /// Detail (second-stage) texture for multi-texture blending
    pub detail_texture: Option<String>,
    /// Multi-texture blend mode: 0=off, 1=MODULATE, 2=ADDSIGNED, 3=BLENDCURRENTALPHA
    pub detail_blend_mode: u8,
    /// Material properties
    pub properties: MaterialProperties,
}

/// Material properties
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialProperties {
    /// Diffuse color
    pub diffuse_color: [f32; 4],
    /// Specular color
    pub specular_color: [f32; 3],
    /// Emissive color
    pub emissive_color: [f32; 3],
    /// Shininess
    pub shininess: f32,
    /// Alpha cutoff for transparency
    pub alpha_cutoff: f32,
    /// Enable alpha test / cutout behavior
    pub alpha_test: bool,
    /// Is transparent
    pub transparent: bool,
    /// Is double-sided
    pub double_sided: bool,
    /// Bypass dynamic lighting and treat the material as prelit fixed-function output
    pub unlit: bool,
}

impl Default for MaterialProperties {
    fn default() -> Self {
        Self {
            diffuse_color: [1.0, 1.0, 1.0, 1.0],
            specular_color: [1.0, 1.0, 1.0],
            emissive_color: [0.0, 0.0, 0.0],
            shininess: 32.0,
            alpha_cutoff: 0.5,
            alpha_test: false,
            transparent: false,
            double_sided: false,
            unlit: false,
        }
    }
}

/// W3D light description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Light {
    /// Light ID
    pub id: String,
    /// Light name
    pub name: String,
    /// Light type
    pub light_type: LightType,
    /// Light position (for point and spot lights)
    pub position: [f32; 3],
    /// Light direction (for directional and spot lights)
    pub direction: [f32; 3],
    /// Light color
    pub color: [f32; 3],
    /// Light intensity
    pub intensity: f32,
    /// Attenuation parameters (constant, linear, quadratic)
    pub attenuation: [f32; 3],
    /// Spot light parameters (inner cone, outer cone)
    pub spot_params: Option<[f32; 2]>,
}

/// Light types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightType {
    /// Directional light (sun)
    Directional,
    /// Point light
    Point,
    /// Spot light
    Spot,
    /// Area light
    Area,
}

/// W3D camera description
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    /// Camera position
    pub position: [f32; 3],
    /// Camera target/look-at point
    pub target: [f32; 3],
    /// Camera up vector
    pub up: [f32; 3],
    /// Field of view in radians
    pub fov: f32,
    /// Aspect ratio
    pub aspect_ratio: f32,
    /// Near clipping plane
    pub near_plane: f32,
    /// Far clipping plane
    pub far_plane: f32,
    /// View matrix
    pub view_matrix: [[f32; 4]; 4],
    /// Projection matrix
    pub projection_matrix: [[f32; 4]; 4],
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            fov: std::f32::consts::PI / 4.0, // 45 degrees
            aspect_ratio: 16.0 / 9.0,
            near_plane: 0.1,
            far_plane: 1000.0,
            view_matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            projection_matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

impl Camera {
    /// Update view matrix from position, target, and up vectors.
    ///
    /// Computes a proper look-at (right-handed) view matrix matching the C++ behavior
    /// where the view matrix is the inverse of the camera's world-space transform.
    /// C++: `Transform.Get_Inverse(CameraInvTransform)` — forward is -Z in view space.
    #[cfg(feature = "w3d")]
    pub fn update_view_matrix(&mut self) {
        let eye = glam::Vec3::from(self.position);
        let target = glam::Vec3::from(self.target);
        let up = glam::Vec3::from(self.up);
        let view = glam::Mat4::look_at_rh(eye, target, up);
        self.view_matrix = view.to_cols_array_2d();
    }

    /// Fallback view matrix calculation without glam (feature-gated).
    /// Uses a manual look-at computation matching right-handed conventions.
    #[cfg(not(feature = "w3d"))]
    pub fn update_view_matrix(&mut self) {
        let eye = self.position;
        let target = self.target;
        let up = self.up;

        // forward = normalize(target - eye), but C++ convention: forward is -Z in view space
        let f = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
        let f_len = (f[0] * f[0] + f[1] * f[1] + f[2] * f[2]).sqrt();
        let f = [f[0] / f_len, f[1] / f_len, f[2] / f_len];

        // side = normalize(cross(forward, up))
        let s = [
            f[1] * up[2] - f[2] * up[1],
            f[2] * up[0] - f[0] * up[2],
            f[0] * up[1] - f[1] * up[0],
        ];
        let s_len = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        let s = [s[0] / s_len, s[1] / s_len, s[2] / s_len];

        // recompute up = cross(side, forward)
        let u = [
            s[1] * f[2] - s[2] * f[1],
            s[2] * f[0] - s[0] * f[2],
            s[0] * f[1] - s[1] * f[0],
        ];

        self.view_matrix = [
            [s[0], u[0], -f[0], 0.0],
            [s[1], u[1], -f[1], 0.0],
            [s[2], u[2], -f[2], 0.0],
            [
                -(s[0] * eye[0] + s[1] * eye[1] + s[2] * eye[2]),
                -(u[0] * eye[0] + u[1] * eye[1] + u[2] * eye[2]),
                f[0] * eye[0] + f[1] * eye[1] + f[2] * eye[2],
                1.0,
            ],
        ];
    }

    /// Update projection matrix from field of view and aspect ratio
    pub fn update_projection_matrix(&mut self) {
        // Simplified projection matrix calculation
        let f = 1.0 / (self.fov * 0.5).tan();
        let range_inv = 1.0 / (self.near_plane - self.far_plane);

        self.projection_matrix = [
            [f / self.aspect_ratio, 0.0, 0.0, 0.0],
            [0.0, f, 0.0, 0.0],
            [
                0.0,
                0.0,
                (self.near_plane + self.far_plane) * range_inv,
                2.0 * self.near_plane * self.far_plane * range_inv,
            ],
            [0.0, 0.0, -1.0, 0.0],
        ];
    }
}

/// W3D vertex data for GPU upload
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3DVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[cfg(feature = "w3d")]
unsafe impl bytemuck::Pod for W3DVertex {}
#[cfg(feature = "w3d")]
unsafe impl bytemuck::Zeroable for W3DVertex {}

/// W3D uniform data for shaders
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3DUniformData {
    pub model_matrix: [[f32; 4]; 4],
    pub view_matrix: [[f32; 4]; 4],
    pub projection_matrix: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 3]; 3],
}

#[cfg(feature = "w3d")]
unsafe impl bytemuck::Pod for W3DUniformData {}
#[cfg(feature = "w3d")]
unsafe impl bytemuck::Zeroable for W3DUniformData {}

/// W3D light data for lighting calculations
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3DLightData {
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub light_type: u32, // 0=directional, 1=point, 2=spot
}

#[cfg(feature = "w3d")]
unsafe impl bytemuck::Pod for W3DLightData {}
#[cfg(feature = "w3d")]
unsafe impl bytemuck::Zeroable for W3DLightData {}

/// W3D material data for PBR rendering
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct W3DMaterialData {
    pub albedo: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emission: [f32; 3],
}

#[cfg(feature = "w3d")]
unsafe impl bytemuck::Pod for W3DMaterialData {}
#[cfg(feature = "w3d")]
unsafe impl bytemuck::Zeroable for W3DMaterialData {}

/// GPU-uploaded mesh data
#[derive(Debug)]
pub struct W3DMeshGpu {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub vertex_count: u32,
    pub index_count: u32,
}

/// GPU-uploaded material data
#[derive(Debug)]
pub struct W3DMaterialGpu {
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

/// GPU-compiled shader data
#[derive(Debug)]
pub struct W3DShaderGpu {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}
