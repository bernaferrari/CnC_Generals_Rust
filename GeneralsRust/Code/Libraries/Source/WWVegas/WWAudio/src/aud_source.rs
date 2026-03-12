//! # Audio Source Management
//!
//! This module provides a Rust conversion of the original WPAudio source management
//! functionality from Command & Conquer Generals Zero Hour. It handles audio sample
//! creation, format detection, and data management with safe memory handling.
//!
//! The module supports various audio formats including:
//! - Uncompressed PCM data
//! - ADPCM compressed audio
//! - IMA-ADPCM compressed audio  
//! - MP3 audio files
//!
//! Key features:
//! - Safe memory management for audio buffers
//! - Comprehensive format detection and validation
//! - Sample rate and format conversion utilities
//! - Frame-based audio data management
//! - MP3 header parsing and validation

use crate::{
    error::{Result, SourceError},
    formats::AudioFormat,
    source::{convert_enhanced_to_basic, enhanced_from_basic},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::LinkedList,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::Duration,
};
use symphonia::core::{
    audio::{SampleBuffer, SignalSpec},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

/// High-precision timestamp for audio operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TimeStamp {
    nanos: u64,
}

impl TimeStamp {
    /// Create a new timestamp from nanoseconds
    pub fn from_nanos(nanos: u64) -> Self {
        Self { nanos }
    }

    /// Create a new timestamp from milliseconds
    pub fn from_millis(millis: u64) -> Self {
        Self {
            nanos: millis * 1_000_000,
        }
    }

    /// Create a new timestamp from seconds
    pub fn from_seconds(seconds: u64) -> Self {
        Self {
            nanos: seconds * 1_000_000_000,
        }
    }

    /// Create a zero timestamp
    pub fn zero() -> Self {
        Self { nanos: 0 }
    }

    /// Get nanoseconds
    pub fn as_nanos(&self) -> u64 {
        self.nanos
    }

    /// Get milliseconds
    pub fn as_millis(&self) -> u64 {
        self.nanos / 1_000_000
    }

    /// Get seconds
    pub fn as_seconds(&self) -> u64 {
        self.nanos / 1_000_000_000
    }
}

impl From<Duration> for TimeStamp {
    fn from(duration: Duration) -> Self {
        Self::from_nanos(duration.as_nanos() as u64)
    }
}

impl From<TimeStamp> for Duration {
    fn from(timestamp: TimeStamp) -> Self {
        Duration::from_nanos(timestamp.nanos)
    }
}

/// Simple list node for linking frames
#[derive(Debug)]
pub struct ListNode {
    // In a full implementation, this would contain prev/next pointers
    // For now, we use Rust's standard LinkedList
}

impl ListNode {
    /// Create a new list node
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ListNode {
    fn default() -> Self {
        Self::new()
    }
}

/// Audio compression types matching original WPAudio constants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum AudioCompressionType {
    /// No compression - raw PCM data
    None = 0,
    /// Microsoft ADPCM compression
    MsAdpcm = 1,
    /// IMA ADPCM compression
    ImaAdpcm = 2,
    /// MP3 compression
    Mp3 = 3,
    /// Maximum compression type ID for validation
    MaxId = 4,
}

/// Audio format flags matching original WPAudio
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormatFlags(pub u32);

impl AudioFormatFlags {
    /// PCM format flag
    pub const PCM: Self = Self(0x01);
    /// Signed format flag
    pub const SIGNED: Self = Self(0x02);
    /// Big endian format flag
    pub const BIG_ENDIAN: Self = Self(0x04);

    /// Empty flag set
    pub const fn empty() -> Self {
        Self(0)
    }
}

/// Enhanced audio format structure matching C++ AudioFormat
#[derive(Debug, Serialize, Deserialize)]
pub struct EnhancedAudioFormat {
    /// Number of audio channels
    pub channels: u16,
    /// Sample rate in Hz
    pub rate: u32,
    /// Bits per sample (sample width)
    pub sample_width: u16,
    /// Bytes per second for the audio stream
    pub bytes_per_second: u32,
    /// Compression type used
    pub compression: AudioCompressionType,
    /// Format flags
    pub flags: u32,
    /// Compression-specific data
    pub cdata: CompressionData,
}

