use crate::assets::{W3DMaterial, W3DMesh, W3DModel, W3DVertex};
use anyhow::Result;
use glam::{Mat4, Vec3};
use log::info;
use std::collections::HashMap;
use std::sync::Arc;

const FALLBACK_CUBE_DIFFUSE: [f32; 4] = [0.58, 0.58, 0.58, 1.0];
const FALLBACK_CUBE_SHADOW_DIFFUSE: [f32; 4] = [0.46, 0.46, 0.46, 1.0];

/// Material properties uniform - matches shader's MaterialProperties struct
/// Note: WGSL has strict alignment rules. Vec4 = 16 bytes, vec2 needs 8-byte alignment
/// Total must be 80 bytes due to alignment padding in WGSL
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialPropertiesUniform {
    pub diffuse_color: [f32; 4],   // 16 bytes, offset 0
    pub specular_color: [f32; 4],  // 16 bytes, offset 16
    pub emissive_color: [f32; 4],  // 16 bytes, offset 32
    pub opacity: f32,              // 4 bytes, offset 48
    pub shininess: f32,            // 4 bytes, offset 52
    pub _padding1: [f32; 1],       // 4 bytes, offset 56 (alignment padding)
    pub _padding2: [f32; 1], // 4 bytes, offset 60 (alignment padding to align vec2 to 8-byte boundary)
    pub stage0_uv_scale: [f32; 2], // 8 bytes, offset 64
    pub stage1_uv_scale: [f32; 2], // 8 bytes, offset 72
}

impl Default for MaterialPropertiesUniform {
    fn default() -> Self {
        Self {
            diffuse_color: [0.8, 0.8, 0.8, 1.0],  // Default white diffuse
            specular_color: [1.0, 1.0, 1.0, 1.0], // Default white specular
            emissive_color: [0.0, 0.0, 0.0, 1.0], // No emissive by default
            opacity: 1.0,
            shininess: 32.0,
            _padding1: [0.0],
            _padding2: [0.0],
            stage0_uv_scale: [1.0, 1.0],
            stage1_uv_scale: [1.0, 1.0],
        }
    }
}

/// Main graphics subsystem - equivalent to C++ SAGE GraphicsSystem
pub struct GraphicsSystem {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,

    // Model cache for pre-loaded W3D models - matches C++ ModelCache
    loaded_models: HashMap<String, Arc<W3DModel>>,

    // Global shader resources - matches C++ GlobalShaderResources
    global_uniforms: GlobalUniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // Material properties uniforms - matches C++ VertexMaterialClass system
    material_properties: MaterialPropertiesUniform,
    material_properties_buffer: wgpu::Buffer,
    material_properties_bind_group: wgpu::BindGroup,

    // Material properties bind group layout
    material_properties_bind_group_layout: wgpu::BindGroupLayout,

    // Texture bind group layout for materials
    texture_bind_group_layout: wgpu::BindGroupLayout,
    // Cache for loaded material textures and their bind groups
    material_bind_groups: HashMap<String, wgpu::BindGroup>,

    // Statistics
    frame_count: u64,
    triangles_rendered: u64,
    draw_calls: u64,
}

/// Global shader uniforms - equivalent to C++ GlobalShaderResources
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniforms {
    pub view_projection: [[f32; 4]; 4],
    pub view_matrix: [[f32; 4]; 4],
    pub projection_matrix: [[f32; 4]; 4],
    pub camera_position: [f32; 4],
    pub time: f32,
    pub _time_padding: [f32; 3],
    pub ambient_light: [f32; 3],
    pub _ambient_padding: f32,
    pub sun_direction: [f32; 3],
    pub _sun_dir_padding: f32,
    pub sun_color: [f32; 3],
    pub _sun_color_padding: f32,
}

pub const MAX_STAGE_TEXTURES: usize = 4;

