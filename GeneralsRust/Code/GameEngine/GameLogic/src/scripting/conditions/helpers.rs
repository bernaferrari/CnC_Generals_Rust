//! Shared helpers for script condition evaluation.

use super::ScriptValue;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{player_list, Player};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::events::GameEventType;
use crate::{GameLogicError, GameLogicResult};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Host-path script query snapshot (IDs/poses only — no crate `Object`).
#[derive(Debug, Clone, Default)]
pub struct HostScriptQuerySnapshot {
    pub named: HashMap<String, u32>,
    pub team_ids: HashMap<u32, Vec<u32>>,
    pub objects: Vec<HostScriptQueryObject>,
    /// Named trigger-area AABBs (min_x, min_z, max_x, max_z). Missing name →
    /// inside-area cannot be resolved (fail-closed).
    pub areas: HashMap<String, (f32, f32, f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct HostScriptQueryObject {
    pub id: u32,
    pub name: String,
    pub team: u32,
    pub x: f32,
    pub z: f32,
    pub alive: bool,
}

thread_local! {
    static HOST_SCRIPT_QUERY: RefCell<HostScriptQuerySnapshot> =
        RefCell::new(HostScriptQuerySnapshot::default());
}

/// Inject a read-only host name/team/area query map for crate conditions.
pub fn set_host_script_query_snapshot(snap: HostScriptQuerySnapshot) {
    HOST_SCRIPT_QUERY.with(|slot| *slot.borrow_mut() = snap);
}

pub fn clear_host_script_query_snapshot() {
    HOST_SCRIPT_QUERY.with(|slot| *slot.borrow_mut() = HostScriptQuerySnapshot::default());
}

pub fn host_script_named_unit_id(name: &str) -> Option<u32> {
    if name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().named.get(name).copied())
}

/// True when a host snapshot was injected (any named/team/object/area row).
pub fn host_script_query_has_any() -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        !snap.named.is_empty() || !snap.objects.is_empty() || !snap.team_ids.is_empty()
    })
}

/// Host named-unit aliveness from the snapshot (no crate Object).
pub fn host_script_named_unit_alive(name: &str) -> Option<bool> {
    if name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        snap.objects
            .iter()
            .find(|o| o.name == name)
            .map(|o| o.alive)
            .or_else(|| snap.named.contains_key(name).then_some(true))
    })
}

pub fn host_script_team_unit_ids(team: u32) -> Vec<u32> {
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .team_ids
            .get(&team)
            .cloned()
            .unwrap_or_default()
    })
}

pub fn host_script_area_unit_ids(min_x: f32, min_z: f32, max_x: f32, max_z: f32) -> Vec<u32> {
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|o| o.alive && o.x >= min_x && o.x <= max_x && o.z >= min_z && o.z <= max_z)
            .map(|o| o.id)
            .collect()
    })
}

pub fn host_script_named_unit_in_area(
    name: &str,
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
) -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        snap.objects.iter().any(|o| {
            o.alive
                && o.name == name
                && o.x >= min_x
                && o.x <= max_x
                && o.z >= min_z
                && o.z <= max_z
        })
    })
}

pub fn host_script_area_bounds(area_name: &str) -> Option<(f32, f32, f32, f32)> {
    if area_name.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().areas.get(area_name).copied())
}

/// `Some(true/false)` when `area_name` has host bounds; `None` if geometry is unknown.
pub fn host_script_named_unit_in_named_area(unit_name: &str, area_name: &str) -> Option<bool> {
    let (min_x, min_z, max_x, max_z) = host_script_area_bounds(area_name)?;
    Some(host_script_named_unit_in_area(
        unit_name, min_x, min_z, max_x, max_z,
    ))
}

/// Wave 271: host-only path has no dual-world factory objects.
#[inline]
pub(super) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

pub(super) fn normalize_event_name(name: &str) -> String {
    name.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase()
}

