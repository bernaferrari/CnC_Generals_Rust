//! High-precision timing system for audio synchronization.

use crate::error::Result;
use std::time::{Duration, Instant};

/// Time units for audio operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit {
    /// Time in milliseconds
    Milliseconds(u64),
    /// Time in audio samples
    Samples(u64),
    /// Time in audio frames
    Frames(u64),
}

/// Synchronization mode for audio timing
#[derive(Debug, Clone, Copy)]
pub enum SyncMode {
    /// No synchronization
    None,
    /// Synchronize to system clock
    SystemClock,
    /// Synchronize to audio hardware
    AudioHardware,
    /// Custom synchronization callback
    Custom,
}

/// High-precision audio timer
pub struct AudioTimer {
    start_time: Instant,
    _sync_mode: SyncMode,
    sample_rate: u32,
}

impl AudioTimer {
    /// Create new audio timer
    pub fn new(sync_mode: SyncMode, sample_rate: u32) -> Self {
        Self {
            start_time: Instant::now(),
            _sync_mode: sync_mode,
            sample_rate,
        }
    }

    /// Get current audio time
    pub fn current_time(&self) -> TimeUnit {
        let elapsed = self.start_time.elapsed();
        TimeUnit::Milliseconds(elapsed.as_millis() as u64)
    }

    /// Convert between time units
    pub fn convert_time(&self, time: TimeUnit, target_unit: TimeUnit) -> Result<u64> {
        match (time, target_unit) {
            (TimeUnit::Milliseconds(ms), TimeUnit::Samples(_)) => {
                Ok((ms * u64::from(self.sample_rate)) / 1000)
            }
            (TimeUnit::Samples(samples), TimeUnit::Milliseconds(_)) => {
                Ok((samples * 1000) / u64::from(self.sample_rate))
            }
            (TimeUnit::Frames(frames), TimeUnit::Samples(_)) => {
                // Assuming stereo (2 channels) - this should be configurable
                Ok(frames * 2)
            }
            (TimeUnit::Samples(samples), TimeUnit::Frames(_)) => Ok(samples / 2),
            _ => Ok(0), // Same unit conversion
        }
    }

    /// Reset timer
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
    }

    /// Wait for specific duration
    pub async fn wait(&self, duration: Duration) -> Result<()> {
        tokio::time::sleep(duration).await;
        Ok(())
    }

    /// Get high-precision timestamp
    pub fn timestamp(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }
}

/// Audio timing utilities
pub struct TimingUtils;

impl TimingUtils {
    /// Calculate buffer duration in milliseconds
    pub fn buffer_duration_ms(buffer_size: usize, sample_rate: u32, channels: u16) -> u64 {
        let samples_per_channel = buffer_size / usize::from(channels);
        (samples_per_channel as u64 * 1000) / u64::from(sample_rate)
    }

    /// Calculate required buffer size for duration
    pub fn buffer_size_for_duration(
        duration_ms: u64,
        sample_rate: u32,
        channels: u16,
        bytes_per_sample: u8,
    ) -> usize {
        let total_samples = (duration_ms * u64::from(sample_rate)) / 1000;
        (total_samples * u64::from(channels) * u64::from(bytes_per_sample)) as usize
    }

    /// Get current system timestamp in microseconds  
    pub fn system_timestamp_us() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }
}

impl Default for AudioTimer {
    fn default() -> Self {
        Self::new(SyncMode::SystemClock, 44100)
    }
}
