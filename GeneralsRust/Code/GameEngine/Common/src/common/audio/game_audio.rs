////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameAudio - Main audio system manager
//! Westwood Studios Pacific
//! Converted to Rust

use crate::common::audio::{
    audio_event_rts::{
        AudioEventInfo, AudioEventRts, AudioHandle, AudioPriority, AudioType, Coord3D, ObjectId,
    },
    audio_request::{AudioRequest, RequestType},
    game_music::create_music_manager,
    game_sounds::create_sound_manager,
};
use crate::common::system::file::FileAccess;
use crate::common::system::file_system::get_file_system;
use glam::Mat4;
use hound::WavReader;
use lewton::inside_ogg::OggStreamReader;
use minimp3::{Decoder as Mp3Decoder, Error as Mp3Error};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock};
use ww3d_animation::{initialize_animated_sound_mgr, set_sound_library, SoundLibraryBridge};
use ww3d_core::errors::{W3DError, W3DResult};

// Type aliases
pub type AsciiString = String;
pub type Real = f32;
pub type Bool = bool;
pub type Int = i32;
pub type UnsignedInt = u32;

/// Hook for routing AudioEventRts playback to the active client audio backend.
pub trait SoundPlaybackHook: Send + Sync {
    fn play(&self, event: &AudioEventRts) -> Result<(), String>;
    fn stop(&self, handle: AudioHandle);
    fn pause(&self, handle: AudioHandle);
    fn resume(&self, handle: AudioHandle);
    fn is_playing(&self, handle: AudioHandle) -> bool;
    fn set_listener_position(&self, _position: &Coord3D) {}
    fn set_event_volume(&self, _event: &AudioEventRts) {}
}

static SOUND_PLAYBACK_HOOK: OnceLock<Arc<dyn SoundPlaybackHook>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioLocalityRelationship {
    Allies,
    Enemies,
    Neutral,
}

/// Resolver for C++ shouldPlayLocally-style player/team checks.
pub trait AudioLocalityResolver: Send + Sync {
    fn get_local_player_index(&self) -> Option<Int>;
    fn get_observer_look_at_player_index(&self) -> Option<Int> {
        None
    }
    fn is_player_active(&self, player_index: Int) -> Bool;
    fn player_exists(&self, player_index: Int) -> Bool;
    fn has_default_team(&self, player_index: Int) -> Bool;
    fn get_relationship_to_local_team(
        &self,
        source_player_index: Int,
        local_player_index: Int,
    ) -> AudioLocalityRelationship;
}

static AUDIO_LOCALITY_RESOLVER: OnceLock<Arc<dyn AudioLocalityResolver>> = OnceLock::new();

/// Resolver for C++ TheTacticalView and TheTerrainLogic access needed by AudioManager::update().
pub trait AudioViewResolver: Send + Sync {
    fn get_tactical_view_position(&self) -> Coord3D;
    fn get_tactical_view_angle(&self) -> Real;
    fn get_3d_camera_position(&self) -> Coord3D;
    fn get_ground_height(&self, x: Real, y: Real) -> Real;
}

static AUDIO_VIEW_RESOLVER: OnceLock<Arc<dyn AudioViewResolver>> = OnceLock::new();

pub fn register_audio_view_resolver(resolver: Arc<dyn AudioViewResolver>) -> bool {
    AUDIO_VIEW_RESOLVER.set(resolver).is_ok()
}

pub fn register_sound_playback_hook(hook: Arc<dyn SoundPlaybackHook>) -> bool {
    SOUND_PLAYBACK_HOOK.set(hook).is_ok()
}

pub fn register_audio_locality_resolver(resolver: Arc<dyn AudioLocalityResolver>) -> bool {
    AUDIO_LOCALITY_RESOLVER.set(resolver).is_ok()
}

pub(crate) fn with_sound_playback_hook<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&dyn SoundPlaybackHook) -> R,
{
    SOUND_PLAYBACK_HOOK.get().map(|hook| f(hook.as_ref()))
}

fn with_audio_locality_resolver<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&dyn AudioLocalityResolver) -> R,
{
    AUDIO_LOCALITY_RESOLVER
        .get()
        .map(|resolver| f(resolver.as_ref()))
}

// Audio system constants
const MAX_HW_PROVIDERS: usize = 4;
const NUM_VOLUME_TYPES: usize = 4;
const ST_UI: u32 = 0x0001;
const ST_PLAYER: u32 = 0x0020;
const ST_ALLIES: u32 = 0x0040;
const ST_ENEMIES: u32 = 0x0080;
const ST_EVERYONE: u32 = 0x0100;

#[inline]
fn affect_has(mask: AudioAffect, flag: AudioAffect) -> bool {
    mask.has(flag)
}

fn event_matches_audio_affect(event: &AudioEventRts, which: AudioAffect) -> bool {
    if affect_has(which, AudioAffect::All) {
        return true;
    }

    let event_affect = match event.get_audio_event_info().map(|info| info.sound_type) {
        Some(AudioType::Music) => AudioAffect::Music,
        Some(AudioType::Streaming) => AudioAffect::Speech,
        _ => {
            if event.is_positional_audio() {
                AudioAffect::Sound3D
            } else {
                AudioAffect::Sound
            }
        }
    };

    affect_has(which, event_affect)
}

/// Speaker types for audio configuration
pub static SPEAKER_TYPES: &[&str] = &[
    "2 Speakers",
    "Headphones",
    "Surround Sound",
    "4 Speaker",
    "5.1 Surround",
    "7.1 Surround",
];

/// Audio affect flags - what audio types to affect
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioAffect {
    Music = 0x01,
    Sound = 0x02,
    Sound3D = 0x04,
    SoundEffects = 0x06, // Sound | Sound3D
    Speech = 0x08,
    All = 0x0F,
    SystemSetting = 0x10,
    MusicSystemSetting = 0x11,
    SoundSystemSetting = 0x12,
    Sound3DSystemSetting = 0x14,
    SoundEffectsSystemSetting = 0x16,
    SpeechSystemSetting = 0x18,
    AllSystemSetting = 0x1F,
    Ambient = 0x20,
}

impl AudioAffect {
    pub const fn bits(self) -> u32 {
        self as u32
    }

    pub const fn has(self, flag: AudioAffect) -> bool {
        (self.bits() & flag.bits()) != 0
    }

    pub const fn from_bits(bits: u32) -> Option<Self> {
        match bits {
            0x01 => Some(Self::Music),
            0x02 => Some(Self::Sound),
            0x04 => Some(Self::Sound3D),
            0x06 => Some(Self::SoundEffects),
            0x08 => Some(Self::Speech),
            0x0F => Some(Self::All),
            0x10 => Some(Self::SystemSetting),
            0x11 => Some(Self::MusicSystemSetting),
            0x12 => Some(Self::SoundSystemSetting),
            0x14 => Some(Self::Sound3DSystemSetting),
            0x16 => Some(Self::SoundEffectsSystemSetting),
            0x18 => Some(Self::SpeechSystemSetting),
            0x1F => Some(Self::AllSystemSetting),
            0x20 => Some(Self::Ambient),
            _ => None,
        }
    }
}

/// Audio settings configuration
#[derive(Debug, Clone)]
pub struct AudioSettings {
    pub audio_root: AsciiString,
    pub sounds_folder: AsciiString,
    pub music_folder: AsciiString,
    pub streaming_folder: AsciiString,
    pub sounds_extension: AsciiString,

    pub use_digital: Bool,
    pub use_midi: Bool,
    pub output_rate: Int,
    pub output_bits: Int,
    pub output_channels: Int,
    pub sample_count_2d: Int,
    pub sample_count_3d: Int,
    pub stream_count: Int,

