////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! DynamicAudioEventInfo structure
//! Derivation of AudioEventInfo structure, for customized sounds
//! Author: Ian Barkley-Yeung, June 2003
//! Converted to Rust

use crate::common::audio::audio_event_rts::{
    AsciiString, AudioEventInfo, AudioPriority, AudioType,
};
use crate::common::system::xfer::{Xfer, XferMode};
use std::io;

// Type aliases
pub type Real = f32;
pub type Bool = bool;
pub type Int = i32;
pub type UnsignedByte = u8;

// Constants for bit manipulation
const AC_LOOP: u32 = 0x00000001;

/// List of fields we can override
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverriddenFields {
    OverrideName = 0,
    OverrideLoopFlag,
    OverrideLoopCount,
    OverrideVolume,
    OverrideMinVolume,
    OverrideMinRange,
    OverrideMaxRange,
    OverridePriority,
}

const OVERRIDE_COUNT: usize = 8;

/// BitFlags implementation for tracking overridden fields
#[derive(Debug, Clone)]
pub struct BitFlags {
    flags: [bool; OVERRIDE_COUNT],
}

impl BitFlags {
    pub fn new() -> Self {
        BitFlags {
            flags: [false; OVERRIDE_COUNT],
        }
    }

    pub fn set(&mut self, field: OverriddenFields, value: bool) {
        let index = field as usize;
        if index < OVERRIDE_COUNT {
            self.flags[index] = value;
        }
    }

    pub fn test(&self, field: OverriddenFields) -> bool {
        let index = field as usize;
        if index < OVERRIDE_COUNT {
            self.flags[index]
        } else {
            false
        }
    }

    pub fn clear(&mut self, field: OverriddenFields) {
        self.set(field, false);
    }
}

impl Default for BitFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Derivation of AudioEventInfo structure, for customized sounds
///
/// NOTE: This implementation would be a lot cleaner & safer if AudioEventInfo was better
/// written. Ideally, AudioEventInfo would be a class, not a struct, and provide only
/// "get" functions, not "set" functions except for the INI parsing. Then we could
/// force people to go through our override...() functions.
#[derive(Debug)]
pub struct DynamicAudioEventInfo {
    // Base AudioEventInfo fields
    pub audio_event_info: AudioEventInfo,

    // Override tracking
    overridden_fields: BitFlags,

    // Retain the original name so we can look it up later
    original_name: AsciiString,
}

impl DynamicAudioEventInfo {
    /// Default constructor
    pub fn new() -> Self {
        DynamicAudioEventInfo {
            audio_event_info: AudioEventInfo {
                sound_type: AudioType::SoundEffect,
                control: 0,
                audio_name: String::new(),
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
            },
            overridden_fields: BitFlags::new(),
            original_name: String::new(),
        }
    }

    /// Initialize AudioEventInfo portion of DynamicAudioEventInfo as copy; leave remainder uninitialized
    pub fn from_base_info(base_info: &AudioEventInfo) -> Self {
        DynamicAudioEventInfo {
            audio_event_info: AudioEventInfo {
                sound_type: base_info.sound_type,
                control: base_info.control,
                audio_name: base_info.audio_name.clone(),
                volume: base_info.volume,
                sounds: base_info.sounds.clone(),
                attack_sounds: base_info.attack_sounds.clone(),
                decay_sounds: base_info.decay_sounds.clone(),
                pitch_shift_min: base_info.pitch_shift_min,
                pitch_shift_max: base_info.pitch_shift_max,
                volume_shift: base_info.volume_shift,
                min_volume: base_info.min_volume,
                limit: base_info.limit,
                loop_count: base_info.loop_count,
                delay_min: base_info.delay_min,
                delay_max: base_info.delay_max,
                filename: base_info.filename.clone(),
                sound_type_field: base_info.sound_type_field,
                type_field: base_info.type_field,
                priority: base_info.priority,
                min_distance: base_info.min_distance,
                max_distance: base_info.max_distance,
            },
            overridden_fields: BitFlags::new(),
            original_name: String::new(),
        }
    }

    /// Override; dynamic audio events are used only for level-specific stuff at the moment
    pub fn is_level_specific(&self) -> bool {
        true
    }

