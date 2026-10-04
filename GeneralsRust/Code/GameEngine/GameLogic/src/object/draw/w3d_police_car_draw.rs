use super::draw_module::*;
use super::w3d_truck_draw::*;
use crate::common::*;
use crate::helpers::{
    TheGameClient, TheGameLogic, create_scene_point_light, fade_scene_point_light,
    game_client_random_value_real, update_scene_point_light,
};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct W3DPoliceCarDrawModuleData {
    pub base: W3DTruckDrawModuleData,
}
impl W3DPoliceCarDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DTruckDrawModuleData::new(),
        }
    }
    pub fn parse_from_ini(
        &mut self,
        ini: &mut game_engine::common::ini::INI,
    ) -> Result<(), game_engine::common::ini::INIError> {
        self.base.parse_from_ini(ini)
    }
}
impl ModuleData for W3DPoliceCarDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}
impl DrawModuleData for W3DPoliceCarDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DPoliceCarDrawModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

pub struct W3DPoliceCarDraw {
    data: W3DPoliceCarDrawModuleData,
    base: W3DTruckDraw,
    cur_frame: Real,
    light_id: Option<u64>,
}
impl W3DPoliceCarDraw {
    pub fn new(data: W3DPoliceCarDrawModuleData) -> Self {
        Self {
            base: W3DTruckDraw::new(data.base.clone()),
            data,
            cur_frame: game_client_random_value_real(0.0, 10.0),
            light_id: None,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }

    /// Live host flashing light. The scene-light id stays on this module.
    pub fn tick_live_host_light(&mut self, position: [f32; 3], hidden: bool) {
        if hidden {
            return;
        }
        self.tick_live_light(position);
    }

    /// Live host dust/dirt/powerslide on the embedded truck draw.
    pub fn tick_live_host_dust(
        &mut self,
        template_name: &str,
        physics: TruckDrawLivePhysics,
        hidden: bool,
    ) {
        self.base.tick_live_host(template_name, physics, hidden);
    }

    pub fn cur_frame(&self) -> Real {
        self.cur_frame
    }
    /// Leftover C++ `doDrawModule` light residual driven by live host pose.
    fn tick_live_light(&mut self, position: [f32; 3]) {
        let frames = self.base.anim_frame_count();
        if self.base.has_bound_animation() && frames > 1 {
            self.cur_frame += 0.25;
            if self.cur_frame > frames as Real - 1.0 {
                self.cur_frame = 0.0;
            }
        }
        let (red, green, blue) = police_light_color(self.cur_frame);
        if self.light_id.is_none() {
            self.light_id = Some(create_scene_point_light());
        }
        if let Some(light_id) = self.light_id {
            update_scene_point_light(
                light_id,
                [position[0], position[1], position[2] + 8.0],
                [red * 0.5, green * 0.5, blue * 0.5],
                [red, green, blue],
                3.0,
                20.0,
            );
        }
    }
}

impl Drop for W3DPoliceCarDraw {
    fn drop(&mut self) {
        if let Some(id) = self.light_id.take() {
            fade_scene_point_light(id, 5);
        }
    }
}
impl Module for W3DPoliceCarDraw {
    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
    }
    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.base.preload_assets(time_of_day);
    }
    fn on_delete(&mut self) {
        if let Some(id) = self.light_id.take() {
            fade_scene_point_light(id, 5);
        }
        self.base.on_delete();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DPoliceCarDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}
