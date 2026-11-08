////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameAudio - Main audio system manager
//! Westwood Studios Pacific
//! Converted to Rust

mod playback;
mod audio_length;
mod music_tracks;
mod locality_limits;
mod playing_list;
mod rodio_hook;
mod animation_bridge;
#[cfg(not(target_arch = "wasm32"))]
pub use rodio_hook::register_rodio_playback_hook;
pub use animation_bridge::{
    get_global_audio_manager, initialize_global_audio_manager, load_audio_event_inis,
    register_animation_sound_library,
};
use animation_bridge::{
    apply_loaded_audio_settings_to_manager, read_audio_settings_from_game_fs, AnimatedSoundBridge,
};
#[cfg(not(target_arch = "wasm32"))]
use rodio_hook::{
    get_rodio_stream_handle, live_audio_file_cache, wav_channel_count, CachedAudioBytes,
    RodioPlaybackHook, RodioSinkState, RodioVoice,
};
#[cfg(target_arch = "wasm32")]
use rodio_hook::{register_wasm_playback_hook, WasmPlaybackHook};

use super::audio_paths::AudioPathSettings;
use crate::common::audio::{
    audio_cache::AudioFileCache,
    audio_event_rts::{
        miles_event_world_position, miles_get_effective_volume, miles_positional_gain,
        miles_positional_ranges, AudioEventInfo, AudioEventRts, AudioHandle, AudioPriority,
        AudioType, Coord3D, MilesVolumeSliders, ObjectId, PortionToPlay, AC_INTERRUPT, AC_LOOP,
        ST_GLOBAL,
    },
    audio_request::{AudioRequest, RequestType},
    game_music::create_music_manager,
    game_sounds::{create_sound_manager, PlayNowAudioQueries},
    rodio_spatial::{miles_slider_volume, stereo_pan},
};
use crate::common::game_common::MSEC_PER_LOGICFRAME_REAL;
use crate::common::system::file::FileAccess;
use crate::common::system::file_system::get_file_system;
use glam::Mat4;
use hound::WavReader;
use indexmap::IndexMap;
use lewton::inside_ogg::OggStreamReader;
#[cfg(not(target_arch = "wasm32"))]
use minimp3::{Decoder as Mp3Decoder, Error as Mp3Error};
#[cfg(not(target_arch = "wasm32"))]
use rodio_compat::{Decoder, OutputStream, OutputStreamHandle, Sink, Source, SpatialSink};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Instant;
use ww3d_animation::{initialize_animated_sound_mgr, set_sound_library, SoundLibraryBridge};
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
    fn play_with_visibility(&self, event: &AudioEventRts, _on_screen: bool) -> Result<(), String> {
        self.play(event)
    }
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

// SOUND_PLAYBACK_HOOK → SystemStores.sound_playback_hook

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

/// Player facts frozen by the driving instance before audio submission.
/// No player or object lookup is needed while the audio manager admits the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSubmissionLocality {
    owning_player_index: Int,
    owning_player_exists: Bool,
    local_player_index: Option<Int>,
    local_player_active: Bool,
    observer_look_at: Option<Int>,
    local_exists_and_has_team: Bool,
    relationship_to_local: AudioLocalityRelationship,
}

impl AudioSubmissionLocality {
    pub fn from_resolver(owning_player_index: Int, resolver: &dyn AudioLocalityResolver) -> Self {
        let owning_player_exists = resolver.player_exists(owning_player_index);
        let Some(mut local_player_index) = resolver.get_local_player_index() else {
            return Self {
                owning_player_index,
                owning_player_exists,
                local_player_index: None,
                local_player_active: false,
                observer_look_at: resolver.get_observer_look_at_player_index(),
                local_exists_and_has_team: false,
                relationship_to_local: AudioLocalityRelationship::Neutral,
            };
        };
        let local_player_active = resolver.is_player_active(local_player_index);
        if !local_player_active {
            if let Some(observer) = resolver.get_observer_look_at_player_index() {
                local_player_index = observer;
            }
        }
        let local_exists_and_has_team = resolver.player_exists(local_player_index)
            && resolver.has_default_team(local_player_index);
        let relationship_to_local =
            resolver.get_relationship_to_local_team(owning_player_index, local_player_index);
        Self {
            owning_player_index,
            owning_player_exists,
            local_player_index: Some(local_player_index),
            local_player_active,
            observer_look_at: resolver.get_observer_look_at_player_index(),
            local_exists_and_has_team,
            relationship_to_local,
        }
    }

