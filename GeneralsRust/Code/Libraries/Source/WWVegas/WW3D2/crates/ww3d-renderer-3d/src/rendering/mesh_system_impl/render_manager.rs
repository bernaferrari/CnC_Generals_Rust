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

/// C++ `WW3DAssetManager::Get_Texture` parity (W3DAssetManager.cpp:127-225):
/// resolves a W3D pass-texture name to a hydrated `TextureClass` (decoded
/// RGBA8 pixels). The host installs an archive-backed implementation; the mesh
/// manager defers to it when a pass texture carries only its W3D name, then
/// uploads the returned pixels through the same first-bind path as CPU-only
/// pass textures.

static UTBVIEW_ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::env::var("GENERALS_UTBVIEW").as_deref() == Ok("1")
});
static UTBVIEW_SEEN: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<(String, &'static str)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

// UTBMAT (documented diagnostic, GENERALS_UTBMAT=1): once per model, log the
// material uniforms and stage masks that compose the mesh fragment color.
static UTBMAT_ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
    std::env::var("GENERALS_UTBMAT").as_deref() == Ok("1")
});
static UTBMAT_SEEN: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));
pub type MeshPassTextureProvider =
    Arc<dyn Fn(&str) -> Option<TextureClass> + Send + Sync>;


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
        let is_skinned = model.is_skinned();

        // The WGPU compatibility layouts always expose a normal and four UV
        // channels (the same layout used by the normal and projected-shroud
        // shaders).  The old variable-width packing only emitted authored
        // channels, so a mesh without UVs or with fewer than four channels
        // made the GPU read subsequent vertices as attributes.  Keep the
        // source data faithful while padding absent channels with the same
        // defaults as MeshModelClass::create_wgpu_buffers.
        const MAX_VERTEX_UV_SETS: usize = 4;
        let mut stride = 3 + 3 + (MAX_VERTEX_UV_SETS * 2);
        if is_skinned {
            stride += 8; // 4 indices + 4 weights as floats
        }

        let mut vertex_data: Vec<f32> = Vec::with_capacity(model.vertices.len() * stride);
        for index in 0..model.vertices.len() {
            let vertex = &model.vertices[index];
            vertex_data.push(vertex.x);
            vertex_data.push(vertex.y);
            vertex_data.push(vertex.z);

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

            for channel in 0..MAX_VERTEX_UV_SETS {
                // UTBUVCENTER (documented diagnostic, GENERALS_UTBUVCENTER=1):
                // pin every UV to the texture center. If units render textured
                // under this env while authored UVs render flat texel0-white,
                // the UV delivery path (vertex attribute -> sampler) is broken.
                let [u, v] = if std::env::var("GENERALS_UTBUVCENTER").as_deref() == Ok("1") {
                    [0.5, 0.5]
                } else {
                    model.uv_channel_coords(channel, index)
                };
                vertex_data.push(u);
                vertex_data.push(v);
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

        // UTBVERTS (documented diagnostic, GENERALS_UTBVERTS=1, once per
        // model): dump the packed geometry's extent so a collapsed/NaN vertex
        // array (invisible meshes) is visible CPU-side without GPU readback.
        if std::env::var("GENERALS_UTBVERTS").as_deref() == Ok("1")
            && !model.vertices.is_empty()
        {
            static UTBVERTS_SEEN: std::sync::LazyLock<
                std::sync::Mutex<std::collections::HashSet<String>>,
            > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));
            let mut min = [f32::MAX; 3];
            let mut max = [f32::MIN; 3];
            for vertex in &model.vertices {
                min[0] = min[0].min(vertex.x);
                min[1] = min[1].min(vertex.y);
                min[2] = min[2].min(vertex.z);
                max[0] = max[0].max(vertex.x);
                max[1] = max[1].max(vertex.y);
                max[2] = max[2].max(vertex.z);
            }
            let key = format!(
                "model='{}' verts={} tris={} bbox=({:.2}..{:.2}, {:.2}..{:.2}, {:.2}..{:.2}) v0=({:.2},{:.2},{:.2}) ranges={:?}",
                model.name,
                model.vertices.len(),
                model.triangles.len(),
                min[0], max[0], min[1], max[1], min[2], max[2],
                model.vertices[0].x, model.vertices[0].y, model.vertices[0].z,
                pass_index_ranges,
            );
            let unseen = UTBVERTS_SEEN
                .lock()
                .map(|mut seen| seen.insert(key.clone()))
                .unwrap_or(false);
            if unseen {
                log::warn!("UTBVERTS {}", key);
            }
        }

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
        // SAFETY: the Arc was just pushed into `self.buffers`, so the slot
        // behind `ptr` holds a strong ref keeping the wgpu::Buffer alive; the
        // struct retains every pushed buffer until `clear()` after the render
        // pass ends, meeting wgpu's binding-valid-through-the-pass contract.
        // The slice spans the whole buffer, so it is within its size.
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
        // SAFETY: the Arc was just pushed into `self.buffers`, so the slot
        // behind `ptr` holds a strong ref keeping the wgpu::Buffer alive, and
        // the struct retains it until `clear()` after the pass — satisfying
        // wgpu's index-binding-valid-through-the-pass rule. The `..` slice is
        // within the buffer's size.
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
        // SAFETY: the Arc was just pushed into `self.bind_groups`, so the slot
        // behind `ptr` holds a strong ref keeping the wgpu::BindGroup alive;
        // the struct retains every pushed bind group until `clear()` after the
        // pass, meeting wgpu's valid-through-the-pass rule. The empty slice
        // matches the layout's zero dynamic uniform-buffer offsets.
        unsafe { render_pass.set_bind_group(slot, &*ptr, &[]) }
    }

    fn set_pipeline(
        &mut self,
        render_pass: &mut wgpu::RenderPass<'_>,
        pipeline: Arc<wgpu::RenderPipeline>,
    ) {
        self.pipelines.push(pipeline);
        let ptr = Arc::as_ptr(self.pipelines.last().expect("pipeline guard stored"));
        // SAFETY: the Arc was just pushed into `self.pipelines`, so the slot
        // behind `ptr` holds a strong ref keeping the wgpu::RenderPipeline
        // alive; the struct retains it until `clear()` after the render pass
        // ends, meeting wgpu's pipeline-valid-through-the-pass requirement.
        unsafe { render_pass.set_pipeline(&*ptr) }
    }
}

