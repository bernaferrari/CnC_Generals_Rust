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

impl MeshModelClass {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            vertices: Vec::new(),
            normals: Vec::new(),
            triangles: Vec::new(),
            material_info: None,
            shaders: Vec::new(),
            vertex_materials: Vec::new(),
            vertex_bone_links: Vec::new(),
            vertex_influences: Vec::new(),
            texture_coords: Vec::new(),
            stage_texture_coords: Vec::new(),
            per_stage_face_texcoord_ids: Vec::new(),
            stage_uv_sources: Vec::new(),
            sort_level: SORT_LEVEL_NONE,
            flags: 0,
            polygon_renderer_list: Vec::new(),
            material_passes: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
            vertex_count: 0,
            index_count: 0,
            w3d_attributes: 0,
            user_text: None,
            revision: 0,
        }
    }

    /// Construct a mesh model from an asset prototype, mirroring the legacy loader.
    pub fn from_mesh_prototype(
        prototype: &MeshPrototype,
        _hierarchy: Option<&HierarchyPrototype>,
    ) -> W3dResult<Self> {
        let mut model = MeshModelClass::new(&prototype.name);

        model.vertices = prototype.vertices.clone();
        model.normals = prototype.normals.clone();
        model.triangles = prototype.triangles.clone();
        model.material_info = prototype.material_info.clone();
        model.shaders = prototype.shaders.clone();
        model.vertex_materials = prototype.vertex_materials.clone();
        let (uv_sets, stage_channels) = compute_stage_uv_info(&prototype.stage_texcoords);
        model.stage_texture_coords = uv_sets;
        model.stage_uv_sources = stage_channels;
        model.per_stage_face_texcoord_ids = prototype.per_face_texcoord_ids.clone();
        if let Some(stage0) = model.stage_texture_coords.first() {
            model.texture_coords = stage0.clone();
        }
        if let Some(header) = &prototype.header {
            model.sort_level = header.attrs;
            model.w3d_attributes = header.attrs;
        }
        model.ensure_stage_zero();

        if let Some(influences) = &prototype.vertex_influences {
            model.set_vertex_influences(influences.clone());
        }

        model.vertex_count = model.vertices.len() as u32;
        model.index_count = (model.triangles.len() * 3) as u32;

        model.material_passes = build_material_passes_from_prototype(prototype);

        Ok(model)
    }

    /// Create WGPU vertex and index buffers from mesh data
    /// Handles different vertex formats for skinned vs rigid meshes
    pub fn create_wgpu_buffers(&mut self, device: &wgpu::Device) {
        const MAX_UV_SETS: usize = 4;
        // Determine vertex format based on mesh type
        let is_skinned = self.is_skinned();
        let has_normals = self.has_normals();

        // Calculate vertex stride (floats) based on attributes
        let mut stride_floats = 3; // Position (x, y, z)
        if has_normals {
            stride_floats += 3; // Normal (x, y, z)
        }
        stride_floats += 2 * MAX_UV_SETS; // Always provide up to 4 UV sets
        if is_skinned {
            stride_floats += 4; // Bone indices packed as f32 bits
            stride_floats += 4; // Bone weights
        }

        // Create vertex data with proper format
        let mut vertex_data: Vec<f32> = Vec::with_capacity(self.vertices.len() * stride_floats);

        for i in 0..self.vertices.len() {
            // Position (always present)
            vertex_data.push(self.vertices[i].x);
            vertex_data.push(self.vertices[i].y);
            vertex_data.push(self.vertices[i].z);

            // Normal (if available)
            if has_normals {
                if i < self.normals.len() {
                    vertex_data.push(self.normals[i].x);
                    vertex_data.push(self.normals[i].y);
                    vertex_data.push(self.normals[i].z);
                } else {
                    vertex_data.push(0.0);
                    vertex_data.push(1.0);
                    vertex_data.push(0.0);
                }
            }

            // Texture coordinates (if available)
            for channel in 0..MAX_UV_SETS {
                let uv = self.uv_channel_coords(channel, i);
                vertex_data.push(uv[0]);
                vertex_data.push(uv[1]);
            }

            // Bone data for skinned meshes
            if is_skinned {
                let (indices, weights) = self.vertex_influence_view(i);
                for &idx in &indices {
                    vertex_data.push(f32::from_bits(idx));
                }
                vertex_data.extend_from_slice(&weights);
            }
        }

        // Create vertex buffer
        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Vertex Buffer", self.name)),
                contents: bytemuck::cast_slice(&vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );

        self.vertex_count = self.vertices.len() as u32;

        // Create index data from triangles
        let mut index_data: Vec<u32> = Vec::new();
        for triangle in &self.triangles {
            index_data.push(triangle.vindex[0]);
            index_data.push(triangle.vindex[1]);
            index_data.push(triangle.vindex[2]);
        }

        // Create index buffer
        self.index_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} Index Buffer", self.name)),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );

        self.index_count = index_data.len() as u32;
    }

    pub fn get_flag(&self, flag: MeshGeometryClass) -> bool {
        (self.flags & flag as u32) != 0
    }

    pub fn set_flag(&mut self, flag: MeshGeometryClass, value: bool) {
        if self.get_flag(flag) == value {
            return;
        }
        if value {
            self.flags |= flag as u32;
        } else {
            self.flags &= !(flag as u32);
        }
        self.mark_dirty();
    }

    pub fn get_sort_level(&self) -> u32 {
        self.sort_level
    }

    /// Get material pass by index
    pub fn get_material_pass(&self, index: usize) -> Option<&MaterialPassClass> {
        self.material_passes.get(index)
    }

    /// Check whether this mesh has a complete source-shaped skin table.
    ///
    /// The C++ loader only sets `SKIN` after it has read one `BoneIdx` record
    /// per source vertex. Do not let a flag left behind by a malformed caller
    /// choose a skinned WGPU layout without the corresponding exact data.
    pub fn is_skinned(&self) -> bool {
        self.get_flag(MeshGeometryClass::SKIN) && self.vertex_influences().is_some()
    }

    /// Check if mesh has normals
    pub fn has_normals(&self) -> bool {
        !self.normals.is_empty()
    }

    /// Check if mesh has texture coordinates
    pub fn has_tex_coords(&self) -> bool {
        !self.texture_coords.is_empty() || !self.stage_texture_coords.is_empty()
    }

    pub(super) fn uv_channel_coords(&self, channel: usize, vertex_index: usize) -> [f32; 2] {
        if let Some(layer) = self.stage_texture_coords.get(channel) {
            if let Some(tc) = layer.get(vertex_index) {
                return [tc.u, tc.v];
            }
        }
        [0.0, 0.0]
    }

    pub(super) fn ensure_stage_zero(&mut self) {
        if self.stage_texture_coords.is_empty() && !self.texture_coords.is_empty() {
            self.stage_texture_coords.push(self.texture_coords.clone());
        } else if self
            .stage_texture_coords
            .first()
            .is_none_or(|layer| layer.is_empty())
            && !self.texture_coords.is_empty()
        {
            if self.stage_texture_coords.is_empty() {
                self.stage_texture_coords.push(self.texture_coords.clone());
            } else {
                self.stage_texture_coords[0] = self.texture_coords.clone();
            }
        }
    }

    /// Get mesh name - equivalent to C++ Get_Name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set mesh name - equivalent to C++ Set_Name
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Replace the per-vertex bone links. Mirrors C++ `MeshGeometryClass::VertexBoneLink`.
    pub fn set_vertex_bone_links(&mut self, links: Vec<u16>) {
        if self.vertices.is_empty() || links.len() != self.vertices.len() {
            self.vertex_influences.clear();
            self.vertex_bone_links.clear();
            self.set_flag(MeshGeometryClass::SKIN, false);
            return;
        }

        // This explicit API already owns an exact link for every vertex. Keep
        // the public `W3dVertInfStruct` view aligned with it, but never create
        // links from a hierarchy or an absent source chunk.
        self.vertex_influences = links
            .iter()
            .map(|&bone_idx| W3dVertInfStruct {
                bone_idx,
                pad: [0; 6],
            })
            .collect();
        self.vertex_bone_links = links;

        self.set_flag(MeshGeometryClass::SKIN, true);
    }

    /// Install the exact one-record-per-vertex W3D influence table.
    pub fn set_vertex_influences(&mut self, influences: Vec<W3dVertInfStruct>) {
        if self.vertices.is_empty() || influences.len() != self.vertices.len() {
            self.vertex_influences.clear();
            self.vertex_bone_links.clear();
            self.set_flag(MeshGeometryClass::SKIN, false);
            return;
        }

        self.vertex_bone_links = influences
            .iter()
            .map(|influence| influence.bone_idx)
            .collect();
        self.vertex_influences = influences;

        self.set_flag(MeshGeometryClass::SKIN, true);
    }

    /// Access the per-vertex bone links if the data is present and aligned with the vertex array.
    pub fn vertex_bone_links(&self) -> Option<&[u16]> {
        if !self.vertices.is_empty() && self.vertex_bone_links.len() == self.vertices.len() {
            Some(&self.vertex_bone_links)
        } else {
            None
        }
    }

    pub fn vertex_influences(&self) -> Option<&[W3dVertInfStruct]> {
        if !self.vertices.is_empty() && self.vertex_influences.len() == self.vertices.len() {
            Some(&self.vertex_influences)
        } else {
            None
        }
    }

    pub(super) fn vertex_influence_view(&self, index: usize) -> ([u32; 4], [f32; 4]) {
        let mut indices = [0u32; 4];
        let mut weights = [0.0f32; 4];

        // CRITICAL: C++ uses single-bone-per-vertex skinning
        if let Some(influence) = self.vertex_influences.get(index) {
            indices[0] = influence.bone_idx as u32;
            weights[0] = 1.0; // Single bone, full weight
        } else if let Some(link) = self.vertex_bone_links.get(index) {
            indices[0] = *link as u32;
            weights[0] = 1.0;
        } else {
            weights[0] = 1.0;
        }

        let mut weight_sum = weights.iter().copied().sum::<f32>();
        if weight_sum <= f32::EPSILON {
            weights = [1.0, 0.0, 0.0, 0.0];
            weight_sum = 1.0;
        }

        let inv = 1.0 / weight_sum;
        for w in &mut weights {
            *w *= inv;
        }

        (indices, weights)
    }

    /// Get user text - equivalent to C++ Get_User_Text
    pub fn get_user_text(&self) -> Option<&str> {
        self.user_text.as_deref()
    }

    /// Set user text
    pub fn set_user_text(&mut self, text: &str) {
        self.user_text = Some(text.to_string());
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn mark_dirty(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.vertex_buffer = None;
        self.index_buffer = None;
    }

    /// Get W3D attributes
    pub fn get_w3d_attributes(&self) -> u32 {
        self.w3d_attributes
    }

    /// Set W3D attributes
    pub fn set_w3d_attributes(&mut self, attributes: u32) {
        if self.w3d_attributes == attributes {
            return;
        }
        self.w3d_attributes = attributes;
        self.mark_dirty();
    }

    /// Scale the mesh geometry - equivalent to C++ MeshModelClass::Scale
    pub fn scale_geometry(&mut self, scale: Vec3) {
        // Scale all vertices
        for vertex in &mut self.vertices {
            vertex.x *= scale.x;
            vertex.y *= scale.y;
            vertex.z *= scale.z;
        }
        self.mark_dirty();
    }

    /// Make geometry unique - equivalent to C++ Make_Geometry_Unique
    pub fn make_geometry_unique(&mut self) {
        self.mark_dirty();
    }

    /// Register the mesh for rendering with proper material pass ordering
    pub fn register_for_rendering(&mut self) {
        // Set vertex and index counts
        if !self.vertices.is_empty() {
            self.vertex_count = self.vertices.len() as u32;
        }

        if !self.triangles.is_empty() {
            self.index_count = (self.triangles.len() * 3) as u32;
        }

        self.sort_material_passes();
    }

    /// Sort material passes by render order for proper state management
    pub fn sort_material_passes(&mut self) {
        // Sort material passes to minimize state changes
        // 1. Opaque passes first
        // 2. Alpha-tested passes
        // 3. Transparent passes last
        // Within each category, sort by:
        // - Shader type
        // - Texture bindings
        // - Material properties

        self.material_passes.sort_by(|a, b| {
            use std::cmp::Ordering;

            // Primary sort: blend mode (opaque < alpha-test < transparent)
            let blend_order_a = Self::get_blend_sort_order(a);
            let blend_order_b = Self::get_blend_sort_order(b);

            let blend_cmp = blend_order_a.cmp(&blend_order_b);
            if blend_cmp != Ordering::Equal {
                return blend_cmp;
            }

            // Secondary sort: shader type
            let shader_cmp = a.shader.id().cmp(&b.shader.id());
            if shader_cmp != Ordering::Equal {
                return shader_cmp;
            }

            // Tertiary sort: texture count (fewer textures first for simpler passes)
            let tex_count_a = a.get_texture_count();
            let tex_count_b = b.get_texture_count();
            tex_count_a.cmp(&tex_count_b)
        });

        self.mark_dirty();
    }

    /// Get blend mode sort order for material pass ordering
    pub(super) fn get_blend_sort_order(pass: &MaterialPassClass) -> u32 {
        let base = match pass.shader.blend_mode() {
            MaterialBlendMode::Opaque => 0,
            MaterialBlendMode::Decal => 1,
            MaterialBlendMode::Multiply => 1, // Darken blend (same as decal)
            MaterialBlendMode::Alpha => 2,
            MaterialBlendMode::Additive => 3,
            MaterialBlendMode::Screen => 3, // Lighten blend (same as additive)
        };

        if base == 0
            && pass
                .vertex_material
                .as_ref()
                .map(|mat| mat.opacity < 1.0 || mat.translucency > 0.0)
                .unwrap_or(false)
        {
            2
        } else {
            base
        }
    }

    /// Get number of material passes
    pub fn get_pass_count(&self) -> usize {
        self.material_passes.len()
    }

    /// Get number of polygons (triangles)
    pub fn get_polygon_count(&self) -> usize {
        self.triangles.len()
    }

    /// Check if a texture stage has a texture array (per-polygon textures)
    /// Per-polygon texture arrays are not supported in this renderer path.
    pub fn has_texture_array(&self, _pass_idx: usize, _stage_idx: usize) -> bool {
        false
    }

    /// Peek at texture for a specific polygon, pass, and stage
    /// Returns None as we don't support per-polygon textures in the modern renderer
    pub fn peek_texture(
        &self,
        _poly_idx: usize,
        _pass_idx: usize,
        _stage_idx: usize,
    ) -> Option<&TextureClass> {
        None
    }

    /// Peek at single texture (shared across all polygons) for a pass and stage
    pub fn peek_single_texture(&self, pass_idx: usize, stage_idx: usize) -> Option<&TextureClass> {
        self.material_passes
            .get(pass_idx)
            .and_then(|pass| pass.textures.get(stage_idx))
            .and_then(|opt_tex| opt_tex.as_ref())
            .map(|arc_tex| arc_tex.as_ref())
    }

    /// Set texture for a specific polygon, pass, and stage
    /// This is a no-op in the modern renderer as we don't support per-polygon textures
    pub fn set_texture(
        &mut self,
        _poly_idx: usize,
        _new_texture: Arc<crate::texture_system::TextureClass>,
        _pass_idx: usize,
        _stage_idx: usize,
    ) {
        // Legacy C++ supported per-polygon textures, but modern renderer uses shared textures
        // This method is kept for API compatibility but does nothing
    }

    /// Set single texture (shared across all polygons) for a pass and stage
    pub fn set_single_texture(
        &mut self,
        new_texture: Arc<crate::texture_system::TextureClass>,
        pass_idx: usize,
        stage_idx: usize,
    ) {
        if let Some(pass) = self.material_passes.get_mut(pass_idx) {
            if stage_idx < pass.textures.len() {
                pass.textures[stage_idx] = Some(new_texture);
            }
        }
    }
}

impl Clone for MeshModelClass {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            vertices: self.vertices.clone(),
            normals: self.normals.clone(),
            triangles: self.triangles.clone(),
            material_info: self.material_info.clone(),
            shaders: self.shaders.clone(),
            vertex_materials: self.vertex_materials.clone(),
            vertex_bone_links: self.vertex_bone_links.clone(),
            vertex_influences: self.vertex_influences.clone(),
            texture_coords: self.texture_coords.clone(),
            stage_texture_coords: self.stage_texture_coords.clone(),
            per_stage_face_texcoord_ids: self.per_stage_face_texcoord_ids.clone(),
            stage_uv_sources: self.stage_uv_sources.clone(),
            sort_level: self.sort_level,
            flags: self.flags,
            polygon_renderer_list: self.polygon_renderer_list.clone(),
            material_passes: self.material_passes.clone(),
            vertex_buffer: None, // Cannot clone wgpu::Buffer
            index_buffer: None,  // Cannot clone wgpu::Buffer
            vertex_count: self.vertex_count,
            index_count: self.index_count,
            user_text: self.user_text.clone(),
            w3d_attributes: self.w3d_attributes,
            revision: self.revision,
        }
    }
}
