use super::draw_module::*;
use super::w3d_truck_draw::*;
use crate::common::*;
use crate::helpers::{MeshUvOverrideState, TheGameClient, TheGameLogic, TheParticleSystemManager};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DTankTruckDrawModuleData {
    pub base: W3DTruckDrawModuleData,
    pub tread_debris_name_left: AsciiString,
    pub tread_debris_name_right: AsciiString,
    pub tread_animation_rate: Real,
    pub tread_pivot_speed_fraction: Real,
    pub tread_drive_speed_fraction: Real,
}

impl W3DTankTruckDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DTruckDrawModuleData::new(),
            tread_debris_name_left: AsciiString::from("TrackDebrisDirtLeft"),
            tread_debris_name_right: AsciiString::from("TrackDebrisDirtRight"),
            tread_animation_rate: 0.0,
            tread_pivot_speed_fraction: 0.6,
            tread_drive_speed_fraction: 0.3,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::EndOfFile);
            }
            let tokens = ini
                .get_line_tokens()
                .into_iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>();
            let Some(key) = tokens.first().cloned() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }
            let values = tokens
                .iter()
                .map(String::as_str)
                .skip(1)
                .filter(|t| *t != "=")
                .collect::<Vec<_>>();
            let handled = match key.to_ascii_uppercase().as_str() {
                "DUST" => {
                    self.base.dust_effect_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "DIRTSPRAY" => {
                    self.base.dirt_effect_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "POWERSLIDESPRAY" => {
                    self.base.powerslide_effect_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "LEFTFRONTTIREBONE" => {
                    self.base.front_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "RIGHTFRONTTIREBONE" => {
                    self.base.front_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "LEFTREARTIREBONE" => {
                    self.base.rear_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "RIGHTREARTIREBONE" => {
                    self.base.rear_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDLEFTFRONTTIREBONE" => {
                    self.base.mid_front_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDRIGHTFRONTTIREBONE" => {
                    self.base.mid_front_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDLEFTREARTIREBONE" => {
                    self.base.mid_rear_left_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "MIDRIGHTREARTIREBONE" => {
                    self.base.mid_rear_right_tire_bone_name = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "TIREROTATIONMULTIPLIER" => {
                    self.base.rotation_speed_multiplier =
                        INI::parse_real(required_value(&values)?)?;
                    true
                }
                "POWERSLIDEROTATIONADDITION" => {
                    self.base.powerslide_rotation_addition =
                        INI::parse_real(required_value(&values)?)?;
                    true
                }
                "TREADDEBRISLEFT" => {
                    self.tread_debris_name_left = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "TREADDEBRISRIGHT" => {
                    self.tread_debris_name_right = AsciiString::from(
                        INI::parse_ascii_string(required_value(&values)?)?.as_str(),
                    );
                    true
                }
                "TREADANIMATIONRATE" => {
                    self.tread_animation_rate = INI::parse_velocity_real(required_value(&values)?)?;
                    true
                }
                "TREADPIVOTSPEEDFRACTION" => {
                    self.tread_pivot_speed_fraction = INI::parse_real(required_value(&values)?)?;
                    true
                }
                "TREADDRIVESPEEDFRACTION" => {
                    self.tread_drive_speed_fraction = INI::parse_real(required_value(&values)?)?;
                    true
                }
                _ => false,
            };
            if !handled && !self.base.base.parse_ini_field(ini, key.as_str(), &values)? {
                return Err(INIError::UnknownToken);
            }
        }
        Ok(())
    }
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|t| !t.is_empty())
        .ok_or(INIError::InvalidData)
}

impl Default for W3DTankTruckDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DTankTruckDrawModuleData {
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

impl DrawModuleData for W3DTankTruckDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DTankTruckDrawModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct W3DTankTruckDraw {
    data: W3DTankTruckDrawModuleData,
    base: W3DTruckDraw,
    tread_uv_offsets: Vec<Real>,
    tread_uv_offset: Real,
    last_direction: Coord3D,
    tread_debris_left: Option<u32>,
    tread_debris_right: Option<u32>,
    tread_debris_active: bool,
    current_velocity: Real,
    max_velocity: Real,
}

impl W3DTankTruckDraw {
    pub fn new(data: W3DTankTruckDrawModuleData) -> Self {
        Self {
            base: W3DTruckDraw::new(data.base.clone()),
            data,
            tread_uv_offsets: Vec::new(),
            tread_uv_offset: 0.0,
            last_direction: Coord3D::new(1.0, 0.0, 0.0),
            tread_debris_left: None,
            tread_debris_right: None,
            tread_debris_active: false,
            current_velocity: 0.0,
            max_velocity: 1.0,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }

    fn update_tread_objects(&mut self) {
        self.tread_uv_offsets.clear();
        // C++ only caches real RenderObj sub-objects named TREADS* whose materials
        // use LinearOffsetTextureMapper. Until the WGPU/W3D bridge exposes those
        // sub-objects, keep this empty instead of inventing visual treads.
    }

    fn update_tread_animation(
        &mut self,
        velocity: Real,
        max_velocity: Real,
        is_motive: bool,
        direction: &Coord3D,
    ) {
        if self.data.tread_animation_rate == 0.0 {
            self.last_direction = *direction;
            return;
        }

        let speed_fraction = if max_velocity > 0.0 {
            velocity / max_velocity
        } else {
            0.0
        };

        // C++ W3DTankTruckDraw deliberately disables pivot differential scrolling:
        // wheel+tread vehicles only scroll treads while driving above the threshold.
        if is_motive && speed_fraction >= self.data.tread_drive_speed_fraction {
            let tread_scroll_speed = self.data.tread_animation_rate;
            self.tread_uv_offset = wrap_uv_offset(self.tread_uv_offset - tread_scroll_speed);
            for uv_offset in &mut self.tread_uv_offsets {
                let offset = *uv_offset - tread_scroll_speed;
                *uv_offset = wrap_uv_offset(offset);
            }
        }

        self.last_direction = *direction;
    }

    fn publish_tread_uv_overrides(&self) {
        let Some(owner_id) = self.base.owner_id() else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };
        let Some(mut state) = client.get_drawable_model_draw(owner_id) else {
            return;
        };

        state.mesh_uv_overrides.push(MeshUvOverrideState {
            mesh_name_prefix: "TREADS".to_string(),
            u_offset: self.tread_uv_offset,
            v_offset: 0.0,
        });
        client.set_drawable_model_draw(owner_id, state);
    }

    fn create_tread_emitters(&mut self) {
        if !self.base.is_visible() {
            return;
        }
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        let owner_id = self.base.owner_id();

        if self.tread_debris_left.is_none() && !self.data.tread_debris_name_left.is_empty() {
            if let Some(id) =
                ps_manager.create_particle_system(Some(self.data.tread_debris_name_left.as_str()))
            {
                if let Some(owner_id) = owner_id {
                    ps_manager.attach_particle_system_to_drawable(id, owner_id);
                }
                ps_manager.stop_particle_system(id);
                self.tread_debris_left = Some(id);
            }
        }

        if self.tread_debris_right.is_none() && !self.data.tread_debris_name_right.is_empty() {
            if let Some(id) =
                ps_manager.create_particle_system(Some(self.data.tread_debris_name_right.as_str()))
            {
                if let Some(owner_id) = owner_id {
                    ps_manager.attach_particle_system_to_drawable(id, owner_id);
                }
                ps_manager.stop_particle_system(id);
                self.tread_debris_right = Some(id);
            }
        }
    }

    fn toss_tread_emitters(&mut self) {
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            for id in [self.tread_debris_left, self.tread_debris_right]
                .into_iter()
                .flatten()
            {
                ps_manager.destroy_particle_system(id);
            }
        }
        self.tread_debris_left = None;
        self.tread_debris_right = None;
        self.tread_debris_active = false;
    }

    fn start_move_debris(&mut self) {
        if self.tread_debris_active || !self.base.is_visible() {
            return;
        }
        self.tread_debris_active = true;
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            if let Some(id) = self.tread_debris_left {
                ps_manager.start_particle_system(id);
            }
            if let Some(id) = self.tread_debris_right {
                ps_manager.start_particle_system(id);
            }
        }
    }

    fn stop_move_debris(&mut self) {
        if !self.tread_debris_active {
            return;
        }
        self.tread_debris_active = false;
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            if let Some(id) = self.tread_debris_left {
                ps_manager.stop_particle_system(id);
            }
            if let Some(id) = self.tread_debris_right {
                ps_manager.stop_particle_system(id);
            }
        }
    }

    fn update_tread_debris(&mut self) {
        const DEBRIS_THRESHOLD: Real = 0.00001;
        let velocity_mag_sq = self.current_velocity * self.current_velocity;
        if velocity_mag_sq > DEBRIS_THRESHOLD && self.base.is_visible() {
            self.start_move_debris();
        } else {
            self.stop_move_debris();
        }

        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        let vel_mag = self.current_velocity;
        let x = (0.5 * vel_mag + 0.1).min(1.0);
        let z = (vel_mag + 0.1).min(1.0);
        let vel_mult = Coord3D::new(x, x, z);
        for id in [self.tread_debris_left, self.tread_debris_right]
            .into_iter()
            .flatten()
        {
            ps_manager.set_particle_system_velocity_multiplier(id, &vel_mult);
            ps_manager.set_particle_system_burst_count_multiplier(id, z);
        }
    }
}

impl Module for W3DTankTruckDraw {
    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
        self.create_tread_emitters();
        self.update_tread_objects();
    }
    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.base.preload_assets(time_of_day);
    }
    fn on_delete(&mut self) {
        self.toss_tread_emitters();
        self.base.on_delete();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DTankTruckDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        self.base.do_draw_module(transform_mtx);

        let mut direction = Coord3D::new(transform_mtx.x_axis.x, transform_mtx.x_axis.y, 0.0);
        let mut is_motive = false;
        if let Some(owner_id) = self.base.owner_id() {
            if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
                if let Ok(owner_guard) = owner.read() {
                    let (dir_x, dir_y) = owner_guard.get_unit_direction_vector_2d();
                    if dir_x != 0.0 || dir_y != 0.0 {
                        direction = Coord3D::new(dir_x, dir_y, 0.0);
                    }
                    if let Some(physics) = owner_guard.get_physics() {
                        if let Ok(physics_guard) = physics.lock() {
                            let velocity = physics_guard.get_velocity();
                            self.current_velocity =
                                (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
                            is_motive = self.current_velocity > 0.0;
                        }
                    }
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai.lock() {
                            let locomotor_speed = ai_guard.get_cur_locomotor_speed();
                            if locomotor_speed > 0.0 {
                                self.max_velocity = locomotor_speed;
                            }
                        }
                    }
                }
            }
        }
        if self.max_velocity <= 0.0 {
            self.max_velocity = 1.0;
        }
        self.update_tread_animation(
            self.current_velocity,
            self.max_velocity,
            is_motive,
            &direction,
        );
        self.publish_tread_uv_overrides();
        self.update_tread_debris();
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
        if fully_obscured {
            self.stop_move_debris();
        } else {
            self.create_tread_emitters();
        }
        self.base.set_fully_obscured_by_shroud(fully_obscured);
    }
    fn set_hidden(&mut self, hidden: bool) {
        DrawModule::set_hidden(&mut self.base, hidden);
        if hidden {
            self.stop_move_debris();
            self.toss_tread_emitters();
        } else {
            self.create_tread_emitters();
        }
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
        self.update_tread_objects();
    }
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        self.base.get_object_draw_interface()
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        self.base.get_object_draw_interface_mut()
    }
}

