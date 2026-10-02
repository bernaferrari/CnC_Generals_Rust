//! Stream buffering system for efficient audio data management.
//!
//! This module provides safe buffer management and streaming access for audio data,
//! converting from the original C++ stream buffering implementation while maintaining
//! the same interface and functionality. It uses Rust's ownership system to ensure
//! safe memory operations and channels for data flow.

use crate::error::{Error, Result};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Stream buffer identification
pub type BufferId = u32;

/// Stream access identification
pub type AccessId = u32;

/// Stream access mode for reading/writing operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamAccessMode {
    /// Update mode - automatically updates downstream when data is consumed
    Update,
    /// Manual mode - requires explicit advancement calls
    Manual,
}

/// Stream access type (input/output)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamAccessType {
    /// Input access for writing data into the stream
    Input = 0,
    /// Output access for reading data from the stream
    Output = 1,
}

/// Maximum number of concurrent accessors per stream
pub const MAX_STREAM_ACCESSORS: usize = 2;

/// Stream operation results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamResult {
    /// Operation completed successfully
    Ok,
    /// End of stream reached
    Eof,
    /// Operation failed
    Fail,
}

/// Data block for stream operations
#[derive(Debug, Clone)]
pub struct StreamDataBlock {
    /// Pointer to data buffer
    pub data: Vec<u8>,
    /// Number of valid bytes in buffer
    pub bytes: usize,
    /// Buffer capacity
    pub capacity: usize,
}

impl StreamDataBlock {
    /// Create a new empty data block
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            bytes: 0,
            capacity: 0,
        }
    }

    /// Create a data block with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            bytes: 0,
            capacity,
        }
    }

    /// Get slice of valid data
    pub fn valid_data(&self) -> &[u8] {
        &self.data[..self.bytes]
    }

    /// Get mutable slice of valid data
    pub fn valid_data_mut(&mut self) -> &mut [u8] {
        &mut self.data[..self.bytes]
    }
}

impl Default for StreamDataBlock {
    fn default() -> Self {
        Self::new()
    }
}

/// Stream buffer region for different accessors
#[derive(Debug, Clone)]
pub struct StreamBufferRegion {
    /// Data pointer for this region
    pub data: Vec<u8>,
    /// Number of bytes available in this region
    pub bytes: usize,
    /// Region offset within the parent buffer
    pub offset: usize,
}

impl StreamBufferRegion {
    /// Create a new buffer region
    pub fn new(size: usize, offset: usize) -> Self {
        Self {
            data: vec![0u8; size],
            bytes: size,
            offset,
        }
    }

    /// Reset region to full capacity
    pub fn reset(&mut self, data: Vec<u8>) {
        self.bytes = data.len();
        self.data = data;
        self.offset = 0;
    }
}

/// Stream buffer for holding audio data
pub struct StreamBuffer {
    /// Unique buffer identifier
    pub id: BufferId,
    /// Data regions for different accessors
    pub regions: [StreamBufferRegion; MAX_STREAM_ACCESSORS],
    /// Main data region that regions point into
    pub data_region: StreamDataBlock,
    /// Buffer memory alignment
    pub alignment: usize,
    /// Raw memory allocation
    _buffer_memory: Vec<u8>,
}

impl StreamBuffer {
    /// Create a new stream buffer with specified size and alignment
    pub fn new(size: usize, alignment: usize) -> Result<Self> {
        if size == 0 {
            return Err(Error::Memory("Buffer size cannot be zero".to_string()));
        }

        // Allocate memory with alignment padding
        let total_size = size + alignment;
        let buffer_memory = vec![0u8; total_size];

        // Calculate aligned start address
        let aligned_start = {
            let raw_ptr = buffer_memory.as_ptr() as usize;
            (raw_ptr + alignment - 1) & !(alignment - 1)
        };
        let offset = aligned_start - buffer_memory.as_ptr() as usize;

        // Create data region
        let mut data_region = StreamDataBlock::with_capacity(size);
        data_region.data = buffer_memory[offset..offset + size].to_vec();
        data_region.bytes = size;
        data_region.capacity = size;

        // Initialize regions for each accessor
        let regions = [
            StreamBufferRegion::new(size, 0),
            StreamBufferRegion::new(size, 0),
        ];

        Ok(Self {
            id: 0,
            regions,
            data_region,
            alignment,
            _buffer_memory: buffer_memory,
        })
    }

