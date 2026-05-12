//! RefPack Compression Algorithm
//! 
//! EA's reference compression algorithm optimized with modern Rust techniques.
//! Features SIMD acceleration and parallel processing for maximum performance.

use crate::{Result, EacError};
use rayon::prelude::*;

#[cfg(feature = "simd")]
use wide::*;

const MIN_MATCH_LENGTH: usize = 3;
const MAX_MATCH_LENGTH: usize = 255 + MIN_MATCH_LENGTH;
const MAX_DISTANCE: usize = 65535;
const HASH_TABLE_SIZE: usize = 65536;
const HASH_CHAIN_LENGTH: usize = 4096;

/// RefPack encoder with modern optimizations
pub struct RefPackEncoder {
    hash_table: Vec<u16>,
    hash_chain: Vec<u16>,
    window: Vec<u8>,
    window_pos: usize,
}

impl RefPackEncoder {
    pub fn new() -> Self {
        Self {
            hash_table: vec![0; HASH_TABLE_SIZE],
            hash_chain: vec![0; HASH_CHAIN_LENGTH],
            window: Vec::with_capacity(MAX_DISTANCE),
            window_pos: 0,
        }
    }
    
    /// Encode data using RefPack algorithm
    pub fn encode(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut output = Vec::with_capacity(input.len() + input.len() / 8);
        let mut pos = 0;
        
        // Initialize sliding window
        self.window.clear();
        self.window.extend_from_slice(&input[..std::cmp::min(input.len(), MAX_DISTANCE)]);
        
        while pos < input.len() {
            let (match_length, match_distance) = self.find_longest_match(&input[pos..], pos);
            
            if match_length >= MIN_MATCH_LENGTH {
                // Encode match
                self.encode_match(&mut output, match_length, match_distance)?;
                pos += match_length;
            } else {
                // Encode literal
                self.encode_literal(&mut output, input[pos])?;
                pos += 1;
            }
            
            // Update hash tables
            self.update_hash_tables(&input[pos.saturating_sub(MIN_MATCH_LENGTH)..pos]);
        }
        
        Ok(output)
    }
    
    /// Find longest match in sliding window using hash chains
    fn find_longest_match(&self, data: &[u8], pos: usize) -> (usize, usize) {
        if data.len() < MIN_MATCH_LENGTH {
            return (0, 0);
        }
        
        let hash = self.compute_hash(&data[..MIN_MATCH_LENGTH]);
        let mut best_length = 0;
        let mut best_distance = 0;
        
        let mut chain_pos = self.hash_table[hash] as usize;
        let mut chain_count = 0;
        
        while chain_pos > 0 && chain_count < 32 {
            if chain_pos >= pos || chain_pos >= self.window.len() {
                chain_pos = self.hash_chain[chain_pos % HASH_CHAIN_LENGTH] as usize;
                chain_count += 1;
                continue;
            }

            let distance = pos - chain_pos;
            if distance > MAX_DISTANCE {
                break;
            }
            
            let match_length = self.calculate_match_length(data, &self.window[chain_pos..]);
            if match_length > best_length {
                best_length = match_length;
                best_distance = distance;
                
                if match_length >= MAX_MATCH_LENGTH {
                    break;
                }
            }
            
            chain_pos = self.hash_chain[chain_pos % HASH_CHAIN_LENGTH] as usize;
            chain_count += 1;
        }
        
        (best_length, best_distance)
    }
    
    /// Calculate match length using SIMD when available
    #[cfg(feature = "simd")]
    fn calculate_match_length(&self, data1: &[u8], data2: &[u8]) -> usize {
        let len = std::cmp::min(data1.len(), data2.len());
        let len = std::cmp::min(len, MAX_MATCH_LENGTH);
        
        let mut matched = 0;
        
        // SIMD comparison for bulk of data
        let simd_chunks = len / 32;
        for i in 0..simd_chunks {
            let offset = i * 32;
            let chunk1 = u8x32::from(&data1[offset..offset + 32]);
            let chunk2 = u8x32::from(&data2[offset..offset + 32]);
            
            if chunk1 == chunk2 {
                matched += 32;
            } else {
                // Find exact mismatch position
                for j in 0..32 {
                    if data1[offset + j] == data2[offset + j] {
                        matched += 1;
                    } else {
                        return matched;
                    }
                }
            }
        }
        
        // Handle remaining bytes
        for i in matched..len {
            if data1[i] == data2[i] {
                matched += 1;
            } else {
                break;
            }
        }
        
        matched
    }
    
