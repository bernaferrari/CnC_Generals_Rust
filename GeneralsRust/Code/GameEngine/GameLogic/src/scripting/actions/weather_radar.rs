//! Weather, shroud, and radar script actions
//!
//! C++: ScriptActions.cpp reveal/shroud L2984–3090, radar L2840–2898,
//! weather L3801.
//!
//! Split from `scripting/actions.rs` for module-size parity.
//! Observable script behavior is unchanged.

use super::ScriptAction;
use super::helpers::*;
use crate::action_manager::TheActionManager;
use crate::ai::integration::with_ai_integration_mut;
use crate::ai::{AiCommandInterface, AiCommandParams, AiCommandType, AiGroup, GuardMode, the_ai};
use crate::commands::command::CommandType;
use crate::commands::{Command, CommandPriority, QueuedCommand, get_command_queue_manager};
use crate::common::PlayerIndex;
use crate::common::{
    AsciiString, CommandSourceType, Coord3D, INVALID_OBJECT_ID, LocomotorSetType, Real,
    Relationship,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::effects::FXList;
use crate::helpers::{TheGameLogic, TheVictoryConditions};
use crate::modules::{AIUpdateInterfaceExt, ContainModuleInterfaceExt};
use crate::object::object_factory::{GameObjectInstance, get_object_factory};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object_manager::{ObjectCreationFlags, get_object_manager};
use crate::player::{PlayerType, player_list};
use crate::scripting::core::{LOCAL_PLAYER, TEAM_THE_PLAYER, THE_PLAYER, THIS_PLAYER, THIS_TEAM};
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::{ScriptContext, ScriptResult, ScriptValue};
use crate::system::shroud_manager::get_shroud_manager;
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{RadarEventType, get_radar_system};

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Reveal map area action
pub(super) struct RevealMapAreaAction;

#[async_trait]
impl ScriptAction for RevealMapAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let x = get_float_param(parameters, "x")? as f32;
        let y = get_float_param(parameters, "y")? as f32;
        let radius = get_float_param(parameters, "radius")? as f32;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Revealing map area at ({}, {}) with radius {} for player {}",
            x,
            y,
            radius,
            player
        );

        let center = Coord3D::new(x, y, 0.0);
        let player_mask = 1u32 << (player.max(0) as u32);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.do_shroud_reveal(&center, radius, player_mask);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "reveal_map_area"
    }

    fn description(&self) -> &str {
        "Reveals an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Shroud map area action
pub(super) struct ShroudMapAreaAction;

#[async_trait]
impl ScriptAction for ShroudMapAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let radius = get_float_param(parameters, "radius")?;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Shrouding map area at ({}, {}) with radius {} for player {}",
            x,
            y,
            radius,
            player
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "shroud_map_area"
    }

    fn description(&self) -> &str {
        "Shrouds an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set weather action
pub(super) struct SetWeatherAction;

#[async_trait]
impl ScriptAction for SetWeatherAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let weather_type = get_string_param(parameters, "weather_type")?;
        let intensity = get_float_param_optional(parameters, "intensity").unwrap_or(1.0);

        log::info!(
            "Setting weather to '{}' with intensity {}",
            weather_type,
            intensity
        );

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_weather"
    }

    fn description(&self) -> &str {
        "Changes the weather conditions"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["weather_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["intensity".to_string()]
    }
}

/// Set time of day action
pub(super) struct SetTimeOfDayAction;

