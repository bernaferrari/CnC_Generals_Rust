//! FireWeaponWhenDamagedBehavior - Rust conversion of C++ FireWeaponWhenDamagedBehavior
//!
//! Fires weapons when the object takes damage, with different weapons based on damage state.
//! Original C++: FireWeaponWhenDamagedBehavior.cpp
//! Rust conversion: 2025
//!
//! FILE: FireWeaponWhenDamagedBehavior.cpp lines 1-342

use crate::common::{AsciiString, Bool, ModuleData, Real, UnsignedInt, XferVersion};
use crate::damage::{
    get_damage_type_flag, BodyDamageType, DamageInfo, DamageType, DamageTypeFlags,
};
use crate::helpers::TheGameLogic;
use crate::modules::{
    BehaviorModuleInterface, DamageModuleInterface, UpdateModuleInterface, UpdateSleepTime,
    UpgradeModuleInterface,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use crate::upgrade::{UpgradeMask, UpgradeMux, UpgradeMuxData};
use crate::weapon::with_weapon_store;
use crate::weapon::{Weapon, WeaponSlotType, WeaponStatus, WeaponTemplate};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock, Weak};

const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

/// FireWeaponWhenDamagedBehaviorModuleData - Configuration
/// Matches C++ FireWeaponWhenDamagedBehavior.h module data structure
#[derive(Clone, Debug)]
pub struct FireWeaponWhenDamagedBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub upgrade_mux_data: UpgradeMuxData,

    // Reaction weapons - fire once on damage. Matches C++ lines 50-73
    pub reaction_weapon_pristine: Option<Arc<WeaponTemplate>>,
    pub reaction_weapon_damaged: Option<Arc<WeaponTemplate>>,
    pub reaction_weapon_really_damaged: Option<Arc<WeaponTemplate>>,
    pub reaction_weapon_rubble: Option<Arc<WeaponTemplate>>,

    // Continuous weapons - fire repeatedly. Matches C++ lines 76-99
    pub continuous_weapon_pristine: Option<Arc<WeaponTemplate>>,
    pub continuous_weapon_damaged: Option<Arc<WeaponTemplate>>,
    pub continuous_weapon_really_damaged: Option<Arc<WeaponTemplate>>,
    pub continuous_weapon_rubble: Option<Arc<WeaponTemplate>>,

    // Damage filter. Matches C++ line 154
    pub damage_types: DamageTypeFlags, // Bitmask of damage types to respond to
    pub damage_amount: Real,           // Minimum damage to trigger. Matches C++ line 159
    pub initially_active: Bool,        // Matches C++ line 101
}

impl Default for FireWeaponWhenDamagedBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            upgrade_mux_data: UpgradeMuxData::default(),
            reaction_weapon_pristine: None,
            reaction_weapon_damaged: None,
            reaction_weapon_really_damaged: None,
            reaction_weapon_rubble: None,
            continuous_weapon_pristine: None,
            continuous_weapon_damaged: None,
            continuous_weapon_really_damaged: None,
            continuous_weapon_rubble: None,
            damage_types: DamageTypeFlags::all_flags(),
            damage_amount: 0.0,
            initially_active: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(FireWeaponWhenDamagedBehaviorModuleData, base);

impl FireWeaponWhenDamagedBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_WEAPON_WHEN_DAMAGED_FIELDS)
    }
}

fn parse_starts_active(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.initially_active = INI::parse_bool(token)?;
    Ok(())
}

fn parse_weapon_template(token: &str) -> Option<Arc<WeaponTemplate>> {
    with_weapon_store(|store| store.find_weapon_template(token).cloned())
        .ok()
        .flatten()
}

fn parse_reaction_weapon_pristine(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.reaction_weapon_pristine = parse_weapon_template(token);
    Ok(())
}

fn parse_reaction_weapon_damaged(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.reaction_weapon_damaged = parse_weapon_template(token);
    Ok(())
}

fn parse_reaction_weapon_really_damaged(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.reaction_weapon_really_damaged = parse_weapon_template(token);
    Ok(())
}

