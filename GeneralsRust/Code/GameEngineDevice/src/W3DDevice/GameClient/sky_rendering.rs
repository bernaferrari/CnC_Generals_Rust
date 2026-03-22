// FILE: sky_rendering.rs
//
// Complete sky/sun rendering system for Generals Zero Hour Rust port
// Implements skybox rendering, sun/directional lighting, and day/night cycles
//
// C++ Reference: /GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWater.cpp (skybox)
//                /GeneralsMD/Code/GameEngine/Source/Common/GlobalData.cpp (lighting data)
//                /GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp (scene lighting)

use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Maximum number of global directional lights (sun, moon, fill lights)
/// C++ Reference: GlobalData.h line 32
pub const MAX_GLOBAL_LIGHTS: usize = 3;

/// Number of playable time-of-day slots used by terrain lighting.
/// The C++ enum also carries invalid/count sentinels, but the lighting arrays
/// only index the four actual periods.
pub const TIME_OF_DAY_COUNT: usize = 4;

/// Time of day enumeration matching C++ TIME_OF_DAY
/// C++ Reference: GameType.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TimeOfDay {
    Invalid = 0,
    Morning = 1,
    Afternoon = 2,
    Evening = 3,
    Night = 4,
    Count = 5,
}

impl TimeOfDay {
    pub fn count() -> usize {
        TIME_OF_DAY_COUNT
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(TimeOfDay::Morning),
            1 => Some(TimeOfDay::Afternoon),
            2 => Some(TimeOfDay::Evening),
            3 => Some(TimeOfDay::Night),
            _ => None,
        }
    }

    pub fn array_index(self) -> Option<usize> {
        match self {
            TimeOfDay::Morning => Some(0),
            TimeOfDay::Afternoon => Some(1),
            TimeOfDay::Evening => Some(2),
            TimeOfDay::Night => Some(3),
            _ => None,
        }
    }

    /// Interpolation factor [0,1] between this time and next
    pub fn interpolation_factor(time_progress: f32) -> (TimeOfDay, TimeOfDay, f32) {
        // Time progress is [0,1] representing time through the day.
        let scaled = time_progress.rem_euclid(1.0) * TIME_OF_DAY_COUNT as f32;
        let index = scaled.floor() as usize;
        let t = scaled - index as f32;

        let current = Self::from_index(index).unwrap_or(TimeOfDay::Morning);
        let next = Self::from_index((index + 1) % TIME_OF_DAY_COUNT).unwrap_or(TimeOfDay::Afternoon);

        (current, next, t)
    }
}

/// Terrain lighting configuration for a single time of day
/// C++ Reference: GlobalData.h lines 58-63 (TerrainLighting struct)
#[derive(Debug, Clone, Copy)]
pub struct TerrainLighting {
    pub ambient: [f32; 3],   // RGB ambient color
    pub diffuse: [f32; 3],   // RGB diffuse color
    pub light_pos: [f32; 3], // Light direction (normalized)
}

impl Default for TerrainLighting {
    fn default() -> Self {
        Self {
            ambient: [0.0, 0.0, 0.0],
            diffuse: [0.0, 0.0, 0.0],
            light_pos: [0.0, 0.0, -1.0], // Matches GlobalData.cpp default
        }
    }
}

impl TerrainLighting {
    /// Interpolate between two lighting configurations
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let lerp_vec3 = |a: [f32; 3], b: [f32; 3], t: f32| -> [f32; 3] {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };

        Self {
            ambient: lerp_vec3(self.ambient, other.ambient, t),
            diffuse: lerp_vec3(self.diffuse, other.diffuse, t),
            light_pos: lerp_vec3(self.light_pos, other.light_pos, t),
        }
    }

    /// Normalize light position to direction vector
    pub fn get_direction(&self) -> [f32; 3] {
        let len = (self.light_pos[0] * self.light_pos[0] +
                   self.light_pos[1] * self.light_pos[1] +
                   self.light_pos[2] * self.light_pos[2]).sqrt();
        if len > 0.0001 {
            [
                self.light_pos[0] / len,
                self.light_pos[1] / len,
                self.light_pos[2] / len,
            ]
        } else {
            [0.0, 0.0, -1.0] // Matches GlobalData.cpp default light direction
        }
    }
}

