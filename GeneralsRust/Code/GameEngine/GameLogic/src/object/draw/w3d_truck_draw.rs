use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::helpers::{
    BoneOverrideState, DrawWheelInfo, TheAudio, TheGameClient, TheGameLogic,
    TheParticleSystemManager,
};
use game_engine::common::ini::{INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

/// C++ `W3DTruckDraw.cpp:380-614` emitter/audio inputs.
#[derive(Debug, Clone, Copy, Default)]
pub struct TruckDrawLivePhysics {
    pub speed: Real,
    pub vel_x: Real,
    pub vel_y: Real,
    pub accel_x: Real,
    pub accel_y: Real,
    pub is_motive: bool,
    pub airborne: bool,
    pub frames_airborne: i32,
    pub turning: Real,
}


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
    last_live_speed: Real,
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
            last_live_speed: 0.0,
        }
    }
    pub(crate) fn fx_bone_name_for_shot(
        &self,
        weapon_slot: usize,
        barrel_index: i32,
    ) -> Option<String> {
        self.base.fx_bone_name_for_shot(weapon_slot, barrel_index)
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.base.owner_id()
    }

    pub fn last_live_speed(&self) -> Real {
        self.last_live_speed
    }
    pub fn has_render_model(&self) -> bool {
        self.base.has_render_model()
    }
    pub fn anim_frame_count(&self) -> i32 {
        self.base.anim_frame_count()
    }
    pub fn has_bound_animation(&self) -> bool {
        self.base.has_bound_animation()
    }

    pub fn bind_per_unit_sounds(
        &mut self,
        landing: Option<crate::common::audio::AudioEventRts>,
        powerslide: Option<crate::common::audio::AudioEventRts>,
    ) {
        if landing.is_some() {
            self.landing_sound = landing;
        }
        if powerslide.is_some() {
            self.powerslide_sound = powerslide;
        }
    }

    /// Bind `TruckLandingSound` / `TruckPowerslideSound` from leftover ThingFactory.
    pub fn bind_sounds_from_template(&mut self, template_name: &str) {
        if template_name.is_empty() {
            return;
        }
        if self.landing_sound.is_some() && self.powerslide_sound.is_some() {
            return;
        }
        let Ok(guard) = game_engine::common::thing::get_thing_factory() else {
            return;
        };
        let Some(factory) = guard.as_ref() else {
            return;
        };
        let Some(template) = factory.find_template(template_name, false) else {
            return;
        };
        if self.landing_sound.is_none() {
            if let Some(sound) = template.get_per_unit_sound(&String::from("TruckLandingSound")) {
                let name = sound.get_event_name();
                if !name.is_empty() {
                    self.landing_sound = Some(crate::common::audio::AudioEventRts::new(name));
                }
            }
        }
        if self.powerslide_sound.is_none() {
            if let Some(sound) = template.get_per_unit_sound(&String::from("TruckPowerslideSound"))
            {
                let name = sound.get_event_name();
                if !name.is_empty() {
                    self.powerslide_sound = Some(crate::common::audio::AudioEventRts::new(name));
                }
            }
        }
    }

    pub fn fully_obscured_by_shroud(&self) -> bool {
        self.base.fully_obscured_by_shroud()
    }

    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        self.base.set_animation_loop_duration(num_frames);
    }

    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        self.base.set_animation_completion_time(num_frames);
    }

    pub fn set_animation_frame(&mut self, frame: i32) {
        self.base.set_animation_frame(frame);
    }

    pub fn show_sub_object(&mut self, name: &str, show: bool) {
        self.base.show_sub_object(name, show);
    }

    pub fn update_sub_objects(&mut self) {
        self.base.update_sub_objects();
    }

    pub fn xfer_model_draw(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
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
                        // C++ `ParticleSystem::attachToObject`.
                        ps.attach_particle_system_to_object(id, owner_id);
                    }
                    ps.set_particle_system_saveable(id, false);
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
        // C++ always calls createEmitters (hidden-first-time tunnel case).
        self.create_emitters();
        self.effects_initialized = true;
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

    /// Live host pose. Dust/dirt/powerslide ids and speed stay on this module.
    pub fn tick_live_host(
        &mut self,
        template_name: &str,
        mut physics: TruckDrawLivePhysics,
        hidden: bool,
    ) {
        if hidden {
            self.enable_emitters(false);
            return;
        }
        self.bind_sounds_from_template(template_name);
        let delta = physics.speed - self.last_live_speed;
        if physics.speed > 0.0001 {
            physics.accel_x = physics.vel_x / physics.speed * delta;
            physics.accel_y = physics.vel_y / physics.speed * delta;
        } else {
            physics.accel_x = delta;
            physics.accel_y = 0.0;
        }
        self.tick_live(physics);
    }
    /// C++ `W3DTruckDraw::doDrawModule` dust/dirt/powerslide + landing/slide audio.
    pub fn tick_live(&mut self, physics: TruckDrawLivePhysics) {
        const ACCEL_THRESHOLD: Real = 0.01;
        const SIZE_CAP: Real = 2.0;
        let frames_airborne = physics.frames_airborne;

        let was_powersliding = self.is_powersliding;
        self.is_powersliding = false;
        if physics.is_motive && !physics.airborne {
            self.enable_emitters(true);
            let accel_len =
                (physics.accel_x * physics.accel_x + physics.accel_y * physics.accel_y).sqrt();
            let mut accelerating = accel_len > ACCEL_THRESHOLD;
            if accelerating {
                let dot = physics.accel_x * physics.vel_x + physics.accel_y * physics.vel_y;
                if dot < 0.0 {
                    accelerating = false;
                }
            }
            let speed = physics.speed;
            if let Some(ps) = TheParticleSystemManager::get() {
                if let Some(id) = self.dust_effect {
                    let size = speed.min(SIZE_CAP);
                    ps.set_particle_system_size_multiplier(id, size);
                    ps.set_particle_system_burst_count_multiplier(id, size);
                }
                if self.dirt_effect.is_some() {
                    if frames_airborne > 3 {
                        if let Some(id) = self.dust_effect {
                            let factor = (1.0 + frames_airborne as Real / 16.0).min(2.0);
                            ps.set_particle_system_size_multiplier(id, factor * SIZE_CAP);
                            ps.set_particle_system_burst_count_multiplier(id, factor * SIZE_CAP);
                            ps.trigger_particle_system(id);
                        }
                        if let (Some(audio), Some(mut sound)) =
                            (TheAudio::get(), self.landing_sound.clone())
                        {
                            if let Some(owner_id) = self.base.owner_id() {
                                sound.set_object_id(owner_id);
                            }
                            audio.add_audio_event(&sound);
                        }
                    } else if !accelerating || speed > 2.0 {
                        if let Some(id) = self.dirt_effect {
                            ps.stop_particle_system(id);
                        }
                    }
                }
                if let Some(id) = self.powerslide_effect {
                    if physics.turning.abs() <= 0.0001 {
                        ps.stop_particle_system(id);
                    } else {
                        self.is_powersliding = true;
                        ps.start_particle_system(id);
                    }
                }
                if let Some(id) = self.dirt_effect {
                    if !accelerating || speed > 2.0 {
                        ps.stop_particle_system(id);
                    }
                }
            }
            // C++ dirt stop is already applied above.
        } else {
            self.enable_emitters(false);
        }

        if let Some(audio) = TheAudio::get() {
            if !was_powersliding && self.is_powersliding {
                if let Some(mut sound) = self.powerslide_sound.clone() {
                    if let Some(owner_id) = self.base.owner_id() {
                        sound.set_object_id(owner_id);
                    }
                    self.powerslide_handle = audio.add_audio_event(&sound);
                }
            } else if was_powersliding && !self.is_powersliding && self.powerslide_handle != 0 {
                audio.remove_audio_event(self.powerslide_handle);
                self.powerslide_handle = 0;
            }
        }
        self.was_airborne = physics.airborne;
        self.last_live_speed = physics.speed;
    }
    fn append_bone_overrides(&mut self, _speed: Real, turning: Real, _backwards: bool) {
        let Some(owner_id) = self.base.owner_id() else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };
        // A Truck wrapper may refine only the base W3D result from this same
        // draw-module invocation.  Reading a committed object record here
        // would let a failed base draw mutate a preceding module's model.
        let Some(mut state) = client.with_active_object_model_draw(owner_id, |state| state.clone())
        else {
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
        let wheel_info = client.get_object_wheel_info(owner_id);
        let wheel_angle = wheel_info.map(|info| info.wheel_angle).unwrap_or(turning);
        let heights = wheel_info.unwrap_or(DrawWheelInfo::default());
        let front = self.front_wheel_rotation;
        let rear = self.rear_wheel_rotation;
        let steered = |height: Real, spin: Real| {
            Matrix3D::from_translation(glam::Vec3::new(0.0, 0.0, height))
                * Matrix3D::from_rotation_z(wheel_angle)
                * Matrix3D::from_rotation_y(spin)
        };
        let unsteered = |height: Real, spin: Real| {
            Matrix3D::from_translation(glam::Vec3::new(0.0, 0.0, height))
                * Matrix3D::from_rotation_y(spin)
        };
        add(
            &mut overrides,
            self.bone_index(info, &self.data.front_left_tire_bone_name),
            steered(heights.front_left_height_offset, front),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.front_right_tire_bone_name),
            steered(heights.front_right_height_offset, front),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.rear_left_tire_bone_name),
            unsteered(heights.rear_left_height_offset, rear),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.rear_right_tire_bone_name),
            unsteered(heights.rear_right_height_offset, rear),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_front_left_tire_bone_name),
            steered(
                heights.front_left_height_offset,
                self.mid_front_wheel_rotation,
            ),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_front_right_tire_bone_name),
            steered(
                heights.front_right_height_offset,
                self.mid_front_wheel_rotation,
            ),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_rear_left_tire_bone_name),
            unsteered(
                heights.rear_left_height_offset,
                self.mid_rear_wheel_rotation,
            ),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_rear_right_tire_bone_name),
            unsteered(
                heights.rear_right_height_offset,
                self.mid_rear_wheel_rotation,
            ),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_mid_left_tire_bone_name),
            unsteered(
                heights.rear_left_height_offset,
                self.mid_rear_wheel_rotation,
            ),
        );
        add(
            &mut overrides,
            self.bone_index(info, &self.data.mid_mid_right_tire_bone_name),
            unsteered(
                heights.rear_right_height_offset,
                self.mid_rear_wheel_rotation,
            ),
        );
        let mut desired_cab = wheel_angle * self.data.cab_rotation_factor;
        if wheel_info.is_some() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        if ai_guard.has_nonempty_path() {
                            if let Some(point) = ai_guard.peek_cached_point_on_path() {
                            let pos = *owner_guard.get_position();
                            let facing = owner_guard.get_orientation();
                            let angle_to_goal = relative_angle_2d(pos, facing, point);
                            if angle_to_goal < 0.0 {
                                if desired_cab < angle_to_goal {
                                    desired_cab = angle_to_goal;
                                }
                                if desired_cab > 0.0 {
                                    desired_cab = 0.0;
                                }
                            } else {
                                if desired_cab > angle_to_goal {
                                    desired_cab = angle_to_goal;
                                }
                                if desired_cab < 0.0 {
                                    desired_cab = 0.0;
                                }
                            }
                            }
                        }
                    }
                }
                });
        }
        let cab_index = self.bone_index(info, &self.data.cab_bone_name);
        let trailer_index = self.bone_index(info, &self.data.trailer_bone_name);
        if wheel_info.is_some() && cab_index != 0 {
            let cab_damping = self.data.rotation_damping_factor.max(0.0);
            let cab_delta = (desired_cab - self.cur_cab_rotation) * cab_damping;
            self.cur_cab_rotation += cab_delta;
            add(
                &mut overrides,
                cab_index,
                Matrix3D::from_rotation_z(self.cur_cab_rotation),
            );
            if trailer_index != 0 {
                let desired_trailer = -wheel_angle * self.data.trailer_rotation_factor;
                let trailer_delta =
                    (desired_trailer - self.cur_trailer_rotation) * cab_damping;
                self.cur_trailer_rotation += trailer_delta;
                add(
                    &mut overrides,
                    trailer_index,
                    Matrix3D::from_rotation_z(self.cur_trailer_rotation),
                );
            }
        }
        // C++ W3DTruckDraw calls the base model draw first, then controls
        // wheel/cab bones on that same render object.  The base has already
        // applied instance scaling, so replacing its world transform here
        // would silently drop it.
        state.bone_overrides = overrides;
        client.set_active_object_model_draw(owner_id, state);
    }
}

