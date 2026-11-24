//! Borrow the existing application audio service only at a native operation root.
//! No lock spans a simulation walk: legacy callbacks may submit other audio.
use game_engine::common::audio::audio_event_rts::AudioType;
use game_engine::common::audio::{AudioEventRts, AudioInstance, AudioManager, AudioSubmissionLocality};
use std::sync::{Arc, Mutex};

impl super::CnCGameEngine {
    pub(super) fn play_receiving_ui_cue(&mut self, name: &str, player: Option<u32>) {
        use crate::game_logic::NativeAudioEffects;

        let player = player.map_or(-1, |id| id as i32);
        let locality = self.game_logic.receiving_audio_locality_for_player(player);
        if let Some(mut audio) = NativeAudioService::receiving(&mut self.subsystem_manager) {
            let mut event = AudioEventRts::with_event_name(name);
            event.set_player_index(player);
            let mut selection = crate::game_logic::SessionAudioSelection {
                logic: &self.game_logic.logic_random,
                audio: &mut self.game_logic.session_view.audio_random,
            };
            if let Err(error) = audio.submit_selected_audio(
                &event,
                &locality,
                &mut selection,
                self.game_logic.queued_audio_events.policy(),
            ) {
                log::debug!("Could not submit receiving UI cue {name:?}: {error}");
            }
        }
    }
}

pub(crate) struct NativeAudioService<'a> {
    device: &'a Arc<Mutex<AudioManager>>,
    instance: &'a AudioInstance,
    receipts: &'a mut Vec<u32>,
    queued_events: &'a mut Vec<crate::game_logic::audio_dispatch_impl::AdmittedAudioRequest>,
}
impl<'a> NativeAudioService<'a> {
    pub(crate) fn receiving(
        subsystems: &'a mut crate::subsystem_manager::SubsystemManager,
    ) -> Option<Self> {
        let (device, instance, receipts, queued_events) = subsystems
            .get_mut::<crate::subsystem_manager::AudioManagerSubsystem>()?
            .receiving_audio_parts()?;
        Some(Self {
            device,
            instance,
            receipts,
            queued_events,
        })
    }
    fn is_playback_receipt(handle: u32) -> bool {
        (1000..0xFFFF_FFF0).contains(&handle)
    }

    fn record(&mut self, handle: u32) {
        if Self::is_playback_receipt(handle) {
            self.receipts.push(handle);
        }
    }

    /// ScriptEngine.cpp:9290–9307 restarts the saved track for the local player.
    fn resume_saved_music(&mut self, world: &mut crate::game_logic::GameLogic) -> Result<(), String> {
        let Some((interpreter, player)) = world
            .owned_script_engine
            .as_ref()
            .zip(world.local_player_id())
        else {
            return Ok(());
        };
        let track = interpreter.get_current_track_name();
        if track.is_empty() {
            return Ok(());
        }

        let locality = world.receiving_audio_locality_for_player(player as i32);
        let mut selection = crate::game_logic::SessionAudioSelection {
            logic: &world.logic_random,
            audio: &mut world.session_view.audio_random,
        };
        let handle = {
            let mut device = self
                .device
                .lock()
                .map_err(|_| "receiving native music device poisoned".to_owned())?;
            let Some(info) = device
                .find_audio_event_info(&track)
                .filter(|info| info.sound_type == AudioType::Music)
            else {
                log::warn!("Saved music track {track:?} is unavailable on the receiving device");
                return Ok(());
            };
            let mut event = AudioEventRts::with_event_name(&track);
            event.admit_instance(self.instance);
            event.set_player_index(player as i32);
            event.set_volume(info.volume);
            event.set_audio_event_info(info);
            let Some(mut event) = device.prepare_audio_event_with_selection(&event, &mut selection)?
            else {
                return Ok(());
            };
            event.admit_script_policy(world.queued_audio_events.policy());
            let handle = device.add_prepared_audio_event_with_locality(&event, &locality);
            if Self::is_playback_receipt(handle) {
                // The app's device may still carry the old match or loading
                // track. Replace only this session's music before the mixer
                // starts the new request.
                device.stop_music_for_handles(self.receipts, false);
            }
            handle
        };
        self.record(handle);
        Ok(())
    }

