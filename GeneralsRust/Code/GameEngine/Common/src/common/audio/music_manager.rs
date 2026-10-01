//! Music Manager Implementation
//!
//! This module provides a comprehensive music management system that handles
//! background music streaming, crossfading, playlist management, and integration
//! with the overall audio system. It's designed to match the C++ MusicManager API
//! while providing modern streaming capabilities.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rodio_compat::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use symphonia::core::io::MediaSourceStream;

use crate::common::audio::{
    AsciiString, AudioAffect, AudioEventInfo, AudioEventRts, AudioHandle, AudioType, Bool, Coord3D,
    Int, Real, UnsignedInt,
};

/// Music playback state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MusicState {
    Stopped,
    Playing,
    Paused,
    Fading,
    Loading,
    Error,
}

/// Music track information
#[derive(Debug, Clone)]
pub struct MusicTrack {
    /// Track identifier/name
    pub name: String,
    /// File path to the music file
    pub file_path: PathBuf,
    /// Track volume (0.0 to 1.0)
    pub volume: Real,
    /// Whether this track loops
    pub loops: bool,
    /// Fade in duration in seconds
    pub fade_in_duration: Real,
    /// Fade out duration in seconds  
    pub fade_out_duration: Real,
    /// Track priority (higher = more important)
    pub priority: Int,
    /// Associated audio event info
    pub event_info: Option<Arc<AudioEventInfo>>,
    /// Track duration in seconds (if known)
    pub duration: Option<Real>,
    /// Track category (combat, ambient, menu, etc.)
    pub category: MusicCategory,
}

impl MusicTrack {
    pub fn new<P: AsRef<Path>>(name: String, file_path: P) -> Self {
        Self {
            name,
            file_path: file_path.as_ref().to_path_buf(),
            volume: 1.0,
            loops: false,
            fade_in_duration: 0.0,
            fade_out_duration: 0.0,
            priority: 0,
            event_info: None,
            duration: None,
            category: MusicCategory::Ambient,
        }
    }

    pub fn with_volume(mut self, volume: Real) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn with_looping(mut self, loops: bool) -> Self {
        self.loops = loops;
        self
    }

    pub fn with_fade_durations(mut self, fade_in: Real, fade_out: Real) -> Self {
        self.fade_in_duration = fade_in;
        self.fade_out_duration = fade_out;
        self
    }

    pub fn with_priority(mut self, priority: Int) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_category(mut self, category: MusicCategory) -> Self {
        self.category = category;
        self
    }
}

/// Music categories for different game contexts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MusicCategory {
    Menu,
    Ambient,
    Combat,
    Victory,
    Defeat,
    Dramatic,
    Peaceful,
    Custom(u8),
}

/// Music commands for the manager
#[derive(Debug)]
pub enum MusicCommand {
    Play { track: MusicTrack },
    Stop { fade_out: bool },
    Pause,
    Resume,
    SetVolume { volume: Real },
    NextTrack,
    PreviousTrack,
    SetPlaylist { tracks: Vec<MusicTrack> },
    AddTrack { track: MusicTrack },
    RemoveTrack { name: String },
    SetCrossfadeDuration { duration: Real },
    SetCategory { category: MusicCategory },
    Shutdown,
}

/// Music events for notifications
#[derive(Debug, Clone)]
pub enum MusicEvent {
    TrackStarted { name: String },
    TrackFinished { name: String },
    TrackFailed { name: String, error: String },
    PlaylistFinished,
    VolumeChanged { volume: Real },
    StateChanged { state: MusicState },
}

