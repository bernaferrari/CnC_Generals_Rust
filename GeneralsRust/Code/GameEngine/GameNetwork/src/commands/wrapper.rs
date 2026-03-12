//! Wrapper command for handling large messages
//!
//! When a GameMessage exceeds the maximum packet size, it must be split into
//! multiple wrapper commands for transmission. This module implements the
//! wrapper command system matching the C++ NetWrapperCommandMsg class.
//!
//! ## C++ NetWrapperCommandMsg Structure
//!
//! From NetCommandMsg.h:
//! ```cpp
//! class NetWrapperCommandMsg : public NetCommandMsg {
//!     UnsignedByte *m_data;              // Chunk data
//!     UnsignedInt m_dataLength;          // Length of this chunk
//!     UnsignedInt m_dataOffset;          // Offset within total data
//!     UnsignedInt m_totalDataLength;     // Total size of wrapped command
//!     UnsignedInt m_chunkNumber;         // This chunk's index (0-based)
//!     UnsignedInt m_numChunks;           // Total number of chunks
//!     UnsignedShort m_wrappedCommandID;  // ID of the original command
//! };
//! ```
//!
//! ## Binary Wire Format
//!
//! ```text
//! +---+---+---+---+---+---+---+---+
//! | Wrapped Command ID (u16)      |
//! +---+---+---+---+---+---+---+---+
//! | Chunk Number (u32)            |
//! +---+---+---+---+---+---+---+---+
//! | Total Chunks (u32)            |
//! +---+---+---+---+---+---+---+---+
//! | Data Offset (u32)             |
//! +---+---+---+---+---+---+---+---+
//! | Total Data Length (u32)       |
//! +---+---+---+---+---+---+---+---+
//! | Data Length (u32)             |
//! +---+---+---+---+---+---+---+---+
//! | Chunk Data (variable)         |
//! +---+---+---+---+---+---+---+---+
//! ```

use crate::error::{NetworkError, NetworkResult};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Write};
use tracing::{debug, trace, warn};

/// Maximum data size per wrapper chunk
/// Matches C++ MAX_PACKET_SIZE (476) minus wrapper header size (32) => 444
pub const MAX_WRAPPER_CHUNK_SIZE: usize = 444;

/// Wrapper command for splitting large messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WrapperCommand {
    /// ID of the wrapped command (from original NetCommandMsg)
    pub wrapped_command_id: u16,
    /// This chunk's number (0-based)
    pub chunk_number: u32,
    /// Total number of chunks
    pub num_chunks: u32,
    /// Offset of this chunk's data within the complete message
    pub data_offset: u32,
    /// Total length of the complete (unsplit) message
    pub total_data_length: u32,
    /// Length of data in this chunk
    pub data_length: u32,
    /// Actual chunk data
    pub data: Vec<u8>,
}

impl WrapperCommand {
    /// Create a new wrapper command
    pub fn new(
        wrapped_command_id: u16,
        chunk_number: u32,
        num_chunks: u32,
        data_offset: u32,
        total_data_length: u32,
        data: Vec<u8>,
    ) -> Self {
        let data_length = data.len() as u32;
        Self {
            wrapped_command_id,
            chunk_number,
            num_chunks,
            data_offset,
            total_data_length,
            data_length,
            data,
        }
    }

    /// Split a large message into wrapper commands
    pub fn split_message(command_id: u16, data: Vec<u8>) -> NetworkResult<Vec<WrapperCommand>> {
        let total_length = data.len();

        if total_length == 0 {
            return Err(NetworkError::invalid_command("cannot wrap empty message"));
        }

        // Calculate number of chunks needed
        let num_chunks = (total_length + MAX_WRAPPER_CHUNK_SIZE - 1) / MAX_WRAPPER_CHUNK_SIZE;

        if num_chunks > u32::MAX as usize {
            return Err(NetworkError::invalid_command(format!(
                "message too large to split: {} bytes",
                total_length
            )));
        }

        debug!(
            "Splitting message {} bytes into {} chunks",
            total_length, num_chunks
        );

        let mut chunks = Vec::with_capacity(num_chunks);

        for chunk_num in 0..num_chunks {
            let offset = chunk_num * MAX_WRAPPER_CHUNK_SIZE;
            let end = ((chunk_num + 1) * MAX_WRAPPER_CHUNK_SIZE).min(total_length);
            let chunk_data = data[offset..end].to_vec();

            let wrapper = WrapperCommand::new(
                command_id,
                chunk_num as u32,
                num_chunks as u32,
                offset as u32,
                total_length as u32,
                chunk_data,
            );

            trace!(
                "Created chunk {}/{}: offset={}, size={}",
                chunk_num + 1,
                num_chunks,
                offset,
                wrapper.data_length
            );

            chunks.push(wrapper);
        }

        Ok(chunks)
    }