    /// Reset buffer to initial state
    pub fn reset(&mut self) {
        for region in &mut self.regions {
            region.reset(self.data_region.data.clone());
        }
    }
}

/// Stream access interface for reading/writing stream data.
///
/// An accessor is plain bookkeeping state plus an index into the owning
/// `StreamBuffering` ring; the C++ original used raw `StreamBuffer*`
/// pointers, which only ever pointed inside the same buffering object.
pub struct StreamAccess {
    /// Access identifier
    pub id: AccessId,
    /// Access type (input/output)
    pub access_type: StreamAccessType,
    /// Access mode (update/manual)
    pub mode: StreamAccessMode,
    /// Current data block being accessed
    pub block: StreamDataBlock,
    /// Ring index of the buffer currently being accessed
    current_buffer: Option<usize>,
    /// Ring index of the buffer this access starts at
    start_buffer: Option<usize>,
    /// Current position within access
    access_position: usize,
    /// Absolute position in stream
    absolute_position: u64,
    /// Bytes consumed by this accessor
    bytes_out: u64,
    /// Bytes produced for this accessor
    bytes_in: u64,
    /// Last operation error
    last_error: Option<Error>,
    /// Access flags
    flags: u32,
}

/// Stream access flags
const ACCESS_FLAG_TOP_OF_START: u32 = 0x0001;

impl StreamAccess {
    /// Create a new stream access
    pub fn new(id: AccessId, access_type: StreamAccessType) -> Self {
        Self {
            id,
            access_type,
            mode: StreamAccessMode::Update,
            block: StreamDataBlock::new(),
            current_buffer: None,
            start_buffer: None,
            access_position: 0,
            absolute_position: 0,
            bytes_out: 0,
            bytes_in: 0,
            last_error: None,
            flags: 0,
        }
    }

    /// Set access mode
    pub fn set_mode(&mut self, mode: StreamAccessMode) {
        self.mode = mode;
    }

    /// Get current access mode
    pub fn get_mode(&self) -> StreamAccessMode {
        self.mode
    }

    /// Get access ID
    pub fn get_id(&self) -> AccessId {
        self.id
    }

    /// Get last error
    pub fn get_error(&self) -> Option<&Error> {
        self.last_error.as_ref()
    }

    /// Get total available bytes for this access
    pub fn get_total_bytes(&self) -> usize {
        self.bytes_in.wrapping_sub(self.bytes_out) as usize
    }

    /// Get current position in stream
    pub fn get_position(&self) -> u64 {
        self.absolute_position
    }
}

/// Main stream buffering system
///
/// Owns its ring of `StreamBuffer`s outright and the two accessors that walk
/// them. The whole object is driven by a single owner (the streaming task),
/// so buffers are stored by value and addressed by ring index instead of
/// shared handle types.
pub struct StreamBuffering {
    /// Ring of buffers owned by this stream
    buffers: Vec<StreamBuffer>,
    /// Stream access interfaces
    accessors: [Option<StreamAccess>; MAX_STREAM_ACCESSORS],
    /// Total bytes in all buffers
    total_bytes: usize,
    /// Bytes queued for input processing
    input_queued: usize,
    /// Bytes queued for output processing
    output_queued: usize,
    /// Number of buffers
    buffer_count: usize,
    /// Stream flags
    flags: u32,
    /// Next buffer ID to assign
    next_buffer_id: BufferId,
}

/// Stream flags
const STREAM_FLAG_RESET_DONE: u32 = 0x0001;

impl StreamBuffering {
    /// Create a new stream buffering system
    pub fn new() -> Self {
        Self {
            buffers: Vec::new(),
            accessors: [None, None],
            total_bytes: 0,
            input_queued: 0,
            output_queued: 0,
            buffer_count: 0,
            flags: 0,
            next_buffer_id: 0,
        }
    }

