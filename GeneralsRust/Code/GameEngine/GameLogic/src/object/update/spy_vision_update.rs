//! SpyVisionUpdate Module
//!
//! Port of SpyVisionUpdate.h and SpyVisionUpdate.cpp
//!
//! Handles the logic for revealing enemy vision to the player.

use crate::common::*;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::Object;
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;
use crate::upgrade::{UpgradeMask, UpgradeMux, UpgradeMuxData};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleData as EngineModuleData, NameKeyType, SpyVisionControlInterface,
};
use log::{debug, warn};
use std::sync::{Arc, RwLock};

/// Wave 434: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// C++ `UpdateModule.h`: `UPDATE_SLEEP_FOREVER` is `0x3fffffff`, `PHASE_NORMAL` is 2.
const UPDATE_SLEEP_FOREVER_FRAMES: UnsignedInt = 0x3fff_ffff;
const PHASE_NORMAL: UnsignedInt = 2;

/// C++ `UPDATE_SLEEP(n)`. GameLogic turns a 0 return into `UPDATE_SLEEP_NONE` (1).
/// Values at or above `UPDATE_SLEEP_FOREVER` clamp to the forever wake.
fn cpp_update_sleep(frames: UnsignedInt) -> UpdateSleepTime {
    if frames == 0 {
        UpdateSleepTime::None
    } else if frames >= UPDATE_SLEEP_FOREVER_FRAMES {
        UpdateSleepTime::Forever
    } else {
        UpdateSleepTime::Frames(frames)
    }
}

#[derive(Debug, Clone)]
pub struct SpyVisionUpdateModuleData {
    module_tag_name_key: NameKeyType,
    spy_on_kind_of: KindOfMaskType,
    self_powered: Bool,
    self_powered_duration: UnsignedInt,
    self_powered_interval: UnsignedInt,
    needs_upgrade: Bool,
    upgrade_mux_data: UpgradeMuxData,
}

impl Default for SpyVisionUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            spy_on_kind_of: !0u128,
            self_powered: false,
            self_powered_duration: 0,
            self_powered_interval: 0,
            needs_upgrade: false,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl SpyVisionUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPY_VISION_UPDATE_FIELDS)
    }
}

impl ModuleData for SpyVisionUpdateModuleData {
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

impl Snapshotable for SpyVisionUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_u128(&mut self.spy_on_kind_of)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.self_powered)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.self_powered_duration)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.self_powered_interval)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.needs_upgrade)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct SpyVisionController {
    data: Arc<SpyVisionUpdateModuleData>,
    object_id: ObjectID,
    deactivate_frame: UnsignedInt,
    currently_active: Bool,
    reset_timers_next_update: Bool,
    disabled_until_frame: UnsignedInt,
}

impl SpyVisionController {
    pub fn new(data: Arc<SpyVisionUpdateModuleData>, object_id: ObjectID) -> Self {
        Self {
            data,
            object_id,
            deactivate_frame: 0,
            currently_active: false,
            reset_timers_next_update: false,
            disabled_until_frame: 0,
        }
    }

    pub fn activate_spy_vision(&mut self, duration: UnsignedInt) {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        if duration == 0 {
            self.deactivate_frame = u32::MAX;
        } else {
            self.deactivate_frame = current_frame.wrapping_add(duration);
        }

        // Simulating doActivationWork with object ID lookup inside update or specialized method
        self.do_activation_work_for_current_owner(true);
    }

    fn do_activation_work_for_owner(&mut self, owner: &crate::player::Player, setting: bool) {
        let spying_player_index = owner.get_player_index();

        let Ok(list_guard) = player_list().read() else {
            // C++: ThePlayerList == NULL → return without changing m_currentlyActive.
            return;
        };

        for target_player_arc in list_guard.iter() {
            let Ok(target_player_read) = target_player_arc.read() else {
                continue;
            };
            if target_player_read.get_player_index() == spying_player_index {
                continue;
            }
            // C++ getRelationship(player->getDefaultTeam()) == ENEMIES, not the player-only map.
            let default_team = target_player_read.get_default_team();
            drop(target_player_read);
            let is_enemy = default_team
                .as_ref()
                .and_then(|team_arc| team_arc.read().ok())
                .is_some_and(|team| {
                    owner.get_relationship_with_team(&team)
                        == Relationship::Enemies
                });

            if !is_enemy {
                continue;
            }

            if let Ok(mut target_player_write) = target_player_arc.write() {
                target_player_write.set_units_vision_spied(
                    setting,
                    self.data.spy_on_kind_of,
                    spying_player_index,
                );
            }
        }
        self.currently_active = setting;
    }

