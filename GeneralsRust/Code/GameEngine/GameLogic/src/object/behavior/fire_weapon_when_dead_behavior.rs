//! FireWeaponWhenDeadBehavior - Rust conversion of C++ FireWeaponWhenDeadBehavior
//!
//! Fires a weapon when the object dies.
//! Original C++: FireWeaponWhenDeadBehavior.cpp
//! Rust conversion: 2025
//!
//! FILE: FireWeaponWhenDeadBehavior.cpp lines 1-145

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, ModuleData, ObjectID, ObjectStatusTypes, UpgradeMaskType, XferVersion,
};
use crate::damage::DamageInfo;
use crate::modules::{BehaviorModuleInterface, DieModuleInterface, UpgradeModuleInterface};
use crate::object::Object as GameObject;
use crate::object::behavior::behavior_module::{
    BehaviorModuleData, xfer_behavior_module_base_versions,
};
use crate::object::die::{
    DieMuxData, parse_death_type_flags_tokens, parse_object_status_mask_tokens,
    parse_veterancy_level_flags_tokens,
};
use crate::upgrade::modules::upgrade_mux::UpgradeMuxData;
use crate::upgrade::{UpgradeMask, UpgradeMux};
use crate::weapon::WeaponTemplate;
use crate::weapon::with_weapon_store;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

/// FireWeaponWhenDeadBehaviorModuleData - Configuration
/// Matches C++ FireWeaponWhenDeadBehavior.h module data structure
#[derive(Clone, Debug)]
pub struct FireWeaponWhenDeadBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// Die conditions. Matches C++ line 69
    pub die_mux_data: DieMuxData,
    /// Upgrade mux data (activation/conflict/removal).
    pub upgrade_mux_data: UpgradeMuxData,
    /// Weapon to fire on death. Matches C++ line 90
    pub death_weapon: Option<Arc<WeaponTemplate>>,
    /// Whether starts active. Matches C++ line 45
    pub initially_active: Bool,
}

impl Default for FireWeaponWhenDeadBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            die_mux_data: DieMuxData::default(),
            upgrade_mux_data: UpgradeMuxData::default(),
            death_weapon: None,
            initially_active: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(FireWeaponWhenDeadBehaviorModuleData, base);

impl FireWeaponWhenDeadBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_WEAPON_WHEN_DEAD_FIELDS)
    }
}

fn parse_starts_active(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.initially_active = INI::parse_bool(token)?;
    Ok(())
}

fn parse_death_weapon(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.death_weapon = with_weapon_store(|store| store.find_weapon_template(token).cloned())
        .ok()
        .flatten();
    Ok(())
}

fn parse_death_types(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.death_types = parse_death_type_flags_tokens(tokens)?;
    Ok(())
}

fn parse_veterancy_levels(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.veterancy_levels = parse_veterancy_level_flags_tokens(tokens)?;
    Ok(())
}

