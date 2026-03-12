//! INI parsing for AudioSettings definitions
//!
//! This module handles parsing AudioSettings block from INI files.
//! AudioSettings contains global audio configuration including volume settings,
//! speaker types, 3D providers, and audio hardware settings.
//!
//! C++ Reference: GeneralsMD/Code/GameEngine/Include/Common/AudioSettings.h
//! C++ Parser: GeneralsMD/Code/GameEngine/Source/Common/Audio/GameAudio.cpp
//!
//! Rust port: 2025

use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;

/// Maximum number of hardware 3D providers
pub const MAX_HW_PROVIDERS: usize = 4;

/// Speaker type enumeration
/// Matches C++ speaker types from GameAudio.cpp
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum SpeakerType {
    #[default]
    TwoSpeakers = 0,
    Headphones = 1,
    SurroundSound = 2,
    FourSpeaker = 3,
    FivePointOne = 4,
    SevenPointOne = 5,
}

impl SpeakerType {
    /// Parse speaker type from string (matches C++ translateSpeakerTypeToUnsignedInt)
    pub fn from_str(s: &str) -> INIResult<Self> {
        match s.to_ascii_lowercase().as_str() {
            "2 speakers" | "2speakers" | "two speakers" => Ok(SpeakerType::TwoSpeakers),
            "headphones" | "headphone" => Ok(SpeakerType::Headphones),
            "surround sound" | "surroundsound" => Ok(SpeakerType::SurroundSound),
            "4 speaker" | "4speaker" | "four speaker" | "fourspeaker" => {
                Ok(SpeakerType::FourSpeaker)
            }
            "5.1 surround" | "5.1surround" | "5.1" => Ok(SpeakerType::FivePointOne),
            "7.1 surround" | "7.1surround" | "7.1" => Ok(SpeakerType::SevenPointOne),
            _ => {
                // Try parsing as number
                if let Ok(num) = s.parse::<u32>() {
                    Self::from_u32(num)
                } else {
                    Err(INIError::InvalidData)
                }
            }
        }
    }

    /// Convert from u32 value
    pub fn from_u32(value: u32) -> INIResult<Self> {
        match value {
            0 => Ok(SpeakerType::TwoSpeakers),
            1 => Ok(SpeakerType::Headphones),
            2 => Ok(SpeakerType::SurroundSound),
            3 => Ok(SpeakerType::FourSpeaker),
            4 => Ok(SpeakerType::FivePointOne),
            5 => Ok(SpeakerType::SevenPointOne),
            _ => Err(INIError::InvalidData),
        }
    }

    /// Convert to u32 value
    pub fn to_u32(self) -> u32 {
        self as u32
    }

    /// Get string representation (matches C++ TheSpeakerTypes array)
    pub fn to_str(self) -> &'static str {
        match self {
            SpeakerType::TwoSpeakers => "2 Speakers",
            SpeakerType::Headphones => "Headphones",
            SpeakerType::SurroundSound => "Surround Sound",
            SpeakerType::FourSpeaker => "4 Speaker",
            SpeakerType::FivePointOne => "5.1 Surround",
            SpeakerType::SevenPointOne => "7.1 Surround",
        }
    }
}

/// Audio settings structure
///
/// Contains all global audio configuration for the game engine.
/// This matches the C++ AudioSettings struct from AudioSettings.h.
#[derive(Debug, Clone)]
pub struct AudioSettings {
    // Folder paths and extensions
    pub audio_root: String,
    pub sounds_folder: String,
    pub music_folder: String,
    pub streaming_folder: String,
    pub sounds_extension: String,

    // Audio hardware settings
    pub use_digital: bool,
    pub use_midi: bool,
    pub output_rate: i32,
    pub output_bits: i32,
    pub output_channels: i32,
    pub sample_count_2d: i32,
    pub sample_count_3d: i32,
    pub stream_count: i32,

    // Global audio ranges
    pub global_min_range: i32,
    pub global_max_range: i32,

    // Timing settings (in frames)
    pub drawable_ambient_frames: u32,
    pub fade_audio_frames: u32,

    // Cache settings
    pub max_cache_size: u32,

    // Volume settings
    pub min_volume: f32, // Samples below this volume will be culled
    pub relative_2d_volume: f32,

