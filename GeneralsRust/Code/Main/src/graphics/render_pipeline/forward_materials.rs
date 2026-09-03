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
use ww3d_renderer_3d::rendering::texture_system::dds_loader::{
    DdsCompression, decode_dxt1, decode_dxt3, decode_dxt5,
};

impl ForwardPass {
    pub(super) fn build_mesh_model(
        &mut self,
        cache_key: &str,
        mesh: &crate::assets::models::W3DMesh,
        material: &W3DMaterial,
    ) -> Result<MeshModelClass> {
        let mut model = MeshModelClass::new(cache_key);
        let axis = if mesh.vertices_in_render_space {
            Mat4::IDENTITY
        } else {
            gameplay_to_render_axis_matrix()
        };

        model.vertices = mesh
            .vertices
            .iter()
            .map(|v| {
                let pos = axis.transform_point3(Vec3::from_array(v.position));
                W3dVectorStruct {
                    x: pos.x,
                    y: pos.y,
                    z: pos.z,
                }
            })
            .collect();
        model.normals = mesh
            .vertices
            .iter()
            .map(|v| {
                let normal = axis
                    .transform_vector3(Vec3::from_array(v.normal))
                    .normalize_or_zero();
                W3dVectorStruct {
                    x: normal.x,
                    y: normal.y,
                    z: normal.z,
                }
            })
            .collect();

        if mesh.has_explicit_vertex_colors && !mesh.vertices.is_empty() {
            let mut color_sum = Vec4::ZERO;
            for vertex in &mesh.vertices {
                color_sum += Vec4::new(
                    vertex.color[0],
                    vertex.color[1],
                    vertex.color[2],
                    vertex.color[3],
                );
            }
            let inv = 1.0 / mesh.vertices.len() as f32;
            let avg = color_sum * inv;
            if avg.x.max(avg.y).max(avg.z) <= 0.05 {
                static LOW_VERTEX_COLOR_WARNINGS: AtomicUsize = AtomicUsize::new(0);
                let count = LOW_VERTEX_COLOR_WARNINGS.fetch_add(1, Ordering::Relaxed);
                if count < 20 {
                    warn!(
                        "Mesh '{}' has explicit vertex colors but near-black average ({:.3},{:.3},{:.3},{:.3}); model '{}'",
                        mesh.name, avg.x, avg.y, avg.z, avg.w, cache_key
                    );
                }
            } else {
                static EXPLICIT_VERTEX_COLOR_DEBUGS: AtomicUsize = AtomicUsize::new(0);
                let count = EXPLICIT_VERTEX_COLOR_DEBUGS.fetch_add(1, Ordering::Relaxed);
                if count < 8 {
                    debug!(
                        "Mesh '{}' explicit vertex-color average ({:.3},{:.3},{:.3},{:.3})",
                        mesh.name, avg.x, avg.y, avg.z, avg.w
                    );
                }
            }
        }

        model.stage_texture_coords = mesh
            .stage_texcoords
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .map(|uv| W3dTexCoordStruct { u: uv[0], v: uv[1] })
                    .collect()
            })
            .collect();

        model.stage_uv_sources = mesh.stage_uv_channels.clone();

        model.texture_coords = if let Some(stage0) = model.stage_texture_coords.first() {
            stage0.clone()
        } else {
            mesh.vertices
                .iter()
                .map(|v| W3dTexCoordStruct {
                    u: v.uv[0],
                    v: v.uv[1],
                })
                .collect()
        };
        if model.stage_texture_coords.is_empty() && !model.texture_coords.is_empty() {
            model
                .stage_texture_coords
                .push(model.texture_coords.clone());
        }
        model.triangles = mesh
            .indices
            .chunks(3)
            .filter_map(|chunk| {
                if chunk.len() != 3 {
                    return None;
                }
                let i0 = chunk[0] as usize;
                let i1 = chunk[1] as usize;
                let i2 = chunk[2] as usize;
                if i0 >= mesh.vertices.len()
                    || i1 >= mesh.vertices.len()
                    || i2 >= mesh.vertices.len()
                {
                    return None;
                }

                let p0 = axis.transform_point3(Vec3::from_array(mesh.vertices[i0].position));
                let p1 = axis.transform_point3(Vec3::from_array(mesh.vertices[i1].position));
                let p2 = axis.transform_point3(Vec3::from_array(mesh.vertices[i2].position));

                let normal = (p1 - p0).cross(p2 - p0);
                let (normal_vec, distance) = if normal.length_squared() > f32::EPSILON {
                    let n = normal.normalize();
                    (n, n.dot(p0))
                } else {
                    (Vec3::Y, 0.0)
                };

                Some(W3dTriangleStruct {
                    vindex: [chunk[0], chunk[1], chunk[2]],
                    attributes: 0,
                    normal: W3dVectorStruct {
                        x: normal_vec.x,
                        y: normal_vec.y,
                        z: normal_vec.z,
                    },
                    distance,
                })
            })
            .collect();

        model.vertex_count = model.vertices.len() as u32;
        model.index_count = (model.triangles.len() * 3) as u32;
        let (pass_count, vertex_material_count, shader_count, texture_count) =
            if !mesh.passes.is_empty() {
                let texture_total = mesh
                    .per_pass_stage_texture_names
                    .iter()
                    .flat_map(|stages| stages.iter())
                    .map(|names| names.len() as u32)
                    .sum::<u32>();
                (
                    mesh.passes.len() as u32,
                    mesh.vertex_materials.len() as u32,
                    mesh.shaders.len() as u32,
                    texture_total,
                )
            } else {
                (
                    1,
                    1,
                    1,
                    if material.texture_name.is_some() {
                        1
                    } else {
                        0
                    },
                )
            };

        model.material_info = Some(W3dMaterialInfoStruct {
            pass_count,
            vert_matl_count: vertex_material_count.max(1),
            shader_count: shader_count.max(1),
            texture_count,
        });

        if !mesh.vertex_materials.is_empty() {
            model.vertex_materials = mesh.vertex_materials.clone();
        } else {
            model.vertex_materials = vec![Self::build_w3d_vertex_material(material)];
        }

        if !mesh.shaders.is_empty() {
            model.shaders = mesh.shaders.clone();
        }

        if let Some(influences) = &mesh.vertex_influences {
            model.set_vertex_influences(influences.clone());
        }
        model.per_stage_face_texcoord_ids = mesh.per_stage_face_texcoord_ids.clone();

        if !mesh.passes.is_empty() {
            let vertex_material_cache = self.build_vertex_material_cache(mesh, material);
            let mut passes = Vec::with_capacity(mesh.passes.len());
            for pass_index in 0..mesh.passes.len() {
                if let Some(pass) =
                    self.build_material_pass_from_mesh(mesh, pass_index, &vertex_material_cache)?
                {
                    passes.push(pass);
                }
            }
            if passes.is_empty() {
                passes.push(self.build_material_pass(material)?);
            }
            model.material_passes = passes;
        } else {
            model.material_passes = vec![self.build_material_pass(material)?];
        }

        Ok(model)
    }

    pub(super) fn build_material_pass(
        &mut self,
        material: &W3DMaterial,
    ) -> Result<MaterialPassClass> {
        let mut pass = MaterialPassClass::new();
        let vertex_material = Arc::new(Self::build_vertex_material(material));
        pass.vertex_material = Some(Arc::clone(&vertex_material));
        pass.set_shader(Self::shader_for_material(material));

        if let Some(texture_name) = material_stage_texture(material, 0) {
            if let Some(texture) = self.ensure_texture(texture_name)? {
                pass.set_texture(0, texture);
            }
        }

        for stage in 1..4 {
            if let Some(texture_name) = material_stage_texture(material, stage) {
                if let Some(texture) = self.ensure_texture(texture_name)? {
                    pass.set_texture(stage, texture);
                }
            }
        }

        Ok(pass)
    }

    pub(super) fn build_vertex_material_cache(
        &self,
        mesh: &crate::assets::models::W3DMesh,
        fallback: &W3DMaterial,
    ) -> Vec<Arc<VertexMaterialClass>> {
        if mesh.vertex_materials.is_empty() {
            return vec![Arc::new(Self::build_vertex_material(fallback))];
        }

        mesh.vertex_materials
            .iter()
            .enumerate()
            .map(|(index, material)| {
                let name = format!("{}_VM{}", mesh.name, index);
                Arc::new(VertexMaterialClass::from_w3d_material(&name, material))
            })
            .collect()
    }

    pub(super) fn build_material_pass_from_mesh(
        &mut self,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
        vertex_materials: &[Arc<VertexMaterialClass>],
    ) -> Result<Option<MaterialPassClass>> {
        if pass_index >= mesh.passes.len() {
            return Ok(None);
        }

        let mut pass = MaterialPassClass::new();
        Self::assign_vertex_material_for_pass(&mut pass, mesh, pass_index, vertex_materials);
        if let Some(shader_id_list) = mesh.per_pass_shader_ids.get(pass_index) {
            if let Some(&shader_id) = shader_id_list.first() {
                if let Some(shader_struct) = mesh.shaders.get(shader_id as usize) {
                    pass.shader = ShaderClass::from_w3d_shader(shader_struct);
                }
            }
        } else if let Some(shader_struct) = mesh.shaders.first() {
            pass.shader = ShaderClass::from_w3d_shader(shader_struct);
        } else {
            pass.set_shader(Self::shader_for_material(&mesh.material));
        }

        if pass.shader.get_color_mask()
            == ww3d_renderer_3d::rendering::shader_system::shader::ColorMaskType::Disable
        {
            static DISABLED_COLOR_MASK_WARNINGS: AtomicUsize = AtomicUsize::new(0);
            let count = DISABLED_COLOR_MASK_WARNINGS.fetch_add(1, Ordering::Relaxed);
            if count < 40 {
                warn!(
                    "Shader color mask disabled for mesh='{}' pass={} (shader_bits=0x{:08X})",
                    mesh.name,
                    pass_index,
                    pass.shader.get_bits()
                );
            }
        }

        let has_bound_texture = self.assign_stage_textures_for_pass(&mut pass, mesh, pass_index)?;
        if !has_bound_texture {
            // C++ Get_Texture miss binds MissingTexture (magenta), never disables
            // texturing and never falls back to unmodulated white output.
            // W3DAssetManager.cpp:127-225; dx8wrapper.cpp:2875-2889. This includes
            // the 13 in-match passes whose stages resolve empty (`stages=[]`):
            // they previously rendered unmodulated white; they now carry the
            // shared `w3d_missing_texture.tga` identity so missing texture data
            // is visible exactly as retail renders an unresolvable texture.
            pass.set_texture(0, self.ensure_fallback_texture()?);
        }
        Self::assign_vertex_colors_for_pass(&mut pass, mesh, pass_index);
        Self::assign_mapper_for_pass(&mut pass, mesh, pass_index);
        pass.pass_index = pass_index;
        Ok(Some(pass))
    }

    pub(super) fn assign_vertex_material_for_pass(
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
        cache: &[Arc<VertexMaterialClass>],
    ) {
        if let Some(vm_ids) = mesh.per_pass_vertex_material_ids.get(pass_index) {
            if let Some(&vm_id) = vm_ids.first() {
                if let Some(vm) = cache.get(vm_id as usize) {
                    pass.vertex_material = Some(Arc::clone(vm));
                    return;
                }
            }
        }

        if let Some(vm) = cache.first() {
            pass.vertex_material = Some(Arc::clone(vm));
        }
    }

    pub(super) fn assign_stage_textures_for_pass(
        &mut self,
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
    ) -> Result<bool> {
        let mut assigned = false;
        if let Some(stage_sets) = mesh.per_pass_stage_texture_names.get(pass_index) {
            for (stage, names) in stage_sets.iter().enumerate() {
                let channel = Self::stage_uv_channel_for(mesh, pass_index, stage);
                pass.set_stage_uv_channel(stage, channel);

                if let Some(texture_name) =
                    names.iter().find(|name| Self::is_valid_texture_name(name))
                {
                    if let Some(texture) = self.ensure_texture(texture_name.as_str())? {
                        pass.set_texture(stage, texture);
                        assigned = true;
                        continue;
                    }
                }

                for fallback in mesh.stage_texture_names_from_ids(pass_index, stage) {
                    if !Self::is_valid_texture_name(&fallback) {
                        continue;
                    }
                    if let Some(texture) = self.ensure_texture(&fallback)? {
                        pass.set_texture(stage, texture);
                        assigned = true;
                        break;
                    }
                }
            }
        }

        if !assigned {
            let channel = Self::stage_uv_channel_for(mesh, pass_index, 0);
            pass.set_stage_uv_channel(0, channel);
            self.apply_base_texture(pass, &mesh.material)?;
            assigned = pass.get_texture(0).is_some();
        }
        Ok(assigned)
    }

    pub(super) fn stage_uv_channel_for(
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
        stage_index: usize,
    ) -> u8 {
        let preceding_stages = Self::stage_layer_offset(mesh, pass_index);
        let idx = preceding_stages + stage_index;
        mesh.stage_uv_channels
            .get(idx)
            .copied()
            .unwrap_or(stage_index as u8)
    }

    pub(super) fn stage_layer_offset(
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
    ) -> usize {
        if !mesh.per_pass_stage_texture_ids.is_empty() {
            mesh.per_pass_stage_texture_ids
                .iter()
                .take(pass_index)
                .map(|stages| stages.len())
                .sum()
        } else if !mesh.per_pass_stage_texture_names.is_empty() {
            mesh.per_pass_stage_texture_names
                .iter()
                .take(pass_index)
                .map(|stages| stages.len())
                .sum()
        } else {
            mesh.passes
                .iter()
                .take(pass_index)
                .map(|info| info.texture_count as usize)
                .sum()
        }
    }

    pub(super) fn is_valid_texture_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        if name.eq_ignore_ascii_case("default") {
            return false;
        }
        name.parse::<usize>().is_err()
    }

    pub(super) fn assign_vertex_colors_for_pass(
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
    ) {
        if let Some(colors) = mesh.per_pass_dcg_colors.get(pass_index) {
            if !colors.is_empty() {
                pass.diffuse_vertex_colors = Some(Self::colors_to_vec4(colors));
            }
        }

        if let Some(colors) = mesh.per_pass_dig_colors.get(pass_index) {
            if !colors.is_empty() {
                pass.illumination_vertex_colors = Some(Self::colors_to_vec4(colors));
            }
        }

        if pass.diffuse_vertex_colors.is_none() && mesh.has_explicit_vertex_colors {
            pass.diffuse_vertex_colors = Some(
                mesh.vertices
                    .iter()
                    .map(|vertex| {
                        Vec4::new(
                            vertex.color[0],
                            vertex.color[1],
                            vertex.color[2],
                            vertex.color[3],
                        )
                    })
                    .collect(),
            );
        }
    }

    pub(super) fn assign_mapper_for_pass(
        pass: &mut MaterialPassClass,
        mesh: &crate::assets::models::W3DMesh,
        pass_index: usize,
    ) {
        let vm_index = mesh
            .per_pass_vertex_material_ids
            .get(pass_index)
            .and_then(|ids| ids.first())
            .copied()
            .and_then(|id| usize::try_from(id).ok());

        if let Some(index) = vm_index {
            if let Some(mapper_info) = mesh.vertex_mappers.get(index) {
                if let Some(mapper) = mapper_info.stage0.or(mapper_info.stage1) {
                    pass.set_mapper_id(mapper.mapper_type);
                    for (idx, arg) in mapper.args.iter().enumerate() {
                        pass.set_mapper_arg(idx, *arg);
                    }
                    pass.set_mapper_float_args(mapper.float_args);
                }
            }
        }
    }

    pub(super) fn colors_to_vec4(colors: &[W3dRGBAStruct]) -> Vec<Vec4> {
        colors
            .iter()
            .map(|c| {
                Vec4::new(
                    c.r as f32 / 255.0,
                    c.g as f32 / 255.0,
                    c.b as f32 / 255.0,
                    c.a as f32 / 255.0,
                )
            })
            .collect()
    }

    pub(super) fn apply_base_texture(
        &mut self,
        pass: &mut MaterialPassClass,
        material: &W3DMaterial,
    ) -> Result<()> {
        if let Some(name) = material_stage_texture(material, 0) {
            if let Some(texture) = self.ensure_texture(name)? {
                pass.set_texture(0, texture);
            } else {
                pass.set_texture(0, self.ensure_fallback_texture()?);
            }
        }
        Ok(())
    }

    pub(super) fn ensure_texture(
        &mut self,
        texture_name: &str,
    ) -> Result<Option<Arc<TextureClass>>> {
        if texture_name.is_empty() {
            return Ok(None);
        }

        let cache_key = texture_name.to_lowercase();
        if let Some(texture) = self.texture_cache.get(&cache_key) {
            return Ok(Some(texture.clone()));
        }

        if self.is_known_missing_texture(texture_name) {
            let fallback = self.ensure_fallback_texture()?;
            self.texture_cache.insert(cache_key, fallback.clone());
            return Ok(Some(fallback));
        }

        if let Ok(texture) = self.create_texture_from_cached_assets(texture_name) {
            self.texture_cache.insert(cache_key, texture.clone());
            return Ok(Some(texture));
        }

        // First visible use: C++ Get_Texture loads synchronously (W3DAssetManager.cpp:127-225).
        let _ = self.prime_texture_raw_blocking(texture_name);
        if let Ok(texture) = self.create_texture_from_cached_assets(texture_name) {
            self.texture_cache.insert(cache_key, texture.clone());
            return Ok(Some(texture));
        }

        // True miss: MissingTexture surface (dx8wrapper.cpp:2875-2889), keep texturing on.
        let fallback = self.ensure_fallback_texture()?;
        self.texture_cache.insert(cache_key, fallback.clone());
        Ok(Some(fallback))
    }

    pub(super) fn create_texture_from_cached_assets(
        &self,
        texture_name: &str,
    ) -> Result<Arc<TextureClass>> {
        let asset_manager =
            get_asset_manager().ok_or_else(|| anyhow::anyhow!("Asset manager unavailable"))?;
        let asset_manager = asset_manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Asset manager mutex poisoned"))?;
        let texture_key = texture_name.to_lowercase();

        let raw = asset_manager
            .get_raw_texture(&texture_key)
            .ok_or_else(|| anyhow::anyhow!("Texture '{}' not cached", texture_name))?;
        self.build_texture(texture_name, raw)
    }

    pub(super) fn is_known_missing_texture(&self, texture_name: &str) -> bool {
        let Some(asset_manager_arc) = get_asset_manager() else {
            return false;
        };
        let Ok(asset_manager) = asset_manager_arc.lock() else {
            return false;
        };
        asset_manager.is_known_missing_texture(texture_name)
    }

    pub(super) fn prime_texture_raw_blocking(&self, texture_name: &str) -> Result<()> {
        let asset_manager =
            get_asset_manager().ok_or_else(|| anyhow::anyhow!("Asset manager unavailable"))?;
        let mut asset_manager = asset_manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Asset manager mutex poisoned"))?;
        asset_manager.prime_texture_raw_blocking(texture_name);
        Ok(())
    }

    pub(super) fn queue_texture_stream(&mut self, texture_name: &str) {
        let key = texture_name.to_lowercase();
        if self.texture_cache.contains_key(&key) || self.queued_texture_stream.contains(&key) {
            return;
        }

        self.pending_texture_stream
            .push_back(texture_name.to_string());
        self.queued_texture_stream.insert(key.clone());
    }

    pub(super) fn texture_stream_budget(&self) -> usize {
        let pending = self.pending_texture_stream.len();
        if pending == 0 {
            0
        } else if pending > 256 {
            64
        } else if pending > 96 {
            32
        } else if pending > 24 {
            16
        } else {
            8
        }
    }

    pub(super) fn stream_pending_textures(&mut self, per_frame_budget: usize) {
        if per_frame_budget == 0 || self.pending_texture_stream.is_empty() {
            return;
        }

        for _ in 0..per_frame_budget {
            let Some(texture_name) = self.pending_texture_stream.pop_front() else {
                break;
            };
            let cache_key = texture_name.to_lowercase();
            self.queued_texture_stream.remove(&cache_key);

            if self.texture_cache.contains_key(&cache_key) {
                continue;
            }

            if self.is_known_missing_texture(&texture_name) {
                if let Ok(fallback) = self.ensure_fallback_texture() {
                    self.texture_cache.insert(cache_key, fallback);
                }
                continue;
            }

            if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                self.texture_cache.insert(cache_key, texture);
                continue;
            }

            let _ = self.prime_texture_raw_blocking(&texture_name);
            if let Ok(texture) = self.create_texture_from_cached_assets(&texture_name) {
                self.texture_cache.insert(cache_key, texture);
                continue;
            }

            self.queue_texture_stream(&texture_name);
        }
    }

    pub(super) fn create_fallback_texture(&self, texture_name: &str) -> Result<Arc<TextureClass>> {
        let raw = RawTexture::solid_color(texture_name.to_string(), 4, 4, [255, 0, 255, 255]);
        self.build_texture(&raw.name, &raw)
    }

    pub(super) fn ensure_fallback_texture(&mut self) -> Result<Arc<TextureClass>> {
        if let Some(texture) = &self.fallback_texture {
            return Ok(texture.clone());
        }
        let texture = self.create_fallback_texture("__missing_texture__")?;
        self.fallback_texture = Some(texture.clone());
        Ok(texture)
    }

    pub(super) fn build_texture(
        &self,
        texture_name: &str,
        raw: &RawTexture,
    ) -> Result<Arc<TextureClass>> {
        let format = if raw.has_alpha {
            TextureFormat::Rgba8Unorm
        } else {
            TextureFormat::Rgba8Unorm
        };
        // C++ W3DTextureLoad (W3DAssetManager) hands the rasterizer uncompressed
        // 32-bit surfaces. The mesh bind path (MeshRenderManager::
        // ensure_gpu_texture_view) can only upload 32-bit RGBA payloads — a DXT
        // payload fails its width*height*4 length guard and the pass falls back
        // to the white missing-texture, which painted every compressed-DDS unit
        // texture (AVChinook.dds / Housecolor2.dds from Textures.big) white.
        // Decode block-compressed payloads to RGBA8 here, matching the
        // asset-lane fallback in TextureManager::create_gpu_texture.
        let data = match raw.dds_compression {
            Some(DdsCompression::Dxt1) => Some(
                decode_dxt1(&raw.data, raw.width, raw.height)
                    .map_err(|e| anyhow::anyhow!("DXT1 decode for '{}': {e}", texture_name)),
            ),
            Some(DdsCompression::Dxt3) => Some(
                decode_dxt3(&raw.data, raw.width, raw.height)
                    .map_err(|e| anyhow::anyhow!("DXT3 decode for '{}': {e}", texture_name)),
            ),
            Some(DdsCompression::Dxt5) => Some(
                decode_dxt5(&raw.data, raw.width, raw.height)
                    .map_err(|e| anyhow::anyhow!("DXT5 decode for '{}': {e}", texture_name)),
            ),
            None => None,
        };
        let data = data.transpose()?.unwrap_or_else(|| raw.data.clone());

        let mut texture = TextureClass::with_format(texture_name, raw.width, raw.height, format);
        texture
            .replace_pixels(data)
            .map_err(|e| anyhow::anyhow!("Failed to upload pixels for '{}': {e}", texture_name))?;
        Ok(Arc::new(texture))
    }
    pub(super) fn build_vertex_material(material: &W3DMaterial) -> VertexMaterialClass {
        let mut vm = VertexMaterialClass::new(&material.name);
        vm.diffuse = glam::Vec3::new(
            material.diffuse_color.x,
            material.diffuse_color.y,
            material.diffuse_color.z,
        );
        vm.specular = glam::Vec3::new(
            material.specular_color.x,
            material.specular_color.y,
            material.specular_color.z,
        );
        vm.emissive = glam::Vec3::new(
            material.emissive_color.x,
            material.emissive_color.y,
            material.emissive_color.z,
        );
        vm.opacity = material.opacity;
        vm.shininess = material.shininess.max(1.0);
        vm.translucency = 1.0 - material.opacity;
        vm
    }

    pub(super) fn build_w3d_vertex_material(material: &W3DMaterial) -> W3dVertexMaterialStruct {
        W3dVertexMaterialStruct {
            attributes: 0,
            ambient: Self::vec_to_rgba(glam::Vec3::splat(0.2), 1.0),
            diffuse: Self::vec_to_rgba(material.diffuse_color, material.opacity),
            specular: Self::vec_to_rgba(material.specular_color, 1.0),
            emissive: Self::vec_to_rgba(material.emissive_color, 1.0),
            shininess: material.shininess,
            opacity: material.opacity,
            translucency: 1.0 - material.opacity,
        }
    }

    pub(super) fn vec_to_rgba(color: glam::Vec3, alpha: f32) -> W3dRGBAStruct {
        fn to_u8(value: f32) -> u8 {
            (value.clamp(0.0, 1.0) * 255.0).round() as u8
        }

        W3dRGBAStruct {
            r: to_u8(color.x),
            g: to_u8(color.y),
            b: to_u8(color.z),
            a: to_u8(alpha),
        }
    }

    pub(super) fn shader_for_material(material: &W3DMaterial) -> ShaderClass {
        match material.blend_mode {
            crate::assets::models::BlendMode::Opaque => ShaderClass::get_opaque_shader(),
            crate::assets::models::BlendMode::Alpha => ShaderClass::get_alpha_shader(),
            crate::assets::models::BlendMode::Additive => ShaderClass::get_additive_shader(),
            crate::assets::models::BlendMode::Modulate => ShaderClass::get_opaque_shader(),
        }
    }
}
