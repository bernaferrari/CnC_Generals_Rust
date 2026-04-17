//! ParticleUplinkCannonUpdate - Superweapon logic
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::command_button::CommandButton;
use crate::common::audio::AudioEventRts;
use crate::common::{
    AsciiString, Bool, Coord3D, DisabledType, DrawableID, Matrix3D, ModelConditionFlags,
    ModuleData, ObjectID, ObjectShroudStatus, ParticleSystemID, PlayerMaskType, Real, UnsignedInt,
    LOGICFRAMES_PER_SECOND,
};
use crate::damage::DamageInfo;
use crate::helpers::TheParticleSystemManager;
use crate::helpers::{
    game_client_random_value, TheAudio, TheFXListStore, TheGameClient, TheGameLogic,
    ThePartitionManager, TheTerrainLogic, TheThingFactory,
};
use crate::modules::{
    BehaviorModuleInterface, SpecialPowerModuleInterface, SpecialPowerUpdateInterface,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::special_power_module::SpecialPowerCommandOptions;
use crate::object::special_power_module::Waypoint;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object::DrawableArcExt;
use crate::object::{Object as GameObject, ObjectId};
use crate::player::ThePlayerList;
use crate::system::shroud_manager::get_shroud_manager;
use crate::weapon::{DamageType, DeathType};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemID = 0;
const INVALID_DRAWABLE_ID: DrawableID = 0;
const SCORCH_1: i32 = 1;
const SCORCH_4: i32 = 4;

/// Status for the Particle Uplink Cannon
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PUCStatus {
    Idle,
    Charging,
    Preparing,
    AlmostReady,
    ReadyToFire,
    PreFire,
    Firing,
    PostFire,
    Packing,
}

/// Status for the laser beam
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaserStatus {
    None,
    Born,
    Decaying,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntensityTypes {
    Light,
    Medium,
    Intense,
    #[allow(dead_code)]
    Finish,
}

#[derive(Clone, Debug)]
pub struct ParticleUplinkCannonUpdateModuleData {
    pub base: BehaviorModuleData,
    pub special_power_template: Option<Arc<SpecialPowerTemplate>>,
    pub begin_charge_frames: UnsignedInt,
    pub raise_antenna_frames: UnsignedInt,
    pub ready_delay_frames: UnsignedInt,
    pub width_grow_frames: UnsignedInt,
    pub beam_travel_frames: UnsignedInt,
    pub total_firing_frames: UnsignedInt,
    pub frames_between_launch_fx_refresh: UnsignedInt,

    pub outer_effect_base_bone_name: AsciiString,
    pub outer_effect_num_bones: UnsignedInt,
    pub outer_nodes_light_flare_particle_system_name: AsciiString,
    pub outer_nodes_medium_flare_particle_system_name: AsciiString,
    pub outer_nodes_intense_flare_particle_system_name: AsciiString,

    pub connector_bone_name: AsciiString,
    pub connector_medium_laser_name: AsciiString,
    pub connector_intense_laser_name: AsciiString,
    pub connector_medium_flare_particle_system_name: AsciiString,
    pub connector_intense_flare_particle_system_name: AsciiString,

    pub laser_base_light_flare_particle_system_name: AsciiString,
    pub laser_base_medium_flare_particle_system_name: AsciiString,
    pub laser_base_intense_flare_particle_system_name: AsciiString,

    pub fire_bone_name: AsciiString,
    pub particle_beam_laser_name: AsciiString,

    // FX list names resolved at runtime against TheFXListStore.
    // Unresolved names are skipped without synthetic placeholder creation.
    pub ground_hit_fx_name: AsciiString,
    pub beam_launch_fx_name: AsciiString,

    pub swath_of_death_distance: Real,
    pub swath_of_death_amplitude: Real,
    pub total_scorch_marks: UnsignedInt,
    pub scorch_mark_scalar: Real,

    pub total_damage_pulses: UnsignedInt,
    pub damage_per_second: Real,
    pub damage_type: DamageType,
    pub death_type: DeathType,
    pub damage_radius_scalar: Real,
    pub reveal_range: Real,

    pub powerup_sound_name: AsciiString,
    pub unpack_to_ready_sound_name: AsciiString,
    pub firing_to_idle_sound_name: AsciiString,
    pub annihilation_sound_name: AsciiString,
    pub damage_pulse_remnant_object_name: AsciiString,

    pub manual_driving_speed: Real,
    pub manual_fast_driving_speed: Real,
    pub double_click_to_fast_drive_delay: UnsignedInt,
}

impl Default for ParticleUplinkCannonUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_power_template: None,
            begin_charge_frames: 0,
            raise_antenna_frames: 0,
            ready_delay_frames: 0,
            width_grow_frames: 0,
            beam_travel_frames: 0,
            total_firing_frames: 0,
            frames_between_launch_fx_refresh: 30,
            outer_effect_base_bone_name: AsciiString::new(),
            outer_effect_num_bones: 0,
            outer_nodes_light_flare_particle_system_name: AsciiString::new(),
            outer_nodes_medium_flare_particle_system_name: AsciiString::new(),
            outer_nodes_intense_flare_particle_system_name: AsciiString::new(),
            connector_bone_name: AsciiString::new(),
            connector_medium_laser_name: AsciiString::new(),
            connector_intense_laser_name: AsciiString::new(),
            connector_medium_flare_particle_system_name: AsciiString::new(),
            connector_intense_flare_particle_system_name: AsciiString::new(),
            laser_base_light_flare_particle_system_name: AsciiString::new(),
            laser_base_medium_flare_particle_system_name: AsciiString::new(),
            laser_base_intense_flare_particle_system_name: AsciiString::new(),
            fire_bone_name: AsciiString::new(),
            particle_beam_laser_name: AsciiString::new(),
            ground_hit_fx_name: AsciiString::new(),
            beam_launch_fx_name: AsciiString::new(),
            swath_of_death_distance: 0.0,
            swath_of_death_amplitude: 0.0,
            total_scorch_marks: 0,
            scorch_mark_scalar: 1.0,
            total_damage_pulses: 0,
            damage_per_second: 0.0,
            damage_type: DamageType::Laser,
            death_type: DeathType::Lasered,
            damage_radius_scalar: 1.0,
            reveal_range: 0.0,
            powerup_sound_name: AsciiString::new(),
            unpack_to_ready_sound_name: AsciiString::new(),
            firing_to_idle_sound_name: AsciiString::new(),
            annihilation_sound_name: AsciiString::new(),
            damage_pulse_remnant_object_name: AsciiString::new(),
            manual_driving_speed: 0.0,
            manual_fast_driving_speed: 0.0,
            double_click_to_fast_drive_delay: 500,
        }
    }
}

