//! SpyVisionUpdate Module
//!
//! Port of SpyVisionUpdate.h and SpyVisionUpdate.cpp
//!
//! Handles the logic for revealing enemy vision to the player.

use crate::common::*;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::player::player_list;
use crate::upgrade::{UpgradeMask, UpgradeMux, UpgradeMuxData};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, ModuleData as EngineModuleData, NameKeyType,
};
use game_engine::common::thing::KindOfMaskType;
use log::{debug, warn};
use std::sync::{Arc, Mutex, RwLock};

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
            spy_on_kind_of: KIND_OF_MASK_ALL,
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

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
            self.deactivate_frame = current_frame + duration;
        }

        // Simulating doActivationWork with object ID lookup inside update or specialized method
        self.do_activation_work_for_current_owner(true);
    }

    fn do_activation_work_for_owner(&mut self, owner: &crate::player::Player, setting: bool) {
        let spying_player_index = owner.get_player_index();

        let Ok(list_guard) = player_list().read() else {
            return;
        };

        for target_player_arc in list_guard.iter() {
            let Ok(target_player_read) = target_player_arc.read() else {
                continue;
            };
            if target_player_read.get_player_index() == spying_player_index {
                continue;
            }
            let is_enemy = owner.is_enemy_with_player(&*target_player_read);
            drop(target_player_read);

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
    }

    fn do_activation_work_for_current_owner(&mut self, setting: bool) {
        self.currently_active = setting;

        let Some(owner_obj_arc) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return;
        };
        let Ok(owner_obj_guard) = owner_obj_arc.read() else {
            return;
        };
        let Some(spying_player_id) = owner_obj_guard.get_controlling_player_id() else {
            return;
        };
        drop(owner_obj_guard);

        let Ok(list_guard) = player_list().read() else {
            return;
        };
        let Some(spying_player_arc) =
            list_guard.get_player(spying_player_id as crate::player::PlayerIndex)
        else {
            return;
        };
        let Ok(spying_player_guard) = spying_player_arc.read() else {
            return;
        };
        self.do_activation_work_for_owner(&spying_player_guard, setting);
    }

    pub fn on_capture(
        &mut self,
        old_owner: Option<&Arc<RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        if !self.currently_active {
            return;
        }

        if let Some(old_owner) = old_owner {
            if let Ok(old_guard) = old_owner.read() {
                self.do_activation_work_for_owner(&old_guard, false);
            }
        }
        if let Some(new_owner) = new_owner {
            if let Ok(new_guard) = new_owner.read() {
                self.do_activation_work_for_owner(&new_guard, true);
            }
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
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        if self.disabled_until_frame > current_frame {
            return UpdateSleepTime::frames(self.disabled_until_frame - current_frame);
        }

        // Handle reset timers (e.g. after being disabled)
        if self.reset_timers_next_update {
            self.reset_timers_next_update = false;

            if self.data.self_powered {
                if self.data.self_powered_interval == 0 {
                    // Always on self-powered
                    self.do_activation_work_for_current_owner(true);
                    return UpdateSleepTime::Forever;
                } else {
                    // Reset interval timer via sleeping before reactivation
                    return UpdateSleepTime::frames(self.data.self_powered_interval);
                }
            }
        }

        // Handle deactivation
        if self.currently_active && current_frame >= self.deactivate_frame {
            self.do_activation_work_for_current_owner(false);
            self.deactivate_frame = 0;
        } else if !self.currently_active && self.data.self_powered {
            // Turn on self-powered
            self.do_activation_work_for_current_owner(true);
            if self.data.self_powered_duration == 0 {
                self.deactivate_frame = u32::MAX;
            } else {
                self.deactivate_frame = current_frame + self.data.self_powered_duration;
            }
        }

        // Handle self-powered cycling (active -> inactive -> active)
        if self.data.self_powered {
            if self.currently_active {
                return UpdateSleepTime::from_u32(self.data.self_powered_duration);
            } else {
                return UpdateSleepTime::from_u32(self.data.self_powered_interval);
            }
        }

        UpdateSleepTime::Forever
    }
}

pub struct SpyVisionUpdate {
    module_name_key: NameKeyType,
    data: Arc<SpyVisionUpdateModuleData>,
    controller: Arc<Mutex<SpyVisionController>>,
    object_id: ObjectID,
    upgrade_mux: UpgradeMux,
}

