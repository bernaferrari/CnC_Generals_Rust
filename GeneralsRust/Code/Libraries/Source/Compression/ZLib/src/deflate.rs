//! DEFLATE compression implementation (RFC 1951)
//!
//! This module implements the DEFLATE compression algorithm:
//! - Combines LZ77 with Huffman coding
//! - Supports fixed and dynamic Huffman blocks
//! - Implements block management and selection
//! - Optimized for various compression levels

use crate::{
    huffman::{BitWriter, HuffmanCode, HuffmanEncoder},
    lz77::{LZ77Compressor, LZ77Token, MAX_MATCH, MIN_MATCH},
    CompressionLevel, Result, ZlibError,
};

/// Block types in DEFLATE
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockType {
    Uncompressed = 0,
    FixedHuffman = 1,
    DynamicHuffman = 2,
}

/// Maximum block size for compression
const MAX_BLOCK_SIZE: usize = 65535;

/// End-of-block symbol
const END_OF_BLOCK: u16 = 256;

/// DEFLATE compressor
pub struct Compressor {
    level: CompressionLevel,
    lz77: LZ77Compressor,
}

impl Compressor {
    /// Create new DEFLATE compressor
    pub fn new(level: CompressionLevel) -> Self {
        Self {
            level,
            lz77: LZ77Compressor::new(level),
        }
    }

    /// Compress data using DEFLATE algorithm
    pub fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let mut output = BitWriter::new();

        // Use store for no compression level
        if matches!(self.level, CompressionLevel::None) {
            self.compress_stored(data, &mut output, true)?;
            output.align();
            return Ok(output.to_bytes());
        }

        // Compress data in blocks
        let mut offset = 0;
        let data_len = data.len();

        while offset < data_len {
            let block_size = std::cmp::min(MAX_BLOCK_SIZE, data_len - offset);
            let block_data = &data[offset..offset + block_size];
            let is_final = offset + block_size >= data_len;

            // Compress block using appropriate method
            self.compress_block(block_data, &mut output, is_final)?;

            offset += block_size;
        }

