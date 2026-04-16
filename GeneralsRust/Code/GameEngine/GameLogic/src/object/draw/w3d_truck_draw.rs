use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::helpers::{
    BoneOverrideState, TheAudio, TheGameClient, TheGameLogic, TheParticleSystemManager,
};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DTruckDrawModuleData {
    pub base: W3DModelDrawModuleData,
    pub dust_effect_name: AsciiString,
    pub dirt_effect_name: AsciiString,
    pub powerslide_effect_name: AsciiString,
    pub front_left_tire_bone_name: AsciiString,
    pub front_right_tire_bone_name: AsciiString,
    pub rear_left_tire_bone_name: AsciiString,
    pub rear_right_tire_bone_name: AsciiString,
    pub mid_front_left_tire_bone_name: AsciiString,
    pub mid_front_right_tire_bone_name: AsciiString,
    pub mid_rear_left_tire_bone_name: AsciiString,
    pub mid_rear_right_tire_bone_name: AsciiString,
    pub mid_mid_left_tire_bone_name: AsciiString,
    pub mid_mid_right_tire_bone_name: AsciiString,
    pub cab_bone_name: AsciiString,
    pub trailer_bone_name: AsciiString,
    pub cab_rotation_factor: Real,
    pub trailer_rotation_factor: Real,
    pub rotation_damping_factor: Real,
    pub rotation_speed_multiplier: Real,
    pub powerslide_rotation_addition: Real,
}

