//! INFLATE decompression implementation (RFC 1951)
//!
//! This module implements the INFLATE decompression algorithm:
//! - Decompresses DEFLATE-compressed data
//! - Handles fixed and dynamic Huffman blocks
//! - Supports stored (uncompressed) blocks
//! - Implements LZ77 back-reference expansion

use crate::{
    huffman::{BitReader, HuffmanDecoder},
    Result, ZlibError,
};

/// Maximum window size for INFLATE
const WINDOW_SIZE: usize = 32768;

/// End-of-block symbol
const END_OF_BLOCK: u16 = 256;

/// Block types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockType {
    Uncompressed = 0,
    FixedHuffman = 1,
    DynamicHuffman = 2,
    Reserved = 3,
}

impl BlockType {
    fn from_u16(value: u16) -> Result<Self> {
        match value {
            0 => Ok(Self::Uncompressed),
            1 => Ok(Self::FixedHuffman),
            2 => Ok(Self::DynamicHuffman),
            3 => Err(ZlibError::InvalidDeflateStream(
                "Reserved block type".to_string(),
            )),
            _ => Err(ZlibError::InvalidDeflateStream(format!(
                "Invalid block type: {}",
                value
            ))),
        }
    }
}

/// Length base values for codes 257-285
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];

/// Extra bits for length codes
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

/// Distance base values
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];

/// Extra bits for distance codes
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Code length alphabet order
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// INFLATE decompressor
pub struct Decompressor {
    output: Vec<u8>,
    window: Vec<u8>,
}

impl Decompressor {
    /// Create new INFLATE decompressor
    pub fn new() -> Self {
        Self {
            output: Vec::new(),
            window: Vec::new(),
        }
    }

    /// Decompress DEFLATE-compressed data
    pub fn decompress(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        self.output.clear();
        self.window.clear();

        if data.is_empty() {
            return Ok(Vec::new());
        }

        let mut reader = BitReader::new(data);
        let mut is_final = false;

        while !is_final {
            // Read block header
            is_final = reader.read(1)? != 0;
            let block_type_value = reader.read(2)?;
            let block_type = BlockType::from_u16(block_type_value)?;

            // Decompress block
            match block_type {
                BlockType::Uncompressed => {
                    self.decompress_uncompressed(&mut reader)?;
                }
                BlockType::FixedHuffman => {
                    self.decompress_fixed(&mut reader)?;
                }
                BlockType::DynamicHuffman => {
                    self.decompress_dynamic(&mut reader)?;
                }
                BlockType::Reserved => {
                    return Err(ZlibError::InvalidDeflateStream(
                        "Reserved block type encountered".to_string(),
                    ));
                }
            }
        }

        Ok(self.output.clone())
    }

    /// Decompress uncompressed block
    fn decompress_uncompressed(&mut self, reader: &mut BitReader) -> Result<()> {
        // Skip to byte boundary
        reader.align();

        // Read length and complement
        let len = reader.read(16)?;
        let nlen = reader.read(16)?;

        // Verify complement
        if len != !nlen {
            return Err(ZlibError::InvalidDeflateStream(
                "Uncompressed block length mismatch".to_string(),
            ));
        }

        // Read uncompressed data
        for _ in 0..len {
            let byte = reader.read(8)? as u8;
            self.output_byte(byte);
        }

        Ok(())
    }

    /// Decompress fixed Huffman block
    fn decompress_fixed(&mut self, reader: &mut BitReader) -> Result<()> {
        let decoder = HuffmanDecoder::fixed();
        self.decompress_huffman(reader, &decoder)
    }

    /// Decompress dynamic Huffman block
    fn decompress_dynamic(&mut self, reader: &mut BitReader) -> Result<()> {
        // Read tree sizes
        let hlit = reader.read(5)? as usize + 257; // Literal/length codes
        let hdist = reader.read(5)? as usize + 1; // Distance codes
        let hclen = reader.read(4)? as usize + 4; // Code length codes

        if hlit > 286 {
            return Err(ZlibError::InvalidDeflateStream(format!(
                "Invalid HLIT: {}",
                hlit
            )));
        }

        if hdist > 30 {
            return Err(ZlibError::InvalidDeflateStream(format!(
                "Invalid HDIST: {}",
                hdist
            )));
        }

        // Read code length code lengths
        let mut code_length_lengths = vec![0u8; 19];
        for i in 0..hclen {
            code_length_lengths[CODE_LENGTH_ORDER[i]] = reader.read(3)? as u8;
        }

        // Build code length decoder
        let cl_decoder = HuffmanDecoder::new(&code_length_lengths, &[])?;

        // Decode literal/length code lengths
        let literal_lengths = self.decode_code_lengths(reader, &cl_decoder, hlit)?;

        // Decode distance code lengths
        let distance_lengths = self.decode_code_lengths(reader, &cl_decoder, hdist)?;

        // Build Huffman decoder
        let decoder = HuffmanDecoder::new(&literal_lengths, &distance_lengths)?;

        // Decompress data
        self.decompress_huffman(reader, &decoder)
    }

