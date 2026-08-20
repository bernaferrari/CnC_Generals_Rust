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

impl crate::render_object_system::RenderObjClass for MeshClass {
    fn clone_obj(&self) -> Box<dyn crate::render_object_system::RenderObjClass> {
        Box::new(self.clone())
    }

    fn class_id(&self) -> ww3d_core::RenderObjClassId {
        ww3d_core::RenderObjClassId::Mesh
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    fn get_num_polys(&self) -> usize {
        if let Some(model) = &self.model {
            model.triangles.len()
        } else {
            0
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn render(
        &self,
        rinfo: &crate::render_object_system::RenderInfoClass,
    ) -> crate::core::error::Result<()> {
        if !self.is_not_hidden_at_all() || self.is_disabled_by_debugger {
            return Ok(());
        }

        // C++ MeshClass::Render: skins skip frustum, others Overlap_Test(bbox).
        if !self.should_render_with_frustum_culling(rinfo) {
            return Ok(());
        }

        let mut render_base_passes = !rinfo
            .current_override_flags()
            .contains(crate::render_object_system::RenderInfoOverrideFlags::ADDITIONAL_PASSES_ONLY);
        if rinfo
            .current_override_flags()
            .contains(crate::render_object_system::RenderInfoOverrideFlags::SHADOW_RENDERING)
            && self.is_alpha()
        {
            render_base_passes = true;
        }

        // Static-sort deferral matches C++ SORT_LEVEL_NONE check.
        if ww3d_core::WW3D::are_static_sort_lists_enabled() && self.sort_level != SORT_LEVEL_NONE {
            return Ok(());
        }

        let _ = render_base_passes;
        Ok(())
    }

    fn special_render(
        &self,
        _rinfo: &crate::render_object_system::SpecialRenderInfoClass,
    ) -> crate::core::error::Result<()> {
        // C++ Reference: mesh.cpp lines 1027-1070
        // Special render passes handle visibility testing and shadow rendering
        //
        // C++ Implementation handles two main render types:
        // 1. RENDER_VIS: Visibility/occlusion rendering
        //    - Uses specialized rasterizer for visibility testing
        //    - Enables two-sided rendering if mesh has TWO_SIDED flag
        //    - For skinned meshes, deforms vertices before rendering
        //    - Used for occlusion culling calculations
        //
        // 2. RENDER_SHADOW: Shadow map rendering
        //    - Calls Model->Shadow_Render() with transform and hierarchy
        //    - Renders mesh geometry into shadow maps
        //    - May use simplified materials (no textures, just depth)
        //
        // Additional render types could include:
        // - Reflection rendering (for water reflections)
        // - Glow/bloom passes
        // - Outline rendering (for selection highlights)
        //
        // For full implementation, would need:
        // - Access to specialized rasterizers (vis, shadow)
        // - Skinning deformation for dynamic meshes
        // - Material pass filtering based on render type
        //
        // Currently returns Ok as special passes are handled by the main renderer
        Ok(())
    }

    fn cast_ray(&self, raytest: &mut crate::render_object_system::RayCollisionTestClass) -> bool {
        // C++ Reference: meshgeometry.cpp Cast_Ray implementation
        // Transforms ray to object space, tests against triangles, returns closest hit
        if let Some(model) = &self.model {
            // Transform ray from world space to object space
            let inv_transform = self.transform.inverse();
            let mut local_ray = raytest.clone();
            local_ray.line.start = inv_transform.transform_point3(raytest.line.start);
            local_ray.line.end = inv_transform.transform_point3(raytest.line.end);

            // Cast ray in object space
            if model.cast_ray(&mut local_ray) {
                // Transform result back to world space
                let contact_point = local_ray.result.contact_point;
                let normal = local_ray.result.normal;
                raytest.result = local_ray.result;
                raytest.result.contact_point = self.transform.transform_point3(contact_point);
                raytest.result.normal = self.transform.transform_vector3(normal).normalize();
                return true;
            }
        }
        false
    }

    fn cast_aabox(
        &self,
        boxtest: &mut crate::render_object_system::AABoxCollisionTestClass,
    ) -> bool {
        // C++ Reference: meshgeometry.cpp Cast_AABox implementation
        // Tests axis-aligned box movement against mesh triangles
        if let Some(model) = &self.model {
            // Transform box and movement vector to object space
            let inv_transform = self.transform.inverse();
            let mut local_test = boxtest.clone();
            local_test.box_obj.center = inv_transform.transform_point3(boxtest.box_obj.center);
            local_test.move_vector = inv_transform.transform_vector3(boxtest.move_vector);

            // Cast in object space
            if model.cast_aabox(&mut local_test) {
                // Transform result back to world space
                boxtest.result = local_test.result;
                return true;
            }
        }
        false
    }

    fn cast_obbox(
        &self,
        boxtest: &mut crate::render_object_system::OBBoxCollisionTestClass,
    ) -> bool {
        // C++ meshgeometry.cpp Cast_OBBox — swept SAT, not start/end static probes.
        if let Some(model) = &self.model {
            let inv_transform = self.transform.inverse();
            let mut local_test = boxtest.transformed_by_matrix(inv_transform);
            if model.cast_obbox(&mut local_test) {
                boxtest.result = local_test.result;
                boxtest.collided_render_obj = Some(self as *const MeshClass as usize);
                return true;
            }
        }
        false
    }

    fn intersect_aabox(
        &self,
        boxtest: &crate::render_object_system::AABoxIntersectionTestClass,
    ) -> bool {
        // C++ Reference: meshgeometry.cpp Intersect_AABox implementation
        // Simple boolean test - does box intersect any triangle?
        if let Some(model) = &self.model {
            // Transform box to object space
            let inv_transform = self.transform.inverse();
            let mut local_test = boxtest.clone();
            local_test.box_obj.center = inv_transform.transform_point3(boxtest.box_obj.center);

            return model.intersect_aabox(&local_test);
        }
        false
    }

    fn intersect_obbox(
        &self,
        boxtest: &crate::render_object_system::OBBoxIntersectionTestClass,
    ) -> bool {
        // C++ Reference: meshgeometry.cpp Intersect_OBBox implementation
        // Tests if oriented bounding box intersects mesh
        if let Some(model) = &self.model {
            // Transform OBBox to object space for intersection test
            let inv_transform = self.transform.inverse();
            let mut local_test = boxtest.clone();
            local_test.box_obj.center = inv_transform.transform_point3(boxtest.box_obj.center);

            return model.intersect_obbox(&local_test);
        }
        false
    }

    fn get_obj_space_bounding_sphere(&self) -> crate::render_object_system::SphereClass {
        crate::render_object_system::SphereClass::new(
            self.bounding_sphere.center,
            self.bounding_sphere.radius,
        )
    }

    fn get_obj_space_bounding_box(&self) -> crate::render_object_system::AABoxClass {
        // C++ Reference: Simple type conversion helper
        // Returns the object-space bounding box (before transform is applied)
        // The bounding box is typically computed from mesh vertices at load time
        crate::render_object_system::AABoxClass {
            center: self.bounding_box.center,
            extent: self.bounding_box.extent,
        }
    }

    fn scale(&mut self, scale: f32) {
        self.transform = Mat4::from_scale(Vec3::new(scale, scale, scale)) * self.transform;
        self.clear_deformed_world_vertices();
        self.update_cached_bounding_volumes();
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        self.transform = Mat4::from_scale(Vec3::new(scalex, scaley, scalez)) * self.transform;
        self.clear_deformed_world_vertices();
        self.update_cached_bounding_volumes();
    }

    fn get_material_info(&self) -> Option<&crate::render_object_system::MaterialInfoClass> {
        let model = self.model.as_ref()?;
        Some(
            self.material_info_cache
                .get_or_init(|| MeshClass::build_material_info_from_model(model.as_ref())),
        )
    }

    fn set_animation_hidden(&mut self, hidden: bool) {
        MeshClass::set_animation_hidden(self, hidden);
    }

    fn get_sort_level(&self) -> i32 {
        self.sort_level as i32
    }

    fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level as u32;
    }

    fn create_decal(&mut self, generator: &mut crate::render_object_system::DecalGeneratorClass) {
        MeshClass::create_decal(self, generator);
    }

    fn delete_decal(&mut self, decal_id: u32) {
        MeshClass::delete_decal(self, decal_id);
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        MeshClass::set_transform(self, transform);
    }
}
