//! W3D Occlusion Stencil and Shadow Mask Rendering Pipeline
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DVolumetricShadow.cpp
//!   (renderShadows, renderStencilShadows)
//! - GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DProjectedShadow.cpp
//!   (renderProjectedTerrainShadow)
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DShadow.h
//!
//! Implements the multi-pass stencil approach for:
//! 1. Occlusion testing - rendering objects to stencil buffer for visibility
//! 2. Shadow volume rendering - building shadow mask in stencil via increment/decrement
//! 3. Shadow mask application - fullscreen quad with shadow darkening where stencil > 0
//!
//! The C++ uses D3D stencil states (D3DRS_STENCILENABLE, D3DRS_STENCILFUNC, etc.)
//! to implement a two-pass shadow volume technique (sometimes called "Carmack's reverse"):
//!   - Front-face pass (CW cull): STENCILPASS = INCR
//!   - Back-face pass (CCW cull): STENCILPASS = DECRSAT
//!   - Final: render fullscreen quad where stencil ref (1) <= stencil buffer value

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use wgpu::{
    BlendComponent, BlendFactor, BlendOperation, Buffer, BufferUsages, ColorTargetState,
    ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Device, Face, FragmentState,
    FrontFace, MultisampleState, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology,
    RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, StencilFaceState,
    StencilOperation, StencilState, TextureFormat, VertexState,
};

/// Shadow volume vertex (position only, no color needed)
/// C++: struct SHADOW_STATIC_VOLUME_VERTEX { float x,y,z; }
/// C++: struct SHADOW_DYNAMIC_VOLUME_VERTEX { float x,y,z; DWORD diffuse; } (debug only)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShadowVolumeVertex {
    pub position: [f32; 3],
}

impl ShadowVolumeVertex {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            position: [x, y, z],
        }
    }

    /// WGPU vertex buffer layout descriptor for shadow volume vertices
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowVolumeVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }
}

/// Fullscreen quad vertex with position + color for shadow mask pass
/// C++: struct _TRANSLITVERTEX { D3DXVECTOR4 p; DWORD color; }
/// Used in renderStencilShadows() to draw the shadow overlay
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShadowMaskVertex {
    /// Screen-space position (x, y, z=0, w=1 for ortho projection)
    pub position: [f32; 4],
    /// Shadow color (ARGB)
    pub color: u32,
}

impl ShadowMaskVertex {
    pub fn new(x: f32, y: f32, z: f32, w: f32, color: u32) -> Self {
        Self {
            position: [x, y, z, w],
            color,
        }
    }

    /// WGPU vertex buffer layout for shadow mask vertices
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowMaskVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

/// Occlusion stencil configuration flags
/// PARITY_NOTE: C++ does not have a struct for this, but uses these states inline.
/// We collect them for reuse across the two shadow volume passes.
#[derive(Debug, Clone)]
pub struct OcclusionStencilConfig {
    /// Whether the occlusion pass is enabled
    pub enabled: bool,
    /// Whether shadow volumes are enabled
    pub shadow_volumes_enabled: bool,
    /// Whether shadow decals are enabled
    pub shadow_decals_enabled: bool,
    /// Stencil shadow mask value
    /// C++: m_stencilShadowMask - isolates bits used for player color / occlusion
    pub stencil_shadow_mask: u32,
    /// Shadow color (ARGB format)
    /// C++: m_shadowColor = 0x7fa0a0a0
    pub shadow_color: u32,
}

impl Default for OcclusionStencilConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            shadow_volumes_enabled: true,
            shadow_decals_enabled: true,
            stencil_shadow_mask: 0x80808080,
            shadow_color: 0x7fa0a0a0,
        }
    }
}

