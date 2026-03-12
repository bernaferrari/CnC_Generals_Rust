//! Helpers to build and cache bind groups for camera/model/bones/textures
use crate::core::error::Result;
use crate::material_system::MaterialPassClass;
use crate::render_object_system::{FogSettings, RenderInfoClass};
use crate::rendering::frame_uniform_arena::FrameUniformArena;
use crate::rendering::lighting_system::{LightClass, LightType};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use ww3d_gpu::device::GpuDevice;

#[derive(Debug, Clone)]
pub struct CameraBinds {
    pub buffer: Arc<wgpu::Buffer>,
    pub bind_group: Arc<wgpu::BindGroup>,
}

#[derive(Debug, Clone)]
pub struct ModelBinds {
    pub model_buffer: Arc<wgpu::Buffer>,
    pub lighting_buffer: Arc<wgpu::Buffer>,
    pub bind_group: Arc<wgpu::BindGroup>,
}

#[derive(Debug, Clone)]
pub struct BonesBinds {
    pub buffer: Arc<wgpu::Buffer>,
    pub bind_group: Arc<wgpu::BindGroup>,
}

#[derive(Debug, Clone)]
pub struct TextureBinds {
    pub bind_group: Arc<wgpu::BindGroup>,
}

#[derive(Debug, Clone)]
pub struct ColorBinds {
    pub bind_group: Arc<wgpu::BindGroup>,
}

#[derive(Debug, Clone)]
pub struct UVTransformBinds {
    pub buffer: Arc<wgpu::Buffer>,
    pub bind_group: Arc<wgpu::BindGroup>,
}

#[derive(Debug, Clone)]
pub struct SkinnedGroup2Binds {
    pub bones_buffer: Arc<wgpu::Buffer>,
    pub uv_transform_buffer: Arc<wgpu::Buffer>,
    pub bind_group: Arc<wgpu::BindGroup>,
}

pub struct WgpuMaterialBinds;

