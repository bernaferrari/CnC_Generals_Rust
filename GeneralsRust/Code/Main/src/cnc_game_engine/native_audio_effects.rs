//! Borrow the existing application audio service only at a native operation root.
//! No lock spans a simulation walk: legacy callbacks may submit other audio.
use game_engine::common::audio::{AudioEventRts, AudioManager, AudioSubmissionLocality};
use std::sync::{Arc, Mutex};

pub(super) struct NativeAudioService(Arc<Mutex<AudioManager>>);
impl NativeAudioService {
    pub(super) fn live() -> Option<Self> {
        game_engine::common::audio::game_audio::get_global_audio_manager().map(Self)
    }
    pub(super) fn receiver(&mut self) -> &mut dyn crate::game_logic::NativeAudioEffects {
        self
    }
}
impl crate::game_logic::NativeAudioEffects for NativeAudioService {
    fn add_prepared_audio_event(
        &mut self,
        event: &AudioEventRts,
        locality: &AudioSubmissionLocality,
    ) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .add_prepared_audio_event_with_locality(event, locality);
    }

    #[cfg(feature = "game_client")]
    fn submit_fx_sound(
        &mut self,
        name: &str,
        position: Option<&gamelogic::common::Coord3D>,
        player: Option<i32>,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
    ) -> Result<(), String> {
        let mut audio = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        game_client::fx_list::submit_sound_fx_owned(
            name, position, player, &mut audio, locality, selection,
        )
        .map(|_| ())
    }

    #[cfg(feature = "game_client")]
    fn native_ambient(
        &mut self,
    ) -> Option<&mut dyn game_client::drawable::drawable::NativeAmbientAudio> {
        Some(self)
    }
    fn add_audio_event(&mut self, event: &AudioEventRts, locality: &AudioSubmissionLocality) {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .add_audio_event_with_locality(event, locality);
    }
}

/// Outputs of detached map construction. Rejection drops them; the accepted
/// world install admits them once, before the next real audio update.
#[derive(Default)]
pub(super) struct PendingNativeAudio {
    events: Vec<(AudioEventRts, AudioSubmissionLocality, bool)>,
    fx_service: Option<NativeAudioService>,
}
impl PendingNativeAudio {
    pub(super) fn capture_sound_service(&mut self) {
        self.fx_service = NativeAudioService::live();
    }

    pub(super) fn clear(&mut self) {
        self.events.clear();
    }
    pub(super) fn admit_to_live(mut self) {
        if let Some(mut audio) = self.fx_service.take() {
            self.deliver_to(audio.receiver());
        } else if !self.events.is_empty() {
            log::warn!(
                "receiving native map has no captured audio device; {} pending events omitted",
                self.events.len()
            );
        }
    }
    pub(super) fn deliver_to(&mut self, audio: &mut dyn crate::game_logic::NativeAudioEffects) {
        for (event, locality, prepared) in self.events.drain(..) {
            if prepared {
                audio.add_prepared_audio_event(&event, &locality);
            } else {
                audio.add_audio_event(&event, &locality);
            }
        }
    }
}
impl crate::game_logic::NativeAudioEffects for PendingNativeAudio {
    #[cfg(feature = "game_client")]
    fn submit_fx_sound(
        &mut self,
        name: &str,
        position: Option<&gamelogic::common::Coord3D>,
        player: Option<i32>,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn game_engine::common::audio::audio_event_rts::AudioEventSelection,
    ) -> Result<(), String> {
        if name.is_empty() || name == "NoSound" {
            return Ok(());
        }
        // A native build may intentionally have no available audio device.
        // Never discover another application's replacement mixer here.
        let Some(service) = self.fx_service.as_ref() else {
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
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(event) = audio.prepare_audio_event_with_selection(&event, selection)? {
            self.events.push((event, *locality, true));
        }
        Ok(())
    }

    fn add_audio_event(&mut self, event: &AudioEventRts, locality: &AudioSubmissionLocality) {
        self.events.push((event.clone(), *locality, false));
    }
}

#[cfg(feature = "game_client")]
impl game_client::drawable::drawable::NativeAmbientAudio for NativeAudioService {
    fn restart_ambient(
        &mut self,
        drawable: &mut game_client::drawable::drawable::BasicDrawable,
        state: gamelogic::common::types::BodyDamageType,
        time_of_day: game_client::system::TimeOfDay,
        locality: &AudioSubmissionLocality,
    ) -> Result<(), String> {
        // Only local stop/select/admit touches the mixer; native model-condition
        // callback traversal has completed before this loan is acquired.
        let mut audio = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        drawable.start_ambient_sound_for_state_with_audio(
            state,
            time_of_day,
            false,
            &mut audio,
            locality,
        )
    }
}
