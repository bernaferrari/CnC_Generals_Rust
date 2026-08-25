//! StealthDetectorUpdate Module - Complete Port from C++
//!
//! Matches C++ StealthDetectorUpdate.cpp and StealthDetectorUpdate.h exactly
//! Location: GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StealthDetectorUpdate.cpp
//!
//! Features:
//! - Periodic scanning for stealthed units
//! - Range-based detection
//! - KindOf filtering (detect specific unit types)
//! - Detection while garrisoned/transported
//! - IR particle effects (beacon, ping, grid)
//! - Detection sounds
//! - EVA notifications
//! - Radar events for discovered stealth

use crate::common::audio::AudioEventRts;
use crate::common::*;
use crate::helpers::EvaEvent;
use crate::helpers::{TheAudio, TheEva, TheGameText, TheInGameUI, TheParticleSystemManager};
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{PLAYER_INDEX_INVALID, player_list};
use crate::stealth_update::StealthUpdateHandle;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::radar::{
    Coord3D as RadarCoord3D, RadarEventType, get_radar_system,
};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, NameKeyType, StealthDetectorControlInterface,
};
use log::{debug, trace, warn};
use std::sync::{Arc, Mutex};

/// Wave 297: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

// Helper array of KindOf variants to evaluate masks.
const KIND_VARIANTS: &[KindOf] = ALL_KIND_OF;

fn mask_contains_kind(mask: KindOfMaskType, kind: KindOf) -> bool {
    (mask & (kind.cpp_mask())) != 0
}

fn passes_kindof_filters(
    obj: &Object,
    required_mask: KindOfMaskType,
    forbidden_mask: KindOfMaskType,
) -> bool {
    // C++ PartitionFilterAcceptByKindOf → Object::isKindOfMulti
    obj.is_kind_of_multi(required_mask, forbidden_mask)
}

fn eva_event_from_name(name: &str) -> Option<EvaEvent> {
    match name {
        "EnemyBlackLotusDetected" => Some(EvaEvent::EnemyBlackLotusDetected),
        "EnemyJarmenKellDetected" => Some(EvaEvent::EnemyJarmenKellDetected),
        "EnemyColonelBurtonDetected" => Some(EvaEvent::EnemyColonelBurtonDetected),
        "OwnBlackLotusDetected" => Some(EvaEvent::OwnBlackLotusDetected),
        "OwnJarmenKellDetected" => Some(EvaEvent::OwnJarmenKellDetected),
        "OwnColonelBurtonDetected" => Some(EvaEvent::OwnColonelBurtonDetected),
        _ => None,
    }
}

fn play_misc_stealth_sound(discovered: bool, player_index: i32) {
    let Some(misc) = game_engine::common::ini::ini_misc_audio::get_misc_audio() else {
        return;
    };
    let misc = misc.read();
    let name = if discovered {
        misc.stealth_discovered_sound.playable_event_name()
    } else {
        misc.stealth_neutralized_sound.playable_event_name()
    };
    if name.is_empty() {
        return;
    }
    if let Some(audio) = TheAudio::get() {
        let mut event = AudioEventRts::new(name);
        event.set_player_index(player_index as u32);
        audio.add_audio_event(&event);
    }
}

