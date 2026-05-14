use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use wgpu::util::DeviceExt;

pub const DEFAULT_VALUE: u32 = 0;
pub const MAX_VALUE: u32 = 1000;
const DEFAULT_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: [f32; 4],
    pub size: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ParticleVertex {
    position: [f32; 3],
    color: [f32; 4],
    size: f32,
}

impl ParticleVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4, 2 => Float32];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ParticleUniforms {
    view_proj: [[f32; 4]; 4],
}

#[derive(Debug, Default)]
pub struct ParticleSystem {
    pub particles: Vec<Particle>,
    pub device: Option<wgpu::Device>,
    pub pipeline: Option<wgpu::RenderPipeline>,
    pub vertex_buffer: Option<wgpu::Buffer>,
    surface_format: Option<wgpu::TextureFormat>,
    uniform_buffer: Option<wgpu::Buffer>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    bind_group: Option<wgpu::BindGroup>,
}

pub type WthreeDParticleSys = ParticleSystem;

impl ParticleSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_device(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let mut system = Self::new();
        system.set_device(device, surface_format);
        system
    }

    pub fn set_device(&mut self, device: &wgpu::Device, surface_format: wgpu::TextureFormat) {
        self.device = Some(device.clone());
        self.surface_format = Some(surface_format);
    }

    pub fn emit_burst(
        &mut self,
        position: Vec3,
        count: u32,
        color: [f32; 4],
        lifetime: f32,
        speed: f32,
    ) {
        let lifetime = lifetime.max(0.001);
        let base_len = self.particles.len() as u32;

        for index in 0..count {
            let seed = base_len.wrapping_add(index).wrapping_add(1);
            let direction = pseudo_random_direction(seed);
            let size = 0.05 + pseudo_random(seed.wrapping_mul(31)) * 0.15;

            self.particles.push(Particle {
                position,
                velocity: direction * speed,
                color,
                size,
                lifetime,
                max_lifetime: lifetime,
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }

        for particle in &mut self.particles {
            particle.position += particle.velocity * dt;
            particle.lifetime -= dt;
        }

        self.particles.retain(|particle| particle.lifetime > 0.0);
    }

    pub fn render<'pass>(&'pass mut self, render_pass: &mut wgpu::RenderPass<'pass>) {
        self.render_with_camera(render_pass, None);
    }

    /// Render particles with an optional view-projection matrix.
    /// When `view_proj` is provided, particles are properly transformed through the camera.
    /// When `None`, particles render in screen-space (legacy behavior).
    pub fn render_with_camera<'pass>(
        &'pass mut self,
        render_pass: &mut wgpu::RenderPass<'pass>,
        view_proj: Option<&[[f32; 4]; 4]>,
    ) {
        if self.particles.is_empty() {
            self.vertex_buffer = None;
            return;
        }

        let Some(device) = self.device.as_ref() else {
            return;
        };

        let surface_format = self.surface_format.unwrap_or(DEFAULT_SURFACE_FORMAT);
        self.ensure_pipeline(device, surface_format);

        let Some(pipeline) = self.pipeline.as_ref() else {
            return;
        };

        // Upload view-projection uniform if provided
        if let Some(vp) = view_proj {
            self.ensure_uniforms(device, vp);
        }

        let vertices = self.build_vertex_data();
        if vertices.is_empty() {
            self.vertex_buffer = None;
            return;
        }

        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle System Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );

        let Some(vertex_buffer) = self.vertex_buffer.as_ref() else {
            return;
        };

        render_pass.set_pipeline(pipeline);

        if let Some(ref bg) = self.bind_group {
            render_pass.set_bind_group(0, bg, &[]);
        }

        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);
    }

    fn ensure_uniforms(&mut self, device: &wgpu::Device, view_proj: &[[f32; 4]; 4]) {
        let uniforms = ParticleUniforms {
            view_proj: *view_proj,
        };

        match (&self.uniform_buffer, &self.bind_group) {
            (Some(ub), Some(_)) => {
                device.queue().write_buffer(ub, 0, bytemuck::bytes_of(&uniforms));
            }
            _ => {
                let ub = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Uniforms"),
                    contents: bytemuck::bytes_of(&uniforms),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Particle Uniform Layout"),
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

                let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Particle Uniform Bind Group"),
                    layout: &bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: ub.as_entire_binding(),
                    }],
                });

                self.uniform_buffer = Some(ub);
                self.bind_group_layout = Some(bgl);
                self.bind_group = Some(bg);

                // Pipeline must be recreated with bind group layout
                self.pipeline = None;
            }
        }
    }

    fn ensure_pipeline(&mut self, device: &wgpu::Device, surface_format: wgpu::TextureFormat) {
        if self.pipeline.is_some() {
            return;
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle System Shader"),
            source: wgpu::ShaderSource::Wgsl(PARTICLE_SHADER.into()),
        });

        let has_camera = self.bind_group_layout.is_some();
        let layouts: Vec<&wgpu::BindGroupLayout> = if let Some(ref bgl) = self.bind_group_layout {
            vec![bgl]
        } else {
            vec![]
        };

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle System Pipeline Layout"),
            bind_group_layouts: &layouts,
            push_constant_ranges: &[],
        });

        let depth_stencil = Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        });

        let entry_point = if has_camera { "vs_camera" } else { "vs_screen" };

        self.pipeline = Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Particle System Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[ParticleVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            }),
        );
    }

    fn build_vertex_data(&self) -> Vec<ParticleVertex> {
        let mut vertices = Vec::with_capacity(self.particles.len() * 6);

        for particle in &self.particles {
            let half_size = particle.size * 0.5;
            let alpha =
                particle.color[3] * (particle.lifetime / particle.max_lifetime).clamp(0.0, 1.0);
            let color = [
                particle.color[0],
                particle.color[1],
                particle.color[2],
                alpha,
            ];

            let bottom_left = particle.position + Vec3::new(-half_size, -half_size, 0.0);
            let bottom_right = particle.position + Vec3::new(half_size, -half_size, 0.0);
            let top_left = particle.position + Vec3::new(-half_size, half_size, 0.0);
            let top_right = particle.position + Vec3::new(half_size, half_size, 0.0);

            vertices.extend_from_slice(&[
                ParticleVertex::new(bottom_left, color, particle.size),
                ParticleVertex::new(bottom_right, color, particle.size),
                ParticleVertex::new(top_right, color, particle.size),
                ParticleVertex::new(bottom_left, color, particle.size),
                ParticleVertex::new(top_right, color, particle.size),
                ParticleVertex::new(top_left, color, particle.size),
            ]);
        }

        vertices
    }
}

