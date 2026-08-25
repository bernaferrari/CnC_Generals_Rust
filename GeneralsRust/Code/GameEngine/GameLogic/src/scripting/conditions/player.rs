//! Player script conditions.

use super::helpers::{
    compare_f64, compare_i64, dual_world_registry_unavailable, event_type_from_name,
    get_player_arc, get_str_param, get_str_param_optional, lookup_named_object_id,
    parse_nested_condition, parse_object_status_mask, perform_comparison, with_script_engine_mut,
};
use super::{ConditionRegistry, ScriptCondition, ScriptContext, ScriptValue};
use crate::common::{Coord3D, KindOf, LOGICFRAMES_PER_SECOND, Relationship};
use crate::helpers::{TheGameLogic, ThePartitionManager, TheVictoryConditions};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::{Player, PlayerType, player_list};
use crate::scripting::engine::{
    get_area_tracker, get_event_manager, get_named_object_tracker, get_script_engine,
};
use crate::scripting::events::{EventFilter, GameEventType};
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::upgrade::center::get_upgrade_center;
use crate::{GameLogicError, GameLogicResult};
use async_trait::async_trait;
use game_engine::common::rts::{SCIENCE_INVALID, get_science_store};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// Built-in condition implementations

/// Player alive condition
pub(super) struct PlayerAliveCondition;