    pub preferred_3d_provider: [AsciiString; 5], // 4 HW + 1 SW
    pub default_speaker_type_2d: UnsignedInt,
    pub default_speaker_type_3d: UnsignedInt,

    pub min_volume: Real,
    pub global_min_range: Int,
    pub global_max_range: Int,
    pub drawable_ambient_frames: UnsignedInt,
    pub fade_audio_frames: UnsignedInt,
    pub max_cache_size: UnsignedInt,
    pub relative_2d_volume: Real,
    pub default_sound_volume: Real,
    pub default_3d_sound_volume: Real,
    pub default_speech_volume: Real,
    pub default_music_volume: Real,
    pub microphone_desired_height_above_terrain: Real,
    pub microphone_max_percentage_between_ground_and_camera: Real,
    pub zoom_min_distance: Real,
    pub zoom_max_distance: Real,
    pub zoom_sound_volume_percentage_amount: Real,

    // User preference volumes
    pub preferred_sound_volume: Real,
    pub preferred_3d_sound_volume: Real,
    pub preferred_speech_volume: Real,
    pub preferred_music_volume: Real,
}

impl Default for AudioSettings {
    fn default() -> Self {
        AudioSettings {
            audio_root: "Data\\Audio".to_string(),
            sounds_folder: "Sounds".to_string(),
            music_folder: "Music".to_string(),
            streaming_folder: "Speech".to_string(),
            sounds_extension: "wav".to_string(),

            use_digital: true,
            use_midi: false,
            output_rate: 44100,
            output_bits: 16,
            output_channels: 2,
            sample_count_2d: 16,
            sample_count_3d: 16,
            stream_count: 8,

            preferred_3d_provider: [
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ],
            default_speaker_type_2d: 0,
            default_speaker_type_3d: 0,

            min_volume: 0.01,
            global_min_range: 25,
            global_max_range: 1000,
            drawable_ambient_frames: 30,
            fade_audio_frames: 60,
            max_cache_size: 16 * 1024 * 1024, // 16 MB
            relative_2d_volume: 1.0,
            default_sound_volume: 0.75,
            default_3d_sound_volume: 0.75,
            default_speech_volume: 0.55,
            default_music_volume: 0.55,
            microphone_desired_height_above_terrain: 200.0,
            microphone_max_percentage_between_ground_and_camera: 0.5,
            zoom_min_distance: 200.0,
            zoom_max_distance: 2000.0,
            zoom_sound_volume_percentage_amount: 0.25,

            preferred_sound_volume: 0.75,
            preferred_3d_sound_volume: 0.75,
            preferred_speech_volume: 0.55,
            preferred_music_volume: 0.55,
        }
    }
}

/// Miscellaneous audio events
#[derive(Debug, Default)]
pub struct MiscAudio {
    // This would contain predefined audio events for UI sounds, etc.
    pub ui_sounds: HashMap<String, AudioEventRts>,
}

/// Forward declaration for managers
pub trait MusicManager: Send + Sync {
    fn add_audio_event(&mut self, event: AudioEventRts);
    fn remove_audio_event(&mut self, handle: AudioHandle);
}

pub trait SoundManager: Send + Sync {
    fn add_audio_event(&mut self, event: AudioEventRts) -> Result<(), String>;
    fn can_play_now(&self, event: &AudioEventRts) -> bool;
    fn post_process_load(&mut self) {}
    fn update(&mut self) {}
    fn reset(&mut self) {}
    fn set_listener_position(&mut self, _position: &Coord3D) {}
    fn configure_sample_capacity(&mut self, _samples_2d: UnsignedInt, _samples_3d: UnsignedInt) {}
    fn notify_of_2d_sample_start(&mut self);
    fn notify_of_3d_sample_start(&mut self);
    fn notify_of_2d_sample_completion(&mut self);
    fn notify_of_3d_sample_completion(&mut self);
    fn get_available_samples(&self) -> Int;
    fn get_available_3d_samples(&self) -> Int;
    fn stop_all_sounds(&mut self) {
        // Default: no-op. Concrete implementations should clear their playing sound lists.
    }
    fn cleanup_completed_sounds(&mut self) {
        // Default: no-op. Concrete implementations should prune finished sounds.
    }
}

/// The main audio manager - the life of audio
///
/// When audio is requested to play, it is done so in the following manner:
/// 1) An AudioEventRts is created on the stack.
/// 2) Its guts are copied from elsewhere (for instance, a ThingTemplate, or MiscAudio).
/// 3) It is added to TheAudio via TheAudio.add_audio_event(...)
///
/// The return value from add_audio_event can be saved in case the sound needs to loop and/or be
/// terminated at some point.
pub struct AudioManager {
    // Settings and configuration
    audio_settings: AudioSettings,
    misc_audio: MiscAudio,

    // Managers
    music_manager: Option<Box<dyn MusicManager + Send + Sync>>,
    sound_manager: Option<Box<dyn SoundManager + Send + Sync>>,

    // State
    listener_position: Coord3D,
    listener_orientation: Coord3D,
    audio_requests: Vec<AudioRequest>,
    active_audio_events: HashMap<AudioHandle, AudioEventRts>,
    music_tracks: Vec<AsciiString>,
    current_music_track: AsciiString,

    // Audio event registry
    all_audio_event_info: HashMap<AsciiString, Arc<AudioEventInfo>>,
    audio_handle_pool: AudioHandle,
    adjusted_volumes: Vec<(AsciiString, Real)>,

    // Volume controls
    music_volume: Real,
    sound_volume: Real,
    sound_3d_volume: Real,
    speech_volume: Real,

    script_music_volume: Real,
    script_sound_volume: Real,
    script_sound_3d_volume: Real,
    script_speech_volume: Real,

    system_music_volume: Real,
    system_sound_volume: Real,
    system_sound_3d_volume: Real,
    system_speech_volume: Real,
    zoom_volume: Real,

    silent_audio_event: AudioEventRts,
    saved_values: Option<[Real; NUM_VOLUME_TYPES]>,

    // State flags
    speech_on: Bool,
    sound_on: Bool,
    sound_3d_on: Bool,
    music_on: Bool,
    volume_has_changed: Bool,
    hardware_accel: Bool,
    surround_speakers: Bool,
    music_playing_from_cd: Bool,
    disallow_speech: Bool,
}

impl AudioManager {
    pub fn new() -> Self {
        AudioManager {
            audio_settings: AudioSettings::default(),
            misc_audio: MiscAudio::default(),
            music_manager: Some(create_music_manager()),
            sound_manager: Some(create_sound_manager()),
            listener_position: Coord3D::new(),
            listener_orientation: Coord3D {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            audio_requests: Vec::new(),
            active_audio_events: HashMap::new(),
            music_tracks: Vec::new(),
            current_music_track: String::new(),
            all_audio_event_info: HashMap::new(),
            audio_handle_pool: 1000, // Start at some reasonable value
            adjusted_volumes: Vec::new(),

            music_volume: 0.0,
            sound_volume: 0.0,
            sound_3d_volume: 0.0,
            speech_volume: 0.0,

            script_music_volume: 1.0,
            script_sound_volume: 1.0,
            script_sound_3d_volume: 1.0,
            script_speech_volume: 1.0,

            system_music_volume: 0.55,
            system_sound_volume: 0.75,
            system_sound_3d_volume: 0.75,
            system_speech_volume: 0.55,
            zoom_volume: 1.0,

            silent_audio_event: AudioEventRts::new(),
            saved_values: None,

            speech_on: true,
            sound_on: true,
            sound_3d_on: true,
            music_on: true,
            volume_has_changed: false,
            hardware_accel: false,
            surround_speakers: false,
            music_playing_from_cd: false,
            disallow_speech: false,
        }
    }

    pub fn init(&mut self) {
        // Initialize volumes from settings
        self.system_music_volume = self.audio_settings.preferred_music_volume;
        self.system_sound_volume = self.audio_settings.preferred_sound_volume;
        self.system_sound_3d_volume = self.audio_settings.preferred_3d_sound_volume;
        self.system_speech_volume = self.audio_settings.preferred_speech_volume;

        self.music_volume = self.system_music_volume;
        self.sound_volume = self.system_sound_volume;
        self.sound_3d_volume = self.system_sound_3d_volume;
        self.speech_volume = self.system_speech_volume;

        if let Some(sound_mgr) = &mut self.sound_manager {
            let samples_2d = self.audio_settings.sample_count_2d.max(1) as UnsignedInt;
            let samples_3d = self.audio_settings.sample_count_3d.max(1) as UnsignedInt;
            sound_mgr.configure_sample_capacity(samples_2d, samples_3d);
        }
    }

    pub fn post_process_load(&mut self) {
        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.post_process_load();
        }
    }

