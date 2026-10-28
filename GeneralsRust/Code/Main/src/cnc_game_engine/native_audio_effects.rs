//! Borrow the existing application audio service only at a native operation root.
//! No lock spans a simulation walk: legacy callbacks may submit other audio.
use game_engine::common::audio::{AudioEventRts, AudioManager, AudioSubmissionLocality};
use std::sync::{Arc, Mutex};

pub(super) struct NativeAudioService<'a> {
    device: &'a Arc<Mutex<AudioManager>>,
    receipts: &'a mut Vec<u32>,
    queued_events: &'a mut Vec<crate::game_logic::audio_dispatch_impl::AdmittedAudioRequest>,
}
impl<'a> NativeAudioService<'a> {
    pub(super) fn receiving(
        subsystems: &'a mut crate::subsystem_manager::SubsystemManager,
    ) -> Option<Self> {
        let (device, receipts, queued_events) = subsystems
            .get_mut::<crate::subsystem_manager::AudioManagerSubsystem>()?
            .receiving_audio_parts()?;
        Some(Self {
            device,
            receipts,
            queued_events,
        })
    }
    fn record(&mut self, handle: u32) {
        if (1000..0xFFFF_FFF0).contains(&handle) {
            self.receipts.push(handle);
        }
    }
    pub(super) fn receiver(&mut self) -> &mut dyn crate::game_logic::NativeAudioEffects {
        self
    }

    pub(super) fn update_with_view(
        &mut self,
        observation: game_engine::common::audio::game_audio::AudioViewObservation,
        visible: &mut dyn FnMut(&game_engine::common::audio::audio_event_rts::Coord3D) -> bool,
    ) {
        if let Ok(mut device) = self.device.lock() {
            device.update_with_view(observation, visible);
        }
    }
}

/// EVA keeps its playback handle in the receiving speaker. The shared device
/// remains a real native service, while admission and receipts belong here.
#[cfg(feature = "game_client")]
pub(super) struct NativeEvaPlayback<'a> {
    pub(super) service: Option<NativeAudioService<'a>>,
    pub(super) locality: AudioSubmissionLocality,
}

