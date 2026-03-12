////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameMusic - Music management system
//! Westwood Studios Pacific
//! Converted to Rust

use crate::common::audio::{
    audio_event_rts::{AudioEventRts, AudioHandle},
    audio_request::{AudioRequest, RequestType},
};

// Type aliases
pub type AsciiString = String;
pub type Real = f32;
pub type Bool = bool;
pub type Int = i32;

const AHSV_STOP_THE_MUSIC: AudioHandle = 0xFFFF_FFF0;
const AHSV_STOP_THE_MUSIC_FADE: AudioHandle = 0xFFFF_FFF1;

/// The MusicTrack struct holds all information about a music track.
/// Place data in TrackInfo that is useful to the game code in determining
/// what tracks to play.
#[derive(Debug, Clone)]
pub struct MusicTrack {
    pub index: Int,
    pub name: AsciiString,
    pub filename: AsciiString,
    pub volume: Real,
    pub ambient: Bool,
    pub next: Option<Box<MusicTrack>>,
    pub prev: Option<Box<MusicTrack>>,
}

impl MusicTrack {
    pub fn new() -> Self {
        MusicTrack {
            index: 0,
            name: String::new(),
            filename: String::new(),
            volume: 0.5,
            ambient: false,
            next: None,
            prev: None,
        }
    }

    pub fn new_with_data(
        index: Int,
        name: String,
        filename: String,
        volume: Real,
        ambient: Bool,
    ) -> Self {
        MusicTrack {
            index,
            name,
            filename,
            volume,
            ambient,
            next: None,
            prev: None,
        }
    }
}

impl Default for MusicTrack {
    fn default() -> Self {
        Self::new()
    }
}

/// Music Manager - handles music playback and track management
pub struct MusicManagerImpl {
    // Current track information
    current_track: Option<MusicTrack>,

    // Playback state
    is_playing: Bool,
    current_handle: Option<AudioHandle>,

    // Track list
    tracks: Vec<MusicTrack>,
    current_track_index: usize,

    // Volume control
    volume: Real,
}

impl MusicManagerImpl {
    pub fn new() -> Self {
        MusicManagerImpl {
            current_track: None,
            is_playing: false,
            current_handle: None,
            tracks: Vec::new(),
            current_track_index: 0,
            volume: 0.5,
        }
    }

    /// Play a music track using the provided audio event
    pub fn play_track(&mut self, event_to_use: AudioEventRts) {
        // Create an audio request to play the music
        let audio_request = AudioRequest::new_with_event(RequestType::Play, event_to_use);

        // In the original C++, this would append to TheAudio's request list
        // For now, we'll store the handle if available
        if let Some(event) = audio_request.get_pending_event() {
            self.current_handle = Some(event.get_playing_handle());
            self.is_playing = true;
        }

        // In a real implementation, we would:
        // TheAudio->appendAudioRequest(audio_request);
    }

    /// Stop the currently playing track
    pub fn stop_track(&mut self, event_to_remove: AudioHandle) {
        let audio_request = AudioRequest::new_with_handle(RequestType::Stop, event_to_remove);

        // Reset our state
        if Some(event_to_remove) == self.current_handle {
            self.current_handle = None;
            self.is_playing = false;
        }

        // In a real implementation, we would:
        // TheAudio->appendAudioRequest(audio_request);
    }

    /// Add a new track to our collection
    pub fn add_track(&mut self, track: MusicTrack) {
        self.tracks.push(track);
    }

    /// Get the current track
    pub fn get_current_track(&self) -> Option<&MusicTrack> {
        self.current_track.as_ref()
    }

    /// Set the current track by index
    pub fn set_current_track(&mut self, index: usize) -> Result<(), &'static str> {
        if index >= self.tracks.len() {
            return Err("Track index out of bounds");
        }

