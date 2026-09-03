//! Shared helpers for script actions.
//!
//! C++: no direct analog (`getParameter` lives on C++ `ScriptAction`).
//!
//! Split from `scripting/actions.rs` for module-size parity.
//! Observable script behavior is unchanged.

use crate::ai::{AiGroup, the_ai};
use crate::common::{Coord3D, Relationship};
use crate::helpers::TheGameLogic;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;
use crate::scripting::ScriptValue;
use crate::scripting::core::{LOCAL_PLAYER, TEAM_THE_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_TEAM};
use crate::scripting::engine::get_script_engine;
use crate::team::get_team_factory;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{RadarEventType, get_radar_system};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Wave 295: host-only path has no dual-world factory objects.
#[inline]
pub(super) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

pub(crate) fn is_money_resource(resource_type: &str) -> bool {
    matches!(
        resource_type.trim().to_ascii_lowercase().as_str(),
        "money" | "cash" | "resource" | "resources" | "supply" | "supplies"
    )
}

pub(super) fn clamp_script_money(amount: i64) -> i32 {
    amount.clamp(0, i32::MAX as i64) as i32
}

pub(super) fn set_script_player_money(player: &mut crate::player::Player, new_amount: i32) {
    let current = player.get_money().get_money();
    player.get_money_mut().set_money(new_amount);
    record_script_money_delta(player, new_amount as i64 - current as i64);
}

pub(super) fn grant_script_player_money(player: &mut crate::player::Player, amount: i32) {
    if amount <= 0 {
        return;
    }

    let current = player.get_money().get_money();
    player
        .get_money_mut()
        .set_money(current.saturating_add(amount));
    record_script_money_delta(player, amount as i64);
}

pub(super) fn spend_script_player_money(player: &mut crate::player::Player, amount: i32) {
    if amount <= 0 {
        return;
    }

    let current = player.get_money().get_money();
    let withdrawn = amount.min(current.max(0));
    if withdrawn > 0 {
        player.get_money_mut().set_money(current - withdrawn);
        record_script_money_delta(player, -(withdrawn as i64));
    }
}

pub(super) fn record_script_money_delta(player: &mut crate::player::Player, delta: i64) {
    if delta > 0 {
        let amount = delta.min(u32::MAX as i64) as u32;
        player.get_score_keeper_mut().add_money_earned(amount);
        player.get_academy_stats_mut().record_income(delta as i32);
    } else if delta < 0 {
        player
            .get_score_keeper_mut()
            .add_money_spent(delta.saturating_neg().min(u32::MAX as i64) as u32);
    }
}

pub(super) fn with_script_engine_mut<F>(f: F) -> GameLogicResult<()>
where
    F: FnOnce(&mut crate::scripting::engine::ScriptEngine) -> GameLogicResult<()>,
{
    let engine_lock = get_script_engine();
    let mut engine_guard = engine_lock
        .write()
        .map_err(|_| GameLogicError::Threading("Failed to lock ScriptEngine".to_string()))?;
    let Some(engine) = engine_guard.as_mut() else {
        return Ok(());
    };
    f(engine)
}

pub(super) fn dispatch_named_timer(name: &str, text: &str, countdown: bool) {
    if let Ok(engine_guard) = get_script_engine().read() {
        if let Some(ref script_engine) = *engine_guard {
            if let Some(handler) = script_engine.action_handler() {
                if let Err(err) = handler.add_named_timer(name, text, countdown) {
                    log::warn!("Script action handler add_named_timer failed: {}", err);
                }
            }
        }
    }
}

pub(super) fn parse_script_relationship(value: &str) -> GameLogicResult<Relationship> {
    match value.trim().to_ascii_uppercase().as_str() {
        "ALLY" | "ALLIES" => Ok(Relationship::Allies),
        "ENEMY" | "ENEMIES" => Ok(Relationship::Enemies),
        "NEUTRAL" => Ok(Relationship::Neutral),
        _ => Err(GameLogicError::Configuration(format!(
            "Unknown relationship '{}'",
            value
        ))),
    }
}

// Helper functions for parameter extraction

