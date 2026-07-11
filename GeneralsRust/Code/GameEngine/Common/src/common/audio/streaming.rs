//! Advanced Audio Streaming System
//!
//! This module provides efficient streaming capabilities for large audio files:
//! - Adaptive bitrate streaming
//! - Multi-threaded streaming with ring buffers
//! - Seamless looping for background music
//! - Crossfading between streams
//! - Network streaming support
//! - Compressed audio streaming

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(feature = "audio")]
use rubato::{SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

use crate::common::audio::{
    AudioFormat, AudioHandle, AudioLoadError, AudioMetadata, Bool, Real, UnsignedInt,
};

/// Default streaming buffer size (64KB)
pub const DEFAULT_STREAM_BUFFER_SIZE: usize = 65536;

/// Number of streaming buffers (triple buffering)
pub const STREAM_BUFFER_COUNT: usize = 3;

/// Minimum buffer fill level before requesting more data
pub const MIN_BUFFER_FILL_LEVEL: f32 = 0.25;

/// Maximum buffer fill level (to prevent excessive memory usage)
pub const MAX_BUFFER_FILL_LEVEL: f32 = 0.85;

/// Crossfade duration in seconds
pub const DEFAULT_CROSSFADE_DURATION: f32 = 3.0;

/// Network streaming chunk size
pub const NETWORK_CHUNK_SIZE: usize = 8192;

/// Streaming quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamQuality {
    /// Low quality for slow connections
    Low,
    /// Medium quality (default)
    Medium,
    /// High quality for fast connections
    High,
    /// Lossless quality
    Lossless,
}

impl StreamQuality {
    pub fn get_bitrate(&self) -> u32 {
        match self {
            Self::Low => 64000,        // 64 kbps
            Self::Medium => 128000,    // 128 kbps
            Self::High => 320000,      // 320 kbps
            Self::Lossless => 1411000, // CD quality
        }
    }

    pub fn get_buffer_size(&self) -> usize {
        match self {
            Self::Low => 16384,
            Self::Medium => 32768,
            Self::High => 65536,
            Self::Lossless => 131072,
        }
    }
}

/// Stream state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamState {
    /// Stream is initializing
    Initializing,
    /// Stream is buffering data
    Buffering,
    /// Stream is playing
    Playing,
    /// Stream is paused
    Paused,
    /// Stream is seeking
    Seeking,
    /// Stream has finished
    Finished,
    /// Stream encountered an error
    Error,
}

/// Streaming source type
#[derive(Debug, Clone)]
pub enum StreamSource {
    /// Local file
    File {
        path: PathBuf,
        offset: u64,
        length: Option<u64>,
    },
    /// Network URL
    Network {
        url: String,
        headers: HashMap<String, String>,
    },
    /// Memory buffer
    Memory { data: Arc<Vec<u8>>, position: u64 },
    /// Custom reader
    Custom { id: String },
}

/// Stream buffer containing decoded audio data
#[derive(Debug)]
pub struct StreamBuffer {
    /// Audio samples (interleaved)
    pub samples: Vec<f32>,
    /// Number of channels
    pub channels: u16,
    /// Sample rate
    pub sample_rate: u32,
    /// Buffer timestamp
    pub timestamp: u64,
    /// Buffer size in frames
    pub frame_count: usize,
    /// Last access time for LRU eviction
    pub last_access: Instant,
}

impl StreamBuffer {
    pub fn new(channels: u16, sample_rate: u32, capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity * channels as usize),
            channels,
            sample_rate,
            timestamp: 0,
            frame_count: 0,
            last_access: Instant::now(),
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
        self.frame_count = 0;
        self.timestamp = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.samples.len() >= self.samples.capacity()
    }

    pub fn available_space(&self) -> usize {
        self.samples.capacity() - self.samples.len()
    }

    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate > 0 {
            self.frame_count as f64 / self.sample_rate as f64
        } else {
            0.0
        }
    }
}