impl Snapshotable for W3DTankTruckDraw {
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
        self.base.load_post_process()?;
        self.toss_tread_emitters();
        self.update_tread_objects();
        Ok(())
    }
}

fn wrap_uv_offset(offset: Real) -> Real {
    offset - offset.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_tread_objects_does_not_fabricate_tanktruck_treads() {
        let mut draw = W3DTankTruckDraw::new(W3DTankTruckDrawModuleData {
            tread_animation_rate: 1.0,
            ..W3DTankTruckDrawModuleData::default()
        });
        draw.tread_uv_offsets.push(0.0);

        draw.update_tread_objects();

        assert!(draw.tread_uv_offsets.is_empty());
    }

    #[test]
    fn tanktruck_treads_scroll_only_when_driving_above_threshold() {
        let mut draw = W3DTankTruckDraw::new(W3DTankTruckDrawModuleData {
            tread_animation_rate: 0.25,
            tread_drive_speed_fraction: 0.3,
            ..W3DTankTruckDrawModuleData::default()
        });
        draw.tread_uv_offsets.push(0.0);
        let direction = Coord3D::new(1.0, 0.0, 0.0);

        draw.update_tread_animation(2.0, 10.0, true, &direction);
        assert_eq!(draw.tread_uv_offsets[0], 0.0);

        draw.update_tread_animation(4.0, 10.0, true, &direction);
        assert_eq!(draw.tread_uv_offsets[0], 0.75);
    }

    #[test]
    fn tanktruck_treads_use_uniform_drive_scroll() {
        let mut draw = W3DTankTruckDraw::new(W3DTankTruckDrawModuleData {
            tread_animation_rate: 0.25,
            tread_drive_speed_fraction: 0.0,
            ..W3DTankTruckDrawModuleData::default()
        });
        draw.tread_uv_offsets.extend([0.0, 0.5, 0.9]);
        let direction = Coord3D::new(1.0, 0.0, 0.0);

        draw.update_tread_animation(1.0, 10.0, true, &direction);

        assert_eq!(draw.tread_uv_offsets, vec![0.75, 0.25, 0.65]);
    }

    #[test]
    fn stop_move_debris_clears_active_flag_without_particle_manager() {
        let mut draw = W3DTankTruckDraw::new(W3DTankTruckDrawModuleData::default());
        draw.tread_debris_active = true;

        draw.stop_move_debris();

        assert!(!draw.tread_debris_active);
    }
}
