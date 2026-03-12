//! # Audio Device Layer
//!
//! This module provides the complete audio device abstraction layer, converting the original
//! Miles Sound System integration to modern Rust with cross-platform support.
//!
//! ## Architecture
//!
//! The audio system is built around these core components:
//!
//! - **MilesAudioDevice**: Main audio device interface, converted from C++ MilesAudioManager
//! - **AudioDriver**: Cross-platform audio driver abstraction
//! - **SoundBuffer**: High-performance audio buffer management
//! - **StreamingSound**: Efficient streaming for large audio files
//! - **DeviceManager**: Device enumeration, selection, and lifecycle management
//!
//! ## Features
//!
//! - Cross-platform audio support (WASAPI, ALSA, CoreAudio)
//! - Hardware acceleration when available
//! - 3D positional audio with HRTF support
//! - Real-time audio streaming
//! - Zero-latency audio processing
//! - Memory-safe audio buffer management

pub mod audio_driver;
pub mod device_manager;
pub mod error_recovery;
pub mod kira_audio_driver;
pub mod miles_audio_device;
pub mod sound_buffer;
pub mod spatial_audio;
pub mod streaming_sound;

// Re-exports
pub use audio_driver::{AudioDriver, DriverCapabilities, DriverType};
pub use device_manager::{AudioDeviceInfo, DeviceManager, DeviceSelection};
pub use error_recovery::{
    ErrorContext, ErrorRecoveryManager, PerformanceTracker, RecoveryAction, RecoveryError,
    RecoveryStrategy, ResourceMonitor,
};
pub use kira_audio_driver::{KiraAudioDriver, ModernAudioDevice};
pub use miles_audio_device::{MilesAudioConfig, MilesAudioDevice};
pub use sound_buffer::{BufferFormat, BufferState, SoundBuffer};
pub use spatial_audio::{
    AudioCone, EnvironmentalAudio, HrtfDatabase, SpatialAudioProcessor, SpatialAudioSource,
};
pub use streaming_sound::{StreamConfig, StreamState, StreamingSound};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Audio device error types
#[derive(Error, Debug)]
pub enum AudioDeviceError {
    /// Device initialization failed
    #[error("Device initialization failed: {0}")]
    InitializationFailed(String),

    /// Device not found
    #[error("Audio device not found: {0}")]
    DeviceNotFound(String),

    /// Driver error
    #[error("Audio driver error: {0}")]
    DriverError(String),

    /// Buffer error
    #[error("Audio buffer error: {0}")]
    BufferError(String),

    /// Streaming error
    #[error("Audio streaming error: {0}")]
    StreamingError(String),

    /// Format not supported
    #[error("Audio format not supported: {0}")]
    FormatNotSupported(String),

    /// Device busy or in use
    #[error("Audio device busy: {0}")]
    DeviceBusy(String),

    /// Hardware acceleration not available
    #[error("Hardware acceleration not available: {0}")]
    HardwareAccelNotAvailable(String),

    /// I/O error
    #[error("Audio I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Platform-specific error
    #[error("Platform error: {0}")]
    PlatformError(String),

    /// Spatial audio processing error
    #[error("Spatial audio error: {0}")]
    SpatialAudioError(String),

    /// Playback failed
    #[error("Audio playback failed: {0}")]
    PlaybackFailed(String),

    /// Invalid parameter
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

/// Result type for audio operations
pub type Result<T> = std::result::Result<T, AudioDeviceError>;

/// Audio format specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u16,
    /// Bits per sample
    pub bits_per_sample: u16,
    /// Audio format type
    pub format_type: AudioFormatType,
}

/// Audio format types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormatType {
    /// PCM integer samples
    PcmInt,
    /// PCM floating point samples
    PcmFloat,
    /// Compressed audio (MP3, etc.)
    Compressed(CompressionType),
}

/// Audio compression types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    /// MP3 compression
    Mp3,
    /// IMA ADPCM compression
    ImaAdpcm,
    /// Microsoft ADPCM compression
    MsAdpcm,
    /// Ogg Vorbis compression
    OggVorbis,
}

