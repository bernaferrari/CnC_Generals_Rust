//! Sphere render object (C++ parity wrapper)

use glam::{Mat4, Vec3};
use std::sync::Arc;
use ww3d_core::WW3D;
use ww3d_renderer_3d::core::error::RendererResult;
use ww3d_renderer_3d::math_utilities::Vector4;
use ww3d_renderer_3d::pointgr::SphereObjClass;
use ww3d_renderer_3d::render_object_system::{
    AABoxClass, AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass,
    MaterialInfoClass, OBBoxCollisionTestClass, OBBoxIntersectionTestClass, RayCollisionTestClass,
    RenderInfoClass, RenderObjClass, RenderObjClassId, SpecialRenderInfoClass, SphereClass,
    StaticSortRenderObject,
};
use ww3d_renderer_3d::rendering::mesh_system::SORT_LEVEL_NONE;
use ww3d_renderer_3d::rendering::shader_core::{ShaderClass, ShaderPreset};

/// Helper function to transpose a 3x3 basis matrix represented as [Vec3; 3]
/// Basis vectors are stored as columns, so we transpose to get rows for transformation
fn transpose_basis(basis: &[Vec3; 3]) -> Mat4 {
    // The basis is stored as column vectors: [x_axis, y_axis, z_axis]
    // To transpose, we need to create a matrix where:
    // row 0 = [x_axis.x, y_axis.x, z_axis.x]
    // row 1 = [x_axis.y, y_axis.y, z_axis.y]
    // row 2 = [x_axis.z, y_axis.z, z_axis.z]
    use glam::Vec4;
    Mat4::from_cols(
        Vec4::new(basis[0].x, basis[1].x, basis[2].x, 0.0),
        Vec4::new(basis[0].y, basis[1].y, basis[2].y, 0.0),
        Vec4::new(basis[0].z, basis[1].z, basis[2].z, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

#[derive(Debug, Clone)]
pub struct SphereRenderObj {
    pub sphere: SphereObjClass,
    pub transform: Mat4,
    shader: ShaderClass,
    name: String,
    sort_level: i32,
}

impl SphereRenderObj {
    pub fn new(center: Vec3, radius: f32, color: Vector4) -> Self {
        let mut sphere = SphereObjClass::new(center, radius);
        sphere.set_color(color);
        let mut shader = ShaderClass::new();
        let preset = if color.w < 1.0 {
            ShaderPreset::Alpha
        } else {
            ShaderPreset::Opaque
        };
        shader.set_preset(preset);
        Self {
            sphere,
            transform: Mat4::IDENTITY,
            shader,
            name: "Sphere".to_string(),
            sort_level: 0,
        }
    }
}

impl RenderObjClass for SphereRenderObj {
    fn clone_obj(&self) -> Box<dyn RenderObjClass> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn class_id(&self) -> RenderObjClassId {
        RenderObjClassId::Sphere
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
    fn get_num_polys(&self) -> usize {
        24
    } // Typical sphere tessellation

    fn render(&self, rinfo: &RenderInfoClass) -> RendererResult<()> {
        let sort_enabled = WW3D::is_sorting_enabled() && WW3D::are_static_sort_lists_enabled();
        let sort_level = self.shader.guess_sort_level();
        if sort_enabled && sort_level != SORT_LEVEL_NONE {
            let sort_object = StaticSortRenderObject::from_arc(Arc::new(self.clone()));
            if WW3D::add_to_static_sort_list(sort_object, sort_level).is_ok() {
                return Ok(());
            }
        }
        // Immediate path delegates to underlying sphere render
        // Note: SphereClass::render() returns RendererResult, already compatible
        // C++ equivalent: SphereClass::Render (sphere.cpp) - renders via point group
        self.sphere.render(rinfo)?;
        Ok(())
    }

    fn special_render(&self, _rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        // Default implementation - no special rendering
        Ok(())
    }

    fn cast_ray(&self, raytest: &mut RayCollisionTestClass) -> bool {
        // Port of C++ ray-sphere intersection algorithm
        // Standard ray-sphere intersection test

        // Transform sphere center to world space
        let world_center = self.transform.transform_point3(self.sphere.center);
        let world_radius = self.sphere.radius;

        // Get ray direction and origin
        let ray_origin = raytest.line.start;
        let ray_dir = (raytest.line.end - raytest.line.start).normalize();
        let ray_length = (raytest.line.end - raytest.line.start).length();

        // Vector from ray origin to sphere center
        let oc = ray_origin - world_center;

        // Solve quadratic equation: t^2 + 2bt + c = 0
        let b = oc.dot(ray_dir);
        let c = oc.dot(oc) - world_radius * world_radius;
        let discriminant = b * b - c;

        // If discriminant < 0, no intersection
        if discriminant < 0.0 {
            return false;
        }

        // Calculate intersection point (closest one)
        let t = -b - discriminant.sqrt();

        // Check if intersection is within ray bounds and closer than current result
        if t >= 0.0 && t <= ray_length {
            let fraction = t / ray_length;
            if fraction < raytest.result.fraction {
                // Fill in raytest result
                raytest.result.fraction = fraction;
                raytest.result.surface_type = 0; // SURFACE_TYPE_DEFAULT
                raytest.result.contact_point = ray_origin + ray_dir * t;

                // Calculate normal at intersection point
                let normal = (raytest.result.contact_point - world_center).normalize();
                raytest.result.normal = normal;

                return true;
            }
        }

        false
    }

    fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool {
        // Sphere-AABB collision test
        // Find closest point on AABB to sphere center, check if within radius

        let world_center = self.transform.transform_point3(self.sphere.center);
        let world_radius = self.sphere.radius;

        // Find closest point on AABB to sphere center
        let box_min = boxtest.box_obj.center - boxtest.box_obj.extent;
        let box_max = boxtest.box_obj.center + boxtest.box_obj.extent;

        let closest = Vec3::new(
            world_center.x.clamp(box_min.x, box_max.x),
            world_center.y.clamp(box_min.y, box_max.y),
            world_center.z.clamp(box_min.z, box_max.z),
        );

        // Check if closest point is within sphere radius
        let dist_sq = (closest - world_center).length_squared();
        dist_sq <= world_radius * world_radius
    }

    fn cast_obbox(&self, boxtest: &mut OBBoxCollisionTestClass) -> bool {
        // C++ Reference: CollisionMath::Intersection_Test (colmathsphere.cpp:97-111)
        // Transform sphere center into box's coordinate system
        let world_center = self.transform.transform_point3(self.sphere.center);
        let world_radius = self.sphere.radius;

        // Transform world center into box-relative coordinates
        let basis_transposed = transpose_basis(&boxtest.box_obj.basis);
        let box_rel_center =
            basis_transposed.transform_point3(world_center - boxtest.box_obj.center);

        // Check if sphere center is outside box extents (using box-local coordinates)
        // If center is outside box + radius in any axis, no collision possible
        if boxtest.box_obj.extent.x + world_radius < box_rel_center.x.abs() {
            return false;
        }
        if boxtest.box_obj.extent.y + world_radius < box_rel_center.y.abs() {
            return false;
        }
        if boxtest.box_obj.extent.z + world_radius < box_rel_center.z.abs() {
            return false;
        }

        // Conservative check: sphere intersects OBB if sphere center is within
        // box extents expanded by sphere radius. C++ uses same approach (colmathobb.cpp).
        true
    }

    fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool {
        // Same as cast_aabox for static intersection test
        let world_center = self.transform.transform_point3(self.sphere.center);
        let world_radius = self.sphere.radius;

        // Find closest point on AABB to sphere center
        let box_min = boxtest.box_obj.center - boxtest.box_obj.extent;
        let box_max = boxtest.box_obj.center + boxtest.box_obj.extent;

        let closest = Vec3::new(
            world_center.x.clamp(box_min.x, box_max.x),
            world_center.y.clamp(box_min.y, box_max.y),
            world_center.z.clamp(box_min.z, box_max.z),
        );

        // Check if closest point is within sphere radius
        let dist_sq = (closest - world_center).length_squared();
        dist_sq <= world_radius * world_radius
    }

    fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool {
        // C++ Reference: CollisionMath::Intersection_Test (colmathsphere.cpp:97-111)
        // Transform sphere center into box's coordinate system
        let world_center = self.transform.transform_point3(self.sphere.center);
        let world_radius = self.sphere.radius;

        // Transform world center into box-relative coordinates
        let basis_transposed = transpose_basis(&boxtest.box_obj.basis);
        let box_rel_center =
            basis_transposed.transform_point3(world_center - boxtest.box_obj.center);

        // Check if sphere center is outside box extents (using box-local coordinates)
        // If center is outside box + radius in any axis, no collision possible
        if boxtest.box_obj.extent.x + world_radius < box_rel_center.x.abs() {
            return false;
        }
        if boxtest.box_obj.extent.y + world_radius < box_rel_center.y.abs() {
            return false;
        }
        if boxtest.box_obj.extent.z + world_radius < box_rel_center.z.abs() {
            return false;
        }

        // Conservative check: sphere intersects OBB if sphere center is within
        // box extents expanded by sphere radius. C++ uses same approach (colmathobb.cpp).
        true
    }

    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        SphereClass::new(Vec3::ZERO, self.sphere.radius)
    }

    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let r = self.sphere.radius;
        AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::splat(r))
    }

    fn scale(&mut self, scale: f32) {
        self.sphere.radius *= scale;
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        // For sphere, use average scale
        let avg_scale = (scalex + scaley + scalez) / 3.0;
        self.sphere.radius *= avg_scale;
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        // Spheres don't have complex materials
        None
    }

    fn get_sort_level(&self) -> i32 {
        self.sort_level
    }

    fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level;
    }

    fn create_decal(&mut self, _generator: &mut DecalGeneratorClass) {
        // Spheres don't support decals
    }

    fn delete_decal(&mut self, _decal_id: u32) {
        // Spheres don't support decals
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }
}
