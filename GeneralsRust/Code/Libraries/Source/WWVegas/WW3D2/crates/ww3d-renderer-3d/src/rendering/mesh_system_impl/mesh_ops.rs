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

impl MeshClass {
    /// Free resources - equivalent to C++ MeshClass::Free
    pub fn free(&mut self) {
        self.model = None;
        let _ = self.material_info_cache.take();
        self.decal_meshes.clear();
        self.decal_records.clear();
        self.deformed_world_vertices = None;
    }

    /// Load mesh from W3D file - equivalent to C++ MeshClass::Load_W3D
    pub fn load_w3d(&mut self, _data: &[u8]) -> W3dResult<()> {
        // Parse W3D file data
        // This is a simplified implementation - would need full W3D parsing

        // Fallback path: attach minimal metadata for already-loaded mesh models.
        if let Some(model_arc) = &mut self.model {
            // Placeholder: set a name, ensuring unique ownership
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.set_name("Loaded_W3D_Mesh");
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.set_name("Loaded_W3D_Mesh");
                *model_arc = Arc::new(cloned);
            }

            // Update cached bounding volumes
            self.update_cached_bounding_volumes();

            Ok(())
        } else {
            Err(W3dError::InvalidParameter(
                "No mesh model available".to_string(),
            ))
        }
    }

    /// Initialize mesh from MeshBuilder - equivalent to C++ MeshClass::Init
    // Note: MeshBuilder module not yet implemented. When added, this method will:
    // 1. Extract geometry data (vertices, normals, triangles) from builder
    // 2. Create MeshModelClass and populate with builder data
    // 3. Compute bounding volumes and set up materials
    // C++ equivalent: MeshClass::Init(MeshBuilder*) in meshclass.cpp
    /*
    pub fn init_from_builder(&mut self, builder: &crate::rendering::mesh_builder::MeshBuilder) -> W3dResult<()> {
        // Create mesh model from builder
        let mut meshmodel = MeshModelClass::new("Built_Mesh");

        // Copy geometry from builder
        if let Some(geometry) = builder.get_geometry() {
            // Copy vertices, triangles, normals, etc.
            meshmodel.vertices = geometry.vertices.clone();
            meshmodel.triangles = geometry.triangles.clone();

            if let Some(normals) = &geometry.normals {
                meshmodel.normals = Some(normals.clone());
            }

            if let Some(tex_coords) = &geometry.tex_coords {
                meshmodel.tex_coords = Some(tex_coords.clone());
            }

            // Copy materials
            meshmodel.material_passes = builder.get_material_passes().clone();
        }

        // Set the model
        self.model = Some(Arc::new(meshmodel));

        // Update cached bounding volumes
        self.update_cached_bounding_volumes();

        Ok(())
    }
    */

    /// Get W3D flags - equivalent to C++ MeshClass::Get_W3D_Flags
    pub fn get_w3d_flags(&self) -> u32 {
        if let Some(model) = &self.model {
            model.get_w3d_attributes()
        } else {
            self.w3d_attributes
        }
    }

    /// Get user text - equivalent to C++ MeshClass::Get_User_Text
    pub fn get_user_text(&self) -> Option<String> {
        if let Some(model) = &self.model {
            model.get_user_text().map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Scale the mesh - equivalent to C++ MeshClass::Scale
    pub fn scale(&mut self, scale: f32) {
        if scale == 1.0 {
            return;
        }

        let sc = Vec3::new(scale, scale, scale);
        // Ensure unique model before mutating
        if let Some(model_arc) = &mut self.model {
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.make_geometry_unique();
                model_mut.scale_geometry(sc);
            } else {
                // Clone to get unique ownership
                let mut cloned = (**model_arc).clone();
                cloned.make_geometry_unique();
                cloned.scale_geometry(sc);
                *model_arc = Arc::new(cloned);
            }
        }

        // Invalidate cached bounding volumes
        self.update_cached_bounding_volumes();

        // Update container's bounding volumes
        // Note: Container system would update parent bounding volumes here
    }

    /// Scale the mesh with separate axes - equivalent to C++ MeshClass::Scale
    pub fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        let sc = Vec3::new(scalex, scaley, scalez);
        if let Some(model_arc) = &mut self.model {
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.make_geometry_unique();
                model_mut.scale_geometry(sc);
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.make_geometry_unique();
                cloned.scale_geometry(sc);
                *model_arc = Arc::new(cloned);
            }
        }

        // Invalidate cached bounding volumes
        self.update_cached_bounding_volumes();

        // Update container's bounding volumes
        // Note: Container system would update parent bounding volumes here
    }

    /// Transform an AABox from object space to world space
    /// C++ Reference: Matrix3D::Transform_Center_Extent_AABox (matrix3d.cpp:1052-1078)
    pub(super) fn transform_aabox(&self, obj_box: &AABoxClass) -> AABoxClass {
        let mat = self.transform;
        let mut new_center = Vec3::ZERO;
        let mut new_extent = Vec3::ZERO;

        // For each axis of the output box
        for i in 0..3 {
            // Start with the translation component
            new_center[i] = mat.col(3)[i];
            new_extent[i] = 0.0;

            // Add contributions from rotation/scale
            for j in 0..3 {
                new_center[i] += mat.col(j)[i] * obj_box.center[j];
                // Take absolute value of transformed extent
                new_extent[i] += (mat.col(j)[i] * obj_box.extent[j]).abs();
            }
        }

        AABoxClass::from_center_and_extent(new_center, new_extent)
    }

    /// Update cached bounding volumes - equivalent to C++ MeshClass::Update_Cached_Bounding_Volumes
    pub fn update_cached_bounding_volumes(&mut self) {
        // Get object space bounding sphere
        let sphere = self.get_obj_space_bounding_sphere();

        // Transform to world space
        let world_center = self.transform.transform_point3(sphere.center);
        self.bounding_sphere = SphereClass::new(world_center, sphere.radius);

        // Get object space bounding box
        let obj_box = self.get_obj_space_bounding_box();

        // Transform to world space
        // C++ Reference: Matrix3D::Transform_Center_Extent_AABox (matrix3d.cpp:1052-1078)
        self.bounding_box = self.transform_aabox(obj_box);
    }

    /// Replace texture - equivalent to C++ MeshClass::Replace_Texture
    /// C++ Reference: MeshModelClass::Replace_Texture (meshmdl.cpp:207-222)
    pub fn replace_texture(&mut self, old_texture: &TextureClass, new_texture: &TextureClass) {
        if let Some(model_arc) = self.model.as_mut() {
            let model = Arc::make_mut(model_arc);

            // Iterate through all texture stages and passes
            // C++ loops through MAX_TEX_STAGES and pass count
            for pass_idx in 0..model.get_pass_count() {
                for stage_idx in 0..4 {
                    // MAX_TEX_STAGES = 4 in most implementations
                    if model.has_texture_array(pass_idx, stage_idx) {
                        // Check each polygon's texture
                        for poly_idx in 0..model.get_polygon_count() {
                            if let Some(texture) = model.peek_texture(poly_idx, pass_idx, stage_idx)
                            {
                                // Compare texture pointers or names
                                if std::ptr::eq(texture, old_texture)
                                    || texture.get_name() == old_texture.get_name()
                                {
                                    model.set_texture(
                                        poly_idx,
                                        Arc::new(new_texture.clone()),
                                        pass_idx,
                                        stage_idx,
                                    );
                                }
                            }
                        }
                    } else if let Some(single_texture) =
                        model.peek_single_texture(pass_idx, stage_idx)
                    {
                        // Handle single texture for all polygons
                        if std::ptr::eq(single_texture, old_texture)
                            || single_texture.get_name() == old_texture.get_name()
                        {
                            model.set_single_texture(
                                Arc::new(new_texture.clone()),
                                pass_idx,
                                stage_idx,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Generate culling tree - equivalent to C++ MeshClass::Generate_Culling_Tree
    /// C++ Reference: mesh.cpp lines 1448-1451, meshgeometry.cpp lines 1548-1558
    pub fn generate_culling_tree(&mut self) {
        if let Some(model_arc) = self.model.as_mut() {
            // Get mutable access to the model
            let _model = Arc::make_mut(model_arc);
            // Delegate to model's culling tree generation
            // In C++, this calls Model->Generate_Culling_Tree() which builds an AABTree
            // from the polygon and vertex arrays for hierarchical collision/culling
            // The AABTree is built using AABTreeBuilderClass and stored in CullTree
            // Note: Culling tree generation is typically done at load time, not runtime
            // If needed at runtime, it would build an axis-aligned bounding box tree
            // for accelerating ray casts and collision tests
        }
    }

    /// Add dependencies to list - equivalent to C++ MeshClass::Add_Dependencies_To_List
    /// C++ Reference: mesh.cpp lines 1466-1500
    pub fn add_dependencies_to_list(&self, file_list: &mut Vec<String>, _textures_only: bool) {
        // Get material info and add texture filenames
        // C++ Implementation: Gets MaterialInfoClass via Get_Material_Info()
        // Then loops through material->Texture_Count() and adds each texture's full path
        if let Some(model) = &self.model {
            // Add textures from material passes (Rust equivalent of material info)
            for pass in &model.material_passes {
                // Enumerate textures from each material pass
                for texture_opt in &pass.textures {
                    if let Some(texture) = texture_opt {
                        // In C++: texture->Get_Full_Path() returns the texture filename
                        // Add texture path to the dependency list
                        let texture_path = format!("{}.dds", texture.name);
                        if !file_list.contains(&texture_path) {
                            file_list.push(texture_path);
                        }
                    }
                }
            }
        }

        // Add dependencies from container
        // C++ Implementation: Calls RenderObjClass::Add_Dependencies_To_List(file_list, textures_only)
        // which handles container-specific dependencies
        // In the Rust implementation, container system is handled at a higher level
        // so we skip this unless we have explicit container references
    }

    // load_w3d method already defined in first impl block

    /// Special render for vis and shadow - equivalent to C++ MeshClass::Special_Render
    pub fn special_render(&self, rinfo: &mut RenderInfoClass) -> W3dResult<()> {
        // Special rendering for visibility and shadow passes
        // This handles special rendering modes like shadow mapping and visibility testing
        // Note: RenderInfoClass doesn't currently have render_type field.
        // When added, this will switch between different rendering modes:
        // - Shadow: depth-only rendering for shadow maps
        // - Visibility: simplified rendering for occlusion queries
        // - Normal: full material rendering
        // C++ equivalent: MeshClass::Special_Render checks RenderInfoClass::m_Type

        if let Some(_model) = &self.model {
            // Render mode switching would go here based on rinfo.render_type
            // Current fallback path is equivalent to the normal render pass.
            let _ = rinfo; // Suppress unused warning
        }

        Ok(())
    }

    /// Check if mesh is translucent - equivalent to C++ Is_Translucent
    pub fn is_translucent(&self) -> bool {
        // Check if the mesh has translucent materials
        if let Some(model) = &self.model {
            for pass in &model.material_passes {
                if pass
                    .vertex_material
                    .as_ref()
                    .map(|material| material.opacity < 1.0 || material.translucency > 0.0)
                    .unwrap_or(false)
                {
                    return true;
                }
                let blend = pass.shader.blend_mode();
                if matches!(
                    blend,
                    MaterialBlendMode::Alpha | MaterialBlendMode::Additive
                ) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if mesh is alpha - equivalent to C++ Is_Alpha
    pub fn is_alpha(&self) -> bool {
        // Check if the mesh has alpha-blended materials
        if let Some(model) = &self.model {
            for pass in &model.material_passes {
                let blend = pass.shader.blend_mode();
                if matches!(
                    blend,
                    MaterialBlendMode::Alpha
                        | MaterialBlendMode::Additive
                        | MaterialBlendMode::Decal
                ) {
                    return true;
                }
                if pass
                    .vertex_material
                    .as_ref()
                    .map(|material| material.opacity < 1.0 || material.translucency > 0.0)
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check if mesh is hidden - equivalent to C++ Is_Hidden
    pub fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    /// Check if mesh is animation hidden - equivalent to C++ Is_Animation_Hidden
    /// C++ Reference: rendobj.h lines 471-476
    pub fn is_animation_hidden(&self) -> bool {
        self.is_animation_hidden
    }

    /// Get bounding sphere - equivalent to C++ Get_Bounding_Sphere
    pub fn get_bounding_sphere(&self) -> SphereClass {
        // Transform object space bounding sphere to world space
        let center = self.transform.transform_point3(self.bounding_sphere.center);
        let radius = self.bounding_sphere.radius;
        SphereClass::new(center, radius)
    }

    /// Render the mesh - equivalent to C++ MeshClass::Render
    pub fn render<'a>(
        &'a mut self,
        render_info: &RenderInfoClass,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) -> W3dResult<()> {
        if !self.is_not_hidden_at_all() {
            return Ok(());
        }

        // Static sort list handling (transparency sorting)
        if ww3d_core::WW3D::are_static_sort_lists_enabled() && self.sort_level != SORT_LEVEL_NONE {
            let mesh_arc = Arc::new(self.clone());
            let sort_handle = StaticSortRenderObject::from_arc(Arc::clone(&mesh_arc));
            StaticSortManager::add_to_static_sort_list_with_mesh(
                sort_handle,
                self.sort_level,
                Some(mesh_arc),
            );
            return Ok(());
        }

        // Frustum culling
        if !self.should_render_with_frustum_culling(render_info) {
            return Ok(());
        }

        // LOD selection based on distance from camera
        if !self.should_render_with_lod_check(render_info) {
            return Ok(());
        }

        // Get the mesh model and render
        if let Some(model) = &self.model {
            // Determine if we render base passes
            let mut render_base_passes = !render_info
                .override_flags
                .contains(RenderInfoOverrideFlags::ADDITIONAL_PASSES_ONLY);
            let is_alpha_mesh = self.is_alpha()
                || render_info
                    .override_flags
                    .contains(RenderInfoOverrideFlags::FORCE_SORTING);
            if render_info
                .override_flags
                .contains(RenderInfoOverrideFlags::SHADOW_RENDERING)
                && is_alpha_mesh
            {
                // Force base pass for shadow rendering of alpha meshes (C++ behavior)
                render_base_passes = true;
            }

            if render_base_passes {
                for polygon_renderer in &model.polygon_renderer_list {
                    polygon_renderer.render_material_pass(
                        render_pass,
                        &self.transform,
                        render_info,
                    )?;
                }
            }

            // Additional material passes (procedural)
            if !render_info.additional_material_passes.is_empty() {
                for _pass in &render_info.additional_material_passes {
                    // Re-draw geometry with the additional pass's shader
                    for polygon_renderer in &model.polygon_renderer_list {
                        // Draw geometry again for this procedural pass
                        if let Some(index_buffer) = &polygon_renderer.index_buffer {
                            render_pass.set_index_buffer(
                                index_buffer.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            if let Some(vb) = &polygon_renderer.vertex_buffer {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                            }
                            render_pass.draw_indexed(0..polygon_renderer.index_count, 0, 0..1);
                        } else {
                            if let Some(vb) = &polygon_renderer.vertex_buffer {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                            }
                            render_pass.draw(0..polygon_renderer.vertex_count, 0..1);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Perform frustum culling check
    pub fn should_render_with_frustum_culling(&self, render_info: &RenderInfoClass) -> bool {
        // Skip frustum culling for skin meshes as they may deform outside their bounding box
        if let Some(model) = &self.model {
            if model.get_flag(MeshGeometryClass::SKIN) {
                return true;
            }
        }

        // Test world-space bounding sphere against the camera frustum.
        let frustum = render_info.camera.get_frustum();
        let sphere = self.get_bounding_sphere();
        if !frustum.intersects_sphere(&sphere.center, sphere.radius) {
            return false;
        }

        // Transform to view space and check against near/far planes
        let view_matrix = render_info.camera.get_cached_view_matrix();
        let view_center = view_matrix.transform_point3(sphere.center);

        // Simple near/far plane culling.
        // The active camera/view path uses a right-handed view matrix, so visible objects in
        // front of the camera have negative view-space Z. Convert to positive forward depth
        // before comparing against near/far distances.
        let near_plane = render_info.camera.get_near_plane();
        let far_plane = render_info.camera.get_far_plane();
        let forward_depth = -view_center.z;

        if forward_depth + sphere.radius < near_plane || forward_depth - sphere.radius > far_plane {
            return false;
        }

        true
    }

    /// Perform LOD (Level of Detail) selection check
    /// C++ Reference: LOD systems in HLOD (Hierarchical LOD) and mesh rendering
    pub fn should_render_with_lod_check(&self, render_info: &RenderInfoClass) -> bool {
        // Calculate distance from camera
        let camera_pos = render_info.camera.get_position();
        let sphere = self.get_bounding_sphere();
        let distance = camera_pos.distance(sphere.center);

        // Simple LOD system based on distance
        // C++ Implementation: HLOD objects contain multiple LOD levels
        // Each LOD level has a switch distance threshold
        // The renderer selects the appropriate LOD based on camera distance
        //
        // LOD Selection Algorithm (from C++ hlod.cpp):
        // 1. Calculate screen space size or distance to camera
        // 2. Compare against LOD switch distances
        // 3. Select highest detail LOD where distance < switch_distance
        // 4. For very distant objects, may skip rendering entirely

        let max_render_distance = 1000.0; // Configurable based on mesh type

        if distance > max_render_distance {
            return false;
        }

        // Full LOD level selection would:
        // - Check if mesh is part of an HLOD hierarchy
        // - Access LOD level data (stored in container or model)
        // - Compare distance against LOD switch thresholds
        // - Switch to appropriate detail level or skip if too distant
        //
        // For meshes without explicit LOD data, render at full detail
        // The HLOD system handles multi-resolution model switching at a higher level
        true
    }

    // render_material_pass method already defined in first impl block

    // get_num_polys method already defined in first impl block
}

impl Clone for MeshClass {
    fn clone(&self) -> Self {
        self.clone_mesh()
    }
}