/// Simple sample format enum for device capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleFormat {
    /// 32-bit float samples
    F32,
    /// 16-bit integer samples
    I16,
    /// 32-bit integer samples
    I32,
}

/// Audio priority levels (matching original C++ enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum Priority {
    /// Low priority - first to be stopped under resource pressure
    Low = 0,
    /// Normal priority for most game audio
    Normal = 50,
    /// High priority for important gameplay audio  
    High = 80,
    /// Critical priority for UI and essential audio
    Critical = 100,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Simple device capabilities for Kira audio driver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleDeviceCapabilities {
    /// Supported sample rates
    pub sample_rates: Vec<u32>,
    /// Supported sample formats
    pub formats: Vec<SampleFormat>,
    /// Maximum input channels
    pub max_input_channels: u32,
    /// Maximum output channels
    pub max_output_channels: u32,
    /// Version string
    pub version: String,
}

/// Audio volume (0.0 - 1.0 range)
pub type Volume = f32;

/// Audio handle for tracking playing sounds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AudioHandle(u64);

impl AudioHandle {
    /// Invalid audio handle constant
    pub const INVALID: Self = Self(0);

    /// Create a new audio handle
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Check if handle is valid
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl Default for AudioHandle {
    fn default() -> Self {
        Self::INVALID
    }
}

/// 3D position in world space
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position3D {
    /// X coordinate
    pub x: f32,
    /// Y coordinate  
    pub y: f32,
    /// Z coordinate
    pub z: f32,
}

impl Position3D {
    /// Create new 3D position
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Origin position
    pub const fn origin() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Calculate distance to another position
    pub fn distance_to(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl From<[f32; 3]> for Position3D {
    fn from(pos: [f32; 3]) -> Self {
        Self::new(pos[0], pos[1], pos[2])
    }
}

impl From<Position3D> for [f32; 3] {
    fn from(pos: Position3D) -> Self {
        [pos.x, pos.y, pos.z]
    }
}

/// 3D velocity vector
pub type Velocity3D = Position3D;

/// Audio listener configuration for 3D audio
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioListener {
    /// Listener position
    pub position: Position3D,
    /// Listener velocity
    pub velocity: Velocity3D,
    /// Forward vector
    pub forward: Position3D,
    /// Up vector
    pub up: Position3D,
    /// Master volume
    pub volume: Volume,
}

impl Default for AudioListener {
    fn default() -> Self {
        Self {
            position: Position3D::origin(),
            velocity: Position3D::origin(),
            forward: Position3D::new(0.0, 0.0, -1.0), // Looking down negative Z
            up: Position3D::new(0.0, 1.0, 0.0),       // Y is up
            volume: 1.0,
        }
    }
}

/// Audio source configuration for 3D positioned sounds
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioSource {
    /// Source position
    pub position: Position3D,
    /// Source velocity  
    pub velocity: Velocity3D,
    /// Volume
    pub volume: Volume,
    /// Pitch multiplier
    pub pitch: f32,
    /// Whether sound loops
    pub looping: bool,
    /// Priority level
    pub priority: Priority,
    /// Maximum distance for 3D effects
    pub max_distance: f32,
    /// Minimum distance for volume calculation
    pub min_distance: f32,
}

impl Default for AudioSource {
    fn default() -> Self {
        Self {
            position: Position3D::origin(),
            velocity: Position3D::origin(),
            volume: 1.0,
            pitch: 1.0,
            looping: false,
            priority: Priority::Normal,
            max_distance: 100.0,
            min_distance: 1.0,
        }
    }
}

/// Audio effect parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioEffects {
    /// Reverb settings
    pub reverb: Option<ReverbSettings>,
    /// Echo/Delay settings
    pub echo: Option<EchoSettings>,
    /// Low-pass filter cutoff frequency
    pub low_pass_filter: Option<f32>,
    /// High-pass filter cutoff frequency
    pub high_pass_filter: Option<f32>,
}

/// Reverb effect settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReverbSettings {
    /// Room size (0.0 - 1.0)
    pub room_size: f32,
    /// Damping amount (0.0 - 1.0)
    pub damping: f32,
    /// Wet/dry mix (0.0 - 1.0)
    pub wet_level: f32,
    /// Dry level (0.0 - 1.0)
    pub dry_level: f32,
}

/// Echo/Delay effect settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EchoSettings {
    /// Delay time in milliseconds
    pub delay_ms: f32,
    /// Feedback amount (0.0 - 1.0)
    pub feedback: f32,
    /// Wet/dry mix (0.0 - 1.0)
    pub wet_level: f32,
}