    /// Borrow an acquired accessor
    fn access_mut(&mut self, access_id: AccessId) -> Result<&mut StreamAccess> {
        self.accessors
            .get_mut(access_id as usize)
            .and_then(|slot| slot.as_mut())
            .ok_or_else(|| Error::Memory("Access not acquired".to_string()))
    }

    /// Ring index that follows `index`, wrapping around the ring
    fn next_buffer_index(&self, index: usize) -> Option<usize> {
        if self.buffers.is_empty() {
            None
        } else if index + 1 < self.buffers.len() {
            Some(index + 1)
        } else {
            Some(0)
        }
    }

    /// Add a buffer to the stream
    pub fn add_buffer(&mut self, mut buffer: StreamBuffer) -> Result<()> {
        buffer.id = self.next_buffer_id;
        self.next_buffer_id += 1;
        let capacity = buffer.data_region.capacity;

        self.buffers.push(buffer);

        self.buffer_count += 1;
        self.total_bytes = self.total_bytes.saturating_add(capacity);
        self.flags &= !STREAM_FLAG_RESET_DONE;
        Ok(())
    }

    /// Create multiple buffers of specified size
    pub fn create_buffers(
        &mut self,
        count: usize,
        buffer_size: usize,
        alignment: usize,
    ) -> Result<usize> {
        let mut created = 0;

        for _ in 0..count {
            match StreamBuffer::new(buffer_size, alignment) {
                Ok(buffer) => {
                    self.add_buffer(buffer)?;
                    created += 1;
                }
                Err(_) => break,
            }
        }

        Ok(created)
    }

    /// Acquire access to the stream
    pub fn acquire_access(&mut self, access_type: StreamAccessType) -> Result<&mut StreamAccess> {
        let access_id = access_type as AccessId;

        if self.accessors[access_id as usize].is_some() {
            return Err(Error::Memory("Access already acquired".to_string()));
        }

        if self.buffer_count == 0 {
            return Err(Error::Memory("No buffers in stream".to_string()));
        }

        // Reset stream if this is the first access
        if self.accessors.iter().all(|a| a.is_none()) {
            self.reset_internal()?;
        }

        let mut access = StreamAccess::new(access_id, access_type);
        access.start_buffer = if self.buffers.is_empty() { None } else { Some(0) };
        access.current_buffer = access.start_buffer;
        if let Some(buffer_index) = access.current_buffer {
            let region = &self.buffers[buffer_index].regions[access_id as usize];
            access.block.data = region.data.clone();
            access.block.bytes = region.bytes;
            access.block.capacity = region.data.len();
        }

        // For input accessor, initialize with available space
        if access_type == StreamAccessType::Input {
            access.bytes_in = access.bytes_in.wrapping_add(self.total_bytes as u64);
        }

        self.accessors[access_id as usize] = Some(access);
        Ok(self.accessors[access_id as usize].as_mut().unwrap())
    }

    /// Release access to the stream
    pub fn release_access(&mut self, access_id: AccessId) -> Result<()> {
        if access_id as usize >= MAX_STREAM_ACCESSORS {
            return Err(Error::Memory("Invalid access ID".to_string()));
        }

        self.accessors[access_id as usize] = None;
        Ok(())
    }

    /// Reset the stream to initial state
    pub fn reset(&mut self) -> Result<()> {
        // Check that no accessors are currently active
        if self.accessors.iter().any(|a| a.is_some()) {
            return Err(Error::Memory(
                "Cannot reset stream with active accessors".to_string(),
            ));
        }

        self.reset_internal()
    }

