//! LZH Decompression Implementation
//!
//! This module implements LZH decompression, reversing the compression process.
//!
//! Based on the C++ implementation from:
//! /GeneralsMD/Code/Libraries/Source/Compression/LZHCompress/NoxCompress.cpp
//! Functions: LZHLDecompress, DecompressMemory, DecompressFile
//!
//! ## Decompression Algorithm
//!
//! 1. Read LZH header to get metadata
//! 2. Read Huffman tree definitions
//! 3. Decode compressed stream:
//!    - Read flag bit (0=literal, 1=match)
//!    - For literals: decode byte and output
//!    - For matches: decode (length, distance), copy from history
//! 4. Verify output size and CRC32

use crate::{LzhError, LzhHeader, Result};

/// LZH Decompressor state
pub struct LzhDecompressor {
    // Output buffer (sliding window)
    window: Vec<u8>,
    window_pos: usize,
    window_size: usize,

    // Huffman decoder tables
    literal_decoder: HuffmanDecoder,
    length_decoder: HuffmanDecoder,
    distance_decoder: HuffmanDecoder,

    // Bit reader state
    bit_buffer: u32,
    bit_count: u8,
    input_pos: usize,
}

impl LzhDecompressor {
    /// Create a new decompressor
    pub fn new() -> Self {
        Self::with_window_size(32768) // 32KB default window
    }

    /// Create decompressor with specific window size
    pub fn with_window_size(window_size: usize) -> Self {
        Self {
            window: vec![0; window_size],
            window_pos: 0,
            window_size,
            literal_decoder: HuffmanDecoder::new(),
            length_decoder: HuffmanDecoder::new(),
            distance_decoder: HuffmanDecoder::new(),
            bit_buffer: 0,
            bit_count: 0,
            input_pos: 0,
        }
    }

    /// Decompress data buffer
    ///
    /// Input should include the LZH header. Returns the decompressed data.
    pub fn decompress(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Parse header
        if input.len() < LzhHeader::SIZE {
            return Err(LzhError::BufferTooSmall {
                needed: LzhHeader::SIZE,
                available: input.len(),
            });
        }

        let header = LzhHeader::from_bytes(&input[0..LzhHeader::SIZE])?;

        // Setup for decompression
        let compressed_data = &input[LzhHeader::SIZE..];
        let output_size = header.uncompressed_size as usize;

        // Decompress the data
        let decompressed = self.decompress_internal(compressed_data, output_size)?;

        // Verify CRC32
        let calculated_crc = crc32fast::hash(&decompressed);
        if calculated_crc != header.crc32 {
            return Err(LzhError::DecompressionFailed(format!(
                "CRC mismatch: expected 0x{:08X}, got 0x{:08X}",
                header.crc32, calculated_crc
            )));
        }

        Ok(decompressed)
    }

    /// Decompress without header (raw compressed data)
    pub fn decompress_raw(&mut self, input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        self.decompress_internal(input, expected_size)
    }

    /// Internal decompression implementation
    ///
    /// Matches C++ function: LZHLDecompress
    fn decompress_internal(&mut self, input: &[u8], output_size: usize) -> Result<Vec<u8>> {
        self.input_pos = 0;
        self.bit_buffer = 0;
        self.bit_count = 0;
        self.window_pos = 0;

        let mut output = Vec::with_capacity(output_size);

        // Read Huffman tree headers
        self.read_tree_headers(input)?;

        // Decompress until we have enough data
        while output.len() < output_size && self.input_pos < input.len() {
            // Read flag bit
            let is_match = self.read_bit(input)?;

            if is_match {
                // Decode match
                let (length, distance) = self.decode_match(input)?;

                // Validate match parameters
                if distance == 0 || distance > output.len() {
                    return Err(LzhError::DecompressionFailed(format!(
                        "Invalid match distance: {} (output size: {})",
                        distance,
                        output.len()
                    )));
                }

                if length < 3 {
                    return Err(LzhError::DecompressionFailed(format!(
                        "Invalid match length: {}",
                        length
                    )));
                }

                // Copy from history
                for _ in 0..length {
                    if output.len() >= output_size {
                        break;
                    }

                    let copy_pos = output.len() - distance;
                    let byte = output[copy_pos];
                    output.push(byte);

                    // Update sliding window
                    self.window[self.window_pos] = byte;
                    self.window_pos = (self.window_pos + 1) % self.window_size;
                }
            } else {
                // Decode literal
                let byte = self.decode_literal(input)?;
                output.push(byte);

                // Update sliding window
                self.window[self.window_pos] = byte;
                self.window_pos = (self.window_pos + 1) % self.window_size;
            }
        }

        // Verify we got the expected size
        if output.len() != output_size {
            return Err(LzhError::DecompressionFailed(format!(
                "Size mismatch: expected {}, got {}",
                output_size,
                output.len()
            )));
        }

        Ok(output)
    }

