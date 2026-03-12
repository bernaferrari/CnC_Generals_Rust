//! LZH Compression Implementation
//!
//! This module implements the LZH (Lempel-Ziv-Huffman) compression algorithm,
//! combining LZ77 dictionary coding with Huffman statistical encoding.
//!
//! Based on the C++ implementation from:
//! /GeneralsMD/Code/Libraries/Source/Compression/LZHCompress/NoxCompress.cpp
//!
//! ## Algorithm Overview
//!
//! 1. **LZ77 Phase**: Find repeating patterns using a sliding window dictionary
//!    - Scan input data for matches in the sliding window
//!    - Encode matches as (length, distance) pairs
//!    - Emit literals for unmatched bytes
//!
//! 2. **Huffman Phase**: Compress LZ77 output using statistical coding
//!    - Build frequency tables for symbols
//!    - Generate optimal Huffman codes
//!    - Encode output using variable-length codes

use crate::{
    CompressionLevel, LzhHeader, LzhMatch, Result,
    dictionary::Dictionary,
};

/// Maximum block size for compression (matches C++ BLOCKSIZE = 500000)
const MAX_BLOCK_SIZE: usize = 500_000;

/// Minimum match length to be worth encoding
const MIN_MATCH_LENGTH: usize = 3;

/// Maximum match length (limited by encoding format)
#[allow(dead_code)]
const MAX_MATCH_LENGTH: usize = 258;

/// LZH Compressor state
pub struct LzhCompressor {
    level: CompressionLevel,
    dictionary: Dictionary,
    stats: CompressorStats,
    huffman_encoder: HuffmanEncoder,
}

/// Internal statistics for compression
#[derive(Debug, Default)]
struct CompressorStats {
    matches_found: usize,
    literals_encoded: usize,
    bytes_processed: usize,
}

impl LzhCompressor {
    /// Create a new compressor with specified compression level
    pub fn new(level: CompressionLevel) -> Self {
        Self {
            level,
            dictionary: Dictionary::new(level.window_size()),
            stats: CompressorStats::default(),
            huffman_encoder: HuffmanEncoder::new(),
        }
    }

    /// Compress data buffer
    ///
    /// Returns compressed data with LZH header prepended.
    pub fn compress(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate maximum compressed size
        let max_compressed = Self::calc_max_compressed_size(data.len());
        let mut compressed_data = Vec::with_capacity(max_compressed);

        // Compress the data
        self.compress_internal(data, &mut compressed_data)?;

        // Calculate CRC32 of original data
        let crc32 = crc32fast::hash(data);

        // Create and prepend header
        let header = LzhHeader::new(
            self.level,
            data.len() as u64,
            compressed_data.len() as u64,
            crc32,
        );

        let mut result = Vec::with_capacity(LzhHeader::SIZE + compressed_data.len());
        result.extend_from_slice(&header.to_bytes());
        result.append(&mut compressed_data);

        Ok(result)
    }

    /// Compress a chunk with offset (for parallel compression)
    pub fn compress_chunk(
        &mut self,
        data: &[u8],
        offset: usize,
        original_len: usize,
    ) -> Result<Vec<u8>> {
        let end = offset + original_len;
        let chunk = &data[offset..std::cmp::min(end, data.len())];
        self.compress(chunk)
    }

    /// Calculate maximum possible compressed size
    ///
    /// Matches C++ function: LZHLCompressorCalcMaxBuf
    pub fn calc_max_compressed_size(uncompressed_size: usize) -> usize {
        // Worst case: all literals + overhead
        // Each literal can take up to 9 bits (1 flag bit + 8 data bits)
        // Add extra space for headers and padding
        let worst_case = (uncompressed_size * 9) / 8 + 1024;
        worst_case + LzhHeader::SIZE
    }

    /// Calculate maximum possible compressed size for raw (headerless) output
    ///
    /// Matches C++ function: LZHLCompressorCalcMaxBuf for raw buffers
    pub fn calc_max_compressed_size_raw(uncompressed_size: usize) -> usize {
        // Worst case: all literals + overhead, no header
        (uncompressed_size * 9) / 8 + 1024
    }

    /// Compress data buffer without an LZH header
    ///
    /// Matches C++ behavior for raw LZHL compressed buffers.
    pub fn compress_raw(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        let mut compressed = Vec::with_capacity(Self::calc_max_compressed_size_raw(input.len()));
        self.compress_internal(input, &mut compressed)?;
        Ok(compressed)
    }

