//! W3D Chunk Reading Infrastructure
//!
//! This module provides the core chunk reading functionality for W3D files,
//! ported from chunkio.cpp in the original C++ codebase.
//!
//! # C++ Reference
//! - File: `/Code/Libraries/Source/WWVegas/WWLib/chunkio.cpp`
//! - Lines: 388-796 (ChunkLoadClass implementation)
//!
//! The W3D file format uses a hierarchical chunk structure where each chunk
//! has a header containing:
//! - `chunk_type`: 32-bit identifier (W3D_CHUNK_*)
//! - `chunk_size`: 32-bit size with MSB as sub-chunk flag
//!
//! # Key Features
//! - Stack-based chunk navigation (matches C++ MAX_STACK_DEPTH behavior)
//! - Micro-chunk support for small data elements
//! - Automatic position tracking and bounds checking
//! - Error handling for malformed data

use glam::{Quat, Vec2, Vec3, Vec4};
use std::convert::TryInto;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use thiserror::Error;

/// Maximum chunk recursion depth
/// C++ Reference: chunkio.h, line 159 - 256 levels supported
/// Previous Rust value was 10, but C++ supports up to 256 nested chunks
/// This was a critical bug that could fail on deeply nested W3D files
pub const MAX_STACK_DEPTH: usize = 256;

/// Chunk read errors
#[derive(Debug, Error)]
pub enum ChunkError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Unexpected end of chunk data")]
    UnexpectedEof,

    #[error("Invalid chunk header")]
    InvalidHeader,

    #[error("Chunk stack overflow (depth > {})", MAX_STACK_DEPTH)]
    StackOverflow,

    #[error("Chunk stack underflow")]
    StackUnderflow,

    #[error("Read would exceed chunk bounds")]
    BoundsExceeded,

    #[error("Invalid UTF-8 string data")]
    InvalidString(#[from] std::string::FromUtf8Error),

    #[error("Micro-chunk already open")]
    MicroChunkAlreadyOpen,

    #[error("No micro-chunk open")]
    NoMicroChunkOpen,
}

pub type ChunkResult<T> = Result<T, ChunkError>;

/// Chunk header structure
/// C++ Reference: w3d_file.h W3D_CHUNK_HEADER
#[derive(Debug, Clone, Copy)]
pub struct ChunkHeader {
    /// Chunk type identifier (W3D_CHUNK_*)
    pub chunk_type: u32,
    /// Chunk size in bytes (MSB = sub-chunk flag)
    pub chunk_size: u32,
}

impl ChunkHeader {
    /// Get the actual chunk size (without sub-chunk flag)
    /// C++ Reference: chunkio.h ChunkHeader::Get_Size()
    pub fn actual_size(&self) -> u32 {
        self.chunk_size & 0x7FFFFFFF
    }

    /// Check if this chunk contains sub-chunks
    /// C++ Reference: chunkio.h ChunkHeader::Get_Sub_Chunk_Flag()
    pub fn has_sub_chunks(&self) -> bool {
        (self.chunk_size & 0x80000000) != 0
    }

    /// Get chunk type
    /// C++ Reference: chunkio.h ChunkHeader::Get_Type()
    pub fn get_type(&self) -> u32 {
        self.chunk_type
    }
}

/// Micro-chunk header (8-bit ID + 8-bit size)
/// C++ Reference: w3d_file.h MICRO_CHUNK_HEADER
#[derive(Debug, Clone, Copy)]
struct MicroChunkHeader {
    chunk_type: u8,
    chunk_size: u8,
}

/// W3D Chunk Reader
///
/// Provides hierarchical chunk reading with automatic position tracking.
/// Ported from ChunkLoadClass in chunkio.cpp.
///
/// # C++ Reference
/// - Class: ChunkLoadClass
/// - File: chunkio.cpp, lines 388-796
pub struct ChunkReader<R: Read + Seek> {
    /// Underlying data source
    reader: R,

    /// Stack of chunk headers
    /// C++ Reference: chunkio.h ChunkLoadClass::HeaderStack
    header_stack: [ChunkHeader; MAX_STACK_DEPTH],

    /// Stack of positions within each chunk
    /// C++ Reference: chunkio.h ChunkLoadClass::PositionStack
    position_stack: [u32; MAX_STACK_DEPTH],