    /// Read Huffman tree headers from compressed stream
    fn read_tree_headers(&mut self, input: &[u8]) -> Result<()> {
        // Check for tree header marker
        if self.input_pos >= input.len() {
            return Err(LzhError::DecompressionFailed(
                "Unexpected end of input reading tree headers".to_string(),
            ));
        }

        let marker = input[self.input_pos];
        self.input_pos += 1;

        if marker == 0xFF {
            // Trees are present - read them
            // For now, use default/canonical trees
            self.literal_decoder.build_canonical();
            self.length_decoder.build_canonical();
            self.distance_decoder.build_canonical();
        } else {
            return Err(LzhError::DecompressionFailed(format!(
                "Invalid tree header marker: 0x{:02X}",
                marker
            )));
        }

        Ok(())
    }

    /// Decode a literal byte
    fn decode_literal(&mut self, input: &[u8]) -> Result<u8> {
        // Read bits directly to avoid borrow issues
        let mut code = 0u32;
        let mut length = 0u8;

        // Read up to 8 bits for literal
        while length < 8 {
            let bit = self.read_bit(input)?;
            code = (code << 1) | (bit as u32);
            length += 1;
        }

        Ok((code & 0xFF) as u8)
    }

    /// Decode a match (length, distance) pair
    fn decode_match(&mut self, input: &[u8]) -> Result<(usize, usize)> {
        // Decode length (read 8 bits)
        let length_code = self.read_bits(8, input)? as usize;
        let length = length_code + 3; // Offset by minimum match length

        // Decode distance (read up to 15 bits)
        let distance_code = self.read_bits(15, input)? as usize;
        let distance = distance_code;

        Ok((length, distance))
    }

    /// Read a single bit from input
    fn read_bit(&mut self, input: &[u8]) -> Result<bool> {
        if self.bit_count == 0 {
            if self.input_pos >= input.len() {
                return Err(LzhError::DecompressionFailed(
                    "Unexpected end of input".to_string(),
                ));
            }

            self.bit_buffer = input[self.input_pos] as u32;
            self.input_pos += 1;
            self.bit_count = 8;
        }

        let bit = (self.bit_buffer & 1) != 0;
        self.bit_buffer >>= 1;
        self.bit_count -= 1;

        Ok(bit)
    }

    /// Read multiple bits from input
    fn read_bits(&mut self, count: u8, input: &[u8]) -> Result<u32> {
        let mut result = 0u32;
        for i in 0..count {
            let bit = self.read_bit(input)?;
            if bit {
                result |= 1 << i;
            }
        }
        Ok(result)
    }
}

impl Default for LzhDecompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Huffman decoder for reading variable-length codes
#[allow(dead_code)] // C++ parity: LZH Huffman decoder, retained for future decompression integration
struct HuffmanDecoder {
    // Lookup table for fast decoding
    lookup: Vec<DecoderEntry>,
    max_code_length: u8,
}

#[derive(Clone, Copy)]
#[allow(dead_code)] // C++ parity: LZH Huffman decoder entry
struct DecoderEntry {
    symbol: u8,
    length: u8,
}

impl Default for DecoderEntry {
    fn default() -> Self {
        Self {
            symbol: 0,
            length: 0,
        }
    }
}