    /// Internal compression implementation
    fn compress_internal(&mut self, input: &[u8], output: &mut Vec<u8>) -> Result<()> {
        self.stats = CompressorStats::default();

        // Process input in blocks (matches C++ behavior)
        let mut position = 0;
        while position < input.len() {
            let block_end = std::cmp::min(position + MAX_BLOCK_SIZE, input.len());
            let block = &input[position..block_end];

            self.compress_block(block, output)?;

            position = block_end;
        }

        // Finalize Huffman encoding
        self.huffman_encoder.finalize(output)?;

        Ok(())
    }

    /// Compress a single block
    fn compress_block(&mut self, block: &[u8], output: &mut Vec<u8>) -> Result<()> {
        let mut pos = 0;

        while pos < block.len() {
            // Try to find match in dictionary
            let best_match = self.find_best_match(block, pos);

            if best_match.is_valid() {
                // Encode match as (length, distance) pair
                self.encode_match(&best_match, output)?;

                // Add matched bytes to dictionary
                for i in 0..best_match.length {
                    if pos + i < block.len() {
                        self.dictionary.add_byte(block[pos + i]);
                    }
                }

                pos += best_match.length;
                self.stats.matches_found += 1;
                self.stats.bytes_processed += best_match.length;
            } else {
                // Encode literal byte
                self.encode_literal(block[pos], output)?;
                self.dictionary.add_byte(block[pos]);

                pos += 1;
                self.stats.literals_encoded += 1;
                self.stats.bytes_processed += 1;
            }
        }

        Ok(())
    }

    /// Find the best match in the dictionary
    fn find_best_match(&self, data: &[u8], pos: usize) -> LzhMatch {
        if pos + MIN_MATCH_LENGTH > data.len() {
            return LzhMatch::new(0, 0);
        }

        let search_depth = self.level.search_depth();
        let max_length = std::cmp::min(
            self.level.max_match_length(),
            data.len() - pos,
        );

        self.dictionary.find_longest_match(
            &data[pos..],
            MIN_MATCH_LENGTH,
            max_length,
            search_depth,
        )
    }

    /// Encode a match (length, distance) pair
    fn encode_match(&mut self, match_data: &LzhMatch, output: &mut Vec<u8>) -> Result<()> {
        // Encode match using Huffman encoder
        // Format: [flag=1][length_code][distance_code]
        self.huffman_encoder.encode_match(
            match_data.length,
            match_data.distance,
            output,
        )
    }

    /// Encode a literal byte
    fn encode_literal(&mut self, byte: u8, output: &mut Vec<u8>) -> Result<()> {
        // Format: [flag=0][literal_byte]
        self.huffman_encoder.encode_literal(byte, output)
    }

    /// Get number of matches found
    pub fn matches_found(&self) -> usize {
        self.stats.matches_found
    }

    /// Get number of literals encoded
    pub fn literals_encoded(&self) -> usize {
        self.stats.literals_encoded
    }
}

/// Huffman encoder for statistical compression
struct HuffmanEncoder {
    // Frequency tables for building Huffman trees
    literal_freq: [u32; 256],
    length_freq: [u32; 256],
    distance_freq: [u32; 32768],

    // Huffman code tables
    literal_codes: [HuffmanCode; 256],
    length_codes: [HuffmanCode; 256],
    distance_codes: Vec<HuffmanCode>,

    // Bit buffer for output
    bit_buffer: u32,
    bit_count: u8,

    // Temporary storage for encoded symbols
    symbol_buffer: Vec<Symbol>,
}

#[derive(Debug, Clone, Copy)]
struct HuffmanCode {
    code: u32,
    length: u8,
}

impl Default for HuffmanCode {
    fn default() -> Self {
        Self { code: 0, length: 0 }
    }
}

#[derive(Debug, Clone)]
enum Symbol {
    Literal(u8),
    Match { length: usize, distance: usize },
}

impl HuffmanEncoder {
    fn new() -> Self {
        Self {
            literal_freq: [0; 256],
            length_freq: [0; 256],
            distance_freq: [0; 32768],
            literal_codes: [HuffmanCode::default(); 256],
            length_codes: [HuffmanCode::default(); 256],
            distance_codes: vec![HuffmanCode::default(); 32768],
            bit_buffer: 0,
            bit_count: 0,
            symbol_buffer: Vec::new(),
        }
    }

