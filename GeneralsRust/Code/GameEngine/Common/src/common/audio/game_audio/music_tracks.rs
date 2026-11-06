use super::*;

impl AudioManager {
    pub fn add_track_name(&mut self, track_name: String) {
        self.music_tracks.push(track_name);
    }

    /// C++ track names in admission order for this audio manager.
    pub fn music_track_names(&self) -> &[AsciiString] {
        &self.music_tracks
    }

    pub fn set_music_track_name(&mut self, track_name: String) {
        self.current_music_track = track_name;
    }

    /// C++ `MilesAudioManager::getMusicTrackName` (MilesAudioManager.cpp:1389-1416):
    /// pending AR_Play music request first, then a playing AT_Music stream.
    pub fn get_music_track_name(&self) -> String {
        for request in &self.audio_requests {
            if request.request != RequestType::Play || !request.use_pending_event {
                continue;
            }
            if let Some(event) = request.get_pending_event() {
                if event
                    .get_audio_event_info()
                    .is_some_and(|info| info.sound_type == AudioType::Music)
                {
                    return event.get_event_name().to_string();
                }
            }
        }

        let mut playing_music: Option<(AudioHandle, String)> = None;
        for event in self.active_audio_events.values() {
            if event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type == AudioType::Music)
            {
                let handle = event.get_playing_handle();
                if playing_music
                    .as_ref()
                    .is_none_or(|(oldest, _)| handle < *oldest)
                {
                    playing_music = Some((handle, event.get_event_name().to_string()));
                }
            }
        }
        if let Some((_, name)) = playing_music {
            return name;
        }

        self.current_music_track.clone()
    }

    pub fn next_track_name(&self, current_track: &str) -> String {
        if let Some(pos) = self.music_tracks.iter().position(|x| x == current_track) {
            let next_pos = (pos + 1) % self.music_tracks.len();
            self.music_tracks[next_pos].clone()
        } else if !self.music_tracks.is_empty() {
            self.music_tracks[0].clone()
        } else {
            String::new()
        }
    }

    /// C++ `MilesAudioManager::nextMusicTrack` (MilesAudioManager.cpp:1313-1331):
    /// stop playing AT_Music via AHSV_StopTheMusic, then addAudioEvent(next).
    pub fn next_music_track(&mut self) -> String {
        let current = self.playing_or_current_music_name();
        self.remove_audio_event(AHSV_STOP_THE_MUSIC);
        let next_track = self.next_track_name(&current);
        self.queue_music_track(&next_track);
        next_track
    }

    /// C++ `MilesAudioManager::prevMusicTrack` (MilesAudioManager.cpp:1334-1351).
    pub fn prev_music_track(&mut self) -> String {
        let current = self.playing_or_current_music_name();
        self.remove_audio_event(AHSV_STOP_THE_MUSIC);
        let prev_track = self.prev_track_name(&current);
        self.queue_music_track(&prev_track);
        prev_track
    }
    pub fn prev_track_name(&self, current_track: &str) -> String {
        if let Some(pos) = self.music_tracks.iter().position(|x| x == current_track) {
            let prev_pos = if pos == 0 {
                self.music_tracks.len() - 1
            } else {
                pos - 1
            };
            self.music_tracks[prev_pos].clone()
        } else if !self.music_tracks.is_empty() {
            self.music_tracks[self.music_tracks.len() - 1].clone()
        } else {
            String::new()
        }
    }

    pub(super) fn playing_or_current_music_name(&self) -> String {
        let mut playing_music: Option<(AudioHandle, String)> = None;
        for event in self.active_audio_events.values() {
            if event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type == AudioType::Music)
            {
                let handle = event.get_playing_handle();
                if playing_music
                    .as_ref()
                    .is_none_or(|(oldest, _)| handle < *oldest)
                {
                    playing_music = Some((handle, event.get_event_name().to_string()));
                }
            }
        }
        playing_music
            .map(|(_, name)| name)
            .unwrap_or_else(|| self.current_music_track.clone())
    }

    pub(super) fn queue_music_track(&mut self, track_name: &str) {
        if track_name.is_empty() {
            return;
        }
        self.current_music_track = track_name.to_string();
        let mut event = AudioEventRts::with_event_name(track_name);
        if let Some(info) = self.find_audio_event_info(track_name) {
            event.set_audio_event_info(info);
        }
        let _ = self.add_audio_event(&event);
    }

    /// C++ `MilesAudioManager::isMusicPlayingFromCD` flag mirror
    /// (`m_musicPlayingFromCD`). Field exists here; the accessor is what
    /// `platform::cd::unload_music_files_from_cd` queries.
    pub fn is_music_playing_from_cd(&self) -> Bool {
        self.music_playing_from_cd
    }

    /// C++ `MilesAudioManager::isMusicPlaying` (MilesAudioManager.cpp:1355-1366).
    pub fn is_music_playing(&self) -> Bool {
        self.active_audio_events.values().any(|event| {
            event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type == AudioType::Music)
                && self
                    .playback_recipient
                    .with_hook(|hook| {
                        let handle = event.get_playing_handle();
                        hook.is_playing(handle) && !hook.is_sink_paused(handle)
                    })
                    .unwrap_or(true)
        })
    }

    /// C++ `MilesAudioManager::hasMusicTrackCompleted` (MilesAudioManager.cpp:1370-1385).
    pub fn has_music_track_completed(&self, track_name: &str, number_of_times: Int) -> Bool {
        self.music_track_completed_selected(track_name, number_of_times, None)
    }

    /// The same source music-loop observation, restricted to the receiving session's receipts.
    pub fn has_owned_music_track_completed(
        &self,
        track_name: &str,
        number_of_times: Int,
        receipts: &[AudioHandle],
    ) -> Bool {
        self.music_track_completed_selected(track_name, number_of_times, Some(receipts))
    }

    pub(super) fn music_track_completed_selected(
        &self,
        track_name: &str,
        number_of_times: Int,
        receipts: Option<&[AudioHandle]>,
    ) -> Bool {
        self.active_audio_events.values().any(|event| {
            receipts.is_none_or(|owned| owned.contains(&event.get_playing_handle()))
                && event.get_event_name() == track_name
                && event
                    .get_audio_event_info()
                    .is_some_and(|info| info.sound_type == AudioType::Music)
                && self
                    .playback_recipient
                    .with_hook(|hook| {
                        hook.music_loop_count(event.get_playing_handle()) >= number_of_times
                    })
                    .unwrap_or(false)
        })
    }
}
