//! Thin wrapper that exposes a `ww3d_renderer_3d::MeshClass` through the effects crate.

use std::sync::Arc;

use glam::Mat4;

use ww3d_renderer_3d::render_object_system::{
    AABoxClass, AABoxCollisionTestClass, AABoxIntersectionTestClass, DecalGeneratorClass,
    MaterialInfoClass, OBBoxCollisionTestClass, OBBoxIntersectionTestClass, RayCollisionTestClass,
    RenderInfoClass, RenderObjClass, RenderObjClassId, SpecialRenderInfoClass, SphereClass,
};
use ww3d_renderer_3d::{core::error::RendererResult, MeshClass};

/// Higher level handle that lets systems in the effects crate work with the renderer mesh type.
#[derive(Debug, Clone)]
pub struct MeshRenderObj {
    mesh: Arc<MeshClass>,
}

impl MeshRenderObj {
    /// Construct from a shared mesh instance.
    pub fn new(mesh: Arc<MeshClass>) -> Self {
        Self { mesh }
    }

    /// Access the underlying mesh.
    pub fn mesh(&self) -> &Arc<MeshClass> {
        &self.mesh
    }
}

impl RenderObjClass for MeshRenderObj {
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
        RenderObjClassId::Mesh
    }

    fn get_name(&self) -> &str {
        RenderObjClass::get_name(&*self.mesh)
    }

    fn set_name(&mut self, name: &str) {
        RenderObjClass::set_name(Arc::make_mut(&mut self.mesh), name);
    }

    fn get_num_polys(&self) -> usize {
        RenderObjClass::get_num_polys(&*self.mesh)
    }

    fn render(&self, rinfo: &RenderInfoClass) -> RendererResult<()> {
        RenderObjClass::render(&*self.mesh, rinfo)
    }

    fn special_render(&self, rinfo: &SpecialRenderInfoClass) -> RendererResult<()> {
        RenderObjClass::special_render(&*self.mesh, rinfo)
    }

    fn cast_ray(&self, raytest: &mut RayCollisionTestClass) -> bool {
        self.mesh.cast_ray(raytest)
    }

    fn cast_aabox(&self, boxtest: &mut AABoxCollisionTestClass) -> bool {
        self.mesh.cast_aabox(boxtest)
    }

    fn cast_obbox(&self, boxtest: &mut OBBoxCollisionTestClass) -> bool {
        self.mesh.cast_obbox(boxtest)
    }

    fn intersect_aabox(&self, boxtest: &AABoxIntersectionTestClass) -> bool {
        self.mesh.intersect_aabox(boxtest)
    }

    fn intersect_obbox(&self, boxtest: &OBBoxIntersectionTestClass) -> bool {
        self.mesh.intersect_obbox(boxtest)
    }

    fn get_obj_space_bounding_sphere(&self) -> SphereClass {
        RenderObjClass::get_obj_space_bounding_sphere(&*self.mesh)
    }

    fn get_obj_space_bounding_box(&self) -> AABoxClass {
        RenderObjClass::get_obj_space_bounding_box(&*self.mesh)
    }

    fn scale(&mut self, scale: f32) {
        RenderObjClass::scale(Arc::make_mut(&mut self.mesh), scale);
    }

    fn scale_xyz(&mut self, scalex: f32, scaley: f32, scalez: f32) {
        RenderObjClass::scale_xyz(Arc::make_mut(&mut self.mesh), scalex, scaley, scalez);
    }

    fn get_material_info(&self) -> Option<&MaterialInfoClass> {
        RenderObjClass::get_material_info(&*self.mesh)
    }

    fn get_sort_level(&self) -> i32 {
        RenderObjClass::get_sort_level(&*self.mesh)
    }

    fn set_sort_level(&mut self, level: i32) {
        RenderObjClass::set_sort_level(Arc::make_mut(&mut self.mesh), level);
    }

    fn set_animation_hidden(&mut self, hidden: bool) {
        Arc::make_mut(&mut self.mesh).set_animation_hidden(hidden);
    }

    fn create_decal(&mut self, generator: &mut DecalGeneratorClass) {
        Arc::make_mut(&mut self.mesh).create_decal(generator);
    }

    fn delete_decal(&mut self, decal_id: u32) {
        Arc::make_mut(&mut self.mesh).delete_decal(decal_id);
    }

    fn transform(&self) -> &Mat4 {
        RenderObjClass::transform(&*self.mesh)
    }

    fn set_transform(&mut self, transform: Mat4) {
        RenderObjClass::set_transform(Arc::make_mut(&mut self.mesh), transform);
    }
}