/// Compression-specific data union equivalent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionData {
    /// ADPCM-specific data
    Adpcm {
        /// Block size for ADPCM compression
        block_size: u32,
        /// Number of samples per block
        samples_per_block: u16,
        /// Coefficient table for MS-ADPCM
        coefficients: Vec<[i16; 2]>,
    },
    /// MP3-specific data
    Mp3 {
        /// MP3 frame header
        header: u32,
        /// Calculated frame size
        frame_size: u32,
        /// Padding bit
        padding: bool,
    },
    /// No compression data
    None,
}

/// Audio frame representing a chunk of audio data
#[derive(Debug)]
pub struct AudioFrame {
    /// Size of the frame data in bytes
    pub bytes: u32,
    /// Pointer to the frame data
    pub data: Vec<u8>,
    /// Reference to parent sample
    pub sample: Option<*mut AudioSample>,
    /// List node for linking frames
    pub node: ListNode,
}

/// Audio sample containing audio data and metadata
#[derive(Debug)]
pub struct AudioSample {
    /// Audio data buffer
    pub data: Option<Vec<u8>>,
    /// Total size of audio data in bytes
    pub bytes: u32,
    /// Audio format information
    pub format: Option<Box<EnhancedAudioFormat>>,
    /// Additional attributes
    pub attribs: Option<Vec<u8>>,
    /// List of audio frames
    pub frames: LinkedList<AudioFrame>,
    /// Debug name for the sample
    #[cfg(debug_assertions)]
    pub name: String,
}

unsafe impl Send for AudioFrame {}
unsafe impl Sync for AudioFrame {}

unsafe impl Send for AudioSample {}
unsafe impl Sync for AudioSample {}

/// MS-ADPCM standard coefficients table
pub const MSADPCM_STD_COEF: [[i16; 2]; 7] = [
    [256, 0],
    [512, -256],
    [0, 0],
    [192, 64],
    [240, 0],
    [460, -208],
    [392, -232],
];

/// Sample rate lookup table for MP3: [MPEG25][MPEG version][value]
const MP3_SAMPLE_RATES: [[[u32; 4]; 2]; 2] = [
    [[22050, 24000, 16000, 22050], [44100, 48000, 32000, 44100]],
    [[11025, 12000, 8000, 11025], [44100, 48000, 32000, 44100]],
];

/// Bit rate lookup table for MP3: [MPEG version][value]
const MP3_BIT_RATES: [[u32; 15]; 2] = [
    [
        0, 8000, 16000, 24000, 32000, 40000, 48000, 56000, 64000, 80000, 96000, 112000, 128000,
        144000, 160000,
    ],
    [
        0, 32000, 40000, 48000, 56000, 64000, 80000, 96000, 112000, 128000, 160000, 192000, 224000,
        256000, 320000,
    ],
];

/// Maximum bytes to search for MP3 sync
const MAX_SYNC_SEARCH: usize = 10 * 1024;

impl AudioSample {
    /// Creates a new audio sample with the specified buffer size
    ///
    /// # Arguments
    /// * `bytes` - Size of the audio buffer to allocate in bytes
    ///
    /// # Returns
    /// * `Result<AudioSample>` - New audio sample or error
    ///
    /// # Errors
    /// Returns `SourceError` if memory allocation fails
    pub fn create(bytes: u32) -> Result<Self> {
        if bytes == 0 {
            return Err(
                SourceError::InvalidFormat("Buffer size cannot be zero".to_string()).into(),
            );
        }

        let mut sample = Self::new();
        sample.bytes = bytes;

        // Allocate the audio data buffer
        sample.data = Some(vec![0u8; bytes as usize]);

        Ok(sample)
    }

    /// Initializes a new empty audio sample
    pub fn new() -> Self {
        Self {
            data: None,
            bytes: 0,
            format: None,
            attribs: None,
            frames: LinkedList::new(),
            #[cfg(debug_assertions)]
            name: String::new(),
        }
    }