    /// Serialize this wrapper command to bytes (C++ binary format)
    pub fn serialize(&self) -> NetworkResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(22 + self.data.len());

        // Write header fields (22 bytes total before data)
        buffer.write_u16::<LittleEndian>(self.wrapped_command_id)?;
        buffer.write_u32::<LittleEndian>(self.chunk_number)?;
        buffer.write_u32::<LittleEndian>(self.num_chunks)?;
        buffer.write_u32::<LittleEndian>(self.data_offset)?;
        buffer.write_u32::<LittleEndian>(self.total_data_length)?;
        buffer.write_u32::<LittleEndian>(self.data_length)?;

        // Write chunk data
        buffer.write_all(&self.data)?;

        trace!(
            "Serialized wrapper chunk {}/{} ({} bytes)",
            self.chunk_number + 1,
            self.num_chunks,
            buffer.len()
        );

        Ok(buffer)
    }

    /// Deserialize a wrapper command from bytes (C++ binary format)
    pub fn deserialize(data: &[u8]) -> NetworkResult<Self> {
        if data.len() < 22 {
            return Err(NetworkError::invalid_packet(format!(
                "wrapper command too short: {} bytes (minimum 22)",
                data.len()
            )));
        }

        let mut cursor = Cursor::new(data);

        // Read header fields
        let wrapped_command_id = cursor.read_u16::<LittleEndian>()?;
        let chunk_number = cursor.read_u32::<LittleEndian>()?;
        let num_chunks = cursor.read_u32::<LittleEndian>()?;
        let data_offset = cursor.read_u32::<LittleEndian>()?;
        let total_data_length = cursor.read_u32::<LittleEndian>()?;
        let data_length = cursor.read_u32::<LittleEndian>()?;

        // Read chunk data
        let pos = cursor.position() as usize;
        let chunk_data = data[pos..].to_vec();

        // Validate data length matches
        if chunk_data.len() != data_length as usize {
            warn!(
                "Wrapper data length mismatch: expected {}, got {}",
                data_length,
                chunk_data.len()
            );
        }

        trace!(
            "Deserialized wrapper chunk {}/{} ({} bytes)",
            chunk_number + 1,
            num_chunks,
            chunk_data.len()
        );

        Ok(Self {
            wrapped_command_id,
            chunk_number,
            num_chunks,
            data_offset,
            total_data_length,
            data_length,
            data: chunk_data,
        })
    }

    /// Validate this wrapper command
    pub fn validate(&self) -> NetworkResult<()> {
        // Check chunk number is within range
        if self.chunk_number >= self.num_chunks {
            return Err(NetworkError::invalid_command(format!(
                "chunk number {} >= num_chunks {}",
                self.chunk_number, self.num_chunks
            )));
        }

        // Check data offset is valid
        if self.data_offset as usize + self.data.len() > self.total_data_length as usize {
            return Err(NetworkError::invalid_command(format!(
                "data offset {} + length {} exceeds total length {}",
                self.data_offset,
                self.data.len(),
                self.total_data_length
            )));
        }

        // Check data length matches
        if self.data.len() != self.data_length as usize {
            return Err(NetworkError::invalid_command(format!(
                "data length mismatch: {} != {}",
                self.data.len(),
                self.data_length
            )));
        }

        // Check chunk isn't too large
        if self.data.len() > MAX_WRAPPER_CHUNK_SIZE {
            return Err(NetworkError::invalid_command(format!(
                "chunk too large: {} > {}",
                self.data.len(),
                MAX_WRAPPER_CHUNK_SIZE
            )));
        }

        Ok(())
    }
}

/// Reassembler for wrapper commands
///
/// Handles collecting chunks and reassembling them into the original message.
/// Supports out-of-order arrival and missing chunk detection.
#[derive(Debug)]
pub struct WrapperReassembler {
    /// Map of command ID -> partial message data
    pending: HashMap<u16, PartialMessage>,
}

#[derive(Debug)]
struct PartialMessage {
    /// Expected number of chunks
    num_chunks: u32,
    /// Total data length when complete
    total_length: u32,
    /// Received chunks (chunk_number -> data)
    chunks: HashMap<u32, Vec<u8>>,
}

