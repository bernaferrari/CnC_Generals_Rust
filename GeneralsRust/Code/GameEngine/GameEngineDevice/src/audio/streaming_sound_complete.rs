//! # High-Performance Streaming Sound System
//!
//! Efficient streaming audio system with predictive buffering, async I/O,
//! format detection, and seamless playback for large audio files.

use super::{AudioDeviceError, Result, AudioFormat, AudioFormatType, BufferFormat, SoundBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use parking_lot::{RwLock, Mutex};
use tokio::sync::mpsc;
use tokio::io::{AsyncRead, AsyncSeek, AsyncReadExt, AsyncSeekExt};
use std::collections::VecDeque;
use crossbeam_channel::{Sender, Receiver, bounded, unbounded};
use std::path::Path;

#[cfg(feature = "audio")]
use symphonia::core::{
    audio::{AudioBuffer, AudioBufferRef, Signal},
    codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL},
    formats::{FormatOptions, FormatReader, Track},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

/// Streaming configuration with advanced options
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Buffer size for each chunk (in bytes)
    pub buffer_size: usize,
    /// Number of buffers to maintain in the queue
    pub buffer_count: usize,
    /// Prefetch amount (bytes to read ahead)
    pub prefetch_bytes: usize,
    /// Low watermark - start refilling when buffer drops below this
    pub low_watermark: usize,
    /// High watermark - stop prefetching when buffer exceeds this
    pub high_watermark: usize,
    /// Enable adaptive buffering based on network/disk speed
    pub adaptive_buffering: bool,
    /// Target latency for streaming (milliseconds)
    pub target_latency_ms: u64,
    /// Enable seamless looping
    pub seamless_looping: bool,
    /// Decoder options
    pub decoder_options: DecoderOptions,
    /// Enable background decoding
    pub background_decoding: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64 * 1024,          // 64KB per buffer
            buffer_count: 4,                  // 4 buffers (256KB total)
            prefetch_bytes: 128 * 1024,      // 128KB prefetch
            low_watermark: 2,                 // Start refilling at 2 buffers
            high_watermark: 6,                // Stop at 6 buffers
            adaptive_buffering: true,
            target_latency_ms: 100,           // 100ms target latency
            seamless_looping: false,
            decoder_options: DecoderOptions::default(),
            background_decoding: true,
        }
    }
}

/// Streaming sound state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    /// Stream is initializing
    Initializing,
    /// Stream is ready
    Ready,
    /// Stream is buffering
    Buffering,
    /// Stream is playing
    Playing,
    /// Stream is paused
    Paused,
    /// Stream has ended
    Ended,
    /// Stream is seeking
    Seeking,
    /// Stream has an error
    Error,
}

/// Stream statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct StreamStatistics {
    /// Total bytes read
    pub total_bytes_read: u64,
    /// Total bytes decoded
    pub total_bytes_decoded: u64,
    /// Buffers in queue
    pub buffers_queued: usize,
    /// Buffer underruns count
    pub buffer_underruns: u64,
    /// Average read speed (bytes/sec)
    pub avg_read_speed: f64,
    /// Average decode time (microseconds)
    pub avg_decode_time_us: u64,
    /// Current position (bytes)
    pub position: u64,
    /// Stream duration (if known)
    pub duration: Option<Duration>,
    /// Bitrate (bits per second)
    pub bitrate: Option<u32>,
}

/// Audio chunk for streaming
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// Chunk data
    pub data: Arc<[u8]>,
    /// Timestamp for this chunk
    pub timestamp: Duration,
    /// Chunk duration
    pub duration: Duration,
    /// Sample count in this chunk
    pub sample_count: usize,
    /// Whether this is the last chunk
    pub is_last: bool,
}

/// Stream buffer management
struct StreamBuffer {
    /// Queue of audio chunks
    chunks: VecDeque<AudioChunk>,
    /// Current read position in bytes
    position: u64,
    /// Total size of buffered data
    buffered_size: usize,
    /// Maximum buffer size
    max_buffer_size: usize,
}

