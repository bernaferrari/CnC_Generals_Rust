//! Rhai Scripting Language Integration
//!
//! This module provides integration with the Rhai scripting language, allowing map
//! authors and modders to write custom scripts in Rhai syntax that integrate with
//! the C++ script engine behavior.
//!
//! Rhai is a lightweight embedded scripting language for Rust that provides:
//! - Familiar JavaScript-like syntax
//! - Type safety at runtime
//! - Good error messages
//! - Excellent performance
//!
//! # Example Rhai Script
//!
//! ```rhai
//! // Check if player has enough money
//! if get_player_money(0) >= 1000 {
//!     // Create a unit at waypoint
//!     create_unit("Tank", "teamPlayer", "WaypointStart");
//!     // Set a flag to track this event
//!     set_flag("TankCreated", true);
//! }
//!
//! // Check a counter
//! let kills = get_counter("PlayerKills");
//! if kills > 10 {
//!     display_text("You've destroyed 10 enemy units!");
//! }
//! ```

use super::{ScriptContext, ScriptValue};
use crate::common::{Coord3D, LOGICFRAMES_PER_SECOND};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::object_manager::get_object_manager;
use crate::player::player_list;
use crate::scripting::engine::{
    get_area_tracker, get_event_manager, get_named_object_tracker, get_script_engine,
};
use crate::scripting::events::{GameEvent, GameEventType};
use crate::system::game_logic::get_game_logic;
use crate::team::get_team_factory;
use crate::{GameLogicError, GameLogicResult};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use game_engine::common::rts::{get_science_store, SCIENCE_INVALID};
use rhai::{Array as RhaiArray, Dynamic, Engine, EvalAltResult, Map as RhaiMap, Scope};

/// Rhai script executor
///
/// Manages Rhai script execution within the game engine, providing access to
/// game state and actions through registered functions.
pub struct RhaiScriptExecutor {
    engine: Arc<RwLock<Engine>>,
}

