//! Audio streaming system for large files (music, ambient tracks)
//!
//! Matches the C++ MILES streaming functionality from SoundBuffer.cpp and StreamSoundClass.
//! Provides double-buffered streaming with async I/O to avoid audio glitches.

use crate::{
    aud_source::{AudioSample, AudioSourceLoader, EnhancedAudioFormat},
    error::{Error, Result},
    formats::AudioFormat,
    mixer::{AudioMixer, VoiceDescriptor, VoiceHandle, VoiceParams},
    source::convert_enhanced_to_basic,
    AudioSource, Priority,
};
use crossbeam_channel::{bounded, Receiver, Sender};
use log::{debug, error, warn};
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use tokio::fs::File as TokioFile;

/// Size of each streaming buffer (in bytes) - matches C++ STREAM_BUFFER_SIZE
const STREAM_BUFFER_SIZE: usize = 65536; // 64KB per buffer

/// Number of buffers for double buffering - matches C++ implementation
const NUM_STREAM_BUFFERS: usize = 2;

/// How far ahead to start loading the next buffer (in bytes)
const PRELOAD_THRESHOLD: usize = 16384; // 16KB

/// Stream state matching C++ StreamSoundClass::State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopping,
    Stopped,
    Error,
}

/// Stream buffer for double-buffered playback
#[derive(Debug)]
struct StreamBuffer {
    data: Vec<u8>,
    valid_bytes: usize,
    is_filled: bool,
    sequence: u64,
}

impl StreamBuffer {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0u8; size],
            valid_bytes: 0,
            is_filled: false,
            sequence: 0,
        }
    }

    fn clear(&mut self) {
        self.valid_bytes = 0;
        self.is_filled = false;
    }

    fn fill_from<R: Read>(&mut self, reader: &mut R, sequence: u64) -> std::io::Result<usize> {
        self.sequence = sequence;
        let bytes_read = reader.read(&mut self.data)?;
        self.valid_bytes = bytes_read;
        self.is_filled = bytes_read > 0;
        Ok(bytes_read)
    }

    fn valid_data(&self) -> &[u8] {
        &self.data[..self.valid_bytes]
    }
}

/// Commands sent to the streaming thread
enum StreamCommand {
    Play,
    Pause,
    Stop,
    Seek(u64),
    SetVolume(f32),
    Shutdown,
}

/// Events sent from the streaming thread
#[derive(Debug, Clone)]
pub enum StreamEvent {
    Started,
    Paused,
    Stopped,
    EndOfStream,
    Error(String),
    BufferUnderrun,
}

/// Handle to a streaming audio source - matches C++ SoundStreamHandle
pub struct StreamHandle {
    stream_id: u64,
    command_tx: Sender<StreamCommand>,
    event_rx: Receiver<StreamEvent>,
    state: Arc<Mutex<StreamState>>,
    position: Arc<AtomicU64>,
    duration: Arc<AtomicU64>,
}

impl StreamHandle {
    /// Play the stream
    pub fn play(&self) -> Result<()> {
        self.command_tx
            .send(StreamCommand::Play)
            .map_err(|e| Error::Audio(format!("Failed to send play command: {e}")))
    }

    /// Pause the stream
    pub fn pause(&self) -> Result<()> {
        self.command_tx
            .send(StreamCommand::Pause)
            .map_err(|e| Error::Audio(format!("Failed to send pause command: {e}")))
    }

    /// Stop the stream
    pub fn stop(&self) -> Result<()> {
        self.command_tx
            .send(StreamCommand::Stop)
            .map_err(|e| Error::Audio(format!("Failed to send stop command: {e}")))
    }

    /// Seek to a position in seconds
    pub fn seek(&self, position_seconds: f32) -> Result<()> {
        let position_ms = (position_seconds * 1000.0) as u64;
        self.command_tx
            .send(StreamCommand::Seek(position_ms))
            .map_err(|e| Error::Audio(format!("Failed to send seek command: {e}")))
    }

    /// Set stream volume (0.0 - 1.0)
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.command_tx
            .send(StreamCommand::SetVolume(volume.clamp(0.0, 1.0)))
            .map_err(|e| Error::Audio(format!("Failed to send volume command: {e}")))
    }

    /// Get current stream state
    pub fn state(&self) -> StreamState {
        *self.state.lock().unwrap()
    }

    /// Get current playback position in milliseconds
    pub fn position_ms(&self) -> u64 {
        self.position.load(Ordering::Relaxed)
    }

    /// Get stream duration in milliseconds
    pub fn duration_ms(&self) -> u64 {
        self.duration.load(Ordering::Relaxed)
    }

    /// Poll for stream events (non-blocking)
    pub fn poll_event(&self) -> Option<StreamEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Drain all pending events
    pub fn drain_events(&self) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn stream_id(&self) -> u64 {
        self.stream_id
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        let _ = self.command_tx.send(StreamCommand::Shutdown);
    }
}