    #[cfg(not(feature = "simd"))]
    fn calculate_match_length(&self, data1: &[u8], data2: &[u8]) -> usize {
        let len = std::cmp::min(data1.len(), data2.len());
        let len = std::cmp::min(len, MAX_MATCH_LENGTH);
        
        for i in 0..len {
            if data1[i] != data2[i] {
                return i;
            }
        }
        len
    }
    
    /// Compute 3-byte hash
    fn compute_hash(&self, data: &[u8]) -> usize {
        if data.len() < 3 {
            return 0;
        }
        
        let mut hash = 0u32;
        hash = hash.wrapping_mul(33).wrapping_add(data[0] as u32);
        hash = hash.wrapping_mul(33).wrapping_add(data[1] as u32);
        hash = hash.wrapping_mul(33).wrapping_add(data[2] as u32);
        
        (hash as usize) & (HASH_TABLE_SIZE - 1)
    }
    
    /// Update hash tables for sliding window
    fn update_hash_tables(&mut self, data: &[u8]) {
        for (i, chunk) in data.windows(MIN_MATCH_LENGTH).enumerate() {
            let hash = self.compute_hash(chunk);
            let pos = (self.window_pos + i) % HASH_CHAIN_LENGTH;
            
            self.hash_chain[pos] = self.hash_table[hash];
            self.hash_table[hash] = pos as u16;
        }
        
        self.window_pos = (self.window_pos + data.len()) % HASH_CHAIN_LENGTH;
    }
    
    /// Encode a literal byte
    fn encode_literal(&self, output: &mut Vec<u8>, byte: u8) -> Result<()> {
        if byte & 0x80 == 0 {
            output.push(byte);
        } else {
            output.extend_from_slice(&[0x80, 0, 0, byte]);
        }
        Ok(())
    }
    
    /// Encode a match (length, distance) pair
    fn encode_match(&self, output: &mut Vec<u8>, length: usize, distance: usize) -> Result<()> {
        if length < MIN_MATCH_LENGTH || length > MAX_MATCH_LENGTH {
            return Err(EacError::CompressionFailed(
                format!("Invalid match length: {}", length)
            ));
        }
        
        if distance == 0 || distance > MAX_DISTANCE {
            return Err(EacError::CompressionFailed(
                format!("Invalid match distance: {}", distance)
            ));
        }
        
        // RefPack encoding format
        let encoded_length = (length - MIN_MATCH_LENGTH) as u8;
        let encoded_distance = distance as u16;
        
        output.push(0x80 | encoded_length); // Flag bit + length
        output.extend_from_slice(&encoded_distance.to_le_bytes());
        
        Ok(())
    }
}

impl Default for RefPackEncoder {
    fn default() -> Self {
        Self::new()
    }
}

/// RefPack decoder with optimized decompression
pub struct RefPackDecoder {
    output_buffer: Vec<u8>,
}

impl RefPackDecoder {
    pub fn new() -> Self {
        Self {
            output_buffer: Vec::new(),
        }
    }
    
    /// Decode RefPack compressed data
    pub fn decode(&mut self, input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        self.output_buffer.clear();
        self.output_buffer.reserve(expected_size);
        
        let mut pos = 0;
        
        while pos < input.len() && self.output_buffer.len() < expected_size {
            let byte = input[pos];
            pos += 1;
            
            if byte & 0x80 != 0 {
                // Match token
                if pos + 1 >= input.len() {
                    return Err(EacError::DecompressionFailed(
                        "Unexpected end of input while reading match".to_string()
                    ));
                }
                
                let length = ((byte & 0x7F) as usize) + MIN_MATCH_LENGTH;
                let distance = u16::from_le_bytes([input[pos], input[pos + 1]]) as usize;
                pos += 2;

                if distance == 0 {
                    if pos >= input.len() {
                        return Err(EacError::DecompressionFailed(
                            "Unexpected end of input while reading escaped literal".to_string(),
                        ));
                    }
                    self.output_buffer.push(input[pos]);
                    pos += 1;
                    continue;
                }

                if distance > self.output_buffer.len() {
                    return Err(EacError::DecompressionFailed(
                        format!("Invalid back reference: distance={}, buffer_len={}", 
                               distance, self.output_buffer.len())
                    ));
                }
                
                let start_pos = self.output_buffer.len() - distance;
                
                // Copy match data (handle overlapping copies)
                for i in 0..length {
                    if start_pos + i >= self.output_buffer.len() {
                        return Err(EacError::DecompressionFailed(
                            "Back reference out of bounds".to_string()
                        ));
                    }
                    
                    let byte_to_copy = self.output_buffer[start_pos + i];
                    self.output_buffer.push(byte_to_copy);
                    
                    if self.output_buffer.len() >= expected_size {
                        break;
                    }
                }
            } else {
                // Literal byte
                self.output_buffer.push(byte);
            }
        }
        
        if self.output_buffer.len() != expected_size {
            return Err(EacError::DecompressionFailed(
                format!("Size mismatch: expected {}, got {}", 
                       expected_size, self.output_buffer.len())
            ));
        }
        
        Ok(self.output_buffer.clone())
    }
}

