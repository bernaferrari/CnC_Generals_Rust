//! Shared helpers for script condition evaluation.

use super::ScriptValue;
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::{player_list, Player};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::events::GameEventType;
use crate::{GameLogicError, GameLogicResult};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Host-path script query snapshot (IDs/poses only — no crate `Object`).
#[derive(Debug, Clone, Default)]
pub struct HostScriptQuerySnapshot {
    pub named: HashMap<String, u32>,
    pub team_ids: HashMap<u32, Vec<u32>>,
    pub objects: Vec<HostScriptQueryObject>,
    /// Named trigger-area AABBs (min_x, min_z, max_x, max_z). Circular/script
    /// pads only — map polygons are tested with `point_in_trigger_int`.
    pub areas: HashMap<String, (f32, f32, f32, f32)>,
    /// Script team-instance name → host object ids (C++ Team member list).
    pub team_instance_ids: HashMap<String, Vec<u32>>,
    /// Live AIPlayer::isSupplySourceAttacked keyed by player name.
    pub supply_source_attacked: HashMap<String, bool>,
    /// Cash at the preferred warehouse (or -1 if none).
    pub supply_center_cash: HashMap<String, i32>,
    /// isLocationSafe of that warehouse.
    pub supply_center_location_safe: HashMap<String, bool>,
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

/// Merge additional host-query rows into the current snapshot.
pub fn merge_host_script_query_snapshot(f: impl FnOnce(&mut HostScriptQuerySnapshot)) {
    HOST_SCRIPT_QUERY.with(|slot| f(&mut *slot.borrow_mut()));
}

/// Inject a read-only host name/team/area query map for crate conditions.
pub fn set_host_script_query_snapshot(snap: HostScriptQuerySnapshot) {
    HOST_SCRIPT_QUERY.with(|slot| *slot.borrow_mut() = snap);
}

pub fn clear_host_script_query_snapshot() {
    HOST_SCRIPT_QUERY.with(|slot| *slot.borrow_mut() = HostScriptQuerySnapshot::default());
    clear_host_trigger_flags();
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

fn host_player_query_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// Live `AIPlayer::isSupplySourceAttacked` for leftover conditions.
pub fn host_query_supply_source_attacked(player_name: &str) -> Option<bool> {
    let key = host_player_query_key(player_name);
    if key.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| slot.borrow().supply_source_attacked.get(&key).copied())
}

/// Live `AIPlayer::isSupplySourceSafe(min)` for leftover conditions.
pub fn host_query_supply_source_safe(player_name: &str, min_supplies: i32) -> Option<bool> {
    let key = host_player_query_key(player_name);
    if key.is_empty() {
        return None;
    }
    HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        let cash = *snap.supply_center_cash.get(&key)?;
        if cash < min_supplies {
            return Some(true);
        }
        Some(
            snap.supply_center_location_safe
                .get(&key)
                .copied()
                .unwrap_or(true),
        )
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

/// Host XZ → C++ trigger XY (`Object.cpp:2572-2574`, host Y-up).
pub fn host_xz_to_trigger_point(x: f32, z: f32) -> crate::common::ICoord3D {
    crate::common::ICoord3D::new(x as i32, z as i32, 0)
}

/// Leftover `TerrainLogic` polygon by qualified name.
pub fn host_script_lookup_polygon_trigger(
    area_name: &str,
) -> Option<crate::polygon_trigger::PolygonTrigger> {
    if area_name.is_empty() {
        return None;
    }
    let resolved = crate::scripting::engine::qualify_trigger_area_name(area_name, None)?;
    crate::terrain::get_terrain_logic()
        .read()
        .ok()?
        .get_trigger_area_by_name(&resolved)
        .cloned()
}

fn host_named_unit_point_in_trigger(
    unit_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
) -> bool {
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow().objects.iter().any(|o| {
            o.name == unit_name
                && trigger.point_in_trigger_int(&host_xz_to_trigger_point(o.x, o.z))
        })
    })
}

/// `Some(true/false)` when leftover polygon or host AABB geometry exists.
/// C++ `evaluateNamedInsideArea` uses `pointInTrigger` on current position.
pub fn host_script_named_unit_in_named_area(unit_name: &str, area_name: &str) -> Option<bool> {
    if let Some(trigger) = host_script_lookup_polygon_trigger(area_name) {
        return Some(host_named_unit_point_in_trigger(unit_name, &trigger));
    }
    let (min_x, min_z, max_x, max_z) = host_script_area_bounds(area_name)?;
    Some(host_script_named_unit_in_area(
        unit_name, min_x, min_z, max_x, max_z,
    ))
}

const HOST_MAX_TRIGGER_INFOS: usize = 5;

