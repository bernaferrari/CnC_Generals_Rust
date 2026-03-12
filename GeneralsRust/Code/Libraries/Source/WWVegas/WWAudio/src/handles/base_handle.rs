//! Common utilities shared by sound handle implementations.

use crate::AudioSource;
use std::sync::Arc;

/// Convert Miles-style 0-127 volumes into WPAudio's 0-100 scale.
pub fn miles_to_volume(volume: i32) -> crate::Volume {
    let clamped = volume.clamp(0, 127);
    ((clamped * 100) / 127).clamp(0, 100) as crate::Volume
}

/// Convert WPAudio volumes back into Miles 0-127 range.
pub fn volume_to_miles(volume: crate::Volume) -> i32 {
    (i32::from(volume) * 127 / 100).clamp(0, 127)
}

/// Shared state for handle types.
#[derive(Default, Clone)]
pub struct BaseSoundHandle {
    buffer: Option<Arc<AudioSource>>,
    miles_handle: Option<u32>,
}

impl BaseSoundHandle {
    pub fn new() -> Self {
        Self {
            buffer: None,
            miles_handle: None,
        }
    }

    /// Attach the audio buffer this handle will control.
    pub fn initialize(&mut self, buffer: Arc<AudioSource>) {
        self.buffer = Some(buffer);
    }

    /// Set the legacy Miles handle identifier (for tooling interoperability).
    pub fn set_miles_handle(&mut self, handle: u32) {
        self.miles_handle = Some(handle);
    }

    /// Retrieve the stored Miles handle identifier, if any.
    pub fn miles_handle(&self) -> Option<u32> {
        self.miles_handle
    }

    /// Access the attached buffer.
    pub fn buffer(&self) -> Option<Arc<AudioSource>> {
        self.buffer.clone()
    }
}
