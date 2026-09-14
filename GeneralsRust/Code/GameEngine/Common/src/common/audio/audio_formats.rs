//! Audio Format Support
//! 
//! This module provides comprehensive audio format support using the Symphonia
//! audio library, enabling playback of MP3, WAV, OGG, FLAC, and other formats
//! commonly used in Command & Conquer games.

use std::io::Cursor;
use std::num::NonZero;
use std::sync::Arc;
use std::time::Duration;

use rodio::{ChannelCount, Sample, SampleRate, Source};
use symphonia::core::audio::conv::FromSample;
use symphonia::core::audio::{Audio, AudioBuffer, GenericAudioBufferRef};
use symphonia::core::codecs::audio::{AudioDecoder, well_known};
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatReader, SeekMode, SeekTo, Track, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::units::Time;

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
pub struct SymphoniaDecoder {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    current_samples: Option<Vec<f32>>,
    sample_offset: usize,
    sample_rate: u32,
    channels: u16,
    metadata: AudioMetadata,
}

impl SymphoniaDecoder {
    /// Create a new decoder for the given reader
    pub fn new<R>(reader: R) -> Result<Self, String>
    where
        R: MediaSource + 'static,
    {
        let media_source =
            MediaSourceStream::new(Box::new(reader), MediaSourceStreamOptions::default());

        let hint = Hint::new();
        let mut format_reader = symphonia::default::get_probe()
            .probe(&hint, media_source, Default::default(), Default::default())
            .map_err(|e| format!("Failed to probe audio format: {}", e))?;

        let track = format_reader
            .default_track(TrackType::Audio)
            .ok_or("No suitable audio track found")?;
        let track_id = track.id;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .cloned()
            .ok_or("No suitable audio track found")?;

        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &Default::default())
            .map_err(|e| format!("Failed to create decoder: {}", e))?;

        let metadata = Self::extract_metadata(track);
        let sample_rate = params.sample_rate.unwrap_or(44100);
        let channels = params.channels.map(|ch| ch.count() as u16).unwrap_or(2);

        Ok(Self {
            format_reader,
            decoder,
            track_id,
            current_samples: None,
            sample_offset: 0,
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

        self.format_reader
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time::from_nanos_u64(position.as_nanos() as u64),
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| format!("Seek failed: {}", e))?;

        self.decoder.reset();
        self.current_samples = None;
        self.sample_offset = 0;

        Ok(())
    }

    fn extract_metadata(track: &Track) -> AudioMetadata {
        let mut meta = AudioMetadata::default();
        let Some(params) = track.codec_params.as_ref().and_then(|p| p.audio()) else {
            return meta;
        };

        if let Some(sample_rate) = params.sample_rate {
            meta.sample_rate = sample_rate;
        }
        if let Some(channels) = params.channels.as_ref() {
            meta.channels = channels.count() as u16;
        }
        if let Some(bits_per_sample) = params.bits_per_sample {
            meta.bits_per_sample = Some(bits_per_sample as u16);
        }
        if let Some(frames) = track.num_frames {
            meta.total_frames = Some(frames);
            if meta.sample_rate > 0 {
                meta.duration = Some(Duration::from_secs_f64(frames as f64 / meta.sample_rate as f64));
            }
        }

        meta.format = match params.codec {
            well_known::CODEC_ID_MP3 => AudioFormat::Mp3,
            well_known::CODEC_ID_VORBIS => AudioFormat::Ogg,
            well_known::CODEC_ID_FLAC => AudioFormat::Flac,
            well_known::CODEC_ID_AAC => AudioFormat::Aac,
            _ => AudioFormat::Wav,
        };
        meta.is_lossless =
            matches!(meta.format, AudioFormat::Wav | AudioFormat::Flac | AudioFormat::Aiff);
        meta
    }

    fn interleaved_f32(buffer: &GenericAudioBufferRef<'_>) -> Vec<f32> {
        match buffer {
            GenericAudioBufferRef::U8(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::U16(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::U24(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::U32(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::S8(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::S16(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::S24(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::S32(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::F32(buf) => Self::interleave_buffer(buf),
            GenericAudioBufferRef::F64(buf) => Self::interleave_buffer(buf),
        }
    }

    fn interleave_buffer<S>(buf: &AudioBuffer<S>) -> Vec<f32>
    where
        S: symphonia::core::audio::sample::Sample,
        f32: FromSample<S>,
    {
        let planes = buf.num_planes();
        let frames = buf.frames();
        let mut out = Vec::with_capacity(frames * planes);
        for i in 0..frames {
            for p in 0..planes {
                if let Some(plane) = buf.plane(p) {
                    out.push(f32::from_sample(plane[i]));
                }
            }
        }
        out
    }
}

impl Iterator for SymphoniaDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(samples) = self.current_samples.as_ref() {
                if self.sample_offset < samples.len() {
                    let sample = samples[self.sample_offset];
                    self.sample_offset += 1;
                    return Some(sample);
                }
                self.current_samples = None;
                self.sample_offset = 0;
            }

            match self.format_reader.next_packet() {
                Ok(Some(packet)) if packet.track_id == self.track_id => {
                    match self.decoder.decode(&packet) {
                        Ok(decoded) => {
                            self.current_samples = Some(Self::interleaved_f32(&decoded));
                            self.sample_offset = 0;
                        }
                        Err(_) => continue,
                    }
                }
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => return None,
            }
        }
    }
}

impl Source for SymphoniaDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::new(self.channels.max(1)).expect("channels")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(self.sample_rate.max(1)).expect("sample_rate")
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
pub struct StreamingDecoder {
    decoder: SymphoniaDecoder,
    buffer: Vec<f32>,
    buffer_size: usize,
    position: usize,
}

impl StreamingDecoder {
    /// Create new streaming decoder with specified buffer size
    pub fn new<R>(reader: R, buffer_size: usize) -> Result<Self, String>
    where
        R: MediaSource + 'static,
    {
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
pub fn create_decoder_from_data(data: Vec<u8>) -> Result<SymphoniaDecoder, String> {
    let cursor = Cursor::new(data);
    SymphoniaDecoder::new(cursor)
}

/// Create a streaming decoder from raw audio data
pub fn create_streaming_decoder_from_data(
    data: Vec<u8>,
    buffer_size: usize,
) -> Result<StreamingDecoder, String> {
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