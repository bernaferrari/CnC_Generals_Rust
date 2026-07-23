//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/AutoHealBehavior.cpp`.
//!
//! AutoHealBehavior - Rust conversion of C++ AutoHealBehavior
//!
//! Update module that heals itself or nearby allies.
//! Author: Colin Day, December 2001 (C++ version)
//! Modified by: Kris Morness, September 2002 (added effects, radius healing, object type restrictions)
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Bool, Coord3D, GameLogicRandomValue, Int, KindOf, KindOfMaskType,
    NameKeyGenerator, ObjectID, ParticleSystemID, ParticleSystemTemplate, Real, UnsignedInt,
    UpgradeMaskType, XferVersion, KIND_OF_MASK_ALL, KIND_OF_MASK_NONE,
};
use crate::damage::{BodyDamageType, DamageInfo};
use crate::helpers::{TheGameLogic, TheParticleSystemManager};
use crate::modules::{
    BehaviorModuleInterface, DamageModuleInterface, UpdateModuleInterface, UpdateSleepTime,
    UpgradeModuleInterface, UpgradeMuxData,
};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::{
    registry::OBJECT_REGISTRY, Object as GameObject, INVALID_ID as OBJECT_INVALID_ID,
};
use crate::upgrade::upgrade_mask_for_ascii;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::NameKeyType;
use game_engine::common::system::{Snapshotable, Xfer, XferMode};
use game_engine::common::thing::module::{
    Module, ModuleData, Object as ModuleObjectTrait, Thing as ModuleThing,
};
use log::warn;
use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

// Constants
const UINT_MAX: UnsignedInt = u32::MAX;
const NEVER: UnsignedInt = u32::MAX;
const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;
const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemID = 0;

#[derive(Debug, Clone)]
struct AutoHealObjectHandle {
    object_id: ObjectID,
}

impl ModuleObjectTrait for AutoHealObjectHandle {
    fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    fn remove_upgrade(
        &self,
        upgrade_template: Option<&game_engine::common::ini::ini_upgrade::UpgradeTemplate>,
    ) {
        let Some(template) = upgrade_template else {
            return;
        };
        let upgrade_name = template.name.as_str();
        if upgrade_name.is_empty() {
            return;
        }

        let mask_bits = upgrade_mask_for_ascii(upgrade_name);
        if mask_bits.is_empty() {
            return;
        }

        if self.object_id == OBJECT_INVALID_ID {
            return;
        }
        if let Some(arc) = OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(mut guard) = arc.write() {
                guard.remove_upgrade_mask(mask_bits);
            }
        }
    }
}

fn object_matches_kind_mask(obj: &GameObject, mask: KindOfMaskType) -> bool {
    if mask == KIND_OF_MASK_ALL {
        return true;
    }
    if mask == KIND_OF_MASK_NONE {
        return false;
    }

    for &kind in crate::common::ALL_KIND_OF {
        let bit = 1u64 << (kind as u32);
        if (mask & bit) != 0 && obj.is_kind_of(kind) {
            return true;
        }
    }

    false
}

