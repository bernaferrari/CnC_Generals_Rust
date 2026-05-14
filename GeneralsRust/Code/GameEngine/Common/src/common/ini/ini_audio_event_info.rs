//! INI Audio Event Info parsing module
//! Author: Colin Day, July 2002
//! Desc: Parsing AudioEvent, MusicTrack and DialogEvent INI entries

use super::ini::{INIError, INIResult, INI};
use crate::common::audio::audio_event_rts::{
    AudioEventInfo as EngineAudioEventInfo, AudioPriority as EngineAudioPriority,
    AudioType as EngineAudioType,
};
use crate::common::audio::game_audio::{get_global_audio_manager, initialize_global_audio_manager};
use rand::Rng;
use std::collections::HashMap;

/// Audio type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioType {
    Music,
    SoundEffect,
    Streaming,
    Voice,
}

impl Default for AudioType {
    fn default() -> Self {
        AudioType::SoundEffect
    }
}

/// Audio priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPriority {
    Lowest = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl Default for AudioPriority {
    fn default() -> Self {
        AudioPriority::Normal
    }
}

/// Audio control flags
#[derive(Debug, Clone, Copy)]
pub struct AudioControlFlags {
    pub loop_audio: bool,
    pub random: bool,
    pub all: bool,
    pub post_delay: bool,
    pub interrupt: bool,
}

impl Default for AudioControlFlags {
    fn default() -> Self {
        Self {
            loop_audio: false,
            random: false,
            all: false,
            post_delay: false,
            interrupt: false,
        }
    }
}

const AC_LOOP: u32 = 0x00000001;
const AC_RANDOM: u32 = 0x00000002;
const AC_ALL: u32 = 0x00000004;
const AC_POSTDELAY: u32 = 0x00000008;
const AC_INTERRUPT: u32 = 0x00000010;

fn map_audio_type(sound_type: AudioType) -> EngineAudioType {
    match sound_type {
        AudioType::Music => EngineAudioType::Music,
        AudioType::Streaming | AudioType::Voice => EngineAudioType::Streaming,
        AudioType::SoundEffect => EngineAudioType::SoundEffect,
    }
}

fn map_audio_priority(priority: AudioPriority) -> EngineAudioPriority {
    match priority {
        AudioPriority::Lowest => EngineAudioPriority::Lowest,
        AudioPriority::Low => EngineAudioPriority::Low,
        AudioPriority::Normal => EngineAudioPriority::Normal,
        AudioPriority::High => EngineAudioPriority::High,
        AudioPriority::Critical => EngineAudioPriority::Critical,
    }
}

fn control_flags_to_bits(flags: AudioControlFlags) -> u32 {
    let mut bits = 0;
    if flags.loop_audio {
        bits |= AC_LOOP;
    }
    if flags.random {
        bits |= AC_RANDOM;
    }
    if flags.all {
        bits |= AC_ALL;
    }
    if flags.post_delay {
        bits |= AC_POSTDELAY;
    }
    if flags.interrupt {
        bits |= AC_INTERRUPT;
    }
    bits
}

fn register_audio_event_info(info: &AudioEventInfo) {
    let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
    if let Ok(mut guard) = manager.lock() {
        guard.register_audio_event_info(info.to_engine_info());
    };
}

/// Audio Event Information structure
#[derive(Debug, Clone)]
pub struct AudioEventInfo {
    pub audio_name: String,
    pub sound_type: AudioType,
    pub filename: String,
    pub volume: f32,
    pub volume_shift: f32,
    pub min_volume: f32,
    pub pitch_shift_min: f32,
    pub pitch_shift_max: f32,
    pub delay_min: i32,
    pub delay_max: i32,
    pub limit: i32,
    pub loop_count: i32,
    pub priority: AudioPriority,
    pub audio_type: u32, // Bit flags for sound types
    pub control: AudioControlFlags,
    pub sounds: Vec<String>,
    pub sounds_night: Vec<String>,
    pub sounds_evening: Vec<String>,
    pub sounds_morning: Vec<String>,
    pub attack_sounds: Vec<String>,
    pub decay_sounds: Vec<String>,
    pub min_distance: f32,
    pub max_distance: f32,
    pub low_pass_freq: f32,
}