fn parse_reaction_weapon_rubble(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.reaction_weapon_rubble = parse_weapon_template(token);
    Ok(())
}

fn parse_continuous_weapon_pristine(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.continuous_weapon_pristine = parse_weapon_template(token);
    Ok(())
}

fn parse_continuous_weapon_damaged(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.continuous_weapon_damaged = parse_weapon_template(token);
    Ok(())
}

fn parse_continuous_weapon_really_damaged(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.continuous_weapon_really_damaged = parse_weapon_template(token);
    Ok(())
}

fn parse_continuous_weapon_rubble(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.continuous_weapon_rubble = parse_weapon_template(token);
    Ok(())
}

fn parse_damage_types(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut flags = DamageTypeFlags::empty();
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = DamageTypeFlags::empty();
                continue;
            }

            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };

            if let Ok(damage_type) = DamageType::from_str(name) {
                let flag = DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
                if remove {
                    flags.remove(flag);
                } else {
                    flags.insert(flag);
                }
            }
        }
    }

    data.damage_types = flags;
    Ok(())
}

fn parse_damage_amount(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.damage_amount = INI::parse_real(token)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
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
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
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
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
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
    data: &mut FireWeaponWhenDamagedBehaviorModuleData,
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

const FIRE_WEAPON_WHEN_DAMAGED_FIELDS: &[FieldParse<FireWeaponWhenDamagedBehaviorModuleData>] = &[
    FieldParse {
        token: "StartsActive",
        parse: parse_starts_active,
    },
    FieldParse {
        token: "ReactionWeaponPristine",
        parse: parse_reaction_weapon_pristine,
    },
    FieldParse {
        token: "ReactionWeaponDamaged",
        parse: parse_reaction_weapon_damaged,
    },
    FieldParse {
        token: "ReactionWeaponReallyDamaged",
        parse: parse_reaction_weapon_really_damaged,
    },
    FieldParse {
        token: "ReactionWeaponRubble",
        parse: parse_reaction_weapon_rubble,
    },
    FieldParse {
        token: "ContinuousWeaponPristine",
        parse: parse_continuous_weapon_pristine,
    },
    FieldParse {
        token: "ContinuousWeaponDamaged",
        parse: parse_continuous_weapon_damaged,
    },
    FieldParse {
        token: "ContinuousWeaponReallyDamaged",
        parse: parse_continuous_weapon_really_damaged,
    },
    FieldParse {
        token: "ContinuousWeaponRubble",
        parse: parse_continuous_weapon_rubble,
    },
    FieldParse {
        token: "DamageTypes",
        parse: parse_damage_types,
    },
    FieldParse {
        token: "DamageAmount",
        parse: parse_damage_amount,
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

/// FireWeaponWhenDamagedBehavior - Fires weapons when damaged
/// Matches C++ FireWeaponWhenDamagedBehavior.cpp lines 35-342
pub struct FireWeaponWhenDamagedBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<FireWeaponWhenDamagedBehaviorModuleData>,

    // Reaction weapons (fire once on damage). Matches C++ lines 37-44
    reaction_weapon_pristine: Option<Arc<Mutex<Weapon>>>,
    reaction_weapon_damaged: Option<Arc<Mutex<Weapon>>>,
    reaction_weapon_really_damaged: Option<Arc<Mutex<Weapon>>>,
    reaction_weapon_rubble: Option<Arc<Mutex<Weapon>>>,

    // Continuous weapons (fire repeatedly). Matches C++ lines 41-44
    continuous_weapon_pristine: Option<Arc<Mutex<Weapon>>>,
    continuous_weapon_damaged: Option<Arc<Mutex<Weapon>>>,
    continuous_weapon_really_damaged: Option<Arc<Mutex<Weapon>>>,
    continuous_weapon_rubble: Option<Arc<Mutex<Weapon>>>,

    next_call_frame_and_phase: UnsignedInt,
    upgrade_mux: UpgradeMux,
}

impl FireWeaponWhenDamagedBehavior {
    /// Creates new FireWeaponWhenDamagedBehavior. Matches C++ lines 35-119
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<FireWeaponWhenDamagedBehaviorModuleData>()
            .ok_or("Invalid module data for FireWeaponWhenDamagedBehavior")?;

        let data = Arc::new(specific_data.clone());
        let object_id = object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(crate::common::INVALID_ID);

        // Allocate reaction weapons. Matches C++ lines 50-73
        let reaction_weapon_pristine = data
            .reaction_weapon_pristine
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));
        let reaction_weapon_damaged = data
            .reaction_weapon_damaged
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));
        let reaction_weapon_really_damaged = data
            .reaction_weapon_really_damaged
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));
        let reaction_weapon_rubble = data
            .reaction_weapon_rubble
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));

        // Allocate continuous weapons. Matches C++ lines 76-99
        let continuous_weapon_pristine = data
            .continuous_weapon_pristine
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));
        let continuous_weapon_damaged = data
            .continuous_weapon_damaged
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));
        let continuous_weapon_really_damaged = data
            .continuous_weapon_really_damaged
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));
        let continuous_weapon_rubble = data
            .continuous_weapon_rubble
            .as_ref()
            .map(|tmpl| Self::allocate_weapon(tmpl.clone(), object_id));

        let mut upgrade_mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        if data.initially_active {
            if let Ok(mut obj_guard) = object.write() {
                upgrade_mux.data.perform_upgrade_fx(&mut obj_guard);
                upgrade_mux.data.process_upgrade_removal(&mut obj_guard);
            }
            upgrade_mux.set_upgrade_executed(true);
        }

        let has_continuous_weapon = continuous_weapon_pristine.is_some()
            || continuous_weapon_damaged.is_some()
            || continuous_weapon_really_damaged.is_some()
            || continuous_weapon_rubble.is_some();

        if let Ok(obj_guard) = object.read() {
            let should_wake = upgrade_mux.is_already_upgraded() && has_continuous_weapon;
            let sleep_time = if should_wake {
                UPDATE_SLEEP_NONE
            } else {
                UPDATE_SLEEP_FOREVER
            };
            TheGameLogic::set_wake_frame(obj_guard.get_id(), sleep_time);
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: data,
            reaction_weapon_pristine,
            reaction_weapon_damaged,
            reaction_weapon_really_damaged,
            reaction_weapon_rubble,
            continuous_weapon_pristine,
            continuous_weapon_damaged,
            continuous_weapon_really_damaged,
            continuous_weapon_rubble,
            next_call_frame_and_phase: 0,
            upgrade_mux,
        })
    }

    /// Allocate a weapon from template. Matches C++ TheWeaponStore->allocateNewWeapon()
    /// from FireWeaponWhenDamagedBehavior.cpp lines 52-99
    fn allocate_weapon(
        template: Arc<WeaponTemplate>,
        object_id: crate::common::ObjectID,
    ) -> Arc<Mutex<Weapon>> {
        // Create new weapon instance from template, using PRIMARY_WEAPON slot
        // This matches C++ line 53: TheWeaponStore->allocateNewWeapon(d->m_reactionWeaponPristine, PRIMARY_WEAPON)
        let weapon = Weapon::new(template, WeaponSlotType::Primary);

        // Wrap in Arc<Mutex<>> for thread-safe shared ownership
        // The weapon will be reloaded via load_ammo_now() or reload_ammo() when needed
        let weapon = Arc::new(Mutex::new(weapon));
        if object_id != crate::common::INVALID_ID {
            if let Ok(mut guard) = weapon.lock() {
                let _ = guard.load_ammo_now(object_id);
            }
        }
        weapon
    }

    /// Fire reaction weapon based on damage state. Matches C++ lines 166-194
    fn fire_reaction_weapon(
        &mut self,
        body_damage_type: BodyDamageType,
        obj_id: crate::common::ObjectID,
        position: &crate::common::Coord3D,
    ) {
        let weapon = match body_damage_type {
            BodyDamageType::Rubble => &self.reaction_weapon_rubble, // Matches C++ lines 166-171
            BodyDamageType::ReallyDamaged => &self.reaction_weapon_really_damaged, // Matches C++ lines 173-178
            BodyDamageType::Damaged => &self.reaction_weapon_damaged, // Matches C++ lines 180-185
            _ => &self.reaction_weapon_pristine, // Matches C++ lines 187-192 (pristine/undamaged)
        };

        if let Some(weapon_arc) = weapon {
            if let Ok(mut weapon_guard) = weapon_arc.lock() {
                if weapon_guard.get_status() == WeaponStatus::ReadyToFire {
                    // Matches C++ line 169: m_reactionWeaponPristine->forceFireWeapon( obj, obj->getPosition() )
                    let _ = weapon_guard.force_fire_weapon(obj_id, position);
                }
            }
        }
    }

    /// Fire continuous weapon based on damage state. Matches C++ lines 200-242
    fn fire_continuous_weapon(
        &mut self,
        body_damage_type: BodyDamageType,
        obj_id: crate::common::ObjectID,
        position: &crate::common::Coord3D,
    ) {
        let weapon = match body_damage_type {
            BodyDamageType::Rubble => &self.continuous_weapon_rubble, // Matches C++ lines 211-216
            BodyDamageType::ReallyDamaged => &self.continuous_weapon_really_damaged, // Matches C++ lines 219-224
            BodyDamageType::Damaged => &self.continuous_weapon_damaged, // Matches C++ lines 226-231
            _ => &self.continuous_weapon_pristine, // Matches C++ lines 233-238 (pristine)
        };

        if let Some(weapon_arc) = weapon {
            if let Ok(mut weapon_guard) = weapon_arc.lock() {
                if weapon_guard.get_status() == WeaponStatus::ReadyToFire {
                    // Matches C++ line 215: m_continuousWeaponPristine->forceFireWeapon( obj, obj->getPosition() )
                    let _ = weapon_guard.force_fire_weapon(obj_id, position);
                }
            }
        }
    }

    fn set_wake_frame(&self, sleep_time: UpdateSleepTime) {
        if let Some(obj) = self.object.upgrade() {
            if let Ok(obj_guard) = obj.read() {
                TheGameLogic::set_wake_frame(obj_guard.get_id(), sleep_time);
            }
        }
    }

    fn ensure_weapon_for_xfer(
        slot_template: &Option<Arc<WeaponTemplate>>,
        object_id: crate::common::ObjectID,
    ) -> Result<Arc<Mutex<Weapon>>, String> {
        let Some(template) = slot_template.as_ref() else {
            return Err("Weapon snapshot present but template missing".to_string());
        };
        Ok(Self::allocate_weapon(template.clone(), object_id))
    }

    fn xfer_weapon_option(
        xfer: &mut dyn Xfer,
        weapon: &mut Option<Arc<Mutex<Weapon>>>,
        template: &Option<Arc<WeaponTemplate>>,
        object_id: crate::common::ObjectID,
    ) -> Result<(), String> {
        let mut has_weapon = weapon.is_some();
        xfer.xfer_bool(&mut has_weapon)
            .map_err(|e| format!("Failed to xfer weapon presence: {:?}", e))?;

        if has_weapon {
            if weapon.is_none() {
                *weapon = Some(Self::ensure_weapon_for_xfer(template, object_id)?);
            }
            if let Some(ref weapon_arc) = weapon {
                if let Ok(mut weapon_guard) = weapon_arc.lock() {
                    weapon_guard.xfer(xfer)?;
                }
            }
        } else {
            *weapon = None;
        }

        Ok(())
    }
}