    pub fn reset(&mut self) {
        // Stop all actively playing sounds through the backend before clearing bookkeeping.
        let handles: Vec<AudioHandle> = self.active_audio_events.keys().copied().collect();
        for handle in handles {
            let _ = with_sound_playback_hook(|hook| hook.stop(handle));
        }

        // Clear out any adjusted volumes
        self.adjusted_volumes.clear();
        self.active_audio_events.clear();
        self.audio_requests.clear();
        self.current_music_track.clear();

        // Reset scripted volumes (C++ resets to 1.0)
        self.script_music_volume = 1.0;
        self.script_sound_volume = 1.0;
        self.script_sound_3d_volume = 1.0;
        self.script_speech_volume = 1.0;

        // Restore the final values to system defaults
        self.music_volume = self.system_music_volume;
        self.sound_volume = self.system_sound_volume;
        self.sound_3d_volume = self.system_sound_3d_volume;
        self.speech_volume = self.system_speech_volume;

        self.disallow_speech = false;
        self.volume_has_changed = true;

        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.reset();
        }
    }

    pub fn update(&mut self) {
        self.process_request_list();
        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.update();
        }
        self.purge_inactive_events();

        if let Some(resolver) = AUDIO_VIEW_RESOLVER.get() {
            let ground_pos = resolver.get_tactical_view_position();
            let angle = resolver.get_tactical_view_angle();
            let camera_pos = resolver.get_3d_camera_position();
            let ground_height = resolver.get_ground_height(ground_pos.x, ground_pos.y);

            let forward_x = -angle.sin();
            let forward_y = angle.cos();
            let forward_z = 0.0;

            let look_to = Coord3D {
                x: forward_x,
                y: forward_y,
                z: forward_z,
            };

            let desired_height = self.audio_settings.microphone_desired_height_above_terrain;
            let max_percentage = self
                .audio_settings
                .microphone_max_percentage_between_ground_and_camera;

            let mut ground_to_camera = Coord3D {
                x: camera_pos.x - ground_pos.x,
                y: camera_pos.y - ground_pos.y,
                z: camera_pos.z - ground_pos.z,
            };

            let best_scale_factor = if camera_pos.z <= desired_height || ground_to_camera.z <= 0.0 {
                max_percentage
            } else {
                let z_scale = desired_height / ground_to_camera.z;
                max_percentage.min(z_scale)
            };

            ground_to_camera.x *= best_scale_factor;
            ground_to_camera.y *= best_scale_factor;
            ground_to_camera.z *= best_scale_factor;

            let mut microphone_pos = Coord3D {
                x: ground_pos.x,
                y: ground_pos.y,
                z: ground_height,
            };
            microphone_pos.x += ground_to_camera.x;
            microphone_pos.y += ground_to_camera.y;
            microphone_pos.z += ground_to_camera.z;

            self.set_listener_position(&microphone_pos, &look_to);

            let max_boost_scalar = self.audio_settings.zoom_sound_volume_percentage_amount;
            let min_dist = self.audio_settings.zoom_min_distance;
            let max_dist = self.audio_settings.zoom_max_distance;

            self.zoom_volume = 1.0 - max_boost_scalar;

            if max_boost_scalar > 0.0 {
                let dx = camera_pos.x - microphone_pos.x;
                let dy = camera_pos.y - microphone_pos.y;
                let dz = camera_pos.z - microphone_pos.z;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                if dist < min_dist {
                    self.zoom_volume = 1.0;
                } else if dist < max_dist {
                    let scalar = (dist - min_dist) / (max_dist - min_dist);
                    self.zoom_volume = 1.0 - scalar * max_boost_scalar;
                }
            }

            self.set_3d_volume_adjustment(self.zoom_volume);
        }
    }

    /// Add an audio event to be played
    pub fn add_audio_event(&mut self, event_to_add: &AudioEventRts) -> AudioHandle {
        if event_to_add.get_event_name().is_empty() || event_to_add.get_event_name() == "NoSound" {
            return AHSV_NO_SOUND;
        }

        let mut audio_event = event_to_add.clone();
        if audio_event.get_audio_event_info().is_none() {
            if let Some(info) = self.find_audio_event_info(event_to_add.get_event_name()) {
                audio_event.set_audio_event_info(info);
            }
        }

        let Some(resolved_info) = audio_event.get_audio_event_info() else {
            return AHSV_ERROR;
        };
        let sound_type = resolved_info.sound_type;

        // Check if audio type is enabled
        match sound_type {
            AudioType::Music => {
                if !self.is_on(AudioAffect::Music) {
                    return AHSV_NO_SOUND;
                }
            }
            AudioType::SoundEffect => {
                if !self.is_on(AudioAffect::Sound) || !self.is_on(AudioAffect::Sound3D) {
                    return AHSV_NO_SOUND;
                }
            }
            AudioType::Streaming => {
                if !self.is_on(AudioAffect::Speech) {
                    return AHSV_NO_SOUND;
                }
            }
        }

        // Check if we're disallowing speech
        if self.disallow_speech && sound_type == AudioType::Streaming {
            return AHSV_NO_SOUND;
        }

        let handle = self.allocate_new_handle();
        audio_event.set_playing_handle(handle);
        audio_event.generate_filename();
        audio_event.generate_play_info();

        // Check volume adjustments
        for (name, volume) in &self.adjusted_volumes {
            if *name == audio_event.get_event_name() {
                audio_event.set_volume(*volume);
                break;
            }
        }

        if !audio_event.get_uninterruptable() && !self.should_play_locally(&audio_event) {
            return AHSV_NOT_FOR_LOCAL;
        }

        // Check if volume is too low
        if audio_event.get_volume() < self.audio_settings.min_volume {
            return AHSV_MUTED;
        }

        // Route to appropriate manager
        match sound_type {
            AudioType::Music => {
                if let Some(music_mgr) = &mut self.music_manager {
                    music_mgr.add_audio_event(audio_event.clone());
                    self.track_active_event(&audio_event);
                    handle
                } else {
                    AHSV_NO_SOUND
                }
            }
            _ => {
                if let Some(sound_mgr) = &mut self.sound_manager {
                    if sound_mgr.add_audio_event(audio_event.clone()).is_ok() {
                        self.track_active_event(&audio_event);
                        handle
                    } else {
                        AHSV_NO_SOUND
                    }
                } else {
                    AHSV_NO_SOUND
                }
            }
        }
    }

    pub fn get_audio_event_info(&self, event_name: &str) -> Option<Arc<AudioEventInfo>> {
        self.find_audio_event_info(event_name)
    }

    pub fn remove_audio_event(&mut self, audio_event: AudioHandle) {
        if audio_event == AHSV_STOP_THE_MUSIC || audio_event == AHSV_STOP_THE_MUSIC_FADE {
            if let Some(music_mgr) = &mut self.music_manager {
                music_mgr.remove_audio_event(audio_event);
            }
            return;
        }

        if audio_event == AHSV_ERROR
            || audio_event == AHSV_NOT_FOR_LOCAL
            || audio_event == AHSV_MUTED
        {
            return;
        }

        if audio_event < AHSV_FIRST_HANDLE {
            return;
        }

        self.active_audio_events.remove(&audio_event);
        let request = AudioRequest::new_with_handle(RequestType::Stop, audio_event);
        self.append_audio_request(request);
    }

    pub fn is_currently_playing(&self, audio_event: AudioHandle) -> bool {
        if audio_event < 1000 {
            return false;
        }

        with_sound_playback_hook(|hook| hook.is_playing(audio_event)).unwrap_or(false)
    }

    pub fn get_audio_length_ms(&self, event: &AudioEventRts) -> Real {
        // C++ parity: clone event, resolve concrete filenames, then sum attack/main/decay lengths.
        let mut tmp_event = event.clone();
        if tmp_event.get_audio_event_info().is_none() {
            if let Some(info) = self.find_audio_event_info(event.get_event_name()) {
                tmp_event.set_audio_event_info(info);
            } else {
                return 0.0;
            }
        }

        tmp_event.generate_filename();
        tmp_event.generate_play_info();

        self.get_file_length_ms(tmp_event.get_attack_filename())
            + self.get_file_length_ms(tmp_event.get_filename())
            + self.get_file_length_ms(tmp_event.get_decay_filename())
    }

    fn get_file_length_ms(&self, file_path: &str) -> Real {
        let normalized = file_path.trim();
        if normalized.is_empty() {
            return 0.0;
        }

        static FILE_LENGTH_CACHE: OnceLock<Mutex<HashMap<AsciiString, Real>>> = OnceLock::new();
        let cache = FILE_LENGTH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

        if let Ok(guard) = cache.lock() {
            if let Some(length) = guard.get(normalized) {
                return *length;
            }
        }

        let length = Self::read_audio_file_bytes(normalized)
            .and_then(|bytes| Self::duration_ms_from_audio_data(&bytes))
            .unwrap_or(0.0);

        if let Ok(mut guard) = cache.lock() {
            guard.insert(normalized.to_string(), length);
        }

        length
    }

    fn read_audio_file_bytes(file_path: &str) -> Option<Vec<u8>> {
        let mut candidates: Vec<String> = Vec::new();
        let trimmed = file_path.trim();
        if trimmed.is_empty() {
            return None;
        }

        candidates.push(trimmed.to_string());
        let slash_variant = trimmed.replace('\\', "/");
        if slash_variant != trimmed {
            candidates.push(slash_variant.clone());
        }

        if std::path::Path::new(trimmed).extension().is_none() {
            for ext in [".wav", ".mp3", ".ogg"] {
                candidates.push(format!("{trimmed}{ext}"));
                if slash_variant != trimmed {
                    candidates.push(format!("{slash_variant}{ext}"));
                }
            }
        }

        candidates.sort();
        candidates.dedup();

        for candidate in candidates {
            if let Some(data) = Self::read_from_virtual_file_system(&candidate) {
                return Some(data);
            }

            if let Ok(data) = std::fs::read(&candidate) {
                return Some(data);
            }
        }

        None
    }

    fn read_from_virtual_file_system(path: &str) -> Option<Vec<u8>> {
        let file_system = get_file_system();
        let Ok(mut guard) = file_system.lock() else {
            return None;
        };
        let mut file = guard.open_file(path, FileAccess::READ.combine(FileAccess::BINARY))?;
        file.read_entire_and_close().ok()
    }

    fn duration_ms_from_audio_data(data: &[u8]) -> Option<Real> {
        Self::duration_ms_from_wav(data)
            .or_else(|| Self::duration_ms_from_mp3(data))
            .or_else(|| Self::duration_ms_from_ogg(data))
    }

    fn duration_ms_from_wav(data: &[u8]) -> Option<Real> {
        let reader = WavReader::new(Cursor::new(data)).ok()?;
        let spec = reader.spec();
        if spec.sample_rate == 0 {
            return None;
        }

        // hound duration is interleaved-sample count; divide by channel count to get frame count.
        let channels = spec.channels.max(1) as f64;
        let samples = reader.duration() as f64;
        let frames = samples / channels;
        Some((frames * 1000.0 / spec.sample_rate as f64) as Real)
    }

    fn duration_ms_from_mp3(data: &[u8]) -> Option<Real> {
        let mut decoder = Mp3Decoder::new(Cursor::new(data));
        let mut total_ms = 0.0f64;

        loop {
            match decoder.next_frame() {
                Ok(frame) => {
                    if frame.sample_rate <= 0 {
                        continue;
                    }
                    let channels = frame.channels.max(1) as f64;
                    let samples = frame.data.len() as f64;
                    let frames = samples / channels;
                    total_ms += frames * 1000.0 / frame.sample_rate as f64;
                }
                Err(Mp3Error::Eof) => break,
                Err(Mp3Error::SkippedData) => continue,
                Err(_) => return None,
            }
        }

        (total_ms > 0.0).then_some(total_ms as Real)
    }

    fn duration_ms_from_ogg(data: &[u8]) -> Option<Real> {
        let mut reader = OggStreamReader::new(Cursor::new(data)).ok()?;
        let sample_rate = reader.ident_hdr.audio_sample_rate;
        if sample_rate == 0 {
            return None;
        }

        let channels = reader.ident_hdr.audio_channels.max(1) as f64;
        let mut total_frames = 0.0f64;
        loop {
            match reader.read_dec_packet_itl() {
                Ok(Some(packet)) => {
                    total_frames += packet.len() as f64 / channels;
                }
                Ok(None) => break,
                Err(_) => return None,
            }
        }

        (total_frames > 0.0).then_some((total_frames * 1000.0 / sample_rate as f64) as Real)
    }

    pub fn is_valid_audio_event(&self, event_to_check: &AudioEventRts) -> bool {
        if event_to_check.get_event_name().is_empty() {
            return false;
        }

        event_to_check.get_audio_event_info().is_some()
            || self
                .find_audio_event_info(event_to_check.get_event_name())
                .is_some()
    }

    pub fn add_track_name(&mut self, track_name: String) {
        self.music_tracks.push(track_name);
    }

    pub fn set_music_track_name(&mut self, track_name: String) {
        self.current_music_track = track_name;
    }

    pub fn get_music_track_name(&self) -> &str {
        &self.current_music_track
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

    pub fn next_music_track(&mut self) -> String {
        let next_track = self.next_track_name(&self.current_music_track);
        self.current_music_track = next_track.clone();
        next_track
    }

    pub fn prev_music_track(&mut self) -> String {
        let prev_track = self.prev_track_name(&self.current_music_track);
        self.current_music_track = prev_track.clone();
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

    pub fn set_audio_event_enabled(&mut self, event_to_affect: String, enable: Bool) {
        let volume = if enable { -1.0 } else { 0.0 };
        self.set_audio_event_volume_override(event_to_affect, volume);
    }

    pub fn set_audio_event_volume_override(&mut self, event_to_affect: String, new_volume: Real) {
        if event_to_affect.is_empty() {
            self.adjusted_volumes.clear();
            return;
        }

        // C++ parity: live playing sounds are adjusted when setting an explicit override.
        if new_volume != -1.0 {
            self.adjust_volume_of_playing_audio(&event_to_affect, new_volume);
        }

        // Find existing adjustment
        for (name, volume) in &mut self.adjusted_volumes {
            if *name == event_to_affect {
                if new_volume == -1.0 {
                    // Remove the adjustment - we'll handle this after the loop
                } else {
                    *volume = new_volume;
                    return;
                }
            }
        }

        // Remove adjustment if new_volume is -1.0
        if new_volume == -1.0 {
            self.adjusted_volumes
                .retain(|(name, _)| *name != event_to_affect);
        } else {
            // Add new adjustment
            self.adjusted_volumes.push((event_to_affect, new_volume));
        }
    }

    pub fn is_on(&self, which_to_get: AudioAffect) -> Bool {
        if affect_has(which_to_get, AudioAffect::Music) {
            self.music_on
        } else if affect_has(which_to_get, AudioAffect::Sound) {
            self.sound_on
        } else if affect_has(which_to_get, AudioAffect::Sound3D) {
            self.sound_3d_on
        } else {
            self.speech_on
        }
    }

    pub fn set_on(&mut self, turn_on: Bool, which_to_affect: AudioAffect) {
        if affect_has(which_to_affect, AudioAffect::Music) {
            self.music_on = turn_on;
        }
        if affect_has(which_to_affect, AudioAffect::Sound) {
            self.sound_on = turn_on;
        }
        if affect_has(which_to_affect, AudioAffect::Sound3D) {
            self.sound_3d_on = turn_on;
        }
        if affect_has(which_to_affect, AudioAffect::Speech) {
            self.speech_on = turn_on;
        }
    }

    pub fn set_volume(&mut self, volume: Real, which_to_affect: AudioAffect) {
        let system_setting = affect_has(which_to_affect, AudioAffect::SystemSetting);

        if affect_has(which_to_affect, AudioAffect::Music) {
            if system_setting {
                self.system_music_volume = volume;
            } else {
                self.script_music_volume = volume;
            }
            self.music_volume = self.script_music_volume * self.system_music_volume;
        }

        if affect_has(which_to_affect, AudioAffect::Sound) {
            if system_setting {
                self.system_sound_volume = volume;
            } else {
                self.script_sound_volume = volume;
            }
            self.sound_volume = self.script_sound_volume * self.system_sound_volume;
        }

        if affect_has(which_to_affect, AudioAffect::Sound3D) {
            if system_setting {
                self.system_sound_3d_volume = volume;
            } else {
                self.script_sound_3d_volume = volume;
            }
            self.sound_3d_volume = self.script_sound_3d_volume * self.system_sound_3d_volume;
        }

        if affect_has(which_to_affect, AudioAffect::Speech) {
            if system_setting {
                self.system_speech_volume = volume;
            } else {
                self.script_speech_volume = volume;
            }
            self.speech_volume = self.script_speech_volume * self.system_speech_volume;
        }

        self.volume_has_changed = true;
    }

    pub fn get_volume(&self, which_to_get: AudioAffect) -> Real {
        if affect_has(which_to_get, AudioAffect::Music) {
            self.music_volume
        } else if affect_has(which_to_get, AudioAffect::Sound) {
            self.sound_volume
        } else if affect_has(which_to_get, AudioAffect::Sound3D) {
            self.sound_3d_volume
        } else {
            self.speech_volume
        }
    }

    pub fn set_3d_volume_adjustment(&mut self, volume_adjustment: Real) {
        self.sound_3d_volume =
            volume_adjustment * self.script_sound_3d_volume * self.system_sound_3d_volume;

        // Clamp
        self.sound_3d_volume = self.sound_3d_volume.clamp(0.0, 1.0);

        if !self.has_3d_sensitive_streams_playing() {
            self.volume_has_changed = true;
        }
    }

    pub fn set_listener_position(
        &mut self,
        new_listener_pos: &Coord3D,
        new_listener_orientation: &Coord3D,
    ) {
        self.listener_position = *new_listener_pos;
        self.listener_orientation = *new_listener_orientation;
        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.set_listener_position(new_listener_pos);
        }
        let _ = with_sound_playback_hook(|hook| hook.set_listener_position(new_listener_pos));
    }

    pub fn get_listener_position(&self) -> &Coord3D {
        &self.listener_position
    }

    pub fn allocate_audio_request(&self, use_audio_event: Bool) -> AudioRequest {
        let mut request = AudioRequest::default();
        request.use_pending_event = use_audio_event;
        request.requires_check_for_sample = false;
        request
    }

    pub fn append_audio_request(&mut self, request: AudioRequest) {
        self.audio_requests.push(request);
    }

    pub fn process_request_list(&mut self) {
        let pending = std::mem::take(&mut self.audio_requests);
        for request in pending {
            match request.request {
                RequestType::Stop => {
                    if let Some(handle) = request.get_handle() {
                        self.active_audio_events.remove(&handle);
                        let _ = with_sound_playback_hook(|hook| {
                            hook.stop(handle);
                        });
                    }
                }
                RequestType::Play => {
                    if let Some(event) = request.get_pending_event() {
                        if let Some(sound_mgr) = &mut self.sound_manager {
                            if sound_mgr.add_audio_event(event.clone()).is_ok() {
                                self.track_active_event(event);
                            }
                        }
                    }
                }
                RequestType::Pause => {
                    if let Some(handle) = request.get_handle() {
                        let _ = with_sound_playback_hook(|hook| {
                            hook.pause(handle);
                        });
                    }
                }
            }
        }
    }

    pub fn new_audio_event_info(&mut self, audio_name: String) -> Option<Arc<AudioEventInfo>> {
        if self.all_audio_event_info.contains_key(&audio_name) {
            // Already exists
            return self.all_audio_event_info.get(&audio_name).cloned();
        }

        let event_info = Arc::new(AudioEventInfo {
            sound_type: AudioType::SoundEffect,
            control: 0,
            audio_name: audio_name.clone(),
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
        });

        self.all_audio_event_info
            .insert(audio_name, event_info.clone());
        Some(event_info)
    }

    pub fn find_audio_event_info(&self, event_name: &str) -> Option<Arc<AudioEventInfo>> {
        self.all_audio_event_info.get(event_name).cloned()
    }

    pub fn register_audio_event_info(&mut self, info: AudioEventInfo) {
        self.all_audio_event_info
            .insert(info.audio_name.clone(), Arc::new(info));
    }

    pub fn get_audio_settings(&self) -> &AudioSettings {
        &self.audio_settings
    }

    pub fn get_misc_audio(&self) -> &MiscAudio {
        &self.misc_audio
    }

    pub fn get_disallow_speech(&self) -> Bool {
        self.disallow_speech
    }

    pub fn set_disallow_speech(&mut self, disallow_speech: Bool) {
        self.disallow_speech = disallow_speech;
    }

    pub fn get_zoom_volume(&self) -> Real {
        self.zoom_volume
    }

    pub fn allocate_new_handle(&mut self) -> AudioHandle {
        let handle = self.audio_handle_pool;
        self.audio_handle_pool += 1;
        handle
    }

    pub fn lose_focus(&mut self) {
        // Save current volumes and set to 0
        self.saved_values = Some([
            self.system_music_volume,
            self.system_sound_volume,
            self.system_sound_3d_volume,
            self.system_speech_volume,
        ]);

        self.system_music_volume = 0.0;
        self.system_sound_volume = 0.0;
        self.system_sound_3d_volume = 0.0;
        self.system_speech_volume = 0.0;
        self.music_volume = self.script_music_volume * self.system_music_volume;
        self.sound_volume = self.script_sound_volume * self.system_sound_volume;
        self.sound_3d_volume = self.script_sound_3d_volume * self.system_sound_3d_volume;
        self.speech_volume = self.script_speech_volume * self.system_speech_volume;
        self.volume_has_changed = true;
    }

    pub fn regain_focus(&mut self) {
        if let Some(saved) = self.saved_values.take() {
            self.system_music_volume = saved[0];
            self.system_sound_volume = saved[1];
            self.system_sound_3d_volume = saved[2];
            self.system_speech_volume = saved[3];
            self.music_volume = self.script_music_volume * self.system_music_volume;
            self.sound_volume = self.script_sound_volume * self.system_sound_volume;
            self.sound_3d_volume = self.script_sound_3d_volume * self.system_sound_3d_volume;
            self.speech_volume = self.script_speech_volume * self.system_speech_volume;
            self.volume_has_changed = true;
        }
    }

    pub fn pause_audio(&mut self, which: AudioAffect) {
        let handles: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter(|event| event_matches_audio_affect(event, which))
            .map(|event| event.get_playing_handle())
            .collect();

        let _ = with_sound_playback_hook(|hook| {
            for handle in handles {
                hook.pause(handle);
            }
        });
    }

    pub fn resume_audio(&mut self, which: AudioAffect) {
        let handles: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter(|event| event_matches_audio_affect(event, which))
            .map(|event| event.get_playing_handle())
            .collect();

        if with_sound_playback_hook(|hook| {
            for handle in handles {
                hook.resume(handle);
            }
        })
        .is_some()
        {
            return;
        }

        if affect_has(which, AudioAffect::SoundEffects) || affect_has(which, AudioAffect::All) {
            if let Some(sound_mgr) = &mut self.sound_manager {
                sound_mgr.reset();
                sound_mgr.update();
            }
        }
    }

    pub fn translate_speaker_type_to_unsigned_int(&self, speaker_type: &str) -> UnsignedInt {
        for (i, &speaker) in SPEAKER_TYPES.iter().enumerate() {
            if speaker == speaker_type {
                return i as UnsignedInt;
            }
        }
        0
    }

    pub fn translate_unsigned_int_to_speaker_type(
        &self,
        speaker_type: UnsignedInt,
    ) -> &'static str {
        let index = speaker_type as usize;
        if index < SPEAKER_TYPES.len() {
            SPEAKER_TYPES[index]
        } else {
            SPEAKER_TYPES[0]
        }
    }

    fn should_play_locally(&self, audio_event: &AudioEventRts) -> Bool {
        let Some(event_info) = audio_event.get_audio_event_info() else {
            return false;
        };

        if event_info.sound_type == AudioType::Music {
            return true;
        }

        let player_restriction_mask = ST_PLAYER | ST_ALLIES | ST_ENEMIES | ST_EVERYONE;
        if (event_info.type_field & player_restriction_mask) == 0 {
            return true;
        }

        if (event_info.type_field & ST_EVERYONE) != 0 {
            return true;
        }

        // Preserve previous behavior until game logic resolver is registered.
        with_audio_locality_resolver(|resolver| {
            self.should_play_locally_with_resolver(audio_event, &event_info, resolver)
        })
        .unwrap_or(true)
    }

    fn should_play_locally_with_resolver(
        &self,
        audio_event: &AudioEventRts,
        event_info: &AudioEventInfo,
        resolver: &dyn AudioLocalityResolver,
    ) -> Bool {
        let owning_player_index = audio_event.get_player_index();
        let owning_player_exists = resolver.player_exists(owning_player_index);

        if (event_info.type_field & ST_PLAYER) != 0
            && (event_info.type_field & ST_UI) != 0
            && !owning_player_exists
        {
            return true;
        }

        if !owning_player_exists {
            return false;
        }

        let mut local_player_index = match resolver.get_local_player_index() {
            Some(index) => index,
            None => return false,
        };

        if !resolver.is_player_active(local_player_index) {
            local_player_index = match resolver.get_observer_look_at_player_index() {
                Some(index) => index,
                None => return false,
            };
        }

        if !resolver.player_exists(local_player_index)
            || !resolver.has_default_team(local_player_index)
        {
            return false;
        }

        if (event_info.type_field & ST_PLAYER) != 0 {
            return owning_player_index == local_player_index;
        }

        if (event_info.type_field & ST_ALLIES) != 0 {
            return owning_player_index != local_player_index
                && resolver
                    .get_relationship_to_local_team(owning_player_index, local_player_index)
                    == AudioLocalityRelationship::Allies;
        }

        if (event_info.type_field & ST_ENEMIES) != 0 {
            return resolver
                .get_relationship_to_local_team(owning_player_index, local_player_index)
                == AudioLocalityRelationship::Enemies;
        }

        false
    }

    // C++ parity methods used by SoundManager for audio culling

    /// Check if playing this event would violate its per-event limit.
    /// Returns true if the event has a positive limit and that many instances
    /// are already playing.
    pub fn does_violate_limit(&self, event: &AudioEventRts) -> Bool {
        let Some(event_info) = event.get_audio_event_info() else {
            return false;
        };

        // Negative or zero limit means "no limit" in C++ data.
        if event_info.limit <= 0 {
            return false;
        }

        // Count how many instances of this event name are already playing.
        let event_name = event.get_event_name();
        let playing_count = with_sound_playback_hook(|hook| {
            self.active_audio_events
                .values()
                .filter(|e| {
                    e.get_event_name() == event_name && hook.is_playing(e.get_playing_handle())
                })
                .count() as Int
        })
        .unwrap_or_else(|| {
            self.active_audio_events
                .values()
                .filter(|e| e.get_event_name() == event_name)
                .count() as Int
        });

        playing_count >= event_info.limit
    }

    /// Check if there are any sounds playing with lower priority than the given event.
    /// Returns true if we can kill a lower-priority sound to make room.
    pub fn is_playing_lower_priority(&self, event: &AudioEventRts) -> Bool {
        let event_priority = event.get_audio_priority();

        with_sound_playback_hook(|hook| {
            self.active_audio_events
                .values()
                .filter_map(|e| {
                    if hook.is_playing(e.get_playing_handle()) {
                        Some(e.get_audio_priority())
                    } else {
                        None
                    }
                })
                .any(|priority| priority < event_priority)
        })
        .unwrap_or_else(|| {
            self.active_audio_events
                .values()
                .any(|e| e.get_audio_priority() < event_priority)
        })
    }

    /// Check if a sound with the same event name is already playing.
    /// Used for interrupting sounds of the same type.
    pub fn is_playing_already(&self, event: &AudioEventRts) -> Bool {
        let event_name = event.get_event_name();

        with_sound_playback_hook(|hook| {
            self.active_audio_events
                .values()
                .filter_map(|e| {
                    if e.get_event_name() == event_name && hook.is_playing(e.get_playing_handle()) {
                        Some(())
                    } else {
                        None
                    }
                })
                .next()
                .is_some()
        })
        .unwrap_or_else(|| {
            self.active_audio_events
                .values()
                .any(|e| e.get_event_name() == event_name)
        })
    }

    /// Check if a specific object is currently playing a voice sound.
    /// Used to prevent multiple voice sounds from the same object.
    pub fn is_object_playing_voice(&self, object_id: ObjectId) -> Bool {
        const ST_VOICE: u32 = 0x00000010;

        with_sound_playback_hook(|hook| {
            self.active_audio_events
                .values()
                .filter_map(|e| {
                    let is_voice = e
                        .get_audio_event_info()
                        .map(|info| (info.type_field & ST_VOICE) != 0)
                        .unwrap_or(false);
                    if is_voice
                        && e.get_object_id() == object_id
                        && hook.is_playing(e.get_playing_handle())
                    {
                        Some(())
                    } else {
                        None
                    }
                })
                .next()
                .is_some()
        })
        .unwrap_or_else(|| {
            self.active_audio_events.values().any(|e| {
                let is_voice = e
                    .get_audio_event_info()
                    .map(|info| (info.type_field & ST_VOICE) != 0)
                    .unwrap_or(false);
                is_voice && e.get_object_id() == object_id
            })
        })
    }

    /// Remove all audio requests from the queue
    pub fn remove_all_audio_requests(&mut self) {
        self.audio_requests.clear();
    }

    /// Get the number of 2D samples configured
    pub fn get_num_2d_samples(&self) -> Int {
        self.audio_settings.sample_count_2d
    }

    /// Get the number of 3D samples configured
    pub fn get_num_3d_samples(&self) -> Int {
        self.audio_settings.sample_count_3d
    }

    /// Adjust the volume of currently playing audio events matching the given name
    pub fn adjust_volume_of_playing_audio(&mut self, event_name: &str, new_volume: Real) {
        for event in self.active_audio_events.values_mut() {
            if event.get_event_name() == event_name {
                event.set_volume(new_volume);
                let _ = with_sound_playback_hook(|hook| hook.set_event_volume(event));
            }
        }
    }

    /// Remove all playing audio events matching the given name
    pub fn remove_playing_audio(&mut self, event_name: &str) {
        let handles_to_stop: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter_map(|e| {
                if e.get_event_name() == event_name {
                    Some(e.get_playing_handle())
                } else {
                    None
                }
            })
            .collect();

        for handle in handles_to_stop {
            self.remove_audio_event(handle);
        }
    }

    /// Remove all disabled audio events (volume = 0)
    pub fn remove_all_disabled_audio(&mut self) {
        let handles_to_stop: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter_map(|e| {
                if e.get_volume() <= 0.0 {
                    Some(e.get_playing_handle())
                } else {
                    None
                }
            })
            .collect();

        for handle in handles_to_stop {
            self.remove_audio_event(handle);
        }
    }

    /// Check if there are any 3D-sensitive streams currently playing
    pub fn has_3d_sensitive_streams_playing(&self) -> Bool {
        with_sound_playback_hook(|hook| {
            self.active_audio_events
                .values()
                .filter_map(|e| {
                    if e.is_positional_audio() && hook.is_playing(e.get_playing_handle()) {
                        Some(())
                    } else {
                        None
                    }
                })
                .next()
                .is_some()
        })
        .unwrap_or_else(|| {
            self.active_audio_events
                .values()
                .any(AudioEventRts::is_positional_audio)
        })
    }

    fn track_active_event(&mut self, event: &AudioEventRts) {
        let handle = event.get_playing_handle();
        if handle >= AHSV_FIRST_HANDLE {
            self.active_audio_events.insert(handle, event.clone());
        }
    }

    fn purge_inactive_events(&mut self) {
        let _ = with_sound_playback_hook(|hook| {
            self.active_audio_events
                .retain(|handle, _| hook.is_playing(*handle));
        });
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

fn get_rodio_stream_handle() -> Option<OutputStreamHandle> {
    thread_local! {
        static STATE: RefCell<Option<(OutputStream, OutputStreamHandle)>> = const { RefCell::new(None) };
    }

    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if state.is_none() {
            if let Ok((stream, handle)) = OutputStream::try_default() {
                *state = Some((stream, handle));
            }
        }

        state.as_ref().map(|(_, handle)| handle.clone())
    })
}

