////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! AudioEventRTS structure
//! Author: John K. McDonald, March 2002
//! Converted to Rust

use crate::common::random_value::{
    get_game_audio_random_value, get_game_audio_random_value_real, get_game_logic_random_value,
};
use crate::common::system::file_system::get_file_system;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, OnceLock, RwLock,
    atomic::{AtomicI32, AtomicU32, Ordering},
};
use tokio::sync::mpsc;

// Common imports - these would come from other modules
pub type AsciiString = String;
pub type AudioHandle = u32;
pub type ObjectId = u32;
pub type DrawableId = u32;
pub type Real = f32;
pub type Bool = bool;
pub type Int = i32;

/// Audio event information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEventInfo {
    pub sound_type: AudioType,
    pub control: u32,
    pub audio_name: AsciiString,
    pub volume: Real,
    pub sounds_morning: Vec<AsciiString>,
    pub sounds: Vec<AsciiString>,
    pub sounds_night: Vec<AsciiString>,
    pub sounds_evening: Vec<AsciiString>,
    pub attack_sounds: Vec<AsciiString>,
    pub decay_sounds: Vec<AsciiString>,
    pub pitch_shift_min: Real,
    pub pitch_shift_max: Real,
    pub volume_shift: Real,
    #[serde(default)]
    pub min_volume: Real,
    pub limit: Int,
    pub loop_count: Int,
    pub delay_min: Real,
    pub delay_max: Real,
    pub filename: AsciiString,
    pub sound_type_field: AudioType,
    pub type_field: u32,
    pub priority: AudioPriority,
    pub min_distance: Real,
    pub max_distance: Real,
    /// C++ `AudioEventInfo::m_lowPassFreq` (LowPassCutoff). Off-screen 3D occlusion.
    #[serde(default = "default_low_pass_freq")]
    pub low_pass_freq: Real,
    /// C++ `AudioEventInfo::m_isLevelSpecific` — map-authored overrides purged on reset.
    #[serde(default)]
    pub is_level_specific: bool,
}

impl AudioEventInfo {
    /// C++ `AudioEventInfo::isLevelSpecific`.
    #[must_use]
    pub fn is_level_specific(&self) -> bool {
        self.is_level_specific
    }

    /// C++ `AudioEventInfo::isPermanentSound`: looping with `loopCount == 0`.
    #[must_use]
    pub fn is_permanent_sound(&self) -> bool {
        (self.control & AC_LOOP) != 0 && self.loop_count == 0
    }
}

fn default_low_pass_freq() -> Real {
    1.0
}