const MAX_LIGHTS: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct CameraUniform {
    view_proj: Mat4,
    view: Mat4,
    projection: Mat4,
    eye_position: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ModelUniform {
    model: Mat4,
    normal_matrix: Mat4,
    texture_stage_mask: [u32; 4],
    texture_stage_uv_map: [u32; 4],
    material_diffuse: [f32; 4],
    material_specular: [f32; 4],
    material_emissive: [f32; 4],
    material_overrides: [f32; 4],
    // Fog-of-war visibility fields (Week 7 rendering integration)
    visibility_alpha: f32,   // 0.0 (hidden) to 1.0 (fully visible)
    visibility_falloff: f32, // Gradient strength for smooth transitions
    is_explored: f32,        // 1.0 = explored territory, 0.0 = unexplored
    visibility_pad: f32,     // Padding for alignment (12-byte boundary)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PackedLight {
    direction: [f32; 4],
    color: [f32; 4],
    position_range: [f32; 4],
    spot_params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct LightingUniform {
    ambient_color: [f32; 4],
    fog_color: [f32; 4],
    fog_params: [f32; 4],
    light_meta: [f32; 4],
    lights: [PackedLight; MAX_LIGHTS],
}

/// UV Texture Transform Uniform - GPU-side representation for texture coordinate transforms
/// Maps to WGSL UVTransformUniform struct in shaders
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct UVTransformUniform {
    /// Mapper metadata packed into one vec4-equivalent for std140-compatible uniform layout.
    /// x = mapper type ID (0=UV, 4=LinearOffset, 7=Grid, 8=Rotate, 9=SineLinearOffset, etc.)
    pub mapper_meta: [u32; 4],
    /// Generic integer arguments for mapper-specific parameters
    pub mapper_args: [i32; 4],
    /// Float arguments for advanced mapper control
    pub mapper_float_args: [f32; 4],
    /// Animation parameters packed into vec4-equivalent.
    /// x = current animation time in seconds.
    pub animation: [f32; 4],
}

impl Default for UVTransformUniform {
    fn default() -> Self {
        Self {
            mapper_meta: [0, 0, 0, 0], // UV mapper (pass-through)
            mapper_args: [0, 0, 0, 0],
            mapper_float_args: [1.0, 1.0, 0.0, 0.0],
            animation: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

impl WgpuMaterialBinds {
    pub fn camera(
        gpu: &GpuDevice,
        pipeline: &wgpu::RenderPipeline,
        slot: u32,
        arena: &mut FrameUniformArena,
        render_info: &RenderInfoClass,
    ) -> Result<CameraBinds> {
        let camera = &render_info.camera;
        let view_proj = camera.get_cached_view_projection_matrix();
        let view = camera.get_cached_view_matrix();
        let projection = camera.get_cached_projection_matrix();
        let eye = camera.get_position();

        let uniform = CameraUniform {
            view_proj,
            view,
            projection,
            eye_position: [eye.x, eye.y, eye.z, 1.0],
        };

        let slice = arena.allocate(gpu, bytemuck::bytes_of(&uniform), 256)?;
        let bind_group = gpu
            .wgpu_device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Camera BG"),
                layout: &pipeline.get_bind_group_layout(slot),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(slice.as_binding()),
                }],
            });
        Ok(CameraBinds {
            buffer: Arc::clone(&slice.buffer),
            bind_group: Arc::new(bind_group),
        })
    }

    pub fn model(
        gpu: &GpuDevice,
        pipeline: &wgpu::RenderPipeline,
        slot: u32,
        model: &Mat4,
        render_info: &RenderInfoClass,
        texture_stage_mask: u8,
        cube_stage_mask: u32,
        texture_stage_hints: u32,
        texture_alpha_mask: u32,
        texture_stage_uv_bits: u32,
        material_diffuse: [f32; 4],
        material_specular: [f32; 4],
        material_emissive: [f32; 4],
        material_overrides: [f32; 4],
        arena: &mut FrameUniformArena,
        // FOW visibility parameters (optional)
        visibility_alpha: Option<f32>,
        visibility_falloff: Option<f32>,
        is_explored: Option<f32>,
    ) -> Result<ModelBinds> {
        let normal_matrix = model.inverse().transpose();
        let stage_mask_u32 = texture_stage_mask as u32;
        let cube_mask_u32 = cube_stage_mask;
        let stage_info = [
            stage_mask_u32,
            texture_stage_hints,
            texture_alpha_mask,
            cube_mask_u32,
        ];
        let model_uniform = ModelUniform {
            model: *model,
            normal_matrix,
            texture_stage_mask: stage_info,
            texture_stage_uv_map: [texture_stage_uv_bits, 0, 0, 0],
            material_diffuse,
            material_specular,
            material_emissive,
            material_overrides,
            // Use provided FOW visibility values or defaults
            visibility_alpha: visibility_alpha.unwrap_or(1.0),
            visibility_falloff: visibility_falloff.unwrap_or(1.0),
            is_explored: is_explored.unwrap_or(1.0),
            visibility_pad: 0.0,
        };

        let lighting_uniform = build_lighting_uniform(render_info);

        let model_slice = arena.allocate(gpu, bytemuck::bytes_of(&model_uniform), 256)?;
        let lighting_slice = arena.allocate(gpu, bytemuck::bytes_of(&lighting_uniform), 256)?;

        let bind_group = gpu
            .wgpu_device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Model BG"),
                layout: &pipeline.get_bind_group_layout(slot),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(model_slice.as_binding()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(lighting_slice.as_binding()),
                    },
                ],
            });

        Ok(ModelBinds {
            model_buffer: Arc::clone(&model_slice.buffer),
            lighting_buffer: Arc::clone(&lighting_slice.buffer),
            bind_group: Arc::new(bind_group),
        })
    }

    pub fn bones(
        gpu: &GpuDevice,
        pipeline: &wgpu::RenderPipeline,
        slot: u32,
        bones: &[Mat4],
        arena: &mut FrameUniformArena,
    ) -> Result<BonesBinds> {
        let slice = arena.allocate(gpu, bytemuck::cast_slice(bones), 256)?;
        let bind_group = gpu
            .wgpu_device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bones BG"),
                layout: &pipeline.get_bind_group_layout(slot),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(slice.as_binding()),
                }],
            });
        Ok(BonesBinds {
            buffer: Arc::clone(&slice.buffer),
            bind_group: Arc::new(bind_group),
        })
    }

    pub fn skinned_group2(
        gpu: &GpuDevice,
        pipeline: &wgpu::RenderPipeline,
        slot: u32,
        bones: &[Mat4],
        material_pass: Option<&MaterialPassClass>,
        animation_time: f32,
        arena: &mut FrameUniformArena,
    ) -> Result<SkinnedGroup2Binds> {
        let bones_slice = arena.allocate(gpu, bytemuck::cast_slice(bones), 256)?;
        let uv_uniform = build_uv_transform_uniform(material_pass, animation_time);
        let uv_slice = arena.allocate(gpu, bytemuck::bytes_of(&uv_uniform), 256)?;

        let bind_group = gpu
            .wgpu_device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Skinned Group2 BG"),
                layout: &pipeline.get_bind_group_layout(slot),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(bones_slice.as_binding()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(uv_slice.as_binding()),
                    },
                ],
            });

        Ok(SkinnedGroup2Binds {
            bones_buffer: Arc::clone(&bones_slice.buffer),
            uv_transform_buffer: Arc::clone(&uv_slice.buffer),
            bind_group: Arc::new(bind_group),
        })
    }

    pub fn vertex_colors(
        device: &wgpu::Device,
        pipeline: &wgpu::RenderPipeline,
        slot: u32,
        diffuse_buffer: &wgpu::Buffer,
        illumination_buffer: &wgpu::Buffer,
    ) -> ColorBinds {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Vertex Colors BG"),
            layout: &pipeline.get_bind_group_layout(slot),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: diffuse_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: illumination_buffer.as_entire_binding(),
                },
            ],
        });
        ColorBinds {
            bind_group: Arc::new(bind_group),
        }
    }

    /// Create UV transform bind group from material pass mapper data
    /// This enables animated texture mappers to work in the GPU shader
    pub fn uv_transform(
        device: &wgpu::Device,
        pipeline: &wgpu::RenderPipeline,
        slot: u32,
        material_pass: Option<&MaterialPassClass>,
        animation_time: f32,
    ) -> Result<UVTransformBinds> {
        let uniform = build_uv_transform_uniform(material_pass, animation_time);

        let buffer = Arc::new(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("UV Transform Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }),
        );

        let bind_group = Arc::new(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UV Transform Bind Group"),
            layout: &pipeline.get_bind_group_layout(slot),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        }));

        Ok(UVTransformBinds { buffer, bind_group })
    }
}

