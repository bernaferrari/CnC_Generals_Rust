//! StealthDetectorUpdate - Rust conversion of C++ StealthDetectorUpdate
//!
//! Update module that detects stealthed units within a radius.
//! Author: Steven Johnson, May 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Bool, Coord3D, KindOf, KindOfMaskType, ModuleData, NameKeyType,
    ObjectShroudStatus, ObjectStatusTypes, ParticleSystemID, Real, UnsignedInt, XferVersion,
    ALL_KIND_OF,
};
use crate::helpers::{TheAudio, TheParticleSystemManager, ThePartitionManager};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::{Object as GameObject, ObjectID, INVALID_ID as OBJECT_INVALID_ID};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::Thing as ModuleThing;
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType as EngineNameKeyType,
    StealthDetectorControlInterface,
};
use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

#[allow(dead_code)]
const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
#[allow(dead_code)]
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

/// Module data for StealthDetectorUpdate
#[derive(Clone, Debug)]
pub struct StealthDetectorUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub update_rate: UnsignedInt,
    pub detection_range: Real,
    pub initially_disabled: Bool,
    pub ping_sound: Option<String>,
    pub loud_ping_sound: Option<String>,
    pub ir_beacon_particle_sys: Option<String>,
    pub ir_particle_sys: Option<String>,
    pub ir_bright_particle_sys: Option<String>,
    pub ir_grid_particle_sys: Option<String>,
    pub ir_particle_sys_bone: String,
    pub extra_detect_kindof: KindOfMaskType,
    pub extra_detect_kindof_not: KindOfMaskType,
    pub can_detect_while_garrisoned: Bool,
    pub can_detect_while_transported: Bool,
}

impl Default for StealthDetectorUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            update_rate: 1,
            detection_range: 0.0,
            initially_disabled: false,
            ping_sound: None,
            loud_ping_sound: None,
            ir_beacon_particle_sys: None,
            ir_particle_sys: None,
            ir_bright_particle_sys: None,
            ir_grid_particle_sys: None,
            ir_particle_sys_bone: String::new(),
            extra_detect_kindof: 0,
            extra_detect_kindof_not: 0,
            can_detect_while_garrisoned: false,
            can_detect_while_transported: false,
        }
    }
}

impl crate::common::LegacyModuleData for StealthDetectorUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for StealthDetectorUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.update_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.detection_range)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.initially_disabled)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.extra_detect_kindof)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.extra_detect_kindof_not)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.can_detect_while_garrisoned)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.can_detect_while_transported)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl EngineModuleData for StealthDetectorUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: EngineNameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> EngineNameKeyType {
        self.module_tag_name_key
    }
}

impl crate::common::types::ModuleData for StealthDetectorUpdateModuleData {}

impl StealthDetectorUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STEALTH_DETECTOR_UPDATE_FIELDS)
    }
}

fn parse_duration_field(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_real_field(tokens: &[&str]) -> Result<Real, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_bool_field(tokens: &[&str]) -> Result<Bool, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    INI::parse_bool(token)
}

fn parse_optional_string(tokens: &[&str]) -> Result<Option<String>, INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        return Ok(None);
    }
    Ok(Some(token.to_string()))
}

fn parse_kindof_mask(tokens: &[&str]) -> KindOfMaskType {
    crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens)
}

fn parse_detection_rate(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.update_rate = parse_duration_field(tokens)?;
    Ok(())
}

fn parse_detection_range(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.detection_range = parse_real_field(tokens)?;
    Ok(())
}

fn parse_initially_disabled(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.initially_disabled = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_ping_sound(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ping_sound = parse_optional_string(tokens)?;
    Ok(())
}

fn parse_loud_ping_sound(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.loud_ping_sound = parse_optional_string(tokens)?;
    Ok(())
}

fn parse_ir_beacon_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ir_beacon_particle_sys = parse_optional_string(tokens)?;
    Ok(())
}

fn parse_ir_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ir_particle_sys = parse_optional_string(tokens)?;
    Ok(())
}

fn parse_ir_bright_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ir_bright_particle_sys = parse_optional_string(tokens)?;
    Ok(())
}

