//! Music Manager Implementation
//! 
//! This module provides a comprehensive music management system that handles
//! background music streaming, crossfading, playlist management, and integration
//! with the overall audio system. It's designed to match the C++ MusicManager API
//! while providing modern streaming capabilities.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::thread;
use std::sync::mpsc::{self, Sender, Receiver};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Source, Sink};
use symphonia::core::io::MediaSourceStream;

use crate::common::audio::{
    AudioEventRts, AudioEventInfo, AudioHandle, AudioType, AudioAffect, Coord3D,
    Real, Bool, Int, UnsignedInt, AsciiString,
};

/// Music playback state
#[derive(Debug, Clone, PartialEq)]
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
        let mut rng = rand::thread_rng();
        for i in (1..self.shuffle_order.len()).rev() {
            let j = rng.gen_range(0..=i);
            self.shuffle_order.swap(i, j);
        }
    }
}

/// Current playing music state
#[derive(Debug)]
struct PlayingMusic {
    track: MusicTrack,
    sink: Arc<Mutex<Sink>>,
    handle: AudioHandle,
    start_time: Instant,
    fade_start: Option<Instant>,
    fade_duration: Real,
    target_volume: Real,
    current_volume: Real,
}

/// Main Music Manager implementation
pub struct MusicManager {
    // Audio system
    stream_handle: OutputStreamHandle,
    
    // Current state
    state: Arc<RwLock<MusicState>>,
    current_music: Arc<Mutex<Option<PlayingMusic>>>,
    playlist: Arc<Mutex<Playlist>>,
    
    // Settings
    master_volume: Arc<RwLock<Real>>,
    crossfade_duration: Arc<RwLock<Real>>,
    current_category: Arc<RwLock<MusicCategory>>,
    
    // Communication
    command_sender: Sender<MusicCommand>,
    command_receiver: Arc<Mutex<Receiver<MusicCommand>>>,
    event_sender: Arc<Mutex<Option<Sender<MusicEvent>>>>,
    
    // Handle pool
    next_handle: Arc<Mutex<AudioHandle>>,
    
    // Track registry
    track_registry: Arc<RwLock<HashMap<String, MusicTrack>>>,
    
    // Search paths for music files
    search_paths: Arc<RwLock<Vec<PathBuf>>>,
    
    // Statistics
    tracks_played: Arc<RwLock<u64>>,
    total_play_time: Arc<RwLock<Duration>>,
}

impl MusicManager {
    /// Create a new music manager
    pub fn new(stream_handle: OutputStreamHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let (command_sender, command_receiver) = mpsc::channel();
        
        Ok(Self {
            stream_handle,
            state: Arc::new(RwLock::new(MusicState::Stopped)),
            current_music: Arc::new(Mutex::new(None)),
            playlist: Arc::new(Mutex::new(Playlist::new())),
            master_volume: Arc::new(RwLock::new(1.0)),
            crossfade_duration: Arc::new(RwLock::new(3.0)), // 3 second default crossfade
            current_category: Arc::new(RwLock::new(MusicCategory::Ambient)),
            command_sender,
            command_receiver: Arc::new(Mutex::new(command_receiver)),
            event_sender: Arc::new(Mutex::new(None)),
            next_handle: Arc::new(Mutex::new(10000)), // Start music handles at 10000
            track_registry: Arc::new(RwLock::new(HashMap::new())),
            search_paths: Arc::new(RwLock::new(vec![
                PathBuf::from("./data/audio/music/"),
                PathBuf::from("./assets/audio/music/"),
                PathBuf::from("./music/"),
            ])),
            tracks_played: Arc::new(RwLock::new(0)),
            total_play_time: Arc::new(RwLock::new(Duration::ZERO)),
        })
    }