    fn do_activation_work_for_current_owner(&mut self, setting: bool) {
        // Wave 434: empty dual-world → no-op (m_currentlyActive unchanged).
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(spying_player_id) = OBJECT_REGISTRY
            .with_object(self.object_id, |owner_obj_guard| {
                owner_obj_guard.get_controlling_player_id()
            })
            .flatten()
        else {
            // C++: playerToSetFor == NULL → return without changing m_currentlyActive.
            return;
        };

        // Clone the player out and drop the list lock. do_activation_work locks
        // the same list again; std::sync::RwLock does not reenter.
        let spying_player_arc = {
            let Ok(list_guard) = player_list().read() else {
                return;
            };
            let Some(player) =
                list_guard.get_player(spying_player_id as crate::player::PlayerIndex)
            else {
                return;
            };
            Arc::clone(player)
        };
        let Ok(spying_player_guard) = spying_player_arc.read() else {
            return;
        };
        // currently_active is assigned only after the player-list work succeeds.
        self.do_activation_work_for_owner(&spying_player_guard, setting);
    }

    pub fn on_capture(
        &mut self,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) {
        if !self.currently_active {
            return;
        }

        if let Some(old_owner) = old_owner {
            let _ = crate::player::with_player(old_owner, |old_guard| {
                self.do_activation_work_for_owner(old_guard, false);
            });
        }
        if let Some(new_owner) = new_owner {
            let _ = crate::player::with_player(new_owner, |new_guard| {
                self.do_activation_work_for_owner(new_guard, true);
            });
        }
    }

    pub fn set_disabled_until_frame(&mut self, frame: UnsignedInt) {
        let now = crate::helpers::TheGameLogic::get_frame();
        if frame > now {
            if self.currently_active {
                self.do_activation_work_for_current_owner(false);
            }
            self.disabled_until_frame = frame;
            self.reset_timers_next_update = true;
        } else {
            self.disabled_until_frame = now;
            self.reset_timers_next_update = true;
        }
    }

    pub fn update(&mut self) -> UpdateSleepTime {
        let now = crate::helpers::TheGameLogic::get_frame();

        // C++ update() does not re-read m_disabledUntilFrame. The wake was
        // already programmed by setDisabledUntilFrame.
        if self.reset_timers_next_update {
            self.reset_timers_next_update = false;

            if self.data.self_powered {
                if self.data.self_powered_interval == 0 {
                    // Always-on self-powered: turn back on and sleep forever.
                    // C++ returns UPDATE_SLEEP(UPDATE_SLEEP_FOREVER).
                    self.do_activation_work_for_current_owner(true);
                    return UpdateSleepTime::Forever;
                } else {
                    return cpp_update_sleep(self.data.self_powered_interval);
                }
            }
        }

        if self.currently_active && self.deactivate_frame <= now {
            self.do_activation_work_for_current_owner(false);
            self.deactivate_frame = 0;
        } else if !self.currently_active && self.data.self_powered {
            self.do_activation_work_for_current_owner(true);
            if self.data.self_powered_duration == 0 {
                self.deactivate_frame = u32::MAX;
            } else {
                self.deactivate_frame = now.wrapping_add(self.data.self_powered_duration);
            }
        }

        if self.data.self_powered {
            if self.currently_active {
                return cpp_update_sleep(self.data.self_powered_duration);
            } else {
                return cpp_update_sleep(self.data.self_powered_interval);
            }
        }

        UpdateSleepTime::Forever
    }
}

pub struct SpyVisionUpdate {
    module_name_key: NameKeyType,
    data: Arc<SpyVisionUpdateModuleData>,
    controller: SpyVisionController,
    next_call_frame_and_phase: UnsignedInt,
    object_id: ObjectID,
    upgrade_mux: UpgradeMux,
    update_proxy: Option<crate::object::UpdateModulePtr>,
}

