//! Camera and UI/text script actions
//!
//! C++: ScriptActions.cpp camera suite L442–948, `doLetterBoxMode` L3747,
//! `doZoomCamera` L791, `doDisplayText` L2521.
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

/// Move camera action
pub(super) struct MoveCameraAction;

#[async_trait]
impl ScriptAction for MoveCameraAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let x = get_float_param(parameters, "x")? as f32;
        let y = get_float_param(parameters, "y")? as f32;
        let z = get_float_param_optional(parameters, "z").unwrap_or(0.0) as f32;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0) as f32;

        log::info!(
            "Moving camera to ({}, {}, {}) over {} seconds",
            x,
            y,
            z,
            duration
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_to(x, y, z, duration, 0.0, 0.0, 0.0) {
                        log::warn!("Script action handler move_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "move_camera"
    }

    fn description(&self) -> &str {
        "Moves the camera to the specified location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["x".to_string(), "y".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["z".to_string(), "duration".to_string()]
    }
}

/// Show text message action
pub(super) struct ShowTextMessageAction;

#[async_trait]
impl ScriptAction for ShowTextMessageAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let message = get_string_param(parameters, "message")?;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(5.0);

        log::info!("Showing message: '{}' for {} seconds", message, duration);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.display_text(&message) {
                        log::warn!("Script action handler display_text failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "show_text_message"
    }

    fn description(&self) -> &str {
        "Displays a text message to the player"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["message".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Move camera
pub(super) struct CameraMoveToWaypointAction;

#[async_trait]
impl ScriptAction for CameraMoveToWaypointAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0);

        log::info!(
            "Moving camera to waypoint '{}' over {} seconds",
            waypoint,
            duration
        );

        // Integration with camera system:
        // Smoothly moves camera to position
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D destination = *way->getLocation()
        // 3. TheTacticalView->moveToPosition(destination, duration)
        // 4. If duration > 0: smooth interpolated movement
        // 5. If duration == 0: instant jump
        // Camera system handles easing and interpolation

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            })
            .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.move_camera_to(
                        target.x,
                        target.y,
                        target.z,
                        duration as f32,
                        0.0,
                        0.0,
                        0.0,
                    ) {
                        log::warn!("Script action handler move_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_move_to_waypoint"
    }

    fn description(&self) -> &str {
        "Moves the camera to a specific waypoint"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Camera follows unit
pub(super) struct CameraTrackNamedAction;

#[async_trait]
impl ScriptAction for CameraTrackNamedAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        // Wave 295: empty dual-world → Success(None).
        if dual_world_registry_unavailable() {
            return Ok(ScriptResult::Success(None));
        }

        let unit_name = get_string_param(parameters, "unit_name")?;
        let snap = parameters
            .get("snap")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(false);

        log::info!(
            "Camera tracking named unit '{}' (snap: {})",
            unit_name,
            snap
        );

        // Matches C++ ScriptActions.cpp:doCameraFollowNamed line 444
        // Integration with camera system (from C++):
        // 1. Object *theObj = TheScriptEngine->getUnitNamed(unit_name)
        // 2. TheTacticalView->setCameraLock(theObj->getID())
        // 3. if (snap) TheTacticalView->snapToCameraLock() // Instant
        // 4. TheTacticalView->setSnapMode(View::LOCK_FOLLOW, 0.0f)
        // 5. Camera continuously follows unit movement
        // Used for cinematic sequences and important unit tracking

        let tracker = crate::scripting::engine::get_named_object_tracker();
        let mut object_id = tracker.get_object_id(&unit_name).ok().flatten();

        if object_id.is_none() {
            let lower = unit_name.to_ascii_lowercase();
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

        if let Some(object_id) = object_id {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.camera_follow_object(object_id, snap) {
                            log::warn!(
                                "Script action handler camera_follow_object failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else {
            log::warn!("Camera track failed: unit '{}' not found", unit_name);
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_track_named"
    }

    fn description(&self) -> &str {
        "Commands the camera to track a named unit"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["unit_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["snap".to_string()]
    }
}

/// Start letterbox
pub(super) struct CameraLetterboxBeginAction;

#[async_trait]
impl ScriptAction for CameraLetterboxBeginAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Starting letterbox mode");

        // Integration with camera/UI system:
        // Letterbox mode adds black bars top/bottom for cinematic effect
        // 1. TheDisplay->setLetterboxMode(true)
        // 2. Animates black bars expanding from screen edges
        // 3. Typically used with camera sequences
        // 4. Hides UI elements for immersive experience
        // Common in campaign briefings and cutscenes

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_begin() {
                        log::warn!(
                            "Script action handler camera_letterbox_begin failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_letterbox_begin"
    }

    fn description(&self) -> &str {
        "Begins letterbox (cinematic) mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// End letterbox
pub(super) struct CameraLetterboxEndAction;

#[async_trait]
impl ScriptAction for CameraLetterboxEndAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Ending letterbox mode");

        // Integration with camera/UI system:
        // Removes letterbox, returns to normal view
        // 1. TheDisplay->setLetterboxMode(false)
        // 2. Animates black bars retracting
        // 3. Restores UI elements
        // 4. Returns to gameplay view

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_end() {
                        log::warn!("Script action handler camera_letterbox_end failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_letterbox_end"
    }

    fn description(&self) -> &str {
        "Ends letterbox (cinematic) mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Set zoom
pub(super) struct CameraSetFinalZoomAction;

#[async_trait]
impl ScriptAction for CameraSetFinalZoomAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let zoom = get_float_param(parameters, "zoom")? as f32;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0) as f32;

        log::info!("Setting camera zoom to {} over {} seconds", zoom, duration);

        // Integration with camera system:
        // Sets camera zoom level (height above terrain)
        // 1. TheTacticalView->setFinalZoom(zoom)
        // 2. If duration > 0: interpolate zoom over time
        // 3. If duration == 0: instant zoom change
        // 4. Zoom values typically 100-1000 (game units)
        // Higher = further away, lower = closer to ground

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_camera_zoom(zoom, duration) {
                        log::warn!("Script action handler set_camera_zoom failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_set_final_zoom"
    }

    fn description(&self) -> &str {
        "Sets the camera's final zoom level"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["zoom".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Show text message
pub(super) struct TextDisplayAction;

#[async_trait]
impl ScriptAction for TextDisplayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let message = get_string_param(parameters, "message")?;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(5.0);

        log::info!("Displaying text: '{}' for {} seconds", message, duration);

        // Matches C++ ScriptActions.cpp:doDisplayText line 2523
        // Integration with UI system:
        // 1. TheInGameUI->message(message)
        // 2. Text appears in UI message area
        // 3. Auto-fades after duration
        // 4. May support localization via TheGameText->fetch()
        // Rust: ui_system.display_message(message, duration)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.display_text(&message) {
                        log::warn!("Script action handler display_text failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "text_display"
    }

    fn description(&self) -> &str {
        "Displays a text message on screen"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["message".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Camera Zoom Action - Matches C++ ScriptActionType::ZOOM_CAMERA
pub(super) struct CameraZoomAction;

#[async_trait]
impl ScriptAction for CameraZoomAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let zoom_level = get_float_param(parameters, "zoom_level")? as f32;
        let duration = get_float_param_optional(parameters, "duration").unwrap_or(0.0) as f32;

        log::info!(
            "Zooming camera to level {} over {} seconds",
            zoom_level,
            duration
        );

        // Matches C++ ScriptActions.cpp:doZoomCamera
        // Integration with camera system (from C++):
        // 1. TheDisplay->Set_Zoom(zoom_level, duration)
        // Or: TheTacticalView->setZoom(zoom_level, duration)
        // 2. Interpolates zoom over duration
        // 3. Same as CameraSetFinalZoomAction
        // Rust: camera.set_zoom(zoom_level, duration)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.set_camera_zoom(zoom_level, duration) {
                        log::warn!("Script action handler set_camera_zoom failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "camera_zoom"
    }

    fn description(&self) -> &str {
        "Changes camera zoom level"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["zoom_level".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["duration".to_string()]
    }
}

/// Snap Camera Action - Instant camera movement (no animation)
pub(super) struct SnapCameraAction;

#[async_trait]
impl ScriptAction for SnapCameraAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Snapping camera to waypoint '{}'", waypoint);

        // Matches C++ camera snap functionality
        // Implementation:
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D destination = *way->getLocation()
        // 3. TheTacticalView->moveCameraTo(&destination, 0, 0, true, 0.0f, 0.0f)
        // duration=0 means instant snap (no animation)
        // Used for cutscenes and quick transitions
        // Rust: camera.snap_to_position(waypoint_position)

        let waypoint_ascii = AsciiString::from(waypoint.as_str());
        let target = get_terrain_logic().read().ok().and_then(|terrain| {
            terrain
                .get_waypoint_by_name(&waypoint_ascii)
                .map(|w| *w.get_location())
        });

        let Some(target) = target else {
            log::warn!("Snap camera failed: waypoint '{}' not found", waypoint);
            return Ok(ScriptResult::Success(None));
        };

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) =
                        handler.reset_camera_to(target.x, target.y, target.z, 0.0, 0.0, 0.0)
                    {
                        log::warn!("Script action handler reset_camera_to failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "snap_camera"
    }

    fn description(&self) -> &str {
        "Instantly moves camera to waypoint (no animation)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Letter Box Begin Action - Start cinematic letterbox mode
pub(super) struct LetterBoxBeginAction;

#[async_trait]
impl ScriptAction for LetterBoxBeginAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Starting letterbox cinematic mode");

        // C++ Implementation:
        // Adds black bars to top/bottom of screen for cinematic effect
        // 1. TheInGameUI->setLetterboxMode(true)
        // 2. Animates black bars expanding from edges
        // 3. Hides UI elements (command bar, minimap, etc)
        // 4. Often combined with camera movements
        // Typical animation: 0.5-1.0 seconds expand
        // Rust: ui_manager.enable_letterbox(true)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_begin() {
                        log::warn!(
                            "Script action handler camera_letterbox_begin failed: {}",
                            err
                        );
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "letterbox_begin"
    }

    fn description(&self) -> &str {
        "Starts cinematic letterbox mode (black bars)"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Letter Box End Action - End cinematic letterbox mode
pub(super) struct LetterBoxEndAction;

#[async_trait]
impl ScriptAction for LetterBoxEndAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Ending letterbox cinematic mode");

        // C++ Implementation:
        // Removes letterbox black bars
        // 1. TheInGameUI->setLetterboxMode(false)
        // 2. Animates black bars retracting to edges
        // 3. Restores UI elements
        // 4. Returns to normal gameplay view
        // Typical animation: 0.5-1.0 seconds retract
        // Rust: ui_manager.enable_letterbox(false)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.camera_letterbox_end() {
                        log::warn!("Script action handler camera_letterbox_end failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "letterbox_end"
    }

    fn description(&self) -> &str {
        "Ends cinematic letterbox mode"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