    /// Initialize and start the music manager
    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.start_background_thread();
        Ok(())
    }

    /// Set event callback for music notifications
    pub fn set_event_callback(&self, sender: Sender<MusicEvent>) {
        let mut event_sender = self.event_sender.lock().unwrap();
        *event_sender = Some(sender);
    }

    /// Add search path for music files
    pub fn add_search_path<P: AsRef<Path>>(&self, path: P) {
        let mut search_paths = self.search_paths.write().unwrap();
        search_paths.push(path.as_ref().to_path_buf());
    }

    /// Register a music track
    pub fn register_track(&self, track: MusicTrack) {
        let mut registry = self.track_registry.write().unwrap();
        registry.insert(track.name.clone(), track);
    }

    /// Get registered track by name
    pub fn get_track(&self, name: &str) -> Option<MusicTrack> {
        let registry = self.track_registry.read().unwrap();
        registry.get(name).cloned()
    }

    /// Play a specific music track
    pub fn play_track(&self, track_name: &str) -> Result<AudioHandle, String> {
        let track = self.get_track(track_name)
            .ok_or_else(|| format!("Track '{}' not found", track_name))?;
        
        self.command_sender.send(MusicCommand::Play { track })
            .map_err(|_| "Failed to send play command")?;
        
        // Return a handle (in real implementation, this would be returned from the background thread)
        let mut next_handle = self.next_handle.lock().unwrap();
        let handle = *next_handle;
        *next_handle += 1;
        Ok(handle)
    }

    /// Stop current music
    pub fn stop_music(&self, fade_out: bool) -> Result<(), String> {
        self.command_sender.send(MusicCommand::Stop { fade_out })
            .map_err(|_| "Failed to send stop command")?;
        Ok(())
    }

    /// Pause current music
    pub fn pause_music(&self) -> Result<(), String> {
        self.command_sender.send(MusicCommand::Pause)
            .map_err(|_| "Failed to send pause command")?;
        Ok(())
    }

    /// Resume paused music
    pub fn resume_music(&self) -> Result<(), String> {
        self.command_sender.send(MusicCommand::Resume)
            .map_err(|_| "Failed to send resume command")?;
        Ok(())
    }

    /// Set music volume (0.0 to 1.0)
    pub fn set_volume(&self, volume: Real) -> Result<(), String> {
        let clamped_volume = volume.clamp(0.0, 1.0);
        self.command_sender.send(MusicCommand::SetVolume { volume: clamped_volume })
            .map_err(|_| "Failed to send volume command")?;
        Ok(())
    }

    /// Get current music volume
    pub fn get_volume(&self) -> Real {
        *self.master_volume.read().unwrap()
    }

    /// Go to next track in playlist
    pub fn next_track(&self) -> Result<(), String> {
        self.command_sender.send(MusicCommand::NextTrack)
            .map_err(|_| "Failed to send next track command")?;
        Ok(())
    }

    /// Go to previous track in playlist  
    pub fn previous_track(&self) -> Result<(), String> {
        self.command_sender.send(MusicCommand::PreviousTrack)
            .map_err(|_| "Failed to send previous track command")?;
        Ok(())
    }

    /// Set playlist of tracks to cycle through
    pub fn set_playlist(&self, tracks: Vec<MusicTrack>) -> Result<(), String> {
        self.command_sender.send(MusicCommand::SetPlaylist { tracks })
            .map_err(|_| "Failed to send playlist command")?;
        Ok(())
    }

    /// Add track to current playlist
    pub fn add_to_playlist(&self, track: MusicTrack) -> Result<(), String> {
        self.command_sender.send(MusicCommand::AddTrack { track })
            .map_err(|_| "Failed to send add track command")?;
        Ok(())
    }

    /// Remove track from playlist
    pub fn remove_from_playlist(&self, name: String) -> Result<(), String> {
        self.command_sender.send(MusicCommand::RemoveTrack { name })
            .map_err(|_| "Failed to send remove track command")?;
        Ok(())
    }

    /// Set crossfade duration between tracks
    pub fn set_crossfade_duration(&self, duration: Real) -> Result<(), String> {
        self.command_sender.send(MusicCommand::SetCrossfadeDuration { duration })
            .map_err(|_| "Failed to send crossfade duration command")?;
        Ok(())
    }

    /// Set current music category
    pub fn set_category(&self, category: MusicCategory) -> Result<(), String> {
        self.command_sender.send(MusicCommand::SetCategory { category })
            .map_err(|_| "Failed to send category command")?;
        Ok(())
    }

    /// Get current music state
    pub fn get_state(&self) -> MusicState {
        *self.state.read().unwrap()
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
        let current = self.current_music.lock().unwrap();
        current.as_ref().map(|music| music.track.name.clone())
    }

    /// Get current playlist length
    pub fn get_playlist_length(&self) -> usize {
        let playlist = self.playlist.lock().unwrap();
        playlist.len()
    }

    /// Check if a specific track has completed playing
    pub fn has_track_completed(&self, track_name: &str, times: Int) -> bool {
        // This would need to be implemented with proper track completion tracking
        // For now, just return false
        false
    }

    /// Get statistics
    pub fn get_statistics(&self) -> (u64, Duration) {
        let tracks_played = *self.tracks_played.read().unwrap();
        let total_play_time = *self.total_play_time.read().unwrap();
        (tracks_played, total_play_time)
    }

    /// Shutdown the music manager
    pub fn shutdown(&self) -> Result<(), String> {
        self.command_sender.send(MusicCommand::Shutdown)
            .map_err(|_| "Failed to send shutdown command")?;
        Ok(())
    }

    /// Start the background processing thread
    fn start_background_thread(&self) {
        let command_receiver = Arc::clone(&self.command_receiver);
        let state = Arc::clone(&self.state);
        let current_music = Arc::clone(&self.current_music);
        let playlist = Arc::clone(&self.playlist);
        let master_volume = Arc::clone(&self.master_volume);
        let crossfade_duration = Arc::clone(&self.crossfade_duration);
        let current_category = Arc::clone(&self.current_category);
        let event_sender = Arc::clone(&self.event_sender);
        let track_registry = Arc::clone(&self.track_registry);
        let search_paths = Arc::clone(&self.search_paths);
        let tracks_played = Arc::clone(&self.tracks_played);
        let total_play_time = Arc::clone(&self.total_play_time);
        let stream_handle = self.stream_handle.clone();

        thread::spawn(move || {
            let mut should_shutdown = false;
            
            while !should_shutdown {
                // Process commands
                if let Ok(command) = command_receiver.lock().unwrap().try_recv() {
                    match command {
                        MusicCommand::Play { track } => {
                            Self::handle_play_command(
                                &stream_handle,
                                &state,
                                &current_music,
                                &master_volume,
                                &event_sender,
                                &tracks_played,
                                track,
                            );
                        }
                        MusicCommand::Stop { fade_out } => {
                            Self::handle_stop_command(&current_music, &state, fade_out);
                        }
                        MusicCommand::Pause => {
                            Self::handle_pause_command(&current_music, &state);
                        }
                        MusicCommand::Resume => {
                            Self::handle_resume_command(&current_music, &state);
                        }
                        MusicCommand::SetVolume { volume } => {
                            Self::handle_volume_command(&master_volume, &current_music, &event_sender, volume);
                        }
                        MusicCommand::NextTrack => {
                            Self::handle_next_track_command(
                                &stream_handle,
                                &playlist,
                                &state,
                                &current_music,
                                &master_volume,
                                &event_sender,
                                &tracks_played,
                            );
                        }
                        MusicCommand::PreviousTrack => {
                            Self::handle_previous_track_command(
                                &stream_handle,
                                &playlist,
                                &state,
                                &current_music,
                                &master_volume,
                                &event_sender,
                                &tracks_played,
                            );
                        }
                        MusicCommand::SetPlaylist { tracks } => {
                            Self::handle_set_playlist_command(&playlist, tracks);
                        }
                        MusicCommand::AddTrack { track } => {
                            Self::handle_add_track_command(&playlist, track);
                        }
                        MusicCommand::RemoveTrack { name } => {
                            Self::handle_remove_track_command(&playlist, &name);
                        }
                        MusicCommand::SetCrossfadeDuration { duration } => {
                            *crossfade_duration.write().unwrap() = duration;
                        }
                        MusicCommand::SetCategory { category } => {
                            *current_category.write().unwrap() = category;
                        }
                        MusicCommand::Shutdown => {
                            should_shutdown = true;
                            Self::handle_stop_command(&current_music, &state, false);
                        }
                    }
                }

                // Update fading music
                Self::update_fading_music(&current_music, &state);

                // Check for finished tracks
                Self::check_finished_tracks(
                    &stream_handle,
                    &playlist,
                    &current_music,
                    &state,
                    &master_volume,
                    &event_sender,
                    &tracks_played,
                    &total_play_time,
                );

                // Small sleep to prevent busy waiting
                thread::sleep(Duration::from_millis(50));
            }
        });
    }
}

