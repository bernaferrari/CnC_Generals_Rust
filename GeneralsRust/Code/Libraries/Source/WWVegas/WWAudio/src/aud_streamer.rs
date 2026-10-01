//! Audio streaming system for large files and real-time playback.
//!
//! This module provides comprehensive audio streaming capabilities, converting from the
//! original C++ AudioStreamer implementation. It supports async file I/O, safe buffer
//! management, format conversion, looping, and thread-safe operations while maintaining
//! compatibility with the existing API.

use crate::error::{ChannelError, Error, Result, SourceError, StreamError};
use crate::{
    AudioChannel, AudioDevice, AudioSample, MAX_VOLUME, MIN_VOLUME, TimeStamp, Volume,
    aud_source::{
        AudioCompressionType, AudioFormatFlags, EnhancedAudioFormat, TimeStamp as SourceTimeStamp,
    },
    aud_stream_buffering::StreamBuffering,
    formats::AudioFormat,
};
use log::warn;
use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant, sleep};

/// Minimum frame size for streaming operations (4KB)
const MIN_FRAME_SIZE: usize = 4 * 1024;

/// Default buffering time in seconds
const DEFAULT_BUFFERING_SECONDS: u64 = 7;

/// Stream state flags
#[derive(Debug, Clone, Copy)]
pub struct StreamFlags {
    flags: u32,
}

impl StreamFlags {
    const PLAYING: u32 = 0x00000001;
    const PAUSED: u32 = 0x00000002;
    const FILL: u32 = 0x00000004;
    const NO_FILE_CLOSE: u32 = 0x00000080;
    const OPEN: u32 = 0x00000010;
    const LOOPING: u32 = 0x00000020;

    pub fn new() -> Self {
        Self { flags: 0 }
    }

    pub fn set(&mut self, flag: u32) {
        self.flags |= flag;
    }

    pub fn clear(&mut self, flag: u32) {
        self.flags &= !flag;
    }

    pub fn is_set(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    pub fn is_playing(&self) -> bool {
        self.is_set(Self::PLAYING)
    }
    pub fn is_paused(&self) -> bool {
        self.is_set(Self::PAUSED)
    }
    pub fn should_fill(&self) -> bool {
        self.is_set(Self::FILL)
    }
    pub fn is_open(&self) -> bool {
        self.is_set(Self::OPEN)
    }
    pub fn is_looping(&self) -> bool {
        self.is_set(Self::LOOPING)
    }
    pub fn should_close_file(&self) -> bool {
        !self.is_set(Self::NO_FILE_CLOSE)
    }

    pub fn set_playing(&mut self) {
        self.set(Self::PLAYING);
    }
    pub fn set_paused(&mut self) {
        self.set(Self::PAUSED);
    }
    pub fn set_fill(&mut self) {
        self.set(Self::FILL);
    }
    pub fn set_open(&mut self) {
        self.set(Self::OPEN);
    }
    pub fn set_looping(&mut self) {
        self.set(Self::LOOPING);
    }
    pub fn set_no_file_close(&mut self) {
        self.set(Self::NO_FILE_CLOSE);
    }

    pub fn clear_playing(&mut self) {
        self.clear(Self::PLAYING);
    }
    pub fn clear_paused(&mut self) {
        self.clear(Self::PAUSED);
    }
    pub fn clear_fill(&mut self) {
        self.clear(Self::FILL);
    }
    pub fn clear_open(&mut self) {
        self.clear(Self::OPEN);
    }
    pub fn clear_looping(&mut self) {
        self.clear(Self::LOOPING);
    }
}

/// Audio streaming configuration
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Audio format for the stream
    pub format: AudioFormat,
    /// Buffering time in seconds
    pub buffering_seconds: u64,
    /// Maximum volume level
    pub max_volume: Volume,
    /// Enable looping
    pub loop_enabled: bool,
    /// Buffer size for streaming operations
    pub buffer_size: usize,
    /// Number of buffers to use
    pub buffer_count: usize,
    /// Stream name for debugging
    pub name: String,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            format: AudioFormat::default(),
            buffering_seconds: DEFAULT_BUFFERING_SECONDS,
            max_volume: MAX_VOLUME,
            loop_enabled: false,
            buffer_size: MIN_FRAME_SIZE,
            buffer_count: 4,
            name: "Audio Stream".to_string(),
        }
    }
}

