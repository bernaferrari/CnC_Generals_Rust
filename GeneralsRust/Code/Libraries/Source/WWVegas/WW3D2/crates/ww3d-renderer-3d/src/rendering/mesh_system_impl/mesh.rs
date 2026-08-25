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

/// Thread-safe debug ID counter for mesh objects
/// C++ Reference: Original code used static mut for mesh_debug_id assignment
/// Rust Implementation: Uses AtomicU32 with SeqCst ordering for thread safety
static MESH_DEBUG_ID_COUNT: AtomicU32 = AtomicU32::new(0);

impl Default for MeshClass {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshClass {
    pub fn new() -> Self {
        // Atomically increment and get the debug ID (thread-safe, no unsafe needed)
        let debug_id = MESH_DEBUG_ID_COUNT.fetch_add(1, Ordering::SeqCst);
        Self {
            name: String::new(),
            model: None,
            transform: Mat4::IDENTITY,
            bounding_box: AABoxClass::new(),
            bounding_sphere: SphereClass::new(Vec3::new(0.0, 0.0, 0.0), 0.0),
            sort_level: SORT_LEVEL_NONE,
            is_hidden: false,
            is_animation_hidden: false,
            alpha_override: 1.0,
            presentation_opacity: 1.0,
            material_pass_alpha_override: 1.0,
            material_pass_emissive_override: 1.0,
            frozen_fow_visibility: FrozenFowVisibility::default(),
            projected_shroud_eligible: false,
            lighting_environment: None,
            decal_meshes: Vec::new(),
            base_vertex_offset: 0,
            is_disabled_by_debugger: false,
            mesh_debug_id: debug_id,
            next_visible_skin: None,
            collision_type: 0xFFFFFFFF, // Default: collide with everything
            w3d_attributes: 0,
            material_info_cache: OnceLock::new(),
            decal_records: Vec::new(),
            deformed_world_vertices: None,
            bone_palette: Vec::new(),
            bone_palette_version: 0,
            uv_offset_override: None,
            is_decal_instance: false,
        }
    }

    pub fn get_name(&self) -> &str {
        if let Some(model) = &self.model {
            model.get_name()
        } else {
            &self.name
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
        if let Some(model_arc) = &mut self.model {
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.set_name(name);
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.set_name(name);
                *model_arc = Arc::new(cloned);
            }
        }
    }

    pub fn set_uv_offset_override(&mut self, offset: Option<[f32; 2]>) {
        self.uv_offset_override = offset;
    }

    pub fn uv_offset_override(&self) -> Option<[f32; 2]> {
        self.uv_offset_override
    }

    /// Set the frozen Drawable-level presentation opacity. Partial values
    /// force the ordinary mesh onto the alpha-blended render path; opaque
    /// values retain the authored material state.
    pub fn set_presentation_opacity(&mut self, opacity: f32) {
        self.presentation_opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if self.presentation_opacity < 0.999 && self.sort_level == SORT_LEVEL_NONE {
            self.sort_level = SORT_LEVEL_BIN1;
        }
    }

    #[inline]
    pub fn presentation_opacity(&self) -> f32 {
        self.presentation_opacity
    }

    /// Set the presentation snapshot consumed by every active material pass.
    /// This method intentionally accepts already-frozen values only.
    pub fn set_frozen_fow_visibility(&mut self, visibility: FrozenFowVisibility) {
        self.frozen_fow_visibility = visibility;
        // C++ wraps the complete RenderObj::Render call in its shroud pass.
        // Existing derived decal meshes therefore must not retain an earlier
        // clear snapshot when their parent is assigned frozen FOW later.
        for decal_mesh in &mut self.decal_meshes {
            Arc::make_mut(decal_mesh).set_frozen_fow_visibility(visibility);
        }
    }

    /// Return the immutable FOW snapshot associated with this mesh instance.
    #[inline]
    pub fn frozen_fow_visibility(&self) -> FrozenFowVisibility {
        self.frozen_fow_visibility
    }

    /// Install the exact frozen scene decision for the projected additional
    /// pass. This must never be derived from `FrozenFowVisibility`.
    pub fn set_projected_shroud_eligible(&mut self, eligible: bool) {
        self.projected_shroud_eligible = eligible;
        for decal_mesh in &mut self.decal_meshes {
            Arc::make_mut(decal_mesh).set_projected_shroud_eligible(eligible);
        }
    }

    #[inline]
    pub fn projected_shroud_eligible(&self) -> bool {
        self.projected_shroud_eligible
    }