// Background thread handlers
impl MusicManager {
    fn handle_play_command(
        stream_handle: &OutputStreamHandle,
        state: &Arc<RwLock<MusicState>>,
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        master_volume: &Arc<RwLock<Real>>,
        event_sender: &Arc<Mutex<Option<Sender<MusicEvent>>>>,
        tracks_played: &Arc<RwLock<u64>>,
        track: MusicTrack,
    ) {
        // Stop current music if any
        {
            let mut current = current_music.lock().unwrap();
            if let Some(playing) = current.take() {
                let sink = playing.sink.lock().unwrap();
                sink.stop();
            }
        }

        *state.write().unwrap() = MusicState::Loading;

        // Try to load and play the track
        match Self::load_and_play_track(stream_handle, &track, *master_volume.read().unwrap()) {
            Ok((sink, handle)) => {
                let playing = PlayingMusic {
                    track: track.clone(),
                    sink: Arc::new(Mutex::new(sink)),
                    handle,
                    start_time: Instant::now(),
                    fade_start: if track.fade_in_duration > 0.0 { Some(Instant::now()) } else { None },
                    fade_duration: track.fade_in_duration,
                    target_volume: track.volume,
                    current_volume: if track.fade_in_duration > 0.0 { 0.0 } else { track.volume },
                };

                *current_music.lock().unwrap() = Some(playing);
                *state.write().unwrap() = MusicState::Playing;
                
                // Update statistics
                *tracks_played.write().unwrap() += 1;

                // Send event
                Self::send_event(event_sender, MusicEvent::TrackStarted { name: track.name.clone() });
                Self::send_event(event_sender, MusicEvent::StateChanged { state: MusicState::Playing });
            }
            Err(error) => {
                *state.write().unwrap() = MusicState::Error;
                Self::send_event(event_sender, MusicEvent::TrackFailed { 
                    name: track.name.clone(), 
                    error 
                });
            }
        }
    }