    /// Decode code lengths using code length alphabet
    fn decode_code_lengths(
        &self,
        reader: &mut BitReader,
        decoder: &HuffmanDecoder,
        count: usize,
    ) -> Result<Vec<u8>> {
        let mut lengths = Vec::new();

        while lengths.len() < count {
            let symbol = decoder.decode_literal(reader)?;

            match symbol {
                0..=15 => {
                    // Literal code length
                    lengths.push(symbol as u8);
                }
                16 => {
                    // Copy previous code length 3-6 times
                    if lengths.is_empty() {
                        return Err(ZlibError::InvalidDeflateStream(
                            "Code length 16 without previous value".to_string(),
                        ));
                    }
                    let prev = *lengths.last().unwrap();
                    let repeat = reader.read(2)? as usize + 3;
                    for _ in 0..repeat {
                        lengths.push(prev);
                    }
                }
                17 => {
                    // Repeat 0 for 3-10 times
                    let repeat = reader.read(3)? as usize + 3;
                    for _ in 0..repeat {
                        lengths.push(0);
                    }
                }
                18 => {
                    // Repeat 0 for 11-138 times
                    let repeat = reader.read(7)? as usize + 11;
                    for _ in 0..repeat {
                        lengths.push(0);
                    }
                }
                _ => {
                    return Err(ZlibError::InvalidDeflateStream(format!(
                        "Invalid code length symbol: {}",
                        symbol
                    )));
                }
            }
        }

        Ok(lengths)
    }

    /// Decompress Huffman-encoded data
    fn decompress_huffman(
        &mut self,
        reader: &mut BitReader,
        decoder: &HuffmanDecoder,
    ) -> Result<()> {
        loop {
            let symbol = decoder.decode_literal(reader)?;

            if symbol < 256 {
                // Literal byte
                self.output_byte(symbol as u8);
            } else if symbol == END_OF_BLOCK {
                // End of block
                break;
            } else if symbol <= 285 {
                // Length/distance pair
                let length = self.decode_length(reader, symbol)?;
                let distance = self.decode_distance(reader, decoder)?;

                // Copy from history
                self.copy_from_history(length, distance)?;
            } else {
                return Err(ZlibError::InvalidDeflateStream(format!(
                    "Invalid literal/length symbol: {}",
                    symbol
                )));
            }
        }

        Ok(())
    }

    /// Decode length from symbol
    fn decode_length(&self, reader: &mut BitReader, symbol: u16) -> Result<u16> {
        if symbol < 257 || symbol > 285 {
            return Err(ZlibError::InvalidDeflateStream(format!(
                "Invalid length symbol: {}",
                symbol
            )));
        }

        let index = (symbol - 257) as usize;
        let base = LENGTH_BASE[index];
        let extra_bits = LENGTH_EXTRA[index];

        if extra_bits > 0 {
            let extra = reader.read(extra_bits as usize)?;
            Ok(base + extra)
        } else {
            Ok(base)
        }
    }

    /// Decode distance from bit stream
    fn decode_distance(&self, reader: &mut BitReader, decoder: &HuffmanDecoder) -> Result<u16> {
        let symbol = decoder.decode_distance(reader)?;

        if symbol >= 30 {
            return Err(ZlibError::InvalidDeflateStream(format!(
                "Invalid distance symbol: {}",
                symbol
            )));
        }

        let base = DISTANCE_BASE[symbol as usize];
        let extra_bits = DISTANCE_EXTRA[symbol as usize];

        if extra_bits > 0 {
            let extra = reader.read(extra_bits as usize)?;
            Ok(base + extra)
        } else {
            Ok(base)
        }
    }