/// Audio playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    /// Audio is currently playing
    Playing,
    /// Audio is paused
    Paused,
    /// Audio has stopped
    Stopped,
    /// Audio is being loaded/buffered
    Loading,
    /// Audio failed to load/play
    Error,
}

/// Audio statistics for monitoring performance
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AudioStatistics {
    /// Number of active channels
    pub active_channels: usize,
    /// Number of 2D samples playing
    pub samples_2d: usize,
    /// Number of 3D samples playing
    pub samples_3d: usize,
    /// Number of streaming sounds
    pub streams: usize,
    /// Total memory usage in bytes
    pub memory_usage: u64,
    /// CPU usage percentage
    pub cpu_usage: f32,
    /// Average latency in milliseconds
    pub average_latency: f32,
    /// Number of dropped frames/buffers
    pub dropped_frames: u64,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: 16,
            format_type: AudioFormatType::PcmInt,
        }
    }
}

impl AudioFormat {
    /// Create a new audio format
    pub const fn new(sample_rate: u32, channels: u16, bits_per_sample: u16) -> Self {
        Self {
            sample_rate,
            channels,
            bits_per_sample,
            format_type: AudioFormatType::PcmInt,
        }
    }

    /// Create CD quality format (44.1kHz, 16-bit stereo)
    pub const fn cd_quality() -> Self {
        Self::new(44100, 2, 16)
    }

    /// Create DVD quality format (48kHz, 16-bit stereo)
    pub const fn dvd_quality() -> Self {
        Self::new(48000, 2, 16)
    }

    /// Create high quality format (48kHz, 24-bit stereo)
    pub const fn high_quality() -> Self {
        Self::new(48000, 2, 24)
    }

    /// Get bytes per sample
    pub const fn bytes_per_sample(self) -> u16 {
        self.bits_per_sample / 8
    }

    /// Get bytes per frame (all channels)
    pub const fn bytes_per_frame(self) -> u32 {
        self.channels as u32 * self.bytes_per_sample() as u32
    }

    /// Calculate bytes needed for duration
    pub fn bytes_for_duration(self, duration: Duration) -> u64 {
        let frames = (duration.as_secs_f64() * self.sample_rate as f64) as u64;
        frames * self.bytes_per_frame() as u64
    }

    /// Check if format is compatible with another format
    pub fn is_compatible_with(self, other: Self) -> bool {
        self.sample_rate == other.sample_rate && self.channels == other.channels
    }
}

/// Audio device capabilities and information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    /// Device supports hardware mixing
    pub hardware_mixing: bool,
    /// Device supports 3D audio
    pub hardware_3d: bool,
    /// Supported sample rates
    pub supported_sample_rates: Vec<u32>,
    /// Maximum number of channels
    pub max_channels: u16,
    /// Minimum buffer size in frames
    pub min_buffer_size: u32,
    /// Maximum buffer size in frames
    pub max_buffer_size: u32,
    /// Supported audio formats
    pub supported_formats: Vec<AudioFormat>,
    /// Device latency in milliseconds
    pub latency_ms: f32,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self {
            hardware_mixing: false,
            hardware_3d: false,
            supported_sample_rates: vec![44100, 48000],
            max_channels: 2,
            min_buffer_size: 256,
            max_buffer_size: 8192,
            supported_formats: vec![AudioFormat::default()],
            latency_ms: 10.0,
        }
    }
}