#[async_trait]
impl ScriptCondition for PlayerAliveCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;

        log::debug!("Checking if player {} is alive", player);

        // Check actual player state using player_list
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    // Player is alive if not defeated
                    return Ok(!player_guard.is_defeated());
                }
            }
        }

        // If player not found, consider them not alive
        Ok(false)
    }

    fn name(&self) -> &str {
        "player_alive"
    }

    fn description(&self) -> &str {
        "Checks if a player is still alive in the game"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player defeated condition
pub(super) struct PlayerDefeatedCondition;

#[async_trait]
impl ScriptCondition for PlayerDefeatedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;

        log::debug!("Checking if player {} is defeated", player);

        // Check actual player defeated state
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    return Ok(player_guard.is_defeated());
                }
            }
        }

        // If player not found, consider them defeated
        Ok(true)
    }

    fn name(&self) -> &str {
        "player_defeated"
    }

    fn description(&self) -> &str {
        "Checks if a player has been defeated"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has resource condition
pub(super) struct PlayerHasResourceCondition;

#[async_trait]
impl ScriptCondition for PlayerHasResourceCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let resource_type =
            crate::scripting::actions::get_string_param(parameters, "resource_type")?;
        let amount = crate::scripting::actions::get_int_param(parameters, "amount")?;

        log::debug!(
            "Checking if player {} has {} {}",
            player,
            amount,
            resource_type
        );

        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    if crate::scripting::actions::is_money_resource(&resource_type) {
                        let player_money = player_guard.get_money().get_money() as i64;
                        return Ok(player_money >= amount);
                    }
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "player_has_resource"
    }

    fn description(&self) -> &str {
        "Checks if a player has a certain amount of resources"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "resource_type".to_string(),
            "amount".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has units condition
pub(super) struct PlayerHasUnitsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasUnitsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let unit_type = crate::scripting::actions::get_string_param(parameters, "unit_type")?;
        let count = crate::scripting::actions::get_int_param(parameters, "count")?;

        log::debug!(
            "Checking if player {} has {} units of type '{}'",
            player,
            count,
            unit_type
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };

        let owned = manager.get_objects_owned_by_player(player_id);
        let mut matches = 0i64;
        for object_id in owned {
            let Some(obj_arc) = manager.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let __base_arc = obj_guard.base();
            let Ok(base_guard) = __base_arc.read() else {
                continue;
            };
            if base_guard.is_destroyed() {
                continue;
            }

            let Some(template) = obj_guard.template.as_ref() else {
                continue;
            };

            if template.is_kind_of(KindOf::Structure) || template.is_kind_of(KindOf::Building) {
                continue;
            }

            if template
                .get_name()
                .as_str()
                .eq_ignore_ascii_case(unit_type.as_str())
            {
                matches += 1;
                if matches >= count {
                    return Ok(true);
                }
            }
        }

        Ok(matches >= count)
    }

    fn name(&self) -> &str {
        "player_has_units"
    }

    fn description(&self) -> &str {
        "Checks if a player has a certain number of specific units"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has buildings condition
pub(super) struct PlayerHasBuildingsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasBuildingsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let building_type =
            crate::scripting::actions::get_string_param(parameters, "building_type")?;
        let count = crate::scripting::actions::get_int_param(parameters, "count")?;

        log::debug!(
            "Checking if player {} has {} buildings of type '{}'",
            player,
            count,
            building_type
        );

        let player_id: u32 = player
            .try_into()
            .map_err(|_| GameLogicError::Configuration("Invalid player id".to_string()))?;

        let obj_manager = get_object_manager();
        let Ok(manager) = obj_manager.read() else {
            return Ok(false);
        };

        let owned = manager.get_objects_owned_by_player(player_id);
        let mut matches = 0i64;
        for object_id in owned {
            let Some(obj_arc) = manager.get_object(object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let __base_arc = obj_guard.base();
            let Ok(base_guard) = __base_arc.read() else {
                continue;
            };
            if base_guard.is_destroyed() {
                continue;
            }

            let Some(template) = obj_guard.template.as_ref() else {
                continue;
            };

            let is_building =
                template.is_kind_of(KindOf::Structure) || template.is_kind_of(KindOf::Building);
            if !is_building {
                continue;
            }

            if template
                .get_name()
                .as_str()
                .eq_ignore_ascii_case(building_type.as_str())
            {
                matches += 1;
                if matches >= count {
                    return Ok(true);
                }
            }
        }

        Ok(matches >= count)
    }

    fn name(&self) -> &str {
        "player_has_buildings"
    }

    fn description(&self) -> &str {
        "Checks if a player has a certain number of specific buildings"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "building_type".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Players allied condition
pub(super) struct PlayersAlliedCondition;

#[async_trait]
impl ScriptCondition for PlayersAlliedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player1 = crate::scripting::actions::get_int_param(parameters, "player1")?;
        let player2 = crate::scripting::actions::get_int_param(parameters, "player2")?;

        log::debug!("Checking if players {} and {} are allied", player1, player2);
        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(p1) = list.get_player(player1 as i32) else {
            return Ok(false);
        };
        let Some(p2) = list.get_player(player2 as i32) else {
            return Ok(false);
        };
        let (Ok(p1_guard), Ok(p2_guard)) = (p1.read(), p2.read()) else {
            return Ok(false);
        };

        let rel = p1_guard.get_relationship(&p2_guard);
        Ok(matches!(rel, crate::common::Relationship::Allies))
    }

    fn name(&self) -> &str {
        "players_allied"
    }

    fn description(&self) -> &str {
        "Checks if two players are allied"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player1".to_string(), "player2".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has technology condition
pub(super) struct PlayerHasTechnologyCondition;

#[async_trait]
impl ScriptCondition for PlayerHasTechnologyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let technology = crate::scripting::actions::get_string_param(parameters, "technology")?;

        log::debug!(
            "Checking if player {} has technology '{}'",
            player,
            technology
        );
        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(player as i32) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let Some(store) = get_science_store() else {
            return Ok(false);
        };
        let science = store.get_science_from_internal_name(technology.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }
        Ok(player_guard.has_science(science))
    }

    fn name(&self) -> &str {
        "player_has_technology"
    }

    fn description(&self) -> &str {
        "Checks if a player has researched a technology"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "technology".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player has upgrade condition
pub(super) struct PlayerHasUpgradeCondition;

#[async_trait]
impl ScriptCondition for PlayerHasUpgradeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let upgrade = crate::scripting::actions::get_string_param(parameters, "upgrade")?;

        log::debug!("Checking if player {} has upgrade '{}'", player, upgrade);
        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(player as i32) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let upgrade_center = get_upgrade_center();
        let Ok(center) = upgrade_center.read() else {
            return Ok(false);
        };
        let Some(template) = center.find_upgrade(upgrade.as_str()) else {
            return Ok(false);
        };
        Ok(player_guard.has_upgrade_complete(&template))
    }

    fn name(&self) -> &str {
        "player_has_upgrade"
    }

    fn description(&self) -> &str {
        "Checks if a player has an upgrade"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "upgrade".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Special power available condition
pub(super) struct SpecialPowerAvailableCondition;

#[async_trait]
impl ScriptCondition for SpecialPowerAvailableCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let power_name = crate::scripting::actions::get_string_param(parameters, "power_name")?;

        log::debug!(
            "Checking if special power '{}' is available for player {}",
            power_name,
            player
        );

        let player_id: crate::common::ObjectID = if player >= 0 {
            player as crate::common::ObjectID
        } else {
            return Ok(false);
        };

        let Some(registry_lock) = crate::special_power_module::get_power_registry() else {
            return Ok(false);
        };
        let Ok(registry) = registry_lock.read() else {
            return Ok(false);
        };

        let power_name_lower = power_name.to_ascii_lowercase();
        for power in registry.get_all_powers() {
            let Ok(power) = power.lock() else {
                continue;
            };
            if power.get_data().name.to_string().to_ascii_lowercase() != power_name_lower {
                continue;
            }
            if power.get_data().check_prerequisites(player_id) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "special_power_available"
    }

    fn description(&self) -> &str {
        "Checks if a special power is available for use"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Resources Exceed Condition - Player has more resources than threshold
pub(super) struct ResourcesExceedCondition;

#[async_trait]
impl ScriptCondition for ResourcesExceedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let amount = crate::scripting::actions::get_int_param(parameters, "amount")?;

        log::debug!(
            "Checking if player {} has more than {} resources",
            player,
            amount
        );

        // Get actual player resources (money)
        // In C++: pPlayer->Get_Money() > amount
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    let player_money = player_guard.get_money().get_money() as i64;
                    return Ok(player_money > amount);
                }
            }
        }

        // If player not found, return false
        Ok(false)
    }

    fn name(&self) -> &str {
        "resources_exceed"
    }

    fn description(&self) -> &str {
        "Checks if player resources exceed a threshold"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Structure Built Condition - Player has built specific building
pub(super) struct StructureBuiltCondition;

#[async_trait]
impl ScriptCondition for StructureBuiltCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let building_type =
            crate::scripting::actions::get_string_param(parameters, "building_type")?;

        log::debug!(
            "Checking if player {} has built building '{}'",
            player,
            building_type
        );

        // Query player's built structures
        // In C++: Check if pPlayer has building of type in built list
        let obj_manager = get_object_manager();
        if let Ok(manager) = obj_manager.read() {
            let owned_objects = manager.get_objects_owned_by_player(player as u32);
            for obj_id in owned_objects {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    let (template_name, base_arc) = match obj_arc.read() {
                        Ok(obj) => (
                            obj.template.as_ref().map(|t| t.get_name().to_string()),
                            Some(obj.base()),
                        ),
                        Err(_) => (None, None),
                    };
                    if let (Some(template_name), Some(base_arc)) = (template_name, base_arc) {
                        if let Ok(base) = base_arc.read() {
                            // Check if it's a structure and matches the building type
                            if template_name.eq_ignore_ascii_case(&building_type) {
                                use crate::common::KindOf;
                                if base.is_kind_of(KindOf::Structure) {
                                    return Ok(true);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "structure_built"
    }

    fn description(&self) -> &str {
        "Checks if player has built a specific structure"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "building_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Unit Type Count Exceeds - Player has N or more units of type
pub(super) struct UnitTypeCountExceedsCondition;

#[async_trait]
impl ScriptCondition for UnitTypeCountExceedsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let unit_type = crate::scripting::actions::get_string_param(parameters, "unit_type")?;
        let count = crate::scripting::actions::get_int_param(parameters, "count")?;

        log::debug!(
            "Checking if player {} has more than {} units of type '{}'",
            player,
            count,
            unit_type
        );

        // Count player's units of this type
        // In C++: Count objects owned by player with matching template
        let obj_manager = get_object_manager();
        let mut actual_count = 0i64;

        if let Ok(manager) = obj_manager.read() {
            let owned_objects = manager.get_objects_owned_by_player(player as u32);
            for obj_id in owned_objects {
                if let Some(obj_arc) = manager.get_object(obj_id) {
                    if let Ok(obj) = obj_arc.read() {
                        if let Some(template) = &obj.template {
                            if template.get_name().eq_ignore_ascii_case(&unit_type) {
                                actual_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(actual_count > count)
    }

    fn name(&self) -> &str {
        "unit_type_count_exceeds"
    }

    fn description(&self) -> &str {
        "Checks if player has more than N units of a specific type"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "unit_type".to_string(),
            "count".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Player Won - Matches C++ victory condition check
pub(super) struct PlayerWonCondition;

#[async_trait]
impl ScriptCondition for PlayerWonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;

        log::debug!("Checking if player {} has won", player);

        // C++ parity behavior for mission/skirmish checks:
        // a player is considered "won" if they are still active and all other
        // non-observer, non-neutral players are defeated.
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(player_arc) = list.get_player(player as i32) {
                if let Ok(player_guard) = player_arc.read() {
                    if player_guard.is_defeated()
                        || player_guard.is_player_observer()
                        || player_guard.get_player_type() == PlayerType::Neutral
                    {
                        return Ok(false);
                    }

                    let this_player_index = player_guard.get_player_index();
                    drop(player_guard);

                    for other_arc in list.iter() {
                        let Ok(other_guard) = other_arc.read() else {
                            continue;
                        };
                        if other_guard.get_player_index() == this_player_index {
                            continue;
                        }
                        if other_guard.is_player_observer()
                            || other_guard.get_player_type() == PlayerType::Neutral
                        {
                            continue;
                        }
                        if !other_guard.is_defeated() {
                            return Ok(false);
                        }
                    }

                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "player_won"
    }

    fn description(&self) -> &str {
        "Checks if player has achieved victory"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Allies With Team - Players are allied
pub(super) struct AlliesWithTeamCondition;

#[async_trait]
impl ScriptCondition for AlliesWithTeamCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player1 = crate::scripting::actions::get_int_param(parameters, "player1")?;
        let player2 = crate::scripting::actions::get_int_param(parameters, "player2")?;

        log::debug!("Checking if player {} and {} are allies", player1, player2);

        // Check player relationship
        // In C++: pPlayer1->Get_Relationship(pPlayer2) == ALLIES
        let player_list_lock = player_list();
        if let Ok(list) = player_list_lock.read() {
            if let Some(p1_arc) = list.get_player(player1 as i32) {
                if let Ok(p1) = p1_arc.read() {
                    if let Some(p2_arc) = list.get_player(player2 as i32) {
                        if let Ok(p2) = p2_arc.read() {
                            return Ok(p1.is_allied_with_player(&p2));
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "allies_with_team"
    }

    fn description(&self) -> &str {
        "Checks if two players/teams are allied"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player1".to_string(), "player2".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Research Complete - Player has completed science/upgrade
pub(super) struct ResearchCompleteCondition;

#[async_trait]
impl ScriptCondition for ResearchCompleteCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let science_name = crate::scripting::actions::get_string_param(parameters, "science_name")?;

        log::debug!(
            "Checking if player {} has completed research '{}'",
            player,
            science_name
        );

        let Ok(list) = player_list().read() else {
            return Ok(false);
        };
        let Some(player_arc) = list.get_player(player as i32) else {
            return Ok(false);
        };
        let Ok(player_guard) = player_arc.read() else {
            return Ok(false);
        };
        let Some(store) = get_science_store() else {
            return Ok(false);
        };
        let science = store.get_science_from_internal_name(science_name.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        Ok(player_guard.has_science(science))
    }

    fn name(&self) -> &str {
        "research_complete"
    }

    fn description(&self) -> &str {
        "Checks if player has completed a research/science"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Special Power Ready - Player can use special power
pub(super) struct SpecialPowerReadyCondition;

#[async_trait]
impl ScriptCondition for SpecialPowerReadyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player = crate::scripting::actions::get_int_param(parameters, "player")?;
        let power_name = crate::scripting::actions::get_string_param(parameters, "power_name")?;

        log::debug!(
            "Checking if special power '{}' is ready for player {}",
            power_name,
            player
        );

        let player_id: crate::common::ObjectID = if player >= 0 {
            player as crate::common::ObjectID
        } else {
            return Ok(false);
        };

        let Some(registry_lock) = crate::special_power_module::get_power_registry() else {
            return Ok(false);
        };
        let Ok(registry) = registry_lock.read() else {
            return Ok(false);
        };

        let power_name_lower = power_name.to_ascii_lowercase();
        for power in registry.get_all_powers() {
            let Ok(power) = power.lock() else {
                continue;
            };
            if power.get_data().name.to_string().to_ascii_lowercase() != power_name_lower {
                continue;
            }

            if !power.get_data().check_prerequisites(player_id) {
                continue;
            }

            if power.is_ready() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str {
        "special_power_ready"
    }

    fn description(&self) -> &str {
        "Checks if special power is available/ready for use"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//=================================================================================================
// C++ Parity Script Conditions
// Ported from GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptConditions.cpp
//=================================================================================================

//-------------------------------------------------------------------------------------------------
// PLAYER_ALL_DESTROYED - evaluateAllDestroyed
// Returns true if player has no objects (everything destroyed).
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerAllDestroyedCondition;

#[async_trait]
impl ScriptCondition for PlayerAllDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(true), // Non-existent player is all destroyed
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        Ok(!player.has_any_objects())
    }

    fn name(&self) -> &str {
        "player_all_destroyed"
    }
    fn description(&self) -> &str {
        "Checks if a player has no objects remaining (C++ PLAYER_ALL_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_CREDITS - evaluatePlayerHasCredits
// Compares player's money against a threshold using a comparison operator.
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerHasCreditsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasCreditsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let credits = crate::scripting::actions::get_int_param(parameters, "credits")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let money = player.get_money().count_money() as i64;
        Ok(perform_comparison(credits, &comparison, money))
    }

    fn name(&self) -> &str {
        "player_has_credits"
    }
    fn description(&self) -> &str {
        "Checks if player has credits matching comparison (C++ PLAYER_HAS_CREDITS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "credits".to_string(),
            "comparison".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_POWER - evaluatePlayerHasPower
// Returns true if player has sufficient power.
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerHasPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerHasPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        Ok(player.get_energy().has_sufficient_power())
    }

    fn name(&self) -> &str {
        "player_has_power"
    }
    fn description(&self) -> &str {
        "Checks if player has sufficient power (C++ PLAYER_HAS_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_NO_POWER - !evaluatePlayerHasPower
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerHasNoPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerHasNoPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        PlayerHasPowerCondition
            .evaluate(parameters, context)
            .await
            .map(|b| !b)
    }

    fn name(&self) -> &str {
        "player_has_no_power"
    }
    fn description(&self) -> &str {
        "Checks if player does NOT have sufficient power (C++ PLAYER_HAS_NO_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// BUILT_BY_PLAYER - evaluateBuiltByPlayer
//-------------------------------------------------------------------------------------------------
pub(super) struct BuiltByPlayerCondition;

#[async_trait]
impl ScriptCondition for BuiltByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_id = player.get_id() as u32;
        drop(player);

        let object_type = get_str_param(parameters, "object_type")?;

        // Search all objects for matching type owned by player
        // Host path: empty dual-world registry → no object residual.
        if OBJECT_REGISTRY.is_empty() {
            return Ok(false);
        }
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    if obj.is_effectively_dead() {
                        continue;
                    }
                    if let Some(owner_id) = obj.get_controlling_player_id() {
                        if owner_id == player_id {
                            let template_name = obj.get_template_name();
                            if template_name == object_type {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "built_by_player"
    }
    fn description(&self) -> &str {
        "Checks if player has built an object of a specific type (C++ BUILT_BY_PLAYER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["object_type".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_TRIGGERED_SPECIAL_POWER
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerTriggeredSpecialPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerTriggeredSpecialPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_triggered(
                player_index,
                &power_name,
                true,
                crate::common::INVALID_ID,
            )
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_triggered_special_power"
    }
    fn description(&self) -> &str {
        "Checks if player triggered a special power (C++ PLAYER_TRIGGERED_SPECIAL_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerTriggeredSpecialPowerFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerTriggeredSpecialPowerFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_triggered(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_triggered_special_power_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player triggered a special power from a named unit (C++ PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "power_name".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_MIDWAY_SPECIAL_POWER
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerMidwaySpecialPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerMidwaySpecialPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_midway(
                player_index,
                &power_name,
                true,
                crate::common::INVALID_ID,
            )
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_midway_special_power"
    }
    fn description(&self) -> &str {
        "Checks if player's special power is midway (C++ PLAYER_MIDWAY_SPECIAL_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerMidwaySpecialPowerFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerMidwaySpecialPowerFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_midway(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_midway_special_power_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player is midway through a special power from a named unit (C++ PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "power_name".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_COMPLETED_SPECIAL_POWER
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerCompletedSpecialPowerCondition;

#[async_trait]
impl ScriptCondition for PlayerCompletedSpecialPowerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_complete(
                player_index,
                &power_name,
                true,
                crate::common::INVALID_ID,
            )
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_completed_special_power"
    }
    fn description(&self) -> &str {
        "Checks if player completed a special power (C++ PLAYER_COMPLETED_SPECIAL_POWER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerCompletedSpecialPowerFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerCompletedSpecialPowerFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };
        let player_index = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?
            .get_player_index() as usize;

        Ok(with_script_engine_mut(|engine| {
            engine.is_special_power_complete(player_index, &power_name, true, source_id)
        })
        .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "player_completed_special_power_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player completed a special power from a named unit (C++ PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "power_name".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_BUILT_UPGRADE
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerBuiltUpgradeCondition;

#[async_trait]
impl ScriptCondition for PlayerBuiltUpgradeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let upgrade_name = get_str_param(parameters, "upgrade")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_index = player.get_player_index() as usize;
        let completed_mask = player.get_completed_upgrade_mask();
        let upgrade_mask = crate::upgrade::upgrade_mask_for_name(&upgrade_name);
        let mask_bits = crate::common::UpgradeMaskType::from_bits_retain(upgrade_mask.to_bits());
        let has_upgrade = completed_mask.intersects(mask_bits);
        drop(player);

        let engine_hit = with_script_engine_mut(|engine| {
            engine.is_upgrade_complete(player_index, &upgrade_name, true, crate::common::INVALID_ID)
        })
        .unwrap_or(false);

        Ok(engine_hit || has_upgrade)
    }

    fn name(&self) -> &str {
        "player_built_upgrade"
    }
    fn description(&self) -> &str {
        "Checks if player built an upgrade (C++ PLAYER_BUILT_UPGRADE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "upgrade".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_BUILT_UPGRADE_FROM_NAMED
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerBuiltUpgradeFromNamedCondition;

#[async_trait]
impl ScriptCondition for PlayerBuiltUpgradeFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let upgrade_name = get_str_param(parameters, "upgrade")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_index = player.get_player_index() as usize;
        let completed_mask = player.get_completed_upgrade_mask();
        let upgrade_mask = crate::upgrade::upgrade_mask_for_name(&upgrade_name);
        let mask_bits = crate::common::UpgradeMaskType::from_bits_retain(upgrade_mask.to_bits());
        let has_upgrade = completed_mask.intersects(mask_bits);
        drop(player);

        let engine_hit = with_script_engine_mut(|engine| {
            engine.is_upgrade_complete(player_index, &upgrade_name, true, source_id)
        })
        .unwrap_or(false);

        Ok(engine_hit || has_upgrade)
    }

    fn name(&self) -> &str {
        "player_built_upgrade_from_named"
    }
    fn description(&self) -> &str {
        "Checks if a player built an upgrade from a named unit (C++ PLAYER_BUILT_UPGRADE_FROM_NAMED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "upgrade".to_string(),
            "unit_name".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_ACQUIRED_SCIENCE
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerAcquiredScienceCondition;

#[async_trait]
impl ScriptCondition for PlayerAcquiredScienceCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let science_name = get_str_param(parameters, "science")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let Some(store) = get_science_store() else {
            return Ok(false);
        };
        let science = store.get_science_from_internal_name(science_name.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        let player_index = player.get_player_index() as usize;
        drop(player);

        Ok(
            with_script_engine_mut(|engine| {
                engine.is_science_acquired(player_index, science, true)
            })
            .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "player_acquired_science"
    }
    fn description(&self) -> &str {
        "Checks if player has acquired a science (C++ PLAYER_ACQUIRED_SCIENCE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_CAN_PURCHASE_SCIENCE
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerCanPurchaseScienceCondition;

#[async_trait]
impl ScriptCondition for PlayerCanPurchaseScienceCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let science_name = get_str_param(parameters, "science")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let Some(store) = get_science_store() else {
            return Ok(false);
        };
        let science = store.get_science_from_internal_name(science_name.as_str());
        if science == SCIENCE_INVALID {
            return Ok(false);
        }

        Ok(player.is_capable_of_purchasing_science(science))
    }

    fn name(&self) -> &str {
        "player_can_purchase_science"
    }
    fn description(&self) -> &str {
        "Checks if player can purchase a science (C++ PLAYER_CAN_PURCHASE_SCIENCE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_SCIENCEPURCHASEPOINTS
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerHasSciencePurchasePointsCondition;

#[async_trait]
impl ScriptCondition for PlayerHasSciencePurchasePointsCondition {
    async fn evaluate(
        self: &Self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let points = crate::scripting::actions::get_int_param(parameters, "points")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        Ok((player.get_science_purchase_points() as i64) >= points)
    }

    fn name(&self) -> &str {
        "player_has_science_purchase_points"
    }
    fn description(&self) -> &str {
        "Checks if player has enough science purchase points (C++ PLAYER_HAS_SCIENCEPURCHASEPOINTS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "points".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_POWER_COMPARE_PERCENT - evaluatePlayerHasComparisonPercentPower
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerPowerComparePercentCondition;

#[async_trait]
impl ScriptCondition for PlayerPowerComparePercentCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let percent = crate::scripting::actions::get_int_param(parameters, "percent")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let ratio = player.get_energy().supply_ratio();
        Ok(perform_comparison(
            (ratio * 100.0) as i64,
            &comparison,
            percent as i64,
        ))
    }

    fn name(&self) -> &str {
        "player_power_compare_percent"
    }
    fn description(&self) -> &str {
        "Compares player power supply ratio (C++ PLAYER_POWER_COMPARE_PERCENT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "percent".to_string(),
            "comparison".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_EXCESS_POWER_COMPARE_VALUE - evaluatePlayerHasComparisonValueExcessPower
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerExcessPowerCompareValueCondition;

#[async_trait]
impl ScriptCondition for PlayerExcessPowerCompareValueCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let kwh = crate::scripting::actions::get_int_param(parameters, "kwh")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let energy = player.get_energy();
        let actual_kwh = energy.production() - energy.consumption();
        Ok(perform_comparison(actual_kwh as i64, &comparison, kwh))
    }

    fn name(&self) -> &str {
        "player_excess_power_compare_value"
    }
    fn description(&self) -> &str {
        "Compares player excess power in KWH (C++ PLAYER_EXCESS_POWER_COMPARE_VALUE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "kwh".to_string(),
            "comparison".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_LOST_OBJECT_TYPE - evaluatePlayerLostObjectType
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerLostObjectTypeCondition;

#[async_trait]
impl ScriptCondition for PlayerLostObjectTypeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = get_player_arc(parameters, "player")?;
        let player = match player_arc {
            Some(p) => p,
            None => return Ok(false),
        };
        let player_index = {
            let p_guard = player
                .read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
            p_guard.get_player_index()
        };

        let object_type = get_str_param(parameters, "object_type")?;

        let current_count = get_script_engine()
            .read()
            .ok()
            .and_then(|engine| {
                engine
                    .as_ref()
                    .map(|engine| engine.get_object_count(player_index, &object_type))
            })
            .unwrap_or(0);

        let object_manager = get_object_manager();
        let sum_of_objs = object_manager
            .read()
            .ok()
            .map(|manager| {
                manager
                    .all_object_ids()
                    .into_iter()
                    .filter(|object_id| {
                        let Some(obj_arc) = manager.get_object(*object_id) else {
                            return false;
                        };
                        let Ok(obj_guard) = obj_arc.read() else {
                            return false;
                        };
                        if obj_guard.is_destroyed() {
                            return false;
                        }
                        let owner = {
                            let player = obj_guard.get_controlling_player();
                            player
                                .and_then(|p| p.read().ok().map(|g| g.get_player_index()))
                                .unwrap_or(-1)
                        };
                        if owner != player_index {
                            return false;
                        }
                        obj_guard
                            .template
                            .as_ref()
                            .map(|template| template.get_name() == object_type.as_str())
                            .unwrap_or(false)
                    })
                    .count() as i32
            })
            .unwrap_or(0);

        if sum_of_objs != current_count {
            if let Ok(mut engine_guard) = get_script_engine().write() {
                if let Some(ref mut engine) = *engine_guard {
                    engine.set_object_count(player_index, &object_type, sum_of_objs);
                }
            }
        }

        Ok(sum_of_objs < current_count)
    }

    fn name(&self) -> &str {
        "player_lost_object_type"
    }
    fn description(&self) -> &str {
        "Checks if player lost an object type (C++ PLAYER_LOST_OBJECT_TYPE)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "object_type".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// PLAYER_HAS_OBJECT_COMPARISON - C++ ScriptConditions::evaluatePlayerUnitCondition
// Counts objects the player owns matching the given thing template,
// then compares the count against the threshold using the given operator.
//-------------------------------------------------------------------------------------------------
pub(super) struct PlayerHasObjectComparisonCondition;

#[async_trait]
impl ScriptCondition for PlayerHasObjectComparisonCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let player_arc = get_player_arc(parameters, "player")?;
        let player = match player_arc {
            Some(p) => p,
            None => return Ok(false),
        };

        let comparison = get_str_param(parameters, "comparison")?;
        let count_threshold = match parameters.get("count").or(parameters.get("threshold")) {
            Some(ScriptValue::Int(v)) => *v,
            Some(ScriptValue::Float(v)) => *v as i64,
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'count' parameter".to_string(),
                ));
            }
        };

        // Get the object type name to match
        let object_type_name = get_str_param(parameters, "object_type")
            .or_else(|_| get_str_param(parameters, "unit_type"))?;

        // Iterate player's owned objects and count those matching the template name.
        // C++ uses countObjectsByThingTemplate which matches by template pointer;
        // we match by template name string which is equivalent for single-template queries.
        let player_guard = player
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;

        let mut count: i64 = 0;
        let all_objects = player_guard.get_all_objects();
        for obj_id in all_objects {
            let matches = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    obj_guard.is_alive() && obj_guard.get_template_name() == object_type_name
                })
                .unwrap_or(false);
            if matches {
                count += 1;
            }
        }

        Ok(perform_comparison(count, &comparison, count_threshold))
    }

    fn name(&self) -> &str {
        "player_has_object_comparison"
    }
    fn description(&self) -> &str {
        "Player has N objects of type, compared against threshold (C++ PLAYER_HAS_OBJECT_COMPARISON)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "comparison".to_string(),
            "count".to_string(),
            "object_type".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec!["unit_type".to_string(), "threshold".to_string()]
    }
}

pub(super) fn register_player_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(PlayerAliveCondition));
    registry.register_condition(Box::new(PlayerDefeatedCondition));
    registry.register_condition(Box::new(PlayerHasResourceCondition));
    registry.register_condition(Box::new(PlayerHasUnitsCondition));
    registry.register_condition(Box::new(PlayerHasBuildingsCondition));
    registry.register_condition(Box::new(PlayersAlliedCondition));
    registry.register_condition(Box::new(PlayerHasTechnologyCondition));
    registry.register_condition(Box::new(PlayerHasUpgradeCondition));
    registry.register_condition(Box::new(SpecialPowerAvailableCondition));
    registry.register_condition(Box::new(ResourcesExceedCondition));
    registry.register_condition(Box::new(StructureBuiltCondition));
    registry.register_condition(Box::new(UnitTypeCountExceedsCondition));
    registry.register_condition(Box::new(PlayerWonCondition));
    registry.register_condition(Box::new(AlliesWithTeamCondition));
    registry.register_condition(Box::new(ResearchCompleteCondition));
    registry.register_condition(Box::new(SpecialPowerReadyCondition));
    registry.register_condition(Box::new(PlayerAllDestroyedCondition));
    registry.register_condition(Box::new(PlayerHasCreditsCondition));
    registry.register_condition(Box::new(PlayerHasPowerCondition));
    registry.register_condition(Box::new(PlayerHasNoPowerCondition));
    registry.register_condition(Box::new(BuiltByPlayerCondition));
    registry.register_condition(Box::new(PlayerTriggeredSpecialPowerCondition));
    registry.register_condition(Box::new(PlayerTriggeredSpecialPowerFromNamedCondition));
    registry.register_condition(Box::new(PlayerMidwaySpecialPowerCondition));
    registry.register_condition(Box::new(PlayerMidwaySpecialPowerFromNamedCondition));
    registry.register_condition(Box::new(PlayerCompletedSpecialPowerCondition));
    registry.register_condition(Box::new(PlayerCompletedSpecialPowerFromNamedCondition));
    registry.register_condition(Box::new(PlayerBuiltUpgradeCondition));
    registry.register_condition(Box::new(PlayerBuiltUpgradeFromNamedCondition));
    registry.register_condition(Box::new(PlayerAcquiredScienceCondition));
    registry.register_condition(Box::new(PlayerCanPurchaseScienceCondition));
    registry.register_condition(Box::new(PlayerHasSciencePurchasePointsCondition));
    registry.register_condition(Box::new(PlayerPowerComparePercentCondition));
    registry.register_condition(Box::new(PlayerExcessPowerCompareValueCondition));
    registry.register_condition(Box::new(PlayerLostObjectTypeCondition));
    registry.register_condition(Box::new(PlayerHasObjectComparisonCondition));
}