/// Pipeline bundle for the occlusion stencil shadow volume passes
/// Holds the two render pipelines needed for the two-pass stencil technique:
///   1. Front-face increment pass (CW culling)
///   2. Back-face decrement pass (CCW culling)
pub struct OcclusionStencilPipeline {
    /// Pipeline for shadow volume front-face pass (CW cull, INCR stencil)
    /// C++: renderShadows() first pass with D3DRS_CULLMODE=D3DCULL_CW, STENCILPASS=INCR
    pub front_face_pipeline: RenderPipeline,
    /// Pipeline for shadow volume back-face pass (CCW cull, DECRSAT stencil)
    /// C++: renderShadows() second pass with D3DRS_CULLMODE=D3DCULL_CCW, STENCILPASS=DECRSAT
    pub back_face_pipeline: RenderPipeline,
    /// Pipeline for projected shadow terrain (stencil mark)
    /// C++: renderProjectedTerrainShadow() with STENCILPASS=INCR
    pub projected_terrain_pipeline: RenderPipeline,
}

/// Pipeline for rendering the shadow mask (fullscreen quad with stencil test)
/// C++: renderStencilShadows() draws a transparent rectangle over the screen
pub struct ShadowMaskPipeline {
    /// Pipeline for shadow mask application
    /// C++: renderStencilShadows() - alpha-blended quad with STENCILFUNC=LESSEQUAL
    pub mask_pipeline: RenderPipeline,
    /// Vertex buffer for fullscreen quad (4 vertices)
    pub quad_vertex_buffer: Buffer,
    /// Index buffer for 2 triangles
    pub quad_index_buffer: Buffer,
}

// ---------------------------------------------------------------------------
// WGSL Shaders
// ---------------------------------------------------------------------------

/// Shadow volume vertex shader - transforms position, outputs nothing (no color write)
/// The fragment shader is trivial since color writes are disabled.
/// C++: SHADOW_STATIC_VOLUME_FVF = D3DFVF_XYZ, no pixel shader needed
const SHADOW_VOLUME_SHADER: &str = r#"
struct ShadowUniforms {
    mvp: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: ShadowUniforms;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.mvp * vec4<f32>(pos, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // Color write is disabled on this pipeline, but we need a valid return
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
"#;

/// Shadow mask shader - renders a fullscreen quad with shadow color where stencil > 0
/// C++: renderStencilShadows() uses D3DFVF_XYZRHW|D3DFVF_DIFFUSE with a triangle strip
const SHADOW_MASK_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) color: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) shadow_color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = input.position;
    // Unpack ARGB u32 to RGBA float4
    let r = f32((input.color >> 16u) & 0xFFu) / 255.0;
    let g = f32((input.color >> 8u) & 0xFFu) / 255.0;
    let b = f32(input.color & 0xFFu) / 255.0;
    let a = f32((input.color >> 24u) & 0xFFu) / 255.0;
    out.shadow_color = vec4<f32>(r, g, b, a);
    return out;
}

@fragment
fn fs_main(@location(0) shadow_color: vec4<f32>) -> @location(0) vec4<f32> {
    return shadow_color;
}
"#;

// ---------------------------------------------------------------------------
// Occlusion stencil pipeline creation
// ---------------------------------------------------------------------------

