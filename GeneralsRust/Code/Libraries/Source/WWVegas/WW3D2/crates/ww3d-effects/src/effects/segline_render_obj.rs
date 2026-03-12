//! SegLine render object (C++ parity with SegmentedLineClass)

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use ww3d_core::WW3D;
use ww3d_renderer_3d::core::error::RendererResult;
use ww3d_renderer_3d::math_utilities::Vector4;
use ww3d_renderer_3d::render_object_system::{
    AABoxClass, AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass,
    MaterialInfoClass, OBBoxCollisionTestClass, OBBoxIntersectionTestClass, RayCollisionTestClass,
    RenderInfoClass, RenderObjClass, RenderObjClassId, SpecialRenderInfoClass, SphereClass,
    StaticSortRenderObject,
};
use ww3d_renderer_3d::rendering::mesh_system::{MeshClass, SORT_LEVEL_NONE};
use ww3d_renderer_3d::rendering::shader_system::shader::ShaderClass;
use ww3d_renderer_3d::seglinerenderer::SegLineRenderer;
use ww3d_renderer_3d::Renderer;

/// Helper function to transpose a 3x3 basis matrix represented as [Vec3; 3]
/// Basis vectors are stored as columns, so we transpose to get rows for transformation
fn transpose_basis(basis: &[Vec3; 3]) -> Mat4 {
    // The basis is stored as column vectors: [x_axis, y_axis, z_axis]
    // To transpose, we need to create a matrix where:
    // row 0 = [x_axis.x, y_axis.x, z_axis.x]
    // row 1 = [x_axis.y, y_axis.y, z_axis.y]
    // row 2 = [x_axis.z, y_axis.z, z_axis.z]
    Mat4::from_cols(
        Vec4::new(basis[0].x, basis[1].x, basis[2].x, 0.0),
        Vec4::new(basis[0].y, basis[1].y, basis[2].y, 0.0),
        Vec4::new(basis[0].z, basis[1].z, basis[2].z, 0.0),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
    )
}

#[derive(Debug)]
pub struct SegLineRenderObj {
    pub a: Vec3,
    pub b: Vec3,
    pub disable_sorting: bool,
    pub transform: Mat4,
    renderer: SegLineRenderer,
    name: String,
    sort_level: i32,
}

impl SegLineRenderObj {
    /// Helper function for line-box intersection using slab method
    /// C++ Reference: Test_Aligned_Box (colmathline.cpp:379-480)
    fn test_aligned_box(&self, p0: &Vec3, dp: &Vec3, box_min: &Vec3, box_max: &Vec3) -> bool {
        const BOX_SIDE_NEGATIVE: i32 = 0;
        const BOX_SIDE_POSITIVE: i32 = 1;
        const BOX_SIDE_MIDDLE: i32 = 2;

        let mut quadrant = [0i32; 3];
        let mut candidate_plane = [0.0f32; 3];
        let mut maxt = [0.0f32; 3];
        let mut inside = true;

        // Determine which side of the box the ray origin is on for each axis
        for i in 0..3 {
            if p0[i] < box_min[i] {
                quadrant[i] = BOX_SIDE_NEGATIVE;
                candidate_plane[i] = box_min[i];
                inside = false;
            } else if p0[i] > box_max[i] {
                quadrant[i] = BOX_SIDE_POSITIVE;
                candidate_plane[i] = box_max[i];
                inside = false;
            } else {
                quadrant[i] = BOX_SIDE_MIDDLE;
            }
        }

        // Ray starts inside box
        if inside {
            return true;
        }

        // Calculate distances to candidate planes
        for i in 0..3 {
            if quadrant[i] != BOX_SIDE_MIDDLE && dp[i] != 0.0 {
                maxt[i] = (candidate_plane[i] - p0[i]) / dp[i];
            } else {
                maxt[i] = -1.0;
            }
        }

        // Get largest t (furthest intersection)
        let mut intersection_plane = 0;
        for i in 1..3 {
            if maxt[i] > maxt[intersection_plane] {
                intersection_plane = i;
            }
        }

        // Ray is in front of all planes
        if maxt[intersection_plane] < 0.0 || maxt[intersection_plane] > 1.0 {
            return false;
        }

        // Check if intersection point is inside box on other two axes
        for i in 0..3 {
            if intersection_plane != i {
                let coord = p0[i] + maxt[intersection_plane] * dp[i];
                if coord < box_min[i] || coord > box_max[i] {
                    return false;
                }
            }
        }

        true
    }