    /// Internal reset implementation
    fn reset_internal(&mut self) -> Result<()> {
        // Reset all buffers
        for (index, buffer) in self.buffers.iter_mut().enumerate() {
            buffer.reset();
            buffer.id = index as BufferId;
        }
        self.total_bytes = self
            .buffers
            .iter()
            .map(|buffer| buffer.data_region.capacity)
            .sum();

        // Reset accessor states (if any exist but not currently acquired)
        for i in 0..MAX_STREAM_ACCESSORS {
            if self.accessors[i].is_some() {
                self.reset_access(i as AccessId);
            }
        }

        self.flags |= STREAM_FLAG_RESET_DONE;
        self.input_queued = self.buffers.len()
            * self
                .buffers
                .first()
                .map(|buffer| buffer.data_region.capacity)
                .unwrap_or(0);
        self.output_queued = 0;
        Ok(())
    }

    /// Rewind one accessor to the start of the ring
    fn reset_access(&mut self, access_id: AccessId) {
        let start = if self.buffers.is_empty() { None } else { Some(0) };
        if let Ok(access) = self.access_mut(access_id) {
            access.start_buffer = start;
            access.bytes_in = 0;
            access.bytes_out = 0;
            access.absolute_position = 0;
            access.access_position = 0;
            access.current_buffer = start;
        }
    }

    /// Destroy all buffers in the stream
    pub fn destroy_buffers(&mut self) {
        self.buffers.clear();
        self.buffer_count = 0;
        self.total_bytes = 0;
        self.input_queued = 0;
        self.output_queued = 0;

        // Reset all accessors
        for accessor in &mut self.accessors {
            *accessor = None;
        }

        self.reset_internal().unwrap_or(());
    }

    /// Recompute queued byte counts for all accessors
    pub fn recalc_queue_stats(&mut self) {
        self.recalculate_queue_state_interior();
    }

    fn recalculate_queue_state_interior(&mut self) {
        let buffer_capacity = self
            .buffers
            .first()
            .map(|buffer| buffer.data_region.capacity)
            .unwrap_or(0);

        self.input_queued = self.buffers.len() * buffer_capacity;
        self.output_queued =
            if let Some(ref access) = self.accessors[StreamAccessType::Output as usize] {
                access.get_total_bytes()
            } else {
                0
            };
    }

    /// Get total bytes in stream
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Get total bytes available for input
    pub fn total_bytes_till_full(&self) -> usize {
        if let Some(ref access) = self.accessors[StreamAccessType::Input as usize] {
            access.get_total_bytes()
        } else {
            0
        }
    }

    /// Get total bytes available for output
    pub fn total_bytes_in(&self) -> usize {
        if let Some(ref access) = self.accessors[StreamAccessType::Output as usize] {
            access.get_total_bytes()
        } else {
            0
        }
    }

    /// Check if stream is full
    pub fn is_full(&self) -> bool {
        self.total_bytes_till_full() == 0
    }

    /// Ring index of the buffer that follows `buffer_id`
    pub fn get_next_buffer(&self, buffer_id: BufferId) -> Option<usize> {
        let found = self.buffers.iter().position(|buffer| buffer.id == buffer_id)?;

        // Wrap around to the first buffer for circular behavior
        Some(if found + 1 < self.buffers.len() {
            found + 1
        } else {
            0
        })
    }

    /// Update upstream accessor when downstream consumes data
    pub fn update_upstream_accessor(&mut self, downstream_id: AccessId, bytes: u64) -> Result<()> {
        let upstream_id = if downstream_id == 0 { 1 } else { 0 };

        if let Some(ref mut upstream) = self.accessors[upstream_id] {
            upstream.bytes_in = upstream.bytes_in.wrapping_add(bytes);
        }

        Ok(())
    }

    /// Transfer data to/from the stream through one accessor
    pub async fn transfer(&mut self, access_id: AccessId, data: &mut [u8]) -> Result<usize> {
        let mut transferred = 0;
        let mut remaining = data.len();
        let mut data_offset = 0;

        while remaining > 0 {
            self.get_block(access_id).await?;

            let (block_bytes, access_type) = {
                let access = self.access_mut(access_id)?;
                (access.block.bytes, access.access_type)
            };

            if block_bytes == 0 {
                break; // Stream exhausted
            }

            let transfer_size = remaining.min(block_bytes);

            {
                let access = self.access_mut(access_id)?;
                match access_type {
                    StreamAccessType::Input => {
                        // Copy data into stream
                        access.block.data[..transfer_size]
                            .copy_from_slice(&data[data_offset..data_offset + transfer_size]);
                    }
                    StreamAccessType::Output => {
                        // Copy data from stream
                        data[data_offset..data_offset + transfer_size]
                            .copy_from_slice(&access.block.valid_data()[..transfer_size]);
                    }
                }
            }

            self.advance(access_id, transfer_size).await?;
            data_offset += transfer_size;
            remaining -= transfer_size;
            transferred += transfer_size;
        }

        Ok(transferred)
    }

