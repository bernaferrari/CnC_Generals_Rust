//! # Audio Engine
//!
//! Core audio engine for the GameClient. Provides sound loading/caching, 3D
//! positional audio, volume control, and the main event loop that feeds the
//! kira backend.
//!
//! Ported from C++ `GameAudio.cpp` / `AudioManager`.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kira::manager::{AudioManager as KiraManager, AudioManagerSettings};
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle, StaticSoundSettings};
use kira::sound::streaming::StreamingSoundSettings;
use kira::sound::PlaybackRate;
use kira::tween::Tween;
use kira::Volume;

use crate::system::{SubsystemInterface, TimeOfDay};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Opaque handle returned when an audio event is added to the engine.
/// A handle of zero means "no sound" / "invalid".
pub type AudioHandle = u32;

/// Which category of audio a request targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioAffect {
    None = 0x00,
    Music = 0x01,
    Sound = 0x02,
    Sound3D = 0x04,
    Speech = 0x08,
    All = 0x0F,
    SystemSetting = 0x10,
}

impl std::ops::BitOr for AudioAffect {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        AudioAffect::from_bits(self as u32 | rhs as u32)
    }
}

impl AudioAffect {
    pub fn from_bits(bits: u32) -> Self {
        match bits & 0x0F {
            0x01 => Self::Music,
            0x02 => Self::Sound,
            0x04 => Self::Sound3D,
            0x08 => Self::Speech,
            0x0F => Self::All,
            _ => Self::None,
        }
    }

    pub fn has(&self, flag: AudioAffect) -> bool {
        (self.clone() as u32) & (flag as u32) != 0
    }
}

/// Priority of an audio event (matches C++ `AudioPriority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioPriority {
    Lowest = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl Default for AudioPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Type of audio event (matches C++ `AudioType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCategory {
    Music,
    Streaming,
    SoundEffect,
}

/// Category of sound within the game world (matches C++ `SoundType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundType {
    Ui = 0x0001,
    World = 0x0002,
    Shrouded = 0x0004,
    Global = 0x0008,
    Voice = 0x0010,
    Player = 0x0020,
    Allies = 0x0040,
    Enemies = 0x0080,
    Everyone = 0x0100,
}

/// Control flags for a sound event (matches C++ `AudioControl`).
#[derive(Debug, Clone, Copy)]
pub struct AudioControl(pub u16);

impl AudioControl {
    pub const LOOP: u16 = 0x0001;
    pub const RANDOM: u16 = 0x0002;
    pub const ALL: u16 = 0x0004;
    pub const POSTDELAY: u16 = 0x0008;
    pub const INTERRUPT: u16 = 0x0010;
}

/// 3-D world position.
#[derive(Debug, Clone, Copy, Default)]
pub struct AudioPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl AudioPosition {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Euclidean distance to another position.
    pub fn distance_to(&self, other: &AudioPosition) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl From<(f32, f32, f32)> for AudioPosition {
    fn from((x, y, z): (f32, f32, f32)) -> Self {
        Self { x, y, z }
    }
}

/// Information that the engine reads from INI AudioEvents (matches
/// C++ `AudioEventInfo`).
#[derive(Debug, Clone)]
pub struct AudioEventInfo {
    pub name: String,
    pub filename: String,
    pub volume: f32,
    pub volume_shift: f32,
    pub min_volume: f32,
    pub pitch_shift_min: f32,
    pub pitch_shift_max: f32,
    pub delay_min_ms: u32,
    pub delay_max_ms: u32,
    pub limit: u32,
    pub loop_count: i32,
    pub priority: AudioPriority,
    pub sound_type_flags: u32,
    pub control_flags: u16,
    pub sounds: Vec<String>,
    pub sounds_morning: Vec<String>,
    pub sounds_night: Vec<String>,
    pub sounds_evening: Vec<String>,
    pub attack_sounds: Vec<String>,
    pub decay_sounds: Vec<String>,
    pub low_pass_freq: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub category: AudioCategory,
}

impl Default for AudioEventInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            filename: String::new(),
            volume: 1.0,
            volume_shift: 0.0,
            min_volume: 0.01,
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            delay_min_ms: 0,
            delay_max_ms: 0,
            limit: 4,
            loop_count: 1,
            priority: AudioPriority::Normal,
            sound_type_flags: 0,
            control_flags: 0,
            sounds: Vec::new(),
            sounds_morning: Vec::new(),
            sounds_night: Vec::new(),
            sounds_evening: Vec::new(),
            attack_sounds: Vec::new(),
            decay_sounds: Vec::new(),
            low_pass_freq: 1.0,
            min_distance: 25.0,
            max_distance: 1000.0,
            category: AudioCategory::SoundEffect,
        }
    }
}

