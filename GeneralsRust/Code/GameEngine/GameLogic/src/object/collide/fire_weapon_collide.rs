//! Fire Weapon Collision Module
//!
//! FILE: fire_weapon_collide.rs
//! Author: Converted from Graham Smallwood's C++ implementation, April 2002
//! Desc: Shoot something that collides with me every frame with my weapon

use super::*;
use crate::common::types::WeaponBonusConditionFlags;
use crate::helpers::TheGameLogic;
use crate::weapon::{with_weapon_store, Weapon, WeaponSlotType, WeaponTemplate};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{ModuleData, NameKeyType};
use std::sync::Arc;
use std::sync::RwLock;

/// Module data for fire weapon collision
#[derive(Debug, Clone)]
pub struct FireWeaponCollideModuleData {
    module_tag_name_key: NameKeyType,
    collide_weapon_template_name: Option<String>,
    /// Resolved weapon template reference (populated via INI or explicit setup)
    pub collide_weapon_template: Arc<RwLock<Option<Arc<WeaponTemplate>>>>,
    /// Whether the weapon can only fire once ever
    pub fire_once: bool,
    /// Required status bits for the weapon to fire
    pub required_status: ObjectStatusMask,
    /// Forbidden status bits that prevent firing
    pub forbidden_status: ObjectStatusMask,
}

impl Default for FireWeaponCollideModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            collide_weapon_template_name: None,
            collide_weapon_template: Arc::new(RwLock::new(None)),
            fire_once: false,
            required_status: ObjectStatusMask::empty(),
            forbidden_status: ObjectStatusMask::empty(),
        }
    }
}

impl crate::common::LegacyModuleData for FireWeaponCollideModuleData {
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

impl FireWeaponCollideModuleData {
    /// Associate a resolved weapon template with this module data.
    pub fn set_weapon_template(&mut self, template: Arc<WeaponTemplate>) {
        self.collide_weapon_template_name = Some(template.name.clone());
        if let Ok(mut guard) = self.collide_weapon_template.write() {
            *guard = Some(template);
        }
    }

    /// Record the weapon template name to resolve later via the global weapon store.
    pub fn set_weapon_template_name<S: Into<String>>(&mut self, name: S) {
        self.collide_weapon_template_name = Some(name.into());
        if let Ok(mut guard) = self.collide_weapon_template.write() {
            *guard = None;
        }
    }

    /// Apply the required-status mask from a list of status names.
    pub fn set_required_status_from_names(&mut self, names: &[&str]) -> Result<(), String> {
        let mask = ObjectStatusMaskType::parse_tokens(names.iter().copied())?;
        self.required_status = ObjectStatusMask::from_mask(mask);
        Ok(())
    }

    /// Apply the forbidden-status mask from a list of status names.
    pub fn set_forbidden_status_from_names(&mut self, names: &[&str]) -> Result<(), String> {
        let mask = ObjectStatusMaskType::parse_tokens(names.iter().copied())?;
        self.forbidden_status = ObjectStatusMask::from_mask(mask);
        Ok(())
    }

    /// Populate this module data from an INI definition.
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_WEAPON_COLLIDE_FIELDS)
    }
}

impl Snapshotable for FireWeaponCollideModuleData {
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

/// Fire Weapon Collide Module
///
/// This module fires a weapon at anything that collides with the owning object.
/// It can be configured to fire once or repeatedly, and can have status requirements.
pub struct FireWeaponCollide {
    object_id: ObjectId,
    module_data: Arc<FireWeaponCollideModuleData>,
    #[allow(dead_code)]
    weapon_template: Arc<WeaponTemplate>,
    collide_weapon: Weapon,
    ever_fired: bool,
    version: u32,
}

impl FireWeaponCollide {
    /// Create a new FireWeaponCollide instance
    ///
    /// # Arguments
    /// * `object_id` - The ID of the object this module belongs to
    /// * `module_data` - Configuration data for the fire weapon collision behavior
    pub fn new(
        object_id: ObjectId,
        module_data: Arc<FireWeaponCollideModuleData>,
    ) -> Result<Self, CollisionError> {
        let cached_template = module_data
            .collide_weapon_template
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(Arc::clone));

        let weapon_template = if let Some(template) = cached_template {
            template
        } else if let Some(name) = module_data.collide_weapon_template_name.as_ref() {
            with_weapon_store(|store| {
                store
                    .find_weapon_template(name)
                    .map(|template| Arc::clone(template))
            })
            .map_err(|err| {
                CollisionError::InvalidObject(format!(
                    "Failed to access weapon store for '{}': {err}",
                    name
                ))
            })?
            .ok_or_else(|| {
                CollisionError::InvalidObject(format!(
                    "Collide weapon template '{}' not found",
                    name
                ))
            })?
        } else {
            return Err(CollisionError::InvalidObject(
                "FireWeaponCollide requires a collide weapon template".to_string(),
            ));
        };

        let collide_weapon = with_weapon_store(|store| {
            store.allocate_new_weapon(&weapon_template, WeaponSlotType::Primary)
        })
        .map_err(|err| {
            CollisionError::InvalidObject(format!(
                "Failed to allocate collide weapon instance: {err}"
            ))
        })?;

        if let Ok(mut guard) = module_data.collide_weapon_template.write() {
            if guard.is_none() {
                *guard = Some(Arc::clone(&weapon_template));
            }
        }