    /// Transfer data to/from file through one accessor
    pub async fn file_transfer(
        &mut self,
        access_id: AccessId,
        file: &mut File,
        bytes: usize,
    ) -> Result<(usize, StreamResult)> {
        let mut transferred = 0;
        let mut remaining = bytes;

        while remaining > 0 {
            self.get_block(access_id).await?;

            let block_bytes = self.access_mut(access_id)?.block.bytes;
            if block_bytes == 0 {
                return Ok((transferred, StreamResult::Eof));
            }

            let transfer_size = remaining.min(block_bytes);

            let result = {
                let access = self.access_mut(access_id)?;
                match access.access_type {
                    StreamAccessType::Input => {
                        // Read from file into stream
                        match file.read(&mut access.block.data[..transfer_size]).await {
                            Ok(bytes_read) => {
                                if bytes_read == 0 {
                                    return Ok((transferred, StreamResult::Eof));
                                }
                                bytes_read
                            }
                            Err(e) => {
                                access.last_error = Some(Error::Io(e));
                                return Ok((transferred, StreamResult::Fail));
                            }
                        }
                    }
                    StreamAccessType::Output => {
                        // Write from stream to file
                        match file.write(&access.block.valid_data()[..transfer_size]).await {
                            Ok(bytes_written) => bytes_written,
                            Err(e) => {
                                access.last_error = Some(Error::Io(e));
                                return Ok((transferred, StreamResult::Fail));
                            }
                        }
                    }
                }
            };

            self.advance(access_id, result).await?;
            remaining -= result;
            transferred += result;

            if result < transfer_size {
                // Partial transfer indicates end of file or error
                return Ok((transferred, StreamResult::Eof));
            }
        }

        Ok((transferred, StreamResult::Ok))
    }

    /// Get current data block for an access
    pub async fn get_block(&mut self, access_id: AccessId) -> Result<usize> {
        self.advance(access_id, 0).await?;
        Ok(self.access_mut(access_id)?.block.bytes)
    }

    /// Advance an access position
    pub async fn advance(&mut self, access_id: AccessId, bytes_to_advance: usize) -> Result<usize> {
        let total_bytes = self.access_mut(access_id)?.get_total_bytes();

        let access_position = {
            let access = self.access_mut(access_id)?;
            if access.access_position >= total_bytes {
                access.block.bytes = 0;
                return Ok(0);
            }
            access.access_position
        };

        let available_bytes = total_bytes - access_position;
        let advance_bytes = bytes_to_advance.min(available_bytes);

        // Update positions
        {
            let access = self.access_mut(access_id)?;
            access.access_position += advance_bytes;
            access.absolute_position += advance_bytes as u64;
        }

        // Update queued byte counts
        let access_type = self.access_mut(access_id)?.access_type;
        if access_type == StreamAccessType::Input {
            self.input_queued = self.input_queued.saturating_add(advance_bytes);
        } else {
            self.output_queued = self.output_queued.saturating_add(advance_bytes);
        }

        // Handle wrap-around for absolute position
        let total_stream_bytes = self.total_bytes as u64;
        if total_stream_bytes != 0 {
            let access = self.access_mut(access_id)?;
            if access.absolute_position >= total_stream_bytes {
                access.absolute_position %= total_stream_bytes;
            }
        }

        // Update buffer access if needed
        self.update_buffer_access(access_id)?;

        // Perform update if in update mode
        let needs_update = {
            let access = self.access_mut(access_id)?;
            access.access_position > 0 && access.mode == StreamAccessMode::Update
        };
        if needs_update {
            self.update_access(access_id)?;
        }

        Ok(advance_bytes)
    }