impl DamageModuleInterface for FireWeaponWhenDamagedBehavior {
    /// Called when damage is received. Matches C++ lines 147-195
    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.upgrade_mux.is_already_upgraded() {
            return Ok(()); // Matches C++ lines 149-150
        }

        let data = &self.module_data;

        // Check damage type filter. Matches C++ lines 154-156
        if !get_damage_type_flag(data.damage_types, damage_info.input.damage_type) {
            return Ok(());
        }

        // Check damage amount (use actual post-armor damage). Matches C++ lines 158-160
        if damage_info.output.actual_damage_dealt < data.damage_amount {
            return Ok(());
        }

        let object = match self.object.upgrade() {
            Some(obj) => obj,
            None => return Ok(()),
        };

        let obj_read = match object.read() {
            Ok(guard) => guard,
            Err(_) => return Ok(()),
        };

        // Get body damage state. Matches C++ line 163
        let body_damage_type = obj_read
            .get_body_module()
            .and_then(|body| body.lock().ok().map(|guard| guard.get_damage_state()))
            .unwrap_or(BodyDamageType::Pristine);

        // Get object ID and position for weapon firing
        let obj_id = obj_read.get_id();
        let position = obj_read.get_position().clone();

        // Fire appropriate reaction weapon. Matches C++ lines 165-194
        self.fire_reaction_weapon(body_damage_type, obj_id, &position);