    /// Override; change the name of this audio event
    pub fn override_audio_name(&mut self, new_name: &str) {
        // Record new name. Needed later for load & save
        self.original_name = self.audio_event_info.audio_name.clone();

        self.overridden_fields
            .set(OverriddenFields::OverrideName, true);

        self.audio_event_info.audio_name = new_name.to_string();
    }

    /// Override; change the looping property of this audio event
    pub fn override_loop_flag(&mut self, new_loop_flag: Bool) {
        self.overridden_fields
            .set(OverriddenFields::OverrideLoopFlag, true);

        if new_loop_flag {
            self.audio_event_info.control |= AC_LOOP;
        } else {
            self.audio_event_info.control &= !AC_LOOP;
        }
    }

    /// Override; change the looping properties of this audio event
    pub fn override_loop_count(&mut self, new_loop_count: Int) {
        self.overridden_fields
            .set(OverriddenFields::OverrideLoopCount, true);
        self.audio_event_info.loop_count = new_loop_count;
    }

    /// Override; change the volume of this audio event
    pub fn override_volume(&mut self, new_volume: Real) {
        self.overridden_fields
            .set(OverriddenFields::OverrideVolume, true);
        self.audio_event_info.volume = new_volume;
    }

    /// Override; change the minimum volume of this audio event
    pub fn override_min_volume(&mut self, new_min_volume: Real) {
        self.overridden_fields
            .set(OverriddenFields::OverrideMinVolume, true);
        self.audio_event_info.min_volume = new_min_volume;
    }

    /// Override; change the minimum range of this audio event
    pub fn override_min_range(&mut self, new_min_range: Real) {
        self.overridden_fields
            .set(OverriddenFields::OverrideMinRange, true);
        self.audio_event_info.min_distance = new_min_range;
    }

    /// Override; change the maximum range of this audio event
    pub fn override_max_range(&mut self, new_max_range: Real) {
        self.overridden_fields
            .set(OverriddenFields::OverrideMaxRange, true);
        self.audio_event_info.max_distance = new_max_range;
    }

    /// Override; change the priority of this audio event
    pub fn override_priority(&mut self, new_priority: AudioPriority) {
        self.overridden_fields
            .set(OverriddenFields::OverridePriority, true);
        self.audio_event_info.priority = new_priority;
    }

    /// Get the name of the INI entry this event info was derived from
    pub fn get_original_name(&self) -> &str {
        if self.was_audio_name_overridden() {
            &self.original_name
        } else {
            &self.audio_event_info.audio_name
        }
    }

    // Query methods to check if fields have been overridden

    /// Query: was override_audio_name called?
    pub fn was_audio_name_overridden(&self) -> Bool {
        self.overridden_fields.test(OverriddenFields::OverrideName)
    }

    /// Query: was override_loop_flag called?
    pub fn was_loop_flag_overridden(&self) -> Bool {
        self.overridden_fields
            .test(OverriddenFields::OverrideLoopFlag)
    }

    /// Query: was override_loop_count called?
    pub fn was_loop_count_overridden(&self) -> Bool {
        self.overridden_fields
            .test(OverriddenFields::OverrideLoopCount)
    }

    /// Query: was override_volume called?
    pub fn was_volume_overridden(&self) -> Bool {
        self.overridden_fields
            .test(OverriddenFields::OverrideVolume)
    }

    /// Query: was override_min_volume called?
    pub fn was_min_volume_overridden(&self) -> Bool {
        self.overridden_fields
            .test(OverriddenFields::OverrideMinVolume)
    }

    /// Query: was override_min_range called?
    pub fn was_min_range_overridden(&self) -> Bool {
        self.overridden_fields
            .test(OverriddenFields::OverrideMinRange)
    }

    /// Query: was override_max_range called?
    pub fn was_max_range_overridden(&self) -> Bool {
        self.overridden_fields
            .test(OverriddenFields::OverrideMaxRange)
    }

    /// Query: was override_priority called?
    pub fn was_priority_overridden(&self) -> Bool {
        self.overridden_fields
            .test(OverriddenFields::OverridePriority)
    }

    /// Get reference to the underlying AudioEventInfo
    pub fn get_audio_event_info(&self) -> &AudioEventInfo {
        &self.audio_event_info
    }

    /// Get mutable reference to the underlying AudioEventInfo
    pub fn get_audio_event_info_mut(&mut self) -> &mut AudioEventInfo {
        &mut self.audio_event_info
    }