fn parse_ir_grid_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ir_grid_particle_sys = parse_optional_string(tokens)?;
    Ok(())
}

fn parse_ir_particle_bone(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ir_particle_sys_bone = token.to_string();
    Ok(())
}

fn parse_extra_required_kindof(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.extra_detect_kindof = parse_kindof_mask(tokens);
    Ok(())
}

fn parse_extra_forbidden_kindof(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.extra_detect_kindof_not = parse_kindof_mask(tokens);
    Ok(())
}

fn parse_detect_while_garrisoned(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.can_detect_while_garrisoned = parse_bool_field(tokens)?;
    Ok(())
}

fn parse_detect_while_transported(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.can_detect_while_transported = parse_bool_field(tokens)?;
    Ok(())
}

const STEALTH_DETECTOR_UPDATE_FIELDS: &[FieldParse<StealthDetectorUpdateModuleData>] = &[
    FieldParse {
        token: "DetectionRate",
        parse: parse_detection_rate,
    },
    FieldParse {
        token: "DetectionRange",
        parse: parse_detection_range,
    },
    FieldParse {
        token: "InitiallyDisabled",
        parse: parse_initially_disabled,
    },
    FieldParse {
        token: "PingSound",
        parse: parse_ping_sound,
    },
    FieldParse {
        token: "LoudPingSound",
        parse: parse_loud_ping_sound,
    },
    FieldParse {
        token: "IRBeaconParticleSysName",
        parse: parse_ir_beacon_particle,
    },
    FieldParse {
        token: "IRParticleSysName",
        parse: parse_ir_particle,
    },
    FieldParse {
        token: "IRBrightParticleSysName",
        parse: parse_ir_bright_particle,
    },
    FieldParse {
        token: "IRGridParticleSysName",
        parse: parse_ir_grid_particle,
    },
    FieldParse {
        token: "IRParticleSysBone",
        parse: parse_ir_particle_bone,
    },
    FieldParse {
        token: "ExtraRequiredKindOf",
        parse: parse_extra_required_kindof,
    },
    FieldParse {
        token: "ExtraForbiddenKindOf",
        parse: parse_extra_forbidden_kindof,
    },
    FieldParse {
        token: "CanDetectWhileGarrisoned",
        parse: parse_detect_while_garrisoned,
    },
    FieldParse {
        token: "CanDetectWhileContained",
        parse: parse_detect_while_transported,
    },
];

/// StealthDetectorUpdate behavior module
pub struct StealthDetectorUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<StealthDetectorUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    enabled: Bool,
    grid_particle_ids: Vec<ParticleSystemID>,
    ping_particle_id: Option<ParticleSystemID>,
    beacon_particle_id: Option<ParticleSystemID>,
}