pub(crate) fn parse_kind_of(token: &str) -> Option<KindOf> {
    let token = token.trim().trim_matches(',').trim();
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
        "SUPPLYSOURCEONPREVIEW" | "SUPPLY_SOURCE_ON_PREVIEW" => Some(KindOf::SupplySourceOnPreview),
        "SUPPLYSOURCE" | "SUPPLY_SOURCE" => Some(KindOf::SupplySource),
        "DISGUISER" => Some(KindOf::Disguiser),
        "PORTABLE_STRUCTURE" | "PORTABLESTRUCTURE" => Some(KindOf::PortableStructure),
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
        "HARVESTER" => Some(KindOf::Harvester),
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
        "CAN_BE_REPULSED" | "CANBEREPULSED" => Some(KindOf::CanBeRepulsed),
        "EMP_HARDENED" | "EMPHARDENED" => Some(KindOf::EmpHardened),
        "SPAWNS_ARE_THE_WEAPONS" | "SPAWNSARETHEWEAPONS" => Some(KindOf::SpawnsAreTheWeapons),
        "IGNORE_DOCKING_BONES" | "IGNOREDOCKINGBONES" => Some(KindOf::IgnoreDockingBones),
        "REPAIRPAD" | "REPAIR_PAD" => Some(KindOf::RepairPad),
        "REJECT_UNMANNED" | "REJECTUNMANNED" => Some(KindOf::RejectUnmanned),
        "IGNORED_IN_GUI" | "IGNOREDINGUI" | "IGNORED_IN_GUI_OBJECT" => Some(KindOf::IgnoredInGui),
        "MOB_NEXUS" | "MOBNEXUS" => Some(KindOf::MobNexus),
        "CAPTURABLE" => Some(KindOf::Capturable),
        "IMMUNE_TO_CAPTURE" | "IMMUNETOCAPTURE" => Some(KindOf::ImmuneToCapture),
        "CASH_GENERATOR" | "CASHGENERATOR" => Some(KindOf::CashGenerator),
        "REBUILD_HOLE" | "REBUILDHOLE" => Some(KindOf::RebuildHole),
        "FS_TECHNOLOGY" | "FSTECHNOLOGY" => Some(KindOf::FSTechnology),
        "GARRISONABLE_UNTIL_DESTROYED" => Some(KindOf::GarrisonableUntilDestroyed),
        "NO_GARRISON" | "NOGARRISON" => Some(KindOf::NoGarrison),
        _ => None,
    }
}

pub(crate) fn parse_kind_of_mask(tokens: &[&str]) -> KindOfMaskType {
    let mut mask: KindOfMaskType = 0;
    for token in tokens.iter().copied().filter(|t| *t != "=") {
        if token.eq_ignore_ascii_case("KindOf") || token.eq_ignore_ascii_case("ForbiddenKindOf") {
            continue;
        }
        if token.eq_ignore_ascii_case("ALL") {
            return KIND_OF_MASK_ALL;
        }
        if token.eq_ignore_ascii_case("NONE") {
            continue;
        }
        if let Some(kind) = parse_kind_of(token) {
            mask |= 1u64 << (kind as u32);
        }
    }
    mask
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_bool_flag(value: &str) -> Result<bool, INIError> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(INIError::InvalidData),
    }
}

fn parse_int(value: &str) -> Result<Int, INIError> {
    value.parse::<Int>().map_err(|_| INIError::InvalidData)
}

fn parse_real(value: &str) -> Result<Real, INIError> {
    value.parse::<Real>().map_err(|_| INIError::InvalidData)
}

fn parse_starts_active_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.initially_active = parse_bool_flag(value)?;
    Ok(())
}

fn parse_single_burst_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.single_burst = parse_bool_flag(value)?;
    Ok(())
}

fn parse_healing_amount_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.healing_amount = parse_int(value)?;
    Ok(())
}

fn parse_healing_delay_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.healing_delay = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

fn parse_start_healing_delay_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.start_healing_delay = INI::parse_duration_unsigned_int(value)?;
    Ok(())
}

fn parse_radius_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.radius = parse_real(value)?;
    Ok(())
}

fn parse_affects_whole_player_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.affects_whole_player = parse_bool_flag(value)?;
    Ok(())
}

fn parse_skip_self_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.skip_self_for_healing = parse_bool_flag(value)?;
    Ok(())
}

fn parse_kind_of_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_forbidden_kind_of_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.forbidden_kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_particle_system_template(
    tokens: &[&str],
) -> Result<Option<Arc<ParticleSystemTemplate>>, INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let name = AsciiString::from(value);
    Ok(Some(Arc::new(ParticleSystemTemplate::new(name))))
}

fn parse_radius_particle_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.radius_particle_system_tmpl = parse_particle_system_template(tokens)?;
    Ok(())
}

fn parse_pulse_particle_field(
    _ini: &mut INI,
    data: &mut AutoHealBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.unit_heal_pulse_particle_system_tmpl = parse_particle_system_template(tokens)?;
    Ok(())
}