    pub(super) fn receiver(&mut self) -> &mut dyn crate::game_logic::NativeAudioEffects {
        self
    }

    pub(super) fn update_with_view(
        &mut self,
        observation: game_engine::common::audio::game_audio::AudioViewObservation,
        visible: &mut dyn FnMut(&game_engine::common::audio::audio_event_rts::Coord3D) -> bool,
        lookup: impl FnMut(
            u32,
            &std::sync::Weak<()>,
        ) -> Option<(
            game_engine::common::audio::audio_event_rts::Coord3D,
            i32,
        )>,
    ) {
        if let Ok(mut device) = self.device.lock() {
            device.refresh_receiving_object_audio_positions(self.receipts, lookup);
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
    pub(super) selection:
        &'a mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
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
        speech.admit_instance(service.instance);
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
            device.add_audio_event_with_locality_and_selection(
                &speech,
                &self.locality,
                self.selection,
            )
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
        let device = self
            .device
            .lock()
            .map_err(|_| "receiving native music device poisoned".to_owned())?;
        Ok(device.has_owned_music_track_completed(track, number_of_times, self.receipts)
            || device.has_owned_music_track_completed(track, number_of_times, additional_handles))
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
            let Some(mut prepared) = device.prepare_audio_event_for_instance(event, selection, self.instance)?
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
            .add_prepared_audio_event_for_instance(event, locality, self.instance);
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
                name, position, player, &mut audio, Some(self.instance), locality, selection, policy,
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
            .add_audio_event_for_instance(event, locality, self.instance);
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
    /// Admit constructor audio and the ScriptEngine's post-load cue at commit.
    pub(super) fn admit_after_restore(
        self,
        world: &mut crate::game_logic::GameLogic,
        mut audio: Option<&mut NativeAudioService<'_>>,
    ) -> Result<(), String> {
        world.finish_saved_snapshot_admission()?;
        self.admit_to_live(audio.as_deref_mut())?;
        if let Some(audio) = audio {
            audio.resume_saved_music(world)?;
        }
        Ok(())
    }

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
        _instance: &AudioInstance,
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
            self.instance,
            locality,
            Some(policy),
        )
    }
}

#[cfg(test)]
mod music_completion_tests {
    use super::*;
    use crate::game_logic::NativeAudioEffects;
    use game_engine::common::audio::audio_event_rts::{AudioEventInfo, AudioType};
    use game_engine::common::audio::game_audio::SoundPlaybackHook;
    use game_engine::common::audio::script_policy::ScriptAudioPolicy;
    use gamelogic::scripting::engine::ScriptAudioRequest;

    /// Shared hardware stand-in: every handle loops twice, handle order decides nothing.
    struct LoopingBackend;
    impl SoundPlaybackHook for LoopingBackend {
        fn play(&self, _: &AudioEventRts) -> Result<(), String> {
            Ok(())
        }
        fn stop(&self, _: u32) {}
        fn pause(&self, _: u32) {}
        fn resume(&self, _: u32) {}
        fn is_playing(&self, _: u32) -> bool {
            true
        }
        fn music_loop_count(&self, _: u32) -> i32 {
            2
        }
    }

    #[derive(Default)]
    struct MusicPlaybackLog {
        starts: Vec<u32>,
        stops: Vec<u32>,
    }

    struct RecordingBackend(Arc<Mutex<MusicPlaybackLog>>);

    impl SoundPlaybackHook for RecordingBackend {
        fn play(&self, event: &AudioEventRts) -> Result<(), String> {
            self.0.lock().unwrap().starts.push(event.get_playing_handle());
            Ok(())
        }

        fn stop(&self, handle: u32) {
            self.0.lock().unwrap().stops.push(handle);
        }

        fn pause(&self, _: u32) {}
        fn resume(&self, _: u32) {}
        fn is_playing(&self, _: u32) -> bool {
            true
        }
    }

    fn recording_manager() -> (AudioManager, Arc<Mutex<MusicPlaybackLog>>) {
        let playback = Arc::new(Mutex::new(MusicPlaybackLog::default()));
        let mut manager =
            AudioManager::with_playback_hook(Box::new(RecordingBackend(Arc::clone(&playback))));
        manager.init();
        (manager, playback)
    }