    /// Current stack depth (0 = no chunks open)
    /// C++ Reference: chunkio.h ChunkLoadClass::StackIndex
    stack_index: usize,

    /// Whether currently inside a micro-chunk
    /// C++ Reference: chunkio.h ChunkLoadClass::InMicroChunk
    in_micro_chunk: bool,

    /// Micro-chunk header (if inside one)
    /// C++ Reference: chunkio.h ChunkLoadClass::MCHeader
    micro_chunk_header: MicroChunkHeader,

    /// Position within current micro-chunk
    /// C++ Reference: chunkio.h ChunkLoadClass::MicroChunkPosition
    micro_chunk_position: u32,
}

impl<R: Read + Seek> ChunkReader<R> {
    /// Create a new chunk reader
    /// C++ Reference: chunkio.cpp, line 388 (ChunkLoadClass::ChunkLoadClass)
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            header_stack: [ChunkHeader {
                chunk_type: 0,
                chunk_size: 0,
            }; MAX_STACK_DEPTH],
            position_stack: [0; MAX_STACK_DEPTH],
            stack_index: 0,
            in_micro_chunk: false,
            micro_chunk_header: MicroChunkHeader {
                chunk_type: 0,
                chunk_size: 0,
            },
            micro_chunk_position: 0,
        }
    }

    /// Open the next chunk in the file
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Open_Chunk()
    /// - File: chunkio.cpp, lines 412-433
    ///
    /// # Returns
    /// - `Ok(true)` if a chunk was opened successfully
    /// - `Ok(false)` if no more chunks available (parent chunk exhausted)
    /// - `Err` on read failure or stack overflow
    pub fn open_chunk(&mut self) -> ChunkResult<bool> {
        // C++ Line 415: check user didn't leave micro chunks open
        if self.in_micro_chunk {
            return Err(ChunkError::MicroChunkAlreadyOpen);
        }

        // C++ Line 418: check for stack overflow
        if self.stack_index >= MAX_STACK_DEPTH - 1 {
            return Err(ChunkError::StackOverflow);
        }

        // C++ Line 421: if parent chunk has been completely read, return false
        if self.stack_index > 0 {
            let parent_idx = self.stack_index - 1;
            if self.position_stack[parent_idx] >= self.header_stack[parent_idx].actual_size() {
                return Ok(false);
            }
        }

        // C++ Line 426: read the chunk header
        let mut header_bytes = [0u8; 8];
        match self.reader.read_exact(&mut header_bytes) {
            Ok(_) => {}
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(false),
            Err(e) => return Err(ChunkError::Io(e)),
        }

        let chunk_type = u32::from_le_bytes(header_bytes[0..4].try_into().unwrap());
        let chunk_size = u32::from_le_bytes(header_bytes[4..8].try_into().unwrap());

        self.header_stack[self.stack_index] = ChunkHeader {
            chunk_type,
            chunk_size,
        };
        self.position_stack[self.stack_index] = 0;
        self.stack_index += 1;

        Ok(true)
    }

    /// Close the current chunk
    ///
    /// Seeks to the end of the chunk if not all data was read.
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Close_Chunk()
    /// - File: chunkio.cpp, lines 448-469
    pub fn close_chunk(&mut self) -> ChunkResult<()> {
        // C++ Line 451: check user didn't leave micro chunks open
        if self.in_micro_chunk {
            return Err(ChunkError::MicroChunkAlreadyOpen);
        }

        // C++ Line 454: check for stack underflow
        if self.stack_index == 0 {
            return Err(ChunkError::StackUnderflow);
        }

        let idx = self.stack_index - 1;
        let chunk_size = self.header_stack[idx].actual_size();
        let pos = self.position_stack[idx];

        // C++ Line 459: seek past unread data
        if pos < chunk_size {
            self.reader
                .seek(SeekFrom::Current((chunk_size - pos) as i64))?;
        }

        self.stack_index -= 1;

        // C++ Line 464: update parent chunk position
        if self.stack_index > 0 {
            self.position_stack[self.stack_index - 1] += chunk_size + 8; // +8 for header
        }

        Ok(())
    }

    /// Get the current chunk ID
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Cur_Chunk_ID()
    /// - File: chunkio.cpp, lines 484-488
    pub fn current_chunk_id(&self) -> ChunkResult<u32> {
        if self.stack_index == 0 {
            return Err(ChunkError::StackUnderflow);
        }
        Ok(self.header_stack[self.stack_index - 1].chunk_type)
    }

    /// Get the current chunk length
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Cur_Chunk_Length()
    /// - File: chunkio.cpp, lines 503-507
    pub fn current_chunk_length(&self) -> ChunkResult<u32> {
        if self.stack_index == 0 {
            return Err(ChunkError::StackUnderflow);
        }
        Ok(self.header_stack[self.stack_index - 1].actual_size())
    }

    /// Get the current chunk recursion depth
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Cur_Chunk_Depth()
    /// - File: chunkio.cpp, lines 522-525
    pub fn current_chunk_depth(&self) -> usize {
        self.stack_index
    }

    /// Check if current chunk contains sub-chunks
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Contains_Chunks()
    /// - File: chunkio.cpp, lines 540-543
    pub fn contains_chunks(&self) -> ChunkResult<bool> {
        if self.stack_index == 0 {
            return Err(ChunkError::StackUnderflow);
        }
        Ok(self.header_stack[self.stack_index - 1].has_sub_chunks())
    }

    /// Open a micro-chunk
    ///
    /// Micro-chunks are non-hierarchical 8-bit ID + 8-bit size structures
    /// used for wrapping individual variables.
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Open_Micro_Chunk()
    /// - File: chunkio.cpp, lines 557-570
    pub fn open_micro_chunk(&mut self) -> ChunkResult<bool> {
        // C++ Line 559: assert not already in micro-chunk
        if self.in_micro_chunk {
            return Err(ChunkError::MicroChunkAlreadyOpen);
        }

        // C++ Line 563: read the micro-chunk header using Read()
        let mut header_bytes = [0u8; 2];
        let bytes_read = self.read(&mut header_bytes)?;
        if bytes_read != 2 {
            return Ok(false);
        }

        self.micro_chunk_header = MicroChunkHeader {
            chunk_type: header_bytes[0],
            chunk_size: header_bytes[1],
        };

        self.in_micro_chunk = true;
        self.micro_chunk_position = 0;

        Ok(true)
    }

    /// Close the current micro-chunk
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Close_Micro_Chunk()
    /// - File: chunkio.cpp, lines 585-605
    pub fn close_micro_chunk(&mut self) -> ChunkResult<()> {
        // C++ Line 587: assert in micro-chunk
        if !self.in_micro_chunk {
            return Err(ChunkError::NoMicroChunkOpen);
        }

        self.in_micro_chunk = false;

        let chunk_size = self.micro_chunk_header.chunk_size as u32;
        let pos = self.micro_chunk_position;

        // C++ Line 594: seek past unread micro-chunk data
        if pos < chunk_size {
            let skip_bytes = chunk_size - pos;
            self.reader.seek(SeekFrom::Current(skip_bytes as i64))?;

            // C++ Line 599: update parent chunk position
            if self.stack_index > 0 {
                self.position_stack[self.stack_index - 1] += skip_bytes;
            }
        }

        Ok(())
    }

    /// Get current micro-chunk ID
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Cur_Micro_Chunk_ID()
    /// - File: chunkio.cpp, lines 622-626
    pub fn current_micro_chunk_id(&self) -> ChunkResult<u8> {
        if !self.in_micro_chunk {
            return Err(ChunkError::NoMicroChunkOpen);
        }
        Ok(self.micro_chunk_header.chunk_type)
    }

    /// Get current micro-chunk length
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Cur_Micro_Chunk_Length()
    /// - File: chunkio.cpp, lines 643-647
    pub fn current_micro_chunk_length(&self) -> ChunkResult<u8> {
        if !self.in_micro_chunk {
            return Err(ChunkError::NoMicroChunkOpen);
        }
        Ok(self.micro_chunk_header.chunk_size)
    }

    /// Seek forward within the current chunk
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Seek()
    /// - File: chunkio.cpp, lines 650-678
    pub fn seek(&mut self, bytes: u32) -> ChunkResult<u32> {
        if self.stack_index == 0 {
            return Err(ChunkError::StackUnderflow);
        }

        let idx = self.stack_index - 1;

        // C++ Line 655: don't seek past end of chunk
        if self.position_stack[idx] + bytes > self.header_stack[idx].actual_size() {
            return Err(ChunkError::BoundsExceeded);
        }

        // C++ Line 660: don't seek past end of micro-chunk
        if self.in_micro_chunk
            && self.micro_chunk_position + bytes > self.micro_chunk_header.chunk_size as u32
        {
            return Err(ChunkError::BoundsExceeded);
        }

        // C++ Line 665: perform the seek
        self.reader.seek(SeekFrom::Current(bytes as i64))?;

        // C++ Line 670: update position tracking
        self.position_stack[idx] += bytes;

        // C++ Line 673: update micro-chunk position if in one
        if self.in_micro_chunk {
            self.micro_chunk_position += bytes;
        }

        Ok(bytes)
    }

    /// Read raw bytes from the current chunk
    ///
    /// Automatically tracks position and enforces chunk boundaries.
    ///
    /// # C++ Reference
    /// - Method: ChunkLoadClass::Read()
    /// - File: chunkio.cpp, lines 692-719
    pub fn read(&mut self, buf: &mut [u8]) -> ChunkResult<usize> {
        let nbytes = buf.len() as u32;

        // C++ Line 694: check stack
        if self.stack_index == 0 {
            return Err(ChunkError::StackUnderflow);
        }

        let idx = self.stack_index - 1;

        // C++ Line 697: don't read past end of chunk
        if self.position_stack[idx] + nbytes > self.header_stack[idx].actual_size() {
            return Err(ChunkError::BoundsExceeded);
        }

        // C++ Line 702: don't read past end of micro-chunk
        if self.in_micro_chunk
            && self.micro_chunk_position + nbytes > self.micro_chunk_header.chunk_size as u32
        {
            return Err(ChunkError::BoundsExceeded);
        }

        // C++ Line 706: perform the read
        self.reader.read_exact(buf)?;

        // C++ Line 711: update position tracking
        self.position_stack[idx] += nbytes;

        // C++ Line 714: update micro-chunk position if in one
        if self.in_micro_chunk {
            self.micro_chunk_position += nbytes;
        }

        Ok(buf.len())
    }

    // Convenience methods for reading common types

    /// Read a u8
    pub fn read_u8(&mut self) -> ChunkResult<u8> {
        let mut buf = [0u8; 1];
        self.read(&mut buf)?;
        Ok(buf[0])
    }

    /// Read a u16 (little-endian)
    pub fn read_u16(&mut self) -> ChunkResult<u16> {
        let mut buf = [0u8; 2];
        self.read(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    /// Read a u32 (little-endian)
    pub fn read_u32(&mut self) -> ChunkResult<u32> {
        let mut buf = [0u8; 4];
        self.read(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    /// Read an i32 (little-endian)
    pub fn read_i32(&mut self) -> ChunkResult<i32> {
        let mut buf = [0u8; 4];
        self.read(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    /// Read an f32 (little-endian)
    pub fn read_f32(&mut self) -> ChunkResult<f32> {
        let mut buf = [0u8; 4];
        self.read(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }

    /// Read a Vec2 (two f32s)
    pub fn read_vec2(&mut self) -> ChunkResult<Vec2> {
        let x = self.read_f32()?;
        let y = self.read_f32()?;
        Ok(Vec2::new(x, y))
    }

    /// Read a Vec3 (three f32s)
    /// C++ Reference: chunkio.cpp, line 753 (Read IOVector3Struct)
    pub fn read_vec3(&mut self) -> ChunkResult<Vec3> {
        let x = self.read_f32()?;
        let y = self.read_f32()?;
        let z = self.read_f32()?;
        Ok(Vec3::new(x, y, z))
    }

    /// Read a Vec4 (four f32s)
    /// C++ Reference: chunkio.cpp, line 772 (Read IOVector4Struct)
    pub fn read_vec4(&mut self) -> ChunkResult<Vec4> {
        let x = self.read_f32()?;
        let y = self.read_f32()?;
        let z = self.read_f32()?;
        let w = self.read_f32()?;
        Ok(Vec4::new(x, y, z, w))
    }

    /// Read a quaternion (w, x, y, z order)
    /// C++ Reference: chunkio.cpp, line 791 (Read IOQuaternionStruct)
    pub fn read_quaternion(&mut self) -> ChunkResult<Quat> {
        let w = self.read_f32()?;
        let x = self.read_f32()?;
        let y = self.read_f32()?;
        let z = self.read_f32()?;
        Ok(Quat::from_xyzw(x, y, z, w))
    }

    /// Read a fixed-length null-terminated string
    ///
    /// Reads exactly `len` bytes and converts to UTF-8 string,
    /// stopping at the first null byte.
    pub fn read_fixed_string(&mut self, len: usize) -> ChunkResult<String> {
        let mut buf = vec![0u8; len];
        self.read(&mut buf)?;

        // Find null terminator
        let end = buf.iter().position(|&b| b == 0).unwrap_or(len);

        // Convert to string
        String::from_utf8(buf[..end].to_vec()).map_err(ChunkError::InvalidString)
    }

    /// Get remaining bytes in current chunk
    pub fn remaining(&self) -> ChunkResult<u32> {
        if self.stack_index == 0 {
            return Err(ChunkError::StackUnderflow);
        }

        let idx = self.stack_index - 1;
        let size = self.header_stack[idx].actual_size();
        let pos = self.position_stack[idx];

        Ok(size.saturating_sub(pos))
    }

    /// Get access to the underlying reader
    pub fn inner(&self) -> &R {
        &self.reader
    }

    /// Get mutable access to the underlying reader
    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Consume the ChunkReader and return the underlying reader
    pub fn into_inner(self) -> R {
        self.reader
    }
}

/// Convenience type for chunk readers over byte slices
pub type ChunkReaderSlice<'a> = ChunkReader<Cursor<&'a [u8]>>;

impl<'a> ChunkReaderSlice<'a> {
    /// Create a new chunk reader from a byte slice
    pub fn from_slice(data: &'a [u8]) -> Self {
        ChunkReader::new(Cursor::new(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test chunk with the given type, size, and data
    fn create_chunk(chunk_type: u32, has_sub_chunks: bool, data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();

        // Write chunk type
        result.extend_from_slice(&chunk_type.to_le_bytes());

        // Write chunk size with sub-chunk flag
        let size = data.len() as u32;
        let size_with_flag = if has_sub_chunks {
            size | 0x80000000
        } else {
            size
        };
        result.extend_from_slice(&size_with_flag.to_le_bytes());

        // Write data
        result.extend_from_slice(data);

        result
    }

    #[test]
    fn test_chunk_header_actual_size() {
        let header = ChunkHeader {
            chunk_type: 0x00000002,
            chunk_size: 0x80000100, // Sub-chunk flag set, size 256
        };

        assert_eq!(header.actual_size(), 256);
        assert!(header.has_sub_chunks());
    }

    #[test]
    fn test_open_close_chunk() {
        let data = create_chunk(0x00000002, false, &[1, 2, 3, 4]);
        let mut reader = ChunkReaderSlice::from_slice(&data);

        assert!(reader.open_chunk().unwrap());
        assert_eq!(reader.current_chunk_id().unwrap(), 0x00000002);
        assert_eq!(reader.current_chunk_length().unwrap(), 4);
        assert_eq!(reader.current_chunk_depth(), 1);

        reader.close_chunk().unwrap();
        assert_eq!(reader.current_chunk_depth(), 0);
    }

    #[test]
    fn test_read_within_chunk() {
        let data = create_chunk(0x00000002, false, &[10, 20, 30, 40]);
        let mut reader = ChunkReaderSlice::from_slice(&data);

        reader.open_chunk().unwrap();

        let mut buf = [0u8; 2];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [10, 20]);

        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [30, 40]);

        reader.close_chunk().unwrap();
    }

    #[test]
    fn test_read_bounds_checking() {
        let data = create_chunk(0x00000002, false, &[1, 2, 3]);
        let mut reader = ChunkReaderSlice::from_slice(&data);

        reader.open_chunk().unwrap();

        let mut buf = [0u8; 5]; // Try to read more than available
        let result = reader.read(&mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn test_nested_chunks() {
        // Create parent chunk containing child chunk
        let child_data = create_chunk(0x00000003, false, &[5, 6, 7, 8]);
        let parent_data = create_chunk(0x00000002, true, &child_data);

        let mut reader = ChunkReaderSlice::from_slice(&parent_data);

        // Open parent
        assert!(reader.open_chunk().unwrap());
        assert_eq!(reader.current_chunk_id().unwrap(), 0x00000002);
        assert!(reader.contains_chunks().unwrap());

        // Open child
        assert!(reader.open_chunk().unwrap());
        assert_eq!(reader.current_chunk_id().unwrap(), 0x00000003);
        assert_eq!(reader.current_chunk_depth(), 2);

        // Read child data
        let mut buf = [0u8; 4];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [5, 6, 7, 8]);

        // Close child
        reader.close_chunk().unwrap();
        assert_eq!(reader.current_chunk_depth(), 1);

        // Close parent
        reader.close_chunk().unwrap();
        assert_eq!(reader.current_chunk_depth(), 0);
    }

    #[test]
    fn test_read_typed_values() {
        let mut data = Vec::new();
        data.extend_from_slice(&42u32.to_le_bytes());
        data.extend_from_slice(&3.14f32.to_le_bytes());

        let chunk_data = create_chunk(0x00000002, false, &data);
        let mut reader = ChunkReaderSlice::from_slice(&chunk_data);

        reader.open_chunk().unwrap();

        assert_eq!(reader.read_u32().unwrap(), 42);
        assert!((reader.read_f32().unwrap() - 3.14).abs() < 0.001);

        reader.close_chunk().unwrap();
    }

    #[test]
    fn test_read_vec3() {
        let mut data = Vec::new();
        data.extend_from_slice(&1.0f32.to_le_bytes());
        data.extend_from_slice(&2.0f32.to_le_bytes());
        data.extend_from_slice(&3.0f32.to_le_bytes());

        let chunk_data = create_chunk(0x00000002, false, &data);
        let mut reader = ChunkReaderSlice::from_slice(&chunk_data);

        reader.open_chunk().unwrap();

        let vec = reader.read_vec3().unwrap();
        assert_eq!(vec, Vec3::new(1.0, 2.0, 3.0));

        reader.close_chunk().unwrap();
    }

    #[test]
    fn test_read_fixed_string() {
        let data = b"Hello\0\0\0\0\0\0\0\0\0\0\0".to_vec(); // 16 bytes
        let chunk_data = create_chunk(0x00000002, false, &data);
        let mut reader = ChunkReaderSlice::from_slice(&chunk_data);

        reader.open_chunk().unwrap();

        let s = reader.read_fixed_string(16).unwrap();
        assert_eq!(s, "Hello");

        reader.close_chunk().unwrap();
    }

    #[test]
    fn test_micro_chunks() {
        let mut data = Vec::new();
        // Micro-chunk: type=5, size=4, data=[1,2,3,4]
        data.push(5u8);
        data.push(4u8);
        data.extend_from_slice(&[1, 2, 3, 4]);

        let chunk_data = create_chunk(0x00000002, false, &data);
        let mut reader = ChunkReaderSlice::from_slice(&chunk_data);

        reader.open_chunk().unwrap();

        assert!(reader.open_micro_chunk().unwrap());
        assert_eq!(reader.current_micro_chunk_id().unwrap(), 5);
        assert_eq!(reader.current_micro_chunk_length().unwrap(), 4);

        let mut buf = [0u8; 4];
        reader.read(&mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3, 4]);

        reader.close_micro_chunk().unwrap();
        reader.close_chunk().unwrap();
    }

    #[test]
    fn test_seek() {
        let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let chunk_data = create_chunk(0x00000002, false, &data);
        let mut reader = ChunkReaderSlice::from_slice(&chunk_data);

        reader.open_chunk().unwrap();

        // Read first byte
        assert_eq!(reader.read_u8().unwrap(), 0);

        // Seek forward 3 bytes
        reader.seek(3).unwrap();

        // Read next byte (should be 4)
        assert_eq!(reader.read_u8().unwrap(), 4);

        reader.close_chunk().unwrap();
    }

    #[test]
    fn test_remaining() {
        let data = vec![1, 2, 3, 4, 5];
        let chunk_data = create_chunk(0x00000002, false, &data);
        let mut reader = ChunkReaderSlice::from_slice(&chunk_data);

        reader.open_chunk().unwrap();

        assert_eq!(reader.remaining().unwrap(), 5);

        reader.read_u8().unwrap();
        assert_eq!(reader.remaining().unwrap(), 4);

        reader.read_u8().unwrap();
        reader.read_u8().unwrap();
        assert_eq!(reader.remaining().unwrap(), 2);

        reader.close_chunk().unwrap();
    }
}