impl OcclusionStencilPipeline {
    /// Create the shadow volume stencil pipelines
    /// C++: The equivalent setup in renderShadows() is:
    ///   - Set material to PRELIT_DIFFUSE, shader to OpaqueShader
    ///   - Disable color writes (D3DRS_COLORWRITEENABLE=0) or fake via alpha blend
    ///   - Enable stencil: STENCILFUNC=NOTEQUAL/GREATEREQUAL, STENCILREF=0x80808080
    ///   - STENCILPASS=INCR for front faces (CW), DECRSAT for back faces (CCW)
    pub fn new(device: &Device, config: &OcclusionStencilConfig) -> Self {
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Volume Shader"),
            source: SHADOW_VOLUME_SHADER,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Volume Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Volume Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Determine stencil function based on mask
        // C++: if (getStencilShadowMask() == 0x80808080) NOTEQUAL else GREATEREQUAL
        // PARITY_NOTE: C++ uses the MSB as a potential occluder bit. In WGPU, stencil
        // is 8-bit, so we simplify to GREATEREQUAL with ref 1 for the shadow volume pass.
        let stencil_func = if config.stencil_shadow_mask == 0x80808080 {
            CompareFunction::NotEqual
        } else {
            CompareFunction::GreaterEqual
        };

        // Stencil reference for the occlusion check
        // C++: m_pDev->SetRenderState(D3DRS_STENCILREF, 0x80808080)
        // In WGPU (8-bit stencil), we use 0x80 as the high-bit marker
        let stencil_ref: u32 = 0x80;

        // Stencil mask isolates upper bits for player color / occlusion
        // C++: m_pDev->SetRenderState(D3DRS_STENCILMASK, getStencilShadowMask())
        let stencil_read_mask: u32 = (config.stencil_shadow_mask & 0xFF) as u32;
        let stencil_write_mask: u32 = 0xFF;

        // --- Front-face pass: CW culling, STENCILPASS = INCR ---
        // C++: D3DRS_CULLMODE = D3DCULL_CW (cull clockwise = render CCW front faces)
        // C++: D3DRS_STENCILPASS = D3DSTENCILOP_INCR
        let front_face_stencil = StencilFaceState {
            compare: stencil_func,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            pass_op: StencilOperation::IncrementClamp,
        };

        let front_face_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Volume Front Face Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[ShadowVolumeVertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    // PARITY_NOTE: Color writes disabled via write_mask below
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: None,
                    write_mask: ColorWrites::empty(),
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                // C++: D3DRS_CULLMODE = D3DCULL_CW
                // In WGPU, CullMode::Front culls front faces (CW in this context)
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Front),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                // C++: D3DRS_ZFUNC = D3DCMP_LESSEQUAL
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState {
                    front: front_face_stencil,
                    back: StencilFaceState::default(),
                    read_mask: stencil_read_mask,
                    write_mask: stencil_write_mask,
                },
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // --- Back-face pass: CCW culling, STENCILPASS = DECRSAT ---
        // C++: D3DRS_CULLMODE = D3DCULL_CCW (cull CCW = render CW front faces)
        // C++: D3DRS_STENCILPASS = D3DSTENCILOP_DECRSAT
        let back_face_stencil = StencilFaceState {
            compare: stencil_func,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            // C++: D3DSTENCILOP_DECRSAT - decrement and saturate at 0
            pass_op: StencilOperation::DecrementClamp,
        };

        let back_face_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Volume Back Face Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[ShadowVolumeVertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: None,
                    write_mask: ColorWrites::empty(),
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                // C++: D3DRS_CULLMODE = D3DCULL_CCW
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState {
                    front: back_face_stencil,
                    back: StencilFaceState::default(),
                    read_mask: stencil_read_mask,
                    write_mask: stencil_write_mask,
                },
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // --- Projected terrain shadow pipeline ---
        // C++: renderProjectedTerrainShadow() uses STENCILFUNC=ALWAYS, STENCILPASS=INCR
        // to mark terrain cells that receive projected shadows
        let projected_terrain_stencil = StencilFaceState {
            // C++: D3DRS_STENCILFUNC = D3DCMP_ALWAYS
            compare: CompareFunction::Always,
            fail_op: StencilOperation::Keep,
            // C++: D3DRS_STENCILZFAIL = D3DSTENCILOP_KEEP
            depth_fail_op: StencilOperation::Keep,
            // C++: D3DRS_STENCILPASS = D3DSTENCILOP_INCR
            pass_op: StencilOperation::IncrementWrap,
        };

        let projected_terrain_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Projected Shadow Terrain Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[ShadowVolumeVertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    blend: None,
                    // C++: Also disables color writes for projected terrain stencil pass
                    write_mask: ColorWrites::empty(),
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                // C++: D3DRS_ZFUNC defaults for this pass
                depth_compare: CompareFunction::LessEqual,
                stencil: StencilState {
                    front: projected_terrain_stencil,
                    back: StencilFaceState::default(),
                    // C++: D3DRS_STENCILREF = 0x1
                    read_mask: 0xFF,
                    // C++: D3DRS_STENCILWRITEMASK = 0xffffffff
                    write_mask: 0xFF,
                },
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // We drop the bind_group_layout since the pipeline layout holds it
        drop(bind_group_layout);

        Self {
            front_face_pipeline,
            back_face_pipeline,
            projected_terrain_pipeline,
        }
    }
}

// ---------------------------------------------------------------------------
// Shadow mask pipeline creation
// ---------------------------------------------------------------------------

impl ShadowMaskPipeline {
    /// Create the shadow mask rendering pipeline and fullscreen quad buffers
    /// C++: renderStencilShadows() renders a fullscreen triangle strip:
    ///   - 4 vertices covering the screen (D3DFVF_XYZRHW | D3DFVF_DIFFUSE)
    ///   - Alpha blending: SRCBLEND=DESTCOLOR, DESTBLEND=ZERO (multiplicative)
    ///   - Stencil: STENCILENABLE=TRUE, STENCILFUNC=LESSEQUAL, STENCILREF=0x1
    ///     STENCILMASK=~getStencilShadowMask(), STENCILPASS=KEEP
    pub fn new(device: &Device, config: &OcclusionStencilConfig) -> Self {
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Shadow Mask Shader"),
            source: SHADOW_MASK_SHADER,
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Mask Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        // C++: D3DRS_STENCILFUNC = D3DCMP_LESSEQUAL, STENCILREF = 0x1
        // C++: D3DRS_STENCILMASK = ~getStencilShadowMask()
        // C++: D3DRS_STENCILPASS = D3DSTENCILOP_KEEP
        let stencil_face = StencilFaceState {
            compare: CompareFunction::LessEqual,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            pass_op: StencilOperation::Keep,
        };

        // C++: D3DRS_ZFUNC = D3DCMP_ALWAYS (no depth testing for the overlay)
        // C++: D3DRS_ALPHABLENDENABLE = TRUE
        // C++: D3DRS_SRCBLEND = D3DBLEND_DESTCOLOR
        // C++: D3DRS_DESTBLEND = D3DBLEND_ZERO
        let mask_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Shadow Mask Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: &[ShadowMaskVertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8UnormSrgb,
                    // C++: Multiplicative blend for shadow darkening
                    blend: Some(wgpu::BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::DstColor,
                            dst_factor: BlendFactor::Zero,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::DstColor,
                            dst_factor: BlendFactor::Zero,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth24PlusStencil8,
                depth_write_enabled: false,
                // C++: D3DRS_ZFUNC = D3DCMP_ALWAYS
                depth_compare: CompareFunction::Always,
                stencil: StencilState {
                    front: stencil_face,
                    back: stencil_face,
                    // C++: D3DRS_STENCILMASK = ~TheW3DShadowManager->getStencilShadowMask()
                    // Invert upper bits to only test lower shadow bits
                    read_mask: !(config.stencil_shadow_mask & 0xFF) as u32,
                    write_mask: 0,
                },
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Create fullscreen quad vertex buffer
        // C++: 4 vertices as D3DPT_TRIANGLESTRIP
        // We use 4 vertices + 6 indices (2 triangles) for TriangleList topology
        // Vertices are in normalized device coordinates (NDC)
        // PARITY_NOTE: C++ uses screen-space coordinates from TheTacticalView.
        // We use NDC [-1,1] which is equivalent after projection.
        let quad_vertices: [ShadowMaskVertex; 4] = [
            // Top-right
            ShadowMaskVertex::new(1.0, 1.0, 0.0, 1.0, config.shadow_color),
            // Top-left
            ShadowMaskVertex::new(-1.0, 1.0, 0.0, 1.0, config.shadow_color),
            // Bottom-right
            ShadowMaskVertex::new(1.0, -1.0, 0.0, 1.0, config.shadow_color),
            // Bottom-left
            ShadowMaskVertex::new(-1.0, -1.0, 0.0, 1.0, config.shadow_color),
        ];

        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Mask Quad Vertices"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: BufferUsages::VERTEX,
        });

        // Two triangles forming a quad: [0,1,2] and [2,1,3]
        let quad_indices: [u16; 6] = [0, 1, 2, 2, 1, 3];
        let quad_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Mask Quad Indices"),
            contents: bytemuck::cast_slice(&quad_indices),
            usage: BufferUsages::INDEX,
        });

        Self {
            mask_pipeline,
            quad_vertex_buffer,
            quad_index_buffer,
        }
    }
}