    fn recording_device() -> (Arc<Mutex<AudioManager>>, Arc<Mutex<MusicPlaybackLog>>) {
        let (mut manager, playback) = recording_manager();
        for (name, sound_type) in [
            ("Shell", AudioType::Music),
            ("Game_USA_11", AudioType::Music),
            ("GUIClick", AudioType::SoundEffect),
        ] {
            manager.register_audio_event_info(AudioEventInfo {
                audio_name: name.into(),
                sound_type,
                volume: 1.,
                ..Default::default()
            });
        }
        (Arc::new(Mutex::new(manager)), playback)
    }

    fn submit_music_fixture(service: &mut NativeAudioService<'_>, name: &str, fade_in: bool) -> u32 {
        use game_engine::common::audio::audio_event_rts::AudioSelectionStreams;
        let mut selection = AudioSelectionStreams {
            logic: &mut Default::default(),
            audio: &mut Default::default(),
        };
        let mut event = AudioEventRts::with_event_name(name);
        event.set_should_fade(fade_in);
        service
            .submit_selected_audio(
                &event,
                &crate::game_logic::audio_dispatch_impl::receiving_submission_locality(-1, None),
                &mut selection,
                &ScriptAudioPolicy::default(),
            )
            .unwrap();
        *service.receipts.last().unwrap()
    }

    #[test]
    fn receiving_engine_audio_reset_retires_every_producer_on_the_shared_mixer() {
        use crate::subsystem_manager::{AudioManagerSubsystem, SubsystemInterface, SubsystemManager};

        let (device, playback) = recording_device();
        let mut subsystems = SubsystemManager::new();
        let mut owner = AudioManagerSubsystem::new();
        owner.bind_common_audio_device(Arc::clone(&device));
        let instance = owner.audio_instance().clone();
        subsystems.add_subsystem(owner);

        let semantic = submit_music_fixture(
            &mut NativeAudioService::receiving(&mut subsystems).unwrap(),
            "GUIClick",
            false,
        );
        let peer = AudioInstance::default();
        let locality =
            crate::game_logic::audio_dispatch_impl::receiving_submission_locality(-1, None);
        let (direct, menu, peer_playing, old_pending, peer_pending) = {
            let mut mixer = device.lock().unwrap();
            let mut play = |name, owner| {
                mixer.add_audio_event_for_instance(
                    &AudioEventRts::with_event_name(name),
                    &locality,
                    owner,
                )
            };
            let direct = play("GUIClick", &instance);
            let menu = play("Shell", &instance);
            let peer_playing = play("Shell", &peer);
            mixer.process_request_list();
            mixer.queue_music_fade_for_instance(&instance);
            mixer.process_request_list();
            assert_eq!(mixer.fading_audio_count(), 1);
            let old_pending = mixer.add_audio_event_for_instance(
                &AudioEventRts::with_event_name("Game_USA_11"), &locality, &instance,
            );
            let peer_pending = mixer.add_audio_event_for_instance(
                &AudioEventRts::with_event_name("Game_USA_11"), &locality, &peer,
            );
            (direct, menu, peer_playing, old_pending, peer_pending)
        };

        let owner = subsystems.get_mut::<AudioManagerSubsystem>().unwrap();
        owner.reset().unwrap();
        let mut mixer = device.lock().unwrap();
        for handle in [semantic, direct, menu, old_pending] {
            assert!(!mixer.contains_audio_handle(handle), "outgoing audio {handle} survived reset");
        }
        for handle in [peer_playing, peer_pending] {
            assert!(mixer.contains_audio_handle(handle), "peer lost audio {handle}");
        }
        mixer.process_request_list();
        let playback = playback.lock().unwrap();
        assert!(playback.starts.contains(&peer_pending));
        assert!(!playback.starts.contains(&old_pending));
        assert!(playback.stops.contains(&semantic));
        assert!(playback.stops.contains(&direct));
        assert!(playback.stops.contains(&menu));
        assert!(!playback.stops.contains(&peer_playing));
    }

