//! Audio Format Support
//! 
//! This module provides comprehensive audio format support using the Symphonia
//! audio library, enabling playback of MP3, WAV, OGG, FLAC, and other formats
//! commonly used in Command & Conquer games.

use std::io::{Read, Seek, Cursor};
use std::time::Duration;
use std::sync::Arc;

use symphonia::core::audio::{AudioBuffer, AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, Track};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::{MetadataOptions, MetadataRevision};
use symphonia::core::probe::Hint;
use rodio::{Source, Sample};

use crate::common::audio::{Real, Bool, Int, UnsignedInt};

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Mp3,
    Ogg,
    Flac,
    Aac,
    Wma,
    Aiff,
    Au,
    Unknown,
}

impl AudioFormat {
    /// Detect format from file extension
    pub fn from_extension(extension: &str) -> Self {
        match extension.to_lowercase().as_str() {
            "wav" | "wave" => AudioFormat::Wav,
            "mp3" => AudioFormat::Mp3,
            "ogg" => AudioFormat::Ogg,
            "flac" => AudioFormat::Flac,
            "aac" | "m4a" => AudioFormat::Aac,
            "wma" => AudioFormat::Wma,
            "aiff" | "aif" => AudioFormat::Aiff,
            "au" => AudioFormat::Au,
            _ => AudioFormat::Unknown,
        }
    }

    /// Detect format from file header/magic bytes
    pub fn from_magic_bytes(data: &[u8]) -> Self {
        if data.len() < 12 {
            return AudioFormat::Unknown;
        }

        // WAV format
        if data.starts_with(b"RIFF") && &data[8..12] == b"WAVE" {
            return AudioFormat::Wav;
        }

        // MP3 format
        if data.starts_with(&[0xFF, 0xFB]) || data.starts_with(&[0xFF, 0xFA]) || data.starts_with(b"ID3") {
            return AudioFormat::Mp3;
        }

        // OGG format
        if data.starts_with(b"OggS") {
            return AudioFormat::Ogg;
        }

        // FLAC format
        if data.starts_with(b"fLaC") {
            return AudioFormat::Flac;
        }

        // AIFF format
        if data.starts_with(b"FORM") && &data[8..12] == b"AIFF" {
            return AudioFormat::Aiff;
        }

        // AU format
        if data.starts_with(b".snd") {
            return AudioFormat::Au;
        }

        // AAC in M4A container
        if data.len() >= 8 && &data[4..8] == b"ftyp" {
            return AudioFormat::Aac;
        }

        AudioFormat::Unknown
    }

    /// Get the typical file extension for this format
    pub fn typical_extension(&self) -> &'static str {
        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Flac => "flac",
            AudioFormat::Aac => "m4a",
            AudioFormat::Wma => "wma",
            AudioFormat::Aiff => "aiff",
            AudioFormat::Au => "au",
            AudioFormat::Unknown => "unknown",
        }
    }

    /// Check if this format supports streaming
    pub fn supports_streaming(&self) -> bool {
        match self {
            AudioFormat::Mp3 | AudioFormat::Ogg | AudioFormat::Aac => true,
            AudioFormat::Wav | AudioFormat::Flac | AudioFormat::Aiff | AudioFormat::Au => false,
            AudioFormat::Wma => true, // Depends on implementation
            AudioFormat::Unknown => false,
        }
    }

    /// Check if this format supports seeking
    pub fn supports_seeking(&self) -> bool {
        match self {
            AudioFormat::Wav | AudioFormat::Mp3 | AudioFormat::Flac | AudioFormat::Aiff => true,
            AudioFormat::Ogg | AudioFormat::Aac => true,
            AudioFormat::Au | AudioFormat::Wma => false,
            AudioFormat::Unknown => false,
        }
    }
}

/// Audio metadata extracted from files
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub format: AudioFormat,
    pub duration: Option<Duration>,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: Option<u16>,
    pub bitrate: Option<u32>,
    pub is_lossless: bool,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub total_frames: Option<u64>,
}

impl Default for AudioMetadata {
    fn default() -> Self {
        Self {
            format: AudioFormat::Unknown,
            duration: None,
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: None,
            bitrate: None,
            is_lossless: false,
            title: None,
            artist: None,
            album: None,
            track_number: None,
            total_frames: None,
        }
    }
}

/// Audio decoder using Symphonia
pub struct SymphoniaDecoder<R>
where
    R: Read + Seek + Send + Sync,
{
    reader: MediaSourceStream<R>,
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    current_frame: Option<AudioBufferRef>,
    frame_offset: usize,
    sample_rate: u32,
    channels: u16,
    metadata: AudioMetadata,
}

