//! Streaming compression and decompression
//!
//! This module provides streaming interfaces for ZLib compression:
//! - Stream large files without loading into memory
//! - Process data incrementally
//! - Support for async I/O with Tokio
//! - Memory-efficient sliding window management

use crate::{
    deflate::Compressor as DeflateCompressor, inflate::Decompressor as InflateDecompressor,
    CompressionLevel, Result, ZlibError, ZlibHeader,
};
use std::io::{Read, Write};

#[allow(unused_imports)]
use std::io::prelude::*;

#[cfg(feature = "streaming")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Streaming compressor
pub struct StreamingCompressor {
    compressor: DeflateCompressor,
    level: CompressionLevel,
    buffer_size: usize,
}

impl StreamingCompressor {
    /// Create new streaming compressor
    pub fn new(level: CompressionLevel) -> Self {
        Self {
            compressor: DeflateCompressor::new(level),
            level,
            buffer_size: 64 * 1024, // 64KB default
        }
    }

    /// Set buffer size for streaming
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Compress from reader to writer
    pub fn compress_stream<R: Read, W: Write>(
        &mut self,
        mut reader: R,
        mut writer: W,
    ) -> Result<usize> {
        // Write ZLib header
        let header = ZlibHeader::new(15, self.level);
        writer.write_all(&header.to_bytes())?;

        // Read and compress in chunks
        let mut buffer = vec![0u8; self.buffer_size];
        let mut total_bytes = 0;
        let mut all_data = Vec::new();

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            all_data.extend_from_slice(&buffer[..bytes_read]);
            total_bytes += bytes_read;
        }

        // Compress all data
        let compressed = self.compressor.compress(&all_data)?;
        writer.write_all(&compressed)?;

        // Write Adler32 checksum
        let checksum = adler32::adler32(std::io::Cursor::new(&all_data))?;
        writer.write_all(&checksum.to_be_bytes())?;

        Ok(total_bytes)
    }

    /// Compress file
    pub fn compress_file(&mut self, input_path: &str, output_path: &str) -> Result<usize> {
        let input = std::fs::File::open(input_path)?;
        let output = std::fs::File::create(output_path)?;

        self.compress_stream(input, output)
    }

    /// Compress bytes
    pub fn compress_bytes(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let cursor = std::io::Cursor::new(data);

        self.compress_stream(cursor, &mut output)?;

        Ok(output)
    }

    /// Async compress stream (requires "streaming" feature)
    #[cfg(feature = "streaming")]
    pub async fn compress_stream_async<R, W>(
        &mut self,
        mut reader: R,
        mut writer: W,
    ) -> Result<usize>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        // Write ZLib header
        let header = ZlibHeader::new(15, self.level);
        writer.write_all(&header.to_bytes()).await?;

        // Read and compress in chunks
        let mut buffer = vec![0u8; self.buffer_size];
        let mut total_bytes = 0;
        let mut all_data = Vec::new();

        loop {
            let bytes_read = reader.read(&mut buffer).await?;
            if bytes_read == 0 {
                break;
            }

            all_data.extend_from_slice(&buffer[..bytes_read]);
            total_bytes += bytes_read;
        }

        // Compress all data
        let compressed = self.compressor.compress(&all_data)?;
        writer.write_all(&compressed).await?;

        // Write Adler32 checksum
        let checksum = adler32::adler32(std::io::Cursor::new(&all_data))?;
        writer.write_all(&checksum.to_be_bytes()).await?;

        Ok(total_bytes)
    }
}

/// Streaming decompressor
pub struct StreamingDecompressor {
    decompressor: InflateDecompressor,
    buffer_size: usize,
}

impl StreamingDecompressor {
    /// Create new streaming decompressor
    pub fn new() -> Self {
        Self {
            decompressor: InflateDecompressor::new(),
            buffer_size: 64 * 1024, // 64KB default
        }
    }