impl Default for AudioEventInfo {
    fn default() -> Self {
        Self {
            sound_type: AudioType::SoundEffect,
            control: 0,
            audio_name: String::new(),
            volume: 0.5,
            sounds_morning: Vec::new(),
            sounds: Vec::new(),
            sounds_night: Vec::new(),
            sounds_evening: Vec::new(),
            attack_sounds: Vec::new(),
            decay_sounds: Vec::new(),
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            volume_shift: 0.0,
            min_volume: 0.0,
            limit: -1,
            loop_count: 1,
            delay_min: 0.0,
            delay_max: 0.0,
            filename: String::new(),
            sound_type_field: AudioType::SoundEffect,
            type_field: 0,
            priority: AudioPriority::Normal,
            min_distance: 0.0,
            max_distance: 100.0,
            low_pass_freq: 1.0,
            is_level_specific: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Coord3D {
    pub fn new() -> Self {
        Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn zero(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
    }

    pub fn set(&mut self, other: &Coord3D) {
        self.x = other.x;
        self.y = other.y;
        self.z = other.z;
    }

    pub fn sub(&mut self, other: &Coord3D) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
    }

    pub fn add(&mut self, other: &Coord3D) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }

    pub fn scale(&mut self, factor: Real) {
        self.x *= factor;
        self.y *= factor;
        self.z *= factor;
    }

    pub fn length(&self) -> Real {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

// Enums
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OwnerType {
    Positional,
    Drawable,
    Object,
    Dead,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PortionToPlay {
    Attack,
    Sound,
    Decay,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AudioPriority {
    Lowest,
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AudioType {
    Music,
    SoundEffect,
    Streaming,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeOfDay {
    Morning,
    Afternoon,
    Evening,
    Night,
    Day, // Alias for general day time
}

// Constants
const INVALID_ID: u32 = 0xFFFFFFFF;
const INVALID_DRAWABLE_ID: u32 = 0xFFFFFFFF;
const TIME_OF_DAY_AFTERNOON: TimeOfDay = TimeOfDay::Afternoon;
const AP_NORMAL: AudioPriority = AudioPriority::Normal;
/// C++ `AudioControl::AC_LOOP` (`AudioEventInfo.h`).
pub const AC_LOOP: u32 = 0x00000001;
const AC_RANDOM: u32 = 0x00000002;
/// C++ `AudioControl::AC_ALL`.
pub const AC_ALL: u32 = 0x00000004;
/// C++ `AudioControl::AC_INTERRUPT` (`AudioEventInfo.h`).
pub const AC_INTERRUPT: u32 = 0x00000010;
/// C++ `ST_WORLD` — positional/world audio bit.
pub const ST_WORLD: u32 = 0x00000002;
/// C++ `ST_UI`.
pub const ST_UI: u32 = 0x00000001;
/// C++ `ST_SHROUDED` — positional events cull when the cell is not CLEAR.
pub const ST_SHROUDED: u32 = 0x00000004;
/// C++ `ST_PLAYER`.
pub const ST_PLAYER: u32 = 0x00000020;
/// C++ `ST_ALLIES`.
pub const ST_ALLIES: u32 = 0x00000040;
/// C++ `ST_ENEMIES`.
pub const ST_ENEMIES: u32 = 0x00000080;
/// C++ `ST_EVERYONE`.
pub const ST_EVERYONE: u32 = 0x00000100;
/// C++ `ST_GLOBAL` — uses audio-settings global min/max ranges.
pub const ST_GLOBAL: u32 = 0x00000008;
const ST_VOICE: u32 = 0x00000010;

/// C++ `MilesAudioManager::getEffectiveVolume` positional gain.
///
/// `volume *= 1 / (objDistance / objMinDistance)` when past min range,
/// and `volume = 0` when `objDistance >= objMaxDistance`.
#[must_use]
pub fn miles_positional_gain(distance: f32, min_distance: f32, max_distance: f32) -> f32 {
    if distance >= max_distance {
        return 0.0;
    }
    let min_distance = min_distance.max(1.0e-6);
    if distance > min_distance {
        1.0 / (distance / min_distance)
    } else {
        1.0
    }
}

/// Resolve C++ `objMinDistance` / `objMaxDistance` for a playing event.
#[must_use]
pub fn miles_positional_ranges(
    info: Option<&AudioEventInfo>,
    global_min_range: f32,
    global_max_range: f32,
) -> (f32, f32) {
    match info {
        Some(info) if (info.type_field & ST_GLOBAL) != 0 => (global_min_range, global_max_range),
        Some(info) => (info.min_distance, info.max_distance),
        None => (global_min_range, global_max_range),
    }
}

/// C++ `MilesAudioManager` category sliders + global 3D ranges.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MilesVolumeSliders {
    pub music_volume: f32,
    pub speech_volume: f32,
    pub sound_volume: f32,
    pub sound_3d_volume: f32,
    pub global_min_range: f32,
    pub global_max_range: f32,
}

impl Default for MilesVolumeSliders {
    fn default() -> Self {
        Self {
            music_volume: 0.55,
            speech_volume: 0.55,
            sound_volume: 0.75,
            sound_3d_volume: 0.75,
            global_min_range: 25.0,
            global_max_range: 1000.0,
        }
    }
}

/// Resolve the Miles play position, following object/drawable owners.
#[must_use]
pub fn miles_event_world_position(event: &AudioEventRts) -> Coord3D {
    match event.owner_type {
        OwnerType::Object => with_audio_event_owner_resolver(|resolver| {
            resolver.resolve_object_position(event.object_id)
        })
        .flatten()
        .unwrap_or(event.position_of_audio),
        OwnerType::Drawable => with_audio_event_owner_resolver(|resolver| {
            resolver.resolve_drawable_position(event.drawable_id)
        })
        .flatten()
        .unwrap_or(event.position_of_audio),
        OwnerType::Positional | OwnerType::Dead => event.position_of_audio,
        OwnerType::Invalid => event.position_of_audio,
    }
}

/// C++ `MilesAudioManager::getEffectiveVolume`.
///
/// `volume = getVolume() * getVolumeShift()`, then:
/// - `AT_Music` → `* m_musicVolume`
/// - `AT_Streaming` → `* m_speechVolume`
/// - else positional → `* m_sound3DVolume` then inverse-min falloff / 0 at max
/// - else → `* m_soundVolume`
#[must_use]
pub fn miles_get_effective_volume(
    event: &AudioEventRts,
    listener: &Coord3D,
    sliders: &MilesVolumeSliders,
) -> f32 {
    let mut volume = event.get_volume() * event.get_volume_shift();
    match event
        .get_audio_event_info()
        .as_deref()
        .map(|info| info.sound_type)
    {
        Some(AudioType::Music) => volume *= sliders.music_volume,
        Some(AudioType::Streaming) => volume *= sliders.speech_volume,
        _ => {
            if event.is_positional_audio() {
                volume *= sliders.sound_3d_volume;
                let pos = miles_event_world_position(event);
                let dx = listener.x - pos.x;
                let dy = listener.y - pos.y;
                let dz = listener.z - pos.z;
                let distance = (dx * dx + dy * dy + dz * dz).sqrt();
                let (min_distance, max_distance) = miles_positional_ranges(
                    event.get_audio_event_info().as_deref(),
                    sliders.global_min_range,
                    sliders.global_max_range,
                );
                volume *= miles_positional_gain(distance, min_distance, max_distance);
            } else {
                volume *= sliders.sound_volume;
            }
        }
    }
    volume
}

#[derive(Debug, Clone)]
struct AudioPathSettings {
    audio_root: String,
    sounds_folder: String,
    music_folder: String,
    streaming_folder: String,
    sounds_extension: String,
}

impl Default for AudioPathSettings {
    fn default() -> Self {
        Self {
            audio_root: "Data\\Audio".to_string(),
            sounds_folder: "Sounds".to_string(),
            music_folder: "Music".to_string(),
            streaming_folder: "Speech".to_string(),
            sounds_extension: "wav".to_string(),
        }
    }
}

pub trait AudioEventOwnerResolver: Send + Sync {
    fn resolve_object_position(&self, object_id: ObjectId) -> Option<Coord3D>;
    fn resolve_drawable_position(&self, drawable_id: DrawableId) -> Option<Coord3D>;
    fn resolve_object_player_index(&self, object_id: ObjectId) -> Option<Int>;
    fn resolve_drawable_player_index(&self, drawable_id: DrawableId) -> Option<Int>;
}

static AUDIO_EVENT_OWNER_RESOLVER: OnceLock<Arc<dyn AudioEventOwnerResolver>> = OnceLock::new();
static AUDIO_EVENT_LOCALIZATION_LANGUAGE: OnceLock<RwLock<String>> = OnceLock::new();
static ASYNC_PLAYBACK_HANDLE_POOL: AtomicU32 = AtomicU32::new(10_000_000);

pub fn register_audio_event_owner_resolver(resolver: Arc<dyn AudioEventOwnerResolver>) -> bool {
    AUDIO_EVENT_OWNER_RESOLVER.set(resolver).is_ok()
}

pub fn set_audio_event_localization_language(language: &str) {
    let lock =
        AUDIO_EVENT_LOCALIZATION_LANGUAGE.get_or_init(|| RwLock::new(String::from("English")));
    if let Ok(mut guard) = lock.write() {
        guard.clear();
        guard.push_str(language.trim());
    }
}

fn with_audio_event_owner_resolver<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&dyn AudioEventOwnerResolver) -> R,
{
    AUDIO_EVENT_OWNER_RESOLVER
        .get()
        .map(|resolver| f(resolver.as_ref()))
}

fn current_localization_language() -> String {
    if let Ok(from_env) = std::env::var("GENERALS_REGISTRY_LANGUAGE") {
        let trimmed = from_env.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let lock =
        AUDIO_EVENT_LOCALIZATION_LANGUAGE.get_or_init(|| RwLock::new(String::from("English")));
    match lock.read() {
        Ok(guard) => {
            if guard.trim().is_empty() {
                "English".to_string()
            } else {
                guard.clone()
            }
        }
        Err(_) => "English".to_string(),
    }
}

fn current_audio_path_settings() -> AudioPathSettings {
    let mut settings = AudioPathSettings::default();

    if let Some(manager) = super::game_audio::get_global_audio_manager() {
        // Use try_lock: generate_filename is often called while AudioManager is already
        // locked (add_audio_event → generate_filename). A blocking re-lock deadlocks
        // the calling thread forever (seen in special-power money withdraw tests).
        match manager.try_lock() {
            Ok(guard) => {
                let audio_settings = guard.get_audio_settings();
                settings.audio_root = audio_settings.audio_root.clone();
                settings.sounds_folder = audio_settings.sounds_folder.clone();
                settings.music_folder = audio_settings.music_folder.clone();
                settings.streaming_folder = audio_settings.streaming_folder.clone();
                settings.sounds_extension = audio_settings.sounds_extension.clone();
            }
            Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                let guard = poisoned.into_inner();
                let audio_settings = guard.get_audio_settings();
                settings.audio_root = audio_settings.audio_root.clone();
                settings.sounds_folder = audio_settings.sounds_folder.clone();
                settings.music_folder = audio_settings.music_folder.clone();
                settings.streaming_folder = audio_settings.streaming_folder.clone();
                settings.sounds_extension = audio_settings.sounds_extension.clone();
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                // Nested call under an existing AudioManager lock — defaults are fine.
            }
        }
    }

    settings
}

fn audio_file_exists(filename: &str) -> bool {
    let file_system = get_file_system();
    if let Ok(guard) = file_system.lock() {
        if guard.does_file_exist(filename) {
            return true;
        }
    }

    Path::new(filename).exists() || Path::new(&filename.replace('\\', "/")).exists()
}

/// This is called AudioEventRts because AudioEvent is a typedef in ww3d
/// Enhanced with async capabilities and modern Rust patterns
#[derive(Debug)]
pub struct AudioEventRts {
    pub filename_to_load: AsciiString,
    pub event_info: Option<Arc<AudioEventInfo>>, // Use Arc for sharing
    pub playing_handle: AudioHandle,
    pub kill_this_handle: AudioHandle, // Sometimes sounds will cannibalize other sounds
    pub event_name: AsciiString,
    pub attack_name: AsciiString,
    pub decay_name: AsciiString,
    pub priority: AudioPriority,
    pub volume: Real,
    pub time_of_day: TimeOfDay,
    pub position_of_audio: Coord3D,
    pub object_id: ObjectId,     // Union in C++
    pub drawable_id: DrawableId, // Union in C++
    pub owner_type: OwnerType,
    pub should_fade: Bool,
    pub is_logical_audio: Bool,
    pub uninterruptable: Bool,
    pub pitch_shift: Real,
    pub volume_shift: Real,
    pub delay: Real,
    pub loop_count: Int,
    pub playing_audio_index: AtomicI32,
    pub all_count: Int,
    pub player_index: Int,
    pub portion_to_play_next: PortionToPlay,
    /// C++ `PlayingAudio::m_requestStop` — leave the sample running for Decay.
    pub request_stop: bool,
    // New async-related fields
    pub playback_future: Option<tokio::task::JoinHandle<Result<(), AudioError>>>,
    pub completion_sender: Option<mpsc::UnboundedSender<AudioEventComplete>>,
}

/// Audio error types for async operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum AudioError {
    #[error("Audio file not found: {filename}")]
    FileNotFound { filename: String },
    #[error("Audio format not supported: {format}")]
    UnsupportedFormat { format: String },
    #[error("Audio playback failed: {reason}")]
    PlaybackFailed { reason: String },
    #[error("Audio device error: {device_error}")]
    DeviceError { device_error: String },
    #[error("Audio stream error: {stream_error}")]
    StreamError { stream_error: String },
    #[error("Audio initialization failed")]
    InitializationFailed,
    #[error("Audio event cancelled")]
    Cancelled,
    #[error("Audio timeout")]
    Timeout,
}

/// Audio event completion notification
#[derive(Debug, Clone)]
pub struct AudioEventComplete {
    pub handle: AudioHandle,
    pub event_name: String,
    pub success: bool,
    pub error: Option<AudioError>,
}

/// Async audio event trait
#[async_trait]
pub trait AsyncAudioEvent {
    async fn play(&mut self) -> Result<(), AudioError>;
    async fn stop(&mut self) -> Result<(), AudioError>;
    async fn pause(&mut self) -> Result<(), AudioError>;
    async fn resume(&mut self) -> Result<(), AudioError>;
    async fn set_volume(&mut self, volume: Real) -> Result<(), AudioError>;
    async fn set_position(&mut self, position: &Coord3D) -> Result<(), AudioError>;
    async fn is_playing(&self) -> bool;
    async fn get_duration(&self) -> Option<f64>;
    async fn get_current_time(&self) -> Option<f64>;
}

impl Default for AudioEventRts {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AudioEventRts {
    fn clone(&self) -> Self {
        AudioEventRts {
            filename_to_load: self.filename_to_load.clone(),
            event_info: self.event_info.clone(),
            playing_handle: self.playing_handle,
            kill_this_handle: self.kill_this_handle,
            event_name: self.event_name.clone(),
            attack_name: self.attack_name.clone(),
            decay_name: self.decay_name.clone(),
            priority: self.priority,
            volume: self.volume,
            time_of_day: self.time_of_day,
            position_of_audio: Coord3D {
                x: self.position_of_audio.x,
                y: self.position_of_audio.y,
                z: self.position_of_audio.z,
            },
            object_id: self.object_id,
            drawable_id: self.drawable_id,
            owner_type: self.owner_type,
            should_fade: self.should_fade,
            is_logical_audio: self.is_logical_audio,
            uninterruptable: self.uninterruptable,
            pitch_shift: self.pitch_shift,
            volume_shift: self.volume_shift,
            delay: self.delay,
            loop_count: self.loop_count,
            playing_audio_index: AtomicI32::new(self.playing_audio_index.load(Ordering::Relaxed)),
            all_count: self.all_count,
            player_index: self.player_index,
            portion_to_play_next: self.portion_to_play_next,
            request_stop: self.request_stop,
            // Don't clone async handles - they should be recreated
            playback_future: None,
            completion_sender: None,
        }
    }
}

impl AudioEventRts {
    pub fn new() -> Self {
        AudioEventRts {
            filename_to_load: String::new(),
            event_info: None,
            playing_handle: 0,
            kill_this_handle: 0,
            event_name: String::new(),
            attack_name: String::new(),
            decay_name: String::new(),
            priority: AP_NORMAL,
            volume: -1.0,
            time_of_day: TIME_OF_DAY_AFTERNOON,
            position_of_audio: Coord3D::new(),
            object_id: INVALID_ID,
            drawable_id: INVALID_DRAWABLE_ID,
            owner_type: OwnerType::Invalid,
            should_fade: false,
            is_logical_audio: false,
            uninterruptable: false,
            pitch_shift: 1.0,
            volume_shift: 0.0,
            delay: 0.0,
            loop_count: 1,
            playing_audio_index: AtomicI32::new(-1),
            all_count: 0,
            player_index: -1,
            portion_to_play_next: PortionToPlay::Attack,
            request_stop: false,
            // Initialize async fields
            playback_future: None,
            completion_sender: None,
        }
    }

    pub fn with_event_name(event_name: &str) -> Self {
        let mut audio_event = Self::new();
        audio_event.event_name = event_name.to_string();
        audio_event
    }

    pub fn with_object_id(event_name: &str, owner_id: ObjectId) -> Self {
        let mut audio_event = Self::with_event_name(event_name);
        if owner_id != 0 {
            audio_event.object_id = owner_id;
            audio_event.owner_type = OwnerType::Object;
        } else {
            audio_event.object_id = INVALID_ID;
        }
        audio_event
    }

    pub fn with_drawable_id(event_name: &str, drawable_id: DrawableId) -> Self {
        let mut audio_event = Self::with_event_name(event_name);
        if drawable_id != 0 {
            audio_event.drawable_id = drawable_id;
            audio_event.owner_type = OwnerType::Drawable;
        } else {
            audio_event.drawable_id = INVALID_DRAWABLE_ID;
        }
        audio_event
    }

    pub fn with_position(event_name: &str, position_of_audio: &Coord3D) -> Self {
        let mut audio_event = Self::with_event_name(event_name);
        audio_event.position_of_audio.set(position_of_audio);
        audio_event.owner_type = OwnerType::Positional;
        audio_event
    }

    pub fn set_event_name(&mut self, name: String) {
        if name != self.event_name && self.event_info.is_some() {
            // Clear out the audio event info, cause its not valid for the new event.
            self.event_info = None;
        }
        self.event_name = name;
    }

    pub fn get_event_name(&self) -> &str {
        &self.event_name
    }

    pub fn generate_filename(&mut self) {
        let Some(event_info) = self.event_info.as_ref().map(Arc::clone) else {
            return;
        };

        self.filename_to_load = self.generate_filename_prefix(event_info.sound_type, false);

        if matches!(
            event_info.sound_type,
            AudioType::Music | AudioType::Streaming
        ) {
            self.filename_to_load.push_str(&event_info.filename);
            let sound_type = event_info.sound_type;
            let mut localized = self.filename_to_load.clone();
            self.adjust_for_localization(&mut localized, sound_type);
            self.filename_to_load = localized;
            if let Some(resolved) = resolve_extracted_audiozh_path(&self.filename_to_load) {
                self.filename_to_load = resolved;
            }
            return;
        }

        if event_info.sounds.is_empty() {
            self.filename_to_load.clear();
            return;
        }

        let which = if (event_info.control & AC_RANDOM) != 0 {
            let max_index = event_info.sounds.len() as i32 - 1;
            let mut idx = if self.is_logical_audio {
                get_game_logic_random_value(0, max_index) as usize
            } else {
                get_game_audio_random_value(0, max_index) as usize
            };

            if idx as i32 == self.playing_audio_index.load(Ordering::Relaxed)
                && event_info.sounds.len() > 2
            {
                idx = (idx + 1) % event_info.sounds.len();
            }

            self.playing_audio_index
                .store(idx as i32, Ordering::Relaxed);
            idx
        } else {
            let next = (self.playing_audio_index.load(Ordering::Relaxed) + 1)
                .rem_euclid(event_info.sounds.len() as i32);
            self.playing_audio_index.store(next, Ordering::Relaxed);
            next as usize
        };

        self.filename_to_load.push_str(&event_info.sounds[which]);
        self.filename_to_load
            .push_str(&self.generate_filename_extension(event_info.sound_type));
        let sound_type = event_info.sound_type;
        let mut localized = self.filename_to_load.clone();
        self.adjust_for_localization(&mut localized, sound_type);
        self.filename_to_load = localized;
        if let Some(resolved) = resolve_extracted_audiozh_path(&self.filename_to_load) {
            self.filename_to_load = resolved;
        }

        self.delay = get_game_audio_random_value_real(event_info.delay_min, event_info.delay_max);
    }

    fn pick_random_index(&self, count: usize) -> usize {
        if count == 0 {
            return 0;
        }

        let max_index = count as i32 - 1;
        if self.is_logical_audio {
            get_game_logic_random_value(0, max_index) as usize
        } else {
            get_game_audio_random_value(0, max_index) as usize
        }
    }

    pub fn get_filename(&self) -> &str {
        &self.filename_to_load
    }

    pub fn generate_play_info(&mut self) {
        let Some(event_info) = self.event_info.as_ref().map(Arc::clone) else {
            self.is_logical_audio = false;
            return;
        };

        self.pitch_shift = get_game_audio_random_value_real(
            event_info.pitch_shift_min,
            event_info.pitch_shift_max,
        );
        self.volume_shift = get_game_audio_random_value_real(1.0 + event_info.volume_shift, 1.0);
        self.loop_count = event_info.loop_count;

        self.portion_to_play_next = PortionToPlay::Attack;

        let attack_size = event_info.attack_sounds.len();
        if attack_size > 0 {
            self.attack_name = self.generate_filename_prefix(event_info.sound_type, false);
            let attack_index = self.pick_random_index(attack_size);
            self.attack_name
                .push_str(&event_info.attack_sounds[attack_index]);
            self.attack_name
                .push_str(&self.generate_filename_extension(event_info.sound_type));
            let mut localized = self.attack_name.clone();
            self.adjust_for_localization(&mut localized, event_info.sound_type);
            self.attack_name = localized;
        } else {
            self.portion_to_play_next = PortionToPlay::Sound;
        }

        let decay_size = event_info.decay_sounds.len();
        if decay_size > 0 {
            self.decay_name = self.generate_filename_prefix(event_info.sound_type, false);
            let decay_index = self.pick_random_index(decay_size);
            self.decay_name
                .push_str(&event_info.decay_sounds[decay_index]);
            self.decay_name
                .push_str(&self.generate_filename_extension(event_info.sound_type));
            let mut localized = self.decay_name.clone();
            self.adjust_for_localization(&mut localized, event_info.sound_type);
            self.decay_name = localized;
        }

        self.is_logical_audio = false;
    }

    // Getters and setters
    pub fn get_audio_event_info(&self) -> Option<Arc<AudioEventInfo>> {
        // C++ parity: cached event info is only valid while audio_name matches
        // the current event name; stale cache entries are treated as missing.
        self.event_info.as_ref().and_then(|info| {
            if info.audio_name == self.event_name {
                Some(info.clone())
            } else {
                None
            }
        })
    }

    pub fn set_audio_event_info(&mut self, info: Arc<AudioEventInfo>) {
        self.event_info = Some(info);
    }

    pub fn get_playing_handle(&self) -> AudioHandle {
        self.playing_handle
    }

    pub fn set_playing_handle(&mut self, handle: AudioHandle) {
        self.playing_handle = handle;
    }

    pub fn is_currently_playing(&self) -> bool {
        if let Some(manager) = super::game_audio::get_global_audio_manager() {
            if let Ok(guard) = manager.lock() {
                return guard.is_currently_playing(self.playing_handle);
            }
        }

        self.playing_handle != 0
    }

    pub fn get_request_stop(&self) -> bool {
        self.request_stop
    }

    pub fn set_request_stop(&mut self, request_stop: bool) {
        self.request_stop = request_stop;
    }

    pub fn set_object_id(&mut self, id: ObjectId) {
        if !matches!(self.owner_type, OwnerType::Object | OwnerType::Invalid) {
            return;
        }

        self.object_id = id;
        self.owner_type = OwnerType::Object;
    }

    pub fn get_volume(&self) -> Real {
        if self.volume == -1.0 {
            if let Some(event_info) = self.event_info.as_ref() {
                return event_info.volume;
            }
            return 0.5;
        }

        self.volume
    }

    pub fn set_volume(&mut self, volume: Real) {
        self.volume = volume;
    }

    pub fn get_priority(&self) -> AudioPriority {
        self.priority
    }

    pub fn set_priority(&mut self, priority: AudioPriority) {
        self.priority = priority;
    }

    pub fn get_position(&self) -> &Coord3D {
        &self.position_of_audio
    }

    pub fn set_position(&mut self, position: &Coord3D) {
        if !matches!(self.owner_type, OwnerType::Positional | OwnerType::Invalid) {
            return;
        }

        self.position_of_audio.set(position);
        self.owner_type = OwnerType::Positional;
    }

    pub fn is_positional(&self) -> bool {
        matches!(self.owner_type, OwnerType::Positional)
    }

    pub fn is_3d(&self) -> bool {
        self.is_positional()
            && self
                .event_info
                .as_ref()
                .map(|info| info.sound_type == AudioType::SoundEffect)
                .unwrap_or(false)
    }

    pub fn should_be_logical(&self) -> bool {
        !self.is_positional()
    }

    pub fn get_object_id(&self) -> ObjectId {
        if self.owner_type == OwnerType::Object {
            self.object_id
        } else {
            INVALID_ID
        }
    }

    pub fn get_drawable_id(&self) -> DrawableId {
        if self.owner_type == OwnerType::Drawable {
            self.drawable_id
        } else {
            INVALID_DRAWABLE_ID
        }
    }

    pub fn get_owner_type(&self) -> OwnerType {
        self.owner_type
    }

    /// Async audio playback methods
    pub async fn start_async_playback(&mut self) -> Result<(), AudioError> {
        if self.playback_future.is_some() {
            return Err(AudioError::PlaybackFailed {
                reason: "Already playing".to_string(),
            });
        }

        if self.playing_handle == 0 {
            self.playing_handle = ASYNC_PLAYBACK_HANDLE_POOL.fetch_add(1, Ordering::Relaxed);
        }
        let event = self.clone();

        let (completion_tx, _completion_rx) = mpsc::unbounded_channel();
        self.completion_sender = Some(completion_tx.clone());

        let handle = tokio::spawn(async move {
            // This would integrate with the actual audio backend (rodio, cpal, etc.)
            Self::async_audio_playback(event, completion_tx).await
        });

        self.playback_future = Some(handle);
        Ok(())
    }

    pub async fn stop_async_playback(&mut self) -> Result<(), AudioError> {
        if let Some(handle) = self.playback_future.take() {
            handle.abort();
            // Send completion notification
            if let Some(sender) = &self.completion_sender {
                let _ = sender.send(AudioEventComplete {
                    handle: self.playing_handle,
                    event_name: self.event_name.clone(),
                    success: false,
                    error: Some(AudioError::Cancelled),
                });
            }
            self.completion_sender = None;
            Ok(())
        } else {
            Err(AudioError::PlaybackFailed {
                reason: "No active playback to stop".to_string(),
            })
        }
    }

    pub fn is_async_playing(&self) -> bool {
        self.playback_future
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    async fn async_audio_playback(
        mut event: AudioEventRts,
        completion_sender: mpsc::UnboundedSender<AudioEventComplete>,
    ) -> Result<(), AudioError> {
        let event_name = event.event_name.clone();
        let handle = event.playing_handle;

        if event.filename_to_load.is_empty() {
            event.generate_filename();
        }
        if event.filename_to_load.is_empty() {
            if let Some(resolved) = event.resolve_filename() {
                event.filename_to_load = resolved;
            }
        }
        event.generate_play_info();

        if let Some(play_result) =
            super::game_audio::with_sound_playback_hook(|hook| hook.play(&event))
        {
            if let Err(reason) = play_result {
                let error = AudioError::PlaybackFailed { reason };
                let _ = completion_sender.send(AudioEventComplete {
                    handle,
                    event_name,
                    success: false,
                    error: Some(error.clone()),
                });
                return Err(error);
            }

            let poll_interval = std::time::Duration::from_millis(33);
            loop {
                if completion_sender.is_closed() {
                    let _ = super::game_audio::with_sound_playback_hook(|hook| hook.stop(handle));
                    return Err(AudioError::Cancelled);
                }

                let still_playing =
                    super::game_audio::with_sound_playback_hook(|hook| hook.is_playing(handle))
                        .unwrap_or(false);
                if !still_playing {
                    break;
                }

                tokio::time::sleep(poll_interval).await;
            }

            let _ = completion_sender.send(AudioEventComplete {
                handle,
                event_name,
                success: true,
                error: None,
            });
            return Ok(());
        }

        let filename = event.filename_to_load.clone();

        if filename.is_empty() {
            let error = AudioError::FileNotFound {
                filename: filename.clone(),
            };
            let _ = completion_sender.send(AudioEventComplete {
                handle,
                event_name,
                success: false,
                error: Some(error.clone()),
            });
            return Err(error);
        }

        let duration = std::time::Duration::from_millis(250);
        let simulated_loops = event.loop_count.max(1);

        for _ in 0..simulated_loops {
            // Check if cancelled
            if completion_sender.is_closed() {
                return Err(AudioError::Cancelled);
            }

            tokio::time::sleep(duration).await;
        }

        // Send completion notification
        let _ = completion_sender.send(AudioEventComplete {
            handle,
            event_name,
            success: true,
            error: None,
        });

        Ok(())
    }

    /// Check if the audio event should interrupt other audio
    pub fn should_interrupt(&self) -> bool {
        if let Some(event_info) = &self.event_info {
            (event_info.control & AC_INTERRUPT) != 0
        } else {
            false
        }
    }

    /// Check if the audio event should loop
    pub fn should_loop(&self) -> bool {
        if let Some(event_info) = &self.event_info {
            (event_info.control & AC_LOOP) != 0
        } else {
            false
        }
    }

    /// Check if the audio event is random
    pub fn is_random(&self) -> bool {
        if let Some(event_info) = &self.event_info {
            (event_info.control & AC_RANDOM) != 0
        } else {
            false
        }
    }

    /// Get the effective volume (including shifts)
    pub fn get_effective_volume(&self) -> Real {
        let base_volume = if self.volume < 0.0 {
            // Use event info volume if no explicit volume set
            self.event_info
                .as_ref()
                .map(|info| info.volume)
                .unwrap_or(1.0)
        } else {
            self.volume
        };

        (base_volume + self.volume_shift).clamp(0.0, 1.0)
    }

    /// Get the effective pitch
    pub fn get_effective_pitch(&self) -> Real {
        self.pitch_shift.clamp(0.1, 10.0) // Reasonable pitch range
    }

    /// Reset the audio event to initial state
    pub fn reset(&mut self) {
        self.filename_to_load.clear();
        self.event_info = None;
        self.playing_handle = 0;
        self.kill_this_handle = 0;
        self.priority = AP_NORMAL;
        self.volume = -1.0;
        self.time_of_day = TIME_OF_DAY_AFTERNOON;
        self.position_of_audio.zero();
        self.object_id = INVALID_ID;
        self.drawable_id = INVALID_DRAWABLE_ID;
        self.owner_type = OwnerType::Invalid;
        self.should_fade = false;
        self.is_logical_audio = false;
        self.uninterruptable = false;
        self.pitch_shift = 1.0;
        self.volume_shift = 0.0;
        self.delay = 0.0;
        self.loop_count = 1;
        self.playing_audio_index.store(-1, Ordering::Relaxed);
        self.all_count = 0;
        self.player_index = -1;
        self.portion_to_play_next = PortionToPlay::Attack;
        self.request_stop = false;

        // Reset async components
        if let Some(handle) = self.playback_future.take() {
            handle.abort();
        }
        self.completion_sender = None;
    }

    pub fn get_pitch_shift(&self) -> Real {
        self.pitch_shift
    }

    pub fn set_pitch_shift(&mut self, pitch_shift: Real) {
        self.pitch_shift = pitch_shift;
    }

    pub fn get_volume_shift(&self) -> Real {
        self.volume_shift
    }

    pub fn set_volume_shift(&mut self, volume_shift: Real) {
        self.volume_shift = volume_shift;
    }

    pub fn get_delay(&self) -> Real {
        self.delay
    }

    pub fn set_delay(&mut self, delay: Real) {
        self.delay = delay;
    }

    pub fn get_loop_count(&self) -> Int {
        self.loop_count
    }

    pub fn set_loop_count(&mut self, loop_count: Int) {
        self.loop_count = loop_count;
    }

    pub fn get_player_index(&self) -> Int {
        match self.owner_type {
            OwnerType::Object => {
                if let Some(player_index) = with_audio_event_owner_resolver(|resolver| {
                    resolver.resolve_object_player_index(self.object_id)
                })
                .flatten()
                {
                    return player_index;
                }
            }
            OwnerType::Drawable => {
                if let Some(player_index) = with_audio_event_owner_resolver(|resolver| {
                    resolver.resolve_drawable_player_index(self.drawable_id)
                })
                .flatten()
                {
                    return player_index;
                }
            }
            _ => {}
        }

        self.player_index
    }

    pub fn set_player_index(&mut self, player_index: Int) {
        self.player_index = player_index;
    }

    pub fn get_time_of_day(&self) -> TimeOfDay {
        self.time_of_day
    }

    pub fn set_time_of_day(&mut self, time_of_day: TimeOfDay) {
        self.time_of_day = time_of_day;
    }

    pub fn should_fade(&self) -> Bool {
        self.should_fade
    }

    pub fn set_should_fade(&mut self, should_fade: Bool) {
        self.should_fade = should_fade;
    }

    pub fn is_uninterruptable(&self) -> Bool {
        self.uninterruptable
    }

    pub fn set_uninterruptable(&mut self, uninterruptable: Bool) {
        self.uninterruptable = uninterruptable;
    }

    pub fn get_attack_filename(&self) -> &str {
        &self.attack_name
    }

    pub fn get_decay_filename(&self) -> &str {
        &self.decay_name
    }

    pub fn decrement_delay(&mut self, time_to_decrement: Real) {
        self.delay -= time_to_decrement;
    }

    pub fn get_next_play_portion(&self) -> PortionToPlay {
        self.portion_to_play_next
    }

    pub fn advance_next_play_portion(&mut self) {
        match self.portion_to_play_next {
            PortionToPlay::Attack => {
                self.portion_to_play_next = PortionToPlay::Sound;
            }
            PortionToPlay::Sound => {
                if let Some(event_info) = &self.event_info {
                    if (event_info.control & AC_ALL) != 0 {
                        if self.all_count == event_info.sounds.len() as i32 {
                            self.portion_to_play_next = PortionToPlay::Decay;
                        }
                        self.all_count += 1;
                    }
                }
                if !self.decay_name.is_empty() {
                    self.portion_to_play_next = PortionToPlay::Decay;
                } else {
                    self.portion_to_play_next = PortionToPlay::Done;
                }
            }
            PortionToPlay::Decay => {
                self.portion_to_play_next = PortionToPlay::Done;
            }
            PortionToPlay::Done => {} // Already done
        }
    }

    pub fn set_next_play_portion(&mut self, ptp: PortionToPlay) {
        self.portion_to_play_next = ptp;
    }

    pub fn decrease_loop_count(&mut self) {
        if self.loop_count == 1 {
            self.loop_count = -1;
        } else if self.loop_count > 1 {
            self.loop_count -= 1;
        }
    }

    pub fn has_more_loops(&self) -> Bool {
        self.loop_count >= 0
    }

    // Note: Methods are already implemented above

    pub fn set_position_override(&mut self, pos: &Coord3D) {
        if !matches!(self.owner_type, OwnerType::Positional | OwnerType::Invalid) {
            return;
        }
        self.position_of_audio = Coord3D {
            x: pos.x,
            y: pos.y,
            z: pos.z,
        };
        self.owner_type = OwnerType::Positional;
    }

    pub fn get_position_override(&self) -> Option<&Coord3D> {
        if self.owner_type != OwnerType::Invalid {
            Some(&self.position_of_audio)
        } else {
            None
        }
    }

    pub fn set_object_id_override(&mut self, obj_id: ObjectId) {
        if !matches!(self.owner_type, OwnerType::Object | OwnerType::Invalid) {
            return;
        }
        self.object_id = obj_id;
        self.owner_type = OwnerType::Object;
    }

    pub fn get_object_id_override(&self) -> ObjectId {
        if self.owner_type == OwnerType::Object {
            self.object_id
        } else {
            INVALID_ID
        }
    }

    pub fn set_drawable_id_override(&mut self, draw_id: DrawableId) {
        if !matches!(self.owner_type, OwnerType::Drawable | OwnerType::Invalid) {
            return;
        }
        self.drawable_id = draw_id;
        self.owner_type = OwnerType::Drawable;
    }

    pub fn get_playing_audio_index(&self) -> Int {
        self.playing_audio_index.load(Ordering::Relaxed)
    }

    pub fn set_playing_audio_index(&self, pai: Int) {
        self.playing_audio_index.store(pai, Ordering::Relaxed);
    }

    pub fn set_handle_to_kill(&mut self, handle_to_kill: AudioHandle) {
        self.kill_this_handle = handle_to_kill;
    }

    pub fn get_handle_to_kill(&self) -> AudioHandle {
        self.kill_this_handle
    }

    pub fn set_is_logical_audio(&mut self, is_logical_audio: Bool) {
        self.is_logical_audio = is_logical_audio;
    }

    pub fn get_is_logical_audio(&self) -> Bool {
        self.is_logical_audio
    }

    pub fn is_positional_audio(&self) -> Bool {
        if let Some(event_info) = &self.event_info {
            if (event_info.type_field & ST_WORLD) == 0 {
                return false;
            }
        }
        if self.owner_type != OwnerType::Invalid {
            if self.drawable_id != INVALID_DRAWABLE_ID
                || self.object_id != INVALID_ID
                || self.owner_type == OwnerType::Positional
            {
                return true;
            }
        }
        false
    }

    pub fn get_audio_priority(&self) -> AudioPriority {
        self.priority
    }

    pub fn set_audio_priority(&mut self, new_priority: AudioPriority) {
        self.priority = new_priority;
    }

    pub fn is_dead(&self) -> bool {
        self.owner_type == OwnerType::Dead
    }

    pub fn get_uninterruptable(&self) -> Bool {
        self.uninterruptable
    }

    pub fn sound_candidates(&self) -> Vec<String> {
        let mut candidates = Vec::new();
        if let Some(info) = self.event_info.as_ref() {
            for name in &info.sounds {
                if !name.is_empty() {
                    candidates.push(name.clone());
                }
            }
            for name in &info.attack_sounds {
                if !name.is_empty() {
                    candidates.push(name.clone());
                }
            }
            for name in &info.decay_sounds {
                if !name.is_empty() {
                    candidates.push(name.clone());
                }
            }
            if candidates.is_empty() && !info.filename.is_empty() {
                candidates.push(info.filename.clone());
            }
        }
        if candidates.is_empty() && !self.event_name.is_empty() {
            candidates.push(self.event_name.clone());
        }
        candidates
    }

    pub fn resolve_filename(&self) -> Option<String> {
        let audio_type = self
            .event_info
            .as_ref()
            .map(|info| info.sound_type)
            .unwrap_or(AudioType::SoundEffect);
        let prefix = self.generate_filename_prefix(audio_type, false);
        let extension = self.generate_filename_extension(audio_type);

        for candidate in self.sound_candidates() {
            let trimmed = candidate.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains('/') || trimmed.contains('\\') {
                return Some(trimmed.to_string());
            }
            if trimmed.contains('.') {
                return Some(format!("{prefix}{trimmed}"));
            }
            return Some(format!("{prefix}{trimmed}{extension}"));
        }
        None
    }

    // Owner-bound state is resolved through optional runtime hooks.
    pub fn get_current_position(&mut self) -> Option<&Coord3D> {
        match self.owner_type {
            OwnerType::Positional => Some(&self.position_of_audio),
            OwnerType::Object => {
                match with_audio_event_owner_resolver(|resolver| {
                    resolver.resolve_object_position(self.object_id)
                }) {
                    Some(Some(position)) => {
                        self.position_of_audio.set(&position);
                    }
                    Some(None) => {
                        self.owner_type = OwnerType::Dead;
                    }
                    None => {}
                }
                Some(&self.position_of_audio)
            }
            OwnerType::Drawable => {
                match with_audio_event_owner_resolver(|resolver| {
                    resolver.resolve_drawable_position(self.drawable_id)
                }) {
                    Some(Some(position)) => {
                        self.position_of_audio.set(&position);
                    }
                    Some(None) => {
                        self.owner_type = OwnerType::Dead;
                    }
                    None => {}
                }
                Some(&self.position_of_audio)
            }
            OwnerType::Dead => Some(&self.position_of_audio),
            OwnerType::Invalid => None,
        }
    }

    pub fn generate_filename_prefix(
        &self,
        audio_type_to_play: AudioType,
        localized: Bool,
    ) -> String {
        let settings = current_audio_path_settings();

        let mut ret_str = settings.audio_root;
        ret_str.push('\\');
        match audio_type_to_play {
            AudioType::Music => ret_str.push_str(&settings.music_folder),
            AudioType::Streaming => ret_str.push_str(&settings.streaming_folder),
            _ => ret_str.push_str(&settings.sounds_folder),
        }
        ret_str.push('\\');
        if localized {
            ret_str.push_str(&current_localization_language());
            ret_str.push('\\');
        }
        ret_str
    }

    pub fn generate_filename_extension(&self, audio_type_to_play: AudioType) -> String {
        if audio_type_to_play != AudioType::Music {
            let settings = current_audio_path_settings();
            let extension = settings.sounds_extension.trim();
            if extension.starts_with('.') {
                extension.to_string()
            } else {
                format!(".{extension}")
            }
        } else {
            String::new()
        }
    }

    fn adjust_for_localization(&self, filename: &mut String, audio_type_to_play: AudioType) {
        let Some(index) = filename.rfind('\\') else {
            return;
        };

        let mut localized_path = self.generate_filename_prefix(audio_type_to_play, true);
        localized_path.push_str(&filename[index..]);

        if audio_file_exists(&localized_path) {
            *filename = localized_path;
        }
    }
}

/// Dynamic version that can be allocated on the heap
pub struct DynamicAudioEventRts {
    pub event: AudioEventRts,
}

impl DynamicAudioEventRts {
    pub fn new() -> Self {
        DynamicAudioEventRts {
            event: AudioEventRts::new(),
        }
    }

    pub fn from_event(event: AudioEventRts) -> Self {
        DynamicAudioEventRts { event }
    }
}

impl Default for DynamicAudioEventRts {
    fn default() -> Self {
        Self::new()
    }
}

// Copy trait is now derived above with Clone

/// Resolve WAV/MP3 under `windows_game/extracted_big_files/AudioZH` when present.
/// No Miles 3D — this only remaps the filename the mixer will open.
pub fn resolve_extracted_audiozh_path(filename: &str) -> Option<String> {
    if filename.is_empty() {
        return None;
    }
    let native = filename.replace('\\', "/");
    let leaf = Path::new(&native)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(&native);
    let stem = Path::new(leaf)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(leaf);
    const ROOTS: &[&str] = &[
        "windows_game/extracted_big_files/AudioZH",
        "windows_game/extracted_big_files_v2/AudioZH",
        "../windows_game/extracted_big_files/AudioZH",
        "../../windows_game/extracted_big_files/AudioZH",
    ];
    const SUBS: &[&str] = &["", "Sounds", "Music", "Speech", "Streams"];
    const EXTS: &[&str] = &[".wav", ".mp3", ".WAV", ".MP3"];
    let mut candidates: Vec<PathBuf> = Vec::new();
    for root in ROOTS {
        candidates.push(PathBuf::from(root).join(&native));
        for sub in SUBS {
            let dir = if sub.is_empty() {
                PathBuf::from(root)
            } else {
                PathBuf::from(root).join(sub)
            };
            candidates.push(dir.join(leaf));
            for ext in EXTS {
                candidates.push(dir.join(format!("{stem}{ext}")));
            }
        }
    }
    candidates.into_iter().find(|p| p.is_file()).map(|p| {
        p.to_string_lossy()
            .replace('/', std::path::MAIN_SEPARATOR_STR)
    })
}

// Tests module
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[test]
    fn test_audio_event_creation() {
        let event = AudioEventRts::new();
        assert_eq!(event.event_name, "");
        assert_eq!(event.volume, -1.0);
        assert_eq!(event.priority, AP_NORMAL);
        assert_eq!(event.owner_type, OwnerType::Invalid);
    }

    #[test]
    fn test_audio_event_with_name() {
        let event = AudioEventRts::with_event_name("test_sound");
        assert_eq!(event.event_name, "test_sound");
    }

    #[test]
    fn test_audio_event_with_position() {
        let pos = Coord3D {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };
        let event = AudioEventRts::with_position("test_sound", &pos);
        assert_eq!(event.position_of_audio.x, 10.0);
        assert_eq!(event.position_of_audio.y, 20.0);
        assert_eq!(event.position_of_audio.z, 30.0);
        assert_eq!(event.owner_type, OwnerType::Positional);
    }

    #[test]
    fn test_coord3d_operations() {
        let mut coord = Coord3D::new();
        assert_eq!(coord.x, 0.0);
        assert_eq!(coord.y, 0.0);
        assert_eq!(coord.z, 0.0);

        let other = Coord3D {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        coord.set(&other);
        assert_eq!(coord.x, 1.0);
        assert_eq!(coord.y, 2.0);
        assert_eq!(coord.z, 3.0);
        coord.add(&other);
        assert_eq!(coord.x, 2.0);
        assert_eq!(coord.y, 4.0);
        assert_eq!(coord.z, 6.0);

        let length = coord.length();
        assert!((length - (4.0 + 16.0 + 36.0_f32).sqrt()).abs() < 0.001);
    }

    #[test]
    fn miles_positional_gain_matches_cpp_inverse_min_distance() {
        assert_eq!(miles_positional_gain(10.0, 25.0, 1000.0), 1.0);
        assert!((miles_positional_gain(50.0, 25.0, 1000.0) - 0.5).abs() < 1.0e-5);
        assert_eq!(miles_positional_gain(1000.0, 25.0, 1000.0), 0.0);
        assert_eq!(miles_positional_gain(1001.0, 25.0, 1000.0), 0.0);
    }

    #[test]
    fn miles_get_effective_volume_matches_cpp_music_speech_and_3d() {
        fn info(sound_type: AudioType, type_field: u32, min_d: f32, max_d: f32) -> AudioEventInfo {
            AudioEventInfo {
                sound_type,
                control: 0,
                audio_name: "Test".to_string(),
                volume: 1.0,
                sounds_morning: Vec::new(),
                sounds: Vec::new(),
                sounds_night: Vec::new(),
                sounds_evening: Vec::new(),
                attack_sounds: Vec::new(),
                decay_sounds: Vec::new(),
                pitch_shift_min: 1.0,
                pitch_shift_max: 1.0,
                volume_shift: 0.0,
                min_volume: 0.0,
                limit: 0,
                loop_count: 1,
                delay_min: 0.0,
                delay_max: 0.0,
                filename: String::new(),
                sound_type_field: sound_type,
                type_field,
                priority: AudioPriority::Normal,
                min_distance: min_d,
                max_distance: max_d,
                ..Default::default()
            }
        }

        let sliders = MilesVolumeSliders::default();
        let listener = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };

        let mut music = AudioEventRts::new();
        music.set_event_name("Test".to_string());
        music.set_audio_event_info(std::sync::Arc::new(info(AudioType::Music, 0, 25.0, 1000.0)));
        music.set_volume(0.8);
        music.set_volume_shift(1.0);
        assert!((miles_get_effective_volume(&music, &listener, &sliders) - 0.44).abs() < 1e-5);

        let mut speech = AudioEventRts::new();
        speech.set_event_name("Test".to_string());
        speech.set_audio_event_info(std::sync::Arc::new(info(
            AudioType::Streaming,
            0,
            25.0,
            1000.0,
        )));
        speech.set_volume(1.0);
        speech.set_volume_shift(1.0);
        assert!((miles_get_effective_volume(&speech, &listener, &sliders) - 0.55).abs() < 1e-5);

        let mut sfx2d = AudioEventRts::new();
        sfx2d.set_event_name("Test".to_string());
        sfx2d.set_audio_event_info(std::sync::Arc::new(info(
            AudioType::SoundEffect,
            0,
            25.0,
            1000.0,
        )));
        sfx2d.set_volume(1.0);
        sfx2d.set_volume_shift(1.0);
        assert!((miles_get_effective_volume(&sfx2d, &listener, &sliders) - 0.75).abs() < 1e-5);

        let mut sfx3d = AudioEventRts::new();
        sfx3d.set_event_name("Test".to_string());
        sfx3d.set_audio_event_info(std::sync::Arc::new(info(
            AudioType::SoundEffect,
            ST_WORLD,
            25.0,
            1000.0,
        )));
        sfx3d.set_volume(1.0);
        sfx3d.set_volume_shift(1.0);
        sfx3d.set_position(&Coord3D {
            x: 50.0,
            y: 0.0,
            z: 0.0,
        });
        // 0.75 * 1/(50/25) = 0.375
        assert!((miles_get_effective_volume(&sfx3d, &listener, &sliders) - 0.375).abs() < 1e-5);

        sfx3d.set_position(&Coord3D {
            x: 1000.0,
            y: 0.0,
            z: 0.0,
        });
        assert_eq!(miles_get_effective_volume(&sfx3d, &listener, &sliders), 0.0);

        let mut global3d = AudioEventRts::new();
        global3d.set_audio_event_info(std::sync::Arc::new(info(
            AudioType::SoundEffect,
            ST_WORLD | ST_GLOBAL,
            5.0,
            40.0,
        )));
        global3d.set_volume(1.0);
        global3d.set_volume_shift(1.0);
        global3d.set_position(&Coord3D {
            x: 50.0,
            y: 0.0,
            z: 0.0,
        });
        // ST_GLOBAL uses 25/1000, not 5/40 → same 0.375
        assert!((miles_get_effective_volume(&global3d, &listener, &sliders) - 0.375).abs() < 1e-5);
    }

    fn miles_positional_ranges_use_global_when_st_global() {
        let mut info = AudioEventInfo {
            sound_type: AudioType::SoundEffect,
            control: 0,
            audio_name: String::new(),
            volume: 1.0,
            sounds_morning: Vec::new(),
            sounds: Vec::new(),
            sounds_night: Vec::new(),
            sounds_evening: Vec::new(),
            attack_sounds: Vec::new(),
            decay_sounds: Vec::new(),
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            volume_shift: 0.0,
            min_volume: 0.0,
            limit: 0,
            loop_count: 1,
            delay_min: 0.0,
            delay_max: 0.0,
            filename: String::new(),
            sound_type_field: AudioType::SoundEffect,
            type_field: ST_GLOBAL,
            priority: AudioPriority::Normal,
            min_distance: 5.0,
            max_distance: 40.0,
            ..Default::default()
        };
        assert_eq!(
            miles_positional_ranges(Some(&info), 25.0, 1000.0),
            (25.0, 1000.0)
        );
        info.type_field = 0;
        assert_eq!(
            miles_positional_ranges(Some(&info), 25.0, 1000.0),
            (5.0, 40.0)
        );
        assert!(info.is_permanent_sound() == false);
        info.control = AC_LOOP;
        info.loop_count = 0;
        assert!(info.is_permanent_sound());
    }

    #[test]
    fn test_volume_operations() {
        let mut event = AudioEventRts::new();
        event.set_volume(0.5);
        assert_eq!(event.get_volume(), 0.5);

        event.set_volume_shift(0.2);
        assert_eq!(event.get_effective_volume(), 0.7);

        event.set_volume_shift(-0.3);
        assert!((event.get_effective_volume() - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_loop_operations() {
        let mut event = AudioEventRts::new();
        assert_eq!(event.loop_count, 1);
        assert!(event.has_more_loops());

        event.decrease_loop_count();
        assert_eq!(event.loop_count, -1);
        assert!(!event.has_more_loops());

        event.set_loop_count(3);
        assert!(event.has_more_loops());
        event.decrease_loop_count();
        assert_eq!(event.loop_count, 2);
        assert!(event.has_more_loops());
    }

    #[test]
    fn test_portion_advancement() {
        let mut event = AudioEventRts::new();
        assert_eq!(event.portion_to_play_next, PortionToPlay::Attack);

        event.advance_next_play_portion();
        assert_eq!(event.portion_to_play_next, PortionToPlay::Sound);

        event.advance_next_play_portion();
        assert_eq!(event.portion_to_play_next, PortionToPlay::Done);
    }

    #[test]
    fn test_owner_type_operations() {
        let mut event = AudioEventRts::new();

        event.set_object_id_override(123);
        assert_eq!(event.owner_type, OwnerType::Object);
        assert_eq!(event.get_object_id_override(), 123);

        event.set_drawable_id_override(456);
        // Should not change because it's already set to Object
        assert_eq!(event.owner_type, OwnerType::Object);

        let mut new_event = AudioEventRts::new();
        new_event.set_drawable_id_override(456);
        assert_eq!(new_event.owner_type, OwnerType::Drawable);
        assert_eq!(new_event.get_drawable_id(), 456);
    }

    #[tokio::test]
    async fn test_async_playback() {
        let mut event = AudioEventRts::with_event_name("test_async");
        event.filename_to_load = "test.wav".to_string();

        let result = event.start_async_playback().await;
        assert!(result.is_ok());
        assert!(event.is_async_playing());

        // Allow some time for simulated playback
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let stop_result = event.stop_async_playback().await;
        assert!(stop_result.is_ok());
        assert!(!event.is_async_playing());
    }

    #[tokio::test]
    async fn test_async_playback_file_not_found() {
        let mut event = AudioEventRts::with_event_name("test_error");
        // Leave filename empty to trigger error

        let result = event.start_async_playback().await;
        assert!(result.is_ok()); // Starting is ok, but playback will fail

        // Give it time to fail
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[test]
    fn resolve_extracted_audiozh_empty_is_none() {
        assert!(resolve_extracted_audiozh_path("").is_none());
        // Missing extract tree stays fail-closed (None), no Miles 3D invent.
        assert!(resolve_extracted_audiozh_path("NoSuchEvent.wav").is_none());
    }

    #[test]
    fn test_effective_pitch() {
        let mut event = AudioEventRts::new();
        assert_eq!(event.get_effective_pitch(), 1.0);

        event.set_pitch_shift(2.0);
        assert_eq!(event.get_effective_pitch(), 2.0);

        event.set_pitch_shift(15.0); // Should be clamped
        assert_eq!(event.get_effective_pitch(), 10.0);

        event.set_pitch_shift(0.05); // Should be clamped
        assert_eq!(event.get_effective_pitch(), 0.1);
    }

    #[test]
    fn test_filename_generation() {
        let event = AudioEventRts::new();

        let music_prefix = event.generate_filename_prefix(AudioType::Music, false);
        assert!(music_prefix.contains("Music"));

        let sound_prefix = event.generate_filename_prefix(AudioType::SoundEffect, true);
        assert!(sound_prefix.contains("Sounds"));
        assert!(sound_prefix.contains(&current_localization_language()));

        let wav_ext = event.generate_filename_extension(AudioType::SoundEffect);
        assert_eq!(wav_ext, ".wav");

        let music_ext = event.generate_filename_extension(AudioType::Music);
        assert_eq!(music_ext, "");
    }

    #[test]
    fn test_reset() {
        let mut event = AudioEventRts::with_event_name("test_reset");
        event.set_volume(0.8);
        event.set_pitch_shift(1.5);
        event.set_loop_count(3);

        event.reset();

        assert_eq!(event.event_name, "test_reset");
        assert_eq!(event.volume, -1.0);
        assert_eq!(event.pitch_shift, 1.0);
        assert_eq!(event.loop_count, 1);
        assert_eq!(event.owner_type, OwnerType::Invalid);
    }
}