pub(super) fn event_type_from_name(name: &str) -> GameEventType {
    let normalized = normalize_event_name(name);
    match normalized.as_str() {
        "unitcreated" | "unit_created" => GameEventType::UnitCreated,
        "unitdestroyed" | "unit_destroyed" => GameEventType::UnitDestroyed,
        "unitdamaged" | "unit_damaged" => GameEventType::UnitDamaged,
        "unitmoved" | "unit_moved" => GameEventType::UnitMoved,
        "unitattacked" | "unit_attacked" => GameEventType::UnitAttacked,
        "weaponfired" | "weapon_fired" => GameEventType::WeaponFired,
        "combats_started" | "combatstarted" | "combat_started" => GameEventType::CombatStarted,
        "combatended" | "combat_ended" => GameEventType::CombatEnded,
        "playerdefeated" | "player_defeated" => GameEventType::PlayerDefeated,
        "playervictorious" | "player_victorious" => GameEventType::PlayerVictorious,
        "timerexpired" | "timer_expired" => GameEventType::TimerExpired,
        _ => GameEventType::Custom(name.to_string()),
    }
}

pub(super) fn compare_i64(actual: i64, comparison: &str, expected: i64) -> GameLogicResult<bool> {
    Ok(match comparison {
        "greater" => actual > expected,
        "less" => actual < expected,
        "equal" => actual == expected,
        "greater_equal" => actual >= expected,
        "less_equal" => actual <= expected,
        _ => {
            return Err(GameLogicError::Configuration(format!(
                "Invalid comparison operator: {}",
                comparison
            )))
        }
    })
}

pub(super) fn compare_f64(actual: f64, comparison: &str, expected: f64) -> GameLogicResult<bool> {
    Ok(match comparison {
        "greater" => actual > expected,
        "less" => actual < expected,
        "equal" => (actual - expected).abs() < 0.01,
        "greater_equal" => actual >= expected,
        "less_equal" => actual <= expected,
        _ => {
            return Err(GameLogicError::Configuration(format!(
                "Invalid comparison operator: {}",
                comparison
            )))
        }
    })
}