    /// Copy bytes from history window
    fn copy_from_history(&mut self, length: u16, distance: u16) -> Result<()> {
        let distance = distance as usize;
        let length = length as usize;

        if distance > self.window.len() {
            return Err(ZlibError::InvalidDeflateStream(format!(
                "Distance {} exceeds window size {}",
                distance,
                self.window.len()
            )));
        }

        let start_pos = self.window.len() - distance;

        // Handle overlapping copies (distance < length)
        for i in 0..length {
            let pos = start_pos + (i % distance);
            let byte = self.window[pos];
            self.output_byte(byte);
        }

        Ok(())
    }

    /// Output a byte and update window
    fn output_byte(&mut self, byte: u8) {
        self.output.push(byte);
        self.window.push(byte);

        // Maintain sliding window
        if self.window.len() > WINDOW_SIZE {
            self.window.remove(0);
        }
    }
}

impl Default for Decompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Decompress DEFLATE data (convenience function)
pub fn inflate(data: &[u8]) -> Result<Vec<u8>> {
    let mut decompressor = Decompressor::new();
    decompressor.decompress(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deflate::Compressor;
    use crate::CompressionLevel;

    #[test]
    fn test_inflate_empty() {
        let mut decompressor = Decompressor::new();
        let result = decompressor.decompress(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_round_trip_stored() {
        let data = b"Hello, World!";
        let mut compressor = Compressor::new(CompressionLevel::None);
        let compressed = compressor.compress(data).unwrap();

        let mut decompressor = Decompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_round_trip_fixed() {
        let data = b"The quick brown fox jumps over the lazy dog.";
        let mut compressor = Compressor::new(CompressionLevel::Fast);
        let compressed = compressor.compress(data).unwrap();

        let mut decompressor = Decompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_round_trip_dynamic() {
        let data = b"Test data with repeated patterns. Test data with repeated patterns.";
        let mut compressor = Compressor::new(CompressionLevel::Best);
        let compressed = compressor.compress(data).unwrap();

        let mut decompressor = Decompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_round_trip_large() {
        let data = vec![b'A'; 50000];
        let mut compressor = Compressor::new(CompressionLevel::Default);
        let compressed = compressor.compress(&data).unwrap();

        let mut decompressor = Decompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_round_trip_binary() {
        let data: Vec<u8> = (0..255).cycle().take(1000).collect();
        let mut compressor = Compressor::new(CompressionLevel::Default);
        let compressed = compressor.compress(&data).unwrap();

        let mut decompressor = Decompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_multiple_blocks() {
        let data = vec![b'X'; 100000]; // Large enough to force multiple blocks
        let mut compressor = Compressor::new(CompressionLevel::Default);
        let compressed = compressor.compress(&data).unwrap();

        let mut decompressor = Decompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_overlapping_copy() {
        // Create data that will result in overlapping LZ77 matches
        let data = b"aaaaaaaaaa"; // Repeated 'a' will create distance < length copies
        let mut compressor = Compressor::new(CompressionLevel::Best);
        let compressed = compressor.compress(data).unwrap();

        let mut decompressor = Decompressor::new();
        let decompressed = decompressor.decompress(&compressed).unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_all_compression_levels() {
        let data = b"Sample data for testing all compression levels in DEFLATE.";

        for level in [
            CompressionLevel::None,
            CompressionLevel::Fast,
            CompressionLevel::Fast2,
            CompressionLevel::Default,
            CompressionLevel::Good,
            CompressionLevel::Best,
        ] {
            let mut compressor = Compressor::new(level);
            let compressed = compressor.compress(data).unwrap();

            let mut decompressor = Decompressor::new();
            let decompressed = decompressor.decompress(&compressed).unwrap();

            assert_eq!(
                data,
                &decompressed[..],
                "Failed round-trip for level {:?}",
                level
            );
        }
    }

    #[test]
    fn test_invalid_block_type() {
        // Manually create invalid compressed data
        let mut reader = BitReader::new(&[0xFF, 0xFF]);
        reader.read(1).unwrap(); // is_final
        let block_type = reader.read(2).unwrap();

        // Block type 3 is reserved
        if block_type == 3 {
            assert!(BlockType::from_u16(block_type).is_err());
        }
    }

    #[test]
    fn test_convenience_function() {
        let data = b"Test inflate convenience function";
        let mut compressor = Compressor::new(CompressionLevel::Default);
        let compressed = compressor.compress(data).unwrap();

        let decompressed = inflate(&compressed).unwrap();
        assert_eq!(data, &decompressed[..]);
    }
}
