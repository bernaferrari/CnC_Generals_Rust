//! Huffman coding implementation for DEFLATE compression
//!
//! This module implements canonical Huffman coding used in DEFLATE:
//! - Dynamic Huffman code generation
//! - Fixed Huffman tables (RFC 1951)
//! - Bit-level encoding/decoding
//! - Code length limiting (max 15 bits)

use crate::{Result, ZlibError};
use bit_vec::BitVec;
use std::cmp::Ordering;

/// Maximum code length for Huffman codes (DEFLATE spec)
pub const MAX_BITS: usize = 15;

/// Maximum number of literal/length codes
pub const MAX_LITERALS: usize = 286;

/// Maximum number of distance codes
pub const MAX_DISTANCES: usize = 30;

/// Maximum number of code length codes
pub const MAX_CODE_LENGTHS: usize = 19;

/// Huffman code tree node
#[derive(Debug, Clone)]
struct HuffmanNode {
    symbol: Option<u16>,
    frequency: u32,
    left: Option<Box<HuffmanNode>>,
    right: Option<Box<HuffmanNode>>,
}

impl HuffmanNode {
    fn leaf(symbol: u16, frequency: u32) -> Self {
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

    fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }
}

impl PartialEq for HuffmanNode {
    fn eq(&self, other: &Self) -> bool {
        self.frequency == other.frequency
    }
}

impl Eq for HuffmanNode {}

impl PartialOrd for HuffmanNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HuffmanNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other.frequency.cmp(&self.frequency)
    }
}

/// Huffman code representation
#[derive(Debug, Clone, Copy)]
pub struct HuffmanCode {
    pub code: u16,
    pub length: u8,
}

impl Default for HuffmanCode {
    fn default() -> Self {
        Self { code: 0, length: 0 }
    }
}

/// Huffman encoder with code tables
#[derive(Debug, Clone)]
pub struct HuffmanEncoder {
    literal_codes: Vec<HuffmanCode>,
    distance_codes: Vec<HuffmanCode>,
}

impl HuffmanEncoder {
    /// Create new Huffman encoder from frequency tables
    pub fn new(literal_freqs: &[u32], distance_freqs: &[u32]) -> Result<Self> {
        let literal_codes = Self::build_codes(literal_freqs, MAX_LITERALS)?;
        let distance_codes = Self::build_codes(distance_freqs, MAX_DISTANCES)?;

        Ok(Self {
            literal_codes,
            distance_codes,
        })
    }

    /// Create fixed Huffman encoder (RFC 1951 section 3.2.6)
    pub fn fixed() -> Self {
        let mut literal_codes = vec![HuffmanCode::default(); MAX_LITERALS];
        let mut distance_codes = vec![HuffmanCode::default(); MAX_DISTANCES];

        // Fixed literal/length codes
        for i in 0..=143 {
            literal_codes[i] = HuffmanCode {
                code: (0b00110000 + i) as u16,
                length: 8,
            };
        }
        for i in 144..=255 {
            literal_codes[i] = HuffmanCode {
                code: (0b110010000 + (i - 144)) as u16,
                length: 9,
            };
        }
        for i in 256..=279 {
            literal_codes[i] = HuffmanCode {
                code: (0b0000000 + (i - 256)) as u16,
                length: 7,
            };
        }
        for i in 280..=285 {
            literal_codes[i] = HuffmanCode {
                code: (0b11000000 + (i - 280)) as u16,
                length: 8,
            };
        }

        // Fixed distance codes (all 5 bits)
        for i in 0..MAX_DISTANCES {
            distance_codes[i] = HuffmanCode {
                code: i as u16,
                length: 5,
            };
        }

        Self {
            literal_codes,
            distance_codes,
        }
    }

    /// Encode a literal byte
    pub fn encode_literal(&self, literal: u8) -> HuffmanCode {
        self.literal_codes[literal as usize]
    }

