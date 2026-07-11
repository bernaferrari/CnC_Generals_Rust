//! SimplePlayer - Audio file playback implementation
//! Converted from Windows Media Format SDK to cross-platform Rust
//! Original Windows-specific implementation converted to use modern audio libraries

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

use super::engine::{AudioEngine, AudioEngineConfig};
use super::AudioHandle;
pub type HResult = i32;
pub type Bool = bool;

// Audio format information
#[derive(Debug, Clone)]
pub struct WaveFormat {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub byte_rate: u32,
    pub block_align: u16,
}

impl Default for WaveFormat {
    fn default() -> Self {
        WaveFormat {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            byte_rate: 176400, // sample_rate * channels * bits_per_sample / 8
            block_align: 4,    // channels * bits_per_sample / 8
        }
    }
}

/// Audio buffer for storing decoded audio samples
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    pub data: Vec<u8>,
    pub length: usize,
    pub timestamp: Duration,
    pub is_done: bool,
}

impl AudioBuffer {
    pub fn new(capacity: usize) -> Self {
        AudioBuffer {
            data: vec![0u8; capacity],
            length: 0,
            timestamp: Duration::ZERO,
            is_done: false,
        }
    }

    pub fn with_data(data: Vec<u8>, timestamp: Duration) -> Self {
        let length = data.len();
        AudioBuffer {
            data,
            length,
            timestamp,
            is_done: false,
        }
    }
}

/// Events for communication between audio thread and main thread
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerEvent {
    Opened,
    Started,
    Stopped,
    EndOfFile,
    Error(HResult),
    BufferingStart,
    BufferingStop,
}

/// Status of the simple player
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlayerStatus {
    Idle,
    Opening,
    Opened,
    Playing,
    Paused,
    Stopped,
    Error,
}

/// Simple audio player for streaming media files
/// Replaces the Windows Media Format SDK functionality with cross-platform audio
pub struct SimplePlayer {
    ref_count: AtomicUsize,

    status: Arc<Mutex<PlayerStatus>>,
    url: Arc<Mutex<Option<PathBuf>>>,
    format: Arc<Mutex<WaveFormat>>,

    buffers_outstanding: Arc<AtomicUsize>,
    audio_buffers: Arc<Mutex<VecDeque<AudioBuffer>>>,

    completion_handler: Arc<Mutex<Option<Box<dyn Fn(HResult) + Send + Sync>>>>,
    event_queue: Arc<Mutex<VecDeque<PlayerEvent>>>,

    playback_thread: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    should_stop: Arc<AtomicBool>,

    audio_engine: AudioEngine,
    current_handle: Arc<Mutex<Option<AudioHandle>>>,

    last_error: Arc<Mutex<HResult>>,
}

// Constants for HRESULT values (simplified)
const S_OK: HResult = 0;
const E_FAIL: HResult = -1;
const E_OUTOFMEMORY: HResult = -2;
const E_INVALIDARG: HResult = -3;
const E_UNEXPECTED: HResult = -4;

impl SimplePlayer {
    /// Create a new SimplePlayer instance
    pub fn new() -> Result<Self, HResult> {
        let engine = AudioEngine::with_config(AudioEngineConfig::default()).map_err(|_| E_FAIL)?;

        Ok(SimplePlayer {
            ref_count: AtomicUsize::new(1),
            status: Arc::new(Mutex::new(PlayerStatus::Idle)),
            url: Arc::new(Mutex::new(None)),
            format: Arc::new(Mutex::new(WaveFormat::default())),
            buffers_outstanding: Arc::new(AtomicUsize::new(0)),
            audio_buffers: Arc::new(Mutex::new(VecDeque::new())),
            completion_handler: Arc::new(Mutex::new(None)),
            event_queue: Arc::new(Mutex::new(VecDeque::new())),
            playback_thread: Arc::new(Mutex::new(None)),
            should_stop: Arc::new(AtomicBool::new(false)),
            audio_engine: engine,
            current_handle: Arc::new(Mutex::new(None)),
            last_error: Arc::new(Mutex::new(S_OK)),
        })
    }

    /// Play an audio file
    pub fn play(
        &mut self,
        url: &str,
        duration_seconds: u32,
        completion_callback: Option<Box<dyn Fn(HResult) + Send + Sync>>,
    ) -> HResult {
        // Validate input
        if url.is_empty() {
            return E_INVALIDARG;
        }

        // Convert URL to path
        let file_path = if !url.contains("://") && !url.starts_with("\\\\") && !url.contains(":\\")
        {
            // Relative path - convert to absolute
            match std::env::current_dir() {
                Ok(current_dir) => current_dir.join(url),
                Err(_) => return E_FAIL,
            }
        } else {
            PathBuf::from(url)
        };

        // Store the URL
        {
            let mut url_guard = self.url.lock().unwrap();
            *url_guard = Some(file_path.clone());
        }

        // Store completion handler
        {
            let mut handler_guard = self.completion_handler.lock().unwrap();
            *handler_guard = completion_callback;
        }

        // Reset state
        self.should_stop.store(false, Ordering::Relaxed);
        self.buffers_outstanding.store(0, Ordering::Relaxed);

        {
            let mut status_guard = self.status.lock().unwrap();
            *status_guard = PlayerStatus::Opening;
        }

        // Start playback thread
        self.start_playback_thread(file_path, Duration::from_secs(duration_seconds as u64))
    }