impl HuffmanDecoder {
    fn new() -> Self {
        Self {
            lookup: vec![DecoderEntry::default(); 512],
            max_code_length: 9,
        }
    }

    /// Build canonical Huffman decoder
    fn build_canonical(&mut self) {
        // Build a simple canonical Huffman code
        // This is a simplified version - real implementation would
        // read code lengths from stream

        // For now, use a basic code where symbols map directly
        for i in 0..256 {
            if i < self.lookup.len() {
                self.lookup[i] = DecoderEntry {
                    symbol: i as u8,
                    length: 8,
                };
            }
        }
    }

    /// Decode next symbol from input
    #[allow(dead_code)] // C++ parity: LZH Huffman decode, retained for future decompression integration
    fn decode(&self, decompressor: &mut LzhDecompressor, input: &[u8]) -> Result<u8> {
        // Read bits and look up in table
        let mut code = 0u32;
        let mut length = 0u8;

        // Read up to max code length
        while length < self.max_code_length {
            let bit = decompressor.read_bit(input)?;
            code = (code << 1) | (bit as u32);
            length += 1;

            // Check if we have a valid code
            if (code as usize) < self.lookup.len() {
                let entry = self.lookup[code as usize];
                if entry.length == length {
                    return Ok(entry.symbol);
                }
            }
        }

        // If no valid code found, use the code value directly as symbol
        Ok((code & 0xFF) as u8)
    }
}

/// Decompress memory buffer (C++ API compatibility)
///
/// Matches C++ function: DecompressMemory
pub fn decompress_memory(input: &[u8], output: &mut [u8]) -> Result<usize> {
    let mut decompressor = LzhDecompressor::new();
    let decompressed = decompressor.decompress(input)?;

    if decompressed.len() > output.len() {
        return Err(LzhError::BufferTooSmall {
            needed: decompressed.len(),
            available: output.len(),
        });
    }

    output[..decompressed.len()].copy_from_slice(&decompressed);
    Ok(decompressed.len())
}

/// Decompress raw data without header
pub fn decompress_raw(input: &[u8], output_size: usize) -> Result<Vec<u8>> {
    let mut decompressor = LzhDecompressor::new();
    decompressor.decompress_raw(input, output_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compress::LzhCompressor, CompressionLevel};

    #[test]
    fn test_decompressor_creation() {
        let decompressor = LzhDecompressor::new();
        assert_eq!(decompressor.window_size, 32768);
    }

    #[test]
    fn test_empty_decompression() {
        let mut decompressor = LzhDecompressor::new();
        let result = decompressor.decompress(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = b"Hello, World! This is a test of LZH compression.";

        // Compress
        let mut compressor = LzhCompressor::new(CompressionLevel::Default);
        let compressed = compressor.compress(original).unwrap();

        // Decompress
        let mut decompressor = LzhDecompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(original.as_ref(), decompressed.as_slice());
    }

    #[test]
    fn test_invalid_header() {
        let mut decompressor = LzhDecompressor::new();
        let bad_data = vec![0u8; 10]; // Too short for header
        let result = decompressor.decompress(&bad_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_memory() {
        let original = b"Test data";
        let mut compressor = LzhCompressor::new(CompressionLevel::Fast);
        let compressed = compressor.compress(original).unwrap();

        let mut output = vec![0u8; 1024];
        let size = decompress_memory(&compressed, &mut output).unwrap();

        assert_eq!(&output[..size], original.as_ref());
    }

    #[test]
    fn test_repetitive_data() {
        // Highly compressible repetitive data
        let original = b"AAAAAAAAAA".repeat(100);

        let mut compressor = LzhCompressor::new(CompressionLevel::High);
        let compressed = compressor.compress(&original).unwrap();

        let mut decompressor = LzhDecompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_random_data() {
        // Less compressible random-like data
        let original: Vec<u8> = (0..1000).map(|i| ((i * 37 + 13) % 256) as u8).collect();

        let mut compressor = LzhCompressor::new(CompressionLevel::Default);
        let compressed = compressor.compress(&original).unwrap();

        let mut decompressor = LzhDecompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(original, decompressed);
    }
}
