//! FXListDie - Plays visual effects when the object dies
//!
//! Original C++ location: GameLogic/Module/FXListDie.h/.cpp
//! Original C++ Author: Steven Johnson, January 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::{AsciiString, Bool, ModuleData, UpgradeMaskType};
use crate::damage::DamageInfo;
use crate::helpers::{TheFXListStore, TheGameLogic};
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::Object;
use crate::upgrade::{UpgradeMask, UpgradeMux};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for FXListDie
/// (Matches C++ FXListDieModuleData)
#[derive(Debug, Clone)]
pub struct FXListDieModuleData {
    pub base: DieModuleData,
    /// Default death FX to play
    pub default_death_fx: Option<String>,
    /// Whether to orient the FX to the object that caused the death
    pub orient_to_object: Bool,
    /// Whether the module is initially active (for upgrade system)
    pub initially_active: Bool,
    /// Upgrade mux data (trigger/conflict/removal/FX)
    pub upgrade_mux_data: crate::upgrade::UpgradeMuxData,
}

impl Default for FXListDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            default_death_fx: None,
            orient_to_object: true,
            initially_active: true, // Matches C++ default (Patch 1.02 hack)
            upgrade_mux_data: crate::upgrade::UpgradeMuxData::default(),
        }
    }
}

impl Snapshotable for FXListDieModuleData {
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

crate::impl_legacy_module_data_via_base!(FXListDieModuleData, base);

impl FXListDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FX_LIST_DIE_FIELDS)
    }
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&str]) -> impl Iterator<Item = &'a str> + 'a {
    tokens.iter().copied().filter(|token| *token != "=")
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_starts_active(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.initially_active = INI::parse_bool(token)?;
    Ok(())
}

fn parse_death_fx(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    if token.eq_ignore_ascii_case("None") {
        data.default_death_fx = None;
    } else {
        data.default_death_fx = Some((*token).to_string());
    }
    Ok(())
}

fn parse_orient_to_object(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.orient_to_object = INI::parse_bool(token)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let mut parsed_any = false;
    for token in value_tokens(tokens) {
        data.upgrade_mux_data
            .trigger_upgrade_names
            .push(AsciiString::from(token));
        parsed_any = true;
    }
    if !parsed_any {
        return Err(INIError::InvalidData);
    }
    Ok(())
}

fn parse_conflicts_with(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let mut parsed_any = false;
    for token in value_tokens(tokens) {
        data.upgrade_mux_data
            .conflicting_upgrade_names
            .push(AsciiString::from(token));
        parsed_any = true;
    }
    if !parsed_any {
        return Err(INIError::InvalidData);
    }
    Ok(())
}

fn parse_removes_upgrades(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let mut parsed_any = false;
    for token in value_tokens(tokens) {
        data.upgrade_mux_data
            .removal_upgrade_names
            .push(AsciiString::from(token));
        parsed_any = true;
    }
    if !parsed_any {
        return Err(INIError::InvalidData);
    }
    Ok(())
}

fn parse_requires_all_triggers(
    _ini: &mut INI,
    data: &mut FXListDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.upgrade_mux_data.requires_all_triggers = INI::parse_bool(token)?;
    Ok(())
}

const FX_LIST_DIE_FIELDS: &[FieldParse<FXListDieModuleData>] = &[
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
    FieldParse {
        token: "StartsActive",
        parse: parse_starts_active,
    },
    FieldParse {
        token: "DeathFX",
        parse: parse_death_fx,
    },
    FieldParse {
        token: "OrientToObject",
        parse: parse_orient_to_object,
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

/// FXListDie - Plays visual and audio effects on death
///
/// This module is responsible for playing particle effects, sounds, and
/// other visual feedback when an object dies. It can be tied to the
/// upgrade system to only play effects when certain upgrades are present.
///
/// Effects can be oriented toward the damage dealer (e.g., explosion
/// points toward the attacker) or positioned at the object location.
/// (Matches C++ FXListDie)
#[derive(Debug)]
pub struct FXListDie {
    base: DieModule<FXListDieModuleData>,
    /// Upgrade mux state
    upgrade_mux: UpgradeMux,
}

impl FXListDie {
    /// Create a new FXListDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<FXListDieModuleData>) -> Self {
        let initially_active = module_data.initially_active;
        let mut upgrade_mux = UpgradeMux::new(module_data.upgrade_mux_data.clone());
        if initially_active {
            if let Ok(mut obj_guard) = object.write() {
                upgrade_mux.data.perform_upgrade_fx(&mut obj_guard);
                upgrade_mux.data.process_upgrade_removal(&mut obj_guard);
            }
            upgrade_mux.set_upgrade_executed(true);
        }
        Self {
            base: DieModule::new(object, module_data),
            upgrade_mux,
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "FXListDie"
    }

    /// Check if the module is currently active (upgraded)
    fn is_upgrade_active(&self) -> bool {
        self.upgrade_mux.is_already_upgraded()
    }

    /// Check if conflicting upgrades are present
    fn check_conflicting_upgrades(&self, object: &Object) -> bool {
        let mut mux_data = self.base.module_data.upgrade_mux_data.clone();
        let (_activation, conflicting) = mux_data.get_upgrade_activation_masks();
        if !conflicting.any() {
            return false;
        }
        let conflicting_bits = UpgradeMaskType::from_bits_retain(conflicting.to_bits());
        if object.completed_upgrades().intersects(conflicting_bits) {
            return true;
        }
        if let Some(player_arc) = object.get_controlling_player() {
            if let Ok(player_guard) = player_arc.read() {
                if player_guard
                    .get_completed_upgrade_mask()
                    .intersects(conflicting_bits)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Play the death FX
    fn play_death_fx(&self, object: &Object, damage_info: &DamageInfo) {
        let fx_name = match &self.base.module_data.default_death_fx {
            Some(fx) => fx,
            None => return, // No FX to play
        };

        let Some(fx_list) = TheFXListStore::find_fx_list(fx_name.as_str()) else {
            return;
        };

        if self.base.module_data.orient_to_object {
            let source_id = TheGameLogic::find_object_by_id(damage_info.input.source_id)
                .map(|_| damage_info.input.source_id);
            let _ = fx_list.do_fx_obj_ids(object.get_id(), source_id, None);
        } else {
            let _ = fx_list.do_fx_at_position(object.get_position());
        }
    }
}

impl DieModuleInterface for FXListDie {
    /// Called when the object dies - plays death effects
    /// (Matches C++ FXListDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if upgrade is active
        if !self.is_upgrade_active() {
            return;
        }

        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        // Check for conflicting upgrades
        if self.check_conflicting_upgrades(object) {
            return;
        }

        log::debug!(
            "FXListDie: Object {} died, playing death FX",
            object.get_id()
        );

        // Play the death effects
        self.play_death_fx(object, damage_info);
    }
}

impl crate::modules::UpgradeModuleInterface for FXListDie {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.upgrade_mux.is_already_upgraded()
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        if self.upgrade_mux.is_already_upgraded() {
            return false;
        }

        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        if !self.upgrade_mux.would_upgrade(key_mask) {
            return false;
        }

        let Some(obj_arc) = self.base.get_object() else {
            return false;
        };
        let Ok(mut obj_guard) = obj_arc.write() else {
            return false;
        };

        self.upgrade_mux.data.perform_upgrade_fx(&mut obj_guard);
        self.upgrade_mux
            .data
            .process_upgrade_removal(&mut obj_guard);
        self.upgrade_mux.set_upgrade_executed(true);
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        let _ = self.upgrade_mux.reset_upgrade(key_mask);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_field(data: &mut FXListDieModuleData, token: &str, values: &[&str]) {
        let field = FX_LIST_DIE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn test_fx_list_die_module_data_default() {
        let data = FXListDieModuleData::default();
        assert_eq!(data.default_death_fx, None);
        assert_eq!(data.orient_to_object, true);
        assert_eq!(data.initially_active, true);
        assert_eq!(data.upgrade_mux_data.trigger_upgrade_names.len(), 0);
        assert_eq!(data.upgrade_mux_data.conflicting_upgrade_names.len(), 0);
        assert_eq!(data.upgrade_mux_data.requires_all_triggers, false);
    }

    #[test]
    fn test_fx_list_die_module_name() {
        assert_eq!(FXListDie::get_module_name(), "FXListDie");
    }

    #[test]
    fn test_fx_list_die_with_fx() {
        let mut data = FXListDieModuleData::default();
        data.default_death_fx = Some("FX_TankExplosion".to_string());
        assert!(data.default_death_fx.is_some());
        assert_eq!(data.default_death_fx.unwrap(), "FX_TankExplosion");
    }

    #[test]
    fn test_fx_list_die_orientation_options() {
        let mut data = FXListDieModuleData::default();
        data.orient_to_object = false;
        assert_eq!(data.orient_to_object, false);
    }

    #[test]
    fn fx_list_die_fields_accept_ini_equals_token() {
        let mut data = FXListDieModuleData::default();

        parse_field(&mut data, "StartsActive", &["=", "No"]);
        parse_field(&mut data, "DeathFX", &["=", "FX_TankExplosion"]);
        parse_field(&mut data, "OrientToObject", &["=", "No"]);
        parse_field(&mut data, "TriggeredBy", &["=", "Upgrade_A", "Upgrade_B"]);
        parse_field(&mut data, "ConflictsWith", &["=", "Upgrade_C"]);
        parse_field(&mut data, "RemovesUpgrades", &["=", "Upgrade_D"]);
        parse_field(&mut data, "RequiresAllTriggers", &["=", "Yes"]);

        assert!(!data.initially_active);
        assert_eq!(data.default_death_fx.as_deref(), Some("FX_TankExplosion"));
        assert!(!data.orient_to_object);
        assert_eq!(
            data.upgrade_mux_data
                .trigger_upgrade_names
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            vec!["Upgrade_A", "Upgrade_B"]
        );
        assert_eq!(
            data.upgrade_mux_data.conflicting_upgrade_names[0].as_str(),
            "Upgrade_C"
        );
        assert_eq!(
            data.upgrade_mux_data.removal_upgrade_names[0].as_str(),
            "Upgrade_D"
        );
        assert!(data.upgrade_mux_data.requires_all_triggers);
    }

    #[test]
    fn fx_list_die_death_fx_none_skips_equals_separator() {
        let mut data = FXListDieModuleData::default();

        parse_field(&mut data, "DeathFX", &["=", "None"]);

        assert!(data.default_death_fx.is_none());
    }
}
