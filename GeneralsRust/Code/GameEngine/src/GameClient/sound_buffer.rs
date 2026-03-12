//! Sound Buffer Management
//!
//! Manages raw sound data for audio playback
//! Ports C++ SoundBufferClass from:
//! /GeneralsMD/Code/Libraries/Source/WWVegas/WWAudio/SoundBuffer.h
//! /GeneralsMD/Code/Libraries/Source/WWVegas/WWAudio/SoundBuffer.cpp

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

/// Sound buffer data container
///
/// Matches C++ SoundBufferClass
#[derive(Clone)]
pub struct SoundBufferData {
    /// Raw PCM audio data
    pub data: Arc<Vec<u8>>,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Bits per sample (8, 16, 24, 32)
    pub bits_per_sample: u32,
    /// Number of channels (1=mono, 2=stereo)
    pub channels: u32,
    /// Duration in milliseconds
    pub duration_ms: u32,
    /// Original filename
    pub filename: Option<String>,
}

impl SoundBufferData {
    /// Create new empty sound buffer
    pub fn new() -> Self {
        Self {
            data: Arc::new(Vec::new()),
            sample_rate: 44100,
            bits_per_sample: 16,
            channels: 2,
            duration_ms: 0,
            filename: None,
        }
    }

    /// Get raw buffer reference
    ///
    /// Matches C++ Get_Raw_Buffer()
    pub fn get_raw_buffer(&self) -> &[u8] {
        &self.data
    }

    /// Get raw buffer length
    ///
    /// Matches C++ Get_Raw_Length()
    pub fn get_raw_length(&self) -> usize {
        self.data.len()
    }

    /// Get duration in milliseconds
    ///
    /// Matches C++ Get_Duration()
    pub fn get_duration(&self) -> u32 {
        self.duration_ms
    }

    /// Get sample rate
    ///
    /// Matches C++ Get_Rate()
    pub fn get_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get bits per sample
    ///
    /// Matches C++ Get_Bits()
    pub fn get_bits(&self) -> u32 {
        self.bits_per_sample
    }

    /// Get number of channels
    ///
    /// Matches C++ Get_Channels()
    pub fn get_channels(&self) -> u32 {
        self.channels
    }

    /// Get filename
    ///
    /// Matches C++ Get_Filename()
    pub fn get_filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Calculate duration from buffer size
    fn calculate_duration(&mut self) {
        if self.data.is_empty() {
            self.duration_ms = 0;
            return;
        }

        let bytes_per_sample = (self.bits_per_sample / 8) * self.channels;
        let total_samples = (self.data.len() as u32) / bytes_per_sample;
        self.duration_ms = (total_samples * 1000) / self.sample_rate;
    }
}

/// Sound buffer with file loading capabilities
///
/// Matches C++ SoundBufferClass functionality
pub struct SoundBuffer {
    data: SoundBufferData,
}

impl SoundBuffer {
    /// Create new empty sound buffer
    ///
    /// Matches C++ SoundBufferClass constructor
    pub fn new() -> Self {
        Self {
            data: SoundBufferData::new(),
        }
    }

    /// Load sound from file
    ///
    /// Matches C++ Load_From_File(const char *filename)
    pub fn load_from_file(file_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = file_path.as_ref();
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut buffer = Self::new();
        buffer.data.filename = Some(path.to_string_lossy().to_string());

        match extension.as_str() {
            "wav" => buffer.load_wav_file(path)?,
            "mp3" => return Err("MP3 loading not yet implemented".into()),
            _ => return Err(format!("Unsupported audio format: {}", extension).into()),
        }

        buffer.data.calculate_duration();

        Ok(buffer)
    }

    /// Load from raw memory buffer
    ///
    /// Matches C++ Load_From_Memory(unsigned char *mem_buffer, unsigned long size)
    pub fn load_from_memory(
        data: Vec<u8>,
        sample_rate: u32,
        bits_per_sample: u32,
        channels: u32,
    ) -> Self {
        let mut buffer_data = SoundBufferData {
            data: Arc::new(data),
            sample_rate,
            bits_per_sample,
            channels,
            duration_ms: 0,
            filename: None,
        };

        buffer_data.calculate_duration();

        Self {
            data: buffer_data,
        }
    }

    /// Get buffer data
    pub fn get_data(&self) -> &SoundBufferData {
        &self.data
    }

    /// Get mutable buffer data
    pub fn get_data_mut(&mut self) -> &mut SoundBufferData {
        &mut self.data
    }

    /// Load WAV file format
    fn load_wav_file(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;

        // Read RIFF header
        let mut riff_header = [0u8; 12];
        file.read_exact(&mut riff_header)?;

        // Verify RIFF/WAVE format
        if &riff_header[0..4] != b"RIFF" {
            return Err("Not a valid WAV file (missing RIFF header)".into());
        }
        if &riff_header[8..12] != b"WAVE" {
            return Err("Not a valid WAV file (missing WAVE header)".into());
        }

        // Parse chunks
        loop {
            let mut chunk_header = [0u8; 8];
            if file.read_exact(&mut chunk_header).is_err() {
                break; // End of file
            }

            let chunk_id = &chunk_header[0..4];
            let chunk_size = u32::from_le_bytes([
                chunk_header[4],
                chunk_header[5],
                chunk_header[6],
                chunk_header[7],
            ]);

            match chunk_id {
                b"fmt " => self.parse_wav_fmt_chunk(&mut file, chunk_size)?,
                b"data" => self.parse_wav_data_chunk(&mut file, chunk_size)?,
                _ => {
                    // Skip unknown chunk
                    file.seek(SeekFrom::Current(chunk_size as i64))?;
                }
            }
        }

        Ok(())
    }