pub fn get_string_param(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> GameLogicResult<String> {
    match parameters.get(name) {
        Some(ScriptValue::String(s)) => Ok(s.clone()),
        Some(other) => Err(GameLogicError::Configuration(format!(
            "Parameter '{}' must be a string, got: {}",
            name, other
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Required parameter '{}' not found",
            name
        ))),
    }
}

pub fn get_int_param(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> GameLogicResult<i64> {
    match parameters.get(name) {
        Some(ScriptValue::Int(i)) => Ok(*i),
        Some(ScriptValue::Float(f)) => Ok(*f as i64),
        Some(other) => Err(GameLogicError::Configuration(format!(
            "Parameter '{}' must be an integer, got: {}",
            name, other
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Required parameter '{}' not found",
            name
        ))),
    }
}

pub fn get_int_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> Option<i64> {
    match parameters.get(name) {
        Some(ScriptValue::Int(i)) => Some(*i),
        Some(ScriptValue::Float(f)) => Some(*f as i64),
        _ => None,
    }
}

pub fn get_float_param(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> GameLogicResult<f64> {
    match parameters.get(name) {
        Some(ScriptValue::Float(f)) => Ok(*f),
        Some(ScriptValue::Int(i)) => Ok(*i as f64),
        Some(other) => Err(GameLogicError::Configuration(format!(
            "Parameter '{}' must be a number, got: {}",
            name, other
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Required parameter '{}' not found",
            name
        ))),
    }
}

pub(super) fn get_bool_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> Option<bool> {
    match parameters.get(name) {
        Some(ScriptValue::Bool(value)) => Some(*value),
        Some(ScriptValue::Int(value)) => Some(*value != 0),
        _ => None,
    }
}

pub(super) fn get_coord_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> Option<Coord3D> {
    match parameters.get(name) {
        Some(ScriptValue::Coord3D([x, y, z])) => Some(Coord3D::new(*x, *y, *z)),
        _ => None,
    }
}

pub(super) fn radar_event_type_from_int(event_type: i32) -> RadarEventType {
    match event_type {
        1 => RadarEventType::Construction,
        2 => RadarEventType::Upgrade,
        3 => RadarEventType::UnderAttack,
        4 => RadarEventType::Information,
        5 => RadarEventType::BeaconPulse,
        6 => RadarEventType::Infiltration,
        7 => RadarEventType::BattlePlan,
        8 => RadarEventType::StealthDiscovered,
        9 => RadarEventType::StealthNeutralized,
        10 => RadarEventType::Fake,
        _ => RadarEventType::Invalid,
    }
}

pub(super) fn create_radar_event_for_position(
    position: Coord3D,
    event_type: i32,
) -> GameLogicResult<()> {
    if let Ok(mut radar) = get_radar_system().write() {
        let radar_pos =
            game_engine::common::system::radar::Coord3D::new(position.x, position.y, position.z);
        radar.create_event(&radar_pos, radar_event_type_from_int(event_type), 4.0);
    }

    if let Ok(engine_guard) = get_script_engine().read() {
        if let Some(ref script_engine) = *engine_guard {
            if let Some(handler) = script_engine.action_handler() {
                handler.create_radar_event(position.x, position.y, position.z, event_type)?;
            }
        }
    }

    Ok(())
}

pub(super) fn resolve_player_name_token(raw: &str) -> String {
    match raw {
        THE_PLAYER => {
            if !crate::scripting::core::is_generals_challenge_campaign() {
                raw.to_string()
            } else {
                player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_local_player().cloned())
                    .and_then(|p| {
                        p.read()
                            .ok()
                            .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
                    })
                    .unwrap_or_else(|| raw.to_string())
            }
        }
        THIS_PLAYER => get_script_engine()
            .read()
            .ok()
            .and_then(|g| {
                g.as_ref()
                    .and_then(|e| e.get_current_player_name().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| raw.to_string()),
        LOCAL_PLAYER => player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| {
                p.read()
                    .ok()
                    .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
            })
            .unwrap_or_else(|| raw.to_string()),
        _ => raw.to_string(),
    }
}

pub(super) fn resolve_named_object_id(name: &str) -> Option<u32> {
    // Wave 295: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

    let tracker = crate::scripting::engine::get_named_object_tracker();
    let mut object_id = tracker.get_object_id(name).ok().flatten();

    if object_id.is_none() {
        let lower = name.to_ascii_lowercase();
        object_id = OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .find_map(|obj_ref| {
                obj_ref.read().ok().and_then(|obj| {
                    if obj.get_name().to_ascii_lowercase() == lower {
                        Some(obj.get_id())
                    } else {
                        None
                    }
                })
            });
    }

    object_id
}

pub(super) fn resolve_team_name_token(raw: &str) -> String {
    match raw {
        THIS_TEAM => get_script_engine()
            .read()
            .ok()
            .and_then(|g| {
                g.as_ref().and_then(|e| {
                    e.get_condition_team_name()
                        .or_else(|| e.get_calling_team_name())
                        .map(|s| s.to_string())
                })
            })
            .unwrap_or_else(|| raw.to_string()),
        TEAM_THE_PLAYER => {
            if !crate::scripting::core::is_generals_challenge_campaign() {
                return raw.to_string();
            }
            player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
                .and_then(|p| p.read().ok().and_then(|p| p.get_default_team()))
                .and_then(|team| team.read().ok().map(|t| t.get_name().to_string()))
                .unwrap_or_else(|| raw.to_string())
        }
        _ => raw.to_string(),
    }
}

pub(super) fn create_ai_group_from_team(team_name: &str) -> GameLogicResult<Arc<RwLock<AiGroup>>> {
    let resolved_team = resolve_team_name_token(team_name);
    let factory = get_team_factory();
    let team_arc = factory
        .lock()
        .map_err(|_| GameLogicError::Threading("Failed to lock TeamFactory".to_string()))?
        .find_team(&resolved_team)
        .ok_or_else(|| {
            GameLogicError::Configuration(format!("Team '{}' not found", resolved_team))
        })?;

    let members = team_arc
        .read()
        .map_err(|_| GameLogicError::Threading("Failed to read Team".to_string()))?
        .get_members()
        .to_vec();

    let ai_store = the_ai();let mut ai_guard = ai_store
        .write()
        .map_err(|_| GameLogicError::Threading("Failed to lock AI system".to_string()))?;
    let group = ai_guard.create_group();

    if let Ok(mut group_guard) = group.write() {
        for member_id in members {
            if let Some(_obj_arc) = TheGameLogic::find_object_by_id(member_id) {
                group_guard.add(member_id);
            }
        }
    }

    Ok(group)
}

pub fn get_float_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    name: &str,
) -> Option<f64> {
    match parameters.get(name) {
        Some(ScriptValue::Float(f)) => Some(*f),
        Some(ScriptValue::Int(i)) => Some(*i as f64),
        _ => None,
    }
}