    pub fn new(a: Vec3, b: Vec3, color: Vector4, width: f32) -> Self {
        let mut renderer = SegLineRenderer::new();
        renderer.set_color(color);
        renderer.set_width(width);
        let shader = if color.w < 1.0 {
            ShaderClass::get_alpha_shader()
        } else {
            ShaderClass::get_opaque_shader()
        };
        renderer.set_shader(shader);
        Self {
            a,
            b,
            disable_sorting: false,
            transform: Mat4::IDENTITY,
            renderer,
            name: "SegLine".to_string(),
            sort_level: 0,
        }
    }

    pub fn set_disable_sorting(&mut self, onoff: bool) {
        self.disable_sorting = onoff;
    }
}

impl RenderObjClass for SegLineRenderObj {
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
        RenderObjClassId::SegLine
    }
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }
    fn get_num_polys(&self) -> usize {
        2
    } // Line segment has 2 triangles

    fn render(&self, rinfo: &RenderInfoClass) -> RendererResult<()> {
        // Sorting policy matches C++ SegmentedLineClass
        let sort_enabled = WW3D::is_sorting_enabled()
            && WW3D::are_static_sort_lists_enabled()
            && !self.disable_sorting;
        let sort_hint = self.renderer.sort_hint();
        let sort_level = if self.sort_level >= 0 {
            self.sort_level as u32
        } else {
            sort_hint
        };
        if sort_enabled && sort_level != SORT_LEVEL_NONE {
            let sort_object = StaticSortRenderObject::from_arc(Arc::new(self.clone()));
            if WW3D::add_to_static_sort_list(sort_object, sort_level).is_ok() {
                return Ok(());
            }
        }

        if let Some(mesh) = self.build_render_mesh(rinfo) {
            Renderer::with_global_mut(|renderer| {
                renderer.queue_mesh(mesh.clone())?;
                Ok(())
            })?;
        }
        Ok(())
    }

    fn special_render(&self, _rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        // Default implementation - no special rendering
        Ok(())
    }

    fn cast_ray(&self, raytest: &mut RayCollisionTestClass) -> bool {
        // Port of C++ SegmentedLineClass::Cast_Ray (segline.cpp:540-589)
        // Check each line segment of this segmented line against the ray

        // For now, we only have a single segment from a to b
        // Transform points to world space
        let curr_start = self.transform.transform_point3(self.a);
        let curr_end = self.transform.transform_point3(self.b);

        // Create line segment for this section
        let line_seg_dir = curr_end - curr_start;
        let line_seg_length = line_seg_dir.length();

        if line_seg_length < 1e-6 {
            return false; // Degenerate segment
        }

        // Get ray as line segment
        let ray_dir = raytest.line.end - raytest.line.start;
        let ray_length = ray_dir.length();

        if ray_length < 1e-6 {
            return false; // Degenerate ray
        }

        // Find closest points between ray and line segment using C++ algorithm
        // Based on LineSegClass::Find_Intersection (lineseg.cpp:166-230)
        let cross1 = line_seg_dir.cross(ray_dir);
        let cross1_len_sq = cross1.length_squared();

        // If lines are parallel, cross product will be near zero
        if cross1_len_sq < 1e-6 {
            return false;
        }

        let cross2 = (raytest.line.start - curr_start).cross(ray_dir);
        let top1 = cross2.dot(cross1);
        let bottom1 = cross1_len_sq;

        let length1 = top1 / bottom1;
        let _fraction1 = length1 / line_seg_length;

        // Calculate closest point on line segment
        let p0 = curr_start + line_seg_dir * (length1 / line_seg_length);

        // Calculate corresponding point on ray
        let cross3 = ray_dir.cross(line_seg_dir);
        let cross4 = (curr_start - raytest.line.start).cross(line_seg_dir);
        let top2 = cross4.dot(cross3);
        let bottom2 = cross3.length_squared();

        if bottom2 < 1e-6 {
            return false;
        }

        let length2 = top2 / bottom2;
        let fraction2 = length2 / ray_length;

        let p1 = raytest.line.start + ray_dir * (length2 / ray_length);

        // Check if ray was close enough to line to be considered intersecting
        let dist = (p0 - p1).length();
        let width = self.renderer.width();

        if dist <= width && fraction2 >= 0.0 && fraction2 < raytest.result.fraction {
            // Fill in raytest result
            raytest.result.fraction = fraction2;
            raytest.result.surface_type = 0; // SURFACE_TYPE_DEFAULT
                                             // Contact point is the point on the line segment
            raytest.result.contact_point = p0;
            return true;
        }

        false
    }

    fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool {
        // C++ Reference: CollisionMath::Collide (colmathline.cpp:279-311)
        // Transform line to world space
        let world_a = self.transform.transform_point3(self.a);
        let world_b = self.transform.transform_point3(self.b);
        let dp = world_b - world_a;

        let box_min = boxtest.box_obj.center - boxtest.box_obj.extent;
        let box_max = boxtest.box_obj.center + boxtest.box_obj.extent;

        // Use slab method for ray-box intersection
        self.test_aligned_box(&world_a, &dp, &box_min, &box_max)
    }

    fn cast_obbox(&self, boxtest: &mut OBBoxCollisionTestClass) -> bool {
        // C++ Reference: CollisionMath::Collide (colmathline.cpp:313-363)
        // Transform line to world space
        let world_a = self.transform.transform_point3(self.a);
        let world_b = self.transform.transform_point3(self.b);

        // Transform into box's local coordinate system
        let basis_transposed = transpose_basis(&boxtest.box_obj.basis);
        let box_rel_a = basis_transposed.transform_point3(world_a - boxtest.box_obj.center);
        let box_rel_b = basis_transposed.transform_point3(world_b - boxtest.box_obj.center);
        let dp = box_rel_b - box_rel_a;

        let box_min = -boxtest.box_obj.extent;
        let box_max = boxtest.box_obj.extent;

        // Test in box-local space (now aligned)
        self.test_aligned_box(&box_rel_a, &dp, &box_min, &box_max)
    }

    fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool {
        // For static intersection, same as collision test
        let world_a = self.transform.transform_point3(self.a);
        let world_b = self.transform.transform_point3(self.b);
        let dp = world_b - world_a;

        let box_min = boxtest.box_obj.center - boxtest.box_obj.extent;
        let box_max = boxtest.box_obj.center + boxtest.box_obj.extent;

        self.test_aligned_box(&world_a, &dp, &box_min, &box_max)
    }

    fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool {
        // For static intersection, same as collision test
        let world_a = self.transform.transform_point3(self.a);
        let world_b = self.transform.transform_point3(self.b);

        // Transform into box's local coordinate system
        let basis_transposed = transpose_basis(&boxtest.box_obj.basis);
        let box_rel_a = basis_transposed.transform_point3(world_a - boxtest.box_obj.center);
        let box_rel_b = basis_transposed.transform_point3(world_b - boxtest.box_obj.center);
        let dp = box_rel_b - box_rel_a;

        let box_min = -boxtest.box_obj.extent;
        let box_max = boxtest.box_obj.extent;

        self.test_aligned_box(&box_rel_a, &dp, &box_min, &box_max)
    }

    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        let center = (self.a + self.b) * 0.5;
        let radius = self.a.distance(self.b) * 0.5 + self.renderer.width() * 0.5;
        SphereClass::new(center, radius)
    }

    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        let half = self.renderer.width() * 0.5;
        let min = self.a.min(self.b) - Vec3::splat(half);
        let max = self.a.max(self.b) + Vec3::splat(half);
        let center = (min + max) * 0.5;
        let extent = (max - min) * 0.5;
        AABoxClass::from_center_and_extent(center, extent)
    }

    fn scale(&mut self, scale: f32) {
        self.a *= scale;
        self.b *= scale;
        self.renderer.set_width(self.renderer.width() * scale.abs());
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        self.a.x *= scalex;
        self.a.y *= scaley;
        self.a.z *= scalez;
        self.b.x *= scalex;
        self.b.y *= scaley;
        self.b.z *= scalez;
        // Scale width by average scale
        let avg_scale = (scalex + scaley + scalez) / 3.0;
        self.renderer
            .set_width(self.renderer.width() * avg_scale.abs());
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        // Lines don't have complex materials
        None
    }

    fn get_sort_level(&self) -> i32 {
        self.sort_level
    }

    fn set_sort_level(&mut self, level: i32) {
        self.sort_level = level;
    }

    fn create_decal(&mut self, _generator: &mut DecalGeneratorClass) {
        // Lines don't support decals
    }

    fn delete_decal(&mut self, _decal_id: u32) {
        // Lines don't support decals
    }

    fn transform(&self) -> &Mat4 {
        &self.transform
    }

    fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }
}