    // Default volume levels (these are the INI defaults)
    pub default_sound_volume: f32,
    pub default_3d_sound_volume: f32,
    pub default_speech_volume: f32,
    pub default_music_volume: f32,

    // Preferred volume levels (these can be overridden by user prefs)
    pub preferred_sound_volume: f32,
    pub preferred_3d_sound_volume: f32,
    pub preferred_speech_volume: f32,
    pub preferred_music_volume: f32,

    // 3D Audio providers (5 slots: 4 hardware + 1 software)
    pub preferred_3d_provider: [String; MAX_HW_PROVIDERS + 1],

    // Speaker type settings
    pub default_speaker_type_2d: u32,
    pub default_speaker_type_3d: u32,

    // Microphone/camera settings for 3D audio
    pub microphone_desired_height_above_terrain: f32,
    pub microphone_max_percentage_between_ground_and_camera: f32,

    // Zoom audio settings
    pub zoom_min_distance: f32,
    pub zoom_max_distance: f32,
    pub zoom_sound_volume_percentage_amount: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            audio_root: String::new(),
            sounds_folder: String::new(),
            music_folder: String::new(),
            streaming_folder: String::new(),
            sounds_extension: String::new(),

            use_digital: true,
            use_midi: false,
            output_rate: 22050,
            output_bits: 16,
            output_channels: 2,
            sample_count_2d: 32,
            sample_count_3d: 32,
            stream_count: 8,

            global_min_range: 100,
            global_max_range: 1000,

            drawable_ambient_frames: 0,
            fade_audio_frames: 0,

            max_cache_size: 0,

            min_volume: 0.0,
            relative_2d_volume: 1.0,

            default_sound_volume: 1.0,
            default_3d_sound_volume: 1.0,
            default_speech_volume: 1.0,
            default_music_volume: 1.0,

            preferred_sound_volume: 1.0,
            preferred_3d_sound_volume: 1.0,
            preferred_speech_volume: 1.0,
            preferred_music_volume: 1.0,

            preferred_3d_provider: Default::default(),

            default_speaker_type_2d: SpeakerType::TwoSpeakers.to_u32(),
            default_speaker_type_3d: SpeakerType::SurroundSound.to_u32(),

            microphone_desired_height_above_terrain: 0.0,
            microphone_max_percentage_between_ground_and_camera: 0.0,

            zoom_min_distance: 0.0,
            zoom_max_distance: 0.0,
            zoom_sound_volume_percentage_amount: 0.0,
        }
    }
}

impl AudioSettings {
    /// Create a new AudioSettings with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a reference to the preferred 3D provider at the given index
    pub fn get_preferred_3d_provider(&self, index: usize) -> Option<&str> {
        self.preferred_3d_provider.get(index).map(|s| s.as_str())
    }

    /// Check if hardware acceleration is available
    pub fn has_hardware_provider(&self) -> bool {
        self.preferred_3d_provider[0..MAX_HW_PROVIDERS]
            .iter()
            .any(|p| !p.is_empty())
    }

    /// Get the software fallback provider
    pub fn get_software_provider(&self) -> &str {
        &self.preferred_3d_provider[MAX_HW_PROVIDERS]
    }
}

// ============================================================================
// Field parse functions (matching C++ audioSettingsFieldParseTable)
// ============================================================================

