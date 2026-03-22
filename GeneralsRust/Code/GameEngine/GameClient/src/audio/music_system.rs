//! # Music System
//!
//! Mood-based music playback with crossfade support.
//!
//! Ported from C++ `GameMusic.cpp` / `MusicManager`.
//!
//! The music system maintains playlists for several moods (peaceful, combat,
//! tense, victory) and crossfades between tracks when the mood changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;

use super::audio_engine::{AudioEngine, AudioHandle, AudioPosition};

// ---------------------------------------------------------------------------
// MusicMood
// ---------------------------------------------------------------------------

/// The current mood determines which playlist the music system draws from.
/// Matches the C++ concept of mood-driven music selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MusicMood {
    /// Default / ambient exploration music.
    Peaceful,
    /// Active combat music.
    Combat,
    /// Tension-building music (building destroyed, enemy approaching, etc.).
    Tense,
    /// Victory / mission-complete stinger.
    Victory,
    /// Defeat / mission-failed stinger.
    Defeat,
}

impl Default for MusicMood {
    fn default() -> Self {
        Self::Peaceful
    }
}

impl MusicMood {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "peaceful" | "normal" | "peace" => Some(Self::Peaceful),
            "combat" | "fight" | "battle" => Some(Self::Combat),
            "tense" | "tension" => Some(Self::Tense),
            "victory" | "win" => Some(Self::Victory),
            "defeat" | "lose" | "loss" => Some(Self::Defeat),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Peaceful => "Peaceful",
            Self::Combat => "Combat",
            Self::Tense => "Tense",
            Self::Victory => "Victory",
            Self::Defeat => "Defeat",
        }
    }
}

// ---------------------------------------------------------------------------
// MusicTrack
// ---------------------------------------------------------------------------

/// Describes a single music track loaded from INI.
/// Matches C++ `MusicTrack`.
#[derive(Debug, Clone)]
pub struct MusicTrack {
    /// Index in the track list.
    pub index: usize,
    /// Logical name of the track.
    pub name: String,
    /// File name (relative to the music folder).
    pub filename: String,
    /// Base mixing level for this track (0.0..=1.0).
    pub volume: f32,
    /// Whether this track is ambient (looping) or a one-shot stinger.
    pub ambient: bool,
    /// Mood(s) this track belongs to.
    pub moods: Vec<MusicMood>,
}

// ---------------------------------------------------------------------------
// CrossfadeState
// ---------------------------------------------------------------------------

/// State machine for crossfading between two tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrossfadeState {
    /// Only one track playing (or none).
    None,
    /// Fading out the old track and fading in the new one.
    Fading,
}

// ---------------------------------------------------------------------------
// MusicSystem
// ---------------------------------------------------------------------------

/// Manages mood-based music playlists, track sequencing, and crossfade.
///
/// Matches C++ `MusicManager`.
pub struct MusicSystem {
    /// All registered tracks.
    tracks: Vec<MusicTrack>,

    /// Per-mood playlists (indices into `tracks`).
    playlists: HashMap<MusicMood, Vec<usize>>,

    /// Current mood.
    mood: MusicMood,

    /// Handle of the currently playing track (0 = none).
    current_handle: AudioHandle,
    /// Handle of the track being faded out during crossfade (0 = none).
    fading_handle: AudioHandle,

    /// Crossfade state.
    crossfade_state: CrossfadeState,
    /// Duration of the crossfade in seconds.
    crossfade_duration: Duration,
    /// When the crossfade started.
    crossfade_start: Option<Instant>,

    /// Index of the current track within the current playlist.
    current_track_index: usize,

    /// Whether music is enabled.
    enabled: bool,
    /// Whether music is currently playing.
    playing: bool,
    /// Shuffle the playlist (random order).
    shuffle: bool,

    /// Track names list (for next/prev navigation, matches C++ m_musicTracks).
    track_names: Vec<String>,
    /// Current position in the sequential track list.
    sequential_index: usize,
}

impl MusicSystem {
    /// Create a new music system with default settings.
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            playlists: HashMap::new(),
            mood: MusicMood::Peaceful,
            current_handle: 0,
            fading_handle: 0,
            crossfade_state: CrossfadeState::None,
            crossfade_duration: Duration::from_secs(3),
            crossfade_start: None,
            current_track_index: 0,
            enabled: true,
            playing: false,
            shuffle: true,
            track_names: Vec::new(),
            sequential_index: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Track management
    // -----------------------------------------------------------------------