impl WrapperReassembler {
    /// Create a new reassembler
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Add a wrapper chunk and try to reassemble
    ///
    /// Returns Some(data) if the message is complete, None if still waiting for chunks
    pub fn add_chunk(&mut self, wrapper: WrapperCommand) -> NetworkResult<Option<Vec<u8>>> {
        // Validate the chunk first
        wrapper.validate()?;

        let command_id = wrapper.wrapped_command_id;
        let chunk_num = wrapper.chunk_number;

        // Get or create partial message entry
        let partial = self.pending.entry(command_id).or_insert_with(|| {
            debug!(
                "Started reassembly for command {} ({} chunks expected)",
                command_id, wrapper.num_chunks
            );
            PartialMessage {
                num_chunks: wrapper.num_chunks,
                total_length: wrapper.total_data_length,
                chunks: HashMap::new(),
            }
        });

        // Verify chunk parameters match
        if partial.num_chunks != wrapper.num_chunks {
            return Err(NetworkError::invalid_command(format!(
                "chunk count mismatch for command {}: expected {}, got {}",
                command_id, partial.num_chunks, wrapper.num_chunks
            )));
        }

        if partial.total_length != wrapper.total_data_length {
            return Err(NetworkError::invalid_command(format!(
                "total length mismatch for command {}: expected {}, got {}",
                command_id, partial.total_length, wrapper.total_data_length
            )));
        }

        // Check for duplicate chunk
        if partial.chunks.contains_key(&chunk_num) {
            warn!(
                "Duplicate chunk {} for command {} (ignoring)",
                chunk_num, command_id
            );
            return Ok(None);
        }

        // Store this chunk
        partial.chunks.insert(chunk_num, wrapper.data);

        trace!(
            "Received chunk {}/{} for command {} ({}/{} chunks)",
            chunk_num + 1,
            partial.num_chunks,
            command_id,
            partial.chunks.len(),
            partial.num_chunks
        );

        // Check if we have all chunks
        if partial.chunks.len() == partial.num_chunks as usize {
            debug!(
                "All chunks received for command {}, reassembling",
                command_id
            );

            // Reassemble in order
            let mut reassembled = Vec::with_capacity(partial.total_length as usize);

            for chunk_idx in 0..partial.num_chunks {
                let chunk_data = partial.chunks.get(&chunk_idx).ok_or_else(|| {
                    NetworkError::invalid_command(format!(
                        "missing chunk {} during reassembly",
                        chunk_idx
                    ))
                })?;
                reassembled.extend_from_slice(chunk_data);
            }

            // Verify total length
            if reassembled.len() != partial.total_length as usize {
                return Err(NetworkError::invalid_command(format!(
                    "reassembled size mismatch: {} != {}",
                    reassembled.len(),
                    partial.total_length
                )));
            }

            // Remove from pending
            self.pending.remove(&command_id);

            debug!(
                "Successfully reassembled command {} ({} bytes)",
                command_id,
                reassembled.len()
            );

            Ok(Some(reassembled))
        } else {
            // Still waiting for more chunks
            Ok(None)
        }
    }

    /// Check if there are pending reassemblies
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Get number of pending command IDs
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get percent complete for a pending command (matches C++ chunk-count math).
    pub fn percent_complete(&self, command_id: u16) -> Option<u8> {
        let partial = self.pending.get(&command_id)?;
        if partial.num_chunks == 0 {
            return Some(0);
        }
        let present = partial.chunks.len() as u32;
        if present >= partial.num_chunks {
            return Some(100);
        }
        let percent = (present.saturating_mul(100) / partial.num_chunks).min(99);
        Some(percent as u8)
    }

    /// Clear a specific pending command (e.g., on timeout)
    pub fn clear_command(&mut self, command_id: u16) {
        if self.pending.remove(&command_id).is_some() {
            debug!("Cleared pending reassembly for command {}", command_id);
        }
    }

    /// Clear all pending reassemblies
    pub fn clear_all(&mut self) {
        self.pending.clear();
    }
}

impl Default for WrapperReassembler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrapper_serialization() {
        let wrapper = WrapperCommand::new(123, 0, 3, 0, 1000, vec![1, 2, 3, 4, 5]);

