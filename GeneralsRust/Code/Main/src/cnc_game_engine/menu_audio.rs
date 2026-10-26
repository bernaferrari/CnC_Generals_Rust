//! Native owner binds the shared production menu-audio backend once.
//! Canonical handles/retirement/selection policy live in GameClient's service.
use game_client::gui::menu_audio::{CommonMenuAudio, CommonMenuAudioService};
use game_engine::common::audio::{AudioManager, AudioSubmissionLocality};
use game_engine::common::random_value::RandomState;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct NativeMenuAudioService(CommonMenuAudioService);
impl NativeMenuAudioService {
    pub(super) fn bind(&mut self, device: Arc<Mutex<AudioManager>>) {
        self.0.bind(device);
    }
    pub(super) fn require_bound(&self) -> Result<(), String> {
        self.0.require_bound()
    }
    pub(super) fn visit_owned_receipts(
        &self,
        visit: &mut dyn FnMut(&Arc<Mutex<AudioManager>>, &[u32]),
    ) {
        self.0.visit_owned_receipts(visit);
    }
    pub(super) fn reset(&mut self) {
        self.0.reset();
    }
    pub(super) fn lend<'a>(
        &'a mut self,
        random: &'a mut RandomState,
        policy: &'a game_engine::common::audio::script_policy::ScriptAudioPolicy,
        locality: AudioSubmissionLocality,
        view: super::audio_view::CompletedAudioView,
    ) -> NativeMenuAudio<'a> {
        self.0.lend(random, policy, locality, view)
    }
}
pub(super) type NativeMenuAudio<'a> = CommonMenuAudio<'a, super::audio_view::CompletedAudioView>;
