//! DemoTrapUpdate - Rust conversion of C++ DemoTrapUpdate
//!
//! Handles proximity triggering of demo traps, manual detonation, and weapon slot switching.
//! Author: Kris Morness, August 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, KindOfMask, ModuleData, ObjectID, ObjectStatusTypes, Real,
    TheGameLogic, UnsignedInt, INVALID_ID,
};
use crate::helpers::ThePartitionManager;
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_FOREVER,
    UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::contain::open_contain::ObjectRelationship;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use crate::weapon::{
    with_weapon_store, DamageType as WeaponDamageType, WeaponLockType, WeaponSetType,
    WeaponSlotType,
};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};
use log::warn;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct DemoTrapUpdateModuleData {
    pub base: BehaviorModuleData,
    pub detonation_weapon_name: String,
    pub ignore_kind_of: KindOfMask,
    pub manual_mode_weapon_slot: WeaponSlotType,
    pub detonation_weapon_slot: WeaponSlotType,
    pub proximity_mode_weapon_slot: WeaponSlotType,
    pub trigger_detonation_range: Real,
    pub scan_frames: u32,
    pub defaults_to_proximity_mode: Bool,
    pub friendly_detonation: Bool,
    pub detonate_when_killed: Bool,
}

impl Default for DemoTrapUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            detonation_weapon_name: String::new(),
            ignore_kind_of: KindOfMask::default(),
            manual_mode_weapon_slot: WeaponSlotType::Primary,
            detonation_weapon_slot: WeaponSlotType::Primary,
            proximity_mode_weapon_slot: WeaponSlotType::Primary,
            trigger_detonation_range: 0.0,
            scan_frames: 0,
            defaults_to_proximity_mode: false,
            friendly_detonation: false,
            detonate_when_killed: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(DemoTrapUpdateModuleData, base);

impl DemoTrapUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DEMO_TRAP_UPDATE_FIELDS)
    }
}

fn parse_default_proximity_mode(
    _ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.defaults_to_proximity_mode = INI::parse_bool(token)?;
    Ok(())
}

fn parse_weapon_slot(
    _ini: &mut INI,
    out: &mut WeaponSlotType,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let slot = match token.to_ascii_uppercase().as_str() {
        "PRIMARY" | "PRIMARY_WEAPON" => WeaponSlotType::Primary,
        "SECONDARY" | "SECONDARY_WEAPON" => WeaponSlotType::Secondary,
        "TERTIARY" | "TERTIARY_WEAPON" => WeaponSlotType::Tertiary,
        _ => return Err(INIError::InvalidData),
    };
    *out = slot;
    Ok(())
}

fn parse_detonation_weapon_slot(
    ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_weapon_slot(ini, &mut data.detonation_weapon_slot, tokens)
}

fn parse_proximity_mode_weapon_slot(
    ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_weapon_slot(ini, &mut data.proximity_mode_weapon_slot, tokens)
}

fn parse_manual_mode_weapon_slot(
    ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_weapon_slot(ini, &mut data.manual_mode_weapon_slot, tokens)
}

fn parse_trigger_detonation_range(
    _ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.trigger_detonation_range = INI::parse_real(token)?;
    Ok(())
}