// ---------------------------------------------------------------------------
// Audio file cache
// ---------------------------------------------------------------------------

struct AudioCache {
    entries: HashMap<String, Arc<Vec<u8>>>,
    order: VecDeque<String>,
    current_size: usize,
    max_size: usize,
}

impl AudioCache {
    fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            current_size: 0,
            max_size,
        }
    }

    fn get_or_load(&mut self, path: &str) -> Option<Arc<Vec<u8>>> {
        if let Some(data) = self.entries.get(path) {
            // Move to back of LRU.
            if let Some(pos) = self.order.iter().position(|p| p == path) {
                self.order.remove(pos);
            }
            self.order.push_back(path.to_string());
            return Some(data.clone());
        }

        match std::fs::read(path) {
            Ok(data) => {
                let sz = data.len();
                let arc = Arc::new(data);
                self.ensure_space(sz);
                self.entries.insert(path.to_string(), arc.clone());
                self.current_size += sz;
                self.order.push_back(path.to_string());
                Some(arc)
            }
            Err(_) => None,
        }
    }

    fn ensure_space(&mut self, needed: usize) {
        while self.current_size + needed > self.max_size {
            if let Some(oldest) = self.order.pop_front() {
                if let Some(data) = self.entries.remove(&oldest) {
                    self.current_size -= data.len();
                }
            } else {
                break;
            }
        }
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.current_size = 0;
    }
}

// ---------------------------------------------------------------------------
// Playing instance tracking
// ---------------------------------------------------------------------------

/// Tracks a currently playing sound instance.
struct PlayingInstance {
    handle: AudioHandle,
    kira_handle: StaticSoundHandle,
    info: AudioEventInfo,
    volume: f32,
    fade: Option<AudioFade>,
    position: Option<AudioPosition>,
    start_time: Instant,
    is_looping: bool,
    category: AudioCategory,
    sound_type_flags: u32,
}

struct AudioFade {
    started_at: Instant,
    duration: Duration,
    from_volume: f32,
    to_volume: f32,
    stop_when_done: bool,
}

// ---------------------------------------------------------------------------
// AudioEngine
// ---------------------------------------------------------------------------

/// The main audio engine for the GameClient.
///
/// Wraps the `kira` audio backend and provides the CnC Generals audio API:
///   - 3D positional audio with distance-based attenuation
///   - Per-category volume (master, music, sfx, speech, ambient)
///   - Audio event registry (populated from INI)
///   - Sound caching and prioritisation
///
/// Matches C++ `AudioManager`.
pub struct AudioEngine {
    // kira backend
    kira: KiraManager,

    // Listener
    listener_position: AudioPosition,
    listener_forward: AudioPosition,
    listener_up: AudioPosition,

    // Volume (0.0..=1.0) -- system * user * script = effective
    master_volume: f32,
    music_volume: f32,
    sfx_volume: f32,
    speech_volume: f32,
    ambient_volume: f32,
    sound_3d_volume: f32,

    script_music_volume: f32,
    script_sfx_volume: f32,
    script_speech_volume: f32,
    script_sound_3d_volume: f32,

    system_music_volume: f32,
    system_sfx_volume: f32,
    system_speech_volume: f32,
    system_sound_3d_volume: f32,

    zoom_volume: f32,

    // Event registry
    event_registry: HashMap<String, AudioEventInfo>,

    // Playing instances
    instances: HashMap<AudioHandle, PlayingInstance>,
    handle_pool: u32,

    // File cache
    cache: AudioCache,

    // Search paths
    audio_root: PathBuf,
    sounds_folder: PathBuf,
    music_folder: PathBuf,
    speech_folder: PathBuf,

    // Feature toggles
    speech_on: bool,
    sound_on: bool,
    sound_3d_on: bool,
    music_on: bool,
}

