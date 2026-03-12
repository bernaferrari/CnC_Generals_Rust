//! Audio source management and format handling.

use crate::{
    aud_source::{
        AudioCompressionType, AudioFormatFlags, AudioSample, AudioSourceLoader, CompressionData,
        EnhancedAudioFormat,
    },
    error::Result,
    formats::{AudioFormat, ChannelLayout, SampleRate, SampleWidth},
    Priority,
};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc};
use tokio::{fs, task};

/// Audio source types
#[derive(Debug, Clone)]
pub enum SourceType {
    /// File-based source
    File(String),
    /// Memory-based source
    Memory(Vec<u8>),
    /// Streaming source
    Stream(String),
}

/// Audio source configuration
#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub format: AudioFormat,
    pub preload: bool,
    pub cache_priority: Priority,
}

/// Audio source representation
#[derive(Clone)]
pub struct AudioSource {
    source_type: SourceType,
    config: SourceConfig,
    metadata: SourceMetadata,
    sample: Option<Arc<AudioSample>>,
}

/// Source metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMetadata {
    pub duration_ms: u64,
    pub file_size: usize,
    pub format: AudioFormat,
    pub bitrate: Option<u32>,
}

impl AudioSource {
    /// Load audio source from file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let file_path = path.clone();

        // Gather file metadata asynchronously
        let file_metadata = fs::metadata(&file_path).await?;

        // Decode using the high-fidelity loader on a blocking thread
        let (sample, enhanced_format) =
            task::spawn_blocking(move || AudioSourceLoader::load_file(&file_path))
                .await
                .map_err(|e| {
                    crate::error::Error::Audio(format!("Audio decode task failed: {e}"))
                })??;

        let audio_format = convert_enhanced_to_basic(&enhanced_format);
        let duration_ms = compute_duration(sample.bytes, enhanced_format.bytes_per_second);
        let bitrate = compute_bitrate(enhanced_format.bytes_per_second);

        Ok(Self {
            source_type: SourceType::File(path.to_string_lossy().into_owned()),
            config: SourceConfig::default(),
            metadata: SourceMetadata {
                duration_ms,
                file_size: file_metadata.len() as usize,
                format: audio_format,
                bitrate,
            },
            sample: Some(Arc::new(sample)),
        })
    }

    /// Create audio source from memory buffer
    pub fn from_memory(data: Vec<u8>, format: AudioFormat) -> Result<Self> {
        let mut sample = AudioSample::new();
        sample.bytes = data.len() as u32;
        sample.data = Some(data.clone());
        sample.format = Some(Box::new(enhanced_from_basic(&format)));

        let bytes_per_second = format.bytes_per_second();
        let duration_ms = compute_duration(sample.bytes, bytes_per_second);
        let bitrate = compute_bitrate(bytes_per_second);

        Ok(Self {
            source_type: SourceType::Memory(data),
            config: SourceConfig {
                format,
                preload: true,
                cache_priority: Priority::Normal,
            },
            metadata: SourceMetadata {
                duration_ms,
                file_size: sample.bytes as usize,
                format,
                bitrate,
            },
            sample: Some(Arc::new(sample)),
        })
    }

    /// Create streaming source from file
    pub async fn from_stream<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let file_path = path.clone();
        let file_metadata = fs::metadata(&file_path).await?;

        // Probe the stream to derive format information without retaining decoded data
        let enhanced_format = task::spawn_blocking(move || -> Result<EnhancedAudioFormat> {
            let (_sample, format) = AudioSourceLoader::load_file(&file_path)?;
            Ok(format)
        })
        .await
        .map_err(|e| crate::error::Error::Audio(format!("Audio probe task failed: {e}")))??;

        let audio_format = convert_enhanced_to_basic(&enhanced_format);
        let duration_ms =
            compute_duration(file_metadata.len() as u32, enhanced_format.bytes_per_second);
        let bitrate = compute_bitrate(enhanced_format.bytes_per_second);

        Ok(Self {
            source_type: SourceType::Stream(path.to_string_lossy().into_owned()),
            config: SourceConfig {
                format: audio_format,
                preload: false,
                cache_priority: Priority::Normal,
            },
            metadata: SourceMetadata {
                duration_ms,
                file_size: file_metadata.len() as usize,
                format: audio_format,
                bitrate,
            },
            sample: None,
        })
    }

    /// Get source metadata
    pub fn metadata(&self) -> &SourceMetadata {
        &self.metadata
    }

    /// Get source configuration
    pub fn config(&self) -> &SourceConfig {
        &self.config
    }

    /// Get audio format description for this source
    pub fn format(&self) -> &AudioFormat {
        &self.metadata.format
    }

    /// Returns a human-readable identifier for the source (usually the file path)
    pub fn identifier(&self) -> &str {
        match &self.source_type {
            SourceType::File(path) | SourceType::Stream(path) => path.as_str(),
            SourceType::Memory(_) => "<memory>",
        }
    }

    /// Get total playback duration for this source
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.metadata.duration_ms)
    }

    /// Access decoded sample data when available
    pub fn sample(&self) -> Option<Arc<AudioSample>> {
        self.sample.as_ref().map(Arc::clone)
    }
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            format: AudioFormat::default(),
            preload: true,
            cache_priority: Priority::Normal,
        }
    }
}