impl DrawModule for W3DPoliceCarDraw {
    fn on_drawable_bound_to_object_with_context(
        &mut self,
        binding: &DrawModuleBindingContext,
        drawable: &ModelDrawContext,
    ) -> bool {
        let handled = self
            .base
            .on_drawable_bound_to_object_with_context(binding, drawable);
        handled
    }

    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        // C++ returns before the light and the truck draw when the render object is null.
        if !self.base.has_render_model() {
            return;
        }
        let frames = self.base.anim_frame_count();
        if self.base.has_bound_animation() && frames > 1 {
            self.cur_frame += 0.25;
            if self.cur_frame > frames as Real - 1.0 {
                self.cur_frame = 0.0;
            }
        }
        let (red, green, blue) = police_light_color(self.cur_frame);
        if self.light_id.is_none() {
            self.light_id = Some(create_scene_point_light());
        }
        if let Some(light_id) = self.light_id {
            let mut pos = Coord3D::origin();
            if let Some(owner_id) = self.base.owner_id() {
                let _ =
                    crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                        pos = owner_guard
                            .get_drawable()
                            .and_then(|drawable| {
                                drawable.read().ok().map(|guard| guard.get_position())
                            })
                            .unwrap_or_else(|| *owner_guard.get_position());
                    });
            }
            update_scene_point_light(
                light_id,
                [pos.x, pos.y, pos.z + 8.0],
                [red * 0.5, green * 0.5, blue * 0.5],
                [red, green, blue],
                3.0,
                20.0,
            );
        }
        self.base.do_draw_module(transform_mtx);
    }
    fn set_shadows_enabled(&mut self, enable: bool) {
        self.base.set_shadows_enabled(enable);
    }
    fn release_shadows(&mut self) {
        self.base.release_shadows();
    }
    fn allocate_shadows(&mut self) {
        self.base.allocate_shadows();
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.base.set_fully_obscured_by_shroud(fully_obscured);
    }
    fn set_hidden(&mut self, hidden: bool) {
        DrawModule::set_hidden(&mut self.base, hidden);
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn react_to_transform_change(
        &mut self,
        old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        old_angle: Real,
    ) {
        self.base
            .react_to_transform_change(old_mtx, old_pos, old_angle);
    }
    fn react_to_geometry_change(&mut self) {
        self.base.react_to_geometry_change();
    }
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        self.base.get_object_draw_interface()
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        self.base.get_object_draw_interface_mut()
    }
}
impl Snapshotable for W3DPoliceCarDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

fn police_light_color(cur_frame: Real) -> (Real, Real, Real) {
    let mut red = 0.0;
    let mut green = 0.0;
    let mut blue = 0.0;
    if cur_frame < 3.0 {
        red = 1.0;
        green = 0.5;
    } else if cur_frame < 6.0 {
        red = 1.0;
    } else if cur_frame < 7.0 {
        red = 1.0;
        green = 0.5;
    } else if cur_frame < 9.0 {
        red = 0.5 + (9.0 - cur_frame) / 4.0;
        blue = (cur_frame - 5.0) / 6.0;
    } else if cur_frame < 12.0 {
        blue = 1.0;
    } else if cur_frame <= 14.0 {
        green = (cur_frame - 11.0) / 3.0;
        blue = (14.0 - cur_frame) / 2.0;
        red = (cur_frame - 11.0) / 3.0;
    }
    (red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::name_key_generator::NameKeyGenerator;

    #[test]
    fn constructor_randomizes_frame_in_cpp_range() {
        let draw = W3DPoliceCarDraw::new(W3DPoliceCarDrawModuleData::new());
        assert!((0.0..=10.0).contains(&draw.cur_frame()));
    }

    #[test]
    fn module_name_key_is_police_car_draw_not_base_truck_draw() {
        let draw = W3DPoliceCarDraw::new(W3DPoliceCarDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DPoliceCarDraw")
        );
    }

    #[test]
    fn tick_live_host_police_car_light_creates_scene_point_light() {
        use game_engine::common::thing::module::Module;
        let mut draw = W3DPoliceCarDraw::new(W3DPoliceCarDrawModuleData::new());
        draw.tick_live_host_light([10.0, 20.0, 3.0], false);
        let lights = crate::helpers::scene_point_lights();
        assert!(
            lights.iter().any(|l| {
                (l.pos[0] - 10.0).abs() < 0.01
                    && (l.pos[1] - 20.0).abs() < 0.01
                    && (l.pos[2] - 11.0).abs() < 0.01
                    && (l.far_start - 3.0).abs() < 0.01
                    && (l.far_end - 20.0).abs() < 0.01
            }),
            "C++ W3DPoliceCarDraw light is pos.z+8 atten 3-20"
        );
        draw.on_delete();
    }
}