    /// Parse WAV format chunk
    fn parse_wav_fmt_chunk(&mut self, file: &mut File, size: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut fmt_data = vec![0u8; size as usize];
        file.read_exact(&mut fmt_data)?;

        // Parse format data
        let format_tag = u16::from_le_bytes([fmt_data[0], fmt_data[1]]);
        if format_tag != 1 {
            return Err(format!("Unsupported WAV format: {}", format_tag).into());
        }

        self.data.channels = u16::from_le_bytes([fmt_data[2], fmt_data[3]]) as u32;
        self.data.sample_rate = u32::from_le_bytes([
            fmt_data[4],
            fmt_data[5],
            fmt_data[6],
            fmt_data[7],
        ]);
        self.data.bits_per_sample = u16::from_le_bytes([fmt_data[14], fmt_data[15]]) as u32;

        Ok(())
    }

    /// Parse WAV data chunk
    fn parse_wav_data_chunk(&mut self, file: &mut File, size: u32) -> Result<(), Box<dyn std::error::Error>> {
        let mut data = vec![0u8; size as usize];
        file.read_exact(&mut data)?;

        self.data.data = Arc::new(data);

        Ok(())
    }
}

/// Streaming sound buffer for large audio files
///
/// Matches C++ StreamSoundBufferClass
pub struct StreamingSoundBuffer {
    file_path: PathBuf,
    sample_rate: u32,
    bits_per_sample: u32,
    channels: u32,
    total_size: u64,
}

impl StreamingSoundBuffer {
    /// Create new streaming sound buffer
    ///
    /// Matches C++ StreamSoundBufferClass constructor
    pub fn new(file_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = file_path.as_ref();

        // Open file to read header
        let mut file = File::open(path)?;
        let metadata = file.metadata()?;

        // Parse WAV header to get format info
        let mut riff_header = [0u8; 12];
        file.read_exact(&mut riff_header)?;

        if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
            return Err("Not a valid WAV file".into());
        }

        let mut sample_rate = 44100;
        let mut bits_per_sample = 16;
        let mut channels = 2;

        // Parse fmt chunk
        loop {
            let mut chunk_header = [0u8; 8];
            if file.read_exact(&mut chunk_header).is_err() {
                break;
            }

            let chunk_id = &chunk_header[0..4];
            let chunk_size = u32::from_le_bytes([
                chunk_header[4],
                chunk_header[5],
                chunk_header[6],
                chunk_header[7],
            ]);

            if chunk_id == b"fmt " {
                let mut fmt_data = vec![0u8; chunk_size as usize];
                file.read_exact(&mut fmt_data)?;

                channels = u16::from_le_bytes([fmt_data[2], fmt_data[3]]) as u32;
                sample_rate = u32::from_le_bytes([
                    fmt_data[4],
                    fmt_data[5],
                    fmt_data[6],
                    fmt_data[7],
                ]);
                bits_per_sample = u16::from_le_bytes([fmt_data[14], fmt_data[15]]) as u32;
                break;
            } else {
                file.seek(SeekFrom::Current(chunk_size as i64))?;
            }
        }

        Ok(Self {
            file_path: path.to_path_buf(),
            sample_rate,
            bits_per_sample,
            channels,
            total_size: metadata.len(),
        })
    }

    /// Get file path
    pub fn get_file_path(&self) -> &Path {
        &self.file_path
    }

    /// Get sample rate
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get bits per sample
    pub fn get_bits_per_sample(&self) -> u32 {
        self.bits_per_sample
    }

    /// Get number of channels
    pub fn get_channels(&self) -> u32 {
        self.channels
    }

    /// Get total file size
    pub fn get_total_size(&self) -> u64 {
        self.total_size
    }

    /// Check if this is a streaming buffer
    ///
    /// Matches C++ Is_Streaming()
    pub fn is_streaming(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_buffer_creation() {
        let buffer = SoundBuffer::new();
        assert_eq!(buffer.get_data().get_raw_length(), 0);
        assert_eq!(buffer.get_data().get_duration(), 0);
    }

    #[test]
    fn test_sound_buffer_from_memory() {
        let data = vec![0u8; 88200]; // 1 second of 16-bit stereo @ 44.1kHz
        let buffer = SoundBuffer::load_from_memory(data, 44100, 16, 2);

        assert_eq!(buffer.get_data().get_raw_length(), 88200);
        assert_eq!(buffer.get_data().get_sample_rate(), 44100);
        assert_eq!(buffer.get_data().get_bits_per_sample(), 16);
        assert_eq!(buffer.get_data().get_channels(), 2);
        assert_eq!(buffer.get_data().get_duration(), 500); // ~0.5 seconds
    }

    #[test]
    fn test_duration_calculation() {
        let data = vec![0u8; 176400]; // 2 seconds of 16-bit stereo @ 44.1kHz
        let buffer = SoundBuffer::load_from_memory(data, 44100, 16, 2);

        // 176400 bytes / (2 bytes/sample * 2 channels) = 44100 samples
        // 44100 samples / 44100 Hz = 1 second
        assert_eq!(buffer.get_data().get_duration(), 1000);
    }
}