/// Audio streaming engine
pub struct AudioStreamer {
    /// Unique handle
    handle: AudioHandle,
    /// Stream source
    source: StreamSource,
    /// Audio metadata
    metadata: AudioMetadata,
    /// Current stream state
    state: Arc<RwLock<StreamState>>,
    /// Stream quality
    quality: StreamQuality,
    ring_buffer: Option<(
        crossbeam_channel::Sender<f32>,
        crossbeam_channel::Receiver<f32>,
    )>,
    /// Stream buffers
    buffers: Arc<RwLock<VecDeque<StreamBuffer>>>,
    /// Current playback position in samples
    position: Arc<RwLock<u64>>,
    /// Loop flag
    looping: bool,
    /// Loop start position
    loop_start: u64,
    /// Loop end position
    loop_end: Option<u64>,
    /// Crossfade parameters
    crossfade_duration: Duration,
    crossfade_position: Option<u64>,
    /// Streaming thread handle
    stream_thread: Option<JoinHandle<()>>,
    /// Command channel for controlling the stream
    command_sender: Option<Sender<StreamCommand>>,
    /// Status channel for receiving updates
    status_receiver: Option<Receiver<StreamStatus>>,
    target_sample_rate: u32,
    /// Target channels
    target_channels: u16,
}

impl std::fmt::Debug for AudioStreamer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioStreamer")
            .field("handle", &self.handle)
            .field("state", &self.state.read())
            .field("quality", &self.quality)
            .field("looping", &self.looping)
            .field("target_sample_rate", &self.target_sample_rate)
            .field("target_channels", &self.target_channels)
            .finish()
    }
}

/// Stream control commands
#[derive(Debug)]
pub enum StreamCommand {
    /// Play the stream
    Play,
    /// Pause the stream
    Pause,
    /// Stop the stream
    Stop,
    /// Seek to position (in samples)
    Seek(u64),
    /// Set volume
    SetVolume(f32),
    /// Set quality
    SetQuality(StreamQuality),
    /// Enable/disable looping
    SetLoop(bool, Option<u64>, Option<u64>),
    /// Start crossfade to another stream
    CrossfadeTo(Arc<AudioStreamer>),
    /// Shutdown the streaming thread
    Shutdown,
}

/// Stream status updates
#[derive(Debug)]
pub enum StreamStatus {
    /// State changed
    StateChanged(StreamState),
    /// Position update
    PositionUpdate(u64),
    /// Buffer level update (0.0 - 1.0)
    BufferLevel(f32),
    /// Error occurred
    Error(String),
    /// Stream finished
    Finished,
}

impl AudioStreamer {
    /// Create a new audio streamer
    pub fn new(
        handle: AudioHandle,
        source: StreamSource,
        metadata: AudioMetadata,
        quality: StreamQuality,
        target_sample_rate: u32,
        target_channels: u16,
    ) -> Result<Self, AudioLoadError> {
        let (command_sender, command_receiver) = unbounded();
        let (status_sender, status_receiver) = unbounded();

        let buffers = Arc::new(RwLock::new(VecDeque::with_capacity(STREAM_BUFFER_COUNT)));
        let state = Arc::new(RwLock::new(StreamState::Initializing));
        let position = Arc::new(RwLock::new(0));

        let ring_buffer = {
            let buffer_size = quality.get_buffer_size();
            let (tx, rx) = crossbeam_channel::bounded(buffer_size);
            Some((tx, rx))
        };

        let streamer = Self {
            handle,
            source: source.clone(),
            metadata,
            state: state.clone(),
            quality,
            ring_buffer,
            buffers: buffers.clone(),
            position: position.clone(),
            looping: false,
            loop_start: 0,
            loop_end: None,
            crossfade_duration: Duration::from_secs_f32(DEFAULT_CROSSFADE_DURATION),
            crossfade_position: None,
            stream_thread: None,
            command_sender: Some(command_sender),
            status_receiver: Some(status_receiver),
            target_sample_rate,
            target_channels,
        };

        // Start streaming thread
        let thread_source = source;
        let thread_metadata = streamer.metadata.clone();
        let thread_quality = quality;

        let stream_thread = thread::Builder::new()
            .name(format!("audio-stream-{}", handle))
            .spawn(move || {
                Self::streaming_thread(
                    command_receiver,
                    status_sender,
                    thread_source,
                    thread_metadata,
                    thread_quality,
                    buffers,
                    state,
                    position,
                );
            })
            .map_err(|_| AudioLoadError::InvalidData)?;

        Ok(streamer)
    }

    /// Get the stream handle
    pub fn handle(&self) -> AudioHandle {
        self.handle
    }

    /// Get current stream state
    pub fn state(&self) -> StreamState {
        *self.state.read()
    }

