//! Die Modules - Rust conversion of C++ Die Module classes
//!
//! This module contains all die modules that handle object death behaviors.
//! Die modules control what happens when an object is destroyed - whether it
//! spawns other objects, plays effects, leaves wreckage, etc.
//!
//! Original C++ location: GameLogic/Module/DieModule.h and Object/Die/
//! Original C++ Authors: Colin Day, Steven Johnson, and others (2001-2003)
//! Rust conversion: 2025

pub mod create_crate_die;
pub mod create_object_die;
pub mod crush_die;
pub mod dam_die;
pub mod destroy_die;
pub mod die_module;
pub mod eject_pilot_die;
pub mod fx_list_die;
pub mod keep_object_die;
pub mod rebuild_hole_expose_die;
pub mod special_power_completion_die;
pub mod upgrade_die;

// Re-export all die modules for convenience
pub use create_crate_die::{CreateCrateDie, CreateCrateDieModuleData};
pub use create_object_die::{CreateObjectDie, CreateObjectDieModuleData};
pub use crush_die::{CrushDie, CrushDieModuleData, CrushEnum};
pub use dam_die::{DamDie, DamDieModuleData};
pub use destroy_die::DestroyDie;
pub use die_module::*;
pub use eject_pilot_die::{EjectPilotDie, EjectPilotDieModuleData};
pub use fx_list_die::{FXListDie, FXListDieModuleData};
pub use keep_object_die::KeepObjectDie;
pub use rebuild_hole_expose_die::{RebuildHoleExposeDie, RebuildHoleExposeDieModuleData};
pub use special_power_completion_die::{
    SpecialPowerCompletionDie, SpecialPowerCompletionDieModuleData,
};
pub use upgrade_die::{UpgradeDie, UpgradeDieModuleData};

use crate::common::{
    AsAny, AsciiString, Bool, NameKeyType, ObjectID, ObjectStatusMaskType, ObjectStatusTypes, Real,
    VeterancyLevel,
};
use crate::damage::{
    clear_death_type_flag, set_death_type_flag, DamageInfo, DeathType, DeathTypeFlags,
    DEATH_TYPE_FLAGS_ALL, DEATH_TYPE_FLAGS_NONE,
};
use crate::object::Object;
use bitflags::bitflags;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData};
use std::any::Any;
use std::sync::{Arc, RwLock};

use crate::object::behavior::behavior_module::xfer_behavior_module_base_versions;

/// Veterancy level flags for die module filtering
pub type VeterancyLevelFlags = u32;

/// All veterancy levels enabled
pub const VETERANCY_LEVEL_FLAGS_ALL: VeterancyLevelFlags = 0xffff_ffff;
pub const VETERANCY_LEVEL_FLAGS_NONE: VeterancyLevelFlags = 0x0000_0000;

/// Check if a veterancy level flag is set
pub fn get_veterancy_level_flag(flags: VeterancyLevelFlags, level: VeterancyLevel) -> bool {
    let bit = 1u32 << (level as u32);
    (flags & bit) != 0
}

pub fn parse_death_type_flags_tokens(tokens: &[&str]) -> Result<DeathTypeFlags, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut flags = DEATH_TYPE_FLAGS_NONE;
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = DEATH_TYPE_FLAGS_ALL;
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = DEATH_TYPE_FLAGS_NONE;
                continue;
            }

            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };

            if let Some(death_type) = death_type_from_name(name) {
                flags = if remove {
                    clear_death_type_flag(flags, death_type)
                } else {
                    set_death_type_flag(flags, death_type)
                };
            }
        }
    }

    Ok(flags)
}

pub fn parse_veterancy_level_flags_tokens(
    tokens: &[&str],
) -> Result<VeterancyLevelFlags, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut flags = VETERANCY_LEVEL_FLAGS_NONE;
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = VETERANCY_LEVEL_FLAGS_ALL;
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = VETERANCY_LEVEL_FLAGS_NONE;
                continue;
            }

            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };

            if let Some(level) = veterancy_level_from_name(name) {
                let bit = 1u32 << (level as u32);
                if remove {
                    flags &= !bit;
                } else {
                    flags |= bit;
                }
            }
        }
    }

    Ok(flags)
}