#[async_trait]
impl ScriptAction for SetTimeOfDayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let time = get_float_param(parameters, "time")?; // 0.0 = midnight, 0.5 = noon, 1.0 = midnight
        let transition_duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0);

        log::info!(
            "Setting time of day to {} over {} seconds",
            time,
            transition_duration
        );

        let time_of_day = if time >= 0.25 && time < 0.5 {
            crate::common::audio::TimeOfDay::Morning
        } else if time >= 0.5 && time < 0.75 {
            crate::common::audio::TimeOfDay::Day
        } else if time >= 0.75 {
            crate::common::audio::TimeOfDay::Evening
        } else {
            crate::common::audio::TimeOfDay::Night
        };

        if let Some(global) = crate::helpers::TheGlobalData::get() {
            global.set_time_of_day(time_of_day);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "set_time_of_day"
    }

    fn description(&self) -> &str {
        "Changes the time of day"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["time".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

// ============================================================================
// MAP/CAMERA ACTIONS (8 critical actions)
// ============================================================================

/// Reveal fog of war
pub(super) struct MapRevealAreaAction;

#[async_trait]
impl ScriptAction for MapRevealAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;
        let radius = get_float_param(parameters, "radius")?;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Revealing map area at waypoint '{}' radius {} for player {}",
            waypoint,
            radius,
            player
        );

        // Integration with fog of war/shroud system:
        // Matches C++ ScriptActions.cpp:doMapReveal
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D position = *way->getLocation()
        // 3. PartitionManager->revealArea(position, radius, player, permanent)
        // 4. Updates shroud cells in radius to visible
        // 5. Can be temporary or permanent reveal
        // Uses: ShroudManager from system/shroud_manager.rs

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "Map reveal failed: waypoint '{}' not found",
                waypoint_ascii.as_str()
            );
            return Ok(ScriptResult::Success(None));
        };

        let player_mask = 1u32 << (player.max(0) as u32);

        if player_mask != 0 {
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                shroud_mgr.do_shroud_reveal(&target, radius as f32, player_mask);
                shroud_mgr.undo_shroud_reveal(&target, radius as f32, player_mask);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "map_reveal_area"
    }

    fn description(&self) -> &str {
        "Reveals an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "waypoint".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Add fog
pub(super) struct MapShroudAreaAction;

#[async_trait]
impl ScriptAction for MapShroudAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;
        let radius = get_float_param(parameters, "radius")?;
        let player = get_int_param(parameters, "player")?;

        log::info!(
            "Shrouding map area at waypoint '{}' radius {} for player {}",
            waypoint,
            radius,
            player
        );

        // Integration with fog of war/shroud system:
        // Re-applies fog to previously revealed area
        // 1. Resolve waypoint position and radius
        // 2. PartitionManager->shroudArea(position, radius, player)
        // 3. Sets shroud cells back to unexplored/hidden
        // 4. Useful for dynamic map changes or scripted events
        // Uses: ShroudManager from system/shroud_manager.rs

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });
        let Some(target) = target else {
            log::warn!(
                "Map shroud failed: waypoint '{}' not found",
                waypoint_ascii.as_str()
            );
            return Ok(ScriptResult::Success(None));
        };

        let player_mask = 1u32 << (player.max(0) as u32);

        if player_mask != 0 {
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                shroud_mgr.do_shroud_cover(&target, radius as f32, player_mask);
                shroud_mgr.undo_shroud_cover(&target, radius as f32, player_mask);
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "map_shroud_area"
    }

    fn description(&self) -> &str {
        "Shrouds an area of the map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "waypoint".to_string(),
            "radius".to_string(),
            "player".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set weather
pub(super) struct WeatherSetAction;

#[async_trait]
impl ScriptAction for WeatherSetAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let weather_type = get_string_param(parameters, "weather_type")?;
        let enabled = parameters
            .get("enabled")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        log::info!("Setting weather '{}' to {}", weather_type, enabled);

        let handler = get_script_engine()
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock ScriptEngine".to_string()))?
            .as_ref()
            .and_then(|engine| engine.action_handler());

        if let Some(handler) = handler {
            if let Err(err) = handler.set_weather_visible(enabled) {
                log::warn!(
                    "Script action handler set_weather_visible failed for '{}': {}",
                    weather_type,
                    err
                );
            }
        } else {
            log::debug!(
                "No script action handler registered for weather '{}' visibility {}",
                weather_type,
                enabled
            );
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "weather_set"
    }

    fn description(&self) -> &str {
        "Sets the weather effects"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["weather_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["enabled".to_string()]
    }
}

/// Create a radar event at an explicit world position.
pub(super) struct RadarCreateEventAction;

#[async_trait]
impl ScriptAction for RadarCreateEventAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let position = get_coord_param_optional(parameters, "position").unwrap_or_else(|| {
            let x = get_float_param_optional(parameters, "x").unwrap_or(0.0) as f32;
            let y = get_float_param_optional(parameters, "y").unwrap_or(0.0) as f32;
            let z = get_float_param_optional(parameters, "z").unwrap_or(0.0) as f32;
            Coord3D::new(x, y, z)
        });
        let event_type = get_int_param(parameters, "event_type")? as i32;

        create_radar_event_for_position(position, event_type)?;

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "radar_create_event"
    }

    fn description(&self) -> &str {
        "Creates a radar event at a world position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["event_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![
            "position".to_string(),
            "x".to_string(),
            "y".to_string(),
            "z".to_string(),
        ]
    }
}

/// Create a radar event at a named object's current position.
pub(super) struct ObjectCreateRadarEventAction;