/// Combined occlusion stencil and shadow mask system
/// C++: W3DVolumetricShadowManager + W3DShadowManager coordinate these passes
pub struct OcclusionStencilSystem {
    /// Configuration flags
    pub config: OcclusionStencilConfig,
    /// Shadow volume stencil pipelines (front/back face passes)
    pub stencil_pipeline: Option<OcclusionStencilPipeline>,
    /// Shadow mask rendering pipeline (fullscreen quad)
    pub mask_pipeline: Option<ShadowMaskPipeline>,
    /// Whether the system has been initialized with GPU resources
    pub initialized: bool,
}

impl Default for OcclusionStencilSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl OcclusionStencilSystem {
    /// Create a new occlusion stencil system
    pub fn new() -> Self {
        Self {
            config: OcclusionStencilConfig::default(),
            stencil_pipeline: None,
            mask_pipeline: None,
            initialized: false,
        }
    }

    /// Initialize GPU resources (pipelines, buffers)
    /// Must be called after the WGPU device is available
    pub fn init(&mut self, device: &Device) {
        self.stencil_pipeline = Some(OcclusionStencilPipeline::new(device, &self.config));
        self.mask_pipeline = Some(ShadowMaskPipeline::new(device, &self.config));
        self.initialized = true;
    }

    /// Release GPU resources
    /// C++: W3DVolumetricShadowManager::ReleaseResources()
    pub fn release_resources(&mut self) {
        self.stencil_pipeline = None;
        self.mask_pipeline = None;
        self.initialized = false;
    }