/// Playlist management
#[derive(Debug)]
pub struct Playlist {
    tracks: Vec<MusicTrack>,
    current_index: usize,
    shuffle: bool,
    repeat: bool,
    shuffle_order: Vec<usize>,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: 0,
            shuffle: false,
            repeat: false,
            shuffle_order: Vec::new(),
        }
    }

    pub fn add_track(&mut self, track: MusicTrack) {
        self.tracks.push(track);
        if self.shuffle {
            self.regenerate_shuffle_order();
        }
    }

    pub fn remove_track(&mut self, name: &str) -> bool {
        if let Some(pos) = self.tracks.iter().position(|t| t.name == name) {
            self.tracks.remove(pos);
            if pos <= self.current_index && self.current_index > 0 {
                self.current_index -= 1;
            }
            if self.shuffle {
                self.regenerate_shuffle_order();
            }
            true
        } else {
            false
        }
    }

    pub fn current_track(&self) -> Option<&MusicTrack> {
        if self.shuffle && !self.shuffle_order.is_empty() {
            let shuffle_index = self.current_index % self.shuffle_order.len();
            let track_index = self.shuffle_order[shuffle_index];
            self.tracks.get(track_index)
        } else {
            self.tracks.get(self.current_index)
        }
    }

    pub fn next_track(&mut self) -> Option<&MusicTrack> {
        if self.tracks.is_empty() {
            return None;
        }

        let max_index = if self.shuffle {
            self.shuffle_order.len()
        } else {
            self.tracks.len()
        };

        self.current_index = (self.current_index + 1) % max_index;

        if self.current_index == 0 && !self.repeat {
            None // Reached end of playlist and not repeating
        } else {
            self.current_track()
        }
    }

    pub fn previous_track(&mut self) -> Option<&MusicTrack> {
        if self.tracks.is_empty() {
            return None;
        }

        let max_index = if self.shuffle {
            self.shuffle_order.len()
        } else {
            self.tracks.len()
        };

        self.current_index = if self.current_index == 0 {
            max_index - 1
        } else {
            self.current_index - 1
        };

        self.current_track()
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.shuffle = shuffle;
        if shuffle {
            self.regenerate_shuffle_order();
        }
    }

    pub fn set_repeat(&mut self, repeat: bool) {
        self.repeat = repeat;
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    fn regenerate_shuffle_order(&mut self) {
        self.shuffle_order = (0..self.tracks.len()).collect();

        // Fisher-Yates shuffle
        use rand::Rng;
        let mut rng = rand::rng();
        for i in (1..self.shuffle_order.len()).rev() {
            let j = rng.random_range(0..=i);
            self.shuffle_order.swap(i, j);
        }
    }
}

/// Current playing music state
#[derive(Debug)]
struct PlayingMusic {
    track: MusicTrack,
    sink: Sink,
    handle: AudioHandle,
    start_time: Instant,
    fade_start: Option<Instant>,
    fade_duration: Real,
    target_volume: Real,
    current_volume: Real,
}

/// State shared between the `MusicManager` and its background audio thread.
///
/// The manager talks to a real thread (rule: keep the boundary lock), but the
/// many former per-field `Arc<Mutex<..>>`/`Arc<RwLock<..>>` handles are
/// consolidated into this single `Mutex`-protected struct so there is exactly
/// one boundary lock. Event send order is unchanged: events are still emitted
/// sequentially by the background thread in the same places as before.
#[derive(Debug)]
struct SharedMusicState {
    state: MusicState,
    current_music: Option<PlayingMusic>,
    playlist: Playlist,
    master_volume: Real,
    crossfade_duration: Real,
    current_category: MusicCategory,
    tracks_played: u64,
    total_play_time: Duration,
    track_completion_counts: HashMap<String, u32>,
    /// Event callback; cloned under the lock and sent to from outside it.
    event_sender: Option<Sender<MusicEvent>>,
}

/// Main Music Manager implementation
pub struct MusicManager {
    // Audio system
    stream_handle: OutputStreamHandle,

    // State shared with the background thread (single boundary lock)
    shared: Arc<Mutex<SharedMusicState>>,

    // Communication
    command_sender: Sender<MusicCommand>,
    command_receiver: Arc<Mutex<Receiver<MusicCommand>>>,

    // Main-thread-only state (owned; never touched by the background thread)
    /// Handle pool
    next_handle: AudioHandle,

    /// Track registry
    track_registry: HashMap<String, MusicTrack>,

    // Search paths for music files
    search_paths: Vec<PathBuf>,
}