    fn field_from_index(field: usize) -> Option<OverriddenFields> {
        match field {
            0 => Some(OverriddenFields::OverrideName),
            1 => Some(OverriddenFields::OverrideLoopFlag),
            2 => Some(OverriddenFields::OverrideLoopCount),
            3 => Some(OverriddenFields::OverrideVolume),
            4 => Some(OverriddenFields::OverrideMinVolume),
            5 => Some(OverriddenFields::OverrideMinRange),
            6 => Some(OverriddenFields::OverrideMaxRange),
            7 => Some(OverriddenFields::OverridePriority),
            _ => None,
        }
    }

    fn to_overridden_flags_byte(&self) -> UnsignedByte {
        debug_assert!(
            OVERRIDE_COUNT <= UnsignedByte::BITS as usize,
            "override flags exceed UnsignedByte capacity"
        );

        let mut overridden_flags: UnsignedByte = 0;
        for field in 0..OVERRIDE_COUNT {
            let Some(override_field) = Self::field_from_index(field) else {
                continue;
            };
            if self.overridden_fields.test(override_field) {
                overridden_flags |= 1 << field;
            }
        }
        overridden_flags
    }

    fn set_overridden_flags_from_byte(&mut self, overridden_flags: UnsignedByte) {
        debug_assert!(
            OVERRIDE_COUNT <= UnsignedByte::BITS as usize,
            "override flags exceed UnsignedByte capacity"
        );

        for field in 0..OVERRIDE_COUNT {
            let Some(override_field) = Self::field_from_index(field) else {
                continue;
            };
            self.overridden_fields
                .set(override_field, (overridden_flags & (1 << field)) != 0);
        }
    }

    fn priority_to_cxx_byte(priority: AudioPriority) -> UnsignedByte {
        match priority {
            AudioPriority::Lowest => 0,
            AudioPriority::Low => 1,
            AudioPriority::Normal => 2,
            AudioPriority::High => 3,
            AudioPriority::Critical => 4,
        }
    }

    fn priority_from_cxx_byte(priority: UnsignedByte) -> AudioPriority {
        match priority {
            0 => AudioPriority::Lowest,
            1 => AudioPriority::Low,
            2 => AudioPriority::Normal,
            3 => AudioPriority::High,
            4 => AudioPriority::Critical,
            _ => AudioPriority::Normal,
        }
    }

    /// C++ parity: transfer all overridden fields except the customized audio name.
    pub fn xfer_no_name(&mut self, xfer: &mut dyn Xfer) -> io::Result<()> {
        // C++ uses UnsignedByte (u8) for version - matches C++ parity
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        if xfer.get_xfer_mode() == XferMode::Load {
            let mut overridden_flags = 0u8;
            xfer.xfer_unsigned_byte(&mut overridden_flags)?;
            self.set_overridden_flags_from_byte(overridden_flags);
        } else {
            let mut overridden_flags = self.to_overridden_flags_byte();
            xfer.xfer_unsigned_byte(&mut overridden_flags)?;
        }

        if self.was_loop_flag_overridden() {
            let mut loop_flag = (self.audio_event_info.control & AC_LOOP) != 0;
            xfer.xfer_bool(&mut loop_flag)?;
            if loop_flag {
                self.audio_event_info.control |= AC_LOOP;
            } else {
                self.audio_event_info.control &= !AC_LOOP;
            }
        }

        if self.was_loop_count_overridden() {
            xfer.xfer_int(&mut self.audio_event_info.loop_count)?;
        }

        if self.was_volume_overridden() {
            xfer.xfer_real(&mut self.audio_event_info.volume)?;
        }

        if self.was_min_volume_overridden() {
            xfer.xfer_real(&mut self.audio_event_info.min_volume)?;
        }

        if self.was_min_range_overridden() {
            xfer.xfer_real(&mut self.audio_event_info.min_distance)?;
        }

        if self.was_max_range_overridden() {
            xfer.xfer_real(&mut self.audio_event_info.max_distance)?;
        }

        if self.was_priority_overridden() {
            let mut priority = Self::priority_to_cxx_byte(self.audio_event_info.priority);
            xfer.xfer_unsigned_byte(&mut priority)?;
            self.audio_event_info.priority = Self::priority_from_cxx_byte(priority);
        }

        Ok(())
    }
}

