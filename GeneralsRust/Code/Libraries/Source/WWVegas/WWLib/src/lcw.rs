//! LCW (Lempel-Ziv-based Compression/Decompression) Algorithm Implementation
//!
//! This module provides safe Rust implementations of the LCW compression and decompression
//! algorithms used in Westwood Studios games like Command & Conquer Generals.
//!
//! LCW is a variant of the LZ compression family optimized for fast decompression at the
//! expense of slower compression. It uses various encoding schemes to efficiently compress
//! different types of data patterns.
//!
//! # Compression Format
//!
//! The LCW format uses different command codes based on the bit patterns:
//!
//! - `n=0xxxyyyy,yyyyyyyy`: Short copy back y bytes and run x+3 from destination
//! - `n=10xxxxxx,n1,n2,...,nx+1`: Medium length copy the next x+1 bytes from source
//! - `n=11xxxxxx,w1`: Medium copy from dest x+3 bytes from offset w1
//! - `n=11111111,w1,w2`: Long copy from dest w1 bytes from offset w2
//! - `n=11111110,w1,b1`: Long run of byte b1 for w1 bytes
//! - `n=10000000`: End of data reached
//!
//! # Examples
//!
//! ```rust
//! use wwlib_rust::lcw::{decompress, compress, LcwError};
//!
//! // Compress some data
//! let original = b"Hello, World! This is a test string with repeated patterns.";
//! let compressed = compress(original).unwrap();
//!
//! // Decompress it back
//! let decompressed = decompress(&compressed).unwrap();
//! assert_eq!(original, &decompressed[..]);
//! ```

use std::cmp;

/// Error types for LCW operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LcwError {
    /// Input buffer is too small to contain valid data
    InputTooSmall,
    /// Output buffer is too small for the decompressed data
    OutputTooSmall,
    /// Corrupt or invalid compressed data was encountered
    CorruptData(String),
    /// Offset in compressed data points outside valid range
    InvalidOffset,
    /// Unexpected end of input data
    UnexpectedEndOfInput,
}

impl std::fmt::Display for LcwError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LcwError::InputTooSmall => write!(f, "Input buffer too small"),
            LcwError::OutputTooSmall => write!(f, "Output buffer too small"),
            LcwError::CorruptData(msg) => write!(f, "Corrupt data: {}", msg),
            LcwError::InvalidOffset => write!(f, "Invalid offset in compressed data"),
            LcwError::UnexpectedEndOfInput => write!(f, "Unexpected end of input data"),
        }
    }
}

impl std::error::Error for LcwError {}

/// Result type for LCW operations
pub type LcwResult<T> = Result<T, LcwError>;

/// Decompress LCW-encoded data
///
/// # Arguments
///
/// * `source` - The compressed data to decompress
///
/// # Returns
///
/// A vector containing the decompressed data, or an error if decompression fails
///
/// # Errors
///
/// Returns `LcwError` if:
/// - The input data is corrupt or invalid
/// - An offset points outside the valid range
/// - The input data is truncated
pub fn decompress(source: &[u8]) -> LcwResult<Vec<u8>> {
    if source.is_empty() {
        return Err(LcwError::InputTooSmall);
    }

    let mut dest = Vec::with_capacity(source.len() * 2); // Initial estimate
    decompress_to_vec(source, &mut dest)?;
    Ok(dest)
}