    /// Set audio format metadata from a basic format description
    pub fn set_format(&mut self, format: AudioFormat) {
        self.format = Some(Box::new(EnhancedAudioFormat::from_basic(&format)));
    }

    /// Resize the underlying sample buffer
    pub fn set_size(&mut self, bytes: usize) {
        self.bytes = bytes as u32;
        self.data = Some(vec![0u8; bytes]);
    }

    /// Write raw PCM data into the sample buffer
    pub fn write_data(&mut self, data: &[u8]) {
        self.bytes = data.len() as u32;
        match self.data.as_mut() {
            Some(buf) => {
                buf.clear();
                buf.extend_from_slice(data);
            }
            None => {
                self.data = Some(data.to_vec());
            }
        }
    }

    /// Sets the debug name for the audio sample (debug builds only)
    ///
    /// # Arguments
    /// * `orig_name` - Original filename or identifier
    #[cfg(debug_assertions)]
    pub fn set_name(&mut self, orig_name: &str) {
        // Extract base filename without extension
        let name = Path::new(orig_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(orig_name);

        // Limit name length to reasonable size
        const MAX_NAME_LEN: usize = 32;
        if name.len() > MAX_NAME_LEN {
            self.name = format!("...{}", &name[name.len() - MAX_NAME_LEN + 3..]);
        } else {
            self.name = name.to_string();
        }
    }

    /// Adds an audio frame to this sample
    ///
    /// # Arguments
    /// * `frame` - Audio frame to add
    pub fn add_frame(&mut self, frame: AudioFrame) {
        self.bytes += frame.bytes;
        self.frames.push_back(frame);
    }

    /// Gets the first frame in the sample
    ///
    /// # Returns
    /// * `Option<&AudioFrame>` - Reference to first frame or None
    pub fn first_frame(&self) -> Option<&AudioFrame> {
        self.frames.front()
    }

    /// Gets mutable reference to the first frame
    pub fn first_frame_mut(&mut self) -> Option<&mut AudioFrame> {
        self.frames.front_mut()
    }
}

impl Default for AudioSample {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioSample {
    fn drop(&mut self) {
        // Rust automatically handles memory cleanup
        // This is just for debugging in debug builds
        #[cfg(debug_assertions)]
        {
            if !self.name.is_empty() {
                eprintln!("Dropping AudioSample: {}", self.name);
            }
        }
    }
}

impl AudioFrame {
    /// Initializes a new audio frame
    ///
    /// # Arguments
    /// * `data` - Audio data for this frame
    /// * `bytes` - Size of the data in bytes
    pub fn new(data: Vec<u8>, bytes: u32) -> Self {
        Self {
            bytes,
            data,
            sample: None,
            node: ListNode::new(),
        }
    }
}

impl EnhancedAudioFormat {
    /// Initializes a new empty audio format
    pub fn new() -> Self {
        Self {
            channels: 0,
            rate: 0,
            sample_width: 0,
            bytes_per_second: 0,
            compression: AudioCompressionType::None,
            flags: 0,
            cdata: CompressionData::None,
        }
    }

    /// Construct from a basic audio format description
    pub fn from_basic(format: &AudioFormat) -> Self {
        enhanced_from_basic(format)
    }

    /// Convert into the simplified audio format description
    pub fn to_basic(&self) -> AudioFormat {
        convert_enhanced_to_basic(self)
    }