impl SpyVisionUpdate {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<SpyVisionUpdateModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let upgrade_mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        let controller = SpyVisionController::new(data.clone(), object_id);
        Self {
            module_name_key,
            data,
            controller,
            next_call_frame_and_phase: (UPDATE_SLEEP_FOREVER_FRAMES << 2) | PHASE_NORMAL,
            object_id,
            upgrade_mux,
            update_proxy: None,
        }
    }

    pub fn activate_spy_vision(&mut self, duration: UnsignedInt) {
        self.controller.activate_spy_vision(duration);
        // C++ setWakeFrame: absolute wake is now + delay, clamped to FOREVER.
        // Duration 0 is UPDATE_SLEEP_FOREVER, not now+0.
        let now = crate::helpers::TheGameLogic::get_frame();
        let wake = if duration == 0 {
            UPDATE_SLEEP_FOREVER_FRAMES
        } else {
            let summed = now.wrapping_add(duration);
            if summed > UPDATE_SLEEP_FOREVER_FRAMES {
                UPDATE_SLEEP_FOREVER_FRAMES
            } else {
                summed
            }
        };
        let stored_phase = self.next_call_frame_and_phase & 3;
        let phase = if stored_phase == 0 {
            PHASE_NORMAL
        } else {
            stored_phase
        };
        self.next_call_frame_and_phase = (wake << 2) | phase;
        self.awaken_if_not_current(wake);
    }

    fn awaken_if_not_current(&self, wake: UnsignedInt) {
        let Some(proxy) = self.update_proxy.clone() else {
            return;
        };
        // process_sleepy_updates already holds the GameLogic mutex. C++ ignores
        // setWakeFrame when this module is the current update and uses the return.
        if crate::system::game_logic::is_cur_update_module(&proxy) {
            return;
        }
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            logic.friend_awaken_update_module(&proxy, wake);
        }
    }
    pub(crate) fn bind_update_proxy(&mut self, proxy: crate::object::UpdateModulePtr) {
        self.update_proxy = Some(proxy);
    }

    /// C++ `SpyVisionUpdate::upgradeImplementation`.
    pub fn upgrade_implementation(&mut self) {
        if self.data.needs_upgrade && !self.upgrade_mux.is_already_upgraded() {
            self.activate_spy_vision(self.data.self_powered_duration);
        }
    }

    pub fn set_disabled_until_frame(&mut self, frame: UnsignedInt) {
        let now = crate::helpers::TheGameLogic::get_frame();
        self.controller.set_disabled_until_frame(frame);
        // C++ setWakeFrame(disabledUntil - now), or UPDATE_SLEEP_NONE (1) on the wakeup branch.

        let wake = if frame > now {
            frame.min(UPDATE_SLEEP_FOREVER_FRAMES)
        } else {
            now.saturating_add(1).min(UPDATE_SLEEP_FOREVER_FRAMES)
        };
        let stored_phase = self.next_call_frame_and_phase & 3;
        let phase = if stored_phase == 0 { 2 } else { stored_phase };
        self.next_call_frame_and_phase = (wake << 2) | phase;
        self.awaken_if_not_current(wake);
    }

    fn handle_on_delete(&mut self) {
        if self.controller.currently_active {
            self.controller
                .do_activation_work_for_current_owner(false);
        }
    }

    /// Unpacked C++ `friend_getNextCallFrame`.
    pub fn initial_wake_frame(&self) -> UnsignedInt {
        self.next_call_frame_and_phase >> 2
    }

    fn build_upgrade_mask(&self, obj: &Object) -> UpgradeMask {
        let mut mask = obj.completed_upgrades();
        if let Some(player) = obj.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                mask |= player_guard.get_completed_upgrade_mask();
            }
        }
        UpgradeMask::from_bits_retain(mask.bits())
    }

    fn maybe_trigger_upgrade(&mut self) {
        // Wave 434: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if !self.data.needs_upgrade || self.upgrade_mux.is_already_upgraded() {
            return;
        }

        let mux_data = self.upgrade_mux.data.clone();
        let would = OBJECT_REGISTRY.with_object_mut(self.object_id, |obj_guard| {
            let upgrade_mask = self.build_upgrade_mask(obj_guard);
            if self.upgrade_mux.would_upgrade(upgrade_mask) {
                mux_data.perform_upgrade_fx(obj_guard);
                mux_data.process_upgrade_removal(obj_guard);
                true
            } else {
                false
            }
        });
        if would == Some(true) {
            // C++ UpgradeMux::giveSelfUpgrade → upgradeImplementation.
            self.upgrade_implementation();
            self.upgrade_mux.set_upgrade_executed(true);
        }
    }
}