struct RodioPlaybackHook {
    sinks: Mutex<HashMap<AudioHandle, RodioSinkState>>,
    listener_position: Mutex<Coord3D>,
}

struct RodioSinkState {
    sink: Arc<Mutex<Sink>>,
    base_volume: Real,
    position: Option<Coord3D>,
}

impl RodioPlaybackHook {
    fn new() -> Self {
        let _ = get_rodio_stream_handle();
        Self {
            sinks: Mutex::new(HashMap::new()),
            listener_position: Mutex::new(Coord3D::new()),
        }
    }

    fn build_path_candidates(filename: &str) -> Vec<String> {
        let trimmed = filename.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let normalized = trimmed.replace('\\', "/");
        let mut candidates = vec![trimmed.to_string()];
        if normalized != trimmed {
            candidates.push(normalized.clone());
        }

        if std::path::Path::new(trimmed).extension().is_none() {
            for ext in [".wav", ".mp3", ".ogg"] {
                candidates.push(format!("{trimmed}{ext}"));
                if normalized != trimmed {
                    candidates.push(format!("{normalized}{ext}"));
                }
            }
        }

        candidates.sort();
        candidates.dedup();
        candidates
    }

    fn resolve_audio_data(&self, event: &AudioEventRts) -> Option<(String, Vec<u8>)> {
        for candidate in Self::build_path_candidates(event.get_filename()) {
            if let Some(data) = AudioManager::read_from_virtual_file_system(&candidate) {
                return Some((candidate, data));
            }

            if let Ok(data) = std::fs::read(&candidate) {
                return Some((candidate, data));
            }
        }

        None
    }