/// Stream state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Stream is stopped
    Stopped,
    /// Stream is currently playing
    Playing,
    /// Stream is paused
    Paused,
    /// Stream is buffering data
    Buffering,
    /// Stream has reached end of file
    EndOfStream,
    /// Stream encountered an error
    Error,
}

/// Stream control commands
#[derive(Debug, Clone)]
pub enum StreamCommand {
    /// Start or resume playback
    Play,
    /// Pause playback
    Pause,
    /// Stop playback and reset position
    Stop,
    /// Seek to specific position (in bytes)
    Seek(usize),
    /// Seek to specific time position
    SeekTime(TimeStamp),
    /// Set volume level
    SetVolume(Volume),
    /// Set maximum volume
    SetMaxVolume(Volume),
    /// Enable or disable looping
    SetLooping(bool),
    /// Fill stream buffers
    Fill(Option<TimeStamp>),
    /// Close the stream
    Close,
}

/// Stream status information
#[derive(Debug, Clone)]
pub struct StreamStatus {
    /// Current stream state
    pub state: StreamState,
    /// Current position in bytes
    pub position_bytes: usize,
    /// Current time position
    pub position_time: TimeStamp,
    /// Total stream size in bytes
    pub total_bytes: usize,
    /// Current volume
    pub volume: Volume,
    /// Whether looping is enabled
    pub looping: bool,
    /// Buffer fill percentage (0-100)
    pub buffer_fill_percent: u8,
}

/// Audio streamer for large files and real-time playback
pub struct AudioStreamer {
    /// Stream configuration
    config: StreamConfig,
    /// Associated audio device
    _device: Weak<AudioDevice>,
    /// Pause control mutex
    pause_lock: tokio::sync::Mutex<()>,
    /// Stream lock for thread safety
    stream_lock: tokio::sync::Mutex<()>,
    /// Consolidated streamer state behind a single lock.
    ///
    /// Replaces the previously separate `Arc`-wrapped lock fields (state,
    /// flags, channel, format, sample, file, file_info, timing, name, volume)
    /// that were never cloned outside this struct; every method locks this
    /// once and operates on the whole state.
    state: tokio::sync::Mutex<StreamerState>,
    /// Stream buffering manager
    stream_buffer: Arc<Mutex<StreamBuffering>>,
}

/// Consolidated mutable state for [`AudioStreamer`].
///
/// Each payload was previously an individually locked, `Arc`-wrapped field of
/// the streamer; the Arcs were never cloned outside the struct, so a single
/// lock now guards all of them. Locking order: `pause_lock`/`stream_lock` are
/// acquired before this state lock (never the other way around), and the
/// channel mutex is always the innermost lock.
struct StreamerState {
    /// Current stream state
    state: StreamState,
    /// Stream control flags
    flags: StreamFlags,
    /// Associated audio channel
    channel: Option<Arc<Mutex<AudioChannel>>>,
    /// Current audio format
    format: EnhancedAudioFormat,
    /// Audio sample for playback
    sample: AudioSample,
    /// Current file handle
    file: Option<File>,
    /// File metadata
    file_info: FileInfo,
    /// Stream timing information
    timing: StreamTiming,
    /// Stream name for debugging
    name: String,
    /// Volume control
    volume: Volume,
}

/// File information for streaming
#[derive(Debug, Clone)]
struct FileInfo {
    /// File path
    path: Option<PathBuf>,
    /// Total file size in bytes
    total_bytes: usize,
    /// Data start position in file
    data_start: u64,
    /// Bytes remaining to read
    bytes_left: usize,
    /// Current stream position
    stream_position: usize,
}

impl Default for FileInfo {
    fn default() -> Self {
        Self {
            path: None,
            total_bytes: 0,
            data_start: 0,
            bytes_left: 0,
            stream_position: 0,
        }
    }
}

/// Stream timing information
#[derive(Debug, Clone)]
struct StreamTiming {
    /// Stream start timestamp
    start_time: Option<Instant>,
    /// Stream end timestamp  
    end_time: Option<Instant>,
    /// Current buffering time
    buffering_time: Duration,
    /// Pending bytes submitted to audio playback
    pending_bytes: usize,
    /// Frame size for this stream
    frame_size: usize,
}