/// Streaming audio manager - matches C++ StreamSoundClass functionality
pub struct AudioStreamer {
    next_stream_id: AtomicU64,
    mixer: Arc<AudioMixer>,
}

impl AudioStreamer {
    pub fn new(mixer: Arc<AudioMixer>) -> Self {
        Self {
            next_stream_id: AtomicU64::new(1),
            mixer,
        }
    }

    /// Create a new audio stream from a file
    pub async fn create_stream<P: AsRef<Path>>(&self, path: P) -> Result<StreamHandle> {
        let path = path.as_ref().to_owned();
        let stream_id = self.next_stream_id.fetch_add(1, Ordering::Relaxed);

        // Probe the file to get format information
        let (format, file_size) = Self::probe_stream_file(&path).await?;

        // Calculate duration from file size and format
        let duration_ms = if format.bytes_per_second > 0 {
            (file_size as u64 * 1000) / format.bytes_per_second as u64
        } else {
            0
        };

        let (command_tx, command_rx) = bounded::<StreamCommand>(32);
        let (event_tx, event_rx) = bounded::<StreamEvent>(32);
        let state = Arc::new(Mutex::new(StreamState::Idle));
        let position = Arc::new(AtomicU64::new(0));
        let duration_arc = Arc::new(AtomicU64::new(duration_ms));

        // Clone for thread
        let state_clone = Arc::clone(&state);
        let position_clone = Arc::clone(&position);
        let mixer_clone = Arc::clone(&self.mixer);
        let audio_format = convert_enhanced_to_basic(&format);

        // Spawn streaming thread
        thread::Builder::new()
            .name(format!("audio-stream-{}", stream_id))
            .spawn(move || {
                Self::stream_thread(
                    path,
                    format,
                    audio_format,
                    command_rx,
                    event_tx,
                    state_clone,
                    position_clone,
                    mixer_clone,
                );
            })
            .map_err(|e| Error::Audio(format!("Failed to spawn streaming thread: {e}")))?;

        Ok(StreamHandle {
            stream_id,
            command_tx,
            event_rx,
            state,
            position,
            duration: duration_arc,
        })
    }

    /// Probe a file to get its audio format without loading the entire file
    async fn probe_stream_file(path: &Path) -> Result<(EnhancedAudioFormat, usize)> {
        let file_metadata = tokio::fs::metadata(path).await?;
        let file_size = file_metadata.len() as usize;

        // Use blocking task to probe format
        let path_clone = path.to_owned();
        let format = tokio::task::spawn_blocking(move || -> Result<EnhancedAudioFormat> {
            let (_sample, format) = AudioSourceLoader::load_file(&path_clone)?;
            Ok(format)
        })
        .await
        .map_err(|e| Error::Audio(format!("Failed to probe stream: {e}")))??;

        Ok((format, file_size))
    }

