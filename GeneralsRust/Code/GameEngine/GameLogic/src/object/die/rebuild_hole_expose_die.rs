//! RebuildHoleExposeDie - Creates rebuild hole on structure death
//!
//! Original C++ location: GameLogic/Module/RebuildHoleExposeDie.h/.cpp
//! Original C++ Author: Colin Day, June 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::ai::THE_AI;
use crate::common::{AsciiString, Bool, ObjectStatusTypes, Real};
use crate::damage::DamageInfo;
use crate::helpers::TheThingFactory;
use crate::object::behavior::behavior_module::RebuildHoleBehaviorInterface;
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::Object;
use crate::scripting::engine::transfer_object_name;
use crate::system::game_logic::get_game_logic;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for RebuildHoleExposeDie
/// (Matches C++ RebuildHoleExposeDieModuleData)
#[derive(Debug, Clone)]
pub struct RebuildHoleExposeDieModuleData {
    pub base: DieModuleData,
    /// Name of the hole object template to create
    pub hole_name: AsciiString,
    /// Maximum health of the rebuild hole
    pub hole_max_health: Real,
    /// Whether to transfer attackers to the hole
    pub transfer_attackers: Bool,
}

impl Default for RebuildHoleExposeDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            hole_name: AsciiString::default(),
            hole_max_health: 0.0,
            transfer_attackers: true,
        }
    }
}

impl Snapshotable for RebuildHoleExposeDieModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData xfer version: {e:?}"))?;

        self.base.xfer(xfer)?;

        let mut name = self.hole_name.as_str().to_string();
        xfer.xfer_ascii_string(&mut name)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData hole_name: {e:?}"))?;
        self.hole_name = AsciiString::from(name.as_str());

        let mut health = self.hole_max_health;
        xfer.xfer_real(&mut health)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData hole_max_health: {e:?}"))?;
        self.hole_max_health = health;

        let mut transfer = self.transfer_attackers;
        xfer.xfer_bool(&mut transfer)
            .map_err(|e| format!("RebuildHoleExposeDieModuleData transfer_attackers: {e:?}"))?;
        self.transfer_attackers = transfer;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_via_base!(RebuildHoleExposeDieModuleData, base);

impl RebuildHoleExposeDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, REBUILD_HOLE_EXPOSE_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_hole_name(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.hole_name = AsciiString::from(*token);
    Ok(())
}

fn parse_hole_max_health(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.hole_max_health = INI::parse_real(token)?;
    Ok(())
}

fn parse_transfer_attackers(
    _ini: &mut INI,
    data: &mut RebuildHoleExposeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.transfer_attackers = INI::parse_bool(token)?;
    Ok(())
}

const REBUILD_HOLE_EXPOSE_DIE_FIELDS: &[FieldParse<RebuildHoleExposeDieModuleData>] = &[
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
        token: "HoleName",
        parse: parse_hole_name,
    },
    FieldParse {
        token: "HoleMaxHealth",
        parse: parse_hole_max_health,
    },
    FieldParse {
        token: "TransferAttackers",
        parse: parse_transfer_attackers,
    },
];

/// RebuildHoleExposeDie - Creates a rebuild hole when structure dies
///
/// When a structure with this module is destroyed, it creates a "rebuild hole"
/// in its place. The rebuild hole is a special object that:
/// - Appears as rubble/destroyed foundation
/// - Can automatically rebuild the structure over time
/// - Can be captured by other players
/// - Can be repaired to speed up rebuilding
///
/// This is used for tech buildings and other important structures that
/// should be capturable rather than permanently destroyed.
///
/// Optionally transfers attackers from the dying structure to the hole,
/// so units will continue attacking the hole instead of wandering off.
/// (Matches C++ RebuildHoleExposeDie)
#[derive(Debug)]
pub struct RebuildHoleExposeDie {
    base: DieModule<RebuildHoleExposeDieModuleData>,
}