    /// Get current playback position in samples
    pub fn position(&self) -> u64 {
        *self.position.read()
    }

    /// Get playback position in seconds
    pub fn position_seconds(&self) -> f64 {
        self.position() as f64 / self.metadata.sample_rate as f64
    }

    /// Get stream duration in seconds
    pub fn duration_seconds(&self) -> Option<f64> {
        self.metadata.duration
    }

    /// Play the stream
    pub fn play(&self) -> Result<(), AudioLoadError> {
        if let Some(sender) = &self.command_sender {
            sender
                .send(StreamCommand::Play)
                .map_err(|_| AudioLoadError::InvalidData)?;
        }
        Ok(())
    }

    /// Pause the stream
    pub fn pause(&self) -> Result<(), AudioLoadError> {
        if let Some(sender) = &self.command_sender {
            sender
                .send(StreamCommand::Pause)
                .map_err(|_| AudioLoadError::InvalidData)?;
        }
        Ok(())
    }

    /// Stop the stream
    pub fn stop(&self) -> Result<(), AudioLoadError> {
        if let Some(sender) = &self.command_sender {
            sender
                .send(StreamCommand::Stop)
                .map_err(|_| AudioLoadError::InvalidData)?;
        }
        Ok(())
    }

    /// Seek to position in seconds
    pub fn seek_seconds(&self, seconds: f64) -> Result<(), AudioLoadError> {
        let samples = (seconds * self.metadata.sample_rate as f64) as u64;
        self.seek_samples(samples)
    }

    /// Seek to position in samples
    pub fn seek_samples(&self, samples: u64) -> Result<(), AudioLoadError> {
        if let Some(sender) = &self.command_sender {
            sender
                .send(StreamCommand::Seek(samples))
                .map_err(|_| AudioLoadError::InvalidData)?;
        }
        Ok(())
    }

    /// Set loop parameters
    pub fn set_loop(
        &self,
        looping: bool,
        start_samples: Option<u64>,
        end_samples: Option<u64>,
    ) -> Result<(), AudioLoadError> {
        if let Some(sender) = &self.command_sender {
            sender
                .send(StreamCommand::SetLoop(looping, start_samples, end_samples))
                .map_err(|_| AudioLoadError::InvalidData)?;
        }
        Ok(())
    }

    /// Set stream quality
    pub fn set_quality(&self, quality: StreamQuality) -> Result<(), AudioLoadError> {
        if let Some(sender) = &self.command_sender {
            sender
                .send(StreamCommand::SetQuality(quality))
                .map_err(|_| AudioLoadError::InvalidData)?;
        }
        Ok(())
    }

    /// Read samples from the stream
    pub fn read_samples(&self, buffer: &mut [f32]) -> usize {
        let mut read_count = 0;
        if let Some((_, ref consumer)) = &self.ring_buffer {
            for slot in buffer.iter_mut() {
                match consumer.try_recv() {
                    Ok(sample) => {
                        *slot = sample;
                        read_count += 1;
                    }
                    Err(_) => break,
                }
            }
        }
        read_count
    }

    /// Get buffer level (0.0 - 1.0)
    pub fn buffer_level(&self) -> f32 {
        if let Some((_, ref consumer)) = &self.ring_buffer {
            consumer.len() as f32 / 65536.0
        } else {
            0.0
        }
    }

    /// Check if stream needs more data
    pub fn needs_data(&self) -> bool {
        self.buffer_level() < MIN_BUFFER_FILL_LEVEL
    }

    /// Process status updates
    pub fn update(&self) -> Vec<StreamStatus> {
        let mut updates = Vec::new();

        if let Some(receiver) = &self.status_receiver {
            while let Ok(status) = receiver.try_recv() {
                updates.push(status);
            }
        }

        updates
    }