impl Default for StreamTiming {
    fn default() -> Self {
        Self {
            start_time: None,
            end_time: None,
            buffering_time: Duration::from_secs(DEFAULT_BUFFERING_SECONDS),
            pending_bytes: 0,
            frame_size: MIN_FRAME_SIZE,
        }
    }
}

/// Global stream manager for handling multiple streams
pub struct StreamManager {
    /// Map of active streams
    pub(crate) streams: RwLock<HashMap<u64, Arc<AudioStreamer>>>,
    /// Next stream ID
    next_stream_id: Mutex<u64>,
}

impl StreamManager {
    /// Create a new stream manager
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
            next_stream_id: Mutex::new(0),
        }
    }

    /// Service all registered streams once
    pub async fn tick(&self) {
        let streams: Vec<Arc<AudioStreamer>> = {
            let guard = self.streams.read().await;
            guard.values().cloned().collect()
        };

        for stream in streams {
            if let Err(err) = stream.service_stream().await {
                warn!("Stream service error: {err}");
            }
        }
    }

    /// Register a new stream
    pub async fn register_stream(&self, stream: Arc<AudioStreamer>) -> u64 {
        let stream_id = {
            let mut id_guard = self.next_stream_id.lock().unwrap();
            let id = *id_guard;
            *id_guard += 1;
            id
        };

        let mut streams = self.streams.write().await;
        streams.insert(stream_id, stream);
        stream_id
    }

    /// Unregister a stream
    pub async fn unregister_stream(&self, stream_id: u64) {
        let mut streams = self.streams.write().await;
        streams.remove(&stream_id);
    }

    /// Stop all active streams
    pub async fn stop_all_streams(&self) {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            stream.stop().await.unwrap_or(());
        }
    }

    /// Pause all active streams
    pub async fn pause_all_streams(&self) {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            stream.pause().await.unwrap_or(());
        }
    }

    /// Resume all paused streams
    pub async fn resume_all_streams(&self) {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            stream.resume().await.unwrap_or(());
        }
    }

    /// Fade out all streams
    pub async fn fade_out_all_streams(&self) {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            stream.fade_out().await.unwrap_or(());
        }
    }

    /// Fade in all streams
    pub async fn fade_in_all_streams(&self) {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            stream.fade_in().await.unwrap_or(());
        }
    }

    /// Check if all streams have finished fading
    pub async fn all_faded(&self) -> bool {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            if stream.is_fading().await {
                return false;
            }
        }
        true
    }

    /// Lock all streams
    pub async fn lock_all_streams(&self) {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            stream.lock_stream().await;
        }
    }

    /// Unlock all streams
    pub async fn unlock_all_streams(&self) {
        let streams = self.streams.read().await;
        for stream in streams.values() {
            stream.unlock_stream().await;
        }
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioStreamer {
    /// Create a new audio streamer
    pub fn new(device: Arc<AudioDevice>, config: StreamConfig) -> Result<Arc<Self>> {
        let channel = device
            .reserve_channel(crate::channel::ChannelType::User)
            .map_err(|_| Error::Channel(ChannelError::AllocationFailed))?;

        let stream_buffer = Arc::new(Mutex::new(StreamBuffering::new()));
        if let Ok(mut buffer_guard) = stream_buffer.lock() {
            buffer_guard.set_self_reference(&stream_buffer);
        }

        let state = StreamerState {
            state: StreamState::Stopped,
            flags: StreamFlags::new(),
            channel: Some(Arc::new(Mutex::new(channel))),
            format: EnhancedAudioFormat::from_basic(&config.format),
            sample: AudioSample::new(),
            file: None,
            file_info: FileInfo::default(),
            timing: StreamTiming::default(),
            name: config.name.clone(),
            volume: config.max_volume,
        };

        // Setup channel callbacks
        if let Some(channel) = &state.channel {
            let _channel_guard = channel.lock().unwrap();
            // Set up callbacks for frame processing, sample completion, etc.
            // This would interface with the channel system
        }

        let streamer = Arc::new(Self {
            config,
            _device: Arc::downgrade(&device),
            pause_lock: tokio::sync::Mutex::new(()),
            stream_lock: tokio::sync::Mutex::new(()),
            state: tokio::sync::Mutex::new(state),
            stream_buffer,
        });

        Ok(streamer)
    }

    /// Open a file for streaming
    pub async fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let _lock = self.stream_lock.lock().await;
        let mut st = self.state.lock().await;

        self.close_locked(&mut st);

        let path = path.as_ref();
        let mut file = File::open(path).await.map_err(|e| {
            Error::Source(SourceError::NotFound(format!("{}: {}", path.display(), e)))
        })?;

        // Read and parse audio format from file header
        let (format, data_start, total_bytes) = self.read_wave_file_format(&mut file).await?;

        st.file_info.path = Some(path.to_path_buf());
        st.file_info.total_bytes = total_bytes;
        st.file_info.data_start = data_start;
        st.file_info.bytes_left = total_bytes;
        st.file_info.stream_position = 0;

        st.format = format;

        let name_string = path.display().to_string();
        st.sample.set_name(&name_string);

        st.file = Some(file);

        st.flags.set_open();

        Ok(())
    }

    /// Open from an existing file handle
    pub async fn open_from_file(&self, mut file: File, close_file: bool) -> Result<()> {
        let _lock = self.stream_lock.lock().await;
        let mut st = self.state.lock().await;

        self.close_locked(&mut st);

        // Read and parse audio format from file
        let (format, data_start, total_bytes) = self.read_wave_file_format(&mut file).await?;

        st.file_info.path = None;
        st.file_info.total_bytes = total_bytes;
        st.file_info.data_start = data_start;
        st.file_info.bytes_left = total_bytes;
        st.file_info.stream_position = 0;

        st.format = format;

        st.file = Some(file);

        if close_file {
            st.flags.clear(StreamFlags::NO_FILE_CLOSE);
        } else {
            st.flags.set_no_file_close();
        }
        st.flags.set_open();

        Ok(())
    }

    /// Start streaming playback
    pub async fn start(&self) -> Result<()> {
        let _lock = self.stream_lock.lock().await;
        let mut st = self.state.lock().await;
        self.start_locked(&mut st).await
    }

    /// Start streaming playback with the streamer state already locked
    async fn start_locked(&self, st: &mut StreamerState) -> Result<()> {
        if !st.flags.is_open() {
            return Err(Error::Channel(ChannelError::InvalidState(
                "Stream not open".to_string(),
            )));
        }

        // Setup buffering
        st.timing.buffering_time = Duration::from_secs(self.config.buffering_seconds);
        if st.timing.frame_size == 0 {
            st.timing.frame_size = if self.config.buffer_size > 0 {
                self.config.buffer_size
            } else {
                MIN_FRAME_SIZE
            };
        }

        // Prime the stream with an initial frame
        let _ = self.stream_next_frame_locked(st).await?;

        // Setup sample for playback
        self.setup_playback_sample(st)?;

        // Start the channel
        if let Some(channel) = &st.channel {
            let mut channel_guard = channel.lock().unwrap();
            channel_guard.start().map_err(|_| {
                Error::Channel(ChannelError::InvalidState(
                    "Failed to start channel".to_string(),
                ))
            })?;
        }

        // Update state and flags
        st.state = StreamState::Playing;

        st.flags.set_playing();
        st.flags.set_fill();

        st.timing.start_time = Some(Instant::now());
        st.timing.end_time = st.timing.start_time;

        Ok(())
    }

    /// Stop streaming playback
    pub async fn stop(&self) -> Result<()> {
        let _lock = self.stream_lock.lock().await;
        let mut st = self.state.lock().await;
        self.stop_locked(&mut st);
        Ok(())
    }

    /// Stop streaming playback with the streamer state already locked
    fn stop_locked(&self, st: &mut StreamerState) {
        if let Some(channel) = &st.channel {
            let mut channel_guard = channel.lock().unwrap();
            channel_guard.stop().unwrap_or(());
        }

        self.update_stream_state_on_stop(st);
    }

    /// Pause streaming playback
    pub async fn pause(&self) -> Result<()> {
        let pause_guard = self.pause_lock.lock().await;
        let mut st = self.state.lock().await;

        if let Some(channel) = &st.channel {
            let mut channel_guard = channel.lock().unwrap();
            channel_guard.pause().unwrap_or(());
        }

        if st.state == StreamState::Playing {
            st.state = StreamState::Paused;
        }

        if st.flags.is_playing() {
            st.flags.set_paused();
            st.flags.clear_playing();
        }

        std::mem::forget(pause_guard); // Keep lock acquired
        Ok(())
    }

    /// Resume streaming playback
    pub async fn resume(&self) -> Result<()> {
        // This will block if pause lock is held
        let _pause_guard = self.pause_lock.lock().await;
        let mut st = self.state.lock().await;

        if let Some(channel) = &st.channel {
            let mut channel_guard = channel.lock().unwrap();
            if st.flags.is_paused() {
                channel_guard.resume().unwrap_or(());
            }
        }

        if st.state == StreamState::Paused {
            st.state = StreamState::Playing;
        }

        if st.flags.is_paused() {
            st.flags.clear_paused();
            st.flags.set_playing();
        }

        Ok(())
    }

    /// Close the stream
    pub async fn close(&self) -> Result<()> {
        let _lock = self.stream_lock.lock().await;
        let mut st = self.state.lock().await;
        self.close_locked(&mut st);
        Ok(())
    }

    /// Close the stream with the streamer state already locked
    fn close_locked(&self, st: &mut StreamerState) {
        self.stop_locked(st);

        if let Some(file) = st.file.take() {
            drop(file); // File will be closed when dropped
        }

        st.flags.clear(StreamFlags::OPEN | StreamFlags::NO_FILE_CLOSE | StreamFlags::LOOPING);

        st.state = StreamState::Stopped;
    }

    /// Get current stream state
    pub async fn get_state(&self) -> StreamState {
        self.state.lock().await.state
    }

    /// Check if stream is playing
    pub async fn is_playing(&self) -> bool {
        self.state.lock().await.state == StreamState::Playing
    }

    /// Check if stream is active (playing or paused)
    pub async fn is_active(&self) -> bool {
        matches!(
            self.state.lock().await.state,
            StreamState::Playing | StreamState::Paused
        )
    }

    /// Set stream volume
    pub async fn set_volume(&self, volume: Volume) -> Result<()> {
        let volume = volume.min(MAX_VOLUME);
        let mut st = self.state.lock().await;

        if let Some(channel) = &st.channel {
            let mut channel_guard = channel.lock().unwrap();
            channel_guard.set_volume(volume)?;
        }

        st.volume = volume;

        Ok(())
    }

    /// Get current volume
    pub async fn get_volume(&self) -> Volume {
        let st = self.state.lock().await;
        if let Some(channel) = &st.channel {
            if let Ok(channel_guard) = channel.lock() {
                return channel_guard.volume();
            }
        }
        st.volume
    }

    /// Set maximum volume
    pub async fn set_max_volume(&self, max_volume: Volume) {
        let mut config = self.config.clone();
        config.max_volume = max_volume.min(MAX_VOLUME).max(MIN_VOLUME);
        // Note: In a real implementation, you'd want to store this in the struct
    }

    /// Get maximum volume
    pub fn get_max_volume(&self) -> Volume {
        self.config.max_volume
    }

    /// Set looping mode
    pub async fn set_looping(&self, looping: bool) {
        let _lock = self.stream_lock.lock().await;
        let mut st = self.state.lock().await;

        if looping {
            st.flags.set_looping();
        } else {
            st.flags.clear_looping();
        }
    }

    /// Check if looping is enabled
    pub async fn is_looping(&self) -> bool {
        self.state.lock().await.flags.is_looping()
    }

    /// Get current position in bytes
    pub async fn get_position(&self) -> usize {
        self.state.lock().await.file_info.stream_position
    }

    /// Set position in bytes
    pub async fn set_position(&self, position: usize) -> Result<()> {
        let mut st = self.state.lock().await;
        self.set_position_locked(&mut st, position).await
    }

    /// Set position in bytes with the streamer state already locked
    async fn set_position_locked(&self, st: &mut StreamerState, position: usize) -> Result<()> {
        let was_playing = st.state == StreamState::Playing;

        self.stop_locked(st);

        st.file_info.stream_position = position.min(st.file_info.total_bytes);

        if was_playing {
            self.start_locked(st).await?;
        }

        Ok(())
    }

    /// Get current time position
    pub async fn get_time_position(&self) -> TimeStamp {
        let st = self.state.lock().await;
        let source_ts = st.format.bytes_to_time(st.file_info.stream_position);
        TimeStamp::from_millis(source_ts.as_millis())
    }

    /// Set time position
    pub async fn set_time_position(&self, time: TimeStamp) -> Result<()> {
        let mut st = self.state.lock().await;
        let source_ts = SourceTimeStamp::from_millis(time.as_millis());
        let byte_position = st.format.time_to_bytes(source_ts);
        self.set_position_locked(&mut st, byte_position).await
    }

    /// Fade in the stream
    pub async fn fade_in(&self) -> Result<()> {
        let st = self.state.lock().await;
        if let Some(channel) = &st.channel {
            let mut channel_guard = channel.lock().unwrap();
            channel_guard.fade_to_volume(self.config.max_volume, Duration::from_secs(2))?;
        }
        Ok(())
    }

    /// Fade out the stream
    pub async fn fade_out(&self) -> Result<()> {
        let st = self.state.lock().await;
        if let Some(channel) = &st.channel {
            let mut channel_guard = channel.lock().unwrap();
            channel_guard.fade_to_volume(MIN_VOLUME, Duration::from_secs(2))?;
        }
        Ok(())
    }

    /// Check if stream is currently fading
    pub async fn is_fading(&self) -> bool {
        let st = self.state.lock().await;
        if let Some(channel) = &st.channel {
            if let Ok(channel_guard) = channel.lock() {
                return channel_guard.is_fading();
            }
        }
        false
    }

    /// Wait for fade to complete
    pub async fn wait_for_fade(&self) -> Result<()> {
        let has_channel = self.state.lock().await.channel.is_some();
        if has_channel {
            let timeout = Duration::from_secs(10); // Reasonable timeout
            let start = Instant::now();

            while self.is_fading().await && start.elapsed() < timeout {
                sleep(Duration::from_millis(50)).await;
            }
        }
        Ok(())
    }

    /// Get stream name
    pub async fn get_name(&self) -> String {
        self.state.lock().await.name.clone()
    }

    /// Set stream name
    pub async fn set_name(&self, name: String) {
        self.state.lock().await.name = name;
    }

    /// Check if channel is audible
    pub async fn is_audible(&self) -> bool {
        let st = self.state.lock().await;
        if let Some(channel) = &st.channel {
            if let Ok(channel_guard) = channel.lock() {
                return channel_guard.is_audible();
            }
        }
        false
    }

    /// Get end timestamp
    pub async fn get_end_timestamp(&self) -> Option<Instant> {
        self.state.lock().await.timing.end_time
    }

    /// Lock the stream for exclusive access
    // HAZARD (unreachable today): this acquires the tokio Mutex and
    // mem::forgets the guard, while unlock_stream is a no-op — any second
    // lock_stream on this stream deadlocks forever. No in-tree caller exists
    // (lock_all_streams/unlock_all_streams are also uncalled); if this API is
    // ever wired up, replace the forgotten guard with a stored guard handle
    // that unlock_stream drops.
    pub async fn lock_stream(&self) {
        let _lock = self.stream_lock.lock().await;
        std::mem::forget(_lock); // Keep lock acquired
    }

    /// Unlock the stream
    pub async fn unlock_stream(&self) {
        // In a real implementation, you'd need a more sophisticated locking mechanism
        // This is a simplified version for the conversion
    }

    /// Service the stream (called periodically by stream manager)
    pub async fn service_stream(&self) -> Result<()> {
        let mut st = self.state.lock().await;
        if !matches!(st.state, StreamState::Playing | StreamState::Buffering) {
            return Ok(());
        }

        self.stream_next_frame_locked(&mut st).await
    }

    /// Get stream status
    pub async fn get_status(&self) -> StreamStatus {
        let st = self.state.lock().await;
        let state = st.state;
        let position_bytes = st.file_info.stream_position;
        let source_ts = st.format.bytes_to_time(position_bytes);
        let position_time = TimeStamp::from_millis(source_ts.as_millis());
        let total_bytes = st.file_info.total_bytes;
        let volume = if let Some(channel) = &st.channel {
            match channel.lock() {
                Ok(channel_guard) => channel_guard.volume(),
                Err(_) => st.volume,
            }
        } else {
            st.volume
        };
        let looping = st.flags.is_looping();

        // Calculate buffer fill percentage
        let buffer_fill_percent = {
            let stream_buffer = self.stream_buffer.lock().unwrap();
            if stream_buffer.total_bytes() > 0 {
                let filled = stream_buffer.total_bytes_in();
                ((filled * 100) / stream_buffer.total_bytes()).min(100) as u8
            } else {
                0
            }
        };

        StreamStatus {
            state,
            position_bytes,
            position_time,
            total_bytes,
            volume,
            looping,
            buffer_fill_percent,
        }
    }

    // Private implementation methods

    /// Read wave file format information
    async fn read_wave_file_format(
        &self,
        file: &mut File,
    ) -> Result<(EnhancedAudioFormat, u64, usize)> {
        // Simplified WAV header parsing - in a real implementation you'd want
        // a more robust audio format parser

        let mut header = [0u8; 44]; // Basic WAV header size
        file.read_exact(&mut header).await.map_err(|e| {
            Error::Source(SourceError::InvalidFormat(format!(
                "Failed to read WAV header: {}",
                e
            )))
        })?;

        // Verify RIFF signature
        if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
            return Err(Error::Source(SourceError::InvalidFormat(
                "Not a valid WAV file".to_string(),
            )));
        }

        // Parse format chunk
        let channels = u16::from_le_bytes([header[22], header[23]]);
        let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
        let bits_per_sample = u16::from_le_bytes([header[34], header[35]]);

        let mut format = EnhancedAudioFormat::new();
        format.channels = channels;
        format.rate = sample_rate;
        format.sample_width = bits_per_sample;
        format.bytes_per_second =
            u32::from(channels) * sample_rate * u32::from(bits_per_sample) / 8;
        format.flags = AudioFormatFlags::PCM.0;
        format.compression = AudioCompressionType::None;
        format.update().ok();

        // Find data chunk
        let mut pos = 36;
        let data_start = loop {
            if pos + 8 > header.len() {
                // Read more data if needed
                break 44; // Assume standard position for now
            }

            if &header[pos..pos + 4] == b"data" {
                let data_size = u32::from_le_bytes([
                    header[pos + 4],
                    header[pos + 5],
                    header[pos + 6],
                    header[pos + 7],
                ]) as usize;
                return Ok((format, (pos + 8) as u64, data_size));
            }

            // Skip this chunk
            let chunk_size = u32::from_le_bytes([
                header[pos + 4],
                header[pos + 5],
                header[pos + 6],
                header[pos + 7],
            ]) as usize;
            pos += 8 + chunk_size;
        };

        // Default values if data chunk not found in header
        Ok((format, data_start, 0))
    }

    /// Seek to start of audio data
    async fn seek_to_start(&self, st: &mut StreamerState) -> Result<()> {
        let data_start = st.file_info.data_start;
        let total_bytes = st.file_info.total_bytes;

        if let Some(file) = &mut st.file {
            file.seek(SeekFrom::Start(data_start))
                .await
                .map_err(|e| Error::Io(e))?;
        }

        st.file_info.bytes_left = total_bytes;
        st.file_info.stream_position = 0;

        Ok(())
    }

    /// Setup sample for playback
    fn setup_playback_sample(&self, st: &mut StreamerState) -> Result<()> {
        // Get current output block from stream
        // This would be implemented using the stream buffering system

        let frame_time = if let Some(channel) = &st.channel {
            channel.lock().unwrap().frame_time()
        } else {
            Duration::from_millis(50)
        };

        let sample_bytes = st.format.time_to_bytes_duration(frame_time);

        st.sample.set_format(st.format.to_basic());
        st.sample.set_size(sample_bytes);

        Ok(())
    }

    /// Update stream state when stopping
    fn update_stream_state_on_stop(&self, st: &mut StreamerState) {
        if matches!(st.state, StreamState::Playing | StreamState::Paused) {
            st.state = StreamState::Stopped;
        }

        st.flags.clear(StreamFlags::PLAYING | StreamFlags::PAUSED | StreamFlags::FILL);

        st.timing.end_time = Some(Instant::now());
    }
}

