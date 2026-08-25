//! Named-object script conditions.

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

//-------------------------------------------------------------------------------------------------
// NAMED_NOT_DESTROYED - evaluateNamedUnitExists
// Returns true if named unit exists and is not effectively dead.
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedUnitExistsCondition;

#[async_trait]
impl ScriptCondition for NamedUnitExistsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        // Host path: name→id map / host query, no crate Object required.
        if dual_world_registry_unavailable() {
            return Ok(
                super::helpers::host_script_named_unit_id(&unit_name).is_some()
                    || lookup_named_object_id(&unit_name)?.is_some(),
            );
        }

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| !obj.is_effectively_dead())
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_unit_exists"
    }
    fn description(&self) -> &str {
        "Checks if named unit exists and is not dead (C++ NAMED_NOT_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_DESTROYED - evaluateNamedUnitDestroyed
// Returns true if named unit is effectively dead, or existed previously but no longer exists.
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedUnitDestroyedCondition;

#[async_trait]
impl ScriptCondition for NamedUnitDestroyedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Empty dual-world: host snapshot while the unit still exists; after
        // processDestroyList C++ getUnitNamed is NULL and didUnitExist stays true.
        if dual_world_registry_unavailable() {
            let unit_name = get_str_param(parameters, "unit_name")?;
            if let Some(obj) = super::helpers::host_script_query_object(&unit_name) {
                return Ok(obj.effectively_dead || !obj.alive);
            }
            if let Some(alive) = super::helpers::host_script_named_unit_alive(&unit_name) {
                return Ok(!alive);
            }
            let tracker = get_named_object_tracker();
            return tracker.did_object_exist(&unit_name);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => {
                // Object not in tracker — check if it previously existed
                let tracker = get_named_object_tracker();
                return tracker.did_object_exist(&unit_name);
            }
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| obj.is_effectively_dead())
            .unwrap_or(true)) // Was in tracker but gone from registry = destroyed
    }

    fn name(&self) -> &str {
        "named_unit_destroyed"
    }
    fn description(&self) -> &str {
        "Checks if named unit is destroyed (C++ NAMED_DESTROYED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_DYING - evaluateNamedUnitDying
// Returns true if named unit exists and is effectively dead (dying but not yet fully removed).
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedUnitDyingCondition;

#[async_trait]
impl ScriptCondition for NamedUnitDyingCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false), // Already totally dead, not just dying
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| obj.is_effectively_dead())
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_unit_dying"
    }
    fn description(&self) -> &str {
        "Checks if named unit is dying but not yet fully removed (C++ NAMED_DYING)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_TOTALLY_DEAD - evaluateNamedUnitTotallyDead
// Returns true if named unit previously existed but no longer exists in the object registry.
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedUnitTotallyDeadCondition;

#[async_trait]
impl ScriptCondition for NamedUnitTotallyDeadCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // C++ evaluateNamedUnitTotallyDead: getUnitNamed succeeds → false;
        // didUnitExist (name known, object gone) → true; never existed → false.
        if dual_world_registry_unavailable() {
            let unit_name = get_str_param(parameters, "unit_name")?;
            if super::helpers::host_script_named_unit_alive(&unit_name).is_some() {
                return Ok(false);
            }
            let tracker = get_named_object_tracker();
            return tracker.did_object_exist(&unit_name);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => {
                // Not in tracker — check history
                let tracker = get_named_object_tracker();
                return tracker.did_object_exist(&unit_name);
            }
        };
        // If still in tracker AND in registry, not totally dead
        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_some() {
            Ok(false)
        } else {
            let tracker = get_named_object_tracker();
            tracker.did_object_exist(&unit_name)
        }
    }

    fn name(&self) -> &str {
        "named_unit_totally_dead"
    }
    fn description(&self) -> &str {
        "Checks if named unit has been fully removed from the game (C++ NAMED_TOTALLY_DEAD)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_OWNED_BY_PLAYER - evaluateNamedOwnedByPlayer
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedOwnedByPlayerCondition;

#[async_trait]
impl ScriptCondition for NamedOwnedByPlayerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_id = player.get_id() as u32;
        drop(player);

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                obj.get_controlling_player_id() == Some(player_id)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_owned_by_player"
    }
    fn description(&self) -> &str {
        "Checks if named unit is owned by a specific player (C++ NAMED_OWNED_BY_PLAYER)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_INSIDE_AREA - evaluateNamedInsideArea
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedInsideAreaCondition;

#[async_trait]
impl ScriptCondition for NamedInsideAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let area_name = get_str_param(parameters, "area_name")?;
        if dual_world_registry_unavailable() {
            // Existence is not inside-area. Require mapped host AABB bounds.
            return Ok(super::helpers::host_script_named_unit_in_named_area(
                &unit_name, &area_name,
            )
            .unwrap_or(false));
        }

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        if OBJECT_REGISTRY.with_object(object_id, |_| ()).is_none() {
            return Ok(false);
        }

        // Check if object is in the area's tracked objects
        let area_tracker = get_area_tracker();
        let objects_in_area = area_tracker
            .get_objects_in_area(&area_name)
            .unwrap_or_default();
        Ok(objects_in_area.contains(&object_id))
    }

    fn name(&self) -> &str {
        "named_inside_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit is inside a trigger area (C++ NAMED_INSIDE_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_OUTSIDE_AREA - evaluateNamedOutsideArea
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedOutsideAreaCondition;

#[async_trait]
impl ScriptCondition for NamedOutsideAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Do not invert an unresolved inside-area (would fail-open).
        if dual_world_registry_unavailable() {
            let unit_name = get_str_param(parameters, "unit_name")?;
            let area_name = get_str_param(parameters, "area_name")?;
            return Ok(
                match super::helpers::host_script_named_unit_in_named_area(&unit_name, &area_name) {
                    Some(inside) => !inside,
                    None => false,
                },
            );
        }
        NamedInsideAreaCondition
            .evaluate(parameters, context)
            .await
            .map(|b| !b)
    }

    fn name(&self) -> &str {
        "named_outside_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit is outside a trigger area (C++ NAMED_OUTSIDE_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_DISCOVERED - evaluateNamedDiscovered
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedDiscoveredCondition;

#[async_trait]
impl ScriptCondition for NamedDiscoveredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        let player_arc = match get_player_arc(parameters, "player")? {
            Some(p) => p,
            None => return Ok(false),
        };
        let player = player_arc
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read player: {}", e)))?;
        let player_index = player.get_player_index();
        drop(player);

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
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
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_discovered"
    }
    fn description(&self) -> &str {
        "Checks if named unit has been discovered by a player (C++ NAMED_DISCOVERED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "player".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_BUILDING_IS_EMPTY - evaluateIsBuildingEmpty
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedBuildingIsEmptyCondition;

#[async_trait]
impl ScriptCondition for NamedBuildingIsEmptyCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let building_name = get_str_param(parameters, "building_name")?;

        let object_id = match lookup_named_object_id(&building_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                let Some(contain) = obj.get_contain() else {
                    return false;
                };
                contain
                    .lock()
                    .ok()
                    .map(|contain_guard| contain_guard.get_contain_count() == 0)
                    .unwrap_or(false)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_building_is_empty"
    }
    fn description(&self) -> &str {
        "Checks if named building has no units inside (C++ NAMED_BUILDING_IS_EMPTY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["building_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_HAS_FREE_CONTAINER_SLOTS - evaluateNamedHasFreeContainerSlots
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedHasFreeContainerSlotsCondition;

#[async_trait]
impl ScriptCondition for NamedHasFreeContainerSlotsCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                let Some(contain) = obj.get_contain() else {
                    return false;
                };
                contain
                    .lock()
                    .ok()
                    .map(|contain_guard| {
                        let max = contain_guard.get_contain_max() as u32;
                        let cur = contain_guard.get_contain_count();
                        cur < max
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_has_free_container_slots"
    }
    fn description(&self) -> &str {
        "Checks if named unit has free container/garrison slots (C++ NAMED_HAS_FREE_CONTAINER_SLOTS)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_CREATED - evaluateNamedCreated
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedCreatedCondition;

#[async_trait]
impl ScriptCondition for NamedCreatedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        match lookup_named_object_id(&unit_name)? {
            Some(id) => {
                // Also verify the object actually exists in the registry
                Ok(OBJECT_REGISTRY.with_object(id, |_| ()).is_some())
            }
            None => Ok(false),
        }
    }

    fn name(&self) -> &str {
        "named_created"
    }
    fn description(&self) -> &str {
        "Checks if named unit has been created (C++ NAMED_CREATED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_SELECTED - evaluateNamedSelected
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedSelectedCondition;

#[async_trait]
impl ScriptCondition for NamedSelectedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        if dual_world_registry_unavailable() {
            return Ok(super::helpers::host_script_named_unit_selected(&unit_name).unwrap_or(false));
        }
        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };

        // Check if object is in the current selection via selection manager
        let sel_mgr = crate::commands::selection::get_selection_manager();
        let mgr = sel_mgr.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to read selection manager: {}", e))
        })?;
        Ok(mgr.is_object_selected_by_any_player(object_id))
    }

    fn name(&self) -> &str {
        "named_selected"
    }
    fn description(&self) -> &str {
        "Checks if named unit is currently selected (C++ NAMED_SELECTED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_REACHED_WAYPOINTS_END - evaluateNamedReachedWaypointsEnd
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedReachedWaypointsEndCondition;

#[async_trait]
impl ScriptCondition for NamedReachedWaypointsEndCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        // Wave 271: empty dual-world → fail-closed condition.
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let unit_name = get_str_param(parameters, "unit_name")?;
        let _waypoint_path = get_str_param(parameters, "waypoint_path")?;

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                let Some(ai) = obj.get_ai_update_interface() else {
                    return false;
                };
                ai.try_lock()
                    .ok()
                    .map(|ai_guard| ai_guard.is_idle())
                    .unwrap_or(false)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_reached_waypoints_end"
    }
    fn description(&self) -> &str {
        "Checks if named unit has reached the end of its waypoint path (C++ NAMED_REACHED_WAYPOINTS_END)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "waypoint_path".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// C++ Parity Conditions - Session 8 additions
// Area entry/exit, special power, science, power, multiplayer, audio/video, misc
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
// NAMED_ENTERED_AREA - evaluateNamedEnteredArea
// Returns true if named unit has entered a trigger area.
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedEnteredAreaCondition;

#[async_trait]
impl ScriptCondition for NamedEnteredAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let area_name = get_str_param(parameters, "area_name")?;
        let Some(trigger) = crate::scripting::host_script_lookup_polygon_trigger(&area_name)
            .or_else(|| {
                get_script_engine().read().ok().and_then(|guard| {
                    guard
                        .as_ref()
                        .and_then(|engine| engine.get_qualified_trigger_area_by_name(&area_name))
                })
            })
        else {
            return Ok(false);
        };
        if dual_world_registry_unavailable() {
            let Some(object_id) = super::helpers::host_script_named_unit_id(&unit_name) else {
                return Ok(false);
            };
            return Ok(super::helpers::host_object_did_enter(object_id, &trigger));
        }

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => {
                return Ok(false);
            }
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                if obj.is_effectively_dead() || obj.is_kind_of(KindOf::Inert) {
                    return false;
                }
                obj.did_enter(&trigger)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_entered_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit entered a trigger area (C++ NAMED_ENTERED_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// NAMED_EXITED_AREA - evaluateNamedExitedArea
// Returns true if named unit has exited a trigger area (was inside, now outside).
//-------------------------------------------------------------------------------------------------
pub(super) struct NamedExitedAreaCondition;

#[async_trait]
impl ScriptCondition for NamedExitedAreaCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let unit_name = get_str_param(parameters, "unit_name")?;
        let area_name = get_str_param(parameters, "area_name")?;
        let Some(trigger) = crate::scripting::host_script_lookup_polygon_trigger(&area_name)
            .or_else(|| {
                get_script_engine().read().ok().and_then(|guard| {
                    guard
                        .as_ref()
                        .and_then(|engine| engine.get_qualified_trigger_area_by_name(&area_name))
                })
            })
        else {
            return Ok(false);
        };
        if dual_world_registry_unavailable() {
            let Some(object_id) = super::helpers::host_script_named_unit_id(&unit_name) else {
                return Ok(false);
            };
            return Ok(super::helpers::host_object_did_exit(object_id, &trigger));
        }

        let object_id = match lookup_named_object_id(&unit_name)? {
            Some(id) => id,
            None => {
                return Ok(false);
            }
        };
        Ok(OBJECT_REGISTRY
            .with_object(object_id, |obj| {
                if obj.is_effectively_dead() {
                    return false;
                }
                obj.did_exit(&trigger)
            })
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "named_exited_area"
    }
    fn description(&self) -> &str {
        "Checks if named unit exited a trigger area (C++ NAMED_EXITED_AREA)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string(), "area_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

pub(super) fn register_named_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(NamedUnitExistsCondition));
    registry.register_condition(Box::new(NamedUnitDestroyedCondition));
    registry.register_condition(Box::new(NamedUnitDyingCondition));
    registry.register_condition(Box::new(NamedUnitTotallyDeadCondition));
    registry.register_condition(Box::new(NamedOwnedByPlayerCondition));
    registry.register_condition(Box::new(NamedInsideAreaCondition));
    registry.register_condition(Box::new(NamedOutsideAreaCondition));
    registry.register_condition(Box::new(NamedDiscoveredCondition));
    registry.register_condition(Box::new(NamedBuildingIsEmptyCondition));
    registry.register_condition(Box::new(NamedHasFreeContainerSlotsCondition));
    registry.register_condition(Box::new(NamedCreatedCondition));
    registry.register_condition(Box::new(NamedSelectedCondition));
    registry.register_condition(Box::new(NamedReachedWaypointsEndCondition));
    registry.register_condition(Box::new(NamedEnteredAreaCondition));
    registry.register_condition(Box::new(NamedExitedAreaCondition));
}