/// Decompress LCW-encoded data into a preallocated vector
///
/// This function is more efficient when you can estimate the output size,
/// as it avoids repeated memory allocations.
///
/// # Arguments
///
/// * `source` - The compressed data to decompress
/// * `dest` - The destination vector to write decompressed data to (will be cleared)
///
/// # Returns
///
/// The number of bytes written to the destination
pub fn decompress_to_vec(source: &[u8], dest: &mut Vec<u8>) -> LcwResult<usize> {
    dest.clear();

    let mut source_idx = 0;

    loop {
        if source_idx >= source.len() {
            return Err(LcwError::UnexpectedEndOfInput);
        }

        let op_code = source[source_idx];
        source_idx += 1;

        if (op_code & 0x80) == 0 {
            // Short copy from destination: 0xxxyyyy,yyyyyyyy
            if source_idx >= source.len() {
                return Err(LcwError::UnexpectedEndOfInput);
            }

            let count = ((op_code >> 4) + 3) as usize;
            let offset = (source[source_idx] as usize) + (((op_code & 0x0f) as usize) << 8);
            source_idx += 1;

            if offset > dest.len() {
                return Err(LcwError::InvalidOffset);
            }

            let copy_start = dest.len() - offset;

            // Handle overlapping copy - this mimics the C++ behavior where copy_ptr advances
            let dest_start_len = dest.len();
            for i in 0..count {
                let src_idx = copy_start + i;
                // If we're copying from beyond our current write position, we need to look
                // at what we've already written in this loop
                if src_idx >= dest_start_len {
                    let relative_idx = src_idx - dest_start_len;
                    if relative_idx >= dest.len() - dest_start_len {
                        return Err(LcwError::InvalidOffset);
                    }
                    let byte_to_copy = dest[dest_start_len + relative_idx];
                    dest.push(byte_to_copy);
                } else {
                    if src_idx >= dest.len() {
                        return Err(LcwError::InvalidOffset);
                    }
                    let byte_to_copy = dest[src_idx];
                    dest.push(byte_to_copy);
                }
            }
        } else if (op_code & 0x40) == 0 {
            // Medium operations
            if op_code == 0x80 {
                // End of data
                break;
            } else {
                // Medium copy from source: 10xxxxxx,n1,n2,...,nx+1
                let count = (op_code & 0x3f) as usize;

                if source_idx + count > source.len() {
                    return Err(LcwError::UnexpectedEndOfInput);
                }

                dest.extend_from_slice(&source[source_idx..source_idx + count]);
                source_idx += count;
            }
        } else {
            // High bit operations (11xxxxxx)
            if op_code == 0xfe {
                // Long run: 11111110,w1,b1
                if source_idx + 2 >= source.len() {
                    return Err(LcwError::UnexpectedEndOfInput);
                }

                let count =
                    (source[source_idx] as usize) + ((source[source_idx + 1] as usize) << 8);
                let data_byte = source[source_idx + 2];
                source_idx += 3;

                // Optimized run filling
                dest.resize(dest.len() + count, data_byte);
            } else if op_code == 0xff {
                // Long copy from destination: 11111111,w1,w2
                if source_idx + 3 >= source.len() {
                    return Err(LcwError::UnexpectedEndOfInput);
                }

                let count =
                    (source[source_idx] as usize) + ((source[source_idx + 1] as usize) << 8);
                let offset =
                    (source[source_idx + 2] as usize) + ((source[source_idx + 3] as usize) << 8);
                source_idx += 4;

                if offset >= dest.len() {
                    return Err(LcwError::InvalidOffset);
                }

                // Safe copying from absolute offset
                for i in 0..count {
                    if offset + i >= dest.len() {
                        return Err(LcwError::InvalidOffset);
                    }
                    let byte_to_copy = dest[offset + i];
                    dest.push(byte_to_copy);
                }
            } else {
                // Medium copy from destination: 11xxxxxx,w1
                if source_idx + 1 >= source.len() {
                    return Err(LcwError::UnexpectedEndOfInput);
                }

                let count = ((op_code & 0x3f) + 3) as usize;
                let offset =
                    (source[source_idx] as usize) + ((source[source_idx + 1] as usize) << 8);
                source_idx += 2;

                if offset >= dest.len() {
                    return Err(LcwError::InvalidOffset);
                }

                // Safe copying from absolute offset
                for i in 0..count {
                    if offset + i >= dest.len() {
                        return Err(LcwError::InvalidOffset);
                    }
                    let byte_to_copy = dest[offset + i];
                    dest.push(byte_to_copy);
                }
            }
        }
    }

    Ok(dest.len())
}

