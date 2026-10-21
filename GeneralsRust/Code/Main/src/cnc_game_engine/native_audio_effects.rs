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
pub(super) struct PendingNativeAudio(Vec<(AudioEventRts, AudioSubmissionLocality)>);
impl PendingNativeAudio {
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }
    pub(super) fn admit_to_live(mut self) {
        if let Some(mut audio) = NativeAudioService::live() {
            self.deliver_to(audio.receiver());
        }
    }
    pub(super) fn deliver_to(&mut self, audio: &mut dyn crate::game_logic::NativeAudioEffects) {
        for (event, locality) in self.0.drain(..) {
            audio.add_audio_event(&event, &locality);
        }
    }
}
impl crate::game_logic::NativeAudioEffects for PendingNativeAudio {
    fn add_audio_event(&mut self, event: &AudioEventRts, locality: &AudioSubmissionLocality) {
        self.0.push((event.clone(), *locality));
    }
}