    /// Encode a length code
    pub fn encode_length(&self, length: u16) -> (HuffmanCode, u16, u8) {
        let (code_idx, extra_bits, num_bits) = Self::length_to_code(length);
        (self.literal_codes[code_idx], extra_bits, num_bits)
    }

    /// Encode a distance code
    pub fn encode_distance(&self, distance: u16) -> (HuffmanCode, u16, u8) {
        let (code_idx, extra_bits, num_bits) = Self::distance_to_code(distance);
        (self.distance_codes[code_idx], extra_bits, num_bits)
    }

    /// Get literal codes table
    pub fn literal_codes(&self) -> &[HuffmanCode] {
        &self.literal_codes
    }

    /// Get distance codes table
    pub fn distance_codes(&self) -> &[HuffmanCode] {
        &self.distance_codes
    }

    /// Build Huffman codes from frequency table
    fn build_codes(frequencies: &[u32], max_symbols: usize) -> Result<Vec<HuffmanCode>> {
        let mut codes = vec![HuffmanCode::default(); max_symbols];

        // Count non-zero frequencies
        let non_zero: Vec<_> = frequencies
            .iter()
            .enumerate()
            .filter(|(_, &freq)| freq > 0)
            .collect();

        if non_zero.is_empty() {
            return Ok(codes);
        }

        // Build Huffman tree
        let mut nodes: Vec<HuffmanNode> = non_zero
            .iter()
            .map(|(idx, &freq)| HuffmanNode::leaf(*idx as u16, freq))
            .collect();

        if nodes.len() == 1 {
            // Special case: only one symbol
            codes[non_zero[0].0] = HuffmanCode { code: 0, length: 1 };
            return Ok(codes);
        }

        // Build tree using priority queue
        while nodes.len() > 1 {
            nodes.sort_unstable();
            let left = nodes.pop().unwrap();
            let right = nodes.pop().unwrap();
            nodes.push(HuffmanNode::internal(left, right));
        }

        let root = &nodes[0];

        // Extract code lengths from tree
        let mut lengths = vec![0u8; max_symbols];
        Self::extract_lengths(root, &mut lengths, 0);

        // Limit code lengths to MAX_BITS
        Self::limit_lengths(&mut lengths, MAX_BITS);

        // Generate canonical codes from lengths
        Self::canonical_codes(&lengths, &mut codes);

        Ok(codes)
    }

    /// Extract code lengths from Huffman tree
    fn extract_lengths(node: &HuffmanNode, lengths: &mut [u8], depth: u8) {
        if let Some(symbol) = node.symbol {
            lengths[symbol as usize] = depth;
        } else {
            if let Some(ref left) = node.left {
                Self::extract_lengths(left, lengths, depth + 1);
            }
            if let Some(ref right) = node.right {
                Self::extract_lengths(right, lengths, depth + 1);
            }
        }
    }

    /// Limit code lengths to maximum (package-merge algorithm simplified)
    fn limit_lengths(lengths: &mut [u8], max_bits: usize) {
        let max_bits = max_bits as u8;

        loop {
            let max_len = *lengths.iter().max().unwrap_or(&0);
            if max_len <= max_bits {
                break;
            }

            // Find longest code and redistribute
            for len in lengths.iter_mut() {
                if *len > max_bits {
                    *len = max_bits;
                }
            }
        }
    }

    /// Generate canonical Huffman codes from lengths
    fn canonical_codes(lengths: &[u8], codes: &mut [HuffmanCode]) {
        // Count codes per length
        let mut bl_count = [0u16; MAX_BITS + 1];
        for &len in lengths {
            if len > 0 {
                bl_count[len as usize] += 1;
            }
        }

        // Find first code for each length
        let mut next_code = [0u16; MAX_BITS + 1];
        let mut code = 0u16;
        for bits in 1..=MAX_BITS {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }

        // Assign codes to symbols
        for (symbol, &len) in lengths.iter().enumerate() {
            if len > 0 {
                codes[symbol] = HuffmanCode {
                    code: next_code[len as usize],
                    length: len,
                };
                next_code[len as usize] += 1;
            }
        }
    }