/// Helper: get string parameter from condition parameters
pub(crate) fn get_str_param(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> GameLogicResult<String> {
    match parameters.get(key) {
        Some(ScriptValue::String(s)) => Ok(s.clone()),
        Some(v) => Err(GameLogicError::Configuration(format!(
            "Expected string for '{}', got {:?}",
            key, v
        ))),
        None => Err(GameLogicError::Configuration(format!(
            "Missing parameter '{}'",
            key
        ))),
    }
}

/// Helper: get optional string parameter
pub(super) fn get_str_param_optional(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> Option<String> {
    match parameters.get(key) {
        Some(ScriptValue::String(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Helper: get player arc from parameter value
pub(crate) fn get_player_arc(
    parameters: &HashMap<String, ScriptValue>,
    key: &str,
) -> GameLogicResult<Option<Arc<RwLock<Player>>>> {
    let val = parameters
        .get(key)
        .ok_or_else(|| GameLogicError::Configuration(format!("Missing parameter '{}'", key)))?;
    match val {
        ScriptValue::PlayerId(id) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            Ok(guard.get_player(*id as i32).cloned())
        }
        ScriptValue::String(name) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            for i in 0..guard.get_player_count() {
                if let Some(player_arc) = guard.get_player(i as i32) {
                    if let Ok(player) = player_arc.read() {
                        if player.get_general_name() == name.as_str() {
                            return Ok(Some(player_arc.clone()));
                        }
                    }
                }
            }
            Ok(None)
        }
        ScriptValue::Int(id) => {
            let list = player_list();
            let guard = list.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to read player list: {}", e))
            })?;
            Ok(guard.get_player(*id as i32).cloned())
        }
        _ => Err(GameLogicError::Configuration(format!(
            "Expected player id/name for '{}', got {:?}",
            key, val
        ))),
    }
}

/// Helper: look up a named object from the script engine's named object tracker.
/// Returns the ObjectID if found.
pub(crate) fn lookup_named_object_id(name: &str) -> GameLogicResult<Option<u32>> {
    let tracker = get_named_object_tracker();
    tracker.get_object_id(name)
}

/// Helper: perform C++-style comparison (less_than, less_equal, equal, etc.)
pub(crate) fn perform_comparison(actual: i64, comparison: &str, expected: i64) -> bool {
    match comparison.to_lowercase().as_str() {
        "less_than" | "<" => actual < expected,
        "less_equal" | "<=" => actual <= expected,
        "equal" | "==" | "=" => actual == expected,
        "greater_equal" | ">=" => actual >= expected,
        "greater" | ">" => actual > expected,
        "not_equal" | "!=" => actual != expected,
        _ => false,
    }
}

pub(super) fn with_script_engine_mut<R>(
    f: impl FnOnce(&mut crate::scripting::engine::ScriptEngine) -> R,
) -> Option<R> {
    let engine = get_script_engine();
    let mut engine_guard = engine.write().ok()?;
    engine_guard.as_mut().map(f)
}

pub(super) fn parse_nested_condition(
    value: &ScriptValue,
) -> GameLogicResult<(String, HashMap<String, ScriptValue>)> {
    match value {
        ScriptValue::Object(map) => {
            let name_value = map
                .get("name")
                .or_else(|| map.get("condition"))
                .or_else(|| map.get("type"))
                .ok_or_else(|| {
                    GameLogicError::Configuration(
                        "Nested condition object missing 'name'".to_string(),
                    )
                })?;
            let ScriptValue::String(name) = name_value else {
                return Err(GameLogicError::Configuration(
                    "Nested condition 'name' must be a string".to_string(),
                ));
            };

            let params = match map.get("parameters") {
                Some(ScriptValue::Object(params)) => params.clone(),
                Some(_) => {
                    return Err(GameLogicError::Configuration(
                        "Nested condition 'parameters' must be an object".to_string(),
                    ))
                }
                None => HashMap::new(),
            };

            Ok((name.clone(), params))
        }
        _ => Err(GameLogicError::Configuration(
            "Nested condition must be an object".to_string(),
        )),
    }
}

//-------------------------------------------------------------------------------------------------
// Helper: parse object status mask from string name
//-------------------------------------------------------------------------------------------------
pub(super) fn parse_object_status_mask(status_str: &str) -> crate::common::ObjectStatusMaskType {
    use crate::common::ObjectStatusMaskType as OSM;
    match status_str.to_lowercase().as_str() {
        "destroyed" => OSM::DESTROYED,
        "can_attack" => OSM::CAN_ATTACK,
        "under_construction" => OSM::UNDER_CONSTRUCTION,
        "unselectable" => OSM::UNSELECTABLE,
        "no_collisions" => OSM::NO_COLLISIONS,
        "no_attack" => OSM::NO_ATTACK,
        "airborne_target" => OSM::AIRBORNE_TARGET,
        "parachuting" => OSM::PARACHUTING,
        "hijacked" => OSM::HIJACKED,
        "aflame" => OSM::AFLAME,
        "burned" => OSM::BURNED,
        "stealthed" | "cloaked" => OSM::STEALTHED,
        "detected" => OSM::DETECTED,
        "can_stealth" => OSM::CAN_STEALTH,
        "sold" => OSM::SOLD,
        "undergoing_repair" => OSM::UNDERGOING_REPAIR,
        "reconstructing" => OSM::RECONSTRUCTING,
        "masked" => OSM::MASKED,
        "is_attacking" => OSM::IS_ATTACKING,
        "is_using_ability" => OSM::IS_USING_ABILITY,
        "is_aiming_weapon" => OSM::IS_AIMING_WEAPON,
        "no_attack_from_ai" => OSM::NO_ATTACK_FROM_AI,
        "ignoring_stealth" => OSM::IGNORING_STEALTH,
        "is_car_bomb" => OSM::IS_CAR_BOMB,
        "is_firing_weapon" => OSM::IS_FIRING_WEAPON,
        "braking" => OSM::BRAKING,
        "wet" => OSM::WET,
        "repulsor" => OSM::REPULSOR,
        "rider1" => OSM::RIDER1,
        "rider2" => OSM::RIDER2,
        "rider3" => OSM::RIDER3,
        "rider4" => OSM::RIDER4,
        "rider5" => OSM::RIDER5,
        "rider6" => OSM::RIDER6,
        "rider7" => OSM::RIDER7,
        "rider8" => OSM::RIDER8,
        _ => {
            log::warn!("Unknown object status: {}", status_str);
            OSM::NONE
        }
    }
}