    /// Update configuration. If a device is provided, pipelines are recreated immediately.
    /// Otherwise pipelines are invalidated and must be recreated via init().
    pub fn set_config(&mut self, config: OcclusionStencilConfig, device: Option<&Device>) {
        self.config = config;
        if let Some(dev) = device {
            self.init(dev);
        } else {
            self.stencil_pipeline = None;
            self.mask_pipeline = None;
            self.initialized = false;
        }
    }

    /// Check if system is ready for rendering
    pub fn is_ready(&self) -> bool {
        self.initialized && self.stencil_pipeline.is_some() && self.mask_pipeline.is_some()
    }
}

// ---------------------------------------------------------------------------
// Integration helper for the render loop
// ---------------------------------------------------------------------------

/// Describes a single shadow volume render task with its GPU buffers
/// C++: W3DVolumetricShadowRenderTask { m_parentShadow, m_meshIndex, m_lightIndex }
#[derive(Debug, Clone)]
pub struct ShadowVolumeRenderTask {
    pub mesh_index: u8,
    pub light_index: u8,
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub bind_group: Option<wgpu::BindGroup>,
    pub index_count: u32,
}

impl Default for ShadowVolumeRenderTask {
    fn default() -> Self {
        Self {
            mesh_index: 0,
            light_index: 0,
            vertex_buffer: None,
            index_buffer: None,
            bind_group: None,
            index_count: 0,
        }
    }
}

/// Result of the shadow volume rendering pass
#[derive(Debug, Clone, Default)]
pub struct ShadowPassResult {
    /// Number of shadow volumes rendered
    pub num_rendered_shadows: u32,
    /// Whether any shadows were rendered (used to decide if mask pass is needed)
    pub has_shadows: bool,
}