    /// Updates calculated fields based on format parameters
    ///
    /// # Errors
    /// Returns error if format parameters are invalid
    pub fn update(&mut self) -> Result<()> {
        if self.channels == 0 {
            return Err(SourceError::InvalidFormat("Channels cannot be zero".to_string()).into());
        }
        if self.sample_width == 0 {
            return Err(
                SourceError::InvalidFormat("Sample width cannot be zero".to_string()).into(),
            );
        }
        if self.rate == 0 {
            return Err(
                SourceError::InvalidFormat("Sample rate cannot be zero".to_string()).into(),
            );
        }

        match self.compression {
            AudioCompressionType::None => {
                self.bytes_per_second =
                    u32::from(self.channels) * u32::from(self.sample_width) * self.rate / 8;
            }
            AudioCompressionType::ImaAdpcm | AudioCompressionType::MsAdpcm => {
                self.bytes_per_second =
                    u32::from(self.channels) * u32::from(self.sample_width) * self.rate / 32;
                // 4:1 compression
            }
            AudioCompressionType::Mp3 => {
                if let CompressionData::Mp3 { header, .. } = &self.cdata {
                    let mpeg1 = (header >> 19) & 1;
                    let bitrate_index = ((header >> 12) & 0xF) as usize;

                    if bitrate_index < MP3_BIT_RATES[mpeg1 as usize].len() {
                        self.bytes_per_second = MP3_BIT_RATES[mpeg1 as usize][bitrate_index] / 8;
                    }
                }
            }
            AudioCompressionType::MaxId => {
                return Err(
                    SourceError::InvalidFormat("Invalid compression type".to_string()).into(),
                );
            }
        }

        Ok(())
    }

    /// Calculates the number of bytes for a given time duration
    ///
    /// # Arguments
    /// * `time` - Time duration
    ///
    /// # Returns
    /// Number of bytes required for the time duration
    pub fn bytes_for_time(&self, time: TimeStamp) -> u32 {
        if self.bytes_per_second == 0 {
            return 0;
        }
        ((u64::from(self.bytes_per_second) * time.as_nanos()) / 1_000_000_000) as u32
    }

    /// Convenience wrapper returning timestamp for a byte count
    pub fn bytes_to_time(&self, bytes: usize) -> TimeStamp {
        self.time_for_bytes(bytes.min(u32::MAX as usize) as u32)
    }

    /// Convert a timestamp to byte offset
    pub fn time_to_bytes(&self, time: TimeStamp) -> usize {
        self.bytes_for_time(time) as usize
    }

    /// Convert a standard duration to the equivalent byte count
    pub fn time_to_bytes_duration(&self, duration: std::time::Duration) -> usize {
        self.bytes_for_time(TimeStamp::from(duration)) as usize
    }

    /// Calculates the time duration for a given number of bytes
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes
    ///
    /// # Returns
    /// Time duration for the bytes
    pub fn time_for_bytes(&self, bytes: u32) -> TimeStamp {
        if self.bytes_per_second == 0 {
            return TimeStamp::zero();
        }
        TimeStamp::from_nanos((u64::from(bytes) * 1_000_000_000) / u64::from(self.bytes_per_second))
    }

    /// Checks if two audio formats are compatible
    ///
    /// # Arguments
    /// * `other` - Other format to compare with
    ///
    /// # Returns
    /// `true` if formats are the same, `false` otherwise
    pub fn is_same(&self, other: &Self) -> bool {
        self.rate == other.rate
            && self.compression == other.compression
            && self.sample_width == other.sample_width
            && self.channels == other.channels
            && self.flags == other.flags
            && self.compression_data_matches(other)
    }

    /// Checks if compression-specific data matches
    fn compression_data_matches(&self, other: &Self) -> bool {
        match (&self.cdata, &other.cdata) {
            (
                CompressionData::Adpcm {
                    block_size: bs1, ..
                },
                CompressionData::Adpcm {
                    block_size: bs2, ..
                },
            ) => bs1 == bs2,
            (CompressionData::Mp3 { header: h1, .. }, CompressionData::Mp3 { header: h2, .. }) => {
                h1 == h2
            }
            (CompressionData::None, CompressionData::None) => true,
            _ => false,
        }
    }

