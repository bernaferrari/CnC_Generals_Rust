//! WGPU Shader Management
//!
//! This module handles shader creation, compilation, and management for WGPU,
//! equivalent to the DirectX8 shader functionality.

use crate::core::error::{Error, Result};
use bytemuck::cast_slice;
use naga::back::wgsl::WriterFlags;
use naga::{
    front::spv,
    valid::{Capabilities, ValidationFlags, Validator},
};
use std::sync::Arc;
use wgpu::{Device, ShaderModule, ShaderModuleDescriptor};

/// WGPU Shader wrapper
#[derive(Debug)]
pub struct WgpuShader {
    /// Vertex shader module
    vertex_module: Option<Arc<ShaderModule>>,
    /// Fragment shader module
    fragment_module: Option<Arc<ShaderModule>>,
    /// Compute shader module (for future use)
    compute_module: Option<Arc<ShaderModule>>,
    /// Shader label for debugging
    label: Option<String>,
    /// Reference count
    ref_count: std::sync::atomic::AtomicU32,
}

impl WgpuShader {
    /// Create shader from WGSL source
    pub fn from_wgsl(
        device: &Device,
        vertex_source: &str,
        fragment_source: Option<&str>,
        label: Option<&str>,
    ) -> Result<Self> {
        let vertex_label = label.map(|l| format!("{}_vertex", l));
        let vertex_module = device.create_shader_module(ShaderModuleDescriptor {
            label: vertex_label.as_deref(),
            source: wgpu::ShaderSource::Wgsl(vertex_source.into()),
        });

        let fragment_module = if let Some(frag_src) = fragment_source {
            let fragment_label = label.map(|l| format!("{}_fragment", l));
            Some(device.create_shader_module(ShaderModuleDescriptor {
                label: fragment_label.as_deref(),
                source: wgpu::ShaderSource::Wgsl(frag_src.into()),
            }))
        } else {
            None
        };

        Ok(Self {
            vertex_module: Some(Arc::new(vertex_module)),
            fragment_module: fragment_module.map(Arc::new),
            compute_module: None,
            label: label.map(|s| s.to_string()),
            ref_count: std::sync::atomic::AtomicU32::new(1),
        })
    }

    /// Create shader from SPIR-V binary
    pub fn from_spirv(
        device: &Device,
        vertex_spirv: &[u32],
        fragment_spirv: Option<&[u32]>,
        label: Option<&str>,
    ) -> Result<Self> {
        // Convert SPIR-V to WGSL using naga (WGPU's shader translator)
        let vertex_wgsl = Self::spirv_to_wgsl(vertex_spirv)?;
        let vertex_label = label.map(|l| format!("{}_vertex", l));
        let vertex_module = device.create_shader_module(ShaderModuleDescriptor {
            label: vertex_label.as_deref(),
            source: wgpu::ShaderSource::Wgsl(vertex_wgsl.into()),
        });

        let fragment_module = if let Some(frag_spirv) = fragment_spirv {
            let fragment_wgsl = Self::spirv_to_wgsl(frag_spirv)?;
            let fragment_label = label.map(|l| format!("{}_fragment", l));
            Some(device.create_shader_module(ShaderModuleDescriptor {
                label: fragment_label.as_deref(),
                source: wgpu::ShaderSource::Wgsl(fragment_wgsl.into()),
            }))
        } else {
            None
        };

        Ok(Self {
            vertex_module: Some(Arc::new(vertex_module)),
            fragment_module: fragment_module.map(Arc::new),
            compute_module: None,
            label: label.map(|s| s.to_string()),
            ref_count: std::sync::atomic::AtomicU32::new(1),
        })
    }

    /// Get vertex shader module
    pub fn vertex_module(&self) -> Option<&Arc<ShaderModule>> {
        self.vertex_module.as_ref()
    }

    /// Get fragment shader module
    pub fn fragment_module(&self) -> Option<&Arc<ShaderModule>> {
        self.fragment_module.as_ref()
    }

    /// Get compute shader module
    pub fn compute_module(&self) -> Option<&Arc<ShaderModule>> {
        self.compute_module.as_ref()
    }

