//! Remaining script conditions (time, media, bridges, mission).

use super::helpers::{
    compare_f64, compare_i64, dual_world_registry_unavailable, event_type_from_name,
    get_player_arc, get_str_param, get_str_param_optional, lookup_named_object_id,
    parse_nested_condition, parse_object_status_mask, perform_comparison, with_script_engine_mut,
};
use super::{ConditionRegistry, ScriptCondition, ScriptContext, ScriptValue};
use crate::common::{Coord3D, KindOf, LOGICFRAMES_PER_SECOND, Relationship};
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, TheVictoryConditions};
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

/// Game time condition
pub(super) struct GameTimeCondition;

#[async_trait]
impl ScriptCondition for GameTimeCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let comparison = crate::scripting::actions::get_string_param(parameters, "comparison")?;
        let time = crate::scripting::actions::get_float_param(parameters, "time")?;

        let game_time = context.game_time.as_secs_f64();

        let result = match comparison.as_str() {
            "greater" => game_time > time,
            "less" => game_time < time,
            "equal" => (game_time - time).abs() < 1.0, // 1 second tolerance
            "greater_equal" => game_time >= time,
            "less_equal" => game_time <= time,
            _ => {
                return Err(GameLogicError::Configuration(format!(
                    "Invalid comparison operator: {}",
                    comparison
                )));
            }
        };

        Ok(result)
    }

    fn name(&self) -> &str {
        "game_time"
    }

    fn description(&self) -> &str {
        "Checks the current game time"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["comparison".to_string(), "time".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Timer condition
pub(super) struct TimerCondition;

#[async_trait]
impl ScriptCondition for TimerCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let timer_name = crate::scripting::actions::get_string_param(parameters, "timer_name")?;
        let comparison = crate::scripting::actions::get_string_param(parameters, "comparison")?;
        let time = crate::scripting::actions::get_float_param(parameters, "time")?;

        log::debug!("Checking timer '{}' {} {}", timer_name, comparison, time);

        let timer_frames_left = get_script_engine()
            .read()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .and_then(|engine| engine.get_counter(&timer_name).map(|c| c.value))
            })
            .unwrap_or(0);
        let timer_value = timer_frames_left.max(0) as f64 / LOGICFRAMES_PER_SECOND as f64;

        compare_f64(timer_value, comparison.as_str(), time)
    }

    fn name(&self) -> &str {
        "timer"
    }

    fn description(&self) -> &str {
        "Checks a named timer value"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![
            "timer_name".to_string(),
            "comparison".to_string(),
            "time".to_string(),
        ]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Event occurred condition
pub(super) struct EventOccurredCondition;

#[async_trait]
impl ScriptCondition for EventOccurredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let event_name = crate::scripting::actions::get_string_param(parameters, "event_name")?;

        log::debug!("Checking if event '{}' occurred", event_name);

        let event_type = event_type_from_name(event_name.as_str());
        let filter = EventFilter {
            event_types: vec![event_type],
            player_id: None,
            object_id: None,
            parameter_filters: HashMap::new(),
            min_priority: crate::scripting::ScriptPriority::Low,
        };

        let event_manager = get_event_manager();
        Ok(!event_manager.query_history(&filter, 1).await?.is_empty())
    }

    fn name(&self) -> &str {
        "event_occurred"
    }

    fn description(&self) -> &str {
        "Checks if a specific event has occurred"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["event_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// BRIDGE_REPAIRED - evaluateBridgeRepaired
//-------------------------------------------------------------------------------------------------
pub(super) struct BridgeRepairedCondition;

#[async_trait]
impl ScriptCondition for BridgeRepairedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let bridge_name = get_str_param(parameters, "bridge_name")?;
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(crate::scripting::host_bridge_repaired(&bridge_name));
        }
        let object_id = match lookup_named_object_id(&bridge_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        let terrain = get_terrain_logic()
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read terrain: {}", e)))?;
        if !terrain.bridge_damage_states_changed() {
            return Ok(false);
        }
        Ok(terrain.is_bridge_repaired(object_id))
    }

    fn name(&self) -> &str {
        "bridge_repaired"
    }
    fn description(&self) -> &str {
        "Checks if a named bridge has been repaired (C++ BRIDGE_REPAIRED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["bridge_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// BRIDGE_BROKEN - evaluateBridgeBroken
//-------------------------------------------------------------------------------------------------
pub(super) struct BridgeBrokenCondition;

#[async_trait]
impl ScriptCondition for BridgeBrokenCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let bridge_name = get_str_param(parameters, "bridge_name")?;
        if crate::object::registry::OBJECT_REGISTRY.is_empty() {
            return Ok(crate::scripting::host_bridge_broken(&bridge_name));
        }
        let object_id = match lookup_named_object_id(&bridge_name)? {
            Some(id) => id,
            None => return Ok(false),
        };
        let terrain = get_terrain_logic()
            .read()
            .map_err(|e| GameLogicError::Threading(format!("Failed to read terrain: {}", e)))?;
        if !terrain.bridge_damage_states_changed() {
            return Ok(false);
        }
        Ok(terrain.is_bridge_broken(object_id))
    }

    fn name(&self) -> &str {
        "bridge_broken"
    }
    fn description(&self) -> &str {
        "Checks if a named bridge is broken (C++ BRIDGE_BROKEN)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["bridge_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// HAS_FINISHED_VIDEO
//-------------------------------------------------------------------------------------------------
pub(super) struct VideoCompletedCondition;

#[async_trait]
impl ScriptCondition for VideoCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let video_name = get_str_param(parameters, "video")?;
        Ok(
            with_script_engine_mut(|engine| engine.is_video_complete(&video_name, true))
                .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "video_completed"
    }
    fn description(&self) -> &str {
        "Checks if video has finished playing (C++ HAS_FINISHED_VIDEO)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["video".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// HAS_FINISHED_SPEECH
//-------------------------------------------------------------------------------------------------
pub(super) struct SpeechCompletedCondition;

#[async_trait]
impl ScriptCondition for SpeechCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let speech_name = get_str_param(parameters, "speech")?;
        Ok(
            with_script_engine_mut(|engine| engine.is_speech_complete(&speech_name, true))
                .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "speech_completed"
    }
    fn description(&self) -> &str {
        "Checks if speech has finished (C++ HAS_FINISHED_SPEECH)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["speech".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// HAS_FINISHED_AUDIO
//-------------------------------------------------------------------------------------------------
pub(super) struct AudioCompletedCondition;

#[async_trait]
impl ScriptCondition for AudioCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let audio_name = get_str_param(parameters, "audio")?;
        Ok(
            with_script_engine_mut(|engine| engine.is_audio_complete(&audio_name, true))
                .unwrap_or(false),
        )
    }

    fn name(&self) -> &str {
        "audio_completed"
    }
    fn description(&self) -> &str {
        "Checks if audio has finished (C++ HAS_FINISHED_AUDIO)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["audio".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MUSIC_TRACK_HAS_COMPLETED
//-------------------------------------------------------------------------------------------------
pub(super) struct MusicTrackCompletedCondition;

#[async_trait]
impl ScriptCondition for MusicTrackCompletedCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let track = get_str_param(parameters, "track")?;
        let param = parameters
            .get("param")
            .and_then(|value| match value {
                ScriptValue::Int(value) => Some(*value),
                _ => None,
            })
            .unwrap_or(0);

        Ok(TheAudio::get()
            .map(|audio| audio.has_music_track_completed(&track, param as i32))
            .unwrap_or(false))
    }

    fn name(&self) -> &str {
        "music_track_completed"
    }
    fn description(&self) -> &str {
        "Checks if music track has completed (C++ MUSIC_TRACK_HAS_COMPLETED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["track".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// CAMERA_MOVEMENT_FINISHED
//-------------------------------------------------------------------------------------------------
pub(super) struct CameraMovementFinishedCondition;

#[async_trait]
impl ScriptCondition for CameraMovementFinishedCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(engine) = engine_guard.as_ref() {
                if let Some(handler) = engine.action_handler() {
                    return Ok(handler.is_camera_movement_finished());
                }
            }
        }
        Ok(false)
    }

    fn name(&self) -> &str {
        "camera_movement_finished"
    }
    fn description(&self) -> &str {
        "Checks if camera movement finished (C++ CAMERA_MOVEMENT_FINISHED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// MISSION_ATTEMPTS - Matches C++ ScriptConditions::evaluateMissionAttempts (line 1208)
// C++ returns false unconditionally; the player lookup is commented out.
//-------------------------------------------------------------------------------------------------
pub(super) struct MissionAttemptsCondition;

#[async_trait]
impl ScriptCondition for MissionAttemptsCondition {
    async fn evaluate(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        Ok(false)
    }

    fn name(&self) -> &str {
        "mission_attempts"
    }
    fn description(&self) -> &str {
        "Checks mission attempts (C++ MISSION_ATTEMPTS — always false)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec![
            "player".to_string(),
            "comparison".to_string(),
            "attempts".to_string(),
        ]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

//-------------------------------------------------------------------------------------------------
// TIMER_EXPIRED - C++ ScriptEngine::evaluateTimer
// Checks if a countdown timer counter has expired (value < 1).
//-------------------------------------------------------------------------------------------------
pub(super) struct TimerExpiredCondition;

#[async_trait]
impl ScriptCondition for TimerExpiredCondition {
    async fn evaluate(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<bool> {
        let timer_name = get_str_param(parameters, "timer_name")
            .or_else(|_| get_str_param(parameters, "timer"))?;

        let expired = get_script_engine()
            .read()
            .ok()
            .and_then(|guard| {
                guard.as_ref().and_then(|engine| {
                    engine.get_counter(&timer_name).map(|c| {
                        // C++: timers decrement down to -1; expired when value < 1
                        c.is_countdown_timer && c.value < 1
                    })
                })
            })
            .unwrap_or(false);

        Ok(expired)
    }

    fn name(&self) -> &str {
        "timer_expired"
    }
    fn description(&self) -> &str {
        "If a named timer has expired (C++ TIMER_EXPIRED)"
    }
    fn required_parameters(&self) -> Vec<String> {
        vec!["timer_name".to_string()]
    }
    fn optional_parameters(&self) -> Vec<String> {
        vec!["timer".to_string()]
    }
}

pub(super) fn register_leftover_conditions(registry: &mut ConditionRegistry) {
    registry.register_condition(Box::new(GameTimeCondition));
    registry.register_condition(Box::new(TimerCondition));
    registry.register_condition(Box::new(EventOccurredCondition));
    registry.register_condition(Box::new(BridgeRepairedCondition));
    registry.register_condition(Box::new(BridgeBrokenCondition));
    registry.register_condition(Box::new(VideoCompletedCondition));
    registry.register_condition(Box::new(SpeechCompletedCondition));
    registry.register_condition(Box::new(AudioCompletedCondition));
    registry.register_condition(Box::new(MusicTrackCompletedCondition));
    registry.register_condition(Box::new(CameraMovementFinishedCondition));
    registry.register_condition(Box::new(MissionAttemptsCondition));
    registry.register_condition(Box::new(TimerExpiredCondition));
}
