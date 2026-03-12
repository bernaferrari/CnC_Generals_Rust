//! Audio compression and decompression support.

use crate::error::{Result, SourceError};
use serde::{Deserialize, Serialize};

/// Supported compression types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    ADPCM,
    IMAADPCM,
    MP3,
    OggVorbis,
}

/// Compression level settings
#[derive(Debug, Clone, Copy)]
pub enum CompressionLevel {
    Low,
    Medium,
    High,
    Maximum,
}

/// Codec information
#[derive(Debug, Clone)]
pub struct CodecInfo {
    pub name: String,
    pub compression_type: CompressionType,
    pub supported_sample_rates: Vec<u32>,
    pub supported_channels: Vec<u16>,
}

/// Audio compression handler
pub struct CompressionHandler {
    available_codecs: Vec<CodecInfo>,
}

impl CompressionHandler {
    /// Create new compression handler
    pub fn new() -> Self {
        Self {
            available_codecs: Self::get_available_codecs(),
        }
    }

    /// Compress audio data
    pub fn compress(
        &self,
        data: &[u8],
        compression_type: CompressionType,
        _level: CompressionLevel,
    ) -> Result<Vec<u8>> {
        match compression_type {
            CompressionType::None => Ok(data.to_vec()),
            _ => Err(SourceError::CompressionError(format!(
                "{compression_type:?} compression is not available in this build"
            ))
            .into()),
        }
    }

    /// Decompress audio data
    pub fn decompress(&self, data: &[u8], compression_type: CompressionType) -> Result<Vec<u8>> {
        match compression_type {
            CompressionType::None => Ok(data.to_vec()),
            _ => Err(SourceError::CompressionError(format!(
                "{compression_type:?} decompression is not available in this build"
            ))
            .into()),
        }
    }

    /// Get available codecs
    pub fn available_codecs(&self) -> &[CodecInfo] {
        &self.available_codecs
    }

    /// Check if compression type is supported
    pub fn is_supported(&self, compression_type: CompressionType) -> bool {
        self.available_codecs
            .iter()
            .any(|codec| codec.compression_type == compression_type)
    }

    /// Get codec information
    pub fn get_codec_info(&self, compression_type: CompressionType) -> Option<&CodecInfo> {
        self.available_codecs
            .iter()
            .find(|codec| codec.compression_type == compression_type)
    }

    fn get_available_codecs() -> Vec<CodecInfo> {
        vec![
            CodecInfo {
                name: "Uncompressed PCM".to_string(),
                compression_type: CompressionType::None,
                supported_sample_rates: vec![8000, 11025, 16000, 22050, 44100, 48000],
                supported_channels: vec![1, 2, 4, 6, 8],
            },
            CodecInfo {
                name: "ADPCM".to_string(),
                compression_type: CompressionType::ADPCM,
                supported_sample_rates: vec![11025, 22050, 44100],
                supported_channels: vec![1, 2],
            },
            CodecInfo {
                name: "IMA ADPCM".to_string(),
                compression_type: CompressionType::IMAADPCM,
                supported_sample_rates: vec![11025, 22050, 44100],
                supported_channels: vec![1, 2],
            },
        ]
    }
}

impl Default for CompressionHandler {
    fn default() -> Self {
        Self::new()
    }
}