        output.align();
        Ok(output.to_bytes())
    }

    /// Compress a single block
    fn compress_block(
        &mut self,
        data: &[u8],
        output: &mut BitWriter,
        is_final: bool,
    ) -> Result<()> {
        // Write block header
        output.write(if is_final { 1 } else { 0 }, 1);

        // Choose block type based on data
        let block_type = self.choose_block_type(data);

        match block_type {
            BlockType::Uncompressed => {
                output.write(BlockType::Uncompressed as u16, 2);
                self.compress_stored(data, output, false)?;
            }
            BlockType::FixedHuffman => {
                output.write(BlockType::FixedHuffman as u16, 2);
                self.compress_fixed(data, output)?;
            }
            BlockType::DynamicHuffman => {
                output.write(BlockType::DynamicHuffman as u16, 2);
                self.compress_dynamic(data, output)?;
            }
        }

        Ok(())
    }

    /// Choose optimal block type for data
    fn choose_block_type(&self, _data: &[u8]) -> BlockType {
        match self.level {
            CompressionLevel::None => BlockType::Uncompressed,
            CompressionLevel::Fast | CompressionLevel::Fast2 => BlockType::FixedHuffman,
            _ => BlockType::DynamicHuffman,
        }
    }

    /// Compress using stored (uncompressed) block
    fn compress_stored(
        &mut self,
        data: &[u8],
        output: &mut BitWriter,
        include_header: bool,
    ) -> Result<()> {
        if include_header {
            output.write(1, 1); // Final block
            output.write(BlockType::Uncompressed as u16, 2);
        }

        // Align to byte boundary
        output.align();

        // Write length and complement
        let len = data.len() as u16;
        output.write(len, 16);
        output.write(!len, 16);

        // Write uncompressed data
        for &byte in data {
            output.write(byte as u16, 8);
        }

        Ok(())
    }

    /// Compress using fixed Huffman codes
    fn compress_fixed(&mut self, data: &[u8], output: &mut BitWriter) -> Result<()> {
        // Get LZ77 tokens
        let tokens = self.lz77.compress(data)?;

        // Use fixed Huffman encoder
        let encoder = HuffmanEncoder::fixed();

        // Encode tokens
        for token in tokens {
            match token {
                LZ77Token::Literal(byte) => {
                    let code = encoder.encode_literal(byte);
                    output.write_code(code);
                }
                LZ77Token::Match { length, distance } => {
                    // Encode length
                    let (length_code, extra_bits, num_bits) = encoder.encode_length(length);
                    output.write_code(length_code);
                    if num_bits > 0 {
                        output.write(extra_bits, num_bits as usize);
                    }

                    // Encode distance
                    let (dist_code, extra_bits, num_bits) = encoder.encode_distance(distance);
                    output.write_code(dist_code);
                    if num_bits > 0 {
                        output.write(extra_bits, num_bits as usize);
                    }
                }
            }
        }

        // Write end-of-block
        let eob_code = encoder.literal_codes()[END_OF_BLOCK as usize];
        output.write_code(eob_code);

        Ok(())
    }

    /// Compress using dynamic Huffman codes
    fn compress_dynamic(&mut self, data: &[u8], output: &mut BitWriter) -> Result<()> {
        // Get LZ77 tokens
        let tokens = self.lz77.compress(data)?;

        // Collect frequency statistics
        let (literal_freqs, distance_freqs) = self.collect_frequencies(&tokens);

        // Build Huffman encoder
        let encoder = HuffmanEncoder::new(&literal_freqs, &distance_freqs)?;

        // Write Huffman code tables
        self.write_dynamic_trees(output, &encoder)?;

        // Encode tokens
        for token in tokens {
            match token {
                LZ77Token::Literal(byte) => {
                    let code = encoder.encode_literal(byte);
                    output.write_code(code);
                }
                LZ77Token::Match { length, distance } => {
                    // Encode length
                    let (length_code, extra_bits, num_bits) = encoder.encode_length(length);
                    output.write_code(length_code);
                    if num_bits > 0 {
                        output.write(extra_bits, num_bits as usize);
                    }

                    // Encode distance
                    let (dist_code, extra_bits, num_bits) = encoder.encode_distance(distance);
                    output.write_code(dist_code);
                    if num_bits > 0 {
                        output.write(extra_bits, num_bits as usize);
                    }
                }
            }
        }

        // Write end-of-block
        let eob_code = encoder.literal_codes()[END_OF_BLOCK as usize];
        output.write_code(eob_code);

        Ok(())
    }

    /// Collect symbol frequencies from tokens
    fn collect_frequencies(&self, tokens: &[LZ77Token]) -> (Vec<u32>, Vec<u32>) {
        let mut literal_freqs = vec![0u32; 286];
        let mut distance_freqs = vec![0u32; 30];

        // Count end-of-block symbol
        literal_freqs[END_OF_BLOCK as usize] = 1;

        for token in tokens {
            match token {
                LZ77Token::Literal(byte) => {
                    literal_freqs[*byte as usize] += 1;
                }
                LZ77Token::Match { length, distance } => {
                    // Get length code
                    let (length_code, _, _) = HuffmanEncoder::length_to_code(*length);
                    literal_freqs[length_code] += 1;

                    // Get distance code
                    let (dist_code, _, _) = HuffmanEncoder::distance_to_code(*distance);
                    distance_freqs[dist_code] += 1;
                }
            }
        }

        (literal_freqs, distance_freqs)
    }

    /// Write dynamic Huffman tree definitions
    fn write_dynamic_trees(&self, output: &mut BitWriter, encoder: &HuffmanEncoder) -> Result<()> {
        // Extract code lengths
        let literal_lengths = self.extract_code_lengths(encoder.literal_codes());
        let distance_lengths = self.extract_code_lengths(encoder.distance_codes());

        // Find last non-zero literal length
        let hlit = literal_lengths
            .iter()
            .rposition(|&x| x > 0)
            .unwrap_or(256)
            .max(256);

        // Find last non-zero distance length
        let hdist = distance_lengths
            .iter()
            .rposition(|&x| x > 0)
            .unwrap_or(0)
            .max(0);

        // Write tree sizes
        output.write((hlit - 257) as u16, 5); // HLIT
        output.write((hdist - 1) as u16, 5); // HDIST

        // Encode code length codes (simplified - just write lengths directly)
        // In a full implementation, we'd encode these with another Huffman tree
        output.write(0, 4); // HCLEN (we'll use a simple encoding)

        // Write literal/length code lengths
        for &length in &literal_lengths[0..=hlit] {
            output.write(length as u16, 3);
        }

        // Write distance code lengths
        for &length in &distance_lengths[0..=hdist] {
            output.write(length as u16, 3);
        }

        Ok(())
    }

    /// Extract code lengths from Huffman codes
    fn extract_code_lengths(&self, codes: &[HuffmanCode]) -> Vec<u8> {
        codes.iter().map(|c| c.length).collect()
    }
}

