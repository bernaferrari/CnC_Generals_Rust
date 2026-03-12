//! # Streaming Sound System
//!
//! Efficient streaming audio for large files with predictive buffering.

use super::{AudioDeviceError, AudioFormat, Result};
use tokio::io::{AsyncRead, AsyncSeek};

/// Object-safe trait alias for async stream sources.
trait AsyncReadSeek: AsyncRead + AsyncSeek {}

impl<T> AsyncReadSeek for T where T: AsyncRead + AsyncSeek + ?Sized {}

/// Streaming configuration
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Buffer size for streaming
    pub buffer_size: usize,
    /// Number of buffers to maintain
    pub buffer_count: usize,
    /// Prefetch amount
    pub prefetch_bytes: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64 * 1024,
            buffer_count: 3,
            prefetch_bytes: 128 * 1024,
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
    /// Stream has an error
    Error,
}

/// Streaming sound implementation
pub struct StreamingSound {
    /// Stream identifier
    pub id: uuid::Uuid,
    /// Audio format
    pub format: AudioFormat,
    /// Stream configuration
    pub config: StreamConfig,
    /// Current state
    pub state: StreamState,
    /// Stream source
    source: Option<Box<dyn AsyncReadSeek + Send + Sync + Unpin>>,
    /// Current position
    pub position: u64,
    /// Total length (if known)
    pub length: Option<u64>,
}

impl StreamingSound {
    /// Create a new streaming sound
    pub fn new(format: AudioFormat, config: StreamConfig) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            format,
            config,
            state: StreamState::Initializing,
            source: None,
            position: 0,
            length: None,
        }
    }

    /// Start streaming from a source
    pub async fn start_streaming<T>(&mut self, source: T) -> Result<()>
    where
        T: AsyncRead + AsyncSeek + Send + Sync + Unpin + 'static,
    {
        self.source = Some(Box::new(source));
        self.state = StreamState::Ready;
        Ok(())
    }

    /// Read next audio chunk
    pub async fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<usize> {
        if let Some(source) = &mut self.source {
            use tokio::io::AsyncReadExt;
            let bytes_read = source.read(buffer).await?;
            self.position += bytes_read as u64;
            Ok(bytes_read)
        } else {
            Err(AudioDeviceError::StreamingError(
                "No source available".to_string(),
            ))
        }
    }

    /// Seek to position
    pub async fn seek(&mut self, position: u64) -> Result<()> {
        if let Some(source) = &mut self.source {
            use tokio::io::AsyncSeekExt;
            source.seek(tokio::io::SeekFrom::Start(position)).await?;
            self.position = position;
            Ok(())
        } else {
            Err(AudioDeviceError::StreamingError(
                "No source available".to_string(),
            ))
        }
    }
}
