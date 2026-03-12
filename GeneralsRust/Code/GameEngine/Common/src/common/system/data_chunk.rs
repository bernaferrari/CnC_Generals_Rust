////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Data Chunk System Implementation
//!
//! Provides efficient data storage and retrieval using chunked memory blocks.
//! Used for managing large amounts of game data with optimal memory usage.
//!
//! Rust conversion: 2025

use super::compression::{
    compress_data, decompress_data, get_preferred_compression, is_data_compressed, CompressionLevel,
};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

/// Data chunk header information
#[derive(Debug, Clone)]
pub struct DataChunkHeader {
    pub chunk_id: u32,
    pub chunk_type: u32,
    pub data_size: u32,
    pub flags: u32,
    pub checksum: u32,
}

impl Default for DataChunkHeader {
    fn default() -> Self {
        Self {
            chunk_id: 0,
            chunk_type: 0,
            data_size: 0,
            flags: 0,
            checksum: 0,
        }
    }
}

// Data chunk flags
bitflags::bitflags! {
    pub struct ChunkFlags: u32 {
        const COMPRESSED = 0x00000001;
        const ENCRYPTED = 0x00000002;
        const CACHED = 0x00000004;
        const READ_ONLY = 0x00000008;
        const DIRTY = 0x00000010;
    }
}

/// Data chunk implementation
pub struct DataChunk {
    header: DataChunkHeader,
    data: Vec<u8>,
    is_loaded: bool,
}

impl Default for DataChunk {
    fn default() -> Self {
        Self::new()
    }
}

impl DataChunk {
    /// Create a new empty data chunk
    pub fn new() -> Self {
        Self {
            header: DataChunkHeader::default(),
            data: Vec::new(),
            is_loaded: false,
        }
    }

    /// Create a data chunk with specific data
    pub fn new_with_data(chunk_id: u32, chunk_type: u32, data: Vec<u8>) -> Self {
        let checksum = Self::calculate_checksum(&data);
        Self {
            header: DataChunkHeader {
                chunk_id,
                chunk_type,
                data_size: data.len() as u32,
                flags: 0,
                checksum,
            },
            data,
            is_loaded: true,
        }
    }

    /// Get the chunk header
    pub fn get_header(&self) -> &DataChunkHeader {
        &self.header
    }

    /// Get mutable access to the chunk header
    pub fn get_header_mut(&mut self) -> &mut DataChunkHeader {
        &mut self.header
    }

    /// Get the chunk data
    pub fn get_data(&self) -> &[u8] {
        &self.data
    }

    /// Get mutable access to the chunk data
    pub fn get_data_mut(&mut self) -> &mut Vec<u8> {
        self.mark_dirty();
        &mut self.data
    }

    /// Set the chunk data
    pub fn set_data(&mut self, data: Vec<u8>) {
        self.header.data_size = data.len() as u32;
        self.header.checksum = Self::calculate_checksum(&data);
        self.data = data;
        self.is_loaded = true;
        self.mark_dirty();
    }

    /// Check if the chunk is loaded
    pub fn is_loaded(&self) -> bool {
        self.is_loaded
    }

    /// Load chunk data from the currently attached payload.
    pub fn load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_loaded {
            return Ok(());
        }

        if self.data.is_empty() {
            return Err("DataChunk has no backing data payload".into());
        }

        if self.has_flag(ChunkFlags::COMPRESSED) || is_data_compressed(&self.data) {
            self.decompress()?;
        }