    fn calculate_3d_volume_falloff(&self, position: &Coord3D) -> Real {
        let listener = self
            .listener_position
            .lock()
            .ok()
            .map(|l| *l)
            .unwrap_or_else(|| Coord3D::new());
        let dx = position.x - listener.x;
        let dy = position.y - listener.y;
        let dz = position.z - listener.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        const MIN_DISTANCE: Real = 25.0;
        const MAX_DISTANCE: Real = 1000.0;
        if distance <= MIN_DISTANCE {
            1.0
        } else if distance >= MAX_DISTANCE {
            0.0
        } else {
            let falloff = (MAX_DISTANCE - distance) / (MAX_DISTANCE - MIN_DISTANCE);
            falloff.clamp(0.0, 1.0)
        }
    }

    fn effective_volume(&self, state: &RodioSinkState) -> Real {
        let base = state.base_volume.clamp(0.0, 1.0);
        if let Some(pos) = state.position.as_ref() {
            base * self.calculate_3d_volume_falloff(pos)
        } else {
            base
        }
    }

    fn refresh_sink_volume(&self, state: &RodioSinkState) {
        if let Ok(sink) = state.sink.lock() {
            sink.set_volume(self.effective_volume(state));
        }
    }
}

impl SoundPlaybackHook for RodioPlaybackHook {
    fn play(&self, event: &AudioEventRts) -> Result<(), String> {
        let handle = event.get_playing_handle();
        if handle == 0 {
            return Err("No handle assigned".to_string());
        }
        let (file_path, audio_data) = self.resolve_audio_data(event).ok_or_else(|| {
            format!(
                "Could not resolve audio data for event '{}' (filename '{}')",
                event.get_event_name(),
                event.get_filename()
            )
        })?;
        let cursor = Cursor::new(audio_data);
        let source = Decoder::new(cursor)
            .map_err(|e| format!("Failed to decode audio file '{}': {}", file_path, e))?;
        let stream_handle = get_rodio_stream_handle()
            .ok_or_else(|| "Audio output stream not available".to_string())?;
        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| format!("Failed to create audio sink: {}", e))?;
        let volume = event.get_volume().clamp(0.0, 1.0);
        let pitch = event.get_effective_pitch();
        if (pitch - 1.0).abs() > 0.01 {
            sink.append(source.speed(pitch));
        } else {
            sink.append(source);
        }
        let state = RodioSinkState {
            sink: Arc::new(Mutex::new(sink)),
            base_volume: volume,
            position: event.is_positional_audio().then(|| *event.get_position()),
        };
        self.refresh_sink_volume(&state);
        self.sinks.lock().unwrap().insert(handle, state);
        Ok(())
    }

    fn stop(&self, handle: AudioHandle) {
        if let Some(state) = self.sinks.lock().unwrap().remove(&handle) {
            let s = state.sink.lock().unwrap();
            s.stop();
        }
    }

    fn pause(&self, handle: AudioHandle) {
        if let Some(state) = self.sinks.lock().unwrap().get(&handle) {
            let s = state.sink.lock().unwrap();
            s.pause();
        }
    }

    fn resume(&self, handle: AudioHandle) {
        if let Some(state) = self.sinks.lock().unwrap().get(&handle) {
            let s = state.sink.lock().unwrap();
            s.play();
        }
    }

    fn is_playing(&self, handle: AudioHandle) -> bool {
        let mut sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get(&handle) else {
            return false;
        };

        let is_playing = if let Ok(s) = state.sink.lock() {
            !s.empty()
        } else {
            false
        };

        if !is_playing {
            sinks.remove(&handle);
        }

        is_playing
    }

    fn set_listener_position(&self, position: &Coord3D) {
        if let Ok(mut listener) = self.listener_position.lock() {
            *listener = *position;
        }
        if let Ok(sinks) = self.sinks.lock() {
            for state in sinks.values() {
                if state.position.is_some() {
                    self.refresh_sink_volume(state);
                }
            }
        }
    }

    fn set_event_volume(&self, event: &AudioEventRts) {
        let handle = event.get_playing_handle();
        let mut sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get_mut(&handle) else {
            return;
        };

        state.base_volume = event.get_volume();
        if event.is_positional_audio() {
            state.position = Some(*event.get_position());
        }
        self.refresh_sink_volume(state);
    }
}

