//! One match's script mix and ordered semantic audio admissions.
use super::AudioEventRequest;
use game_engine::common::audio::{game_audio::AudioAffect, script_policy::ScriptAudioPolicy};
#[derive(Debug, Default)]
pub struct SessionAudio {
    policy: ScriptAudioPolicy,
    requests: Vec<AudioEventRequest>,
}
impl SessionAudio {
    pub fn policy(&self) -> &ScriptAudioPolicy {
        &self.policy
    }
    pub fn push(&mut self, mut request: AudioEventRequest) {
        request.script_audio_policy = Some(self.policy.clone());
        self.requests.push(request);
    }
    /// Discard pending operations without changing the owning session's mix.
    pub fn clear(&mut self) {
        self.requests.clear();
    }
    pub fn reset(&mut self) {
        self.requests.clear();
        self.policy = Default::default();
    }
    pub fn drain_requests(&mut self) -> Vec<AudioEventRequest> {
        std::mem::take(&mut self.requests)
    }
    /// Reinsert already-admitted continuations without recapturing named overrides.
    pub fn restore_requests(&mut self, requests: Vec<AudioEventRequest>) {
        self.requests = requests;
    }
    pub fn extend_admitted(&mut self, requests: Vec<AudioEventRequest>) {
        self.requests.extend(requests);
    }
    pub fn set_volume(&mut self, volume: f32, affect: AudioAffect) {
        self.policy.set_volume(volume, affect);
        // Bus changes affect pending events; named raw-volume overrides remain source-time.
        for event in &mut self.requests {
            if let Some(policy) = &mut event.script_audio_policy {
                policy.volumes = self.policy.volumes;
            }
        }
    }
    pub fn pause_pending(&mut self) {
        self.requests.retain(|event| event.stop);
    }
    pub fn set_named_volume(&mut self, name: &str, volume: f32) {
        self.policy.set_named_volume(name, volume);
    }
    pub fn restore_policy(&mut self, policy: ScriptAudioPolicy) {
        self.policy = policy;
    }
}
impl std::ops::Deref for SessionAudio {
    type Target = [AudioEventRequest];
    fn deref(&self) -> &Self::Target {
        &self.requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_name_overrides_are_source_time_but_bus_updates_reach_pending() {
        let mut a = SessionAudio::default();
        let mut b = SessionAudio::default();
        a.set_named_volume("LateResolved", 0.2);
        a.push(AudioEventRequest::new("LateResolved"));
        a.set_named_volume("LateResolved", 0.7);
        b.push(AudioEventRequest::new("LateResolved"));
        a.set_volume(0.4, AudioAffect::SoundEffects);
        let pending = a.drain_requests();
        let captured = pending[0].script_audio_policy.as_ref().unwrap();
        assert_eq!(captured.override_for("LateResolved"), Some(0.2));
        assert_eq!(captured.volumes.sound, 0.4);
        assert_eq!(
            b[0].script_audio_policy
                .as_ref()
                .unwrap()
                .override_for("LateResolved"),
            None
        );
        assert_eq!(a.policy().override_for("LateResolved"), Some(0.7));
        a.clear();
        assert_eq!(a.policy().override_for("LateResolved"), Some(0.7));
        b.reset();
        assert_eq!(a.policy().volumes.sound, 0.4);
        a.reset();
        assert_eq!(a.policy(), &ScriptAudioPolicy::default());
    }
}

pub(crate) struct SessionAudioSelection<'a> {
    pub(crate) logic: &'a game_engine::common::random_value::SessionRandom,
    pub(crate) audio: &'a mut game_engine::common::random_value::RandomState,
}
impl game_engine::common::audio::audio_event_rts::AudioEventSelection
    for SessionAudioSelection<'_>
{
    fn index(&mut self, logical: bool, count: usize) -> usize {
        if count == 0 {
            0
        } else if logical {
            self.logic.next_u32() as usize % count
        } else {
            self.audio.next_u32() as usize % count
        }
    }
    fn real(&mut self, low: f32, high: f32) -> f32 {
        self.audio.next_real(low, high)
    }
}
