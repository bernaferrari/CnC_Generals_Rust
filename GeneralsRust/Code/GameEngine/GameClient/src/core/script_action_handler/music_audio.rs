//! Borrowed music recipient for the actual ScriptActions fade/track operation.
use super::{
    AHSV_STOP_THE_MUSIC, AHSV_STOP_THE_MUSIC_FADE, AudioEventRts, GameClientScriptActionHandler,
    TheAudio,
};
use game_engine::common::audio::game_audio::AudioManager;

pub(super) trait MusicAudioRecipient {
    fn remove(&mut self, handle: u32);
    fn add_track(&mut self, track: &str, fade_in: bool, player: Option<u32>) -> u32;
}

impl MusicAudioRecipient for &TheAudio {
    fn remove(&mut self, handle: u32) {
        TheAudio::remove_audio_event(self, handle);
    }
    fn add_track(&mut self, track: &str, fade_in: bool, player: Option<u32>) -> u32 {
        let mut event = AudioEventRts::new(track);
        event.set_should_fade(fade_in);
        if let Some(player) = player {
            event.set_player_index(player);
        }
        TheAudio::add_audio_event(self, &event)
    }
}

impl MusicAudioRecipient for AudioManager {
    fn remove(&mut self, handle: u32) {
        self.remove_audio_event(handle);
    }
    fn add_track(&mut self, track: &str, fade_in: bool, player: Option<u32>) -> u32 {
        let mut event = game_engine::common::audio::AudioEventRts::with_event_name(track);
        event.set_should_fade(fade_in);
        if let Some(player) = player {
            event.set_player_index(player as i32);
        }
        self.add_audio_event(&event)
    }
}

impl GameClientScriptActionHandler {
    /// ScriptActions.cpp3271: sentinel first, then the new track's fade/player facts.
    pub(super) fn music_set_track_with_recipient(
        &self,
        recipient: &mut impl MusicAudioRecipient,
        track: &str,
        fade_out: bool,
        fade_in: bool,
        player: Option<u32>,
    ) -> u32 {
        recipient.remove(if fade_out {
            AHSV_STOP_THE_MUSIC_FADE
        } else {
            AHSV_STOP_THE_MUSIC
        });
        recipient.add_track(track, fade_in, player)
    }

    /// LoadScreen.cpp1296: stopping music is the fade sentinel, not handle removal.
    pub(super) fn stop_music_with_recipient(&self, recipient: &mut impl MusicAudioRecipient) {
        recipient.remove(AHSV_STOP_THE_MUSIC_FADE);
    }
}

#[cfg(test)]
pub(super) fn test_audio(tracks: &[&str]) -> AudioManager {
    use game_engine::common::audio::{AudioEventInfo, AudioPriority, AudioType};
    let mut manager =
        AudioManager::with_playback_hook(Box::new(logical_backend::MusicVoices::default()));
    manager.init();
    for track in tracks {
        manager.register_audio_event_info(AudioEventInfo {
            sound_type: AudioType::Music,
            audio_name: (*track).into(),
            volume: 1.,
            sounds: vec![format!("{track}.logical")],
            filename: format!("{track}.logical"),
            sound_type_field: AudioType::Music,
            priority: AudioPriority::Normal,
            min_distance: 25.,
            max_distance: 1000.,
            ..Default::default()
        });
    }
    manager
}

#[cfg(test)]
pub(super) fn tick_test_audio(manager: &mut AudioManager) {
    use game_engine::common::audio::game_audio::AudioViewObservation;
    manager.update_with_view(
        AudioViewObservation {
            ground_position: Default::default(),
            heading: 0.,
            camera_position: Default::default(),
            ground_height: 0.,
        },
        &mut |_| true,
    );
}

#[cfg(test)]
mod logical_backend {
    use game_engine::common::audio::AudioHandle;
    use game_engine::common::audio::game_audio::SoundPlaybackHook;
    use game_engine::common::audio::{AudioEventRts, AudioType};
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Owned logical streams, independent of device/file availability. The
    /// existing Send+Sync backend boundary requires atomic voice state.
    #[derive(Default)]
    pub(super) struct MusicVoices {
        active: AtomicU64,
        paused: AtomicU64,
    }
    impl MusicVoices {
        fn bit(handle: AudioHandle) -> Option<u64> {
            handle
                .checked_sub(1000)
                .filter(|index| *index < 64)
                .map(|index| 1u64 << index)
        }
    }
    impl SoundPlaybackHook for MusicVoices {
        fn play(&self, event: &AudioEventRts) -> Result<(), String> {
            if !event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type == AudioType::Music)
            {
                return Err("logical music recipient requires an admitted music event".into());
            }
            let bit =
                Self::bit(event.get_playing_handle()).ok_or("logical voice capacity exceeded")?;
            self.active.fetch_or(bit, Ordering::SeqCst);
            Ok(())
        }
        fn stop(&self, handle: AudioHandle) {
            if let Some(bit) = Self::bit(handle) {
                self.active.fetch_and(!bit, Ordering::SeqCst);
                self.paused.fetch_and(!bit, Ordering::SeqCst);
            }
        }
        fn pause(&self, handle: AudioHandle) {
            if let Some(bit) = Self::bit(handle) {
                self.paused.fetch_or(bit, Ordering::SeqCst);
            }
        }
        fn resume(&self, handle: AudioHandle) {
            if let Some(bit) = Self::bit(handle) {
                self.paused.fetch_and(!bit, Ordering::SeqCst);
            }
        }
        fn is_playing(&self, handle: AudioHandle) -> bool {
            Self::bit(handle).is_some_and(|bit| self.active.load(Ordering::SeqCst) & bit != 0)
        }
        fn is_sink_paused(&self, handle: AudioHandle) -> bool {
            Self::bit(handle).is_some_and(|bit| self.paused.load(Ordering::SeqCst) & bit != 0)
        }
    }
}
