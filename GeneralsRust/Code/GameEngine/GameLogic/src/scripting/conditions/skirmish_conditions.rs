//! Skirmish AI Script Conditions
//!
//! Implements skirmish-specific script conditions used by AI players in skirmish mode.
//! These conditions enable the AI to make strategic decisions based on game state,
//! resources, enemy positions, and build capabilities.

use super::{get_player_arc, get_str_param, lookup_named_object_id, perform_comparison, ConditionRegistry, ScriptCondition, ScriptContext, ScriptValue};
use crate::common::{Coord3D, KindOf, LOGICFRAMES_PER_SECOND};
use crate::ai::integration::{with_ai_integration_mut, IntegratedAiPlayer};
use crate::helpers::{TheGameLogic, ThePartitionManager, TheThingFactory};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::{get_special_power_store, SpecialPowerTemplate};
use crate::object::Object;
use crate::object_manager::get_object_manager;
use crate::player::{player_list, GameDifficulty, PlayerType};
use crate::scripting::engine::get_area_tracker;
use crate::team::get_team_factory;
use crate::{GameLogicError, GameLogicResult};

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn has_hostile_object_near_owned_objects<F>(player: &crate::player::Player, radius: f32, filter: F) -> bool
where
    F: Fn(&Object) -> bool,
{
    let Some(partition) = ThePartitionManager::get() else {
        return false;
    };
    let Ok(players) = player_list().read() else {
        return false;
    };
    let player_index = player.get_player_index();

    for object_id in player.get_all_objects() {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            continue;
        };
        let Ok(obj) = obj_arc.read() else {
            continue;
        };
        if obj.is_destroyed() || obj.is_effectively_dead() || !filter(&obj) {
            continue;
        }

        let status = obj.get_status_bits();
        if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
            && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
            && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
        {
            continue;
        }

        for candidate_id in partition.get_objects_in_range(obj.get_position(), radius) {
            let Some(candidate_arc) = OBJECT_REGISTRY.get_object(candidate_id) else {
                continue;
            };
            let Ok(candidate) = candidate_arc.read() else {
                continue;
            };
            if candidate.is_destroyed() || candidate.is_effectively_dead() {
                continue;
            }
            let Some(owner_id) = candidate.get_controlling_player_id() else {
                continue;
            };
            if owner_id as i32 == player_index {
                continue;
            }
            let hostile = players
                .get_player(owner_id as i32)
                .cloned()
                .and_then(|owner_arc| owner_arc.read().ok().map(|owner| {
                    owner.get_player_type() != PlayerType::Neutral
                        && !owner.is_player_observer()
                        && !player.is_allied_with_player(&owner)
                }))
                .unwrap_or(true);
            if hostile {
                return true;
            }
        }
    }

    false
}

fn is_player_recently_under_attack(player: &crate::player::Player, window_frames: u32) -> bool {
    let current_frame = TheGameLogic::get_frame();
    let attacked_frame = player.get_attacked_frame();
    if attacked_frame != 0 && current_frame.saturating_sub(attacked_frame) <= window_frames {
        return true;
    }

    let Ok(players) = player_list().read() else {
        return false;
    };
    for (index, player_arc) in players.iter().enumerate() {
        if index as i32 == player.get_player_index() {
            continue;
        }
        if !player.get_attacked_by(index as i32) {
            continue;
        }
        if let Ok(other_player) = player_arc.read() {
            if other_player.get_player_type() != PlayerType::Neutral
                && !other_player.is_player_observer()
            {
                return true;
            }
        }
    }

    false
}