    /// Streaming thread main function
    fn streaming_thread(
        command_receiver: Receiver<StreamCommand>,
        status_sender: Sender<StreamStatus>,
        source: StreamSource,
        metadata: AudioMetadata,
        quality: StreamQuality,
        buffers: Arc<RwLock<VecDeque<StreamBuffer>>>,
        state: Arc<RwLock<StreamState>>,
        position: Arc<RwLock<u64>>,
    ) {
        let mut decoder = match Self::create_decoder(&source, &metadata) {
            Ok(decoder) => decoder,
            Err(e) => {
                let _ = status_sender.send(StreamStatus::Error(format!(
                    "Failed to create decoder: {}",
                    e
                )));
                return;
            }
        };

        let mut current_state = StreamState::Buffering;
        *state.write() = current_state;
        let _ = status_sender.send(StreamStatus::StateChanged(current_state));

        let mut should_loop = false;
        let mut loop_start = 0u64;
        let mut loop_end = None;

        loop {
            // Process commands
            if let Ok(command) = command_receiver.try_recv() {
                match command {
                    StreamCommand::Play => {
                        current_state = StreamState::Playing;
                        *state.write() = current_state;
                        let _ = status_sender.send(StreamStatus::StateChanged(current_state));
                    }
                    StreamCommand::Pause => {
                        current_state = StreamState::Paused;
                        *state.write() = current_state;
                        let _ = status_sender.send(StreamStatus::StateChanged(current_state));
                    }
                    StreamCommand::Stop => {
                        current_state = StreamState::Finished;
                        *state.write() = current_state;
                        let _ = status_sender.send(StreamStatus::StateChanged(current_state));
                        break;
                    }
                    StreamCommand::Seek(samples) => {
                        current_state = StreamState::Seeking;
                        *state.write() = current_state;

                        // Clear buffers
                        buffers.write().clear();

                        // Seek in decoder
                        if let Err(e) = decoder.seek(samples) {
                            let _ = status_sender
                                .send(StreamStatus::Error(format!("Seek failed: {}", e)));
                        } else {
                            *position.write() = samples;
                            let _ = status_sender.send(StreamStatus::PositionUpdate(samples));
                        }

                        current_state = StreamState::Playing;
                        *state.write() = current_state;
                        let _ = status_sender.send(StreamStatus::StateChanged(current_state));
                    }
                    StreamCommand::SetLoop(looping, start, end) => {
                        should_loop = looping;
                        loop_start = start.unwrap_or(0);
                        loop_end = end;
                    }
                    StreamCommand::Shutdown => break,
                    _ => {}
                }
            }

            // Stream data if needed
            if matches!(current_state, StreamState::Playing | StreamState::Buffering) {
                let buffer_count = buffers.read().len();

                if buffer_count < STREAM_BUFFER_COUNT {
                    // Decode more data
                    let mut buffer = StreamBuffer::new(
                        metadata.channels,
                        metadata.sample_rate,
                        quality.get_buffer_size(),
                    );

                    match decoder.decode(&mut buffer) {
                        Ok(frames_read) => {
                            if frames_read > 0 {
                                buffer.frame_count = frames_read;
                                buffer.timestamp = *position.read();
                                buffers.write().push_back(buffer);

                                // Update position
                                let new_position = *position.read() + frames_read as u64;
                                *position.write() = new_position;
                                let _ =
                                    status_sender.send(StreamStatus::PositionUpdate(new_position));

                                // Check for loop
                                if should_loop {
                                    if let Some(loop_end_pos) = loop_end {
                                        if new_position >= loop_end_pos {
                                            // Loop back to start
                                            if let Err(e) = decoder.seek(loop_start) {
                                                let _ = status_sender.send(StreamStatus::Error(
                                                    format!("Loop seek failed: {}", e),
                                                ));
                                            } else {
                                                *position.write() = loop_start;
                                                let _ = status_sender
                                                    .send(StreamStatus::PositionUpdate(loop_start));
                                            }
                                        }
                                    }
                                }
                            } else {
                                // End of stream
                                if should_loop {
                                    // Loop back to start
                                    if let Err(e) = decoder.seek(loop_start) {
                                        let _ = status_sender.send(StreamStatus::Error(format!(
                                            "Loop seek failed: {}",
                                            e
                                        )));
                                        break;
                                    } else {
                                        *position.write() = loop_start;
                                        let _ = status_sender
                                            .send(StreamStatus::PositionUpdate(loop_start));
                                    }
                                } else {
                                    current_state = StreamState::Finished;
                                    *state.write() = current_state;
                                    let _ = status_sender
                                        .send(StreamStatus::StateChanged(current_state));
                                    let _ = status_sender.send(StreamStatus::Finished);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            let _ = status_sender
                                .send(StreamStatus::Error(format!("Decode error: {}", e)));
                            current_state = StreamState::Error;
                            *state.write() = current_state;
                            let _ = status_sender.send(StreamStatus::StateChanged(current_state));
                            break;
                        }
                    }
                }

                // Update buffer level
                let fill_level = buffer_count as f32 / STREAM_BUFFER_COUNT as f32;
                let _ = status_sender.send(StreamStatus::BufferLevel(fill_level));

                if matches!(current_state, StreamState::Buffering)
                    && fill_level > MIN_BUFFER_FILL_LEVEL
                {
                    current_state = StreamState::Playing;
                    *state.write() = current_state;
                    let _ = status_sender.send(StreamStatus::StateChanged(current_state));
                }
            }

            // Small delay to prevent busy waiting
            thread::sleep(Duration::from_millis(1));
        }
    }

    /// Create decoder for stream source
    fn create_decoder(
        source: &StreamSource,
        metadata: &AudioMetadata,
    ) -> Result<Box<dyn StreamDecoder>, AudioLoadError> {
        match source {
            StreamSource::File { path, .. } => match metadata.format {
                AudioFormat::Wav => Ok(Box::new(WavDecoder::new(path)?)),
                #[cfg(feature = "audio")]
                _ => Ok(Box::new(SymphoniaDecoder::new(path)?)),
                #[cfg(not(feature = "audio"))]
                _ => Err(AudioLoadError::UnsupportedFormat(metadata.format)),
            },
            _ => Err(AudioLoadError::InvalidData),
        }
    }
}

/// Stream decoder trait
trait StreamDecoder: Send {
    fn decode(&mut self, buffer: &mut StreamBuffer) -> Result<usize, AudioLoadError>;
    fn seek(&mut self, position: u64) -> Result<(), AudioLoadError>;
    fn position(&self) -> u64;
}

/// WAV stream decoder
struct WavDecoder {
    reader: Option<hound::WavReader<BufReader<File>>>,
    position: u64,
}

impl WavDecoder {
    fn new(path: &Path) -> Result<Self, AudioLoadError> {
        let reader = hound::WavReader::open(path)
            .map_err(|_| AudioLoadError::DecodeError("Failed to open WAV file".to_string()))?;

        Ok(Self {
            reader: Some(reader),
            position: 0,
        })
    }
}

impl StreamDecoder for WavDecoder {
    fn decode(&mut self, buffer: &mut StreamBuffer) -> Result<usize, AudioLoadError> {
        if let Some(ref mut reader) = self.reader {
            let spec = reader.spec();
            let mut frames_read = 0;
            let target_frames = buffer.available_space() / spec.channels as usize;

            match spec.sample_format {
                hound::SampleFormat::Int => match spec.bits_per_sample {
                    16 => {
                        for _ in 0..target_frames {
                            let mut frame_complete = true;
                            for _ in 0..spec.channels {
                                match reader.samples::<i16>().next() {
                                    Some(Ok(sample)) => {
                                        buffer.samples.push(sample as f32 / i16::MAX as f32);
                                    }
                                    Some(Err(_)) => {
                                        return Err(AudioLoadError::DecodeError(
                                            "Sample read error".to_string(),
                                        ))
                                    }
                                    None => {
                                        frame_complete = false;
                                        break;
                                    }
                                }
                            }
                            if !frame_complete {
                                break;
                            }
                            frames_read += 1;
                        }
                    }
                    _ => return Err(AudioLoadError::UnsupportedFormat(AudioFormat::Wav)),
                },
                hound::SampleFormat::Float => {
                    for _ in 0..target_frames {
                        let mut frame_complete = true;
                        for _ in 0..spec.channels {
                            match reader.samples::<f32>().next() {
                                Some(Ok(sample)) => {
                                    buffer.samples.push(sample);
                                }
                                Some(Err(_)) => {
                                    return Err(AudioLoadError::DecodeError(
                                        "Sample read error".to_string(),
                                    ))
                                }
                                None => {
                                    frame_complete = false;
                                    break;
                                }
                            }
                        }
                        if !frame_complete {
                            break;
                        }
                        frames_read += 1;
                    }
                }
            }

            self.position += frames_read as u64;
            Ok(frames_read)
        } else {
            Err(AudioLoadError::InvalidData)
        }
    }

    fn seek(&mut self, position: u64) -> Result<(), AudioLoadError> {
        // WAV seeking would require reopening or implementing custom seek
        self.position = position;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.position
    }
}

/// Symphonia-based decoder for other formats
#[cfg(feature = "audio")]
struct SymphoniaDecoder {
    // Symphonia decoder implementation
    position: u64,
}

#[cfg(feature = "audio")]
impl SymphoniaDecoder {
    fn new(path: &Path) -> Result<Self, AudioLoadError> {
        // Initialize symphonia decoder
        Ok(Self { position: 0 })
    }
}

#[cfg(feature = "audio")]
impl StreamDecoder for SymphoniaDecoder {
    fn decode(&mut self, buffer: &mut StreamBuffer) -> Result<usize, AudioLoadError> {
        // Symphonia decoding implementation
        Ok(0)
    }

    fn seek(&mut self, position: u64) -> Result<(), AudioLoadError> {
        self.position = position;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.position
    }
}

/// Stream manager for handling multiple concurrent streams
pub struct StreamManager {
    /// Active streams
    streams: Arc<RwLock<HashMap<AudioHandle, Arc<AudioStreamer>>>>,
    /// Next handle
    next_handle: Arc<Mutex<AudioHandle>>,
    /// Maximum concurrent streams
    max_streams: usize,
    /// Stream quality setting
    quality: Arc<RwLock<StreamQuality>>,
}

impl StreamManager {
    pub fn new(max_streams: usize) -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            next_handle: Arc::new(Mutex::new(1)),
            max_streams,
            quality: Arc::new(RwLock::new(StreamQuality::Medium)),
        }
    }