impl<R> SymphoniaDecoder<R>
where
    R: Read + Seek + Send + Sync + 'static,
{
    /// Create a new decoder for the given reader
    pub fn new(mut reader: R) -> Result<Self, String> {
        // Create media source stream
        let media_source = MediaSourceStream::new(
            Box::new(reader),
            MediaSourceStreamOptions::default(),
        );

        // Probe the format
        let mut hint = Hint::new();
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        
        let probed = symphonia::default::get_probe()
            .format(&hint, media_source, &format_opts, &metadata_opts)
            .map_err(|e| format!("Failed to probe audio format: {}", e))?;

        let mut format_reader = probed.format;

        // Get the default track
        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("No suitable audio track found")?;

        let track_id = track.id;

        // Create decoder
        let decode_opts = DecoderOptions::default();
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decode_opts)
            .map_err(|e| format!("Failed to create decoder: {}", e))?;

        // Extract metadata
        let metadata = Self::extract_metadata(&track, probed.metadata.as_ref());
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.map(|ch| ch.count() as u16).unwrap_or(2);

        Ok(Self {
            reader: media_source,
            format_reader,
            decoder,
            track_id,
            current_frame: None,
            frame_offset: 0,
            sample_rate,
            channels,
            metadata,
        })
    }

    /// Get audio metadata
    pub fn metadata(&self) -> &AudioMetadata {
        &self.metadata
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get number of channels
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Get total duration if known
    pub fn total_duration(&self) -> Option<Duration> {
        self.metadata.duration
    }

    /// Seek to a specific time position
    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        if !self.metadata.format.supports_seeking() {
            return Err("Format does not support seeking".to_string());
        }

        let timestamp = (position.as_secs_f64() * self.sample_rate as f64) as u64;
        
        self.format_reader
            .seek(symphonia::core::formats::SeekMode::Accurate, symphonia::core::units::TimeBase::new(1, self.sample_rate), timestamp)
            .map_err(|e| format!("Seek failed: {}", e))?;

        // Reset decoder state
        self.decoder.reset();
        self.current_frame = None;
        self.frame_offset = 0;

        Ok(())
    }

    /// Extract metadata from track and format metadata
    fn extract_metadata(track: &Track, metadata: Option<&MetadataRevision>) -> AudioMetadata {
        let mut meta = AudioMetadata::default();

        // Basic info from codec parameters
        if let Some(sample_rate) = track.codec_params.sample_rate {
            meta.sample_rate = sample_rate;
        }

        if let Some(channels) = track.codec_params.channels {
            meta.channels = channels.count() as u16;
        }

        if let Some(bits_per_sample) = track.codec_params.bits_per_sample {
            meta.bits_per_sample = Some(bits_per_sample);
        }

        // Duration
        if let (Some(frames), Some(sample_rate)) = (track.codec_params.n_frames, track.codec_params.sample_rate) {
            meta.total_frames = Some(frames);
            meta.duration = Some(Duration::from_secs_f64(frames as f64 / sample_rate as f64));
        }

        // Format detection
        meta.format = match track.codec_params.codec.as_str() {
            "pcm" => AudioFormat::Wav,
            "mp3" => AudioFormat::Mp3,
            "vorbis" => AudioFormat::Ogg,
            "flac" => AudioFormat::Flac,
            "aac" => AudioFormat::Aac,
            _ => AudioFormat::Unknown,
        };

        meta.is_lossless = matches!(meta.format, AudioFormat::Wav | AudioFormat::Flac | AudioFormat::Aiff);

        // Extract metadata tags if available
        if let Some(metadata_revision) = metadata {
            for tag in metadata_revision.tags() {
                match tag.key.as_str() {
                    "TITLE" => meta.title = Some(tag.value.to_string()),
                    "ARTIST" => meta.artist = Some(tag.value.to_string()),
                    "ALBUM" => meta.album = Some(tag.value.to_string()),
                    "TRACK" => {
                        if let Ok(track_num) = tag.value.parse::<u32>() {
                            meta.track_number = Some(track_num);
                        }
                    }
                    _ => {}
                }
            }
        }

        meta
    }
}