impl SpyVisionUpdate {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<SpyVisionUpdateModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let upgrade_mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        let controller = Arc::new(Mutex::new(SpyVisionController::new(
            data.clone(),
            object_id,
        )));
        Self {
            module_name_key,
            data,
            controller,
            object_id,
            upgrade_mux,
        }
    }

    pub fn activate_spy_vision(&self, duration: UnsignedInt) {
        if let Ok(mut controller) = self.controller.lock() {
            controller.activate_spy_vision(duration);
        }
    }

    pub fn set_disabled_until_frame(&mut self, frame: UnsignedInt) {
        if let Ok(mut controller) = self.controller.lock() {
            controller.set_disabled_until_frame(frame);
        }
    }

    fn handle_on_delete(&mut self) {
        if let Ok(mut controller) = self.controller.lock() {
            if controller.currently_active {
                controller.do_activation_work_for_current_owner(false);
            }
        }
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
        if !self.data.needs_upgrade || self.upgrade_mux.is_already_upgraded() {
            return;
        }

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return;
        };
        let Ok(mut obj_guard) = obj_arc.write() else {
            return;
        };
        let upgrade_mask = self.build_upgrade_mask(&obj_guard);
        if self.upgrade_mux.would_upgrade(upgrade_mask) {
            self.upgrade_mux.data.perform_upgrade_fx(&mut obj_guard);
            self.upgrade_mux
                .data
                .process_upgrade_removal(&mut obj_guard);
            self.activate_spy_vision(self.data.self_powered_duration);
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
}

impl Snapshotable for SpyVisionUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 2;
        _xfer
            .xfer_version(&mut version, 2)
            .map_err(|e| format!("SpyVisionUpdate xfer version failed: {:?}", e))?;
        if let Ok(mut controller) = self.controller.lock() {
            _xfer
                .xfer_unsigned_int(&mut controller.deactivate_frame)
                .map_err(|e| format!("SpyVisionUpdate xfer deactivate_frame failed: {:?}", e))?;
            _xfer
                .xfer_bool(&mut controller.currently_active)
                .map_err(|e| format!("SpyVisionUpdate xfer currently_active failed: {:?}", e))?;
            if version >= 2 {
                _xfer
                    .xfer_bool(&mut controller.reset_timers_next_update)
                    .map_err(|e| {
                        format!(
                            "SpyVisionUpdate xfer reset_timers_next_update failed: {:?}",
                            e
                        )
                    })?;
                _xfer
                    .xfer_unsigned_int(&mut controller.disabled_until_frame)
                    .map_err(|e| {
                        format!("SpyVisionUpdate xfer disabled_until_frame failed: {:?}", e)
                    })?;
            }
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
}

impl UpdateModuleInterface for SpyVisionUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.maybe_trigger_upgrade();
        if let Ok(mut controller) = self.controller.lock() {
            let sleep = controller.update();
            return Ok(sleep);
        }
        Ok(UpdateSleepTime::None)
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
        old_owner: Option<&Arc<RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<RwLock<crate::player::Player>>>,
    ) {
        if let Ok(mut controller) = self.controller.lock() {
            controller.on_capture(old_owner, new_owner);
        }
    }

    fn on_disabled_edge(&mut self, now_disabled: bool) {
        if let Ok(mut controller) = self.controller.lock() {
            if now_disabled {
                controller.set_disabled_until_frame(u32::MAX);
            } else {
                controller.set_disabled_until_frame(0);
            }
        }
    }

    fn get_spy_vision_update(
        &mut self,
    ) -> Option<&mut dyn crate::object::behavior::behavior_module::SpyVisionUpdate> {
        Some(self)
    }
}

impl crate::object::behavior::behavior_module::SpyVisionUpdate for SpyVisionUpdate {}

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
        if token.eq_ignore_ascii_case("SpyOnKindOf") {
            continue;
        }
        if token.eq_ignore_ascii_case("ALL") {
            mask = u64::MAX;
            break;
        }
        if token.eq_ignore_ascii_case("NONE") {
            mask = 0;
            continue;
        }
        if let Some(kind) = parse_kind(token) {
            mask |= 1u64 << (kind as u32);
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
    data.self_powered_duration = INI::parse_unsigned_int(value)?;
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
    data.self_powered_interval = INI::parse_unsigned_int(value)?;
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
        token: "SpyOnKindOf",
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