impl MusicManager {
    /// Create a new music manager
    pub fn new(stream_handle: OutputStreamHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let (command_sender, command_receiver) = mpsc::channel();

        Ok(Self {
            stream_handle,
            shared: Arc::new(Mutex::new(SharedMusicState {
                state: MusicState::Stopped,
                current_music: None,
                playlist: Playlist::new(),
                master_volume: 1.0,
                crossfade_duration: 3.0, // 3 second default crossfade
                current_category: MusicCategory::Ambient,
                tracks_played: 0,
                total_play_time: Duration::ZERO,
                track_completion_counts: HashMap::new(),
                event_sender: None,
            })),
            command_sender,
            command_receiver: Arc::new(Mutex::new(command_receiver)),
            next_handle: 10000, // Start music handles at 10000
            track_registry: HashMap::new(),
            search_paths: vec![
                PathBuf::from("./data/audio/music/"),
                PathBuf::from("./assets/audio/music/"),
                PathBuf::from("./music/"),
            ],
        })
    }

    /// Initialize and start the music manager
    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.start_background_thread();
        Ok(())
    }

    /// Set event callback for music notifications
    pub fn set_event_callback(&self, sender: Sender<MusicEvent>) {
        self.shared.lock().event_sender = Some(sender);
    }

    /// Add search path for music files
    pub fn add_search_path<P: AsRef<Path>>(&mut self, path: P) {
        self.search_paths.push(path.as_ref().to_path_buf());
    }

    /// Register a music track
    pub fn register_track(&mut self, track: MusicTrack) {
        self.track_registry.insert(track.name.clone(), track);
    }

    /// Get registered track by name
    pub fn get_track(&self, name: &str) -> Option<MusicTrack> {
        self.track_registry.get(name).cloned()
    }

    /// Play a specific music track
    pub fn play_track(&mut self, track_name: &str) -> Result<AudioHandle, String> {
        let track = self
            .get_track(track_name)
            .ok_or_else(|| format!("Track '{}' not found", track_name))?;

        self.command_sender
            .send(MusicCommand::Play { track })
            .map_err(|_| "Failed to send play command")?;

        // Return a handle (in real implementation, this would be returned from the background thread)
        let handle = self.next_handle;
        self.next_handle += 1;
        Ok(handle)
    }

    /// Stop current music
    pub fn stop_music(&self, fade_out: bool) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::Stop { fade_out })
            .map_err(|_| "Failed to send stop command")?;
        Ok(())
    }

    /// Pause current music
    pub fn pause_music(&self) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::Pause)
            .map_err(|_| "Failed to send pause command")?;
        Ok(())
    }

    /// Resume paused music
    pub fn resume_music(&self) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::Resume)
            .map_err(|_| "Failed to send resume command")?;
        Ok(())
    }

    /// Set music volume (0.0 to 1.0)
    pub fn set_volume(&self, volume: Real) -> Result<(), String> {
        let clamped_volume = volume.clamp(0.0, 1.0);
        self.command_sender
            .send(MusicCommand::SetVolume {
                volume: clamped_volume,
            })
            .map_err(|_| "Failed to send volume command")?;
        Ok(())
    }

    /// Get current music volume
    pub fn get_volume(&self) -> Real {
        self.shared.lock().master_volume
    }

    /// Go to next track in playlist
    pub fn next_track(&self) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::NextTrack)
            .map_err(|_| "Failed to send next track command")?;
        Ok(())
    }

    /// Go to previous track in playlist  
    pub fn previous_track(&self) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::PreviousTrack)
            .map_err(|_| "Failed to send previous track command")?;
        Ok(())
    }

    /// Set playlist of tracks to cycle through
    pub fn set_playlist(&self, tracks: Vec<MusicTrack>) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::SetPlaylist { tracks })
            .map_err(|_| "Failed to send playlist command")?;
        Ok(())
    }

    /// Add track to current playlist
    pub fn add_to_playlist(&self, track: MusicTrack) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::AddTrack { track })
            .map_err(|_| "Failed to send add track command")?;
        Ok(())
    }

    /// Remove track from playlist
    pub fn remove_from_playlist(&self, name: String) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::RemoveTrack { name })
            .map_err(|_| "Failed to send remove track command")?;
        Ok(())
    }

    /// Set crossfade duration between tracks
    pub fn set_crossfade_duration(&self, duration: Real) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::SetCrossfadeDuration { duration })
            .map_err(|_| "Failed to send crossfade duration command")?;
        Ok(())
    }

    /// Set current music category
    pub fn set_category(&self, category: MusicCategory) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::SetCategory { category })
            .map_err(|_| "Failed to send category command")?;
        Ok(())
    }

    /// Get current music state
    pub fn get_state(&self) -> MusicState {
        self.shared.lock().state
    }

    /// Check if music is currently playing
    pub fn is_playing(&self) -> bool {
        matches!(self.get_state(), MusicState::Playing | MusicState::Fading)
    }

    /// Check if music is paused
    pub fn is_paused(&self) -> bool {
        matches!(self.get_state(), MusicState::Paused)
    }

    /// Get currently playing track name
    pub fn get_current_track_name(&self) -> Option<String> {
        let shared = self.shared.lock();
        shared.current_music.as_ref().map(|music| music.track.name.clone())
    }

    /// Get current playlist length
    pub fn get_playlist_length(&self) -> usize {
        self.shared.lock().playlist.len()
    }

    /// Check if a specific track has completed playing
    pub fn has_track_completed(&self, track_name: &str, times: Int) -> bool {
        let shared = self.shared.lock();
        shared
            .track_completion_counts
            .get(track_name)
            .copied()
            .unwrap_or(0) >= times as u32
    }

    /// Get statistics
    pub fn get_statistics(&self) -> (u64, Duration) {
        let shared = self.shared.lock();
        (shared.tracks_played, shared.total_play_time)
    }

    /// Shutdown the music manager
    pub fn shutdown(&self) -> Result<(), String> {
        self.command_sender
            .send(MusicCommand::Shutdown)
            .map_err(|_| "Failed to send shutdown command")?;
        Ok(())
    }

    /// Start the background processing thread
    fn start_background_thread(&self) {
        let command_receiver = Arc::clone(&self.command_receiver);
        let shared = Arc::clone(&self.shared);
        let stream_handle = self.stream_handle.clone();

        thread::spawn(move || {
            let mut should_shutdown = false;

            while !should_shutdown {
                // Process commands
                if let Ok(command) = command_receiver.lock().try_recv() {
                    match command {
                        MusicCommand::Play { track } => {
                            Self::handle_play_command(&stream_handle, &shared, track, true);
                        }
                        MusicCommand::Stop { fade_out } => {
                            Self::handle_stop_command(&shared, fade_out);
                        }
                        MusicCommand::Pause => {
                            Self::handle_pause_command(&shared);
                        }
                        MusicCommand::Resume => {
                            Self::handle_resume_command(&shared);
                        }
                        MusicCommand::SetVolume { volume } => {
                            Self::handle_volume_command(&shared, volume);
                        }
                        MusicCommand::NextTrack => {
                            Self::handle_next_track_command(&stream_handle, &shared);
                        }
                        MusicCommand::PreviousTrack => {
                            Self::handle_previous_track_command(&stream_handle, &shared);
                        }
                        MusicCommand::SetPlaylist { tracks } => {
                            Self::handle_set_playlist_command(&shared, tracks);
                        }
                        MusicCommand::AddTrack { track } => {
                            Self::handle_add_track_command(&shared, track);
                        }
                        MusicCommand::RemoveTrack { name } => {
                            Self::handle_remove_track_command(&shared, &name);
                        }
                        MusicCommand::SetCrossfadeDuration { duration } => {
                            shared.lock().crossfade_duration = duration;
                        }
                        MusicCommand::SetCategory { category } => {
                            shared.lock().current_category = category;
                        }
                        MusicCommand::Shutdown => {
                            should_shutdown = true;
                            Self::handle_stop_command(&shared, false);
                        }
                    }
                }

                // Update fading music
                Self::update_fading_music(&shared);

                // Check for finished tracks
                Self::check_finished_tracks(&stream_handle, &shared);

                // Small sleep to prevent busy waiting
                thread::sleep(Duration::from_millis(50));
            }
        });
    }
}