    /// Convert length to DEFLATE length code
    pub fn length_to_code(length: u16) -> (usize, u16, u8) {
        match length {
            3..=10 => (257 + (length - 3) as usize, 0, 0),
            11..=18 => {
                let base = 265;
                let offset = (length - 11) / 2;
                let extra = (length - 11) % 2;
                (base + offset as usize, extra, 1)
            }
            19..=34 => {
                let base = 269;
                let offset = (length - 19) / 4;
                let extra = (length - 19) % 4;
                (base + offset as usize, extra, 2)
            }
            35..=66 => {
                let base = 273;
                let offset = (length - 35) / 8;
                let extra = (length - 35) % 8;
                (base + offset as usize, extra, 3)
            }
            67..=130 => {
                let base = 277;
                let offset = (length - 67) / 16;
                let extra = (length - 67) % 16;
                (base + offset as usize, extra, 4)
            }
            131..=257 => {
                let base = 281;
                let offset = (length - 131) / 32;
                let extra = (length - 131) % 32;
                (base + offset as usize, extra, 5)
            }
            258 => (285, 0, 0),
            _ => (257, 0, 0), // Default to min length
        }
    }

    /// Convert distance to DEFLATE distance code
    pub fn distance_to_code(distance: u16) -> (usize, u16, u8) {
        match distance {
            1..=4 => ((distance - 1) as usize, 0, 0),
            5..=8 => {
                let base = 4;
                let offset = (distance - 5) / 2;
                let extra = (distance - 5) % 2;
                (base + offset as usize, extra, 1)
            }
            9..=16 => {
                let base = 6;
                let offset = (distance - 9) / 4;
                let extra = (distance - 9) % 4;
                (base + offset as usize, extra, 2)
            }
            17..=32 => {
                let base = 8;
                let offset = (distance - 17) / 8;
                let extra = (distance - 17) % 8;
                (base + offset as usize, extra, 3)
            }
            33..=64 => {
                let base = 10;
                let offset = (distance - 33) / 16;
                let extra = (distance - 33) % 16;
                (base + offset as usize, extra, 4)
            }
            65..=128 => {
                let base = 12;
                let offset = (distance - 65) / 32;
                let extra = (distance - 65) % 32;
                (base + offset as usize, extra, 5)
            }
            129..=256 => {
                let base = 14;
                let offset = (distance - 129) / 64;
                let extra = (distance - 129) % 64;
                (base + offset as usize, extra, 6)
            }
            257..=512 => {
                let base = 16;
                let offset = (distance - 257) / 128;
                let extra = (distance - 257) % 128;
                (base + offset as usize, extra, 7)
            }
            513..=1024 => {
                let base = 18;
                let offset = (distance - 513) / 256;
                let extra = (distance - 513) % 256;
                (base + offset as usize, extra, 8)
            }
            1025..=2048 => {
                let base = 20;
                let offset = (distance - 1025) / 512;
                let extra = (distance - 1025) % 512;
                (base + offset as usize, extra, 9)
            }
            2049..=4096 => {
                let base = 22;
                let offset = (distance - 2049) / 1024;
                let extra = (distance - 2049) % 1024;
                (base + offset as usize, extra, 10)
            }
            4097..=8192 => {
                let base = 24;
                let offset = (distance - 4097) / 2048;
                let extra = (distance - 4097) % 2048;
                (base + offset as usize, extra, 11)
            }
            8193..=16384 => {
                let base = 26;
                let offset = (distance - 8193) / 4096;
                let extra = (distance - 8193) % 4096;
                (base + offset as usize, extra, 12)
            }
            16385..=32768 => {
                let base = 28;
                let offset = (distance - 16385) / 8192;
                let extra = (distance - 16385) % 8192;
                (base + offset as usize, extra, 13)
            }
            _ => (0, 0, 0), // Default to min distance
        }
    }
}

/// Huffman decoder with code tables
#[derive(Debug, Clone)]
pub struct HuffmanDecoder {
    literal_codes: Vec<(u16, u8)>,
    distance_codes: Vec<(u16, u8)>,
}