    /// Stop playback and close the player
    pub fn close(&mut self) -> HResult {
        self.should_stop.store(true, Ordering::Relaxed);

        if let Some(handle) = self.current_handle.lock().unwrap().take() {
            let _ = self.audio_engine.stop_source(handle);
        }

        if let Some(handle) = self.playback_thread.lock().unwrap().take() {
            let _ = handle.join();
        }

        let _ = self.audio_engine.stop();

        {
            let mut status_guard = self.status.lock().unwrap();
            *status_guard = PlayerStatus::Stopped;
        }

        S_OK
    }

    /// Add reference (COM-style reference counting)
    pub fn add_ref(&self) -> usize {
        self.ref_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Release reference (COM-style reference counting)
    pub fn release(&self) -> usize {
        let prev_count = self.ref_count.fetch_sub(1, Ordering::Relaxed);
        prev_count - 1
    }

    /// Check if there are any events to process
    pub fn has_events(&self) -> bool {
        !self.event_queue.lock().unwrap().is_empty()
    }

    /// Get the next event from the queue
    pub fn get_next_event(&self) -> Option<PlayerEvent> {
        self.event_queue.lock().unwrap().pop_front()
    }

    /// Get current playback status
    pub fn get_status(&self) -> PlayerStatus {
        *self.status.lock().unwrap()
    }

    /// Get last error
    pub fn get_last_error(&self) -> HResult {
        *self.last_error.lock().unwrap()
    }

    /// Start the playback thread
    fn start_playback_thread(&mut self, file_path: PathBuf, duration: Duration) -> HResult {
        let handle = match self
            .audio_engine
            .play(&file_path.to_string_lossy(), 1.0, false, None)
        {
            Ok(h) => h,
            Err(_) => return E_FAIL,
        };

        *self.current_handle.lock().unwrap() = Some(handle);

        {
            let mut status_guard = self.status.lock().unwrap();
            *status_guard = PlayerStatus::Playing;
        }

        {
            let mut queue_guard = self.event_queue.lock().unwrap();
            queue_guard.push_back(PlayerEvent::Opened);
            queue_guard.push_back(PlayerEvent::Started);
        }

        let status = Arc::clone(&self.status);
        let should_stop = Arc::clone(&self.should_stop);
        let event_queue = Arc::clone(&self.event_queue);
        let current_handle = Arc::clone(&self.current_handle);
        let completion_handler = Arc::clone(&self.completion_handler);
        let last_error = Arc::clone(&self.last_error);
        let buffers_outstanding = Arc::clone(&self.buffers_outstanding);

        let thread_handle = thread::spawn(move || {
            let result = S_OK;
            let start_time = Instant::now();
            let duration_limit = if duration.as_secs() > 0 {
                Some(duration)
            } else {
                None
            };

            buffers_outstanding.store(1, Ordering::Relaxed);

            loop {
                if should_stop.load(Ordering::Relaxed) {
                    break;
                }

                if let Some(limit) = duration_limit {
                    if start_time.elapsed() >= limit {
                        break;
                    }
                }

                thread::sleep(Duration::from_millis(20));
            }

            if should_stop.load(Ordering::Relaxed) {
                {
                    let mut status_guard = status.lock().unwrap();
                    *status_guard = PlayerStatus::Stopped;
                }

                let mut queue_guard = event_queue.lock().unwrap();
                queue_guard.push_back(PlayerEvent::Stopped);
            } else {
                {
                    let mut status_guard = status.lock().unwrap();
                    *status_guard = PlayerStatus::Stopped;
                }

                let mut queue_guard = event_queue.lock().unwrap();
                queue_guard.push_back(PlayerEvent::EndOfFile);
            }

            *current_handle.lock().unwrap() = None;

            if let Some(handler) = completion_handler.lock().unwrap().as_ref() {
                handler(result);
            }

            buffers_outstanding.store(0, Ordering::Relaxed);
        });

        {
            let mut thread_guard = self.playback_thread.lock().unwrap();
            *thread_guard = Some(thread_handle);
        }

        S_OK
    }

    /// Process any pending events (should be called regularly by the main thread)
    pub fn process_events(&self) {
        while let Some(event) = self.get_next_event() {
            match event {
                PlayerEvent::Opened => {
                    println!("Audio file opened successfully");
                }
                PlayerEvent::Started => {
                    println!("Audio playback started");
                }
                PlayerEvent::Stopped => {
                    println!("Audio playback stopped");
                }
                PlayerEvent::EndOfFile => {
                    println!("End of audio file reached");
                }
                PlayerEvent::Error(hr) => {
                    println!("Audio playback error: {}", hr);
                }
                PlayerEvent::BufferingStart => {
                    println!("Audio buffering started");
                }
                PlayerEvent::BufferingStop => {
                    println!("Audio buffering stopped");
                }
            }
        }
    }

    /// Get the number of buffers currently being processed
    pub fn get_buffers_outstanding(&self) -> usize {
        self.buffers_outstanding.load(Ordering::Relaxed)
    }

    /// Check if the player is currently playing
    pub fn is_playing(&self) -> bool {
        matches!(self.get_status(), PlayerStatus::Playing)
    }

    /// Pause playback (simplified implementation)
    pub fn pause(&self) -> HResult {
        let mut status_guard = self.status.lock().unwrap();
        if *status_guard != PlayerStatus::Playing {
            return E_FAIL;
        }

        if let Some(handle) = self.current_handle.lock().unwrap().as_ref().copied() {
            if self.audio_engine.pause(handle).is_ok() {
                *status_guard = PlayerStatus::Paused;
                return S_OK;
            }
        }

        E_FAIL
    }

    pub fn resume(&self) -> HResult {
        let mut status_guard = self.status.lock().unwrap();
        if *status_guard != PlayerStatus::Paused {
            return E_FAIL;
        }

        if let Some(handle) = self.current_handle.lock().unwrap().as_ref().copied() {
            if self.audio_engine.resume(handle).is_ok() {
                *status_guard = PlayerStatus::Playing;
                return S_OK;
            }
        }

        E_FAIL
    }
}

impl Default for SimplePlayer {
    fn default() -> Self {
        Self::new().expect("Failed to create SimplePlayer")
    }
}

impl Drop for SimplePlayer {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

// Utility functions to match the original C++ interface

/// Create a new simple player instance
pub fn create_simple_player() -> Result<SimplePlayer, HResult> {
    SimplePlayer::new()
}

/// Play an audio file with a simple interface
pub fn play_audio_file(
    file_path: &str,
    duration_seconds: u32,
    completion_callback: Option<Box<dyn Fn(HResult) + Send + Sync>>,
) -> Result<SimplePlayer, HResult> {
    let mut player = SimplePlayer::new()?;
    let result = player.play(file_path, duration_seconds, completion_callback);

    if result == S_OK {
        Ok(player)
    } else {
        Err(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_simple_player_creation() {
        let player = SimplePlayer::new();
        assert!(player.is_ok());

        let player = player.unwrap();
        assert_eq!(player.get_status(), PlayerStatus::Idle);
        assert_eq!(player.ref_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_reference_counting() {
        let player = SimplePlayer::new().unwrap();
        assert_eq!(player.ref_count.load(Ordering::Relaxed), 1);

        let count = player.add_ref();
        assert_eq!(count, 2);

        let count = player.release();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_status_transitions() {
        let mut player = SimplePlayer::new().unwrap();
        assert_eq!(player.get_status(), PlayerStatus::Idle);

        // Note: actual file playback testing would require test audio files
        // For now, we just test status management
        {
            let mut status_guard = player.status.lock().unwrap();
            *status_guard = PlayerStatus::Playing;
        }

        assert_eq!(player.get_status(), PlayerStatus::Playing);
        assert!(player.is_playing());
    }

    #[test]
    fn test_event_queue() {
        let player = SimplePlayer::new().unwrap();
        assert!(!player.has_events());

        {
            let mut queue_guard = player.event_queue.lock().unwrap();
            queue_guard.push_back(PlayerEvent::Opened);
            queue_guard.push_back(PlayerEvent::Started);
        }

        assert!(player.has_events());

        let event1 = player.get_next_event();
        assert_eq!(event1, Some(PlayerEvent::Opened));

        let event2 = player.get_next_event();
        assert_eq!(event2, Some(PlayerEvent::Started));

        let event3 = player.get_next_event();
        assert_eq!(event3, None);

        assert!(!player.has_events());
    }

    #[test]
    fn test_wave_format() {
        let format = WaveFormat::default();
        assert_eq!(format.channels, 2);
        assert_eq!(format.sample_rate, 44100);
        assert_eq!(format.bits_per_sample, 16);
        assert_eq!(format.byte_rate, 176400);
        assert_eq!(format.block_align, 4);
    }

    #[test]
    fn test_audio_buffer() {
        let buffer = AudioBuffer::new(1024);
        assert_eq!(buffer.data.len(), 1024);
        assert_eq!(buffer.length, 0);
        assert!(!buffer.is_done);

        let data = vec![1, 2, 3, 4, 5];
        let buffer2 = AudioBuffer::with_data(data.clone(), Duration::from_millis(100));
        assert_eq!(buffer2.data, data);
        assert_eq!(buffer2.length, 5);
        assert_eq!(buffer2.timestamp, Duration::from_millis(100));
    }
}