impl<R> Iterator for SymphoniaDecoder<R>
where
    R: Read + Seek + Send + Sync,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // If we have a current frame, try to get the next sample
            if let Some(ref frame) = self.current_frame {
                match frame {
                    AudioBufferRef::F32(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(sample);
                        }
                    }
                    AudioBufferRef::U8(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample));
                        }
                    }
                    AudioBufferRef::U16(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample));
                        }
                    }
                    AudioBufferRef::U24(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample.inner()));
                        }
                    }
                    AudioBufferRef::U32(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample));
                        }
                    }
                    AudioBufferRef::S8(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample));
                        }
                    }
                    AudioBufferRef::S16(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample));
                        }
                    }
                    AudioBufferRef::S24(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample.inner()));
                        }
                    }
                    AudioBufferRef::S32(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(Sample::to_f32(&sample));
                        }
                    }
                    AudioBufferRef::F64(buffer) => {
                        if self.frame_offset < buffer.frames() * self.channels as usize {
                            let sample = buffer.chan(0)[self.frame_offset / self.channels as usize];
                            self.frame_offset += 1;
                            return Some(sample as f32);
                        }
                    }
                }
            }

            // Current frame exhausted, try to get next frame
            match self.format_reader.next_packet() {
                Ok(packet) => {
                    // Make sure this packet belongs to our track
                    if packet.track_id() == self.track_id {
                        match self.decoder.decode(&packet) {
                            Ok(decoded) => {
                                self.current_frame = Some(decoded);
                                self.frame_offset = 0;
                                continue; // Try again with the new frame
                            }
                            Err(_) => continue, // Skip bad frames
                        }
                    }
                }
                Err(_) => return None, // End of stream or error
            }
        }
    }
}

impl<R> Source for SymphoniaDecoder<R>
where
    R: Read + Seek + Send + Sync,
{
    fn current_frame_len(&self) -> Option<usize> {
        None // Unknown frame length for compressed formats
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.metadata.duration
    }
}

/// Audio format detection and metadata extraction utilities
pub struct AudioFormatDetector;

impl AudioFormatDetector {
    /// Detect audio format from data
    pub fn detect_format(data: &[u8]) -> AudioFormat {
        AudioFormat::from_magic_bytes(data)
    }

    /// Extract metadata from audio data
    pub fn extract_metadata(data: &[u8]) -> Result<AudioMetadata, String> {
        let cursor = Cursor::new(data);
        let decoder = SymphoniaDecoder::new(cursor)?;
        Ok(decoder.metadata().clone())
    }

    /// Check if format is supported
    pub fn is_supported(format: AudioFormat) -> bool {
        !matches!(format, AudioFormat::Unknown)
    }

    /// Get list of supported formats
    pub fn supported_formats() -> Vec<AudioFormat> {
        vec![
            AudioFormat::Wav,
            AudioFormat::Mp3,
            AudioFormat::Ogg,
            AudioFormat::Flac,
            AudioFormat::Aac,
            AudioFormat::Aiff,
            AudioFormat::Au,
        ]
    }

    /// Get list of supported file extensions
    pub fn supported_extensions() -> Vec<&'static str> {
        Self::supported_formats()
            .iter()
            .map(|f| f.typical_extension())
            .collect()
    }
}

/// Audio conversion utilities
pub struct AudioConverter;

impl AudioConverter {
    /// Convert audio data between sample formats
    pub fn convert_samples_f32_to_i16(samples: &[f32]) -> Vec<i16> {
        samples
            .iter()
            .map(|&sample| {
                let clamped = sample.clamp(-1.0, 1.0);
                (clamped * 32767.0) as i16
            })
            .collect()
    }

    /// Convert audio data from i16 to f32
    pub fn convert_samples_i16_to_f32(samples: &[i16]) -> Vec<f32> {
        samples
            .iter()
            .map(|&sample| sample as f32 / 32767.0)
            .collect()
    }

    /// Resample audio data (simple linear interpolation)
    pub fn resample_linear(
        input: &[f32],
        input_rate: u32,
        output_rate: u32,
        channels: u16,
    ) -> Vec<f32> {
        if input_rate == output_rate {
            return input.to_vec();
        }

        let ratio = input_rate as f64 / output_rate as f64;
        let input_frames = input.len() / channels as usize;
        let output_frames = (input_frames as f64 / ratio) as usize;
        let mut output = Vec::with_capacity(output_frames * channels as usize);

        for output_frame in 0..output_frames {
            let input_frame_f = output_frame as f64 * ratio;
            let input_frame = input_frame_f as usize;
            let fraction = input_frame_f - input_frame as f64;

            for ch in 0..channels as usize {
                if input_frame + 1 < input_frames {
                    let sample1 = input[input_frame * channels as usize + ch];
                    let sample2 = input[(input_frame + 1) * channels as usize + ch];
                    let interpolated = sample1 + (sample2 - sample1) * fraction as f32;
                    output.push(interpolated);
                } else if input_frame < input_frames {
                    output.push(input[input_frame * channels as usize + ch]);
                } else {
                    output.push(0.0);
                }
            }
        }

        output
    }

