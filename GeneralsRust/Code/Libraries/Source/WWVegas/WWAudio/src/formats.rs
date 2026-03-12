//! Audio format definitions and utilities.

use serde::{Deserialize, Serialize};

/// Audio format specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    /// Number of channels (1 = mono, 2 = stereo, etc.)
    pub channels: u16,
    /// Sample rate in Hz
    pub sample_rate: SampleRate,
    /// Bits per sample
    pub sample_width: SampleWidth,
    /// Channel layout
    pub channel_layout: ChannelLayout,
}

/// Sample rate enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleRate {
    Hz8000 = 8000,
    Hz11025 = 11025,
    Hz16000 = 16000,
    Hz22050 = 22050,
    Hz44100 = 44100,
    Hz48000 = 48000,
    Hz96000 = 96000,
    Hz192000 = 192000,
}

/// Sample width (bits per sample)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleWidth {
    U8 = 8,
    S16 = 16,
    S24 = 24,
    S32 = 32,
    F32 = 132, // Different discriminant, but still represents 32-bit float
}

/// Channel layout configurations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    Surround21,
    Surround41,
    Surround51,
    Surround71,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            channels: 2,
            sample_rate: SampleRate::Hz44100,
            sample_width: SampleWidth::S16,
            channel_layout: ChannelLayout::Stereo,
        }
    }
}

impl From<SampleRate> for u32 {
    fn from(rate: SampleRate) -> Self {
        rate as u32
    }
}

impl From<SampleWidth> for u8 {
    fn from(width: SampleWidth) -> Self {
        match width {
            SampleWidth::U8 => 8,
            SampleWidth::S16 => 16,
            SampleWidth::S24 => 24,
            SampleWidth::S32 => 32,
            SampleWidth::F32 => 32, // F32 still represents 32 bits
        }
    }
}

impl AudioFormat {
    /// Calculate bytes per second for this format
    pub fn bytes_per_second(&self) -> u32 {
        let sample_rate: u32 = self.sample_rate.into();
        let bits_per_sample: u8 = self.sample_width.into();
        sample_rate * u32::from(self.channels) * u32::from(bits_per_sample) / 8
    }

    /// Calculate bytes per frame
    pub fn bytes_per_frame(&self) -> usize {
        let bits_per_sample: u8 = self.sample_width.into();
        (usize::from(self.channels) * usize::from(bits_per_sample)) / 8
    }

    /// Check if format is supported
    pub fn is_supported(&self) -> bool {
        // Basic validation
        self.channels > 0 && self.channels <= 8
    }
}