    /// Reads MP3 format information from a file
    ///
    /// # Arguments
    /// * `file` - File to read from
    /// * `datasize` - Optional output parameter for data size
    ///
    /// # Returns
    /// `Result<()>` - Success or error
    pub fn read_mp3_file<R: Read + Seek>(
        &mut self,
        file: &mut R,
        datasize: Option<&mut u32>,
    ) -> Result<()> {
        let mut buffer = vec![0u8; MAX_SYNC_SEARCH + 4];
        let data_start = file.seek(SeekFrom::Current(0))? as u32;

        let bytes_read = file.read(&mut buffer)?;
        let mut pos = 0;
        let mut header = 0u32;

        // Search for MP3 sync header
        while header == 0 && bytes_read >= 4 {
            let mut mask = 0xFFu8;

            // Find next sync
            while (pos + 3) < bytes_read {
                if (buffer[pos] & mask) == mask {
                    if mask == 0xE0 {
                        if pos > 0 {
                            pos -= 1;
                        }
                        // Construct 32-bit header
                        header = u32::from_be_bytes([
                            buffer[pos],
                            buffer[pos + 1],
                            buffer[pos + 2],
                            buffer[pos + 3],
                        ]);
                        break;
                    }
                    mask = 0xE0;
                } else {
                    mask = 0xFF;
                }
                pos += 1;
            }

            if header == 0 {
                break;
            }

            // Validate the header
            let bitrate_index = ((header >> 12) & 0xF) as usize;
            let sampling_frequency = ((header >> 10) & 0x3) as usize;
            let layer = (header >> 17) & 0x3;

            if bitrate_index == 0x0F || sampling_frequency == 0x03 || layer != 1 {
                header = 0;
                pos += 1;
                continue;
            }
        }

        if header == 0 {
            return Err(SourceError::InvalidFormat("No valid MP3 header found".to_string()).into());
        }

        // Parse MP3 header
        let mpeg25 = ((header >> 20) & 1) == 0;
        let mpeg1 = (header >> 19) & 1;
        let mode = (header >> 6) & 0x3;
        let bitrate_index = ((header >> 12) & 0xF) as usize;
        let sampling_frequency = ((header >> 10) & 0x3) as usize;
        let padding = ((header >> 9) & 1) != 0;

        // Set format parameters
        self.compression = AudioCompressionType::Mp3;
        self.sample_width = 16; // MP3 is typically decoded to 16-bit
        self.channels = if mode == 3 { 1 } else { 2 };

        if bitrate_index < MP3_BIT_RATES[mpeg1 as usize].len() {
            self.bytes_per_second = MP3_BIT_RATES[mpeg1 as usize][bitrate_index] / 8;
        }

        let mpeg25_idx = if mpeg25 { 1 } else { 0 };
        if sampling_frequency < MP3_SAMPLE_RATES[mpeg25_idx][mpeg1 as usize].len() {
            self.rate = MP3_SAMPLE_RATES[mpeg25_idx][mpeg1 as usize][sampling_frequency];
        }

        self.flags = AudioFormatFlags::PCM.0;

        // Calculate frame size for MP3
        let frame_size = if mpeg1 == 1 {
            144 * MP3_BIT_RATES[mpeg1 as usize][bitrate_index] / self.rate
                + if padding { 1 } else { 0 }
        } else {
            72 * MP3_BIT_RATES[mpeg1 as usize][bitrate_index] / self.rate
                + if padding { 1 } else { 0 }
        };

        self.cdata = CompressionData::Mp3 {
            header,
            frame_size,
            padding,
        };

        self.update()?;

        // Set data size if requested
        if let Some(size) = datasize {
            // Calculate remaining file size
            let current_pos = data_start + pos as u32;
            file.seek(SeekFrom::End(0))?;
            let file_size = file.seek(SeekFrom::Current(0))? as u32;
            *size = file_size - current_pos;
        }

        // Seek to start of audio data
        file.seek(SeekFrom::Start((data_start + pos as u32) as u64))?;

        Ok(())
    }