impl Module for SpyVisionUpdate {
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

    fn on_delete(&mut self) {
        self.handle_on_delete();
    }

    fn on_object_created(&mut self) {
        // C++ ctor setWakeFrame runs before registerObject. If registration
        // used wake 0, push the packed forever (or later) frame now.
        let wake = self.initial_wake_frame();
        self.awaken_if_not_current(wake);
    }
}

impl Snapshotable for SpyVisionUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ SpyVisionUpdate::crc only calls UpdateModule::crc, which is empty.
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("SpyVisionUpdate xfer version failed: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.controller.deactivate_frame)
            .map_err(|e| format!("SpyVisionUpdate xfer deactivate_frame failed: {:?}", e))?;
        xfer.xfer_bool(&mut self.controller.currently_active)
            .map_err(|e| format!("SpyVisionUpdate xfer currently_active failed: {:?}", e))?;
        if version >= 2 {
            xfer.xfer_bool(&mut self.controller.reset_timers_next_update)
                .map_err(|e| {
                    format!(
                        "SpyVisionUpdate xfer reset_timers_next_update failed: {:?}",
                        e
                    )
                })?;
            xfer.xfer_unsigned_int(&mut self.controller.disabled_until_frame)
                .map_err(|e| {
                    format!("SpyVisionUpdate xfer disabled_until_frame failed: {:?}", e)
                })?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes SpyVisionUpdate through the common Module trait.
pub struct SpyVisionUpdateModule {
    behavior: SpyVisionUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SpyVisionUpdateModuleData>,
}

impl SpyVisionUpdateModule {
    pub fn new(
        behavior: SpyVisionUpdate,
        module_name: &AsciiString,
        module_data: Arc<SpyVisionUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SpyVisionUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SpyVisionUpdateModule {
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

impl Module for SpyVisionUpdateModule {
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

    fn get_spy_vision_control_interface(&mut self) -> Option<&mut dyn SpyVisionControlInterface> {
        Some(self.behavior_mut())
    }

    fn on_object_created(&mut self) {
        Module::on_object_created(&mut self.behavior);
    }
}

impl UpdateModuleInterface for SpyVisionUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        self.maybe_trigger_upgrade();
        self.controller.update()
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.update_simple())
    }
}

impl BehaviorModuleInterface for SpyVisionUpdate {
    fn get_module_name(&self) -> &'static str {
        "SpyVisionUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn on_capture(
        &mut self,
        old_owner: Option<PlayerIndex>,
        new_owner: Option<PlayerIndex>,
    ) {
        self.controller.on_capture(old_owner, new_owner);
    }

    fn on_disabled_edge(&mut self, now_disabled: bool) {
        // C++ onDisabledEdge calls setDisabledUntilFrame, which setWakeFrame's.
        if now_disabled {
            SpyVisionUpdate::set_disabled_until_frame(self, u32::MAX);
        } else {
            SpyVisionUpdate::set_disabled_until_frame(self, 0);
        }
    }

    fn get_spy_vision_update(
        &mut self,
    ) -> Option<&mut dyn crate::object::behavior::behavior_module::SpyVisionUpdate> {
        Some(self)
    }

    fn get_spy_vision_control_interface(&mut self) -> Option<&mut dyn SpyVisionControlInterface> {
        Some(self)
    }
}

impl crate::object::behavior::behavior_module::SpyVisionUpdate for SpyVisionUpdate {
    fn activate_spy_vision(&mut self, duration: UnsignedInt) {
        SpyVisionUpdate::activate_spy_vision(self, duration);
    }

    fn set_disabled_until_frame(
        &mut self,
        frame: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        SpyVisionUpdate::set_disabled_until_frame(self, frame);
        Ok(())
    }
}

impl SpyVisionControlInterface for SpyVisionUpdate {
    fn set_disabled_until_frame(&mut self, frame: u32) {
        SpyVisionUpdate::set_disabled_until_frame(self, frame);
    }
}

// INI Parsing
fn parse_spy_on_kind_of(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    use crate::common::KindOf;

    fn parse_kind(token: &str) -> Option<KindOf> {
        let token = token.trim().trim_matches(',');
        let token = token.strip_prefix("KINDOF_").unwrap_or(token);
        let token = token.strip_prefix("KINDOF").unwrap_or(token);
        let upper = token.to_ascii_uppercase();

        match upper.as_str() {
            "SELECTABLE" => Some(KindOf::Selectable),
            "UNIT" => Some(KindOf::Unit),
            "BUILDING" => Some(KindOf::Building),
            "VEHICLE" => Some(KindOf::Vehicle),
            "INFANTRY" => Some(KindOf::Infantry),
            "AIRCRAFT" => Some(KindOf::Aircraft),
            "DRONE" => Some(KindOf::Drone),
            "CLIFFJUMPER" | "CLIFF_JUMPER" => Some(KindOf::CliffJumper),
            "STRUCTURE" => Some(KindOf::Structure),
            "WEAPON" => Some(KindOf::Weapon),
            "PROJECTILE" => Some(KindOf::Projectile),
            "CANSEETHROUGH" | "CAN_SEE_THROUGH" => Some(KindOf::CanSeeThrough),
            "ALWAYSSELECTABLE" | "ALWAYS_SELECTABLE" => Some(KindOf::AlwaysSelectable),
            "CRATE" => Some(KindOf::Crate),
            "RESOURCENODE" | "RESOURCE_NODE" => Some(KindOf::ResourceNode),
            "TECHBUILDING" | "TECH_BUILDING" => Some(KindOf::TechBuilding),
            "BRIDGE" => Some(KindOf::Bridge),
            "BARRIER" => Some(KindOf::Barrier),
            "CIVILIAN" => Some(KindOf::Civilian),
            "DESTRUCTIBLE" => Some(KindOf::Destructible),
            "CANCROSSBRIDGES" | "CAN_CROSS_BRIDGES" => Some(KindOf::CanCrossBridges),
            "AMPHIBIOUS" => Some(KindOf::Amphibious),
            "AMPHIBIOUSTRANSPORT" | "AMPHIBIOUS_TRANSPORT" => Some(KindOf::AmphibiousTransport),
            "CAPTURE" | "CAN_CAPTURE" => Some(KindOf::CanCapture),
            "SABOTEUR" => Some(KindOf::Saboteur),
            "HACKER" => Some(KindOf::Hacker),
            "HERO" => Some(KindOf::Hero),
            "KEYSTRUCTURE" | "KEY_STRUCTURE" => Some(KindOf::KeyStructure),
            "COMMANDCENTER" | "COMMAND_CENTER" => Some(KindOf::CommandCenter),
            "POWERPLANT" | "POWER_PLANT" => Some(KindOf::PowerPlant),
            "REFINERY" => Some(KindOf::Refinery),
            "FACTORY" => Some(KindOf::Factory),
            "DEFENSE" => Some(KindOf::Defense),
            "SHRUBBERY" => Some(KindOf::Shrubbery),
            "DOZER" => Some(KindOf::Dozer),
            "HULK" => Some(KindOf::Hulk),
            "SALVAGER" => Some(KindOf::Salvager),
            "WEAPONSALVAGER" | "WEAPON_SALVAGER" => Some(KindOf::WeaponSalvager),
            "ARMORSALVAGER" | "ARMOR_SALVAGER" => Some(KindOf::ArmorSalvager),
            "AIRCRAFTCARRIER" | "AIRCRAFT_CARRIER" => Some(KindOf::AircraftCarrier),
            "FSBARRACKS" | "FS_BARRACKS" => Some(KindOf::FSBarracks),
            "FSWARFACTORY" | "FS_WARFACTORY" => Some(KindOf::FSWarfactory),
            "FSAIRFIELD" | "FS_AIRFIELD" => Some(KindOf::FSAirfield),
            "FSINTERNETCENTER" | "FS_INTERNET_CENTER" => Some(KindOf::FSInternetCenter),
            "FSPOWER" | "FS_POWER" => Some(KindOf::FSPower),
            "FSSUPPLYDROPZONE" | "FS_SUPPLY_DROPZONE" => Some(KindOf::FSSupplyDropzone),
            "FSSUPPLYCENTER" | "FS_SUPPLY_CENTER" => Some(KindOf::FSSupplyCenter),
            "FSSUPERWEAPON" | "FS_SUPERWEAPON" => Some(KindOf::FSSuperweapon),
            "FSSTRATEGYCENTER" | "FS_STRATEGY_CENTER" => Some(KindOf::FSStrategyCenter),
            "COUNTSFORVICTORY" | "COUNTS_FOR_VICTORY" => Some(KindOf::CountsForVictory),
            "MINE" => Some(KindOf::Mine),
            "PORTABLE_STRUCTURE" | "PORTABLESTRUCTURE" => Some(KindOf::PortableStructure),
            _ => None,
        }
    }

    let mut mask: KindOfMaskType = 0;
    for token in tokens.iter().copied().filter(|t| *t != "=") {
        if token.eq_ignore_ascii_case("SpyOnKindof") {
            continue;
        }
        if token.eq_ignore_ascii_case("ALL") {
            mask = !0u128;
            break;
        }
        if token.eq_ignore_ascii_case("NONE") {
            mask = 0;
            continue;
        }
        if let Some(kind) = parse_kind(token) {
            mask |= kind.cpp_mask();
        }
    }

    data.spy_on_kind_of = mask;
    Ok(())
}

fn parse_self_powered(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.self_powered = INI::parse_bool(value)?;
    Ok(())
}

fn parse_self_powered_duration(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.self_powered_duration = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

fn parse_self_powered_interval(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.self_powered_interval = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

fn parse_needs_upgrade(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.needs_upgrade = INI::parse_bool(value)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .trigger_upgrade_names
                .push(crate::common::AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_conflicts_with(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .conflicting_upgrade_names
                .push(crate::common::AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_removes_upgrades(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .removal_upgrade_names
                .push(crate::common::AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_requires_all_triggers(
    _ini: &mut INI,
    data: &mut SpyVisionUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.upgrade_mux_data.requires_all_triggers = INI::parse_bool(value)?;
    Ok(())
}

const SPY_VISION_UPDATE_FIELDS: &[FieldParse<SpyVisionUpdateModuleData>] = &[
    FieldParse {
        token: "SpyOnKindof",
        parse: parse_spy_on_kind_of,
    },
    FieldParse {
        token: "SelfPowered",
        parse: parse_self_powered,
    },
    FieldParse {
        token: "SelfPoweredDuration",
        parse: parse_self_powered_duration,
    },
    FieldParse {
        token: "SelfPoweredInterval",
        parse: parse_self_powered_interval,
    },
    FieldParse {
        token: "NeedsUpgrade",
        parse: parse_needs_upgrade,
    },
    FieldParse {
        token: "TriggeredBy",
        parse: parse_triggered_by,
    },
    FieldParse {
        token: "ConflictsWith",
        parse: parse_conflicts_with,
    },
    FieldParse {
        token: "RemovesUpgrades",
        parse: parse_removes_upgrades,
    },
    FieldParse {
        token: "RequiresAllTriggers",
        parse: parse_requires_all_triggers,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctor_packs_forever_wake_like_cpp() {
        let update = SpyVisionUpdate::new(0, Arc::new(SpyVisionUpdateModuleData::default()), 1);
        assert_eq!(update.initial_wake_frame(), UPDATE_SLEEP_FOREVER_FRAMES);
        assert_eq!(update.next_call_frame_and_phase & 3, PHASE_NORMAL);
    }

    #[test]
    fn update_sleep_matches_cpp_update_sleep_range() {
        assert_eq!(cpp_update_sleep(0), UpdateSleepTime::None);
        assert_eq!(cpp_update_sleep(1), UpdateSleepTime::Frames(1));
        assert_eq!(cpp_update_sleep(90), UpdateSleepTime::Frames(90));
        assert_eq!(
            cpp_update_sleep(UPDATE_SLEEP_FOREVER_FRAMES),
            UpdateSleepTime::Forever
        );
        assert_eq!(cpp_update_sleep(u32::MAX), UpdateSleepTime::Forever);
    }

    #[test]
    fn update_does_not_resleep_on_disabled_until_frame() {
        // C++ update() ignores m_disabledUntilFrame; only reset-timers / deactivate run.
        let mut controller =
            SpyVisionController::new(Arc::new(SpyVisionUpdateModuleData::default()), 1);
        controller.disabled_until_frame = u32::MAX;
        assert_eq!(controller.update(), UpdateSleepTime::Forever);
        assert!(!controller.currently_active);
        assert!(!controller.reset_timers_next_update);
    }
}