    /// Add a track to the system.
    pub fn add_track(&mut self, track: MusicTrack) {
        let idx = self.tracks.len();
        self.track_names.push(track.name.clone());
        for &mood in &track.moods {
            self.playlists.entry(mood).or_default().push(idx);
        }
        self.tracks.push(track);
    }

    /// Register a track name (for sequential next/prev, matches C++).
    pub fn add_track_name(&mut self, name: &str) {
        self.track_names.push(name.to_string());
    }

    /// Get the name of the next track in the sequential list.
    pub fn next_track_name(&self, current: &str) -> Option<String> {
        if self.track_names.is_empty() {
            return None;
        }
        let pos = self
            .track_names
            .iter()
            .position(|t| t == current)
            .unwrap_or(0);
        let next = (pos + 1) % self.track_names.len();
        Some(self.track_names[next].clone())
    }

    /// Get the name of the previous track in the sequential list.
    pub fn prev_track_name(&self, current: &str) -> Option<String> {
        if self.track_names.is_empty() {
            return None;
        }
        let pos = self
            .track_names
            .iter()
            .position(|t| t == current)
            .unwrap_or(0);
        let prev = if pos == 0 {
            self.track_names.len() - 1
        } else {
            pos - 1
        };
        Some(self.track_names[prev].clone())
    }

    // -----------------------------------------------------------------------
    // Playback control
    // -----------------------------------------------------------------------

    /// Start playing music for the current mood.
    /// The `engine` reference is used to actually play audio files.
    pub fn play(&mut self, engine: &mut AudioEngine) -> AudioHandle {
        if !self.enabled || self.tracks.is_empty() {
            return 0;
        }

        let playlist = self.playlists.get(&self.mood).cloned().unwrap_or_default();
        if playlist.is_empty() {
            // Fall back to any track.
            let indices: Vec<usize> = (0..self.tracks.len()).collect();
            self.pick_and_play(&indices, engine)
        } else {
            self.pick_and_play(&playlist, engine)
        }
    }

    /// Stop all music.
    pub fn stop(&mut self, engine: &mut AudioEngine) {
        if self.current_handle != 0 {
            engine.stop_event(self.current_handle);
            self.current_handle = 0;
        }
        if self.fading_handle != 0 {
            engine.stop_event(self.fading_handle);
            self.fading_handle = 0;
        }
        self.playing = false;
        self.crossfade_state = CrossfadeState::None;
        self.crossfade_start = None;
    }

    /// Advance to the next track in the current playlist.
    pub fn next_track(&mut self, engine: &mut AudioEngine) -> AudioHandle {
        if let Some(playlist) = self.playlists.get(&self.mood).cloned() {
            if !playlist.is_empty() {
                self.current_track_index = (self.current_track_index + 1) % playlist.len();
            }
        }
        self.play(engine)
    }

    /// Go back to the previous track.
    pub fn prev_track(&mut self, engine: &mut AudioEngine) -> AudioHandle {
        if let Some(playlist) = self.playlists.get(&self.mood).cloned() {
            if !playlist.is_empty() {
                self.current_track_index = if self.current_track_index == 0 {
                    playlist.len() - 1
                } else {
                    self.current_track_index - 1
                };
            }
        }
        self.play(engine)
    }

    // -----------------------------------------------------------------------
    // Mood control
    // -----------------------------------------------------------------------

    /// Change the music mood and crossfade to a track from the new mood's
    /// playlist.
    pub fn set_mood(&mut self, mood: MusicMood, engine: &mut AudioEngine) {
        if self.mood == mood {
            return;
        }
        self.mood = mood;
        self.current_track_index = 0;

        // Start crossfade.
        if self.current_handle != 0 && self.playing {
            self.fading_handle = self.current_handle;
            self.current_handle = 0;
            self.crossfade_state = CrossfadeState::Fading;
            self.crossfade_start = Some(Instant::now());
        }

        // Start new track.
        self.play(engine);
    }

    /// Get the current mood.
    pub fn mood(&self) -> MusicMood {
        self.mood
    }

    // -----------------------------------------------------------------------
    // Volume
    // -----------------------------------------------------------------------

