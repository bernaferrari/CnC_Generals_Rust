////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameAudio - Main audio system manager
//! Westwood Studios Pacific
//! Converted to Rust

use crate::common::audio::{
    audio_cache::AudioFileCache,
    audio_event_rts::{
        AC_INTERRUPT, AC_LOOP, AudioEventInfo, AudioEventRts, AudioHandle, AudioPriority,
        AudioType, Coord3D, MilesVolumeSliders, ObjectId, PortionToPlay, ST_GLOBAL,
        miles_event_world_position, miles_get_effective_volume, miles_positional_gain,
        miles_positional_ranges,
    },
    audio_request::{AudioRequest, RequestType},
    game_music::create_music_manager,
    game_sounds::{PlayNowAudioQueries, create_sound_manager},
    rodio_spatial::{miles_slider_volume, stereo_pan},
};
use crate::common::game_common::MSEC_PER_LOGICFRAME_REAL;
use crate::common::system::file::FileAccess;
use crate::common::system::file_system::get_file_system;
use glam::Mat4;
use hound::WavReader;
use lewton::inside_ogg::OggStreamReader;
use minimp3::{Decoder as Mp3Decoder, Error as Mp3Error};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source, SpatialSink};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Instant;
use ww3d_animation::{SoundLibraryBridge, initialize_animated_sound_mgr, set_sound_library};
use ww3d_core::errors::{W3DError, W3DResult};

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

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
    /// C++ `AIL_set_3D_orientation` (`MilesAudioManager.cpp:2630`).
    fn set_listener_orientation(&self, _orientation: &Coord3D) {}
    fn set_event_volume(&self, _event: &AudioEventRts) {}
    /// C++ `AIL_set_*_volume` used by `processFadingList` / `adjustPlayingVolume`.
    fn set_sink_volume(&self, _handle: AudioHandle, _volume: Real) {}
    /// C++ `INFINITE_LOOP_COUNT - AIL_stream_loop_count` for currently playing music.
    fn music_loop_count(&self, _handle: AudioHandle) -> Int {
        0
    }
    fn is_sink_paused(&self, _handle: AudioHandle) -> bool {
        false
    }
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
    /// C++ `MilesAudioManager::isOnScreen` via `TheTacticalView->worldToScreen`.
    fn is_world_position_on_screen(&self, pos: &Coord3D) -> bool {
        let cam = self.get_3d_camera_position();
        let ground = self.get_tactical_view_position();
        let mut look_x = ground.x - cam.x;
        let mut look_y = ground.y - cam.y;
        let mut look_z = ground.z - cam.z;
        let look_len = (look_x * look_x + look_y * look_y + look_z * look_z).sqrt();
        if look_len < 0.001 {
            let angle = self.get_tactical_view_angle();
            look_x = -angle.sin();
            look_y = angle.cos();
            look_z = 0.0;
        } else {
            look_x /= look_len;
            look_y /= look_len;
            look_z /= look_len;
        }
        let to_x = pos.x - cam.x;
        let to_y = pos.y - cam.y;
        let to_z = pos.z - cam.z;
        let to_len = (to_x * to_x + to_y * to_y + to_z * to_z).sqrt();
        if to_len < 0.001 {
            return true;
        }
        let dot = (to_x * look_x + to_y * look_y + to_z * look_z) / to_len;
        dot > 0.64
    }
}

static AUDIO_VIEW_RESOLVER: OnceLock<Arc<dyn AudioViewResolver>> = OnceLock::new();

pub fn register_audio_view_resolver(resolver: Arc<dyn AudioViewResolver>) -> bool {
    AUDIO_VIEW_RESOLVER.set(resolver).is_ok()
}

pub fn register_sound_playback_hook(hook: Arc<dyn SoundPlaybackHook>) -> bool {
    SOUND_PLAYBACK_HOOK.set(hook).is_ok()
}