pub fn parse_object_status_mask_tokens(tokens: &[&str]) -> Result<ObjectStatusMask, INIError> {
    let mask = ObjectStatusMaskType::parse_tokens(tokens.iter().copied())
        .map_err(|_| INIError::InvalidData)?;
    Ok(ObjectStatusMask::from_bits_truncate(mask.bits() as u32))
}

pub fn parse_die_mux_death_types(
    die_mux_data: &mut DieMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    die_mux_data.death_types = parse_death_type_flags_tokens(tokens)?;
    Ok(())
}

pub fn parse_die_mux_veterancy_levels(
    die_mux_data: &mut DieMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    die_mux_data.veterancy_levels = parse_veterancy_level_flags_tokens(tokens)?;
    Ok(())
}

pub fn parse_die_mux_exempt_status(
    die_mux_data: &mut DieMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    die_mux_data.exempt_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

pub fn parse_die_mux_required_status(
    die_mux_data: &mut DieMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    die_mux_data.required_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

bitflags! {
    /// Object status mask for die module filtering
    #[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct ObjectStatusMask: u32 {
        const NONE = 0;
        const ALL = 0xffff_ffff;
    }
}

/// Die multiplexer data for filtering when a die module should activate
/// (matches C++ DieMuxData)
#[derive(Debug, Clone)]
pub struct DieMuxData {
    /// Death types this module responds to
    pub death_types: DeathTypeFlags,
    /// Veterancy levels this module responds to
    pub veterancy_levels: VeterancyLevelFlags,
    /// Status bits that exempt object from this die module
    pub exempt_status: ObjectStatusMask,
    /// Status bits required for this die module to activate
    pub required_status: ObjectStatusMask,
}

impl Default for DieMuxData {
    fn default() -> Self {
        Self {
            death_types: DEATH_TYPE_FLAGS_ALL,
            veterancy_levels: VETERANCY_LEVEL_FLAGS_ALL,
            exempt_status: ObjectStatusMask::NONE,
            required_status: ObjectStatusMask::NONE,
        }
    }
}

impl DieMuxData {
    /// Check if this die module is applicable given the object and damage info
    /// (matches C++ DieMuxData::isDieApplicable)
    pub fn is_die_applicable(&self, obj: &Object, damage_info: &DamageInfo) -> bool {
        let obj_veterancy_level = obj.get_veterancy_level();
        let obj_status_bits =
            ObjectStatusMask::from_bits_truncate(obj.get_status_bits().bits() as u32);

        // Check death type
        if !crate::damage::get_death_type_flag(self.death_types, damage_info.input.death_type) {
            return false;
        }

        // Check veterancy level
        if !get_veterancy_level_flag(self.veterancy_levels, obj_veterancy_level) {
            return false;
        }

        // Check exempt status - all exempt bits must be clear
        if !self.exempt_status.is_empty() && obj_status_bits.intersects(self.exempt_status) {
            return false;
        }

        // Check required status - all required bits must be set
        if !self.required_status.is_empty() && !obj_status_bits.contains(self.required_status) {
            return false;
        }

        true
    }
}

fn death_type_from_name(name: &str) -> Option<DeathType> {
    match name.to_ascii_uppercase().as_str() {
        "CRUSHED" => Some(DeathType::Crushed),
        "BURNED" => Some(DeathType::Burned),
        "EXPLODED" => Some(DeathType::Exploded),
        "POISONED" => Some(DeathType::Poisoned),
        "TOPPLED" => Some(DeathType::Toppled),
        "FLOODED" => Some(DeathType::Flooded),
        "SUICIDED" => Some(DeathType::Suicided),
        "LASERED" => Some(DeathType::Lasered),
        "DETONATED" => Some(DeathType::Detonated),
        "SPLATTED" => Some(DeathType::Splatted),
        "POISONED_BETA" => Some(DeathType::PoisonedBeta),
        "POISONED_GAMMA" => Some(DeathType::PoisonedGamma),
        "EXTRA2" => Some(DeathType::Extra2),
        "EXTRA3" => Some(DeathType::Extra3),
        "EXTRA4" => Some(DeathType::Extra4),
        "EXTRA5" => Some(DeathType::Extra5),
        "EXTRA6" => Some(DeathType::Extra6),
        "EXTRA7" => Some(DeathType::Extra7),
        "EXTRA8" => Some(DeathType::Extra8),
        _ => None,
    }
}

fn veterancy_level_from_name(name: &str) -> Option<VeterancyLevel> {
    match name.to_ascii_uppercase().as_str() {
        "REGULAR" => Some(VeterancyLevel::Regular),
        "VETERAN" => Some(VeterancyLevel::Veteran),
        "ELITE" => Some(VeterancyLevel::Elite),
        "HEROIC" => Some(VeterancyLevel::Heroic),
        _ => None,
    }
}

/// Base data for all die modules (matches C++ DieModuleData)
#[derive(Debug, Clone)]
pub struct DieModuleData {
    pub die_mux_data: DieMuxData,
    pub module_tag_name_key: NameKeyType,
}

impl Default for DieModuleData {
    fn default() -> Self {
        Self {
            die_mux_data: DieMuxData::default(),
            module_tag_name_key: 0,
        }
    }
}

impl Snapshotable for DieModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("DieModuleData crc version: {e:?}"))?;
        let mut death_types = self.die_mux_data.death_types;
        xfer.xfer_unsigned_int(&mut death_types)
            .map_err(|e| format!("DieModuleData crc death_types: {e:?}"))?;
        let mut veterancy_levels = self.die_mux_data.veterancy_levels;
        xfer.xfer_unsigned_int(&mut veterancy_levels)
            .map_err(|e| format!("DieModuleData crc veterancy_levels: {e:?}"))?;
        let mut exempt_status = self.die_mux_data.exempt_status.bits();
        xfer.xfer_unsigned_int(&mut exempt_status)
            .map_err(|e| format!("DieModuleData crc exempt_status: {e:?}"))?;
        let mut required_status = self.die_mux_data.required_status.bits();
        xfer.xfer_unsigned_int(&mut required_status)
            .map_err(|e| format!("DieModuleData crc required_status: {e:?}"))?;
        let mut module_tag_name_key = self.module_tag_name_key;
        xfer.xfer_unsigned_int(&mut module_tag_name_key)
            .map_err(|e| format!("DieModuleData crc module_tag_name_key: {e:?}"))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("DieModuleData xfer version: {e:?}"))?;

        let mut death_types = self.die_mux_data.death_types;
        xfer.xfer_unsigned_int(&mut death_types)
            .map_err(|e| format!("DieModuleData death_types: {e:?}"))?;
        self.die_mux_data.death_types = death_types;

        let mut veterancy_levels = self.die_mux_data.veterancy_levels;
        xfer.xfer_unsigned_int(&mut veterancy_levels)
            .map_err(|e| format!("DieModuleData veterancy_levels: {e:?}"))?;
        self.die_mux_data.veterancy_levels = veterancy_levels;

        let mut exempt_status = self.die_mux_data.exempt_status.bits();
        xfer.xfer_unsigned_int(&mut exempt_status)
            .map_err(|e| format!("DieModuleData exempt_status: {e:?}"))?;
        self.die_mux_data.exempt_status = ObjectStatusMask::from_bits_truncate(exempt_status);

        let mut required_status = self.die_mux_data.required_status.bits();
        xfer.xfer_unsigned_int(&mut required_status)
            .map_err(|e| format!("DieModuleData required_status: {e:?}"))?;
        self.die_mux_data.required_status = ObjectStatusMask::from_bits_truncate(required_status);

        xfer.xfer_unsigned_int(&mut self.module_tag_name_key)
            .map_err(|e| format!("DieModuleData module_tag_name_key: {e:?}"))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(DieModuleData, module_tag_name_key);

