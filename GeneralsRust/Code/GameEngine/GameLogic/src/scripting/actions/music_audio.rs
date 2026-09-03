//! Music, speech, movie, and sound script actions
//!
//! C++: ScriptActions.cpp `doPlaySoundEffect` L329, `doPlaySoundEffectAt` L341,
//! `doMusicTrackChange` L3269, `doSpeechPlay` L2737.
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

/// Play sound action
pub(super) struct PlaySoundAction;

#[async_trait]
impl ScriptAction for PlaySoundAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let sound_name = get_string_param(parameters, "sound_name")?;
        let volume = get_float_param_optional(parameters, "volume").unwrap_or(1.0);

        log::info!("Playing sound '{}' at volume {}", sound_name, volume);

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.play_sound_effect(&sound_name) {
                        log::warn!("Script action handler play_sound_effect failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "play_sound"
    }

    fn description(&self) -> &str {
        "Plays a sound effect"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["sound_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["volume".to_string()]
    }
}

/// Play music action
pub(super) struct PlayMusicAction;

#[async_trait]
impl ScriptAction for PlayMusicAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let music_name = get_string_param(parameters, "music_name")?;
        let fade_in = get_float_param_optional(parameters, "fade_in").unwrap_or(0.0);

        log::info!(
            "Playing music '{}' with {} second fade-in",
            music_name,
            fade_in
        );

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.music_set_track(&music_name, true, fade_in > 0.0) {
                        log::warn!("Script action handler music_set_track failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "play_music"
    }

    fn description(&self) -> &str {
        "Plays background music"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["music_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["fade_in".to_string()]
    }
}

// ============================================================================
// AUDIO/VISUAL ACTIONS (7 critical actions)
// ============================================================================

/// Play sound effect
pub(super) struct SoundPlayAction;

#[async_trait]
impl ScriptAction for SoundPlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let sound_name = get_string_param(parameters, "sound_name")?;
        let volume = get_float_param_optional(parameters, "volume").unwrap_or(1.0);

        log::info!("Playing sound '{}' at volume {}", sound_name, volume);

        // Matches C++ ScriptActions.cpp:doPlaySoundEffect line 329
        // Integration with audio system (from C++):
        // 1. AudioEventRTS audioEvent(sound_name)
        // 2. audioEvent.setIsLogicalAudio(true)
        // 3. audioEvent.setPlayerIndex(localPlayer)
        // 4. TheAudio->addAudioEvent(&audioEvent)
        // Rust: audio_system.play_sound(sound_name, volume, AudioType::Sound)
        // Uses: AudioHandle and AudioType from common/audio.rs

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.play_sound_effect(&sound_name) {
                        log::warn!("Script action handler play_sound_effect failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "sound_play"
    }

    fn description(&self) -> &str {
        "Plays a sound effect"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["sound_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["volume".to_string()]
    }
}

/// Play music track
pub(super) struct MusicPlayAction;

#[async_trait]
impl ScriptAction for MusicPlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let track_name = get_string_param(parameters, "track_name")?;

        log::info!("Playing music track '{}'", track_name);

        // Matches C++ ScriptActions.cpp music system
        // Integration with audio system:
        // 1. Stop current music track (fade out)
        // 2. AudioEventRTS event(track_name)
        // 3. event.setPlayerIndex(localPlayer)
        // 4. TheAudio->addAudioEvent(&event)
        // 5. Music loops until stopped or new track played
        // Rust: audio_system.play_music(track_name)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.music_set_track(&track_name, true, true) {
                        log::warn!("Script action handler music_set_track failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "music_play"
    }

    fn description(&self) -> &str {
        "Plays a music track"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["track_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Play video
pub(super) struct MoviePlayAction;

#[async_trait]
impl ScriptAction for MoviePlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let movie_name = get_string_param(parameters, "movie_name")?;
        let fullscreen = parameters
            .get("fullscreen")
            .and_then(|v| match v {
                ScriptValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true);

        log::info!(
            "Playing movie '{}' (fullscreen: {})",
            movie_name,
            fullscreen
        );

        // Matches C++ ScriptActions.cpp:doMoviePlayFullScreen line 2707
        // Integration with video playback system:
        // 1. TheDisplay->playMovie(movie_name) // Fullscreen
        // Or: TheInGameUI->playMovie(movie_name) // In radar area
        // 2. Pauses game during playback
        // 3. Plays .bik video files
        // 4. Returns to game after video ends or skip
        // Rust: video_system.play_movie(movie_name, fullscreen)

        if fullscreen {
            if let Ok(engine_guard) = get_script_engine().read() {
                if let Some(ref script_engine) = *engine_guard {
                    if let Some(handler) = script_engine.action_handler() {
                        if let Err(err) = handler.movie_play_fullscreen(&movie_name) {
                            log::warn!(
                                "Script action handler movie_play_fullscreen failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        } else if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.display_text(&movie_name) {
                        log::warn!("Script action handler display_text failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "movie_play"
    }

    fn description(&self) -> &str {
        "Plays a video/movie"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["movie_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["fullscreen".to_string()]
    }
}

/// Play speech
pub(super) struct SpeechPlayAction;

#[async_trait]
impl ScriptAction for SpeechPlayAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let speech_name = get_string_param(parameters, "speech_name")?;
        let allow_overlap = get_bool_param_optional(parameters, "allow_overlap").unwrap_or(false);

        log::info!(
            "Playing speech '{}' (overlap: {})",
            speech_name,
            allow_overlap
        );

        // Matches C++ ScriptActions.cpp:doSpeechPlay line 2743
        // Integration with audio/EVA system (from C++):
        // 1. AudioEventRTS speech(speech_name)
        // 2. speech.setIsLogicalAudio(true)
        // 3. speech.setPlayerIndex(localPlayer)
        // 4. speech.setUninterruptable(!allowOverlap)
        // 5. TheAudio->addAudioEvent(&speech)
        // 6. May display subtitle via TheInGameUI->militarySubtitle()
        // EVA = Electronic Video Agent (voice system)

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.speech_play(&speech_name, allow_overlap) {
                        log::warn!("Script action handler speech_play failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "speech_play"
    }

    fn description(&self) -> &str {
        "Plays a speech/voice line"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["speech_name".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec!["allow_overlap".to_string()]
    }
}

/// Play Sound At Action - Matches C++ ScriptActions::doPlaySoundEffectAt (line 341)
pub(super) struct PlaySoundAtAction;

#[async_trait]
impl ScriptAction for PlaySoundAtAction {
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        let sound = get_string_param(parameters, "sound")?;
        let waypoint = get_string_param(parameters, "waypoint")?;

        log::info!("Playing sound '{}' at waypoint '{}'", sound, waypoint);

        // Matches C++ ScriptActions.cpp:doPlaySoundEffectAt line 341
        // Implementation:
        // 1. Waypoint *way = TheTerrainLogic->getWaypointByName(waypoint)
        // 2. Coord3D *pos = way->getLocation()
        // 3. AudioEventRTS *audioEvent = g_theAudio->NewAudioEventRTS(sound)
        // 4. audioEvent->setSoundPosition(pos)
        // 5. audioEvent->Execute()
        // Plays 3D positional sound at specific location
        // Volume/pan based on distance from camera
        // Rust: audio_manager.play_sound_at(sound, waypoint_position)

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
                    if let Err(err) =
                        handler.play_sound_effect_at(&sound, target.x, target.y, target.z)
                    {
                        log::warn!("Script action handler play_sound_effect_at failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "play_sound_at"
    }

    fn description(&self) -> &str {
        "Plays a sound effect at a waypoint location"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec!["sound".to_string(), "waypoint".to_string()]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}

/// Stop Music Action - Stops currently playing music
pub(super) struct StopMusicAction;

#[async_trait]
impl ScriptAction for StopMusicAction {
    async fn execute(
        &self,
        _parameters: &HashMap<String, ScriptValue>,
        _context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult> {
        log::info!("Stopping background music");

        // C++ Implementation:
        // 1. g_theAudio->StopMusic()
        // Or: TheMusicManager->stopCurrentTrack()
        // 2. Fades out current music over ~1 second
        // 3. Clears music queue
        // Used for dramatic moments or transitioning to silence
        // Often combined with PlayMusic for music changes
        // Rust: audio_manager.stop_music()

        if let Ok(engine_guard) = get_script_engine().read() {
            if let Some(ref script_engine) = *engine_guard {
                if let Some(handler) = script_engine.action_handler() {
                    if let Err(err) = handler.stop_music() {
                        log::warn!("Script action handler stop_music failed: {}", err);
                    }
                }
            }
        }

        Ok(ScriptResult::Success(None))
    }

    fn name(&self) -> &str {
        "stop_music"
    }

    fn description(&self) -> &str {
        "Stops the currently playing background music"
    }

    fn required_parameters(&self) -> Vec<String> {
        vec![]
    }

    fn optional_parameters(&self) -> Vec<String> {
        vec![]
    }
}
