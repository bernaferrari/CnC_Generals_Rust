//! DamDie - Special death behavior for dam structures
//!
//! Original C++ location: GameLogic/Module/DamDie.h/.cpp
//! Original C++ Author: Colin Day, April 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::{DisabledType, KindOf};
use crate::damage::DamageInfo;
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::Object;
use crate::system::game_logic::get_game_logic;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for DamDie
/// (Matches C++ DamDieModuleData)
#[derive(Debug, Clone)]
pub struct DamDieModuleData {
    pub base: DieModuleData,
    // The C++ version has no additional fields - dam-specific logic
    // is handled in the implementation
}

impl Default for DamDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
        }
    }
}

impl Snapshotable for DamDieModuleData {
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

crate::impl_legacy_module_data_via_base!(DamDieModuleData, base);

impl DamDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DAM_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut DamDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut DamDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut DamDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut DamDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

const DAM_DIE_FIELDS: &[FieldParse<DamDieModuleData>] = &[
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

/// DamDie - Handles destruction of dam structures
///
/// This specialized die module handles the destruction of dam structures.
/// When a dam is destroyed, it:
/// - Releases water (flooding effects)
/// - Triggers water flow physics
/// - Updates water level in affected areas
/// - May spawn water debris/effects
///
/// This is used specifically for the GLA Three Gorges Dam and similar
/// water-controlling structures in the game.
/// (Matches C++ DamDie)
#[derive(Debug)]
pub struct DamDie {
    base: DieModule<DamDieModuleData>,
}

impl DamDie {
    /// Create a new DamDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<DamDieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "DamDie"
    }

    /// Enable wave guide objects once the dam is destroyed.
    fn enable_waveguides(&self) {
        let Ok(game_logic) = get_game_logic().lock() else {
            return;
        };

        let mut current = game_logic.get_first_object();
        while let Some(obj) = current {
            let next = if let Ok(mut obj_guard) = obj.write() {
                if obj_guard.is_kind_of(KindOf::WaveGuide) {
                    obj_guard.clear_disabled(DisabledType::DisabledDefault);
                }
                obj_guard.get_next_object()
            } else {
                None
            };

            current = next;
        }
    }
}

impl DieModuleInterface for DamDie {
    /// Called when the dam dies - releases water and triggers flooding
    /// (Matches C++ DamDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        self.enable_waveguides();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::types::ModuleData;

    #[test]
    fn test_dam_die_module_data_default() {
        let data = DamDieModuleData::default();
        assert_eq!(data.get_module_type(), "DamDieModuleData");
    }

    #[test]
    fn test_dam_die_module_name() {
        assert_eq!(DamDie::get_module_name(), "DamDie");
    }
}