impl Default for AudioEventInfo {
    fn default() -> Self {
        Self {
            audio_name: String::new(),
            sound_type: AudioType::default(),
            filename: String::new(),
            volume: 1.0,
            volume_shift: 0.0,
            min_volume: 0.0,
            pitch_shift_min: 1.0,
            pitch_shift_max: 1.0,
            delay_min: 0,
            delay_max: 0,
            limit: -1, // No limit
            loop_count: 1,
            priority: AudioPriority::default(),
            audio_type: 0,
            control: AudioControlFlags::default(),
            sounds: Vec::new(),
            sounds_night: Vec::new(),
            sounds_evening: Vec::new(),
            sounds_morning: Vec::new(),
            attack_sounds: Vec::new(),
            decay_sounds: Vec::new(),
            min_distance: 0.0,
            max_distance: 1000.0,
            low_pass_freq: 1.0,
        }
    }
}

impl AudioEventInfo {
    /// Create a new audio event info with the given name
    pub fn new(name: String, sound_type: AudioType) -> Self {
        Self {
            audio_name: name,
            sound_type,
            ..Default::default()
        }
    }

    /// Parse audio event info from INI
    pub fn parse_from_ini(ini: &mut INI, name: String, sound_type: AudioType) -> INIResult<Self> {
        let mut info = Self::new(name, sound_type);
        info.parse_audio_fields(ini)?;
        Ok(info)
    }

    /// Parse audio-specific fields
    fn parse_audio_fields(&mut self, ini: &mut INI) -> INIResult<()> {
        // Mirror C++ parser behavior: parse known fields and ignore unknown keys.
        ini.init_from_ini_with_fields_allow_unknown(self, Self::get_field_parse())
    }

    /// Parse delay values (min and max)
    pub fn parse_delay(tokens: &[&str]) -> INIResult<(i32, i32)> {
        if tokens.len() < 2 {
            return Err(INIError::InvalidData);
        }

        let min: i32 = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
        let max: i32 = tokens[1].parse().map_err(|_| INIError::InvalidData)?;

        if min < 0 || max < min {
            return Err(INIError::InvalidData);
        }

        Ok((min, max))
    }

    /// Parse pitch shift values (min and max percentages)
    pub fn parse_pitch_shift(tokens: &[&str]) -> INIResult<(f32, f32)> {
        if tokens.len() < 2 {
            return Err(INIError::InvalidData);
        }

        let min_percent: f32 = tokens[0].parse().map_err(|_| INIError::InvalidData)?;
        let max_percent: f32 = tokens[1].parse().map_err(|_| INIError::InvalidData)?;

        if min_percent <= -100.0 || max_percent < min_percent {
            return Err(INIError::InvalidData);
        }

        // Convert percentages to multipliers
        let min_multiplier = 1.0 + min_percent / 100.0;
        let max_multiplier = 1.0 + max_percent / 100.0;

        Ok((min_multiplier, max_multiplier))
    }

    /// Parse sounds list from tokens
    pub fn parse_sounds_list(tokens: &[&str]) -> Vec<String> {
        tokens
            .iter()
            .filter(|&&token| !token.is_empty())
            .map(|&token| token.to_string())
            .collect()
    }