    /// Get shader label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Add engine reference
    pub fn add_engine_ref(&self) {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Release engine reference
    pub fn release_engine_ref(&self) {
        let old_count = self
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        if old_count == 1 {
            // Shader will be dropped when this Arc goes out of scope
        }
    }

    /// Get reference count
    pub fn engine_ref_count(&self) -> u32 {
        self.ref_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Convert SPIR-V binary to WGSL using naga
    fn spirv_to_wgsl(spirv_data: &[u32]) -> Result<String> {
        let options = spv::Options {
            adjust_coordinate_space: true,
            strict_capabilities: false,
            block_ctx_dump_prefix: None,
        };

        let module = spv::parse_u8_slice(cast_slice(spirv_data), &options)
            .map_err(|err| Error::InvalidData(format!("SPIR-V parse error: {err}")))?;

        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        let module_info = validator
            .validate(&module)
            .map_err(|err| Error::InvalidData(format!("SPIR-V validation error: {err}")))?;

        let wgsl = naga::back::wgsl::write_string(&module, &module_info, WriterFlags::empty())
            .map_err(|err| Error::InvalidData(format!("WGSL write error: {err}")))?;

        Ok(wgsl)
    }
}

/// Shader manager for handling multiple shaders
pub struct WgpuShaderManager {
    /// Collection of managed shaders
    shaders: std::collections::HashMap<String, Arc<WgpuShader>>,
    /// Device reference
    device: Option<Arc<wgpu::Device>>,
}

impl WgpuShaderManager {
    /// Create new shader manager
    pub fn new() -> Self {
        Self {
            shaders: std::collections::HashMap::new(),
            device: None,
        }
    }

    /// Set device
    pub fn set_device(&mut self, device: Arc<wgpu::Device>) {
        self.device = Some(device);
    }

    /// Load shader from WGSL files
    pub fn load_wgsl_shader(
        &mut self,
        name: &str,
        vertex_path: &str,
        fragment_path: Option<&str>,
    ) -> Result<Arc<WgpuShader>> {
        let device = self.device.as_ref().ok_or_else(|| {
            Error::DeviceNotInitialized("WGPU device not initialized".to_string())
        })?;

        let vertex_source = std::fs::read_to_string(vertex_path)?;
        let fragment_source = if let Some(path) = fragment_path {
            Some(std::fs::read_to_string(path)?)
        } else {
            None
        };

        let shader = WgpuShader::from_wgsl(
            device,
            &vertex_source,
            fragment_source.as_deref(),
            Some(name),
        )?;

        let shader_arc = Arc::new(shader);
        self.shaders.insert(name.to_string(), shader_arc.clone());

        Ok(shader_arc)
    }

    /// Create shader from embedded WGSL source
    pub fn create_wgsl_shader(
        &mut self,
        name: &str,
        vertex_source: &str,
        fragment_source: Option<&str>,
    ) -> Result<Arc<WgpuShader>> {
        let device = self.device.as_ref().ok_or_else(|| {
            Error::DeviceNotInitialized("WGPU device not initialized".to_string())
        })?;

        let shader = WgpuShader::from_wgsl(device, vertex_source, fragment_source, Some(name))?;

        let shader_arc = Arc::new(shader);
        self.shaders.insert(name.to_string(), shader_arc.clone());

        Ok(shader_arc)
    }

    /// Get shader by name
    pub fn get_shader(&self, name: &str) -> Option<&Arc<WgpuShader>> {
        self.shaders.get(name)
    }

    /// Remove shader
    pub fn remove_shader(&mut self, name: &str) -> bool {
        self.shaders.remove(name).is_some()
    }

    /// Clear all shaders
    pub fn clear(&mut self) {
        self.shaders.clear();
    }

    /// Get shader count
    pub fn shader_count(&self) -> usize {
        self.shaders.len()
    }

    /// Cleanup resources
    pub fn cleanup(&mut self) {
        self.clear();
        self.device = None;
    }
}

impl Default for WgpuShaderManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Shader utilities for common shader operations
pub struct ShaderUtils;

impl ShaderUtils {
    /// Basic vertex shader source
    pub const BASIC_VERTEX_SHADER: &str = r#"
        struct VertexInput {
            @location(0) position: vec3<f32>,
            @location(1) tex_coords: vec2<f32>,
        };

        struct VertexOutput {
            @builtin(position) clip_position: vec4<f32>,
            @location(0) tex_coords: vec2<f32>,
        };

        @vertex
        fn main(
            model: VertexInput,
        ) -> VertexOutput {
            var out: VertexOutput;
            out.tex_coords = model.tex_coords;
            out.clip_position = vec4<f32>(model.position, 1.0);
            return out;
        }
    "#;

    /// Basic fragment shader source
    pub const BASIC_FRAGMENT_SHADER: &str = r#"
        @group(0) @binding(0)
        var t_diffuse: texture_2d<f32>;
        @group(0) @binding(1)
        var s_diffuse: sampler;

        @fragment
        fn main(in: VertexOutput) -> @location(0) vec4<f32> {
            return textureSample(t_diffuse, s_diffuse, in.tex_coords);
        }

        struct VertexOutput {
            @builtin(position) clip_position: vec4<f32>,
            @location(0) tex_coords: vec2<f32>,
        };
    "#;

    /// Create basic shader pipeline
    pub fn create_basic_pipeline(
        device: &Device,
        shader: &WgpuShader,
        texture_format: wgpu::TextureFormat,
    ) -> Result<wgpu::RenderPipeline> {
        let shader_module = shader.vertex_module().ok_or(Error::Generic(
            "Shader vertex module not available".to_string(),
        ))?;

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Basic Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        // Pre-create color targets to avoid lifetime issues
        let color_targets = [Some(wgpu::ColorTargetState {
            format: texture_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Basic Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 20, // 3 * 4 + 2 * 4 bytes
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: if let Some(frag_module) = shader.fragment_module() {
                Some(wgpu::FragmentState {
                    module: frag_module,
                    entry_point: Some("main"),
                    targets: &color_targets,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                })
            } else {
                None
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(pipeline)
    }
}

/// Shader constants and bindings
#[derive(Debug, Clone)]
pub struct ShaderConstants {
    /// Vertex shader constants
    pub vertex_constants: Vec<f32>,
    /// Pixel shader constants
    pub pixel_constants: Vec<f32>,
}

impl ShaderConstants {
    /// Create new shader constants
    pub fn new(vertex_count: usize, pixel_count: usize) -> Self {
        Self {
            vertex_constants: vec![0.0; vertex_count * 4], // 4 floats per constant
            pixel_constants: vec![0.0; pixel_count * 4],
        }
    }

    /// Set vertex shader constant
    pub fn set_vertex_constant(&mut self, index: usize, value: [f32; 4]) {
        let start = index * 4;
        if start + 4 <= self.vertex_constants.len() {
            self.vertex_constants[start..start + 4].copy_from_slice(&value);
        }
    }

    /// Set pixel shader constant
    pub fn set_pixel_constant(&mut self, index: usize, value: [f32; 4]) {
        let start = index * 4;
        if start + 4 <= self.pixel_constants.len() {
            self.pixel_constants[start..start + 4].copy_from_slice(&value);
        }
    }

    /// Get vertex constant
    pub fn get_vertex_constant(&self, index: usize) -> Option<[f32; 4]> {
        let start = index * 4;
        if start + 4 <= self.vertex_constants.len() {
            Some([
                self.vertex_constants[start],
                self.vertex_constants[start + 1],
                self.vertex_constants[start + 2],
                self.vertex_constants[start + 3],
            ])
        } else {
            None
        }
    }

    /// Get pixel constant
    pub fn get_pixel_constant(&self, index: usize) -> Option<[f32; 4]> {
        let start = index * 4;
        if start + 4 <= self.pixel_constants.len() {
            Some([
                self.pixel_constants[start],
                self.pixel_constants[start + 1],
                self.pixel_constants[start + 2],
                self.pixel_constants[start + 3],
            ])
        } else {
            None
        }
    }
}

impl Default for ShaderConstants {
    fn default() -> Self {
        Self::new(96, 8) // Default sizes matching original
    }
}