const AUTO_HEAL_BEHAVIOR_FIELDS: &[FieldParse<AutoHealBehaviorModuleData>] = &[
    FieldParse {
        token: "StartsActive",
        parse: parse_starts_active_field,
    },
    FieldParse {
        token: "SingleBurst",
        parse: parse_single_burst_field,
    },
    FieldParse {
        token: "HealingAmount",
        parse: parse_healing_amount_field,
    },
    FieldParse {
        token: "HealingDelay",
        parse: parse_healing_delay_field,
    },
    FieldParse {
        token: "StartHealingDelay",
        parse: parse_start_healing_delay_field,
    },
    FieldParse {
        token: "Radius",
        parse: parse_radius_field,
    },
    FieldParse {
        token: "AffectsWholePlayer",
        parse: parse_affects_whole_player_field,
    },
    FieldParse {
        token: "SkipSelfForHealing",
        parse: parse_skip_self_field,
    },
    FieldParse {
        token: "KindOf",
        parse: parse_kind_of_field,
    },
    FieldParse {
        token: "ForbiddenKindOf",
        parse: parse_forbidden_kind_of_field,
    },
    FieldParse {
        token: "RadiusParticleSystemName",
        parse: parse_radius_particle_field,
    },
    FieldParse {
        token: "UnitHealPulseParticleSystemName",
        parse: parse_pulse_particle_field,
    },
];

/// Minimal Update module data carrier until the shared implementation lands.
#[derive(Clone, Debug, Default)]
pub struct UpdateModuleData {
    module_tag_name_key: NameKeyType,
}

impl UpdateModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl Snapshotable for UpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(UpdateModuleData, module_tag_name_key);

/// AutoHealBehaviorModuleData - Configuration for AutoHeal behavior
#[derive(Clone, Debug)]
pub struct AutoHealBehaviorModuleData {
    pub base: UpdateModuleData,
    pub upgrade_mux_data: UpgradeMuxData,
    pub initially_active: Bool,
    pub single_burst: Bool,
    pub healing_amount: Int,
    pub healing_delay: UnsignedInt,
    pub start_healing_delay: UnsignedInt,
    pub radius: Real,
    pub affects_whole_player: Bool,
    pub skip_self_for_healing: Bool,
    pub kind_of: KindOfMaskType,
    pub forbidden_kind_of: KindOfMaskType,
    pub radius_particle_system_tmpl: Option<Arc<ParticleSystemTemplate>>,
    pub unit_heal_pulse_particle_system_tmpl: Option<Arc<ParticleSystemTemplate>>,
}

impl AutoHealBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: UpdateModuleData::new(),
            upgrade_mux_data: UpgradeMuxData::new(),
            initially_active: false,
            single_burst: false,
            healing_amount: 0,
            healing_delay: UINT_MAX,
            start_healing_delay: 0,
            radius: 0.0,
            affects_whole_player: false,
            skip_self_for_healing: false,
            kind_of: KIND_OF_MASK_ALL,
            forbidden_kind_of: KIND_OF_MASK_NONE,
            radius_particle_system_tmpl: None,
            unit_heal_pulse_particle_system_tmpl: None,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, AUTO_HEAL_BEHAVIOR_FIELDS)
    }
}

impl Default for AutoHealBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(AutoHealBehaviorModuleData, base);

/// Helper struct for scanning player objects for auto heal
pub struct AutoHealPlayerScanHelper {
    pub kind_of_to_test: KindOfMaskType,
    pub forbidden_kind_of: KindOfMaskType,
    /// Healer object id (stable; resolve only for the duration of a check).
    pub the_healer: Option<ObjectID>,
    /// Candidate object ids to pulse-heal after the scan.
    pub object_list: Vec<ObjectID>,
    pub skip_self_for_healing: Bool,
}

impl AutoHealPlayerScanHelper {
    pub fn new() -> Self {
        Self {
            kind_of_to_test: KIND_OF_MASK_ALL,
            forbidden_kind_of: KIND_OF_MASK_NONE,
            the_healer: None,
            object_list: Vec::new(),
            skip_self_for_healing: false,
        }
    }