    /// Install per-vertex bone links on the underlying model geometry.
    pub fn set_vertex_bone_links(&mut self, links: Vec<u16>) {
        if let Some(model_arc) = &mut self.model {
            Arc::make_mut(model_arc).set_vertex_bone_links(links);
        }
        if !self.bone_palette.is_empty() {
            let _ = self.recompute_deformed_vertices_from_palette();
        }
    }

    pub fn set_vertex_influences(&mut self, influences: Vec<W3dVertInfStruct>) {
        if let Some(model_arc) = &mut self.model {
            Arc::make_mut(model_arc).set_vertex_influences(influences);
        }
        if !self.bone_palette.is_empty() {
            let _ = self.recompute_deformed_vertices_from_palette();
        }
    }

    pub fn vertex_bone_links(&self) -> Option<&[u16]> {
        self.model
            .as_ref()
            .and_then(|model| model.vertex_bone_links())
    }

    /// Create WGPU buffers for the mesh model
    pub fn create_wgpu_buffers(&mut self, device: &wgpu::Device) {
        if let Some(model_arc) = self.model.as_mut() {
            Arc::make_mut(model_arc).create_wgpu_buffers(device);
        }
    }

    /// Update the cached bone palette and recompute skinned vertices when possible.
    pub fn set_bone_palette_slice(&mut self, matrices: &[Mat4]) {
        self.bone_palette.clear();
        self.bone_palette.extend_from_slice(matrices);
        self.bone_palette_version = self.bone_palette_version.wrapping_add(1);
        if self.bone_palette.is_empty() {
            self.deformed_world_vertices = None;
        } else {
            let _ = self.recompute_deformed_vertices_from_palette();
        }
    }

    /// Remove any cached palette information.
    pub fn clear_bone_palette(&mut self) {
        self.bone_palette.clear();
        self.bone_palette_version = self.bone_palette_version.wrapping_add(1);
        self.deformed_world_vertices = None;
    }

    /// Borrow the current palette together with its version counter.
    pub fn bone_palette_view(&self) -> Option<BonePaletteView<'_>> {
        if self.bone_palette.is_empty() {
            None
        } else {
            Some(BonePaletteView {
                matrices: &self.bone_palette,
                version: self.bone_palette_version,
            })
        }
    }

    pub(super) fn ensure_deformed_vertices_for_skin(&mut self) -> Option<&[Vec3]> {
        if self.deformed_world_vertices.is_none()
            && !self.recompute_deformed_vertices_from_palette()
        {
            return None;
        }
        self.deformed_world_vertices.as_deref()
    }

    pub(super) fn compute_deformed_vertices_from_palette(&self) -> Option<Vec<Vec3>> {
        let model_arc = self.model.as_ref()?;
        if self.bone_palette.is_empty() {
            return None;
        }

        let model_ref = model_arc.as_ref();
        let links = model_ref.vertex_bone_links();
        if let Some(links) = links.filter(|links| links.len() == model_ref.vertices.len()) {
            return Some(super::skin_deform::deform_vertices_single_bone(
                &model_ref.vertices,
                links,
                &self.bone_palette,
            ));
        }

        Some(super::skin_deform::deform_vertices_weighted(
            &model_ref.vertices,
            |index| model_ref.vertex_influence_view(index),
            &self.bone_palette,
            model_ref.vertex_bone_links(),
        ))
    }

    pub(super) fn recompute_deformed_vertices_from_palette(&mut self) -> bool {
        if let Some(vertices) = self.compute_deformed_vertices_from_palette() {
            self.deformed_world_vertices = Some(vertices);
            true
        } else {
            self.deformed_world_vertices = None;
            false
        }
    }