        Ok(())
    }

    fn receive_damage(
        &mut self,
        _object_id: crate::common::ObjectID,
        _damage: &DamageInfo,
    ) -> Real {
        0.0
    }

    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

impl UpdateModuleInterface for FireWeaponWhenDamagedBehavior {
    /// Update for continuous weapons. Matches C++ lines 200-242
    fn update_simple(&mut self) -> UpdateSleepTime {
        if !self.upgrade_mux.is_already_upgraded() {
            return UPDATE_SLEEP_FOREVER; // Matches C++ lines 202-206
        }

        let object = match self.object.upgrade() {
            Some(obj) => obj,
            None => return UPDATE_SLEEP_FOREVER,
        };

        let obj_read = match object.read() {
            Ok(guard) => guard,
            Err(_) => return UPDATE_SLEEP_FOREVER,
        };

        // Get body damage state. Matches C++ line 209
        let body_damage_type = obj_read
            .get_body_module()
            .and_then(|body| body.lock().ok().map(|guard| guard.get_damage_state()))
            .unwrap_or(BodyDamageType::Pristine);

        // Get object ID and position for weapon firing
        let obj_id = obj_read.get_id();
        let position = obj_read.get_position().clone();

        // Fire appropriate continuous weapon. Matches C++ lines 211-239
        self.fire_continuous_weapon(body_damage_type, obj_id, &position);

        UPDATE_SLEEP_NONE // Matches C++ line 241
    }
}