impl HuffmanDecoder {
    /// Create decoder from code length tables
    pub fn new(literal_lengths: &[u8], distance_lengths: &[u8]) -> Result<Self> {
        let literal_codes = Self::build_codes_from_lengths(literal_lengths);
        let distance_codes = Self::build_codes_from_lengths(distance_lengths);

        Ok(Self {
            literal_codes,
            distance_codes,
        })
    }

    /// Create fixed Huffman decoder (RFC 1951)
    pub fn fixed() -> Self {
        let mut literal_lengths = vec![0u8; MAX_LITERALS];
        let distance_lengths = vec![5u8; MAX_DISTANCES];

        // Fixed literal/length code lengths
        for i in 0..=143 {
            literal_lengths[i] = 8;
        }
        for i in 144..=255 {
            literal_lengths[i] = 9;
        }
        for i in 256..=279 {
            literal_lengths[i] = 7;
        }
        for i in 280..=285 {
            literal_lengths[i] = 8;
        }

        Self::new(&literal_lengths, &distance_lengths).unwrap()
    }

    /// Decode next literal/length symbol from bit stream
    pub fn decode_literal(&self, bits: &mut BitReader) -> Result<u16> {
        Self::decode_symbol(&self.literal_codes, bits)
    }

    /// Decode next distance symbol from bit stream
    pub fn decode_distance(&self, bits: &mut BitReader) -> Result<u16> {
        Self::decode_symbol(&self.distance_codes, bits)
    }

    /// Build canonical codes from lengths
    fn build_codes_from_lengths(lengths: &[u8]) -> Vec<(u16, u8)> {
        let mut codes = vec![(0u16, 0u8); lengths.len()];

        // Count codes per length
        let mut bl_count = [0u16; MAX_BITS + 1];
        for &len in lengths {
            if len > 0 {
                bl_count[len as usize] += 1;
            }
        }

        // Find first code for each length
        let mut next_code = [0u16; MAX_BITS + 1];
        let mut code = 0u16;
        for bits in 1..=MAX_BITS {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }

        // Assign codes to symbols
        for (symbol, &len) in lengths.iter().enumerate() {
            if len > 0 {
                codes[symbol] = (next_code[len as usize], len);
                next_code[len as usize] += 1;
            }
        }

        codes
    }

    /// Decode symbol from bit stream
    fn decode_symbol(codes: &[(u16, u8)], bits: &mut BitReader) -> Result<u16> {
        let mut code = 0u16;
        let mut len = 0;

        // Read bits one at a time and try to match a code
        for _ in 0..MAX_BITS {
            let bit = bits.read(1)?;
            code = (code << 1) | bit;
            len += 1;

            // Check if this code matches any symbol
            for (symbol, &(symbol_code, symbol_len)) in codes.iter().enumerate() {
                if symbol_len == len && symbol_code == code {
                    return Ok(symbol as u16);
                }
            }
        }

        Err(ZlibError::InvalidDeflateStream(
            "Failed to decode Huffman symbol".to_string(),
        ))
    }
}

