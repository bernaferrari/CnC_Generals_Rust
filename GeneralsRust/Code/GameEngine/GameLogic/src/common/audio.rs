//! Audio system utilities

use crate::common::*;
use serde::{Deserialize, Serialize};

/// Audio handle type
pub type AudioHandle = u32;

/// Audio type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioType {
    Music,
    Sound,
    Voice,
}

/// Audio affect enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioAffect {
    None,
    Volume,
    Pitch,
}

/// Time of day enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeOfDay {
    Morning,
    Day,
    Evening,
    Night,
}
/// C++ `AudioEventRTS::m_ownerType` (`AudioEventRTS.h`). Exclusive: once Object
/// or Drawable is set, later `setPosition` is a no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LeftoverAudioOwner {
    #[default]
    Invalid,
    Positional,
    Object,
    Drawable,
    Dead,
}

/// C++ `AudioEventRTS::m_volume` default `-1` means “use AudioEventInfo”.
pub const LEFTOVER_UNSET_VOLUME: f32 = -1.0;

/// Audio event for RTS-style events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEventRts {
    pub event_name: String,
    pub object_id: u32,
    pub drawable_id: Option<u32>,
    pub time_of_day: Option<TimeOfDay>,
    pub position: Option<(f32, f32, f32)>,
    pub player_index: Option<u32>,
    pub is_logical_audio: bool,
    pub uninterruptable: bool,
    pub should_fade: bool,
    pub playing_handle: AudioHandle,
    pub volume: f32,
    #[serde(default)]
    pub owner_type: LeftoverAudioOwner,
}

impl Default for AudioEventRts {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl AudioEventRts {
    pub fn with_event_name(event_name: &str) -> Self {
        Self::new(event_name)
    }

    pub fn new(event_name: impl Into<String>) -> Self {
        Self {
            event_name: event_name.into(),
            object_id: 0,
            drawable_id: None,
            time_of_day: None,
            position: None,
            player_index: None,
            is_logical_audio: false,
            uninterruptable: false,
            should_fade: false,
            playing_handle: 0,
            volume: LEFTOVER_UNSET_VOLUME,
            owner_type: LeftoverAudioOwner::Invalid,
        }
    }

    pub fn set_event_name(&mut self, name: impl Into<String>) {
        self.event_name = name.into();
    }

    pub fn get_event_name(&self) -> &str {
        &self.event_name
    }

    /// C++ `AudioEventRTS::isCurrentlyPlaying` (`AudioEventRTS.cpp:666-668`)
    /// queries `TheAudio->isCurrentlyPlaying(m_playingHandle)`. A leftover
    /// `playing_handle != 0` would keep Afterburner / train / silo loops from
    /// ever restarting after the sample ends.
    pub fn is_currently_playing(&self) -> bool {
        if self.playing_handle == 0 {
            return false;
        }
        game_engine::common::audio::game_audio::get_global_audio_manager()
            .and_then(|manager| {
                manager
                    .lock()
                    .ok()
                    .map(|guard| guard.is_currently_playing(self.playing_handle))
            })
            .unwrap_or(false)
    }

    pub fn get_playing_handle(&self) -> AudioHandle {
        self.playing_handle
    }

    pub fn set_playing_handle(&mut self, handle: AudioHandle) {
        self.playing_handle = handle;
    }

    pub fn set_object_id(&mut self, id: u32) {
        if !matches!(
            self.owner_type,
            LeftoverAudioOwner::Object | LeftoverAudioOwner::Invalid
        ) {
            return;
        }
        self.object_id = id;
        self.owner_type = LeftoverAudioOwner::Object;
    }

    pub fn set_drawable_id(&mut self, id: u32) {
        if !matches!(
            self.owner_type,
            LeftoverAudioOwner::Drawable | LeftoverAudioOwner::Invalid
        ) {
            return;
        }
        self.drawable_id = Some(id);
        self.owner_type = LeftoverAudioOwner::Drawable;
    }