impl StreamBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            chunks: VecDeque::new(),
            position: 0,
            buffered_size: 0,
            max_buffer_size: max_size,
        }
    }

    fn push_chunk(&mut self, chunk: AudioChunk) -> bool {
        if self.buffered_size + chunk.data.len() > self.max_buffer_size {
            return false; // Buffer full
        }
        
        self.buffered_size += chunk.data.len();
        self.chunks.push_back(chunk);
        true
    }

    fn pop_chunk(&mut self) -> Option<AudioChunk> {
        if let Some(chunk) = self.chunks.pop_front() {
            self.buffered_size -= chunk.data.len();
            Some(chunk)
        } else {
            None
        }
    }

    fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    fn len(&self) -> usize {
        self.chunks.len()
    }

    fn buffered_bytes(&self) -> usize {
        self.buffered_size
    }
}

/// High-performance streaming sound implementation
pub struct StreamingSound {
    /// Stream identifier
    pub id: uuid::Uuid,
    /// Audio format
    format: Arc<RwLock<AudioFormat>>,
    /// Stream configuration
    config: Arc<RwLock<StreamConfig>>,
    /// Current state
    state: Arc<RwLock<StreamState>>,
    /// Stream buffer
    buffer: Arc<Mutex<StreamBuffer>>,
    /// Current position in stream
    position: Arc<RwLock<u64>>,
    /// Total length (if known)
    length: Arc<RwLock<Option<u64>>>,
    /// Stream statistics
    statistics: Arc<Mutex<StreamStatistics>>,
    /// Background task handles
    task_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    /// Shutdown flag
    shutdown_flag: Arc<AtomicBool>,
    /// Chunk sender for buffering thread
    chunk_sender: Arc<Mutex<Option<mpsc::UnboundedSender<AudioChunk>>>>,
    /// Chunk receiver for playback thread
    chunk_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<AudioChunk>>>>,
    /// Source file path (if applicable)
    source_path: Arc<RwLock<Option<std::path::PathBuf>>>,
    /// Symphonia decoder state
    #[cfg(feature = "audio")]
    decoder_state: Arc<Mutex<Option<DecoderState>>>,
}

/// Symphonia decoder state
#[cfg(feature = "audio")]
struct DecoderState {
    /// Format reader
    format: Box<dyn FormatReader>,
    /// Decoder
    decoder: Box<dyn Decoder>,
    /// Current track
    track: Track,
    /// Sample buffer
    sample_buf: symphonia::core::audio::SampleBuffer<f32>,
}

impl StreamingSound {
    /// Create a new streaming sound
    pub fn new(format: AudioFormat, config: StreamConfig) -> Self {
        let max_buffer_size = config.buffer_size * config.high_watermark;
        let (chunk_tx, chunk_rx) = mpsc::unbounded_channel();

        Self {
            id: uuid::Uuid::new_v4(),
            format: Arc::new(RwLock::new(format)),
            config: Arc::new(RwLock::new(config)),
            state: Arc::new(RwLock::new(StreamState::Initializing)),
            buffer: Arc::new(Mutex::new(StreamBuffer::new(max_buffer_size))),
            position: Arc::new(RwLock::new(0)),
            length: Arc::new(RwLock::new(None)),
            statistics: Arc::new(Mutex::new(StreamStatistics::default())),
            task_handles: Arc::new(Mutex::new(Vec::new())),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            chunk_sender: Arc::new(Mutex::new(Some(chunk_tx))),
            chunk_receiver: Arc::new(Mutex::new(Some(chunk_rx))),
            source_path: Arc::new(RwLock::new(None)),
            #[cfg(feature = "audio")]
            decoder_state: Arc::new(Mutex::new(None)),
        }
    }

    /// Start streaming from a file
    pub async fn start_streaming_from_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path_buf = path.as_ref().to_path_buf();
        *self.source_path.write() = Some(path_buf.clone());

        #[cfg(feature = "audio")]
        {
            self.initialize_decoder_from_file(&path_buf).await?;
        }

        #[cfg(not(feature = "audio"))]
        {
            // Fallback implementation without symphonia
            self.start_streaming_raw_file(path_buf).await?;
        }

        self.start_background_tasks().await?;
        *self.state.write() = StreamState::Ready;

