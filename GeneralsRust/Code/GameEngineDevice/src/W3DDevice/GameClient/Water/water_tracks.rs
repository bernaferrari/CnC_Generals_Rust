//! Water Tracks (Wakes/Ripples) Module
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DWaterTracks.h
//! - GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWaterTracks.cpp
//!
//! This module implements animated water wakes and ripples from units and projectiles.

use wgpu::util::DeviceExt;
use super::water_config::WaveParameters;

/// Wave/track type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WaveType {
    /// Small ripple from projectile impact
    Ripple,
    /// Medium wake from small unit
    SmallWake,
    /// Large wake from large unit (ship, boat)
    LargeWake,
    /// Splash from explosion
    Splash,
}

/// Vertex format for water tracks
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WaterTrackVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl WaterTrackVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x2,
        2 => Float32x4,
    ];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<WaterTrackVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Individual water track/wake object
/// Matches C++ WaterTracksObj from W3DWaterTracks.h
pub struct WaterTrack {
    /// Type of wave
    wave_type: WaveType,

    /// Starting position of wave in world space
    start_pos: [f32; 2],
    /// Direction of wave travel (normalized)
    wave_dir: [f32; 2],
    /// Direction perpendicular to wave travel
    perp_dir: [f32; 2],

    /// Wave animation parameters
    params: WaveParameters,

    /// Elapsed time since wave creation (milliseconds)
    elapsed_ms: f32,

    /// Active state
    active: bool,
    /// Bound to owner (can accept new edges)
    bound: bool,

    /// Alpha for fade out
    alpha: f32,

    /// Texture filename
    texture_name: String,
}

impl WaterTrack {
    /// Create new water track
    /// Matches C++ WaterTracksObj::init()
    pub fn new(
        wave_type: WaveType,
        start: [f32; 2],
        end: [f32; 2],
        texture_name: String,
    ) -> Self {
        // Calculate wave direction and perpendicular
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let length = (dx * dx + dy * dy).sqrt();

        let wave_dir = if length > 0.001 {
            [dx / length, dy / length]
        } else {
            [1.0, 0.0]
        };

        let perp_dir = [-wave_dir[1], wave_dir[0]];

        // Get default parameters based on wave type
        let params = Self::get_wave_parameters(wave_type);

        Self {
            wave_type,
            start_pos: start,
            wave_dir,
            perp_dir,
            params,
            elapsed_ms: 0.0,
            active: true,
            bound: false,
            alpha: 1.0,
            texture_name,
        }
    }

    /// Get wave parameters based on type
    fn get_wave_parameters(wave_type: WaveType) -> WaveParameters {
        let mut params = WaveParameters::default();

        match wave_type {
            WaveType::Ripple => {
                params.initial_width = 1.0;
                params.initial_height = 0.2;
                params.final_width = 5.0;
                params.final_height = 0.5;
                params.initial_velocity = 0.15;
                params.wave_distance = 30.0;
                params.total_ms = 3000.0;
                params.fade_ms = 1000.0;
            }
            WaveType::SmallWake => {
                params.initial_width = 2.0;
                params.initial_height = 0.3;
                params.final_width = 8.0;
                params.final_height = 0.8;
                params.initial_velocity = 0.1;
                params.wave_distance = 50.0;
                params.total_ms = 5000.0;
                params.fade_ms = 1500.0;
            }
            WaveType::LargeWake => {
                params.initial_width = 4.0;
                params.initial_height = 0.5;
                params.final_width = 15.0;
                params.final_height = 1.2;
                params.initial_velocity = 0.08;
                params.wave_distance = 80.0;
                params.total_ms = 8000.0;
                params.fade_ms = 2000.0;
            }
            WaveType::Splash => {
                params.initial_width = 3.0;
                params.initial_height = 1.0;
                params.final_width = 12.0;
                params.final_height = 2.0;
                params.initial_velocity = 0.2;
                params.wave_distance = 40.0;
                params.total_ms = 2000.0;
                params.fade_ms = 800.0;
            }
        }

        params
    }

    /// Update animation state
    /// Returns true if track is still active
    /// Matches C++ WaterTracksObj::update()
    pub fn update(&mut self, delta_ms: f32) -> bool {
        if !self.active {
            return false;
        }

        self.elapsed_ms += delta_ms;

        // Check if animation complete
        if self.elapsed_ms >= self.params.total_ms {
            self.active = false;
            return false;
        }

        // Calculate fade alpha
        let fade_start = self.params.total_ms - self.params.fade_ms;
        if self.elapsed_ms > fade_start {
            let fade_progress = (self.elapsed_ms - fade_start) / self.params.fade_ms;
            self.alpha = 1.0 - fade_progress;
        } else {
            self.alpha = 1.0;
        }

        true
    }