impl StealthDetectorUpdate {
    /// Create a new StealthDetectorUpdate instance
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<StealthDetectorUpdateModuleData>()
            .ok_or("Invalid module data type for StealthDetectorUpdate")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            enabled: !specific_data.initially_disabled,
            grid_particle_ids: Vec::new(),
            ping_particle_id: None,
            beacon_particle_id: None,
        })
    }

    /// Check if detector is enabled
    pub fn is_enabled(&self) -> Bool {
        self.enabled
    }

    /// Set detector enabled state
    pub fn set_enabled(&mut self, enabled: Bool) {
        self.enabled = enabled;
    }

    fn clear_grid_particles(&mut self) {
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            for system_id in self.grid_particle_ids.drain(..) {
                ps_manager.destroy_particle_system(system_id);
            }
        } else {
            self.grid_particle_ids.clear();
        }
    }

    fn clear_ping_beacon_particles(&mut self) {
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            if let Some(system_id) = self.ping_particle_id.take() {
                ps_manager.destroy_particle_system(system_id);
            }
            if let Some(system_id) = self.beacon_particle_id.take() {
                ps_manager.destroy_particle_system(system_id);
            }
        } else {
            self.ping_particle_id = None;
            self.beacon_particle_id = None;
        }
    }

    fn mask_contains_kind(mask: KindOfMaskType, kind: KindOf) -> bool {
        (mask & (1u64 << (kind as u32))) != 0
    }

    fn passes_kindof_filters(
        &self,
        obj: &GameObject,
        required_mask: KindOfMaskType,
        forbidden_mask: KindOfMaskType,
    ) -> bool {
        if required_mask != 0 {
            for kind in ALL_KIND_OF {
                if Self::mask_contains_kind(required_mask, *kind) && !obj.is_kind_of(*kind) {
                    return false;
                }
            }
        }

        if forbidden_mask != 0 {
            for kind in ALL_KIND_OF {
                if Self::mask_contains_kind(forbidden_mask, *kind) && obj.is_kind_of(*kind) {
                    return false;
                }
            }
        }

        true
    }

    /// Perform detection scan
    /// Matches C++ StealthDetectorUpdate.cpp lines 164-335
    fn perform_detection_scan(&mut self) -> Bool {
        if !self.enabled {
            return false;
        }

        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                let self_id = obj.get_id();
                // Get object position (C++ line 179)
                let position = *obj.get_position();
                let self_team_id = obj.get_team_id();

                // Use detection range or vision range (C++ lines 172-176)
                let vision_range = if self.module_data.detection_range > 0.0 {
                    self.module_data.detection_range
                } else {
                    obj.get_vision_range()
                };

                // Query nearby objects within detection_range using partition manager
                // C++ lines 179-181: ThePartitionManager->iterateObjectsInRange
                let nearby_objects = ThePartitionManager::get()
                    .map(|p| p.get_objects_in_range(&position, vision_range))
                    .unwrap_or_default();
                let mut found_someone = false;

                // For each nearby object (C++ lines 182-335)
                for obj_id in nearby_objects {
                    if let Some(target_obj) =
                        crate::object::registry::OBJECT_REGISTRY.get_object(obj_id)
                    {
                        if let Ok(target) = target_obj.read() {
                            if obj_id == self_id {
                                continue;
                            }

                            // Check if effectively dead (C++ lines 184-185)
                            if target.is_effectively_dead() {
                                continue;
                            }

                            if obj.is_off_map() != target.is_off_map() {
                                continue;
                            }

                            let relationship = obj.relationship_to(&target);
                            if !matches!(
                                relationship,
                                crate::common::Relationship::Enemies
                                    | crate::common::Relationship::Neutral
                            ) {
                                continue;
                            }

                            // Apply KindOf filters (C++ line 168)
                            if !self.passes_kindof_filters(
                                &target,
                                self.module_data.extra_detect_kindof,
                                self.module_data.extra_detect_kindof_not,
                            ) {
                                continue;
                            }

                            let target_contained = target.get_container().is_some();
                            if target_contained
                                && !(self.module_data.can_detect_while_garrisoned
                                    || self.module_data.can_detect_while_transported)
                            {
                                continue;
                            }

                            // Check if stealthed (C++ line 187)
                            if target.is_stealthed() {
                                let distance = (*target.get_position() - position).length();
                                if distance > vision_range {
                                    continue;
                                }

                                found_someone = true;

                                if let Some(stealth_module) = target.get_stealth_module() {
                                    drop(target);
                                    if let Ok(mut stealth_guard) = stealth_module.lock() {
                                        stealth_guard.mark_as_detected();
                                    }
                                } else {
                                    drop(target);
                                }

                                if let Some(template_name) =
                                    self.module_data.ir_grid_particle_sys.as_ref()
                                {
                                    if let Some(ps_manager) = TheParticleSystemManager::get() {
                                        if let Some(system_id) = ps_manager
                                            .create_particle_system(Some(template_name.as_str()))
                                        {
                                            let mut grid_pos = position;
                                            if let Some(target_obj) =
                                                crate::object::registry::OBJECT_REGISTRY
                                                    .get_object(obj_id)
                                            {
                                                if let Ok(target_guard) = target_obj.read() {
                                                    grid_pos = *target_guard.get_position();
                                                }
                                            }
                                            grid_pos.z = position.z + 17.0;
                                            let ix = grid_pos.x as i32;
                                            let iy = grid_pos.y as i32;
                                            grid_pos.x -= (ix % 12) as f32;
                                            grid_pos.y -= (iy % 12) as f32;
                                            ps_manager
                                                .set_particle_system_position(system_id, &grid_pos);
                                            self.grid_particle_ids.push(system_id);
                                        }
                                    }
                                }

                                continue;
                            }

                            // Check if container holds stealthed units
                            if let Some(contain) = target.get_contain() {
                                drop(target);
                                if let Ok(contain_guard) = contain.lock() {
                                    for &rider_id in contain_guard.get_contained_objects() {
                                        if let Some(rider_obj) =
                                            crate::object::registry::OBJECT_REGISTRY
                                                .get_object(rider_id)
                                        {
                                            if let Ok(rider_guard) = rider_obj.read() {
                                                if rider_guard.is_stealthed()
                                                    && rider_guard.get_team_id() != self_team_id
                                                {
                                                    found_someone = true;
                                                    if let Some(stealth_module) =
                                                        rider_guard.get_stealth_module()
                                                    {
                                                        drop(rider_guard);
                                                        if let Ok(mut stealth_guard) =
                                                            stealth_module.lock()
                                                        {
                                                            stealth_guard.mark_as_detected();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                return found_someone;
            }
        }

        false
    }
}

impl Snapshotable for StealthDetectorUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_bool(&mut self.enabled)
            .map_err(|e| format!("Failed to xfer enabled: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpdateModuleInterface for StealthDetectorUpdate {
    /// Update callback - matches C++ StealthDetectorUpdate::update (lines 123-401)
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.enabled {
            self.clear_grid_particles();
            self.clear_ping_beacon_particles();
            return UpdateSleepTime::Forever;
        }

        if let Some(object) = self.object.upgrade() {
            if let Ok(obj) = object.read() {
                // Check if effectively dead (C++ lines 128-129)
                if obj.is_effectively_dead() {
                    self.clear_grid_particles();
                    self.clear_ping_beacon_particles();
                    return UpdateSleepTime::Forever;
                }

                // Wait until fully constructed (C++ lines 131-133)
                if obj.test_status(ObjectStatusTypes::UnderConstruction) {
                    return UpdateSleepTime::None;
                }

                // Turn off forever when sold (C++ lines 135-137)
                if obj.test_status(ObjectStatusTypes::Sold) {
                    self.clear_grid_particles();
                    self.clear_ping_beacon_particles();
                    return UpdateSleepTime::Forever;
                }

                // Check if contained and whether we can detect while contained (C++ lines 139-162)
                if let Some(contained_by_id) = obj.get_contained_by() {
                    // Get the container object
                    if let Some(container) =
                        crate::object::registry::OBJECT_REGISTRY.get_object(contained_by_id)
                    {
                        if let Ok(_container_obj) = container.read() {
                            // Check if container has contain module
                            // C++ lines 143-161 check if garrisonable or regular transport
                            // For now, we assume we can check the container type
                            //
                            // If garrisonable (C++ lines 147-154)
                            if !self.module_data.can_detect_while_garrisoned {
                                // Can't detect while garrisoned
                                return UpdateSleepTime::from_u32(self.module_data.update_rate);
                            }

                            // If transported (C++ lines 156-160)
                            if !self.module_data.can_detect_while_transported {
                                // Can't detect while transported
                                return UpdateSleepTime::from_u32(self.module_data.update_rate);
                            }
                        }
                    }
                }

                self.clear_grid_particles();
                // Perform the actual detection scan
                let found_someone = self.perform_detection_scan();

                let is_visible = if let Ok(obj_guard) = object.read() {
                    let local_player_index = crate::player::ThePlayerList()
                        .read()
                        .ok()
                        .map(|list| list.get_local_player_index())
                        .unwrap_or(-1);
                    let shroud = obj_guard.get_shrouded_status(local_player_index);
                    (shroud as u8) <= (ObjectShroudStatus::PartialClear as u8)
                } else {
                    false
                };

                if is_visible {
                    self.clear_ping_beacon_particles();
                    let ping_template = if found_someone {
                        &self.module_data.ir_bright_particle_sys
                    } else {
                        &self.module_data.ir_particle_sys
                    };

                    if let Some(template_name) = ping_template.as_ref() {
                        if let Some(ps_manager) = TheParticleSystemManager::get() {
                            if let Some(system_id) =
                                ps_manager.create_particle_system(Some(template_name.as_str()))
                            {
                                let mut ping_pos = *obj.get_position();
                                if !self.module_data.ir_particle_sys_bone.is_empty() {
                                    if let Some(drawable) = obj.get_drawable() {
                                        if let Ok(drawable_guard) = drawable.read() {
                                            if let Some(bone_matrix) = drawable_guard
                                                .get_current_worldspace_client_bone_positions(
                                                    &self.module_data.ir_particle_sys_bone,
                                                )
                                            {
                                                let translation = bone_matrix.w_axis;
                                                ping_pos = Coord3D::new(
                                                    translation.x,
                                                    translation.y,
                                                    translation.z,
                                                );
                                            }
                                        }
                                    }
                                }
                                ps_manager.set_particle_system_position(system_id, &ping_pos);
                                ps_manager
                                    .attach_particle_system_to_object(system_id, obj.get_id());
                                self.ping_particle_id = Some(system_id);
                            }
                        }
                    }

                    if let Some(template_name) = self.module_data.ir_beacon_particle_sys.as_ref() {
                        if let Some(ps_manager) = TheParticleSystemManager::get() {
                            if let Some(system_id) =
                                ps_manager.create_particle_system(Some(template_name.as_str()))
                            {
                                ps_manager
                                    .set_particle_system_position(system_id, obj.get_position());
                                ps_manager
                                    .attach_particle_system_to_object(system_id, obj.get_id());
                                self.beacon_particle_id = Some(system_id);
                            }
                        }
                    }

                    let ping_sound = if found_someone {
                        &self.module_data.loud_ping_sound
                    } else {
                        &self.module_data.ping_sound
                    };

                    if let Some(sound_name) = ping_sound.as_ref() {
                        if let Some(audio) = TheAudio::get() {
                            let mut event =
                                crate::common::audio::AudioEventRts::new(sound_name.clone());
                            event.set_object_id(obj.get_id());
                            audio.add_audio_event(&event);
                        }
                    }
                } else {
                    self.clear_ping_beacon_particles();
                }

                // Return sleep time for next update (C++ line 400)
                return UpdateSleepTime::from_u32(self.module_data.update_rate);
            }
        }

        UpdateSleepTime::Forever
    }
}

impl BehaviorModuleInterface for StealthDetectorUpdate {
    fn get_module_name(&self) -> &'static str {
        "StealthDetectorUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl StealthDetectorControlInterface for StealthDetectorUpdate {
    fn set_sd_enabled(&mut self, enabled: bool) {
        self.set_enabled(enabled);
    }
}

/// Glue that exposes StealthDetectorUpdate through the common Module trait.
pub struct StealthDetectorUpdateModule {
    behavior: StealthDetectorUpdate,
    module_name_key: EngineNameKeyType,
    module_data: Arc<StealthDetectorUpdateModuleData>,
}

impl StealthDetectorUpdateModule {
    pub fn new(
        behavior: StealthDetectorUpdate,
        module_name: &AsciiString,
        module_data: Arc<StealthDetectorUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut StealthDetectorUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for StealthDetectorUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for StealthDetectorUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> EngineNameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> EngineNameKeyType {
        EngineModuleData::get_module_tag_name_key(self.module_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn get_stealth_detector_control_interface(
        &mut self,
    ) -> Option<&mut dyn StealthDetectorControlInterface> {
        Some(&mut self.behavior)
    }
}

// Factory for creating StealthDetectorUpdate instances
pub struct StealthDetectorUpdateFactory;

impl StealthDetectorUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let behavior = StealthDetectorUpdate::new(thing, module_data)?;
        Ok(Box::new(behavior))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_detector_creation() {
        let data = StealthDetectorUpdateModuleData::default();
        assert_eq!(data.update_rate, 1);
        assert_eq!(data.detection_range, 0.0);
        assert!(!data.initially_disabled);
    }
}