    /// Update one accessor after its data was consumed
    fn update_access(&mut self, access_id: AccessId) -> Result<usize> {
        let bytes_to_update = self.access_mut(access_id)?.access_position;
        if bytes_to_update == 0 {
            return Ok(0);
        }

        // Update byte counts
        {
            let access = self.access_mut(access_id)?;
            access.bytes_out = access.bytes_out.wrapping_add(bytes_to_update as u64);
        }

        // Notify upstream accessor
        self.update_upstream_accessor(access_id, bytes_to_update as u64)?;

        // Update buffer positions
        self.update_start_buffer(access_id, bytes_to_update)?;
        self.return_to_start(access_id)?;

        Ok(bytes_to_update)
    }

    /// Return an access to its start position
    fn return_to_start(&mut self, access_id: AccessId) -> Result<()> {
        {
            let access = self.access_mut(access_id)?;
            access.access_position = 0;
            access.current_buffer = access.start_buffer;
        }

        let current = self.access_mut(access_id)?.current_buffer;
        if let Some(buffer_index) = current {
            let region_data = self.buffers[buffer_index].regions[access_id as usize].data.clone();
            let access = self.access_mut(access_id)?;
            access.block.data = region_data;
            access.block.bytes = 0; // Will be set by next get_block call
        }

        let access = self.access_mut(access_id)?;
        access.flags &= !ACCESS_FLAG_TOP_OF_START;
        Ok(())
    }

    /// Update buffer access positions for one accessor
    fn update_buffer_access(&mut self, access_id: AccessId) -> Result<()> {
        let current = self.access_mut(access_id)?.current_buffer;
        let Some(buffer_index) = current else {
            return Ok(());
        };

        let access_type = self.access_mut(access_id)?.access_type;
        let (available_in_region, region_copy) = {
            let region = &self.buffers[buffer_index].regions[access_id as usize];
            let copy = if access_type == StreamAccessType::Output && region.bytes > 0 {
                let copy_len = region.bytes.min(region.data.len());
                Some(region.data[..copy_len].to_vec())
            } else {
                None
            };
            (region.bytes, copy)
        };

        let access = self.access_mut(access_id)?;
        if access.block.data.len() < available_in_region {
            access.block.data.resize(available_in_region, 0);
            access.block.capacity = access.block.data.len();
        }
        if let Some(region_copy) = region_copy {
            access.block.data[..region_copy.len()].copy_from_slice(&region_copy);
        }

        let total_available = access.get_total_bytes();
        access.block.bytes = available_in_region.min(total_available);
        Ok(())
    }

    /// Update the start buffer of one accessor after consumption
    fn update_start_buffer(&mut self, access_id: AccessId, bytes_consumed: usize) -> Result<()> {
        let mut remaining_bytes = bytes_consumed;

        while remaining_bytes > 0 {
            let Some(buffer_index) = self.access_mut(access_id)?.start_buffer else {
                break;
            };

            let stayed = {
                let buffer = &mut self.buffers[buffer_index];
                let reset_data = buffer.data_region.data.clone();
                let region = &mut buffer.regions[access_id as usize];

                if region.bytes <= remaining_bytes {
                    remaining_bytes -= region.bytes;
                    region.reset(reset_data);
                    false
                } else {
                    region.data.drain(..remaining_bytes);
                    region.bytes -= remaining_bytes;
                    region.offset += remaining_bytes;
                    remaining_bytes = 0;
                    true
                }
            };

            let next = if stayed {
                Some(buffer_index)
            } else {
                let buffer_id = self.buffers[buffer_index].id;
                self.get_next_buffer(buffer_id)
            };
            self.access_mut(access_id)?.start_buffer = next;
        }

        Ok(())
    }
}