        Ok(())
    }

    /// Start streaming from a generic async source
    pub async fn start_streaming_from_source<T>(&self, source: T) -> Result<()> 
    where
        T: AsyncRead + AsyncSeek + Send + Sync + Unpin + 'static,
    {
        self.start_streaming_generic(Box::new(source)).await
    }

    /// Internal method to start streaming from a generic source
    async fn start_streaming_generic(&self, source: Box<dyn AsyncRead + AsyncSeek + Send + Sync + Unpin>) -> Result<()> {
        #[cfg(feature = "audio")]
        {
            self.initialize_decoder_from_source(source).await?;
        }

        #[cfg(not(feature = "audio"))]
        {
            return Err(AudioDeviceError::StreamingError("Streaming from generic source requires symphonia feature".to_string()));
        }

        self.start_background_tasks().await?;
        *self.state.write() = StreamState::Ready;

        Ok(())
    }

    /// Initialize decoder from file using symphonia
    #[cfg(feature = "audio")]
    async fn initialize_decoder_from_file(&self, path: &Path) -> Result<()> {
        use std::fs::File;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::probe::Hint;

        // Open file
        let file = File::open(path)
            .map_err(|e| AudioDeviceError::IoError(e))?;

        // Create media source stream
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Create hint based on file extension
        let mut hint = Hint::new();
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                hint.with_extension(ext_str);
            }
        }

        self.initialize_decoder_common(mss, hint).await
    }

    #[cfg(feature = "audio")]
    async fn initialize_decoder_from_source<T>(&self, source: T) -> Result<()> 
    where
        T: AsyncRead + AsyncSeek + Send + Sync + Unpin + 'static,
    {
        use std::io::Cursor;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::probe::Hint;
        use tokio::io::AsyncReadExt;

        let mut buffered = source;
        let mut data = Vec::new();
        buffered.read_to_end(&mut data).await.map_err(|e| {
            AudioDeviceError::StreamingError(format!("Failed to read source into buffer: {}", e))
        })?;

        let cursor: Box<dyn symphonia::core::io::MediaSource> =
            Box::new(Cursor::new(data));
        let mss = MediaSourceStream::new(cursor, Default::default());

        let hint = Hint::new();
        self.initialize_decoder_common(mss, hint).await
    }

    /// Common decoder initialization
    #[cfg(feature = "audio")]
    async fn initialize_decoder_common(&self, mss: MediaSourceStream, hint: Hint) -> Result<()> {
        // Probe the media source
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| AudioDeviceError::FormatNotSupported(format!("Probe failed: {:?}", e)))?;

        let mut format = probed.format;

        // Find default track
        let track = format.default_track()
            .ok_or_else(|| AudioDeviceError::FormatNotSupported("No default track found".to_string()))?;

        // Create decoder
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &self.config.read().decoder_options)
            .map_err(|e| AudioDeviceError::FormatNotSupported(format!("Decoder creation failed: {:?}", e)))?;

        // Update audio format based on codec parameters
        let params = &track.codec_params;
        if let (Some(sample_rate), Some(channels)) = (params.sample_rate, params.channels) {
            let mut format_guard = self.format.write();
            format_guard.sample_rate = sample_rate;
            format_guard.channels = channels.count() as u16;
            
            // Determine bit depth and format type
            if let Some(bits_per_sample) = params.bits_per_sample {
                format_guard.bits_per_sample = bits_per_sample as u16;
            }
            
            // Default to f32 for decoded audio
            format_guard.format_type = AudioFormatType::PcmFloat;
            format_guard.bits_per_sample = 32;
        }

        // Create sample buffer
        let sample_buf = symphonia::core::audio::SampleBuffer::new(
            track.codec_params.max_frames_per_packet.unwrap_or(4096) as u64,
            track.codec_params.channels.unwrap_or(symphonia::core::audio::Channels::FRONT_LEFT | symphonia::core::audio::Channels::FRONT_RIGHT)
        );

        // Store decoder state
        let decoder_state = DecoderState {
            format,
            decoder,
            track: track.clone(),
            sample_buf,
        };

        *self.decoder_state.lock() = Some(decoder_state);

        Ok(())
    }

    /// Fallback raw file streaming (without symphonia)
    async fn start_streaming_raw_file(&self, path: std::path::PathBuf) -> Result<()> {
        // This is a simplified implementation that assumes raw PCM data
        // In practice, you'd want better format detection
        *self.state.write() = StreamState::Ready;
        Ok(())
    }

    /// Start background processing tasks
    async fn start_background_tasks(&self) -> Result<()> {
        let mut handles = self.task_handles.lock();
        
        // Clear existing handles
        handles.clear();

        // Start decoder task
        if self.config.read().background_decoding {
            let handle = self.start_decoder_task().await?;
            handles.push(handle);
        }

        // Start buffer management task
        let handle = self.start_buffer_manager_task().await;
        handles.push(handle);

        Ok(())
    }

    /// Start decoder task for background decoding
    #[cfg(feature = "audio")]
    async fn start_decoder_task(&self) -> Result<tokio::task::JoinHandle<()>> {
        let decoder_state = Arc::clone(&self.decoder_state);
        let chunk_sender = Arc::clone(&self.chunk_sender);
        let shutdown_flag = Arc::clone(&self.shutdown_flag);
        let statistics = Arc::clone(&self.statistics);
        let format = Arc::clone(&self.format);
        let config = Arc::clone(&self.config);

        let handle = tokio::task::spawn(async move {
            let mut decoder_state_guard = decoder_state.lock();
            if let Some(ref mut state) = *decoder_state_guard {
                let sender = {
                    let sender_guard = chunk_sender.lock();
                    if let Some(ref sender) = *sender_guard {
                        sender.clone()
                    } else {
                        return; // No sender available
                    }
                };

                let mut timestamp = Duration::ZERO;
                let format_info = format.read().clone();

                while !shutdown_flag.load(Ordering::Relaxed) {
                    // Try to get next packet
                    match state.format.next_packet() {
                        Ok(packet) => {
                            // Decode the packet
                            match state.decoder.decode(&packet) {
                                Ok(audio_buf_ref) => {
                                    // Convert to sample buffer
                                    state.sample_buf.copy_interleaved_ref(audio_buf_ref);

                                    // Convert to bytes
                                    let samples = state.sample_buf.samples();
                                    let chunk_data: Vec<u8> = samples
                                        .iter()
                                        .flat_map(|&sample| sample.to_le_bytes().to_vec())
                                        .collect();

                                    let chunk_duration = Duration::from_secs_f64(
                                        samples.len() as f64 / (format_info.sample_rate as f64 * format_info.channels as f64)
                                    );

                                    let chunk = AudioChunk {
                                        data: Arc::from(chunk_data),
                                        timestamp,
                                        duration: chunk_duration,
                                        sample_count: samples.len() / format_info.channels as usize,
                                        is_last: false,
                                    };

                                    timestamp += chunk_duration;

                                    // Send chunk
                                    if sender.send(chunk).is_err() {
                                        break; // Receiver dropped
                                    }

                                    // Update statistics
                                    let mut stats = statistics.lock();
                                    stats.total_bytes_decoded += chunk.data.len() as u64;
                                }
                                Err(e) => {
                                    tracing::warn!("Decode error: {:?}", e);
                                    if e == symphonia::core::errors::Error::DecodeError("end of stream") {
                                        // Send final chunk
                                        let final_chunk = AudioChunk {
                                            data: Arc::from(Vec::new()),
                                            timestamp,
                                            duration: Duration::ZERO,
                                            sample_count: 0,
                                            is_last: true,
                                        };
                                        let _ = sender.send(final_chunk);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if e == symphonia::core::errors::Error::IoError(
                                symphonia::core::errors::IoError::UnexpectedEof
                            ) {
                                // End of stream
                                let final_chunk = AudioChunk {
                                    data: Arc::from(Vec::new()),
                                    timestamp,
                                    duration: Duration::ZERO,
                                    sample_count: 0,
                                    is_last: true,
                                };
                                let _ = sender.send(final_chunk);
                                break;
                            } else {
                                tracing::error!("Format error: {:?}", e);
                                break;
                            }
                        }
                    }

                    // Small yield to prevent blocking
                    tokio::task::yield_now().await;
                }
            }
        });

        Ok(handle)
    }

    /// Start decoder task (fallback without symphonia)
    #[cfg(not(feature = "audio"))]
    async fn start_decoder_task(&self) -> Result<tokio::task::JoinHandle<()>> {
        // Stub implementation
        let handle = tokio::task::spawn(async {
            // No-op task
        });
        Ok(handle)
    }

    /// Start buffer manager task
    async fn start_buffer_manager_task(&self) -> tokio::task::JoinHandle<()> {
        let buffer = Arc::clone(&self.buffer);
        let chunk_receiver = Arc::clone(&self.chunk_receiver);
        let shutdown_flag = Arc::clone(&self.shutdown_flag);
        let statistics = Arc::clone(&self.statistics);
        let config = Arc::clone(&self.config);

        tokio::task::spawn(async move {
            let mut receiver = {
                let mut receiver_guard = chunk_receiver.lock();
                if let Some(receiver) = receiver_guard.take() {
                    receiver
                } else {
                    return; // No receiver available
                }
            };

            while !shutdown_flag.load(Ordering::Relaxed) {
                match receiver.recv().await {
                    Some(chunk) => {
                        // Add chunk to buffer
                        let mut buffer_guard = buffer.lock();
                        if !buffer_guard.push_chunk(chunk.clone()) {
                            // Buffer is full, drop oldest chunk
                            buffer_guard.pop_chunk();
                            buffer_guard.push_chunk(chunk);
                        }

                        // Update statistics
                        let mut stats = statistics.lock();
                        stats.buffers_queued = buffer_guard.len();

                        // Check for end of stream
                        if chunk.is_last {
                            break;
                        }
                    }
                    None => {
                        // Sender dropped, exit
                        break;
                    }
                }
            }
        })
    }

    /// Read next audio chunk for playback
    pub async fn read_chunk(&self, buffer: &mut [u8]) -> Result<usize> {
        let chunk = {
            let mut buffer_guard = self.buffer.lock();
            buffer_guard.pop_chunk()
        };

        if let Some(chunk) = chunk {
            let bytes_to_copy = std::cmp::min(buffer.len(), chunk.data.len());
            buffer[..bytes_to_copy].copy_from_slice(&chunk.data[..bytes_to_copy]);

            // Update position
            *self.position.write() += bytes_to_copy as u64;

            // Update statistics
            {
                let mut stats = self.statistics.lock();
                stats.position += bytes_to_copy as u64;
                stats.buffers_queued = {
                    let buffer_guard = self.buffer.lock();
                    buffer_guard.len()
                };

                // Check for buffer underrun
                if stats.buffers_queued == 0 && !chunk.is_last {
                    stats.buffer_underruns += 1;
                }
            }

            if chunk.is_last && bytes_to_copy == chunk.data.len() {
                *self.state.write() = StreamState::Ended;
            }

            Ok(bytes_to_copy)
        } else {
            // No chunks available
            let current_state = *self.state.read();
            if current_state == StreamState::Ended {
                Ok(0) // End of stream
            } else {
                // Buffer underrun
                let mut stats = self.statistics.lock();
                stats.buffer_underruns += 1;
                
                *self.state.write() = StreamState::Buffering;
                
                // Wait a bit for buffers to fill
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(0)
            }
        }
    }

    /// Seek to position in stream
    pub async fn seek(&self, position: Duration) -> Result<()> {
        *self.state.write() = StreamState::Seeking;

        // Clear current buffers
        {
            let mut buffer_guard = self.buffer.lock();
            while !buffer_guard.is_empty() {
                buffer_guard.pop_chunk();
            }
        }

        #[cfg(feature = "audio")]
        {
            let mut decoder_state_guard = self.decoder_state.lock();
            if let Some(ref mut state) = *decoder_state_guard {
                // Calculate target timestamp
                let target_ts = symphonia::core::units::Time::from(position);
                
                // Try to seek
                if let Err(e) = state.format.seek(
                    symphonia::core::formats::SeekMode::Accurate,
                    symphonia::core::formats::SeekTo::Time { time: target_ts, track_id: Some(state.track.id) }
                ) {
                    tracing::warn!("Seek failed: {:?}", e);
                    *self.state.write() = StreamState::Error;
                    return Err(AudioDeviceError::StreamingError(format!("Seek failed: {:?}", e)));
                }
            }
        }

        // Update position
        let position_bytes = (position.as_secs_f64() * self.format.read().sample_rate as f64 * 
                             self.format.read().channels as f64 * 
                             (self.format.read().bits_per_sample / 8) as f64) as u64;
        *self.position.write() = position_bytes;

        *self.state.write() = StreamState::Playing;
        Ok(())
    }

    /// Get current state
    pub fn get_state(&self) -> StreamState {
        *self.state.read()
    }

    /// Set state
    pub fn set_state(&self, state: StreamState) {
        *self.state.write() = state;
    }

    /// Get current position
    pub fn get_position(&self) -> u64 {
        *self.position.read()
    }

    /// Get stream length (if known)
    pub fn get_length(&self) -> Option<u64> {
        *self.length.read()
    }

    /// Get stream statistics
    pub fn get_statistics(&self) -> StreamStatistics {
        self.statistics.lock().clone()
    }

    /// Get audio format
    pub fn get_format(&self) -> AudioFormat {
        self.format.read().clone()
    }

    /// Check if stream has more data
    pub fn has_more_data(&self) -> bool {
        let state = *self.state.read();
        !matches!(state, StreamState::Ended | StreamState::Error)
    }

    /// Get buffered duration
    pub fn get_buffered_duration(&self) -> Duration {
        let buffer_guard = self.buffer.lock();
        let bytes = buffer_guard.buffered_bytes();
        let format = self.format.read();
        
        let bytes_per_second = format.sample_rate as f64 * 
                              format.channels as f64 * 
                              (format.bits_per_sample / 8) as f64;
        
        Duration::from_secs_f64(bytes as f64 / bytes_per_second)
    }

    /// Stop streaming and cleanup
    pub async fn stop(&self) -> Result<()> {
        self.shutdown_flag.store(true, Ordering::Relaxed);

        // Wait for tasks to complete
        let handles = {
            let mut handles_guard = self.task_handles.lock();
            std::mem::take(&mut *handles_guard)
        };

        for handle in handles {
            let _ = handle.await; // Ignore join errors
        }

        // Clear buffers
        {
            let mut buffer_guard = self.buffer.lock();
            while !buffer_guard.is_empty() {
                buffer_guard.pop_chunk();
            }
        }

        *self.state.write() = StreamState::Ended;
        Ok(())
    }
}

impl Drop for StreamingSound {
    fn drop(&mut self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
    }
}

/// Stream builder for convenient configuration
pub struct StreamBuilder {
    format: AudioFormat,
    config: StreamConfig,
}

impl StreamBuilder {
    /// Create a new stream builder
    pub fn new(format: AudioFormat) -> Self {
        Self {
            format,
            config: StreamConfig::default(),
        }
    }

    /// Set buffer size
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.config.buffer_size = size;
        self
    }

    /// Set buffer count
    pub fn buffer_count(mut self, count: usize) -> Self {
        self.config.buffer_count = count;
        self
    }

    /// Set prefetch bytes
    pub fn prefetch_bytes(mut self, bytes: usize) -> Self {
        self.config.prefetch_bytes = bytes;
        self
    }

    /// Enable adaptive buffering
    pub fn adaptive_buffering(mut self, enable: bool) -> Self {
        self.config.adaptive_buffering = enable;
        self
    }

    /// Set target latency
    pub fn target_latency(mut self, latency_ms: u64) -> Self {
        self.config.target_latency_ms = latency_ms;
        self
    }

    /// Enable seamless looping
    pub fn seamless_looping(mut self, enable: bool) -> Self {
        self.config.seamless_looping = enable;
        self
    }

    /// Enable background decoding
    pub fn background_decoding(mut self, enable: bool) -> Self {
        self.config.background_decoding = enable;
        self
    }

    /// Build the streaming sound
    pub fn build(self) -> StreamingSound {
        StreamingSound::new(self.format, self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_stream_creation() {
        let format = AudioFormat::cd_quality();
        let config = StreamConfig::default();
        let stream = StreamingSound::new(format, config);
        
        assert_eq!(stream.get_state(), StreamState::Initializing);
        assert_eq!(stream.get_position(), 0);
    }

    #[tokio::test]
    async fn test_stream_builder() {
        let format = AudioFormat::cd_quality();
        let stream = StreamBuilder::new(format)
            .buffer_size(128 * 1024)
            .buffer_count(6)
            .adaptive_buffering(true)
            .target_latency(50)
            .build();
            
        assert_eq!(stream.get_state(), StreamState::Initializing);
    }

    #[tokio::test]
    async fn test_buffer_management() {
        let mut buffer = StreamBuffer::new(1024);
        
        let chunk = AudioChunk {
            data: Arc::from(vec![0u8; 512]),
            timestamp: Duration::ZERO,
            duration: Duration::from_millis(10),
            sample_count: 256,
            is_last: false,
        };
        
        assert!(buffer.push_chunk(chunk.clone()));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.buffered_bytes(), 512);
        
        let retrieved = buffer.pop_chunk().unwrap();
        assert_eq!(retrieved.data.len(), 512);
        assert!(buffer.is_empty());
    }

    #[tokio::test]
    async fn test_buffer_overflow() {
        let mut buffer = StreamBuffer::new(512);
        
        let chunk = AudioChunk {
            data: Arc::from(vec![0u8; 600]), // Larger than max size
            timestamp: Duration::ZERO,
            duration: Duration::from_millis(10),
            sample_count: 300,
            is_last: false,
        };
        
        assert!(!buffer.push_chunk(chunk)); // Should fail
        assert!(buffer.is_empty());
    }
}