pub struct MeshRenderManager {
    gpu_device: Arc<GpuDevice>,
    preparedmodels: HashMap<usize, Arc<PreparedMeshModel>>,
    /// Lazily uploaded GPU views for pass textures that carry CPU-only
    /// pixels. C++ parity: WW3DAssetManager::Get_Texture creates the D3D
    /// texture on first use (W3DAssetManager.cpp:127-225); the port's pass
    /// textures are built with pixel data but no GPU upload, so the mesh
    /// manager owns the first-bind upload, keyed by texture name.
    gpu_texture_views: Mutex<HashMap<String, Arc<wgpu::TextureView>>>,
    stats: MeshRenderStats,
    pipeline_mgr: WgpuPipelineManager,
    asset_manager: Option<Arc<Mutex<AssetManager>>>,
    pass_texture_provider: Option<MeshPassTextureProvider>,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    fallback_textures: MeshFallbackTextures,
    default_sampler: Arc<wgpu::Sampler>,
    empty_vertex_color_buffer: Arc<wgpu::Buffer>,
    decal_queue: Vec<Arc<MeshClass>>,
    fvf_containers: Vec<Arc<DX8FVFCategoryContainer>>,
    live_csm: crate::rendering::shadow_system::live_cascade_shadow::LiveCascadeShadowMap,
    cascade_depth_pipeline: Option<Arc<wgpu::RenderPipeline>>,
    cascade_depth_pipeline_skinned: Option<Arc<wgpu::RenderPipeline>>,

    cascade_light_bgl: Option<wgpu::BindGroupLayout>,
    cascade_model_bgl: Option<wgpu::BindGroupLayout>,
    last_cascade_casters_drawn: u32,

    /// Frozen presentation texture installed for the current frame. The
    /// dedicated rigid/skinned draw pipeline is intentionally separate work;
    /// retaining it here closes the resource-ownership ingress without a live
    /// simulation query or scalar substitute.
    projected_shroud: Option<crate::rendering::projected_shroud::FrozenProjectedShroudTexture>,
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
            crate::rendering::shadow_system::live_cascade_shadow::LiveCascadeShadowMap::new(device);

