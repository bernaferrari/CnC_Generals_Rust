//! Deliver simulation media actions through the receiving driver or installed callback.

use super::ScriptActionDispatcher;
use crate::common::audio::AudioEventRts;
use crate::helpers::TheAudio;
use crate::scripting::engine::ScriptEngine;
use game_engine::common::audio::AudioAffect;
use generals_sim::GameLogicResult;
use generals_sim::scripting::driver::ScriptExecutionDriver;
use generals_sim::scripting::media_actions::{ScriptMediaWorld, execute_media_action};
use generals_sim::scripting::presentation::ScriptActionHandler;
use generals_sim::scripting::program::ScriptAction;
use generals_sim::scripting::world::{
    HostScriptObjectSoundRequest, ScriptAudioRequest, ScriptDisplayRequest, ScriptObjectEffect,
    ScriptWorldRequest,
};
use generals_sim::scripting::{ScriptActionResult, ScriptError};
use glam::Vec3;
use std::sync::Arc;

impl ScriptActionDispatcher<'_> {
    pub(super) fn execute_media_action(
        &self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        let engine = self.context.borrowed_engine();
        execute_media_action(
            action,
            engine.interpreter(),
            &mut MediaWorld { engine, driver },
        )
    }
}

struct MediaWorld<'engine, 'driver> {
    engine: &'engine ScriptEngine,
    driver: &'driver mut dyn ScriptExecutionDriver<ScriptEngine>,
}

impl MediaWorld<'_, '_> {
    fn handler(&self) -> Option<Arc<dyn ScriptActionHandler>> {
        self.engine
            .with_execution_read(ScriptEngine::action_handler)
            .flatten()
    }

    fn standalone_audio(&self) -> Option<TheAudio> {
        self.engine
            .owning_world_services()
            .map(|world| TheAudio::of(&world))
    }

    fn named_sound_source(&self, name: &str) -> Option<u32> {
        self.engine
            .cached_named_object_id(name)
            .or_else(|| self.engine.host_query().host_script_named_unit_id(name))
    }
}

impl ScriptMediaWorld for MediaWorld<'_, '_> {
    fn display(&mut self, request: ScriptDisplayRequest<'_>) -> GameLogicResult<()> {
        if let Some(result) = self.driver.display(request) {
            return result;
        }
        self.handler().map_or(Ok(()), |handler| match request {
            ScriptDisplayRequest::Text(text) => handler.display_text(text),
            ScriptDisplayRequest::Cinematic {
                text,
                font,
                duration_seconds,
            } => handler.display_cinematic_text(text, font, duration_seconds),
            ScriptDisplayRequest::MilitaryCaption { text, duration_ms } => {
                handler.military_caption(text, duration_ms)
            }
        })
    }

    fn military_caption_disabled(&self) -> bool {
        self.engine.military_caption_disabled()
    }

    fn waypoint_position(&mut self, name: &str) -> GameLogicResult<Option<Vec3>> {
        self.driver
            .script_waypoint_position(name)
            .unwrap_or_else(|| Ok(self.engine.authored_waypoint_position(name)))
    }

    fn sound_effect(&mut self, name: &str, position: Option<Vec3>) -> GameLogicResult<()> {
        self.handler().map_or(Ok(()), |handler| match position {
            Some(position) => {
                handler.play_sound_effect_at(name, position.x, position.y, position.z)
            }
            None => handler.play_sound_effect(name),
        })
    }

    fn object_sound(&mut self, request: HostScriptObjectSoundRequest) -> GameLogicResult<()> {
        let (object, calling, condition) = self
            .engine
            .with_execution_read(|engine| {
                (
                    engine.script_object_id(),
                    engine.get_calling_team_id(),
                    engine.get_condition_team_id(),
                )
            })
            .unwrap_or_default();
        if let Some(result) = self.driver.object_effect(
            ScriptObjectEffect::Sound(&request),
            object,
            calling,
            condition,
        ) {
            return result;
        }

        match request {
            HostScriptObjectSoundRequest::PlayNamed { sound, unit } => {
                if let Some(handler) = self.handler() {
                    if let Err(error) = handler.sound_play_named(&sound, &unit) {
                        log::warn!("Script action handler sound_play_named failed: {error}");
                    }
                } else if let (Some(id), Some(audio)) =
                    (self.named_sound_source(&unit), self.standalone_audio())
                {
                    let mut event = AudioEventRts::new(sound.as_str());
                    event.set_object_id(id);
                    event.set_is_logical_audio(true);
                    let _ = audio.add_audio_event(&event);
                }
            }
            HostScriptObjectSoundRequest::Enable { unit, enable } => {
                if let Some(handler) = self.handler() {
                    if let Err(error) = handler.enable_object_sound(&unit, enable) {
                        log::warn!(
                            "Script action handler enable_object_sound({enable}) failed: {error}"
                        );
                    }
                }
                if let Some(drawable) = self
                    .named_sound_source(&unit)
                    .and_then(|id| self.engine.standalone_object_drawable(id))
                {
                    if let Ok(mut drawable) = drawable.write() {
                        drawable.enable_ambient_sound_from_script(enable);
                    }
                }
            }
        }
        Ok(())
    }

    fn speech(&mut self, name: &str, allow_overlap: bool) -> GameLogicResult<()> {
        self.driver
            .speech_play(name, allow_overlap)
            .unwrap_or_else(|| {
                self.handler()
                    .map_or(Ok(()), |handler| handler.speech_play(name, allow_overlap))
            })
    }

    fn music(&mut self, track: &str, fade_out: bool, fade_in: bool) -> GameLogicResult<()> {
        self.driver
            .world(ScriptWorldRequest::Music {
                track,
                fade_out,
                fade_in,
            })
            .unwrap_or_else(|| {
                self.handler().map_or(Ok(()), |handler| {
                    handler.music_set_track(track, fade_out, fade_in)
                })
            })
    }

    fn movie(&mut self, name: &str, fullscreen: bool) -> GameLogicResult<()> {
        if fullscreen {
            if let Some(result) = self.driver.world(ScriptWorldRequest::MovieFullscreen(name)) {
                return result;
            }
        }
        self.handler().map_or(Ok(()), |handler| {
            if fullscreen {
                handler.movie_play_fullscreen(name)
            } else {
                handler.movie_play_radar(name)
            }
        })
    }

    fn audio(&mut self, request: ScriptAudioRequest<'_>) -> GameLogicResult<()> {
        if let Some(result) = self.driver.audio(request) {
            return result;
        }
        let Some(audio) = self.standalone_audio() else {
            return Ok(());
        };
        match request {
            ScriptAudioRequest::SetVolume { volume, affect }
                if affect == AudioAffect::SoundEffects =>
            {
                audio.set_volume(volume, AudioAffect::Sound);
                audio.set_volume(volume, AudioAffect::Sound3D);
            }
            ScriptAudioRequest::SetVolume { volume, affect } => audio.set_volume(volume, affect),
            ScriptAudioRequest::NamedVolume { name, volume } => {
                audio.set_audio_event_volume_override(name, volume)
            }
            ScriptAudioRequest::RemoveNamed(name) => audio.remove_audio_event_by_name(name),
            ScriptAudioRequest::RemoveDisabled => audio.remove_disabled_events(),
            ScriptAudioRequest::Pause(affect) => audio.pause_audio(affect),
            ScriptAudioRequest::Resume(affect) => audio.resume_audio(affect),
            ScriptAudioRequest::StopMusic { .. } => {
                return self
                    .handler()
                    .map_or(Ok(()), |handler| handler.stop_music());
            }
        }
        Ok(())
    }
}