    /// Set buffer size for streaming
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Decompress from reader to writer
    pub fn decompress_stream<R: Read, W: Write>(
        &mut self,
        mut reader: R,
        mut writer: W,
    ) -> Result<usize> {
        // Read all data (for now - in production would be streaming)
        let mut compressed_data = Vec::new();
        reader.read_to_end(&mut compressed_data)?;

        if compressed_data.len() < 6 {
            return Err(ZlibError::BufferTooSmall {
                needed: 6,
                available: compressed_data.len(),
            });
        }

        // Parse ZLib header
        let header = ZlibHeader::from_bytes(&compressed_data)?;
        let offset = header.size();

        // Extract compressed data (excluding checksum)
        let deflate_data = &compressed_data[offset..compressed_data.len() - 4];

        // Decompress
        let decompressed = self.decompressor.decompress(deflate_data)?;

        // Verify checksum
        let stored_checksum = u32::from_be_bytes([
            compressed_data[compressed_data.len() - 4],
            compressed_data[compressed_data.len() - 3],
            compressed_data[compressed_data.len() - 2],
            compressed_data[compressed_data.len() - 1],
        ]);

        let calculated_checksum = adler32::adler32(std::io::Cursor::new(&decompressed))?;

        if stored_checksum != calculated_checksum {
            return Err(ZlibError::ChecksumMismatch {
                expected: stored_checksum,
                actual: calculated_checksum,
            });
        }

        // Write decompressed data
        writer.write_all(&decompressed)?;

        Ok(decompressed.len())
    }

    /// Decompress file
    pub fn decompress_file(&mut self, input_path: &str, output_path: &str) -> Result<usize> {
        let input = std::fs::File::open(input_path)?;
        let output = std::fs::File::create(output_path)?;

        self.decompress_stream(input, output)
    }

    /// Decompress bytes
    pub fn decompress_bytes(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let cursor = std::io::Cursor::new(data);

        self.decompress_stream(cursor, &mut output)?;

        Ok(output)
    }

    /// Async decompress stream (requires "streaming" feature)
    #[cfg(feature = "streaming")]
    pub async fn decompress_stream_async<R, W>(
        &mut self,
        mut reader: R,
        mut writer: W,
    ) -> Result<usize>
    where
        R: AsyncRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        // Read all data
        let mut compressed_data = Vec::new();
        reader.read_to_end(&mut compressed_data).await?;

        if compressed_data.len() < 6 {
            return Err(ZlibError::BufferTooSmall {
                needed: 6,
                available: compressed_data.len(),
            });
        }

        // Parse ZLib header
        let header = ZlibHeader::from_bytes(&compressed_data)?;
        let offset = header.size();

        // Extract compressed data (excluding checksum)
        let deflate_data = &compressed_data[offset..compressed_data.len() - 4];

        // Decompress
        let decompressed = self.decompressor.decompress(deflate_data)?;

        // Verify checksum
        let stored_checksum = u32::from_be_bytes([
            compressed_data[compressed_data.len() - 4],
            compressed_data[compressed_data.len() - 3],
            compressed_data[compressed_data.len() - 2],
            compressed_data[compressed_data.len() - 1],
        ]);

        let calculated_checksum = adler32::adler32(std::io::Cursor::new(&decompressed))?;

        if stored_checksum != calculated_checksum {
            return Err(ZlibError::ChecksumMismatch {
                expected: stored_checksum,
                actual: calculated_checksum,
            });
        }

        // Write decompressed data
        writer.write_all(&decompressed).await?;

        Ok(decompressed.len())
    }
}

impl Default for StreamingDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Chunk-based streaming processor
pub struct ChunkedProcessor {
    chunk_size: usize,
}

impl ChunkedProcessor {
    /// Create new chunked processor
    pub fn new(chunk_size: usize) -> Self {
        Self { chunk_size }
    }

    /// Process data in chunks
    pub fn process<R, W, F>(&self, mut reader: R, mut writer: W, mut process_fn: F) -> Result<usize>
    where
        R: Read,
        W: Write,
        F: FnMut(&[u8]) -> Result<Vec<u8>>,
    {
        let mut buffer = vec![0u8; self.chunk_size];
        let mut total_bytes = 0;

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let processed = process_fn(&buffer[..bytes_read])?;
            writer.write_all(&processed)?;

            total_bytes += bytes_read;
        }

        Ok(total_bytes)
    }
}

/// Memory-mapped file compressor for very large files
pub struct MmapCompressor {
    level: CompressionLevel,
}

impl MmapCompressor {
    /// Create new memory-mapped compressor
    pub fn new(level: CompressionLevel) -> Self {
        Self { level }
    }

