//! Receiving backend selection for one AudioManager.
use super::{SoundPlaybackHook, with_sound_playback_hook};
#[derive(Default)]
pub(super) struct PlaybackRecipient {
    owned: Option<Box<dyn SoundPlaybackHook>>,
}
impl PlaybackRecipient {
    pub(super) fn owned(recipient: Box<dyn SoundPlaybackHook>) -> Self {
        Self {
            owned: Some(recipient),
        }
    }
    pub(super) fn with_hook<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&dyn SoundPlaybackHook) -> R,
    {
        match self.owned.as_deref() {
            Some(recipient) => Some(f(recipient)),
            None => with_sound_playback_hook(f),
        }
    }
}