#[cfg(feature = "game_client")]
impl game_client::eva::EvaPlayback for NativeEvaPlayback<'_> {
    fn is_playing(&mut self, handle: u32) -> bool {
        if handle == 0 {
            return false;
        }
        self.service.as_ref().is_some_and(|service| {
            service
                .device
                .lock()
                .ok()
                .is_some_and(|audio| audio.is_currently_playing(handle))
        })
    }

    fn play_speech(&mut self, event: &gamelogic::common::audio::AudioEventRts) -> u32 {
        let Some(service) = self.service.as_mut() else {
            return 0;
        };
        // EVA owns a non-positional speech event; its player and flags are
        // copied into the receiving device's event without a global resolver.
        let mut speech = AudioEventRts::with_event_name(event.get_event_name());
        if let Some(player) = event.player_index {
            speech.set_player_index(player as i32);
        }
        speech.set_should_fade(event.should_fade());
        speech.set_is_logical_audio(event.is_logical_audio());
        speech.set_uninterruptable(event.is_uninterruptable());
        if event.has_caller_volume() {
            speech.set_volume(event.volume);
        }
        let handle = {
            let Ok(mut device) = service.device.lock() else {
                return 0;
            };
            if let Some(info) = device.find_audio_event_info(speech.get_event_name()) {
                speech.set_audio_event_info(info);
            }
            device.add_audio_event_with_locality(&speech, &self.locality)
        };
        service.record(handle);
        handle
    }
}
impl crate::game_logic::NativeAudioEffects for NativeAudioService<'_> {
    fn music_track_completed(
        &mut self,
        track: &str,
        number_of_times: i32,
        additional_handles: &[u32],
    ) -> Result<bool, String> {
        let mut owned = self.receipts.clone();
        owned.extend_from_slice(additional_handles);
        let device = self
            .device
            .lock()
            .map_err(|_| "receiving native music device poisoned".to_owned())?;
        Ok(device.has_owned_music_track_completed(track, number_of_times, &owned))
    }

    fn accepts_audio_device(&self, device: &Arc<Mutex<AudioManager>>) -> bool {
        Arc::ptr_eq(device, self.device)
    }

    fn ambient_retry_frames(&mut self) -> Option<u32> {
        self.device
            .lock()
            .ok()
            .map(|device| device.get_audio_settings().drawable_ambient_frames)
    }

    fn ambient_retry_observation(
        &mut self,
        object_id: u32,
        name: &str,
    ) -> Option<crate::game_logic::AmbientAudioRetryObservation> {
        let device = self.device.lock().ok()?;
        let is_playing =
            device.is_owned_named_event_playing_for_object(object_id, name, self.receipts);
        let is_permanent = (!is_playing).then(|| {
            device
                .find_audio_event_info(name)
                .map(|info| info.is_permanent_sound())
                .unwrap_or(true)
        });
        Some(crate::game_logic::AmbientAudioRetryObservation {
            is_playing,
            is_permanent,
        })
    }

    fn script_audio(
        &mut self,
        request: gamelogic::scripting::engine::ScriptAudioRequest<'_>,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
        additional_handles: &[u32],
    ) -> Result<(), String> {
        use gamelogic::scripting::engine::ScriptAudioRequest as Request;
        match request {
            Request::SetVolume { .. } => {
                for event in &mut *self.queued_events {
                    event.refresh_script_bus(policy.volumes);
                }
            }
            Request::Pause(_) => self.queued_events.retain(|event| event.stop),
            _ => {}
        }
        let mut owned = self.receipts.clone();
        owned.extend_from_slice(additional_handles);
        let mut device = self
            .device
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match request {
            Request::StopMusic { fade } => device.stop_music_for_handles(&owned, fade),
            Request::SetVolume { .. } => {
                device.refresh_owned_script_volumes(&owned, policy.volumes)
            }
            Request::NamedVolume { name, volume } if !name.is_empty() && volume != -1. => {
                device.adjust_owned_playing_volume(&owned, name, volume)
            }
            Request::NamedVolume { .. } => {}
            Request::RemoveNamed(name) => device.remove_owned_playing_audio(name, &owned),
            Request::RemoveDisabled => device.remove_owned_disabled_audio(&owned),
            Request::Pause(affect) => device.pause_owned_audio(&owned, affect),
            Request::Resume(affect) => device.resume_owned_audio(&owned, affect),
        }
        Ok(())
    }

    fn submit_selected_audio(
        &mut self,
        event: &AudioEventRts,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        let handle = {
            let mut device = self.device.lock().expect("native audio device poisoned");
            let Some(mut prepared) = device.prepare_audio_event_with_selection(event, selection)?
            else {
                return Ok(());
            };
            prepared.admit_script_policy(policy);
            device.add_prepared_audio_event_with_locality(&prepared, locality)
        };
        self.record(handle);
        Ok(())
    }
    fn add_prepared_audio_event(
        &mut self,
        event: &AudioEventRts,
        locality: &AudioSubmissionLocality,
    ) {
        let handle = self
            .device
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .add_prepared_audio_event_with_locality(event, locality);
        self.record(handle);
    }

    #[cfg(feature = "game_client")]
    fn submit_fx_sound(
        &mut self,
        name: &str,
        position: Option<&gamelogic::common::Coord3D>,
        player: Option<i32>,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        let handle = {
            let mut audio = self
                .device
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            game_client::fx_list::submit_sound_fx_with_policy(
                name, position, player, &mut audio, locality, selection, policy,
            )?
        };
        self.record(handle);
        Ok(())
    }

    #[cfg(feature = "game_client")]
    fn native_ambient(
        &mut self,
    ) -> Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio> {
        Some(self)
    }
    fn add_audio_event(&mut self, event: &AudioEventRts, locality: &AudioSubmissionLocality) {
        let handle = self
            .device
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .add_audio_event_with_locality(event, locality);
        self.record(handle);
    }
}

#[derive(Clone)]
enum PendingScriptAudio {
    StopMusic(bool),
    SetVolume(f32, game_engine::common::audio::game_audio::AudioAffect),
    Named(String, f32),
    Remove(String),
    RemoveDisabled,
    Pause(game_engine::common::audio::game_audio::AudioAffect),
    Resume(game_engine::common::audio::game_audio::AudioAffect),
}
impl PendingScriptAudio {
    fn capture(request: gamelogic::scripting::engine::ScriptAudioRequest<'_>) -> Self {
        use gamelogic::scripting::engine::ScriptAudioRequest as R;
        match request {
            R::StopMusic { fade } => Self::StopMusic(fade),
            R::SetVolume { volume, affect } => Self::SetVolume(volume, affect),
            R::NamedVolume { name, volume } => Self::Named(name.to_owned(), volume),
            R::RemoveNamed(name) => Self::Remove(name.to_owned()),
            R::RemoveDisabled => Self::RemoveDisabled,
            R::Pause(affect) => Self::Pause(affect),
            R::Resume(affect) => Self::Resume(affect),
        }
    }
    fn borrowed(&self) -> gamelogic::scripting::engine::ScriptAudioRequest<'_> {
        use gamelogic::scripting::engine::ScriptAudioRequest as R;
        match self {
            Self::StopMusic(fade) => R::StopMusic { fade: *fade },
            Self::SetVolume(volume, affect) => R::SetVolume {
                volume: *volume,
                affect: *affect,
            },
            Self::Named(name, volume) => R::NamedVolume {
                name,
                volume: *volume,
            },
            Self::Remove(name) => R::RemoveNamed(name),
            Self::RemoveDisabled => R::RemoveDisabled,
            Self::Pause(affect) => R::Pause(*affect),
            Self::Resume(affect) => R::Resume(*affect),
        }
    }
}
enum PendingAudioOutput {
    Event(AudioEventRts, AudioSubmissionLocality, bool),
    Script(
        PendingScriptAudio,
        game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ),
}
/// Outputs of detached map construction. Rejection drops them; the accepted
/// world install admits them once, before the next real audio update.
#[derive(Default)]
pub(super) struct PendingNativeAudio {
    events: Vec<PendingAudioOutput>,
    fx_device: Option<Arc<Mutex<AudioManager>>>,
}
impl PendingNativeAudio {
    pub(super) fn capture_sound_service(&mut self, device: Option<Arc<Mutex<AudioManager>>>) {
        self.fx_device = device;
    }