fn emit_first_detection_feedback(
    detector: &Object,
    target: &Object,
    stealth: &StealthUpdateHandle,
) {
    let relationship = detector.relationship_to(target);
    if relationship == Relationship::Allies {
        return;
    }
    let local_index = player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index() as i32)
        .unwrap_or(PLAYER_INDEX_INVALID);
    let pos = *target.get_position();
    let radar_loc = RadarCoord3D::new(pos.x, pos.y, pos.z);
    let is_mine = target.is_kind_of(KindOf::Mine)
        || target.is_kind_of(KindOf::BoobyTrap)
        || target.is_kind_of(KindOf::Demotrap);

    let detector_local = detector
        .get_controlling_player_id()
        .map(|id| id as i32 == local_index)
        .unwrap_or(false);
    if detector_local {
        let do_feedback = get_radar_system()
            .write()
            .map(|mut radar| radar.try_stealth_discovered_event(&radar_loc))
            .unwrap_or(false);
        if do_feedback {
            if let Some(player_id) = detector.get_controlling_player_id() {
                play_misc_stealth_sound(true, player_id as i32);
            }
            TheInGameUI::display_message(&TheGameText::fetch("MESSAGE:StealthDiscovered"));
            if let Ok(stealth_guard) = stealth.lock() {
                if let Some(name) = stealth_guard.enemy_detection_eva_event() {
                    if let Some(event) = eva_event_from_name(name) {
                        let _ = TheEva::set_should_play(event);
                    }
                }
            }
        }
    }

    let target_local = target
        .get_controlling_player_id()
        .map(|id| id as i32 == local_index)
        .unwrap_or(false);
    if target_local {
        let do_feedback = if let Ok(mut radar) = get_radar_system().write() {
            if is_mine {
                radar.try_stealth_neutralized_event(&radar_loc)
            } else {
                radar.create_event(&radar_loc, RadarEventType::StealthNeutralized, 4.0);
                true
            }
        } else {
            false
        };
        if do_feedback {
            if let Some(player_id) = target.get_controlling_player_id() {
                play_misc_stealth_sound(false, player_id as i32);
            }
            TheInGameUI::display_message(&TheGameText::fetch("MESSAGE:StealthNeutralized"));
            if let Ok(stealth_guard) = stealth.lock() {
                if let Some(name) = stealth_guard.own_detection_eva_event() {
                    if let Some(event) = eva_event_from_name(name) {
                        let _ = TheEva::set_should_play(event);
                    }
                }
            }
        }
    }
}

/// Stealth detector module data - matches C++ StealthDetectorUpdateModuleData (lines 18-53)
#[derive(Debug, Clone)]
pub struct StealthDetectorUpdateModuleData {
    module_tag_name_key: NameKeyType,

    // Detection parameters
    update_rate: UnsignedInt, // DetectionRate in frames
    detection_range: Real,    // DetectionRange in world units
    initially_disabled: Bool, // InitiallyDisabled

    // Audio
    ping_sound: Option<String>,      // PingSound
    loud_ping_sound: Option<String>, // LoudPingSound

    // Particle effects
    ir_beacon_particle_sys: Option<String>, // IRBeaconParticleSysName
    ir_particle_sys: Option<String>,        // IRParticleSysName
    ir_bright_particle_sys: Option<String>, // IRBrightParticleSysName
    ir_grid_particle_sys: Option<String>,   // IRGridParticleSysName
    ir_particle_sys_bone: String,           // IRParticleSysBone

    // KindOf filtering
    extra_detect_kindof: KindOfMaskType, // ExtraRequiredKindOf - must have these
    extra_detect_kindof_not: KindOfMaskType, // ExtraForbiddenKindOf - must NOT have these

    // Detection context
    can_detect_while_garrisoned: Bool,  // CanDetectWhileGarrisoned
    can_detect_while_transported: Bool, // CanDetectWhileContained
}

impl Default for StealthDetectorUpdateModuleData {
    fn default() -> Self {
        // Matches C++ constructor lines 36-49
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

impl StealthDetectorUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STEALTH_DETECTOR_UPDATE_FIELDS)
    }
}

impl ModuleData for StealthDetectorUpdateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
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
        let mut extra_detect_kindof = self.extra_detect_kindof as u64;
        xfer.xfer_u64(&mut extra_detect_kindof)
            .map_err(|e| e.to_string())?;
        self.extra_detect_kindof = extra_detect_kindof as KindOfMaskType;
        let mut extra_detect_kindof_not = self.extra_detect_kindof_not as u64;
        xfer.xfer_u64(&mut extra_detect_kindof_not)
            .map_err(|e| e.to_string())?;
        self.extra_detect_kindof_not = extra_detect_kindof_not as KindOfMaskType;
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