    /// Seeks to a specific position in the audio data
    ///
    /// # Arguments
    /// * `file` - File to seek in
    /// * `pos` - Position to seek to
    /// * `data_start` - Start of audio data in file
    ///
    /// # Returns
    /// Actual position seeked to
    pub fn seek_to_pos<R: Read + Seek>(
        &self,
        file: &mut R,
        pos: u32,
        data_start: u32,
    ) -> Result<u32> {
        let mut actual_pos = pos;

        if actual_pos > 0 {
            match self.compression {
                AudioCompressionType::Mp3 => {
                    file.seek(SeekFrom::Start((actual_pos + data_start) as u64))?;

                    // Find next valid MP3 frame
                    let mut temp_format = self.clone();
                    let mut found = false;

                    while !found {
                        match temp_format.read_mp3_file(file, None) {
                            Ok(()) => {
                                if temp_format.is_same(self) {
                                    found = true;
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                        file.seek(SeekFrom::Current(1))?;
                    }

                    if !found {
                        actual_pos = 0;
                    } else {
                        let current = file.seek(SeekFrom::Current(0))? as u32;
                        actual_pos = current - data_start;
                    }
                }
                AudioCompressionType::None => {
                    let block_size = u32::from(self.channels) * u32::from(self.sample_width) / 8;
                    if block_size > 1 {
                        actual_pos = (actual_pos / block_size) * block_size;
                    }
                }
                AudioCompressionType::ImaAdpcm | AudioCompressionType::MsAdpcm => {
                    if let CompressionData::Adpcm { block_size, .. } = &self.cdata {
                        if *block_size > 1 {
                            actual_pos = (actual_pos / block_size) * block_size;
                        }
                    }
                }
                AudioCompressionType::MaxId => {
                    return Err(
                        SourceError::InvalidFormat("Invalid compression type".to_string()).into(),
                    );
                }
            }
        }

        file.seek(SeekFrom::Start((actual_pos + data_start) as u64))?;
        Ok(actual_pos)
    }
}

impl Default for EnhancedAudioFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for EnhancedAudioFormat {
    fn clone(&self) -> Self {
        Self {
            channels: self.channels,
            rate: self.rate,
            sample_width: self.sample_width,
            bytes_per_second: self.bytes_per_second,
            compression: self.compression,
            flags: self.flags,
            cdata: self.cdata.clone(),
        }
    }
}

impl Default for CompressionData {
    fn default() -> Self {
        Self::None
    }
}

/// Utility functions for bit manipulation (matching original C++ W_BitsGet)
pub struct BitUtils;

impl BitUtils {
    /// Extract bits from a 32-bit value
    ///
    /// # Arguments  
    /// * `value` - Source value
    /// * `num_bits` - Number of bits to extract
    /// * `start_bit` - Starting bit position (0-based from LSB)
    ///
    /// # Returns
    /// Extracted bits as u32
    pub fn extract_bits(value: u32, num_bits: u32, start_bit: u32) -> u32 {
        let mask = (1u32 << num_bits) - 1;
        (value >> start_bit) & mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_audio_sample_creation() {
        let sample = AudioSample::create(1024).unwrap();
        assert_eq!(sample.bytes, 1024);
        assert!(sample.data.is_some());
        assert_eq!(sample.data.as_ref().unwrap().len(), 1024);
    }

    #[test]
    fn test_audio_sample_zero_bytes() {
        let result = AudioSample::create(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_audio_format_update() {
        let mut format = EnhancedAudioFormat::new();
        format.channels = 2;
        format.sample_width = 16;
        format.rate = 44100;
        format.compression = AudioCompressionType::None;

        format.update().unwrap();
        assert_eq!(format.bytes_per_second, 2 * 16 * 44100 / 8);
    }

    #[test]
    fn test_format_comparison() {
        let mut format1 = EnhancedAudioFormat::new();
        format1.channels = 2;
        format1.rate = 44100;

        let mut format2 = EnhancedAudioFormat::new();
        format2.channels = 2;
        format2.rate = 44100;

        assert!(format1.is_same(&format2));

        format2.channels = 1;
        assert!(!format1.is_same(&format2));
    }

    #[test]
    fn test_bit_extraction() {
        let value = 0b11010110_10101010_01010101_11110000u32;
        let extracted = BitUtils::extract_bits(value, 4, 4);
        assert_eq!(extracted, 0b1111);
    }

    #[test]
    fn test_msadpcm_coefficients() {
        assert_eq!(MSADPCM_STD_COEF.len(), 7);
        assert_eq!(MSADPCM_STD_COEF[0], [256, 0]);
        assert_eq!(MSADPCM_STD_COEF[1], [512, -256]);
    }

    #[test]
    fn test_mp3_constants() {
        assert_eq!(MP3_SAMPLE_RATES[0][1][0], 44100);
        assert_eq!(MP3_BIT_RATES[1][14], 320000);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_sample_naming() {
        let mut sample = AudioSample::new();
        sample.set_name("sounds/explosion.wav");
        assert_eq!(sample.name, "explosion");

        // Test long name truncation
        let long_name = "very_long_filename_that_exceeds_maximum_length.wav";
        sample.set_name(long_name);
        assert!(sample.name.starts_with("..."));
        assert!(sample.name.len() <= 32);
    }
}

/// Audio source loader using Symphonia for comprehensive format support
pub struct AudioSourceLoader;

impl AudioSourceLoader {
    /// Load an audio file using Symphonia's format detection
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    /// * `Result<(AudioSample, EnhancedAudioFormat)>` - Loaded sample and format info
    pub fn load_file<P: AsRef<Path>>(path: P) -> Result<(AudioSample, EnhancedAudioFormat)> {
        let file = File::open(&path)?;
        let file_size = file.metadata()?.len() as usize;
        let source = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a hint based on file extension
        let mut hint = Hint::new();
        if let Some(extension) = path.as_ref().extension() {
            if let Some(ext_str) = extension.to_str() {
                hint.with_extension(ext_str);
            }
        }

        // Probe the media source
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                source,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| {
                SourceError::InvalidFormat(format!("Failed to probe audio file: {}", e))
            })?;

        let mut reader = probed.format;

        // Get the first audio track
        let track = reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| SourceError::InvalidFormat("No audio tracks found".to_string()))?;

        let track_id = track.id;

        // Create decoder for the track
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &Default::default())
            .map_err(|e| {
                SourceError::CompressionError(format!("Failed to create decoder: {}", e))
            })?;

        // Convert Symphonia format info to our format
        let mut format = Self::symphonia_to_enhanced_format(&track.codec_params)?;
        format.compression = AudioCompressionType::None;
        format.sample_width = 16;
        format.flags = AudioFormatFlags::PCM.0 | AudioFormatFlags::SIGNED.0;

        let mut sample = AudioSample::new();

        #[cfg(debug_assertions)]
        {
            if let Some(path_str) = path.as_ref().to_str() {
                sample.set_name(path_str);
            }
        }

        let mut audio_data = Vec::with_capacity(file_size.max(1024));
        let mut signal_spec: Option<SignalSpec> = None;

        loop {
            let packet = match reader.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break
                }
                Err(e) => {
                    return Err(SourceError::CompressionError(format!("Read error: {}", e)).into())
                }
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    signal_spec.get_or_insert(spec);

                    let mut sample_buffer =
                        SampleBuffer::<i16>::new(decoded.capacity() as u64, spec);
                    sample_buffer.copy_interleaved_ref(decoded);

                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            sample_buffer.samples().as_ptr() as *const u8,
                            sample_buffer.samples().len() * std::mem::size_of::<i16>(),
                        )
                    };
                    audio_data.extend_from_slice(slice);
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(e) => {
                    return Err(
                        SourceError::CompressionError(format!("Decode error: {}", e)).into(),
                    )
                }
            }
        }

        if audio_data.is_empty() {
            return Err(SourceError::InvalidFormat("No audio data decoded".to_string()).into());
        }

        if let Some(spec) = signal_spec {
            if format.channels == 0 {
                format.channels = spec.channels.count() as u16;
            }
            if format.rate == 0 {
                format.rate = spec.rate;
            }
        }

        format.update()?;

        sample.write_data(&audio_data);
        sample.format = Some(Box::new(format.clone()));

        Ok((sample, format))
    }