        self.is_loaded = true;
        Ok(())
    }

    /// Unload chunk data to save memory
    pub fn unload(&mut self) {
        if !self.has_flag(ChunkFlags::CACHED) {
            self.data.clear();
            self.is_loaded = false;
        }
    }

    /// Validate chunk data integrity
    pub fn validate(&self) -> bool {
        let calculated_checksum = Self::calculate_checksum(&self.data);
        calculated_checksum == self.header.checksum
    }

    /// Calculate checksum for data
    fn calculate_checksum(data: &[u8]) -> u32 {
        let mut checksum = 0u32;
        for &byte in data {
            checksum = checksum.wrapping_add(byte as u32);
            checksum = checksum.rotate_left(1);
        }
        checksum
    }

    /// Set a flag
    pub fn set_flag(&mut self, flag: ChunkFlags) {
        self.header.flags |= flag.bits();
    }

    /// Clear a flag
    pub fn clear_flag(&mut self, flag: ChunkFlags) {
        self.header.flags &= !flag.bits();
    }

    /// Check if a flag is set
    pub fn has_flag(&self, flag: ChunkFlags) -> bool {
        (self.header.flags & flag.bits()) != 0
    }

    /// Mark chunk as dirty (modified)
    pub fn mark_dirty(&mut self) {
        self.set_flag(ChunkFlags::DIRTY);
    }

    /// Mark chunk as clean
    pub fn mark_clean(&mut self) {
        self.clear_flag(ChunkFlags::DIRTY);
    }

    /// Check if chunk is dirty
    pub fn is_dirty(&self) -> bool {
        self.has_flag(ChunkFlags::DIRTY)
    }

    /// Compress chunk data.
    pub fn compress(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.has_flag(ChunkFlags::COMPRESSED) {
            return Ok(()); // Already compressed
        }

        if self.data.is_empty() {
            return Ok(());
        }

        let compressed = compress_data(
            &self.data,
            get_preferred_compression(),
            CompressionLevel::Default,
        )?;
        self.data = compressed;
        self.header.data_size = self.data.len() as u32;
        self.header.checksum = Self::calculate_checksum(&self.data);
        self.set_flag(ChunkFlags::COMPRESSED);
        self.mark_dirty();
        Ok(())
    }

    /// Decompress chunk data.
    pub fn decompress(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.has_flag(ChunkFlags::COMPRESSED) {
            return Ok(()); // Not compressed
        }

        if self.data.is_empty() {
            self.clear_flag(ChunkFlags::COMPRESSED);
            return Ok(());
        }

        self.data = decompress_data(&self.data)?;
        self.header.data_size = self.data.len() as u32;
        self.header.checksum = Self::calculate_checksum(&self.data);
        self.clear_flag(ChunkFlags::COMPRESSED);
        self.mark_dirty();
        Ok(())
    }

    /// Get chunk size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Check if chunk is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Clone the chunk data
    pub fn clone_data(&self) -> Vec<u8> {
        self.data.clone()
    }
}

/// Data chunk manager for handling multiple chunks
pub struct DataChunkManager {
    chunks: HashMap<u32, DataChunk>,
    memory_limit: usize,
    current_memory_usage: usize,
}

impl DataChunkManager {
    /// Create a new chunk manager
    pub fn new(memory_limit: usize) -> Self {
        Self {
            chunks: HashMap::new(),
            memory_limit,
            current_memory_usage: 0,
        }
    }

    /// Add a chunk to the manager
    pub fn add_chunk(&mut self, chunk: DataChunk) -> Result<(), Box<dyn std::error::Error>> {
        let chunk_id = chunk.header.chunk_id;
        let chunk_size = chunk.size();

        // Check memory limit
        if self.current_memory_usage + chunk_size > self.memory_limit {
            self.free_memory_if_needed(chunk_size)?;
        }

        self.current_memory_usage += chunk_size;
        self.chunks.insert(chunk_id, chunk);
        Ok(())
    }

    /// Get a chunk by ID
    pub fn get_chunk(&self, chunk_id: u32) -> Option<&DataChunk> {
        self.chunks.get(&chunk_id)
    }

    /// Get mutable access to a chunk by ID
    pub fn get_chunk_mut(&mut self, chunk_id: u32) -> Option<&mut DataChunk> {
        self.chunks.get_mut(&chunk_id)
    }

