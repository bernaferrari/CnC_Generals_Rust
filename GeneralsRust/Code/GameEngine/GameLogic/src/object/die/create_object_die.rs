//! CreateObjectDie - Spawns objects when the object dies
//!
//! Original C++ location: GameLogic/Module/CreateObjectDie.h/.cpp
//! Original C++ Author: Michael S. Booth, January 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::{Bool, INVALID_ID};
use crate::damage::{DamageInfo, DamageType};
use crate::helpers::TheGameLogic;
use crate::helpers::TheObjectCreationListStore;
use crate::modules::{AIUpdateInterface, BodyModuleInterface, BodyModuleInterfaceExt};
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::system::game_logic::get_game_logic;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for CreateObjectDie
/// (Matches C++ CreateObjectDieModuleData)
#[derive(Debug, Clone)]
pub struct CreateObjectDieModuleData {
    pub base: DieModuleData,
    /// Object creation list - names of objects to spawn
    pub ocl: Vec<String>,
    /// Whether to transfer health from the dying object to the created object
    pub transfer_previous_health: Bool,
}

impl Default for CreateObjectDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            ocl: Vec::new(),
            transfer_previous_health: false,
        }
    }
}

impl Snapshotable for CreateObjectDieModuleData {
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

crate::impl_legacy_module_data_via_base!(CreateObjectDieModuleData, base);

impl CreateObjectDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CREATE_OBJECT_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_creation_list(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.ocl = vec![(*token).to_string()];
    Ok(())
}

fn parse_transfer_previous_health(
    _ini: &mut INI,
    data: &mut CreateObjectDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.transfer_previous_health = INI::parse_bool(token)?;
    Ok(())
}

const CREATE_OBJECT_DIE_FIELDS: &[FieldParse<CreateObjectDieModuleData>] = &[
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
        token: "CreationList",
        parse: parse_creation_list,
    },
    FieldParse {
        token: "TransferPreviousHealth",
        parse: parse_transfer_previous_health,
    },
];

/// CreateObjectDie - Creates new objects when this object dies
///
/// This module spawns new objects at the death location. It can be used for:
/// - Debris and wreckage
/// - Spawning pilots when vehicles are destroyed
/// - Creating resource crates
/// - Spawning replacement units
///
/// Optionally transfers the previous health state to the new object(s).
/// (Matches C++ CreateObjectDie)
#[derive(Debug)]
pub struct CreateObjectDie {
    base: DieModule<CreateObjectDieModuleData>,
}

impl CreateObjectDie {
    /// Create a new CreateObjectDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<CreateObjectDieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "CreateObjectDie"
    }

    /// Create objects from the object creation list
    fn create_objects(
        &self,
        dying_object: &Object,
        damage_dealer: Option<&Object>,
    ) -> Vec<Arc<RwLock<Object>>> {
        let mut created_objects = Vec::new();

        let ocl_name = match self.base.module_data.ocl.first() {
            Some(name) => name,
            None => return created_objects,
        };

        if let Some(ocl_handle) = TheObjectCreationListStore::find_object_creation_list(ocl_name) {
            let ctx = crate::object_creation_list::live_creation_context();
            let created = ocl_handle.create_with_objects(&ctx, dying_object, damage_dealer, 0);
            if let Some(obj) = created {
                created_objects.push(obj);
            }
        }

        created_objects
    }

    /// Transfer health and damage state from old object to new object
    fn transfer_health(&self, old_object: &Object, _new_object: &mut Object) {
        if !self.base.module_data.transfer_previous_health {
            return;
        }

        use crate::damage::DeathType;

        let Some(old_body) = old_object.get_body_module() else {
            return;
        };
        let Some(new_body) = _new_object.get_body_module() else {
            return;
        };

        let Ok(old_body_guard) = old_body.lock() else {
            return;
        };
        let Ok(mut new_body_guard) = new_body.lock() else {
            return;
        };

        let subdual_damage = old_body_guard.get_current_subdual_damage_amount();
        if subdual_damage > 0.0 {
            let mut info = DamageInfo::with_simple(
                subdual_damage,
                INVALID_ID,
                DamageType::SubdualUnresistable,
                DeathType::Normal,
            );
            let _ = new_body_guard.attempt_damage(&mut info);
        }

        let last_damage_source = old_body_guard
            .get_last_damage_info()
            .map(|info| info.input.source_id)
            .unwrap_or(INVALID_ID);

        let damage_amount =
            (old_body_guard.get_max_health() - old_body_guard.get_previous_health()).max(0.0);
        if damage_amount > 0.0 {
            let mut info = DamageInfo::with_simple(
                damage_amount,
                last_damage_source,
                DamageType::Unresistable,
                DeathType::Normal,
            );
            let _ = new_body_guard.attempt_damage(&mut info);
        }
    }

    /// Transfer attackers from old object to new object
    fn transfer_attackers(
        &self,
        old_object_id: crate::common::ObjectID,
        new_object_id: crate::common::ObjectID,
    ) {
        let Ok(game_logic) = get_game_logic().lock() else {
            return;
        };

        for &object_id in game_logic.get_all_object_ids() {
            let _ = OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.transfer_attack(old_object_id, new_object_id);
                    }
                }
            });
        }
    }
}

impl DieModuleInterface for CreateObjectDie {
    /// Called when the object dies - creates new objects
    /// (Matches C++ CreateObjectDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        // Find the damage dealer (C++: TheGameLogic->findObjectByID(damageInfo->sourceID)).
        let damage_dealer_arc = TheGameLogic::find_object_by_id(damage_info.input.source_id);
        let damage_dealer_guard = damage_dealer_arc.as_ref().and_then(|h| h.read().ok());

        // Create the objects
        let created_objects = self.create_objects(object, damage_dealer_guard.as_deref());

        // If we created objects and should transfer health
        if self.base.module_data.transfer_previous_health && !created_objects.is_empty() {
            for created_obj_arc in created_objects.iter() {
                if let Ok(mut created_obj) = created_obj_arc.write() {
                    // Transfer health from dying object to new object
                    self.transfer_health(object, &mut created_obj);

                    // Transfer attackers to the new object
                    self.transfer_attackers(object.get_id(), created_obj.get_id());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_object_die_module_data_default() {
        let data = CreateObjectDieModuleData::default();
        assert_eq!(data.ocl.len(), 0);
        assert_eq!(data.transfer_previous_health, false);
    }

    #[test]
    fn test_create_object_die_module_name() {
        assert_eq!(CreateObjectDie::get_module_name(), "CreateObjectDie");
    }

    #[test]
    fn test_create_object_die_with_objects() {
        let mut data = CreateObjectDieModuleData::default();
        data.ocl.push("DebrisSmall".to_string());
        data.ocl.push("DebrisLarge".to_string());
        assert_eq!(data.ocl.len(), 2);
    }
}