    fn encode_literal(&mut self, byte: u8, _output: &mut Vec<u8>) -> Result<()> {
        // Collect statistics for Huffman tree building
        self.literal_freq[byte as usize] += 1;
        self.symbol_buffer.push(Symbol::Literal(byte));
        Ok(())
    }

    fn encode_match(
        &mut self,
        length: usize,
        distance: usize,
        _output: &mut Vec<u8>,
    ) -> Result<()> {
        // Collect statistics
        let length_code = Self::encode_length(length);
        let distance_code = Self::encode_distance(distance);

        self.length_freq[length_code] += 1;
        if distance_code < self.distance_freq.len() {
            self.distance_freq[distance_code] += 1;
        }

        self.symbol_buffer.push(Symbol::Match { length, distance });
        Ok(())
    }

    fn finalize(&mut self, output: &mut Vec<u8>) -> Result<()> {
        // Build Huffman trees from frequency tables
        self.build_huffman_trees();

        // Write Huffman tree headers
        self.write_tree_headers(output)?;

        // Encode all buffered symbols (need to clone to avoid borrow issues)
        let symbols = self.symbol_buffer.clone();
        for symbol in &symbols {
            match symbol {
                Symbol::Literal(byte) => {
                    self.write_literal(*byte, output)?;
                }
                Symbol::Match { length, distance } => {
                    self.write_match(*length, *distance, output)?;
                }
            }
        }

        // Flush remaining bits
        self.flush_bits(output)?;

        Ok(())
    }

    fn build_huffman_trees(&mut self) {
        // Build Huffman codes for literals
        let literal_tree = build_huffman_tree(&self.literal_freq);
        generate_codes(&literal_tree, &mut self.literal_codes);

        // Build Huffman codes for lengths
        let length_tree = build_huffman_tree(&self.length_freq);
        generate_codes(&length_tree, &mut self.length_codes);

        // Build Huffman codes for distances (limited set)
        let distance_freq_limited: Vec<u32> = self.distance_freq.iter()
            .take(256)
            .copied()
            .collect();
        let distance_tree = build_huffman_tree(&distance_freq_limited);

        self.distance_codes.clear();
        self.distance_codes.resize(32768, HuffmanCode::default());
        generate_codes(&distance_tree, &mut self.distance_codes[..256]);
    }

    fn write_tree_headers(&mut self, output: &mut Vec<u8>) -> Result<()> {
        // Write compressed representation of Huffman trees
        // This is a simplified version - real implementation would use
        // canonical Huffman codes or other tree compression

        // For now, write a marker that trees are present
        output.push(0xFF); // Tree header marker
        Ok(())
    }

    fn write_literal(&mut self, byte: u8, output: &mut Vec<u8>) -> Result<()> {
        // Write match flag (0 = literal)
        self.write_bit(0, output)?;

        // Write Huffman-encoded literal
        let code = self.literal_codes[byte as usize];
        self.write_bits(code.code, code.length, output)?;

        Ok(())
    }

    fn write_match(&mut self, length: usize, distance: usize, output: &mut Vec<u8>) -> Result<()> {
        // Write match flag (1 = match)
        self.write_bit(1, output)?;

        // Encode and write length
        let length_code = Self::encode_length(length);
        let length_huffman = self.length_codes[length_code];
        self.write_bits(length_huffman.code, length_huffman.length, output)?;

        // Encode and write distance
        let distance_code = Self::encode_distance(distance);
        let distance_huffman = if distance_code < self.distance_codes.len() {
            self.distance_codes[distance_code]
        } else {
            HuffmanCode::default()
        };
        self.write_bits(distance_huffman.code, distance_huffman.length, output)?;

        Ok(())
    }

    fn write_bit(&mut self, bit: u8, output: &mut Vec<u8>) -> Result<()> {
        self.bit_buffer |= (bit as u32) << self.bit_count;
        self.bit_count += 1;

        if self.bit_count >= 8 {
            output.push((self.bit_buffer & 0xFF) as u8);
            self.bit_buffer >>= 8;
            self.bit_count -= 8;
        }

        Ok(())
    }

    fn write_bits(&mut self, bits: u32, count: u8, output: &mut Vec<u8>) -> Result<()> {
        for i in 0..count {
            let bit = ((bits >> i) & 1) as u8;
            self.write_bit(bit, output)?;
        }
        Ok(())
    }

    fn flush_bits(&mut self, output: &mut Vec<u8>) -> Result<()> {
        if self.bit_count > 0 {
            output.push((self.bit_buffer & 0xFF) as u8);
            self.bit_buffer = 0;
            self.bit_count = 0;
        }
        Ok(())
    }