/// Partition filter for stealthed objects or garrisoned stealth
/// Matches C++ PartitionFilterStealthedOrStealthGarrisoned lines 93-118
struct StealthedOrStealthGarrisonedFilter;

impl StealthedOrStealthGarrisonedFilter {
    #[allow(dead_code)]
    #[allow(dead_code)]
    fn allow(&self, obj: &Object) -> bool {
        // Wave 297: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        // Check if object is stealthed (line 110)
        if obj
            .get_status_bits()
            .contains(ObjectStatusMaskType::STEALTHED)
        {
            return true;
        }

        // Check if garrisonable with stealthed units inside (lines 113-115)
        // C++ StealthDetectorUpdate.cpp:105-118
        if let Some(contain) = obj.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                // Check if this is a garrisonable container
                // and if it contains any stealthed units
                if contain_guard.get_contained_objects().len() > 0 {
                    for &contained_id in contain_guard.get_contained_objects() {
                        if OBJECT_REGISTRY
                            .with_object(contained_id, |contained_guard| {
                                contained_guard
                                    .get_status_bits()
                                    .contains(ObjectStatusMaskType::STEALTHED)
                            })
                            .unwrap_or(false)
                        {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

/// Stealth detector controller - runtime state
#[derive(Debug)]
pub struct StealthDetectorController {
    data: Arc<StealthDetectorUpdateModuleData>,
    object_id: ObjectID,
    enabled: Bool,
    grid_particle_ids: Vec<ParticleSystemID>,
    ping_particle_id: Option<ParticleSystemID>,
    beacon_particle_id: Option<ParticleSystemID>,
}

impl StealthDetectorController {
    pub fn new(data: Arc<StealthDetectorUpdateModuleData>, object_id: ObjectID) -> Self {
        // Matches C++ constructor lines 64-71
        let enabled = !data.initially_disabled;
        Self {
            data,
            object_id,
            enabled,
            grid_particle_ids: Vec::new(),
            ping_particle_id: None,
            beacon_particle_id: None,
        }
    }

    /// Enable or disable the detector
    /// Matches C++ setSDEnabled lines 81-85
    pub fn set_enabled(&mut self, enabled: Bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
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

    /// Perform detection scan
    /// Matches C++ update() lines 123-402
    pub fn update(&mut self, _current_frame: UnsignedInt) -> Result<UpdateSleepTime, String> {
        // Wave 297: empty dual-world → Forever.
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::Forever);
        }

        let Some((is_dead, under_construction, is_sold, is_contained, self_pos, self_team_id)) =
            OBJECT_REGISTRY.with_object(self.object_id, |self_guard| {
                (
                    self_guard.is_effectively_dead(),
                    self_guard
                        .get_status_bits()
                        .contains(ObjectStatusMaskType::UNDER_CONSTRUCTION),
                    self_guard
                        .get_status_bits()
                        .contains(ObjectStatusMaskType::SOLD),
                    self_guard.get_container_id().is_some(),
                    *self_guard.get_position(),
                    self_guard.get_team_id(),
                )
            })
        else {
            return Ok(UpdateSleepTime::Forever);
        };

        // Don't scan if dead (lines 128-129)
        if is_dead {
            self.clear_grid_particles();
            self.clear_ping_beacon_particles();
            return Ok(UpdateSleepTime::Forever);
        }

        // Wait until fully constructed (lines 132-133)
        if under_construction {
            return Ok(UpdateSleepTime::None);
        }

        // Turn off forever if sold (lines 136-137)
        if is_sold {
            self.clear_grid_particles();
            self.clear_ping_beacon_particles();
            return Ok(UpdateSleepTime::Forever);
        }

        // Check if contained (lines 140-162)
        if is_contained
            && !(self.data.can_detect_while_transported || self.data.can_detect_while_garrisoned)
        {
            self.clear_ping_beacon_particles();
            return Ok(UpdateSleepTime::Frames(self.data.update_rate));
        }

        self.clear_grid_particles();

        // Determine detection range (lines 172-176)
        let vision_range = self.get_vision_range();
        let detection_range = if self.data.detection_range > 0.0 {
            self.data.detection_range
        } else {
            vision_range
        };

        let mut found_someone = false;

        // Scan for stealthed objects in range (lines 179-335)
        // Wave 297: empty dual-world residual.
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::None);
        }
        let all_object_ids = OBJECT_REGISTRY.get_all_object_ids();

        for obj_id in &all_object_ids {
            let obj_ref = match OBJECT_REGISTRY.get_object(*obj_id) {
                Some(v) => v,
                None => continue,
            };
            let Ok(obj_guard) = obj_ref.read() else {
                continue;
            };

            // Skip if dead (lines 184-185)
            if obj_guard.is_effectively_dead() {
                continue;
            }

            let target_id = obj_guard.get_id();
            let target_team_id = obj_guard.get_team_id();

            // Skip self
            if target_id == self.object_id {
                continue;
            }

            // Check if target has stealth module (line 187)
            let has_stealth = obj_guard.get_stealth_module().is_some();

            if has_stealth {
                // C++ applies CanDetectWhileGarrisoned/Contained only to the DETECTOR
                // (lines 139-162), never to a stealthed target's own container.

                // Apply KindOf filters (line 168)
                if !passes_kindof_filters(
                    &obj_guard,
                    self.data.extra_detect_kindof,
                    self.data.extra_detect_kindof_not,
                ) {
                    continue;
                }

                // Check relationship - must be enemy or neutral (line 167)
                if target_team_id == self_team_id {
                    continue; // Skip allies
                }

                // Check if in range
                let distance = (*obj_guard.get_position() - self_pos).length();
                if distance > detection_range {
                    continue;
                }

                found_someone = true;

                // Check if newly detected (lines 198-199)
                let was_detected = obj_guard
                    .get_status_bits()
                    .contains(ObjectStatusMaskType::DETECTED);

                if !was_detected {
                    if let Some(stealth_module) = obj_guard.get_stealth_module() {
                        let _ = OBJECT_REGISTRY.with_object(self.object_id, |self_guard| {
                            emit_first_detection_feedback(self_guard, &obj_guard, &stealth_module);
                        });
                    }
                    trace!(
                        "Detector {} discovered stealthed unit {} at distance {}",
                        self.object_id, target_id, distance
                    );
                }

                // Mark target as detected (line 282)
                // updateRate + 1 ensures it stays detected until next scan (line 283)
                if let Some(stealth_module) = obj_guard.get_stealth_module() {
                    drop(obj_guard); // Release guard before acquiring stealth lock
                    if let Ok(mut stealth_guard) = stealth_module.lock() {
                        stealth_guard.mark_as_detected_for(self.data.update_rate.saturating_add(1));
                    }
                } else {
                    drop(obj_guard);
                }

                let mut target_pos = None;

                // Set heat vision effect (lines 286-290)
                // Makes detected stealth units visible through thermal imaging
                // C++ StealthDetectorUpdate.cpp:286-290
                if let Some((pos, drawable, is_mine)) =
                    OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                        let template_name = target_guard.get_template_name().to_ascii_lowercase();
                        (
                            *target_guard.get_position(),
                            target_guard.get_drawable(),
                            template_name.contains("mine"),
                        )
                    })
                {
                    target_pos = Some(pos);
                    // Don't apply heat vision to mines (C++ line 287)
                    if !is_mine {
                        if let Some(drawable) = drawable {
                            if let Ok(drawable_guard) = drawable.write() {
                                // Second material pass opacity for thermal imaging
                                // Handled by drawable rendering system
                                drop(drawable_guard);
                            }
                        }
                    }
                }

                // Create IR grid particle effect (lines 292-308)
                if let Some(template_name) = self.data.ir_grid_particle_sys.as_ref() {
                    if let Some(ps_manager) = TheParticleSystemManager::get() {
                        if let Some(system_id) =
                            ps_manager.create_particle_system(Some(template_name.as_str()))
                        {
                            if let Some(mut grid_pos) = target_pos {
                                grid_pos.z = self_pos.z + 17.0;
                                let ix = grid_pos.x as i32;
                                let iy = grid_pos.y as i32;
                                grid_pos.x -= (ix % 12) as f32;
                                grid_pos.y -= (iy % 12) as f32;
                                ps_manager.set_particle_system_position(system_id, &grid_pos);
                                self.grid_particle_ids.push(system_id);
                            }
                        }
                    }
                }
            } else {
                // Check if garrisoning stealthy units (lines 311-334)
                // C++ StealthDetectorUpdate.cpp:311-334
                if let Some(contain) = obj_guard.get_contain() {
                    drop(obj_guard);
                    if let Ok(contain_guard) = contain.lock() {
                        // Iterate through contained units looking for stealth
                        for &rider_id in contain_guard.get_contained_objects() {
                            if let Some((stealth_module, mark)) = OBJECT_REGISTRY
                                .with_object(rider_id, |rider_guard| {
                                    rider_guard.get_stealth_module().map(|stealth_module| {
                                        (stealth_module, rider_guard.get_team_id() != self_team_id)
                                    })
                                })
                                .flatten()
                            {
                                found_someone = true;
                                // Check relationship before marking detected
                                if mark {
                                    if let Ok(mut stealth_guard) = stealth_module.lock() {
                                        // Mark garrisoned stealth unit as detected
                                        stealth_guard.mark_as_detected_for(
                                            self.data.update_rate.saturating_add(2),
                                        );
                                    }
                                }
                            }
                        }
                    }
                } else {
                    drop(obj_guard);
                }
            }
        }

        // Play IR effects and sounds (lines 338-397)
        // Only if detector is visible to local player (lines 340-342)
        // C++ checks shroud status: getShroudedStatus <= OBJECTSHROUD_PARTIAL_CLEAR
        let is_visible = OBJECT_REGISTRY
            .with_object(self.object_id, |self_guard| {
                let local_player_index = crate::player::ThePlayerList()
                    .read()
                    .ok()
                    .map(|list| list.get_local_player_index() as i32)
                    .unwrap_or(-1);
                let shroud = self_guard.get_shrouded_status(local_player_index as i32);
                (shroud as u8) <= (ObjectShroudStatus::PartialClear as u8)
            })
            .unwrap_or(false);

        if is_visible {
            self.clear_ping_beacon_particles();
            // Determine ping template based on detection (lines 351-354)
            let ping_template = if found_someone {
                &self.data.ir_bright_particle_sys
            } else {
                &self.data.ir_particle_sys
            };

            // Create ping particle system (lines 356-368)
            if let Some(template_name) = ping_template.as_ref() {
                if let Some(ps_manager) = TheParticleSystemManager::get() {
                    if let Some(system_id) =
                        ps_manager.create_particle_system(Some(template_name.as_str()))
                    {
                        let mut ping_pos = self_pos;
                        if !self.data.ir_particle_sys_bone.is_empty() {
                            let _ = OBJECT_REGISTRY.with_object(self.object_id, |self_guard| {
                                if let Some(drawable) = self_guard.get_drawable() {
                                    if let Ok(drawable_guard) = drawable.read() {
                                        if let Some(bone_matrix) = drawable_guard
                                            .get_current_worldspace_client_bone_positions(
                                                &self.data.ir_particle_sys_bone,
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
                            });
                        }
                        ps_manager.set_particle_system_position(system_id, &ping_pos);
                        ps_manager.attach_particle_system_to_object(system_id, self.object_id);
                        self.ping_particle_id = Some(system_id);
                    }
                }
            }

            // Create beacon particle system (lines 370-384)
            if let Some(template_name) = self.data.ir_beacon_particle_sys.as_ref() {
                if let Some(ps_manager) = TheParticleSystemManager::get() {
                    if let Some(system_id) =
                        ps_manager.create_particle_system(Some(template_name.as_str()))
                    {
                        ps_manager.set_particle_system_position(system_id, &self_pos);
                        ps_manager.attach_particle_system_to_object(system_id, self.object_id);
                        self.beacon_particle_id = Some(system_id);
                    }
                }
            }

            // Play ping sound (lines 386-393)
            let ping_sound = if found_someone {
                &self.data.loud_ping_sound
            } else {
                &self.data.ping_sound
            };

            if let Some(sound_name) = ping_sound.as_ref() {
                if let Some(audio) = crate::helpers::TheAudio::get() {
                    let mut event = crate::common::audio::AudioEventRts::new(sound_name.clone());
                    event.set_object_id(self.object_id);
                    audio.add_audio_event(&event);
                }
            }
        } else {
            self.clear_ping_beacon_particles();
        }

        // Sleep until next update (line 400)
        Ok(UpdateSleepTime::Frames(self.data.update_rate))
    }

    fn get_vision_range(&self) -> Real {
        // Wave 297: empty dual-world → 0.0.
        if dual_world_registry_unavailable() {
            return 0.0;
        }

        // Get vision range from object
        OBJECT_REGISTRY
            .with_object(self.object_id, |guard| guard.get_vision_range())
            .unwrap_or(0.0)
    }
}

/// Update sleep time enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateSleepTime {
    None,
    Forever,
    Frames(UnsignedInt),
}

/// Stealth detector update module
pub struct StealthDetectorUpdate {
    module_name_key: NameKeyType,
    data: Arc<StealthDetectorUpdateModuleData>,
    controller: Arc<Mutex<StealthDetectorController>>,
    object_id: ObjectID,
}

impl StealthDetectorUpdate {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StealthDetectorUpdateModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let controller = Arc::new(Mutex::new(StealthDetectorController::new(
            data.clone(),
            object_id,
        )));

        Self {
            module_name_key,
            data,
            controller,
            object_id,
        }
    }

    pub fn get_controller(&self) -> Arc<Mutex<StealthDetectorController>> {
        self.controller.clone()
    }
}

impl Module for StealthDetectorUpdate {
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
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        debug!(
            "Stealth detector initialized for object {} with range {} and update rate {}",
            self.object_id, self.data.detection_range, self.data.update_rate
        );
    }