    pub(super) fn clear(&mut self) {
        self.events.clear();
    }
    pub(super) fn admit_to_live(
        mut self,
        audio: Option<&mut NativeAudioService<'_>>,
    ) -> Result<(), String> {
        if let Some(audio) = audio {
            if let Some(device) = &self.fx_device {
                assert!(
                    Arc::ptr_eq(device, audio.device),
                    "candidate audio belongs to its captured receiving device"
                );
            }
            self.deliver_to(audio.receiver())?;
        } else if !self.events.is_empty() {
            return Err("accepted native audio output has no receiving device".into());
        }
        Ok(())
    }
    pub(super) fn deliver_to(
        &mut self,
        audio: &mut dyn crate::game_logic::NativeAudioEffects,
    ) -> Result<(), String> {
        for output in self.events.drain(..) {
            match output {
                PendingAudioOutput::Event(event, locality, true) => {
                    audio.add_prepared_audio_event(&event, &locality)
                }
                PendingAudioOutput::Event(event, locality, false) => {
                    audio.add_audio_event(&event, &locality)
                }
                PendingAudioOutput::Script(command, policy) => {
                    audio.script_audio(command.borrowed(), &policy, &[])?
                }
            }
        }
        Ok(())
    }
}
impl crate::game_logic::NativeAudioEffects for PendingNativeAudio {
    fn music_track_completed(
        &mut self,
        _track: &str,
        _number_of_times: i32,
        _additional_handles: &[u32],
    ) -> Result<bool, String> {
        // Detached construction has admitted no playing music to the live device.
        Ok(false)
    }

    fn submit_selected_audio(
        &mut self,
        event: &AudioEventRts,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        let Some(device) = &self.fx_device else {
            return Ok(());
        };
        let device = device.lock().expect("candidate audio device poisoned");
        if let Some(mut event) = device.prepare_audio_event_with_selection(event, selection)? {
            event.admit_script_policy(policy);
            self.events
                .push(PendingAudioOutput::Event(event, *locality, true));
        }
        Ok(())
    }

    fn script_audio(
        &mut self,
        request: gamelogic::scripting::engine::ScriptAudioRequest<'_>,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
        _additional_handles: &[u32],
    ) -> Result<(), String> {
        if self.fx_device.is_none() {
            return Ok(());
        }
        self.events.push(PendingAudioOutput::Script(
            PendingScriptAudio::capture(request),
            policy.clone(),
        ));
        Ok(())
    }

    #[cfg(feature = "game_client")]
    fn submit_fx_sound(
        &mut self,
        name: &str,
        position: Option<&gamelogic::common::Coord3D>,
        player: Option<i32>,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        if name.is_empty() || name == "NoSound" {
            return Ok(());
        }
        // A native build may intentionally have no available audio device.
        // Never discover another application's replacement mixer here.
        let Some(service) = self.fx_device.as_ref() else {
            return Ok(());
        };
        let mut event = AudioEventRts::with_event_name(name);
        if let Some(position) = position {
            event.set_position(&game_engine::common::audio::audio_event_rts::Coord3D {
                x: position.x,
                y: position.y,
                z: position.z,
            });
        }
        if let Some(player) = player {
            event.set_player_index(player);
        }
        let audio = service
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(mut event) = audio.prepare_audio_event_with_selection(&event, selection)? {
            event.admit_script_policy(policy);
            self.events
                .push(PendingAudioOutput::Event(event, *locality, true));
        }
        Ok(())
    }

    fn add_audio_event(&mut self, event: &AudioEventRts, locality: &AudioSubmissionLocality) {
        self.events
            .push(PendingAudioOutput::Event(event.clone(), *locality, false));
    }
}

#[cfg(feature = "game_client")]
impl game_client::drawable::drawable::NativeAmbientAudio for NativeAudioService<'_> {
    fn restart_ambient(
        &mut self,
        drawable: &mut game_client::drawable::drawable::BasicDrawable,
        state: gamelogic::common::types::BodyDamageType,
        time_of_day: game_client::system::TimeOfDay,
        locality: &AudioSubmissionLocality,
        policy: &game_engine::common::audio::script_policy::ScriptAudioPolicy,
    ) -> Result<(), String> {
        // Only local stop/select/admit touches the mixer; native model-condition
        // callback traversal has completed before this loan is acquired.
        let mut audio = self
            .device
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        drawable.start_ambient_sound_for_state_with_audio_policy(
            state,
            time_of_day,
            false,
            &mut audio,
            locality,
            Some(policy),
        )
    }
}
