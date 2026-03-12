//! Internal audio buffer management (not exposed in public API).
#![allow(dead_code)]

use crate::{error::Result, formats::AudioFormat};
use parking_lot::RwLock;
use std::sync::Arc;

/// Audio buffer for internal use
pub(crate) struct AudioBuffer {
    data: Vec<u8>,
    format: AudioFormat,
    capacity: usize,
    position: usize,
}

/// Circular audio buffer for streaming
pub(crate) struct CircularBuffer {
    buffer: Vec<u8>,
    read_pos: usize,
    write_pos: usize,
    size: usize,
    capacity: usize,
}

/// Buffer pool for reusing audio buffers
pub(crate) struct BufferPool {
    available: Arc<RwLock<Vec<AudioBuffer>>>,
    max_buffers: usize,
    buffer_size: usize,
}

impl AudioBuffer {
    /// Create new audio buffer
    pub fn new(capacity: usize, format: AudioFormat) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            format,
            capacity,
            position: 0,
        }
    }

    /// Write data to buffer
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let available = self.capacity - self.data.len();
        let to_write = data.len().min(available);

        self.data.extend_from_slice(&data[..to_write]);
        Ok(to_write)
    }

    /// Read data from buffer
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let available = self.data.len() - self.position;
        let to_read = buf.len().min(available);

        buf[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
        self.position += to_read;

        Ok(to_read)
    }

    /// Reset buffer for reuse
    pub fn reset(&mut self) {
        self.data.clear();
        self.position = 0;
    }

    /// Get buffer format
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// Get available space
    pub fn available_space(&self) -> usize {
        self.capacity - self.data.len()
    }

    /// Get available data
    pub fn available_data(&self) -> usize {
        self.data.len() - self.position
    }
}

impl CircularBuffer {
    /// Create new circular buffer
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0; capacity],
            read_pos: 0,
            write_pos: 0,
            size: 0,
            capacity,
        }
    }

    /// Write data to circular buffer
    pub fn write(&mut self, data: &[u8]) -> usize {
        let available = self.capacity - self.size;
        let to_write = data.len().min(available);

        for &byte in &data[..to_write] {
            self.buffer[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            self.size += 1;
        }

        to_write
    }

    /// Read data from circular buffer
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let available = self.size;
        let to_read = buf.len().min(available);

        for i in 0..to_read {
            buf[i] = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % self.capacity;
        }

        self.size -= to_read;
        to_read
    }

    /// Peek at data without consuming
    pub fn peek(&self, buf: &mut [u8]) -> usize {
        let available = self.size;
        let to_peek = buf.len().min(available);
        let mut pos = self.read_pos;

        for i in 0..to_peek {
            buf[i] = self.buffer[pos];
            pos = (pos + 1) % self.capacity;
        }

        to_peek
    }

    /// Get available space for writing
    pub fn available_write(&self) -> usize {
        self.capacity - self.size
    }

    /// Get available data for reading
    pub fn available_read(&self) -> usize {
        self.size
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.read_pos = 0;
        self.write_pos = 0;
        self.size = 0;
    }
}

impl BufferPool {
    /// Create new buffer pool
    pub fn new(max_buffers: usize, buffer_size: usize) -> Self {
        Self {
            available: Arc::new(RwLock::new(Vec::new())),
            max_buffers,
            buffer_size,
        }
    }

    /// Get buffer from pool or create new one
    pub fn get_buffer(&self, format: AudioFormat) -> AudioBuffer {
        let mut available = self.available.write();

        if let Some(mut buffer) = available.pop() {
            buffer.reset();
            buffer
        } else {
            AudioBuffer::new(self.buffer_size, format)
        }
    }

    /// Return buffer to pool
    pub fn return_buffer(&self, buffer: AudioBuffer) {
        let mut available = self.available.write();

        if available.len() < self.max_buffers {
            available.push(buffer);
        }
        // If pool is full, buffer will be dropped
    }

    /// Get pool statistics
    pub fn stats(&self) -> BufferPoolStats {
        let available = self.available.read();
        BufferPoolStats {
            available_buffers: available.len(),
            max_buffers: self.max_buffers,
            buffer_size: self.buffer_size,
        }
    }
}

/// Buffer pool statistics
#[derive(Debug, Clone)]
pub(crate) struct BufferPoolStats {
    pub available_buffers: usize,
    pub max_buffers: usize,
    pub buffer_size: usize,
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new(32, 4096) // 32 buffers of 4KB each
    }
}
