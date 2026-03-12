//! CreateCrateDie - Spawns supply crates on death
//!
//! Original C++ location: GameLogic/Module/CreateCrateDie.h/.cpp
//! Original C++ Author: Graham Smallwood, February 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::science::SCIENCE_INVALID;
use crate::common::PathfindLayerEnum;
use crate::common::{
    Bool, Coord3D, GameLogicRandomValueReal, ObjectID, Relationship, VeterancyLevel, INVALID_ID,
};
use crate::damage::DamageInfo;
use crate::helpers::TheThingFactory;
use crate::object::crate_system::{get_crate_system, CrateTemplate};
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::draw::TerrainDecalType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::player::PlayerType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

/// Module data for CreateCrateDie
/// (Matches C++ CreateCrateDieModuleData)
#[derive(Debug, Clone)]
pub struct CreateCrateDieModuleData {
    pub base: DieModuleData,
    /// List of crate template names that can be spawned
    pub crate_name_list: Vec<String>,
}

impl Default for CreateCrateDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            crate_name_list: Vec::new(),
        }
    }
}

impl Snapshotable for CreateCrateDieModuleData {
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

crate::impl_legacy_module_data_via_base!(CreateCrateDieModuleData, base);

impl CreateCrateDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CREATE_CRATE_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut CreateCrateDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut CreateCrateDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut CreateCrateDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut CreateCrateDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_crate_data(
    _ini: &mut INI,
    data: &mut CreateCrateDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.crate_name_list.push((*token).to_string());
    Ok(())
}

const CREATE_CRATE_DIE_FIELDS: &[FieldParse<CreateCrateDieModuleData>] = &[
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
        token: "CrateData",
        parse: parse_crate_data,
    },
];

/// CreateCrateDie - Creates supply crates on death
///
/// This module has a chance to spawn supply crates when an object dies.
/// Crates can contain:
/// - Money/resources
/// - Veterancy bonuses
/// - Special powers
/// - Unit spawns
/// - Healing effects
///
/// The crate spawned is determined by:
/// - Creation chance (from crate template)
/// - Veterancy level of dying unit
/// - Type of killer (infantry, vehicle, etc.)
/// - Killer's science/upgrades
///
/// This encourages aggressive play by rewarding kills with resources.
/// (Matches C++ CreateCrateDie)
#[derive(Debug)]
pub struct CreateCrateDie {
    base: DieModule<CreateCrateDieModuleData>,
}

use crate::helpers::{FindPositionOptions, FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS, FPF_NONE};

impl CreateCrateDie {
    /// Create a new CreateCrateDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<CreateCrateDieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "CreateCrateDie"
    }

    /// Test if crate should be created based on creation chance
    /// (Matches C++ CreateCrateDie.cpp lines 105-111)
    fn test_creation_chance(&self, template: &CrateTemplate) -> Bool {
        let test_with = GameLogicRandomValueReal(0.0, 1.0);
        test_with < template.creation_chance
    }

    /// Test if veterancy level matches crate requirements
    /// (Matches C++ CreateCrateDie::testVeterancyLevel lines 113-119)
    fn test_veterancy_level(&self, template: &CrateTemplate, object: &Object) -> Bool {
        match template.veterancy_level {
            Some(level) => level == object.get_veterancy_level(),
            None => true,
        }
    }

    /// Test if killer type matches crate requirements
    fn test_killer_type(&self, template: &CrateTemplate, killer: Option<&Object>) -> Bool {
        if template.killed_by_type_kindof == 0 {
            return true;
        }
        let Some(killer) = killer else {
            return false;
        };
        killer.is_kind_of_multi(template.killed_by_type_kindof, 0)
    }

    /// Test if killer has required science/upgrades
    fn test_killer_science(&self, template: &CrateTemplate, killer: Option<&Object>) -> Bool {
        if template.killer_science == SCIENCE_INVALID {
            return true;
        }
        let Some(killer) = killer else {
            return false;
        };
        let Some(player_arc) = killer.get_controlling_player() else {
            return false;
        };
        let Ok(player_guard) = player_arc.read() else {
            return false;
        };
        player_guard.has_science(template.killer_science)
    }

    /// Select which specific crate to create from a list using weighted distribution
    /// (Matches C++ CreateCrateDie.cpp lines 155-174)
    fn select_crate_from_weighted_list(&self, template: &CrateTemplate) -> Option<String> {
        let pick = GameLogicRandomValueReal(0.0, 1.0);
        template.select_crate(pick)
    }

    /// Create a crate object
    /// (Matches C++ CreateCrateDie.cpp lines 150-229)
    fn create_crate(&self, template: &CrateTemplate, dying_object: &Object) -> Option<ObjectID> {
        let center_point = *dying_object.get_position();
        let layer = dying_object.get_layer();

        let crate_name = self.select_crate_from_weighted_list(template)?;
        let crate_type = TheThingFactory::find_template(&crate_name)?;

        let creation_point = if layer != PathfindLayerEnum::Ground {
            center_point
        } else {
            let mut fp_options = FindPositionOptions {
                min_radius: 0.0,
                max_radius: 5.0,
                relationship_object_id: Some(dying_object.get_id()),
                flags: FPF_IGNORE_ALLY_OR_NEUTRAL_UNITS,
                ..Default::default()
            };
            let mut creation_point = center_point;
            if !self.find_position_around(&center_point, &fp_options, &mut creation_point) {
                fp_options.min_radius = 0.0;
                fp_options.max_radius = 125.0;
                fp_options.relationship_object_id = None;
                fp_options.flags = FPF_NONE;
                if !self.find_position_around(&center_point, &fp_options, &mut creation_point) {
                    return None;
                }
            }
            creation_point
        };

        let factory = TheThingFactory::get().ok()?;
        let crate_arc = factory.new_object_optional_team(crate_type, None).ok()?;

        let mut crate_id = INVALID_ID;
        if let Ok(mut crate_obj) = crate_arc.write() {
            crate_id = crate_obj.get_id();
            let _ = crate_obj.set_position(&creation_point);
            let _ = crate_obj.set_orientation(GameLogicRandomValueReal(0.0, 2.0 * PI));
            crate_obj.set_layer(layer);

            if let Some(drawable) = crate_obj.get_drawable() {
                if let Ok(mut drawable_guard) = drawable.write() {
                    drawable_guard.set_terrain_decal(TerrainDecalType::Crate);
                    let size = 2.5 * crate_obj.get_geometry_info().get_major_radius();
                    drawable_guard.set_terrain_decal_size(size, size);
                    drawable_guard.set_terrain_decal_fade_target(1.0, 0.03);
                }
            }
        }

        if crate_id == INVALID_ID {
            return None;
        }

        Some(crate_id)
    }

    fn find_position_around(
        &self,
        center: &Coord3D,
        options: &FindPositionOptions,
        result: &mut Coord3D,
    ) -> Bool {
        if let Some(partition) = crate::helpers::ThePartitionManager::get() {
            return partition.find_position_around_with_options(center, options, result);
        }
        false
    }

    fn set_crate_team(&self, crate_id: ObjectID, owner: &Object) {
        let Some(player_arc) = owner.get_controlling_player() else {
            return;
        };
        let Ok(player_guard) = player_arc.read() else {
            return;
        };
        let Some(team_arc) = player_guard.get_default_team() else {
            return;
        };
        let Some(crate_arc) = OBJECT_REGISTRY.get_object(crate_id) else {
            return;
        };
        let Ok(mut crate_obj) = crate_arc.write() else {
            return;
        };
        let _ = crate_obj.set_team(Some(team_arc));
    }
}