impl ParticleVertex {
    fn new(position: Vec3, color: [f32; 4], size: f32) -> Self {
        Self {
            position: position.to_array(),
            color,
            size,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WthreeDParticleSysType {
    Default = 0,
    Custom = 1,
    Special = 2,
}

fn pseudo_random(seed: u32) -> f32 {
    let value = ((seed as f32 * 12.9898).sin() * 43_758.547).fract();
    if value < 0.0 {
        value + 1.0
    } else {
        value
    }
}

fn pseudo_random_direction(seed: u32) -> Vec3 {
    let theta = pseudo_random(seed) * std::f32::consts::TAU;
    let z = pseudo_random(seed.wrapping_mul(17).wrapping_add(23)) * 2.0 - 1.0;
    let radial = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(radial * theta.cos(), radial * theta.sin(), z)
}

const PARTICLE_SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) pos: vec3f,
    @location(1) color: vec4f,
    @location(2) size: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
};

// Camera-transformed vertex shader: applies view-projection matrix
@vertex
fn vs_camera(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = uniforms.view_proj * vec4f(input.pos, 1.0);
    output.color = input.color;
    return output;
}

// Screen-space vertex shader: legacy fallback without camera transform
@vertex
fn vs_screen(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4f(input.pos.x, input.pos.y, input.pos.z, 1.0);
    output.color = input.color;
    return output;
}

@fragment
fn fs(@location(0) color: vec4f) -> @location(0) vec4f {
    return color;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_burst_creates_particles() {
        let mut system = ParticleSystem::new();
        system.emit_burst(Vec3::ZERO, 8, [1.0, 0.5, 0.25, 1.0], 2.0, 3.0);

        assert_eq!(system.particles.len(), 8);
        assert!(system
            .particles
            .iter()
            .all(|particle| particle.lifetime == 2.0));
    }

    #[test]
    fn update_removes_dead_particles() {
        let mut system = ParticleSystem::new();
        system.emit_burst(Vec3::ZERO, 4, [1.0; 4], 0.25, 1.0);

        system.update(0.5);

        assert!(system.particles.is_empty());
    }
}