impl UpgradeModuleInterface for FireWeaponWhenDamagedBehavior {
    fn can_upgrade(&self, _upgrade_mask: crate::common::UpgradeMaskType) -> bool {
        let mask = UpgradeMask::from_bits_retain(_upgrade_mask.bits());
        self.upgrade_mux.test_upgrade_conditions(mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: crate::common::UpgradeMaskType) -> bool {
        let Some(obj_arc) = self.object.upgrade() else {
            return false;
        };
        let Ok(mut obj_guard) = obj_arc.write() else {
            return false;
        };
        let mask = UpgradeMask::from_bits_retain(_upgrade_mask.bits());
        let upgraded = self.upgrade_mux.attempt_upgrade(mask, &mut obj_guard);
        if upgraded {
            self.set_wake_frame(UPDATE_SLEEP_NONE);
        }
        upgraded
    }

    fn remove_upgrade(&mut self, _upgrade_mask: crate::common::UpgradeMaskType) {
        let mask = UpgradeMask::from_bits_retain(_upgrade_mask.bits());
        let _ = self.upgrade_mux.reset_upgrade(mask);
    }
}

impl BehaviorModuleInterface for FireWeaponWhenDamagedBehavior {
    fn get_module_name(&self) -> &'static str {
        "FireWeaponWhenDamagedBehavior"
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

impl Snapshotable for FireWeaponWhenDamagedBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.upgrade_mux.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer update module base state: {}", e))?;
        self.upgrade_mux
            .xfer(xfer)
            .map_err(|e| format!("Failed to xfer upgrade mux: {}", e))?;