    #[test]
    fn restored_audio_replays_the_saved_track_once_for_the_receiving_world() {
        use crate::game_logic::{GameLogic, Player, Team};
        use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
        use crate::subsystem_manager::{AudioManagerSubsystem, SubsystemInterface, SubsystemManager};

        let (device, playback) = recording_device();
        let mut subsystems = SubsystemManager::new();
        let mut owner = AudioManagerSubsystem::new();
        owner.bind_common_audio_device(Arc::clone(&device));
        subsystems.add_subsystem(owner);
        let mut audio = NativeAudioService::receiving(&mut subsystems).unwrap();
        let mut peer_receipts = Vec::new();
        let mut peer_queued = Vec::new();
        let mut peer = NativeAudioService {
            device: &device,
            instance: &AudioInstance::default(),
            receipts: &mut peer_receipts,
            queued_events: &mut peer_queued,
        };
        let old_music = submit_music_fixture(&mut audio, "Game_USA_11", false);
        let peer_music = submit_music_fixture(&mut peer, "Game_USA_11", false);
        let old_click = submit_music_fixture(&mut audio, "GUIClick", false);
        device.lock().unwrap().process_request_list();
        let queued_music = submit_music_fixture(&mut audio, "Shell", false);

        // The real map admits the receiving Team prototypes before loading
        // their saved roster.
        let world_with_player = || {
            let mut world = GameLogic::new();
            world.add_player(Player::new(5, Team::USA, "Saved local player", true));
            assert!(world.select_local_player(Some(5)));
            world
        };
        let mut source = world_with_player();
        source
            .owned_script_engine
            .as_ref()
            .unwrap()
            .set_current_track_name("Game_USA_11".into());
        source
            .queued_audio_events
            .set_named_volume("Game_USA_11", 0.55);
        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let snapshot = decode_bincode_world_snapshot(&bytes).unwrap();
        let mut world = world_with_player();
        builder.restore_from_snapshot(&snapshot, &mut world).unwrap();
        assert!(world.select_local_player(Some(5)));
        assert_eq!(
            world.owned_script_engine.as_ref().unwrap().get_current_track_name(),
            "Game_USA_11"
        );
        let logic_seed = world.logic_random.seed_words();
        let pending_map_music = || {
            let mut pending = PendingNativeAudio::default();
            pending.capture_sound_service(Some(Arc::clone(&device)));
            pending.add_audio_event(
                &AudioEventRts::with_event_name("Shell"),
                &crate::game_logic::audio_dispatch_impl::receiving_submission_locality(-1, None),
            );
            pending
        };

        assert!(pending_map_music()
            .admit_after_restore(&mut world, Some(&mut audio))
            .is_err());
        assert!(playback.lock().unwrap().stops.is_empty());
        assert_eq!(
            device.lock().unwrap().pending_play_request_count_for("Shell"),
            1
        );

        world.begin_saved_snapshot_admission().unwrap();
        subsystems.get_mut::<AudioManagerSubsystem>().unwrap().reset().unwrap();
        let mut audio = NativeAudioService::receiving(&mut subsystems).unwrap();
        pending_map_music()
            .admit_after_restore(&mut world, Some(&mut audio))
            .unwrap();
        assert!(pending_map_music()
            .admit_after_restore(&mut world, Some(&mut audio))
            .is_err());
        assert_eq!(world.logic_random.seed_words(), logic_seed);
        // Restored ambient follows ScriptEngine's music in the mixer queue.
        let ambient = submit_music_fixture(&mut audio, "GUIClick", false);
        let mut mixer = device.lock().unwrap();
        let resumed = {
            let mut pending = mixer.pending_play_events();
            let event = pending.next().expect("the saved song was queued");
            assert_eq!(event.get_event_name(), "Game_USA_11");
            assert_eq!(event.get_player_index(), 5);
            assert_eq!(event.get_volume(), 0.55);
            assert_eq!(pending.next().unwrap().get_playing_handle(), ambient);
            assert!(pending.next().is_none(), "only restored audio should remain queued");
            event.get_playing_handle()
        };
        assert!(![old_music, peer_music, old_click, queued_music].contains(&resumed));
        mixer.process_request_list();
        let played = playback.lock().unwrap();
        assert_eq!(played.starts, [old_music, peer_music, old_click, resumed, ambient]);
        assert_eq!(played.stops, [old_music, old_click, queued_music]);
    }