fn build_uv_transform_uniform(
    material_pass: Option<&MaterialPassClass>,
    animation_time: f32,
) -> UVTransformUniform {
    if let Some(pass) = material_pass {
        UVTransformUniform {
            mapper_meta: [pass.get_mapper_id(), 0, 0, 0],
            mapper_args: [
                pass.get_mapper_arg(0),
                pass.get_mapper_arg(1),
                pass.get_mapper_arg(2),
                pass.get_mapper_arg(3),
            ],
            mapper_float_args: pass.mapper_float_args(),
            animation: [animation_time, 0.0, 0.0, 0.0],
        }
    } else {
        UVTransformUniform::default()
    }
}

fn build_lighting_uniform(render_info: &RenderInfoClass) -> LightingUniform {
    let default_ambient = Vec3::splat(0.2);
    let fog_defaults = FogSettings {
        enabled: false,
        color: Vec3::ZERO,
        start: 1_000_000.0,
        end: 1_000_001.0,
    };

    let mut ambient = default_ambient;
    let mut lights = [PackedLight::zeroed(); MAX_LIGHTS];
    let mut light_count: usize = 0;

    if let Some(environment) = render_info.lighting.as_ref() {
        ambient = environment.ambient;
        for light in &environment.lights {
            if light_count == MAX_LIGHTS {
                break;
            }
            let Ok(light) = light.lock() else { continue };
            if !light.enabled {
                continue;
            }

            lights[light_count] = pack_light(&light);
            light_count += 1;
        }
    }

    let fog = render_info.fog_settings().copied().unwrap_or(fog_defaults);

    LightingUniform {
        ambient_color: vec3_to_array(ambient, 1.0),
        fog_color: vec3_to_array(fog.color, 1.0),
        fog_params: [fog.start, fog.end, if fog.enabled { 1.0 } else { 0.0 }, 0.0],
        light_meta: [light_count as f32, 0.0, 0.0, 0.0],
        lights,
    }
}

fn pack_light(light: &LightClass) -> PackedLight {
    let mut packed = PackedLight::zeroed();
    let forward = light.direction.normalize_or_zero();
    let color = light.color * light.intensity;
    packed.color = [
        color.x,
        color.y,
        color.z,
        light.attenuation.constant.max(0.0001),
    ];
    packed.position_range = [
        light.position.x,
        light.position.y,
        light.position.z,
        light.range.max(0.001),
    ];
    packed.spot_params = [
        light.inner_cone_angle.cos(),
        light.outer_cone_angle.cos(),
        light.attenuation.linear,
        light.attenuation.quadratic,
    ];

    let (dir, light_type_value) = match light.light_type {
        LightType::Directional => (forward, 0.0),
        LightType::Point => (Vec3::ZERO, 1.0),
        LightType::Spot => (forward, 2.0),
    };

    packed.direction = [dir.x, dir.y, dir.z, light_type_value];
    packed
}

fn vec3_to_array(vec: Vec3, w: f32) -> [f32; 4] {
    [vec.x, vec.y, vec.z, w]
}