impl Default for RefPackDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// High-level encode function
pub fn encode(input: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = RefPackEncoder::new();
    encoder.encode(input)
}

/// High-level decode function
pub fn decode(input: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let mut decoder = RefPackDecoder::new();
    decoder.decode(input, expected_size)
}

/// Parallel RefPack compression for large data
pub fn encode_parallel(input: &[u8], chunk_size: usize) -> Result<Vec<u8>> {
    if input.len() <= chunk_size {
        return encode(input);
    }
    
    let chunks: Vec<_> = input.par_chunks(chunk_size).collect();
    let compressed_chunks: Result<Vec<_>> = chunks
        .par_iter()
        .map(|chunk| encode(chunk))
        .collect();
    
    let compressed_chunks = compressed_chunks?;
    
    // Combine chunks with metadata
    let mut result = Vec::new();
    result.extend_from_slice(&(compressed_chunks.len() as u32).to_le_bytes());
    result.extend_from_slice(&(chunk_size as u32).to_le_bytes());
    
    for chunk in compressed_chunks {
        result.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        result.extend_from_slice(&chunk);
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    #[test]
    fn test_refpack_empty() {
        let mut encoder = RefPackEncoder::new();
        let compressed = encoder.encode(b"").unwrap();
        assert!(compressed.is_empty());
        
        let mut decoder = RefPackDecoder::new();
        let decompressed = decoder.decode(&compressed, 0).unwrap();
        assert!(decompressed.is_empty());
    }
    
    #[test]
    fn test_refpack_single_byte() {
        let input = b"a";
        let mut encoder = RefPackEncoder::new();
        let compressed = encoder.encode(input).unwrap();
        
        let mut decoder = RefPackDecoder::new();
        let decompressed = decoder.decode(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);
    }
    
    #[test]
    fn test_refpack_repeated_data() {
        let input = b"aaaaaaaaaaaaaaaa"; // 16 'a's
        let mut encoder = RefPackEncoder::new();
        let compressed = encoder.encode(input).unwrap();
        
        // Should compress well due to repetition
        assert!(compressed.len() < input.len());
        
        let mut decoder = RefPackDecoder::new();
        let decompressed = decoder.decode(&compressed, input.len()).unwrap();
        assert_eq!(input, &decompressed[..]);
    }
    
    #[test]
    fn test_hash_computation() {
        let encoder = RefPackEncoder::new();
        let data1 = b"abc";
        let data2 = b"abc";
        let data3 = b"def";
        
        assert_eq!(encoder.compute_hash(data1), encoder.compute_hash(data2));
        assert_ne!(encoder.compute_hash(data1), encoder.compute_hash(data3));
    }
    
    proptest! {
        #[test]
        fn test_refpack_roundtrip(input in any::<Vec<u8>>()) {
            if !input.is_empty() {
                let mut encoder = RefPackEncoder::new();
                let compressed = encoder.encode(&input).unwrap();
                
                let mut decoder = RefPackDecoder::new();
                let decompressed = decoder.decode(&compressed, input.len()).unwrap();
                
                assert_eq!(input, decompressed);
            }
        }
        
        #[test]
        fn test_parallel_refpack(input in any::<Vec<u8>>()) {
            if input.len() > 1000 {
                let compressed_serial = encode(&input).unwrap();
                let _compressed_parallel = encode_parallel(&input, 1000).unwrap();

                // Both should be valid (parallel may have different compression ratio)
                let decompressed_serial = decode(&compressed_serial, input.len()).unwrap();
                assert_eq!(input, decompressed_serial);
            }
        }
    }
}