    /// Check if an object should be auto-healed (ID-first).
    pub fn check_for_auto_heal(
        &mut self,
        test_obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if test_obj_id == OBJECT_INVALID_ID {
            return Ok(());
        }

        let should_queue = OBJECT_REGISTRY
            .with_object(test_obj_id, |test_obj_read| -> Result<bool, String> {
                if test_obj_read.is_effectively_dead() {
                    return Ok(false);
                }

                if let Some(healer_id) = self.the_healer {
                    let test_player = test_obj_read.get_controlling_player_id();
                    let healer_player = OBJECT_REGISTRY
                        .with_object(healer_id, |healer| healer.get_controlling_player_id());
                    if let (Some(tp), Some(Some(hp))) = (test_player, healer_player) {
                        if tp != hp {
                            return Ok(false);
                        }
                    }

                    if self.skip_self_for_healing && healer_id == test_obj_id {
                        return Ok(false);
                    }
                }

                if test_obj_read.is_off_map() {
                    return Ok(false);
                }

                if !object_matches_kind_mask(test_obj_read, self.kind_of_to_test) {
                    return Ok(false);
                }

                if object_matches_kind_mask(test_obj_read, self.forbidden_kind_of) {
                    return Ok(false);
                }

                if let Some(body) = test_obj_read.get_body_module() {
                    let body_lock = body
                        .lock()
                        .map_err(|e| format!("auto-heal body lock poisoned: {}", e))?;
                    if body_lock.get_health() >= body_lock.get_max_health() {
                        return Ok(false);
                    }
                }

                Ok(true)
            })
            .transpose()
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?
            .unwrap_or(false);

        if should_queue {
            self.object_list.push(test_obj_id);
        }
        Ok(())
    }
}

/// AutoHealBehavior - Main implementation of auto-healing behavior
pub struct AutoHealBehavior {
    pub module_data: Arc<AutoHealBehaviorModuleData>,

    // Particle system for radius effect
    pub radius_particle_system_id: ParticleSystemID,

    // Timing and state
    pub soonest_heal_frame: UnsignedInt,
    pub stopped: Bool,

    // Upgrade system
    pub next_call_frame_and_phase: UnsignedInt,
    pub upgrade_executed: Bool,
    object_id: ObjectID,
    upgrade_masks: Mutex<Option<(UpgradeMaskType, UpgradeMaskType)>>,
}

impl fmt::Debug for AutoHealBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AutoHealBehavior")
            .field("object_id", &self.object_id)
            .field("stopped", &self.stopped)
            .field("upgrade_executed", &self.upgrade_executed)
            .finish()
    }
}

impl AutoHealBehavior {
    fn construct_with_object_id(
        object_id: ObjectID,
        module_data: Arc<AutoHealBehaviorModuleData>,
    ) -> Self {
        let mut behavior = Self {
            module_data,
            radius_particle_system_id: INVALID_PARTICLE_SYSTEM_ID,
            soonest_heal_frame: 0,
            stopped: false,
            next_call_frame_and_phase: 0,
            upgrade_executed: false,
            object_id,
            upgrade_masks: Mutex::new(None),
        };

        if let Some(radius_tmpl) = &behavior.module_data.radius_particle_system_tmpl {
            if let Some(manager) = TheParticleSystemManager::get() {
                if let Some(ps_id) = manager.create_particle_system(Some(radius_tmpl.name.as_str()))
                {
                    if let Some(obj) = behavior.get_object() {
                        if let Ok(obj_read) = obj.read() {
                            manager.set_particle_system_position(ps_id, obj_read.get_position());
                        }
                    }
                    behavior.radius_particle_system_id = ps_id;
                }
            }
        }

        if behavior.module_data.initially_active {
            behavior.give_self_upgrade();
            let delay = behavior.module_data.healing_delay;
            if delay > 0 {
                let random_delay = GameLogicRandomValue(1, delay as Int) as UnsignedInt;
                behavior.set_wake_frame(update_sleep_time(random_delay));
            } else {
                behavior.set_wake_frame(UPDATE_SLEEP_NONE);
            }
        } else {
            behavior.set_wake_frame(UPDATE_SLEEP_FOREVER);
        }

        behavior
    }