    fn should_play_locally(&self, event_info: &AudioEventInfo) -> Bool {
        should_play_locally_for_players(
            event_info.type_field,
            event_info.sound_type == AudioType::Music,
            self.owning_player_index,
            self.owning_player_exists,
            self.local_player_index,
            self.local_player_active,
            self.observer_look_at,
            self.local_exists_and_has_team,
            self.relationship_to_local,
        )
    }
}

enum AudioSubmission<'a> {
    Ambient(Option<Int>),
    Explicit(&'a AudioSubmissionLocality),
}

// AUDIO_LOCALITY_RESOLVER → SystemStores.audio_locality_resolver

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

include!("game_audio_view.rs");

// AUDIO_VIEW_RESOLVER → SystemStores.audio_view_resolver

pub fn register_audio_view_resolver(resolver: Arc<dyn AudioViewResolver>) -> bool {
    let services = crate::common::common_services::common_services();
    let mut slot = services
        .system
        .audio_view_resolver
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let first = slot.is_none();
    *slot = Some(resolver);
    first
}

pub fn register_sound_playback_hook(hook: Arc<dyn SoundPlaybackHook>) -> bool {
    let services = crate::common::common_services::common_services();
    let mut slot = services
        .system
        .sound_playback_hook
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let first = slot.is_none();
    *slot = Some(hook);
    first
}

pub fn sound_playback_hook_registered() -> bool {
    crate::common::common_services::common_services()
        .system
        .sound_playback_hook
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .is_some()
}

pub fn register_audio_locality_resolver(resolver: Arc<dyn AudioLocalityResolver>) -> bool {
    let services = crate::common::common_services::common_services();
    let mut slot = services
        .system
        .audio_locality_resolver
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let first = slot.is_none();
    *slot = Some(resolver);
    first
}

pub(crate) fn with_sound_playback_hook<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&dyn SoundPlaybackHook) -> R,
{
    let hook = crate::common::common_services::common_services()
        .system
        .sound_playback_hook
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    hook.map(|hook| f(hook.as_ref()))
}

// Session-wide play-failure reports (event name -> reason).
//
// C++ surfaced missing audio with `DEBUG_ASSERTLOG("Missing Audio File:
// '%s'")` at `AudioFileCache::openFile` (MilesAudioManager.cpp:3134). The
// Rust play path previously discarded `hook.play` Err strings and unresolved
// addAudioEvent handles, so a fully mute game was log-identical to a working
// one. Each failing event name logs once per session (rate limit) instead of
// once per frame.
//
/// Report a TheAudio play failure for `event_name`, rate-limited to one
/// `warn!` per event name per session. Returns true when this call logged.
fn report_play_failure_once(event_name: &str, reason: &str) -> Bool {
    let services = crate::common::common_services::common_services();
    let mut reports = services
        .system
        .play_failure_reports
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if reports.contains_key(event_name) {
        return false;
    }
    reports.insert(event_name.to_string(), reason.to_string());
    log::warn!("TheAudio play failed {event_name}: {reason}");
    true
}

#[cfg(test)]
fn reset_play_failure_reports_for_tests() {
    crate::common::common_services::common_services()
        .system
        .play_failure_reports
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
}

#[cfg(test)]
fn play_failure_report_reason_for_tests(event_name: &str) -> Option<AsciiString> {
    crate::common::common_services::common_services()
        .system
        .play_failure_reports
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(event_name)
        .cloned()
}

fn with_audio_locality_resolver<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&dyn AudioLocalityResolver) -> R,
{
    let resolver = crate::common::common_services::common_services()
        .system
        .audio_locality_resolver
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();
    resolver.map(|resolver| f(resolver.as_ref()))
}