    fn handle_stop_command(
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        state: &Arc<RwLock<MusicState>>,
        fade_out: bool,
    ) {
        let mut current = current_music.lock().unwrap();
        if let Some(mut playing) = current.take() {
            if fade_out && playing.track.fade_out_duration > 0.0 {
                // Start fade out
                playing.fade_start = Some(Instant::now());
                playing.fade_duration = playing.track.fade_out_duration;
                playing.target_volume = 0.0;
                *current = Some(playing);
                *state.write().unwrap() = MusicState::Fading;
            } else {
                // Stop immediately
                let sink = playing.sink.lock().unwrap();
                sink.stop();
                *state.write().unwrap() = MusicState::Stopped;
            }
        }
    }

    fn handle_pause_command(
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        state: &Arc<RwLock<MusicState>>,
    ) {
        let current = current_music.lock().unwrap();
        if let Some(playing) = current.as_ref() {
            let sink = playing.sink.lock().unwrap();
            sink.pause();
            *state.write().unwrap() = MusicState::Paused;
        }
    }

    fn handle_resume_command(
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        state: &Arc<RwLock<MusicState>>,
    ) {
        let current = current_music.lock().unwrap();
        if let Some(playing) = current.as_ref() {
            let sink = playing.sink.lock().unwrap();
            sink.play();
            *state.write().unwrap() = MusicState::Playing;
        }
    }

    fn handle_volume_command(
        master_volume: &Arc<RwLock<Real>>,
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        event_sender: &Arc<Mutex<Option<Sender<MusicEvent>>>>,
        volume: Real,
    ) {
        *master_volume.write().unwrap() = volume;
        
        let current = current_music.lock().unwrap();
        if let Some(playing) = current.as_ref() {
            let sink = playing.sink.lock().unwrap();
            sink.set_volume(volume * playing.track.volume);
        }

        Self::send_event(event_sender, MusicEvent::VolumeChanged { volume });
    }

    fn handle_next_track_command(
        stream_handle: &OutputStreamHandle,
        playlist: &Arc<Mutex<Playlist>>,
        state: &Arc<RwLock<MusicState>>,
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        master_volume: &Arc<RwLock<Real>>,
        event_sender: &Arc<Mutex<Option<Sender<MusicEvent>>>>,
        tracks_played: &Arc<RwLock<u64>>,
    ) {
        let mut playlist_guard = playlist.lock().unwrap();
        if let Some(track) = playlist_guard.next_track() {
            let track = track.clone();
            drop(playlist_guard);
            
            Self::handle_play_command(
                stream_handle,
                state,
                current_music,
                master_volume,
                event_sender,
                tracks_played,
                track,
            );
        } else {
            Self::send_event(event_sender, MusicEvent::PlaylistFinished);
        }
    }

    fn handle_previous_track_command(
        stream_handle: &OutputStreamHandle,
        playlist: &Arc<Mutex<Playlist>>,
        state: &Arc<RwLock<MusicState>>,
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        master_volume: &Arc<RwLock<Real>>,
        event_sender: &Arc<Mutex<Option<Sender<MusicEvent>>>>,
        tracks_played: &Arc<RwLock<u64>>,
    ) {
        let mut playlist_guard = playlist.lock().unwrap();
        if let Some(track) = playlist_guard.previous_track() {
            let track = track.clone();
            drop(playlist_guard);
            
            Self::handle_play_command(
                stream_handle,
                state,
                current_music,
                master_volume,
                event_sender,
                tracks_played,
                track,
            );
        }
    }