        let object_id = self
            .object
            .upgrade()
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(crate::common::INVALID_ID);

        Self::xfer_weapon_option(
            xfer,
            &mut self.reaction_weapon_pristine,
            &self.module_data.reaction_weapon_pristine,
            object_id,
        )?;
        Self::xfer_weapon_option(
            xfer,
            &mut self.reaction_weapon_damaged,
            &self.module_data.reaction_weapon_damaged,
            object_id,
        )?;
        Self::xfer_weapon_option(
            xfer,
            &mut self.reaction_weapon_really_damaged,
            &self.module_data.reaction_weapon_really_damaged,
            object_id,
        )?;
        Self::xfer_weapon_option(
            xfer,
            &mut self.reaction_weapon_rubble,
            &self.module_data.reaction_weapon_rubble,
            object_id,
        )?;
        Self::xfer_weapon_option(
            xfer,
            &mut self.continuous_weapon_pristine,
            &self.module_data.continuous_weapon_pristine,
            object_id,
        )?;
        Self::xfer_weapon_option(
            xfer,
            &mut self.continuous_weapon_damaged,
            &self.module_data.continuous_weapon_damaged,
            object_id,
        )?;
        Self::xfer_weapon_option(
            xfer,
            &mut self.continuous_weapon_really_damaged,
            &self.module_data.continuous_weapon_really_damaged,
            object_id,
        )?;
        Self::xfer_weapon_option(
            xfer,
            &mut self.continuous_weapon_rubble,
            &self.module_data.continuous_weapon_rubble,
            object_id,
        )?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.upgrade_mux
            .load_post_process()
            .map_err(|e| format!("Failed to load upgrade mux: {}", e))?;

        if let Some(ref weapon) = self.reaction_weapon_pristine {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }
        if let Some(ref weapon) = self.reaction_weapon_damaged {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }
        if let Some(ref weapon) = self.reaction_weapon_really_damaged {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }
        if let Some(ref weapon) = self.reaction_weapon_rubble {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }
        if let Some(ref weapon) = self.continuous_weapon_pristine {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }
        if let Some(ref weapon) = self.continuous_weapon_damaged {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }
        if let Some(ref weapon) = self.continuous_weapon_really_damaged {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }
        if let Some(ref weapon) = self.continuous_weapon_rubble {
            if let Ok(mut weapon_guard) = weapon.lock() {
                weapon_guard.load_post_process()?;
            }
        }

        Ok(())
    }
}

/// Glue that exposes FireWeaponWhenDamagedBehavior through the common Module trait.
pub struct FireWeaponWhenDamagedBehaviorModule {
    behavior: FireWeaponWhenDamagedBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<FireWeaponWhenDamagedBehaviorModuleData>,
}

impl FireWeaponWhenDamagedBehaviorModule {
    pub fn new(
        behavior: FireWeaponWhenDamagedBehavior,
        module_name: &AsciiString,
        module_data: Arc<FireWeaponWhenDamagedBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut FireWeaponWhenDamagedBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for FireWeaponWhenDamagedBehaviorModule {
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

impl Module for FireWeaponWhenDamagedBehaviorModule {
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

/// Factory for creating FireWeaponWhenDamagedBehavior
pub struct FireWeaponWhenDamagedBehaviorFactory;

impl FireWeaponWhenDamagedBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(FireWeaponWhenDamagedBehavior::new(
            thing,
            module_data,
        )?))
    }
}

// Thread safety
unsafe impl Send for FireWeaponWhenDamagedBehavior {}
unsafe impl Sync for FireWeaponWhenDamagedBehavior {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_defaults() {
        let data = FireWeaponWhenDamagedBehaviorModuleData::default();
        assert_eq!(data.damage_types, DamageTypeFlags::all_flags());
        assert_eq!(data.damage_amount, 0.0);
        assert!(!data.initially_active);
    }
}