impl RhaiScriptExecutor {
    /// Create a new Rhai script executor
    pub fn new() -> GameLogicResult<Self> {
        let mut engine = Engine::new();

        // Register all game-specific functions
        Self::register_game_functions(&mut engine)?;

        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
        })
    }

    /// Helper function to access game logic safely
    #[inline]
    fn with_game_logic<F, R>(f: F) -> Option<R>
    where
        F: FnOnce(&mut crate::system::game_logic::GameLogic) -> R,
    {
        let game_logic_mutex = get_game_logic();
        game_logic_mutex.lock().ok().map(|mut gl| f(&mut *gl))
    }

    /// Register all game-specific functions to Rhai engine
    ///
    /// This matches the C++ ScriptEngine function exposure pattern, providing
    /// access to counters, flags, objects, players, and game state.
    pub(crate) fn register_game_functions(engine: &mut Engine) -> GameLogicResult<()> {
        fn find_object_by_name(name: &str) -> Option<std::sync::Arc<std::sync::RwLock<Object>>> {
            // Prefer the ScriptEngine named-object cache (C++ getUnitNamed behavior).
            let tracker = get_named_object_tracker();
            if let Ok(Some(object_id)) = tracker.get_object_id(name) {
                if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(object_id) {
                    return Some(obj);
                }
            }

            // Fall back to a case-insensitive scan for objects that were not registered with the
            // named-object tracker (e.g. dynamically spawned without a name).
            let lower = name.to_ascii_lowercase();
            OBJECT_REGISTRY
                .get_all_objects()
                .into_iter()
                .find(|obj_ref| {
                    obj_ref
                        .read()
                        .ok()
                        .map(|o| o.get_name().to_ascii_lowercase() == lower)
                        .unwrap_or(false)
                })
        }

        // ============================================================================
        // Counter and Flag Management
        // Matches C++ ScriptEngine counter/flag system
        // ============================================================================

        engine.register_fn("get_counter", |name: &str| -> i64 {
            log::debug!("Rhai: get_counter({})", name);
            // Access script engine counters
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(counter) = script_engine.get_counter(name) {
                        return counter.value as i64;
                    }
                }
            }
            0
        });

        engine.register_fn("set_counter", |name: &str, value: i64| {
            log::debug!("Rhai: set_counter({}, {})", name, value);
            // Set script engine counters
            if let Ok(mut engine_guard) = get_script_engine().write() {
                if let Some(ref mut script_engine) = *engine_guard {
                    if let Err(e) = script_engine.set_counter(name, value as i32) {
                        log::error!("Failed to set counter {}: {}", name, e);
                    }
                }
            }
        });

        engine.register_fn("increment_counter", |name: &str| {
            log::debug!("Rhai: increment_counter({})", name);
            if let Ok(mut engine_guard) = get_script_engine().write() {
                if let Some(ref mut script_engine) = *engine_guard {
                    if let Err(e) = script_engine.increment_counter(name) {
                        log::error!("Failed to increment counter {}: {}", name, e);
                    }
                }
            }
        });

        engine.register_fn("decrement_counter", |name: &str| {
            log::debug!("Rhai: decrement_counter({})", name);
            if let Ok(mut engine_guard) = get_script_engine().write() {
                if let Some(ref mut script_engine) = *engine_guard {
                    if let Err(e) = script_engine.decrement_counter(name) {
                        log::error!("Failed to decrement counter {}: {}", name, e);
                    }
                }
            }
        });

        engine.register_fn("get_flag", |name: &str| -> bool {
            log::debug!("Rhai: get_flag({})", name);
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(flag) = script_engine.get_flag(name) {
                        return flag.value;
                    }
                }
            }
            false
        });

        engine.register_fn("set_flag", |name: &str, value: bool| {
            log::debug!("Rhai: set_flag({}, {})", name, value);
            if let Ok(mut engine_guard) = get_script_engine().write() {
                if let Some(ref mut script_engine) = *engine_guard {
                    if let Err(e) = script_engine.set_flag(name, value) {
                        log::error!("Failed to set flag {}: {}", name, e);
                    }
                }
            }
        });

        // ============================================================================
        // Object Queries
        // Matches C++ ScriptEngine object tracking
        // ============================================================================

        engine.register_fn("object_exists", |name: &str| -> bool {
            log::debug!("Rhai: object_exists({})", name);
            find_object_by_name(name).is_some()
        });

        engine.register_fn("object_health", |name: &str| -> f32 {
            log::debug!("Rhai: object_health({})", name);
            find_object_by_name(name)
                .and_then(|obj| obj.read().ok().map(|o| o.get_health()))
                .unwrap_or(0.0)
        });

        engine.register_fn("object_position_x", |name: &str| -> f32 {
            log::debug!("Rhai: object_position_x({})", name);
            find_object_by_name(name)
                .and_then(|obj| obj.read().ok().map(|o| o.get_position().x))
                .unwrap_or(0.0)
        });

        engine.register_fn("object_position_y", |name: &str| -> f32 {
            log::debug!("Rhai: object_position_y({})", name);
            find_object_by_name(name)
                .and_then(|obj| obj.read().ok().map(|o| o.get_position().y))
                .unwrap_or(0.0)
        });

        engine.register_fn("object_position_z", |name: &str| -> f32 {
            log::debug!("Rhai: object_position_z({})", name);
            find_object_by_name(name)
                .and_then(|obj| obj.read().ok().map(|o| o.get_position().z))
                .unwrap_or(0.0)
        });

        engine.register_fn("object_is_destroyed", |name: &str| -> bool {
            log::debug!("Rhai: object_is_destroyed({})", name);
            find_object_by_name(name)
                .and_then(|obj| obj.read().ok().map(|o| o.is_effectively_dead()))
                .unwrap_or(false)
        });

        engine.register_fn("object_is_alive", |name: &str| -> bool {
            log::debug!("Rhai: object_is_alive({})", name);
            find_object_by_name(name)
                .and_then(|obj| obj.read().ok().map(|o| !o.is_effectively_dead()))
                .unwrap_or(false)
        });

        // ============================================================================
        // Player Queries
        // Matches C++ Player class interface
        // ============================================================================

        engine.register_fn("player_money", |player: i64| -> i64 {
            log::debug!("Rhai: player_money({})", player);
            // Query player money through player system
            let player_list_lock = player_list();
            if let Ok(list) = player_list_lock.read() {
                if let Some(player_arc) = list.get_player(player as i32) {
                    if let Ok(player_guard) = player_arc.read() {
                        return player_guard.get_money().get_money() as i64;
                    }
                }
            }
            0
        });
        engine.register_fn("get_player_money", |player: i64| -> i64 {
            log::debug!("Rhai: get_player_money({})", player);
            let player_list_lock = player_list();
            if let Ok(list) = player_list_lock.read() {
                if let Some(player_arc) = list.get_player(player as i32) {
                    if let Ok(player_guard) = player_arc.read() {
                        return player_guard.get_money().get_money() as i64;
                    }
                }
            }
            0
        });

        engine.register_fn("player_alive", |player: i64| -> bool {
            log::debug!("Rhai: player_alive({})", player);
            // Check if player is alive
            let player_list_lock = player_list();
            if let Ok(list) = player_list_lock.read() {
                if let Some(player_arc) = list.get_player(player as i32) {
                    if let Ok(player_guard) = player_arc.read() {
                        return !player_guard.is_defeated();
                    }
                }
            }
            false
        });

        engine.register_fn("player_defeated", |player: i64| -> bool {
            log::debug!("Rhai: player_defeated({})", player);
            // Check if player is defeated
            let player_list_lock = player_list();
            if let Ok(list) = player_list_lock.read() {
                if let Some(player_arc) = list.get_player(player as i32) {
                    if let Ok(player_guard) = player_arc.read() {
                        return player_guard.is_defeated();
                    }
                }
            }
            true
        });

        engine.register_fn(
            "player_has_building",
            |player: i64, building_type: &str| -> bool {
                log::debug!("Rhai: player_has_building({}, {})", player, building_type);
                // Check if player has specific building type
                use crate::common::KindOf;
                let obj_manager = get_object_manager();
                if let Ok(manager) = obj_manager.read() {
                    let owned_objects = manager.get_objects_owned_by_player(player as u32);
                    for obj_id in owned_objects {
                        if let Some(obj_arc) = manager.get_object(obj_id) {
                            if let Ok(obj) = obj_arc.read() {
                                if let Ok(base) = obj.base.read() {
                                    if let Some(template) = &obj.template {
                                        if template.get_name().eq_ignore_ascii_case(building_type) {
                                            if base.is_kind_of(KindOf::Structure) {
                                                return true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                false
            },
        );

        engine.register_fn("player_unit_count", |player: i64, unit_type: &str| -> i64 {
            log::debug!("Rhai: player_unit_count({}, {})", player, unit_type);
            // Count units of specific type owned by player
            let obj_manager = get_object_manager();
            let mut count = 0i64;
            if let Ok(manager) = obj_manager.read() {
                let owned_objects = manager.get_objects_owned_by_player(player as u32);
                for obj_id in owned_objects {
                    if let Some(obj_arc) = manager.get_object(obj_id) {
                        if let Ok(obj) = obj_arc.read() {
                            if let Some(template) = &obj.template {
                                if template.get_name().eq_ignore_ascii_case(unit_type) {
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            }
            count
        });

        engine.register_fn("player_has_science", |player: i64, science: &str| -> bool {
            log::debug!("Rhai: player_has_science({}, {})", player, science);
            let Some(store) = get_science_store() else {
                return false;
            };
            let science = store.get_science_from_internal_name(science);
            if science == SCIENCE_INVALID {
                return false;
            }

            let player_list_lock = player_list();
            let player_arc = player_list_lock
                .read()
                .ok()
                .and_then(|list| list.get_player(player as i32).cloned());
            let Some(player_arc) = player_arc else {
                return false;
            };

            player_arc
                .read()
                .ok()
                .map(|p| p.has_science(science))
                .unwrap_or(false)
        });

        engine.register_fn("player_power", |player: i64| -> i64 {
            log::debug!("Rhai: player_power({})", player);
            let player_list_lock = player_list();
            let player_arc = player_list_lock
                .read()
                .ok()
                .and_then(|list| list.get_player(player as i32).cloned());
            let Some(player_arc) = player_arc else {
                return 0;
            };
            let Ok(player_guard) = player_arc.read() else {
                return 0;
            };

            let energy = player_guard.get_energy();
            let available = energy.production() - energy.consumption();
            available as i64
        });
        engine.register_fn("get_player_power", |player: i64| -> i64 {
            log::debug!("Rhai: get_player_power({})", player);
            let player_list_lock = player_list();
            let player_arc = player_list_lock
                .read()
                .ok()
                .and_then(|list| list.get_player(player as i32).cloned());
            let Some(player_arc) = player_arc else {
                return 0;
            };
            let Ok(player_guard) = player_arc.read() else {
                return 0;
            };

            let energy = player_guard.get_energy();
            let available = energy.production() - energy.consumption();
            available as i64
        });

        // ============================================================================
        // Team Queries
        // Matches C++ Team class interface
        // ============================================================================

        engine.register_fn("team_exists", |team_name: &str| -> bool {
            log::debug!("Rhai: team_exists({})", team_name);
            // Check if team exists in team manager
            let team_factory = get_team_factory();
            if let Ok(mut factory) = team_factory.lock() {
                return factory.find_team(team_name).is_some();
            }
            false
        });

        engine.register_fn("team_destroyed", |team_name: &str| -> bool {
            log::debug!("Rhai: team_destroyed({})", team_name);
            // Check if all team members are destroyed
            let team_factory = get_team_factory();
            if let Ok(mut factory) = team_factory.lock() {
                if let Some(team_arc) = factory.find_team(team_name) {
                    if let Ok(team) = team_arc.read() {
                        if team.get_member_count() == 0 {
                            return true;
                        }
                        let members = team.get_members().to_vec();
                        drop(team);

                        if let Ok(manager) = get_object_manager().read() {
                            for &member_id in &members {
                                if let Some(obj_arc) = manager.get_object(member_id) {
                                    if let Ok(obj) = obj_arc.read() {
                                        if !obj.is_destroyed() {
                                            return false;
                                        }
                                    }
                                }
                            }
                            return true;
                        }
                    }
                }
            }
            false
        });

        engine.register_fn("team_unit_count", |team_name: &str| -> i64 {
            log::debug!("Rhai: team_unit_count({})", team_name);
            // Count living members of team
            let team_factory = get_team_factory();
            if let Ok(mut factory) = team_factory.lock() {
                if let Some(team_arc) = factory.find_team(team_name) {
                    if let Ok(team) = team_arc.read() {
                        let members = team.get_members().to_vec();
                        drop(team);

                        let mut count = 0i64;
                        if let Ok(manager) = get_object_manager().read() {
                            for &member_id in &members {
                                if let Some(obj_arc) = manager.get_object(member_id) {
                                    if let Ok(obj) = obj_arc.read() {
                                        if !obj.is_destroyed() {
                                            count += 1;
                                        }
                                    }
                                }
                            }
                        }
                        return count;
                    }
                }
            }
            0
        });

        engine.register_fn("team_in_area", |team_name: &str, area_name: &str| -> bool {
            log::debug!("Rhai: team_in_area({}, {})", team_name, area_name);
            let team_factory = get_team_factory();
            let Ok(mut factory) = team_factory.lock() else {
                return false;
            };
            let Some(team_arc) = factory.find_team(team_name) else {
                return false;
            };
            let Ok(team) = team_arc.read() else {
                return false;
            };
            let members = team.get_members().to_vec();
            drop(team);

            let area_tracker = get_area_tracker();
            for member_id in members {
                if area_tracker
                    .is_object_in_area(member_id as u32, area_name)
                    .unwrap_or(false)
                {
                    return true;
                }
            }
            false
        });

        engine.register_fn("team_members", |team_name: &str| -> i64 {
            log::debug!("Rhai: team_members({})", team_name);
            let team_factory = get_team_factory();
            if let Ok(mut factory) = team_factory.lock() {
                if let Some(team_arc) = factory.find_team(team_name) {
                    if let Ok(team) = team_arc.read() {
                        return team.get_member_count() as i64;
                    }
                }
            }
            0
        });

        engine.register_fn("team_average_health", |team_name: &str| -> f64 {
            log::debug!("Rhai: team_average_health({})", team_name);
            let team_factory = get_team_factory();
            let Ok(mut factory) = team_factory.lock() else {
                return 100.0;
            };
            let Some(team_arc) = factory.find_team(team_name) else {
                return 100.0;
            };
            let Ok(team) = team_arc.read() else {
                return 100.0;
            };
            let members = team.get_members().to_vec();
            drop(team);

            let obj_manager = get_object_manager();
            let Ok(manager) = obj_manager.read() else {
                return 100.0;
            };

            let mut total = 0.0f64;
            let mut count = 0u32;
            for member_id in members {
                if let Some(obj_arc) = manager.get_object(member_id) {
                    if let Ok(obj) = obj_arc.read() {
                        total += (obj.get_health_percentage() as f64) * 100.0;
                        count += 1;
                    }
                }
            }
            if count == 0 {
                100.0
            } else {
                (total / count as f64).clamp(0.0, 100.0)
            }
        });

        // ============================================================================
        // Game State Queries
        // Matches C++ GameLogic interface
        // ============================================================================

        engine.register_fn("game_time", || -> f64 {
            log::debug!("Rhai: game_time()");
            // Get actual game time in seconds
            if let Ok(game_logic) = get_game_logic().lock() {
                return game_logic.get_frame() as f64 / LOGICFRAMES_PER_SECOND as f64;
            }
            0.0
        });

        engine.register_fn("game_frame", || -> i64 {
            log::debug!("Rhai: game_frame()");
            // Get current game frame number
            if let Ok(game_logic) = get_game_logic().lock() {
                return game_logic.get_frame() as i64;
            }
            0
        });

        engine.register_fn("is_game_paused", || -> bool {
            log::debug!("Rhai: is_game_paused()");
            // Pause state is not yet modelled in the GameLogic singleton.
            false
        });

        engine.register_fn("get_difficulty", || -> i64 {
            log::debug!("Rhai: get_difficulty()");
            // Resolve from player 0 if available, else default to Normal.
            let player_arc = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(0).cloned());
            let difficulty = player_arc
                .and_then(|p| p.read().ok().map(|p| p.get_player_difficulty()))
                .unwrap_or(crate::player::GameDifficulty::Normal);
            match difficulty {
                crate::player::GameDifficulty::Easy => 0,
                crate::player::GameDifficulty::Normal => 1,
                crate::player::GameDifficulty::Hard => 2,
                crate::player::GameDifficulty::Brutal => 3,
            }
        });

        // ============================================================================
        // Script Actions
        // Expose key script actions for Rhai to trigger
        // ============================================================================

        engine.register_fn("display_text", |text: &str| {
            log::debug!("Rhai: display_text({})", text);
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.display_text(text) {
                            log::warn!("Script action handler display_text failed: {}", err);
                        }
                        return;
                    }
                }
            }
            log::info!("Script display_text: {}", text);
        });

        engine.register_fn("play_sound", |sound_name: &str| {
            log::debug!("Rhai: play_sound({})", sound_name);
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.play_sound_effect(sound_name) {
                            log::warn!("Script action handler play_sound_effect failed: {}", err);
                        }
                        return;
                    }
                }
            }
            log::info!("Script play_sound: {}", sound_name);
        });

        engine.register_fn(
            "create_unit",
            |unit_type: &str, team: &str, waypoint: &str| {
                log::debug!("Rhai: create_unit({}, {}, {})", unit_type, team, waypoint);
                let waypoint_name = crate::common::AsciiString::from(waypoint);
                let spawn_pos = crate::terrain::get_terrain_logic()
                    .read()
                    .ok()
                    .and_then(|terrain| {
                        terrain
                            .get_waypoint_by_name(&waypoint_name)
                            .map(|waypoint| *waypoint.get_location())
                    })
                    .or_else(|| {
                        find_object_by_name(waypoint)
                            .and_then(|obj| obj.read().ok().map(|o| *o.get_position()))
                    })
                    .unwrap_or(crate::common::Coord3D::ZERO);

                let team_arc = get_team_factory()
                    .lock()
                    .ok()
                    .and_then(|mut factory| factory.find_team(team));

                if let Ok(mut manager) = get_object_manager().write() {
                    match manager.create_object(
                        unit_type,
                        spawn_pos,
                        team_arc,
                        crate::object_manager::ObjectCreationFlags::from_template(),
                    ) {
                        Ok(object_id) => log::info!(
                            "Script created unit {} (id {}) for team {} at {:?}",
                            unit_type,
                            object_id,
                            team,
                            spawn_pos
                        ),
                        Err(err) => log::warn!("Script create_unit failed: {}", err),
                    }
                }
            },
        );

        engine.register_fn("victory", || {
            log::debug!("Rhai: victory()");
            let event = GameEvent::new(GameEventType::PlayerVictorious, "Scripted victory".into());
            if let Err(err) = get_event_manager().fire_event_sync(event) {
                log::error!("Failed to queue victory event: {}", err);
            }
        });

        engine.register_fn("defeat", || {
            log::debug!("Rhai: defeat()");
            let event = GameEvent::new(GameEventType::PlayerDefeated, "Scripted defeat".into());
            if let Err(err) = get_event_manager().fire_event_sync(event) {
                log::error!("Failed to queue defeat event: {}", err);
            }
        });

        engine.register_fn("move_camera", |x: f64, y: f64, z: f64| {
            log::debug!("Rhai: move_camera({}, {}, {})", x, y, z);
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.move_camera(x as f32, y as f32, z as f32) {
                            log::warn!("Script action handler move_camera failed: {}", err);
                        }
                        return;
                    }
                }
            }
            log::info!("Script move_camera to ({}, {}, {})", x, y, z);
        });

        engine.register_fn(
            "set_objective",
            |name: &str, description: &str, completed: bool| {
                log::debug!(
                    "Rhai: set_objective({}, {}, {})",
                    name,
                    description,
                    completed
                );
                if let Ok(engine_guard) = get_script_engine().read() {
                    if let Some(ref script_engine) = *engine_guard {
                        if let Some(handler) = script_engine.action_handler() {
                            if let Err(err) = handler.set_objective(name, description, completed) {
                                log::warn!("Script action handler set_objective failed: {}", err);
                            }
                            return;
                        }
                    }
                }
                log::info!(
                    "Script set_objective '{}' completed={} description='{}'",
                    name,
                    completed,
                    description
                );
            },
        );

        engine.register_fn(
            "spawn_effect",
            |effect_type: &str, x: f64, y: f64, z: f64| {
                log::debug!("Rhai: spawn_effect({}, {}, {}, {})", effect_type, x, y, z);
                if let Ok(engine_guard) = get_script_engine().read() {
                    if let Some(ref script_engine) = *engine_guard {
                        if let Some(handler) = script_engine.action_handler() {
                            if let Err(err) =
                                handler.spawn_effect(effect_type, x as f32, y as f32, z as f32)
                            {
                                log::warn!("Script action handler spawn_effect failed: {}", err);
                            }
                            return;
                        }
                    }
                }
                log::info!(
                    "Script spawn_effect {} at ({}, {}, {})",
                    effect_type,
                    x,
                    y,
                    z
                );
            },
        );

        // ============================================================================
        // Utility Functions
        // ============================================================================

        engine.register_fn("log_info", |message: &str| {
            log::info!("Rhai Script: {}", message);
        });

        engine.register_fn("log_debug", |message: &str| {
            log::debug!("Rhai Script: {}", message);
        });

        Ok(())
    }

    /// Execute Rhai script
    ///
    /// # Arguments
    /// * `script` - The Rhai script source code
    /// * `context` - Current script execution context
    ///
    /// # Returns
    /// The result of script execution as a Dynamic value
    pub fn execute(&self, script: &str, context: &ScriptContext) -> GameLogicResult<Dynamic> {
        log::debug!("Executing Rhai script: {} bytes", script.len());

        let engine = self
            .engine
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to lock engine: {}", e)))?;

        // Use a fresh scope per execution to match the C++ script engine behavior:
        // persistent state should live in explicit counters/flags/variables, not in local
        // script-defined variables that leak across runs.
        let mut scope = Scope::new();

        // Inject context variables into scope
        for (key, value) in &context.variables {
            scope.push(key.clone(), script_value_to_dynamic(value));
        }

        // Inject game state
        scope.push(
            "game_time".to_string(),
            Dynamic::from(context.game_time.as_secs_f64()),
        );

        if let Some(player) = context.active_player {
            scope.push("active_player".to_string(), Dynamic::from(player as i64));
        }

        let mut state = RhaiMap::new();
        state.insert(
            "map_name".into(),
            Dynamic::from(context.game_state.map_name.clone()),
        );
        state.insert(
            "game_mode".into(),
            Dynamic::from(context.game_state.game_mode.clone()),
        );

        let mut players = RhaiArray::new();
        for p in &context.game_state.players {
            let mut player = RhaiMap::new();
            player.insert("id".into(), Dynamic::from(p.id as i64));
            player.insert("name".into(), Dynamic::from(p.name.clone()));
            player.insert("team".into(), Dynamic::from(p.team as i64));
            player.insert("color".into(), Dynamic::from(p.color.clone()));
            player.insert("is_human".into(), Dynamic::from(p.is_human));
            player.insert("is_alive".into(), Dynamic::from(p.is_alive));
            player.insert("score".into(), Dynamic::from(p.score));
            players.push(Dynamic::from(player));
        }
        state.insert("players".into(), Dynamic::from(players));

        let mut objectives = RhaiArray::new();
        for o in &context.game_state.objectives {
            let mut objective = RhaiMap::new();
            objective.insert("id".into(), Dynamic::from(o.id.clone()));
            objective.insert("name".into(), Dynamic::from(o.name.clone()));
            objective.insert("description".into(), Dynamic::from(o.description.clone()));
            objective.insert("completed".into(), Dynamic::from(o.completed));
            objective.insert("hidden".into(), Dynamic::from(o.hidden));
            objective.insert("priority".into(), Dynamic::from(o.priority));
            objectives.push(Dynamic::from(objective));
        }
        state.insert("objectives".into(), Dynamic::from(objectives));

        scope.push("game_state".to_string(), Dynamic::from(state));

        // Execute script
        engine
            .eval_with_scope::<Dynamic>(&mut scope, script)
            .map_err(|e| GameLogicError::Configuration(format!("Rhai execution error: {}", e)))
    }

    /// Execute Rhai script from file
    ///
    /// # Arguments
    /// * `path` - Path to the script file
    /// * `context` - Current script execution context
    pub fn execute_file(&self, path: &str, context: &ScriptContext) -> GameLogicResult<Dynamic> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| GameLogicError::IO(format!("Failed to read script file: {}", e)))?;

        self.execute(&script, context)
    }

    /// Register a custom function in the Rhai engine
    ///
    /// Allows dynamic registration of game-specific functions at runtime.
    pub fn register_custom_function<F>(&self, name: &str, _func: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        log::debug!("Registering custom Rhai function: {}", name);
        if let Ok(mut engine) = self.engine.write() {
            engine.register_fn(name, _func);
        }
    }
}