        let serialized = wrapper.serialize().unwrap();
        let deserialized = WrapperCommand::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.wrapped_command_id, 123);
        assert_eq!(deserialized.chunk_number, 0);
        assert_eq!(deserialized.num_chunks, 3);
        assert_eq!(deserialized.data_offset, 0);
        assert_eq!(deserialized.total_data_length, 1000);
        assert_eq!(deserialized.data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_split_small_message() {
        let data = vec![1; 100];
        let chunks = WrapperCommand::split_message(42, data).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_number, 0);
        assert_eq!(chunks[0].num_chunks, 1);
        assert_eq!(chunks[0].data.len(), 100);
    }

    #[test]
    fn test_split_large_message() {
        let data = vec![0xAB; 1000];
        let chunks = WrapperCommand::split_message(99, data).unwrap();

        // Should split into multiple chunks
        assert!(chunks.len() > 1);

        // All chunks should have same metadata
        for chunk in &chunks {
            assert_eq!(chunk.wrapped_command_id, 99);
            assert_eq!(chunk.total_data_length, 1000);
            assert_eq!(chunk.num_chunks, chunks.len() as u32);
        }

        // Chunk numbers should be sequential
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_number, i as u32);
        }
    }

    #[test]
    fn test_reassembly_in_order() {
        let original_data = vec![0x55; 1000];
        let chunks = WrapperCommand::split_message(1, original_data.clone()).unwrap();

        let mut reassembler = WrapperReassembler::new();

        // Add chunks in order
        for (i, chunk) in chunks.iter().enumerate() {
            let result = reassembler.add_chunk(chunk.clone()).unwrap();

            if i == chunks.len() - 1 {
                // Last chunk should complete reassembly
                assert!(result.is_some());
                let reassembled = result.unwrap();
                assert_eq!(reassembled, original_data);
            } else {
                // Earlier chunks should return None
                assert!(result.is_none());
            }
        }
    }

    #[test]
    fn test_reassembly_out_of_order() {
        let original_data = vec![0x77; 800];
        let mut chunks = WrapperCommand::split_message(2, original_data.clone()).unwrap();

        // Shuffle chunks
        chunks.reverse();

        let mut reassembler = WrapperReassembler::new();

        let mut result = None;
        for chunk in chunks {
            result = reassembler.add_chunk(chunk).unwrap();
            if result.is_some() {
                break;
            }
        }

        assert!(result.is_some());
        let reassembled = result.unwrap();
        assert_eq!(reassembled, original_data);
    }

    #[test]
    fn test_wrapper_validation() {
        let valid = WrapperCommand::new(1, 0, 2, 0, 100, vec![1; 50]);
        assert!(valid.validate().is_ok());

        // Chunk number too high
        let invalid_chunk_num = WrapperCommand::new(1, 3, 2, 0, 100, vec![1; 50]);
        assert!(invalid_chunk_num.validate().is_err());

        // Data length mismatch
        let mut invalid_length = valid.clone();
        invalid_length.data_length = 999;
        assert!(invalid_length.validate().is_err());

        // Chunk too large
        let too_large = WrapperCommand::new(1, 0, 1, 0, 10000, vec![1; MAX_WRAPPER_CHUNK_SIZE + 1]);
        assert!(too_large.validate().is_err());
    }

    #[test]
    fn test_duplicate_chunk_handling() {
        let original_data = vec![0x99; 500];
        let chunks = WrapperCommand::split_message(3, original_data.clone()).unwrap();

        let mut reassembler = WrapperReassembler::new();

        // Add first chunk
        assert!(reassembler.add_chunk(chunks[0].clone()).unwrap().is_none());

        // Add same chunk again
        assert!(reassembler.add_chunk(chunks[0].clone()).unwrap().is_none());

        // Add remaining chunks
        for chunk in &chunks[1..] {
            let result = reassembler.add_chunk(chunk.clone()).unwrap();
            if result.is_some() {
                assert_eq!(result.unwrap(), original_data);
                return;
            }
        }
    }

    #[test]
    fn test_round_trip_serialization() {
        let mut original_data = Vec::new();
        for _ in 0..256 {
            original_data.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        }
        let chunks = WrapperCommand::split_message(7, original_data.clone()).unwrap();

        // Serialize and deserialize each chunk
        let mut deserialized_chunks = Vec::new();
        for chunk in chunks {
            let serialized = chunk.serialize().unwrap();
            let deserialized = WrapperCommand::deserialize(&serialized).unwrap();
            deserialized_chunks.push(deserialized);
        }

        // Reassemble
        let mut reassembler = WrapperReassembler::new();
        let mut result = None;
        for chunk in deserialized_chunks {
            result = reassembler.add_chunk(chunk).unwrap();
            if result.is_some() {
                break;
            }
        }

        assert!(result.is_some());
        assert_eq!(result.unwrap(), original_data);
    }
}
