//! # High-Performance Sound Buffer Management
//!
//! Provides efficient audio buffer management with zero-copy operations where possible.

use super::{AudioDeviceError, AudioFormat, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Sound buffer format types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferFormat {
    /// Interleaved samples (LRLRLR...)
    Interleaved,
    /// Planar samples (LLL...RRR...)
    Planar,
}

/// Sound buffer state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferState {
    /// Buffer is empty/uninitialized
    Empty,
    /// Buffer is being loaded
    Loading,
    /// Buffer is ready for playback
    Ready,
    /// Buffer is currently being played
    Playing,
    /// Buffer has an error
    Error,
}

/// High-performance sound buffer
pub struct SoundBuffer {
    /// Buffer identifier
    pub id: uuid::Uuid,
    /// Audio format
    pub format: AudioFormat,
    /// Buffer format (interleaved/planar)
    pub buffer_format: BufferFormat,
    /// Audio data
    pub data: Arc<[u8]>,
    /// Buffer state
    pub state: BufferState,
    /// Sample count
    pub sample_count: usize,
    /// Duration in seconds
    pub duration: std::time::Duration,
}

impl SoundBuffer {
    /// Create a new empty sound buffer
    pub fn new(format: AudioFormat, buffer_format: BufferFormat) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            format,
            buffer_format,
            data: Arc::new([]),
            state: BufferState::Empty,
            sample_count: 0,
            duration: std::time::Duration::ZERO,
        }
    }

    /// Load audio data from bytes
    pub fn load_from_bytes(&mut self, data: &[u8]) -> Result<()> {
        self.state = BufferState::Loading;

        self.data = Arc::from(data);
        self.sample_count =
            data.len() / (self.format.channels as usize * self.format.bytes_per_sample() as usize);
        self.duration = std::time::Duration::from_secs_f64(
            self.sample_count as f64 / self.format.sample_rate as f64,
        );

        self.state = BufferState::Ready;
        Ok(())
    }

    /// Get audio samples as f32 slice
    pub fn get_samples_f32(&self) -> Result<Vec<f32>> {
        match self.format.bits_per_sample {
            16 => {
                let samples = self
                    .data
                    .chunks_exact(2)
                    .map(|chunk| {
                        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                        sample as f32 / i16::MAX as f32
                    })
                    .collect();
                Ok(samples)
            }
            32 => {
                let samples = self
                    .data
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                Ok(samples)
            }
            _ => Err(AudioDeviceError::FormatNotSupported(format!(
                "Unsupported bit depth: {}",
                self.format.bits_per_sample
            ))),
        }
    }
}