impl Drop for W3DTruckDraw {
    fn drop(&mut self) {
        self.toss_emitters();
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
        NameKeyGenerator::name_to_key("W3DTruckDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
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
        let mut vel_x = 0.0;
        let mut vel_y = 0.0;
        let mut accel_x = 0.0;
        let mut accel_y = 0.0;
        let mut turning = 0.0;
        let mut motive = false;
        let mut airborne = false;
        let mut backwards = false;
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
            airborne = owner_guard.is_significantly_above_terrain();
            if let Some(physics) = owner_guard.get_physics() {
                if let Ok(physics_guard) = physics.lock() {
                    let velocity = physics_guard.get_velocity();
                    vel_x = velocity.x;
                    vel_y = velocity.y;
                    speed = velocity.length();
                    turning = physics_guard.get_turning();
                    motive = physics_guard.is_motive();
                    let accel = physics_guard.get_acceleration();
                    accel_x = accel.x;
                    accel_y = accel.y;
                }
            }
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    ai_guard.with_cur_locomotor(&mut |loco| {
                        backwards = loco.is_moving_backwards();
                    });
                }
            }
            });
        let frames_airborne = TheGameClient::get()
            .and_then(|client| client.get_object_wheel_info(owner_id))
            .map(|info| info.frames_airborne)
            .unwrap_or(0);
        self.base.do_draw_module(transform_mtx);
        if !truck_client_physics_active() {
            return;
        }
        // C++ spins wheels from last-frame powerslide, then refreshes emitters.
        let signed_speed = if backwards { -speed } else { speed };
        let signed_addition = if backwards {
            -self.data.powerslide_rotation_addition
        } else {
            self.data.powerslide_rotation_addition
        };
        self.front_wheel_rotation += self.data.rotation_speed_multiplier * signed_speed;
        self.rear_wheel_rotation += self.data.rotation_speed_multiplier
            * if self.is_powersliding {
                signed_speed + signed_addition
            } else {
                signed_speed
            };
        self.mid_front_wheel_rotation = self.front_wheel_rotation;
        self.mid_rear_wheel_rotation = self.rear_wheel_rotation;
        self.append_bone_overrides(speed, turning, backwards);
        self.tick_live(TruckDrawLivePhysics {
            speed,
            vel_x,
            vel_y,
            accel_x,
            accel_y,
            is_motive: motive,
            airborne,
            frames_airborne,
            turning,
        });
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
        if self.base.fully_obscured_by_shroud() != fully_obscured {
            if fully_obscured {
                self.toss_emitters();
            } else {
                self.create_emitters();
            }
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
        // C++ W3DTruckDraw overrides this as a no-op.
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

fn truck_client_physics_active() -> bool {
    let show = game_engine::common::ini::get_global_data()
        .map(|data| data.read().show_client_physics)
        .unwrap_or(true);
    if !show {
        return false;
    }
    let camera_frozen = crate::helpers::get_camera_view_bridge().is_some_and(|view| {
        view.is_time_frozen() && !view.is_camera_movement_finished()
    });
    if camera_frozen
        || crate::helpers::TheScriptEngine::is_time_frozen_debug()
        || crate::helpers::TheScriptEngine::is_time_frozen_script()
    {
        return false;
    }
    true
}

fn relative_angle_2d(from: Coord3D, facing: Real, to: Coord3D) -> Real {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if dx == 0.0 && dy == 0.0 {
        return 0.0;
    }
    let mut delta = dy.atan2(dx) - facing;
    while delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    while delta < -std::f32::consts::PI {
        delta += std::f32::consts::TAU;
    }
    delta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_name_key_is_truck_draw() {
        let draw = W3DTruckDraw::new(W3DTruckDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DTruckDraw")
        );
    }

    #[test]
    fn bind_owner_id_forwards_to_base_model_draw() {
        let mut draw = W3DTruckDraw::new(W3DTruckDrawModuleData::new());
        draw.bind_owner_id(313);
        assert_eq!(draw.owner_id(), Some(313));
    }

}