/// Bit-level reader for Huffman decoding
pub struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    /// Create new bit reader
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    /// Peek at next N bits without consuming
    pub fn peek(&self, n: usize) -> Result<u16> {
        if n > 16 {
            return Err(ZlibError::InvalidDeflateStream(
                "Cannot peek more than 16 bits".to_string(),
            ));
        }

        let mut result = 0u16;
        let mut bits_read = 0;
        let mut byte_pos = self.byte_pos;
        let mut bit_pos = self.bit_pos;

        while bits_read < n {
            if byte_pos >= self.data.len() {
                return Err(ZlibError::InvalidDeflateStream(
                    "Unexpected end of stream".to_string(),
                ));
            }

            let bits_in_byte = 8 - bit_pos;
            let bits_to_read = std::cmp::min(n - bits_read, bits_in_byte);

            let mask = if bits_to_read >= 8 {
                0xFF
            } else {
                (1u8 << bits_to_read) - 1
            };
            let bits = (self.data[byte_pos] >> bit_pos) & mask;

            result |= (bits as u16) << bits_read;
            bits_read += bits_to_read;
            bit_pos += bits_to_read;

            if bit_pos >= 8 {
                byte_pos += 1;
                bit_pos = 0;
            }
        }

        Ok(result)
    }

    /// Consume N bits from stream
    pub fn consume(&mut self, n: usize) -> Result<()> {
        self.bit_pos += n;
        while self.bit_pos >= 8 {
            self.byte_pos += 1;
            self.bit_pos -= 8;
        }

        if self.byte_pos > self.data.len() {
            return Err(ZlibError::InvalidDeflateStream(
                "Read past end of stream".to_string(),
            ));
        }

        Ok(())
    }

    /// Read N bits from stream
    pub fn read(&mut self, n: usize) -> Result<u16> {
        let value = self.peek(n)?;
        self.consume(n)?;
        Ok(value)
    }

    /// Align to byte boundary
    pub fn align(&mut self) {
        if self.bit_pos > 0 {
            self.byte_pos += 1;
            self.bit_pos = 0;
        }
    }

    /// Get current position
    pub fn position(&self) -> usize {
        self.byte_pos * 8 + self.bit_pos
    }
}

/// Bit-level writer for Huffman encoding
pub struct BitWriter {
    bits: BitVec,
}

impl BitWriter {
    /// Create new bit writer
    pub fn new() -> Self {
        Self {
            bits: BitVec::new(),
        }
    }

    /// Write N bits to stream
    pub fn write(&mut self, value: u16, n: usize) {
        for i in 0..n {
            let bit = ((value >> i) & 1) != 0;
            self.bits.push(bit);
        }
    }

    /// Write Huffman code
    pub fn write_code(&mut self, code: HuffmanCode) {
        self.write(code.code, code.length as usize);
    }

    /// Align to byte boundary
    pub fn align(&mut self) {
        while self.bits.len() % 8 != 0 {
            self.bits.push(false);
        }
    }

    /// Get bytes written
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bits.to_bytes()
    }

    /// Get bit length
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }
}

impl Default for BitWriter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_huffman_encoder() {
        let encoder = HuffmanEncoder::fixed();

        // Test literal codes
        let code = encoder.encode_literal(65); // 'A'
        assert!(code.length > 0);

        // Test end-of-block symbol (256)
        let eob = encoder.literal_codes()[256];
        assert_eq!(eob.length, 7);
    }

    #[test]
    fn test_bit_writer() {
        let mut writer = BitWriter::new();
        writer.write(0b101, 3);
        writer.write(0b11, 2);
        writer.align();

        let bytes = writer.to_bytes();
        // Bits are written LSB first: 101 then 11 then padding zeros
        // Byte layout: 11101000 (reversed because bit-vec uses MSB ordering)
        assert_eq!(bytes[0], 0b10111000);
    }

    #[test]
    fn test_bit_reader() {
        let data = vec![0b11010100u8];
        let mut reader = BitReader::new(&data);

        assert_eq!(reader.read(3).unwrap(), 0b100);
        assert_eq!(reader.read(2).unwrap(), 0b10);
    }

    #[test]
    fn test_length_encoding() {
        let (code_idx, extra, bits) = HuffmanEncoder::length_to_code(3);
        assert_eq!(code_idx, 257);
        assert_eq!(extra, 0);
        assert_eq!(bits, 0);

        let (code_idx, extra, bits) = HuffmanEncoder::length_to_code(258);
        assert_eq!(code_idx, 285);
    }

    #[test]
    fn test_distance_encoding() {
        let (code_idx, extra, bits) = HuffmanEncoder::distance_to_code(1);
        assert_eq!(code_idx, 0);

        let (code_idx, extra, bits) = HuffmanEncoder::distance_to_code(32768);
        assert!(code_idx < MAX_DISTANCES);
    }
}