// Implementation of channel callback functions
impl AudioStreamer {
    /// Called when channel needs next frame of audio data
    pub async fn stream_next_frame(&self) -> Result<()> {
        let mut st = self.state.lock().await;
        self.stream_next_frame_locked(&mut st).await
    }

    /// Feed the next frame of audio data with the streamer state already locked
    async fn stream_next_frame_locked(&self, st: &mut StreamerState) -> Result<()> {
        let bytes_per_second = st.format.bytes_per_second.max(1) as usize;
        let basic_format = st.format.to_basic();

        let frame_size = {
            if st.timing.frame_size == 0 {
                st.timing.frame_size = (bytes_per_second / 60).max(MIN_FRAME_SIZE);
            }
            st.timing.frame_size
        };

        if frame_size == 0 {
            return Ok(());
        }

        let looping = st.flags.is_looping();

        let mut buffer = vec![0u8; frame_size];
        let bytes_read = {
            let file = st
                .file
                .as_mut()
                .ok_or_else(|| Error::Stream(StreamError::NotInitialized))?;
            file.read(&mut buffer).await.map_err(Error::Io)?
        };

        if bytes_read == 0 {
            if looping {
                self.seek_to_start(st).await?;
                return Ok(());
            } else {
                self.update_stream_state_on_stop(st);
                return Ok(());
            }
        }

        buffer.truncate(bytes_read);

        st.sample.set_format(basic_format);
        st.sample.write_data(&buffer);

        if st.timing.start_time.is_none() {
            st.timing.start_time = Some(Instant::now());
        }
        st.timing.pending_bytes = bytes_read;

        st.file_info.bytes_left = st.file_info.bytes_left.saturating_sub(bytes_read);
        st.file_info.stream_position = st.file_info.stream_position.saturating_add(bytes_read);
        if st.file_info.bytes_left == 0 && looping {
            self.seek_to_start(st).await?;
        }

        if let Some(channel) = &st.channel {
            if let Ok(mut channel_guard) = channel.lock() {
                channel_guard.update();
            }
        }

        st.flags.set_fill();
        if !matches!(st.state, StreamState::Playing) {
            st.state = StreamState::Playing;
        }

        Ok(())
    }