impl DieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DIE_MODULE_DATA_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut DieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut DieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut DieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut DieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.die_mux_data, tokens)
}

const DIE_MODULE_DATA_FIELDS: &[FieldParse<DieModuleData>] = &[
    FieldParse {
        token: "DeathTypes",
        parse: parse_die_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_die_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_die_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_die_required_status,
    },
];

/// Base trait for all die modules (matches C++ DieModuleInterface)
pub trait DieModuleInterface: Send + Sync + std::fmt::Debug + AsAny + Any {
    /// Called when the object dies
    /// (matches C++ DieModuleInterface::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo);

    /// Check if this die module is applicable for the given damage
    fn is_die_applicable(
        &self,
        object: &Object,
        damage_info: &DamageInfo,
        die_mux_data: &DieMuxData,
    ) -> bool {
        die_mux_data.is_die_applicable(object, damage_info)
    }

    /// Snapshot hook for die-module-specific state.
    fn snapshot_crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Snapshot hook for die-module-specific state.
    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer_die_module_with_derived_version(xfer)
    }

    /// Snapshot hook for die-module-specific state.
    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Optional creator metadata hook used by SpecialPowerCompletionDie.
    fn set_creator(&mut self, _creator_id: ObjectID) {}

    /// Optional script-engine notification hook used by SpecialPowerCompletionDie.
    /// Returns true when the module handled the notification.
    fn notify_script_engine_with_player_index(&self, _player_index: Option<usize>) -> bool {
        false
    }
}

