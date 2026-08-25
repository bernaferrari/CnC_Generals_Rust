//! Multiplayer script conditions.

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
// MULTIPLAYER_ALLIED_VICTORY
//-------------------------------------------------------------------------------------------------
pub(super) struct MultiplayerAlliedVictoryCondition;

#[async_trait]
impl ScriptCondition for MultiplayerAlliedVictoryCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        Ok(TheVictoryConditions::is_local_allied_victory())
    }

    fn name(&self) -> &str {
        "multiplayer_allied_victory"
    }
    fn description(&self) -> &str {
        "Checks if allies have won (C++ MULTIPLAYER_ALLIED_VICTORY)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MULTIPLAYER_ALLIED_DEFEAT
//-------------------------------------------------------------------------------------------------
pub(super) struct MultiplayerAlliedDefeatCondition;

#[async_trait]
impl ScriptCondition for MultiplayerAlliedDefeatCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        Ok(TheVictoryConditions::is_local_allied_defeat())
    }

    fn name(&self) -> &str {
        "multiplayer_allied_defeat"
    }
    fn description(&self) -> &str {
        "Checks if allies have lost (C++ MULTIPLAYER_ALLIED_DEFEAT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MULTIPLAYER_PLAYER_DEFEAT
//-------------------------------------------------------------------------------------------------
pub(super) struct MultiplayerPlayerDefeatCondition;

#[async_trait]
impl ScriptCondition for MultiplayerPlayerDefeatCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let Ok(players) = player_list().read() else {
            return Ok(false);
        };
        let Some(local_player_arc) = players.get_local_player().cloned() else {
            return Ok(false);
        };
        let Ok(local_player) = local_player_arc.read() else {
            return Ok(false);
        };
        let local_defeat = local_player.is_defeated() || local_player.is_player_dead();
        if !local_defeat || local_player.is_player_observer() {
            return Ok(false);
        }
        // C++ isLocalDefeat() && !isLocalAlliedDefeat(): true only if an ally still lives.
        let local_index = local_player.get_player_index();
        drop(local_player);
        let Some(local_again) = players.get_local_player().and_then(|p| p.read().ok()) else {
            return Ok(false);
        };
        for player_arc in players.iter() {
            let Ok(other) = player_arc.read() else {
                continue;
            };
            if other.get_player_index() == local_index {
                continue;
            }
            if local_again.is_allied_with_player(&other)
                && !other.is_defeated()
                && !other.is_player_dead()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "multiplayer_player_defeat"
    }
    fn description(&self) -> &str {
        "Checks if local player is defeated (C++ MULTIPLAYER_PLAYER_DEFEAT)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

pub(super) fn register_multiplayer_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(MultiplayerAlliedVictoryCondition));
    registry.register_condition(Box::new(MultiplayerAlliedDefeatCondition));
    registry.register_condition(Box::new(MultiplayerPlayerDefeatCondition));
}
