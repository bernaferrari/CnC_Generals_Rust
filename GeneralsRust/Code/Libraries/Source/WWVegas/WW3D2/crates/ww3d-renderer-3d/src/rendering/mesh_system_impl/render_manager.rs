#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

#[derive(Clone)]
pub struct PreparedMeshModel {
    vertex_buffer: Arc<wgpu::Buffer>,
    index_buffer: Option<Arc<wgpu::Buffer>>,
    vertex_count: u32,
    index_count: u32,
    material_passes: Vec<MaterialPassClass>,
    is_skinned: bool,
    source_revision: u64,
    /// Index ranges for each material pass (start_index, count)
    /// Maps pass index to (start_index, index_count) for filtering draw calls
    pass_index_ranges: Vec<(u32, u32)>,
}

impl PreparedMeshModel {
    fn frommodel(device: &wgpu::Device, model: &MeshModelClass) -> W3dResult<Self> {
        let vertex_count = model.vertices.len() as u32;
        let has_normals = model.has_normals();
        let has_tex_coords = model.has_tex_coords();
        let is_skinned = model.is_skinned();

        let mut stride = 3;
        if has_normals {
            stride += 3;
        }
        if has_tex_coords {
            stride += 2;
        }
        if is_skinned {
            stride += 8; // 4 indices + 4 weights as floats
        }

        let mut vertex_data: Vec<f32> = Vec::with_capacity(model.vertices.len() * stride);
        for index in 0..model.vertices.len() {
            let vertex = &model.vertices[index];
            vertex_data.push(vertex.x);
            vertex_data.push(vertex.y);
            vertex_data.push(vertex.z);

            if has_normals {
                if index < model.normals.len() {
                    let normal = &model.normals[index];
                    vertex_data.push(normal.x);
                    vertex_data.push(normal.y);
                    vertex_data.push(normal.z);
                } else {
                    vertex_data.push(0.0);
                    vertex_data.push(1.0);
                    vertex_data.push(0.0);
                }
            }

            if has_tex_coords {
                if index < model.texture_coords.len() {
                    let tex = &model.texture_coords[index];
                    vertex_data.push(tex.u);
                    vertex_data.push(tex.v);
                } else {
                    vertex_data.push(0.0);
                    vertex_data.push(0.0);
                }
            }

            if is_skinned {
                let (indices, weights) = model.vertex_influence_view(index);
                for &idx in &indices {
                    vertex_data.push(f32::from_bits(idx));
                }
                vertex_data.extend_from_slice(&weights);
            }
        }

        let vertex_buffer = if vertex_data.is_empty() {
            Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Empty Mesh Vertex Buffer"),
                size: 4,
                usage: wgpu::BufferUsages::VERTEX,
                mapped_at_creation: false,
            }))
        } else {
            Arc::new(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Mesh Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertex_data),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            )
        };

        let mut index_data: Vec<u32> = Vec::with_capacity(model.triangles.len() * 3);
        for triangle in &model.triangles {
            index_data.push(triangle.vindex[0]);
            index_data.push(triangle.vindex[1]);
            index_data.push(triangle.vindex[2]);
        }

        let (index_buffer, index_count) = if index_data.is_empty() {
            (None, 0)
        } else {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh Index Buffer"),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
            });
            (Some(Arc::new(buffer)), index_data.len() as u32)
        };

        let material_passes = if model.material_passes.is_empty() {
            vec![MaterialPassClass::new()]
        } else {
            model.material_passes.clone()
        };

        // Compute per-pass index ranges from polygon renderer list
        // This ensures we only draw geometry belonging to each pass
        let pass_index_ranges = compute_pass_index_ranges(model, &index_data);

        Ok(Self {
            vertex_buffer,
            index_buffer,
            vertex_count,
            index_count,
            material_passes,
            is_skinned,
            source_revision: model.revision(),
            pass_index_ranges,
        })
    }
}

struct MeshFallbackTextures {
    _texture_2d: Arc<wgpu::Texture>,
    view_2d: Arc<wgpu::TextureView>,
    _texture_cube: Arc<wgpu::Texture>,
    view_cube: Arc<wgpu::TextureView>,
}

