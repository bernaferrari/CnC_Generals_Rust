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
        AudioEventInfo, AudioEventRts, AudioHandle, AudioPriority, AudioType, Coord3D,
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
    fn pause(&self, handle: AudioHandle) {
        self.stop(handle);
    }
    fn resume(&self, _handle: AudioHandle) {}
    fn is_playing(&self, _handle: AudioHandle) -> bool {
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
    ((mask as u32) & (flag as u32)) != 0
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioAffect {
    Music = 0x01,
    Sound = 0x02,
    Sound3D = 0x04,
    Speech = 0x08,
    SystemSetting = 0x10,
    SoundEffects = 0x06, // Sound | Sound3D
    Ambient = 0x20,
    All = 0x0F,
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
    music_tracks: Vec<AsciiString>,

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
            music_tracks: Vec::new(),
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
        // Clear out any adjusted volumes
        self.adjusted_volumes.clear();

        // Reset scripted volumes
        self.script_music_volume = 1.0;
        self.script_sound_volume = 1.0;
        self.script_sound_3d_volume = 1.0;
        self.script_speech_volume = 1.0;

        // Restore the final values
        self.music_volume = self.system_music_volume;
        self.sound_volume = self.system_sound_volume;
        self.sound_3d_volume = self.system_sound_3d_volume;
        self.speech_volume = self.system_speech_volume;

        self.disallow_speech = false;

        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.reset();
        }
    }

    pub fn update(&mut self) {
        // Complex update logic for listener position, zoom volume, etc.
        // This would require access to tactical view and terrain logic

        self.process_request_list();
        if let Some(sound_mgr) = &mut self.sound_manager {
            sound_mgr.update();
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
                    music_mgr.add_audio_event(audio_event);
                    handle
                } else {
                    AHSV_NO_SOUND
                }
            }
            _ => {
                if let Some(sound_mgr) = &mut self.sound_manager {
                    if sound_mgr.add_audio_event(audio_event).is_ok() {
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

        self.volume_has_changed = true;
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
    }

    pub fn get_listener_position(&self) -> &Coord3D {
        &self.listener_position
    }

    pub fn allocate_audio_request(&self, use_audio_event: Bool) -> AudioRequest {
        let mut request = AudioRequest::default();
        // request.use_pending_event = use_audio_event;
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
                        let _ = with_sound_playback_hook(|hook| {
                            hook.stop(handle);
                        });
                    }
                }
                RequestType::Play => {
                    if let Some(event) = request.get_pending_event() {
                        if let Some(sound_mgr) = &mut self.sound_manager {
                            let _ = sound_mgr.add_audio_event(event.clone());
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
            sounds: Vec::new(),
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

        self.set_volume(0.0, AudioAffect::All);
    }

    pub fn regain_focus(&mut self) {
        if let Some(saved) = self.saved_values.take() {
            self.set_volume(saved[0], AudioAffect::Music);
            self.set_volume(saved[1], AudioAffect::Sound);
            self.set_volume(saved[2], AudioAffect::Sound3D);
            self.set_volume(saved[3], AudioAffect::Speech);
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
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
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
        register_animation_sound_library(manager.clone());
        manager
    }
}

/// Access the global audio manager if it has been initialised.
pub fn get_global_audio_manager() -> Option<Arc<Mutex<AudioManager>>> {
    THE_AUDIO.get().cloned()
}
