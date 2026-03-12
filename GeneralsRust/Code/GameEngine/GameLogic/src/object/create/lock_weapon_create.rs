//! LockWeaponCreate module - Locks the weapon choice to the slot specified on creation
//!
//! C++ Source: GameLogic/Object/Create/LockWeaponCreate.cpp

use std::sync::Arc;

use crate::helpers::TheGameLogic;
use crate::object::create::{CreateModule, CreateModuleData};
use crate::weapon::{WeaponLockType, WeaponSlotType};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{CreateInterface, ModuleData, Thing as ThingTrait};

/// Data structure for LockWeaponCreate module
#[derive(Debug, Clone)]
pub struct LockWeaponCreateModuleData {
    pub base: CreateModuleData,
    pub slot_to_lock: WeaponSlotType,
}

impl Default for LockWeaponCreateModuleData {
    fn default() -> Self {
        Self {
            base: CreateModuleData::new(),
            slot_to_lock: WeaponSlotType::Primary,
        }
    }
}

impl LockWeaponCreateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, LOCK_WEAPON_CREATE_FIELDS)
    }
}

impl ModuleData for LockWeaponCreateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        ModuleData::set_module_tag_name_key(&mut self.base, key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        ModuleData::get_module_tag_name_key(&self.base)
    }
}

impl Snapshotable for LockWeaponCreateModuleData {
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

/// LockWeaponCreate module implementation
#[derive(Debug)]
pub struct LockWeaponCreate {
    base: CreateModule,
    module_data: Arc<LockWeaponCreateModuleData>,
}

impl LockWeaponCreate {
    pub fn new(thing: Arc<dyn ThingTrait>, module_data: Arc<LockWeaponCreateModuleData>) -> Self {
        Self {
            base: CreateModule::new(thing),
            module_data,
        }
    }
}

impl CreateInterface for LockWeaponCreate {
    fn on_create(&self) {}

    fn on_build_complete(&self) {
        self.base.on_build_complete();

        let object_id = self
            .base
            .get_thing()
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or_default();
        if object_id == 0 {
            return;
        }

        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let Ok(mut obj_guard) = object_arc.write() else {
            return;
        };
        obj_guard.set_weapon_lock(
            self.module_data.slot_to_lock,
            WeaponLockType::LockedPermanently,
        );
    }

    fn should_do_on_build_complete(&self) -> bool {
        self.base.should_do_on_build_complete()
    }
}

impl Snapshotable for LockWeaponCreate {
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

fn parse_slot_to_lock(
    _ini: &mut INI,
    data: &mut LockWeaponCreateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let slot = match token.trim().to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => WeaponSlotType::Primary,
        "SECONDARY" | "SECONDARY_WEAPON" => WeaponSlotType::Secondary,
        "TERTIARY" | "TERTIARY_WEAPON" => WeaponSlotType::Tertiary,
        _ => return Err(INIError::InvalidData),
    };
    data.slot_to_lock = slot;
    Ok(())
}

const LOCK_WEAPON_CREATE_FIELDS: &[FieldParse<LockWeaponCreateModuleData>] = &[FieldParse {
    token: "SlotToLock",
    parse: parse_slot_to_lock,
}];