struct StageResources {
    view_2d: Arc<wgpu::TextureView>,
    view_cube: Arc<wgpu::TextureView>,
    sampler: Arc<wgpu::Sampler>,
}

struct VertexColorResources {
    bind_group: Arc<wgpu::BindGroup>,
    diffuse_buffer: Arc<wgpu::Buffer>,
    illumination_buffer: Arc<wgpu::Buffer>,
}

#[derive(Default)]
pub struct RenderPassResources {
    buffers: Vec<Arc<wgpu::Buffer>>,
    bind_groups: Vec<Arc<wgpu::BindGroup>>,
    pipelines: Vec<Arc<wgpu::RenderPipeline>>,
}

impl RenderPassResources {
    fn clear(&mut self) {
        self.buffers.clear();
        self.bind_groups.clear();
        self.pipelines.clear();
    }

    fn set_vertex_buffer(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        slot: u32,
        buffer: Arc<wgpu::Buffer>,
    ) {
        self.buffers.push(buffer);
        let ptr = Arc::as_ptr(self.buffers.last().expect("buffer guard stored"));
        unsafe { render_pass.set_vertex_buffer(slot, (&*ptr).slice(..)) }
    }

    fn set_index_buffer(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        buffer: Arc<wgpu::Buffer>,
        format: wgpu::IndexFormat,
    ) {
        self.buffers.push(buffer);
        let ptr = Arc::as_ptr(self.buffers.last().expect("buffer guard stored"));
        unsafe { render_pass.set_index_buffer((&*ptr).slice(..), format) }
    }

    fn retain_buffer(&mut self, buffer: Arc<wgpu::Buffer>) {
        self.buffers.push(buffer);
    }

    fn set_bind_group(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        slot: u32,
        bind_group: Arc<wgpu::BindGroup>,
    ) {
        self.bind_groups.push(bind_group);
        let ptr = Arc::as_ptr(self.bind_groups.last().expect("bind group guard stored"));
        unsafe { render_pass.set_bind_group(slot, &*ptr, &[]) }
    }

    fn set_pipeline(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        pipeline: Arc<wgpu::RenderPipeline>,
    ) {
        self.pipelines.push(pipeline);
        let ptr = Arc::as_ptr(self.pipelines.last().expect("pipeline guard stored"));
        unsafe { render_pass.set_pipeline(&*ptr) }
    }
}

pub struct MeshRenderManager {
    gpu_device: Arc<GpuDevice>,
    preparedmodels: HashMap<usize, Arc<PreparedMeshModel>>,
    stats: MeshRenderStats,
    pipeline_mgr: WgpuPipelineManager,
    asset_manager: Option<Arc<Mutex<AssetManager>>>,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    fallback_textures: MeshFallbackTextures,
    default_sampler: Arc<wgpu::Sampler>,
    empty_vertex_color_buffer: Arc<wgpu::Buffer>,
    decal_queue: Vec<Arc<MeshClass>>,
    fvf_containers: Vec<Arc<DX8FVFCategoryContainer>>,
    live_csm: crate::rendering::shadow_system::live_cascade_shadow::LiveCascadeShadowMap,
}

