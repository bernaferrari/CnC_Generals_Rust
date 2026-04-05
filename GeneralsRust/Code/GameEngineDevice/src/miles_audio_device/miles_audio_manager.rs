//! Miles Audio Manager Module
//!
//! Corresponds to C++ file: GeneralsMD/Code/GameEngineDevice/Include/MilesAudioDevice/MilesAudioManager.h
//!
//! Audio playback via rodio, replacing the original Miles Sound System backend.

use std::cell::Cell;
use std::ffi::c_void;
use std::path::Path;
use std::ptr;

use rodio::Sink;

pub struct MilesAudioManager {
    stream: Option<rodio::OutputStream>,
    stream_handle: Option<rodio::OutputStreamHandle>,
    master_volume: f32,
    initialized: bool,
    listener_position: Cell<[f32; 3]>,
    listener_forward: Cell<[f32; 3]>,
    listener_up: Cell<[f32; 3]>,
}

impl MilesAudioManager {
    pub fn new() -> Self {
        Self {
            stream: None,
            stream_handle: None,
            master_volume: 1.0,
            initialized: false,
            listener_position: Cell::new([0.0; 3]),
            listener_forward: Cell::new([0.0, 0.0, -1.0]),
            listener_up: Cell::new([0.0, 1.0, 0.0]),
        }
    }

    pub fn initialize(&mut self) -> Result<(), MilesError> {
        if self.initialized {
            return Ok(());
        }

        let (stream, handle) =
            rodio::OutputStream::try_default().map_err(|_| MilesError::InitializationFailed)?;

        self.stream = Some(stream);
        self.stream_handle = Some(handle);
        self.initialized = true;
        Ok(())
    }

    pub fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }

        self.stream_handle = None;
        self.stream = None;
        self.initialized = false;
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    pub fn get_master_volume(&self) -> f32 {
        self.master_volume
    }

    pub fn load_sample(&self, filename: &str) -> Result<MilesSampleHandle, MilesError> {
        if !self.initialized {
            return Err(MilesError::NotInitialized);
        }

        let path = Path::new(filename);
        let file = std::fs::File::open(path).map_err(|_| MilesError::SampleNotFound)?;

        use std::io::BufReader;
        let decoder =
            rodio::Decoder::new(BufReader::new(file)).map_err(|_| MilesError::SampleNotFound)?;

        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();
        let samples: Vec<f32> = decoder.collect();

        if samples.is_empty() {
            return Err(MilesError::SampleNotFound);
        }

        Ok(MilesSampleHandle::from_decoded(
            sample_rate,
            channels,
            samples,
        ))
    }

    pub fn play_sample(
        &self,
        sample: &MilesSampleHandle,
        volume: f32,
        looping: bool,
    ) -> Result<(), MilesError> {
        if !self.initialized {
            return Err(MilesError::NotInitialized);
        }
        if !sample.is_valid() {
            return Err(MilesError::SampleNotFound);
        }

        let handle = self
            .stream_handle
            .as_ref()
            .ok_or(MilesError::NotInitialized)?;
        let sink = Sink::try_new(handle).map_err(|_| MilesError::HardwareError)?;

        let source = rodio::buffer::SamplesBuffer::new(
            sample.channels,
            sample.sample_rate,
            sample.samples.clone(),
        );

        sink.set_volume(volume * self.master_volume);

        if looping {
            sink.append(source.repeat_infinite());
        } else {
            sink.append(source);
        }

        sink.detach();
        Ok(())
    }

    pub fn set_3d_listener(
        &self,
        position: [f32; 3],
        forward: [f32; 3],
        up: [f32; 3],
    ) -> Result<(), MilesError> {
        if !self.initialized {
            return Err(MilesError::NotInitialized);
        }

        self.listener_position.set(position);
        self.listener_forward.set(forward);
        self.listener_up.set(up);
        Ok(())
    }

    pub fn set_sample_position(&mut self, _position: [f32; 3]) -> Result<(), MilesError> {
        if !self.initialized {
            return Err(MilesError::NotInitialized);
        }
        Ok(())
    }

    pub fn update(&mut self) {
        if !self.initialized {
            return;
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for MilesAudioManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MilesAudioManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct MilesSampleHandle {
    handle: *mut c_void,
    sample_rate: u32,
    channels: u16,
    samples: Vec<f32>,
}

impl MilesSampleHandle {
    pub fn new(handle: *mut c_void) -> Self {
        Self {
            handle,
            sample_rate: 0,
            channels: 0,
            samples: Vec::new(),
        }
    }

    pub fn from_decoded(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Self {
        Self {
            handle: ptr::null_mut(),
            sample_rate,
            channels,
            samples,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.samples.is_empty()
    }
}

/// Miles Sound System errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilesError {
    NotInitialized,
    InitializationFailed,
    InvalidFilename,
    SampleNotFound,
    OutOfMemory,
    HardwareError,
    Unknown,
}

impl std::fmt::Display for MilesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MilesError::NotInitialized => write!(f, "Miles Audio Manager not initialized"),
            MilesError::InitializationFailed => {
                write!(f, "Failed to initialize Miles Audio Manager")
            }
            MilesError::InvalidFilename => write!(f, "Invalid filename provided"),
            MilesError::SampleNotFound => write!(f, "Audio sample not found"),
            MilesError::OutOfMemory => write!(f, "Out of memory"),
            MilesError::HardwareError => write!(f, "Audio hardware error"),
            MilesError::Unknown => write!(f, "Unknown Miles Audio Manager error"),
        }
    }
}

impl std::error::Error for MilesError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miles_manager_creation() {
        let manager = MilesAudioManager::new();
        assert!(!manager.is_initialized());
        assert_eq!(manager.get_master_volume(), 1.0);
    }

    #[test]
    fn test_volume_clamping() {
        let mut manager = MilesAudioManager::new();

        manager.set_master_volume(-0.5);
        assert_eq!(manager.get_master_volume(), 0.0);

        manager.set_master_volume(1.5);
        assert_eq!(manager.get_master_volume(), 1.0);

        manager.set_master_volume(0.5);
        assert_eq!(manager.get_master_volume(), 0.5);
    }

    #[test]
    fn test_sample_handle() {
        let handle = MilesSampleHandle::new(ptr::null_mut());
        assert!(!handle.is_valid());
    }
}