    /// Set the crossfade duration.
    pub fn set_crossfade_duration(&mut self, seconds: f64) {
        self.crossfade_duration = Duration::from_secs_f64(seconds.max(0.5));
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Whether music is currently playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Whether a specific track has completed playback.
    pub fn has_track_completed(&self, track_name: &str, _number_of_times: u32) -> bool {
        // This would require tracking play counts; for now return false.
        let _ = track_name;
        false
    }

    /// Get the name of the currently playing track.
    pub fn current_track_name(&self) -> Option<String> {
        let playlist = self.playlists.get(&self.mood)?;
        if playlist.is_empty() {
            return None;
        }
        let idx = playlist.get(self.current_track_index)?;
        self.tracks.get(*idx).map(|t| t.name.clone())
    }

    /// Enable or disable music.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.playing = false;
        }
    }

    /// Whether music is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable shuffle mode.
    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.shuffle = shuffle;
    }

    /// Update the music system (call every frame).
    /// Handles crossfade progression and auto-advance.
    pub fn update(&mut self, engine: &mut AudioEngine) {
        // Handle crossfade.
        if self.crossfade_state == CrossfadeState::Fading {
            if let Some(start) = self.crossfade_start {
                let elapsed = start.elapsed();
                let progress =
                    (elapsed.as_secs_f64() / self.crossfade_duration.as_secs_f64()).clamp(0.0, 1.0);

                // Fade out old track.
                if self.fading_handle != 0 {
                    // kira does not natively expose per-frame volume control
                    // from a Handle.  The engine will handle volume setting
                    // when we integrate more deeply.  For now we just stop
                    // the fading track when the crossfade is done.
                    if progress >= 1.0 {
                        engine.stop_event(self.fading_handle);
                        self.fading_handle = 0;
                        self.crossfade_state = CrossfadeState::None;
                        self.crossfade_start = None;
                    }
                }
            }
        }

        // Auto-advance: if the current track finished, play the next one.
        if self.playing && self.current_handle != 0 && !engine.is_playing(self.current_handle) {
            self.current_handle = 0;
            self.next_track(engine);
        }
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn pick_and_play(&mut self, playlist: &[usize], engine: &mut AudioEngine) -> AudioHandle {
        if playlist.is_empty() {
            return 0;
        }

        let track_idx = if self.shuffle {
            let mut rng = rand::thread_rng();
            *playlist.choose(&mut rng).unwrap_or(&playlist[0])
        } else {
            playlist[self.current_track_index % playlist.len()]
        };

        if let Some(track) = self.tracks.get(track_idx) {
            let filename = if track.filename.is_empty() {
                track.name.clone()
            } else {
                track.filename.clone()
            };

            let handle = engine.play_event(&filename, None::<AudioPosition>);
            if handle != 0 {
                self.current_handle = handle;
                self.playing = true;
            }
            handle
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> AudioEngine {
        AudioEngine::new().unwrap()
    }

    #[test]
    fn test_mood_from_name() {
        assert_eq!(MusicMood::from_name("peaceful"), Some(MusicMood::Peaceful));
        assert_eq!(MusicMood::from_name("Combat"), Some(MusicMood::Combat));
        assert_eq!(MusicMood::from_name("TENSE"), Some(MusicMood::Tense));
        assert_eq!(MusicMood::from_name("victory"), Some(MusicMood::Victory));
        assert_eq!(MusicMood::from_name("unknown"), None);
    }

    #[test]
    fn test_music_system_add_track() {
        let mut sys = MusicSystem::new();
        sys.add_track(MusicTrack {
            index: 0,
            name: "TrackA".to_string(),
            filename: "track_a.mp3".to_string(),
            volume: 0.8,
            ambient: true,
            moods: vec![MusicMood::Peaceful],
        });
        assert_eq!(sys.tracks.len(), 1);
    }

    #[test]
    fn test_next_prev_track_names() {
        let mut sys = MusicSystem::new();
        sys.add_track_name("Alpha");
        sys.add_track_name("Beta");
        sys.add_track_name("Gamma");
        assert_eq!(sys.next_track_name("Alpha"), Some("Beta".to_string()));
        assert_eq!(sys.next_track_name("Gamma"), Some("Alpha".to_string()));
        assert_eq!(sys.prev_track_name("Alpha"), Some("Gamma".to_string()));
        assert_eq!(sys.prev_track_name("Beta"), Some("Alpha".to_string()));
    }

    #[test]
    fn test_set_enabled() {
        let mut sys = MusicSystem::new();
        assert!(sys.is_enabled());
        sys.set_enabled(false);
        assert!(!sys.is_enabled());
    }
}