impl W3DTruckDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DModelDrawModuleData::new(),
            dust_effect_name: AsciiString::new(),
            dirt_effect_name: AsciiString::new(),
            powerslide_effect_name: AsciiString::new(),
            front_left_tire_bone_name: AsciiString::new(),
            front_right_tire_bone_name: AsciiString::new(),
            rear_left_tire_bone_name: AsciiString::new(),
            rear_right_tire_bone_name: AsciiString::new(),
            mid_front_left_tire_bone_name: AsciiString::new(),
            mid_front_right_tire_bone_name: AsciiString::new(),
            mid_rear_left_tire_bone_name: AsciiString::new(),
            mid_rear_right_tire_bone_name: AsciiString::new(),
            mid_mid_left_tire_bone_name: AsciiString::new(),
            mid_mid_right_tire_bone_name: AsciiString::new(),
            cab_bone_name: AsciiString::new(),
            trailer_bone_name: AsciiString::new(),
            cab_rotation_factor: 0.0,
            trailer_rotation_factor: 0.0,
            rotation_damping_factor: 1.0,
            rotation_speed_multiplier: 0.0,
            powerslide_rotation_addition: 0.0,
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
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.dust_effect_name = AsciiString::from(parsed.as_str());
                    true
                }
                "DIRTSPRAY" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.dirt_effect_name = AsciiString::from(parsed.as_str());
                    true
                }
                "POWERSLIDESPRAY" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.powerslide_effect_name = AsciiString::from(parsed.as_str());
                    true
                }
                "LEFTFRONTTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.front_left_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "RIGHTFRONTTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.front_right_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "LEFTREARTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.rear_left_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "RIGHTREARTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.rear_right_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "MIDLEFTFRONTTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.mid_front_left_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "MIDRIGHTFRONTTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.mid_front_right_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "MIDLEFTREARTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.mid_rear_left_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "MIDRIGHTREARTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.mid_rear_right_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "MIDLEFTMIDTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.mid_mid_left_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "MIDRIGHTMIDTIREBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.mid_mid_right_tire_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "TIREROTATIONMULTIPLIER" => {
                    self.rotation_speed_multiplier = INI::parse_real(required_value(&values)?)?;
                    true
                }
                "POWERSLIDEROTATIONADDITION" => {
                    self.powerslide_rotation_addition = INI::parse_real(required_value(&values)?)?;
                    true
                }
                "CABBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.cab_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "TRAILERBONE" => {
                    let parsed = INI::parse_ascii_string(required_value(&values)?)?;
                    self.trailer_bone_name = AsciiString::from(parsed.as_str());
                    true
                }
                "CABROTATIONMULTIPLIER" => {
                    self.cab_rotation_factor = INI::parse_real(required_value(&values)?)?;
                    true
                }
                "TRAILERROTATIONMULTIPLIER" => {
                    self.trailer_rotation_factor = INI::parse_real(required_value(&values)?)?;
                    true
                }
                "ROTATIONDAMPING" => {
                    self.rotation_damping_factor = INI::parse_real(required_value(&values)?)?;
                    true
                }
                _ => false,
            };
            if !handled && !self.base.parse_ini_field(ini, key.as_str(), &values)? {
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
impl Default for W3DTruckDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}
impl ModuleData for W3DTruckDrawModuleData {
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
impl DrawModuleData for W3DTruckDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DTruckDrawModuleData {
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

pub struct W3DTruckDraw {
    data: W3DTruckDrawModuleData,
    base: W3DModelDraw,
    dust_effect: Option<u32>,
    dirt_effect: Option<u32>,
    powerslide_effect: Option<u32>,
    effects_initialized: bool,
    was_airborne: bool,
    is_powersliding: bool,
    front_wheel_rotation: Real,
    rear_wheel_rotation: Real,
    mid_front_wheel_rotation: Real,
    mid_rear_wheel_rotation: Real,
    cur_cab_rotation: Real,
    cur_trailer_rotation: Real,
    landing_sound: Option<crate::common::audio::AudioEventRts>,
    powerslide_sound: Option<crate::common::audio::AudioEventRts>,
    powerslide_handle: u32,
}

impl W3DTruckDraw {
    pub fn new(data: W3DTruckDrawModuleData) -> Self {
        Self {
            data: data.clone(),
            base: W3DModelDraw::new(data.base.clone()),
            dust_effect: None,
            dirt_effect: None,
            powerslide_effect: None,
            effects_initialized: false,
            was_airborne: false,
            is_powersliding: false,
            front_wheel_rotation: 0.0,
            rear_wheel_rotation: 0.0,
            mid_front_wheel_rotation: 0.0,
            mid_rear_wheel_rotation: 0.0,
            cur_cab_rotation: 0.0,
            cur_trailer_rotation: 0.0,
            landing_sound: None,
            powerslide_sound: None,
            powerslide_handle: 0,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.base.owner_id()
    }
    fn bone_index(&self, state: &ModelConditionInfo, name: &AsciiString) -> i32 {
        if name.is_empty() {
            return 0;
        }
        let key =
            game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name.as_str());
        state
            .pristine_bones
            .get(&key)
            .map(|info| info.bone_index)
            .unwrap_or(0)
    }
    fn create_emitters(&mut self) {
        if !self.base.is_visible() {
            return;
        }
        let Some(ps) = TheParticleSystemManager::get() else {
            return;
        };
        let owner = self.base.owner_id();
        for (slot, name) in [
            (&mut self.dust_effect, &self.data.dust_effect_name),
            (&mut self.dirt_effect, &self.data.dirt_effect_name),
            (
                &mut self.powerslide_effect,
                &self.data.powerslide_effect_name,
            ),
        ] {
            if slot.is_none() && !name.is_empty() {
                if let Some(id) = ps.create_particle_system(Some(name.as_str())) {
                    if let Some(owner_id) = owner {
                        ps.attach_particle_system_to_drawable(id, owner_id);
                    }
                    ps.stop_particle_system(id);
                    *slot = Some(id);
                }
            }
        }
    }
    fn toss_emitters(&mut self) {
        if let Some(ps) = TheParticleSystemManager::get() {
            for id in [self.dust_effect, self.dirt_effect, self.powerslide_effect]
                .into_iter()
                .flatten()
            {
                ps.destroy_particle_system(id);
            }
        }
        self.dust_effect = None;
        self.dirt_effect = None;
        self.powerslide_effect = None;
    }
    fn enable_emitters(&mut self, enable: bool) {
        if !self.effects_initialized {
            self.create_emitters();
            self.effects_initialized = true;
        }
        if let Some(ps) = TheParticleSystemManager::get() {
            for id in [self.dust_effect, self.dirt_effect].into_iter().flatten() {
                if enable {
                    ps.start_particle_system(id)
                } else {
                    ps.stop_particle_system(id)
                }
            }
            if !enable {
                if let Some(id) = self.powerslide_effect {
                    ps.stop_particle_system(id);
                }
            }
        }
    }
    fn append_bone_overrides(
        &self,
        transform_mtx: &Matrix3D,
        speed: Real,
        turning: Real,
        backwards: bool,
    ) {
        let Some(owner_id) = self.base.owner_id() else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };
        let Some(mut state) = client.get_drawable_model_draw(owner_id) else {
            return;
        };
        let conditions = ModelConditionFlags::from_bits_retain(state.condition_flags_bits);
        let Some(info) = self.data.base.find_best_info(&conditions) else {
            return;
        };
        let mut overrides = state.bone_overrides;
        let add = |list: &mut Vec<BoneOverrideState>, bone_index: i32, transform: Matrix3D| {
            if bone_index != 0 {
                list.push(BoneOverrideState {
                    bone_index,
                    transform,
                });
            }
        };
        let wheel_angle = turning;
        let mut front = self.front_wheel_rotation
            + self.data.rotation_speed_multiplier * if backwards { -speed } else { speed };
        let mut rear = self.rear_wheel_rotation
            + self.data.rotation_speed_multiplier
                * if self.is_powersliding {
                    speed + self.data.powerslide_rotation_addition
                } else {
                    speed
                };
        if backwards {
            rear = -rear;
            front = -front;
        }
        add(
            &mut overrides,
            self.bone_index(info, &self.data.front_left_tire_bone_name),
            Matrix3D::from_rotation_z(wheel_angle) * Matrix3D::from_rotation_y(front),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.front_right_tire_bone_name),
            Matrix3D::from_rotation_z(wheel_angle) * Matrix3D::from_rotation_y(front),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.rear_left_tire_bone_name),
            Matrix3D::from_rotation_y(rear),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.rear_right_tire_bone_name),
            Matrix3D::from_rotation_y(rear),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_front_left_tire_bone_name),
            Matrix3D::from_rotation_z(wheel_angle)
                * Matrix3D::from_rotation_y(self.mid_front_wheel_rotation),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_front_right_tire_bone_name),
            Matrix3D::from_rotation_z(wheel_angle)
                * Matrix3D::from_rotation_y(self.mid_front_wheel_rotation),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_rear_left_tire_bone_name),
            Matrix3D::from_rotation_y(self.mid_rear_wheel_rotation),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_rear_right_tire_bone_name),
            Matrix3D::from_rotation_y(self.mid_rear_wheel_rotation),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_mid_left_tire_bone_name),
            Matrix3D::from_rotation_y(self.mid_rear_wheel_rotation),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_mid_right_tire_bone_name),
            Matrix3D::from_rotation_y(self.mid_rear_wheel_rotation),
        );
        let desired_cab = wheel_angle * self.data.cab_rotation_factor;
        let desired_trailer = -wheel_angle * self.data.trailer_rotation_factor;
        let cab_index = self.bone_index(info, &self.data.cab_bone_name);
        let trailer_index = self.bone_index(info, &self.data.trailer_bone_name);
        add(
            &mut overrides,
            cab_index,
            Matrix3D::from_rotation_z(desired_cab * self.data.rotation_damping_factor.max(0.0)),
        );
        add(
            &mut overrides,
            trailer_index,
            Matrix3D::from_rotation_z(desired_trailer * self.data.rotation_damping_factor.max(0.0)),
        );
        state.world_transform = *transform_mtx;
        state.bone_overrides = overrides;
        client.set_drawable_model_draw(owner_id, state);
    }
}