fn parse_ignore_target_types(
    _ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.ignore_kind_of = crate::object::behavior::auto_heal_behavior::parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_scan_rate(
    _ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.scan_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_friendly_detonation(
    _ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.friendly_detonation = INI::parse_bool(token)?;
    Ok(())
}

fn parse_detonation_weapon(
    _ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.detonation_weapon_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_detonate_when_killed(
    _ini: &mut INI,
    data: &mut DemoTrapUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.detonate_when_killed = INI::parse_bool(token)?;
    Ok(())
}

const DEMO_TRAP_UPDATE_FIELDS: &[FieldParse<DemoTrapUpdateModuleData>] = &[
    FieldParse {
        token: "DefaultProximityMode",
        parse: parse_default_proximity_mode,
    },
    FieldParse {
        token: "DetonationWeaponSlot",
        parse: parse_detonation_weapon_slot,
    },
    FieldParse {
        token: "ProximityModeWeaponSlot",
        parse: parse_proximity_mode_weapon_slot,
    },
    FieldParse {
        token: "ManualModeWeaponSlot",
        parse: parse_manual_mode_weapon_slot,
    },
    FieldParse {
        token: "TriggerDetonationRange",
        parse: parse_trigger_detonation_range,
    },
    FieldParse {
        token: "IgnoreTargetTypes",
        parse: parse_ignore_target_types,
    },
    FieldParse {
        token: "ScanRate",
        parse: parse_scan_rate,
    },
    FieldParse {
        token: "AutoDetonationWithFriendsInvolved",
        parse: parse_friendly_detonation,
    },
    FieldParse {
        token: "DetonationWeapon",
        parse: parse_detonation_weapon,
    },
    FieldParse {
        token: "DetonateWhenKilled",
        parse: parse_detonate_when_killed,
    },
];

pub struct DemoTrapUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<DemoTrapUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    next_scan_frames: i32,
    detonated: bool,
}

impl DemoTrapUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<DemoTrapUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            next_scan_frames: 0,
            detonated: false,
        })
    }

    pub fn detonate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.detonated {
            return Ok(());
        }

        let Some(me_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let mut me = me_arc.write().unwrap();

        // Only shoot the weapon if not being built or sold.
        if !me.test_status(ObjectStatusTypes::UnderConstruction)
            && !me.test_status(ObjectStatusTypes::Sold)
        {
            let weapon_name = &self.module_data.detonation_weapon_name;
            let me_id = me.get_id();
            let me_pos = *me.get_position();

            // Use global weapon store to fire the detonation weapon
            let _ = with_weapon_store(|store| {
                if let Some(template) = store.find_weapon_template(weapon_name) {
                    let _ = store.create_and_fire_temp_weapon(template, me_id, None, Some(&me_pos));
                }
            });
        }

        me.kill(None, None);
        self.detonated = true;
        Ok(())
    }
}

impl UpdateModuleInterface for DemoTrapUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.detonated {
            return UPDATE_SLEEP_FOREVER;
        }

        let Some(me_arc) = self.object.upgrade() else {
            return UPDATE_SLEEP_FOREVER;
        };
        let me = me_arc.read().unwrap();

        if me.test_status(ObjectStatusTypes::UnderConstruction)
            || me.test_status(ObjectStatusTypes::Sold)
        {
            return UPDATE_SLEEP_FOREVER;
        }

        if me.is_effectively_dead() {
            if self.module_data.detonate_when_killed {
                drop(me);
                let _ = self.detonate();
            }
            return UPDATE_SLEEP_FOREVER;
        }

        // Get the current weapon slot -- this determines what mode we're in.
        let weapon_slot = if let Some((_weapon, slot)) = me.get_current_weapon() {
            slot
        } else {
            WeaponSlotType::Primary
        };

        if weapon_slot == self.module_data.detonation_weapon_slot {
            // We've been externally triggered by the press of a command button.
            drop(me);
            let _ = self.detonate();
            return UPDATE_SLEEP_FOREVER;
        }

        // Don't scan every frame for performance reasons.
        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            return UPDATE_SLEEP_FOREVER;
        }

        if weapon_slot == self.module_data.manual_mode_weapon_slot {
            // Don't scan!
            return UPDATE_SLEEP_FOREVER;
        }

        // Reset timer here -- because if we are in manual mode, and switch, we want instant
        // gratification (if possible).
        self.next_scan_frames = self.module_data.scan_frames as i32;

        // Scan for a valid enemy in proximity range.
        let me_pos = *me.get_position();
        let range = self.module_data.trigger_detonation_range;

        let candidates = if let Some(pm) = ThePartitionManager::get() {
            pm.get_objects_in_range(&me_pos, range)
        } else {
            Vec::new()
        };

        let mut shall_detonate = false;

        for other_id in candidates {
            if let Some(other_arc) = OBJECT_REGISTRY.get_object(other_id) {
                let other = other_arc.read().unwrap();

                if (other.get_kind_of() & self.module_data.ignore_kind_of) != 0 {
                    continue;
                }
                if other.is_effectively_dead() {
                    continue;
                }

                // Check for dozers disarming
                if other.is_kind_of(crate::common::KindOf::Dozer) {
                    if let Some((weapon, _slot)) = other.get_current_weapon() {
                        if weapon.get_damage_type() == WeaponDamageType::Disarm {
                            if other.test_status(ObjectStatusTypes::IsAttacking) {
                                continue;
                            }
                        }
                    }
                }

                // order matters: we want to know if I consider it to be an enemy, not vice versa
                if me.get_relationship_to(&other) != ObjectRelationship::Enemy {
                    if !self.module_data.friendly_detonation {
                        // Not allowed to proximity detonate with friends nearby
                        return UPDATE_SLEEP_FOREVER;
                    }
                    // Don't shoot our friends!
                    continue;
                }

                if other.is_above_terrain() {
                    // Don't detonate on anything airborne.
                    continue;
                }

                // Anyone close enough?
                // we've already filtered by radius in candidates, but C++ does a squared distance check again
                let dx = other.get_position().x - me_pos.x;
                let dy = other.get_position().y - me_pos.y;
                let dist_sqr = dx * dx + dy * dy;

                if dist_sqr <= range * range {
                    shall_detonate = true;
                    if self.module_data.friendly_detonation {
                        break;
                    }
                }
            }
        }

        if shall_detonate {
            drop(me);
            let _ = self.detonate();
        }

        UPDATE_SLEEP_FOREVER
    }
}