        self.current_track_index = index;
        self.current_track = Some(self.tracks[index].clone());
        Ok(())
    }

    /// Move to the next track
    pub fn next_track(&mut self) -> Option<&MusicTrack> {
        if !self.tracks.is_empty() {
            self.current_track_index = (self.current_track_index + 1) % self.tracks.len();
            self.current_track = Some(self.tracks[self.current_track_index].clone());
            self.current_track.as_ref()
        } else {
            None
        }
    }

    /// Move to the previous track
    pub fn prev_track(&mut self) -> Option<&MusicTrack> {
        if !self.tracks.is_empty() {
            self.current_track_index = if self.current_track_index == 0 {
                self.tracks.len() - 1
            } else {
                self.current_track_index - 1
            };
            self.current_track = Some(self.tracks[self.current_track_index].clone());
            self.current_track.as_ref()
        } else {
            None
        }
    }

    /// Find a track by name
    pub fn find_track_by_name(&self, name: &str) -> Option<&MusicTrack> {
        self.tracks.iter().find(|track| track.name == name)
    }

    /// Get all track names
    pub fn get_track_names(&self) -> Vec<&str> {
        self.tracks
            .iter()
            .map(|track| track.name.as_str())
            .collect()
    }

    /// Check if music is currently playing
    pub fn is_playing(&self) -> Bool {
        self.is_playing
    }

    /// Get the current playing handle
    pub fn get_current_handle(&self) -> Option<AudioHandle> {
        self.current_handle
    }

    /// Set the master volume for music
    pub fn set_volume(&mut self, new_volume: Real) {
        self.volume = new_volume.clamp(0.0, 1.0);
    }

    /// Get the current volume
    pub fn get_volume(&self) -> Real {
        self.volume
    }

    /// Get the number of tracks
    pub fn get_track_count(&self) -> usize {
        self.tracks.len()
    }

    /// Clear all tracks
    pub fn clear_tracks(&mut self) {
        self.tracks.clear();
        self.current_track = None;
        self.current_track_index = 0;
    }

    /// Get track at specific index
    pub fn get_track(&self, index: usize) -> Option<&MusicTrack> {
        self.tracks.get(index)
    }

    /// Remove track by index
    pub fn remove_track(&mut self, index: usize) -> Option<MusicTrack> {
        if index < self.tracks.len() {
            let removed_track = self.tracks.remove(index);

            // Adjust current track index if necessary
            if index <= self.current_track_index && self.current_track_index > 0 {
                self.current_track_index -= 1;
            }

            // Update current track if we removed it
            if self.tracks.is_empty() {
                self.current_track = None;
                self.current_track_index = 0;
            } else if self.current_track_index >= self.tracks.len() {
                self.current_track_index = 0;
                self.current_track = Some(self.tracks[0].clone());
            } else {
                self.current_track = Some(self.tracks[self.current_track_index].clone());
            }

            Some(removed_track)
        } else {
            None
        }
    }

    /// Stop all music
    pub fn stop_all(&mut self) {
        if let Some(handle) = self.current_handle {
            self.stop_track(handle);
        }
    }

    /// Play a track by name
    pub fn play_track_by_name(&mut self, track_name: &str) -> Result<(), &'static str> {
        if let Some(index) = self
            .tracks
            .iter()
            .position(|track| track.name == track_name)
        {
            self.set_current_track(index)?;

            if let Some(track) = &self.current_track {
                // Create an audio event for this track
                let mut audio_event = AudioEventRts::with_event_name(&track.name);
                audio_event.set_volume(track.volume);

                self.play_track(audio_event);
                Ok(())
            } else {
                Err("Failed to set current track")
            }
        } else {
            Err("Track not found")
        }
    }

    /// Shuffle the track list
    pub fn shuffle_tracks(&mut self) {
        use rand::seq::SliceRandom;
        use rand::thread_rng;

        let mut rng = thread_rng();
        self.tracks.shuffle(&mut rng);

        // Reset to first track
        if !self.tracks.is_empty() {
            self.current_track_index = 0;
            self.current_track = Some(self.tracks[0].clone());
        }
    }

    /// Get tracks that are marked as ambient
    pub fn get_ambient_tracks(&self) -> Vec<&MusicTrack> {
        self.tracks.iter().filter(|track| track.ambient).collect()
    }

    /// Get tracks that are not marked as ambient
    pub fn get_non_ambient_tracks(&self) -> Vec<&MusicTrack> {
        self.tracks.iter().filter(|track| !track.ambient).collect()
    }
}

impl Default for MusicManagerImpl {
    fn default() -> Self {
        Self::new()
    }
}

// Implement the trait from game_audio for compatibility
impl super::game_audio::MusicManager for MusicManagerImpl {
    fn add_audio_event(&mut self, event: AudioEventRts) {
        self.play_track(event);
    }

    fn remove_audio_event(&mut self, handle: AudioHandle) {
        if handle == AHSV_STOP_THE_MUSIC || handle == AHSV_STOP_THE_MUSIC_FADE {
            self.stop_all();
            return;
        }
        self.stop_track(handle);
    }
}

/// Factory function to create a new music manager
pub fn create_music_manager() -> Box<dyn super::game_audio::MusicManager> {
    Box::new(MusicManagerImpl::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_music_track_creation() {
        let track = MusicTrack::new_with_data(
            1,
            "Test Track".to_string(),
            "test.wav".to_string(),
            0.8,
            true,
        );

        assert_eq!(track.index, 1);
        assert_eq!(track.name, "Test Track");
        assert_eq!(track.filename, "test.wav");
        assert_eq!(track.volume, 0.8);
        assert!(track.ambient);
    }

    #[test]
    fn test_music_manager_basic_operations() {
        let mut manager = MusicManagerImpl::new();

        let track1 = MusicTrack::new_with_data(
            1,
            "Track 1".to_string(),
            "track1.wav".to_string(),
            0.5,
            false,
        );
        let track2 = MusicTrack::new_with_data(
            2,
            "Track 2".to_string(),
            "track2.wav".to_string(),
            0.7,
            true,
        );

        manager.add_track(track1);
        manager.add_track(track2);

        assert_eq!(manager.get_track_count(), 2);
        assert!(manager.find_track_by_name("Track 1").is_some());
        assert!(manager.find_track_by_name("Nonexistent Track").is_none());

        assert!(manager.set_current_track(0).is_ok());
        assert!(manager.get_current_track().is_some());
        assert_eq!(manager.get_current_track().unwrap().name, "Track 1");
    }

    #[test]
    fn test_track_navigation() {
        let mut manager = MusicManagerImpl::new();

        for i in 0..3 {
            let track = MusicTrack::new_with_data(
                i,
                format!("Track {}", i),
                format!("track{}.wav", i),
                0.5,
                false,
            );
            manager.add_track(track);
        }

        manager.set_current_track(0).unwrap();

        let next = manager.next_track();
        assert!(next.is_some());
        assert_eq!(next.unwrap().name, "Track 1");

        let prev = manager.prev_track();
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().name, "Track 0");
    }
}