impl AudioEngine {
    /// Create a new audio engine backed by `kira`.
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let kira = KiraManager::new(AudioManagerSettings::default())?;
        Ok(Self {
            kira,
            listener_position: AudioPosition::default(),
            listener_forward: AudioPosition::new(0.0, 1.0, 0.0),
            listener_up: AudioPosition::new(0.0, 0.0, 1.0),

            master_volume: 1.0,
            music_volume: 0.75,
            sfx_volume: 0.75,
            speech_volume: 0.55,
            ambient_volume: 0.5,
            sound_3d_volume: 0.75,

            script_music_volume: 1.0,
            script_sfx_volume: 1.0,
            script_speech_volume: 1.0,
            script_sound_3d_volume: 1.0,

            system_music_volume: 1.0,
            system_sfx_volume: 1.0,
            system_speech_volume: 1.0,
            system_sound_3d_volume: 1.0,

            zoom_volume: 1.0,

            event_registry: HashMap::new(),
            instances: HashMap::new(),
            handle_pool: 1000,

            cache: AudioCache::new(16 * 1024 * 1024),

            audio_root: PathBuf::from("Data/Audio"),
            sounds_folder: PathBuf::from("Sounds"),
            music_folder: PathBuf::from("Music"),
            speech_folder: PathBuf::from("Speech"),

            speech_on: true,
            sound_on: true,
            sound_3d_on: true,
            music_on: true,
        })
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Register a new audio event (from INI).
    pub fn register_event(&mut self, info: AudioEventInfo) {
        self.event_registry.insert(info.name.clone(), info);
    }

    /// Look up an event by name.
    pub fn find_event(&self, name: &str) -> Option<&AudioEventInfo> {
        self.event_registry.get(name)
    }

    /// Remove an audio event by name.
    pub fn remove_event(&mut self, name: &str) {
        self.event_registry.remove(name);
    }

    /// Play an audio event by name, optionally at a world position.
    /// Returns an `AudioHandle` that can be used to stop the sound later,
    /// or `0` if the event could not be played.
    pub fn play_event(&mut self, event_name: &str, position: Option<AudioPosition>) -> AudioHandle {
        let info = match self.find_event(event_name) {
            Some(i) => i.clone(),
            None => return 0,
        };

        self.play_event_internal(info, position, None, None, None)
    }

    /// Play an audio event by name attached to an object ID (position will
    /// be resolved each frame from the game-logic resolver).
    pub fn play_event_for_object(
        &mut self,
        event_name: &str,
        object_id: u32,
        player_index: Option<u32>,
    ) -> AudioHandle {
        let info = match self.find_event(event_name) {
            Some(i) => i.clone(),
            None => return 0,
        };

        self.play_event_internal(info, None, Some(object_id), player_index, None)
    }

    /// Stop a playing audio event.
    pub fn stop_event(&mut self, handle: AudioHandle) {
        if handle == 0 {
            return;
        }
        if let Some(mut instance) = self.instances.remove(&handle) {
            if let Err(err) = instance.kira_handle.stop(Tween::default()) {
                log::debug!("AudioEngine: stop failed for handle {}: {}", handle, err);
            }
        }
    }

    /// Immediately kill a playing event (no fade-out).
    pub fn kill_event(&mut self, handle: AudioHandle) {
        self.stop_event(handle);
    }

    /// Check if an audio event handle is currently playing.
    pub fn is_playing(&self, handle: AudioHandle) -> bool {
        self.instances.contains_key(&handle)
    }

    /// Set listener position and orientation (call once per frame from camera).
    pub fn set_listener_position(
        &mut self,
        position: AudioPosition,
        forward: AudioPosition,
        up: AudioPosition,
    ) {
        self.listener_position = position;
        self.listener_forward = forward;
        self.listener_up = up;
    }

    /// Set the listener position only (convenience overload matching C++ API).
    pub fn set_listener_pos(&mut self, pos: AudioPosition) {
        self.listener_position = pos;
    }

    /// Get the current listener position.
    pub fn listener_pos(&self) -> AudioPosition {
        self.listener_position
    }

    // -----------------------------------------------------------------------
    // Per-instance position & volume (matches C++ per-handle control)
    // -----------------------------------------------------------------------
    // PARITY_NOTE: C++ uses Miles Sound System / DirectSound for real audio
    // output. Rust uses kira as the backend; position/volume changes on
    // already-playing instances are tracked in the HashMap but actual
    // real-time panning requires deeper kira integration. The event tracker
    // stores the desired state so that when a real backend is connected the
    // values are already correct.