    // get_name method already defined in first impl block

    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.clear_deformed_world_vertices();
        self.update_cached_bounding_volumes();
    }

    pub fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    pub fn set_sort_level(&mut self, level: u32) {
        self.sort_level = level;
    }

    pub fn get_sort_level(&self) -> u32 {
        self.sort_level
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.is_hidden = hidden;
    }

    pub fn set_animation_hidden(&mut self, hidden: bool) {
        self.is_animation_hidden = hidden;
    }

    pub fn is_not_hidden_at_all(&self) -> bool {
        !self.is_hidden && !self.is_animation_hidden
    }

    pub fn get_bounding_box(&self) -> &AABoxClass {
        &self.bounding_box
    }

    pub fn set_lighting_environment(&mut self, env: Option<Arc<LightEnvironmentClass>>) {
        self.lighting_environment = env;
    }

    pub fn get_lighting_environment(&self) -> Option<&Arc<LightEnvironmentClass>> {
        self.lighting_environment.as_ref()
    }

    /// Cast ray against this mesh - equivalent to C++ MeshClass::Cast_Ray
    pub fn cast_ray(&self, raytest: &mut RayCollisionTestClass) -> bool {
        // Check collision type and visibility flags
        if (self.get_collision_type() & raytest.collision_type) == 0 {
            return false;
        }

        if (self.is_hidden || self.is_animation_hidden) && !raytest.check_hidden {
            return false;
        }

        if raytest.result.start_bad {
            return false;
        }

        let world = if let Some(model) = self.model.as_ref() {
            super::cast_ray_aligned::cast_ray_aligned_world(
                self.transform,
                super::mesh_camera_align::mesh_is_camera_aligned(model),
                super::mesh_camera_align::mesh_is_camera_oriented(model),
                raytest.line.start,
                (raytest.line.end - raytest.line.start).normalize_or_zero(),
            )
        } else {
            self.transform
        };
        let world_to_obj = world.inverse();
        let mut obj_ray = raytest.transformed_by_matrix(world_to_obj);
        obj_ray.result = RayCollisionResult::default();
        obj_ray.collided_render_obj = None;

        if let Some(model) = &self.model {
            if model.cast_ray(&mut obj_ray) {
                raytest.result = obj_ray.result.clone();
                raytest.result.normal = world
                    .transform_vector3(raytest.result.normal)
                    .normalize_or_zero();
                raytest.result.contact_point = world.transform_point3(obj_ray.result.contact_point);
                raytest.collided_render_obj = Some(self as *const MeshClass as usize);
                return true;
            }
        }
        false
    }

    /// Cast AABox against this mesh - equivalent to C++ MeshClass::Cast_AABox
    pub fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool {
        if (self.get_collision_type() & boxtest.collision_type) == 0 {
            return false;
        }

        // Transform AABox to object space
        let world_to_obj = self.transform.inverse();
        let mut obj_box = boxtest.transformed_by_matrix(world_to_obj);

        if let Some(model) = &self.model {
            if model.cast_aabox(&mut obj_box) {
                if let Some(result) = obj_box.result.clone() {
                    let transformed_contacts = result
                        .contact_points
                        .iter()
                        .map(|point| self.transform.transform_point3(*point))
                        .collect::<Vec<_>>();
                    boxtest.result = Some(AABoxCollisionResult {
                        intersection: result.intersection,
                        contact_points: transformed_contacts,
                    });
                } else {
                    boxtest.result = Some(AABoxCollisionResult {
                        intersection: true,
                        contact_points: Vec::new(),
                    });
                }
                boxtest.collided_render_obj = Some(self as *const MeshClass as usize);
                return true;
            } else {
                boxtest.result = obj_box.result;
            }
            false
        } else {
            false
        }
    }

    /// Test intersection with AABox - equivalent to C++ MeshClass::Intersect_AABox
    pub fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool {
        if (self.get_collision_type() & boxtest.collision_type) == 0 {
            return false;
        }

        // Transform AABox to object space
        let world_to_obj = self.transform.inverse();
        let obj_box = boxtest.transformed_by_matrix(world_to_obj);

        if let Some(model) = &self.model {
            model.intersect_aabox(&obj_box)
        } else {
            false
        }
    }

    /// Test intersection with OBBox - equivalent to C++ MeshClass::Intersect_OBBox
    pub fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool {
        if (self.get_collision_type() & boxtest.collision_type) == 0 {
            return false;
        }

        // Transform OBBox to object space
        let world_to_obj = self.transform.inverse();
        let obj_box = boxtest.transformed_by_matrix(world_to_obj);

        if let Some(model) = &self.model {
            model.intersect_obbox(&obj_box)
        } else {
            false
        }
    }

    /// Cast OBBox against this mesh — C++ MeshClass::Cast_OBBox
    pub fn cast_obbox(&self, boxtest: &mut OBBoxCollisionTestClass) -> bool {
        if (self.get_collision_type() & boxtest.collision_type) == 0 {
            return false;
        }

        let world_to_obj = self.transform.inverse();
        let mut obj_box = boxtest.transformed_by_matrix(world_to_obj);

        if let Some(model) = &self.model {
            if model.cast_obbox(&mut obj_box) {
                if let Some(result) = obj_box.result.clone() {
                    let transformed_contacts = result
                        .contact_points
                        .iter()
                        .map(|point| self.transform.transform_point3(*point))
                        .collect::<Vec<_>>();
                    boxtest.result = Some(OBBoxCollisionResult {
                        intersection: result.intersection,
                        contact_points: transformed_contacts,
                    });
                } else {
                    boxtest.result = Some(OBBoxCollisionResult {
                        intersection: true,
                        contact_points: Vec::new(),
                    });
                }
                boxtest.collided_render_obj = Some(self as *const MeshClass as usize);
                return true;
            } else {
                boxtest.result = obj_box.result;
            }
            false
        } else {
            false
        }
    }

    /// Create a decal on this mesh - equivalent to C++ MeshClass::Create_Decal
    pub fn create_decal(&mut self, generator: &mut DecalGeneratorClass) {
        if !ww3d_core::WW3D::are_decals_enabled() {
            return;
        }

        if !generator.allow_translucent_meshes() && self.is_translucent() {
            return;
        }

        let Some(model_arc) = self.model.as_ref().cloned() else {
            return;
        };

        let inv_transform = self.transform.inverse();
        let projector_volume_world = generator.get_bounding_volume();
        let material_pass = generator.material_pass();
        let projector_dir_world = generator.projector_direction();
        let projector_dir_obj = normalize_or(
            inv_transform.transform_vector3(projector_dir_world),
            Vec3::Z,
        );
        let surface_bias = generator.surface_bias();
        let bias_offset_obj = projector_dir_obj * surface_bias;
        let bias_offset_world = projector_dir_world * surface_bias;

        let mut record_vertices = Vec::new();
        let mut record_normals = Vec::new();
        let mut record_texcoords = Vec::new();
        let mut record_indices = Vec::new();

        if model_arc.get_flag(MeshGeometryClass::SKIN) {
            generator.set_mesh_transform(Mat4::IDENTITY);
            let axes_world = [
                normalize_or(projector_volume_world.basis[0], Vec3::X),
                normalize_or(projector_volume_world.basis[1], Vec3::Y),
                normalize_or(projector_volume_world.basis[2], Vec3::Z),
            ];
            let extents_world = Vec3::new(
                projector_volume_world.extent.x.abs(),
                projector_volume_world.extent.y.abs(),
                projector_volume_world.extent.z.abs(),
            );

            {
                let world_vertices: &[Vec3] = match self.ensure_deformed_vertices_for_skin() {
                    Some(verts) => verts,
                    None => {
                        debug!(
                            "Mesh '{}' missing skinned world vertices; using object-space transform as fallback",
                            self.get_name()
                        );
                        let transform = self.transform;
                        let fallback: Vec<Vec3> = model_arc
                            .vertices
                            .iter()
                            .map(|vertex| {
                                transform.transform_point3(Vec3::new(vertex.x, vertex.y, vertex.z))
                            })
                            .collect();
                        self.deformed_world_vertices = Some(fallback);
                        self.deformed_world_vertices.as_deref().unwrap()
                    }
                };

                let mut apt = Vec::new();
                model_arc.generate_skin_apt(&projector_volume_world, &mut apt, world_vertices);
                if apt.is_empty() {
                    debug!(
                        "Decal generator {} did not intersect skinned mesh '{}' geometry",
                        generator.get_decal_id(),
                        self.get_name()
                    );
                    return;
                }

                for poly_index in apt {
                    let Some(triangle) = model_arc.triangles.get(poly_index as usize) else {
                        continue;
                    };

                    let indices = triangle.vindex.map(|idx| idx as usize);
                    if indices.iter().any(|&idx| idx >= world_vertices.len()) {
                        continue;
                    }

                    let world_unbiased = [
                        world_vertices[indices[0]],
                        world_vertices[indices[1]],
                        world_vertices[indices[2]],
                    ];

                    let plane_normal_world = normalize_or(
                        (world_unbiased[1] - world_unbiased[0])
                            .cross(world_unbiased[2] - world_unbiased[0]),
                        projector_dir_world,
                    );
                    if plane_normal_world.dot(projector_dir_world) > generator.backface_threshold()
                    {
                        continue;
                    }

                    let plane_normal_obj = normalize_or(
                        inv_transform.transform_vector3(plane_normal_world),
                        projector_dir_obj,
                    );

                    let mut polygon = Vec::with_capacity(3);
                    for &vertex_index in &triangle.vindex {
                        let idx = vertex_index as usize;
                        if idx >= world_vertices.len() {
                            continue;
                        }
                        let biased_world = world_vertices[idx] + bias_offset_world;
                        let obj_vertex = inv_transform.transform_point3(biased_world);
                        let offset = biased_world - projector_volume_world.center;
                        let local = Vec3::new(
                            offset.dot(axes_world[0]),
                            offset.dot(axes_world[1]),
                            offset.dot(axes_world[2]),
                        );
                        polygon.push(ClipVertex {
                            obj_pos: obj_vertex,
                            world_pos: biased_world,
                            normal: plane_normal_obj,
                            local,
                        });
                    }

                    let clipped = clip_polygon_to_projector(polygon, extents_world);
                    if clipped.len() < 3 {
                        continue;
                    }

                    for tri_idx in 1..clipped.len() - 1 {
                        let v0 = &clipped[0];
                        let v1 = &clipped[tri_idx];
                        let v2 = &clipped[tri_idx + 1];
                        let face_normal = normalize_or(
                            (v1.obj_pos - v0.obj_pos).cross(v2.obj_pos - v0.obj_pos),
                            plane_normal_obj,
                        );

                        let base = record_vertices.len() as u32;
                        for vertex in [v0, v1, v2] {
                            record_vertices.push(vertex.obj_pos);
                            record_normals.push(normalize_or(vertex.normal, face_normal));
                            let tex = generator.compute_mesh_texture_coordinate(vertex.obj_pos);
                            record_texcoords.push(Vec2::new(tex.x, tex.y));
                        }
                        record_indices.extend_from_slice(&[base, base + 1, base + 2]);
                    }
                }
            }
        } else {
            let model_ref = model_arc.as_ref();
            let local_box = projector_volume_world.transformed(inv_transform);
            generator.set_mesh_transform(self.transform);
            let mut apt = Vec::new();
            model_ref.generate_rigid_apt(&local_box, &mut apt);
            if apt.is_empty() {
                debug!(
                    "Decal generator {} did not intersect mesh '{}' geometry",
                    generator.get_decal_id(),
                    self.get_name()
                );
                return;
            }

            let axes = [
                normalize_or(local_box.basis[0], Vec3::X),
                normalize_or(local_box.basis[1], Vec3::Y),
                normalize_or(local_box.basis[2], Vec3::Z),
            ];
            let extents = Vec3::new(
                local_box.extent.x.abs(),
                local_box.extent.y.abs(),
                local_box.extent.z.abs(),
            );

            for poly_index in apt {
                let Some(triangle) = model_ref.triangles.get(poly_index as usize) else {
                    continue;
                };

                let Some(verts) = triangle_vertices(triangle, &model_ref.vertices) else {
                    continue;
                };

                let plane_normal = {
                    let stored = Vec3::new(triangle.normal.x, triangle.normal.y, triangle.normal.z);
                    if stored.length_squared() > RAY_EPSILON {
                        stored.normalize()
                    } else {
                        normalize_or((verts[1] - verts[0]).cross(verts[2] - verts[0]), Vec3::Z)
                    }
                };

                if plane_normal.dot(projector_dir_obj) > generator.backface_threshold() {
                    continue;
                }

                let mut polygon = Vec::with_capacity(3);
                for &vertex_index in &triangle.vindex {
                    let obj_vertex =
                        Vec3::from(model_ref.vertices[vertex_index as usize]) + bias_offset_obj;
                    let world_vertex = self.transform.transform_point3(obj_vertex);
                    let vertex_normal = model_ref
                        .normals
                        .get(vertex_index as usize)
                        .map(|n| Vec3::from(*n))
                        .map(|n| normalize_or(n, plane_normal))
                        .unwrap_or(plane_normal);
                    let offset = obj_vertex - local_box.center;
                    let local = Vec3::new(
                        offset.dot(axes[0]),
                        offset.dot(axes[1]),
                        offset.dot(axes[2]),
                    );
                    polygon.push(ClipVertex {
                        obj_pos: obj_vertex,
                        world_pos: world_vertex,
                        normal: vertex_normal,
                        local,
                    });
                }

                let clipped = clip_polygon_to_projector(polygon, extents);
                if clipped.len() < 3 {
                    continue;
                }

                for tri_idx in 1..clipped.len() - 1 {
                    let v0 = &clipped[0];
                    let v1 = &clipped[tri_idx];
                    let v2 = &clipped[tri_idx + 1];
                    let face_normal = normalize_or(
                        (v1.obj_pos - v0.obj_pos).cross(v2.obj_pos - v0.obj_pos),
                        plane_normal,
                    );

                    let base = record_vertices.len() as u32;
                    for vertex in [v0, v1, v2] {
                        record_vertices.push(vertex.obj_pos);
                        record_normals.push(normalize_or(vertex.normal, face_normal));
                        let tex = generator.compute_mesh_texture_coordinate(vertex.world_pos);
                        record_texcoords.push(Vec2::new(tex.x, tex.y));
                    }
                    record_indices.extend_from_slice(&[base, base + 1, base + 2]);
                }
            }
        }

        if record_indices.is_empty() {
            debug!(
                "Decal generator {} had no clipped geometry on mesh '{}'",
                generator.get_decal_id(),
                self.get_name()
            );
            return;
        }

        let record = DecalRecord {
            id: generator.get_decal_id(),
            material_pass: material_pass.clone(),
            vertices: record_vertices,
            normals: record_normals,
            texcoords: record_texcoords,
            indices: record_indices,
        };

        self.decal_records.push(record);
        generator.add_mesh_handle(self as *const MeshClass);
        self.rebuild_decal_mesh();

        debug!(
            "Created decal {} on mesh '{}' ({} decals active)",
            generator.get_decal_id(),
            self.get_name(),
            self.decal_records.len()
        );
    }

    pub fn delete_decal(&mut self, decal_id: u32) {
        let previous = self.decal_records.len();
        self.decal_records.retain(|record| record.id != decal_id);

        if self.decal_records.len() == previous {
            return;
        }

        if self.decal_records.is_empty() {
            self.decal_meshes.clear();
        } else {
            self.rebuild_decal_mesh();
        }
    }

    /// Cache world-space deformed vertices so skin decals can project onto the animated surface.
    pub fn set_deformed_world_vertices(&mut self, vertices: Vec<Vec3>) {
        if let Some(model) = &self.model {
            if vertices.len() != model.vertices.len() {
                warn!(
                    "set_deformed_world_vertices mismatch for mesh '{}': received {} vertices, expected {}",
                    self.get_name(),
                    vertices.len(),
                    model.vertices.len()
                );
            }
        }
        self.deformed_world_vertices = Some(vertices);
    }

    /// Clear any cached deformed vertex data. Call when animation data is invalidated.
    pub fn clear_deformed_world_vertices(&mut self) {
        self.deformed_world_vertices = None;
    }

    /// Get number of polygons - equivalent to C++ MeshClass::Get_Num_Polys
    pub fn get_num_polys(&self) -> u32 {
        self.model.as_ref().map_or(0, |m| m.triangles.len() as u32)
    }

    /// Get object space bounding sphere - equivalent to C++ Get_Obj_Space_Bounding_Sphere
    pub fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        self.bounding_sphere
    }

    /// Get object space bounding box - equivalent to C++ Get_Obj_Space_Bounding_Box
    pub fn get_obj_space_bounding_box(&self) -> &AABoxClass {
        &self.bounding_box
    }

    pub(super) fn build_material_info_from_model(
        model: &MeshModelClass,
    ) -> crate::render_object_system::MaterialInfoClass {
        let mut vertex_materials = Vec::new();
        let mut textures = Vec::new();

        for pass in &model.material_passes {
            if let Some(vm) = &pass.vertex_material {
                vertex_materials.push((**vm).clone());
            }
            for stage in 0..MAX_TEXTURE_STAGES {
                if let Some(texture) = pass.get_texture(stage) {
                    textures.push(Arc::clone(texture));
                }
            }
        }

        crate::render_object_system::MaterialInfoClass {
            vertex_materials,
            textures,
            passes: model.material_passes.clone(),
        }
    }

    /// Update skin deformation - equivalent to C++ MeshClass::update_skin
    pub fn update_skin(&mut self) {
        let is_skinned = self
            .model
            .as_ref()
            .map(|model| model.get_flag(MeshGeometryClass::SKIN))
            .unwrap_or(false);

        if !is_skinned {
            self.clear_deformed_world_vertices();
            return;
        }

        let _ = self.recompute_deformed_vertices_from_palette();
        self.update_cached_bounding_volumes();
    }

    /// Get deformed vertices for skin - equivalent to C++ MeshClass::Get_Deformed_Vertices
    pub fn get_deformed_vertices(&self, vertices: &mut Vec<W3dVectorStruct>) {
        vertices.clear();

        let Some(model) = &self.model else {
            return;
        };

        let output_positions: Vec<Vec3> = if model.get_flag(MeshGeometryClass::SKIN) {
            if let Some(cached) = self.deformed_world_vertices.as_ref() {
                cached.clone()
            } else if let Some(computed) = self.compute_deformed_vertices_from_palette() {
                computed
            } else {
                model
                    .vertices
                    .iter()
                    .map(|vertex| Vec3::new(vertex.x, vertex.y, vertex.z))
                    .collect()
            }
        } else {
            model
                .vertices
                .iter()
                .map(|vertex| Vec3::new(vertex.x, vertex.y, vertex.z))
                .collect()
        };

        vertices.reserve(output_positions.len());
        for position in output_positions {
            vertices.push(W3dVectorStruct {
                x: position.x,
                y: position.y,
                z: position.z,
            });
        }
    }

    /// Make mesh unique in renderer - equivalent to C++ MeshClass::Make_Unique
    pub fn make_unique(&mut self) {
        if let Some(model_arc) = &mut self.model {
            // Ensure unique before mutating
            if let Some(model_mut) = Arc::get_mut(model_arc) {
                model_mut.make_geometry_unique();
            } else {
                let mut cloned = (**model_arc).clone();
                cloned.make_geometry_unique();
                *model_arc = Arc::new(cloned);
            }

            // Update any cached data that might reference shared geometry
            self.update_cached_bounding_volumes();
        }
    }

    /// Replace vertex material - equivalent to C++ MeshClass::Replace_VertexMaterial
    pub fn replace_vertex_material(
        &mut self,
        old_material: &VertexMaterialClass,
        new_material: &VertexMaterialClass,
    ) {
        if let Some(model_arc) = self.model.as_mut() {
            let model = Arc::make_mut(model_arc);
            // Replace all instances of old_material with new_material in all material passes
            for pass in &mut model.material_passes {
                if let Some(vertex_material) = &mut pass.vertex_material {
                    if vertex_material.name == old_material.name {
                        *vertex_material = Arc::new(new_material.clone());
                    }
                }
            }
            model.mark_dirty();
            let _ = self.material_info_cache.take();
        }
    }

    /// Render specific material pass - equivalent to C++ MeshClass::Render_Material_Pass
    pub fn render_material_pass<'a>(
        &'a self,
        pass: &MaterialPassClass,
        index_buffer: Option<&'a wgpu::Buffer>,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) -> W3dResult<()> {
        if let Some(model) = &self.model {
            if let Some(vertex_buffer) = &model.vertex_buffer {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                if let Some(index_buf) = index_buffer {
                    render_pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
                    let pass_ranges = compute_pass_index_ranges(model, &[]);
                    let pass_index = pass.get_pass_index();
                    let (start_index, index_count) = if pass_index < pass_ranges.len() {
                        pass_ranges[pass_index]
                    } else if !pass_ranges.is_empty() {
                        pass_ranges[0]
                    } else {
                        (0, model.index_count)
                    };
                    if index_count > 0 {
                        render_pass.draw_indexed(start_index..start_index + index_count, 0, 0..1);
                    }
                } else {
                    render_pass.draw(0..model.vertex_count, 0..1);
                }
            }
        }

        Ok(())
    }

    /// Get collision type - equivalent to C++ Get_Collision_Type
    pub fn get_collision_type(&self) -> u32 {
        self.collision_type
    }

    /// Set collision type - equivalent to C++ Set_Collision_Type
    pub fn set_collision_type(&mut self, collision_type: u32) {
        self.collision_type = collision_type;
    }

    /// Check if mesh contains a point - equivalent to C++ MeshClass::Contains
    pub fn contains(&self, point: Vec3) -> bool {
        // Transform point to object space
        let obj_point = self.transform.inverse().transform_point3(point);

        if let Some(_model) = &self.model {
            // Fast rejection: check bounding sphere first
            let distance = obj_point.distance(self.bounding_sphere.center);
            if distance > self.bounding_sphere.radius {
                return false;
            }

            // Use sphere containment as approximation
            // Note: Precise containment would require ray casting through mesh triangles
            // (odd/even intersection count). C++ MeshClass::Contains uses AABTree traversal.
            distance <= self.bounding_sphere.radius
        } else {
            false
        }
    }

    /// Clone the mesh - equivalent to C++ MeshClass::Clone
    pub fn clone_mesh(&self) -> MeshClass {
        let mut new_mesh = MeshClass::new();

        // Copy basic properties
        new_mesh.name = self.name.clone();
        new_mesh.transform = self.transform;
        new_mesh.bounding_box = self.bounding_box;
        new_mesh.bounding_sphere = self.bounding_sphere;
        new_mesh.sort_level = self.sort_level;
        new_mesh.is_hidden = self.is_hidden;
        new_mesh.is_animation_hidden = self.is_animation_hidden;
        new_mesh.alpha_override = self.alpha_override;
        new_mesh.presentation_opacity = self.presentation_opacity;
        new_mesh.material_pass_alpha_override = self.material_pass_alpha_override;
        new_mesh.material_pass_emissive_override = self.material_pass_emissive_override;
        new_mesh.frozen_fow_visibility = self.frozen_fow_visibility;
        new_mesh.projected_shroud_eligible = self.projected_shroud_eligible;
        new_mesh.collision_type = self.collision_type;
        new_mesh.w3d_attributes = self.w3d_attributes;
        new_mesh.is_decal_instance = self.is_decal_instance;
        new_mesh.uv_offset_override = self.uv_offset_override;

        // Clone the model if it exists
        if let Some(model) = &self.model {
            new_mesh.model = Some(Arc::new((**model).clone()));
        }

        // Clone lighting environment
        new_mesh.lighting_environment = self.lighting_environment.clone();

        new_mesh.decal_records = self.decal_records.clone();
        new_mesh.deformed_world_vertices = self.deformed_world_vertices.clone();
        new_mesh.decal_meshes = self.decal_meshes.clone();
        new_mesh.rebuild_decal_mesh();

        new_mesh
    }

    pub(super) fn rebuild_decal_mesh(&mut self) {
        if self.decal_records.is_empty() {
            self.decal_meshes.clear();
            return;
        }

        let mut grouped: BTreeMap<usize, Vec<&DecalRecord>> = BTreeMap::new();
        for record in &self.decal_records {
            let key = Arc::as_ptr(&record.material_pass) as usize;
            grouped.entry(key).or_default().push(record);
        }

        self.decal_meshes.clear();

        for (group_index, records) in grouped.values().enumerate() {
            let mut combined_positions: Vec<Vec3> = Vec::new();
            let mut combined_normals: Vec<Vec3> = Vec::new();
            let mut combined_texcoords: Vec<Vec2> = Vec::new();
            let mut combined_triangles: Vec<[u32; 3]> = Vec::new();

            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);

            for record in records {
                let base = combined_positions.len() as u32;
                for pos in &record.vertices {
                    min = min.min(*pos);
                    max = max.max(*pos);
                }

                combined_positions.extend_from_slice(&record.vertices);
                combined_normals.extend_from_slice(&record.normals);
                combined_texcoords.extend_from_slice(&record.texcoords);

                for chunk in record.indices.chunks(3) {
                    if chunk.len() < 3 {
                        continue;
                    }
                    combined_triangles.push([base + chunk[0], base + chunk[1], base + chunk[2]]);
                }
            }

            if combined_positions.is_empty() || combined_triangles.is_empty() {
                continue;
            }

            let vertices: Vec<W3dVectorStruct> = combined_positions
                .iter()
                .copied()
                .map(W3dVectorStruct::from)
                .collect();
            let normals: Vec<W3dVectorStruct> = combined_normals
                .iter()
                .copied()
                .map(|n| W3dVectorStruct::from(normalize_or(n, Vec3::Z)))
                .collect();
            let texcoords: Vec<W3dTexCoordStruct> = combined_texcoords
                .iter()
                .map(|uv| W3dTexCoordStruct { u: uv.x, v: uv.y })
                .collect();
            let triangles: Vec<W3dTriangleStruct> = combined_triangles
                .iter()
                .map(|tri| {
                    let p0 = combined_positions[tri[0] as usize];
                    let p1 = combined_positions[tri[1] as usize];
                    let p2 = combined_positions[tri[2] as usize];
                    let normal = normalize_or((p1 - p0).cross(p2 - p0), Vec3::Z);
                    let distance = -normal.dot(p0);
                    W3dTriangleStruct {
                        vindex: *tri,
                        attributes: 0,
                        normal: W3dVectorStruct::from(normal),
                        distance,
                    }
                })
                .collect();

            let material_pass = records
                .first()
                .map(|record| Arc::clone(&record.material_pass))
                .expect("records must be non-empty");

            let mut model =
                MeshModelClass::new(&format!("{}_Decals_{}", self.get_name(), group_index));
            model.vertices = vertices;
            model.normals = normals;
            model.texture_coords = texcoords;
            model.triangles = triangles;
            model.vertex_count = model.vertices.len() as u32;
            model.index_count = (combined_triangles.len() * 3) as u32;
            model.material_passes = vec![material_pass.as_ref().clone()];
            model.register_for_rendering();

            let mut decal_mesh = MeshClass::new();
            decal_mesh.name = format!("{}_DecalMesh_{}", self.get_name(), group_index);
            decal_mesh.model = Some(Arc::new(model));
            decal_mesh.transform = self.transform;
            decal_mesh.sort_level = self.sort_level;
            decal_mesh.alpha_override = 1.0;
            decal_mesh.presentation_opacity = self.presentation_opacity;
            decal_mesh.material_pass_alpha_override = 1.0;
            decal_mesh.material_pass_emissive_override = 1.0;
            decal_mesh.frozen_fow_visibility = self.frozen_fow_visibility;
            decal_mesh.projected_shroud_eligible = self.projected_shroud_eligible;
            decal_mesh.is_decal_instance = true;

            let bounding_box = AABoxClass::from_min_max(min, max);
            let obj_center = (min + max) * 0.5;
            let world_center = self.transform.transform_point3(obj_center);
            let mut radius: f32 = 0.0;
            for pos in &combined_positions {
                let world_pos = self.transform.transform_point3(*pos);
                radius = radius.max((world_pos - world_center).length());
            }

            decal_mesh.bounding_box = bounding_box;
            decal_mesh.bounding_sphere = SphereClass::new(world_center, radius);

            self.decal_meshes.push(Arc::new(decal_mesh));
        }
    }
}