/// Global lighting configuration
/// C++ Reference: GlobalData.h lines 190-192 (terrain lighting arrays)
#[derive(Debug, Clone)]
pub struct GlobalLightingConfig {
    /// Terrain lighting per time of day per light
    pub terrain_lighting: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],

    /// Object lighting per time of day per light
    pub terrain_objects_lighting: [[TerrainLighting; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],

    /// Number of active global lights (1-3)
    pub num_global_lights: usize,

    /// Infantry light intensity scale per time of day
    pub infantry_light_scale: [f32; 4],
}

impl Default for GlobalLightingConfig {
    fn default() -> Self {
        // Default lighting matches C++ GlobalData.cpp initialization.
        let default_light = TerrainLighting::default();

        Self {
            terrain_lighting: [[default_light; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
            terrain_objects_lighting: [[default_light; MAX_GLOBAL_LIGHTS]; TIME_OF_DAY_COUNT],
            num_global_lights: MAX_GLOBAL_LIGHTS,
            infantry_light_scale: [1.5; TIME_OF_DAY_COUNT],
        }
    }
}

impl GlobalLightingConfig {
    /// Get interpolated lighting for current time of day
    pub fn get_lighting_for_time(&self, time_progress: f32, is_object: bool) -> Vec<TerrainLighting> {
        let (tod1, tod2, t) = TimeOfDay::interpolation_factor(time_progress);
        let light_count = self.num_global_lights.min(MAX_GLOBAL_LIGHTS);

        let lighting_array = if is_object {
            &self.terrain_objects_lighting
        } else {
            &self.terrain_lighting
        };

        let lights1 = &lighting_array[tod1.array_index().unwrap_or(0)];
        let lights2 = &lighting_array[tod2.array_index().unwrap_or(0)];

        (0..light_count)
            .map(|i| lights1[i].lerp(&lights2[i], t))
            .collect()
    }
}

/// Skybox rendering vertex
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyboxVertex {
    position: [f32; 3],
}

impl SkyboxVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkyboxVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Skybox uniform data for shaders
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct SkyboxUniforms {
    view_proj: [[f32; 4]; 4], // View-projection matrix (camera centered)
    tint_color: [f32; 4],      // Sky tint based on time of day
}

/// Complete sky rendering system
/// C++ Reference: W3DWater.cpp lines 1071-1089 (skybox creation and setup)
pub struct SkyRenderingSystem {
    // WGPU resources
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    // Skybox rendering
    skybox_vertex_buffer: wgpu::Buffer,
    skybox_index_buffer: wgpu::Buffer,
    skybox_index_count: u32,
    skybox_pipeline: wgpu::RenderPipeline,
    skybox_bind_group_layout: wgpu::BindGroupLayout,
    skybox_bind_group: Option<wgpu::BindGroup>,
    skybox_uniform_buffer: wgpu::Buffer,

    // Skybox texture (cubemap)
    skybox_texture: Option<wgpu::Texture>,
    skybox_sampler: wgpu::Sampler,

    // Lighting state
    lighting_config: GlobalLightingConfig,

    // Sky configuration
    sky_box_scale: f32,        // Scale of skybox (from GlobalData)
    sky_box_position_z: f32,   // Z offset for skybox positioning
    draw_sky_box: bool,        // Enable/disable skybox rendering

    // Day/night cycle
    time_of_day_progress: f32, // [0,1] representing time through day (0.25=afternoon start)
    auto_cycle_enabled: bool,  // Auto-advance time
    cycle_speed: f32,          // Seconds per full day cycle
}

impl SkyRenderingSystem {
    /// Create new sky rendering system
    /// C++ Reference: W3DWater.cpp WaterRenderObjClass::init() lines 1071-1089
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        // Create skybox cube mesh
        // C++ creates skybox from "new_skybox" W3D model
        let (vertices, indices) = Self::create_skybox_cube();

        let skybox_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skybox Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let skybox_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skybox Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let skybox_index_count = indices.len() as u32;

        // Create uniform buffer
        let skybox_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Skybox Uniform Buffer"),
            size: std::mem::size_of::<SkyboxUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sampler with clamping (to reduce corner seams)
        // C++ Reference: W3DWater.cpp lines 1083-1087 (texture clamping)
        let skybox_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Skybox Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create bind group layout
        let skybox_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skybox Bind Group Layout"),
            entries: &[
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Cubemap texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create shader and pipeline
        let skybox_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/skybox.wgsl").into()),
        });

        let skybox_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox Pipeline Layout"),
            bind_group_layouts: &[&skybox_bind_group_layout],
            push_constant_ranges: &[],
        });

        let skybox_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Pipeline"),
            layout: Some(&skybox_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skybox_shader,
                entry_point: "vs_main",
                buffers: &[SkyboxVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &skybox_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None, // Opaque skybox
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Render both sides (inside of cube)
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Don't write depth for skybox
                depth_compare: wgpu::CompareFunction::LessEqual, // Draw at far plane
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            device,
            queue,
            skybox_vertex_buffer,
            skybox_index_buffer,
            skybox_index_count,
            skybox_pipeline,
            skybox_bind_group_layout,
            skybox_bind_group: None,
            skybox_uniform_buffer,
            skybox_texture: None,
            skybox_sampler,
            lighting_config: GlobalLightingConfig::default(),
            sky_box_scale: 4.5, // C++ default from GlobalData.cpp line 657
            sky_box_position_z: 0.0,
            draw_sky_box: false,
            time_of_day_progress: 0.25,
            auto_cycle_enabled: false,
            cycle_speed: 1200.0, // 20 minutes per full cycle
        }
    }

    /// Create skybox cube vertices and indices
    /// Returns vertices for a unit cube centered at origin
    fn create_skybox_cube() -> (Vec<SkyboxVertex>, Vec<u16>) {
        // Unit cube vertices
        let vertices = vec![
            // Front face (Z+)
            SkyboxVertex { position: [-1.0, -1.0,  1.0] },
            SkyboxVertex { position: [ 1.0, -1.0,  1.0] },
            SkyboxVertex { position: [ 1.0,  1.0,  1.0] },
            SkyboxVertex { position: [-1.0,  1.0,  1.0] },
            // Back face (Z-)
            SkyboxVertex { position: [-1.0, -1.0, -1.0] },
            SkyboxVertex { position: [-1.0,  1.0, -1.0] },
            SkyboxVertex { position: [ 1.0,  1.0, -1.0] },
            SkyboxVertex { position: [ 1.0, -1.0, -1.0] },
            // Top face (Y+)
            SkyboxVertex { position: [-1.0,  1.0, -1.0] },
            SkyboxVertex { position: [-1.0,  1.0,  1.0] },
            SkyboxVertex { position: [ 1.0,  1.0,  1.0] },
            SkyboxVertex { position: [ 1.0,  1.0, -1.0] },
            // Bottom face (Y-)
            SkyboxVertex { position: [-1.0, -1.0, -1.0] },
            SkyboxVertex { position: [ 1.0, -1.0, -1.0] },
            SkyboxVertex { position: [ 1.0, -1.0,  1.0] },
            SkyboxVertex { position: [-1.0, -1.0,  1.0] },
            // Right face (X+)
            SkyboxVertex { position: [ 1.0, -1.0, -1.0] },
            SkyboxVertex { position: [ 1.0,  1.0, -1.0] },
            SkyboxVertex { position: [ 1.0,  1.0,  1.0] },
            SkyboxVertex { position: [ 1.0, -1.0,  1.0] },
            // Left face (X-)
            SkyboxVertex { position: [-1.0, -1.0, -1.0] },
            SkyboxVertex { position: [-1.0, -1.0,  1.0] },
            SkyboxVertex { position: [-1.0,  1.0,  1.0] },
            SkyboxVertex { position: [-1.0,  1.0, -1.0] },
        ];

        // Indices for 6 faces * 2 triangles * 3 vertices
        let indices = vec![
            0, 1, 2,  2, 3, 0,   // Front
            4, 5, 6,  6, 7, 4,   // Back
            8, 9, 10, 10, 11, 8, // Top
            12, 13, 14, 14, 15, 12, // Bottom
            16, 17, 18, 18, 19, 16, // Right
            20, 21, 22, 22, 23, 20, // Left
        ];

        (vertices, indices)
    }

    /// Load skybox texture from cubemap faces
    /// C++ Reference: W3DWater.cpp line 1071 (loads "new_skybox" model)
    pub fn load_skybox_texture(
        &mut self,
        positive_x: &[u8], // Right
        negative_x: &[u8], // Left
        positive_y: &[u8], // Top
        negative_y: &[u8], // Bottom
        positive_z: &[u8], // Front
        negative_z: &[u8], // Back
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 6,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Skybox Cubemap"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload each face
        let faces = [positive_x, negative_x, positive_y, negative_y, positive_z, negative_z];
        for (i, face_data) in faces.iter().enumerate() {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: i as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                face_data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * width),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });

        // Create bind group
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Bind Group"),
            layout: &self.skybox_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.skybox_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.skybox_sampler),
                },
            ],
        });

        self.skybox_texture = Some(texture);
        self.skybox_bind_group = Some(bind_group);

        Ok(())
    }

    /// Update lighting configuration
    pub fn set_lighting_config(&mut self, config: GlobalLightingConfig) {
        self.lighting_config = config;
    }

    /// Set skybox parameters from GlobalData
    /// C++ Reference: GlobalData.cpp lines 655-657
    pub fn set_skybox_config(&mut self, scale: f32, position_z: f32, draw: bool) {
        self.sky_box_scale = scale;
        self.sky_box_position_z = position_z;
        self.draw_sky_box = draw;
    }

    /// Update day/night cycle
    pub fn update(&mut self, delta_time: f32) {
        if self.auto_cycle_enabled && self.cycle_speed > 0.0 {
            self.time_of_day_progress += delta_time / self.cycle_speed;
            self.time_of_day_progress = self.time_of_day_progress.fract(); // Wrap [0,1]
        }
    }

    /// Get current global lights interpolated for time of day
    pub fn get_current_global_lights(&self, is_object: bool) -> Vec<TerrainLighting> {
        self.lighting_config.get_lighting_for_time(self.time_of_day_progress, is_object)
    }

    /// Get sun direction for current time
    pub fn get_sun_direction(&self) -> [f32; 3] {
        let lights = self.get_current_global_lights(false);
        if !lights.is_empty() {
            lights[0].get_direction()
        } else {
            [0.0, -1.0, 0.0]
        }
    }

    /// Render skybox
    /// C++ Reference: W3DWater.cpp lines 1702-1708 (skybox rendering)
    pub fn render_skybox<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        view_matrix: &[[f32; 4]; 4],
        proj_matrix: &[[f32; 4]; 4],
        camera_position: [f32; 3],
    ) {
        if !self.draw_sky_box {
            return;
        }

        if self.skybox_bind_group.is_none() {
            return; // No texture loaded
        }

        // Create camera-centered view matrix (remove translation)
        // C++ Reference: W3DWater.cpp lines 1704-1706 (centers skybox at camera)
        let mut view_centered = *view_matrix;
        view_centered[3][0] = 0.0;
        view_centered[3][1] = 0.0;
        view_centered[3][2] = self.sky_box_position_z;

        // Compute view-projection matrix
        let view_proj = multiply_matrices(&view_centered, proj_matrix);

        // Compute sky tint based on time of day
        let lights = self.get_current_global_lights(false);
        let tint_color = if !lights.is_empty() {
            let ambient = lights[0].ambient;
            [ambient[0], ambient[1], ambient[2], 1.0]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        // Update uniforms
        let uniforms = SkyboxUniforms {
            view_proj,
            tint_color,
        };

        self.queue.write_buffer(
            &self.skybox_uniform_buffer,
            0,
            bytemuck::cast_slice(&[uniforms]),
        );

        // Render skybox
        render_pass.set_pipeline(&self.skybox_pipeline);
        render_pass.set_bind_group(0, self.skybox_bind_group.as_ref().unwrap(), &[]);
        render_pass.set_vertex_buffer(0, self.skybox_vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.skybox_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..self.skybox_index_count, 0, 0..1);
    }

    /// Set time of day progress [0,1]
    pub fn set_time_of_day_progress(&mut self, progress: f32) {
        self.time_of_day_progress = progress.clamp(0.0, 1.0);
    }

    /// Enable/disable automatic day/night cycling
    pub fn set_auto_cycle(&mut self, enabled: bool, cycle_duration_seconds: f32) {
        self.auto_cycle_enabled = enabled;
        self.cycle_speed = cycle_duration_seconds;
    }
}