#[async_trait]
impl ScriptAction for ObjectCreateRadarEventAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let object_name = get_string_param(parameters, "object_name")
            .or_else(|_| get_string_param(parameters, "unit_name"))?;
        let event_type = get_int_param(parameters, "event_type")? as i32;

        let Some(object_id) = resolve_named_object_id(&object_name) else {
            return Ok(ScriptResult::Success(None));
        };
        let Some(position) =
            OBJECT_REGISTRY.with_object(object_id, |object| *object.get_position())
        else {
            return Ok(ScriptResult::Success(None));
        };

        create_radar_event_for_position(position, event_type)?;

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "object_create_radar_event"
    }

    fn description(&self) -> &str {
        "Creates a radar event at a named object's position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["object_name".to_string(), "event_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }
}

/// Create a radar event at a team's estimated position.
pub(super) struct TeamCreateRadarEventAction;

#[async_trait]
impl ScriptAction for TeamCreateRadarEventAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let team_name = resolve_team_name_token(&get_string_param(parameters, "team_name")?);
        let event_type = get_int_param(parameters, "event_type")? as i32;

        let Some(position) = get_team_factory()
            .lock()
            .ok()
            .and_then(|mut factory| factory.find_team(&team_name))
            .and_then(|team| {
                team.read()
                    .ok()
                    .and_then(|team| team.get_estimate_team_position())
            })
        else {
            return Ok(ScriptResult::Success(None));
        };

        create_radar_event_for_position(position, event_type)?;

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "team_create_radar_event"
    }

    fn description(&self) -> &str {
        "Creates a radar event at a team's estimated position"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["team_name".to_string(), "event_type".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Enable radar
pub(super) struct RadarEnableAction;

#[async_trait]
impl ScriptAction for RadarEnableAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Enabling radar");

        // Matches C++ ScriptActions.cpp:doRadarEnable line 2898
        // Integration with radar system (from C++):
        // 1. TheRadar->hide(false) // Make radar visible
        // 2. Updates UI to show minimap
        // 3. Enables radar functionality
        // Typically used after doRadarDisable or at mission start
        // Rust: radar_system.set_enabled(true)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_enabled(true) {
                        log::warn!(
                            "Script action handler set_radar_enabled(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "radar_enable"
    }

    fn description(&self) -> &str {
        "Enables the radar display"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Disable radar
pub(super) struct RadarDisableAction;

#[async_trait]
impl ScriptAction for RadarDisableAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Disabling radar");

        // Matches C++ ScriptActions.cpp:doRadarDisable line 2890
        // Integration with radar system (from C++):
        // 1. TheRadar->hide(true) // Hide radar from UI
        // 2. Removes minimap display
        // 3. Used for missions where radar is unavailable
        // Player loses strategic overview until re-enabled
        // Rust: radar_system.set_enabled(false)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_enabled(false) {
                        log::warn!(
                            "Script action handler set_radar_enabled(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "radar_disable"
    }

    fn description(&self) -> &str {
        "Disables the radar display"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Force radar on regardless of the player's current radar producers.
pub(super) struct RadarForceEnableAction;

#[async_trait]
impl ScriptAction for RadarForceEnableAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Force enabling radar");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_forced(true) {
                        log::warn!(
                            "Script action handler set_radar_forced(true) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "radar_force_enable"
    }

    fn description(&self) -> &str {
        "Forces the radar display on"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Return radar visibility to normal player/radar-producer rules.
pub(super) struct RadarRevertToNormalAction;

#[async_trait]
impl ScriptAction for RadarRevertToNormalAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Reverting radar to normal");

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_radar_forced(false) {
                        log::warn!(
                            "Script action handler set_radar_forced(false) failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "radar_revert_to_normal"
    }

    fn description(&self) -> &str {
        "Restores normal radar visibility rules"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Reveal Area Action - Matches C++ ScriptActionType::MAP_REVEAL_AT_WAYPOINT
pub(super) struct RevealAreaAction;

#[async_trait]
impl ScriptAction for RevealAreaAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player = get_int_param(parameters, "player")?;
        let x = get_float_param(parameters, "x")?;
        let y = get_float_param(parameters, "y")?;
        let radius = get_float_param(parameters, "radius")?;
        let permanent = get_int_param_optional(parameters, "permanent").unwrap_or(0) != 0;

        log::info!(
            "Revealing area at ({}, {}) radius {} for player {} (permanent: {})",
            x,
            y,
            radius,
            player,
            permanent
        );

        // Integration with shroud/fog of war system (from C++):
        // 1. Coord3D pos = {x, y, z}
        // 2. ThePartitionManager->revealAreaForPlayer(&pos, radius, player, permanent)
        // 3. Updates shroud visibility in circular area
        // 4. If permanent: area stays revealed
        // 5. If temporary: fog returns when no units nearby
        // Uses: ShroudManager and PartitionManager
        // Rust: shroud_manager.reveal_area(position, radius, player, permanent)

        let pos = Coord3D::new(x as f32, y as f32, 0.0);
        let player_mask = 1u32 << (player.max(0) as u32);
        if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
            shroud_mgr.do_shroud_reveal(&pos, radius as f32, player_mask);
            if !permanent {
                let current_frame = crate::helpers::TheGameLogic::get_frame();
                shroud_mgr.queue_undo_shroud_reveal(
                    &pos,
                    radius as f32,
                    player_mask,
                    0,
                    current_frame,
                );
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "reveal_area"
    }

    fn description(&self) -> &str {
        "Reveals fog of war in a circular area for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "x".to_string(),
            "y".to_string(),
            "radius".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["permanent".to_string()]
    }
}

/// Reveal Map Entire Action - Matches C++ ScriptActions::doRevealMapEntire (line 3036)
pub(super) struct RevealMapEntireAction;

#[async_trait]
impl ScriptAction for RevealMapEntireAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;

        log::info!("Revealing entire map for player '{}'", player_name);

        // Matches C++ ScriptActions.cpp:doRevealMapEntire line 3036
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. if player exists:
        //    ThePartitionManager->revealMapForPlayer(player->getPlayerIndex())
        // 3. else (for all human players):
        //    for i in 0..numPlayers:
        //      if player->isHuman():
        //        ThePartitionManager->revealMapForPlayer(i)
        // Reveals entire map shroud permanently
        // Rust: shroud_manager.reveal_map_for_player(player_index)

        let player_list = crate::player::player_list();
        let list_guard = player_list
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;

        let mut shroud_manager = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| GameLogicError::Threading("Failed to lock ShroudManager".to_string()))?;

        if let Some(player) = list_guard.find_player_by_name(&player_name) {
            let player_guard = player
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
            shroud_manager
                .reveal_map_for_player(player_guard.get_player_index() as u32)
                .map_err(GameLogicError::Configuration)?;
        } else {
            for player in list_guard.iter() {
                let player_guard = player
                    .read()
                    .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
                if player_guard.get_player_type() == PlayerType::Human {
                    shroud_manager
                        .reveal_map_for_player(player_guard.get_player_index() as u32)
                        .map_err(GameLogicError::Configuration)?;
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "reveal_map_entire"
    }

    fn description(&self) -> &str {
        "Reveals the entire map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Shroud Map Entire Action - Matches C++ ScriptActions::doShroudMapEntire (line 3090)
pub(super) struct ShroudMapEntireAction;

#[async_trait]
impl ScriptAction for ShroudMapEntireAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let player_name = get_string_param(parameters, "player")?;

        log::info!("Shrouding entire map for player '{}'", player_name);

        // Matches C++ ScriptActions.cpp:doShroudMapEntire line 3090
        // Implementation:
        // 1. Player* player = TheScriptEngine->getPlayerFromAsciiString(playerName)
        // 2. ThePartitionManager->shroudMapForPlayer(player->getPlayerIndex())
        // Re-applies fog of war to entire map
        // Used for dramatic script events or resetting vision
        // Rust: shroud_manager.shroud_map_for_player(player_index)

        let player_list = crate::player::player_list();
        let list_guard = player_list
            .read()
            .map_err(|_| GameLogicError::Threading("Failed to lock PlayerList".to_string()))?;

        let mut shroud_manager = crate::system::shroud_manager::get_shroud_manager()
            .lock()
            .map_err(|_| GameLogicError::Threading("Failed to lock ShroudManager".to_string()))?;

        if let Some(player) = list_guard.find_player_by_name(&player_name) {
            let player_guard = player
                .read()
                .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
            shroud_manager
                .shroud_map_for_player(player_guard.get_player_index() as u32)
                .map_err(GameLogicError::Configuration)?;
        } else {
            for player in list_guard.iter() {
                let player_guard = player
                    .read()
                    .map_err(|_| GameLogicError::Threading("Failed to lock Player".to_string()))?;
                if player_guard.get_player_type() == PlayerType::Human {
                    shroud_manager
                        .shroud_map_for_player(player_guard.get_player_index() as u32)
                        .map_err(GameLogicError::Configuration)?;
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "shroud_map_entire"
    }

    fn description(&self) -> &str {
        "Shrouds the entire map for a player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["player".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