fn player_has_ready_special_power(
    player: &crate::player::Player,
    template: &SpecialPowerTemplate,
) -> bool {
    let required_science = template.get_required_science();
    if required_science != crate::common::science::SCIENCE_INVALID
        && !player.has_science(required_science)
    {
        return false;
    }

    let template_id = template.get_id();
    let template_name = template.get_name();

    for object_id in player.get_all_objects() {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            continue;
        };
        let Ok(obj) = obj_arc.read() else {
            continue;
        };

        if obj.is_destroyed()
            || obj.is_effectively_dead()
            || obj.is_disabled()
            || obj
                .get_status_bits()
                .contains(crate::common::ObjectStatusMaskType::UNDER_CONSTRUCTION)
        {
            continue;
        }

        if obj.get_special_power_module(template_id).is_none() {
            continue;
        }

        let Some(ready) = obj.with_special_power_module_interface_by_name(template_name, |module| {
            module.is_ready()
        }) else {
            continue;
        };

        if ready {
            return true;
        }
    }

    false
}

//-------------------------------------------------------------------------------------------------
// 1. SkirmishSpecialPowerReadyCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishSpecialPowerReadyCondition;

#[async_trait]
impl ScriptCondition for SkirmishSpecialPowerReadyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;

        let Some(store) = get_special_power_store() else {
            return Ok(false);
        };
        let Some(template) = store.find_special_power_template(power_name.as_str()) else {
            return Ok(false);
        };

        if player_has_ready_special_power(&player, template) {
            return Ok(true);
        }

        Ok(false)
    }

    fn name(&self) -> &str { "skirmish_special_power_ready" }
    fn description(&self) -> &str { "Checks if a special power is ready" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string(), "power_name".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// SkirmishSpecialPowerReadyFromNamedCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishSpecialPowerReadyFromNamedCondition;

#[async_trait]
impl ScriptCondition for SkirmishSpecialPowerReadyFromNamedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };
        let power_name = get_str_param(parameters, "power_name")?;
        let unit_name = get_str_param(parameters, "unit_name")?;
        let Some(source_id) = lookup_named_object_id(&unit_name)? else {
            return Ok(false);
        };

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;

        let Some(store) = get_special_power_store() else {
            return Ok(false);
        };
        let Some(template) = store.find_special_power_template(power_name.as_str()) else {
            return Ok(false);
        };

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(source_id) else {
            return Ok(false);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(false);
        };
        if obj.is_destroyed()
            || obj.is_effectively_dead()
            || obj.is_disabled()
            || obj
                .get_status_bits()
                .contains(crate::common::ObjectStatusMaskType::UNDER_CONSTRUCTION)
        {
            return Ok(false);
        }
        if obj.get_special_power_module(template.get_id()).is_none() {
            return Ok(false);
        }

        Ok(obj
            .with_special_power_module_interface_by_name(template.get_name(), |module| {
                let required_science = template.get_required_science();
                (required_science == crate::common::science::SCIENCE_INVALID
                    || player.has_science(required_science))
                    && module.is_ready()
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str { "skirmish_special_power_ready_from_named" }
    fn description(&self) -> &str { "Checks if a named unit has a ready special power" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "power_name".to_string(), "unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 2. SkirmishCommandButtonReadyCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishCommandButtonReadyCondition;

#[async_trait]
impl ScriptCondition for SkirmishCommandButtonReadyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = get_str_param(parameters, "team")?;
        let factory = get_team_factory();
        let factory_guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to lock team factory: {}", e))
        })?;

        let teams = factory_guard.find_team_instances(&team_name);
        for team_arc in &teams {
            let team: std::sync::RwLockReadGuard<'_, crate::team::Team> = team_arc.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read team: {}", e))
            })?;
            if !team.has_any_objects() {
                return Ok(false);
            }
            // Check that at least one member is alive
            for &member_id in team.get_members() {
                if let Some(obj_arc) = OBJECT_REGISTRY.get_object(member_id) {
                    if let Ok(obj) = obj_arc.read() {
                        if !obj.is_effectively_dead() && !obj.is_destroyed() {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str { "skirmish_command_button_ready" }
    fn description(&self) -> &str { "Checks if a team has alive members ready for commands" }
    fn required_parameters(&self) -> Vec<String> { vec!["team".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 3. SkirmishEasyAiCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishEasyAiCondition;

#[async_trait]
impl ScriptCondition for SkirmishEasyAiCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        Ok(player.get_player_type() == PlayerType::Computer
            && player.get_player_difficulty() == GameDifficulty::Easy)
    }

    fn name(&self) -> &str { "skirmish_easy_ai" }
    fn description(&self) -> &str { "Checks if player is an easy AI" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 4. SkirmishMediumAiCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishMediumAiCondition;

#[async_trait]
impl ScriptCondition for SkirmishMediumAiCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        Ok(player.get_player_type() == PlayerType::Computer
            && player.get_player_difficulty() == GameDifficulty::Normal)
    }

    fn name(&self) -> &str { "skirmish_medium_ai" }
    fn description(&self) -> &str { "Checks if player is a medium AI" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 5. SkirmishHardAiCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishHardAiCondition;

#[async_trait]
impl ScriptCondition for SkirmishHardAiCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        Ok(player.get_player_type() == PlayerType::Computer
            && player.get_player_difficulty() == GameDifficulty::Hard)
    }

    fn name(&self) -> &str { "skirmish_hard_ai" }
    fn description(&self) -> &str { "Checks if player is a hard AI" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 6. SkirmishPlayerIsAiCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishPlayerIsAiCondition;

#[async_trait]
impl ScriptCondition for SkirmishPlayerIsAiCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        Ok(player.get_player_type() == PlayerType::Computer)
    }

    fn name(&self) -> &str { "skirmish_player_is_ai" }
    fn description(&self) -> &str { "Checks if player is any AI type" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 7. SkirmishHasEnoughMoneyCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishHasEnoughMoneyCondition;

#[async_trait]
impl ScriptCondition for SkirmishHasEnoughMoneyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let amount = super::super::actions::get_int_param(parameters, "amount")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let money = player.get_money().count_money() as i64;
        Ok(perform_comparison(money, &comparison, amount))
    }

    fn name(&self) -> &str { "skirmish_has_enough_money" }
    fn description(&self) -> &str { "Checks if player has enough money with comparison" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "amount".to_string(), "comparison".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 8. SkirmishNeedsSupplyCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishNeedsSupplyCondition;

#[async_trait]
impl ScriptCondition for SkirmishNeedsSupplyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_id = player.get_player_index() as u32;
        let money = player.get_money().count_money();
        let is_skirmish_ai = player.is_skirmish_ai();
        drop(player);

        if is_skirmish_ai {
            if let Some(result) = with_ai_integration_mut(|manager| {
                manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                    IntegratedAiPlayer::Standard(ai) => !ai.is_supply_source_safe(2000),
                    IntegratedAiPlayer::Skirmish(ai) => !ai.is_supply_source_safe(2000),
                })
            })
            .flatten()
            {
                return Ok(result);
            }
        }

        Ok(money < 2000)
    }

    fn name(&self) -> &str { "skirmish_needs_supply" }
    fn description(&self) -> &str { "Checks if player needs supply" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 9. SkirmishBuildingsDestroyedCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishBuildingsDestroyedCondition;

#[async_trait]
impl ScriptCondition for SkirmishBuildingsDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let count = super::super::actions::get_int_param(parameters, "count")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_id = player.get_id() as u32;

        drop(player); // release lock before accessing object manager

        let manager = get_object_manager();
        let mgr = manager.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read object manager: {}", e))
        })?;
        let owned = mgr.get_objects_owned_by_player(player_id);

        let mut destroyed_count: i64 = 0;
        for obj_id in &owned {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    if obj.is_kind_of(KindOf::Structure) && obj.is_destroyed() {
                        destroyed_count += 1;
                    }
                }
            }
        }

        Ok(perform_comparison(destroyed_count, &comparison, count))
    }

    fn name(&self) -> &str { "skirmish_buildings_destroyed" }
    fn description(&self) -> &str { "Counts player's destroyed structures with comparison" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "count".to_string(), "comparison".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 10. SkirmishUnitsDestroyedCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishUnitsDestroyedCondition;

#[async_trait]
impl ScriptCondition for SkirmishUnitsDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let count = super::super::actions::get_int_param(parameters, "count")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_id = player.get_id() as u32;

        drop(player);

        let manager = get_object_manager();
        let mgr = manager.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read object manager: {}", e))
        })?;
        let owned = mgr.get_objects_owned_by_player(player_id);

        let mut destroyed_count: i64 = 0;
        for obj_id in &owned {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Count units that are not structures
                    if !obj.is_kind_of(KindOf::Structure) && obj.is_destroyed() {
                        destroyed_count += 1;
                    }
                }
            }
        }

        Ok(perform_comparison(destroyed_count, &comparison, count))
    }

    fn name(&self) -> &str { "skirmish_units_destroyed" }
    fn description(&self) -> &str { "Counts player's destroyed units with comparison" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "count".to_string(), "comparison".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 11. SkirmishEnemyInAreaCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishEnemyInAreaCondition;

#[async_trait]
impl ScriptCondition for SkirmishEnemyInAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let area_name = get_str_param(parameters, "area")?;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_id = player.get_id() as u32;

        drop(player);

        let tracker = get_area_tracker();
        let objects = tracker.get_objects_in_area(&area_name)?;

        for obj_id in &objects {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Skip effectively dead or destroyed objects
                    if obj.is_effectively_dead() || obj.is_destroyed() {
                        continue;
                    }
                    // Check if this object is controlled by a different (enemy) player
                    if let Some(owner_id) = obj.get_controlling_player_id() {
                        if owner_id != player_id {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str { "skirmish_enemy_in_area" }
    fn description(&self) -> &str { "Checks if enemy units are in the specified area" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "area".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 12. SkirmishAllUnitsGarrisonedCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishAllUnitsGarrisonedCondition;

#[async_trait]
impl ScriptCondition for SkirmishAllUnitsGarrisonedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = get_str_param(parameters, "team")?;
        let factory = get_team_factory();
        let factory_guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to lock team factory: {}", e))
        })?;

        let teams = factory_guard.find_team_instances(&team_name);
        if teams.is_empty() {
            return Ok(true); // No teams = vacuously true
        }

        for team_arc in &teams {
            let team: std::sync::RwLockReadGuard<'_, crate::team::Team> = team_arc.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read team: {}", e))
            })?;
            let members = team.get_members();
            if members.is_empty() {
                continue;
            }

            for &member_id in members {
                if let Some(obj_arc) = OBJECT_REGISTRY.get_object(member_id) {
                    if let Ok(obj) = obj_arc.read() {
                        if obj.is_effectively_dead() || obj.is_destroyed() {
                            continue; // Dead units don't need to be garrisoned
                        }
                        // Check if the object is disabled by Held type (garrisoned)
                        if !obj.is_disabled_by_type(crate::common::DisabledType::Held) {
                            return Ok(false);
                        }
                    }
                }
                // Object not in registry - assume dead, skip
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str { "skirmish_all_units_garrisoned" }
    fn description(&self) -> &str { "Checks if all team units are garrisoned" }
    fn required_parameters(&self) -> Vec<String> { vec!["team".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 13. SkirmishBaseUnderAttackCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishBaseUnderAttackCondition;

#[async_trait]
impl ScriptCondition for SkirmishBaseUnderAttackCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;

        if is_player_recently_under_attack(&player, 90) {
            return Ok(true);
        }

        if has_hostile_object_near_owned_objects(&player, 250.0, |obj| {
            obj.is_kind_of(KindOf::Structure)
        }) {
            return Ok(true);
        }

        Ok(has_hostile_object_near_owned_objects(&player, 250.0, |_| true))
    }

    fn name(&self) -> &str { "skirmish_base_under_attack" }
    fn description(&self) -> &str { "Checks if player's base is under attack" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 14. SkirmishSupplySourceAttackedCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishSupplySourceAttackedCondition;

#[async_trait]
impl ScriptCondition for SkirmishSupplySourceAttackedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_id = player.get_player_index() as u32;
        let is_skirmish_ai = player.is_skirmish_ai();

        if is_skirmish_ai {
            if let Some(result) = with_ai_integration_mut(|manager| {
                manager.with_ai_player_mut(player_id, |ai_player| match ai_player {
                    IntegratedAiPlayer::Standard(ai) => ai.is_supply_source_attacked(),
                    IntegratedAiPlayer::Skirmish(ai) => ai.is_supply_source_attacked(),
                })
            })
            .flatten()
            {
                return Ok(result);
            }
        }

        Ok(has_hostile_object_near_owned_objects(&player, 120.0, |obj| {
            obj.is_kind_of(KindOf::SupplySource)
                || obj.is_kind_of(KindOf::ResourceNode)
                || obj.is_kind_of(KindOf::FSSupplyCenter)
                || obj.is_kind_of(KindOf::FSSupplyDropzone)
                || obj.is_kind_of(KindOf::Refinery)
        }))
    }

    fn name(&self) -> &str { "skirmish_supply_source_attacked" }
    fn description(&self) -> &str { "Checks if a supply source is being attacked" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 15. SkirmishCanBuildCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishCanBuildCondition;

#[async_trait]
impl ScriptCondition for SkirmishCanBuildCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };
        let object_name = get_str_param(parameters, "object_name")?;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let Some(template) = TheThingFactory::find_template(object_name.as_str()) else {
            return Ok(false);
        };

        Ok(player.can_build_template(template.as_ref()))
    }

    fn name(&self) -> &str { "skirmish_can_build" }
    fn description(&self) -> &str { "Checks if player can build a specific object" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "object_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 16. SkirmishCanReinforceCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishCanReinforceCondition;

#[async_trait]
impl ScriptCondition for SkirmishCanReinforceCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;

        Ok(player.is_skirmish_ai()
            && !player.is_defeated()
            && player.get_current_enemy_player_index().is_some()
            && (player.get_can_build_units() || player.get_can_build_base())
            && player.get_money().count_money() > 0)
    }

    fn name(&self) -> &str { "skirmish_can_reinforce" }
    fn description(&self) -> &str { "Checks if player can reinforce" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 17. SkirmishTeamNearPositionCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishTeamNearPositionCondition;

#[async_trait]
impl ScriptCondition for SkirmishTeamNearPositionCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = get_str_param(parameters, "team")?;
        let x = super::super::actions::get_float_param(parameters, "x")? as f32;
        let y = super::super::actions::get_float_param(parameters, "y")? as f32;
        let radius = super::super::actions::get_float_param(parameters, "radius")? as f32;

        let center = Coord3D { x, y, z: 0.0 };

        let factory = get_team_factory();
        let factory_guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to lock team factory: {}", e))
        })?;

        let teams = factory_guard.find_team_instances(&team_name);
        for team_arc in &teams {
            let team: std::sync::RwLockReadGuard<'_, crate::team::Team> = team_arc.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read team: {}", e))
            })?;
            for &member_id in team.get_members() {
                if let Some(obj_arc) = OBJECT_REGISTRY.get_object(member_id) {
                    if let Ok(obj) = obj_arc.read() {
                        if obj.is_effectively_dead() || obj.is_destroyed() {
                            continue;
                        }
                        let pos = obj.get_position();
                        let dx = pos.x - center.x;
                        let dy = pos.y - center.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist <= radius {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str { "skirmish_team_near_position" }
    fn description(&self) -> &str { "Checks if any member of a team is near a position" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "x".to_string(), "y".to_string(), "radius".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 18. SkirmishPlayerHasScienceCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishPlayerHasScienceCondition;

#[async_trait]
impl ScriptCondition for SkirmishPlayerHasScienceCondition {
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

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;

        // Use the science store to look up the science type by name
        let science_store = game_engine::common::rts::get_science_store();
        let has_it = if let Some(store) = science_store {
            let science_type = store.get_science_from_internal_name(&science_name);
            player.has_science(science_type)
        } else {
            false
        };

        Ok(has_it)
    }

    fn name(&self) -> &str { "skirmish_player_has_science" }
    fn description(&self) -> &str { "Checks if player has a specific science" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "science".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 19. SkirmishPlayerHasUpgradeCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishPlayerHasUpgradeCondition;

#[async_trait]
impl ScriptCondition for SkirmishPlayerHasUpgradeCondition {
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

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;

        // Check the upgrade bitmask
        let mask_bit = crate::upgrade::upgrade_mask_for_name(&upgrade_name);
        let completed_mask = player.get_completed_upgrade_mask();
        Ok(completed_mask.bits() & mask_bit.bits() != 0)
    }

    fn name(&self) -> &str { "skirmish_player_has_upgrade" }
    fn description(&self) -> &str { "Checks if player has completed a specific upgrade" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "upgrade".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 20. SkirmishStructureCountCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishStructureCountCondition;

#[async_trait]
impl ScriptCondition for SkirmishStructureCountCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let count = super::super::actions::get_int_param(parameters, "count")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_id = player.get_id() as u32;

        drop(player);

        let manager = get_object_manager();
        let mgr = manager.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read object manager: {}", e))
        })?;
        let owned = mgr.get_objects_owned_by_player(player_id);

        let mut structure_count: i64 = 0;
        for obj_id in &owned {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    if obj.is_kind_of(KindOf::Structure)
                        && !obj.is_destroyed()
                        && !obj.is_effectively_dead()
                    {
                        structure_count += 1;
                    }
                }
            }
        }

        Ok(perform_comparison(structure_count, &comparison, count))
    }

    fn name(&self) -> &str { "skirmish_structure_count" }
    fn description(&self) -> &str { "Counts player's living structures with comparison" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "count".to_string(), "comparison".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 21. SkirmishUnitCountCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishUnitCountCondition;

#[async_trait]
impl ScriptCondition for SkirmishUnitCountCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let count = super::super::actions::get_int_param(parameters, "count")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_id = player.get_id() as u32;

        drop(player);

        let manager = get_object_manager();
        let mgr = manager.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read object manager: {}", e))
        })?;
        let owned = mgr.get_objects_owned_by_player(player_id);

        let mut unit_count: i64 = 0;
        for obj_id in &owned {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    // Count non-structure, living objects
                    if !obj.is_kind_of(KindOf::Structure)
                        && !obj.is_destroyed()
                        && !obj.is_effectively_dead()
                    {
                        unit_count += 1;
                    }
                }
            }
        }

        Ok(perform_comparison(unit_count, &comparison, count))
    }

    fn name(&self) -> &str { "skirmish_unit_count" }
    fn description(&self) -> &str { "Counts player's living units with comparison" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "count".to_string(), "comparison".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 22. SkirmishPlayerDefeatedCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishPlayerDefeatedCondition;

#[async_trait]
impl ScriptCondition for SkirmishPlayerDefeatedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(true), // Non-existent player is defeated
        };
        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        Ok(player.is_defeated())
    }

    fn name(&self) -> &str { "skirmish_player_defeated" }
    fn description(&self) -> &str { "Checks if player is defeated" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 23. SkirmishAlliedWithHumanCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishAlliedWithHumanCondition;

#[async_trait]
impl ScriptCondition for SkirmishAlliedWithHumanCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        let player_mask = player.get_player_mask();
        drop(player);

        // Iterate all players to find any human player that shares an alliance
        let list = player_list();
        let guard = list.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player list: {}", e))
        })?;

        for i in 0..guard.get_player_count() {
            if let Some(other_arc) = guard.get_player(i as i32) {
                // Skip same player
                if Arc::ptr_eq(&player_arc, &other_arc) {
                    continue;
                }
                if let Ok(other) = other_arc.read() {
                    if other.get_player_type() != PlayerType::Human {
                        continue;
                    }
                    // Simple alliance check: if their player masks overlap,
                    // they are on the same team. For full alliance checking we'd
                    // need the diplomacy system, but same team = allied in skirmish.
                    let other_mask = other.get_player_mask();
                    if player_mask.bits() & other_mask.bits() != 0 {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn name(&self) -> &str { "skirmish_allied_with_human" }
    fn description(&self) -> &str { "Checks if an AI player is allied with a human player" }
    fn required_parameters(&self) -> Vec<String> { vec!["player".to_string()] }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 24. SkirmishEnemyNearBaseCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishEnemyNearBaseCondition;

#[async_trait]
impl ScriptCondition for SkirmishEnemyNearBaseCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(player) => player,
            None => return Ok(false),
        };
        let radius = super::super::actions::get_float_param(parameters, "radius")? as f32;

        let player = player_arc.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read player: {}", e))
        })?;
        if has_hostile_object_near_owned_objects(&player, radius, |obj| {
            obj.is_kind_of(KindOf::Structure)
        }) {
            return Ok(true);
        }

        Ok(has_hostile_object_near_owned_objects(&player, radius, |_| true))
    }

    fn name(&self) -> &str { "skirmish_enemy_near_base" }
    fn description(&self) -> &str { "Checks if enemies are near player's base" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string(), "radius".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// 25. SkirmishTimeElapsedCondition
//-------------------------------------------------------------------------------------------------

pub struct SkirmishTimeElapsedCondition;

#[async_trait]
impl ScriptCondition for SkirmishTimeElapsedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let time_seconds = super::super::actions::get_int_param(parameters, "time")?;
        let comparison = get_str_param(parameters, "comparison")?;

        let frame = TheGameLogic::get_frame();
        let elapsed_seconds = (frame as i64) / (LOGICFRAMES_PER_SECOND as i64);

        Ok(perform_comparison(elapsed_seconds, &comparison, time_seconds))
    }

    fn name(&self) -> &str { "skirmish_time_elapsed" }
    fn description(&self) -> &str { "Checks if game time matches comparison (in seconds)" }
    fn required_parameters(&self) -> Vec<String> {
        vec!["time".to_string(), "comparison".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> { vec![] }
}

//-------------------------------------------------------------------------------------------------
// Registration
//-------------------------------------------------------------------------------------------------

pub fn register_skirmish_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(SkirmishSpecialPowerReadyCondition));
    registry.register_condition(Box::new(SkirmishSpecialPowerReadyFromNamedCondition));
    registry.register_condition(Box::new(SkirmishCommandButtonReadyCondition));
    registry.register_condition(Box::new(SkirmishEasyAiCondition));
    registry.register_condition(Box::new(SkirmishMediumAiCondition));
    registry.register_condition(Box::new(SkirmishHardAiCondition));
    registry.register_condition(Box::new(SkirmishPlayerIsAiCondition));
    registry.register_condition(Box::new(SkirmishHasEnoughMoneyCondition));
    registry.register_condition(Box::new(SkirmishNeedsSupplyCondition));
    registry.register_condition(Box::new(SkirmishBuildingsDestroyedCondition));
    registry.register_condition(Box::new(SkirmishUnitsDestroyedCondition));
    registry.register_condition(Box::new(SkirmishEnemyInAreaCondition));
    registry.register_condition(Box::new(SkirmishAllUnitsGarrisonedCondition));
    registry.register_condition(Box::new(SkirmishBaseUnderAttackCondition));
    registry.register_condition(Box::new(SkirmishSupplySourceAttackedCondition));
    registry.register_condition(Box::new(SkirmishCanBuildCondition));
    registry.register_condition(Box::new(SkirmishCanReinforceCondition));
    registry.register_condition(Box::new(SkirmishTeamNearPositionCondition));
    registry.register_condition(Box::new(SkirmishPlayerHasScienceCondition));
    registry.register_condition(Box::new(SkirmishPlayerHasUpgradeCondition));
    registry.register_condition(Box::new(SkirmishStructureCountCondition));
    registry.register_condition(Box::new(SkirmishUnitCountCondition));
    registry.register_condition(Box::new(SkirmishPlayerDefeatedCondition));
    registry.register_condition(Box::new(SkirmishAlliedWithHumanCondition));
    registry.register_condition(Box::new(SkirmishEnemyNearBaseCondition));
    registry.register_condition(Box::new(SkirmishTimeElapsedCondition));
}