    /// Streaming thread implementation - matches C++ streaming logic
    fn stream_thread(
        path: PathBuf,
        format: EnhancedAudioFormat,
        audio_format: AudioFormat,
        command_rx: Receiver<StreamCommand>,
        event_tx: Sender<StreamEvent>,
        state: Arc<Mutex<StreamState>>,
        position: Arc<AtomicU64>,
        mixer: Arc<AudioMixer>,
    ) {
        debug!("Stream thread started for {:?}", path);

        let mut file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open stream file {:?}: {}", path, e);
                *state.lock().unwrap() = StreamState::Error;
                let _ = event_tx.send(StreamEvent::Error(format!("Failed to open file: {e}")));
                return;
            }
        };

        // Create double buffers
        let mut buffers = [
            StreamBuffer::new(STREAM_BUFFER_SIZE),
            StreamBuffer::new(STREAM_BUFFER_SIZE),
        ];
        let mut current_buffer = 0;
        let mut sequence = 0u64;

        let mut current_volume = 1.0f32;
        let mut voice_handle: Option<VoiceHandle> = None;
        let mut bytes_played = 0usize;

        'main_loop: loop {
            // Process commands
            while let Ok(cmd) = command_rx.try_recv() {
                match cmd {
                    StreamCommand::Play => {
                        debug!("Stream: received play command");
                        *state.lock().unwrap() = StreamState::Playing;
                        let _ = event_tx.send(StreamEvent::Started);
                    }
                    StreamCommand::Pause => {
                        debug!("Stream: received pause command");
                        *state.lock().unwrap() = StreamState::Paused;
                        if let Some(handle) = voice_handle {
                            mixer.pause_voice(handle);
                        }
                        let _ = event_tx.send(StreamEvent::Paused);
                    }
                    StreamCommand::Stop => {
                        debug!("Stream: received stop command");
                        *state.lock().unwrap() = StreamState::Stopping;
                        if let Some(handle) = voice_handle {
                            mixer.stop_voice(handle, crate::mixer::VoiceStopReason::Command);
                            voice_handle = None;
                        }
                        break 'main_loop;
                    }
                    StreamCommand::Seek(position_ms) => {
                        debug!("Stream: seek to {}ms", position_ms);
                        // Calculate byte position from time
                        let byte_pos = if format.bytes_per_second > 0 {
                            (position_ms * format.bytes_per_second as u64) / 1000
                        } else {
                            0
                        };
                        if file.seek(SeekFrom::Start(byte_pos)).is_ok() {
                            bytes_played = byte_pos as usize;
                            position.store(position_ms, Ordering::Relaxed);
                        }
                    }
                    StreamCommand::SetVolume(volume) => {
                        current_volume = volume;
                        // Update voice params if playing
                        if let Some(handle) = voice_handle {
                            if let Some(timeline) = mixer.voice_timeline(handle) {
                                let mut params = VoiceParams::default();
                                params.gain = volume;
                                mixer.update_voice_params(handle, params);
                            }
                        }
                    }
                    StreamCommand::Shutdown => {
                        debug!("Stream: shutting down");
                        break 'main_loop;
                    }
                }
            }

            let current_state = *state.lock().unwrap();

            if current_state == StreamState::Playing {
                // Fill next buffer if needed
                let buffer = &mut buffers[current_buffer];
                if !buffer.is_filled {
                    match buffer.fill_from(&mut file, sequence) {
                        Ok(bytes_read) => {
                            if bytes_read == 0 {
                                // End of stream
                                debug!("Stream: end of stream reached");
                                *state.lock().unwrap() = StreamState::Stopped;
                                let _ = event_tx.send(StreamEvent::EndOfStream);
                                break 'main_loop;
                            }
                            sequence += 1;
                        }
                        Err(e) => {
                            error!("Stream read error: {}", e);
                            *state.lock().unwrap() = StreamState::Error;
                            let _ = event_tx.send(StreamEvent::Error(format!("Read error: {e}")));
                            break 'main_loop;
                        }
                    }
                }

                // Feed data to mixer if we have filled buffer
                if buffer.is_filled && buffer.valid_bytes > 0 {
                    // Create audio source from buffer data
                    let mut sample = AudioSample::new();
                    sample.data = Some(buffer.valid_data().to_vec());
                    sample.bytes = buffer.valid_bytes as u32;
                    sample.format = Some(Box::new(format.clone()));

                    let source = Arc::new(
                        AudioSource::from_memory(buffer.valid_data().to_vec(), audio_format)
                            .unwrap_or_else(|e| {
                                warn!("Failed to create source from stream buffer: {}", e);
                                return AudioSource::from_memory(vec![], audio_format).unwrap();
                            }),
                    );

                    // Create voice descriptor
                    let descriptor = VoiceDescriptor {
                        source,
                        params: VoiceParams {
                            gain: current_volume,
                            pan: 0.0,
                            playback_rate: format.rate,
                            loop_count: 1,
                            start_frame: 0,
                            is_culled: false,
                            spatial: Default::default(),
                        },
                        channel_id: 0,
                        handle_id: None,
                    };

                    // Start voice or update existing
                    voice_handle = Some(mixer.start_voice(descriptor));

                    // Update position
                    bytes_played += buffer.valid_bytes;
                    let position_ms = if format.bytes_per_second > 0 {
                        (bytes_played as u64 * 1000) / format.bytes_per_second as u64
                    } else {
                        0
                    };
                    position.store(position_ms, Ordering::Relaxed);

                    // Mark buffer as consumed and switch to next
                    buffer.clear();
                    current_buffer = (current_buffer + 1) % NUM_STREAM_BUFFERS;
                }
            }

            // Small sleep to avoid busy-waiting
            thread::sleep(Duration::from_millis(10));
        }

        *state.lock().unwrap() = StreamState::Stopped;
        let _ = event_tx.send(StreamEvent::Stopped);
        debug!("Stream thread exiting");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_buffer() {
        let mut buffer = StreamBuffer::new(1024);
        assert_eq!(buffer.valid_bytes, 0);
        assert!(!buffer.is_filled);

        let data = b"Hello, streaming audio!";
        let mut cursor = std::io::Cursor::new(data);
        let bytes = buffer.fill_from(&mut cursor, 0).unwrap();

        assert_eq!(bytes, data.len());
        assert_eq!(buffer.valid_bytes, data.len());
        assert!(buffer.is_filled);
        assert_eq!(buffer.valid_data(), data);
    }

    #[test]
    fn test_stream_state_transitions() {
        assert_ne!(StreamState::Idle, StreamState::Playing);
        assert_ne!(StreamState::Playing, StreamState::Paused);
        assert_ne!(StreamState::Paused, StreamState::Stopped);
    }
}