    pub fn set_time_of_day(&mut self, time_of_day: TimeOfDay) {
        self.time_of_day = Some(time_of_day);
    }

    pub fn set_position(&mut self, pos: &(f32, f32, f32)) {
        if !matches!(
            self.owner_type,
            LeftoverAudioOwner::Positional | LeftoverAudioOwner::Invalid
        ) {
            return;
        }
        self.position = Some(*pos);
        self.owner_type = LeftoverAudioOwner::Positional;
    }

    pub fn set_player_index(&mut self, index: u32) {
        self.player_index = Some(index);
    }

    pub fn set_is_logical_audio(&mut self, is_logical_audio: bool) {
        self.is_logical_audio = is_logical_audio;
    }

    pub fn is_logical_audio(&self) -> bool {
        self.is_logical_audio
    }

    pub fn set_uninterruptable(&mut self, uninterruptable: bool) {
        self.uninterruptable = uninterruptable;
    }

    pub fn is_uninterruptable(&self) -> bool {
        self.uninterruptable
    }

    pub fn set_should_fade(&mut self, should_fade: bool) {
        self.should_fade = should_fade;
    }

    pub fn should_fade(&self) -> bool {
        self.should_fade
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
    }

    /// C++ `AudioEventRTS::getVolume`: `-1` means the caller did not scale.
    pub fn has_caller_volume(&self) -> bool {
        self.volume != LEFTOVER_UNSET_VOLUME
    }
}

#[cfg(test)]
mod leftover_audio_event_rts_tests {
    use super::*;

    #[test]
    fn is_currently_playing_queries_manager_not_nonzero_handle() {
        // C++ AudioEventRTS::isCurrentlyPlaying (AudioEventRTS.cpp:666-668)
        let mut ev = AudioEventRts::new("Afterburner");
        ev.set_playing_handle(1001);
        assert!(
            !ev.is_currently_playing(),
            "nonzero handle is not playing unless TheAudio reports the sample"
        );
    }

    #[test]
    fn leftover_the_audio_missing_info_returns_ahsv_error() {
        // C++ AudioManager::addAudioEvent (GameAudio.cpp:391-396)
        let audio = crate::helpers::TheAudio::get().unwrap();
        let event = AudioEventRts::new("DefinitelyNotARealAudioEvent_hq_wlv76");
        let handle = audio.add_audio_event(&event);
        assert_eq!(
            handle,
            game_engine::common::audio::game_audio::AHSV_ERROR,
            "missing AudioEventInfo must return AHSV_Error, not a blank invented event"
        );
    }

    #[test]
    fn leftover_the_audio_length_ms_missing_info_is_zero() {
        let audio = crate::helpers::TheAudio::get().unwrap();
        let event = AudioEventRts::new("DefinitelyNotARealAudioEvent_hq_wlv76_len");
        assert_eq!(audio.get_audio_length_ms(&event), 0.0);
    }

    #[test]
    fn leftover_set_object_id_ignores_later_position() {
        // C++ AudioEventRTS::setPosition is a no-op once owner is OT_Object.
        let mut ev = AudioEventRts::new("TrainClickety");
        ev.set_object_id(42);
        ev.set_position(&(10.0, 20.0, 30.0));
        assert_eq!(ev.owner_type, LeftoverAudioOwner::Object);
        assert_eq!(ev.object_id, 42);
        assert!(ev.position.is_none());
    }

    #[test]
    fn leftover_default_volume_is_unset() {
        // C++ AudioEventRTS default m_volume is -1 (use AudioEventInfo).
        let ev = AudioEventRts::new("Impact");
        assert_eq!(ev.volume, LEFTOVER_UNSET_VOLUME);
        assert!(!ev.has_caller_volume());
        let mut scaled = ev;
        scaled.set_volume(0.25);
        assert!(scaled.has_caller_volume());
    }
}