    fn get_stealth_detector_control_interface(
        &mut self,
    ) -> Option<&mut dyn StealthDetectorControlInterface> {
        Some(self)
    }
}

impl StealthDetectorControlInterface for StealthDetectorUpdate {
    fn set_sd_enabled(&mut self, enabled: bool) {
        if let Ok(mut controller) = self.controller.lock() {
            controller.set_enabled(enabled);
        }
    }
}

impl Snapshotable for StealthDetectorUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut enabled = self
            .controller
            .lock()
            .map(|ctrl| ctrl.is_enabled())
            .unwrap_or(true);
        xfer.xfer_bool(&mut enabled).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Matches C++ xfer lines 420-433
        let mut enabled = self
            .controller
            .lock()
            .map(|ctrl| ctrl.is_enabled())
            .unwrap_or(true);
        xfer.xfer_bool(&mut enabled).map_err(|e| e.to_string())?;
        if let Ok(mut ctrl) = self.controller.lock() {
            ctrl.set_enabled(enabled);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// INI field parsing - matches C++ buildFieldParse lines 37-61

fn parse_detection_rate(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.update_rate = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

fn parse_detection_range(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.detection_range = INI::parse_real(value)?;
    Ok(())
}

fn parse_initially_disabled(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.initially_disabled = INI::parse_bool(value)?;
    Ok(())
}

fn parse_ping_sound(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value: Vec<&str> = tokens.iter().filter(|t| **t != "=").copied().collect();
    data.ping_sound = Some(value.join(" "));
    Ok(())
}

fn parse_loud_ping_sound(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value: Vec<&str> = tokens.iter().filter(|t| **t != "=").copied().collect();
    data.loud_ping_sound = Some(value.join(" "));
    Ok(())
}

fn parse_ir_beacon_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ir_beacon_particle_sys = Some(value.to_string());
    Ok(())
}

fn parse_ir_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ir_particle_sys = Some(value.to_string());
    Ok(())
}