pub fn xfer_die_module_base_versions(xfer: &mut dyn Xfer) -> Result<(), String> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|err| format!("DieModule::xfer version failed: {err}"))?;
    xfer_behavior_module_base_versions(xfer)
}

pub fn xfer_die_module_with_derived_version(xfer: &mut dyn Xfer) -> Result<(), String> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|err| format!("DieModule derived xfer version failed: {err}"))?;
    xfer_die_module_base_versions(xfer)
}

/// Base struct for die modules with common functionality
#[derive(Debug)]
pub struct DieModule<T: EngineModuleData> {
    pub module_data: Arc<T>,
    pub object_id: ObjectID,
}

impl<T: EngineModuleData> DieModule<T> {
    /// Create a new die module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<T>) -> Self {
        let object_id = object
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        Self {
            module_data,
            object_id,
        }
    }

    /// Get the module data
    pub fn get_module_data(&self) -> &T {
        &self.module_data
    }

    /// Resolve the owner object for the duration of an op.
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
    }
}

/// Module wrapper that exposes die modules through the shared Module system.
#[derive(Debug)]
pub struct DieModuleWrapper {
    module_name_key: NameKeyType,
    module_tag_name_key: NameKeyType,
    module_data: Arc<dyn EngineModuleData>,
    object_id: ObjectID,
    die_module: Box<dyn DieModuleInterface>,
}

impl DieModuleWrapper {
    pub fn new(
        module_name: &AsciiString,
        module_data: Arc<dyn EngineModuleData>,
        object: Arc<RwLock<Object>>,
        die_module: Box<dyn DieModuleInterface>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        let module_tag_name_key = module_data.get_module_tag_name_key();
        let object_id = object
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        Self {
            module_name_key,
            module_tag_name_key,
            module_data,
            object_id,
            die_module,
        }
    }

    fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
    }
}

impl Snapshotable for DieModuleWrapper {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.die_module.snapshot_crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.die_module.snapshot_xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.die_module.snapshot_load_post_process()
    }
}

impl Module for DieModuleWrapper {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

impl crate::modules::DieModuleInterface for DieModuleWrapper {
    fn on_die(
        &mut self,
        damage: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object_arc = self
            .get_object()
            .ok_or("die module wrapper object unavailable")?;
        let mut object = object_arc
            .write()
            .map_err(|_| "die module wrapper object lock poisoned")?;
        self.die_module.on_die(&mut object, damage);
        Ok(())
    }

    fn set_creator(&mut self, creator_id: ObjectID) {
        self.die_module.set_creator(creator_id);
    }

    fn notify_script_engine_with_player_index(&self, player_index: Option<usize>) -> bool {
        self.die_module
            .notify_script_engine_with_player_index(player_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_die_mux_data_default() {
        let mux = DieMuxData::default();
        assert_eq!(mux.death_types, DEATH_TYPE_FLAGS_ALL);
        assert_eq!(mux.veterancy_levels, VETERANCY_LEVEL_FLAGS_ALL);
        assert_eq!(mux.exempt_status, ObjectStatusMask::NONE);
        assert_eq!(mux.required_status, ObjectStatusMask::NONE);
    }

    #[test]
    fn test_veterancy_level_flag() {
        let flags = VETERANCY_LEVEL_FLAGS_ALL;
        assert!(get_veterancy_level_flag(flags, VeterancyLevel::Regular));
        assert!(get_veterancy_level_flag(flags, VeterancyLevel::Veteran));
        assert!(get_veterancy_level_flag(flags, VeterancyLevel::Elite));
        assert!(get_veterancy_level_flag(flags, VeterancyLevel::Heroic));
    }

    #[test]
    fn test_object_status_mask() {
        let mask = ObjectStatusMask::ALL;
        assert!(mask.contains(ObjectStatusMask::ALL));

        let none = ObjectStatusMask::NONE;
        assert!(!none.intersects(ObjectStatusMask::ALL));
    }
}