// Background thread handlers
impl MusicManager {
    /// Handle a play command.
    ///
    /// `update_current` mirrors the original code's ability to pass a dummy
    /// `current_music` slot when advancing the playlist from
    /// `check_finished_tracks`: when false, the currently-playing slot is left
    /// untouched (neither stopped nor replaced), exactly as before.
    fn handle_play_command(
        stream_handle: &OutputStreamHandle,
        shared: &Arc<Mutex<SharedMusicState>>,
        track: MusicTrack,
        update_current: bool,
    ) {
        // Stop current music if any
        let master_volume = {
            let mut s = shared.lock();
            if update_current {
                if let Some(playing) = s.current_music.take() {
                    playing.sink.stop();
                }
            }
            s.state = MusicState::Loading;
            s.master_volume
        };

        // Try to load and play the track (outside the lock, as before)
        match Self::load_and_play_track(stream_handle, &track, master_volume) {
            Ok((sink, handle)) => {
                let playing = PlayingMusic {
                    track: track.clone(),
                    sink,
                    handle,
                    start_time: Instant::now(),
                    fade_start: if track.fade_in_duration > 0.0 {
                        Some(Instant::now())
                    } else {
                        None
                    },
                    fade_duration: track.fade_in_duration,
                    target_volume: track.volume,
                    current_volume: if track.fade_in_duration > 0.0 {
                        0.0
                    } else {
                        track.volume
                    },
                };

                {
                    let mut s = shared.lock();
                    if update_current {
                        s.current_music = Some(playing);
                    }
                    s.state = MusicState::Playing;

                    // Update statistics
                    s.tracks_played += 1;
                }

                // Send events
                Self::send_event(
                    shared,
                    MusicEvent::TrackStarted {
                        name: track.name.clone(),
                    },
                );
                Self::send_event(
                    shared,
                    MusicEvent::StateChanged {
                        state: MusicState::Playing,
                    },
                );
            }
            Err(error) => {
                shared.lock().state = MusicState::Error;
                Self::send_event(
                    shared,
                    MusicEvent::TrackFailed {
                        name: track.name.clone(),
                        error,
                    },
                );
            }
        }
    }