    /// Remove a chunk by ID
    pub fn remove_chunk(&mut self, chunk_id: u32) -> Option<DataChunk> {
        if let Some(chunk) = self.chunks.remove(&chunk_id) {
            self.current_memory_usage -= chunk.size();
            Some(chunk)
        } else {
            None
        }
    }

    /// Get the number of chunks
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.current_memory_usage
    }

    /// Get memory limit
    pub fn memory_limit(&self) -> usize {
        self.memory_limit
    }

    /// Set memory limit
    pub fn set_memory_limit(&mut self, limit: usize) {
        self.memory_limit = limit;
    }

    /// Free memory by unloading least recently used chunks
    fn free_memory_if_needed(
        &mut self,
        needed_space: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut freed_space = 0;
        let mut chunks_to_unload = Vec::new();

        // Find chunks that can be unloaded
        for (chunk_id, chunk) in &self.chunks {
            if !chunk.has_flag(ChunkFlags::CACHED) && chunk.is_loaded() {
                chunks_to_unload.push(*chunk_id);
                freed_space += chunk.size();

                if freed_space >= needed_space {
                    break;
                }
            }
        }

        // Unload chunks
        for chunk_id in chunks_to_unload {
            if let Some(chunk) = self.chunks.get_mut(&chunk_id) {
                let chunk_size = chunk.size();
                chunk.unload();
                self.current_memory_usage -= chunk_size;
            }
        }

        if freed_space < needed_space {
            return Err("Unable to free enough memory".into());
        }

        Ok(())
    }

    /// Validate all chunks
    pub fn validate_all(&self) -> Vec<u32> {
        let mut invalid_chunks = Vec::new();

        for (chunk_id, chunk) in &self.chunks {
            if !chunk.validate() {
                invalid_chunks.push(*chunk_id);
            }
        }

        invalid_chunks
    }

    /// Get chunks by type
    pub fn get_chunks_by_type(&self, chunk_type: u32) -> Vec<&DataChunk> {
        self.chunks
            .values()
            .filter(|chunk| chunk.header.chunk_type == chunk_type)
            .collect()
    }

    /// Compress all compressible chunks
    pub fn compress_all(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let mut compressed_count = 0;

        for chunk in self.chunks.values_mut() {
            if !chunk.has_flag(ChunkFlags::COMPRESSED) && !chunk.has_flag(ChunkFlags::READ_ONLY) {
                chunk.compress()?;
                compressed_count += 1;
            }
        }

        Ok(compressed_count)
    }

    /// Save dirty chunks (mock implementation)
    pub fn save_dirty_chunks(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
        let mut saved_count = 0;

        for chunk in self.chunks.values_mut() {
            if chunk.is_dirty() && !chunk.has_flag(ChunkFlags::READ_ONLY) {
                // Mock save operation
                chunk.mark_clean();
                saved_count += 1;
            }
        }

        Ok(saved_count)
    }

    /// Clear all chunks
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.current_memory_usage = 0;
    }
}

impl Default for DataChunkManager {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024) // Default 64MB limit
    }
}

/// Global chunk manager instance
static CHUNK_MANAGER: OnceCell<Mutex<DataChunkManager>> = OnceCell::new();

/// Initialize the global chunk manager
pub fn init_chunk_manager(memory_limit: Option<usize>) {
    let limit = memory_limit.unwrap_or(64 * 1024 * 1024);

    if CHUNK_MANAGER.get().is_none() {
        let _ = CHUNK_MANAGER.set(Mutex::new(DataChunkManager::new(limit)));
    } else if let Some(cell) = CHUNK_MANAGER.get() {
        if let Ok(mut guard) = cell.lock() {
            *guard = DataChunkManager::new(limit);
        }
    }
}