        Self {
            gpu_device,
            preparedmodels: HashMap::new(),
            gpu_texture_views: Mutex::new(HashMap::new()),
            stats: MeshRenderStats::default(),
            pipeline_mgr,
            asset_manager: None,
            pass_texture_provider: None,
            color_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            depth_format: Some(wgpu::TextureFormat::Depth32Float),
            fallback_textures,
            default_sampler,
            empty_vertex_color_buffer,
            decal_queue: Vec::new(),
            fvf_containers: Vec::new(),
            live_csm,
            cascade_depth_pipeline: None,
            cascade_depth_pipeline_skinned: None,

            cascade_light_bgl: None,
            cascade_model_bgl: None,
            last_cascade_casters_drawn: 0,

            projected_shroud: None,
        }
    }

    pub fn set_projected_shroud(
        &mut self,
        projected_shroud: Option<crate::rendering::projected_shroud::FrozenProjectedShroudTexture>,
    ) {
        self.projected_shroud = projected_shroud;
    }

    #[inline]
    pub fn projected_shroud_projection(
        &self,
    ) -> Option<crate::rendering::projected_shroud::ProjectedShroudProjection> {
        self.projected_shroud
            .as_ref()
            .map(|binding| binding.projection())
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
    /// Install the host's archive-backed pass-texture resolver (C++
    /// `WW3DAssetManager::Get_Texture` parity). Called once at renderer
    /// initialization; the mesh manager consults it only for W3D pass
    /// textures that carry a name but no pixels.
    pub fn set_pass_texture_provider(&mut self, provider: MeshPassTextureProvider) {
        self.pass_texture_provider = Some(provider);
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

    /// C++ `W3DProjectedShadowManager::updateRenderTargetTextures` fills the
    /// shadow map before the scene (W3DDisplay.cpp:1840). Clear each cascade
    /// layer, draw queued opaque casters into light-space depth, then publish
    /// matrices with `enabled = true` so opaque.wgsl PCF-samples a filled map.
    pub fn update_and_fill_live_cascade(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_info: &RenderInfoClass,
        casters: &[Arc<MeshClass>],
    ) {
        self.last_cascade_casters_drawn = 0;
        self.ensure_cascade_depth_pipeline();
        let light_direction = live_cascade_light_direction(render_info);
        self.live_csm.update(
            self.gpu_device.queue(),
            render_info.camera.get_position(),
            render_info.camera.get_forward(),
            light_direction,
            true,
        );

        if self.cascade_depth_pipeline.is_none() {
            return;
        }

        // Prepare geometry first so later layout/view borrows are immutable.
        let mut prepared_casters = Vec::new();
        for mesh in casters {
            if mesh.is_hidden || mesh.is_animation_hidden || mesh.is_decal_instance {
                continue;
            }
            let Some(model) = mesh.model.as_ref() else {
                continue;
            };
            let Ok(prepared) = self.preparemodel(model) else {
                continue;
            };
            if prepared.index_count == 0 && prepared.vertex_count == 0 {
                continue;
            }
            prepared_casters.push((prepared, mesh.transform));
        }

        let Some(pipeline) = self.cascade_depth_pipeline.clone() else {
            return;
        };
        let skinned_pipeline = self
            .cascade_depth_pipeline_skinned
            .clone()
            .unwrap_or_else(|| Arc::clone(&pipeline));
        let Some(light_bgl) = self.cascade_light_bgl.as_ref() else {
            return;
        };
        let Some(model_bgl) = self.cascade_model_bgl.as_ref() else {
            return;
        };

        let device = self.gpu_device.wgpu_device();
        let mut light_groups = Vec::with_capacity(self.live_csm.layer_views.len());
        for view_proj in &self.live_csm.uniform.view_proj {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("live_csm_light_view_proj"),
                contents: bytemuck::bytes_of(view_proj),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("live_csm_light_bg"),
                layout: light_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            light_groups.push((buffer, bind_group));
        }

        let mut model_groups = Vec::with_capacity(prepared_casters.len());
        for (prepared, transform) in prepared_casters {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("live_csm_model"),
                contents: bytemuck::bytes_of(&transform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("live_csm_model_bg"),
                layout: model_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            model_groups.push((prepared, buffer, bind_group));
        }

        for (layer, view) in self.live_csm.layer_views.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("live_csm_first_light_depth"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if let Some((_, light_bg)) = light_groups.get(layer) {
                pass.set_bind_group(0, light_bg, &[]);
            }
            for (prepared, _, model_bg) in &model_groups {
                if prepared.is_skinned {
                    pass.set_pipeline(&skinned_pipeline);
                } else {
                    pass.set_pipeline(&pipeline);
                }
                pass.set_bind_group(1, model_bg, &[]);
                pass.set_vertex_buffer(0, prepared.vertex_buffer.slice(..));
                if let Some(index_buffer) = prepared.index_buffer.as_ref() {
                    pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..prepared.index_count, 0, 0..1);
                } else if prepared.vertex_count > 0 {
                    pass.draw(0..prepared.vertex_count, 0..1);
                }
            }
        }
        self.last_cascade_casters_drawn = model_groups.len() as u32;
    }

    pub fn last_cascade_casters_drawn(&self) -> u32 {
        self.last_cascade_casters_drawn
    }

    pub fn live_cascade_enabled(&self) -> bool {
        self.live_csm.is_enabled()
    }

    fn cascade_depth_vertex_layout(skinned: bool) -> wgpu::VertexBufferLayout<'static> {
        let floats = if skinned {
            3 + 3 + (4 * 2) + 4 + 4
        } else {
            3 + 3 + (4 * 2)
        };
        wgpu::VertexBufferLayout {
            array_stride: (floats * std::mem::size_of::<f32>()) as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3,
            }],
        }
    }

    fn create_cascade_depth_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        module: &wgpu::ShaderModule,
        skinned: bool,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(if skinned {
                "live_csm_depth_pipeline_skinned"
            } else {
                "live_csm_depth_pipeline"
            }),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                buffers: &[Self::cascade_depth_vertex_layout(skinned)],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: None,
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
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 1.5,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    fn ensure_cascade_depth_pipeline(&mut self) {
        if self.cascade_depth_pipeline.is_some() {
            return;
        }
        let device = self.gpu_device.wgpu_device();
        let light_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("live_csm_light_bgl"),
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
        let model_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("live_csm_model_bgl"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("live_csm_depth_layout"),
            bind_group_layouts: &[&light_bgl, &model_bgl],
            push_constant_ranges: &[],
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("live_csm_depth_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shader_system/cascade_depth.wgsl").into(),
            ),
        });
        let rigid = Self::create_cascade_depth_pipeline(device, &layout, &module, false);
        let skinned = Self::create_cascade_depth_pipeline(device, &layout, &module, true);
        self.cascade_light_bgl = Some(light_bgl);
        self.cascade_model_bgl = Some(model_bgl);
        self.cascade_depth_pipeline = Some(Arc::new(rigid));
        self.cascade_depth_pipeline_skinned = Some(Arc::new(skinned));
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

    /// Select the lighting environment owned by this MeshClass when one has
    /// been frozen by its source Drawable.  The ordinary scene environment
    /// remains the fallback.  C++ allows a render object to carry a distinct
    /// environment (the W3D ghost branch uses its always-fogged environment),
    /// so dropping this field at the WGPU boundary would make such objects
    /// indistinguishable from normal geometry.
    fn render_info_for_mesh(mesh: &MeshClass, render_info: &RenderInfoClass) -> RenderInfoClass {
        let mut selected = render_info.clone();
        if let Some(environment) = mesh.get_lighting_environment() {
            selected.set_lighting_environment((**environment).clone());
        }
        selected
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
        let mesh_render_info = Self::render_info_for_mesh(mesh, render_info);
        let render_info = &mesh_render_info;

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

        // C++ W3DScene appends W3DShroudMaterialPass after the authored
        // material passes for a non-clear Drawable.  It is deliberately a
        // separate world-projected pass: authored material UVs must not be
        // reused for terrain shroud sampling.
        if mesh.projected_shroud_eligible() && self.projected_shroud.is_some() {
            self.draw_projected_shroud_pass(
                mesh,
                &prepared,
                render_info,
                render_pass,
                arena,
                resources,
            )?;
        }

        resources.clear();
        Ok(())
    }

    fn draw_projected_shroud_pass(
        &mut self,
        mesh: &MeshClass,
        prepared: &PreparedMeshModel,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'_>,
        arena: &mut FrameUniformArena,
        resources: &mut RenderPassResources,
    ) -> W3dResult<()> {
        let Some(projected) = self.projected_shroud.as_ref() else {
            return Ok(());
        };

        let shader =
            crate::rendering::projected_shroud::ProjectedShroudMaterialPassContract::CXX.shader();
        let vertex_format = if prepared.is_skinned {
            VertexFormat::ProjectedShroudSkinned
        } else {
            VertexFormat::ProjectedShroudBasic
        };
        let pipeline = self.pipeline_mgr.get_or_create(
            &shader,
            0,
            prepared.is_skinned,
            false,
            false,
            wgpu::PrimitiveTopology::TriangleList,
            vertex_format,
            self.color_format,
            self.depth_format,
            0,
            false,
        );

        let camera_binds = WgpuMaterialBinds::camera(
            self.gpu_device.as_ref(),
            pipeline.as_ref(),
            0,
            arena,
            render_info,
        )?;
        let projection = projected.projection();
        let model_binds = WgpuMaterialBinds::model(
            self.gpu_device.as_ref(),
            pipeline.as_ref(),
            1,
            &mesh.transform,
            render_info,
            0,
            0,
            0,
            0,
            0,
            [
                projection.uv_scale[0],
                projection.uv_scale[1],
                projection.uv_offset[0],
                projection.uv_offset[1],
            ],
            [0.0; 4],
            [0.0; 4],
            [
                projection.shroud_color[0],
                projection.shroud_color[1],
                projection.shroud_color[2],
                1.0,
            ],
            arena,
            Some(1.0),
            Some(1.0),
            Some(1.0),
            Some(&self.live_csm),
        )?;

        resources.set_pipeline(render_pass, Arc::clone(&pipeline));
        resources.retain_buffer(Arc::clone(&camera_binds.buffer));
        resources.set_bind_group(render_pass, 0, Arc::clone(&camera_binds.bind_group));
        resources.retain_buffer(Arc::clone(&model_binds.model_buffer));
        resources.retain_buffer(Arc::clone(&model_binds.lighting_buffer));
        resources.set_bind_group(render_pass, 1, Arc::clone(&model_binds.bind_group));

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
                None,
                render_info.time,
                arena,
            )?;
            resources.retain_buffer(Arc::clone(&binds.bones_buffer));
            resources.retain_buffer(Arc::clone(&binds.uv_transform_buffer));
            resources.set_bind_group(render_pass, 2, Arc::clone(&binds.bind_group));
        } else {
            let binds = WgpuMaterialBinds::uv_transform(
                self.gpu_device.wgpu_device(),
                pipeline.as_ref(),
                2,
                None,
                render_info.time,
            )?;
            resources.retain_buffer(Arc::clone(&binds.buffer));
            resources.set_bind_group(render_pass, 2, Arc::clone(&binds.bind_group));
        }

        let projected_group = self.create_projected_shroud_bind_group(pipeline.as_ref(), projected);
        resources.set_bind_group(render_pass, 3, projected_group);

        resources.set_vertex_buffer(render_pass, 0, Arc::clone(&prepared.vertex_buffer));
        if let Some(index_buffer) = prepared.index_buffer.as_ref() {
            resources.set_index_buffer(
                render_pass,
                Arc::clone(index_buffer),
                wgpu::IndexFormat::Uint32,
            );
        }

        // The additional shroud pass covers every authored material range,
        // while retaining each range's index selection and avoiding a second
        // draw of any geometry that the source model does not own.
        for pass in &prepared.material_passes {
            self.issue_draw_call(prepared, pass, render_pass, "«shroud»", [0.0; 3]);
        }
        self.stats.material_passes += prepared.material_passes.len() as u32;
        self.stats.shader_switches += 1;
        Ok(())
    }

    fn create_projected_shroud_bind_group(
        &self,
        pipeline: &wgpu::RenderPipeline,
        projected: &crate::rendering::projected_shroud::FrozenProjectedShroudTexture,
    ) -> Arc<wgpu::BindGroup> {
        let layout = pipeline.get_bind_group_layout(3);
        let shroud_view = projected.texture_view();
        let shroud_sampler = projected.sampler();
        let cube_view = &self.fallback_textures.view_cube;
        let bind_group =
            self.gpu_device
                .wgpu_device()
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Projected W3D shroud texture bind group"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(shroud_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(cube_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(shroud_sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(shroud_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(cube_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(shroud_sampler),
                        },
                    ],
                });
        Arc::new(bind_group)
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
        // UTBMESHNULL (documented diagnostic, GENERALS_UTBMESHNULL=1):
        // skip every ww3d mesh-lane material draw. If the unexplained white
        // shards vanish with this set, they are mesh-lane pixels and the
        // UTBMAGENTA tag demonstrably did not reach the pipeline that draws
        // them; if they persist, a non-mesh pass paints them.
        if std::env::var("GENERALS_UTBMESHNULL").as_deref() == Ok("1") {
            return Ok(());
        }
        // UTBLIGHT (documented diagnostic, GENERALS_UTBLIGHT=1, once per model):
        // dump the mesh-lane lighting environment at draw time to discriminate
        // lighting-uniform-scale blowout (ambient/light colors far above 1.0
        // clamp every fragment to flat white) from an upload/sampler white sample.
        static UTBLIGHT_SEEN: std::sync::LazyLock<
            std::sync::Mutex<std::collections::HashSet<String>>,
        > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));
        let utblight_first = std::env::var("GENERALS_UTBLIGHT").as_deref() == Ok("1")
            && UTBLIGHT_SEEN
                .lock()
                .map(|mut seen| seen.insert(mesh.name.clone()))
                .unwrap_or(false);
        if utblight_first {
            match render_info.lighting.as_ref() {
                Some(env) => {
                    let mut desc = format!(
                        "ambient=({:.4},{:.4},{:.4})",
                        env.ambient.x, env.ambient.y, env.ambient.z
                    );
                    for (i, light) in env.lights.iter().enumerate() {
                        if let Ok(light) = light.lock() {
                            desc.push_str(&format!(
                                " | l{} type={:?} dir=({:.3},{:.3},{:.3}) col=({:.4},{:.4},{:.4}) int={:.3} amb=({:.4},{:.4},{:.4}) enabled={}",
                                i, light.light_type,
                                light.direction.x, light.direction.y, light.direction.z,
                                light.color.x, light.color.y, light.color.z,
                                light.intensity,
                                light.ambient.x, light.ambient.y, light.ambient.z,
                                light.enabled,
                            ));
                        }
                    }
                    log::warn!("UTBLIGHT model='{}' {}", mesh.name, desc);
                }
                None => log::warn!("UTBLIGHT model='{}' lighting=NONE", mesh.name),
            }
        }
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

        // C++ Drawable::setStealthLook drives the instance opacity while
        // preserving the authored material.  Opaque W3D materials therefore
        // need a per-instance alpha variant; the cloned ShaderClass changes
        // only the blend bits, so the cached MaterialPass remains untouched.
        let mut pipeline_shader = pass.shader;
        if mesh.presentation_opacity() < 0.999
            && matches!(pass.shader.blend_mode(), MaterialBlendMode::Opaque)
        {
            pipeline_shader.set_alpha_blend_enable(true);
        }

        // UTBNODEPTH (documented diagnostic, GENERALS_UTBNODEPTH=1):
        // create mesh pipelines with no depth attachment testing; if invisible
        // units pop into view with this set they were depth-occluded (draw
        // order/depth-state bug), if they stay hidden the geometry itself is
        // degenerate or off-screen.
        let utb_nodepth = std::env::var("GENERALS_UTBNODEPTH").as_deref() == Ok("1");
        let pipeline = self.pipeline_mgr.get_or_create(
            &pipeline_shader,
            stage_masks.mask,
            prepared.is_skinned,
            render_info.lighting.is_some(),
            render_info.fog.is_some(),
            wgpu::PrimitiveTopology::TriangleList,
            vertex_format,
            self.color_format,
            if utb_nodepth { None } else { self.depth_format },
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

        let (mut material_diffuse, material_specular, material_emissive) =
            material_properties(pass.get_vertex_material());
        let material_overrides = [
            render_info.alpha_override * mesh.presentation_opacity(),
            render_info.material_pass_alpha_override,
            render_info.material_pass_emissive_override,
            0.0,
        ];
        let (visibility_alpha, visibility_falloff, is_explored) =
            mesh.frozen_fow_visibility().model_uniform_values();
        // UTBMAT (documented diagnostic, GENERALS_UTBMAT=1, once per model):
        // dump the per-model material uniforms that multiply the sampled
        // albedo, to discriminate a GPU material/emissive white-out from an
        // upload-path white sample.
        if *UTBMAT_ENABLED {
            let key = mesh.name.clone();
            let unseen = UTBMAT_SEEN
                .lock()
                .map(|mut seen| seen.insert(key))
                .unwrap_or(false);
            if unseen {
                let tex0 = pass
                    .get_texture(0)
                    .map(|texture| {
                        format!(
                            "'{}' {}x{} fmt={:?} px={}",
                            texture.get_name(),
                            texture.width,
                            texture.height,
                            texture.format,
                            texture.raw_pixels().len()
                        )
                    })
                    .unwrap_or_else(|| "none".to_string());
                log::warn!(
                    "UTBMAT model='{}' mat_diff=({:.3},{:.3},{:.3},{:.3}) mat_spec=({:.3},{:.3},{:.3},{:.3}) mat_emis=({:.3},{:.3},{:.3}) overrides=({:.3},{:.3},{:.3},{:.3}) stage_mask={:02x} cube_mask={:08x} hints={:08x} tex0={}",
                    mesh.name,
                    material_diffuse[0], material_diffuse[1], material_diffuse[2], material_diffuse[3],
                    material_specular[0], material_specular[1], material_specular[2], material_specular[3],
                    material_emissive[0], material_emissive[1], material_emissive[2],
                    material_overrides[0], material_overrides[1], material_overrides[2], material_overrides[3],
                    stage_masks.mask, stage_masks.cube_mask, stage_masks.hints,
                    tex0,
                );
            }
        }
        // UTBMESHVIS (documented diagnostic, GENERALS_UTBMESHVIS=1, once per
        // model): dump per-model visibility (FOW alpha/falloff/explored,
        // alpha_override) and the mesh-lane camera eye, to attribute invisible
        // units to an alpha discard vs a stale camera.
        if std::env::var("GENERALS_UTBMESHVIS").as_deref() == Ok("1") {
            static UTBMESHVIS_SEEN: std::sync::LazyLock<
                std::sync::Mutex<std::collections::HashSet<String>>,
            > = std::sync::LazyLock::new(|| {
                std::sync::Mutex::new(std::collections::HashSet::new())
            });
            let utb_vis_key = format!(
                "{}|{}|{}",
                mesh.name,
                mesh.alpha_override,
                (visibility_alpha, visibility_falloff, is_explored).0
            );
            let unseen = UTBMESHVIS_SEEN
                .lock()
                .map(|mut seen| seen.insert(utb_vis_key))
                .unwrap_or(false);
            if unseen {
                let utb_cam = render_info.camera.get_position();
                log::warn!(
                    "UTBMESHVIS model='{}' alpha_override={:.3} pres_opacity={:.3} fow=(a={:.3} f={:.3} exp={:.3}) cam=({:.1},{:.1},{:.1}) world=({:.1},{:.1},{:.1})",
                    mesh.name,
                    mesh.alpha_override,
                    mesh.presentation_opacity(),
                    visibility_alpha,
                    visibility_falloff,
                    is_explored,
                    utb_cam.x,
                    utb_cam.y,
                    utb_cam.z,
                    mesh.transform.w_axis.x,
                    mesh.transform.w_axis.y,
                    mesh.transform.w_axis.z,
                );
            }
        }
        // UTBPASSTINT (documented diagnostic, GENERALS_UTBPASSTINT=1):
        // per-pass material_diffuse override — pass0 red, pass1 green, pass2
        // blue, pass3+ yellow — to identify which material pass paints the
        // white shards and whether the shader honors material_diffuse.
        // UTBRANGE (same env): once per model+pass, dump the authored draw
        // range so an invisible base pass (start,0) is visible in logs.
        if std::env::var("GENERALS_UTBPASSTINT").as_deref() == Ok("1") {
            material_diffuse = match pass.get_pass_index() {
                0 => [1.0, 0.0, 0.0, 1.0],
                1 => [0.0, 1.0, 0.0, 1.0],
                2 => [0.0, 0.0, 1.0, 1.0],
                _ => [1.0, 1.0, 0.0, 1.0],
            };
            static UTBRANGE_SEEN: std::sync::LazyLock<
                std::sync::Mutex<std::collections::HashSet<String>>,
            > = std::sync::LazyLock::new(|| {
                std::sync::Mutex::new(std::collections::HashSet::new())
            });
            let utb_pi = pass.get_pass_index();
            let utb_key = format!("{}|{}", mesh.name, utb_pi);
            let unseen = UTBRANGE_SEEN
                .lock()
                .map(|mut seen| seen.insert(utb_key))
                .unwrap_or(false);
            if unseen {
                log::warn!(
                    "UTBRANGE model='{}' pass={} range={:?} all={:?} idx_buf={}",
                    mesh.name,
                    utb_pi,
                    prepared.pass_index_ranges.get(utb_pi).copied(),
                    prepared.pass_index_ranges,
                    prepared.index_buffer.is_some(),
                );
            }
        }
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
            Some(visibility_alpha),
            Some(visibility_falloff),
            Some(is_explored),
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

        let utb_t = mesh.transform.w_axis;
        let utb_translation = [utb_t.x, utb_t.y, utb_t.z];
        self.issue_draw_call(
            prepared,
            pass,
            render_pass,
            &mesh.name,
            utb_translation,
        );

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
        mesh_name: &str,
        mesh_translation: [f32; 3],
    ) {
        // Get the pass index for filtering
        let pass_index = pass.get_pass_index();
        // C++ DX8 mesh rendering semantics (MeshClass::Render_Material_Pass /
        // DX8PolygonRendererList): a material pass only re-draws polygon
        // renderers authored for that pass index. The base pass (0) owns the
        // geometry ranges resolved in `compute_pass_index_ranges`; a pass with
        // no authored range (pass_index >= ranges.len() or a (start, 0) entry)
        // owns no geometry and must not fall back to re-drawing the whole
        // mesh with the wrong pass state.
        let (start_index, count) = if pass_index < prepared.pass_index_ranges.len() {
            prepared.pass_index_ranges[pass_index]
        } else {
            (0, 0)
        };
        // UTBDRAW (documented diagnostic, GENERALS_UTBDRAWLOG=1 or
        // GENERALS_UTBDRAWSKIP="start:count,..."): log every distinct
        // material-pass draw; optionally skip exact (start,count) matches to
        // identify which draw produces the unexplained white shards.
        static UTBDRAW_SEEN: std::sync::LazyLock<
            std::sync::Mutex<std::collections::HashSet<String>>,
        > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));
        let utb_skip_spec = std::env::var("GENERALS_UTBDRAWSKIP").ok();
        let utb_skip = utb_skip_spec.as_deref().map(|spec| {
            spec.split(',')
                .filter_map(|entry| {
                    let mut parts = entry.trim().split(':');
                    let start = parts.next()?.trim().parse::<u32>().ok()?;
                    let count = parts.next()?.trim().parse::<u32>().ok()?;
                    Some((start, count))
                })
                .any(|(start, count)| start == start_index && count == count)
        })
        .unwrap_or(false);
        let utb_key = format!(
            "model='{}' pass={} start={} count={} verts={} indexed={} tex0='{}' world=({:.1},{:.1},{:.1})",
            mesh_name,
            pass_index,
            start_index,
            count,
            prepared.vertex_count,
            prepared.index_buffer.is_some(),
            pass.get_texture(0)
                .map(|texture| texture.get_name())
                .unwrap_or_default(),
            mesh_translation[0],
            mesh_translation[1],
            mesh_translation[2],
        );
        let unseen = UTBDRAW_SEEN
            .lock()
            .map(|mut seen| seen.insert(utb_key.clone()))
            .unwrap_or(false);
        let utb_log = std::env::var("GENERALS_UTBDRAWLOG").as_deref() == Ok("1");
        if utb_log && unseen {
            log::warn!("UTBDRAW {}", utb_key);
        }
        if utb_skip {
            if unseen {
                let tex = pass.get_texture(0);
                let px = tex.as_ref().map(|t| t.raw_pixels()).unwrap_or_default();
                let fmt = tex
                    .as_ref()
                    .map(|t| format!("{:?}", t.format))
                    .unwrap_or_default();
                let (tw, th) = tex.as_ref().map(|t| (t.width, t.height)).unwrap_or((0, 0));
                let vdc = pass.diffuse_vertex_colors.as_ref();
                log::warn!(
                    "UTBDRAWSKIP model='{}' tex=({}x{} fmt={} pxlen={} px0={:02x?}) diffvc={} v0={:?} shaderbits={:08x} world=({:.1},{:.1},{:.1})",
                    mesh_name,
                    tw,
                    th,
                    fmt,
                    px.len(),
                    &px[..px.len().min(16)],
                    vdc.map(|colors| colors.len()).unwrap_or(0),
                    vdc.and_then(|colors| colors.first()).copied(),
                    pass.shader.get_bits(),
                    mesh_translation[0],
                    mesh_translation[1],
                    mesh_translation[2],
                );
            }
            return;
        }
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
        let texture_opt = pass.get_texture(stage);
        let utbview_enabled = *UTBVIEW_ENABLED;
        let (resources, branch) = if let Some(texture) = texture_opt {
            if let Some(view) = texture.get_texture_view() {
                let sampler_desc = sampler_descriptor_for_settings(&texture.stage_settings);
                let sampler = Arc::new(self.gpu_device.wgpu_device().create_sampler(&sampler_desc));
                (
                    StageResources {
                        view_2d: Arc::new(view),
                        view_cube: self.fallback_textures.view_cube.clone(),
                        sampler,
                    },
                    "preassigned-view",
                )
            } else if let Some(view) = self.ensure_gpu_texture_view(texture) {
                let sampler_desc = sampler_descriptor_for_settings(&texture.stage_settings);
                let sampler = Arc::new(self.gpu_device.wgpu_device().create_sampler(&sampler_desc));
                (
                    StageResources {
                        view_2d: view,
                        view_cube: self.fallback_textures.view_cube.clone(),
                        sampler,
                    },
                    "uploaded-cpu",
                )
            } else {
                // W3D placeholder pass texture (name only, no pixels/view):
                // resolve it through the host-installed archive-backed provider
                // (C++ WW3DAssetManager::Get_Texture parity) and upload the
                // hydrated pixels through the same first-bind path.
                match self
                    .pass_texture_provider
                    .as_ref()
                    .and_then(|provider| provider(texture.get_name()))
                {
                    Some(hydrated) => {
                        if let Some(view) = self.ensure_gpu_texture_view(&hydrated) {
                            let sampler_desc =
                                sampler_descriptor_for_settings(&hydrated.stage_settings);
                            let sampler = Arc::new(
                                self.gpu_device.wgpu_device().create_sampler(&sampler_desc),
                            );
                            (
                                StageResources {
                                    view_2d: view,
                                    view_cube: self.fallback_textures.view_cube.clone(),
                                    sampler,
                                },
                                "provider-hydrated",
                            )
                        } else {
                            (self.fallback_stage_resources(), "provider-upload-failed")
                        }
                    }
                    None => (self.fallback_stage_resources(), "provider-miss"),
                }
            }
        } else {
            (self.fallback_stage_resources(), "no-texture")
        };

        if utbview_enabled {
            let name = texture_opt
                .map(|texture| texture.get_name().to_string())
                .unwrap_or_default();
            let texel_mid = texture_opt.and_then(|texture| {
                let pixels = texture.raw_pixels();
                let (w, h) = (texture.width as usize, texture.height as usize);
                if w > 0 && h > 0 && pixels.len() >= w * h * 4 {
                    let mid = ((h / 2) * w + w / 2) * 4;
                    Some([pixels[mid], pixels[mid + 1], pixels[mid + 2], pixels[mid + 3]])
                } else {
                    None
                }
            });
            let key = (name.clone(), branch);
            let unseen = UTBVIEW_SEEN
                .lock()
                .map(|mut seen| seen.insert(key))
                .unwrap_or(false);
            if unseen {
                log::warn!(
                    "UTBVIEW stage={} name='{}' branch={} texel_mid={:?}",
                    stage,
                    name,
                    branch,
                    texel_mid,
                );
            }
        }
        resources
    }

    fn fallback_stage_resources(&self) -> StageResources {
        StageResources {
            view_2d: self.fallback_textures.view_2d.clone(),
            view_cube: self.fallback_textures.view_cube.clone(),
            sampler: self.default_sampler.clone(),
        }
    }

    /// Upload a CPU-only pass texture on first bind and cache its view,
    /// keyed by texture name. Only 32-bit uncompressed formats are uploaded;
    /// anything else keeps the manager's fallback texture.
    fn ensure_gpu_texture_view(&self, texture: &TextureClass) -> Option<Arc<wgpu::TextureView>> {
        let key = texture.get_name().to_ascii_lowercase();
        if let Ok(cache) = self.gpu_texture_views.lock() {
            if let Some(view) = cache.get(&key) {
                return Some(Arc::clone(view));
            }
        }

        let pixels = texture.raw_pixels();
        let (width, height) = (texture.width, texture.height);
        if pixels.is_empty() || width == 0 || height == 0 {
            return None;
        }
        let wgpu_format = match texture.format {
            crate::texture_system::TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            crate::texture_system::TextureFormat::Rgba8UnormSrgb => {
                wgpu::TextureFormat::Rgba8UnormSrgb
            }
            crate::texture_system::TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
            crate::texture_system::TextureFormat::Bgra8UnormSrgb => {
                wgpu::TextureFormat::Bgra8UnormSrgb
            }
            _ => return None,
        };
        let expected = width as usize * height as usize * 4;
        if pixels.len() < expected {
            return None;
        }

        let device = self.gpu_device.wgpu_device();
        let queue = self.gpu_device.queue();
        let gpu_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("MeshManager Pass Texture {}", texture.get_name())),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &gpu_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels[..expected],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = Arc::new(gpu_texture.create_view(&wgpu::TextureViewDescriptor::default()));

        if let Ok(mut cache) = self.gpu_texture_views.lock() {
            if let Some(existing) = cache.get(&key) {
                // Another thread won the upload race; reuse its view.
                return Some(Arc::clone(existing));
            }
            cache.insert(key, Arc::clone(&view));
        }
        Some(view)
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