    /// Generate vertices for this track
    /// Matches C++ WaterTracksObj::render()
    pub fn generate_vertices(&self, water_level: f32, strips_x: usize, strips_y: usize) -> Vec<WaterTrackVertex> {
        let mut vertices = Vec::with_capacity(strips_x * strips_y);

        // Calculate wave progress
        let progress = (self.elapsed_ms / self.params.total_ms).min(1.0);

        // Calculate current wave dimensions
        let width_progress = (progress / self.params.final_width_peak_frac).min(1.0);
        let current_width = self.params.initial_width +
            (self.params.final_width - self.params.initial_width) * width_progress;

        let current_height = self.params.initial_height +
            (self.params.final_height - self.params.initial_height) * progress;

        // Calculate wave front position
        let distance_traveled = self.params.wave_distance * progress;

        // Generate strip vertices
        for y in 0..strips_y {
            let v = y as f32 / (strips_y - 1) as f32;

            for x in 0..strips_x {
                let u = x as f32 / (strips_x - 1) as f32;

                // Calculate position along wave
                let along_wave = distance_traveled * u;
                let across_wave = (v - 0.5) * current_width;

                // Calculate world position
                let world_x = self.start_pos[0] + self.wave_dir[0] * along_wave + self.perp_dir[0] * across_wave;
                let world_y = self.start_pos[1] + self.wave_dir[1] * along_wave + self.perp_dir[1] * across_wave;

                // Calculate height using sine wave
                let wave_phase = u * std::f32::consts::PI;
                let height_offset = (wave_phase.sin() * current_height).max(0.0);

                // Edge falloff
                let edge_falloff = (1.0 - (v - 0.5).abs() * 2.0).max(0.0);
                let final_height = height_offset * edge_falloff;

                vertices.push(WaterTrackVertex {
                    position: [world_x, world_y, water_level + final_height],
                    uv: [u, v],
                    color: [1.0, 1.0, 1.0, self.alpha * edge_falloff],
                });
            }
        }

        vertices
    }

    /// Check if track is still active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get wave type
    pub fn get_wave_type(&self) -> WaveType {
        self.wave_type
    }
}

/// Water tracks rendering system
/// Manages multiple water tracks and renders them efficiently
/// Matches C++ WaterTracksRenderSystem from W3DWaterTracks.h
pub struct WaterTracksSystem {
    /// Active tracks being rendered
    tracks: Vec<WaterTrack>,

    /// Maximum number of active tracks
    max_tracks: usize,

    /// Vertex buffer for rendering
    vertex_buffer: wgpu::Buffer,

    /// Index buffer
    index_buffer: wgpu::Buffer,

    /// Render pipeline
    pipeline: wgpu::RenderPipeline,

    /// Bind group for textures
    bind_group: wgpu::BindGroup,

    /// Water level for positioning
    water_level: f32,

    /// Strip resolution
    strip_size_x: usize,
    strip_size_y: usize,