    pub fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Nothing to initialize yet; parity hook for C++ BehaviorModule.
        Ok(())
    }

    pub fn on_destroy(&mut self) {
        self.stop_healing();
        if self.radius_particle_system_id != INVALID_PARTICLE_SYSTEM_ID {
            if let Some(manager) = TheParticleSystemManager::get() {
                manager.destroy_particle_system(self.radius_particle_system_id);
            }
            self.radius_particle_system_id = INVALID_PARTICLE_SYSTEM_ID;
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<AutoHealBehaviorModuleData>,
    ) -> Self {
        let object_id = thing
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or(OBJECT_INVALID_ID);
        Self::construct_with_object_id(object_id, module_data)
    }

    pub fn new_from_object_handle(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<AutoHealBehaviorModuleData>,
    ) -> Self {
        let object_id = object
            .read()
            .ok()
            .map(|obj| obj.get_id())
            .unwrap_or(OBJECT_INVALID_ID);
        Self::construct_with_object_id(object_id, module_data)
    }

    /// Stop healing behavior
    pub fn stop_healing(&mut self) {
        self.stopped = true;
        self.soonest_heal_frame = NEVER;
        self.set_wake_frame(UPDATE_SLEEP_FOREVER);
    }

    /// Undo upgrade - reset to non-upgraded state
    pub fn undo_upgrade(&mut self) {
        self.soonest_heal_frame = 0;
        self.upgrade_executed = false;
    }

    /// Check if upgrade is currently active
    pub fn is_upgrade_active(&self) -> Bool {
        self.upgrade_executed
    }

    /// Give self the upgrade (activate healing)
    pub fn give_self_upgrade(&mut self) {
        self.upgrade_executed = true;
        self.set_wake_frame(UPDATE_SLEEP_NONE);
    }

    /// Pulse heal a single object
    pub fn pulse_heal_object(
        &mut self,
        obj: Arc<RwLock<GameObject>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.stopped {
            return Ok(());
        }

        let data = &self.module_data;
        let healer_handle = self.get_object();
        let healer_guard = healer_handle.as_ref().and_then(|arc| arc.read().ok());
        let healer_ref = healer_guard.as_deref();

        // Attempt healing - different logic for radius vs non-radius
        if data.radius == 0.0 {
            if let Ok(mut obj_write) = obj.write() {
                obj_write.attempt_healing(data.healing_amount as Real, healer_ref)?;
            }
        } else {
            if let Ok(mut obj_write) = obj.write() {
                obj_write.attempt_healing_from_sole_benefactor(
                    data.healing_amount as Real,
                    healer_ref,
                    data.healing_delay,
                )?;
            }
        }

        // Create heal pulse particle effect
        if let Some(pulse_particle_tmpl) = &data.unit_heal_pulse_particle_system_tmpl {
            if let Some(manager) = TheParticleSystemManager::get() {
                if let Some(ps_id) =
                    manager.create_particle_system(Some(pulse_particle_tmpl.name.as_str()))
                {
                    if let Ok(obj_read) = obj.read() {
                        manager.set_particle_system_position(ps_id, obj_read.get_position());
                    }
                }
            }
        }

        // Update soonest heal frame
        if let Ok(current_frame) = self.get_current_frame() {
            self.soonest_heal_frame = current_frame + data.healing_delay;
        }

        Ok(())
    }

    /// ID-first pulse heal: resolve target for the duration of the heal write.
    fn pulse_heal_object_id(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if obj_id == OBJECT_INVALID_ID {
            return Ok(());
        }
        let Some(obj) = OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(());
        };
        self.pulse_heal_object(obj)
    }

    /// Get the object this behavior belongs to
    fn owner_object_id(&self) -> ObjectID {
        self.object_id
    }

    fn with_object<R>(&self, f: impl FnOnce(&GameObject) -> R) -> Option<R> {
        let id = self.owner_object_id();
        if id == OBJECT_INVALID_ID {
            return None;
        }
        OBJECT_REGISTRY.with_object(id, f)
    }

    fn with_object_mut<R>(&self, f: impl FnOnce(&mut GameObject) -> R) -> Option<R> {
        let id = self.owner_object_id();
        if id == OBJECT_INVALID_ID {
            return None;
        }
        OBJECT_REGISTRY.with_object_mut(id, f)
    }

    /// Short-lived Arc resolve; prefer `with_object` / `owner_object_id`.
    fn get_object(&self) -> Option<Arc<RwLock<GameObject>>> {
        let id = self.owner_object_id();
        if id == OBJECT_INVALID_ID {
            return None;
        }
        OBJECT_REGISTRY.get_object(id)
    }

    /// Get current game frame
    fn get_current_frame(&self) -> Result<UnsignedInt, Box<dyn std::error::Error + Send + Sync>> {
        Ok(TheGameLogic::get_frame())
    }

    /// Set wake frame for update scheduling
    fn set_wake_frame(&mut self, sleep_time: UpdateSleepTime) {
        if self.object_id == OBJECT_INVALID_ID {
            return;
        }
        TheGameLogic::set_wake_frame(self.object_id, sleep_time);
    }
}

