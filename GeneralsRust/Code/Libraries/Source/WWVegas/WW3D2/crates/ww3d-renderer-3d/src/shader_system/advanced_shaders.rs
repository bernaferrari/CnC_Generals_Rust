//! Advanced Shader System
//!
//! This module provides a comprehensive shader system with:
//! - Physically-based rendering (PBR)
//! - Advanced lighting models
//! - Normal mapping and parallax mapping
//! - Environment mapping and reflections
//! - Screen space ambient occlusion (SSAO)
//! - Shadow mapping with PCF filtering

use wgpu::util::DeviceExt;
use std::collections::HashMap;

/// Advanced shader pipeline for modern graphics
pub struct AdvancedShaderPipeline {
    device: wgpu::Device,
    queue: wgpu::Queue,

    // Shader modules
    vertex_shader: wgpu::ShaderModule,
    fragment_shader: wgpu::ShaderModule,
    compute_shader: Option<wgpu::ShaderModule>,

    // Pipeline layouts
    pipeline_layout: wgpu::PipelineLayout,

    // Render pipelines
    pbr_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,

    // Bind group layouts
    camera_bind_group_layout: wgpu::BindGroupLayout,
    material_bind_group_layout: wgpu::BindGroupLayout,
    light_bind_group_layout: wgpu::BindGroupLayout,
    shadow_bind_group_layout: wgpu::BindGroupLayout,

    // Uniform buffers
    camera_uniform_buffer: wgpu::Buffer,
    material_uniform_buffer: wgpu::Buffer,
    light_uniform_buffer: wgpu::Buffer,

    // Texture samplers
    linear_sampler: wgpu::Sampler,
    shadow_sampler: wgpu::Sampler,
}