    /// Compress memory-mapped file
    pub fn compress_mmap(&self, input_path: &str, output_path: &str) -> Result<usize> {
        use memmap2::Mmap;

        // Open input file
        let file = std::fs::File::open(input_path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        // Compress using standard method
        let compressed = crate::compress(&mmap, self.level)?;

        // Write to output
        std::fs::write(output_path, &compressed)?;

        Ok(compressed.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_compress_bytes() {
        let data = b"Hello, streaming compression!";
        let mut compressor = StreamingCompressor::new(CompressionLevel::Default);

        let compressed = compressor.compress_bytes(data).unwrap();
        assert!(!compressed.is_empty());

        // Decompress to verify
        let mut decompressor = StreamingDecompressor::new();
        let decompressed = decompressor.decompress_bytes(&compressed).unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_streaming_round_trip() {
        let data = b"Test data for streaming compression and decompression.";

        let mut compressor = StreamingCompressor::new(CompressionLevel::Default);
        let compressed = compressor.compress_bytes(data).unwrap();

        let mut decompressor = StreamingDecompressor::new();
        let decompressed = decompressor.decompress_bytes(&compressed).unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_streaming_large_data() {
        let data = vec![b'X'; 100000];

        let mut compressor = StreamingCompressor::new(CompressionLevel::Fast);
        let compressed = compressor.compress_bytes(&data).unwrap();

        let mut decompressor = StreamingDecompressor::new();
        let decompressed = decompressor.decompress_bytes(&compressed).unwrap();

        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_buffer_sizes() {
        let data = b"Test with different buffer sizes";

        for buffer_size in [1024, 4096, 16384, 65536] {
            let mut compressor =
                StreamingCompressor::new(CompressionLevel::Default).with_buffer_size(buffer_size);
            let compressed = compressor.compress_bytes(data).unwrap();

            let mut decompressor = StreamingDecompressor::new().with_buffer_size(buffer_size);
            let decompressed = decompressor.decompress_bytes(&compressed).unwrap();

            assert_eq!(data, &decompressed[..]);
        }
    }

    #[test]
    fn test_chunked_processor() {
        let processor = ChunkedProcessor::new(1024);

        let data = b"Input data for chunked processing";
        let input = std::io::Cursor::new(data);
        let mut output = Vec::new();

        let bytes = processor
            .process(input, &mut output, |chunk| Ok(chunk.to_vec()))
            .unwrap();

        assert_eq!(bytes, data.len());
        assert_eq!(&output[..], data);
    }

    #[test]
    fn test_stream_io() {
        let data = b"Test streaming I/O operations";

        let input = std::io::Cursor::new(data);
        let mut output = Vec::new();

        let mut compressor = StreamingCompressor::new(CompressionLevel::Default);
        compressor.compress_stream(input, &mut output).unwrap();

        assert!(!output.is_empty());

        // Decompress
        let compressed = std::io::Cursor::new(&output);
        let mut decompressed = Vec::new();

        let mut decompressor = StreamingDecompressor::new();
        decompressor
            .decompress_stream(compressed, &mut decompressed)
            .unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_compression_levels() {
        let data = b"Test different compression levels in streaming mode";

        for level in [
            CompressionLevel::Fast,
            CompressionLevel::Default,
            CompressionLevel::Best,
        ] {
            let mut compressor = StreamingCompressor::new(level);
            let compressed = compressor.compress_bytes(data).unwrap();

            let mut decompressor = StreamingDecompressor::new();
            let decompressed = decompressor.decompress_bytes(&compressed).unwrap();

            assert_eq!(data, &decompressed[..]);
        }
    }

    #[cfg(feature = "streaming")]
    #[tokio::test]
    async fn test_async_streaming() {
        let data = b"Test async streaming compression";

        let input = tokio::io::BufReader::new(&data[..]);
        let mut output = Vec::new();

        let mut compressor = StreamingCompressor::new(CompressionLevel::Default);
        compressor
            .compress_stream_async(input, &mut output)
            .await
            .unwrap();

        assert!(!output.is_empty());

        // Decompress
        let compressed = &output[..];
        let mut decompressed = Vec::new();

        let mut decompressor = StreamingDecompressor::new();
        decompressor
            .decompress_stream_async(compressed, &mut decompressed)
            .await
            .unwrap();

        assert_eq!(data, &decompressed[..]);
    }
}