/// Compress data using the LCW algorithm
///
/// This is a simplified implementation that focuses on correctness and safety
/// over maximum compression efficiency. The original C++ implementation used
/// complex assembly code for optimization.
///
/// # Arguments
///
/// * `source` - The data to compress
///
/// # Returns
///
/// A vector containing the compressed data, or an error if compression fails
pub fn compress(source: &[u8]) -> LcwResult<Vec<u8>> {
    if source.is_empty() {
        return Ok(vec![0x80]); // Just the end marker
    }

    let mut dest = Vec::with_capacity(source.len() + source.len() / 128); // Estimate with overhead
    let mut source_idx = 0;

    while source_idx < source.len() {
        // Look for the best match
        let (match_length, match_offset) = find_best_match(source, source_idx);

        if match_length >= 3 {
            // We found a good match, encode it
            encode_match(&mut dest, match_length, match_offset, source_idx)?;
            source_idx += match_length;
        } else {
            // No good match, start or continue a literal run
            let literal_start = source_idx;

            // Find the end of the literal run (until we find a good match)
            while source_idx < source.len() {
                let (len, _) = find_best_match(source, source_idx);
                if len >= 3 {
                    break;
                }
                source_idx += 1;
            }

            let literal_length = source_idx - literal_start;
            encode_literals(
                &mut dest,
                &source[literal_start..literal_start + literal_length],
            )?;
        }
    }

    // Add end marker
    dest.push(0x80);
    Ok(dest)
}

/// Find the best match for the current position in the source data
///
/// Returns (match_length, match_offset) where match_offset is relative to the start of data
fn find_best_match(source: &[u8], pos: usize) -> (usize, usize) {
    let mut best_length = 0;
    let mut best_offset = 0;

    // Look backward for matches (simple implementation)
    let search_start = if pos > 4095 { pos - 4095 } else { 0 }; // Limit search window

    for search_pos in search_start..pos {
        let mut match_length = 0;
        let max_length = cmp::min(source.len() - pos, 64); // Reasonable maximum

        while match_length < max_length
            && pos + match_length < source.len()
            && source[search_pos + match_length] == source[pos + match_length]
        {
            match_length += 1;
        }

        if match_length > best_length {
            best_length = match_length;
            best_offset = search_pos;
        }
    }

    (best_length, best_offset)
}

/// Encode a match into the destination buffer
fn encode_match(
    dest: &mut Vec<u8>,
    length: usize,
    offset: usize,
    current_pos: usize,
) -> LcwResult<()> {
    let relative_offset = current_pos - offset;

    if length <= 10 && relative_offset <= 0xfff {
        // Short match: 0xxxyyyy,yyyyyyyy
        let count_code = ((length - 3) << 4) as u8;
        let offset_high = ((relative_offset >> 8) & 0x0f) as u8;
        let offset_low = (relative_offset & 0xff) as u8;

        dest.push(count_code | offset_high);
        dest.push(offset_low);
    } else if length <= 64 {
        // Medium match: 11xxxxxx,w1
        let count_code = 0xc0 | ((length - 3) & 0x3f) as u8;
        dest.push(count_code);
        dest.push((offset & 0xff) as u8);
        dest.push(((offset >> 8) & 0xff) as u8);
    } else {
        // Long match: 11111111,w1,w2
        dest.push(0xff);
        dest.push((length & 0xff) as u8);
        dest.push(((length >> 8) & 0xff) as u8);
        dest.push((offset & 0xff) as u8);
        dest.push(((offset >> 8) & 0xff) as u8);
    }

    Ok(())
}