pub fn register_rodio_playback_hook() -> bool {
    let hook = Arc::new(RodioPlaybackHook::new());
    register_sound_playback_hook(hook)
}

const AHSV_NO_SOUND: AudioHandle = 0x0000_0000;
const AHSV_ERROR: AudioHandle = 0xFFFF_FFFF;
const AHSV_NOT_FOR_LOCAL: AudioHandle = 0xFFFF_FFFE;
const AHSV_MUTED: AudioHandle = 0xFFFF_FFFD;
const AHSV_STOP_THE_MUSIC: AudioHandle = 0xFFFF_FFF0;
const AHSV_STOP_THE_MUSIC_FADE: AudioHandle = 0xFFFF_FFF1;
const AHSV_FIRST_HANDLE: AudioHandle = 1000;

static THE_AUDIO: OnceLock<Arc<Mutex<AudioManager>>> = OnceLock::new();

struct AnimatedSoundBridge {
    audio: Arc<Mutex<AudioManager>>,
    active_handles: Mutex<HashMap<String, Vec<AudioHandle>>>,
}

impl AnimatedSoundBridge {
    fn new(audio: Arc<Mutex<AudioManager>>) -> Self {
        Self {
            audio,
            active_handles: Mutex::new(HashMap::new()),
        }
    }