fn compute_duration(bytes: u32, bytes_per_second: u32) -> u64 {
    if bytes_per_second == 0 {
        return 0;
    }
    (u64::from(bytes) * 1_000) / u64::from(bytes_per_second)
}

fn compute_bitrate(bytes_per_second: u32) -> Option<u32> {
    if bytes_per_second == 0 {
        None
    } else {
        Some(bytes_per_second.saturating_mul(8))
    }
}

pub(crate) fn convert_enhanced_to_basic(format: &EnhancedAudioFormat) -> AudioFormat {
    AudioFormat {
        channels: format.channels,
        sample_rate: sample_rate_from_u32(format.rate),
        sample_width: sample_width_from_u16(format.sample_width),
        channel_layout: channel_layout_from_channels(format.channels),
    }
}

pub(crate) fn enhanced_from_basic(format: &AudioFormat) -> EnhancedAudioFormat {
    let mut enhanced = EnhancedAudioFormat::new();
    enhanced.channels = format.channels;
    enhanced.rate = u32::from(format.sample_rate);
    enhanced.sample_width = sample_width_to_u16(format.sample_width);
    enhanced.bytes_per_second = format.bytes_per_second();
    enhanced.compression = AudioCompressionType::None;
    enhanced.flags = AudioFormatFlags::PCM.0;
    enhanced.cdata = CompressionData::None;
    let _ = enhanced.update();
    enhanced
}

fn sample_rate_from_u32(rate: u32) -> SampleRate {
    match rate {
        8_000 => SampleRate::Hz8000,
        11_025 => SampleRate::Hz11025,
        16_000 => SampleRate::Hz16000,
        22_050 => SampleRate::Hz22050,
        44_100 => SampleRate::Hz44100,
        48_000 => SampleRate::Hz48000,
        96_000 => SampleRate::Hz96000,
        192_000 => SampleRate::Hz192000,
        _ => SampleRate::Hz44100,
    }
}

fn sample_width_from_u16(width: u16) -> SampleWidth {
    match width {
        8 => SampleWidth::U8,
        16 => SampleWidth::S16,
        24 => SampleWidth::S24,
        32 => SampleWidth::S32,
        _ => SampleWidth::S16,
    }
}

fn sample_width_to_u16(width: SampleWidth) -> u16 {
    match width {
        SampleWidth::U8 => 8,
        SampleWidth::S16 => 16,
        SampleWidth::S24 => 24,
        SampleWidth::S32 => 32,
        SampleWidth::F32 => 32,
    }
}

fn channel_layout_from_channels(channels: u16) -> ChannelLayout {
    match channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        3 => ChannelLayout::Surround21,
        4 => ChannelLayout::Surround41,
        6 => ChannelLayout::Surround51,
        8 => ChannelLayout::Surround71,
        _ => ChannelLayout::Stereo,
    }
}