impl Default for DynamicAudioEventInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DynamicAudioEventInfo {
    fn clone(&self) -> Self {
        DynamicAudioEventInfo {
            audio_event_info: AudioEventInfo {
                sound_type: self.audio_event_info.sound_type,
                control: self.audio_event_info.control,
                audio_name: self.audio_event_info.audio_name.clone(),
                volume: self.audio_event_info.volume,
                sounds: self.audio_event_info.sounds.clone(),
                attack_sounds: self.audio_event_info.attack_sounds.clone(),
                decay_sounds: self.audio_event_info.decay_sounds.clone(),
                pitch_shift_min: self.audio_event_info.pitch_shift_min,
                pitch_shift_max: self.audio_event_info.pitch_shift_max,
                volume_shift: self.audio_event_info.volume_shift,
                min_volume: self.audio_event_info.min_volume,
                limit: self.audio_event_info.limit,
                loop_count: self.audio_event_info.loop_count,
                delay_min: self.audio_event_info.delay_min,
                delay_max: self.audio_event_info.delay_max,
                filename: self.audio_event_info.filename.clone(),
                sound_type_field: self.audio_event_info.sound_type_field,
                type_field: self.audio_event_info.type_field,
                priority: self.audio_event_info.priority,
                min_distance: self.audio_event_info.min_distance,
                max_distance: self.audio_event_info.max_distance,
            },
            overridden_fields: self.overridden_fields.clone(),
            original_name: self.original_name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::system::xfer_load::XferLoad;
    use crate::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn make_base_info() -> AudioEventInfo {
        AudioEventInfo {
            sound_type: AudioType::SoundEffect,
            control: 0,
            audio_name: "TreeAmbient".to_string(),
            volume: 0.5,
            sounds: vec!["tree_ambient_a.wav".to_string()],
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
            filename: "tree_ambient_a.wav".to_string(),
            sound_type_field: AudioType::SoundEffect,
            type_field: 0,
            priority: AudioPriority::Normal,
            min_distance: 5.0,
            max_distance: 150.0,
        }
    }

    #[test]
    fn xfer_no_name_round_trip_preserves_overrides() {
        let base = make_base_info();

        let mut saved = DynamicAudioEventInfo::from_base_info(&base);
        saved.override_audio_name(" CUSTOM 77 TreeAmbient");
        saved.override_loop_flag(true);
        saved.override_loop_count(0);
        saved.override_volume(0.67);
        saved.override_min_volume(0.12);
        saved.override_min_range(15.0);
        saved.override_max_range(220.0);
        saved.override_priority(AudioPriority::Critical);

        let mut bytes = Vec::new();
        {
            let writer = Cursor::new(&mut bytes);
            let mut saver = XferSave::new(writer, 1);
            saved
                .xfer_no_name(&mut saver)
                .expect("failed to save dynamic audio overrides");
        }

        let mut loaded = DynamicAudioEventInfo::from_base_info(&base);
        {
            let reader = Cursor::new(bytes);
            let mut loader = XferLoad::new(reader, 1);
            loaded
                .xfer_no_name(&mut loader)
                .expect("failed to load dynamic audio overrides");
        }

        assert!(loaded.was_audio_name_overridden());
        assert!(loaded.was_loop_flag_overridden());
        assert!(loaded.was_loop_count_overridden());
        assert!(loaded.was_volume_overridden());
        assert!(loaded.was_min_volume_overridden());
        assert!(loaded.was_min_range_overridden());
        assert!(loaded.was_max_range_overridden());
        assert!(loaded.was_priority_overridden());
        assert_eq!(loaded.audio_event_info.loop_count, 0);
        assert_eq!((loaded.audio_event_info.control & AC_LOOP) != 0, true);
        assert!((loaded.audio_event_info.volume - 0.67).abs() < 0.0001);
        assert!((loaded.audio_event_info.min_volume - 0.12).abs() < 0.0001);
        assert!((loaded.audio_event_info.min_distance - 15.0).abs() < 0.0001);
        assert!((loaded.audio_event_info.max_distance - 220.0).abs() < 0.0001);
        assert_eq!(loaded.audio_event_info.priority, AudioPriority::Critical);
    }
}