    fn normalized_key(name: &str) -> Option<String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_uppercase())
        }
    }

    fn record_handle(&self, sound_name: &str, handle: AudioHandle) {
        // Match classic engine: track only concrete runtime handles, not sentinel values.
        if handle < AHSV_FIRST_HANDLE
            || handle == AHSV_ERROR
            || handle == AHSV_NOT_FOR_LOCAL
            || handle == AHSV_MUTED
            || handle == AHSV_STOP_THE_MUSIC
            || handle == AHSV_STOP_THE_MUSIC_FADE
        {
            return;
        }

        if let Some(key) = Self::normalized_key(sound_name) {
            if let Ok(mut map) = self.active_handles.lock() {
                map.entry(key).or_default().push(handle);
            }
        }
    }

    fn take_handles(&self, sound_name: &str) -> Vec<AudioHandle> {
        if let Some(key) = Self::normalized_key(sound_name) {
            if let Ok(mut map) = self.active_handles.lock() {
                return map.remove(&key).unwrap_or_default();
            }
        }
        Vec::new()
    }

    fn play_internal(&self, sound_name: &str, position: Option<Coord3D>) -> W3DResult<()> {
        if sound_name.trim().is_empty() {
            return Err(W3DError::InvalidParameter(
                "sound name must not be empty".to_string(),
            ));
        }

        let mut manager = self.audio.lock().map_err(|_| W3DError::Unknown)?;

        let mut event = if let Some(pos) = position {
            AudioEventRts::with_position(sound_name, &pos)
        } else {
            AudioEventRts::with_event_name(sound_name)
        };

        let info = manager
            .find_audio_event_info(sound_name)
            .or_else(|| manager.new_audio_event_info(sound_name.to_string()))
            .ok_or_else(|| {
                W3DError::InvalidParameter(format!("audio event info '{sound_name}' not found"))
            })?;

        event.set_audio_event_info(info.clone());
        event.set_volume(info.volume);

        let handle = manager.add_audio_event(&event);
        if handle == AHSV_ERROR {
            Err(W3DError::Unknown)
        } else {
            self.record_handle(sound_name, handle);
            Ok(())
        }
    }
}