crate::impl_behavior_module_data_via_base!(ParticleUplinkCannonUpdateModuleData, base);

pub struct ParticleUplinkCannonUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<ParticleUplinkCannonUpdateModuleData>,

    status: PUCStatus,
    laser_status: LaserStatus,

    // Position/Steering state
    connector_node_position: Coord3D,
    laser_origin_position: Coord3D,
    initial_target_position: Coord3D,
    current_target_position: Coord3D,
    override_target_destination: Coord3D,

    // Timers & Counters
    start_attack_frame: UnsignedInt,
    start_decay_frame: UnsignedInt,
    next_scorch_mark_frame: UnsignedInt,
    scorch_marks_made: UnsignedInt,
    next_damage_pulse_frame: UnsignedInt,
    damage_pulses_made: UnsignedInt,
    next_launch_fx_frame: UnsignedInt,
    ground_to_orbit_decay_end_frame: UnsignedInt,

    // Steering control
    manual_target_mode: Bool,
    scripted_waypoint_mode: Bool,
    #[allow(dead_code)]
    next_dest_waypoint_id: UnsignedInt,

    last_driving_click_frame: UnsignedInt,
    second_last_driving_click_frame: UnsignedInt,

    outer_system_ids: Vec<ParticleSystemID>,
    laser_beam_ids: Vec<DrawableID>,
    ground_to_orbit_beam_id: DrawableID,
    orbit_to_target_beam_id: DrawableID,
    connector_system_id: ParticleSystemID,
    laser_base_system_id: ParticleSystemID,
    outer_node_positions: Vec<Coord3D>,
    outer_node_orientations: Vec<Matrix3D>,
    default_info_cached: Bool,
    up_bones_cached: Bool,

    client_shrouded_last_frame: Bool,
    invalid_settings: Bool,

    powerup_sound: AudioEventRts,
    unpack_to_ready_sound: AudioEventRts,
    firing_to_idle_sound: AudioEventRts,
    annihilation_sound: AudioEventRts,
}