impl UpdateModuleInterface for AutoHealBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if self.stopped || !self.is_upgrade_active() {
            return Ok(UPDATE_SLEEP_FOREVER);
        }

        if self
            .with_object(|obj_read| obj_read.is_effectively_dead())
            .unwrap_or(true)
        {
            return Ok(UPDATE_SLEEP_FOREVER);
        }

        let data = &self.module_data;
        if data.affects_whole_player {
            return self.heal_all_player_objects();
        }

        if data.radius > 0.0 {
            return self.heal_friendlies_in_radius();
        }

        self.heal_self_only()
    }
}

// Implement DamageModuleInterface for handling damage events
impl DamageModuleInterface for AutoHealBehavior {
    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.stopped {
            return Ok(());
        }

        let data = &self.module_data;
        if self.is_upgrade_active() && data.radius == 0.0 {
            // If we have a start healing delay, getting damaged resets our healing process
            if data.start_healing_delay > 0 {
                self.set_wake_frame(update_sleep_time(data.start_healing_delay));
            } else {
                // Check if we can wake up immediately
                if let Ok(current_frame) = self.get_current_frame() {
                    if current_frame > self.soonest_heal_frame {
                        self.set_wake_frame(UPDATE_SLEEP_NONE);
                    }
                }
            }
        }

        Ok(())
    }

    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // No special handling for healing events
        Ok(())
    }

    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // No special handling for damage state changes
        Ok(())
    }
}

/// Glue that exposes AutoHealBehavior through the common Module trait.
pub struct AutoHealBehaviorModule {
    behavior: AutoHealBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<AutoHealBehaviorModuleData>,
}

impl AutoHealBehaviorModule {
    pub fn new(
        behavior: AutoHealBehavior,
        module_name: &AsciiString,
        module_data: Arc<AutoHealBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &AutoHealBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut AutoHealBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for AutoHealBehaviorModule {
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

impl Module for AutoHealBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        if let Err(err) = self.behavior.init() {
            warn!("AutoHealBehavior init failed: {err}");
        }
    }

    fn on_delete(&mut self) {
        self.behavior.on_destroy();
    }
}

impl AutoHealBehavior {
    /// Heal all objects belonging to the same player
    fn heal_all_player_objects(
        &mut self,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let kind_of_to_test = self.module_data.kind_of.clone();
        let forbidden_kind_of = self.module_data.forbidden_kind_of.clone();
        let skip_self = self.module_data.skip_self_for_healing;
        let healing_delay = self.module_data.healing_delay;
        let Some((controlling_player, healer_id)) =
            self.with_object(|obj_read| (obj_read.get_controlling_player(), obj_read.get_id()))
        else {
            return Ok(UPDATE_SLEEP_FOREVER);
        };

        if let Some(player) = controlling_player {
            let mut helper = AutoHealPlayerScanHelper::new();
            helper.kind_of_to_test = kind_of_to_test;
            helper.forbidden_kind_of = forbidden_kind_of;
            helper.the_healer = Some(healer_id);
            helper.skip_self_for_healing = skip_self;

            player
                .read()
                .map_err(|e| format!("auto-heal player lock poisoned: {}", e))?
                .iterate_object_ids(|candidate_id| {
                    helper
                        .check_for_auto_heal(candidate_id)
                        .map_err(|e| crate::common::GameError::ModuleError(e.to_string()))?;
                    Ok(())
                })
                .map_err(|e| format!("auto-heal iterate_objects failed: {:?}", e))?;

            // Heal all qualifying objects
            for heal_id in helper.object_list {
                self.pulse_heal_object_id(heal_id)?;
            }
        }

        Ok(update_sleep_time(healing_delay))
    }