    /// Get the field parsing table for audio event info
    pub fn get_field_parse() -> &'static [super::ini::FieldParse<Self>] {
        FIELD_PARSE_TABLE
    }

    /// Check if this audio event should loop
    pub fn should_loop(&self) -> bool {
        self.control.loop_audio || self.loop_count != 1
    }

    /// Get random pitch shift value
    pub fn get_random_pitch_shift(&self) -> f32 {
        if self.pitch_shift_min == self.pitch_shift_max {
            self.pitch_shift_min
        } else {
            rand::thread_rng().gen_range(self.pitch_shift_min..=self.pitch_shift_max)
        }
    }

    /// Get random delay value
    pub fn get_random_delay(&self) -> i32 {
        if self.delay_min == self.delay_max {
            self.delay_min
        } else {
            rand::thread_rng().gen_range(self.delay_min..=self.delay_max)
        }
    }

    fn to_engine_info(&self) -> EngineAudioEventInfo {
        EngineAudioEventInfo {
            sound_type: map_audio_type(self.sound_type),
            control: control_flags_to_bits(self.control),
            audio_name: self.audio_name.clone(),
            volume: self.volume,
            sounds_morning: self.sounds_morning.clone(),
            sounds: self.sounds.clone(),
            sounds_night: self.sounds_night.clone(),
            sounds_evening: self.sounds_evening.clone(),
            attack_sounds: self.attack_sounds.clone(),
            decay_sounds: self.decay_sounds.clone(),
            pitch_shift_min: self.pitch_shift_min,
            pitch_shift_max: self.pitch_shift_max,
            volume_shift: self.volume_shift,
            min_volume: self.min_volume,
            limit: self.limit,
            loop_count: self.loop_count,
            delay_min: self.delay_min as f32,
            delay_max: self.delay_max as f32,
            filename: self.filename.clone(),
            sound_type_field: map_audio_type(self.sound_type),
            type_field: self.audio_type,
            priority: map_audio_priority(self.priority),
            min_distance: self.min_distance,
            max_distance: self.max_distance,
        }
    }
}

/// Audio priority name mappings
pub const AUDIO_PRIORITY_NAMES: &[&str] = &["LOWEST", "LOW", "NORMAL", "HIGH", "CRITICAL"];

/// Sound type name mappings
pub const SOUND_TYPE_NAMES: &[&str] = &[
    "UI", "WORLD", "SHROUDED", "GLOBAL", "VOICE", "PLAYER", "ALLIES", "ENEMIES", "EVERYONE",
];

/// Audio control name mappings
pub const AUDIO_CONTROL_NAMES: &[&str] = &["LOOP", "RANDOM", "ALL", "POSTDELAY", "INTERRUPT"];