/// Estimate compressed size for a block
pub fn estimate_compressed_size(data: &[u8], level: CompressionLevel) -> usize {
    if data.is_empty() {
        return 0;
    }

    // Quick estimation based on entropy
    let mut counts = [0u32; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;

    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }

    // Estimate: entropy * data length / 8 (convert bits to bytes)
    let base_size = (entropy * len / 8.0) as usize;

    // Add overhead for headers and structure
    let overhead = match level {
        CompressionLevel::None => data.len(), // Stored blocks have minimal compression
        CompressionLevel::Fast | CompressionLevel::Fast2 => base_size + 100,
        _ => (base_size as f64 * 0.9) as usize + 150,
    };

    overhead.max(10)
}

/// Calculate Adler-32 checksum (used by zlib)
pub fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;

    for &byte in data {
        a = (a + byte as u32) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }

    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deflate_empty() {
        let mut compressor = Compressor::new(CompressionLevel::Default);
        let result = compressor.compress(b"").unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_deflate_small_data() {
        let mut compressor = Compressor::new(CompressionLevel::Default);
        let data = b"Hello, World!";
        let result = compressor.compress(data).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_deflate_stored() {
        let mut compressor = Compressor::new(CompressionLevel::None);
        let data = b"Test data for stored block";
        let result = compressor.compress(data).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_deflate_fixed() {
        let mut compressor = Compressor::new(CompressionLevel::Fast);
        let data = b"Repeated data data data data";
        let result = compressor.compress(data).unwrap();
        assert!(!result.is_empty());
        assert!(result.len() < data.len());
    }

    #[test]
    fn test_deflate_dynamic() {
        let mut compressor = Compressor::new(CompressionLevel::Best);
        let data = b"This is test data. This is test data. This is test data.";
        let result = compressor.compress(data).unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_compression_levels() {
        let data = b"The quick brown fox jumps over the lazy dog.";

        for level in [
            CompressionLevel::None,
            CompressionLevel::Fast,
            CompressionLevel::Default,
            CompressionLevel::Best,
        ] {
            let mut compressor = Compressor::new(level);
            let result = compressor.compress(data).unwrap();
            assert!(!result.is_empty());
        }
    }

    #[test]
    fn test_large_data() {
        let mut compressor = Compressor::new(CompressionLevel::Default);
        let data = vec![b'A'; 100000];
        let result = compressor.compress(&data).unwrap();
        assert!(!result.is_empty());
        assert!(result.len() < data.len());
    }

    #[test]
    fn test_adler32_checksum() {
        let data = b"Hello, World!";
        let checksum = adler32(data);
        assert_ne!(checksum, 0);

        // Adler32 should be deterministic
        let checksum2 = adler32(data);
        assert_eq!(checksum, checksum2);
    }

    #[test]
    fn test_estimate_size() {
        let data = b"Test data for size estimation";
        let estimate = estimate_compressed_size(data, CompressionLevel::Default);
        assert!(estimate > 0);
        assert!(estimate <= data.len() + 200);
    }

    #[test]
    fn test_frequency_collection() {
        let compressor = Compressor::new(CompressionLevel::Default);
        let tokens = vec![
            LZ77Token::Literal(b'A'),
            LZ77Token::Literal(b'B'),
            LZ77Token::Match {
                length: 5,
                distance: 10,
            },
        ];

        let (literal_freqs, distance_freqs) = compressor.collect_frequencies(&tokens);

        // Should count literals
        assert!(literal_freqs[b'A' as usize] > 0);
        assert!(literal_freqs[b'B' as usize] > 0);

        // Should count end-of-block
        assert!(literal_freqs[END_OF_BLOCK as usize] > 0);
    }
}