fn parse_audio_root(_ini: &mut INI, settings: &mut AudioSettings, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.audio_root = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_sounds_folder(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.sounds_folder = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_music_folder(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.music_folder = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_streaming_folder(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.streaming_folder = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_sounds_extension(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.sounds_extension = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_use_digital(_ini: &mut INI, settings: &mut AudioSettings, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.use_digital = INI::parse_bool(token)?;
    Ok(())
}

fn parse_use_midi(_ini: &mut INI, settings: &mut AudioSettings, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.use_midi = INI::parse_bool(token)?;
    Ok(())
}

fn parse_output_rate(_ini: &mut INI, settings: &mut AudioSettings, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.output_rate = INI::parse_int(token)?;
    Ok(())
}

fn parse_output_bits(_ini: &mut INI, settings: &mut AudioSettings, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.output_bits = INI::parse_int(token)?;
    Ok(())
}

fn parse_output_channels(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.output_channels = INI::parse_int(token)?;
    Ok(())
}

fn parse_sample_count_2d(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.sample_count_2d = INI::parse_int(token)?;
    Ok(())
}

fn parse_sample_count_3d(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.sample_count_3d = INI::parse_int(token)?;
    Ok(())
}

fn parse_stream_count(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.stream_count = INI::parse_int(token)?;
    Ok(())
}

fn parse_preferred_3d_hw1(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.preferred_3d_provider[0] = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_preferred_3d_hw2(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.preferred_3d_provider[1] = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_preferred_3d_hw3(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.preferred_3d_provider[2] = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_preferred_3d_hw4(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.preferred_3d_provider[3] = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_preferred_3d_sw(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.preferred_3d_provider[4] = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_default_2d_speaker_type(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    let speaker_type = SpeakerType::from_str(token)?;
    settings.default_speaker_type_2d = speaker_type.to_u32();
    Ok(())
}

fn parse_default_3d_speaker_type(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    let speaker_type = SpeakerType::from_str(token)?;
    settings.default_speaker_type_3d = speaker_type.to_u32();
    Ok(())
}

fn parse_min_sample_volume(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.min_volume = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_global_min_range(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.global_min_range = INI::parse_int(token)?;
    Ok(())
}

fn parse_global_max_range(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.global_max_range = INI::parse_int(token)?;
    Ok(())
}

fn parse_time_between_drawable_sounds(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.drawable_ambient_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_time_to_fade_audio(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.fade_audio_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_audio_footprint_in_bytes(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.max_cache_size = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_relative_2d_volume(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    let mut vol = INI::parse_percent_to_real(token)?;
    // Clamp between -1.0 and 1.0 (matches C++ MIN/MAX clamping)
    vol = vol.clamp(-1.0, 1.0);
    settings.relative_2d_volume = vol;
    Ok(())
}

fn parse_default_sound_volume(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.default_sound_volume = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_default_3d_sound_volume(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.default_3d_sound_volume = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_default_speech_volume(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.default_speech_volume = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_default_music_volume(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.default_music_volume = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_microphone_desired_height_above_terrain(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.microphone_desired_height_above_terrain = INI::parse_real(token)?;
    Ok(())
}

fn parse_microphone_max_percentage(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.microphone_max_percentage_between_ground_and_camera =
        INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_zoom_min_distance(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.zoom_min_distance = INI::parse_real(token)?;
    Ok(())
}

fn parse_zoom_max_distance(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.zoom_max_distance = INI::parse_real(token)?;
    Ok(())
}

fn parse_zoom_sound_volume_percentage(
    _ini: &mut INI,
    settings: &mut AudioSettings,
    args: &[&str],
) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    settings.zoom_sound_volume_percentage_amount = INI::parse_percent_to_real(token)?;
    Ok(())
}

/// Field parse table for AudioSettings (matches C++ audioSettingsFieldParseTable)
pub const FIELD_PARSE_TABLE: &[FieldParse<AudioSettings>] = &[
    FieldParse {
        token: "AudioRoot",
        parse: parse_audio_root,
    },
    FieldParse {
        token: "SoundsFolder",
        parse: parse_sounds_folder,
    },
    FieldParse {
        token: "MusicFolder",
        parse: parse_music_folder,
    },
    FieldParse {
        token: "StreamingFolder",
        parse: parse_streaming_folder,
    },
    FieldParse {
        token: "SoundsExtension",
        parse: parse_sounds_extension,
    },
    FieldParse {
        token: "UseDigital",
        parse: parse_use_digital,
    },
    FieldParse {
        token: "UseMidi",
        parse: parse_use_midi,
    },
    FieldParse {
        token: "OutputRate",
        parse: parse_output_rate,
    },
    FieldParse {
        token: "OutputBits",
        parse: parse_output_bits,
    },
    FieldParse {
        token: "OutputChannels",
        parse: parse_output_channels,
    },
    FieldParse {
        token: "SampleCount2D",
        parse: parse_sample_count_2d,
    },
    FieldParse {
        token: "SampleCount3D",
        parse: parse_sample_count_3d,
    },
    FieldParse {
        token: "StreamCount",
        parse: parse_stream_count,
    },
    FieldParse {
        token: "Preferred3DHW1",
        parse: parse_preferred_3d_hw1,
    },
    FieldParse {
        token: "Preferred3DHW2",
        parse: parse_preferred_3d_hw2,
    },
    FieldParse {
        token: "Preferred3DHW3",
        parse: parse_preferred_3d_hw3,
    },
    FieldParse {
        token: "Preferred3DHW4",
        parse: parse_preferred_3d_hw4,
    },
    FieldParse {
        token: "Preferred3DSW",
        parse: parse_preferred_3d_sw,
    },
    FieldParse {
        token: "Default2DSpeakerType",
        parse: parse_default_2d_speaker_type,
    },
    FieldParse {
        token: "Default3DSpeakerType",
        parse: parse_default_3d_speaker_type,
    },
    FieldParse {
        token: "MinSampleVolume",
        parse: parse_min_sample_volume,
    },
    FieldParse {
        token: "GlobalMinRange",
        parse: parse_global_min_range,
    },
    FieldParse {
        token: "GlobalMaxRange",
        parse: parse_global_max_range,
    },
    FieldParse {
        token: "TimeBetweenDrawableSounds",
        parse: parse_time_between_drawable_sounds,
    },
    FieldParse {
        token: "TimeToFadeAudio",
        parse: parse_time_to_fade_audio,
    },
    FieldParse {
        token: "AudioFootprintInBytes",
        parse: parse_audio_footprint_in_bytes,
    },
    FieldParse {
        token: "Relative2DVolume",
        parse: parse_relative_2d_volume,
    },
    FieldParse {
        token: "DefaultSoundVolume",
        parse: parse_default_sound_volume,
    },
    FieldParse {
        token: "Default3DSoundVolume",
        parse: parse_default_3d_sound_volume,
    },
    FieldParse {
        token: "DefaultSpeechVolume",
        parse: parse_default_speech_volume,
    },
    FieldParse {
        token: "DefaultMusicVolume",
        parse: parse_default_music_volume,
    },
    FieldParse {
        token: "MicrophoneDesiredHeightAboveTerrain",
        parse: parse_microphone_desired_height_above_terrain,
    },
    FieldParse {
        token: "MicrophoneMaxPercentageBetweenGroundAndCamera",
        parse: parse_microphone_max_percentage,
    },
    FieldParse {
        token: "ZoomMinDistance",
        parse: parse_zoom_min_distance,
    },
    FieldParse {
        token: "ZoomMaxDistance",
        parse: parse_zoom_max_distance,
    },
    FieldParse {
        token: "ZoomSoundVolumePercentageAmount",
        parse: parse_zoom_sound_volume_percentage,
    },
];

/// Global AudioSettings instance (thread-safe)
static AUDIO_SETTINGS: OnceCell<Arc<RwLock<AudioSettings>>> = OnceCell::new();

/// Ensure the audio settings exist and return a handle to it
pub fn ensure_audio_settings() -> Arc<RwLock<AudioSettings>> {
    AUDIO_SETTINGS
        .get_or_init(|| Arc::new(RwLock::new(AudioSettings::new())))
        .clone()
}

/// Initialize (or reinitialize) the global audio settings
pub fn init_global_audio_settings() {
    let settings = ensure_audio_settings();
    *settings.write() = AudioSettings::new();
}

/// Get a handle to the global audio settings if initialized
pub fn get_audio_settings() -> Option<Arc<RwLock<AudioSettings>>> {
    AUDIO_SETTINGS.get().cloned()
}

/// Get a read guard to the global audio settings
pub fn get_audio_settings_read() -> Option<parking_lot::RwLockReadGuard<'static, AudioSettings>> {
    AUDIO_SETTINGS.get().map(|arc| arc.read())
}

/// Get a write guard to the global audio settings
pub fn get_audio_settings_write() -> Option<parking_lot::RwLockWriteGuard<'static, AudioSettings>> {
    AUDIO_SETTINGS.get().map(|arc| arc.write())
}

/// INI parsing function for AudioSettings definition (matches C++ INI::parseAudioSettingsDefinition)
///
/// This is the main entry point for parsing AudioSettings definitions from INI files.
/// AudioSettings is a singleton - there's only one definition per game.
pub fn parse_audio_settings_definition(ini: &mut INI) -> INIResult<()> {
    // Get or create global audio settings
    let settings_handle = ensure_audio_settings();
    {
        let mut settings = settings_handle.write();

        // Parse using field table
        ini.init_from_ini_with_fields(&mut *settings, FIELD_PARSE_TABLE)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speaker_type_parsing() {
        assert_eq!(
            SpeakerType::from_str("2 Speakers").unwrap(),
            SpeakerType::TwoSpeakers
        );
        assert_eq!(
            SpeakerType::from_str("Headphones").unwrap(),
            SpeakerType::Headphones
        );
        assert_eq!(
            SpeakerType::from_str("Surround Sound").unwrap(),
            SpeakerType::SurroundSound
        );
        assert_eq!(
            SpeakerType::from_str("5.1 Surround").unwrap(),
            SpeakerType::FivePointOne
        );
        assert_eq!(
            SpeakerType::from_str("7.1 Surround").unwrap(),
            SpeakerType::SevenPointOne
        );
    }

    #[test]
    fn test_speaker_type_numeric() {
        assert_eq!(SpeakerType::from_u32(0).unwrap(), SpeakerType::TwoSpeakers);
        assert_eq!(SpeakerType::from_u32(1).unwrap(), SpeakerType::Headphones);
        assert_eq!(
            SpeakerType::from_u32(2).unwrap(),
            SpeakerType::SurroundSound
        );
        assert!(SpeakerType::from_u32(99).is_err());
    }

    #[test]
    fn test_speaker_type_roundtrip() {
        for i in 0..=5 {
            let speaker_type = SpeakerType::from_u32(i).unwrap();
            assert_eq!(speaker_type.to_u32(), i);
            let str_repr = speaker_type.to_str();
            let parsed = SpeakerType::from_str(str_repr).unwrap();
            assert_eq!(parsed, speaker_type);
        }
    }

    #[test]
    fn test_audio_settings_default() {
        let settings = AudioSettings::new();

        assert!(settings.audio_root.is_empty());
        assert!(settings.use_digital);
        assert!(!settings.use_midi);
        assert_eq!(settings.output_rate, 22050);
        assert_eq!(settings.output_bits, 16);
        assert_eq!(settings.output_channels, 2);
        assert_eq!(settings.sample_count_2d, 32);
        assert_eq!(settings.sample_count_3d, 32);
        assert_eq!(settings.stream_count, 8);
        assert_eq!(settings.relative_2d_volume, 1.0);
        assert_eq!(settings.default_sound_volume, 1.0);
    }

    #[test]
    fn test_audio_settings_providers() {
        let mut settings = AudioSettings::new();

        settings.preferred_3d_provider[0] = "Creative Labs EAX".to_string();
        settings.preferred_3d_provider[4] = "Miles Fast 2D".to_string();

        assert!(settings.has_hardware_provider());
        assert_eq!(
            settings.get_preferred_3d_provider(0),
            Some("Creative Labs EAX")
        );
        assert_eq!(settings.get_software_provider(), "Miles Fast 2D");
    }

    #[test]
    fn test_global_audio_settings() {
        init_global_audio_settings();

        {
            let mut settings = get_audio_settings_write().unwrap();
            settings.audio_root = "Data/Audio".to_string();
        }

        let settings = get_audio_settings_read().unwrap();
        assert_eq!(settings.audio_root, "Data/Audio");
    }

    #[test]
    fn test_field_parse_table() {
        assert_eq!(FIELD_PARSE_TABLE.len(), 36);

        let field_names: Vec<&str> = FIELD_PARSE_TABLE.iter().map(|f| f.token).collect();
        assert!(field_names.contains(&"AudioRoot"));
        assert!(field_names.contains(&"SoundsFolder"));
        assert!(field_names.contains(&"Preferred3DHW1"));
        assert!(field_names.contains(&"Preferred3DSW"));
        assert!(field_names.contains(&"Default2DSpeakerType"));
        assert!(field_names.contains(&"Default3DSpeakerType"));
        assert!(field_names.contains(&"DefaultSoundVolume"));
        assert!(field_names.contains(&"ZoomMinDistance"));
    }
}