impl ParticleUplinkCannonUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
        .downcast_ref::<ParticleUplinkCannonUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let obj_guard = object.read().unwrap();
        let position = obj_guard.get_position();
        let outer_count = specific_data.outer_effect_num_bones as usize;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            status: PUCStatus::Idle,
            laser_status: LaserStatus::None,
            connector_node_position: *position,
            laser_origin_position: *position,
            initial_target_position: Coord3D::ZERO,
            current_target_position: Coord3D::ZERO,
            override_target_destination: Coord3D::ZERO,
            start_attack_frame: 0,
            start_decay_frame: 0,
            next_scorch_mark_frame: 0,
            scorch_marks_made: 0,
            next_damage_pulse_frame: 0,
            damage_pulses_made: 0,
            next_launch_fx_frame: 0,
            ground_to_orbit_decay_end_frame: 0,
            manual_target_mode: false,
            scripted_waypoint_mode: false,
            next_dest_waypoint_id: 0,
            last_driving_click_frame: 0,
            second_last_driving_click_frame: 0,
            outer_system_ids: vec![INVALID_PARTICLE_SYSTEM_ID; outer_count],
            laser_beam_ids: vec![INVALID_DRAWABLE_ID; outer_count],
            ground_to_orbit_beam_id: INVALID_DRAWABLE_ID,
            orbit_to_target_beam_id: INVALID_DRAWABLE_ID,
            connector_system_id: INVALID_PARTICLE_SYSTEM_ID,
            laser_base_system_id: INVALID_PARTICLE_SYSTEM_ID,
            outer_node_positions: vec![Coord3D::ZERO; outer_count],
            outer_node_orientations: vec![Matrix3D::IDENTITY; outer_count],
            default_info_cached: false,
            up_bones_cached: false,
            client_shrouded_last_frame: false,
            invalid_settings: specific_data.special_power_template.is_none(),
            powerup_sound: AudioEventRts::new(specific_data.powerup_sound_name.as_str()),
            unpack_to_ready_sound: AudioEventRts::new(
                specific_data.unpack_to_ready_sound_name.as_str(),
            ),
            firing_to_idle_sound: AudioEventRts::new(
                specific_data.firing_to_idle_sound_name.as_str(),
            ),
            annihilation_sound: AudioEventRts::new(specific_data.annihilation_sound_name.as_str()),
        })
    }

    fn with_special_power_module<F, R>(&mut self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn SpecialPowerModuleInterface) -> R,
    {
        let mut func = Some(func);
        let obj_arc = self.object.upgrade()?;
        let obj = obj_arc.read().ok()?;
        let template = self.module_data.special_power_template.as_ref()?;
        obj.with_special_power_module_mut_by_name(template.get_name(), |module| {
            let func = func.take().expect("special power callback already used");
            func(module)
        })
    }

    fn get_ready_frame(&mut self) -> UnsignedInt {
        self.with_special_power_module(|module| module.get_ready_frame())
            .unwrap_or(0)
    }

    fn remove_all_effects(&mut self) {
        let client = TheGameClient::get();
        for id in self.outer_system_ids.iter_mut() {
            *id = INVALID_PARTICLE_SYSTEM_ID;
        }
        for id in self.laser_beam_ids.iter_mut() {
            if let Some(client) = client {
                if *id != INVALID_DRAWABLE_ID {
                    client.destroy_drawable(*id);
                }
            }
            *id = INVALID_DRAWABLE_ID;
        }
        self.connector_system_id = INVALID_PARTICLE_SYSTEM_ID;
        self.laser_base_system_id = INVALID_PARTICLE_SYSTEM_ID;
        if let Some(client) = client {
            if self.ground_to_orbit_beam_id != INVALID_DRAWABLE_ID {
                client.destroy_drawable(self.ground_to_orbit_beam_id);
            }
            if self.orbit_to_target_beam_id != INVALID_DRAWABLE_ID {
                client.destroy_drawable(self.orbit_to_target_beam_id);
            }
        }
        self.ground_to_orbit_beam_id = INVALID_DRAWABLE_ID;
        self.orbit_to_target_beam_id = INVALID_DRAWABLE_ID;
        self.ground_to_orbit_decay_end_frame = 0;
        Self::stop_audio_event(&mut self.powerup_sound);
        Self::stop_audio_event(&mut self.unpack_to_ready_sound);
        Self::stop_audio_event(&mut self.firing_to_idle_sound);
        Self::stop_audio_event(&mut self.annihilation_sound);
    }

    fn set_logical_status(&mut self, status: PUCStatus) {
        if self.status != status {
            if let Some(object_arc) = self.object.upgrade() {
                if let Ok(obj_guard) = object_arc.read() {
                    if let Some(drawable) = obj_guard.get_drawable() {
                        let clear = ModelConditionFlags::Packing | ModelConditionFlags::Unpacking;
                        let set = match status {
                            PUCStatus::Preparing => ModelConditionFlags::Unpacking,
                            PUCStatus::Packing => ModelConditionFlags::Packing,
                            _ => ModelConditionFlags::empty(),
                        };
                        drawable.clear_and_set_model_condition_state(clear, set);
                    }
                }
            }

            if matches!(
                status,
                PUCStatus::Charging
                    | PUCStatus::Preparing
                    | PUCStatus::AlmostReady
                    | PUCStatus::ReadyToFire
            ) {
                self.laser_status = LaserStatus::None;
            }
            if status == PUCStatus::Firing {
                self.next_launch_fx_frame = 0;
            }

            self.status = status;
            self.handle_status_audio(status);
            self.set_client_status(self.status, false);
        }
    }

    fn handle_status_audio(&mut self, status: PUCStatus) {
        let Some((object_id, position)) = self.get_object_id_and_position() else {
            return;
        };

        match status {
            PUCStatus::Charging => {
                Self::play_audio_event(&mut self.powerup_sound, object_id, &position);
            }
            PUCStatus::ReadyToFire => {
                Self::play_audio_event(&mut self.unpack_to_ready_sound, object_id, &position);
            }
            PUCStatus::Firing => {
                Self::play_audio_event(&mut self.annihilation_sound, object_id, &position);
            }
            PUCStatus::Packing => {
                Self::play_audio_event(&mut self.firing_to_idle_sound, object_id, &position);
            }
            _ => {}
        }
    }

    fn get_object_id_and_position(&self) -> Option<(ObjectID, Coord3D)> {
        let object_arc = self.object.upgrade()?;
        let obj_guard = object_arc.read().ok()?;
        let pos = obj_guard.get_position();
        Some((obj_guard.get_id(), Coord3D::new(pos.x, pos.y, pos.z)))
    }

    fn play_audio_event(event: &mut AudioEventRts, object_id: ObjectID, position: &Coord3D) {
        if event.is_currently_playing() || event.get_event_name().is_empty() {
            return;
        }

        event.set_object_id(object_id);
        event.set_position(&(position.x, position.y, position.z));

        if let Some(audio) = TheAudio::get() {
            let handle = audio.add_audio_event(event);
            event.set_playing_handle(handle);
        }
    }

    fn stop_audio_event(event: &mut AudioEventRts) {
        if event.is_currently_playing() {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(event.get_playing_handle());
            }
            event.set_playing_handle(0);
        }
    }

    fn calculate_default_information(&mut self) -> Bool {
        let count = self.module_data.outer_effect_num_bones as usize;
        if self.outer_node_positions.len() != count {
            self.outer_node_positions.resize(count, Coord3D::ZERO);
        }
        if self.outer_node_orientations.len() != count {
            self.outer_node_orientations
                .resize(count, Matrix3D::IDENTITY);
        }
        if self.outer_system_ids.len() != count {
            self.outer_system_ids
                .resize(count, INVALID_PARTICLE_SYSTEM_ID);
        }
        if self.laser_beam_ids.len() != count {
            self.laser_beam_ids.resize(count, INVALID_DRAWABLE_ID);
        }
        true
    }

    fn create_outer_node_particle_systems(&mut self, intensity: IntensityTypes) {
        let name = match intensity {
            IntensityTypes::Light => {
                &self
                    .module_data
                    .outer_nodes_light_flare_particle_system_name
            }
            IntensityTypes::Medium => {
                &self
                    .module_data
                    .outer_nodes_medium_flare_particle_system_name
            }
            IntensityTypes::Intense => {
                &self
                    .module_data
                    .outer_nodes_intense_flare_particle_system_name
            }
            IntensityTypes::Finish => return,
        };
        if name.is_empty() {
            return;
        }
        let Some(manager) = TheParticleSystemManager::get() else {
            return;
        };
        let object_id = self
            .object
            .upgrade()
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_id()));
        for (idx, system_id) in self.outer_system_ids.iter_mut().enumerate() {
            if let Some(new_id) = manager.create_particle_system(Some(name.as_str())) {
                *system_id = new_id;
                if let Some(object_id) = object_id {
                    manager.attach_particle_system_to_object(new_id, object_id);
                }
                if let Some(pos) = self.outer_node_positions.get(idx) {
                    manager.set_particle_system_position(new_id, pos);
                }
                if let Some(orient) = self.outer_node_orientations.get(idx) {
                    manager.set_particle_system_transform(new_id, orient);
                }
            }
        }
    }

    fn create_connector_lasers(&mut self, intensity: IntensityTypes) {
        if !self.up_bones_cached {
            self.calculate_up_bone_positions();
            self.up_bones_cached = true;
        }

        let name = match intensity {
            IntensityTypes::Medium => &self.module_data.connector_medium_laser_name,
            IntensityTypes::Intense => &self.module_data.connector_intense_laser_name,
            _ => return,
        };
        if name.is_empty() {
            return;
        }
        let Some(template) = TheThingFactory::find_template(name.as_str()) else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };
        for (idx, id) in self.laser_beam_ids.iter_mut().enumerate() {
            let new_id = client.create_drawable(template.as_ref());
            *id = new_id;
            if let Some(start) = self.outer_node_positions.get(idx) {
                client.set_drawable_beam(new_id, start, &self.connector_node_position);
            }
        }
    }

    fn calculate_up_bone_positions(&mut self) -> Bool {
        let Some(object_arc) = self.object.upgrade() else {
            return false;
        };
        let Ok(obj_guard) = object_arc.read() else {
            return false;
        };
        let Some(draw) = obj_guard.get_drawable() else {
            return false;
        };
        let Ok(draw_guard) = draw.read() else {
            return false;
        };

        if !self.module_data.connector_bone_name.is_empty() {
            if let Some(matrix) = draw_guard.get_current_worldspace_client_bone_positions(
                self.module_data.connector_bone_name.as_str(),
            ) {
                let world = obj_guard.convert_bone_pos_to_world_pos(None, Some(&matrix));
                let translation = world.w_axis;
                self.connector_node_position =
                    Coord3D::new(translation.x, translation.y, translation.z);
            }
        }

        if !self.module_data.fire_bone_name.is_empty() {
            if let Some(matrix) = draw_guard.get_current_worldspace_client_bone_positions(
                self.module_data.fire_bone_name.as_str(),
            ) {
                let world = obj_guard.convert_bone_pos_to_world_pos(None, Some(&matrix));
                let translation = world.w_axis;
                self.laser_origin_position =
                    Coord3D::new(translation.x, translation.y, translation.z);
            }
        }

        true
    }

    fn create_connector_flare(&mut self, intensity: IntensityTypes) {
        let name = match intensity {
            IntensityTypes::Medium => &self.module_data.connector_medium_flare_particle_system_name,
            IntensityTypes::Intense => {
                &self
                    .module_data
                    .connector_intense_flare_particle_system_name
            }
            _ => return,
        };
        if name.is_empty() {
            return;
        }
        let Some(manager) = TheParticleSystemManager::get() else {
            return;
        };
        if let Some(new_id) = manager.create_particle_system(Some(name.as_str())) {
            self.connector_system_id = new_id;
            manager.set_particle_system_position(new_id, &self.connector_node_position);
        }
    }

    fn create_laser_base_flare(&mut self, intensity: IntensityTypes) {
        let name = match intensity {
            IntensityTypes::Light => &self.module_data.laser_base_light_flare_particle_system_name,
            IntensityTypes::Medium => {
                &self
                    .module_data
                    .laser_base_medium_flare_particle_system_name
            }
            IntensityTypes::Intense => {
                &self
                    .module_data
                    .laser_base_intense_flare_particle_system_name
            }
            _ => return,
        };
        if name.is_empty() {
            return;
        }
        let Some(manager) = TheParticleSystemManager::get() else {
            return;
        };
        if let Some(new_id) = manager.create_particle_system(Some(name.as_str())) {
            self.laser_base_system_id = new_id;
            manager.set_particle_system_position(new_id, &self.laser_origin_position);
        }
    }

    fn create_ground_to_orbit_laser(&mut self, _growth_frames: UnsignedInt) {
        let Some(client) = TheGameClient::get() else {
            return;
        };
        if self.ground_to_orbit_beam_id != INVALID_DRAWABLE_ID {
            client.destroy_drawable(self.ground_to_orbit_beam_id);
            self.ground_to_orbit_beam_id = INVALID_DRAWABLE_ID;
        }
        if self.module_data.particle_beam_laser_name.is_empty() {
            return;
        }
        let Some(template) =
            TheThingFactory::find_template(self.module_data.particle_beam_laser_name.as_str())
        else {
            return;
        };
        let new_id = client.create_drawable(template.as_ref());
        self.ground_to_orbit_beam_id = new_id;
        let mut orbit = self.laser_origin_position;
        orbit.z += 500.0;
        client.set_drawable_beam(new_id, &self.laser_origin_position, &orbit);
    }

    fn create_orbit_to_target_laser(&mut self, _growth_frames: UnsignedInt) {
        let Some(client) = TheGameClient::get() else {
            return;
        };
        if self.orbit_to_target_beam_id != INVALID_DRAWABLE_ID {
            Self::stop_audio_event(&mut self.annihilation_sound);
            client.destroy_drawable(self.orbit_to_target_beam_id);
            self.orbit_to_target_beam_id = INVALID_DRAWABLE_ID;
        }
        if self.module_data.particle_beam_laser_name.is_empty() {
            return;
        }
        let Some(template) =
            TheThingFactory::find_template(self.module_data.particle_beam_laser_name.as_str())
        else {
            return;
        };
        let new_id = client.create_drawable(template.as_ref());
        self.orbit_to_target_beam_id = new_id;
        let mut orbit = self.initial_target_position;
        orbit.z += 500.0;
        client.set_drawable_beam(new_id, &orbit, &self.initial_target_position);
    }

    fn set_client_status(&mut self, status: PUCStatus, reveal_this_frame: Bool) {
        if !self.default_info_cached {
            if !self.calculate_default_information() {
                self.invalid_settings = true;
                return;
            }
            self.default_info_cached = true;
        }

        self.remove_all_effects();

        match status {
            PUCStatus::Idle => {}
            PUCStatus::Charging => {
                self.create_outer_node_particle_systems(IntensityTypes::Light);
            }
            PUCStatus::Preparing => {
                self.create_outer_node_particle_systems(IntensityTypes::Medium);
            }
            PUCStatus::AlmostReady => {
                self.create_outer_node_particle_systems(IntensityTypes::Medium);
                self.create_connector_lasers(IntensityTypes::Medium);
                self.create_connector_flare(IntensityTypes::Medium);
            }
            PUCStatus::ReadyToFire => {
                self.create_outer_node_particle_systems(IntensityTypes::Medium);
                self.create_connector_lasers(IntensityTypes::Medium);
                self.create_connector_flare(IntensityTypes::Medium);
                self.create_laser_base_flare(IntensityTypes::Light);
            }
            PUCStatus::PreFire => {}
            PUCStatus::Firing => {
                let growth_frames = if reveal_this_frame {
                    0
                } else {
                    self.module_data.width_grow_frames
                };
                self.create_ground_to_orbit_laser(growth_frames);
                self.create_outer_node_particle_systems(IntensityTypes::Intense);
                self.create_connector_lasers(IntensityTypes::Intense);
                self.create_connector_flare(IntensityTypes::Intense);
                self.create_laser_base_flare(IntensityTypes::Intense);
            }
            PUCStatus::PostFire => {
                self.create_outer_node_particle_systems(IntensityTypes::Medium);
                self.create_connector_lasers(IntensityTypes::Medium);
                self.create_connector_flare(IntensityTypes::Medium);
                self.create_laser_base_flare(IntensityTypes::Medium);
                self.create_ground_to_orbit_laser(0);
                if self.ground_to_orbit_beam_id != INVALID_DRAWABLE_ID {
                    let now = TheGameLogic::get_frame();
                    self.ground_to_orbit_decay_end_frame =
                        now.saturating_add(self.module_data.width_grow_frames);
                }
            }
            PUCStatus::Packing => {}
        }
    }
}

