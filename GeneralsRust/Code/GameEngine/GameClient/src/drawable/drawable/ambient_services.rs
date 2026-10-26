//! Original Drawable ambient algorithm with the receiving audio service borrowed.
//! Compatibility entrypoints in basic_core remain transitional adapters.
use super::basic_core::ambient_sound_is_in_range;
use super::*;
use crate::system::TimeOfDay;
use game_engine::common::audio::{AudioManager, AudioSubmissionLocality};
use gamelogic::common::types::BodyDamageType;

/// A lexical loan of the existing application audio service. Implementations may
/// synchronize a real mixer boundary; no service handle belongs to a Drawable.
pub trait NativeAmbientAudio {
    fn restart_ambient(
        &mut self,
        drawable: &mut BasicDrawable,
        state: BodyDamageType,
        time_of_day: TimeOfDay,
        locality: &AudioSubmissionLocality,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String>;
}
impl NativeAmbientAudio for AudioManager {
    fn restart_ambient(
        &mut self,
        drawable: &mut BasicDrawable,
        state: BodyDamageType,
        time_of_day: TimeOfDay,
        locality: &AudioSubmissionLocality,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        drawable.start_ambient_sound_for_state_with_audio_policy(
            state,
            time_of_day,
            false,
            self,
            locality,
            Some(policy),
        )
    }
}

impl BasicDrawable {
    pub fn owned_ambient_audio_handle(&self) -> Option<u32> {
        self.ambient_sound_event
            .as_ref()
            .map(|event| event.get_playing_handle())
            .filter(|handle| (1000..0xFFFF_FFF0).contains(handle))
    }

    pub(crate) fn preflight_native_ambient(&self) -> Result<(), String> {
        if self.receiving_template.is_some() {
            Ok(())
        } else {
            Err("native ambient operation requires its receiving template".into())
        }
    }

    /// Retires the actual previous handle through the same receiving manager.
    pub fn stop_ambient_sound_with_audio(&mut self, audio: &mut AudioManager) {
        if let Some(event) = self.ambient_sound_event.take() {
            audio.remove_audio_event(event.get_playing_handle());
        }
    }

    /// CPP Drawable4437–4522. No ambient template/service/TOD discovery.
    /// The caller has already selected this receiving native actor.
    pub fn start_ambient_sound_for_state_with_audio(
        &mut self,
        state: BodyDamageType,
        time_of_day: TimeOfDay,
        only_if_permanent: bool,
        audio: &mut AudioManager,
        locality: &AudioSubmissionLocality,
    ) -> Result<(), String> {
        self.start_ambient_sound_for_state_with_audio_policy(
            state,
            time_of_day,
            only_if_permanent,
            audio,
            locality,
            None,
        )
    }
    pub fn start_ambient_sound_for_state_with_audio_policy(
        &mut self,
        state: BodyDamageType,
        time_of_day: TimeOfDay,
        only_if_permanent: bool,
        audio: &mut AudioManager,
        locality: &AudioSubmissionLocality,
        policy: Option<&game_engine::common::audio::script_policy::ScriptAudioPolicy>,
    ) -> Result<(), String> {
        let template = self
            .receiving_template
            .as_ref()
            .ok_or_else(|| "native ambient operation requires its receiving template".to_owned())?;
        use game_engine::common::thing::thing_template::ThingTemplateAudioType as Slot;
        let read = |state| {
            let slot = match state {
                BodyDamageType::Damaged => Slot::SoundAmbientDamaged,
                BodyDamageType::ReallyDamaged => Slot::SoundAmbientReallyDamaged,
                BodyDamageType::Rubble => Slot::SoundAmbientRubble,
                _ => Slot::SoundAmbient,
            };
            template
                .audio_event(slot)
                .filter(|event| !event.get_event_name().is_empty())
                .cloned()
        };
        // Selection is pure while the template is borrowed; stop still precedes
        // resource admission. A missing resource is ordinary C++ null behavior.
        let mut selected = if state != BodyDamageType::Rubble {
            if self.custom_sound_ambient_off {
                None
            } else if let Some(info) = self.custom_sound_ambient_dynamic_info.as_ref() {
                let mut event = AudioEventRts::new();
                event.set_event_name(info.get_audio_event_info().audio_name.clone());
                event
                    .set_audio_event_info(std::sync::Arc::new(info.get_audio_event_info().clone()));
                Some(event)
            } else {
                read(state)
            }
        } else {
            read(state)
        };
        if selected.is_none()
            && state != BodyDamageType::Pristine
            && state != BodyDamageType::Rubble
            && !self.custom_sound_ambient_off
            && self.custom_sound_ambient_dynamic_info.is_none()
        {
            selected = read(BodyDamageType::Pristine);
        }
        self.stop_ambient_sound_with_audio(audio);
        let Some(mut event) = selected else {
            return Ok(());
        };
        let Some(info) = event
            .get_audio_event_info()
            .or_else(|| audio.find_audio_event_info(event.get_event_name()))
        else {
            return Ok(());
        };
        event.set_audio_event_info(info.clone());
        if (!only_if_permanent || info.is_permanent_sound())
            && ambient_sound_is_in_range(&info, self.position, audio.get_listener_position())
        {
            event.set_drawable_id_override(self.id.0);
            event.set_time_of_day(match time_of_day {
                TimeOfDay::Morning => {
                    game_engine::common::audio::audio_event_rts::TimeOfDay::Morning
                }
                TimeOfDay::Afternoon => {
                    game_engine::common::audio::audio_event_rts::TimeOfDay::Afternoon
                }
                TimeOfDay::Evening => {
                    game_engine::common::audio::audio_event_rts::TimeOfDay::Evening
                }
                TimeOfDay::Night => game_engine::common::audio::audio_event_rts::TimeOfDay::Night,
            });
            if let Some(policy) = policy {
                event.admit_script_policy(policy);
            }
            event.set_playing_handle(audio.add_audio_event_with_locality(&event, locality));
        }
        // C++ retains a valid event even if range/permanent gating skips play.
        self.ambient_sound_event = Some(event);
        Ok(())
    }

    /// CPP Drawable1096: explicit HP observation; snapshot restamps remain inert.
    /// Object's caller suppresses this operation while under construction.
    pub fn react_to_body_damage_state_change_with_audio(
        &mut self,
        state: BodyDamageType,
        loading_map: bool,
        time_of_day: TimeOfDay,
        audio: &mut AudioManager,
        locality: &AudioSubmissionLocality,
    ) -> Result<(), String> {
        if self.receiving_template.is_none() {
            return Err("native Body visual operation requires its receiving template".into());
        }
        self.react_to_body_damage_state_change(state);
        if !loading_map {
            self.start_ambient_sound_for_state_with_audio(
                state,
                time_of_day,
                false,
                audio,
                locality,
            )?;
        }
        Ok(())
    }

    /// Original constructor gate, distinct from the HP callback's loadingMap gate.
    pub fn finish_native_constructor_ambient_with_audio(
        &mut self,
        loading_map: bool,
        in_load_game: bool,
        time_of_day: TimeOfDay,
        audio: &mut AudioManager,
        locality: &AudioSubmissionLocality,
    ) -> Result<(), String> {
        if !loading_map
            && !in_load_game
            && self.ambient_sound_enabled
            && self.ambient_sound_enabled_from_script
        {
            self.start_ambient_sound_for_state_with_audio(
                BodyDamageType::Pristine,
                time_of_day,
                false,
                audio,
                locality,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "ambient_services_tests.rs"]
mod tests;