fn parse_exempt_status(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.exempt_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

fn parse_required_status(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.required_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
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
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
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
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
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
    data: &mut FireWeaponWhenDeadBehaviorModuleData,
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

const FIRE_WEAPON_WHEN_DEAD_FIELDS: &[FieldParse<FireWeaponWhenDeadBehaviorModuleData>] = &[
    FieldParse {
        token: "StartsActive",
        parse: parse_starts_active,
    },
    FieldParse {
        token: "DeathWeapon",
        parse: parse_death_weapon,
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
    FieldParse {
        token: "DeathTypes",
        parse: parse_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_required_status,
    },
];

/// FireWeaponWhenDeadBehavior - Fires weapon on death
/// Matches C++ FireWeaponWhenDeadBehavior.cpp lines 42-145
pub struct FireWeaponWhenDeadBehavior {
    object_id: ObjectID,
    module_data: FireWeaponWhenDeadBehaviorModuleData,
    upgrade_mux: UpgradeMux,
}

impl FireWeaponWhenDeadBehavior {
    /// Creates new FireWeaponWhenDeadBehavior. Matches C++ lines 42-49
    pub fn new(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<FireWeaponWhenDeadBehaviorModuleData>()
            .ok_or("Invalid module data for FireWeaponWhenDeadBehavior")?;

        let data = specific_data.clone();
        // Build the mux before `data` moves into the struct.
        let upgrade_mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        // C++ ctor runs giveSelfUpgrade() on `this`; the Rust factories own
        // the object at every construction site (pre-registration Arc or
        // registry checkout), so they apply it on the object they hold.
        // Constructors here stay inert.
        Ok(Self {
            object_id,
            module_data: data,
            upgrade_mux,
        })
    }
}

impl DieModuleInterface for FireWeaponWhenDeadBehavior {
    /// Called when object dies. Matches C++ lines 60-95.
    ///
    /// Prefer [`Self::die_with_object`] from `Object::on_die`. This trait
    /// method has no object borrow, so a same-id registry checkout can miss
    /// while the caller already holds that object.
    fn on_die(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(fired) = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
            self.object_id,
            |object| Self::die_with_object(self, object, damage_info),
        ) else {
            return Ok(());
        };
        fired
    }

    /// Object-aware onDie: `Object::on_die` passes the dying object, the way
    /// C++ `onDie` reads `getObject()` — no same-id registry re-entry.
    fn on_die_with_object(
        &mut self,
        object: &mut crate::object::Object,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::die_with_object(self, object, damage_info)
    }
}

impl FireWeaponWhenDeadBehavior {
    /// C++ `FireWeaponWhenDeadBehavior::onDie` using the owner already in hand.
    pub fn die_with_object(
        &mut self,
        object: &mut crate::object::Object,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.upgrade_mux.is_already_upgraded() {
            return Ok(());
        }

        let (_, conflicting_mask) = self.get_upgrade_activation_masks();
        if !self
            .module_data
            .die_mux_data
            .is_die_applicable(object, damage_info)
        {
            return Ok(());
        }
        if object.test_status(ObjectStatusTypes::UnderConstruction) {
            return Ok(());
        }
        if object.completed_upgrades().intersects(conflicting_mask) {
            return Ok(());
        }
        if let Some(player) = object.get_controlling_player() {
            if let Some(true) = crate::player::with_player(player, |player_guard| {
                player_guard
                    .get_completed_upgrade_mask()
                    .intersects(conflicting_mask)
            }) {
                return Ok(());
            }
        }

        // C++ FireWeaponWhenDeadBehavior.cpp:93 hands `obj` and
        // `obj->getPosition()` straight to WeaponStore. The dying object cannot
        // be re-entered through the registry from here, so the store entry
        // takes the held source object.
        if let Some(death_weapon_tmpl) = &self.module_data.death_weapon {
            crate::weapon::with_weapon_store_mut(|store| {
                store.create_and_fire_temp_weapon_from_object(
                    death_weapon_tmpl,
                    object,
                    object.get_position(),
                )
            })
            .ok();
        }
        Ok(())
    }
}

impl UpgradeModuleInterface for FireWeaponWhenDeadBehavior {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        let mask = UpgradeMask::from_bits_retain(_upgrade_mask.bits());
        self.upgrade_mux.test_upgrade_conditions(mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        let mask = UpgradeMask::from_bits_retain(_upgrade_mask.bits());
        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(self.object_id, |obj_guard| {
                self.upgrade_mux.attempt_upgrade(mask, obj_guard)
            })
            .unwrap_or(false)
    }

    /// Object-aware apply: the caller holds the owning object, so the mux
    /// upgrades it directly (C++ UpgradeMux::attemptUpgrade on getObject()).
    fn apply_upgrade_with_object(
        &mut self,
        object: &mut crate::object::Object,
        _upgrade_mask: UpgradeMaskType,
    ) -> bool {
        let mask = UpgradeMask::from_bits_retain(_upgrade_mask.bits());
        self.upgrade_mux.attempt_upgrade(mask, object)
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        let mask = UpgradeMask::from_bits_retain(_upgrade_mask.bits());
        let _ = self.upgrade_mux.reset_upgrade(mask);
    }
}

impl FireWeaponWhenDeadBehavior {
    fn get_upgrade_activation_masks(&self) -> (UpgradeMaskType, UpgradeMaskType) {
        let mut mux = self.module_data.upgrade_mux_data.clone();
        let (activation, conflicting) = mux.get_upgrade_activation_masks();
        (
            UpgradeMaskType::from_bits_retain(activation.to_bits()),
            UpgradeMaskType::from_bits_retain(conflicting.to_bits()),
        )
    }
}

impl BehaviorModuleInterface for FireWeaponWhenDeadBehavior {
    fn get_module_name(&self) -> &'static str {
        "FireWeaponWhenDeadBehavior"
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }

    fn get_upgrade(&mut self) -> Option<&mut dyn UpgradeModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for FireWeaponWhenDeadBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.upgrade_mux.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_behavior_module_base_versions(xfer)
            .map_err(|e| format!("Failed to xfer behavior base: {}", e))?;
        self.upgrade_mux.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.upgrade_mux.load_post_process()
    }
}