    fn handle_stop_command(shared: &Arc<Mutex<SharedMusicState>>, fade_out: bool) {
        let mut s = shared.lock();
        if let Some(mut playing) = s.current_music.take() {
            if fade_out && playing.track.fade_out_duration > 0.0 {
                // Start fade out
                playing.fade_start = Some(Instant::now());
                playing.fade_duration = playing.track.fade_out_duration;
                playing.target_volume = 0.0;
                s.current_music = Some(playing);
                s.state = MusicState::Fading;
            } else {
                // Stop immediately
                playing.sink.stop();
                s.state = MusicState::Stopped;
            }
        }
    }

    fn handle_pause_command(shared: &Arc<Mutex<SharedMusicState>>) {
        let mut s = shared.lock();
        if let Some(playing) = s.current_music.as_ref() {
            playing.sink.pause();
            s.state = MusicState::Paused;
        }
    }

    fn handle_resume_command(shared: &Arc<Mutex<SharedMusicState>>) {
        let mut s = shared.lock();
        if let Some(playing) = s.current_music.as_ref() {
            playing.sink.play();
            s.state = MusicState::Playing;
        }
    }

    fn handle_volume_command(shared: &Arc<Mutex<SharedMusicState>>, volume: Real) {
        {
            let mut s = shared.lock();
            s.master_volume = volume;

            if let Some(playing) = s.current_music.as_ref() {
                playing.sink.set_volume(volume * playing.track.volume);
            }
        }

        Self::send_event(shared, MusicEvent::VolumeChanged { volume });
    }

    fn handle_next_track_command(
        stream_handle: &OutputStreamHandle,
        shared: &Arc<Mutex<SharedMusicState>>,
    ) {
        let next = {
            let mut s = shared.lock();
            s.playlist.next_track().cloned()
        };

        if let Some(track) = next {
            Self::handle_play_command(stream_handle, shared, track, true);
        } else {
            Self::send_event(shared, MusicEvent::PlaylistFinished);
        }
    }

    fn handle_previous_track_command(
        stream_handle: &OutputStreamHandle,
        shared: &Arc<Mutex<SharedMusicState>>,
    ) {
        let previous = {
            let mut s = shared.lock();
            s.playlist.previous_track().cloned()
        };

        if let Some(track) = previous {
            Self::handle_play_command(stream_handle, shared, track, true);
        }
    }

    fn handle_set_playlist_command(shared: &Arc<Mutex<SharedMusicState>>, tracks: Vec<MusicTrack>) {
        let mut s = shared.lock();
        s.playlist = Playlist::new();
        for track in tracks {
            s.playlist.add_track(track);
        }
    }

    fn handle_add_track_command(shared: &Arc<Mutex<SharedMusicState>>, track: MusicTrack) {
        shared.lock().playlist.add_track(track);
    }

