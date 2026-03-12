// Mesh Rendering
// Ported from mesh.h and mesh.cpp

use crate::math::*;
use crate::render_object::*;
use crate::mesh_model::MeshModel;
use crate::material::MaterialInfo;
use crate::{Result, W3DError};
use std::sync::Arc;

// Mesh class - instance of a mesh model
pub struct Mesh {
    pub base: RenderObjectBase,
    pub model: Arc<MeshModel>,
    pub alpha_override: f32,
    pub base_vertex_offset: usize,
}

impl Mesh {
    pub fn new(name: String, model: Arc<MeshModel>) -> Self {
        Self {
            base: RenderObjectBase::new(name),
            model,
            alpha_override: 1.0,
            base_vertex_offset: 0,
        }
    }

    pub fn get_model(&self) -> &Arc<MeshModel> {
        &self.model
    }

    pub fn set_alpha_override(&mut self, alpha: f32) {
        self.alpha_override = alpha;
    }

    pub fn get_alpha_override(&self) -> f32 {
        self.alpha_override
    }

    fn update_cached_bounding_volumes(&mut self) {
        if self.base.are_bounding_volumes_valid() {
            return;
        }

        // Get object space bounds
        let obj_space_box = self.model.geometry.get_bounding_box(&mut self.model.geometry.clone());
        let obj_space_sphere = self.model.geometry.get_bounding_sphere(&mut self.model.geometry.clone());

        // Transform to world space
        self.base.cached_bounding_box = obj_space_box.transform(&self.base.transform);
        self.base.cached_bounding_sphere = obj_space_sphere.transform(&self.base.transform);

        self.base.validate_bounding_volumes();
    }
}

impl RenderObject for Mesh {
    fn class_id(&self) -> RenderObjectClassId {
        RenderObjectClassId::Mesh
    }

    fn get_name(&self) -> &str {
        &self.base.name
    }

    fn set_name(&mut self, name: String) {
        self.base.name = name;
    }

    fn get_num_polys(&self) -> u32 {
        self.model.geometry.get_polygon_count() as u32
    }

    fn render(&mut self, rinfo: &RenderInfo) -> Result<()> {
        if !self.is_visible() {
            return Ok(());
        }

        // Render the mesh using the model's material passes
        for (pass_idx, pass) in self.model.material_info.passes.iter().enumerate() {
            // Set up material state for this pass
            // This would interact with the wgpu renderer

            // For now, just a placeholder
            // In a full implementation, this would:
            // 1. Bind vertex buffers
            // 2. Bind index buffers
            // 3. Set up shaders and materials
            // 4. Issue draw calls
        }

        Ok(())
    }

    fn get_transform(&self) -> &Mat4 {
        &self.base.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.base.transform = transform;
        self.base.invalidate_bounding_volumes();
    }

    fn set_position(&mut self, position: Vec3) {
        self.base.transform[(0, 3)] = position.x;
        self.base.transform[(1, 3)] = position.y;
        self.base.transform[(2, 3)] = position.z;
        self.base.invalidate_bounding_volumes();
    }

    fn get_bounding_sphere(&self) -> Sphere {
        let mut mesh = self.clone_for_bounds();
        mesh.update_cached_bounding_volumes();
        mesh.base.cached_bounding_sphere
    }

    fn get_bounding_box(&self) -> AABox {
        let mut mesh = self.clone_for_bounds();
        mesh.update_cached_bounding_volumes();
        mesh.base.cached_bounding_box
    }

    fn get_obj_space_bounding_sphere(&self) -> Sphere {
        self.model.geometry.get_bounding_sphere(&mut self.model.geometry.clone())
    }

    fn get_obj_space_bounding_box(&self) -> AABox {
        self.model.geometry.get_bounding_box(&mut self.model.geometry.clone())
    }

    fn update_obj_space_bounding_volumes(&mut self) {
        // Object space bounds are stored in the model, not the instance
    }

    fn get_flags(&self) -> RenderObjectFlags {
        self.base.flags
    }

    fn set_flags(&mut self, flags: RenderObjectFlags) {
        self.base.flags = flags;
    }

    fn get_material_info(&self) -> Option<&MaterialInfo> {
        Some(&self.model.material_info)
    }

    fn scale(&mut self, scale: f32) {
        if scale == 1.0 {
            return;
        }

        // Scale the transform
        let mut scale_mat = Mat4::identity();
        scale_mat[(0, 0)] = scale;
        scale_mat[(1, 1)] = scale;
        scale_mat[(2, 2)] = scale;

        self.base.transform = self.base.transform * scale_mat;
        self.base.object_scale *= scale;
        self.base.invalidate_bounding_volumes();
    }

    fn cast_ray(&self, ray: &Ray) -> Option<f32> {
        // Transform ray to object space
        let inv_transform = self.base.transform.try_inverse()?;
        let obj_ray = Ray {
            origin: inverse_transform_point(&inv_transform, &ray.origin),
            direction: inverse_transform_vector(&inv_transform, &ray.direction).normalize(),
        };

        // Test against geometry
        let mut min_t = f32::MAX;
        let polygons = self.model.geometry.get_polygon_array();
        let vertices = self.model.geometry.get_vertex_array();

        for tri in polygons {
            let v0 = &vertices[tri.x as usize];
            let v1 = &vertices[tri.y as usize];
            let v2 = &vertices[tri.z as usize];

            if let Some(t) = self.model.geometry.ray_triangle_intersection(&obj_ray, v0, v1, v2) {
                if t < min_t {
                    min_t = t;
                }
            }
        }

        if min_t < f32::MAX {
            Some(min_t)
        } else {
            None
        }
    }
}

impl Mesh {
    fn clone_for_bounds(&self) -> Self {
        Self {
            base: RenderObjectBase {
                name: self.base.name.clone(),
                transform: self.base.transform,
                flags: self.base.flags,
                cached_bounding_sphere: self.base.cached_bounding_sphere,
                cached_bounding_box: self.base.cached_bounding_box,
                native_screen_size: self.base.native_screen_size,
                object_scale: self.base.object_scale,
            },
            model: Arc::clone(&self.model),
            alpha_override: self.alpha_override,
            base_vertex_offset: self.base_vertex_offset,
        }
    }
}