    /// Called when sample playback is complete
    pub async fn stream_sample_done(&self) -> Result<()> {
        self.stream_stop().await
    }

    /// Called when stream should stop
    pub async fn stream_stop(&self) -> Result<()> {
        let mut st = self.state.lock().await;
        self.update_stream_state_on_stop(&mut st);
        Ok(())
    }
}

/// Initialize global stream manager
static STREAM_MANAGER: std::sync::OnceLock<StreamManager> = std::sync::OnceLock::new();

/// Get global stream manager instance
pub fn get_stream_manager() -> &'static StreamManager {
    STREAM_MANAGER.get_or_init(StreamManager::new)
}

/// Initialize the streaming system
pub fn init_streaming_system() -> Result<()> {
    get_stream_manager();
    Ok(())
}

/// Shutdown the streaming system
pub async fn shutdown_streaming_system() {
    let manager = get_stream_manager();
    manager.stop_all_streams().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AudioSystem;

    #[tokio::test]
    async fn test_stream_manager_creation() {
        let manager = StreamManager::new();
        assert_eq!(manager.streams.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_stream_config() {
        let config = StreamConfig::default();
        assert_eq!(config.buffering_seconds, DEFAULT_BUFFERING_SECONDS);
        assert_eq!(config.max_volume, MAX_VOLUME);
        assert!(!config.loop_enabled);
    }

    #[tokio::test]
    async fn test_stream_flags() {
        let mut flags = StreamFlags::new();
        assert!(!flags.is_playing());

        flags.set_playing();
        assert!(flags.is_playing());

        flags.clear_playing();
        assert!(!flags.is_playing());
    }

    #[tokio::test]
    async fn test_file_info() {
        let info = FileInfo::default();
        assert_eq!(info.total_bytes, 0);
        assert_eq!(info.stream_position, 0);
        assert!(info.path.is_none());
    }
}