    /// Update the 3-D world position of a playing audio instance.
    ///
    /// Returns `false` if the handle is not currently playing.
    pub fn set_audio_position(&mut self, handle: AudioHandle, pos: AudioPosition) -> bool {
        if let Some(inst) = self.instances.get_mut(&handle) {
            inst.position = Some(pos);
            true
        } else {
            false
        }
    }

    /// Override the volume of a playing audio instance.
    ///
    /// `volume` is clamped to 0.0–1.0. Returns `false` if the handle is
    /// not currently playing.
    pub fn set_audio_volume(&mut self, handle: AudioHandle, volume: f32) -> bool {
        if let Some(inst) = self.instances.get_mut(&handle) {
            inst.volume = volume.clamp(0.0, 1.0);
            inst.fade = None;
            if let Err(err) = inst
                .kira_handle
                .set_volume(Volume::Amplitude(inst.volume as f64), Tween::default())
            {
                log::debug!(
                    "AudioEngine: volume update failed for handle {}: {}",
                    handle,
                    err
                );
            }
            true
        } else {
            false
        }
    }

    // -----------------------------------------------------------------------
    // Music convenience API (matches C++ TheAudio->playMusic / stopMusic)
    // -----------------------------------------------------------------------

    /// Play a music track by name with an optional fade-in duration.
    ///
    /// Registers a temporary `AudioEventInfo` in the `Music` category and
    /// plays it.  Returns the handle of the playing track, or `0` on failure.
    pub fn play_music_track(&mut self, track_name: &str, fade_in: f32) -> AudioHandle {
        let info = AudioEventInfo {
            name: track_name.to_string(),
            filename: track_name.to_string(),
            category: AudioCategory::Music,
            loop_count: 0, // music loops indefinitely
            volume: self.effective_music_volume(),
            ..AudioEventInfo::default()
        };
        self.play_event_internal(info, None, None, None, Some(fade_in.max(0.0)))
    }

    /// Stop the currently playing music track with an optional fade-out.
    ///
    /// Stops all active instances in the `Music` category.
    pub fn stop_music(&mut self, fade_out: f32) {
        let music_handles: Vec<AudioHandle> = self
            .instances
            .iter()
            .filter_map(|(h, inst)| {
                if inst.category == AudioCategory::Music {
                    Some(*h)
                } else {
                    None
                }
            })
            .collect();
        if fade_out > 0.0 {
            let duration = Duration::from_secs_f32(fade_out);
            for h in music_handles {
                if let Some(inst) = self.instances.get_mut(&h) {
                    inst.fade = Some(AudioFade {
                        started_at: Instant::now(),
                        duration,
                        from_volume: inst.volume,
                        to_volume: 0.0,
                        stop_when_done: true,
                    });
                }
            }
        } else {
            for h in music_handles {
                self.stop_event(h);
            }
        }
    }

    // -----------------------------------------------------------------------
    // EVA voice convenience API (matches C++ speech/EVA play paths)
    // -----------------------------------------------------------------------

    /// Play an EVA voice announcement by event name.
    ///
    /// Creates a temporary `Streaming` (speech) event and plays it at normal
    /// priority.  Returns the handle, or `0` if the event could not be played.
    pub fn play_eva_voice(&mut self, event: &str) -> AudioHandle {
        let info = AudioEventInfo {
            name: event.to_string(),
            filename: event.to_string(),
            category: AudioCategory::Streaming,
            volume: self.effective_speech_volume(),
            priority: AudioPriority::High,
            ..AudioEventInfo::default()
        };
        self.play_event_internal(info, None, None, None, None)
    }

    // -----------------------------------------------------------------------
    // Volume controls (matches C++ setVolume / getVolume)
    // -----------------------------------------------------------------------

    pub fn set_volume(&mut self, volume: f32, affect: AudioAffect) {
        let v = volume.clamp(0.0, 1.0);
        match affect {
            AudioAffect::All => {
                self.system_music_volume = v;
                self.system_sfx_volume = v;
                self.system_sound_3d_volume = v;
                self.system_speech_volume = v;
            }
            AudioAffect::Music => self.system_music_volume = v,
            AudioAffect::Sound => self.system_sfx_volume = v,
            AudioAffect::Sound3D => self.system_sound_3d_volume = v,
            AudioAffect::Speech => self.system_speech_volume = v,
            _ => {}
        }
    }