impl RebuildHoleExposeDie {
    /// Create a new RebuildHoleExposeDie module
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: Arc<RebuildHoleExposeDieModuleData>,
    ) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "RebuildHoleExposeDie"
    }

    /// Create the rebuild hole object
    fn create_hole(&self, dying_object: &Object) -> Option<Arc<RwLock<Object>>> {
        let hole_name = &self.base.module_data.hole_name;

        let position = dying_object.get_position();
        let geometry = dying_object.get_geometry_info().clone();
        let orientation = dying_object.get_orientation();

        let Some(template) = TheThingFactory::find_template(hole_name.as_str()) else {
            return None;
        };

        let factory = match TheThingFactory::get() {
            Ok(factory) => factory,
            Err(_) => return None,
        };

        let hole = match dying_object
            .get_team()
            .as_ref()
            .and_then(|team| team.read().ok())
        {
            Some(team_guard) => factory.new_object(template, &*team_guard),
            None => factory.new_object_optional_team(template, None),
        }
        .ok()?;

        if let Ok(mut hole_guard) = hole.write() {
            hole_guard.set_geometry_info(geometry);
            let _ = hole_guard.set_position(position);
            let _ = hole_guard.set_orientation(orientation);
        }

        if let Ok(hole_guard) = hole.read() {
            let name = dying_object.get_name().clone();
            if !name.is_empty() {
                let _ = transfer_object_name(&name, hole_guard.get_id());
            }
        }

        if let Ok(hole_guard) = hole.read() {
            if let Some(body) = hole_guard.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    if let Err(_err) = body_guard.set_max_health(
                        self.base.module_data.hole_max_health,
                        MaxHealthChangeType::FullyHeal,
                    ) {
                        // Keep C++ behavior: failure to apply health here is non-fatal.
                    }
                }
            }
        }

        Some(hole)
    }

    /// Transfer attackers from dying building to the hole
    fn transfer_attackers(
        &self,
        old_object_id: crate::common::ObjectID,
        new_object_id: crate::common::ObjectID,
    ) {
        if !self.base.module_data.transfer_attackers {
            return;
        }

        let Ok(game_logic) = get_game_logic().lock() else {
            return;
        };

        let mut current = game_logic.get_first_object();
        while let Some(obj) = current {
            let next = if let Ok(obj_guard) = obj.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.transfer_attack(old_object_id, new_object_id);
                    }
                }
                obj_guard.get_next_object()
            } else {
                None
            };

            current = next;
        }
    }
}

impl DieModuleInterface for RebuildHoleExposeDie {
    /// Called when the structure dies - creates rebuild hole
    /// (Matches C++ RebuildHoleExposeDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        if let Some(player) = object.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if player_guard.get_player_type() == crate::player::PlayerType::Neutral {
                    return;
                }
                if !player_guard.is_player_active() {
                    return;
                }
            }
        } else {
            return;
        }

        if object
            .get_status_bits()
            .test(ObjectStatusTypes::UnderConstruction)
        {
            return;
        }

        // Create the rebuild hole
        if let Some(hole_arc) = self.create_hole(object) {
            if let Ok(hole) = hole_arc.read() {
                let hole_id = hole.get_id();
                let pos = *hole.get_position();
                if let Ok(ai_guard) = THE_AI.read() {
                    if let Some(pathfinder) = ai_guard.pathfinder() {
                        if let Ok(mut pf) = pathfinder.write() {
                            pf.add_object_to_map(hole_id, &[pos], false);
                        }
                    }
                }

                for behavior in hole.get_behavior_modules() {
                    let Ok(mut behavior_guard) = behavior.lock() else {
                        continue;
                    };
                    if let Some(rebuild) = behavior_guard.get_rebuild_hole_behavior_interface() {
                        rebuild.start_rebuild_process(
                            Arc::clone(object.get_template()),
                            object.get_id(),
                        );
                        break;
                    }
                }
            }

            // Transfer attackers if requested
            if let Ok(hole) = hole_arc.read() {
                let old_id = object.get_id();
                let new_id = hole.get_id();
                self.transfer_attackers(old_id, new_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebuild_hole_expose_die_module_data_default() {
        let data = RebuildHoleExposeDieModuleData::default();
        assert!(data.hole_name.is_empty());
        assert_eq!(data.hole_max_health, 0.0);
        assert_eq!(data.transfer_attackers, true);
    }

    #[test]
    fn test_rebuild_hole_expose_die_module_name() {
        assert_eq!(
            RebuildHoleExposeDie::get_module_name(),
            "RebuildHoleExposeDie"
        );
    }

    #[test]
    fn test_rebuild_hole_expose_die_with_config() {
        let mut data = RebuildHoleExposeDieModuleData::default();
        data.hole_name = AsciiString::from("RebuildHole_TechBuilding");
        data.hole_max_health = 100.0;
        data.transfer_attackers = true;

        assert_eq!(data.hole_name.as_str(), "RebuildHole_TechBuilding");
        assert_eq!(data.hole_max_health, 100.0);
        assert_eq!(data.transfer_attackers, true);
    }
}