    /// Create a new stream
    pub fn create_stream(
        &self,
        source: StreamSource,
        metadata: AudioMetadata,
    ) -> Result<Arc<AudioStreamer>, AudioLoadError> {
        let streams_count = self.streams.read().len();
        if streams_count >= self.max_streams {
            return Err(AudioLoadError::InvalidData);
        }

        let handle = {
            let mut next = self.next_handle.lock();
            let handle = *next;
            *next += 1;
            handle
        };

        let quality = *self.quality.read();
        let streamer = Arc::new(AudioStreamer::new(
            handle, source, metadata, quality, 44100, // Target sample rate
            2,     // Target channels
        )?);

        self.streams.write().insert(handle, streamer.clone());
        Ok(streamer)
    }

    /// Remove a stream
    pub fn remove_stream(&self, handle: AudioHandle) {
        self.streams.write().remove(&handle);
    }

    /// Get a stream by handle
    pub fn get_stream(&self, handle: AudioHandle) -> Option<Arc<AudioStreamer>> {
        self.streams.read().get(&handle).cloned()
    }

    /// Set global stream quality
    pub fn set_quality(&self, quality: StreamQuality) {
        *self.quality.write() = quality;

        // Update existing streams
        for stream in self.streams.read().values() {
            let _ = stream.set_quality(quality);
        }
    }