/// Encode literal bytes into the destination buffer
fn encode_literals(dest: &mut Vec<u8>, literals: &[u8]) -> LcwResult<()> {
    let mut pos = 0;

    while pos < literals.len() {
        let chunk_size = cmp::min(literals.len() - pos, 63); // Maximum for medium copy

        // Medium copy from source: 10xxxxxx,data...
        let count_code = 0x80 | (chunk_size as u8);
        dest.push(count_code);
        dest.extend_from_slice(&literals[pos..pos + chunk_size]);

        pos += chunk_size;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let result = decompress(&[]);
        assert!(matches!(result, Err(LcwError::InputTooSmall)));
    }

    #[test]
    fn test_end_marker_only() {
        let compressed = [0x80];
        let result = decompress(&compressed).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_simple_literal() {
        // Test medium copy from source: 10000001 (copy 1 byte) + data + end marker
        let compressed = [0x81, b'A', 0x80];
        let result = decompress(&compressed).unwrap();
        assert_eq!(result, b"A");
    }

    #[test]
    fn test_multiple_literals() {
        // Test medium copy from source: 10000100 (copy 4 bytes) + data + end marker
        let compressed = [0x84, b'H', b'e', b'l', b'l', 0x80];
        let result = decompress(&compressed).unwrap();
        assert_eq!(result, b"Hell");
    }

    #[test]
    fn test_long_run() {
        // Test long run: 11111110 (long run marker) + count(5,0) + data(A) + end marker
        let compressed = [0xfe, 5, 0, b'A', 0x80];
        let result = decompress(&compressed).unwrap();
        assert_eq!(result, b"AAAAA");
    }

    #[test]
    fn test_round_trip_simple() {
        let original = b"Hello, World!";
        let compressed = compress(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original, &decompressed[..]);
    }

    #[test]
    fn test_round_trip_repeated_patterns() {
        let original = b"AAABBBCCCAAABBBCCC";
        let compressed = compress(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original, &decompressed[..]);
    }

    #[test]
    fn test_round_trip_long_string() {
        let original =
            "This is a longer test string with various patterns and repeated sequences. "
                .repeat(10);
        let original_bytes = original.as_bytes();
        let compressed = compress(original_bytes).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original_bytes, &decompressed[..]);
    }

    #[test]
    fn test_compression_ratio() {
        // Test with highly repetitive data
        let original = vec![0x42; 1000]; // 1000 bytes of 'B'
        let compressed = compress(&original).unwrap();

        // Should compress significantly
        assert!(compressed.len() < original.len() / 2);

        // Verify decompression
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_error_handling_truncated_input() {
        // Test with truncated medium copy command
        let compressed = [0x84, b'H', b'e']; // Missing bytes and end marker
        let result = decompress(&compressed);
        assert!(matches!(result, Err(LcwError::UnexpectedEndOfInput)));
    }

    #[test]
    fn test_error_handling_invalid_offset() {
        // Test short copy with invalid offset
        let compressed = [0x30, 0x10, 0x80]; // Try to copy from position 16 when buffer is empty
        let result = decompress(&compressed);
        assert!(matches!(result, Err(LcwError::InvalidOffset)));
    }

    #[test]
    fn test_various_data_sizes() {
        // Test edge cases with different data sizes
        let test_cases = vec![
            vec![0u8; 0],     // Empty
            vec![42u8; 1],    // Single byte
            vec![42u8; 2],    // Two bytes
            vec![42u8; 3],    // Three bytes (minimum for match)
            vec![42u8; 63],   // Maximum medium literal
            vec![42u8; 64],   // Just over medium literal
            vec![42u8; 255],  // Byte boundary
            vec![42u8; 1024], // Larger buffer
        ];

        for original in test_cases {
            if original.is_empty() {
                continue; // Skip empty case
            }

            let compressed = compress(&original).unwrap();
            let decompressed = decompress(&compressed).unwrap();
            assert_eq!(original, decompressed, "Failed for size {}", original.len());
        }
    }

    #[test]
    fn test_mixed_patterns() {
        // Test with a mix of literals, runs, and matches
        let original = b"Hello123Hello123AAAAAAAA456456End";
        let compressed = compress(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original, &decompressed[..]);
    }

    #[test]
    fn test_binary_data() {
        // Test with binary data including all byte values
        let mut original = Vec::new();
        for i in 0..=255u8 {
            original.push(i);
        }

        let compressed = compress(&original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original, decompressed);
    }
}