/// Encode the shadow volume stencil passes into a command encoder.
/// This corresponds to C++ W3DVolumetricShadowManager::renderShadows() line 3429.
///
/// Creates two render passes:
///   1. Front-face pass (CW cull, INCR stencil)
///   2. Back-face pass (CCW cull, DECRSAT stencil)
///
/// Call AFTER the depth pre-pass and BEFORE the main color pass.
pub fn encode_shadow_volume_pass(
    encoder: &mut wgpu::CommandEncoder,
    stencil_pipeline: &OcclusionStencilPipeline,
    depth_stencil_view: &wgpu::TextureView,
    color_view: &wgpu::TextureView,
    tasks: &[ShadowVolumeRenderTask],
) -> ShadowPassResult {
    let drawable_tasks: Vec<&ShadowVolumeRenderTask> = tasks
        .iter()
        .filter(|t| t.vertex_buffer.is_some() && t.index_buffer.is_some() && t.index_count > 0)
        .collect();

    if drawable_tasks.is_empty() {
        return ShadowPassResult::default();
    }

    // PARITY_NOTE: C++ renderShadows() steps 4-9:
    // Step 4: CULLMODE=CW, STENCILPASS=INCR → render all volumes
    // Step 6-7: STENCILPASS=DECRSAT, CULLMODE=CCW → re-render all volumes
    let num_rendered = drawable_tasks.len() as u32;

    // --- Pass 1: Front-face increment (CW cull, INCR) ---
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Volume Front Face Pass (INCR)"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_stencil_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&stencil_pipeline.front_face_pipeline);

        for task in &drawable_tasks {
            // PARITY_NOTE: C++ shadow->RenderVolume() binds VB/IB, calls DrawIndexedPrimitive
            if let (Some(ref vb), Some(ref ib)) = (&task.vertex_buffer, &task.index_buffer) {
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                if let Some(ref bg) = task.bind_group {
                    render_pass.set_bind_group(0, bg, &[]);
                }
                render_pass.draw_indexed(0..task.index_count, 0, 0..1);
            }
        }

        drop(render_pass);
    }

    // --- Pass 2: Back-face decrement (CCW cull, DECRSAT) ---
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Shadow Volume Back Face Pass (DECRSAT)"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_stencil_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&stencil_pipeline.back_face_pipeline);

        for task in &drawable_tasks {
            // PARITY_NOTE: C++ re-renders same VB/IB with back-face culling (DECRSAT)
            if let (Some(ref vb), Some(ref ib)) = (&task.vertex_buffer, &task.index_buffer) {
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                if let Some(ref bg) = task.bind_group {
                    render_pass.set_bind_group(0, bg, &[]);
                }
                render_pass.draw_indexed(0..task.index_count, 0, 0..1);
            }
        }

        drop(render_pass);
    }

    ShadowPassResult {
        num_rendered_shadows: num_rendered,
        has_shadows: num_rendered > 0,
    }
}

/// Encode the shadow mask pass (fullscreen quad with stencil test)
/// This corresponds to C++ W3DVolumetricShadowManager::renderStencilShadows()
///
/// Call this AFTER the shadow volume pass and BEFORE the main color pass.
///
/// # Arguments
/// * `encoder` - The command encoder to record commands into
/// * `mask_pipeline` - The shadow mask pipeline with quad buffers
/// * `depth_stencil_view` - The depth-stencil attachment view
/// * `color_view` - The color attachment view
pub fn encode_shadow_mask_pass(
    encoder: &mut wgpu::CommandEncoder,
    mask_pipeline: &ShadowMaskPipeline,
    depth_stencil_view: &wgpu::TextureView,
    color_view: &wgpu::TextureView,
) {
    // C++: renderStencilShadows() line 3363:
    // 1. Create 4 vertices covering the screen (D3DFVF_XYZRHW|D3DFVF_DIFFUSE)
    // 2. Set vertex shader
    // 3. Enable alpha blending: SRCBLEND=DESTCOLOR, DESTBLEND=ZERO
    // 4. Enable stencil: STENCILENABLE=TRUE, STENCILFUNC=LESSEQUAL, STENCILREF=0x1
    // 5. STENCILMASK=~getStencilShadowMask(), STENCILPASS=KEEP
    // 6. ZFUNC=ALWAYS (no depth test)
    // 7. Draw triangle strip (2 triangles, 4 vertices)
    // 8. Disable stencil, alpha blend

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Shadow Mask Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_stencil_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    render_pass.set_pipeline(&mask_pipeline.mask_pipeline);
    render_pass.set_vertex_buffer(0, mask_pipeline.quad_vertex_buffer.slice(..));
    render_pass.set_index_buffer(
        mask_pipeline.quad_index_buffer.slice(..),
        wgpu::IndexFormat::Uint16,
    );

    // Draw fullscreen quad (2 triangles, 6 indices)
    render_pass.draw_indexed(0..6, 0, 0..1);

    drop(render_pass);
}

