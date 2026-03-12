//! # WPAudio - Westwood Pacific Audio System
//!
//! This crate provides a Rust conversion of the original WPAudio library used in
//! Command & Conquer Generals Zero Hour and other Westwood Pacific games.
//!
//! The WPAudio system provides comprehensive audio functionality including:
//! - Multi-channel audio mixing and playback
//! - Various audio format support (WAV, MP3, ADPCM)
//! - Real-time audio streaming for large files  
//! - Audio caching and memory management
//! - 3D positional audio capabilities
//! - Hardware-accelerated audio where available
//! - Thread-safe audio operations
//!
//! ## Architecture
//!
//! The WPAudio system is built around these core components:
//!
//! ### Device Layer
//! - **AudioDevice**: Hardware abstraction and device management
//! - **AudioDriver**: Platform-specific audio drivers (DirectSound, ALSA, etc.)
//! - **DeviceEnumeration**: Audio device discovery and capability detection
//!
//! ### Channel Management  
//! - **AudioChannel**: Individual audio playback channels
//! - **ChannelManager**: Channel allocation and lifecycle management
//! - **MixingEngine**: Real-time audio mixing and effects processing
//!
//! ### Audio Sources and Streaming
//! - **AudioSource**: Audio data abstraction (files, memory, streams)
//! - **StreamingEngine**: Large file streaming with buffering
//! - **CompressionHandler**: Audio format decoding (MP3, ADPCM, etc.)
//!
//! ### Memory and Caching
//! - **AudioCache**: Efficient memory management for frequently used sounds
//! - **MemoryManager**: Custom allocators for audio buffers
//! - **BufferPool**: Pre-allocated buffer management
//!
//! ### Utility Systems
//! - **TimingSystem**: High-precision audio timing and synchronization  
//! - **EventSystem**: Audio event management and callbacks
//! - **ProfilerSystem**: Performance monitoring and debugging
//!
//! ## Example Usage
//!
//! ```rust
//! use wp_audio::{AudioFormat, AudioResult, AudioSystem, Priority};
//!
//! async fn play_game_sound() -> AudioResult<()> {
//!     // Initialize the audio system
//!     let mut audio_system = AudioSystem::new().await?;
//!     
//!     // Open the default audio device
//!     let device = audio_system.open_device(None).await?;
//!     
//!     // Load a sound effect from file
//!     let explosion_sound = audio_system.load_source("sounds/explosion.wav").await?;
//!     
//!     // Create a channel and play the sound
//!     let mut channel = device.create_channel(Priority::High)?;
//!     channel.play_source(explosion_sound, false)?;
//!     
//!     // Wait for playback to complete
//!     channel.wait_for_completion().await?;
//!     
//!     Ok(())
//! }
//! ```

#![cfg_attr(doc, warn(missing_docs))]
#![cfg_attr(doc, warn(rustdoc::missing_crate_level_docs))]
#![cfg_attr(not(doc), allow(missing_docs))]
#![cfg_attr(not(doc), allow(rustdoc::missing_crate_level_docs))]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::struct_excessive_bools)]

// Core audio system modules
pub mod aud_source;
mod backend;
pub mod cache;
pub mod channel;
pub mod device;
mod mixer;
pub mod source;
pub mod streamer;

// Advanced streaming modules
pub mod aud_stream_buffering;
pub mod aud_streamer;

// Complete audio system modules
pub mod audio_3d_complete;
pub mod audio_events_complete;
pub mod streaming;
pub mod voice_system;

// Memory and resource management
pub mod aud_time;
pub mod memory;
pub mod profiler;
pub mod time;

// Audio format and compression support
pub mod compression;
pub mod formats;

// Event and callback systems
pub mod attributes;
pub mod events;

// Utility modules
pub mod assert;
pub mod audible_sound;
pub mod handles;
pub mod level;
pub mod list;
pub mod listener;
pub mod lock;
pub mod logical;
pub mod logical_listener;
pub mod logical_sound;
pub mod math;
pub mod save_load;
pub mod sound3d;
pub mod sound_buffer;
pub mod sound_pseudo3d;
pub mod sound_scene;
pub mod sound_scene_obj;
pub mod sound_types;
pub mod utils;
pub mod wwaudio;
pub mod wwaudio_handles;

pub mod aab_tree_sound_cull_class;
pub mod audio_save_load;
pub mod filtered_sound;
pub mod listenerhandle;
pub mod priority_vector;
pub mod sound2dhandle;
pub mod sound3dhandle;
pub mod sound_chunk_i_ds;
pub mod sound_chunk_ids;
pub mod sound_cull_obj;
pub mod sound_pseudo3_d;
pub mod soundhandle;
pub mod soundstreamhandle;
pub mod threads;
pub use wwaudio::{DriverInfo, DriverType2D, DriverType3D};