        Ok(Self {
            object_id,
            module_data,
            weapon_template,
            collide_weapon,
            ever_fired: false,
            version: 1,
        })
    }

    /// Get the fire weapon collide module data
    pub fn get_fire_weapon_collide_module_data(&self) -> &FireWeaponCollideModuleData {
        self.module_data.as_ref()
    }

    /// Check if the weapon should fire based on status and configuration
    ///
    /// # Arguments
    /// * `owner` - The object that owns this collision module
    pub fn should_fire_weapon(&self, owner: &dyn GameObject) -> bool {
        let status = owner.get_status_bits();

        // We need all required status bits or else we fail
        if !status.test_for_all(self.module_data.required_status) {
            return false;
        }

        // If we have any forbidden status bits, then fail
        if status.test_for_any(self.module_data.forbidden_status) {
            return false;
        }

        // If we can only fire once and have already fired, fail
        if self.ever_fired && self.module_data.fire_once {
            return false;
        }

        true
    }

    /// Get the current version of this module for serialization
    pub fn get_version(&self) -> u32 {
        self.version
    }
}

impl Snapshotable for FireWeaponCollide {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let mut collide_weapon_present = true;
        xfer.xfer_bool(&mut collide_weapon_present)
            .map_err(|e| e.to_string())?;
        self.collide_weapon.crc(xfer)?;

        let mut ever_fired = self.ever_fired;
        xfer.xfer_bool(&mut ever_fired).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.version = version as u32;

        let mut collide_weapon_present = true;
        xfer.xfer_bool(&mut collide_weapon_present)
            .map_err(|e| e.to_string())?;
        if !collide_weapon_present {
            return Err("FireWeaponCollide::xfer missing collide weapon".to_string());
        }
        self.collide_weapon.xfer(xfer)?;

        xfer.xfer_bool(&mut self.ever_fired)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.collide_weapon.load_post_process()
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn normalized_tokens<'a>(tokens: &'a [&'a str]) -> impl Iterator<Item = &'a str> {
    tokens.iter().copied().filter(|token| *token != "=")
}

fn parse_collide_weapon_field(
    _ini: &mut INI,
    data: &mut FireWeaponCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.set_weapon_template_name(name);
    Ok(())
}

fn parse_fire_once_field(
    _ini: &mut INI,
    data: &mut FireWeaponCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.fire_once = parse_bool_flag(value)?;
    Ok(())
}

fn parse_required_status_field(
    _ini: &mut INI,
    data: &mut FireWeaponCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let status_tokens: Vec<&str> = normalized_tokens(tokens).collect();
    data.set_required_status_from_names(&status_tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_forbidden_status_field(
    _ini: &mut INI,
    data: &mut FireWeaponCollideModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let status_tokens: Vec<&str> = normalized_tokens(tokens).collect();
    data.set_forbidden_status_from_names(&status_tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_bool_flag(value: &str) -> Result<bool, INIError> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(INIError::InvalidData),
    }
}

const FIRE_WEAPON_COLLIDE_FIELDS: &[FieldParse<FireWeaponCollideModuleData>] = &[
    FieldParse {
        token: "CollideWeapon",
        parse: parse_collide_weapon_field,
    },
    FieldParse {
        token: "FireOnce",
        parse: parse_fire_once_field,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_required_status_field,
    },
    FieldParse {
        token: "ForbiddenStatus",
        parse: parse_forbidden_status_field,
    },
];

impl CollideModule for FireWeaponCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        // Don't shoot the ground
        let other_obj = match other {
            Some(obj) => obj,
            None => return Ok(()), // Collision with ground, do nothing
        };

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return Ok(());
        };

        // This will fire at the target every frame, because multiple objects could be
        // colliding and we want to hurt them all. Another solution would be to keep
        // a map of object IDs and delays for each individually.
        if self.should_fire_weapon(&owner) {
            let (source_id, source_bonus_flags, container_bonus_flags) =
                if let Ok(owner_guard) = owner.read() {
                    let source_id = owner_guard.get_id();
                    let source_bonus_flags = owner_guard.get_weapon_bonus_condition();
                    let container_bonus_flags = owner_guard
                        .get_contained_by()
                        .and_then(TheGameLogic::find_object_by_id)
                        .and_then(|container| {
                            container
                                .read()
                                .ok()
                                .map(|g| g.get_weapon_bonus_condition())
                        });
                    (source_id, source_bonus_flags, container_bonus_flags)
                } else {
                    (owner.get_id(), WeaponBonusConditionFlags::empty(), None)
                };
            self.collide_weapon.load_ammo_now(source_id).map_err(|e| {
                CollisionError::InvalidObject(format!("Failed to load ammo: {}", e))
            })?;

            self.collide_weapon
                .fire_weapon(
                    source_id,
                    other_obj.get_id(),
                    TheGameLogic::get_frame(),
                    source_bonus_flags,
                    container_bonus_flags,
                )
                .map_err(|e| {
                    CollisionError::InvalidObject(format!("Failed to fire weapon: {}", e))
                })?;
        }

        Ok(())
    }

    fn would_like_to_collide_with(&self, _other: &dyn GameObject) -> bool {
        // C++ FireWeaponCollide does not override base CollideModule behavior.
        // Keep default parity: this predicate remains false.
        false
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.
