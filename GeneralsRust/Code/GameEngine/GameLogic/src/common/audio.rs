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

/// Audio event for RTS-style events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
            volume: 1.0,
        }
    }

    pub fn set_event_name(&mut self, name: impl Into<String>) {
        self.event_name = name.into();
    }

    pub fn get_event_name(&self) -> &str {
        &self.event_name
    }

    pub fn is_currently_playing(&self) -> bool {
        self.playing_handle != 0
    }

    pub fn get_playing_handle(&self) -> AudioHandle {
        self.playing_handle
    }

    pub fn set_playing_handle(&mut self, handle: AudioHandle) {
        self.playing_handle = handle;
    }

    pub fn set_object_id(&mut self, id: u32) {
        self.object_id = id;
    }

    pub fn set_drawable_id(&mut self, id: u32) {
        self.drawable_id = Some(id);
    }

    pub fn set_time_of_day(&mut self, time_of_day: TimeOfDay) {
        self.time_of_day = Some(time_of_day);
    }

    pub fn set_position(&mut self, pos: &(f32, f32, f32)) {
        self.position = Some(*pos);
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
}