    /// Convert mono to stereo
    pub fn mono_to_stereo(mono_data: &[f32]) -> Vec<f32> {
        let mut stereo_data = Vec::with_capacity(mono_data.len() * 2);
        for &sample in mono_data {
            stereo_data.push(sample); // Left channel
            stereo_data.push(sample); // Right channel
        }
        stereo_data
    }

    /// Convert stereo to mono (mix down)
    pub fn stereo_to_mono(stereo_data: &[f32]) -> Vec<f32> {
        let mut mono_data = Vec::with_capacity(stereo_data.len() / 2);
        for chunk in stereo_data.chunks_exact(2) {
            let mixed = (chunk[0] + chunk[1]) * 0.5;
            mono_data.push(mixed);
        }
        mono_data
    }

    /// Apply volume to audio samples
    pub fn apply_volume(samples: &mut [f32], volume: f32) {
        let clamped_volume = volume.clamp(0.0, 2.0);
        for sample in samples {
            *sample *= clamped_volume;
        }
    }

    /// Normalize audio samples to prevent clipping
    pub fn normalize(samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }

        let max_amplitude = samples
            .iter()
            .map(|&s| s.abs())
            .fold(0.0f32, f32::max);

        if max_amplitude > 0.0 && max_amplitude > 1.0 {
            let normalization_factor = 1.0 / max_amplitude;
            for sample in samples {
                *sample *= normalization_factor;
            }
        }
    }
}

/// Audio streaming decoder for large files
pub struct StreamingDecoder<R>
where
    R: Read + Seek + Send + Sync,
{
    decoder: SymphoniaDecoder<R>,
    buffer: Vec<f32>,
    buffer_size: usize,
    position: usize,
}

impl<R> StreamingDecoder<R>
where
    R: Read + Seek + Send + Sync + 'static,
{
    /// Create new streaming decoder with specified buffer size
    pub fn new(reader: R, buffer_size: usize) -> Result<Self, String> {
        let decoder = SymphoniaDecoder::new(reader)?;
        
        Ok(Self {
            decoder,
            buffer: Vec::with_capacity(buffer_size),
            buffer_size,
            position: 0,
        })
    }

    /// Get next chunk of audio data
    pub fn next_chunk(&mut self) -> Option<&[f32]> {
        self.buffer.clear();

        while self.buffer.len() < self.buffer_size {
            if let Some(sample) = self.decoder.next() {
                self.buffer.push(sample);
            } else {
                break; // End of stream
            }
        }

        if self.buffer.is_empty() {
            None
        } else {
            Some(&self.buffer)
        }
    }

    /// Get decoder metadata
    pub fn metadata(&self) -> &AudioMetadata {
        self.decoder.metadata()
    }

    /// Seek to position
    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        self.decoder.seek(position)?;
        self.buffer.clear();
        Ok(())
    }
}

/// Create a Symphonia decoder from raw audio data
pub fn create_decoder_from_data(data: Vec<u8>) -> Result<SymphoniaDecoder<Cursor<Vec<u8>>>, String> {
    let cursor = Cursor::new(data);
    SymphoniaDecoder::new(cursor)
}

/// Create a streaming decoder from raw audio data
pub fn create_streaming_decoder_from_data(
    data: Vec<u8>,
    buffer_size: usize,
) -> Result<StreamingDecoder<Cursor<Vec<u8>>>, String> {
    let cursor = Cursor::new(data);
    StreamingDecoder::new(cursor, buffer_size)
}