impl Module for W3DTruckDraw {
    fn on_object_created(&mut self) {
        self.base.on_object_created();
        if let Some(owner_id) = self
            .base
            .owner_id()
            .and_then(TheGameLogic::find_object_by_id)
        {
            if let Ok(owner) = owner_id.read() {
                self.landing_sound = owner.get_template().get_per_unit_sound("TruckLandingSound");
                self.powerslide_sound = owner
                    .get_template()
                    .get_per_unit_sound("TruckPowerslideSound");
            }
        }
    }
    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
        self.create_emitters();
    }
    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.base.preload_assets(time_of_day);
    }
    fn on_delete(&mut self) {
        self.toss_emitters();
        self.base.on_delete();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DTruckDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        let Some(owner_id) = self.base.owner_id() else {
            self.base.do_draw_module(transform_mtx);
            return;
        };
        let mut speed = 0.0;
        let mut turning = 0.0;
        let mut motive = false;
        let mut airborne = false;
        let mut backwards = false;
        if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
            if let Ok(owner_guard) = owner.read() {
                airborne = owner_guard.is_significantly_above_terrain();
                if let Some(physics) = owner_guard.get_physics() {
                    if let Ok(physics_guard) = physics.lock() {
                        let velocity = physics_guard.get_velocity();
                        speed = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
                        turning = physics_guard.get_turning();
                        motive = speed > 0.0;
                    }
                }
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        backwards = ai_guard
                            .get_cur_locomotor()
                            .and_then(|l| l.lock().ok().map(|loco| loco.is_moving_backwards()))
                            .unwrap_or(false);
                    }
                }
            }
        }
        self.base.do_draw_module(transform_mtx);
        self.is_powersliding = motive && !airborne && turning.abs() > 0.0001;
        self.front_wheel_rotation +=
            self.data.rotation_speed_multiplier * if backwards { -speed } else { speed };
        self.rear_wheel_rotation += self.data.rotation_speed_multiplier
            * if self.is_powersliding {
                speed
                    + self
                        .data
                        .powerslide_rotation_addition
                        .copysign(if backwards { -1.0 } else { 1.0 })
            } else if backwards {
                -speed
            } else {
                speed
            };
        self.mid_front_wheel_rotation = self.front_wheel_rotation;
        self.mid_rear_wheel_rotation = self.rear_wheel_rotation;
        self.append_bone_overrides(transform_mtx, speed, turning, backwards);
        if motive && !airborne {
            self.enable_emitters(true);
            if let Some(ps) = TheParticleSystemManager::get() {
                if let Some(id) = self.dust_effect {
                    ps.set_particle_system_burst_count_multiplier(id, speed.min(2.0));
                }
                if let Some(id) = self.powerslide_effect {
                    if self.is_powersliding {
                        ps.start_particle_system(id);
                    } else {
                        ps.stop_particle_system(id);
                    }
                }
                if let Some(id) = self.dirt_effect {
                    if self.was_airborne && !airborne {
                        ps.start_particle_system(id);
                        if let (Some(audio), Some(mut sound)) =
                            (TheAudio::get(), self.landing_sound.clone())
                        {
                            sound.set_object_id(owner_id);
                            audio.add_audio_event(&sound);
                        }
                    } else if speed > 2.0 || !motive {
                        ps.stop_particle_system(id);
                    }
                }
            }
        } else {
            self.enable_emitters(false);
        }
        if let Some(audio) = TheAudio::get() {
            if self.is_powersliding && self.powerslide_handle == 0 {
                if let Some(mut sound) = self.powerslide_sound.clone() {
                    sound.set_object_id(owner_id);
                    self.powerslide_handle = audio.add_audio_event(&sound);
                }
            } else if !self.is_powersliding && self.powerslide_handle != 0 {
                audio.remove_audio_event(self.powerslide_handle);
                self.powerslide_handle = 0;
            }
        }
        self.was_airborne = airborne;
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
            self.toss_emitters();
        } else {
            self.create_emitters();
        }
        self.base.set_fully_obscured_by_shroud(fully_obscured);
    }
    fn set_hidden(&mut self, hidden: bool) {
        DrawModule::set_hidden(&mut self.base, hidden);
        if hidden {
            self.enable_emitters(false);
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
    }
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(&self.base)
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(&mut self.base)
    }
}

impl Snapshotable for W3DTruckDraw {
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
        self.toss_emitters();
        Ok(())
    }
}