/// Matrix multiplication helper
fn multiply_matrices(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_of_day_interpolation() {
        // Morning start
        let (tod1, tod2, t) = TimeOfDay::interpolation_factor(0.0);
        assert_eq!(tod1, TimeOfDay::Morning);
        assert_eq!(tod2, TimeOfDay::Afternoon);
        assert_eq!(t, 0.0);

        // Mid-morning to afternoon
        let (tod1, tod2, t) = TimeOfDay::interpolation_factor(0.125);
        assert_eq!(tod1, TimeOfDay::Morning);
        assert_eq!(tod2, TimeOfDay::Afternoon);
        assert!(t > 0.4 && t < 0.6);

        // Evening
        let (tod1, tod2, t) = TimeOfDay::interpolation_factor(0.5);
        assert_eq!(tod1, TimeOfDay::Evening);

        // Night to morning wrap
        let (tod1, tod2, t) = TimeOfDay::interpolation_factor(0.9);
        assert_eq!(tod1, TimeOfDay::Night);
        assert_eq!(tod2, TimeOfDay::Morning);
    }

    #[test]
    fn test_time_of_day_enum_matches_cplusplus_order() {
        assert_eq!(TimeOfDay::Invalid as u32, 0);
        assert_eq!(TimeOfDay::Morning as u32, 1);
        assert_eq!(TimeOfDay::Afternoon as u32, 2);
        assert_eq!(TimeOfDay::Evening as u32, 3);
        assert_eq!(TimeOfDay::Night as u32, 4);
        assert_eq!(TimeOfDay::Count as u32, 5);

        assert_eq!(TimeOfDay::Morning.array_index(), Some(0));
        assert_eq!(TimeOfDay::Night.array_index(), Some(3));
        assert_eq!(TimeOfDay::Count.array_index(), None);
    }

    #[test]
    fn test_terrain_lighting_lerp() {
        let morning = TerrainLighting {
            ambient: [0.5, 0.6, 0.7],
            diffuse: [1.0, 1.0, 0.8],
            light_pos: [0.0, -1.0, 0.0],
        };

        let afternoon = TerrainLighting {
            ambient: [0.8, 0.9, 1.0],
            diffuse: [1.0, 1.0, 1.0],
            light_pos: [0.5, -0.7, 0.0],
        };

        let mid = morning.lerp(&afternoon, 0.5);
        assert!((mid.ambient[0] - 0.65).abs() < 0.01);
        assert!((mid.diffuse[2] - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_skybox_cube_generation() {
        let (vertices, indices) = SkyRenderingSystem::create_skybox_cube();

        // Should have 24 vertices (4 per face * 6 faces)
        assert_eq!(vertices.len(), 24);

        // Should have 36 indices (6 triangles * 6 faces)
        assert_eq!(indices.len(), 36);

        // All vertices should be at unit cube bounds
        for v in &vertices {
            assert!(v.position[0].abs() == 1.0);
            assert!(v.position[1].abs() == 1.0);
            assert!(v.position[2].abs() == 1.0);
        }
    }

    #[test]
    fn test_lighting_config_default() {
        let config = GlobalLightingConfig::default();
        assert_eq!(config.num_global_lights, MAX_GLOBAL_LIGHTS);
        assert_eq!(config.infantry_light_scale.len(), 4);
        assert_eq!(config.infantry_light_scale, [1.5, 1.5, 1.5, 1.5]);
        assert_eq!(config.terrain_lighting[0][0].ambient, [0.0, 0.0, 0.0]);
        assert_eq!(config.terrain_lighting[0][0].diffuse, [0.0, 0.0, 0.0]);
        assert_eq!(config.terrain_lighting[0][0].light_pos, [0.0, 0.0, -1.0]);

        // Verify all time-of-day entries exist
        for tod in 0..4 {
            for light in 0..MAX_GLOBAL_LIGHTS {
                let tl = &config.terrain_lighting[tod][light];
                // Should have valid default values
                assert!(tl.ambient[0] >= 0.0 && tl.ambient[0] <= 1.0);
            }
        }
    }
}