/// Quick audio format validation
pub fn validate_audio_data(data: &[u8]) -> Result<AudioMetadata, String> {
    AudioFormatDetector::extract_metadata(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_detection() {
        // WAV format
        let wav_header = b"RIFF\x24\x08\x00\x00WAVE";
        assert_eq!(AudioFormat::from_magic_bytes(wav_header), AudioFormat::Wav);

        // MP3 format
        let mp3_header = b"ID3\x03\x00\x00\x00";
        assert_eq!(AudioFormat::from_magic_bytes(mp3_header), AudioFormat::Mp3);

        // OGG format
        let ogg_header = b"OggS\x00\x02\x00\x00";
        assert_eq!(AudioFormat::from_magic_bytes(ogg_header), AudioFormat::Ogg);

        // FLAC format
        let flac_header = b"fLaC\x00\x00\x00\x22";
        assert_eq!(AudioFormat::from_magic_bytes(flac_header), AudioFormat::Flac);
    }

    #[test]
    fn test_audio_format_extensions() {
        assert_eq!(AudioFormat::from_extension("wav"), AudioFormat::Wav);
        assert_eq!(AudioFormat::from_extension("MP3"), AudioFormat::Mp3);
        assert_eq!(AudioFormat::from_extension("ogg"), AudioFormat::Ogg);
        assert_eq!(AudioFormat::from_extension("flac"), AudioFormat::Flac);
        assert_eq!(AudioFormat::from_extension("unknown"), AudioFormat::Unknown);
    }

    #[test]
    fn test_format_capabilities() {
        assert!(AudioFormat::Mp3.supports_streaming());
        assert!(!AudioFormat::Wav.supports_streaming());
        
        assert!(AudioFormat::Wav.supports_seeking());
        assert!(AudioFormat::Mp3.supports_seeking());
        
        assert_eq!(AudioFormat::Wav.typical_extension(), "wav");
        assert_eq!(AudioFormat::Mp3.typical_extension(), "mp3");
    }

    #[test]
    fn test_audio_converter_sample_conversion() {
        let f32_samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let i16_samples = AudioConverter::convert_samples_f32_to_i16(&f32_samples);
        let back_to_f32 = AudioConverter::convert_samples_i16_to_f32(&i16_samples);

        // Check that conversion is approximately correct
        for (original, converted) in f32_samples.iter().zip(back_to_f32.iter()) {
            assert!((original - converted).abs() < 0.01, 
                   "Original: {}, Converted: {}", original, converted);
        }
    }

    #[test]
    fn test_audio_converter_channel_conversion() {
        let mono_data = vec![1.0, 0.5, -0.5, -1.0];
        let stereo_data = AudioConverter::mono_to_stereo(&mono_data);
        
        assert_eq!(stereo_data.len(), mono_data.len() * 2);
        assert_eq!(stereo_data[0], mono_data[0]);
        assert_eq!(stereo_data[1], mono_data[0]);

        let back_to_mono = AudioConverter::stereo_to_mono(&stereo_data);
        assert_eq!(back_to_mono.len(), mono_data.len());
        
        // Should be approximately equal (accounting for mixing)
        for (original, converted) in mono_data.iter().zip(back_to_mono.iter()) {
            assert!((original - converted).abs() < 0.01);
        }
    }

    #[test]
    fn test_audio_converter_volume() {
        let mut samples = vec![1.0, 0.5, -0.5, -1.0];
        AudioConverter::apply_volume(&mut samples, 0.5);
        
        let expected = vec![0.5, 0.25, -0.25, -0.5];
        for (actual, expected) in samples.iter().zip(expected.iter()) {
            assert!((actual - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_audio_converter_normalization() {
        let mut samples = vec![2.0, 1.0, -1.0, -2.0];
        AudioConverter::normalize(&mut samples);
        
        // After normalization, max amplitude should be 1.0
        let max_amplitude = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
        assert!((max_amplitude - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_audio_format_detector() {
        let supported = AudioFormatDetector::supported_formats();
        assert!(!supported.is_empty());
        assert!(supported.contains(&AudioFormat::Wav));
        assert!(supported.contains(&AudioFormat::Mp3));

        let extensions = AudioFormatDetector::supported_extensions();
        assert!(!extensions.is_empty());
        assert!(extensions.contains(&"wav"));
        assert!(extensions.contains(&"mp3"));

        assert!(AudioFormatDetector::is_supported(AudioFormat::Wav));
        assert!(!AudioFormatDetector::is_supported(AudioFormat::Unknown));
    }

    #[test]
    fn test_resampling() {
        let input = vec![1.0, 0.5, 0.0, -0.5, -1.0, -0.5, 0.0, 0.5]; // 8 samples, mono
        
        // Downsample 44100 -> 22050 (half rate)
        let downsampled = AudioConverter::resample_linear(&input, 44100, 22050, 1);
        assert_eq!(downsampled.len(), 4); // Should be half the length
        
        // Upsample 22050 -> 44100 (double rate)  
        let upsampled = AudioConverter::resample_linear(&downsampled, 22050, 44100, 1);
        assert_eq!(upsampled.len(), 8); // Should be double the length
        
        // Same rate should return identical data
        let same_rate = AudioConverter::resample_linear(&input, 44100, 44100, 1);
        assert_eq!(same_rate, input);
    }

    // Note: Tests for SymphoniaDecoder would require actual audio file data,
    // which we can't easily include in unit tests. Integration tests would
    // use real audio files to test the decoder functionality.
}