impl MeshRenderManager {
    pub fn new(gpu_device: Arc<GpuDevice>) -> Self {
        let pipeline_mgr = WgpuPipelineManager::new(gpu_device.clone());
        let device = gpu_device.wgpu_device();
        let queue = gpu_device.queue();
        let fallback_textures = Self::create_fallback_textures(device, queue);
        let default_sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MeshManager Default Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }));

        let empty_vertex_color_buffer = Arc::new(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("MeshManager Empty Vertex Color Buffer"),
                contents: bytemuck::cast_slice(&[0.0f32; 4]),
                usage: wgpu::BufferUsages::STORAGE,
            },
        ));
        let live_csm =
            crate::rendering::shadow_system::live_cascade_shadow::LiveCascadeShadowMap::new(
                device,
            );

        Self {
            gpu_device,
            preparedmodels: HashMap::new(),
            stats: MeshRenderStats::default(),
            pipeline_mgr,
            asset_manager: None,
            color_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_format: Some(wgpu::TextureFormat::Depth32Float),
            fallback_textures,
            default_sampler,
            empty_vertex_color_buffer,
            decal_queue: Vec::new(),
            fvf_containers: Vec::new(),
            live_csm,
        }
    }

    pub fn ensure_model(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<()> {
        self.prepare_model(model).map(|_| ())
    }

    fn prepare_model(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<Arc<PreparedMeshModel>> {
        let key = Arc::as_ptr(model) as usize;
        if !self.preparedmodels.contains_key(&key) {
            let prepared = Arc::new(PreparedMeshModel::frommodel(
                self.gpu_device.wgpu_device(),
                model.as_ref(),
            )?);
            self.preparedmodels.insert(key, prepared);
        }
        Ok(self
            .preparedmodels
            .get(&key)
            .expect("prepared model must exist")
            .clone())
    }

    pub fn set_asset_manager(
        &mut self,
        asset_manager: Arc<Mutex<AssetManager>>,
    ) -> RendererResult<()> {
        self.asset_manager = Some(asset_manager);
        Ok(())
    }

    fn create_fallback_textures(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> MeshFallbackTextures {
        let white_pixel: [u8; 4] = [255, 255, 255, 255];

        let texture_2d = Arc::new(device.create_texture(&TextureDescriptor {
            label: Some("MeshManager Fallback Texture 2D"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        }));
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: texture_2d.as_ref(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixel,
            TexelCopyBufferLayout {
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
        let view_2d = Arc::new(texture_2d.create_view(&TextureViewDescriptor::default()));

        let texture_cube = Arc::new(device.create_texture(&TextureDescriptor {
            label: Some("MeshManager Fallback Texture Cube"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        }));

        for layer in 0..6 {
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: texture_cube.as_ref(),
                    mip_level: 0,
                    origin: Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &white_pixel,
                TexelCopyBufferLayout {
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
        }

        let view_cube = Arc::new(texture_cube.create_view(&TextureViewDescriptor {
            label: Some("MeshManager Fallback Cube View"),
            dimension: Some(TextureViewDimension::Cube),
            ..Default::default()
        }));

        MeshFallbackTextures {
            _texture_2d: texture_2d,
            view_2d,
            _texture_cube: texture_cube,
            view_cube,
        }
    }

    pub fn ensuremodel(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<()> {
        self.preparemodel(model).map(|_| ())
    }

    fn preparemodel(&mut self, model: &Arc<MeshModelClass>) -> W3dResult<Arc<PreparedMeshModel>> {
        let key = Arc::as_ptr(model) as usize;
        let current_revision = model.revision();
        let needs_rebuild = self
            .preparedmodels
            .get(&key)
            .map(|prepared| prepared.source_revision != current_revision)
            .unwrap_or(true);

        if needs_rebuild {
            let prepared = Arc::new(PreparedMeshModel::frommodel(
                self.gpu_device.wgpu_device(),
                model.as_ref(),
            )?);
            self.preparedmodels.insert(key, prepared);
        }
        Ok(self
            .preparedmodels
            .get(&key)
            .expect("prepared model must exist")
            .clone())
    }

    pub fn get_stats(&self) -> &MeshRenderStats {
        &self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = MeshRenderStats::default();
    }

    pub fn set_render_formats(
        &mut self,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
    ) {
        self.color_format = color_format;
        self.depth_format = depth_format;
    }

    pub fn render_pass(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        opaque_meshes: &[Arc<MeshClass>],
        blended_meshes: &[Arc<MeshClass>],
        render_info: &RenderInfoClass,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        let mut pass_resources = RenderPassResources::default();
        for mesh in opaque_meshes {
            self.render_mesh(mesh, render_info, render_pass, arena, &mut pass_resources)?;
        }
        for mesh in blended_meshes {
            self.render_mesh(mesh, render_info, render_pass, arena, &mut pass_resources)?;
        }
        Ok(())
    }

    fn render_mesh(
        &mut self,
        mesh: &Arc<MeshClass>,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        if mesh.model.is_none() {
            return Ok(());
        }
        self.stats.meshes_rendered += 1;

        let prepared = {
            let model = mesh.model.as_ref().unwrap();
            self.preparemodel(model)?
        };

        resources.set_vertex_buffer(render_pass, 0, Arc::clone(&prepared.vertex_buffer));

        if let Some(index_buffer) = prepared.index_buffer.as_ref() {
            resources.set_index_buffer(
                render_pass,
                Arc::clone(index_buffer),
                wgpu::IndexFormat::Uint32,
            );
        }

        for pass in &prepared.material_passes {
            self.draw_material_pass(
                mesh,
                &prepared,
                pass,
                render_info,
                render_pass,
                arena,
                resources,
            )?;
        }

        for extra_pass in &render_info.additional_material_passes {
            self.draw_material_pass(
                mesh,
                &prepared,
                extra_pass,
                render_info,
                render_pass,
                arena,
                resources,
            )?;
        }

        resources.clear();
        Ok(())
    }

    fn material_pass_with_uv_offset(
        pass: &MaterialPassClass,
        offset: [f32; 2],
    ) -> MaterialPassClass {
        let mut pass = pass.clone();
        // C++ tread draw disables the automatic LinearOffset mapper and pushes
        // the runtime offset as custom UV state. The shader's static grid mapper
        // path is the existing per-draw uniform route for an absolute UV offset.
        pass.set_mapper_id(7);
        pass.set_mapper_arg(0, 1);
        pass.set_mapper_arg(1, 1);
        pass.set_mapper_arg(2, (offset[0] * 1000.0).round() as i32);
        pass.set_mapper_arg(3, (offset[1] * 1000.0).round() as i32);
        pass
    }

    fn draw_material_pass(
        &mut self,
        mesh: &MeshClass,
        prepared: &PreparedMeshModel,
        pass: &MaterialPassClass,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        let uv_override_pass = mesh
            .uv_offset_override()
            .map(|offset| Self::material_pass_with_uv_offset(pass, offset));
        let pass = uv_override_pass.as_ref().unwrap_or(pass);
        let stage_masks = compute_stage_masks(pass);

        let vertex_format = if prepared.is_skinned {
            VertexFormat::Skinned
        } else {
            VertexFormat::Basic
        };
        let force_two_sided = mesh.is_decal_instance
            || render_info.override_flags.intersects(
                RenderInfoOverrideFlags::FORCE_TWO_SIDED | RenderInfoOverrideFlags::DECAL_RENDERING,
            );

        let pipeline = self.pipeline_mgr.get_or_create(
            &pass.shader,
            stage_masks.mask,
            prepared.is_skinned,
            render_info.lighting.is_some(),
            render_info.fog.is_some(),
            wgpu::PrimitiveTopology::TriangleList,
            vertex_format,
            self.color_format,
            self.depth_format,
            0,
            force_two_sided,
        );

        let camera_binds = WgpuMaterialBinds::camera(
            self.gpu_device.as_ref(),
            pipeline.as_ref(),
            0,
            arena,
            render_info,
        )?;

        let (material_diffuse, material_specular, material_emissive) =
            material_properties(pass.get_vertex_material());
        let material_overrides = [
            render_info.alpha_override,
            render_info.material_pass_alpha_override,
            render_info.material_pass_emissive_override,
            0.0,
        ];

        let model_binds = WgpuMaterialBinds::model(
            self.gpu_device.as_ref(),
            pipeline.as_ref(),
            1,
            &mesh.transform,
            render_info,
            stage_masks.mask,
            stage_masks.cube_mask,
            stage_masks.hints,
            stage_masks.alpha_mask,
            stage_masks.uv_channels,
            material_diffuse,
            material_specular,
            material_emissive,
            material_overrides,
            arena,
            // Default FOW values (fully visible) - will be overridden when FOW is integrated
            None, // visibility_alpha
            None, // visibility_falloff
            None, // is_explored
            Some(&self.live_csm),
        )?;
        resources.set_pipeline(render_pass, Arc::clone(&pipeline));

        resources.retain_buffer(Arc::clone(&camera_binds.buffer));
        resources.set_bind_group(render_pass, 0, Arc::clone(&camera_binds.bind_group));

        resources.retain_buffer(Arc::clone(&model_binds.model_buffer));
        resources.retain_buffer(Arc::clone(&model_binds.lighting_buffer));
        resources.set_bind_group(render_pass, 1, Arc::clone(&model_binds.bind_group));

        let next_slot = 3u32;
        if prepared.is_skinned {
            let identity_palette = [Mat4::IDENTITY];
            let palette = mesh
                .bone_palette_view()
                .map(|view| view.matrices)
                .filter(|matrices| !matrices.is_empty())
                .unwrap_or(&identity_palette);

            let binds = WgpuMaterialBinds::skinned_group2(
                self.gpu_device.as_ref(),
                pipeline.as_ref(),
                2,
                palette,
                Some(pass),
                render_info.time,
                arena,
            )?;
            resources.retain_buffer(Arc::clone(&binds.bones_buffer));
            resources.retain_buffer(Arc::clone(&binds.uv_transform_buffer));
            resources.set_bind_group(render_pass, 2, Arc::clone(&binds.bind_group));
        } else {
            // Non-skinned shaders expect UV transform at group 2.
            let uv_transform_binds = WgpuMaterialBinds::uv_transform(
                self.gpu_device.wgpu_device(),
                pipeline.as_ref(),
                2,
                Some(pass),
                render_info.time,
            )?;
            resources.retain_buffer(Arc::clone(&uv_transform_binds.buffer));
            resources.set_bind_group(render_pass, 2, Arc::clone(&uv_transform_binds.bind_group));
        }

        let texture_bind_groups =
            self.create_texture_bind_groups(pipeline.as_ref(), pass, next_slot);
        for (offset, bind_group) in texture_bind_groups.iter().enumerate() {
            resources.set_bind_group(
                render_pass,
                next_slot + offset as u32,
                Arc::clone(bind_group),
            );
        }
        let color_group_index = next_slot + texture_bind_groups.len() as u32;
        let vertex_color =
            self.create_vertex_color_resources(pipeline.as_ref(), pass, color_group_index);
        resources.retain_buffer(Arc::clone(&vertex_color.diffuse_buffer));
        resources.retain_buffer(Arc::clone(&vertex_color.illumination_buffer));
        resources.set_bind_group(
            render_pass,
            color_group_index,
            Arc::clone(&vertex_color.bind_group),
        );

        if pass
            .diffuse_vertex_colors
            .as_ref()
            .map(|colors| !colors.is_empty())
            .unwrap_or(false)
            || pass
                .illumination_vertex_colors
                .as_ref()
                .map(|colors| !colors.is_empty())
                .unwrap_or(false)
        {
            self.stats.vertex_color_passes += 1;
        }

        self.issue_draw_call(prepared, pass, render_pass);

        self.stats.material_passes += 1;
        self.stats.shader_switches += 1;
        if stage_masks.mask != 0 {
            self.stats.texture_switches += 1;
        }

        Ok(())
    }

    // helper slots intentionally minimal; temporary bindings are stored in local vectors to ensure
    // they outlive the render pass borrow.

    fn issue_draw_call(
        &mut self,
        prepared: &PreparedMeshModel,
        pass: &MaterialPassClass,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) {
        // Get the pass index for filtering
        let pass_index = pass.get_pass_index();

        // Find the index range for this specific pass
        let (start_index, count) = if pass_index < prepared.pass_index_ranges.len() {
            prepared.pass_index_ranges[pass_index]
        } else if !prepared.pass_index_ranges.is_empty() {
            // Fallback to first range if pass index is out of bounds
            prepared.pass_index_ranges[0]
        } else if prepared.index_count > 0 {
            // Fallback: render all indices (backward compatibility)
            (0, prepared.index_count)
        } else {
            // Empty mesh
            (0, 0)
        };

        if prepared.index_buffer.is_some() && count > 0 {
            // Draw only the indices for this specific pass
            render_pass.draw_indexed(start_index..start_index + count, 0, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += count / 3;
        } else if count > 0 {
            // For non-indexed rendering, we can't easily filter by pass
            render_pass.draw(0..prepared.vertex_count, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += prepared.vertex_count / 3;
        }
    }

    fn create_vertex_color_resources(
        &self,
        pipeline: &wgpu::RenderPipeline,
        pass: &MaterialPassClass,
        group_index: u32,
    ) -> VertexColorResources {
        let device = self.gpu_device.wgpu_device();

        let diffuse_buffer = pass.diffuse_vertex_colors.as_ref().map(|colors| {
            let mut data = Vec::with_capacity(colors.len() * 4);
            for color in colors {
                data.push(color.x);
                data.push(color.y);
                data.push(color.z);
                data.push(color.w);
            }
            Arc::new(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("MeshManager Diffuse Vertex Colors"),
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::STORAGE,
                }),
            )
        });

        let illumination_buffer = pass.illumination_vertex_colors.as_ref().map(|colors| {
            let mut data = Vec::with_capacity(colors.len() * 4);
            for color in colors {
                data.push(color.x);
                data.push(color.y);
                data.push(color.z);
                data.push(color.w);
            }
            Arc::new(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("MeshManager Illumination Vertex Colors"),
                    contents: bytemuck::cast_slice(&data),
                    usage: wgpu::BufferUsages::STORAGE,
                }),
            )
        });

        let diffuse_buffer =
            diffuse_buffer.unwrap_or_else(|| self.empty_vertex_color_buffer.clone());
        let illumination_buffer =
            illumination_buffer.unwrap_or_else(|| self.empty_vertex_color_buffer.clone());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MeshManager Vertex Color Bind Group"),
            layout: &pipeline.get_bind_group_layout(group_index),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: diffuse_buffer.as_ref().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: illumination_buffer.as_ref().as_entire_binding(),
                },
            ],
        });

        VertexColorResources {
            bind_group: Arc::new(bind_group),
            diffuse_buffer,
            illumination_buffer,
        }
    }

    fn create_texture_bind_groups(
        &self,
        pipeline: &wgpu::RenderPipeline,
        pass: &MaterialPassClass,
        first_group_index: u32,
    ) -> Vec<Arc<wgpu::BindGroup>> {
        let mut bind_groups = Vec::with_capacity(MAX_TEXTURE_STAGE_GROUPS);
        for group in 0..MAX_TEXTURE_STAGE_GROUPS {
            let layout = pipeline.get_bind_group_layout(first_group_index + group as u32);
            let stage_base = group * TEXTURES_PER_GROUP;
            let mut views_2d: Vec<Arc<wgpu::TextureView>> = Vec::with_capacity(TEXTURES_PER_GROUP);
            let mut views_cube: Vec<Arc<wgpu::TextureView>> =
                Vec::with_capacity(TEXTURES_PER_GROUP);
            let mut samplers: Vec<Arc<wgpu::Sampler>> = Vec::with_capacity(TEXTURES_PER_GROUP);

            for stage_offset in 0..TEXTURES_PER_GROUP {
                let stage_index = stage_base + stage_offset;
                let resources = self.stage_resources_for(pass, stage_index);
                views_2d.push(resources.view_2d);
                views_cube.push(resources.view_cube);
                samplers.push(resources.sampler);
            }

            let mut entries = Vec::with_capacity(TEXTURES_PER_GROUP * 3);
            for stage_offset in 0..TEXTURES_PER_GROUP {
                let binding_base = (stage_offset * 3) as u32;
                entries.push(wgpu::BindGroupEntry {
                    binding: binding_base,
                    resource: wgpu::BindingResource::TextureView(views_2d[stage_offset].as_ref()),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: binding_base + 1,
                    resource: wgpu::BindingResource::TextureView(views_cube[stage_offset].as_ref()),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: binding_base + 2,
                    resource: wgpu::BindingResource::Sampler(samplers[stage_offset].as_ref()),
                });
            }

            let bind_group =
                self.gpu_device
                    .wgpu_device()
                    .create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("MeshManager Texture Bind Group"),
                        layout: &layout,
                        entries: &entries,
                    });

            bind_groups.push(Arc::new(bind_group));
        }

        bind_groups
    }

    fn stage_resources_for(&self, pass: &MaterialPassClass, stage: usize) -> StageResources {
        if let Some(texture) = pass.get_texture(stage) {
            if let Some(view) = texture.get_texture_view() {
                let sampler_desc = sampler_descriptor_for_settings(&texture.stage_settings);
                let sampler = Arc::new(self.gpu_device.wgpu_device().create_sampler(&sampler_desc));
                return StageResources {
                    view_2d: Arc::new(view),
                    view_cube: self.fallback_textures.view_cube.clone(),
                    sampler,
                };
            }
        }

        StageResources {
            view_2d: self.fallback_textures.view_2d.clone(),
            view_cube: self.fallback_textures.view_cube.clone(),
            sampler: self.default_sampler.clone(),
        }
    }

    pub fn render_polygon_renderer<'rp>(
        &mut self,
        polygon_renderer: &'rp Arc<DX8PolygonRendererClass>,
        render_pass: &mut wgpu::RenderPass<'rp>,
    ) -> W3dResult<()> {
        if let Some(ref vertex_buffer) = polygon_renderer.vertex_buffer {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        }

        if let Some(ref index_buffer) = polygon_renderer.index_buffer {
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..polygon_renderer.index_count, 0, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += polygon_renderer.index_count / 3;
        } else {
            render_pass.draw(0..polygon_renderer.vertex_count, 0..1);
            self.stats.draw_calls += 1;
            self.stats.triangles_rendered += polygon_renderer.vertex_count / 3;
        }

        Ok(())
    }

    fn render_texture_category(
        &mut self,
        category: &Arc<DX8TextureCategoryClass>,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        let tasks = {
            let mut guard = category
                .render_tasks
                .lock()
                .expect("texture category render tasks mutex poisoned");
            if guard.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut *guard)
        };

        for task in tasks {
            self.render_mesh(&task.mesh, render_info, render_pass, arena, resources)?;
        }
        Ok(())
    }

    fn render_fvf_category_container(
        &mut self,
        container: &DX8FVFCategoryContainer,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        for category in container.texture_categories.values() {
            self.render_texture_category(category, render_info, render_pass, arena, resources)?;
        }
        Ok(())
    }

    fn render_delayed_passes(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        if let Some((entries, sort_levels)) = StaticSortManager::snapshot_static_sort_list() {
            let mut buckets: BTreeMap<u32, Vec<StaticSortEntry>> = BTreeMap::new();
            for (entry, sort_level) in entries.into_iter().zip(sort_levels) {
                buckets.entry(sort_level).or_default().push(entry);
            }

            for (_level, bucket_entries) in buckets.into_iter().rev() {
                for entry in bucket_entries {
                    if let Some(mesh) = entry.mesh_arc() {
                        self.render_mesh(&mesh, render_info, render_pass, arena, resources)?;
                    } else {
                        let render_obj = entry.render_object();
                        render_obj.render(render_info)?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn flush_static_sort_lists(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        let mut resources = RenderPassResources::default();
        self.render_delayed_passes(render_info, render_pass, arena, &mut resources)?;
        StaticSortManager::flush_static_sort_list();
        Ok(())
    }

    pub fn add_decal_to_queue(&mut self, decal: Arc<MeshClass>) {
        self.decal_queue.push(decal);
    }

    pub fn register_fvf_container(&mut self, container: Arc<DX8FVFCategoryContainer>) {
        self.fvf_containers.push(container);
    }

    pub fn render_decal_queue(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        if self.decal_queue.is_empty() {
            return Ok(());
        }

        let camera_pos = render_info.camera.get_position();
        self.decal_queue.sort_by(|a, b| {
            let dist_a = a.get_bounding_sphere().center.distance(camera_pos);
            let dist_b = b.get_bounding_sphere().center.distance(camera_pos);
            dist_b
                .partial_cmp(&dist_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let decals = std::mem::take(&mut self.decal_queue);
        let mut resources = RenderPassResources::default();
        for decal in decals {
            self.render_mesh(&decal, render_info, render_pass, arena, &mut resources)?;
        }
        Ok(())
    }

    pub fn render_all_fvf_containers(
        &mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
    ) -> W3dResult<()> {
        let mut resources = RenderPassResources::default();
        let containers = self.fvf_containers.clone();
        for container in &containers {
            self.render_fvf_category_container(
                container,
                render_info,
                render_pass,
                arena,
                &mut resources,
            )?;
        }
        Ok(())
    }

    pub fn clear_frame_data(&mut self) {
        self.decal_queue.clear();
        self.fvf_containers.clear();
    }
}
