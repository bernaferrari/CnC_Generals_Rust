//! FireOCLAfterWeaponCooldownUpdate - Spawns objects after weapon fires
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, ModuleData, UnsignedInt, LOGICFRAMES_PER_SECOND, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::effects::ObjectCreationList;
use crate::helpers::{TheGameLogic, TheObjectCreationListStore};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use crate::upgrade::{UpgradeMask, UpgradeMux, UpgradeMuxData};
use crate::weapon::WeaponSlotType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct FireOCLAfterWeaponCooldownUpdateModuleData {
    pub base: BehaviorModuleData,
    pub weapon_slot: WeaponSlotType,
    pub min_shots_required: UnsignedInt,
    pub ocl_lifetime_per_second: UnsignedInt,
    pub ocl_max_frames: UnsignedInt,
    pub ocl: Option<Arc<ObjectCreationList>>,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for FireOCLAfterWeaponCooldownUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            weapon_slot: WeaponSlotType::Primary,
            min_shots_required: 1,
            ocl_lifetime_per_second: 1000,
            ocl_max_frames: 1000,
            ocl: None,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(FireOCLAfterWeaponCooldownUpdateModuleData, base);

impl FireOCLAfterWeaponCooldownUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_OCL_AFTER_WEAPON_COOLDOWN_FIELDS)
    }
}

pub struct FireOCLAfterWeaponCooldownUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<FireOCLAfterWeaponCooldownUpdateModuleData>,
    valid: bool,
    consecutive_shots: UnsignedInt,
    start_frame: UnsignedInt,
    next_call_frame_and_phase: UnsignedInt,
    upgrade_mux: UpgradeMux,
}

impl FireOCLAfterWeaponCooldownUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<FireOCLAfterWeaponCooldownUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let upgrade_mux = UpgradeMux::new(specific_data.upgrade_mux_data.clone());

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            valid: false,
            consecutive_shots: 0,
            start_frame: 0,
            next_call_frame_and_phase: 0,
            upgrade_mux,
        })
    }

    fn reset_stats(&mut self) {
        self.consecutive_shots = 0;
        self.start_frame = 0;
    }

    fn fire_ocl(&mut self, obj: &Arc<RwLock<GameObject>>, now: UnsignedInt) {
        let Some(ocl) = self.module_data.ocl.as_ref() else {
            self.reset_stats();
            return;
        };

        let elapsed_frames = now.saturating_sub(self.start_frame);
        let mut seconds = elapsed_frames as f32 * SECONDS_PER_LOGICFRAME_REAL;
        seconds *= self.module_data.ocl_lifetime_per_second as f32 * 0.001;
        let mut ocl_frames = (seconds * LOGICFRAMES_PER_SECOND as f32) as UnsignedInt;
        ocl_frames = ocl_frames.min(self.module_data.ocl_max_frames);

        if let Ok(obj_guard) = obj.read() {
            let ctx = crate::object_creation_list::live_creation_context();
            let _ = ocl.create_with_objects(&ctx, &*obj_guard, Some(&*obj_guard), ocl_frames);
        }

        self.reset_stats();
    }

    fn build_upgrade_mask(&self, obj: &GameObject) -> UpgradeMask {
        let mut mask = obj.completed_upgrades();
        if let Some(player) = obj.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                mask |= player_guard.get_completed_upgrade_mask();
            }
        }
        UpgradeMask::from_bits_retain(mask.bits())
    }
}

impl UpdateModuleInterface for FireOCLAfterWeaponCooldownUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj_arc) = self.object.upgrade() else {
            return Ok(UpdateSleepTime::Forever);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(UpdateSleepTime::None);
        };

        let mut valid_this_frame = true;
        let mut valid_to_fire_ocl = true;
        let mut last_shot_frame = 0u32;
        let mut possible_next_shot_frame = 0u32;
        let has_slot_weapon = obj
            .get_weapon_in_weapon_slot(self.module_data.weapon_slot)
            .is_some();

        if let Some((weapon, slot)) = obj.get_current_weapon() {
            if slot != self.module_data.weapon_slot {
                valid_this_frame = false;
            } else {
                last_shot_frame = weapon.get_last_shot_frame();
                possible_next_shot_frame = weapon.get_possible_next_shot_frame();
            }
        } else {
            valid_this_frame = false;
        }

        let upgrade_mask = self.build_upgrade_mask(&obj);
        if valid_this_frame && !self.upgrade_mux.test_upgrade_conditions(upgrade_mask) {
            valid_this_frame = false;
            valid_to_fire_ocl = false;
        }

        let now = TheGameLogic::get_frame();
        if valid_this_frame {
            if last_shot_frame == now.saturating_sub(1) {
                self.consecutive_shots += 1;
                if self.consecutive_shots == 1 {
                    self.start_frame = now;
                }
            } else if possible_next_shot_frame < now {
                if self.module_data.min_shots_required <= self.consecutive_shots {
                    drop(obj);
                    self.fire_ocl(&obj_arc, now);
                }
            }
        } else if valid_to_fire_ocl {
            if has_slot_weapon && self.module_data.min_shots_required <= self.consecutive_shots {
                drop(obj);
                self.fire_ocl(&obj_arc, now);
            }
        }

        if valid_this_frame != self.valid {
            self.valid = valid_this_frame;
            self.reset_stats();
        }

        Ok(UpdateSleepTime::None)
    }
}