impl Clone for SegLineRenderObj {
    fn clone(&self) -> Self {
        Self {
            a: self.a,
            b: self.b,
            disable_sorting: self.disable_sorting,
            transform: self.transform,
            renderer: self.renderer.clone(),
            name: self.name.clone(),
            sort_level: self.sort_level,
        }
    }
}

impl SegLineRenderObj {
    fn build_render_mesh(&self, rinfo: &RenderInfoClass) -> Option<Arc<MeshClass>> {
        let camera_pos = rinfo.camera.get_position();
        let mesh = self.renderer.generate_mesh(
            self.a,
            self.b,
            self.transform,
            camera_pos,
            rinfo.time,
            &self.name,
        )?;

        if self.sort_level >= 0 {
            let mut owned = (*mesh).clone();
            owned.sort_level = self.sort_level as u32;
            owned.update_cached_bounding_volumes();
            return Some(Arc::new(owned));
        }

        Some(mesh)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use std::sync::Arc;
    use ww3d_renderer_3d::math_utilities::Vector4;
    use ww3d_renderer_3d::render_object_system::RenderInfoClass;
    use ww3d_renderer_3d::rendering::camera_system::camera::CameraClass;

    fn assert_vec3_approx(actual: Vec3, expected: Vec3) {
        let delta = (actual - expected).abs();
        let max_component = delta.max_element();
        assert!(
            max_component <= 1e-4,
            "vector mismatch: actual={actual:?}, expected={expected:?}, delta={delta:?}"
        );
    }

    #[test]
    fn build_render_mesh_produces_expected_quad() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 0.0, 5.0);
        let color = Vector4::new(1.0, 0.5, 0.25, 1.0);
        let width = 2.0;

        let obj = SegLineRenderObj::new(a, b, color, width);
        let camera = Arc::new(CameraClass::new());
        let mut render_info = RenderInfoClass::new(camera);
        render_info.time = 0.0;

        let mesh = obj
            .build_render_mesh(&render_info)
            .expect("segmented line should emit a mesh");
        let model = mesh.model.as_ref().expect("mesh model should be present");

        assert_eq!(
            model.vertex_count, 4,
            "segmented line should render a single quad"
        );
        assert_eq!(
            model.index_count, 6,
            "segmented line quad uses two triangles"
        );

        let bbox = mesh.bounding_box;
        assert_vec3_approx(bbox.center, Vec3::new(0.0, 0.0, 2.5));
        assert_vec3_approx(bbox.extent, Vec3::new(1.0, 0.0, 2.5));

        let sphere = mesh.bounding_sphere;
        assert_vec3_approx(sphere.center, Vec3::new(0.0, 0.0, 2.5));
        let expected_radius = Vec3::new(2.0, 0.0, 5.0).length() * 0.5;
        assert!(
            (sphere.radius - expected_radius).abs() <= 1e-4,
            "sphere radius mismatch: actual={}, expected={expected_radius}",
            sphere.radius
        );
    }
}