// Platform-specific implementations
#[cfg(windows)]
pub mod aud_windows;
#[cfg(windows)]
pub mod dsound;
#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
pub mod unix;

// Internal modules (not exposed in public API)
mod buffer;
pub mod chunk;
mod error;
mod output;
mod thread_pool;

// Core modules already declared above

// Examples and testing (only in test builds)
#[cfg(feature = "examples")]
pub mod streaming_examples;

// Re-export core types and functions at crate level
pub use crate::{
    aud_source::AudioSample,
    aud_time::TimeStamp,
    audible_sound::AudibleSound,
    audio_3d_complete::{
        AttenuationModel, Audio3DBatchProcessor, Audio3DConfig, Audio3DProcessor, Audio3DResult,
        Listener3DConfig, DEFAULT_MAX_DISTANCE, DEFAULT_MIN_DISTANCE, DEFAULT_ROLLOFF_FACTOR,
        SPEED_OF_SOUND_M_S,
    },
    audio_events_complete::{
        AudioBus, AudioCategory, AudioEvent, AudioEventCallback, AudioEventSystem, AudioFader,
        AudioStopReason,
    },
    channel::{AudioChannel, ChannelState, ChannelType},
    device::{
        AudioDevice, AudioFileFactory, AudioPreferences, AudioSystem, DeviceCapabilities,
        DeviceConfig, DeviceInfo, Driver2DInfo, Driver2DKind, LogicalEvent,
    },
    error::{Error as AudioError, Result as AudioResult, StreamError},
    formats::AudioFormat,
    formats::{ChannelLayout, SampleRate, SampleWidth},
    handles::{BaseSoundHandle, ListenerHandle, Sound2DHandle, Sound3DHandle, SoundStreamHandle},
    listener::Listener3D,
    logical_listener::LogicalListener,
    logical_sound::LogicalSound,
    math::{Matrix3D, Vector3},
    mixer::{
        AudioMixer, CrossfadeState, MixBuffer, MixRenderStats, MixerConfig, MixerEvent,
        MixerTimelineSnapshot, MusicManager, VoiceDescriptor, VoiceHandle, VoiceId, VoiceParams,
        VoicePlaybackState, VoiceSpatialMode, VoiceSpatialParams, VoiceStopReason,
        VoiceTimelineState,
    },
    save_load::{
        AudioLoadDeserializer, AudioSaveSerializer, DynamicAudioSaveLoad, StaticAudioSaveLoad,
    },
    sound3d::Sound3D,
    sound_buffer::{SoundBufferClass, StreamSoundBufferClass},
    sound_pseudo3d::SoundPseudo3D,
    sound_scene::SceneSound,
    sound_scene::SoundScene,
    sound_scene_obj::{SoundObjectId, SoundSceneObject},
    sound_types::{SoundClassId, SoundFlags, SoundState, SoundType},
    source::AudioSource,
    streaming::{AudioStreamer, StreamEvent, StreamHandle, StreamState},
    utils::{get_filename_from_path, MMSLockClass},
    voice_system::{VoiceCategory, VoiceSystem, VoiceSystemConfig},
    wwaudio::WWAudioClass,
    wwaudio_handles::WWHandle,
};

// Platform-specific re-exports
#[cfg(windows)]
pub use crate::aud_windows::{
    get_windows_handle, millis_to_duration, read_wave_file_format, seconds_to_duration,
    set_windows_handle, windows_debug_print, AudThread, AudThreadCallback, AudThreadPriority,
    AudioCompression, AudioFormatFlags, AudioServiceInfo, ExtendedAudioFormat, ProfileCPU,
};

/// Audio priority levels for channel and event management
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    /// Lowest priority - first to be stopped under resource pressure
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

/// Volume levels (0-100 scale, matching original WPAudio API)
pub type Volume = u8;

/// Maximum volume level
pub const MAX_VOLUME: Volume = 100;

/// Minimum volume level (silence)
pub const MIN_VOLUME: Volume = 0;

/// Default volume level
pub const DEFAULT_VOLUME: Volume = 80;

