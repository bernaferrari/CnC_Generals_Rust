//! Original Drawable ambient algorithm with the receiving audio service borrowed.
//! Compatibility entrypoints in basic_core remain transitional adapters.
use super::basic_core::ambient_sound_is_in_range;
use super::*;
use crate::system::TimeOfDay;
use game_engine::common::audio::{AudioManager, AudioSubmissionLocality};
use gamelogic::common::types::BodyDamageType;

/// Synchronous receiving audio facts for the end of one native Drawable
/// update. The borrowed service stays outside Drawable state and is only used
/// after client-update callbacks have completed.
pub struct NativeAmbientUpdate<'a> {
    time_of_day: Option<TimeOfDay>,
    locality_for_object: &'a mut dyn FnMut(Option<u32>) -> AudioSubmissionLocality,
}

impl<'a> NativeAmbientUpdate<'a> {
    pub fn new(
        time_of_day: Option<TimeOfDay>,
        locality_for_object: &'a mut dyn FnMut(Option<u32>) -> AudioSubmissionLocality,
    ) -> Self {
        Self {
            time_of_day,
            locality_for_object,
        }
    }

    pub(crate) fn update_after_client_modules(
        &mut self,
        drawable: &mut BasicDrawable,
        audio_device: Option<&std::sync::Arc<std::sync::Mutex<AudioManager>>>,
    ) -> Result<(), String> {
        use game_engine::common::bit_flags::{ModelConditionFlags, create_model_condition_flags};

        // C++ setTimeOfDay queues a local transition; an admitted receiving
        // TOD remains authoritative over compatibility/Core messages.
        let pending_tod = if drawable.receiving_time_of_day.is_some() {
            drawable
                .pending_time_of_day
                .store(0, std::sync::atomic::Ordering::Relaxed);
            None
        } else {
            match drawable
                .pending_time_of_day
                .swap(0, std::sync::atomic::Ordering::Relaxed)
            {
                1 => Some(TimeOfDay::Morning),
                2 => Some(TimeOfDay::Afternoon),
                3 => Some(TimeOfDay::Evening),
                4 => Some(TimeOfDay::Night),
                _ => None,
            }
        };
        // A queued Core TOD is consumed for model-condition timing below, but
        // its audio restart remains owed until a receiving service can apply it.
        if pending_tod.is_some() {
            drawable.ambient_tod_changed = true;
            let previous_body_state = drawable
                .pending_ambient_tod
                .map(|pending| pending.body_state)
                .unwrap_or_else(|| drawable.current_body_damage_type());
            drawable.pending_ambient_tod = pending_tod.map(|target| PendingAmbientTod {
                kind: AmbientTodRestartKind::DrawableCommand,
                body_state: previous_body_state,
                target,
            });
        }
        let model_tod =
            pending_tod.or_else(|| drawable.pending_ambient_tod.map(|pending| pending.target));
        let apply_pending_model_tod = |drawable: &mut BasicDrawable| {
            if let Some(tod) = model_tod {
                let mut clear = create_model_condition_flags();
                let mut set = create_model_condition_flags();
                clear.set(ModelConditionFlags::NIGHT, true);
                set.set(ModelConditionFlags::NIGHT, matches!(tod, TimeOfDay::Night));
                drawable.clear_and_set_model_condition_flags(&clear, &set);
            }
        };

        let time_of_day = pending_tod
            .or_else(|| drawable.pending_ambient_tod.map(|pending| pending.target))
            .or(drawable.receiving_time_of_day)
            .or(self.time_of_day);
        let event = drawable.ambient_sound_event.as_ref().map(|event| {
            (
                event.get_event_name().is_empty(),
                event.get_playing_handle(),
                event.get_time_of_day(),
                event
                    .get_audio_event_info()
                    .map(|info| info.is_permanent_sound())
                    .unwrap_or(true),
            )
        });
        let Some((audio_device, time_of_day)) = audio_device.zip(time_of_day) else {
            // Keep the obligation: a later native tick may have the owner's
            // TOD and audio device even though this one does not. Apply the
            // visual TOD now, retaining the body state for later audio parity.
            apply_pending_model_tod(drawable);
            return Ok(());
        };
        let event_time_of_day = |tod| {
            use game_engine::common::audio::audio_event_rts::TimeOfDay as AudioTimeOfDay;
            match tod {
                TimeOfDay::Morning => AudioTimeOfDay::Morning,
                TimeOfDay::Afternoon => AudioTimeOfDay::Afternoon,
                TimeOfDay::Evening => AudioTimeOfDay::Evening,
                TimeOfDay::Night => AudioTimeOfDay::Night,
            }
        };
        let explicit_tod_transition = drawable.pending_ambient_tod.is_some();
        let time_changed = explicit_tod_transition
            || pending_tod.is_some()
            || (drawable.ambient_tod_changed
                && event.is_none_or(|(_, _, event_tod, _)| {
                    event_tod != event_time_of_day(time_of_day)
                }));
        if !explicit_tod_transition
            && (!drawable.ambient_sound_enabled || !drawable.ambient_sound_enabled_from_script)
        {
            apply_pending_model_tod(drawable);
            drawable.ambient_tod_changed = false;
            drawable.pending_ambient_tod = None;
            return Ok(());
        }

        // C++ Drawable::setTimeOfDay starts the ambient event before replacing
        // NIGHT. If we deferred for missing services, use the body state
        // captured before applying NIGHT on that earlier update.
        if explicit_tod_transition {
            let locality = (self.locality_for_object)(drawable.object_id);
            let mut audio = match audio_device.lock() {
                Ok(audio) => audio,
                Err(_) => {
                    apply_pending_model_tod(drawable);
                    return Err(
                        "receiving audio manager lock poisoned during Drawable ambient refresh"
                            .to_owned(),
                    );
                }
            };
            let state = drawable
                .pending_ambient_tod
                .map(|pending| pending.body_state)
                .unwrap_or_else(|| drawable.current_body_damage_type());
            let restart = drawable.start_ambient_sound_for_state_with_audio(
                state,
                time_of_day,
                false,
                &mut audio,
                &locality,
            );
            apply_pending_model_tod(drawable);
            restart?;
            drawable.ambient_tod_changed = false;
            drawable.pending_ambient_tod = None;
            return Ok(());
        }

        // A snapshot can precede admission of its ambient base even when the
        // native AudioManager already exists. Keep the override on this
        // Drawable and retry it from the ordinary receiving update once the
        // owner catalog exposes the base; do not publish a custom alias.
        if !drawable.custom_sound_ambient_base_resolved
            && let Some(base_name) = drawable.custom_sound_ambient_base_name.as_deref()
        {
            let locality = (self.locality_for_object)(drawable.object_id);
            let mut audio = audio_device.lock().map_err(|_| {
                "receiving audio manager lock poisoned during Drawable ambient refresh".to_owned()
            })?;
            if audio.find_audio_event_info(base_name).is_some() {
                let state = drawable.current_body_damage_type();
                drawable.start_ambient_sound_for_state_with_audio(
                    state,
                    time_of_day,
                    true,
                    &mut audio,
                    &locality,
                )?;
                apply_pending_model_tod(drawable);
                drawable.ambient_tod_changed = false;
                drawable.pending_ambient_tod = None;
                return Ok(());
            }
        }

        let Some((event_name_empty, handle, _, permanent)) = event else {
            if !time_changed {
                apply_pending_model_tod(drawable);
                drawable.ambient_tod_changed = false;
                drawable.pending_ambient_tod = None;
                return Ok(());
            }
            let locality = (self.locality_for_object)(drawable.object_id);
            let mut audio = audio_device.lock().map_err(|_| {
                "receiving audio manager lock poisoned during Drawable ambient refresh".to_owned()
            })?;
            let state = drawable.current_body_damage_type();
            let restart = drawable.start_ambient_sound_for_state_with_audio(
                state,
                time_of_day,
                false,
                &mut audio,
                &locality,
            );
            apply_pending_model_tod(drawable);
            restart?;
            drawable.ambient_tod_changed = false;
            drawable.pending_ambient_tod = None;
            return Ok(());
        };
        if event_name_empty {
            apply_pending_model_tod(drawable);
            drawable.ambient_tod_changed = false;
            drawable.pending_ambient_tod = None;
            return Ok(());
        }

        let locality = (self.locality_for_object)(drawable.object_id);
        let mut audio = audio_device.lock().map_err(|_| {
            "receiving audio manager lock poisoned during Drawable ambient refresh".to_owned()
        })?;
        if time_changed || (!audio.is_currently_playing(handle) && permanent) {
            let state = drawable.current_body_damage_type();
            drawable.start_ambient_sound_for_state_with_audio(
                state,
                time_of_day,
                false,
                &mut audio,
                &locality,
            )?;
        }
        apply_pending_model_tod(drawable);
        drawable.ambient_tod_changed = false;
        drawable.pending_ambient_tod = None;
        Ok(())
    }
}

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
                let base = self
                    .custom_sound_ambient_base_name
                    .as_deref()
                    .and_then(|base_name| audio.find_audio_event_info(base_name));
                self.custom_sound_ambient_base_resolved =
                    base.is_some() || self.custom_sound_ambient_base_name.is_none();
                let event_info = base
                    .map(|base| info.rebase_onto(&base).get_audio_event_info().clone())
                    .unwrap_or_else(|| info.get_audio_event_info().clone());
                let mut event = AudioEventRts::new();
                event.set_event_name(event_info.audio_name.clone());
                event.set_audio_event_info(std::sync::Arc::new(event_info));
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
        if !loading_map && !in_load_game {
            self.start_ambient_sound_current_state_with_audio(false, time_of_day, audio, locality)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "ambient_services_tests.rs"]
mod tests;