    fn handle_set_playlist_command(playlist: &Arc<Mutex<Playlist>>, tracks: Vec<MusicTrack>) {
        let mut playlist_guard = playlist.lock().unwrap();
        *playlist_guard = Playlist::new();
        for track in tracks {
            playlist_guard.add_track(track);
        }
    }

    fn handle_add_track_command(playlist: &Arc<Mutex<Playlist>>, track: MusicTrack) {
        let mut playlist_guard = playlist.lock().unwrap();
        playlist_guard.add_track(track);
    }

    fn handle_remove_track_command(playlist: &Arc<Mutex<Playlist>>, name: &str) {
        let mut playlist_guard = playlist.lock().unwrap();
        playlist_guard.remove_track(name);
    }

    fn update_fading_music(
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        state: &Arc<RwLock<MusicState>>,
    ) {
        let mut current = current_music.lock().unwrap();
        if let Some(playing) = current.as_mut() {
            if let Some(fade_start) = playing.fade_start {
                let elapsed = fade_start.elapsed().as_secs_f32();
                if elapsed >= playing.fade_duration {
                    // Fade complete
                    playing.fade_start = None;
                    playing.current_volume = playing.target_volume;
                    
                    if playing.target_volume == 0.0 {
                        // Fade out complete - stop the music
                        let sink = playing.sink.lock().unwrap();
                        sink.stop();
                        *current = None;
                        *state.write().unwrap() = MusicState::Stopped;
                        return;
                    } else {
                        *state.write().unwrap() = MusicState::Playing;
                    }
                } else {
                    // Update fade volume
                    let progress = elapsed / playing.fade_duration;
                    let start_volume = if playing.target_volume > playing.current_volume { 0.0 } else { playing.track.volume };
                    playing.current_volume = start_volume + (playing.target_volume - start_volume) * progress;
                }
                
                // Apply volume to sink
                let sink = playing.sink.lock().unwrap();
                sink.set_volume(playing.current_volume);
            }
        }
    }

    fn check_finished_tracks(
        stream_handle: &OutputStreamHandle,
        playlist: &Arc<Mutex<Playlist>>,
        current_music: &Arc<Mutex<Option<PlayingMusic>>>,
        state: &Arc<RwLock<MusicState>>,
        master_volume: &Arc<RwLock<Real>>,
        event_sender: &Arc<Mutex<Option<Sender<MusicEvent>>>>,
        tracks_played: &Arc<RwLock<u64>>,
        total_play_time: &Arc<RwLock<Duration>>,
    ) {
        let mut current = current_music.lock().unwrap();
        if let Some(playing) = current.as_ref() {
            let sink = playing.sink.lock().unwrap();
            
            if sink.empty() {
                // Track finished
                let track_name = playing.track.name.clone();
                let play_duration = playing.start_time.elapsed();
                
                drop(sink);
                drop(current);
                
                // Update total play time
                *total_play_time.write().unwrap() += play_duration;
                
                // Send finished event
                Self::send_event(event_sender, MusicEvent::TrackFinished { name: track_name });
                
                // Try to play next track from playlist
                let mut playlist_guard = playlist.lock().unwrap();
                if let Some(next_track) = playlist_guard.next_track() {
                    let next_track = next_track.clone();
                    drop(playlist_guard);
                    
                    Self::handle_play_command(
                        stream_handle,
                        state,
                        &Arc::new(Mutex::new(None)),
                        master_volume,
                        event_sender,
                        tracks_played,
                        next_track,
                    );
                } else {
                    *current_music.lock().unwrap() = None;
                    *state.write().unwrap() = MusicState::Stopped;
                    Self::send_event(event_sender, MusicEvent::PlaylistFinished);
                }
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

    fn send_event(event_sender: &Arc<Mutex<Option<Sender<MusicEvent>>>>, event: MusicEvent) {
        if let Some(sender) = event_sender.lock().unwrap().as_ref() {
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
pub fn create_music_manager(stream_handle: OutputStreamHandle) -> Result<MusicManager, Box<dyn std::error::Error>> {
    let manager = MusicManager::new(stream_handle)?;
    manager.initialize()?;
    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

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