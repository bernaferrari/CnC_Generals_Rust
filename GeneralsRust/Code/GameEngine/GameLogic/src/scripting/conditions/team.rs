//! Team script conditions.

use super::helpers::{
    compare_f64, compare_i64, dual_world_registry_unavailable, event_type_from_name,
    get_player_arc, get_str_param, get_str_param_optional, host_eval_team_has_object_status,
    lookup_named_object_id, parse_nested_condition, parse_object_status_mask, perform_comparison,
    with_script_engine_mut,
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

/// Team All Units Destroyed - All units in team are dead
pub(super) struct TeamAllUnitsDestroyedCondition;

#[async_trait]
impl ScriptCondition for TeamAllUnitsDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = crate::scripting::actions::get_string_param(parameters, "team_name")?;

        log::debug!("Checking if team '{}' is fully destroyed", team_name);

        // Query team status from TeamFactory
        // In C++: Check if all team members are dead/gone
        let team_factory = get_team_factory();
        if let Ok(mut factory) = team_factory.lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    // Team is destroyed if it has no members or all members are dead
                    if team.get_member_count() == 0 {
                        return Ok(true);
                    }

                    // Check if all members are destroyed
                    let members = team.get_members().to_vec();
                    drop(team); // Drop team guard before getting obj_manager

                    if let Ok(manager) = get_object_manager().read() {
                        for &member_id in &members {
                            if let Some(obj_arc) = manager.get_object(member_id) {
                                if let Ok(obj) = obj_arc.read() {
                                    if !obj.is_destroyed() {
                                        return Ok(false); // At least one member alive
                                    }
                                }
                            }
                        }
                        return Ok(true); // All members destroyed
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_all_units_destroyed"
    }

    fn description(&self) -> &str {
        "Checks if all units in a team have been destroyed"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Units In Formation - Team members are grouped
pub(super) struct UnitsInFormationCondition;

#[async_trait]
impl ScriptCondition for UnitsInFormationCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = crate::scripting::actions::get_string_param(parameters, "team_name")?;
        let max_distance =
            crate::scripting::actions::get_float_param_optional(parameters, "max_distance")
                .unwrap_or(50.0);

        log::debug!(
            "Checking if team '{}' is in formation (max distance: {})",
            team_name,
            max_distance
        );

        // Check team member positions, verify they're close together
        // In C++: Calculate spread of team positions
        let team_factory = get_team_factory();
        if let Ok(mut factory) = team_factory.lock() {
            if let Some(team_arc) = factory.find_team(&team_name) {
                if let Ok(team) = team_arc.read() {
                    let members = team.get_members();
                    if members.is_empty() {
                        return Ok(false);
                    }

                    // Calculate center of mass
                    let member_ids = members.to_vec();
                    drop(team); // Drop team guard before getting obj_manager

                    if let Ok(manager) = get_object_manager().read() {
                        let mut positions = Vec::new();
                        for &member_id in &member_ids {
                            if let Some(obj_arc) = manager.get_object(member_id) {
                                if let Ok(obj) = obj_arc.read() {
                                    let pos = obj.get_position();
                                    positions.push((pos.x, pos.y));
                                }
                            }
                        }

                        if positions.is_empty() {
                            return Ok(false);
                        }

                        // Calculate center
                        let center_x =
                            positions.iter().map(|(x, _)| x).sum::<f32>() / positions.len() as f32;
                        let center_y =
                            positions.iter().map(|(_, y)| y).sum::<f32>() / positions.len() as f32;

                        // Check if all units are within max_distance of center
                        for (px, py) in positions {
                            let dist = ((px - center_x).powi(2) + (py - center_y).powi(2)).sqrt();
                            if dist > max_distance as f32 {
                                return Ok(false);
                            }
                        }
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "units_in_formation"
    }

    fn description(&self) -> &str {
        "Checks if team units are grouped together in formation"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["max_distance".to_string()]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_DESTROYED - evaluateIsDestroyed
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamDestroyedCondition;

#[async_trait]
impl ScriptCondition for TeamDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        if dual_world_registry_unavailable() {
            if !super::helpers::host_team_was_fielded(&team_name) {
                return Ok(false);
            }
            return Ok(!super::helpers::host_team_has_any_live_objects(&team_name));
        }
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(!team.has_any_objects())
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_destroyed"
    }
    fn description(&self) -> &str {
        "Checks if a team has been destroyed (no objects remaining) (C++ TEAM_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_HAS_UNITS - evaluateHasUnits
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamHasUnitsCondition;

#[async_trait]
impl ScriptCondition for TeamHasUnitsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        if dual_world_registry_unavailable() {
            return Ok(super::helpers::host_team_has_any_live_units(&team_name));
        }
        let factory = get_team_factory();
        let guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        for team_arc in guard.find_team_instances(&team_name) {
            let team = team_arc
                .read()
                .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;
            if team.has_any_units() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_has_units"
    }
    fn description(&self) -> &str {
        "Checks if a team has any living units (C++ TEAM_HAS_UNITS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_STATE_IS - evaluateTeamStateIs
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamStateIsCondition;

#[async_trait]
impl ScriptCondition for TeamStateIsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let state_name = get_str_param(parameters, "state")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(team.get_state().str() == state_name)
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_state_is"
    }
    fn description(&self) -> &str {
        "Checks if team's state matches a specific state (C++ TEAM_STATE_IS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "state".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_STATE_IS_NOT - evaluateTeamStateIsNot
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamStateIsNotCondition;

#[async_trait]
impl ScriptCondition for TeamStateIsNotCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        TeamStateIsCondition
            .evaluate(parameters, context)
            .await
            .map(|b| !b)
    }

    fn name(&self) -> &str {
        "team_state_is_not"
    }
    fn description(&self) -> &str {
        "Checks if team's state does NOT match a specific state (C++ TEAM_STATE_IS_NOT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "state".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_OWNED_BY_PLAYER - evaluateTeamOwnedByPlayer
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamOwnedByPlayerCondition;

#[async_trait]
impl ScriptCondition for TeamOwnedByPlayerCondition {
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
        let player_id = player.get_id() as u32;
        drop(player);

        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(team.get_controlling_player_id() == Some(player_id))
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_owned_by_player"
    }
    fn description(&self) -> &str {
        "Checks if a team is owned by a specific player (C++ TEAM_OWNED_BY_PLAYER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_DISCOVERED - evaluateTeamDiscovered
// Returns true if any member of the team is visible to the specified player.
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamDiscoveredCondition;

#[async_trait]
impl ScriptCondition for TeamDiscoveredCondition {
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
        let player_index = player.get_player_index();
        drop(player);

        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        for &member_id in team.get_members() {
            let visible = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    if obj.is_disabled_by_type(crate::common::DisabledType::Held) {
                        return false;
                    }
                    let status = obj.get_status_bits();
                    if status.contains(crate::common::ObjectStatusMaskType::STEALTHED)
                        && !status.contains(crate::common::ObjectStatusMaskType::DETECTED)
                        && !status.contains(crate::common::ObjectStatusMaskType::DISGUISED)
                    {
                        return false;
                    }
                    matches!(
                        obj.get_shrouded_status(player_index),
                        crate::common::ObjectShroudStatus::Clear
                            | crate::common::ObjectShroudStatus::PartialClear
                    )
                })
                .unwrap_or(false);
            if visible {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_discovered"
    }
    fn description(&self) -> &str {
        "Checks if any team member is visible to a player (C++ TEAM_DISCOVERED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_CREATED - evaluateTeamCreated
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamCreatedCondition;

#[async_trait]
impl ScriptCondition for TeamCreatedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        if dual_world_registry_unavailable() {
            return Ok(super::helpers::host_team_was_fielded(&team_name));
        }
        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        match guard.find_team(&team_name) {
            Some(team_arc) => {
                let team = team_arc.read().map_err(|e| {
                    GameLogicError::Threading(format!("Failed to read team: {}", e))
                })?;
                Ok(team.is_created())
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "team_created"
    }
    fn description(&self) -> &str {
        "Checks if a team has been created (C++ TEAM_CREATED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_INSIDE_AREA_PARTIALLY - evaluateTeamInsideAreaPartially
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamInsideAreaPartiallyCondition;

#[async_trait]
impl ScriptCondition for TeamInsideAreaPartiallyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        if dual_world_registry_unavailable() {
            let Some(trigger) = super::helpers::host_script_lookup_polygon_trigger(&area_name)
            else {
                return Ok(false);
            };
            return Ok(
                super::helpers::host_team_some_inside_some_outside(&team_name, &trigger, 1)
                    || super::helpers::host_team_all_inside(&team_name, &trigger, 1),
            );
        }

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let members = team.get_members();
        let mut inside_count = 0u32;

        for &member_id in members {
            let counts = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    !obj.is_effectively_dead() && !obj.is_kind_of(KindOf::Inert)
                })
                .unwrap_or(false);
            if !counts {
                continue;
            }
            let area_tracker = get_area_tracker();
            let objects_in_area = area_tracker
                .get_objects_in_area(&area_name)
                .unwrap_or_default();
            if objects_in_area.contains(&member_id) {
                inside_count += 1;
            }
        }

        Ok(inside_count > 0)
    }

    fn name(&self) -> &str {
        "team_inside_area_partially"
    }
    fn description(&self) -> &str {
        "Checks if any team member is inside an area (C++ TEAM_INSIDE_AREA_PARTIALLY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_INSIDE_AREA_ENTIRELY - evaluateTeamInsideAreaEntirely
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamInsideAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamInsideAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        if dual_world_registry_unavailable() {
            let Some(trigger) = super::helpers::host_script_lookup_polygon_trigger(&area_name)
            else {
                return Ok(false);
            };
            return Ok(super::helpers::host_team_all_inside(
                &team_name, &trigger, 1,
            ));
        }

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let members = team.get_members();
        if members.is_empty() {
            return Ok(false);
        }

        let area_tracker = get_area_tracker();
        let mut considered = 0u32;
        for &member_id in members {
            let counts = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    !obj.is_effectively_dead() && !obj.is_kind_of(KindOf::Inert)
                })
                .unwrap_or(false);
            if !counts {
                continue;
            }
            let objects_in_area = area_tracker
                .get_objects_in_area(&area_name)
                .unwrap_or_default();
            if !objects_in_area.contains(&member_id) {
                return Ok(false);
            }
            considered += 1;
        }
        Ok(considered > 0)
    }

    fn name(&self) -> &str {
        "team_inside_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members are inside an area (C++ TEAM_INSIDE_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_OUTSIDE_AREA_ENTIRELY - evaluateTeamOutsideAreaEntirely
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamOutsideAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamOutsideAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Not entirely inside AND not partially inside = entirely outside
        let entirely_inside = TeamInsideAreaEntirelyCondition
            .evaluate(parameters, context)
            .await?;
        let partially_inside = TeamInsideAreaPartiallyCondition
            .evaluate(parameters, context)
            .await?;
        Ok(!entirely_inside && !partially_inside)
    }

    fn name(&self) -> &str {
        "team_outside_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members are outside an area (C++ TEAM_OUTSIDE_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_ALL_HAS_OBJECT_STATUS - evaluateTeamHasObjectStatus(entireTeam=true)
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamAllHasObjectStatusCondition;

#[async_trait]
impl ScriptCondition for TeamAllHasObjectStatusCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let status_str = get_str_param(parameters, "status")?;
        let status_mask = parse_object_status_mask(&status_str);
        if dual_world_registry_unavailable() {
            return Ok(
                host_eval_team_has_object_status(&team_name, status_mask.bits(), true)
                    .unwrap_or(false),
            );
        }

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        for &member_id in team.get_members() {
            let ok = OBJECT_REGISTRY
                .with_object(member_id, |obj| {
                    obj.get_status_bits().intersects(status_mask)
                })
                .unwrap_or(false);
            if !ok {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        "team_all_has_object_status"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members have a specific status (C++ TEAM_ALL_HAS_OBJECT_STATUS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "status".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_SOME_HAS_OBJECT_STATUS - evaluateTeamHasObjectStatus(entireTeam=false)
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamSomeHasObjectStatusCondition;

#[async_trait]
impl ScriptCondition for TeamSomeHasObjectStatusCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let status_str = get_str_param(parameters, "status")?;
        let status_mask = parse_object_status_mask(&status_str);
        if dual_world_registry_unavailable() {
            return Ok(
                host_eval_team_has_object_status(&team_name, status_mask.bits(), false)
                    .unwrap_or(false),
            );
        }

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        for &member_id in team.get_members() {
            match OBJECT_REGISTRY.with_object(member_id, |obj| {
                obj.get_status_bits().intersects(status_mask)
            }) {
                Some(true) => return Ok(true),
                Some(false) => {}
                None => return Ok(false),
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_some_has_object_status"
    }
    fn description(&self) -> &str {
        "Checks if ANY team member has a specific status (C++ TEAM_SOME_HAS_OBJECT_STATUS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "status".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_ENTERED_AREA_ENTIRELY - evaluateTeamEnteredAreaEntirely
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamEnteredAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamEnteredAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let members = team.get_members();
        if members.is_empty() {
            return Ok(false);
        }

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        for &member_id in members {
            if !objects_in_area.contains(&member_id) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn name(&self) -> &str {
        "team_entered_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members entered an area (C++ TEAM_ENTERED_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_ENTERED_AREA_PARTIALLY - evaluateTeamEnteredAreaPartially
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamEnteredAreaPartiallyCondition;

#[async_trait]
impl ScriptCondition for TeamEnteredAreaPartiallyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let team_name = match parameters.get("team") {
            Some(ScriptValue::Team(n)) => n.clone(),
            Some(ScriptValue::String(n)) => n.clone(),
            _ => {
                return Err(GameLogicError::Configuration(
                    "Missing 'team' parameter".to_string(),
                ));
            }
        };
        let area_name = get_str_param(parameters, "area_name")?;

        let factory = get_team_factory();
        let mut guard = factory.lock().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire team factory: {}", e))
        })?;
        let team_arc = match guard.find_team(&team_name) {
            Some(arc) => arc,
            None => return Ok(false),
        };
        let team = team_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read team: {}", e)))?;

        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        for &member_id in team.get_members() {
            if objects_in_area.contains(&member_id) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "team_entered_area_partially"
    }
    fn description(&self) -> &str {
        "Checks if any team member entered an area (C++ TEAM_ENTERED_AREA_PARTIALLY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_EXITED_AREA_ENTIRELY - evaluateTeamExitedAreaEntirely
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamExitedAreaEntirelyCondition;

#[async_trait]
impl ScriptCondition for TeamExitedAreaEntirelyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // All members outside = NOT (some inside OR all inside)
        let partially = TeamEnteredAreaPartiallyCondition
            .evaluate(parameters, _context)
            .await?;
        let entirely = TeamEnteredAreaEntirelyCondition
            .evaluate(parameters, _context)
            .await?;
        Ok(!partially && !entirely)
    }

    fn name(&self) -> &str {
        "team_exited_area_entirely"
    }
    fn description(&self) -> &str {
        "Checks if ALL team members exited an area (C++ TEAM_EXITED_AREA_ENTIRELY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TEAM_EXITED_AREA_PARTIALLY - evaluateTeamExitedAreaPartially
//-------------------------------------------------------------------------------------------------
pub(super) struct TeamExitedAreaPartiallyCondition;

#[async_trait]
impl ScriptCondition for TeamExitedAreaPartiallyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Some members outside = NOT all inside
        let entirely = TeamEnteredAreaEntirelyCondition
            .evaluate(parameters, _context)
            .await?;
        Ok(!entirely)
    }

    fn name(&self) -> &str {
        "team_exited_area_partially"
    }
    fn description(&self) -> &str {
        "Checks if some team members exited an area (C++ TEAM_EXITED_AREA_PARTIALLY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["team".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

pub(super) fn register_team_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(TeamAllUnitsDestroyedCondition));
    registry.register_condition(Box::new(UnitsInFormationCondition));
    registry.register_condition(Box::new(TeamDestroyedCondition));
    registry.register_condition(Box::new(TeamHasUnitsCondition));
    registry.register_condition(Box::new(TeamStateIsCondition));
    registry.register_condition(Box::new(TeamStateIsNotCondition));
    registry.register_condition(Box::new(TeamOwnedByPlayerCondition));
    registry.register_condition(Box::new(TeamDiscoveredCondition));
    registry.register_condition(Box::new(TeamCreatedCondition));
    registry.register_condition(Box::new(TeamInsideAreaPartiallyCondition));
    registry.register_condition(Box::new(TeamInsideAreaEntirelyCondition));
    registry.register_condition(Box::new(TeamOutsideAreaEntirelyCondition));
    registry.register_condition(Box::new(TeamAllHasObjectStatusCondition));
    registry.register_condition(Box::new(TeamSomeHasObjectStatusCondition));
    registry.register_condition(Box::new(TeamEnteredAreaEntirelyCondition));
    registry.register_condition(Box::new(TeamEnteredAreaPartiallyCondition));
    registry.register_condition(Box::new(TeamExitedAreaEntirelyCondition));
    registry.register_condition(Box::new(TeamExitedAreaPartiallyCondition));
}