impl DieModuleInterface for CreateCrateDie {
    /// Called when the object dies - may create a crate
    /// (Matches C++ CreateCrateDie::onDie in CreateCrateDie.cpp lines 46-103)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        let killer = crate::helpers::TheGameLogic::find_object_by_id(damage_info.input.source_id);
        if let Some(killer_arc) = killer.as_ref() {
            if let Ok(killer_guard) = killer_arc.read() {
                if matches!(killer_guard.relationship_to(object), Relationship::Allies) {
                    return;
                }
            }
        }

        let crate_system = get_crate_system();
        let Ok(crate_system_guard) = crate_system.read() else {
            return;
        };

        for crate_name in self.base.module_data.crate_name_list.clone() {
            let Some(template_arc) = crate_system_guard.find_crate_template(&crate_name) else {
                continue;
            };
            let Ok(template_guard) = template_arc.read() else {
                continue;
            };

            if !self.test_creation_chance(&template_guard) {
                continue;
            }

            if template_guard.veterancy_level.is_some()
                && !self.test_veterancy_level(&template_guard, object)
            {
                continue;
            }

            let killer_ref = killer.as_ref().and_then(|k| k.read().ok());
            if !self.test_killer_type(&template_guard, killer_ref.as_deref()) {
                continue;
            }
            drop(killer_ref);

            let killer_ref = killer.as_ref().and_then(|k| k.read().ok());
            if !self.test_killer_science(&template_guard, killer_ref.as_deref()) {
                continue;
            }
            drop(killer_ref);

            if let Some(crate_id) = self.create_crate(&template_guard, object) {
                if template_guard.is_owned_by_maker {
                    self.set_crate_team(crate_id, object);
                }

                if let Some(killer_arc) = killer.as_ref() {
                    if let Ok(killer_guard) = killer_arc.read() {
                        if let Some(player_arc) = killer_guard.get_controlling_player() {
                            if let Ok(player_guard) = player_arc.read() {
                                if player_guard.get_player_type() == PlayerType::Computer {
                                    if let Some(ai) = killer_guard.get_ai_update_interface() {
                                        if let Ok(mut ai_guard) = ai.lock() {
                                            ai_guard.notify_crate(crate_id);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_crate_die_module_data_default() {
        let data = CreateCrateDieModuleData::default();
        assert_eq!(data.crate_name_list.len(), 0);
    }

    #[test]
    fn test_create_crate_die_module_name() {
        assert_eq!(CreateCrateDie::get_module_name(), "CreateCrateDie");
    }

    #[test]
    fn test_create_crate_die_with_crates() {
        let mut data = CreateCrateDieModuleData::default();
        data.crate_name_list.push("Crate_Money".to_string());
        data.crate_name_list.push("Crate_Veterancy".to_string());
        data.crate_name_list.push("Crate_Heal".to_string());

        assert_eq!(data.crate_name_list.len(), 3);
        assert_eq!(data.crate_name_list[0], "Crate_Money");
    }
}