#[derive(Clone)]
struct HostTriggerSlot {
    trigger_id: i32,
    is_inside: bool,
    entered: bool,
    exited: bool,
}

struct HostObjectTriggerState {
    i_x: i32,
    i_y: i32,
    entered_or_exited_frame: u32,
    slots: Vec<HostTriggerSlot>,
}

#[derive(Default)]
struct HostTriggerWorld {
    objects: HashMap<u32, HostObjectTriggerState>,
    team_entered_or_exited: HashMap<String, u32>,
}

thread_local! {
    static HOST_TRIGGER_WORLD: RefCell<HostTriggerWorld> =
        RefCell::new(HostTriggerWorld::default());
}

fn leftover_polygon_triggers() -> Vec<crate::polygon_trigger::PolygonTrigger> {
    crate::terrain::get_terrain_logic()
        .read()
        .ok()
        .map(|terrain| terrain.get_trigger_areas().get_triggers().to_vec())
        .unwrap_or_default()
}

fn host_flag_window(flag_frame: u32, now: u32) -> bool {
    flag_frame == now || (now > 0 && flag_frame == now - 1)
}

fn current_logic_frame() -> u32 {
    crate::system::game_logic::current_frame()
}

/// C++ `Object::setTriggerAreaFlagsForChangeInPosition` for host units.
pub fn update_host_object_trigger_flags(
    object_id: u32,
    x: f32,
    z: f32,
    frame: u32,
    skip: bool,
    team_name: Option<&str>,
) {
    if skip {
        return;
    }
    let new_x = x as i32;
    let new_y = z as i32;
    let triggers = leftover_polygon_triggers();
    HOST_TRIGGER_WORLD.with(|world| {
        let mut world = world.borrow_mut();
        let state = world
            .objects
            .entry(object_id)
            .or_insert_with(|| HostObjectTriggerState {
                i_x: 0,
                i_y: 0,
                entered_or_exited_frame: 0,
                slots: Vec::new(),
            });
        let pos_changed = state.i_x != new_x || state.i_y != new_y;
        // C++ Object.cpp:2575-2578: unchanged integer XY returns even with
        // zero active areas. Required so load can restore m_iPos and skip
        // a fresh ENTERED_AREA edge.
        if !pos_changed {
            return;
        }
        if state.entered_or_exited_frame != 0 && state.entered_or_exited_frame != frame {
            state.slots.retain(|slot| slot.is_inside);
            for slot in &mut state.slots {
                slot.entered = false;
                slot.exited = false;
            }
        }
        if pos_changed {
            let old = crate::common::ICoord3D::new(state.i_x, state.i_y, 0);
            for slot in &mut state.slots {
                let Some(trigger) = triggers.iter().find(|t| t.get_id() == slot.trigger_id) else {
                    continue;
                };
                if !trigger.point_in_trigger_int(&old) {
                    slot.is_inside = false;
                    slot.exited = true;
                    state.entered_or_exited_frame = frame;
                }
            }
            state.i_x = new_x;
            state.i_y = new_y;
        }
        let now_pt = crate::common::ICoord3D::new(state.i_x, state.i_y, 0);
        for trigger in &triggers {
            if state
                .slots
                .iter()
                .any(|slot| slot.trigger_id == trigger.get_id())
            {
                continue;
            }
            if !trigger.point_in_trigger_int(&now_pt) {
                continue;
            }
            if state.slots.len() >= HOST_MAX_TRIGGER_INFOS {
                break;
            }
            state.slots.push(HostTriggerSlot {
                trigger_id: trigger.get_id(),
                is_inside: true,
                entered: true,
                exited: false,
            });
            state.entered_or_exited_frame = frame;
        }
        if state.entered_or_exited_frame == frame {
            if let Some(name) = team_name.filter(|name| !name.is_empty()) {
                world
                    .team_entered_or_exited
                    .insert(name.to_string(), frame);
            }
        }
    });
}

pub fn clear_host_trigger_flags() {
    HOST_TRIGGER_WORLD.with(|world| *world.borrow_mut() = HostTriggerWorld::default());
}

/// C++ `Object::xfer` (`Object.cpp:4218-4246`) per-area slot.
#[derive(Clone, Debug, Default)]
pub struct HostTriggerSlotPersist {
    pub trigger_id: i32,
    pub trigger_name: String,
    pub is_inside: bool,
    pub entered: bool,
    pub exited: bool,
}

/// C++ `Object::xfer` trigger housekeeping: `m_iPos`, `m_enteredOrExitedFrame`,
/// `m_numTriggerAreasActive` + per-area entered/exited/isInside.
#[derive(Clone, Debug, Default)]
pub struct HostObjectTriggerPersist {
    pub object_id: u32,
    pub i_x: i32,
    pub i_y: i32,
    pub entered_or_exited_frame: u32,
    pub slots: Vec<HostTriggerSlotPersist>,
}

