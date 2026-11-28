//! Authored audiovisual effects and callback error policy from ScriptActions.cpp.

use super::interpreter::ScriptInterpreter;
use super::program::{ScriptAction, ScriptActionType};
use super::world::{HostScriptObjectSoundRequest, ScriptAudioRequest, ScriptDisplayRequest};
use super::{ScriptActionResult, ScriptError};
use crate::GameLogicResult;
use game_engine::common::audio::AudioAffect;
use glam::Vec3;

/// The receiving game's synchronous presentation and raw map lookups.
pub trait ScriptMediaWorld {
    fn display(&mut self, request: ScriptDisplayRequest<'_>) -> GameLogicResult<()>;
    fn military_caption_disabled(&self) -> bool;
    fn waypoint_position(&mut self, name: &str) -> GameLogicResult<Option<Vec3>>;
    fn sound_effect(&mut self, name: &str, position: Option<Vec3>) -> GameLogicResult<()>;
    fn object_sound(&mut self, request: HostScriptObjectSoundRequest) -> GameLogicResult<()>;
    fn speech(&mut self, name: &str, allow_overlap: bool) -> GameLogicResult<()>;
    fn music(&mut self, track: &str, fade_out: bool, fade_in: bool) -> GameLogicResult<()>;
    fn movie(&mut self, name: &str, fullscreen: bool) -> GameLogicResult<()>;
    fn audio(&mut self, request: ScriptAudioRequest<'_>) -> GameLogicResult<()>;
}

pub fn execute_media_action(
    action: &ScriptAction,
    interpreter: &ScriptInterpreter,
    world: &mut impl ScriptMediaWorld,
) -> Result<ScriptActionResult, ScriptError> {
    use ScriptActionType::*;
    let parameter = |index| {
        action
            .get_parameter(index)
            .ok_or_else(|| ScriptError::missing_parameter(index))
    };
    let name = |index| parameter(index).map(|value| value.get_string());
    let real = |index| parameter(index).map(|value| value.get_real());
    let flag = |index, default| {
        action
            .get_parameter(index)
            .map_or(default, |v| v.get_int() != 0)
    };
    let execution_error =
        |error: crate::GameLogicError| ScriptError::ExecutionFailed(error.to_string());
    let kind = action.get_action_type();

    let audio_request = match kind {
        DisplayText | DisplayCinematicText | ShowMilitaryCaption => {
            let text = name(0)?;
            let request = match kind {
                DisplayText => ScriptDisplayRequest::Text(text),
                DisplayCinematicText => ScriptDisplayRequest::Cinematic {
                    text,
                    font: action
                        .get_parameter(1)
                        .map_or("Default", |p| p.get_string()),
                    duration_seconds: action.get_parameter(2).map_or(0, |p| p.get_int()),
                },
                _ => {
                    let duration_ms = parameter(1)?.get_int();
                    ScriptDisplayRequest::MilitaryCaption {
                        text,
                        duration_ms: if world.military_caption_disabled() {
                            1
                        } else {
                            duration_ms
                        },
                    }
                }
            };
            warn_if_failed(kind, world.display(request));
            return Ok(ScriptActionResult::Success);
        }
        PlaySoundEffect | PlaySoundEffectAt => {
            let sound = name(0)?;
            let position = if kind == PlaySoundEffectAt {
                let waypoint = name(1)?;
                let position = match world.waypoint_position(waypoint) {
                    Ok(Some(position)) => position,
                    Ok(None) => {
                        log::warn!(
                            "Script sound effect '{sound}': waypoint '{waypoint}' not found"
                        );
                        return Ok(ScriptActionResult::Success);
                    }
                    Err(error) => {
                        warn_if_failed(kind, Err(error));
                        return Ok(ScriptActionResult::Success);
                    }
                };
                Some(position)
            } else {
                None
            };
            warn_if_failed(kind, world.sound_effect(sound, position));
            return Ok(ScriptActionResult::Success);
        }
        SoundPlayNamed | EnableObjectSound | DisableObjectSound => {
            let request = if kind == SoundPlayNamed {
                HostScriptObjectSoundRequest::PlayNamed {
                    sound: name(0)?.to_owned(),
                    unit: name(1)?.to_owned(),
                }
            } else {
                HostScriptObjectSoundRequest::Enable {
                    unit: name(0)?.to_owned(),
                    enable: kind == EnableObjectSound,
                }
            };
            world.object_sound(request).map_err(execution_error)?;
            return Ok(ScriptActionResult::Success);
        }
        SpeechPlay => {
            world
                .speech(name(0)?, flag(1, false))
                .map_err(execution_error)?;
            return Ok(ScriptActionResult::Success);
        }
        MusicSetTrack => {
            let track = name(0)?;
            warn_if_failed(kind, world.music(track, flag(1, true), flag(2, true)));
            interpreter.set_current_track_name(track.to_owned());
            return Ok(ScriptActionResult::Success);
        }
        MoviePlayFullscreen | MoviePlayRadar => {
            warn_if_failed(kind, world.movie(name(0)?, kind == MoviePlayFullscreen));
            return Ok(ScriptActionResult::Success);
        }
        SuspendBackgroundSounds => ScriptAudioRequest::Pause(AudioAffect::Sound),
        ResumeBackgroundSounds => ScriptAudioRequest::Resume(AudioAffect::Sound),
        SoundAmbientPause => ScriptAudioRequest::Pause(AudioAffect::Sound3D),
        SoundAmbientResume => ScriptAudioRequest::Resume(AudioAffect::Sound3D),
        MusicSetVolume | SoundSetVolume | SpeechSetVolume => ScriptAudioRequest::SetVolume {
            volume: (real(0)? / 100.0).clamp(0.0, 1.0),
            affect: match kind {
                MusicSetVolume => AudioAffect::Music,
                SoundSetVolume => AudioAffect::SoundEffects,
                _ => AudioAffect::Speech,
            },
        },
        // GameAudio.cpp setAudioEventEnabled delegates to the same override
        // table. Empty name resets all entries and -1 removes a named entry.
        SoundDisableType | SoundEnableType | AudioRestoreVolumeType => {
            ScriptAudioRequest::NamedVolume {
                name: name(0)?,
                volume: if kind == SoundDisableType { 0.0 } else { -1.0 },
            }
        }
        SoundEnableAll | AudioRestoreVolumeAllType => ScriptAudioRequest::NamedVolume {
            name: "",
            volume: -1.0,
        },
        AudioOverrideVolumeType => ScriptAudioRequest::NamedVolume {
            name: name(0)?,
            volume: real(1)? / 100.0,
        },
        SoundRemoveAllDisabled => ScriptAudioRequest::RemoveDisabled,
        SoundRemoveType => ScriptAudioRequest::RemoveNamed(name(0)?),
        _ => {
            return Err(ScriptError::ExecutionFailed(format!(
                "{kind:?} is not a media action"
            )));
        }
    };
    world.audio(audio_request).map_err(execution_error)?;
    Ok(ScriptActionResult::Success)
}

fn warn_if_failed(kind: ScriptActionType, result: GameLogicResult<()>) {
    if let Err(error) = result {
        log::warn!("Script media action {kind:?} failed: {error}");
    }
}