fn parse_filename_field(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    info.filename = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_percent_field(_ini: &mut INI, target: &mut f32, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    *target = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_real_field(_ini: &mut INI, target: &mut f32, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    *target = INI::parse_real(token)?;
    Ok(())
}

fn parse_int_field(_ini: &mut INI, target: &mut i32, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    *target = INI::parse_int(token)?;
    Ok(())
}

fn parse_volume(ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    parse_percent_field(ini, &mut info.volume, args)
}

fn parse_volume_shift(ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    parse_percent_field(ini, &mut info.volume_shift, args)
}

fn parse_min_volume(ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    parse_percent_field(ini, &mut info.min_volume, args)
}

fn parse_pitch_shift_field(
    _ini: &mut INI,
    info: &mut AudioEventInfo,
    args: &[&str],
) -> INIResult<()> {
    let (min, max) = AudioEventInfo::parse_pitch_shift(args)?;
    info.pitch_shift_min = min;
    info.pitch_shift_max = max;
    Ok(())
}

fn parse_delay_field(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    let (min, max) = AudioEventInfo::parse_delay(args)?;
    info.delay_min = min;
    info.delay_max = max;
    Ok(())
}

fn parse_limit(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    parse_int_field(_ini, &mut info.limit, args)
}

fn parse_loop_count(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    parse_int_field(_ini, &mut info.loop_count, args)
}

fn parse_priority(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    let index = INI::parse_index_list(token, AUDIO_PRIORITY_NAMES)?;
    info.priority = match index {
        0 => AudioPriority::Lowest,
        1 => AudioPriority::Low,
        2 => AudioPriority::Normal,
        3 => AudioPriority::High,
        4 => AudioPriority::Critical,
        _ => return Err(INIError::InvalidData),
    };
    Ok(())
}

fn parse_audio_type_bits(
    _ini: &mut INI,
    info: &mut AudioEventInfo,
    args: &[&str],
) -> INIResult<()> {
    info.audio_type = INI::parse_bit_string_32(args, SOUND_TYPE_NAMES)?;
    Ok(())
}

fn parse_control_bits(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    let bits = INI::parse_bit_string_32(args, AUDIO_CONTROL_NAMES)?;
    info.control.loop_audio = bits & AC_LOOP != 0;
    info.control.random = bits & AC_RANDOM != 0;
    info.control.all = bits & AC_ALL != 0;
    info.control.post_delay = bits & AC_POSTDELAY != 0;
    info.control.interrupt = bits & AC_INTERRUPT != 0;
    Ok(())
}

fn parse_sounds(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    info.sounds = AudioEventInfo::parse_sounds_list(args);
    Ok(())
}

fn parse_sounds_night(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    info.sounds_night = AudioEventInfo::parse_sounds_list(args);
    Ok(())
}

fn parse_sounds_evening(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    info.sounds_evening = AudioEventInfo::parse_sounds_list(args);
    Ok(())
}

fn parse_sounds_morning(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    info.sounds_morning = AudioEventInfo::parse_sounds_list(args);
    Ok(())
}

fn parse_attack_sounds(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    info.attack_sounds = AudioEventInfo::parse_sounds_list(args);
    Ok(())
}

fn parse_decay_sounds(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    info.decay_sounds = AudioEventInfo::parse_sounds_list(args);
    Ok(())
}

fn parse_min_range(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    parse_real_field(_ini, &mut info.min_distance, args)
}

fn parse_max_range(_ini: &mut INI, info: &mut AudioEventInfo, args: &[&str]) -> INIResult<()> {
    parse_real_field(_ini, &mut info.max_distance, args)
}

fn parse_low_pass_cutoff(
    _ini: &mut INI,
    info: &mut AudioEventInfo,
    args: &[&str],
) -> INIResult<()> {
    parse_percent_field(_ini, &mut info.low_pass_freq, args)
}

const FIELD_PARSE_TABLE: &[super::ini::FieldParse<AudioEventInfo>] = &[
    super::ini::FieldParse {
        token: "Filename",
        parse: parse_filename_field,
    },
    super::ini::FieldParse {
        token: "Volume",
        parse: parse_volume,
    },
    super::ini::FieldParse {
        token: "VolumeShift",
        parse: parse_volume_shift,
    },
    super::ini::FieldParse {
        token: "MinVolume",
        parse: parse_min_volume,
    },
    super::ini::FieldParse {
        token: "PitchShift",
        parse: parse_pitch_shift_field,
    },
    super::ini::FieldParse {
        token: "Delay",
        parse: parse_delay_field,
    },
    super::ini::FieldParse {
        token: "Limit",
        parse: parse_limit,
    },
    super::ini::FieldParse {
        token: "LoopCount",
        parse: parse_loop_count,
    },
    super::ini::FieldParse {
        token: "Priority",
        parse: parse_priority,
    },
    super::ini::FieldParse {
        token: "Type",
        parse: parse_audio_type_bits,
    },
    super::ini::FieldParse {
        token: "Control",
        parse: parse_control_bits,
    },
    super::ini::FieldParse {
        token: "Sounds",
        parse: parse_sounds,
    },
    super::ini::FieldParse {
        token: "SoundsNight",
        parse: parse_sounds_night,
    },
    super::ini::FieldParse {
        token: "SoundsEvening",
        parse: parse_sounds_evening,
    },
    super::ini::FieldParse {
        token: "SoundsMorning",
        parse: parse_sounds_morning,
    },
    super::ini::FieldParse {
        token: "Attack",
        parse: parse_attack_sounds,
    },
    super::ini::FieldParse {
        token: "Decay",
        parse: parse_decay_sounds,
    },
    super::ini::FieldParse {
        token: "MinRange",
        parse: parse_min_range,
    },
    super::ini::FieldParse {
        token: "MaxRange",
        parse: parse_max_range,
    },
    super::ini::FieldParse {
        token: "LowPassCutoff",
        parse: parse_low_pass_cutoff,
    },
];

/// Parse music track definition from INI file
pub fn parse_music_track_definition(ini: &mut INI) -> INIResult<()> {
    // Read the track name
    let name = match ini.get_next_value_token().or_else(|| ini.get_first_token()) {
        Some(token) => token,
        None => return Err(INIError::InvalidData),
    };

    // Create new audio event info for music track
    let mut track = AudioEventInfo::new(name.clone(), AudioType::Music);

    // Apply defaults from "DefaultMusicTrack" if it exists
    // This would be handled by the audio system in a real implementation

    // Parse the track fields
    track.parse_audio_fields(ini)?;

    register_audio_event_info(&track);

    Ok(())
}

/// Parse audio event definition from INI file
pub fn parse_audio_event_definition(ini: &mut INI) -> INIResult<()> {
    // Read the event name
    let name = match ini.get_next_value_token().or_else(|| ini.get_first_token()) {
        Some(token) => token,
        None => return Err(INIError::InvalidData),
    };

    // Create new audio event info
    let mut event = AudioEventInfo::new(name.clone(), AudioType::SoundEffect);

    // Apply defaults from "DefaultSoundEffect" if it exists
    // This would be handled by the audio system in a real implementation

    // Parse the event fields
    event.parse_audio_fields(ini)?;

    register_audio_event_info(&event);

    Ok(())
}

/// Parse dialog event definition from INI file
pub fn parse_dialog_definition(ini: &mut INI) -> INIResult<()> {
    // Read the dialog name
    let name = match ini.get_next_value_token().or_else(|| ini.get_first_token()) {
        Some(token) => token,
        None => return Err(INIError::InvalidData),
    };

    // Create new audio event info for dialog
    let mut dialog = AudioEventInfo::new(name.clone(), AudioType::Streaming);

    // Apply defaults from "DefaultDialog" if it exists
    // This would be handled by the audio system in a real implementation

    // Parse the dialog fields
    dialog.parse_audio_fields(ini)?;

    register_audio_event_info(&dialog);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_event_info_creation() {
        let info = AudioEventInfo::new("TestSound".to_string(), AudioType::SoundEffect);
        assert_eq!(info.audio_name, "TestSound");
        assert_eq!(info.sound_type, AudioType::SoundEffect);
        assert_eq!(info.volume, 1.0);
        assert_eq!(info.loop_count, 1);
    }

    #[test]
    fn test_parse_delay() {
        let tokens = vec!["100", "500"];
        let result = AudioEventInfo::parse_delay(&tokens);
        assert!(result.is_ok());

        let (min, max) = result.unwrap();
        assert_eq!(min, 100);
        assert_eq!(max, 500);
    }

    #[test]
    fn test_parse_delay_invalid() {
        let tokens = vec!["-50", "100"];
        let result = AudioEventInfo::parse_delay(&tokens);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_pitch_shift() {
        let tokens = vec!["-10", "20"];
        let result = AudioEventInfo::parse_pitch_shift(&tokens);
        assert!(result.is_ok());

        let (min, max) = result.unwrap();
        assert_eq!(min, 0.9); // 1 + (-10/100)
        assert_eq!(max, 1.2); // 1 + (20/100)
    }

    #[test]
    fn test_parse_sounds_list() {
        let tokens = vec!["sound1.wav", "sound2.wav", "sound3.wav"];
        let sounds = AudioEventInfo::parse_sounds_list(&tokens);

        assert_eq!(sounds.len(), 3);
        assert_eq!(sounds[0], "sound1.wav");
        assert_eq!(sounds[1], "sound2.wav");
        assert_eq!(sounds[2], "sound3.wav");
    }

    #[test]
    fn test_should_loop() {
        let mut info = AudioEventInfo::new("TestSound".to_string(), AudioType::SoundEffect);
        assert!(!info.should_loop());

        info.loop_count = 5;
        assert!(info.should_loop());

        info.loop_count = 1;
        info.control.loop_audio = true;
        assert!(info.should_loop());
    }

    #[test]
    fn test_audio_control_flags() {
        let flags = AudioControlFlags::default();
        assert!(!flags.loop_audio);
        assert!(!flags.random);
        assert!(!flags.all);
        assert!(!flags.post_delay);
        assert!(!flags.interrupt);
    }
}