    /// Heal self only (original system)
    fn heal_self_only(
        &mut self,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let healing_delay = self.module_data.healing_delay;
        let needs_healing = self
            .with_object(|obj_read| {
                if let Some(body) = obj_read.get_body_module() {
                    if let Ok(body_lock) = body.lock() {
                        body_lock.get_health() < body_lock.get_max_health()
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .unwrap_or(false);

        if needs_healing {
            let Some(obj) = self.get_object() else {
                return Ok(UPDATE_SLEEP_FOREVER);
            };
            self.pulse_heal_object(obj)?;
            Ok(update_sleep_time(healing_delay))
        } else {
            // Go to sleep forever - we'll wake up when damaged again
            Ok(UPDATE_SLEEP_FOREVER)
        }
    }

    /// Heal friendlies in radius (expanded system)
    fn heal_friendlies_in_radius(
        &mut self,
    ) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let data = &self.module_data;
        let Some((position, healer_team)) =
            self.with_object(|obj_read| (*obj_read.get_position(), obj_read.get_team()))
        else {
            return Ok(UPDATE_SLEEP_FOREVER);
        };

        let Some(healer_team) = healer_team else {
            return Ok(UPDATE_SLEEP_FOREVER);
        };

        let partition = crate::helpers::ThePartitionManager::get();
        let Some(partition) = partition else {
            return Ok(UPDATE_SLEEP_FOREVER);
        };

        let radius = data.radius;
        let skip_self_for_healing = data.skip_self_for_healing;
        let kind_of = data.kind_of;
        let forbidden_kind_of = data.forbidden_kind_of;
        let single_burst = data.single_burst;
        let healing_delay = data.healing_delay;

        let mut _healed_any = false;
        for object_id in partition.get_objects_in_range(&position, radius) {
            if object_id == self.object_id && skip_self_for_healing {
                continue;
            }

            let Some(candidate) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };

            let (is_friend, passes_kind, needs_heal) = {
                let Ok(candidate_read) = candidate.read() else {
                    continue;
                };
                if candidate_read.is_effectively_dead() || candidate_read.is_off_map() {
                    continue;
                }

                let is_friend = candidate_read
                    .get_team()
                    .zip(healer_team.read().ok())
                    .map(|(team, healer)| {
                        team.read()
                            .ok()
                            .map(|team_guard| healer.get_relationship(&team_guard))
                            .map(|rel| rel == crate::common::Relationship::Allies)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);

                let passes_kind = object_matches_kind_mask(&candidate_read, kind_of)
                    && !object_matches_kind_mask(&candidate_read, forbidden_kind_of);

                let needs_heal = if let Some(body) = candidate_read.get_body_module() {
                    if let Ok(body_guard) = body.lock() {
                        body_guard.get_health() < body_guard.get_max_health()
                    } else {
                        false
                    }
                } else {
                    false
                };

                (is_friend, passes_kind, needs_heal)
            };

            if is_friend && passes_kind && needs_heal {
                self.pulse_heal_object(candidate)?;
                _healed_any = true;
            }
        }

        if single_burst {
            Ok(UPDATE_SLEEP_FOREVER)
        } else {
            Ok(update_sleep_time(healing_delay))
        }
    }
}

// Implement UpgradeModuleInterface for upgrade system integration
impl UpgradeModuleInterface for AutoHealBehavior {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        let (activation_mask, conflicting_mask) = self.compute_upgrade_masks();

        if !conflicting_mask.is_empty() && upgrade_mask.intersects(conflicting_mask) {
            return false;
        }

        if activation_mask.is_empty() {
            return true;
        }

        if self.module_data.upgrade_mux_data.requires_all_triggers() {
            upgrade_mask.contains(activation_mask)
        } else {
            upgrade_mask.intersects(activation_mask)
        }
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        let (_activation_mask, conflicting_mask) = self.compute_upgrade_masks();

        if !conflicting_mask.is_empty() && upgrade_mask.intersects(conflicting_mask) {
            self.remove_upgrade(upgrade_mask);
            return false;
        }

        if !self.can_upgrade(upgrade_mask) {
            return false;
        }

        if !self.upgrade_executed {
            self.give_self_upgrade();
            let _ = self.with_object(|thing| {
                self.module_data.upgrade_mux_data.perform_upgrade_fx(thing);
            });
        }

        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        let (activation_mask, _) = self.compute_upgrade_masks();
        if activation_mask.is_empty() || upgrade_mask.intersects(activation_mask) {
            self.undo_upgrade();
            let object_id = self.owner_object_id();
            if object_id != OBJECT_INVALID_ID {
                let object_handle = AutoHealObjectHandle { object_id };
                self.module_data
                    .upgrade_mux_data
                    .mux_data_process_upgrade_removal(&object_handle);
            }
        }
    }
}

impl BehaviorModuleInterface for AutoHealBehavior {
    fn get_module_name(&self) -> &'static str {
        "AutoHealBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }

    fn get_upgrade(&mut self) -> Option<&mut dyn UpgradeModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for AutoHealBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AutoHealBehavior version xfer failed: {:?}", e))?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .map_err(|e| format!("AutoHealBehavior update module base state: {}", e))?;

        let mut upgrade_mux_version: XferVersion = 1;
        xfer.xfer_version(&mut upgrade_mux_version, 1)
            .map_err(|e| format!("AutoHealBehavior upgrade mux version: {:?}", e))?;
        let mut upgrade_executed = self.upgrade_executed;
        xfer.xfer_bool(&mut upgrade_executed)
            .map_err(|e| e.to_string())?;

        let mut radius_particle_system_id = self.radius_particle_system_id;
        xfer.xfer_unsigned_int(&mut radius_particle_system_id)
            .map_err(|e| e.to_string())?;
        let mut soonest_heal_frame = self.soonest_heal_frame;
        xfer.xfer_unsigned_int(&mut soonest_heal_frame)
            .map_err(|e| e.to_string())?;
        let mut stopped = self.stopped;
        xfer.xfer_bool(&mut stopped).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AutoHealBehavior version xfer failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|e| format!("AutoHealBehavior update module base state: {}", e))?;

        let mut upgrade_mux_version: XferVersion = 1;
        xfer.xfer_version(&mut upgrade_mux_version, 1)
            .map_err(|e| format!("AutoHealBehavior upgrade mux version: {:?}", e))?;
        xfer.xfer_bool(&mut self.upgrade_executed)
            .map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.radius_particle_system_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.soonest_heal_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.stopped)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Load {
            if self.stopped {
                self.set_wake_frame(UPDATE_SLEEP_FOREVER);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// Serialization support
// Serialization support
impl AutoHealBehavior {
    fn compute_upgrade_masks(&self) -> (UpgradeMaskType, UpgradeMaskType) {
        if let Ok(mut cache) = self.upgrade_masks.lock() {
            if let Some(mask_pair) = *cache {
                return mask_pair;
            }

            let activation = self
                .module_data
                .upgrade_mux_data
                .activation_upgrade_names()
                .iter()
                .fold(UpgradeMaskType::none(), |mask, name| {
                    mask | upgrade_mask_for_ascii(name)
                });

            let conflicting = self
                .module_data
                .upgrade_mux_data
                .conflicting_upgrade_names()
                .iter()
                .fold(UpgradeMaskType::none(), |mask, name| {
                    mask | upgrade_mask_for_ascii(name)
                });

            let result = (activation, conflicting);
            *cache = Some(result);
            result
        } else {
            (UpgradeMaskType::none(), UpgradeMaskType::none())
        }
    }
}

// Helper function to create UpdateSleepTime
const fn update_sleep_time(frames: UnsignedInt) -> UpdateSleepTime {
    UpdateSleepTime::Frames(frames)
}

// Thread-safe implementation
unsafe impl Send for AutoHealBehavior {}
unsafe impl Sync for AutoHealBehavior {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_heal_duration_fields_parse_to_logic_frames() {
        let mut data = AutoHealBehaviorModuleData::default();

        parse_healing_delay_field(&mut INI::new(), &mut data, &["=", "3000"]).unwrap();
        parse_start_healing_delay_field(&mut INI::new(), &mut data, &["=", "1.5s"]).unwrap();

        assert_eq!(data.healing_delay, 90);
        assert_eq!(data.start_healing_delay, 45);
    }

    #[test]
    fn test_healing_logic() {
        // Test different healing scenarios
    }

    #[test]
    fn test_upgrade_integration() {
        // Test upgrade system integration
    }
}