    pub fn get_volume(&self, affect: AudioAffect) -> f32 {
        match affect {
            AudioAffect::Music => self.system_music_volume,
            AudioAffect::Sound => self.system_sfx_volume,
            AudioAffect::Sound3D => self.system_sound_3d_volume,
            AudioAffect::Speech => self.system_speech_volume,
            _ => self.master_volume,
        }
    }

    /// Set volume that scales 3-D sounds with zoom level.
    pub fn set_3d_volume_adjustment(&mut self, vol: f32) {
        self.zoom_volume = vol.clamp(0.0, 1.0);
    }

    pub fn get_zoom_volume(&self) -> f32 {
        self.zoom_volume
    }

    pub fn effective_music_volume(&self) -> f32 {
        (self.system_music_volume * self.script_music_volume * self.master_volume).clamp(0.0, 1.0)
    }

    pub fn effective_sfx_volume(&self) -> f32 {
        (self.system_sfx_volume * self.script_sfx_volume * self.master_volume).clamp(0.0, 1.0)
    }

    pub fn effective_speech_volume(&self) -> f32 {
        (self.system_speech_volume * self.script_speech_volume * self.master_volume).clamp(0.0, 1.0)
    }

    pub fn effective_3d_volume(&self) -> f32 {
        (self.system_sound_3d_volume
            * self.script_sound_3d_volume
            * self.master_volume
            * self.zoom_volume)
            .clamp(0.0, 1.0)
    }

    pub fn set_master_volume(&mut self, vol: f32) {
        self.master_volume = vol.clamp(0.0, 1.0);
    }

    pub fn master_volume(&self) -> f32 {
        self.master_volume
    }

    // -----------------------------------------------------------------------
    // On / Off toggles
    // -----------------------------------------------------------------------

    pub fn set_on(&mut self, on: bool, affect: AudioAffect) {
        match affect {
            AudioAffect::All => {
                self.music_on = on;
                self.sound_on = on;
                self.sound_3d_on = on;
                self.speech_on = on;
            }
            AudioAffect::Music => self.music_on = on,
            AudioAffect::Sound => self.sound_on = on,
            AudioAffect::Sound3D => self.sound_3d_on = on,
            AudioAffect::Speech => self.speech_on = on,
            _ => {}
        }
    }

    pub fn is_on(&self, affect: AudioAffect) -> bool {
        match affect {
            AudioAffect::Music => self.music_on,
            AudioAffect::Sound => self.sound_on,
            AudioAffect::Sound3D => self.sound_3d_on,
            AudioAffect::Speech => self.speech_on,
            _ => true,
        }
    }

    // -----------------------------------------------------------------------
    // Audio paths
    // -----------------------------------------------------------------------

    pub fn set_audio_root(&mut self, root: impl Into<PathBuf>) {
        self.audio_root = root.into();
    }

    pub fn sounds_path(&self) -> PathBuf {
        self.audio_root.join(&self.sounds_folder)
    }

    pub fn music_path(&self) -> PathBuf {
        self.audio_root.join(&self.music_folder)
    }

    pub fn speech_path(&self) -> PathBuf {
        self.audio_root.join(&self.speech_folder)
    }

    /// Add a track name for the music system.
    pub fn add_track_name(&mut self, track: &str) {
        // The music system maintains its own list; this is kept for API compat.
        let _ = track;
    }

    /// Get the number of currently playing instances.
    pub fn playing_count(&self) -> usize {
        self.instances.len()
    }