    /// Device and queue
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl WaterTracksSystem {
    /// Create new water tracks system
    /// Matches C++ WaterTracksRenderSystem::init()
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        water_level: f32,
        max_tracks: usize,
    ) -> Self {
        let strip_size_x = 16;
        let strip_size_y = 4;

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Water Track Shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                struct VertexInput {
                    @location(0) position: vec3<f32>,
                    @location(1) uv: vec2<f32>,
                    @location(2) color: vec4<f32>,
                }

                struct VertexOutput {
                    @builtin(position) position: vec4<f32>,
                    @location(0) uv: vec2<f32>,
                    @location(1) color: vec4<f32>,
                }

                @group(0) @binding(0)
                var<uniform> view_proj: mat4x4<f32>;

                @group(1) @binding(0)
                var track_texture: texture_2d<f32>;

                @group(1) @binding(1)
                var track_sampler: sampler;

                @vertex
                fn vs_main(input: VertexInput) -> VertexOutput {
                    var output: VertexOutput;
                    output.position = view_proj * vec4<f32>(input.position, 1.0);
                    output.uv = input.uv;
                    output.color = input.color;
                    return output;
                }

                @fragment
                fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
                    let tex_color = textureSample(track_texture, track_sampler, input.uv);
                    return tex_color * input.color;
                }
                "#.into()
            ),
        });

        // Create buffers
        let max_vertices = max_tracks * strip_size_x * strip_size_y;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Water Track Vertex Buffer"),
            size: (max_vertices * std::mem::size_of::<WaterTrackVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Generate indices for triangle strip
        let mut indices = Vec::new();
        for track in 0..max_tracks {
            let base = (track * strip_size_x * strip_size_y) as u16;
            for y in 0..(strip_size_y - 1) {
                for x in 0..(strip_size_x - 1) {
                    let top_left = base + (y * strip_size_x + x) as u16;
                    let top_right = base + (y * strip_size_x + x + 1) as u16;
                    let bottom_left = base + ((y + 1) * strip_size_x + x) as u16;
                    let bottom_right = base + ((y + 1) * strip_size_x + x + 1) as u16;

                    indices.extend_from_slice(&[
                        top_left, bottom_left, top_right,
                        top_right, bottom_left, bottom_right,
                    ]);
                }
            }
        }

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Track Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Water Track Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Water Track Texture Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Water Track Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Water Track Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[WaterTrackVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Create placeholder texture and bind group
        let texture = Self::create_white_texture(&device, &queue);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water Track Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&wgpu::TextureViewDescriptor::default())
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            tracks: Vec::new(),
            max_tracks,
            vertex_buffer,
            index_buffer,
            pipeline,
            bind_group,
            water_level,
            strip_size_x,
            strip_size_y,
            device,
            queue,
        }
    }

    /// Create white texture for default rendering
    fn create_white_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("White Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8; 4],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        texture
    }

    /// Add a new water track
    /// Matches C++ WaterTracksRenderSystem::bindTrack()
    pub fn add_track(
        &mut self,
        wave_type: WaveType,
        start: [f32; 2],
        end: [f32; 2],
        texture_name: String,
    ) {
        // Remove oldest track if at capacity
        if self.tracks.len() >= self.max_tracks {
            self.tracks.remove(0);
        }

        self.tracks.push(WaterTrack::new(wave_type, start, end, texture_name));
    }

    /// Update all tracks
    /// Matches C++ WaterTracksRenderSystem::update()
    pub fn update(&mut self, delta_ms: f32) {
        // Update all tracks and remove inactive ones
        self.tracks.retain_mut(|track| track.update(delta_ms));
    }

    /// Render all active tracks
    /// Matches C++ WaterTracksRenderSystem::flush()
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.tracks.is_empty() {
            return;
        }

        // Generate vertices for all active tracks
        let mut all_vertices = Vec::new();
        for track in &self.tracks {
            let vertices = track.generate_vertices(self.water_level, self.strip_size_x, self.strip_size_y);
            all_vertices.extend(vertices);
        }

        // Upload vertex data
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&all_vertices));

        // Render
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(1, &self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        let indices_per_track = (self.strip_size_x - 1) * (self.strip_size_y - 1) * 6;
        let total_indices = (self.tracks.len() * indices_per_track) as u32;
        render_pass.draw_indexed(0..total_indices, 0, 0..1);
    }

    /// Get number of active tracks
    pub fn active_count(&self) -> usize {
        self.tracks.len()
    }

    /// Clear all tracks
    pub fn clear(&mut self) {
        self.tracks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wave_parameters() {
        let params = WaterTrack::get_wave_parameters(WaveType::Ripple);
        assert!(params.initial_width < params.final_width);
        assert!(params.total_ms > 0.0);
    }

    #[test]
    fn test_track_creation() {
        let track = WaterTrack::new(
            WaveType::SmallWake,
            [0.0, 0.0],
            [10.0, 0.0],
            "wake.tga".to_string(),
        );

        assert!(track.is_active());
        assert_eq!(track.get_wave_type(), WaveType::SmallWake);
    }

    #[test]
    fn test_track_update() {
        let mut track = WaterTrack::new(
            WaveType::Ripple,
            [0.0, 0.0],
            [5.0, 0.0],
            "ripple.tga".to_string(),
        );

        // Update within lifetime
        assert!(track.update(100.0));
        assert!(track.is_active());

        // Update beyond lifetime
        track.elapsed_ms = 5000.0; // Exceed total_ms
        assert!(!track.update(100.0));
        assert!(!track.is_active());
    }

    #[test]
    fn test_vertex_generation() {
        let track = WaterTrack::new(
            WaveType::LargeWake,
            [0.0, 0.0],
            [20.0, 0.0],
            "wake.tga".to_string(),
        );

        let vertices = track.generate_vertices(0.0, 8, 4);
        assert_eq!(vertices.len(), 32); // 8 * 4
    }
}
