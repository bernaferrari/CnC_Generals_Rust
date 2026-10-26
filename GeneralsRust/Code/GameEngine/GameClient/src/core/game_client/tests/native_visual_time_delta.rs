//! Test-only receiver: existing Drawable behavior plus owned update-delta witness.
use crate::drawable::drawable::*;
use std::any::Any;
#[derive(Debug)]
pub(super) struct DeltaDrawable {
    inner: BasicDrawable,
    pub(super) deltas: Vec<f32>,
}
impl DeltaDrawable {
    pub(super) fn new(id: DrawableId) -> Self {
        Self {
            inner: BasicDrawable::new(id),
            deltas: Vec::new(),
        }
    }
}
impl DrawableDowncast for DeltaDrawable {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
impl Drawable for DeltaDrawable {
    fn get_id(&self) -> DrawableId {
        self.inner.get_id()
    }
    fn get_position(&self) -> Vector3 {
        self.inner.get_position()
    }
    fn set_position(&mut self, position: Vector3) {
        self.inner.set_position(position)
    }
    fn get_transform(&self) -> Matrix4 {
        self.inner.get_transform()
    }
    fn set_base_transform(&mut self, transform: Matrix4) {
        self.inner.set_base_transform(transform)
    }
    fn get_instance_transform(&self) -> Matrix4 {
        self.inner.get_instance_transform()
    }
    fn set_instance_transform(&mut self, transform: Matrix4) {
        self.inner.set_instance_transform(transform)
    }
    fn is_instance_identity(&self) -> bool {
        self.inner.is_instance_identity()
    }
    fn get_instance_scale(&self) -> f32 {
        self.inner.get_instance_scale()
    }
    fn set_instance_scale(&mut self, scale: f32) {
        self.inner.set_instance_scale(scale)
    }
    fn get_status(&self) -> DrawableStatus {
        self.inner.get_status()
    }
    fn set_status(&mut self, status: DrawableStatus) {
        self.inner.set_status(status)
    }
    fn is_visible(&self) -> bool {
        self.inner.is_visible()
    }
    fn set_visible(&mut self, visible: bool) {
        self.inner.set_visible(visible)
    }
    fn is_selected(&self) -> bool {
        self.inner.is_selected()
    }
    fn set_selected(&mut self, selected: bool) {
        self.inner.set_selected(selected)
    }
    fn get_opacity(&self) -> f32 {
        self.inner.get_opacity()
    }
    fn set_opacity(&mut self, opacity: f32) {
        self.inner.set_opacity(opacity)
    }
    fn get_stealth_look(&self) -> StealthLook {
        self.inner.get_stealth_look()
    }
    fn set_stealth_look(&mut self, stealth_look: StealthLook) {
        self.inner.set_stealth_look(stealth_look)
    }
    fn get_tint_color(&self) -> Vector3 {
        self.inner.get_tint_color()
    }
    fn set_tint_color(&mut self, color: Vector3) {
        self.inner.set_tint_color(color)
    }
    fn flash_color(&mut self, color: Vector3, duration_frames: u32) {
        self.inner.flash_color(color, duration_frames)
    }
    fn update(&mut self, delta_time: f32, logic_frame: u32) {
        self.deltas.push(delta_time);
        self.inner.update(delta_time, logic_frame)
    }
    fn render(&mut self, view_matrix: &Matrix4, projection_matrix: &Matrix4) {
        self.inner.render(view_matrix, projection_matrix)
    }
    fn get_bounding_sphere(&self) -> (Vector3, f32) {
        self.inner.get_bounding_sphere()
    }
    fn receives_dynamic_lights(&self) -> bool {
        self.inner.receives_dynamic_lights()
    }
    fn set_receives_dynamic_lights(&mut self, receives: bool) {
        self.inner.set_receives_dynamic_lights(receives)
    }
    fn get_terrain_decal_type(&self) -> TerrainDecalType {
        self.inner.get_terrain_decal_type()
    }
    fn set_terrain_decal_type(&mut self, decal_type: TerrainDecalType) {
        self.inner.set_terrain_decal_type(decal_type)
    }
}