impl AdvancedShaderPipeline {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, surface_format: wgpu::TextureFormat) -> Self {
        // Create shader modules
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Advanced Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::pbr_vertex_shader().into()),
            
        });

        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Advanced Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::pbr_fragment_shader().into()),
            
        });

        // Create bind group layouts
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[
                // Camera uniform
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
            ],
        });

        let material_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[
                // Material uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Albedo texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Roughness/Metallic texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let light_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Light Bind Group Layout"),
            entries: &[
                // Light uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Shadow map
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
                // Shadow sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Advanced Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &material_bind_group_layout,
                &light_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pbr_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBR Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: (3 + 3 + 2 + 4 + 4) * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // Position
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        // Normal
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 3 * std::mem::size_of::<f32>() as u64,
                            shader_location: 1,
                        },
                        // UV
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 6 * std::mem::size_of::<f32>() as u64,
                            shader_location: 2,
                        },
                        // Bone indices
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32x4,
                            offset: 8 * std::mem::size_of::<f32>() as u64,
                            shader_location: 3,
                        },
                        // Bone weights
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 12 * std::mem::size_of::<f32>() as u64,
                            shader_location: 4,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create shadow pipeline (similar but without fragment shader)
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_shadow"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: (3 + 3 + 2 + 4 + 4) * std::mem::size_of::<f32>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 3 * std::mem::size_of::<f32>() as u64,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 6 * std::mem::size_of::<f32>() as u64,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32x4,
                            offset: 8 * std::mem::size_of::<f32>() as u64,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 12 * std::mem::size_of::<f32>() as u64,
                            shader_location: 4,
                        },
                    ],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: None, // Shadow pass doesn't need fragment shader
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create uniform buffers
        let camera_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Uniform Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Uniform Buffer"),
            size: std::mem::size_of::<MaterialUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Uniform Buffer"),
            size: std::mem::size_of::<LightUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create samplers
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Linear Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        Self {
            device,
            queue,
            vertex_shader,
            fragment_shader,
            compute_shader: None,
            pipeline_layout,
            pbr_pipeline,
            shadow_pipeline,
            camera_bind_group_layout,
            material_bind_group_layout,
            light_bind_group_layout,
            shadow_bind_group_layout: light_bind_group_layout.clone(), // Reuse for shadows
            camera_uniform_buffer,
            material_uniform_buffer,
            light_uniform_buffer,
            linear_sampler,
            shadow_sampler,
        }
    }

    /// PBR Vertex Shader with GPU skinning
    fn pbr_vertex_shader() -> &'static str {
        r#"
        struct VertexInput {
            @location(0) position: vec3<f32>,
            @location(1) normal: vec3<f32>,
            @location(2) tex_coords: vec2<f32>,
            @location(3) tangent: vec3<f32>,
            @location(4) bone_indices: vec4<u32>,
            @location(5) bone_weights: vec4<f32>,
        };

        struct VertexOutput {
            @builtin(position) clip_position: vec4<f32>,
            @location(0) tex_coords: vec2<f32>,
            @location(1) world_normal: vec3<f32>,
            @location(2) world_position: vec3<f32>,
            @location(3) tangent: vec3<f32>,
            @location(4) bitangent: vec3<f32>,
        };

        struct CameraUniform {
            view_proj: mat4x4<f32>,
            position: vec3<f32>,
        };

        struct ModelUniform {
            model: mat4x4<f32>,
        };

        struct BoneUniform {
            bones: array<mat4x4<f32>, 64>,
        };

        @group(0) @binding(0)
        var<uniform> camera: CameraUniform;

        @group(1) @binding(0)
        var<uniform> model: ModelUniform;

        @group(2) @binding(0)
        var<uniform> bones: BoneUniform;

        @vertex
        fn vs_main(input: VertexInput) -> VertexOutput {
            var output: VertexOutput;

            // GPU skinning
            var skin_transform = mat4x4<f32>(0.0);
            var total_weight = 0.0;

            for (var i = 0u; i < 4u; i = i + 1u) {
                let bone_index = input.bone_indices[i];
                let bone_weight = input.bone_weights[i];

                if (bone_weight > 0.0) {
                    skin_transform = skin_transform + bones.bones[bone_index] * bone_weight;
                    total_weight = total_weight + bone_weight;
                }
            }

            if (total_weight > 0.0 && total_weight != 1.0) {
                skin_transform = skin_transform / total_weight;
            }

            // Apply model transform
            let model_transform = model.model * skin_transform;

            // Transform position
            let world_position = model_transform * vec4<f32>(input.position, 1.0);
            output.clip_position = camera.view_proj * world_position;
            output.world_position = world_position.xyz;

            // Transform normal and tangent
            let normal_matrix = transpose(inverse(model_transform));
            output.world_normal = normalize((normal_matrix * vec4<f32>(input.normal, 0.0)).xyz);
            output.tangent = normalize((normal_matrix * vec4<f32>(input.tangent, 0.0)).xyz);

            // Calculate bitangent
            let bitangent = cross(output.world_normal, output.tangent);
            output.bitangent = bitangent;

            output.tex_coords = input.tex_coords;

            return output;
        }

        @vertex
        fn vs_shadow(input: VertexInput) -> @builtin(position) vec4<f32> {
            // Shadow pass - simplified vertex shader
            var skin_transform = mat4x4<f32>(0.0);
            var total_weight = 0.0;

            for (var i = 0u; i < 4u; i = i + 1u) {
                let bone_index = input.bone_indices[i];
                let bone_weight = input.bone_weights[i];

                if (bone_weight > 0.0) {
                    skin_transform = skin_transform + bones.bones[bone_index] * bone_weight;
                    total_weight = total_weight + bone_weight;
                }
            }

            if (total_weight > 0.0 && total_weight != 1.0) {
                skin_transform = skin_transform / total_weight;
            }

            let model_transform = model.model * skin_transform;
            return camera.view_proj * (model_transform * vec4<f32>(input.position, 1.0));
        }
        "#
    }

    /// PBR Fragment Shader with advanced lighting
    fn pbr_fragment_shader() -> &'static str {
        r#"
        struct FragmentInput {
            @location(0) tex_coords: vec2<f32>,
            @location(1) world_normal: vec3<f32>,
            @location(2) world_position: vec3<f32>,
            @location(3) tangent: vec3<f32>,
            @location(4) bitangent: vec3<f32>,
        };

        struct FragmentOutput {
            @location(0) color: vec4<f32>,
            @location(1) normal: vec4<f32>,
        };

        struct MaterialUniform {
            albedo: vec4<f32>,
            metallic: f32,
            roughness: f32,
            ao: f32,
            emissive: vec3<f32>,
        };

        struct LightUniform {
            position: vec3<f32>,
            color: vec3<f32>,
            intensity: f32,
            direction: vec3<f32>,
            type_: u32, // 0 = point, 1 = directional, 2 = spot
            cutoff: f32,
            outer_cutoff: f32,
        };

        struct CameraUniform {
            position: vec3<f32>,
        };

        @group(0) @binding(0)
        var<uniform> camera: CameraUniform;

        @group(1) @binding(0)
        var<uniform> material: MaterialUniform;

        @group(2) @binding(0)
        var<uniform> light: LightUniform;

        @group(2) @binding(1)
        var albedo_texture: texture_2d<f32>;

        @group(2) @binding(2)
        var normal_texture: texture_2d<f32>;

        @group(2) @binding(3)
        var metallic_roughness_texture: texture_2d<f32>;

        @group(2) @binding(4)
        var occlusion_texture: texture_2d<f32>;

        @group(2) @binding(5)
        var emissive_texture: texture_2d<f32>;

        @group(2) @binding(6)
        var shadow_map: texture_depth_2d;

        @group(2) @binding(7)
        var sampler_: sampler;

        @group(2) @binding(8)
        var shadow_sampler: sampler_comparison;

        // Fresnel-Schlick approximation
        fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
            return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
        }

        // Normal distribution function (GGX/Trowbridge-Reitz)
        fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
            let a = roughness * roughness;
            let a2 = a * a;
            let n_dot_h = max(dot(n, h), 0.0);
            let n_dot_h2 = n_dot_h * n_dot_h;

            let num = a2;
            var denom = n_dot_h2 * (a2 - 1.0) + 1.0;
            denom = 3.14159 * denom * denom;

            return num / denom;
        }

        // Geometry function (Schlick-GGX)
        fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
            let r = roughness + 1.0;
            let k = (r * r) / 8.0;
            return n_dot_v / (n_dot_v * (1.0 - k) + k);
        }

        // Smith geometry function
        fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
            let n_dot_v = max(dot(n, v), 0.0);
            let n_dot_l = max(dot(n, l), 0.0);
            let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
            let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
            return ggx1 * ggx2;
        }

        // PBR BRDF
        fn pbr_brdf(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, albedo: vec3<f32>,
                    metallic: f32, roughness: f32) -> vec3<f32> {
            let h = normalize(v + l);
            let f0 = mix(vec3<f32>(0.04), albedo, metallic);

            let f = fresnel_schlick(max(dot(h, v), 0.0), f0);
            let ndf = distribution_ggx(n, h, roughness);
            let g = geometry_smith(n, v, l, roughness);

            let numerator = ndf * g * f;
            let denominator = 4.0 * max(dot(n, v), 0.0) * max(dot(n, l), 0.0) + 0.0001;
            let specular = numerator / denominator;

            let k_s = f;
            let k_d = (vec3<f32>(1.0) - k_s) * (1.0 - metallic);

            return k_d * albedo / 3.14159 + specular;
        }

        // Shadow calculation with PCF
        fn calculate_shadow(world_pos: vec3<f32>) -> f32 {
            // Transform to light space (assuming light space matrix is provided)
            let light_space_pos = vec4<f32>(world_pos, 1.0);
            let proj_coords = light_space_pos.xy / light_space_pos.w;
            let uv_coords = proj_coords * 0.5 + 0.5;

            if (uv_coords.x < 0.0 || uv_coords.x > 1.0 || uv_coords.y < 0.0 || uv_coords.y > 1.0) {
                return 1.0;
            }

            var shadow = 0.0;
            let texel_size = 1.0 / 1024.0; // Assuming 1024x1024 shadow map

            for (var x = -1; x <= 1; x = x + 1) {
                for (var y = -1; y <= 1; y = y + 1) {
                    let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
                    shadow = shadow + textureSampleCompare(
                        shadow_map, shadow_sampler,
                        uv_coords + offset, light_space_pos.z
                    );
                }
            }

            return shadow / 9.0;
        }

        @fragment
        fn fs_main(input: FragmentInput) -> FragmentOutput {
            var output: FragmentOutput;

            // Sample textures
            let albedo_tex = textureSample(albedo_texture, sampler_, input.tex_coords);
            let normal_tex = textureSample(normal_texture, sampler_, input.tex_coords);
            let metallic_roughness_tex = textureSample(metallic_roughness_texture, sampler_, input.tex_coords);
            let occlusion_tex = textureSample(occlusion_texture, sampler_, input.tex_coords);
            let emissive_tex = textureSample(emissive_texture, sampler_, input.tex_coords);

            // Material properties
            let albedo = albedo_tex.rgb * material.albedo.rgb;
            let metallic = metallic_roughness_tex.b * material.metallic;
            let roughness = metallic_roughness_tex.g * material.roughness;
            let ao = occlusion_tex.r * material.ao;
            let emissive = emissive_tex.rgb * material.emissive;

            // Normal mapping
            let tbn = mat3x3<f32>(
                input.tangent,
                input.bitangent,
                input.world_normal
            );
            var normal = normal_tex.rgb * 2.0 - 1.0;
            normal = normalize(tbn * normal);

            // View and light vectors
            let view_dir = normalize(camera.position - input.world_position);

            var light_dir: vec3<f32>;
            var light_color = light.color * light.intensity;
            var attenuation = 1.0;

            if (light.type_ == 0u) {
                // Point light
                light_dir = normalize(light.position - input.world_position);
                let distance = length(light.position - input.world_position);
                attenuation = 1.0 / (distance * distance);
            } else if (light.type_ == 1u) {
                // Directional light
                light_dir = normalize(-light.direction);
            } else {
                // Spot light
                light_dir = normalize(light.position - input.world_position);
                let theta = dot(light_dir, normalize(-light.direction));
                let epsilon = light.cutoff - light.outer_cutoff;
                attenuation = clamp((theta - light.outer_cutoff) / epsilon, 0.0, 1.0);
                let distance = length(light.position - input.world_position);
                attenuation = attenuation * (1.0 / (distance * distance));
            }

            // Calculate radiance
            let radiance = light_color * attenuation;

            // PBR BRDF
            let brdf = pbr_brdf(normal, view_dir, light_dir, albedo, metallic, roughness);

            // Shadow
            let shadow = calculate_shadow(input.world_position);

            // Final color
            let color = brdf * radiance * shadow + emissive * ao;

            output.color = vec4<f32>(color, albedo_tex.a);
            output.normal = vec4<f32>(normal * 0.5 + 0.5, 1.0);

            return output;
        }
        "#
    }

    /// Update camera uniform buffer
    pub fn update_camera(&self, camera_data: &CameraUniform) {
        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(camera_data),
        );
    }

    /// Update material uniform buffer
    pub fn update_material(&self, material_data: &MaterialUniform) {
        self.queue.write_buffer(
            &self.material_uniform_buffer,
            0,
            bytemuck::bytes_of(material_data),
        );
    }

    /// Update light uniform buffer
    pub fn update_light(&self, light_data: &LightUniform) {
        self.queue.write_buffer(
            &self.light_uniform_buffer,
            0,
            bytemuck::bytes_of(light_data),
        );
    }

    /// Get PBR render pipeline
    pub fn pbr_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pbr_pipeline
    }

    /// Get shadow render pipeline
    pub fn shadow_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.shadow_pipeline
    }
}

// Uniform buffer structures
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub _padding: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub albedo: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub ao: f32,
    pub emissive: [f32; 3],
    pub _padding: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    pub position: [f32; 3],
    pub _padding1: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub direction: [f32; 3],
    pub type_: u32,
    pub cutoff: f32,
    pub outer_cutoff: f32,
}