    fn handle_remove_track_command(shared: &Arc<Mutex<SharedMusicState>>, name: &str) {
        shared.lock().playlist.remove_track(name);
    }

    fn update_fading_music(shared: &Arc<Mutex<SharedMusicState>>) {
        let mut s = shared.lock();
        if let Some(playing) = s.current_music.as_mut() {
            if let Some(fade_start) = playing.fade_start {
                let elapsed = fade_start.elapsed().as_secs_f32();
                if elapsed >= playing.fade_duration {
                    // Fade complete
                    playing.fade_start = None;
                    playing.current_volume = playing.target_volume;

                    if playing.target_volume == 0.0 {
                        // Fade out complete - stop the music
                        playing.sink.stop();
                        s.current_music = None;
                        s.state = MusicState::Stopped;
                        return;
                    } else {
                        s.state = MusicState::Playing;
                    }
                } else {
                    // Update fade volume
                    let progress = elapsed / playing.fade_duration;
                    let start_volume = if playing.target_volume > playing.current_volume {
                        0.0
                    } else {
                        playing.track.volume
                    };
                    playing.current_volume =
                        start_volume + (playing.target_volume - start_volume) * progress;
                }

                // Apply volume to sink
                playing.sink.set_volume(playing.current_volume);
            }
        }
    }