/// Get reference to the global chunk manager
pub fn get_chunk_manager() -> Option<MutexGuard<'static, DataChunkManager>> {
    CHUNK_MANAGER.get().and_then(|manager| manager.lock().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_chunk_creation() {
        let chunk = DataChunk::new();
        assert!(!chunk.is_loaded());
        assert!(chunk.is_empty());
        assert_eq!(chunk.header.chunk_id, 0);
    }

    #[test]
    fn test_data_chunk_with_data() {
        let data = vec![1, 2, 3, 4, 5];
        let chunk = DataChunk::new_with_data(123, 456, data.clone());

        assert!(chunk.is_loaded());
        assert_eq!(chunk.get_data(), data);
        assert_eq!(chunk.header.chunk_id, 123);
        assert_eq!(chunk.header.chunk_type, 456);
        assert_eq!(chunk.header.data_size, 5);
    }

    #[test]
    fn test_chunk_flags() {
        let mut chunk = DataChunk::new();

        assert!(!chunk.has_flag(ChunkFlags::COMPRESSED));
        chunk.set_flag(ChunkFlags::COMPRESSED);
        assert!(chunk.has_flag(ChunkFlags::COMPRESSED));

        chunk.clear_flag(ChunkFlags::COMPRESSED);
        assert!(!chunk.has_flag(ChunkFlags::COMPRESSED));
    }

    #[test]
    fn test_chunk_validation() {
        let data = vec![1, 2, 3, 4, 5];
        let chunk = DataChunk::new_with_data(1, 1, data);

        assert!(chunk.validate());
    }

    #[test]
    fn test_chunk_manager() {
        let mut manager = DataChunkManager::new(1024);

        let chunk1 = DataChunk::new_with_data(1, 1, vec![1, 2, 3]);
        let chunk2 = DataChunk::new_with_data(2, 1, vec![4, 5, 6]);

        manager.add_chunk(chunk1).unwrap();
        manager.add_chunk(chunk2).unwrap();

        assert_eq!(manager.chunk_count(), 2);
        assert!(manager.get_chunk(1).is_some());
        assert!(manager.get_chunk(2).is_some());
    }

    #[test]
    fn test_chunks_by_type() {
        let mut manager = DataChunkManager::new(1024);

        let chunk1 = DataChunk::new_with_data(1, 100, vec![1, 2, 3]);
        let chunk2 = DataChunk::new_with_data(2, 200, vec![4, 5, 6]);
        let chunk3 = DataChunk::new_with_data(3, 100, vec![7, 8, 9]);

        manager.add_chunk(chunk1).unwrap();
        manager.add_chunk(chunk2).unwrap();
        manager.add_chunk(chunk3).unwrap();

        let type_100_chunks = manager.get_chunks_by_type(100);
        assert_eq!(type_100_chunks.len(), 2);

        let type_200_chunks = manager.get_chunks_by_type(200);
        assert_eq!(type_200_chunks.len(), 1);
    }

    #[test]
    fn test_dirty_tracking() {
        let mut chunk = DataChunk::new();
        assert!(!chunk.is_dirty());

        chunk.mark_dirty();
        assert!(chunk.is_dirty());

        chunk.mark_clean();
        assert!(!chunk.is_dirty());
    }

    #[test]
    fn test_chunk_compress_round_trip() {
        let mut chunk = DataChunk::new_with_data(10, 20, b"compress me please".to_vec());
        let original = chunk.get_data().to_vec();

        chunk.compress().expect("compress");
        assert!(chunk.has_flag(ChunkFlags::COMPRESSED));
        assert_ne!(chunk.get_data(), original);

        chunk.decompress().expect("decompress");
        assert!(!chunk.has_flag(ChunkFlags::COMPRESSED));
        assert_eq!(chunk.get_data(), original);
    }

    #[test]
    fn test_empty_chunk_load_fails_without_payload() {
        let mut chunk = DataChunk::new();
        let err = chunk.load().expect_err("load should fail without payload");
        assert!(err.to_string().contains("no backing data payload"));
    }
}