#[cfg(test)]
mod per_mesh_lighting_tests {
    use super::*;
    use crate::rendering::camera_system::CameraClass;
    use crate::rendering::lighting_system::LightEnvironmentClass;

    #[test]
    fn mesh_owned_lighting_overrides_only_the_selected_render_info() {
        let camera = Arc::new(CameraClass::new());
        let mut scene_info = RenderInfoClass::new(camera);
        scene_info.frame_count = 37;
        scene_info.set_lighting_environment(LightEnvironmentClass::new());

        let mut mesh = MeshClass::new();
        assert!(
            MeshRenderManager::render_info_for_mesh(&mesh, &scene_info)
                .lighting
                .is_some()
        );

        let mesh_environment = Arc::new(LightEnvironmentClass::new());
        mesh.set_lighting_environment(Some(Arc::clone(&mesh_environment)));
        let selected = MeshRenderManager::render_info_for_mesh(&mesh, &scene_info);

        assert_eq!(selected.frame_count, 37);
        assert!(selected.lighting.is_some());
        assert!(
            mesh.get_lighting_environment()
                .is_some_and(|environment| Arc::ptr_eq(environment, &mesh_environment)),
            "the MeshClass keeps ownership of the exact environment selected for this draw"
        );
    }

    #[test]
    fn live_cascade_uses_first_enabled_directional_light() {
        let camera = Arc::new(CameraClass::new());
        let mut info = RenderInfoClass::new(camera);
        let mut environment = LightEnvironmentClass::new();
        environment.add_light(Arc::new(std::sync::Mutex::new(
            crate::rendering::lighting_system::LightClass::directional(
                glam::Vec3::new(0.2, -1.0, 0.1),
                glam::Vec3::ONE,
                1.0,
            ),
        )));
        info.set_lighting_environment(environment);
        let dir = live_cascade_light_direction(&info);
        assert!((dir.y + 1.0 / (0.2f32 * 0.2 + 1.0 + 0.1 * 0.1).sqrt()).abs() < 0.02);
    }