impl UpdateModuleInterface for ParticleUplinkCannonUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.invalid_settings {
            return UpdateSleepTime::None; // C++ checks this first
        }

        // Logic parity check: Object status checks (Sold, UnderConstruction, EffectivelyDead)
        // are typically handled by the BehaviorModule wrapper or the game engine in Rust port,
        // but we mimic C++ structure for fidelity where possible.

        // Get current frame
        let now = TheGameLogic::get_frame();

        let data = self.module_data.clone();

        // Calculate key frames
        let ready_to_fire_frame = self.get_ready_frame();
        let almost_ready_frame = if ready_to_fire_frame > data.ready_delay_frames {
            ready_to_fire_frame - data.ready_delay_frames
        } else {
            0
        };
        let raise_antenna_frame = if almost_ready_frame > data.raise_antenna_frames {
            almost_ready_frame - data.raise_antenna_frames
        } else {
            0
        };
        let begin_charge_frame = if raise_antenna_frame > data.begin_charge_frames {
            raise_antenna_frame - data.begin_charge_frames
        } else {
            0
        };

        // Handle Active Firing State
        if self.start_attack_frame != 0 && self.start_attack_frame <= now {
            if self.ground_to_orbit_decay_end_frame != 0
                && now >= self.ground_to_orbit_decay_end_frame
            {
                if let Some(client) = TheGameClient::get() {
                    if self.ground_to_orbit_beam_id != INVALID_DRAWABLE_ID {
                        client.destroy_drawable(self.ground_to_orbit_beam_id);
                    }
                }
                self.ground_to_orbit_beam_id = INVALID_DRAWABLE_ID;
                self.ground_to_orbit_decay_end_frame = 0;
            }

            if self.start_decay_frame > now {
                if let Some(object_arc) = self.object.upgrade() {
                    if let Ok(obj_guard) = object_arc.read() {
                        if obj_guard.is_disabled_by_type(DisabledType::DisabledUnderpowered)
                            || obj_guard.is_disabled_by_type(DisabledType::DisabledEmp)
                            || obj_guard.is_disabled_by_type(DisabledType::DisabledSubdued)
                            || obj_guard.is_disabled_by_type(DisabledType::DisabledHacked)
                        {
                            self.start_decay_frame = now;
                        }
                    }
                }
            }

            let end_decay_frame = self.start_decay_frame + data.width_grow_frames;
            let orbital_birth_frame = self.start_attack_frame + data.beam_travel_frames;
            let orbital_decay_start = self.start_decay_frame + data.beam_travel_frames;
            let orbital_death_frame = orbital_decay_start + data.width_grow_frames;

            // State Machine for Laser Status
            match self.laser_status {
                LaserStatus::None => {
                    if orbital_birth_frame <= now {
                        // Create Beam!
                        self.create_orbit_to_target_laser(data.width_grow_frames);
                        self.laser_status = LaserStatus::Born;
                        self.scorch_marks_made = 0;
                        self.next_scorch_mark_frame = now;
                        self.damage_pulses_made = 0;
                        self.next_damage_pulse_frame = now;
                    }
                }
                LaserStatus::Born => {
                    if orbital_decay_start <= now {
                        // Start decay animation on beam
                        self.laser_status = LaserStatus::Decaying;
                    }
                }
                LaserStatus::Decaying => {
                    if orbital_death_frame <= now {
                        // Destroy beam
                        if let Some(client) = TheGameClient::get() {
                            if self.orbit_to_target_beam_id != INVALID_DRAWABLE_ID {
                                client.destroy_drawable(self.orbit_to_target_beam_id);
                            }
                        }
                        self.orbit_to_target_beam_id = INVALID_DRAWABLE_ID;
                        self.laser_status = LaserStatus::Dead;
                        self.start_attack_frame = 0;
                        self.set_logical_status(PUCStatus::Idle);
                    }
                }
                LaserStatus::Dead => {}
            }

            // Beam Steering Logic (The "S" Curve or Manual Drive)
            if self.laser_status != LaserStatus::Dead
                && orbital_birth_frame <= now
                && now <= orbital_death_frame
            {
                if !self.manual_target_mode && !self.scripted_waypoint_mode {
                    // AI / Auto Control: "S" Curve Logic
                    let factor = (now.saturating_sub(orbital_birth_frame) as Real)
                        / ((orbital_death_frame.saturating_sub(orbital_birth_frame)) as Real)
                            .max(1.0);

                    // We're generating a swath that travels the points between sin( -1PI ) and sin( 1PI )
                    let radians = (factor * std::f32::consts::TAU) - std::f32::consts::PI;
                    // cx is cartesian x
                    let cx_distance = (factor * data.swath_of_death_distance)
                        - (data.swath_of_death_distance * 0.5);

                    // Now calculate the amplitude value.
                    let height = radians.sin();
                    let cx_height = height * data.swath_of_death_amplitude;

                    // Calculate vector from building to initial target
                    let building_pos = *self
                        .object
                        .upgrade()
                        .unwrap()
                        .read()
                        .unwrap()
                        .get_position();
                    let building_to_initial_target_vector = (
                        self.initial_target_position.x - building_pos.x,
                        self.initial_target_position.y - building_pos.y,
                    );

                    let target_distance = (building_to_initial_target_vector.0.powi(2)
                        + building_to_initial_target_vector.1.powi(2))
                    .sqrt();

                    // Calculate the point position assuming the target position is on the x axis relative to the building.
                    let current_target_local = (cx_distance + target_distance, cx_height, 0.0);
                    let _target_distance_local =
                        (current_target_local.0.powi(2) + current_target_local.1.powi(2)).sqrt();

                    // Rotate that offset so it's aligned along the building -> target vector.
                    let vector_len_sq = building_to_initial_target_vector.0.powi(2)
                        + building_to_initial_target_vector.1.powi(2);
                    let inv_len = if vector_len_sq > 0.0001 {
                        1.0 / vector_len_sq.sqrt()
                    } else {
                        0.0
                    };
                    let building_to_target_normalized = (
                        building_to_initial_target_vector.0 * inv_len,
                        building_to_initial_target_vector.1 * inv_len,
                    );

                    let _cartesian_target_normalized = (1.0, 0.0); // Simplified relative to X-axis assumption

                    // Rotation matrix from X-axis to Building->Target vector
                    // Simple 2D rotation:
                    // x' = x cos θ - y sin θ
                    // y' = x sin θ + y cos θ
                    // cos θ = building_to_target_normalized.x
                    // sin θ = building_to_target_normalized.y

                    self.current_target_position.x = building_pos.x
                        + (current_target_local.0 * building_to_target_normalized.0
                            - current_target_local.1 * building_to_target_normalized.1);
                    self.current_target_position.y = building_pos.y
                        + (current_target_local.0 * building_to_target_normalized.1
                            + current_target_local.1 * building_to_target_normalized.0);
                    self.current_target_position.z = 0.0; // Will be set to ground height later
                } else {
                    // Manual / Waypoint Control
                    let mut speed = data.manual_driving_speed;

                    if self.scripted_waypoint_mode
                        || (self
                            .last_driving_click_frame
                            .saturating_sub(self.second_last_driving_click_frame)
                            < data.double_click_to_fast_drive_delay)
                    {
                        speed = data.manual_fast_driving_speed;
                    }

                    // Convert speed to speed per frame
                    speed /= LOGICFRAMES_PER_SECOND as Real;

                    // Calculate distance to dest
                    let dx = self.override_target_destination.x - self.current_target_position.x;
                    let dy = self.override_target_destination.y - self.current_target_position.y;
                    let dz = self.override_target_destination.z - self.current_target_position.z;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                    if dist < speed {
                        speed = dist;
                        // Handle waypoint advance if scripted here...
                    }

                    if dist > 0.001 {
                        let scale = speed / dist;
                        self.current_target_position.x += dx * scale;
                        self.current_target_position.y += dy * scale;
                        self.current_target_position.z += dz * scale;
                    }
                }

                if let Some(terrain) = TheTerrainLogic::get() {
                    self.current_target_position.z = terrain.get_ground_height(
                        self.current_target_position.x,
                        self.current_target_position.y,
                        None,
                    );
                }

                if self.orbit_to_target_beam_id != INVALID_DRAWABLE_ID {
                    if let Some(client) = TheGameClient::get() {
                        let mut orbit = self.current_target_position;
                        orbit.z += 500.0;
                        client.set_drawable_beam(
                            self.orbit_to_target_beam_id,
                            &orbit,
                            &self.current_target_position,
                        );
                    }
                }

                let mut width_scalar = 1.0;
                if data.width_grow_frames > 0 {
                    if now < orbital_birth_frame.saturating_add(data.width_grow_frames) {
                        let span = data.width_grow_frames as Real;
                        let elapsed = now.saturating_sub(orbital_birth_frame) as Real;
                        width_scalar = (elapsed / span).clamp(0.0, 1.0);
                    } else if now >= orbital_decay_start {
                        let span = data.width_grow_frames as Real;
                        let elapsed = now.saturating_sub(orbital_decay_start) as Real;
                        width_scalar = (1.0 - (elapsed / span)).clamp(0.0, 1.0);
                    }
                }

                let mut base_laser_radius = 1.0;
                if self.orbit_to_target_beam_id != INVALID_DRAWABLE_ID {
                    if let Some(client) = TheGameClient::get() {
                        if let Some(width) =
                            client.get_drawable_beam_width(self.orbit_to_target_beam_id)
                        {
                            base_laser_radius = width * width_scalar;
                        }
                    }
                }
                let scorch_radius = base_laser_radius * data.scorch_mark_scalar;
                let damage_radius = base_laser_radius * data.damage_radius_scalar;

                // Update Scorch Marks
                if self.next_scorch_mark_frame <= now {
                    self.scorch_marks_made += 1;
                    if let Some(client) = TheGameClient::get() {
                        let scorch_id = game_client_random_value(SCORCH_1, SCORCH_4);
                        client.add_scorch(&self.current_target_position, scorch_radius, scorch_id);
                    }

                    if data.total_scorch_marks > 0 {
                        let next_factor =
                            self.scorch_marks_made as Real / data.total_scorch_marks as Real;
                        let duration =
                            orbital_death_frame.saturating_sub(orbital_birth_frame) as Real;
                        self.next_scorch_mark_frame =
                            orbital_birth_frame + (next_factor * duration) as UnsignedInt;
                    }

                    if !data.ground_hit_fx_name.is_empty() {
                        if let Some(fx) =
                            TheFXListStore::lookup_fx_list(data.ground_hit_fx_name.as_str())
                        {
                            let _ = fx.do_fx_at_position(&self.current_target_position);
                        }
                    }

                    if let Some(object_arc) = self.object.upgrade() {
                        if let Ok(obj_guard) = object_arc.read() {
                            if let Some(player) = obj_guard.get_controlling_player() {
                                if let Ok(player_guard) = player.read() {
                                    let mask = player_guard.get_player_mask().bits();
                                    if let Ok(mut shroud) = get_shroud_manager().lock() {
                                        shroud.do_shroud_reveal(
                                            &self.current_target_position,
                                            data.reveal_range,
                                            mask,
                                        );
                                        shroud.undo_shroud_reveal(
                                            &self.current_target_position,
                                            data.reveal_range,
                                            mask,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // Update Damage Pulses
                if self.next_damage_pulse_frame <= now {
                    self.damage_pulses_made += 1;

                    let total_firing_seconds =
                        data.total_firing_frames as Real / LOGICFRAMES_PER_SECOND as Real;
                    let damage_per_pulse = if data.total_damage_pulses > 0 {
                        (total_firing_seconds * data.damage_per_second)
                            / data.total_damage_pulses as Real
                    } else {
                        0.0
                    };

                    if let Some(object_arc) = self.object.upgrade() {
                        let (source_id, source_mask) = if let Ok(obj_guard) = object_arc.read() {
                            let mask = obj_guard
                                .get_controlling_player()
                                .and_then(|player| player.read().ok().map(|g| g.get_player_mask()))
                                .unwrap_or(PlayerMaskType::none());
                            (obj_guard.get_id(), mask)
                        } else {
                            (crate::common::INVALID_ID, PlayerMaskType::none())
                        };

                        let mut damage_info = DamageInfo::with_simple(
                            damage_per_pulse,
                            source_id,
                            data.damage_type.into(),
                            data.death_type.into(),
                        );
                        damage_info.input.source_player_mask = source_mask;
                        damage_info.sync_from_input();

                        if let Some(partition) = ThePartitionManager::get() {
                            for id in partition
                                .get_objects_in_range(&self.current_target_position, damage_radius)
                            {
                                if let Some(target_arc) = TheGameLogic::find_object_by_id(id) {
                                    if let Ok(mut target) = target_arc.write() {
                                        if target.is_effectively_dead() {
                                            continue;
                                        }
                                        let _ = target.attempt_damage(&mut damage_info);
                                    }
                                }
                            }
                        }
                    }

                    if data.total_damage_pulses > 0 {
                        let next_factor =
                            self.damage_pulses_made as Real / data.total_damage_pulses as Real;
                        // orbital_birth_frame + nextFactor * (orbitalDeathFrame - orbitalBirthFrame);
                        let duration =
                            orbital_death_frame.saturating_sub(orbital_birth_frame) as Real;
                        self.next_damage_pulse_frame =
                            orbital_birth_frame + (next_factor * duration) as UnsignedInt;
                    }
                }
            }

            // Status Update based on frames
            if end_decay_frame <= now {
                self.set_logical_status(PUCStatus::Packing);
            } else if self.start_decay_frame <= now {
                self.set_logical_status(PUCStatus::PostFire);
            } else {
                self.set_logical_status(PUCStatus::Firing);
            }
        } else if ready_to_fire_frame <= now {
            self.set_logical_status(PUCStatus::ReadyToFire);
        } else if almost_ready_frame <= now {
            self.set_logical_status(PUCStatus::AlmostReady);
        } else if raise_antenna_frame <= now {
            self.set_logical_status(PUCStatus::Preparing);
        } else if begin_charge_frame <= now {
            self.set_logical_status(PUCStatus::Charging);
        }

        // Firing Effects
        if self.status == PUCStatus::Firing {
            if self.next_launch_fx_frame <= now {
                if !data.beam_launch_fx_name.is_empty() {
                    if let Some(fx) =
                        TheFXListStore::lookup_fx_list(data.beam_launch_fx_name.as_str())
                    {
                        let _ = fx.do_fx_at_position(&self.laser_origin_position);
                    }
                }
                self.next_launch_fx_frame = now + data.frames_between_launch_fx_refresh;
            }
        }

        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(obj_guard) = obj_arc.read() {
                let local_index = ThePlayerList()
                    .read()
                    .ok()
                    .map(|list| list.get_local_player_index())
                    .unwrap_or(-1);
                let shrouded =
                    obj_guard.get_shrouded_status(local_index) != ObjectShroudStatus::Clear;
                if shrouded {
                    self.remove_all_effects();
                } else {
                    let reveal_this_frame = self.client_shrouded_last_frame != shrouded;
                    if reveal_this_frame {
                        self.set_client_status(self.status, reveal_this_frame);
                    }
                }
                self.client_shrouded_last_frame = shrouded;
            }
        }

        UpdateSleepTime::None
    }
}

impl SpecialPowerUpdateInterface for ParticleUplinkCannonUpdate {
    fn initiate_intent_to_do_special_power(
        &mut self,
        special_power_template: &SpecialPowerTemplate,
        target_obj: Option<ObjectId>,
        target_pos: Option<&Coord3D>,
        waypoint: Option<&Waypoint>,
        command_options: SpecialPowerCommandOptions,
    ) -> Bool {
        if let Some(template) = &self.module_data.special_power_template {
            if !std::ptr::eq(template.as_ref(), special_power_template) {
                return false;
            }
        } else {
            return false;
        }

        let now = TheGameLogic::get_frame();
        let data = self.module_data.clone();

        if !command_options.contains(SpecialPowerCommandOptions::COMMAND_FIRED_BY_SCRIPT) {
            if let Some(pos) = target_pos {
                self.start_attack_frame = now;
                self.laser_status = LaserStatus::None;
                self.manual_target_mode = true;
                self.scripted_waypoint_mode = false;
                self.initial_target_position = *pos;
                self.override_target_destination = *pos;
                self.current_target_position = *pos;
            }
        } else if let Some(way) = waypoint {
            let pos = way.position;
            self.start_attack_frame = now.max(1);
            self.scripted_waypoint_mode = true;
            self.manual_target_mode = false;
            self.laser_status = LaserStatus::None;
            self.set_logical_status(PUCStatus::ReadyToFire);
            let _ = self.with_special_power_module(|module| module.set_ready_frame(now));
            self.initial_target_position = pos;
            self.current_target_position = pos;
            self.override_target_destination = pos;
        } else {
            let mut pos = Coord3D::ZERO;
            if let Some(pos_ref) = target_pos {
                pos = *pos_ref;
            } else if let Some(target_id) = target_obj {
                if let Some(obj_arc) = TheGameLogic::find_object_by_id(target_id) {
                    if let Ok(obj_guard) = obj_arc.read() {
                        pos = *obj_guard.get_position();
                    }
                }
            }
            self.initial_target_position = pos;
            self.start_attack_frame = now.max(1);
            self.laser_status = LaserStatus::None;
            self.manual_target_mode = false;
            self.scripted_waypoint_mode = false;
            self.set_logical_status(PUCStatus::ReadyToFire);
            let _ = self.with_special_power_module(|module| module.set_ready_frame(now));
        }

        if self.start_attack_frame != 0 {
            self.start_decay_frame = self.start_attack_frame + data.total_firing_frames;
        }

        let marker_pos = self.initial_target_position;
        let _ = self.with_special_power_module(|module| {
            module.mark_special_power_triggered(Some(&marker_pos));
        });

        true
    }

    fn is_special_ability(&self) -> Bool {
        false
    }

    fn is_special_power(&self) -> Bool {
        true
    }

    fn is_active(&self) -> Bool {
        self.status != PUCStatus::Idle
    }

    fn get_command_option(&self) -> crate::modules::SpecialPowerCommandOptions {
        crate::modules::SpecialPowerCommandOptions::NONE
    }

    fn is_power_currently_in_use(&self, _command: Option<&CommandButton>) -> Bool {
        self.start_attack_frame != 0 && self.start_attack_frame <= TheGameLogic::get_frame()
    }

    fn does_special_power_have_overridable_destination_active(&self) -> Bool {
        matches!(
            self.status,
            PUCStatus::PreFire | PUCStatus::Firing | PUCStatus::PostFire
        )
    }

    fn does_special_power_have_overridable_destination(&self) -> Bool {
        true
    }

    fn set_special_power_overridable_destination(&mut self, loc: &Coord3D) {
        let Some(object_arc) = self.object.upgrade() else {
            return;
        };
        let Ok(obj_guard) = object_arc.read() else {
            return;
        };
        if obj_guard.is_disabled() {
            return;
        }
        self.override_target_destination = *loc;
        self.manual_target_mode = true;
        self.second_last_driving_click_frame = self.last_driving_click_frame;
        self.last_driving_click_frame = TheGameLogic::get_frame();
    }
}

impl BehaviorModuleInterface for ParticleUplinkCannonUpdate {
    fn get_module_name(&self) -> &'static str {
        "ParticleUplinkCannonUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_special_power_update_interface(
        &mut self,
    ) -> Option<&mut dyn SpecialPowerUpdateInterface> {
        Some(self)
    }
    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Module lookup is resolved lazily through Object::with_special_power_module_mut_by_name.
        Ok(())
    }
}

pub struct ParticleUplinkCannonUpdateFactory;
impl ParticleUplinkCannonUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(ParticleUplinkCannonUpdate::new(
            thing,
            module_data,
        )?))
    }
}

pub struct ParticleUplinkCannonUpdateModule {
    behavior: ParticleUplinkCannonUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<ParticleUplinkCannonUpdateModuleData>,
}

impl ParticleUplinkCannonUpdateModule {
    pub fn new(
        behavior: ParticleUplinkCannonUpdate,
        module_name: &AsciiString,
        module_data: Arc<ParticleUplinkCannonUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut ParticleUplinkCannonUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for ParticleUplinkCannonUpdateModule {
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

impl Module for ParticleUplinkCannonUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}