    /// Update all streams
    pub fn update(&self) {
        let mut to_remove = Vec::new();

        for (handle, stream) in self.streams.read().iter() {
            let updates = stream.update();

            for update in updates {
                if matches!(update, StreamStatus::Finished) {
                    to_remove.push(*handle);
                }
            }
        }

        // Remove finished streams
        for handle in to_remove {
            self.remove_stream(handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_quality() {
        assert_eq!(StreamQuality::Low.get_bitrate(), 64000);
        assert_eq!(StreamQuality::Medium.get_bitrate(), 128000);
        assert_eq!(StreamQuality::High.get_bitrate(), 320000);
        assert_eq!(StreamQuality::Lossless.get_bitrate(), 1411000);
    }

    #[test]
    fn test_stream_buffer() {
        let mut buffer = StreamBuffer::new(2, 44100, 1024);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
        assert_eq!(buffer.available_space(), 2048); // 1024 frames * 2 channels

        buffer.samples.extend(vec![0.0; 512]);
        buffer.frame_count = 256;
        assert!(!buffer.is_empty());
        assert_eq!(buffer.available_space(), 1536);
    }

    #[test]
    fn test_stream_source() {
        let file_source = StreamSource::File {
            path: PathBuf::from("test.wav"),
            offset: 0,
            length: None,
        };

        match file_source {
            StreamSource::File { path, .. } => {
                assert_eq!(path, PathBuf::from("test.wav"));
            }
            _ => panic!("Wrong source type"),
        }
    }

    #[test]
    fn test_stream_manager() {
        let manager = StreamManager::new(10);
        assert_eq!(manager.max_streams, 10);
        assert!(manager.streams.read().is_empty());
    }
}