    #[test]
    fn live_cascade_falls_back_when_no_directional_light() {
        let camera = Arc::new(CameraClass::new());
        let info = RenderInfoClass::new(camera);
        let dir = live_cascade_light_direction(&info);
        assert_eq!(dir, glam::Vec3::new(0.35, -0.85, 0.35));
    }

    #[test]
    fn render_with_targets_calls_live_cascade_update() {
        // C++ W3DDisplay.cpp:1840 updateRenderTargetTextures before the scene.
        let renderer_src = include_str!("../../lib.rs");
        assert!(
            renderer_src.contains("update_and_fill_live_cascade"),
            "Renderer::render_with_targets must fill LiveCascadeShadowMap"
        );
        let src = include_str!("render_manager.rs");
        assert!(
            src.contains("self.live_csm.update("),
            "MeshRenderManager must call LiveCascadeShadowMap::update"
        );
        assert!(
            src.contains("live_csm_first_light_depth"),
            "first-light depth pass must clear cascade layers"
        );
        assert!(
            src.contains("draw_indexed(0..prepared.index_count"),
            "first-light depth pass must draw casters, not only clear"
        );
        assert!(
            renderer_src.contains("cascade_casters"),
            "Renderer must pass live casters into cascade fill"
        );
    }

    #[test]
    fn live_cascade_fill_draws_opaque_casters() {
        // C++ W3DDisplay.cpp:1840 updateRenderTargetTextures writes occluder
        // depth before the scene. An empty clear+enable is not a fill.
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
            .ok()
        else {
            return;
        };
        // STANDALONE DEVICE: #[cfg(test)] cascade fill, not on the game path.
        let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                label: Some("live-csm-fill-test"),
                ..Default::default()
            }))
        else {
            return;
        };
        let gpu = Arc::new(GpuDevice::from_shared(Arc::new(device), Arc::new(queue)));
        let mut manager = MeshRenderManager::new(gpu.clone());
        let camera = Arc::new(CameraClass::new());
        let info = RenderInfoClass::new(camera);

        let mut model = MeshModelClass::new("csm_caster");
        model.vertices = vec![
            W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 4.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 4.0,
            },
        ];
        model.triangles = vec![W3dTriangleStruct {
            vindex: [0, 1, 2],
            attributes: 0,
            normal: W3dVectorStruct {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            distance: 0.0,
        }];
        model.vertex_count = 3;
        model.index_count = 3;
        let mut mesh = MeshClass::new();
        mesh.model = Some(Arc::new(model));
        mesh.transform = Mat4::IDENTITY;

        let mut encoder =
            gpu.wgpu_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("live-csm-fill-encoder"),
                });
        manager.update_and_fill_live_cascade(&mut encoder, &info, &[Arc::new(mesh)]);
        gpu.queue().submit(Some(encoder.finish()));
        let _ = gpu.wgpu_device().poll(wgpu::PollType::wait_indefinitely());

        assert!(
            manager.live_cascade_enabled(),
            "filled cascade must enable PCF sampling"
        );
        assert_eq!(
            manager.last_cascade_casters_drawn(),
            1,
            "opaque caster must write depth, not just clear the map"
        );
    }

    #[test]
    fn live_cascade_fill_skips_hidden_and_decal_meshes() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
            .ok()
        else {
            return;
        };
        // STANDALONE DEVICE: #[cfg(test)] cascade skip, not on the game path.
        let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                label: Some("live-csm-skip-test"),
                ..Default::default()
            }))
        else {
            return;
        };
        let gpu = Arc::new(GpuDevice::from_shared(Arc::new(device), Arc::new(queue)));
        let mut manager = MeshRenderManager::new(gpu.clone());
        let camera = Arc::new(CameraClass::new());
        let info = RenderInfoClass::new(camera);

        let mut hidden = MeshClass::new();
        hidden.is_hidden = true;
        let mut model = MeshModelClass::new("hidden");
        model.vertex_count = 3;
        model.index_count = 3;
        hidden.model = Some(Arc::new(model));

        let mut encoder =
            gpu.wgpu_device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("live-csm-skip-encoder"),
                });
        manager.update_and_fill_live_cascade(&mut encoder, &info, &[Arc::new(hidden)]);
        gpu.queue().submit(Some(encoder.finish()));

        assert_eq!(manager.last_cascade_casters_drawn(), 0);
    }
}

pub(crate) fn live_cascade_light_direction(render_info: &RenderInfoClass) -> glam::Vec3 {
    if let Some(environment) = render_info.lighting.as_ref() {
        for light in &environment.lights {
            if let Ok(light) = light.lock() {
                if light.enabled
                    && light.light_type == crate::rendering::lighting_system::LightType::Directional
                    && light.direction.length_squared() > 1e-6
                {
                    return light.direction;
                }
            }
        }
    }
    glam::Vec3::new(0.35, -0.85, 0.35)
}