fn script_value_to_dynamic(value: &ScriptValue) -> Dynamic {
    match value {
        ScriptValue::Null => Dynamic::UNIT,
        ScriptValue::Bool(b) => Dynamic::from(*b),
        ScriptValue::Int(i) => Dynamic::from(*i),
        ScriptValue::Float(f) => Dynamic::from(*f),
        ScriptValue::String(s) => Dynamic::from(s.clone()),
        ScriptValue::Coord3D([x, y, z]) => Dynamic::from(Coord3D::new(*x, *y, *z)),
        ScriptValue::ObjectId(id) => Dynamic::from(*id as i64),
        ScriptValue::PlayerId(id) => Dynamic::from(*id as i64),
        ScriptValue::Team(team) => Dynamic::from(team.clone()),
        ScriptValue::Array(values) => Dynamic::from(
            values
                .iter()
                .map(script_value_to_dynamic)
                .collect::<rhai::Array>(),
        ),
        ScriptValue::Object(map) => {
            let mut out = rhai::Map::new();
            for (k, v) in map {
                out.insert(k.into(), script_value_to_dynamic(v));
            }
            Dynamic::from(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_context() -> ScriptContext {
        ScriptContext {
            game_time: Duration::from_secs(60),
            active_player: None,
            variables: HashMap::new(),
            game_state: crate::scripting::GameStateContext {
                map_name: "TestMap".to_string(),
                game_mode: "Skirmish".to_string(),
                players: vec![],
                objectives: vec![],
            },
        }
    }

    #[test]
    fn test_rhai_executor_creation() {
        let executor = RhaiScriptExecutor::new();
        assert!(executor.is_ok());
    }

    #[test]
    fn test_rhai_basic_execution() {
        let executor = RhaiScriptExecutor::new().unwrap();
        let context = create_test_context();

        let _ = executor.execute("let x = 5 + 3; x", &context);
    }

    #[test]
    fn test_rhai_game_functions() {
        let executor = RhaiScriptExecutor::new().unwrap();
        let context = create_test_context();

        let script = r#"
            set_counter("test", 42);
            let val = get_counter("test");
            val
        "#;

        let _ = executor.execute(script, &context);
    }

    #[test]
    fn test_rhai_context_variables() {
        let executor = RhaiScriptExecutor::new().unwrap();
        let mut variables = HashMap::new();
        variables.insert("test_var".to_string(), ScriptValue::Int(100));

        let context = ScriptContext {
            game_time: Duration::from_secs(0),
            active_player: None,
            variables,
            game_state: crate::scripting::GameStateContext {
                map_name: "Test".to_string(),
                game_mode: "Test".to_string(),
                players: vec![],
                objectives: vec![],
            },
        };

        let _ = executor.execute("test_var", &context);
    }

    #[test]
    fn test_rhai_file_execution() {
        use std::fs::File;
        use std::io::Write;
        use std::path::PathBuf;

        let executor = RhaiScriptExecutor::new().unwrap();
        let context = create_test_context();

        // Create temporary script file (in system temp dir)
        let mut path = std::env::temp_dir();
        path.push("rhai_test_script.rhai");
        let mut temp_file = File::create(&path).unwrap();
        writeln!(temp_file, "let x = 10; x * 2").unwrap();
        let path = PathBuf::from(path);
        let _ = executor.execute_file(path.to_string_lossy().as_ref(), &context);
    }
}