fn audio_view_resolver() -> Option<Arc<dyn AudioViewResolver>> {
    crate::common::common_services::common_services()
        .system
        .audio_view_resolver
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
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
    fn can_play_now_checked_lazy(
        &self,
        event: &mut AudioEventRts,
        queries: &PlayNowAudioQueries,
        violates_limit: &mut dyn FnMut(&mut AudioEventRts) -> bool,
    ) -> bool {
        let mut queries = *queries;
        queries.violates_limit = violates_limit(event);
        self.can_play_now_checked(event, &queries)
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
    /// Optional receiving backend. Absence retains the legacy installed backend.
    playback_recipient: playback::PlaybackRecipient,
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
    active_audio_events: IndexMap<AudioHandle, AudioEventRts>,
    music_tracks: Vec<AsciiString>,
    current_music_track: AsciiString,

    // Audio event registry
    all_audio_event_info: HashMap<AsciiString, Arc<AudioEventInfo>>,
    /// Per-manager audio file duration cache (was process FILE_LENGTH_CACHE).
    file_length_cache: Mutex<HashMap<AsciiString, Real>>,
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
            playback_recipient: playback::PlaybackRecipient::default(),
            audio_settings: AudioSettings::default(),
            misc_audio: MiscAudio::default(),
            music_manager: Some(create_music_manager()),
            sound_manager: Some(create_sound_manager()),
            listener_position: Coord3D::ZERO,
            listener_orientation: Coord3D {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            audio_requests: Vec::new(),
            fading_audio: Vec::new(),
            active_audio_events: IndexMap::new(),
            music_tracks: Vec::new(),
            current_music_track: String::new(),
            all_audio_event_info: HashMap::new(),
            file_length_cache: Mutex::new(HashMap::new()),
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

    /// Bind an already-owned backend without publishing a process recipient.
    /// Construction remains inert; init and update retain their existing roles.
    pub fn with_playback_hook(recipient: Box<dyn SoundPlaybackHook>) -> Self {
        let mut manager = Self::new();
        manager.playback_recipient = playback::PlaybackRecipient::owned(recipient);
        manager
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

    /// Retire receipts belonging to one receiving native owner, without
    /// resetting another session's device events, requests or script volume.
    /// Application teardown still uses reset()/shutdown for the whole service.
    pub fn retire_audio_handles(&mut self, handles: &[AudioHandle]) {
        if handles.is_empty() {
            return;
        }
        for &handle in handles {
            if handle >= AHSV_FIRST_HANDLE && handle < 0xFFFF_FFF0 {
                let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
            }
        }
        self.active_audio_events
            .retain(|handle, _| !handles.contains(handle));
        self.fading_audio
            .retain(|fade| !handles.contains(&fade.event.get_playing_handle()));
        self.audio_force_played
            .retain(|event| !handles.contains(&event.get_playing_handle()));
        self.audio_requests.retain(|request| {
            let handle = request.get_handle().or_else(|| {
                request
                    .get_pending_event()
                    .map(AudioEventRts::get_playing_handle)
            });
            !handle.is_some_and(|handle| handles.contains(&handle))
        });
    }

    pub fn reset(&mut self) {
        // Stop all actively playing sounds through the backend before clearing bookkeeping.
        let handles: Vec<AudioHandle> = self.active_audio_events.keys().copied().collect();
        for handle in handles {
            let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
        }
        // C++ MilesAudioManager::reset/stopAllAudioImmediately also releases
        // m_fadingAudio. These streams have already left active_audio_events.
        for fading in &self.fading_audio {
            let handle = fading.event.get_playing_handle();
            let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
        }
        let force_handles: Vec<AudioHandle> = self
            .audio_force_played
            .iter()
            .map(|event| event.get_playing_handle())
            .collect();
        for handle in force_handles {
            let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
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

    /// Add an audio event to be played
    pub fn add_audio_event(&mut self, event_to_add: &AudioEventRts) -> AudioHandle {
        self.add_audio_event_with_submission(event_to_add, AudioSubmission::Ambient(None))
    }

    /// Submit from an already-borrowed object. C++ reads the owner's player
    /// during shouldPlayLocally; the driving simulation supplies that same
    /// fact without an ambient lookup of its write-locked object. Queued audio
    /// keeps the original object ID and its dynamic position/owner resolution.
    pub fn add_audio_event_for_player(
        &mut self,
        event_to_add: &AudioEventRts,
        owner_player_index: Int,
    ) -> AudioHandle {
        self.add_audio_event_with_submission(
            event_to_add,
            AudioSubmission::Ambient(Some(owner_player_index)),
        )
    }

    /// Admit an event using facts already captured from its driving instance.
    /// The queued event retains its original object and player identity.
    pub fn add_audio_event_with_locality(
        &mut self,
        event_to_add: &AudioEventRts,
        locality: &AudioSubmissionLocality,
    ) -> AudioHandle {
        self.add_audio_event_with_submission(event_to_add, AudioSubmission::Explicit(locality))
    }

    /// FX admission through receiving player facts and sound-selection streams.
    pub fn add_audio_event_with_locality_and_selection(
        &mut self,
        event_to_add: &AudioEventRts,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn super::audio_event_rts::AudioEventSelection,
    ) -> AudioHandle {
        self.add_audio_event_with_submission_and_selection(
            event_to_add,
            AudioSubmission::Explicit(locality),
            selection,
            false,
            None,
        )
    }

    pub fn add_audio_event_with_policy(
        &mut self,
        event: &AudioEventRts,
        locality: &AudioSubmissionLocality,
        selection: &mut dyn super::audio_event_rts::AudioEventSelection,
        policy: &super::script_policy::ScriptAudioPolicy,
        prepared: bool,
    ) -> AudioHandle {
        self.add_audio_event_with_submission_and_selection(
            event,
            AudioSubmission::Explicit(locality),
            selection,
            prepared,
            Some(policy),
        )
    }

    fn add_audio_event_with_submission(
        &mut self,
        event_to_add: &AudioEventRts,
        submission: AudioSubmission<'_>,
    ) -> AudioHandle {
        self.add_audio_event_with_submission_and_selection(
            event_to_add,
            submission,
            &mut super::audio_event_rts::AmbientAudioSelection,
            false,
            None,
        )
    }

    /// Resolve variation at the originating callback without publishing a play
    /// request. A detached native map keeps this owned event until commit.
    pub fn prepare_audio_event_with_selection(
        &self,
        event: &AudioEventRts,
        selection: &mut dyn super::audio_event_rts::AudioEventSelection,
    ) -> Result<Option<AudioEventRts>, String> {
        let mut event = event.clone();
        if event.get_audio_event_info().is_none() {
            let info = self
                .find_audio_event_info(event.get_event_name())
                .ok_or_else(|| {
                    format!("unknown receiving FX sound '{}'", event.get_event_name())
                })?;
            event.set_volume(info.volume);
            event.set_audio_event_info(info);
        }
        let sound_type = event.get_audio_event_info().unwrap().sound_type;
        let enabled = match sound_type {
            AudioType::Music => self.is_on(AudioAffect::Music),
            AudioType::SoundEffect => {
                self.is_on(AudioAffect::Sound) && self.is_on(AudioAffect::Sound3D)
            }
            AudioType::Streaming => self.is_on(AudioAffect::Speech) && !self.disallow_speech,
        };
        if !enabled {
            return Ok(None);
        }
        let paths = self.audio_paths();
        event.generate_filename_with_selection(&paths, selection);
        event.generate_play_info_with_selection(&paths, selection);
        Ok(Some(event))
    }
    /// Admit an already selected event without another instance's random draw.
    pub fn add_prepared_audio_event_with_locality(
        &mut self,
        event: &AudioEventRts,
        locality: &AudioSubmissionLocality,
    ) -> AudioHandle {
        self.add_audio_event_with_submission_and_selection(
            event,
            AudioSubmission::Explicit(locality),
            &mut super::audio_event_rts::AmbientAudioSelection,
            true,
            None,
        )
    }

    fn add_audio_event_with_submission_and_selection(
        &mut self,
        event_to_add: &AudioEventRts,
        submission: AudioSubmission<'_>,
        selection: &mut dyn super::audio_event_rts::AudioEventSelection,
        prepared: bool,
        policy: Option<&super::script_policy::ScriptAudioPolicy>,
    ) -> AudioHandle {
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
            // C++ addAudioEvent returns AHSV_Error for unknown names
            // (GameAudio.cpp:393-395); report once so a silent event name is
            // visible in the log instead of a dropped handle.
            report_play_failure_once(
                audio_event.get_event_name(),
                "addAudioEvent: event name unresolved",
            );
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
        let paths = self.audio_paths();
        if !prepared {
            audio_event.generate_filename_with_selection(&paths, selection);
            // C++ GameAudio.cpp:424 writes the chosen index back onto the caller's event.
            event_to_add.set_playing_audio_index(audio_event.get_playing_audio_index());
            audio_event.generate_play_info_with_selection(&paths, selection);
        }

        if let Some(policy) = policy {
            audio_event.owned_script_volumes = Some(policy.volumes);
            if let Some(volume) = policy.override_for(audio_event.get_event_name()) {
                audio_event.set_volume(volume);
            }
        } else if audio_event.owned_script_volumes.is_none() {
            for (name, volume) in &self.adjusted_volumes {
                if *name == audio_event.get_event_name() {
                    audio_event.set_volume(*volume);
                    break;
                }
            }
        }

        if !audio_event.get_uninterruptable()
            && !match submission {
                AudioSubmission::Ambient(owner_player_index) => {
                    self.should_play_locally_for_player(&audio_event, owner_player_index)
                }
                AudioSubmission::Explicit(locality) => locality.should_play_locally(&resolved_info),
            }
        {
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
            let queries = self.play_now_queries(&audio_event, false);
            let allowed = if let Some(sound_mgr) = &self.sound_manager {
                sound_mgr.can_play_now_checked_lazy(&mut audio_event, &queries, &mut |event| {
                    self.does_violate_limit(event)
                })
            } else {
                !self.does_violate_limit(&mut audio_event)
            };
            if !allowed {
                return AHSV_NO_SOUND;
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
                self.active_audio_events.shift_remove(&handle);
                let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
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

        self.active_audio_events.shift_remove(&audio_event);
        let request = AudioRequest::new_with_handle(RequestType::Stop, audio_event);
        self.append_audio_request(request);
    }

    pub fn is_currently_playing(&self, audio_event: AudioHandle) -> bool {
        if audio_event < 1000 {
            return false;
        }

        self.playback_recipient
            .with_hook(|hook| hook.is_playing(audio_event))
            .unwrap_or(false)
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

    /// Same observation, limited to playback receipts admitted by one
    /// receiving session. The shared device may contain peers with identical
    /// object ids and event names.
    pub fn is_owned_named_event_playing_for_object(
        &self,
        object_id: u32,
        event_name: &str,
        receipts: &[AudioHandle],
    ) -> bool {
        if object_id == 0 || event_name.is_empty() {
            return false;
        }
        self.active_audio_events.iter().any(|(handle, event)| {
            receipts.contains(handle)
                && event.get_event_name() == event_name
                && event.get_object_id() == object_id
        })
    }

    /// Test hook: pretend Miles is still playing this object-owned event.
    pub fn test_insert_active_named_event(&mut self, object_id: u32, event_name: &str) {
        let mut event = AudioEventRts::with_event_name(event_name);
        event.set_object_id(object_id);
        event.set_playing_handle(1001);
        self.active_audio_events.insert(1001, event);
    }

    /// Install a playing event for a hardware-free integration fixture.
    #[doc(hidden)]
    pub fn test_insert_active_audio_event(&mut self, event: AudioEventRts) {
        self.active_audio_events.insert(event.get_playing_handle(), event);
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

    fn event_volume_sliders(&self, event: &AudioEventRts) -> MilesVolumeSliders {
        let Some(owner) = event.owned_script_volumes else {
            return self.miles_volume_sliders();
        };
        MilesVolumeSliders {
            music_volume: self.system_music_volume * owner.music,
            speech_volume: self.system_speech_volume * owner.speech,
            sound_volume: self.system_sound_volume * owner.sound,
            sound_3d_volume: self.system_sound_3d_volume * owner.sound_3d,
            global_min_range: self.audio_settings.global_min_range as f32,
            global_max_range: self.audio_settings.global_max_range as f32,
        }
    }

    /// C++ `MilesAudioManager::getEffectiveVolume`.
    pub fn get_effective_volume(&self, event: &AudioEventRts) -> Real {
        miles_get_effective_volume(
            event,
            &self.listener_position,
            &self.event_volume_sliders(event),
        )
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
        let _ = self.playback_recipient.with_hook(|hook| {
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
        self.process_request_list_with_visibility(&mut legacy_audio_visibility);
    }
    fn process_request_list_with_visibility(&mut self, visible: &mut dyn FnMut(&Coord3D) -> bool) {
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
            self.process_request(request, visible);
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
        let queries = self.play_now_queries(event, false);
        if let Some(sound_mgr) = &self.sound_manager {
            sound_mgr.can_play_now_checked_lazy(event, &queries, &mut |event| {
                self.does_violate_limit(event)
            })
        } else {
            self.does_violate_limit(event);
            true
        }
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
    fn process_request(
        &mut self,
        request: AudioRequest,
        visible: &mut dyn FnMut(&Coord3D) -> bool,
    ) {
        match request.request {
            RequestType::Stop => {
                if let Some(handle) = request.get_handle() {
                    self.stop_playing_handle(handle);
                }
            }
            RequestType::Play => {
                if let Some(event) = request.take_pending_event() {
                    self.play_audio_event(event, visible);
                }
            }
            RequestType::Pause => {
                if let Some(handle) = request.get_handle() {
                    let _ = self.playback_recipient.with_hook(|hook| {
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

    fn audio_paths(&self) -> AudioPathSettings {
        AudioPathSettings::from_settings(&self.audio_settings)
    }

    /// Apply parsed settings to this receiving manager and initialize its mix.
    pub fn apply_receiving_audio_settings(
        &mut self,
        src: &crate::common::ini::ini_audio_settings::AudioSettings,
    ) {
        self.audio_settings.audio_root = src.audio_root.clone();
        self.audio_settings.sounds_folder = src.sounds_folder.clone();
        self.audio_settings.music_folder = src.music_folder.clone();
        self.audio_settings.streaming_folder = src.streaming_folder.clone();
        self.audio_settings.sounds_extension = src.sounds_extension.clone();
        self.audio_settings.use_digital = src.use_digital;
        self.audio_settings.use_midi = src.use_midi;
        self.audio_settings.output_rate = src.output_rate;
        self.audio_settings.output_bits = src.output_bits;
        self.audio_settings.output_channels = src.output_channels;
        self.audio_settings.sample_count_2d = src.sample_count_2d;
        self.audio_settings.sample_count_3d = src.sample_count_3d;
        self.audio_settings.stream_count = src.stream_count;
        self.audio_settings.min_volume = src.min_volume;
        self.audio_settings.global_min_range = src.global_min_range;
        self.audio_settings.global_max_range = src.global_max_range;
        self.audio_settings.drawable_ambient_frames = src.drawable_ambient_frames;
        self.audio_settings.fade_audio_frames = src.fade_audio_frames;
        self.audio_settings.max_cache_size = src.max_cache_size;
        self.audio_settings.relative_2d_volume = src.relative_2d_volume;
        self.audio_settings.default_sound_volume = src.default_sound_volume;
        self.audio_settings.default_3d_sound_volume = src.default_3d_sound_volume;
        self.audio_settings.default_speech_volume = src.default_speech_volume;
        self.audio_settings.default_music_volume = src.default_music_volume;
        self.audio_settings.preferred_sound_volume = src.preferred_sound_volume;
        self.audio_settings.preferred_3d_sound_volume = src.preferred_3d_sound_volume;
        self.audio_settings.preferred_speech_volume = src.preferred_speech_volume;
        self.audio_settings.preferred_music_volume = src.preferred_music_volume;
        self.audio_settings.microphone_desired_height_above_terrain =
            src.microphone_desired_height_above_terrain;
        self.audio_settings
            .microphone_max_percentage_between_ground_and_camera =
            src.microphone_max_percentage_between_ground_and_camera;
        self.audio_settings.zoom_min_distance = src.zoom_min_distance;
        self.audio_settings.zoom_max_distance = src.zoom_max_distance;
        self.audio_settings.zoom_sound_volume_percentage_amount =
            src.zoom_sound_volume_percentage_amount;
        self.init();
    }

    /// Apply the receiving world's parsed MiscAudio UI mappings.
    pub fn apply_receiving_misc_audio(
        &mut self,
        src: &crate::common::ini::ini_misc_audio::MiscAudio,
    ) {
        self.misc_audio.ui_sounds.clear();
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
            &mut self.misc_audio.ui_sounds,
            "RadarNotifyUnitUnderAttackSound",
            &src.radar_unit_under_attack_sound,
        );
        insert(
            &mut self.misc_audio.ui_sounds,
            "RadarNotifyHarvesterUnderAttackSound",
            &src.radar_harvester_under_attack_sound,
        );
        insert(
            &mut self.misc_audio.ui_sounds,
            "RadarNotifyStructureUnderAttackSound",
            &src.radar_structure_under_attack_sound,
        );
        insert(
            &mut self.misc_audio.ui_sounds,
            "RadarNotifyInfiltrationSound",
            &src.radar_infiltration_sound,
        );
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
        let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
    }

    /// C++ `MilesAudioManager::releasePlayingAudio` — hard-stop the sample and drop it.
    fn release_playing_handle(&mut self, handle: AudioHandle) {
        if handle < AHSV_FIRST_HANDLE {
            return;
        }
        if let Some(event) = self.active_audio_events.shift_remove(&handle) {
            self.notify_sample_completion_if_effect(&event);
        }
        let _ = self.playback_recipient.with_hook(|hook| hook.stop(handle));
    }

    fn play_audio_event(
        &mut self,
        mut event: AudioEventRts,
        visible: &mut dyn FnMut(&Coord3D) -> bool,
    ) {
        let Some(info) = event.get_audio_event_info() else {
            report_play_failure_once(
                event.get_event_name(),
                "playAudioEvent: unresolved event info",
            );
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

        event.playback_sliders = Some(self.event_volume_sliders(&event));
        let hook_result = self.playback_recipient.with_hook(|hook| {
            hook.play_with_visibility(&event, playback_visibility(&event, visible))
        });
        let play_ok = match hook_result {
            Some(Ok(())) | None => true,
            Some(Err(msg)) => {
                report_play_failure_once(event.get_event_name(), &msg);
                false
            }
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
            report_play_failure_once(
                event_to_play.get_event_name(),
                "friend_forcePlayAudioEventRTS: unresolved event info",
            );
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

        let paths = self.audio_paths();
        event.generate_filename_with_settings(&paths);
        event.generate_play_info_with_settings(&paths);
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

        event.playback_sliders = Some(self.event_volume_sliders(&event));
        let hook_result = self.playback_recipient.with_hook(|hook| hook.play(&event));
        let play_ok = match hook_result {
            Some(Ok(())) | None => true,
            Some(Err(msg)) => {
                report_play_failure_once(event.get_event_name(), &msg);
                false
            }
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
        self.audio_handle_pool = self.audio_handle_pool.wrapping_add(1);
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

        let _ = self.playback_recipient.with_hook(|hook| {
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

        if self
            .playback_recipient
            .with_hook(|hook| {
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


    /// Actual pending AR_Play outputs in admission order, without copying or draining.
    pub fn pending_play_events(&self) -> impl Iterator<Item = &AudioEventRts> {
        self.audio_requests
            .iter()
            .filter(|request| request.request == RequestType::Play)
            .filter_map(AudioRequest::get_pending_event)
    }

    pub fn pending_play_request_count(&self) -> usize {
        self.audio_requests
            .iter()
            .filter(|request| request.request == RequestType::Play)
            .count()
    }

    /// Queued AR_Play request count for a single event name (diagnostic
    /// seam: did TheAudio accept this event?).
    pub fn pending_play_request_count_for(&self, event_name: &str) -> usize {
        self.audio_requests
            .iter()
            .filter(|request| {
                request.request == RequestType::Play
                    && request
                        .get_pending_event()
                        .is_some_and(|event| event.get_event_name() == event_name)
            })
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
        let Some(mut event) = self.active_audio_events.shift_remove(&handle) else {
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
        let _ = self.playback_recipient.with_hook(|hook| {
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


pub const AHSV_NO_SOUND: AudioHandle = 0x0000_0000;
pub const AHSV_ERROR: AudioHandle = 0xFFFF_FFFF;
pub const AHSV_NOT_FOR_LOCAL: AudioHandle = 0xFFFF_FFFE;
pub const AHSV_MUTED: AudioHandle = 0xFFFF_FFFD;
pub const AHSV_STOP_THE_MUSIC: AudioHandle = 0xFFFF_FFF0;
pub const AHSV_STOP_THE_MUSIC_FADE: AudioHandle = 0xFFFF_FFF1;
const AHSV_FIRST_HANDLE: AudioHandle = 1000;


#[cfg(test)]
mod audio_settings_archive_tests {
    use super::*;
    use crate::common::system::big_file_system::BigArchiveBackend;
    use crate::common::system::file_system::FileSystemBackend as _;
    use crate::common::system::local_file_system::LocalFileSystem;
    use crate::common::system::subsystem_interface::SubsystemInterface as CommonSubsystemInterface;

    fn assets_root() -> Option<std::path::PathBuf> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Main/assets");
        root.join("INIZH.big").is_file().then_some(root)
    }

    /// (c) When the loose AudioSettings.ini fails to parse, the retail
    /// archive copy (INIZH.big `Data\INI\AudioSettings.ini`) must drive the
    /// live mix values instead of the compiled defaults.
    #[test]
    fn archived_audio_settings_apply_retail_mix_values() {
        let Some(assets) = assets_root() else {
            eprintln!("skipping: no INIZH.big under Main/assets");
            return;
        };
        {
            let file_system = get_file_system();
            let mut guard = file_system.lock().expect("FileSystem lock");
            let local: &mut LocalFileSystem = guard.ensure_backend(LocalFileSystem::new);
            local.add_search_path(&assets);
            let big: &mut BigArchiveBackend = guard.ensure_backend(BigArchiveBackend::new);
            big.add_search_path(&assets);
            big.reset();
            guard.clear_cache();
            let _ = CommonSubsystemInterface::init(&mut *guard);
        }

        load_audio_event_inis();

        let settings = crate::common::ini::ini_audio_settings::get_audio_settings()
            .expect("AudioSettings must be parsed (loose or archived)");
        let settings = settings.read();
        // Retail AudioSettings.ini (INIZH.big): SampleCount2D = 4,
        // SampleCount3D = 25, StreamCount = 3.
        assert_eq!(settings.sample_count_2d, 4);
        assert_eq!(settings.sample_count_3d, 25);
        assert_eq!(settings.stream_count, 3);
        assert_eq!(settings.sounds_extension, "wav");
    }
}

#[cfg(test)]
#[path = "game_audio_owned_view_tests.rs"]
mod owned_audio_view_tests;

#[cfg(test)]
#[path = "scoped_receipt_reset_tests.rs"]
mod scoped_receipt_reset_tests;

#[cfg(test)]
#[path = "script_audio_owner_tests.rs"]
mod script_audio_owner_tests;