    #[test]
    fn script_music_replacement_cancels_only_its_unstarted_track() {
        // ShellMapMD's Unshroud and Set Music actions both run before the
        // startup mixer phase. A peer can have its own identically named track.
        for fade in [true, false] {
            let (device, playback) = recording_device();
            let mut receipts = Vec::new();
            let mut queued_events = Vec::new();
            let mut service = NativeAudioService {
                device: &device,
                instance: &AudioInstance::default(),
                receipts: &mut receipts,
                queued_events: &mut queued_events,
            };
            let mut peer_receipts = Vec::new();
            let mut peer_events = Vec::new();
            let mut peer = NativeAudioService {
                device: &device,
                instance: &AudioInstance::default(),
                receipts: &mut peer_receipts,
                queued_events: &mut peer_events,
            };
            let policy = ScriptAudioPolicy::default();
            service
                .script_audio(ScriptAudioRequest::StopMusic { fade: false }, &policy, &[])
                .unwrap();
            let shell = submit_music_fixture(&mut service, "Shell", false);
            let peer_shell = submit_music_fixture(&mut peer, "Shell", false);
            let click = submit_music_fixture(&mut service, "GUIClick", false);
            assert_ne!(shell, peer_shell);
            assert_ne!(shell, click);
            service
                .script_audio(ScriptAudioRequest::StopMusic { fade }, &policy, &[])
                .unwrap();
            let incoming = submit_music_fixture(&mut service, "Game_USA_11", true);

            let mut audio = device.lock().unwrap();
            assert_eq!(
                audio
                    .pending_play_events()
                    .map(AudioEventRts::get_playing_handle)
                    .collect::<Vec<_>>(),
                [peer_shell, click, incoming],
            );
            assert_eq!(audio.fading_audio_count(), 0);
            audio.process_request_list();
            let played = playback.lock().unwrap();
            assert_eq!(played.starts, [peer_shell, click, incoming]);
            assert!(played.stops.is_empty());
        }
    }

    #[test]
    fn script_music_replacement_keeps_an_active_tracks_fade_duration() {
        let (device, playback) = recording_device();
        let mut receipts = Vec::new();
        let mut queued_events = Vec::new();
        let mut service = NativeAudioService {
            device: &device,
            instance: &AudioInstance::default(),
            receipts: &mut receipts,
            queued_events: &mut queued_events,
        };
        let shell = submit_music_fixture(&mut service, "Shell", false);
        device.lock().unwrap().process_request_list();
        assert_eq!(playback.lock().unwrap().starts, [shell]);

        service
            .script_audio(
                ScriptAudioRequest::StopMusic { fade: true },
                &ScriptAudioPolicy::default(),
                &[],
            )
            .unwrap();
        let incoming = submit_music_fixture(&mut service, "Game_USA_11", true);
        let mut audio = device.lock().unwrap();
        let fade_frames = audio.get_audio_settings().fade_audio_frames;
        let observation = game_engine::common::audio::game_audio::AudioViewObservation {
            ground_position: Default::default(),
            heading: 0.,
            camera_position: Default::default(),
            ground_height: 0.,
        };
        assert_eq!(audio.fading_audio_count(), 1);
        for frame in 1..=fade_frames {
            audio.update_with_view(observation, &mut |_| true);
            assert_eq!(audio.fading_frames(), frame);
        }
        assert!(playback.lock().unwrap().stops.is_empty());
        audio.update_with_view(observation, &mut |_| true);
        assert_eq!(audio.fading_audio_count(), 0);
        let played = playback.lock().unwrap();
        assert_eq!(played.starts, [shell, incoming]);
        assert_eq!(played.stops, [shell]);
    }