/// Audio system configuration
#[derive(Debug, Clone)]
pub struct AudioSystemConfig {
    /// Maximum number of concurrent channels
    pub max_channels: usize,
    /// Audio cache size in bytes
    pub cache_size_bytes: usize,
    /// Cache block size for streaming
    pub cache_block_size: usize,
    /// Maximum number of cached audio items
    pub max_cache_items: usize,
    /// Enable performance profiling
    pub enable_profiling: bool,
    /// Enable debug assertions and logging
    pub debug_mode: bool,
    /// Default audio format for new sources
    pub default_format: AudioFormat,
    /// Audio thread pool size
    pub thread_pool_size: usize,
    /// Streaming buffer size in frames
    pub stream_buffer_frames: usize,
    /// Mixer buffer size in frames
    pub mixer_buffer_frames: usize,
    /// Maximum simultaneously active 2D samples
    pub max_2d_samples: u32,
    /// Maximum simultaneously active 3D samples
    pub max_3d_samples: u32,
    /// Maximum size for 2D sound buffers (bytes)
    pub max_2d_buffer_bytes: usize,
    /// Maximum size for 3D sound buffers (bytes)
    pub max_3d_buffer_bytes: usize,
    /// Default global sound-effects volume (0.0 - 1.0)
    pub default_sound_volume: f32,
    /// Default global music volume (0.0 - 1.0)
    pub default_music_volume: f32,
    /// Whether sound effects start enabled
    pub sound_effects_enabled: bool,
    /// Whether music starts enabled
    pub music_enabled: bool,
    /// Default reverb level (0.0 - 1.0)
    pub default_reverb_level: f32,
    /// Default reverb room type identifier
    pub default_reverb_room_type: i32,
}

impl Default for AudioSystemConfig {
    fn default() -> Self {
        Self {
            max_channels: 32,
            cache_size_bytes: 32 * 1024 * 1024, // 32MB cache
            cache_block_size: 8 * 1024,         // 8KB blocks
            max_cache_items: 2000,
            enable_profiling: cfg!(debug_assertions),
            debug_mode: cfg!(debug_assertions),
            default_format: AudioFormat::default(),
            thread_pool_size: shared::platform::cpu_count().max(2).min(8),
            stream_buffer_frames: 4096,
            mixer_buffer_frames: 2048,
            max_2d_samples: 16,
            max_3d_samples: 16,
            max_2d_buffer_bytes: 20_000,
            max_3d_buffer_bytes: 100_000,
            default_sound_volume: DEFAULT_VOLUME as f32 / MAX_VOLUME as f32,
            default_music_volume: DEFAULT_VOLUME as f32 / MAX_VOLUME as f32,
            sound_effects_enabled: true,
            music_enabled: true,
            default_reverb_level: 0.0,
            default_reverb_room_type: 0,
        }
    }
}

/// Global audio system state and version information
pub struct AudioSystemInfo {
    /// Library version string
    pub version: &'static str,
    /// Build timestamp  
    pub build_date: &'static str,
    /// Supported audio formats
    pub supported_formats: &'static [&'static str],
    /// Available audio backends
    pub available_backends: Vec<String>,
}

impl Default for AudioSystemInfo {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            build_date: option_env!("BUILD_DATE").unwrap_or("unknown"),
            supported_formats: &["WAV", "MP3", "ADPCM", "IMA-ADPCM"],
            available_backends: get_available_backends(),
        }
    }
}

/// Get information about available audio backends on this platform
fn get_available_backends() -> Vec<String> {
    let mut backends = Vec::new();

    #[cfg(windows)]
    {
        backends.push("DirectSound".to_string());
        backends.push("WASAPI".to_string());
    }

    #[cfg(unix)]
    {
        backends.push("ALSA".to_string());
        backends.push("PulseAudio".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        backends.push("CoreAudio".to_string());
    }

    backends.push("Cross-platform (cpal)".to_string());
    backends
}

/// Initialize the WPAudio system with default configuration
pub async fn init() -> AudioResult<AudioSystem> {
    AudioSystem::new_with_config(AudioSystemConfig::default()).await
}

/// Initialize the WPAudio system with custom configuration  
pub async fn init_with_config(config: AudioSystemConfig) -> AudioResult<AudioSystem> {
    AudioSystem::new_with_config(config).await
}

/// Get system information
pub fn system_info() -> AudioSystemInfo {
    AudioSystemInfo::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_volume_constants() {
        assert_eq!(MIN_VOLUME, 0);
        assert_eq!(MAX_VOLUME, 100);
        assert!(DEFAULT_VOLUME > MIN_VOLUME);
        assert!(DEFAULT_VOLUME <= MAX_VOLUME);
    }

    #[test]
    fn test_config_defaults() {
        let config = AudioSystemConfig::default();
        assert!(config.max_channels > 0);
        assert!(config.cache_size_bytes > 0);
        assert!(config.thread_pool_size > 0);
    }

    #[tokio::test]
    async fn test_system_initialization() {
        // This test might fail in CI without audio hardware
        if let Ok(system) = init().await {
            let info = system_info();
            assert!(!info.version.is_empty());
            assert!(!info.supported_formats.is_empty());
        }
    }
}