    fn check_finished_tracks(
        stream_handle: &OutputStreamHandle,
        shared: &Arc<Mutex<SharedMusicState>>,
    ) {
        let finished = {
            let s = shared.lock();
            if let Some(playing) = s.current_music.as_ref() {
                if playing.sink.empty() {
                    // Track finished (the playing slot is deliberately left in
                    // place, matching the original drop-without-take behavior)
                    Some((playing.track.name.clone(), playing.start_time.elapsed()))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((track_name, play_duration)) = finished {
            // Update total play time and increment completion count for this track
            {
                let mut s = shared.lock();
                s.total_play_time += play_duration;
                *s.track_completion_counts.entry(track_name.clone()).or_insert(0) += 1;
            }

            // Send finished event
            Self::send_event(shared, MusicEvent::TrackFinished { name: track_name });

            // Try to play next track from playlist
            let next = {
                let mut s = shared.lock();
                s.playlist.next_track().cloned()
            };
            if let Some(next_track) = next {
                Self::handle_play_command(stream_handle, shared, next_track, false);
            } else {
                {
                    let mut s = shared.lock();
                    s.current_music = None;
                    s.state = MusicState::Stopped;
                }
                Self::send_event(shared, MusicEvent::PlaylistFinished);
            }
        }
    }

    fn load_and_play_track(
        stream_handle: &OutputStreamHandle,
        track: &MusicTrack,
        master_volume: Real,
    ) -> Result<(Sink, AudioHandle), String> {
        // Load audio file
        let file = std::fs::File::open(&track.file_path)
            .map_err(|e| format!("Failed to open music file: {}", e))?;

        // Create decoder
        let source = Decoder::new(std::io::BufReader::new(file))
            .map_err(|e| format!("Failed to decode music file: {}", e))?;

        // Create sink
        let sink = Sink::try_new(stream_handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;

        // Apply volume
        let effective_volume = master_volume * track.volume;
        sink.set_volume(effective_volume);

        // Handle looping
        if track.loops {
            let looped_source = source.repeat_infinite();
            sink.append(looped_source);
        } else {
            sink.append(source);
        }

        // Generate handle (in real implementation, this would be more sophisticated)
        let handle = rand::random::<AudioHandle>();

        Ok((sink, handle))
    }

    fn send_event(shared: &Arc<Mutex<SharedMusicState>>, event: MusicEvent) {
        // Clone the sender under the lock, then send outside it so callers
        // never hold the shared lock while sending (send order unchanged).
        if let Some(sender) = shared.lock().event_sender.clone() {
            let _ = sender.send(event);
        }
    }
}

// Trait implementation for compatibility with C++ AudioManager
pub trait MusicManagerTrait {
    fn add_audio_event(&mut self, event: AudioEventRts) -> AudioHandle;
    fn remove_audio_event(&mut self, handle: AudioHandle);
    fn next_music_track(&mut self);
    fn prev_music_track(&mut self);
    fn is_music_playing(&self) -> Bool;
    fn has_music_track_completed(&self, track_name: &str, number_of_times: Int) -> Bool;
    fn get_music_track_name(&self) -> String;
}

impl MusicManagerTrait for MusicManager {
    fn add_audio_event(&mut self, event: AudioEventRts) -> AudioHandle {
        if let Some(info) = event.get_audio_event_info() {
            if info.sound_type == AudioType::Music {
                if let Some(track_name) = info.sounds.first() {
                    match self.play_track(track_name) {
                        Ok(handle) => handle,
                        Err(_) => 0,
                    }
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        }
    }

    fn remove_audio_event(&mut self, _handle: AudioHandle) {
        let _ = self.stop_music(true);
    }

    fn next_music_track(&mut self) {
        let _ = self.next_track();
    }

    fn prev_music_track(&mut self) {
        let _ = self.previous_track();
    }

    fn is_music_playing(&self) -> Bool {
        self.is_playing()
    }

    fn has_music_track_completed(&self, track_name: &str, number_of_times: Int) -> Bool {
        self.has_track_completed(track_name, number_of_times)
    }

    fn get_music_track_name(&self) -> String {
        self.get_current_track_name().unwrap_or_default()
    }
}

/// Create a music manager instance
pub fn create_music_manager(
    stream_handle: OutputStreamHandle,
) -> Result<MusicManager, Box<dyn std::error::Error>> {
    let manager = MusicManager::new(stream_handle)?;
    manager.initialize()?;
    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_music_track_creation() {
        let track = MusicTrack::new("test_track".to_string(), "/path/to/track.mp3")
            .with_volume(0.8)
            .with_looping(true)
            .with_fade_durations(1.0, 2.0)
            .with_priority(5)
            .with_category(MusicCategory::Combat);

        assert_eq!(track.name, "test_track");
        assert_eq!(track.volume, 0.8);
        assert!(track.loops);
        assert_eq!(track.fade_in_duration, 1.0);
        assert_eq!(track.fade_out_duration, 2.0);
        assert_eq!(track.priority, 5);
        assert_eq!(track.category, MusicCategory::Combat);
    }

    #[test]
    fn test_playlist_management() {
        let mut playlist = Playlist::new();

        let track1 = MusicTrack::new("track1".to_string(), "/path1.mp3");
        let track2 = MusicTrack::new("track2".to_string(), "/path2.mp3");

        playlist.add_track(track1.clone());
        playlist.add_track(track2.clone());

        assert_eq!(playlist.len(), 2);
        assert_eq!(playlist.current_track().unwrap().name, "track1");

        playlist.next_track();
        assert_eq!(playlist.current_track().unwrap().name, "track2");

        playlist.previous_track();
        assert_eq!(playlist.current_track().unwrap().name, "track1");

        assert!(playlist.remove_track("track1"));
        assert_eq!(playlist.len(), 1);
        assert!(!playlist.remove_track("nonexistent"));
    }

    #[test]
    fn test_playlist_shuffle() {
        let mut playlist = Playlist::new();

        for i in 0..5 {
            let track = MusicTrack::new(format!("track{}", i), format!("/path{}.mp3", i));
            playlist.add_track(track);
        }

        playlist.set_shuffle(true);
        assert_eq!(playlist.len(), 5);

        // With shuffle, we should still be able to navigate
        let first_track = playlist.current_track().unwrap().name.clone();
        playlist.next_track();
        let second_track = playlist.current_track().unwrap().name.clone();

        // They should be different (with very high probability)
        // In a real test, we might want to test this more thoroughly
    }

    #[test]
    fn test_music_categories() {
        let categories = vec![
            MusicCategory::Menu,
            MusicCategory::Ambient,
            MusicCategory::Combat,
            MusicCategory::Victory,
            MusicCategory::Defeat,
            MusicCategory::Dramatic,
            MusicCategory::Peaceful,
            MusicCategory::Custom(42),
        ];

        for category in categories {
            let track = MusicTrack::new("test".to_string(), "/test.mp3").with_category(category);
            assert_eq!(track.category, category);
        }
    }

    // Note: Testing the actual MusicManager would require creating audio streams
    // and dealing with threading, which is complex for unit tests.
    // Integration tests would be more appropriate for testing the full functionality.
}