    /// Convert Symphonia codec parameters to our Enhanced format
    fn symphonia_to_enhanced_format(
        codec_params: &symphonia::core::codecs::CodecParameters,
    ) -> Result<EnhancedAudioFormat> {
        let mut format = EnhancedAudioFormat::new();

        // Set channels
        if let Some(channels) = codec_params.channels {
            format.channels = channels.count() as u16;
        }

        // Set sample rate
        if let Some(rate) = codec_params.sample_rate {
            format.rate = rate;
        }

        // Set sample width - default to 16 bit if not specified
        format.sample_width = codec_params.bits_per_sample.unwrap_or(16) as u16;

        // Determine compression type based on codec
        format.compression = match codec_params.codec {
            symphonia::core::codecs::CODEC_TYPE_PCM_S16LE
            | symphonia::core::codecs::CODEC_TYPE_PCM_S16BE
            | symphonia::core::codecs::CODEC_TYPE_PCM_S24LE
            | symphonia::core::codecs::CODEC_TYPE_PCM_S24BE
            | symphonia::core::codecs::CODEC_TYPE_PCM_S32LE
            | symphonia::core::codecs::CODEC_TYPE_PCM_S32BE => AudioCompressionType::None,
            symphonia::core::codecs::CODEC_TYPE_MP3 => AudioCompressionType::Mp3,
            _ => AudioCompressionType::None, // Default to uncompressed
        };

        // Set format flags
        format.flags = AudioFormatFlags::PCM.0;
        if format.sample_width > 8 {
            format.flags |= AudioFormatFlags::SIGNED.0;
        }

        // Set compression data
        format.cdata = match format.compression {
            AudioCompressionType::Mp3 => CompressionData::Mp3 {
                header: 0, // Would need to parse MP3 header for this
                frame_size: 0,
                padding: false,
            },
            AudioCompressionType::MsAdpcm | AudioCompressionType::ImaAdpcm => {
                CompressionData::Adpcm {
                    block_size: codec_params.max_frames_per_packet.unwrap_or(1024) as u32,
                    samples_per_block: 0,
                    coefficients: MSADPCM_STD_COEF.to_vec(),
                }
            }
            _ => CompressionData::None,
        };

        Ok(format)
    }