impl SoundLibraryBridge for AnimatedSoundBridge {
    fn play_3d_audio(&self, name: &str, transform: &Mat4) -> W3DResult<()> {
        let translation = transform.w_axis.truncate();
        let position = Coord3D {
            x: translation.x,
            y: translation.y,
            z: translation.z,
        };
        self.play_internal(name, Some(position))
    }

    fn play_2d_audio(&self, name: &str) -> W3DResult<()> {
        self.play_internal(name, None)
    }

    fn stop_audio(&self, name: &str) -> W3DResult<()> {
        let handles = self.take_handles(name);
        let mut manager = self.audio.lock().map_err(|_| W3DError::Unknown)?;
        if !handles.is_empty() {
            for handle in handles {
                manager.remove_audio_event(handle);
            }
        }
        Ok(())
    }
}

/// Register the audio manager with the ww3d animated sound system.
pub fn register_animation_sound_library(manager: Arc<Mutex<AudioManager>>) {
    if let Err(err) = initialize_animated_sound_mgr::<&str>(None) {
        log::debug!("Animated sound metadata not available: {err:?}");
    }

    let bridge: Arc<dyn SoundLibraryBridge> = Arc::new(AnimatedSoundBridge::new(manager));
    set_sound_library(bridge);
}

/// Initialize the global audio manager singleton.
pub fn initialize_global_audio_manager() -> Arc<Mutex<AudioManager>> {
    if let Some(existing) = THE_AUDIO.get() {
        return existing.clone();
    }

    let manager = Arc::new(Mutex::new(AudioManager::new()));
    if THE_AUDIO.set(manager.clone()).is_err() {
        THE_AUDIO.get().expect("THE_AUDIO set but missing").clone()
    } else {
        register_rodio_playback_hook();
        register_animation_sound_library(manager.clone());
        manager
    }
}

/// Access the global audio manager if it has been initialised.
pub fn get_global_audio_manager() -> Option<Arc<Mutex<AudioManager>>> {
    THE_AUDIO.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_affect_system_setting_combinations_match_cpp_flags() {
        assert_eq!(
            AudioAffect::from_bits(AudioAffect::Music.bits() | AudioAffect::SystemSetting.bits()),
            Some(AudioAffect::MusicSystemSetting)
        );
        assert_eq!(
            AudioAffect::from_bits(AudioAffect::Sound.bits() | AudioAffect::Sound3D.bits()),
            Some(AudioAffect::SoundEffects)
        );
        assert_eq!(
            AudioAffect::from_bits(AudioAffect::All.bits() | AudioAffect::SystemSetting.bits()),
            Some(AudioAffect::AllSystemSetting)
        );

        let mut audio_manager = AudioManager::new();
        audio_manager.set_volume(0.25, AudioAffect::AllSystemSetting);
        assert_eq!(audio_manager.system_music_volume, 0.25);
        assert_eq!(audio_manager.system_sound_volume, 0.25);
        assert_eq!(audio_manager.system_sound_3d_volume, 0.25);
        assert_eq!(audio_manager.system_speech_volume, 0.25);

        audio_manager.set_on(false, AudioAffect::SoundEffects);
        assert!(!audio_manager.is_on(AudioAffect::Sound));
        assert!(!audio_manager.is_on(AudioAffect::Sound3D));
        assert!(audio_manager.is_on(AudioAffect::Music));
    }
}
