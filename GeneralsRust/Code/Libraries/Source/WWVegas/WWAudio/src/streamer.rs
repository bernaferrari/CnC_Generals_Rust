//! Audio streaming system for large files and real-time playback.

use crate::{
    error::{Result, StreamError},
    formats::AudioFormat,
};
use std::path::Path;
use tokio::fs;

/// Stream configuration
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub buffer_size: usize,
    pub buffer_count: usize,
    pub format: AudioFormat,
    pub loop_stream: bool,
}

/// Stream state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Stopped,
    Buffering,
    Playing,
    Paused,
    EndOfStream,
}

/// Audio streamer for large files
pub struct AudioStreamer {
    config: StreamConfig,
    state: StreamState,
    buffer: Vec<u8>,
    position_ms: u64,
    duration_ms: u64,
}

impl AudioStreamer {
    /// Create new audio streamer
    pub fn new(config: StreamConfig) -> Self {
        Self {
            config,
            state: StreamState::Stopped,
            buffer: Vec::new(),
            position_ms: 0,
            duration_ms: 0,
        }
    }

    /// Load stream from file
    pub async fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let data = fs::read(path).await?;
        let bytes_per_second = self.config.format.bytes_per_second().max(1);
        let duration_ms = (data.len() as u64).saturating_mul(1_000) / u64::from(bytes_per_second);

        self.buffer = data;
        self.duration_ms = duration_ms;
        self.position_ms = 0;
        self.state = StreamState::Buffering;

        Ok(())
    }

    /// Start streaming
    pub async fn start(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Err(StreamError::NotInitialized.into());
        }

        if self.state == StreamState::EndOfStream {
            if self.config.loop_stream {
                self.position_ms = 0;
            } else {
                return Err(StreamError::EndOfStream.into());
            }
        }

        self.state = StreamState::Playing;
        Ok(())
    }

    /// Pause streaming
    pub async fn pause(&mut self) -> Result<()> {
        if self.state != StreamState::Playing {
            return Err(StreamError::OperationFailed("Stream is not playing".to_string()).into());
        }

        self.state = StreamState::Paused;
        Ok(())
    }

    /// Stop streaming
    pub async fn stop(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Err(StreamError::NotInitialized.into());
        }

        self.position_ms = 0;
        self.state = StreamState::Stopped;
        Ok(())
    }

    /// Get current state
    pub fn state(&self) -> StreamState {
        self.state
    }

    /// Seek to position (in milliseconds)
    pub async fn seek(&mut self, position_ms: u64) -> Result<()> {
        if self.buffer.is_empty() {
            return Err(StreamError::NotInitialized.into());
        }

        self.position_ms = position_ms.min(self.duration_ms);

        if self.position_ms >= self.duration_ms && !self.config.loop_stream {
            self.state = StreamState::EndOfStream;
        } else if self.state == StreamState::Paused {
            self.state = StreamState::Buffering;
        }

        Ok(())
    }
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 4096,
            buffer_count: 4,
            format: AudioFormat::default(),
            loop_stream: false,
        }
    }
}