    /// Get all event names in the registry (for debug / worldbuilder).
    pub fn all_event_names(&self) -> Vec<&str> {
        self.event_registry.keys().map(|s| s.as_str()).collect()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn allocate_handle(&mut self) -> AudioHandle {
        let h = self.handle_pool;
        self.handle_pool = self.handle_pool.wrapping_add(1);
        if self.handle_pool == 0 {
            self.handle_pool = 1000; // skip 0 (reserved for "no handle")
        }
        h
    }

    fn play_event_internal(
        &mut self,
        info: AudioEventInfo,
        position: Option<AudioPosition>,
        _object_id: Option<u32>,
        _player_index: Option<u32>,
        fade_in_seconds: Option<f32>,
    ) -> AudioHandle {
        // Check category toggles
        match info.category {
            AudioCategory::Music if !self.music_on => return 0,
            AudioCategory::SoundEffect if !self.sound_on => return 0,
            AudioCategory::Streaming if !self.speech_on => return 0,
            _ => {}
        }

        // Pick a random filename from the sounds list.
        let filename = if !info.sounds.is_empty() {
            let idx = if info.control_flags & AudioControl::RANDOM != 0 {
                (rand::random::<usize>()) % info.sounds.len()
            } else {
                0
            };
            info.sounds[idx].clone()
        } else {
            info.filename.clone()
        };

        if filename.is_empty() {
            return 0;
        }

        // Build full path.
        let full_path = self.resolve_audio_path(&filename, info.category);

        // Check limit.
        if info.limit > 0 {
            let count = self
                .instances
                .values()
                .filter(|i| i.info.name == info.name)
                .count() as u32;
            if count >= info.limit {
                return 0;
            }
        }

        // Distance check for 3D sounds.
        if let Some(ref pos) = position {
            let dist = pos.distance_to(&self.listener_position);
            if dist > info.max_distance {
                return 0; // too far
            }
        }

        // Calculate effective volume with distance attenuation.
        let effective_vol = self.compute_volume(&info, position.as_ref());

        if effective_vol < 0.001 {
            return 0; // effectively inaudible
        }

        let fade_duration = fade_in_seconds.unwrap_or(0.0).max(0.0);
        let initial_volume = if fade_duration > 0.0 {
            0.0
        } else {
            effective_vol
        };

        // Attempt playback via kira.
        let is_looping = info.loop_count == 0 || info.loop_count > 1;
        let kira_handle = match self.play_file_with_kira(&full_path, initial_volume, is_looping) {
            Ok(h) => h,
            Err(e) => {
                log::warn!("AudioEngine: failed to play {:?}: {}", full_path, e);
                return 0;
            }
        };

        let handle = self.allocate_handle();
        self.instances.insert(
            handle,
            PlayingInstance {
                handle,
                kira_handle,
                info: info.clone(),
                volume: initial_volume,
                fade: if fade_duration > 0.0 {
                    Some(AudioFade {
                        started_at: Instant::now(),
                        duration: Duration::from_secs_f32(fade_duration),
                        from_volume: initial_volume,
                        to_volume: effective_vol,
                        stop_when_done: false,
                    })
                } else {
                    None
                },
                position,
                start_time: Instant::now(),
                is_looping,
                category: info.category,
                sound_type_flags: info.sound_type_flags,
            },
        );

        handle
    }

    fn resolve_audio_path(&self, filename: &str, category: AudioCategory) -> PathBuf {
        let base = match category {
            AudioCategory::Music => self.music_path(),
            AudioCategory::Streaming => self.speech_path(),
            AudioCategory::SoundEffect => self.sounds_path(),
        };
        base.join(filename)
    }

    fn compute_volume(&self, info: &AudioEventInfo, pos: Option<&AudioPosition>) -> f32 {
        let base = info.volume + info.volume_shift;
        let vol = base.clamp(info.min_volume, 2.0);

        // Apply category volume
        let cat_vol = match info.category {
            AudioCategory::Music => self.effective_music_volume(),
            AudioCategory::Streaming => self.effective_speech_volume(),
            AudioCategory::SoundEffect => {
                if pos.is_some() {
                    self.effective_3d_volume()
                } else {
                    self.effective_sfx_volume()
                }
            }
        };

        let mut v = vol * cat_vol;

        // Distance attenuation for 3D positional sounds.
        if let Some(pos) = pos {
            let dist = pos.distance_to(&self.listener_position);
            if dist < info.min_distance {
                // within min range, full volume
            } else if dist < info.max_distance {
                let t = (dist - info.min_distance) / (info.max_distance - info.min_distance);
                v *= 1.0 - t;
            } else {
                v = 0.0;
            }
        }

        v.clamp(0.0, 2.0)
    }

    /// Attempt to load and play a file through kira.
    fn play_file_with_kira(
        &mut self,
        path: &Path,
        volume: f32,
        looping: bool,
    ) -> Result<StaticSoundHandle, Box<dyn std::error::Error + Send + Sync>> {
        // Try to load from cache first, then from disk.
        let _cached = self.cache.get_or_load(&path.to_string_lossy());

        // kira's StaticSoundData::from_file handles loading directly.
        // If the file is missing we fall back gracefully.
        let mut sound_data = StaticSoundData::from_file(
            path,
            StaticSoundSettings::new()
                .volume(Volume::Amplitude(volume as f64))
                .playback_rate(PlaybackRate::Factor(1.0)),
        )?;

        if looping {
            sound_data = sound_data.with_settings(
                StaticSoundSettings::new()
                    .volume(Volume::Amplitude(volume as f64))
                    .playback_rate(PlaybackRate::Factor(1.0))
                    .loop_region(..),
            );
        }

        let handle = self.kira.play(sound_data)?;
        Ok(handle)
    }
}

// ---------------------------------------------------------------------------
// SubsystemInterface impl
// ---------------------------------------------------------------------------

impl SubsystemInterface for AudioEngine {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("AudioEngine: initialised (kira backend)");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut remove_after_fade = Vec::new();
        for inst in self.instances.values_mut() {
            let Some(fade) = inst.fade.as_ref() else {
                continue;
            };
            let duration = fade.duration.as_secs_f32();
            let progress = if duration <= f32::EPSILON {
                1.0
            } else {
                (fade.started_at.elapsed().as_secs_f32() / duration).clamp(0.0, 1.0)
            };
            let new_volume = fade.from_volume + (fade.to_volume - fade.from_volume) * progress;
            inst.volume = new_volume;
            if let Err(err) = inst.kira_handle.set_volume(
                Volume::Amplitude(new_volume.max(0.0) as f64),
                Tween::default(),
            ) {
                log::debug!(
                    "AudioEngine: fade volume update failed for handle {}: {}",
                    inst.handle,
                    err
                );
            }

            if progress >= 1.0 {
                let stop_when_done = fade.stop_when_done;
                inst.fade = None;
                if stop_when_done {
                    remove_after_fade.push(inst.handle);
                }
            }
        }

