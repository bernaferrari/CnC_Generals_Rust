////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! AudioRequest structure
//! EA Pacific
//! John McDonald, Jr
//! Converted to Rust

use crate::common::audio::audio_event_rts::AudioEventRts;

// Type aliases for compatibility
pub type AudioHandle = u32;
pub type Bool = bool;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestType {
    Play,
    Pause,
    Stop,
}

/// Represents a request to the audio system
/// This is used to queue audio operations
pub struct AudioRequest {
    pub request: RequestType,
    pub request_data: RequestData,
    pub use_pending_event: Bool,
    pub requires_check_for_sample: Bool,
}

/// Union-like enum to hold either a pending event or handle
#[derive(Debug)]
pub enum RequestData {
    PendingEvent(Box<AudioEventRts>),
    HandleToInteractOn(AudioHandle),
}

impl AudioRequest {
    /// Create a new AudioRequest with an AudioEventRts
    pub fn new_with_event(request: RequestType, event: AudioEventRts) -> Self {
        AudioRequest {
            request,
            request_data: RequestData::PendingEvent(Box::new(event)),
            use_pending_event: true,
            requires_check_for_sample: false,
        }
    }

    /// Create a new AudioRequest with an AudioHandle
    pub fn new_with_handle(request: RequestType, handle: AudioHandle) -> Self {
        AudioRequest {
            request,
            request_data: RequestData::HandleToInteractOn(handle),
            use_pending_event: false,
            requires_check_for_sample: false,
        }
    }

    /// Get the pending event if this request contains one
    pub fn get_pending_event(&self) -> Option<&AudioEventRts> {
        match &self.request_data {
            RequestData::PendingEvent(event) => Some(event),
            _ => None,
        }
    }

    /// Get the handle if this request contains one
    pub fn get_handle(&self) -> Option<AudioHandle> {
        match &self.request_data {
            RequestData::HandleToInteractOn(handle) => Some(*handle),
            _ => None,
        }
    }

    /// Get mutable reference to pending event if this request contains one
    pub fn get_pending_event_mut(&mut self) -> Option<&mut AudioEventRts> {
        match &mut self.request_data {
            RequestData::PendingEvent(event) => Some(event),
            _ => None,
        }
    }

    /// Take ownership of the pending event, consuming the request data
    pub fn take_pending_event(self) -> Option<AudioEventRts> {
        match self.request_data {
            RequestData::PendingEvent(event) => Some(*event),
            _ => None,
        }
    }

    /// Set whether this request requires a sample check
    pub fn set_requires_check_for_sample(&mut self, requires_check: Bool) {
        self.requires_check_for_sample = requires_check;
    }

    /// Check if this request requires a sample check
    pub fn requires_check_for_sample(&self) -> Bool {
        self.requires_check_for_sample
    }
}

impl Default for AudioRequest {
    fn default() -> Self {
        AudioRequest {
            request: RequestType::Stop,
            request_data: RequestData::HandleToInteractOn(0),
            use_pending_event: false,
            requires_check_for_sample: false,
        }
    }
}

impl Clone for AudioRequest {
    fn clone(&self) -> Self {
        let cloned_data = match &self.request_data {
            RequestData::PendingEvent(event) => RequestData::PendingEvent(event.clone()),
            RequestData::HandleToInteractOn(handle) => RequestData::HandleToInteractOn(*handle),
        };

        AudioRequest {
            request: self.request,
            request_data: cloned_data,
            use_pending_event: self.use_pending_event,
            requires_check_for_sample: self.requires_check_for_sample,
        }
    }
}