impl BehaviorModuleInterface for FireOCLAfterWeaponCooldownUpdate {
    fn get_module_name(&self) -> &'static str {
        "FireOCLAfterWeaponCooldownUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for FireOCLAfterWeaponCooldownUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.upgrade_mux.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer update base: {}", e))?;
        self.upgrade_mux.xfer(xfer)?;
        xfer.xfer_bool(&mut self.valid)
            .map_err(|e| format!("Failed to xfer valid: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.consecutive_shots)
            .map_err(|e| format!("Failed to xfer consecutive shots: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.start_frame)
            .map_err(|e| format!("Failed to xfer start frame: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.upgrade_mux.load_post_process()
    }
}

/// Glue that exposes FireOCLAfterWeaponCooldownUpdate through the common Module trait.
pub struct FireOCLAfterWeaponCooldownUpdateModule {
    behavior: FireOCLAfterWeaponCooldownUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<FireOCLAfterWeaponCooldownUpdateModuleData>,
}

impl FireOCLAfterWeaponCooldownUpdateModule {
    pub fn new(
        behavior: FireOCLAfterWeaponCooldownUpdate,
        module_name: &AsciiString,
        module_data: Arc<FireOCLAfterWeaponCooldownUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut FireOCLAfterWeaponCooldownUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for FireOCLAfterWeaponCooldownUpdateModule {
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

impl Module for FireOCLAfterWeaponCooldownUpdateModule {
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

pub struct FireOCLAfterWeaponCooldownUpdateFactory;
impl FireOCLAfterWeaponCooldownUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(FireOCLAfterWeaponCooldownUpdate::new(
            thing,
            module_data,
        )?))
    }
}

fn parse_weapon_slot(
    _ini: &mut INI,
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let slot = match token.to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => WeaponSlotType::Primary,
        "SECONDARY" | "SECONDARY_WEAPON" => WeaponSlotType::Secondary,
        "TERTIARY" | "TERTIARY_WEAPON" => WeaponSlotType::Tertiary,
        _ => return Err(INIError::InvalidData),
    };
    data.weapon_slot = slot;
    Ok(())
}

fn parse_ocl(
    _ini: &mut INI,
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.ocl = TheObjectCreationListStore::find_object_creation_list(token);
    if data.ocl.is_none() {
        log::warn!(
            "FireOCLAfterWeaponCooldownUpdate: unresolved OCL '{}'",
            token
        );
    }
    Ok(())
}

fn parse_min_shots(
    _ini: &mut INI,
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.min_shots_required = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_ocl_lifetime_per_second(
    _ini: &mut INI,
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.ocl_lifetime_per_second = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_ocl_lifetime_max(
    _ini: &mut INI,
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.ocl_max_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
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
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
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
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
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
    data: &mut FireOCLAfterWeaponCooldownUpdateModuleData,
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

const FIRE_OCL_AFTER_WEAPON_COOLDOWN_FIELDS: &[FieldParse<
    FireOCLAfterWeaponCooldownUpdateModuleData,
>] = &[
    FieldParse {
        token: "WeaponSlot",
        parse: parse_weapon_slot,
    },
    FieldParse {
        token: "OCL",
        parse: parse_ocl,
    },
    FieldParse {
        token: "MinShotsToCreateOCL",
        parse: parse_min_shots,
    },
    FieldParse {
        token: "OCLLifetimePerSecond",
        parse: parse_ocl_lifetime_per_second,
    },
    FieldParse {
        token: "OCLLifetimeMaxCap",
        parse: parse_ocl_lifetime_max,
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
