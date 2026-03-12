//! Shared enums and type definitions for sound objects.

/// Identifiers matching the original WWAudio SOUND_CLASSID enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundClassId {
    Unknown = 0,
    TwoD,
    ThreeD,
    Listener,
    Pseudo3D,
    TwoDTrigger,
    Logical,
    Filtered,
}

impl Default for SoundClassId {
    fn default() -> Self {
        Self::Unknown
    }
}

/// High-level categorisation of audible content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundType {
    SoundEffect,
    Music,
    Voice,
    Ambient,
}

impl Default for SoundType {
    fn default() -> Self {
        Self::SoundEffect
    }
}

/// Playback state for audible objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundState {
    Stopped,
    Playing,
    Paused,
    Stopping,
}

impl Default for SoundState {
    fn default() -> Self {
        Self::Stopped
    }
}

/// Flags describing how a sound should be culled or prioritised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundFlags {
    pub is_static: bool,
    pub is_culled: bool,
    pub persist_after_stop: bool,
}

impl Default for SoundFlags {
    fn default() -> Self {
        Self {
            is_static: false,
            is_culled: false,
            persist_after_stop: false,
        }
    }
}

impl SoundClassId {
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => SoundClassId::TwoD,
            2 => SoundClassId::ThreeD,
            3 => SoundClassId::Listener,
            4 => SoundClassId::Pseudo3D,
            5 => SoundClassId::TwoDTrigger,
            6 => SoundClassId::Logical,
            7 => SoundClassId::Filtered,
            _ => SoundClassId::Unknown,
        }
    }
}