/// Capture live `HOST_TRIGGER_WORLD` slots for WorldSnapshot persist.
pub fn capture_host_object_trigger_persists() -> Vec<HostObjectTriggerPersist> {
    let triggers = leftover_polygon_triggers();
    HOST_TRIGGER_WORLD.with(|world| {
        let world = world.borrow();
        let mut entries: Vec<HostObjectTriggerPersist> = world
            .objects
            .iter()
            .map(|(object_id, state)| HostObjectTriggerPersist {
                object_id: *object_id,
                i_x: state.i_x,
                i_y: state.i_y,
                entered_or_exited_frame: state.entered_or_exited_frame,
                slots: state
                    .slots
                    .iter()
                    .map(|slot| {
                        let trigger_name = triggers
                            .iter()
                            .find(|trigger| trigger.get_id() == slot.trigger_id)
                            .map(|trigger| trigger.get_trigger_name().to_string())
                            .unwrap_or_default();
                        HostTriggerSlotPersist {
                            trigger_id: slot.trigger_id,
                            trigger_name,
                            is_inside: slot.is_inside,
                            entered: slot.entered,
                            exited: slot.exited,
                        }
                    })
                    .collect(),
            })
            .collect();
        entries.sort_by_key(|entry| entry.object_id);
        entries
    })
}

/// Restore slots and integer pose before the first post-load position update.
pub fn restore_host_object_trigger_persists(entries: &[HostObjectTriggerPersist]) {
    let triggers = leftover_polygon_triggers();
    HOST_TRIGGER_WORLD.with(|world| {
        let mut world = world.borrow_mut();
        *world = HostTriggerWorld::default();
        for entry in entries {
            let slots = entry
                .slots
                .iter()
                .map(|slot| {
                    let trigger_id = if slot.trigger_name.is_empty() {
                        slot.trigger_id
                    } else {
                        triggers
                            .iter()
                            .find(|trigger| {
                                trigger.get_trigger_name().to_string() == slot.trigger_name
                            })
                            .map(|trigger| trigger.get_id())
                            .unwrap_or(slot.trigger_id)
                    };
                    HostTriggerSlot {
                        trigger_id,
                        is_inside: slot.is_inside,
                        entered: slot.entered,
                        exited: slot.exited,
                    }
                })
                .collect();
            world.objects.insert(
                entry.object_id,
                HostObjectTriggerState {
                    i_x: entry.i_x,
                    i_y: entry.i_y,
                    entered_or_exited_frame: entry.entered_or_exited_frame,
                    slots,
                },
            );
        }
    });
}

pub fn sync_host_trigger_flags_from_snapshot(frame: u32) {
    let snap = HOST_SCRIPT_QUERY.with(|slot| slot.borrow().clone());
    for obj in &snap.objects {
        let team = snap.team_instance_ids.iter().find_map(|(name, ids)| {
            ids.contains(&obj.id).then_some(name.as_str())
        });
        update_host_object_trigger_flags(obj.id, obj.x, obj.z, frame, false, team);
    }
}

pub fn host_object_did_enter_or_exit(object_id: u32) -> bool {
    let now = current_logic_frame();
    HOST_TRIGGER_WORLD.with(|world| {
        world
            .borrow()
            .objects
            .get(&object_id)
            .is_some_and(|state| host_flag_window(state.entered_or_exited_frame, now))
    })
}

pub fn host_object_did_enter(
    object_id: u32,
    trigger: &crate::polygon_trigger::PolygonTrigger,
) -> bool {
    let now = current_logic_frame();
    HOST_TRIGGER_WORLD.with(|world| {
        let world = world.borrow();
        let Some(state) = world.objects.get(&object_id) else {
            return false;
        };
        host_flag_window(state.entered_or_exited_frame, now)
            && state
                .slots
                .iter()
                .any(|slot| slot.entered && slot.trigger_id == trigger.get_id())
    })
}

pub fn host_object_did_exit(
    object_id: u32,
    trigger: &crate::polygon_trigger::PolygonTrigger,
) -> bool {
    let now = current_logic_frame();
    HOST_TRIGGER_WORLD.with(|world| {
        let world = world.borrow();
        let Some(state) = world.objects.get(&object_id) else {
            return false;
        };
        host_flag_window(state.entered_or_exited_frame, now)
            && state
                .slots
                .iter()
                .any(|slot| slot.exited && slot.trigger_id == trigger.get_id())
    })
}

/// C++ Team.cpp `locoSetMatches` for host units (no leftover AI → GROUND).
pub fn host_script_loco_matches_ground(which_to_consider: u32) -> bool {
    let remapped = (which_to_consider & 0x01) | ((which_to_consider & 0x02) << 2);
    (remapped & 0x01) != 0
}