/// Glue that exposes FireWeaponWhenDeadBehavior through the common Module trait.
pub struct FireWeaponWhenDeadBehaviorModule {
    behavior: FireWeaponWhenDeadBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<FireWeaponWhenDeadBehaviorModuleData>,
}

impl FireWeaponWhenDeadBehaviorModule {
    pub fn new(
        behavior: FireWeaponWhenDeadBehavior,
        module_name: &AsciiString,
        module_data: Arc<FireWeaponWhenDeadBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut FireWeaponWhenDeadBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for FireWeaponWhenDeadBehaviorModule {
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

impl Module for FireWeaponWhenDeadBehaviorModule {
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

/// Factory for creating FireWeaponWhenDeadBehavior
pub struct FireWeaponWhenDeadBehaviorFactory;

impl FireWeaponWhenDeadBehaviorFactory {
    pub fn create_behavior(
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(FireWeaponWhenDeadBehavior::new(
            object_id,
            module_data,
        )?))
    }
}

impl FireWeaponWhenDeadBehavior {
    /// C++ `UpgradeMux::giveSelfUpgrade`: FX, removal, empty `upgradeImplementation`, then the executed flag.
    /// Does nothing unless `StartsActive`, and does not mark the upgrade if it already ran.
    pub fn give_self_upgrade(&mut self, object: &mut crate::object::Object) {
        if self.module_data.initially_active && !self.upgrade_mux.is_already_upgraded() {
            self.upgrade_mux.give_self_upgrade(object);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_defaults() {
        let data = FireWeaponWhenDeadBehaviorModuleData::default();
        assert!(!data.initially_active);
        assert!(data.death_weapon.is_none());
    }

    /// C++ FireWeaponWhenDeadBehavior.cpp:45-47 / UpgradeMux TriggeredBy tokens.
    #[test]
    fn parse_triggered_by_and_conflicts_with_tokens() {
        let mut data = FireWeaponWhenDeadBehaviorModuleData::default();
        data.upgrade_mux_data
            .trigger_upgrade_names
            .push(crate::common::AsciiString::from("Upgrade_HE"));
        data.upgrade_mux_data
            .conflicting_upgrade_names
            .push(crate::common::AsciiString::from("Upgrade_Bio"));
        assert!(data.upgrade_mux_data.is_triggered_by("Upgrade_HE"));
        assert!(
            data.upgrade_mux_data
                .conflicting_upgrade_names
                .iter()
                .any(|n| n.as_str() == "Upgrade_Bio")
        );
    }
}