    /// Load audio data from memory buffer
    ///
    /// # Arguments
    /// * `data` - Raw audio data
    /// * `_format_hint` - Optional format hint (file extension)
    ///
    /// # Returns
    /// * `Result<(AudioSample, EnhancedAudioFormat)>` - Loaded sample and format info
    pub fn load_from_memory(
        data: Vec<u8>,
        _format_hint: Option<&str>,
    ) -> Result<(AudioSample, EnhancedAudioFormat)> {
        // Create a simple sample from the raw data
        // In a full implementation, we would parse the data based on _format_hint
        let mut sample = AudioSample::create(data.len() as u32)?;
        sample.data = Some(data);

        let mut format = EnhancedAudioFormat::new();

        // Set some default values - in a real implementation we would detect these
        format.channels = 2;
        format.rate = 44100;
        format.sample_width = 16;
        format.compression = AudioCompressionType::None;
        format.flags = AudioFormatFlags::PCM.0 | AudioFormatFlags::SIGNED.0;
        format.cdata = CompressionData::None;

        format.update()?;
        sample.format = Some(Box::new(format.clone()));

        Ok((sample, format))
    }

    /// Probe audio format without fully loading the file
    ///
    /// # Arguments  
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    /// * `Result<EnhancedAudioFormat>` - Detected format information
    pub fn probe_format<P: AsRef<Path>>(path: P) -> Result<EnhancedAudioFormat> {
        let file = File::open(&path)?;
        let source = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(extension) = path.as_ref().extension() {
            if let Some(ext_str) = extension.to_str() {
                hint.with_extension(ext_str);
            }
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                source,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| {
                SourceError::InvalidFormat(format!("Failed to probe audio file: {}", e))
            })?;

        let reader = probed.format;

        let track = reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| SourceError::InvalidFormat("No audio tracks found".to_string()))?;

        let mut format = Self::symphonia_to_enhanced_format(&track.codec_params)?;
        format.update()?;

        Ok(format)
    }
}