pub fn host_script_team_member_ids(team_name: &str) -> Vec<u32> {
    if team_name.is_empty() {
        return Vec::new();
    }
    let mut ids = HOST_SCRIPT_QUERY.with(|slot| {
        let snap = slot.borrow();
        snap.team_instance_ids
            .get(team_name)
            .cloned()
            .or_else(|| {
                snap.team_instance_ids.iter().find_map(|(name, listed)| {
                    name.eq_ignore_ascii_case(team_name).then(|| listed.clone())
                })
            })
            .unwrap_or_default()
    });
    if ids.is_empty() {
        if let Ok(factory) = crate::team::get_team_factory().lock() {
            for team in factory.find_team_instances(team_name) {
                if let Ok(team_guard) = team.read() {
                    ids.extend(team_guard.get_members().iter().copied());
                }
            }
        }
    }
    if ids.is_empty() {
        let ord = match team_name.to_ascii_lowercase().as_str() {
            "gla" => 0,
            "usa" | "america" => 1,
            "china" => 2,
            "neutral" => 3,
            _ => team_name.parse::<u32>().unwrap_or(u32::MAX),
        };
        ids = host_script_team_unit_ids(ord);
    }
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub fn host_team_did_enter_or_exit(team_name: &str) -> bool {
    let now = current_logic_frame();
    let flagged = HOST_TRIGGER_WORLD.with(|world| {
        world
            .borrow()
            .team_entered_or_exited
            .get(team_name)
            .copied()
            .is_some_and(|frame| host_flag_window(frame, now))
    });
    flagged
        || host_script_team_member_ids(team_name)
            .into_iter()
            .any(host_object_did_enter_or_exit)
}

fn host_team_alive_positions(team_name: &str) -> Vec<(u32, f32, f32)> {
    let ids: HashSet<u32> = host_script_team_member_ids(team_name).into_iter().collect();
    HOST_SCRIPT_QUERY.with(|slot| {
        slot.borrow()
            .objects
            .iter()
            .filter(|obj| obj.alive && ids.contains(&obj.id))
            .map(|obj| (obj.id, obj.x, obj.z))
            .collect()
    })
}

pub fn host_team_all_inside(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_script_loco_matches_ground(which_to_consider) {
        return false;
    }
    let members = host_team_alive_positions(team_name);
    !members.is_empty()
        && members.iter().all(|(_, x, z)| {
            trigger.point_in_trigger_int(&host_xz_to_trigger_point(*x, *z))
        })
}

pub fn host_team_some_inside_some_outside(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_script_loco_matches_ground(which_to_consider) {
        return false;
    }
    let members = host_team_alive_positions(team_name);
    let mut any_inside = false;
    let mut any_outside = false;
    for (_, x, z) in members {
        if trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
            any_inside = true;
        } else {
            any_outside = true;
        }
    }
    any_inside && any_outside
}

pub fn host_team_did_all_enter(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_script_loco_matches_ground(which_to_consider) || !host_team_did_enter_or_exit(team_name)
    {
        return false;
    }
    let members = host_team_alive_positions(team_name);
    let mut entered = false;
    let mut outside = false;
    for (id, x, z) in members {
        if host_object_did_enter(id, trigger) {
            entered = true;
        } else if !trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
            outside = true;
        }
    }
    entered && !outside
}

pub fn host_team_did_partial_enter(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_script_loco_matches_ground(which_to_consider) || !host_team_did_enter_or_exit(team_name)
    {
        return false;
    }
    host_team_alive_positions(team_name)
        .into_iter()
        .any(|(id, _, _)| host_object_did_enter(id, trigger))
}

pub fn host_team_did_all_exit(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_script_loco_matches_ground(which_to_consider) || !host_team_did_enter_or_exit(team_name)
    {
        return false;
    }
    let members = host_team_alive_positions(team_name);
    let mut exited = false;
    let mut inside = false;
    let mut any = false;
    for (id, x, z) in members {
        any = true;
        if host_object_did_exit(id, trigger) {
            exited = true;
        } else if trigger.point_in_trigger_int(&host_xz_to_trigger_point(x, z)) {
            inside = true;
        }
    }
    any && exited && !inside
}

pub fn host_team_did_partial_exit(
    team_name: &str,
    trigger: &crate::polygon_trigger::PolygonTrigger,
    which_to_consider: u32,
) -> bool {
    if !host_script_loco_matches_ground(which_to_consider) || !host_team_did_enter_or_exit(team_name)
    {
        return false;
    }
    host_team_alive_positions(team_name)
        .into_iter()
        .any(|(id, _, _)| host_object_did_exit(id, trigger))
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