/// Stream profiling for performance monitoring
#[cfg(debug_assertions)]
pub struct StreamProfile {
    /// Total bytes processed
    bytes_processed: u64,
    /// Last update timestamp
    last_update: std::time::Instant,
    /// Update interval
    update_interval: std::time::Duration,
    /// Current processing rate
    processing_rate: u64,
    /// Profile active flag
    active: bool,
}

#[cfg(debug_assertions)]
impl StreamProfile {
    /// Start profiling
    pub fn start(&mut self) {
        self.bytes_processed = 0;
        self.last_update = std::time::Instant::now();
        self.update_interval = std::time::Duration::from_secs(3);
        self.processing_rate = 0;
        self.active = true;
    }

    /// Stop profiling
    pub fn stop(&mut self) {
        self.active = false;
        self.processing_rate = 0;
    }

    /// Update profile with processed bytes
    pub fn update(&mut self, bytes: u64) {
        if !self.active {
            return;
        }

        self.bytes_processed += bytes;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_update);

        if elapsed >= self.update_interval {
            self.processing_rate = (self.bytes_processed * 1000) / elapsed.as_millis() as u64;
            self.last_update = now;
            self.bytes_processed = 0;
        }
    }

    /// Get current processing rate (bytes per second)
    pub fn get_rate(&self) -> u64 {
        if self.active { self.processing_rate } else { 0 }
    }

    /// Check if profiling is active
    pub fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg(debug_assertions)]
impl Default for StreamProfile {
    fn default() -> Self {
        Self {
            bytes_processed: 0,
            last_update: std::time::Instant::now(),
            update_interval: std::time::Duration::from_secs(3),
            processing_rate: 0,
            active: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_stream_buffer_creation() {
        let buffer = StreamBuffer::new(4096, 16).unwrap();
        assert_eq!(buffer.data_region.capacity, 4096);
        assert_eq!(buffer.alignment, 16);
    }

    #[tokio::test]
    async fn test_stream_buffering_basic() {
        let mut stream = StreamBuffering::new();
        assert_eq!(stream.total_bytes(), 0);
        assert_eq!(stream.buffer_count, 0);

        // Add some buffers
        stream.create_buffers(3, 1024, 8).unwrap();
        assert_eq!(stream.buffer_count, 3);
        assert_eq!(stream.total_bytes(), 3 * 1024);
    }

    #[tokio::test]
    async fn test_stream_access_acquisition() {
        let mut stream = StreamBuffering::new();
        stream.create_buffers(2, 1024, 8).unwrap();

        // Acquire input access
        let input_access = stream.acquire_access(StreamAccessType::Input).unwrap();
        assert_eq!(input_access.access_type, StreamAccessType::Input);

        // Acquire output access
        let output_access = stream.acquire_access(StreamAccessType::Output).unwrap();
        assert_eq!(output_access.access_type, StreamAccessType::Output);
    }

    #[tokio::test]
    async fn test_data_transfer() {
        let mut stream = StreamBuffering::new();
        stream.create_buffers(2, 1024, 8).unwrap();

        stream.acquire_access(StreamAccessType::Input).unwrap();

        // Test data transfer
        let test_data = vec![1, 2, 3, 4, 5];
        let mut data_copy = test_data.clone();

        let transferred = stream
            .transfer(StreamAccessType::Input as AccessId, &mut data_copy)
            .await
            .unwrap();
        assert_eq!(transferred, test_data.len());
    }

    #[tokio::test]
    async fn test_stream_reset() {
        let mut stream = StreamBuffering::new();
        stream.create_buffers(2, 1024, 8).unwrap();

        // Reset should work when no accessors are active
        stream.reset().unwrap();
        assert_eq!(stream.buffer_count, 2);
        assert_eq!(stream.total_bytes(), 2 * 1024);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_stream_profiling() {
        let mut profile = StreamProfile::default();
        assert!(!profile.is_active());

        profile.start();
        assert!(profile.is_active());

        profile.update(1024);
        assert_eq!(profile.bytes_processed, 1024);

        profile.stop();
        assert!(!profile.is_active());
        assert_eq!(profile.get_rate(), 0);
    }
}