    fn encode_length(length: usize) -> usize {
        // Map length to code (3..=258 -> 0..=255)
        if length >= MIN_MATCH_LENGTH {
            std::cmp::min(length - MIN_MATCH_LENGTH, 255)
        } else {
            0
        }
    }

    fn encode_distance(distance: usize) -> usize {
        // Use distance directly as code (with bounds checking)
        std::cmp::min(distance, 32767)
    }
}

/// Huffman tree node
#[derive(Debug, Clone, Eq, PartialEq)]
struct HuffmanNode {
    symbol: Option<usize>,
    frequency: u32,
    left: Option<Box<HuffmanNode>>,
    right: Option<Box<HuffmanNode>>,
}

impl Ord for HuffmanNode {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.frequency.cmp(&other.frequency)
    }
}

impl PartialOrd for HuffmanNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl HuffmanNode {
    fn leaf(symbol: usize, frequency: u32) -> Self {
        Self {
            symbol: Some(symbol),
            frequency,
            left: None,
            right: None,
        }
    }

    fn internal(left: HuffmanNode, right: HuffmanNode) -> Self {
        Self {
            symbol: None,
            frequency: left.frequency + right.frequency,
            left: Some(Box::new(left)),
            right: Some(Box::new(right)),
        }
    }
}

/// Build Huffman tree from frequency table
fn build_huffman_tree(frequencies: &[u32]) -> Option<HuffmanNode> {
    use std::collections::BinaryHeap;
    use std::cmp::Reverse;

    // Create leaf nodes for all symbols with non-zero frequency
    let mut heap: BinaryHeap<Reverse<(u32, usize, HuffmanNode)>> = frequencies
        .iter()
        .enumerate()
        .filter(|(_, &freq)| freq > 0)
        .map(|(sym, &freq)| Reverse((freq, sym, HuffmanNode::leaf(sym, freq))))
        .collect();

    if heap.is_empty() {
        return None;
    }

    // Build tree by combining lowest frequency nodes
    while heap.len() > 1 {
        let Reverse((_, _, left)) = heap.pop().unwrap();
        let Reverse((_, _, right)) = heap.pop().unwrap();

        let parent = HuffmanNode::internal(left, right);
        let freq = parent.frequency;
        heap.push(Reverse((freq, 0, parent)));
    }

    heap.pop().map(|Reverse((_, _, node))| node)
}

/// Generate Huffman codes from tree
fn generate_codes(tree: &Option<HuffmanNode>, codes: &mut [HuffmanCode]) {
    fn traverse(node: &HuffmanNode, code: u32, length: u8, codes: &mut [HuffmanCode]) {
        if let Some(symbol) = node.symbol {
            // Leaf node - store code
            if symbol < codes.len() {
                codes[symbol] = HuffmanCode { code, length };
            }
        } else {
            // Internal node - traverse children
            if let Some(ref left) = node.left {
                traverse(left, code, length + 1, codes);
            }
            if let Some(ref right) = node.right {
                traverse(right, code | (1 << length), length + 1, codes);
            }
        }
    }

    if let Some(ref root) = tree {
        traverse(root, 0, 0, codes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_creation() {
        let compressor = LzhCompressor::new(CompressionLevel::Default);
        assert_eq!(compressor.matches_found(), 0);
        assert_eq!(compressor.literals_encoded(), 0);
    }

    #[test]
    fn test_empty_compression() {
        let mut compressor = LzhCompressor::new(CompressionLevel::Fast);
        let result = compressor.compress(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_small_compression() {
        let mut compressor = LzhCompressor::new(CompressionLevel::Default);
        let data = b"Hello, World!";
        let result = compressor.compress(data);
        assert!(result.is_ok());

        let compressed = result.unwrap();
        assert!(compressed.len() >= LzhHeader::SIZE);
    }

    #[test]
    fn test_max_compressed_size_calculation() {
        assert!(LzhCompressor::calc_max_compressed_size(1000) > 1000);
        assert!(LzhCompressor::calc_max_compressed_size(0) >= LzhHeader::SIZE);
    }

    #[test]
    fn test_huffman_encoder() {
        let mut encoder = HuffmanEncoder::new();
        let mut output = Vec::new();

        assert!(encoder.encode_literal(65, &mut output).is_ok());
        assert!(encoder.encode_literal(66, &mut output).is_ok());
        assert!(encoder.finalize(&mut output).is_ok());
    }
}