impl Snapshotable for DemoTrapUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("DemoTrapUpdate xfer version failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_i32(&mut self.next_scan_frames)
            .map_err(|e| format!("DemoTrapUpdate xfer scan frames failed: {:?}", e))?;
        xfer.xfer_bool(&mut self.detonated)
            .map_err(|e| format!("DemoTrapUpdate xfer detonated failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes DemoTrapUpdate through the common Module trait.
pub struct DemoTrapUpdateModule {
    behavior: DemoTrapUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<DemoTrapUpdateModuleData>,
}

impl DemoTrapUpdateModule {
    pub fn new(
        behavior: DemoTrapUpdate,
        module_name: &AsciiString,
        module_data: Arc<DemoTrapUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut DemoTrapUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for DemoTrapUpdateModule {
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

impl Module for DemoTrapUpdateModule {
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

impl BehaviorModuleInterface for DemoTrapUpdate {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.update_simple();
        Ok(())
    }

    fn get_module_name(&self) -> &'static str {
        "DemoTrapUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(me_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let mut me = me_arc.write().unwrap();

        me.set_weapon_set_flag(WeaponSetType::Veteran);

        if self.module_data.defaults_to_proximity_mode {
            me.set_weapon_lock(
                self.module_data.proximity_mode_weapon_slot,
                WeaponLockType::LockedTemporarily,
            );
        } else {
            me.set_weapon_lock(
                self.module_data.manual_mode_weapon_slot,
                WeaponLockType::LockedTemporarily,
            );
        }
        Ok(())
    }
}

pub struct DemoTrapUpdateFactory;
impl DemoTrapUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(DemoTrapUpdate::new(thing, module_data)?))
    }
}

pub fn demo_trap_update_data_factory(ini: Option<&mut INI>) -> Box<dyn EngineModuleData> {
    let mut data = DemoTrapUpdateModuleData::default();
    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DemoTrapUpdate module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }
    Box::new(data)
}

pub fn demo_trap_update_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn EngineModuleData>,
) -> Box<dyn Module> {
    let typed_data = module_data
        .as_any()
        .downcast_ref::<DemoTrapUpdateModuleData>()
        .expect("DemoTrapUpdateModuleData expected");
    let module_data_arc = Arc::new(typed_data.clone());
    let owner_id = thing
        .as_object()
        .map(ModuleObject::get_object_id)
        .unwrap_or(INVALID_ID);
    let object = TheGameLogic::find_object_by_id(owner_id).expect("DemoTrapUpdate requires object");
    let behavior = DemoTrapUpdate::new(object, module_data_arc.clone())
        .expect("DemoTrapUpdate failed to initialize");
    let module_name = AsciiString::from("DemoTrapUpdate");
    Box::new(DemoTrapUpdateModule::new(
        behavior,
        &module_name,
        module_data_arc,
    ))
}