impl GraphicsSystem {
    /// Initialize graphics system - equivalent to C++ GraphicsSystem::Initialize()
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
    ) -> Result<Self> {
        info!("Initializing GraphicsSystem (C++ SAGE equivalent)");

        // Create global uniforms
        let global_uniforms = GlobalUniforms {
            view_projection: Mat4::IDENTITY.to_cols_array_2d(),
            view_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            projection_matrix: Mat4::IDENTITY.to_cols_array_2d(),
            camera_position: [0.0, 0.0, 0.0, 1.0],
            time: 0.0,
            _time_padding: [0.0, 0.0, 0.0],
            ambient_light: [0.3, 0.3, 0.3], // Slightly brighter ambient to match C++ better
            _ambient_padding: 0.0,
            sun_direction: [-0.5, -1.0, -0.5],
            _sun_dir_padding: 0.0,
            sun_color: [1.0, 0.9, 0.8],
            _sun_color_padding: 0.0,
        };

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Uniforms Buffer"),
            size: std::mem::size_of::<GlobalUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create uniform bind group layout
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("global_uniform_bind_group_layout"),
            });

        // Create uniform bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("global_uniform_bind_group"),
        });

        // Create texture bind group layout
        let mut texture_layout_entries = Vec::with_capacity(MAX_STAGE_TEXTURES * 2);
        for stage in 0..MAX_STAGE_TEXTURES {
            let binding_index = (stage * 2) as u32;
            texture_layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: binding_index,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            });
            texture_layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: binding_index + 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &texture_layout_entries,
                label: Some("texture_bind_group_layout"),
            });

        // Create material properties buffer - matches C++ VertexMaterialClass uniform
        let material_properties = MaterialPropertiesUniform::default();
        let material_properties_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Properties Buffer"),
            size: std::mem::size_of::<MaterialPropertiesUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create material properties bind group layout
        let material_properties_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("material_properties_bind_group_layout"),
            });

        // Create material properties bind group
        let material_properties_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &material_properties_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: material_properties_buffer.as_entire_binding(),
            }],
            label: Some("material_properties_bind_group"),
        });

        // Upload initial material properties to GPU
        queue.write_buffer(
            &material_properties_buffer,
            0,
            bytemuck::cast_slice(&[material_properties]),
        );

        let mut loaded_models = HashMap::new();

        // Cache a neutral fallback cube for explicit missing-model diagnostics.
        // The normal render path does not substitute it for missing retail W3D assets.
        {
            let fallback = Self::create_fallback_cube_model();
            loaded_models.insert("__fallback_cube__".to_string(), Arc::new(fallback));
            info!("GraphicsSystem: neutral fallback cube model cached for diagnostics");
        }

        info!("GraphicsSystem initialized successfully and ready for first frame");

        Ok(Self {
            device,
            queue,
            color_format,
            loaded_models,
            global_uniforms,
            uniform_buffer,
            uniform_bind_group,
            material_properties,
            material_properties_buffer,
            material_properties_bind_group,
            material_properties_bind_group_layout,
            texture_bind_group_layout,
            material_bind_groups: HashMap::new(),
            frame_count: 0,
            triangles_rendered: 0,
            draw_calls: 0,
            depth_format,
        })
    }

    /// Update global shader uniforms - equivalent to C++ GraphicsSystem::UpdateGlobalUniforms()
    pub fn update_global_uniforms(
        &mut self,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        camera_position: Vec3,
        time: f32,
    ) {
        self.global_uniforms.view_matrix = view_matrix.to_cols_array_2d();
        self.global_uniforms.projection_matrix = projection_matrix.to_cols_array_2d();
        self.global_uniforms.view_projection =
            (*projection_matrix * *view_matrix).to_cols_array_2d();
        self.global_uniforms.camera_position =
            [camera_position.x, camera_position.y, camera_position.z, 1.0];
        self.global_uniforms.time = time;

        // Upload to GPU
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.global_uniforms]),
        );
    }

    /// Update material properties - equivalent to C++ VertexMaterialClass::Update()
    pub fn update_material_properties(&mut self, material: &crate::assets::W3DMaterial) {
        // Convert material properties to uniform format - matches C++ D3D8 material system
        self.material_properties.diffuse_color = [
            material.diffuse_color.x,
            material.diffuse_color.y,
            material.diffuse_color.z,
            1.0,
        ];
        self.material_properties.specular_color = [
            material.specular_color.x,
            material.specular_color.y,
            material.specular_color.z,
            1.0,
        ];
        self.material_properties.emissive_color = [
            material.emissive_color.x,
            material.emissive_color.y,
            material.emissive_color.z,
            1.0,
        ];
        self.material_properties.opacity = material.opacity;
        self.material_properties.shininess = material.shininess;

        // Upload to GPU
        self.queue.write_buffer(
            &self.material_properties_buffer,
            0,
            bytemuck::cast_slice(&[self.material_properties]),
        );
    }

    /// Get material properties bind group
    pub fn material_properties_bind_group(&self) -> &wgpu::BindGroup {
        &self.material_properties_bind_group
    }

    /// Read the current global lighting defaults used by the renderer.
    pub fn current_lighting(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        (
            self.global_uniforms.ambient_light,
            self.global_uniforms.sun_color,
            self.global_uniforms.sun_direction,
        )
    }

    /// Override global lighting values from map/environment metadata.
    pub fn set_lighting(
        &mut self,
        ambient: Option<[f32; 3]>,
        sun_color: Option<[f32; 3]>,
        sun_direction: Option<[f32; 3]>,
        sky_color: Option<[f32; 3]>,
    ) {
        if let Some(a) = ambient {
            self.global_uniforms.ambient_light = a;
        }
        if let Some(c) = sun_color {
            self.global_uniforms.sun_color = c;
        }
        if let Some(dir) = sun_direction {
            self.global_uniforms.sun_direction = dir;
        }
        if let Some(sky) = sky_color {
            // Approximate clear color from sky for terrain background.
            self.global_uniforms.sun_color = [
                (self.global_uniforms.sun_color[0] + sky[0]) * 0.5,
                (self.global_uniforms.sun_color[1] + sky[1]) * 0.5,
                (self.global_uniforms.sun_color[2] + sky[2]) * 0.5,
            ];
        }
        // Upload to GPU so the change takes effect immediately.
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.global_uniforms]),
        );
    }

    /// Cache W3D model - equivalent to C++ ModelCache::CacheModel()
    pub fn cache_model(&mut self, model_name: String, model: W3DModel) {
        // Match the legacy cache behavior more closely: cache the model structure now and let
        // the active render path materialize GPU bindings lazily when a mesh is actually drawn.
        self.loaded_models.insert(model_name, Arc::new(model));
        // Note: Mesh buffer creation is handled by MeshRenderer when needed
    }

    /// Get cached model - equivalent to C++ ModelCache::GetModel()
    pub fn get_model(&self, model_name: &str) -> Option<&Arc<W3DModel>> {
        self.loaded_models.get(model_name)
    }

    /// Get all cached models - used for texture preloading
    pub fn get_all_models(&self) -> impl Iterator<Item = (&String, &Arc<W3DModel>)> {
        self.loaded_models.iter()
    }

    pub fn debug_model_summary(&self, model_name: &str) -> Option<String> {
        let model = self.loaded_models.get(model_name)?;
        let mesh_count = model.meshes.len();
        let first_mesh = model.meshes.first();
        let first_mesh_name = first_mesh.map(|mesh| mesh.name.as_str()).unwrap_or("none");
        let first_mesh_texture = first_mesh
            .and_then(|mesh| mesh.material.texture_name.as_deref())
            .unwrap_or("none");
        let first_mesh_transform = first_mesh
            .map(|mesh| {
                let t = mesh.transform.w_axis.truncate();
                format!("({:.1},{:.1},{:.1})", t.x, t.y, t.z)
            })
            .unwrap_or_else(|| "(none)".to_string());
        let prototype_mesh_model_count = model.ww3d_mesh_models.len();
        Some(format!(
            "{} meshes={} proto_meshes={} bbox_min=({:.1},{:.1},{:.1}) bbox_max=({:.1},{:.1},{:.1}) first_mesh={} first_tex={} first_mesh_t={}",
            model_name,
            mesh_count,
            prototype_mesh_model_count,
            model.bounding_box_min.x,
            model.bounding_box_min.y,
            model.bounding_box_min.z,
            model.bounding_box_max.x,
            model.bounding_box_max.y,
            model.bounding_box_max.z,
            first_mesh_name,
            first_mesh_texture,
            first_mesh_transform,
        ))
    }

    /// Get uniform bind group
    pub fn uniform_bind_group(&self) -> &wgpu::BindGroup {
        &self.uniform_bind_group
    }

    /// Get texture bind group layout
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    /// Get material bind group (read-only) - equivalent to C++ MaterialManager::GetMaterialBindGroup()
    pub fn get_material_bind_group(
        &self,
        material: &crate::assets::W3DMaterial,
    ) -> Option<&wgpu::BindGroup> {
        let bind_group_key = Self::material_bind_group_key(material);

        self.material_bind_groups.get(&bind_group_key)
    }

    /// Get or create material bind group (mutable) - equivalent to C++ MaterialManager::GetMaterialBindGroup()
    pub fn get_or_create_material_bind_group(
        &mut self,
        material: &crate::assets::W3DMaterial,
    ) -> Option<&wgpu::BindGroup> {
        let bind_group_key = Self::material_bind_group_key(material);

        if !self.material_bind_groups.contains_key(&bind_group_key) {
            if let Some(bind_group) = self.create_material_bind_group(material, &bind_group_key) {
                self.material_bind_groups
                    .insert(bind_group_key.clone(), bind_group);
            } else {
                return None;
            }
        }

        self.material_bind_groups.get(&bind_group_key)
    }

    /// Create material bind group - equivalent to C++ MaterialManager::CreateMaterialBindGroup()
    fn create_material_bind_group(
        &self,
        material: &crate::assets::W3DMaterial,
        key: &str,
    ) -> Option<wgpu::BindGroup> {
        let asset_manager_arc = crate::assets::get_asset_manager()?;
        let asset_manager = asset_manager_arc.lock().unwrap_or_else(|e| e.into_inner());

        // C++ approach: get texture from cache or use default
        // Textures should already be preloaded before rendering starts
        let default_texture = asset_manager.get_default_texture();

        let mut stage_textures = Vec::with_capacity(MAX_STAGE_TEXTURES);
        let mut stage_names = Vec::with_capacity(MAX_STAGE_TEXTURES);

        for stage in 0..MAX_STAGE_TEXTURES {
            if let Some(name) = Self::stage_texture_name(material, stage) {
                stage_names.push(name.clone());
                if let Some(tex) = asset_manager.get_cached_texture(name) {
                    stage_textures.push(tex);
                    continue;
                }
            }
            stage_textures.push(default_texture);
        }

        println!(
            "   📦 Creating bind group '{}' with stage textures: {:?}",
            key, stage_names
        );

        let mut entries = Vec::with_capacity(stage_textures.len() * 2);
        for (stage_idx, tex) in stage_textures.iter().enumerate() {
            let binding_index = (stage_idx * 2) as u32;
            entries.push(wgpu::BindGroupEntry {
                binding: binding_index,
                resource: wgpu::BindingResource::TextureView(&tex.view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: binding_index + 1,
                resource: wgpu::BindingResource::Sampler(&tex.sampler),
            });
        }

        Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
            entries: &entries,
            label: Some(&format!("material_bind_group_{}", key)),
        }))
    }

    /// Begin frame - equivalent to C++ GraphicsSystem::BeginFrame()
    pub fn begin_frame(&mut self) {
        self.frame_count += 1;
        self.triangles_rendered = 0;
        self.draw_calls = 0;
    }

    /// Try to get a cached model by name.
    ///
    /// The diagnostic fallback cube is returned only when requested by its explicit
    /// sentinel name; arbitrary missing retail W3D names must remain misses.
    pub fn get_model_or_fallback(&self, model_name: &str) -> Option<Arc<W3DModel>> {
        Self::lookup_model_or_explicit_fallback(&self.loaded_models, model_name)
    }

    fn lookup_model_or_explicit_fallback(
        loaded_models: &HashMap<String, Arc<W3DModel>>,
        model_name: &str,
    ) -> Option<Arc<W3DModel>> {
        if let Some(model) = loaded_models.get(model_name) {
            return Some(Arc::clone(model));
        }

        if Self::is_fallback_model(model_name) {
            return loaded_models.get("__fallback_cube__").map(Arc::clone);
        }

        None
    }

    /// Check whether a model is the built-in fallback cube.
    pub fn is_fallback_model(model_name: &str) -> bool {
        model_name == "__fallback_cube__"
    }

    /// Create a simple unit cube W3D model for use as a fallback placeholder.
    /// The cube is centered at the origin, extends +/-5.0 on each axis, and uses
    /// neutral gray shading so missing assets remain visible without startup glare.
    fn create_fallback_cube_model() -> W3DModel {
        // 8 corners of a unit cube centered at origin
        let s = 5.0; // half-extent in world units (visible from gameplay camera)
        let vertices = vec![
            // Front face (z = +s)
            W3DVertex {
                position: [-s, -s, s],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [s, -s, s],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [s, s, s],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [-s, s, s],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            // Back face (z = -s)
            W3DVertex {
                position: [s, -s, -s],
                normal: [0.0, 0.0, -1.0],
                uv: [0.0, 0.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [-s, -s, -s],
                normal: [0.0, 0.0, -1.0],
                uv: [1.0, 0.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [-s, s, -s],
                normal: [0.0, 0.0, -1.0],
                uv: [1.0, 1.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [s, s, -s],
                normal: [0.0, 0.0, -1.0],
                uv: [0.0, 1.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            // Top face (y = +s)
            W3DVertex {
                position: [-s, s, s],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [s, s, s],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [s, s, -s],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 1.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [-s, s, -s],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            // Bottom face (y = -s)
            W3DVertex {
                position: [-s, -s, -s],
                normal: [0.0, -1.0, 0.0],
                uv: [0.0, 0.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [s, -s, -s],
                normal: [0.0, -1.0, 0.0],
                uv: [1.0, 0.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [s, -s, s],
                normal: [0.0, -1.0, 0.0],
                uv: [1.0, 1.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [-s, -s, s],
                normal: [0.0, -1.0, 0.0],
                uv: [0.0, 1.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            // Right face (x = +s)
            W3DVertex {
                position: [s, -s, s],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 0.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [s, -s, -s],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 0.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [s, s, -s],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 1.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            W3DVertex {
                position: [s, s, s],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 1.0],
                color: FALLBACK_CUBE_DIFFUSE,
            },
            // Left face (x = -s)
            W3DVertex {
                position: [-s, -s, -s],
                normal: [-1.0, 0.0, 0.0],
                uv: [0.0, 0.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [-s, -s, s],
                normal: [-1.0, 0.0, 0.0],
                uv: [1.0, 0.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [-s, s, s],
                normal: [-1.0, 0.0, 0.0],
                uv: [1.0, 1.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
            W3DVertex {
                position: [-s, s, -s],
                normal: [-1.0, 0.0, 0.0],
                uv: [0.0, 1.0],
                color: FALLBACK_CUBE_SHADOW_DIFFUSE,
            },
        ];

        // Two triangles per face, 6 faces = 36 indices
        let indices: Vec<u32> = vec![
            0, 1, 2, 0, 2, 3, // front
            4, 5, 6, 4, 6, 7, // back
            8, 9, 10, 8, 10, 11, // top
            12, 13, 14, 12, 14, 15, // bottom
            16, 17, 18, 16, 18, 19, // right
            20, 21, 22, 20, 22, 23, // left
        ];

        let mut material = W3DMaterial::default();
        material.name = "__fallback_material__".to_string();
        material.diffuse_color = Vec3::new(0.58, 0.58, 0.58);
        material.emissive_color = Vec3::ZERO;

        let mesh = W3DMesh {
            name: "__fallback_cube_mesh__".to_string(),
            vertices,
            indices,
            material,
            transform: Mat4::IDENTITY,
            header: None,
            stage_texcoords: Vec::new(),
            passes: Vec::new(),
            per_pass_stage_texture_ids: Vec::new(),
            per_pass_stage_texture_names: Vec::new(),
            per_pass_vertex_material_ids: Vec::new(),
            per_pass_shader_ids: Vec::new(),
            per_pass_dcg_colors: Vec::new(),
            per_pass_dig_colors: Vec::new(),
            vertex_materials: Vec::new(),
            shaders: Vec::new(),
            vertex_influences: None,
            vertex_shade_indices: None,
            per_stage_face_texcoord_ids: Vec::new(),
            stage_uv_channels: Vec::new(),
            texture_library: Vec::new(),
            vertex_mappers: Vec::new(),
            vertices_in_render_space: true,
            has_explicit_vertex_colors: true,
        };

        W3DModel {
            name: "__fallback_cube__".to_string(),
            meshes: vec![mesh],
            materials: HashMap::new(),
            texture_names: Vec::new(),
            ww3d_mesh_models: HashMap::new(),
            bounding_box_min: Vec3::new(-5.0, -5.0, -5.0),
            bounding_box_max: Vec3::new(5.0, 5.0, 5.0),
            hierarchy: None,
            animations: Vec::new(),
        }
    }

    /// End frame - equivalent to C++ GraphicsSystem::EndFrame()
    pub fn end_frame(&self) {
        // Frame statistics logging would go here
    }

    /// Get rendering statistics
    pub fn get_statistics(&self) -> GraphicsStatistics {
        GraphicsStatistics {
            frame_count: self.frame_count,
            triangles_rendered: self.triangles_rendered,
            draw_calls: self.draw_calls,
            models_cached: self.loaded_models.len(),
            materials_cached: self.material_bind_groups.len(),
        }
    }

    /// Access the underlying wgpu device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Clone the Arc-wrapped device for systems that need shared ownership.
    pub fn device_arc(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    pub(crate) fn stage_texture_name(
        material: &crate::assets::W3DMaterial,
        stage: usize,
    ) -> Option<&String> {
        match stage {
            0 => material
                .stage0_mapping
                .texture_name
                .as_ref()
                .or(material.texture_name.as_ref()),
            1 => material
                .stage1_mapping
                .as_ref()
                .and_then(|mapping| mapping.texture_name.as_ref()),
            2 => material
                .stage2_mapping
                .as_ref()
                .and_then(|mapping| mapping.texture_name.as_ref()),
            3 => material
                .stage3_mapping
                .as_ref()
                .and_then(|mapping| mapping.texture_name.as_ref()),
            _ => None,
        }
    }

    fn material_bind_group_key(material: &crate::assets::W3DMaterial) -> String {
        let mut parts = Vec::with_capacity(MAX_STAGE_TEXTURES + 1);
        parts.push(material.name.clone());
        for stage in 0..MAX_STAGE_TEXTURES {
            if let Some(name) = Self::stage_texture_name(material, stage) {
                parts.push(name.clone());
            } else if let Some(fallback) = &material.texture_name {
                parts.push(fallback.clone());
            } else {
                parts.push("default".to_string());
            }
        }
        parts.join("|")
    }

    /// Access the wgpu queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Clone the Arc-wrapped queue for systems that need shared ownership.
    pub fn queue_arc(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    /// Back buffer color format provided by the WW3D engine.
    pub fn color_format(&self) -> wgpu::TextureFormat {
        self.color_format
    }

    /// Depth format used by the swapchain if available.
    pub fn depth_format(&self) -> Option<wgpu::TextureFormat> {
        self.depth_format
    }
}

/// Graphics statistics - equivalent to C++ GraphicsSystem::Statistics
#[derive(Debug, Clone)]
pub struct GraphicsStatistics {
    pub frame_count: u64,
    pub triangles_rendered: u64,
    pub draw_calls: u64,
    pub models_cached: usize,
    pub materials_cached: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_cube_is_neutral_not_magenta() {
        let fallback = GraphicsSystem::create_fallback_cube_model();
        let material = &fallback.meshes[0].material;

        assert_eq!(material.diffuse_color, Vec3::new(0.58, 0.58, 0.58));
        assert_eq!(material.emissive_color, Vec3::ZERO);
    }

    #[test]
    fn test_fallback_model_name_marker_stays_stable() {
        assert!(GraphicsSystem::is_fallback_model("__fallback_cube__"));
        assert!(!GraphicsSystem::is_fallback_model("unit_tank"));
    }

    #[test]
    fn test_missing_retail_model_does_not_resolve_to_diagnostic_cube() {
        let fallback = Arc::new(GraphicsSystem::create_fallback_cube_model());
        let mut loaded_models = HashMap::new();
        loaded_models.insert("__fallback_cube__".to_string(), Arc::clone(&fallback));

        assert!(
            GraphicsSystem::lookup_model_or_explicit_fallback(&loaded_models, "unit_tank")
                .is_none(),
            "normal missing model lookups must not synthesize diagnostic cube geometry"
        );
        assert!(Arc::ptr_eq(
            &GraphicsSystem::lookup_model_or_explicit_fallback(&loaded_models, "__fallback_cube__")
                .expect("explicit fallback lookup should return the diagnostic cube"),
            &fallback
        ));
    }
}