        for handle in remove_after_fade {
            self.stop_event(handle);
        }

        // Garbage-collect finished instances.
        let finished: Vec<AudioHandle> = self
            .instances
            .iter()
            .filter_map(|(_, inst)| {
                // A very rough heuristic: if the instance has been playing
                // longer than 10 minutes and is not looping, consider it done.
                // In practice kira will report completion, but for now we
                // rely on the engine update to clean up.
                if !inst.is_looping && inst.start_time.elapsed() > Duration::from_secs(600) {
                    Some(inst.handle)
                } else {
                    None
                }
            })
            .collect();

        for h in finished {
            self.instances.remove(&h);
        }

        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Stop all playing instances.
        self.instances.clear();
        self.cache.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_affect_bits() {
        let all =
            AudioAffect::Music | AudioAffect::Sound | AudioAffect::Sound3D | AudioAffect::Speech;
        assert_eq!(all, AudioAffect::All);
        assert!(all.has(AudioAffect::Music));
        assert!(all.has(AudioAffect::Speech));
    }

    #[test]
    fn test_audio_position_distance() {
        let a = AudioPosition::new(0.0, 0.0, 0.0);
        let b = AudioPosition::new(3.0, 4.0, 0.0);
        assert!((a.distance_to(&b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_event_registry() {
        let mut engine = AudioEngine::new().unwrap();
        let info = AudioEventInfo {
            name: "TestSound".to_string(),
            filename: "test.wav".to_string(),
            ..Default::default()
        };
        engine.register_event(info);
        assert!(engine.find_event("TestSound").is_some());
        assert!(engine.find_event("NonExistent").is_none());
        engine.remove_event("TestSound");
        assert!(engine.find_event("TestSound").is_none());
    }

    #[test]
    fn test_volume_clamping() {
        let mut engine = AudioEngine::new().unwrap();
        engine.set_volume(1.5, AudioAffect::Music);
        assert!((engine.get_volume(AudioAffect::Music) - 1.0).abs() < 0.001);
        engine.set_volume(-0.5, AudioAffect::Music);
        assert!((engine.get_volume(AudioAffect::Music) - 0.0).abs() < 0.001);
    }
}