pub fn sound_playback_hook_registered() -> bool {
    SOUND_PLAYBACK_HOOK.get().is_some()
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
use super::audio_event_rts::{ST_ALLIES, ST_ENEMIES, ST_EVERYONE, ST_PLAYER, ST_UI, ST_WORLD};

/// C++ `AudioManager::shouldPlayLocally` (GameAudio.cpp:988-1051).
///
/// Music and missing player-restriction bits default Everyone. `ST_PLAYER` +
/// `ST_UI` with no owner plays. Else owning player must match / be ally / be enemy.
pub fn should_play_locally_for_players(
    type_field: u32,
    is_music: bool,
    owning_player_index: Int,
    owning_player_exists: bool,
    local_player_index: Option<Int>,
    local_player_active: bool,
    observer_look_at: Option<Int>,
    local_exists_and_has_team: bool,
    relationship_to_local: AudioLocalityRelationship,
) -> Bool {
    if is_music {
        return true;
    }
    let player_restriction_mask = ST_PLAYER | ST_ALLIES | ST_ENEMIES | ST_EVERYONE;
    if (type_field & player_restriction_mask) == 0 {
        return true;
    }
    if (type_field & ST_EVERYONE) != 0 {
        return true;
    }
    if (type_field & ST_PLAYER) != 0 && (type_field & ST_UI) != 0 && !owning_player_exists {
        return true;
    }
    if !owning_player_exists {
        return false;
    }
    let mut local = match local_player_index {
        Some(index) => index,
        None => return false,
    };
    if !local_player_active {
        local = match observer_look_at {
            Some(index) => index,
            None => return false,
        };
    }
    if !local_exists_and_has_team {
        return false;
    }
    if (type_field & ST_PLAYER) != 0 {
        return owning_player_index == local;
    }
    if (type_field & ST_ALLIES) != 0 {
        return owning_player_index != local
            && relationship_to_local == AudioLocalityRelationship::Allies;
    }
    if (type_field & ST_ENEMIES) != 0 {
        return relationship_to_local == AudioLocalityRelationship::Enemies;
    }
    false
}

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

/// C++ `MilesAudioManager::playStream` sets `INFINITE_LOOP_COUNT` for `AT_Music`.
#[must_use]
pub fn music_repeats_source_infinitely(event: &AudioEventRts) -> bool {
    event
        .get_audio_event_info()
        .is_some_and(|info| info.sound_type == AudioType::Music)
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
    fn can_play_now_checked(
        &self,
        event: &mut AudioEventRts,
        queries: &PlayNowAudioQueries,
    ) -> bool {
        let _ = (event, queries);
        false
    }
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

/// C++ `MilesAudioManager` fading-list entry (`m_fadingAudio` / `m_framesFaded`).
struct FadingAudio {
    event: AudioEventRts,
    frames_faded: UnsignedInt,
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
    fading_audio: Vec<FadingAudio>,
    audio_requests: Vec<AudioRequest>,
    active_audio_events: HashMap<AudioHandle, AudioEventRts>,
    music_tracks: Vec<AsciiString>,
    current_music_track: AsciiString,

    // Audio event registry
    all_audio_event_info: HashMap<AsciiString, Arc<AudioEventInfo>>,
    audio_handle_pool: AudioHandle,
    adjusted_volumes: Vec<(AsciiString, Real)>,
    /// C++ `MilesAudioManager::m_audioForcePlayed` (briefing HAUDIO list).
    audio_force_played: Vec<AudioEventRts>,

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
            fading_audio: Vec::new(),
            active_audio_events: HashMap::new(),
            music_tracks: Vec::new(),
            current_music_track: String::new(),
            all_audio_event_info: HashMap::new(),
            audio_handle_pool: 1000, // Start at some reasonable value
            adjusted_volumes: Vec::new(),
            audio_force_played: Vec::new(),

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
        let force_handles: Vec<AudioHandle> = self
            .audio_force_played
            .iter()
            .map(|event| event.get_playing_handle())
            .collect();
        for handle in force_handles {
            let _ = with_sound_playback_hook(|hook| hook.stop(handle));
        }
        self.audio_force_played.clear();

        // Clear out any adjusted volumes
        self.adjusted_volumes.clear();
        self.active_audio_events.clear();
        self.fading_audio.clear();
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
        self.remove_level_specific_audio_event_infos();
    }

    pub fn update(&mut self) {
        // C++ MilesAudioManager::update (MilesAudioManager.cpp:460-468):
        // AudioManager::update (listener/zoom) → setDeviceListenerPosition →
        // processRequestList → processPlayingList → processFadingList → processStoppedList.
        self.update_listener_from_view();
        self.process_request_list();
        self.process_playing_list();
        self.process_fading_list();
        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.update();
        }
        self.purge_inactive_events();
    }

    fn update_listener_from_view(&mut self) {
        let Some(resolver) = AUDIO_VIEW_RESOLVER.get() else {
            return;
        };

        let ground_pos = resolver.get_tactical_view_position();
        let angle = resolver.get_tactical_view_angle();
        let camera_pos = resolver.get_3d_camera_position();
        let ground_height = resolver.get_ground_height(ground_pos.x, ground_pos.y);

        let look_to = Coord3D {
            x: -angle.sin(),
            y: angle.cos(),
            z: 0.0,
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
        // C++ GameAudio.cpp:424 writes the chosen index back onto the caller's event.
        event_to_add.set_playing_audio_index(audio_event.get_playing_audio_index());
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
        // C++ AudioManager::addAudioEvent then MusicManager/SoundManager queue AR_Play
        // (GameMusic.cpp:70-75, GameSounds.cpp:114-121). Miles processRequestList
        // honors delay and checkForSample before playAudioEvent.
        if sound_type != AudioType::Music {
            if self.does_violate_limit(&mut audio_event) {
                return AHSV_NO_SOUND;
            }
            let queries = self.play_now_queries(&audio_event, false);
            if let Some(sound_mgr) = &self.sound_manager {
                if !sound_mgr.can_play_now_checked(&mut audio_event, &queries) {
                    return AHSV_NO_SOUND;
                }
            }
        }

        let request = AudioRequest::new_with_event(RequestType::Play, audio_event);
        self.append_audio_request(request);
        handle
    }

    pub fn get_audio_event_info(&self, event_name: &str) -> Option<Arc<AudioEventInfo>> {
        self.find_audio_event_info(event_name)
    }

    pub fn remove_audio_event(&mut self, audio_event: AudioHandle) {
        if audio_event == AHSV_STOP_THE_MUSIC_FADE {
            // C++ MilesAudioManager::stopAudioEvent (MilesAudioManager.cpp:885-907)
            // pushes AT_Music onto m_fadingAudio instead of releasing immediately.
            self.begin_music_fade();
            return;
        }
        if audio_event == AHSV_STOP_THE_MUSIC {
            if let Some(music_mgr) = &mut self.music_manager {
                music_mgr.remove_audio_event(audio_event);
            }
            self.release_fading_audio();
            let music_handles: Vec<AudioHandle> = self
                .active_audio_events
                .iter()
                .filter_map(|(handle, event)| {
                    event
                        .get_audio_event_info()
                        .filter(|info| info.sound_type == AudioType::Music)
                        .map(|_| *handle)
                })
                .collect();
            for handle in music_handles {
                self.active_audio_events.remove(&handle);
                let _ = with_sound_playback_hook(|hook| hook.stop(handle));
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

    /// Live Drawable::update ambient restart: is this object-owned event still
    /// in `active_audio_events`? `processPlayingList` volume-cull removes it.
    pub fn is_named_event_playing_for_object(&self, object_id: u32, event_name: &str) -> bool {
        if object_id == 0 || event_name.is_empty() {
            return false;
        }
        self.active_audio_events
            .values()
            .any(|event| event.get_event_name() == event_name && event.get_object_id() == object_id)
    }

    /// Test hook: pretend Miles is still playing this object-owned event.
    pub fn test_insert_active_named_event(&mut self, object_id: u32, event_name: &str) {
        let mut event = AudioEventRts::with_event_name(event_name);
        event.set_object_id(object_id);
        event.set_playing_handle(1001);
        self.active_audio_events.insert(1001, event);
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

    fn playing_or_current_music_name(&self) -> String {
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

    fn queue_music_track(&mut self, track_name: &str) {
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

    /// C++ `MilesAudioManager` slider snapshot used by `getEffectiveVolume`.
    pub fn miles_volume_sliders(&self) -> MilesVolumeSliders {
        MilesVolumeSliders {
            music_volume: self.music_volume,
            speech_volume: self.speech_volume,
            sound_volume: self.sound_volume,
            sound_3d_volume: self.sound_3d_volume,
            global_min_range: self.audio_settings.global_min_range as f32,
            global_max_range: self.audio_settings.global_max_range as f32,
        }
    }

    /// C++ `MilesAudioManager::getEffectiveVolume`.
    pub fn get_effective_volume(&self, event: &AudioEventRts) -> Real {
        miles_get_effective_volume(event, &self.listener_position, &self.miles_volume_sliders())
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
        let _ = with_sound_playback_hook(|hook| {
            hook.set_listener_position(new_listener_pos);
            hook.set_listener_orientation(new_listener_orientation);
        });
    }

    pub fn get_listener_position(&self) -> &Coord3D {
        &self.listener_position
    }

    pub fn get_listener_orientation(&self) -> &Coord3D {
        &self.listener_orientation
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

    /// C++ `MilesAudioManager::processRequestList` (MilesAudioManager.cpp:2218-2238).
    pub fn process_request_list(&mut self) {
        let mut i = 0;
        while i < self.audio_requests.len() {
            if !self.should_process_request_this_frame(&self.audio_requests[i]) {
                self.adjust_request(i);
                i += 1;
                continue;
            }
            let missing_info_name = self.audio_requests[i]
                .get_pending_event()
                .and_then(|event| {
                    event
                        .get_audio_event_info()
                        .is_none()
                        .then(|| event.get_event_name().to_string())
                });
            if let Some(name) = missing_info_name {
                if let Some(info) = self.find_audio_event_info(&name) {
                    if let Some(event) = self.audio_requests[i].get_pending_event_mut() {
                        event.set_audio_event_info(info);
                    }
                }
            }

            if self.audio_requests[i].requires_check_for_sample() {
                let mut probe = self.audio_requests[i].get_pending_event().cloned();
                if let Some(probe) = &mut probe {
                    if !self.check_event_can_play_now(probe) {
                        self.audio_requests.remove(i);
                        continue;
                    }
                    if let Some(pending) = self.audio_requests[i].get_pending_event_mut() {
                        pending.set_handle_to_kill(probe.get_handle_to_kill());
                    }
                }
            }

            let request = self.audio_requests.remove(i);
            self.process_request(request);
        }
    }

    /// C++ `MilesAudioManager::shouldProcessRequestThisFrame` (2483-2487).
    fn should_process_request_this_frame(&self, request: &AudioRequest) -> bool {
        if !request.use_pending_event {
            return true;
        }
        request
            .get_pending_event()
            .is_none_or(|event| event.get_delay() < MSEC_PER_LOGICFRAME_REAL)
    }

    /// C++ `MilesAudioManager::adjustRequest` (2491-2498).
    fn adjust_request(&mut self, index: usize) {
        let Some(request) = self.audio_requests.get_mut(index) else {
            return;
        };
        if !request.use_pending_event {
            return;
        }
        if let Some(event) = request.get_pending_event_mut() {
            event.decrement_delay(MSEC_PER_LOGICFRAME_REAL);
        }
        request.set_requires_check_for_sample(true);
    }

    /// C++ `MilesAudioManager::checkForSample` (2502-2519) + `canPlayNow` limit check.
    fn check_for_sample(&self, request: &AudioRequest) -> bool {
        if !request.use_pending_event {
            return true;
        }
        let Some(event) = request.get_pending_event() else {
            return true;
        };
        let mut probe = event.clone();
        self.check_event_can_play_now(&mut probe)
    }

    fn check_event_can_play_now(&self, event: &mut AudioEventRts) -> bool {
        let Some(info) = event.get_audio_event_info() else {
            return true;
        };
        if info.sound_type != AudioType::SoundEffect {
            return true;
        }
        let violates_limit = self.does_violate_limit(event);
        let queries = self.play_now_queries(event, violates_limit);
        self.sound_manager
            .as_ref()
            .is_none_or(|sound_mgr| sound_mgr.can_play_now_checked(event, &queries))
    }

    fn play_now_queries(&self, event: &AudioEventRts, violates_limit: bool) -> PlayNowAudioQueries {
        let object_id = event.get_object_id();
        PlayNowAudioQueries {
            object_playing_voice: object_id != 0 && self.is_object_playing_voice(object_id),
            playing_lower_priority: self.is_playing_lower_priority(event),
            playing_already: self.is_playing_already(event),
            violates_limit,
        }
    }

    /// C++ `MilesAudioManager::processRequest` (2916-2935).
    fn process_request(&mut self, request: AudioRequest) {
        match request.request {
            RequestType::Stop => {
                if let Some(handle) = request.get_handle() {
                    self.stop_playing_handle(handle);
                }
            }
            RequestType::Play => {
                if let Some(event) = request.take_pending_event() {
                    self.play_audio_event(event);
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
            ..Default::default()
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

    /// C++ `AudioManager::removeLevelSpecificAudioEventInfos` (GameAudio.cpp:854-871).
    pub fn remove_level_specific_audio_event_infos(&mut self) {
        self.all_audio_event_info
            .retain(|_, info| !info.is_level_specific());
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

    /// C++ `MilesAudioManager::stopAllSpeech` (MilesAudioManager.cpp:1243-1260)
    /// `releasePlayingAudio` hard-stops AT_Streaming and erases the playing entry.
    pub fn stop_all_speech(&mut self) {
        let handles: Vec<AudioHandle> = self
            .active_audio_events
            .iter()
            .filter_map(|(handle, event)| {
                event
                    .get_audio_event_info()
                    .filter(|info| info.sound_type == AudioType::Streaming)
                    .map(|_| *handle)
            })
            .collect();
        for handle in handles {
            self.release_playing_handle(handle);
        }
    }

    fn notify_sample_completion_if_effect(&mut self, event: &AudioEventRts) {
        if !event
            .get_audio_event_info()
            .is_some_and(|info| info.sound_type == AudioType::SoundEffect)
        {
            return;
        }
        if let Some(sound_mgr) = &mut self.sound_manager {
            if event.is_positional_audio() {
                sound_mgr.notify_of_3d_sample_completion();
            } else {
                sound_mgr.notify_of_2d_sample_completion();
            }
        }
    }

    /// C++ `MilesAudioManager::stopAudioEvent` sets `m_requestStop` so Decay can play.
    fn stop_playing_handle(&mut self, handle: AudioHandle) {
        if let Some(event) = self.active_audio_events.get_mut(&handle) {
            event.set_request_stop(true);
            return;
        }
        let _ = with_sound_playback_hook(|hook| hook.stop(handle));
    }

    /// C++ `MilesAudioManager::releasePlayingAudio` — hard-stop the sample and drop it.
    fn release_playing_handle(&mut self, handle: AudioHandle) {
        if handle < AHSV_FIRST_HANDLE {
            return;
        }
        if let Some(event) = self.active_audio_events.remove(&handle) {
            self.notify_sample_completion_if_effect(&event);
        }
        let _ = with_sound_playback_hook(|hook| hook.stop(handle));
    }

    fn play_audio_event(&mut self, event: AudioEventRts) {
        let Some(info) = event.get_audio_event_info() else {
            return;
        };
        let sound_type = info.sound_type;
        let uninterruptable = event.get_uninterruptable();

        if sound_type == AudioType::Streaming && uninterruptable {
            self.stop_all_speech();
        }

        let handle_to_kill = event.get_handle_to_kill();
        if handle_to_kill != 0 {
            // C++ playAudioEvent handleToKill (MilesAudioManager.cpp:735-813)
            // releasePlayingAudio hard-stops the channel before reuse.
            self.release_playing_handle(handle_to_kill);
        }

        if sound_type == AudioType::SoundEffect && !self.sample_slot_available(&event) {
            // C++ getFirst2D/3DSample empty → killLowestPrioritySoundImmediately.
            if !self.kill_lowest_priority_sound_immediately(&event) {
                return;
            }
        }

        let hook_result = with_sound_playback_hook(|hook| hook.play(&event));
        let play_ok = match hook_result {
            Some(Ok(())) | None => true,
            Some(Err(_)) => false,
        };
        if !play_ok {
            return;
        }

        if sound_type == AudioType::Streaming && uninterruptable {
            self.set_disallow_speech(true);
        }

        if sound_type == AudioType::Music {
            self.current_music_track = event.get_event_name().to_string();
        } else if sound_type == AudioType::SoundEffect {
            if let Some(sound_mgr) = &mut self.sound_manager {
                if event.is_positional_audio() {
                    sound_mgr.notify_of_3d_sample_start();
                } else {
                    sound_mgr.notify_of_2d_sample_start();
                }
            }
        }

        self.track_active_event(&event);
    }

    /// C++ `MilesAudioManager::friend_forcePlayAudioEventRTS` (2971-3017).
    /// Mission-briefing force-play at the speech slider volume.
    pub fn friend_force_play_audio_event_rts(&mut self, event_to_play: &AudioEventRts) {
        let mut event = event_to_play.clone();
        if event.get_audio_event_info().is_none() {
            if let Some(info) = self.find_audio_event_info(event.get_event_name()) {
                event.set_audio_event_info(info);
            }
        }
        let Some(info) = event.get_audio_event_info() else {
            return;
        };
        match info.sound_type {
            AudioType::Music if !self.is_on(AudioAffect::Music) => return,
            AudioType::SoundEffect
                if !self.is_on(AudioAffect::Sound) || !self.is_on(AudioAffect::Sound3D) =>
            {
                return;
            }
            AudioType::Streaming if !self.is_on(AudioAffect::Speech) => return,
            _ => {}
        }

        event.generate_filename();
        event.generate_play_info();
        for (name, volume) in &self.adjusted_volumes {
            if *name == event.get_event_name() {
                event.set_volume(*volume);
                break;
            }
        }

        // C++ applies event.getVolume() * speech slider once on the raw HAUDIO.
        // RodioPlaybackHook::play already multiplies via miles_get_effective_volume.
        let handle = self.allocate_new_handle();
        event.set_playing_handle(handle);

        let hook_result = with_sound_playback_hook(|hook| hook.play(&event));
        let play_ok = match hook_result {
            Some(Ok(())) | None => true,
            Some(Err(_)) => false,
        };
        if play_ok {
            self.audio_force_played.push(event);
        }
    }

    pub fn force_played_count(&self) -> usize {
        self.audio_force_played.len()
    }

    pub fn force_played_volume(&self) -> Option<Real> {
        self.audio_force_played
            .last()
            .map(AudioEventRts::get_volume)
    }

    pub fn pending_play_delay(&self) -> Option<Real> {
        self.audio_requests.iter().find_map(|request| {
            (request.request == RequestType::Play)
                .then(|| request.get_pending_event().map(AudioEventRts::get_delay))
                .flatten()
        })
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

        // C++ MilesAudioManager::pauseAudio (MilesAudioManager.cpp:569-583)
        // erases pending AR_Play requests so they cannot fire after resume.
        self.audio_requests
            .retain(|request| request.request != RequestType::Play);
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

    /// C++ `MilesAudioManager::stopAudio` (MilesAudioManager.cpp:471-524).
    /// Sets playing entries stopped; `processPlayingList` releases them. Here
    /// we release immediately so paused Rodio sinks cannot stay as zombies.
    pub fn stop_audio(&mut self, which: AudioAffect) {
        let handles: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter(|event| event_matches_audio_affect(event, which))
            .map(|event| event.get_playing_handle())
            .collect();
        for handle in handles {
            self.release_playing_handle(handle);
        }
    }

    /// C++ `MilesAudioManager::isMusicPlaying` (MilesAudioManager.cpp:1355-1366).
    pub fn is_music_playing(&self) -> Bool {
        self.active_audio_events.values().any(|event| {
            event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type == AudioType::Music)
                && with_sound_playback_hook(|hook| {
                    let handle = event.get_playing_handle();
                    hook.is_playing(handle) && !hook.is_sink_paused(handle)
                })
                .unwrap_or(true)
        })
    }

    /// C++ `MilesAudioManager::hasMusicTrackCompleted` (MilesAudioManager.cpp:1370-1385).
    pub fn has_music_track_completed(&self, track_name: &str, number_of_times: Int) -> Bool {
        self.active_audio_events.values().any(|event| {
            event.get_event_name() == track_name
                && event
                    .get_audio_event_info()
                    .is_some_and(|info| info.sound_type == AudioType::Music)
                && with_sound_playback_hook(|hook| {
                    hook.music_loop_count(event.get_playing_handle()) >= number_of_times
                })
                .unwrap_or(false)
        })
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

    pub fn should_play_locally(&self, audio_event: &AudioEventRts) -> Bool {
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

        // Live host registers a resolver; leftover PlayerList is not the player path.
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

        let mut local_player_index = match resolver.get_local_player_index() {
            Some(index) => index,
            None => {
                return should_play_locally_for_players(
                    event_info.type_field,
                    event_info.sound_type == AudioType::Music,
                    owning_player_index,
                    owning_player_exists,
                    None,
                    false,
                    resolver.get_observer_look_at_player_index(),
                    false,
                    AudioLocalityRelationship::Neutral,
                );
            }
        };

        let local_player_active = resolver.is_player_active(local_player_index);
        if !local_player_active {
            if let Some(observer) = resolver.get_observer_look_at_player_index() {
                local_player_index = observer;
            }
        }

        let local_exists_and_has_team = resolver.player_exists(local_player_index)
            && resolver.has_default_team(local_player_index);
        let relationship =
            resolver.get_relationship_to_local_team(owning_player_index, local_player_index);

        should_play_locally_for_players(
            event_info.type_field,
            event_info.sound_type == AudioType::Music,
            owning_player_index,
            owning_player_exists,
            Some(local_player_index),
            local_player_active,
            resolver.get_observer_look_at_player_index(),
            local_exists_and_has_team,
            relationship,
        )
    }

    // C++ parity methods used by SoundManager for audio culling

    /// C++ `MilesAudioManager::doesViolateLimit` (MilesAudioManager.cpp:1802-1882).
    /// At limit, records the oldest same-name playing handle on the event.
    /// `AC_INTERRUPT` compares request-list count vs playing count.
    pub fn does_violate_limit(&self, event: &mut AudioEventRts) -> Bool {
        let Some(event_info) = event.get_audio_event_info() else {
            return false;
        };

        if event_info.limit <= 0 {
            return false;
        }
        let limit = event_info.limit;
        let interrupting = (event_info.control & AC_INTERRUPT) != 0;
        let event_name = event.get_event_name().to_string();
        let positional = event.is_positional_audio();

        let mut matching_handles: Vec<AudioHandle> = self
            .active_audio_events
            .values()
            .filter(|playing| {
                playing.get_event_name() == event_name
                    && playing.is_positional_audio() == positional
                    && self.event_counts_as_playing(playing)
            })
            .map(|playing| playing.get_playing_handle())
            .collect();
        matching_handles.sort_unstable();

        let mut total_playing_count = matching_handles.len() as Int;
        if let Some(&oldest) = matching_handles.first() {
            event.set_handle_to_kill(oldest);
        }

        let mut total_request_count = 0;
        for request in &self.audio_requests {
            if !request.use_pending_event {
                continue;
            }
            if request
                .get_pending_event()
                .is_some_and(|pending| pending.get_event_name() == event_name)
            {
                total_request_count += 1;
                total_playing_count += 1;
            }
        }

        if interrupting {
            if total_request_count < limit {
                if total_request_count + (total_playing_count - total_request_count) < limit {
                    event.set_handle_to_kill(0);
                    return false;
                }
                // Exceeding via playing sounds: keep kill handle, still allow the request.
                return false;
            }
        }

        if total_playing_count < limit {
            event.set_handle_to_kill(0);
            return false;
        }

        true
    }

    fn event_counts_as_playing(&self, event: &AudioEventRts) -> bool {
        with_sound_playback_hook(|hook| hook.is_playing(event.get_playing_handle())).unwrap_or(true)
    }

    /// C++ `MilesAudioManager::isPlayingLowerPriority` (1993-2024).
    /// Uses INI `AudioEventInfo.m_priority`, not the per-event field (always Normal).
    pub fn is_playing_lower_priority(&self, event: &AudioEventRts) -> Bool {
        let event_priority = Self::sample_event_priority(event);
        if event_priority == AudioPriority::Lowest {
            return false;
        }
        let positional = event.is_positional_audio();

        let playing_is_lower = |playing: &AudioEventRts| {
            if !Self::is_playing_sample(playing) || playing.is_positional_audio() != positional {
                return false;
            }
            Self::sample_event_priority(playing) < event_priority
        };

        with_sound_playback_hook(|hook| {
            self.active_audio_events.values().any(|playing| {
                hook.is_playing(playing.get_playing_handle()) && playing_is_lower(playing)
            })
        })
        .unwrap_or_else(|| self.active_audio_events.values().any(playing_is_lower))
    }

    fn sample_event_priority(event: &AudioEventRts) -> AudioPriority {
        event
            .get_audio_event_info()
            .map(|info| info.priority)
            .unwrap_or_else(|| event.get_audio_priority())
    }

    fn is_playing_sample(event: &AudioEventRts) -> bool {
        event
            .get_audio_event_info()
            .is_some_and(|info| info.sound_type == AudioType::SoundEffect)
    }

    fn sample_slot_available(&self, event: &AudioEventRts) -> bool {
        let positional = event.is_positional_audio();
        let cap = if positional {
            self.get_num_3d_samples()
        } else {
            self.get_num_2d_samples()
        }
        .max(0) as usize;
        let used = self
            .active_audio_events
            .values()
            .filter(|playing| {
                Self::is_playing_sample(playing) && playing.is_positional_audio() == positional
            })
            .count();
        used < cap
    }

    /// C++ `MilesAudioManager::findLowestPrioritySound` (1935-1990).
    fn find_lowest_priority_sound(&self, event: &AudioEventRts) -> Option<AudioHandle> {
        let priority = Self::sample_event_priority(event);
        if priority == AudioPriority::Lowest {
            return None;
        }
        let positional = event.is_positional_audio();
        let mut lowest: Option<(AudioPriority, AudioHandle)> = None;
        for playing in self.active_audio_events.values() {
            if !Self::is_playing_sample(playing) || playing.is_positional_audio() != positional {
                continue;
            }
            let playing_priority = Self::sample_event_priority(playing);
            if playing_priority >= priority {
                continue;
            }
            if lowest.is_none_or(|(current, _)| playing_priority < current) {
                lowest = Some((playing_priority, playing.get_playing_handle()));
                if playing_priority == AudioPriority::Lowest {
                    break;
                }
            }
        }
        lowest.map(|(_, handle)| handle)
    }

    /// C++ `MilesAudioManager::killLowestPrioritySoundImmediately` (2027-2074).
    fn kill_lowest_priority_sound_immediately(&mut self, event: &AudioEventRts) -> bool {
        let Some(handle) = self.find_lowest_priority_sound(event) else {
            return false;
        };
        self.release_playing_handle(handle);
        true
    }

    /// C++ `MilesAudioManager::isPlayingAlready` (1886-1905).
    /// 2D events scan 2D samples only; 3D events scan 3D samples only.
    pub fn is_playing_already(&self, event: &AudioEventRts) -> Bool {
        let event_name = event.get_event_name();
        let positional = event.is_positional_audio();

        let same_name_same_partition = |playing: &AudioEventRts| {
            playing.get_event_name() == event_name && playing.is_positional_audio() == positional
        };

        with_sound_playback_hook(|hook| {
            self.active_audio_events.values().any(|playing| {
                same_name_same_partition(playing) && hook.is_playing(playing.get_playing_handle())
            })
        })
        .unwrap_or_else(|| {
            self.active_audio_events
                .values()
                .any(same_name_same_partition)
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

    /// C++ `MilesAudioManager::adjustVolumeOfPlayingAudio` (MilesAudioManager.cpp:2079-2116).
    /// Sets event volume then immediately re-applies `AIL_set_*_volume` as sink volume.
    pub fn adjust_volume_of_playing_audio(&mut self, event_name: &str, new_volume: Real) {
        for event in self.active_audio_events.values_mut() {
            if event.get_event_name() == event_name {
                event.set_volume(new_volume);
                let desired_volume = event.get_volume() * event.get_volume_shift();
                let handle = event.get_playing_handle();
                let _ = with_sound_playback_hook(|hook| {
                    hook.set_sink_volume(handle, desired_volume);
                });
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

    /// C++ `MilesAudioManager::has3DSensitiveStreamsPlaying` (2381-2404).
    /// Only music/speech streams count. `Game_` music is not sensitive.
    pub fn has_3d_sensitive_streams_playing(&self) -> Bool {
        let is_sensitive = |event: &AudioEventRts| -> bool {
            let Some(info) = event.get_audio_event_info() else {
                return false;
            };
            match info.sound_type {
                AudioType::Streaming => true,
                AudioType::Music => !event.get_event_name().starts_with("Game_"),
                AudioType::SoundEffect => false,
            }
        };
        with_sound_playback_hook(|hook| {
            self.active_audio_events
                .values()
                .any(|event| is_sensitive(event) && hook.is_playing(event.get_playing_handle()))
        })
        .unwrap_or_else(|| self.active_audio_events.values().any(is_sensitive))
    }

    fn begin_music_fade(&mut self) {
        let music_events: Vec<AudioEventRts> = self
            .active_audio_events
            .values()
            .filter(|event| {
                event
                    .get_audio_event_info()
                    .is_some_and(|info| info.sound_type == AudioType::Music)
            })
            .cloned()
            .collect();

        for event in music_events {
            let handle = event.get_playing_handle();
            if self
                .fading_audio
                .iter()
                .any(|fade| fade.event.get_playing_handle() == handle)
            {
                continue;
            }
            self.active_audio_events.remove(&handle);
            self.fading_audio.push(FadingAudio {
                event,
                frames_faded: 0,
            });
        }
    }

    fn release_fading_audio(&mut self) {
        let fading = std::mem::take(&mut self.fading_audio);
        for fade in fading {
            let handle = fade.event.get_playing_handle();
            let _ = with_sound_playback_hook(|hook| hook.stop(handle));
        }
    }

    fn adjust_playing_volume(&self, event: &AudioEventRts) {
        let volume = self.get_effective_volume(event);
        let _ = with_sound_playback_hook(|hook| {
            hook.set_sink_volume(event.get_playing_handle(), volume);
        });
    }

    fn process_playing_list(&mut self) {
        // C++ MilesAudioManager::processPlayingList (MilesAudioManager.cpp:2242-2368).
        let handles: Vec<AudioHandle> = self.active_audio_events.keys().copied().collect();
        let mut to_stop: Vec<AudioHandle> = Vec::new();
        let volume_changed = self.volume_has_changed;

        for handle in handles {
            let Some(mut event) = self.active_audio_events.remove(&handle) else {
                continue;
            };

            if volume_changed {
                self.adjust_playing_volume(&event);
            }

            if event.is_positional_audio() {
                let has_pos = event.get_current_position().is_some();
                let is_dead = event.is_dead();
                if !has_pos {
                    self.notify_sample_completion_if_effect(&event);
                    to_stop.push(handle);
                    continue;
                } else if is_dead {
                    // C++ stopAudioEvent: requestStop, sample keeps playing for Decay.
                    event.set_request_stop(true);
                } else {
                    let vol_for_consideration = {
                        let effective = self.get_effective_volume(&event);
                        if self.sound_3d_volume > 0.0 {
                            effective / self.sound_volume.max(f32::EPSILON)
                        } else {
                            effective
                        }
                    };
                    let play_anyways = event.get_audio_event_info().is_some_and(|info| {
                        (info.type_field & ST_GLOBAL) != 0
                            || info.priority == AudioPriority::Critical
                    });
                    if vol_for_consideration < self.audio_settings.min_volume && !play_anyways {
                        self.notify_sample_completion_if_effect(&event);
                        to_stop.push(handle);
                        continue;
                    }
                    let _ = with_sound_playback_hook(|hook| hook.set_event_volume(&event));
                }
            }

            if !with_sound_playback_hook(|hook| hook.is_playing(handle)).unwrap_or(true) {
                if self.notify_of_audio_completion(&mut event) {
                    let still_playing =
                        with_sound_playback_hook(|hook| hook.is_playing(handle)).unwrap_or(false);
                    if still_playing {
                        self.active_audio_events.insert(handle, event);
                        continue;
                    }
                    // Delayed loop queued a new request; this sample has ended.
                    self.notify_sample_completion_if_effect(&event);
                    continue;
                }
                self.notify_sample_completion_if_effect(&event);
                continue;
            }

            self.active_audio_events.insert(handle, event);
        }

        for handle in to_stop {
            let _ = with_sound_playback_hook(|hook| hook.stop(handle));
        }

        if self.volume_has_changed {
            self.volume_has_changed = false;
        }
    }

    fn process_fading_list(&mut self) {
        // C++ MilesAudioManager::processFadingList (MilesAudioManager.cpp:2410-2458).
        let fade_frames = self.audio_settings.fade_audio_frames.max(1);
        let mut i = 0;
        while i < self.fading_audio.len() {
            if self.fading_audio[i].frames_faded >= fade_frames {
                let handle = self.fading_audio[i].event.get_playing_handle();
                let _ = with_sound_playback_hook(|hook| hook.stop(handle));
                self.fading_audio.remove(i);
                continue;
            }

            self.fading_audio[i].frames_faded += 1;
            let frames_faded = self.fading_audio[i].frames_faded;
            let mut volume = self.get_effective_volume(&self.fading_audio[i].event);
            volume *= 1.0 - (frames_faded as Real / fade_frames as Real);
            let handle = self.fading_audio[i].event.get_playing_handle();
            let _ = with_sound_playback_hook(|hook| hook.set_sink_volume(handle, volume));
            i += 1;
        }
    }

    fn notify_of_audio_completion(&mut self, event: &mut AudioEventRts) -> bool {
        // C++ MilesAudioManager::notifyOfAudioCompletion (MilesAudioManager.cpp:1507-1566).
        if self.disallow_speech
            && event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type == AudioType::Streaming)
        {
            self.disallow_speech = false;
        }

        let is_loop = event
            .get_audio_event_info()
            .is_some_and(|info| (info.control & AC_LOOP) != 0);
        if is_loop {
            if event.get_next_play_portion() == PortionToPlay::Attack {
                event.set_next_play_portion(PortionToPlay::Sound);
            }
            if event.get_next_play_portion() == PortionToPlay::Sound {
                event.decrease_loop_count();
                if self.start_next_loop(event) {
                    return true;
                }
            }
        }

        event.advance_next_play_portion();
        if event.get_next_play_portion() != PortionToPlay::Done
            && event
                .get_audio_event_info()
                .is_some_and(|info| info.sound_type != AudioType::Music)
            && self.replay_playing_event(event)
        {
            return true;
        }

        if event
            .get_audio_event_info()
            .is_some_and(|info| info.sound_type == AudioType::Music)
            && self.replay_playing_event(event)
        {
            return true;
        }

        false
    }

    fn replay_playing_event(&self, event: &AudioEventRts) -> bool {
        match with_sound_playback_hook(|hook| hook.play(event)) {
            Some(Ok(())) => true,
            Some(Err(_)) => false,
            None => true,
        }
    }

    fn start_next_loop(&mut self, event: &mut AudioEventRts) -> bool {
        // C++ MilesAudioManager::startNextLoop (MilesAudioManager.cpp:2719-2756).
        // Loop count is decreased by notifyOfAudioCompletion before this call.
        if event.get_request_stop() {
            return false;
        }
        if !event.has_more_loops() {
            return false;
        }

        event.generate_filename();
        if event.get_delay() > MSEC_PER_LOGICFRAME_REAL {
            let mut request = AudioRequest::new_with_event(RequestType::Play, event.clone());
            request.set_requires_check_for_sample(true);
            self.append_audio_request(request);
            return true;
        }

        self.replay_playing_event(event)
    }

    pub fn pending_play_request_count(&self) -> usize {
        self.audio_requests
            .iter()
            .filter(|request| request.request == RequestType::Play)
            .count()
    }

    pub fn fading_audio_count(&self) -> usize {
        self.fading_audio.len()
    }

    pub fn volume_has_changed_flag(&self) -> bool {
        self.volume_has_changed
    }

    pub fn fading_frames(&self) -> UnsignedInt {
        self.fading_audio
            .first()
            .map(|fade| fade.frames_faded)
            .unwrap_or(0)
    }

    pub fn force_notify_completion_for_test(&mut self, handle: AudioHandle) -> bool {
        let Some(mut event) = self.active_audio_events.remove(&handle) else {
            return false;
        };
        let restarted = self.notify_of_audio_completion(&mut event);
        if restarted {
            self.active_audio_events.insert(handle, event);
        }
        restarted
    }

    pub fn active_event_mut_for_test(&mut self, handle: AudioHandle) -> Option<&mut AudioEventRts> {
        self.active_audio_events.get_mut(&handle)
    }

    pub fn insert_playing_event_for_test(&mut self, event: AudioEventRts) {
        self.track_active_event(&event);
    }

    pub fn active_event_count(&self) -> usize {
        self.active_audio_events.len()
    }

    pub fn active_event_loop_count(&self, handle: AudioHandle) -> Option<Int> {
        self.active_audio_events
            .get(&handle)
            .map(|event| event.get_loop_count())
    }

    pub fn active_event_portion(&self, handle: AudioHandle) -> Option<PortionToPlay> {
        self.active_audio_events
            .get(&handle)
            .map(|event| event.get_next_play_portion())
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
static LIVE_AUDIO_FILE_CACHE: LazyLock<AudioFileCache> =
    LazyLock::new(|| AudioFileCache::new(16 * 1024 * 1024));

fn live_audio_file_cache() -> &'static AudioFileCache {
    &LIVE_AUDIO_FILE_CACHE
}

/// C++ `AudioFileCache::openFile` stereo-positional reject (MilesAudioManager.cpp:3147-3152).
fn wav_channel_count(data: &[u8]) -> Option<u16> {
    if data.len() < 24 || &data[0..4] != b"RIFF" {
        return None;
    }
    Some(u16::from_le_bytes([data[22], data[23]]))
}

struct CachedAudioBytes(Arc<Vec<u8>>);

impl AsRef<[u8]> for CachedAudioBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

enum RodioVoice {
    Flat(Sink),
    Spatial(SpatialSink),
}

impl RodioVoice {
    fn set_volume(&self, volume: f32) {
        match self {
            Self::Flat(sink) => sink.set_volume(volume),
            Self::Spatial(sink) => sink.set_volume(volume),
        }
    }

    fn stop(&self) {
        match self {
            Self::Flat(sink) => sink.stop(),
            Self::Spatial(sink) => sink.stop(),
        }
    }

    fn pause(&self) {
        match self {
            Self::Flat(sink) => sink.pause(),
            Self::Spatial(sink) => sink.pause(),
        }
    }

    fn play(&self) {
        match self {
            Self::Flat(sink) => sink.play(),
            Self::Spatial(sink) => sink.play(),
        }
    }

    fn empty(&self) -> bool {
        match self {
            Self::Flat(sink) => sink.empty(),
            Self::Spatial(sink) => sink.empty(),
        }
    }

    fn is_paused(&self) -> bool {
        match self {
            Self::Flat(sink) => sink.is_paused(),
            Self::Spatial(sink) => sink.is_paused(),
        }
    }

    fn set_stereo_pan(&self, pan: Real) {
        if let Self::Spatial(sink) = self {
            sink.set_emitter_position([pan, 0.0, -1.0]);
            sink.set_left_ear_position([-0.1, 0.0, 0.0]);
            sink.set_right_ear_position([0.1, 0.0, 0.0]);
        }
    }
}

struct RodioPlaybackHook {
    sinks: Mutex<HashMap<AudioHandle, RodioSinkState>>,
    listener_position: Mutex<Coord3D>,
    listener_orientation: Mutex<Coord3D>,
}

struct RodioSinkState {
    sink: Arc<Mutex<RodioVoice>>,
    base_volume: Real,
    position: Option<Coord3D>,
    min_distance: Real,
    max_distance: Real,
    is_music: bool,
    started_at: Instant,
    duration_ms: Option<Real>,
}

impl RodioPlaybackHook {
    fn new() -> Self {
        let _ = get_rodio_stream_handle();
        Self {
            sinks: Mutex::new(HashMap::new()),
            listener_position: Mutex::new(Coord3D::new()),
            listener_orientation: Mutex::new(Coord3D {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            }),
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

    fn portion_filename(event: &AudioEventRts) -> &str {
        match event.get_next_play_portion() {
            PortionToPlay::Attack if !event.get_attack_filename().is_empty() => {
                event.get_attack_filename()
            }
            PortionToPlay::Decay if !event.get_decay_filename().is_empty() => {
                event.get_decay_filename()
            }
            _ => event.get_filename(),
        }
    }

    fn resolve_audio_data(&self, event: &AudioEventRts) -> Option<(String, Arc<Vec<u8>>)> {
        let mut names = vec![Self::portion_filename(event).to_string()];
        let fallback = event.get_filename();
        if fallback != names[0] {
            names.push(fallback.to_string());
        }
        let cache = live_audio_file_cache();
        for name in names {
            for candidate in Self::build_path_candidates(&name) {
                let Some(data) = cache.get_or_insert_named(&candidate, || {
                    AudioManager::read_from_virtual_file_system(&candidate)
                        .or_else(|| std::fs::read(&candidate).ok())
                }) else {
                    continue;
                };
                if event.is_positional_audio() {
                    if let Some(channels) = wav_channel_count(data.as_ref()) {
                        if channels > 1 {
                            cache.close_named(&candidate);
                            return None;
                        }
                    }
                }
                return Some((candidate, data));
            }
        }
        None
    }

    fn calculate_3d_volume_falloff(
        &self,
        position: &Coord3D,
        min_distance: Real,
        max_distance: Real,
    ) -> Real {
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
        miles_positional_gain(distance, min_distance, max_distance)
    }

    fn effective_volume(&self, state: &RodioSinkState) -> Real {
        let base = state.base_volume.clamp(0.0, 1.0);
        if let Some(pos) = state.position.as_ref() {
            base * self.calculate_3d_volume_falloff(pos, state.min_distance, state.max_distance)
        } else {
            base
        }
    }

    fn refresh_sink_volume(&self, state: &RodioSinkState) {
        if let Ok(sink) = state.sink.lock() {
            sink.set_volume(self.effective_volume(state));
        }
    }

    fn listener_pose(&self) -> (Coord3D, Coord3D) {
        let position = self
            .listener_position
            .lock()
            .ok()
            .map(|l| *l)
            .unwrap_or_else(Coord3D::new);
        let orientation = self
            .listener_orientation
            .lock()
            .ok()
            .map(|l| *l)
            .unwrap_or(Coord3D {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            });
        (position, orientation)
    }

    fn refresh_positional_pan(&self, state: &RodioSinkState) {
        let Some(source) = state.position else {
            return;
        };
        let (listener, orientation) = self.listener_pose();
        let pan = stereo_pan(&listener, orientation.x, orientation.y, &source);
        if let Ok(sink) = state.sink.lock() {
            sink.set_stereo_pan(pan);
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
        let duration_ms = AudioManager::duration_ms_from_audio_data(audio_data.as_ref());
        let cursor = Cursor::new(CachedAudioBytes(audio_data));
        let source = Decoder::new(cursor)
            .map_err(|e| format!("Failed to decode audio file '{}': {}", file_path, e))?;
        let stream_handle = get_rodio_stream_handle()
            .ok_or_else(|| "Audio output stream not available".to_string())?;
        let (listener, orientation) = self.listener_pose();
        // C++ Miles getEffectiveVolume reads sliders already held by
        // AudioManager::update. Never re-lock THE_AUDIO from this hook.
        let sliders = get_global_audio_manager()
            .and_then(|manager| manager.try_lock().ok().map(|m| m.miles_volume_sliders()))
            .unwrap_or_default();
        let mut volume = miles_slider_volume(event, &sliders);
        let position = event
            .is_positional_audio()
            .then(|| miles_event_world_position(event));
        if event.is_positional_audio() {
            if let Some(info) = event.get_audio_event_info() {
                if info.low_pass_freq > 0.0 {
                    let on_screen = AUDIO_VIEW_RESOLVER
                        .get()
                        .map(|resolver| {
                            resolver.is_world_position_on_screen(&miles_event_world_position(event))
                        })
                        .unwrap_or(true);
                    if !on_screen {
                        // C++ AIL_set_3D_sample_occlusion(sample, 1.0 - m_lowPassFreq)
                        volume *= info.low_pass_freq.clamp(0.0, 1.0);
                    }
                }
            }
        }
        let voice = if let Some(pos) = position.as_ref() {
            let pan = stereo_pan(&listener, orientation.x, orientation.y, pos);
            let spatial = SpatialSink::try_new(
                &stream_handle,
                [pan, 0.0, -1.0],
                [-0.1, 0.0, 0.0],
                [0.1, 0.0, 0.0],
            )
            .map_err(|e| format!("Failed to create spatial audio sink: {}", e))?;
            RodioVoice::Spatial(spatial)
        } else {
            let sink = Sink::try_new(&stream_handle)
                .map_err(|e| format!("Failed to create audio sink: {}", e))?;
            RodioVoice::Flat(sink)
        };
        let pitch = event.get_effective_pitch();
        // C++ MilesAudioManager::playStream (MilesAudioManager.cpp:2762-2764)
        // AIL_set_stream_loop_count(stream, INFINITE_LOOP_COUNT) for AT_Music.
        let is_music = music_repeats_source_infinitely(event);
        let pitched = (pitch - 1.0).abs() > 0.01;
        match &voice {
            RodioVoice::Flat(sink) => {
                if is_music {
                    if pitched {
                        sink.append(source.speed(pitch).repeat_infinite());
                    } else {
                        sink.append(source.repeat_infinite());
                    }
                } else if pitched {
                    sink.append(source.speed(pitch));
                } else {
                    sink.append(source);
                }
            }
            RodioVoice::Spatial(sink) => {
                if is_music {
                    if pitched {
                        sink.append(source.speed(pitch).repeat_infinite());
                    } else {
                        sink.append(source.repeat_infinite());
                    }
                } else if pitched {
                    sink.append(source.speed(pitch));
                } else {
                    sink.append(source);
                }
            }
        }
        let (min_distance, max_distance) = miles_positional_ranges(
            event.get_audio_event_info().as_deref(),
            sliders.global_min_range,
            sliders.global_max_range,
        );
        let state = RodioSinkState {
            sink: Arc::new(Mutex::new(voice)),
            base_volume: volume,
            position,
            min_distance,
            max_distance,
            is_music,
            started_at: Instant::now(),
            duration_ms,
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
    fn set_listener_position(&self, position: &Coord3D) {
        if let Ok(mut listener) = self.listener_position.lock() {
            *listener = *position;
        }
        if let Ok(sinks) = self.sinks.lock() {
            for state in sinks.values() {
                if state.position.is_some() {
                    self.refresh_sink_volume(state);
                    self.refresh_positional_pan(state);
                }
            }
        }
    }

    fn set_listener_orientation(&self, orientation: &Coord3D) {
        if let Ok(mut stored) = self.listener_orientation.lock() {
            *stored = *orientation;
        }
        if let Ok(sinks) = self.sinks.lock() {
            for state in sinks.values() {
                if state.position.is_some() {
                    self.refresh_positional_pan(state);
                }
            }
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

    fn set_event_volume(&self, event: &AudioEventRts) {
        let handle = event.get_playing_handle();
        let mut sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get_mut(&handle) else {
            return;
        };

        let sliders = get_global_audio_manager()
            .and_then(|manager| manager.try_lock().ok().map(|m| m.miles_volume_sliders()))
            .unwrap_or_default();
        state.base_volume = miles_slider_volume(event, &sliders);
        if event.is_positional_audio() {
            state.position = Some(miles_event_world_position(event));
        }
        self.refresh_sink_volume(state);
        self.refresh_positional_pan(state);
    }

    fn set_sink_volume(&self, handle: AudioHandle, volume: Real) {
        let mut sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get_mut(&handle) else {
            return;
        };
        state.base_volume = volume.clamp(0.0, 1.0);
        if let Ok(sink) = state.sink.lock() {
            sink.set_volume(state.base_volume);
        }
    }

    fn music_loop_count(&self, handle: AudioHandle) -> Int {
        let sinks = self.sinks.lock().unwrap();
        let Some(state) = sinks.get(&handle) else {
            return 0;
        };
        if !state.is_music {
            return 0;
        }
        let Some(duration) = state.duration_ms.filter(|ms| *ms > 0.0) else {
            return 0;
        };
        let elapsed_ms = state.started_at.elapsed().as_secs_f32() * 1000.0;
        (elapsed_ms / duration) as Int
    }

    fn is_sink_paused(&self, handle: AudioHandle) -> bool {
        let sinks = self.sinks.lock().unwrap();
        sinks
            .get(&handle)
            .and_then(|state| state.sink.lock().ok().map(|sink| sink.is_paused()))
            .unwrap_or(false)
    }
}

pub fn register_rodio_playback_hook() -> bool {
    let hook = Arc::new(RodioPlaybackHook::new());
    register_sound_playback_hook(hook)
}

pub const AHSV_NO_SOUND: AudioHandle = 0x0000_0000;
pub const AHSV_ERROR: AudioHandle = 0xFFFF_FFFF;
pub const AHSV_NOT_FOR_LOCAL: AudioHandle = 0xFFFF_FFFE;
pub const AHSV_MUTED: AudioHandle = 0xFFFF_FFFD;
pub const AHSV_STOP_THE_MUSIC: AudioHandle = 0xFFFF_FFF0;
pub const AHSV_STOP_THE_MUSIC_FADE: AudioHandle = 0xFFFF_FFF1;
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

    let mut created = AudioManager::new();
    // C++ `AudioManager::init` (GameAudio.cpp) copies preferred slider volumes.
    // `new()` leaves effective music/sound/speech volumes at 0 until init.
    created.init();
    let manager = Arc::new(Mutex::new(created));
    if THE_AUDIO.set(manager.clone()).is_err() {
        THE_AUDIO.get().expect("THE_AUDIO set but missing").clone()
    } else {
        register_rodio_playback_hook();
        register_animation_sound_library(manager.clone());
        // Parsers register via THE_AUDIO; load after the singleton is published.
        load_audio_event_inis();
        manager
    }
}

/// Access the global audio manager if it has been initialised.
pub fn get_global_audio_manager() -> Option<Arc<Mutex<AudioManager>>> {
    THE_AUDIO.get().cloned()
}

/// C++ `AudioManager::init` (GameAudio.cpp:186-202) loads AudioSettings.ini,
/// then Music / SoundEffects / Speech / Voice (Default then override), then
/// MiscAudio.ini. AudioSettings drive sample counts, volumes, zoom, and cache
/// size; MiscAudio names UI clicks, radar, sabotage, and other global cues.
pub fn load_audio_event_inis() {
    const AUDIO_INI_FILES: &[&str] = &[
        "Data/INI/AudioSettings.ini",
        "Data/INI/Default/Music.ini",
        "Data/INI/Music.ini",
        "Data/INI/Default/SoundEffects.ini",
        "Data/INI/SoundEffects.ini",
        "Data/INI/Default/Speech.ini",
        "Data/INI/Speech.ini",
        "Data/INI/Default/Voice.ini",
        "Data/INI/Voice.ini",
        "Data/INI/MiscAudio.ini",
    ];

    let mut ini = crate::common::ini::INI::new();
    for virtual_path in AUDIO_INI_FILES {
        let Some(path) = crate::common::system::install_layout::resolve_data_ini_file(virtual_path)
        else {
            continue;
        };
        if let Err(err) = ini.load(&path, crate::common::ini::INILoadType::Overwrite) {
            eprintln!("Failed to load audio INI '{}': {err}", path.display());
        }
    }

    apply_loaded_audio_settings_to_manager();
}

/// Copy parsed AudioSettings.ini / MiscAudio.ini into the live AudioManager.
fn apply_loaded_audio_settings_to_manager() {
    let Some(manager) = get_global_audio_manager() else {
        return;
    };
    let Ok(mut guard) = manager.lock() else {
        return;
    };

    if let Some(parsed) = crate::common::ini::ini_audio_settings::get_audio_settings() {
        let src = parsed.read();
        guard.audio_settings.audio_root = src.audio_root.clone();
        guard.audio_settings.sounds_folder = src.sounds_folder.clone();
        guard.audio_settings.music_folder = src.music_folder.clone();
        guard.audio_settings.streaming_folder = src.streaming_folder.clone();
        guard.audio_settings.sounds_extension = src.sounds_extension.clone();
        guard.audio_settings.use_digital = src.use_digital;
        guard.audio_settings.use_midi = src.use_midi;
        guard.audio_settings.output_rate = src.output_rate;
        guard.audio_settings.output_bits = src.output_bits;
        guard.audio_settings.output_channels = src.output_channels;
        guard.audio_settings.sample_count_2d = src.sample_count_2d;
        guard.audio_settings.sample_count_3d = src.sample_count_3d;
        guard.audio_settings.stream_count = src.stream_count;
        guard.audio_settings.min_volume = src.min_volume;
        guard.audio_settings.global_min_range = src.global_min_range;
        guard.audio_settings.global_max_range = src.global_max_range;
        guard.audio_settings.drawable_ambient_frames = src.drawable_ambient_frames;
        guard.audio_settings.fade_audio_frames = src.fade_audio_frames;
        guard.audio_settings.max_cache_size = src.max_cache_size;
        guard.audio_settings.relative_2d_volume = src.relative_2d_volume;
        guard.audio_settings.default_sound_volume = src.default_sound_volume;
        guard.audio_settings.default_3d_sound_volume = src.default_3d_sound_volume;
        guard.audio_settings.default_speech_volume = src.default_speech_volume;
        guard.audio_settings.default_music_volume = src.default_music_volume;
        guard.audio_settings.preferred_sound_volume = src.preferred_sound_volume;
        guard.audio_settings.preferred_3d_sound_volume = src.preferred_3d_sound_volume;
        guard.audio_settings.preferred_speech_volume = src.preferred_speech_volume;
        guard.audio_settings.preferred_music_volume = src.preferred_music_volume;
        guard.audio_settings.microphone_desired_height_above_terrain =
            src.microphone_desired_height_above_terrain;
        guard
            .audio_settings
            .microphone_max_percentage_between_ground_and_camera =
            src.microphone_max_percentage_between_ground_and_camera;
        guard.audio_settings.zoom_min_distance = src.zoom_min_distance;
        guard.audio_settings.zoom_max_distance = src.zoom_max_distance;
        guard.audio_settings.zoom_sound_volume_percentage_amount =
            src.zoom_sound_volume_percentage_amount;
        guard.init();
    }

    if let Some(parsed) = crate::common::ini::ini_misc_audio::get_misc_audio() {
        let src = parsed.read();
        guard.misc_audio.ui_sounds.clear();
        let insert =
            |map: &mut std::collections::HashMap<String, AudioEventRts>,
             name: &str,
             event: &crate::common::ini::ini_misc_audio::AudioEventRTS| {
                let event_name = event.playable_event_name();
                if !event_name.is_empty() {
                    map.insert(name.to_string(), AudioEventRts::with_event_name(event_name));
                }
            };
        insert(
            &mut guard.misc_audio.ui_sounds,
            "RadarNotifyUnitUnderAttackSound",
            &src.radar_unit_under_attack_sound,
        );
        insert(
            &mut guard.misc_audio.ui_sounds,
            "RadarNotifyHarvesterUnderAttackSound",
            &src.radar_harvester_under_attack_sound,
        );
        insert(
            &mut guard.misc_audio.ui_sounds,
            "RadarNotifyStructureUnderAttackSound",
            &src.radar_structure_under_attack_sound,
        );
        insert(
            &mut guard.misc_audio.ui_sounds,
            "RadarNotifyInfiltrationSound",
            &src.radar_infiltration_sound,
        );
    }
}