/// Encode the projected shadow terrain stencil pass.
/// Corresponds to C++ W3DProjectedShadowManager::renderProjectedTerrainShadow() line 328.
///
/// Marks terrain cells in the stencil buffer that receive projected shadows.
/// Uses STENCILFUNC=ALWAYS, STENCILPASS=INCR to build the shadow mask.
///
/// Call BEFORE the volumetric shadow volume pass (projected shadows render first).
pub fn encode_projected_shadow_terrain_pass(
    encoder: &mut wgpu::CommandEncoder,
    stencil_pipeline: &OcclusionStencilPipeline,
    depth_stencil_view: &wgpu::TextureView,
    color_view: &wgpu::TextureView,
    vertex_buffer: Option<&wgpu::Buffer>,
    index_buffer: Option<&wgpu::Buffer>,
    index_count: u32,
) {
    // PARITY_NOTE: C++ renderProjectedTerrainShadow() line 328:
    // STENCILFUNC=ALWAYS, STENCILREF=0x1, STENCILWRITEMASK=0xffffffff
    // STENCILZFAIL=KEEP, STENCILFAIL=KEEP, STENCILPASS=INCR
    // ALPHABLENDENABLE=TRUE, SRCBLEND=DESTCOLOR, DESTBLEND=ZERO
    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Projected Shadow Terrain Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: depth_stencil_view,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
    });

    render_pass.set_pipeline(&stencil_pipeline.projected_terrain_pipeline);

    // PARITY_NOTE: Terrain vertex generation requires heightmap from terrain system.
    // Draw only if buffers are provided by the caller.
    if let (Some(vb), Some(ib)) = (vertex_buffer, index_buffer) {
        if index_count > 0 {
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..index_count, 0, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_volume_vertex_size() {
        assert_eq!(
            std::mem::size_of::<ShadowVolumeVertex>(),
            12, // 3 x f32
            "ShadowVolumeVertex must be 12 bytes"
        );
    }

    #[test]
    fn test_shadow_mask_vertex_size() {
        assert_eq!(
            std::mem::size_of::<ShadowMaskVertex>(),
            20, // 4 x f32 + u32
            "ShadowMaskVertex must be 20 bytes"
        );
    }

    #[test]
    fn test_occlusion_stencil_config_default() {
        let config = OcclusionStencilConfig::default();
        assert!(config.enabled);
        assert!(config.shadow_volumes_enabled);
        assert!(config.shadow_decals_enabled);
        assert_eq!(config.shadow_color, 0x7fa0a0a0);
    }

    #[test]
    fn test_occlusion_stencil_system_creation() {
        let system = OcclusionStencilSystem::new();
        assert!(!system.initialized);
        assert!(system.stencil_pipeline.is_none());
        assert!(system.mask_pipeline.is_none());
    }

    #[test]
    fn test_occlusion_stencil_system_not_ready_without_init() {
        let system = OcclusionStencilSystem::new();
        assert!(!system.is_ready());
    }

    #[test]
    fn test_shadow_volume_vertex_creation() {
        let v = ShadowVolumeVertex::new(1.0, 2.0, 3.0);
        assert_eq!(v.position, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_shadow_mask_vertex_creation() {
        let v = ShadowMaskVertex::new(1.0, 0.0, 0.0, 1.0, 0x7fa0a0a0);
        assert_eq!(v.position, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(v.color, 0x7fa0a0a0);
    }

    #[test]
    fn test_shadow_pass_result_default() {
        let result = ShadowPassResult::default();
        assert_eq!(result.num_rendered_shadows, 0);
        assert!(!result.has_shadows);
    }

    #[test]
    fn test_config_update_invalidates_pipelines() {
        let mut system = OcclusionStencilSystem::new();
        system.set_config(
            OcclusionStencilConfig {
                shadow_color: 0xff000000,
                ..Default::default()
            },
            None,
        );
        assert!(!system.initialized);
        assert_eq!(system.config.shadow_color, 0xff000000);
    }
}