    #[test]
    #[ignore = "requires mounted Zero Hour content"]
    fn mounted_shellmap_startup_starts_only_the_last_script_music_track() {
        use game_engine::common::ini::INI;

        let install = crate::fixture_content::retail_content_root()
            .join("Command & Conquer Generals Zero Hour/Data");
        let mut files = crate::assets::ArchiveFileSystem::new();
        files.set_local_search_roots(Vec::new());
        for archive in ["INIZH.big", "MapsZH.big"] {
            futures::executor::block_on(files.load_big_file(install.join(archive))).unwrap();
        }

        let (mut manager, playback) = recording_manager();
        for path in ["Data/INI/Default/Music.ini", "Data/INI/Music.ini"] {
            if let Some(bytes) = files.read_optional_definition_source(path).unwrap() {
                INI::new()
                    .with_inline_source(&String::from_utf8_lossy(&bytes), |ini| {
                        ini.parse_current_file_into_audio(&mut manager)
                    })
                    .unwrap();
            }
        }
        for track in ["Shell", "Game_USA_11"] {
            assert!(manager.get_audio_event_info(track).is_some(), "{track}");
        }
        let device = Arc::new(Mutex::new(manager));
        let map_path = tempfile::tempdir().unwrap();
        let shell_map = map_path.path().join("ShellMapMD.map");
        std::fs::write(
            &shell_map,
            files
                .read_optional_definition_source("Maps/ShellMapMD/ShellMapMD.map")
                .unwrap()
                .unwrap(),
        )
        .unwrap();

        let mut world = crate::game_logic::GameLogic::new();
        assert!(world.load_map(shell_map.to_str().unwrap()));
        let mut receipts = Vec::new();
        let mut queued_events = Vec::new();
        let mut audio = NativeAudioService {
            device: &device,
            instance: &AudioInstance::default(),
            receipts: &mut receipts,
            queued_events: &mut queued_events,
        };
        world.tick_logic_frame_with_script_effects(
            1.0 / 30.0,
            None,
            Some(1),
            crate::game_logic::ScriptPresentationEffects {
                audio: Some(&mut audio),
                ..Default::default()
            },
            |_| {},
            |_| {},
        );
        assert!(receipts.len() >= 2, "both startup scripts must submit audio");

        let mut audio = device.lock().unwrap();
        let music: Vec<_> = audio
            .pending_play_events()
            .filter(|event| {
                event
                    .get_audio_event_info()
                    .is_some_and(|info| info.sound_type == AudioType::Music)
            })
            .map(|event| (event.get_playing_handle(), event.get_event_name()))
            .collect();
        assert_eq!(music.len(), 1, "{music:?}");
        assert_eq!(music[0].1, "Game_USA_11");
        let incoming = music[0].0;
        audio.process_request_list();
        let played = playback.lock().unwrap();
        assert!(played.starts.contains(&incoming));
        assert!(!played.starts.contains(&receipts[0]));
        assert!(played.stops.is_empty());
    }

    /// hq-kw72bi: MilesAudioManager.cpp:1370-1385 sweeps the playing list; a
    /// receiving session may only observe music it was issued a receipt for.
    #[test]
    fn music_completion_only_observes_the_receiving_sessions_receipts() {
        let mut manager = AudioManager::with_playback_hook(Box::new(LoopingBackend));
        let info = std::sync::Arc::new(AudioEventInfo {
            audio_name: "SharedMusic".into(),
            sound_type: AudioType::Music,
            ..Default::default()
        });
        let handles = [1001u32, 1002];
        for handle in handles {
            let mut event = AudioEventRts::with_event_name("SharedMusic");
            event.set_playing_handle(handle);
            event.set_audio_event_info(info.clone());
            manager.test_insert_active_audio_event(event);
        }
        let device = Arc::new(Mutex::new(manager));
        let mut queued = Vec::new();
        let mut mine = vec![handles[0]];
        let mut service = NativeAudioService {
            device: &device,
            instance: &AudioInstance::default(),
            receipts: &mut mine,
            queued_events: &mut queued,
        };
        assert_eq!(service.music_track_completed("SharedMusic", 2, &[]), Ok(true));
        assert_eq!(service.music_track_completed("SharedMusic", 3, &[]), Ok(false));
        assert_eq!(service.music_track_completed("OtherMusic", 0, &[]), Ok(false));
        let mut none = Vec::new();
        let mut queued_peer = Vec::new();
        let mut peer = NativeAudioService {
            device: &device,
            instance: &AudioInstance::default(),
            receipts: &mut none,
            queued_events: &mut queued_peer,
        };
        assert_eq!(peer.music_track_completed("SharedMusic", 1, &[]), Ok(false));
        assert_eq!(
            peer.music_track_completed("SharedMusic", 1, &[handles[1]]),
            Ok(true)
        );
    }
}