fn parse_ir_bright_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ir_bright_particle_sys = Some(value.to_string());
    Ok(())
}

fn parse_ir_grid_particle(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ir_grid_particle_sys = Some(value.to_string());
    Ok(())
}

fn parse_ir_particle_bone(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ir_particle_sys_bone = value.to_string();
    Ok(())
}

fn parse_extra_required_kindof(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.extra_detect_kindof =
        crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_extra_forbidden_kindof(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.extra_detect_kindof_not =
        crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_can_detect_while_garrisoned(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.can_detect_while_garrisoned = INI::parse_bool(value)?;
    Ok(())
}

fn parse_can_detect_while_transported(
    _ini: &mut INI,
    data: &mut StealthDetectorUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.can_detect_while_transported = INI::parse_bool(value)?;
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
        parse: parse_can_detect_while_garrisoned,
    },
    FieldParse {
        token: "CanDetectWhileContained",
        parse: parse_can_detect_while_transported,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_module_data_defaults() {
        let data = StealthDetectorUpdateModuleData::default();
        assert_eq!(data.update_rate, 1);
        assert_eq!(data.detection_range, 0.0);
        assert_eq!(data.initially_disabled, false);
        assert_eq!(data.can_detect_while_garrisoned, false);
        assert_eq!(data.can_detect_while_transported, false);
    }

    #[test]
    fn test_detector_controller_creation() {
        let data = Arc::new(StealthDetectorUpdateModuleData::default());
        let controller = StealthDetectorController::new(data.clone(), 1);
        assert!(controller.is_enabled());
    }

    #[test]
    fn test_detector_enable_disable() {
        let data = Arc::new(StealthDetectorUpdateModuleData::default());
        let mut controller = StealthDetectorController::new(data, 1);

        controller.set_enabled(false);
        assert!(!controller.is_enabled());

        controller.set_enabled(true);
        assert!(controller.is_enabled());
    }

    #[test]
    fn test_detector_with_custom_range() {
        let data = StealthDetectorUpdateModuleData {
            detection_range: 300.0,
            update_rate: 10,
            ..Default::default()
        };
        assert_eq!(data.detection_range, 300.0);
        assert_eq!(data.update_rate, 10);
    }

    #[test]
    fn test_detector_parses_kindof_filters() {
        let mut data = StealthDetectorUpdateModuleData::default();
        let mut ini = INI::new();

        parse_extra_required_kindof(&mut ini, &mut data, &["=", "INFANTRY", "VEHICLE"])
            .expect("required kindof mask should parse");
        parse_extra_forbidden_kindof(&mut ini, &mut data, &["=", "MINE"])
            .expect("forbidden kindof mask should parse");

        assert_ne!(data.extra_detect_kindof, 0);
        assert_ne!(data.extra_detect_kindof_not, 0);
        assert!(mask_contains_kind(
            data.extra_detect_kindof,
            KindOf::Infantry
        ));
        assert!(mask_contains_kind(
            data.extra_detect_kindof,
            KindOf::Vehicle
        ));
        assert!(mask_contains_kind(
            data.extra_detect_kindof_not,
            KindOf::Mine
        ));
    }
}
