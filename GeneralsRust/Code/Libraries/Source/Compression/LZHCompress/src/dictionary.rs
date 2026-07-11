//! Sliding Window Dictionary for LZ77 Compression
//!
//! This module implements the sliding window dictionary used in LZ77 compression.
//! The dictionary maintains a history buffer and provides efficient pattern matching.
//!
//! ## Algorithm
//!
//! - **Sliding Window**: Fixed-size circular buffer of recent data
//! - **Hash Table**: Fast lookup structure for finding potential matches
//! - **Hash Chains**: Multiple positions for each hash value
//! - **Match Finding**: Scan hash chains to find longest matching string
//!
//! ## Performance Optimizations
//!
//! - Rolling hash for fast hash updates
//! - Limited chain search depth for speed/quality tradeoff
//! - SIMD acceleration for pattern comparison (when enabled)

use crate::LzhMatch;
use std::collections::HashMap;

/// Sliding window dictionary for LZ77 compression
pub struct Dictionary {
    // Sliding window buffer
    window: Vec<u8>,
    window_size: usize,
    window_pos: usize,
    bytes_in_window: usize,

    // Hash table for fast pattern matching
    hash_table: HashMap<u32, Vec<usize>>,
    hash_mask: u32,

    // Hash chain limits
    max_chain_length: usize,
}

impl Dictionary {
    /// Create a new dictionary with specified window size
    pub fn new(window_size: usize) -> Self {
        Self {
            window: vec![0; window_size],
            window_size,
            window_pos: 0,
            bytes_in_window: 0,
            hash_table: HashMap::new(),
            hash_mask: 0xFFFF,
            max_chain_length: 256,
        }
    }

    /// Add a byte to the dictionary
    pub fn add_byte(&mut self, byte: u8) {
        self.window[self.window_pos] = byte;
        self.window_pos = (self.window_pos + 1) % self.window_size;

        if self.bytes_in_window < self.window_size {
            self.bytes_in_window += 1;
        }

        // Update hash table when we have enough bytes for a pattern
        if self.bytes_in_window >= 3 {
            self.update_hash();
        }
    }

    /// Find the longest match in the dictionary
    ///
    /// Returns a match with (length, distance) if found.
    pub fn find_longest_match(
        &self,
        data: &[u8],
        min_length: usize,
        max_length: usize,
        max_search_depth: usize,
    ) -> LzhMatch {
        if data.len() < min_length {
            return LzhMatch::new(0, 0);
        }

        // Calculate hash for the current position
        let hash = self.calculate_hash(data);

        // Look up positions in hash table
        let positions = match self.hash_table.get(&hash) {
            Some(pos) => pos,
            None => return LzhMatch::new(0, 0),
        };

        let mut best_match = LzhMatch::new(0, 0);
        let search_limit = std::cmp::min(positions.len(), max_search_depth);

        // Search through hash chain for best match
        for &pos in positions.iter().rev().take(search_limit) {
            let distance = self.calculate_distance(pos);

            // Skip if distance is invalid
            if distance == 0 || distance > self.bytes_in_window {
                continue;
            }

            // Find match length at this position
            let match_length = self.find_match_length(pos, data, max_length);

            // Update best match if this is better
            if match_length >= min_length && match_length > best_match.length {
                best_match = LzhMatch::new(match_length, distance);

                // Early exit if we found maximum possible match
                if match_length >= max_length {
                    break;
                }
            }
        }

        best_match
    }

    /// Calculate hash value for pattern
    fn calculate_hash(&self, data: &[u8]) -> u32 {
        if data.len() < 3 {
            return 0;
        }

        // Simple hash function (rolling hash)
        let h1 = data[0] as u32;
        let h2 = data[1] as u32;
        let h3 = data[2] as u32;

        ((h1 << 10) ^ (h2 << 5) ^ h3) & self.hash_mask
    }

    /// Update hash table with current window position
    fn update_hash(&mut self) {
        if self.bytes_in_window < 3 {
            return;
        }

        // Get the pattern at current position
        let pos = (self.window_pos + self.window_size - 1) % self.window_size;
        let pattern = self.get_pattern_at(pos);

        // Calculate hash
        let hash = self.calculate_hash(&pattern);

        // Add position to hash chain
        self.hash_table.entry(hash).or_default().push(pos);

        // Limit hash chain length to prevent excessive memory usage
        let chain = self.hash_table.get_mut(&hash).unwrap();
        if chain.len() > self.max_chain_length {
            chain.remove(0);
        }
    }

    /// Get pattern bytes at position
    fn get_pattern_at(&self, pos: usize) -> Vec<u8> {
        let mut pattern = Vec::with_capacity(3);
        for i in 0..3 {
            let idx = (pos + i) % self.window_size;
            pattern.push(self.window[idx]);
        }
        pattern
    }

    /// Calculate distance from current position
    fn calculate_distance(&self, pos: usize) -> usize {
        let current = if self.window_pos == 0 {
            self.window_size
        } else {
            self.window_pos
        };

        if pos < current {
            current - pos
        } else {
            self.window_size - pos + current
        }
    }

    /// Find match length at specific position
    fn find_match_length(&self, dict_pos: usize, data: &[u8], max_length: usize) -> usize {
        let max_len = std::cmp::min(max_length, data.len());
        let mut length = 0;

        for (i, byte) in data.iter().take(max_len).enumerate() {
            let window_idx = (dict_pos + i) % self.window_size;
            if self.window[window_idx] != *byte {
                break;
            }
            length += 1;
        }

        length
    }

    /// Clear the dictionary
    pub fn clear(&mut self) {
        self.window.fill(0);
        self.window_pos = 0;
        self.bytes_in_window = 0;
        self.hash_table.clear();
    }

    /// Get current window size
    pub fn window_size(&self) -> usize {
        self.window_size
    }

    /// Get number of bytes currently in window
    pub fn bytes_in_window(&self) -> usize {
        self.bytes_in_window
    }
}

impl Default for Dictionary {
    fn default() -> Self {
        Self::new(8192)
    }
}

/// SIMD-accelerated match finding (when feature enabled)
#[cfg(feature = "simd")]
mod simd_match {
    use super::*;

    impl Dictionary {
        /// Find match length using SIMD comparison
        pub fn find_match_length_simd(
            &self,
            dict_pos: usize,
            data: &[u8],
            max_length: usize,
        ) -> usize {
            use wide::*;

            let max_len = std::cmp::min(max_length, data.len());
            let mut length = 0;

            // Process 16 bytes at a time with SIMD
            const SIMD_WIDTH: usize = 16;
            while length + SIMD_WIDTH <= max_len {
                let window_start = (dict_pos + length) % self.window_size;

                // Check if we can do a contiguous comparison
                if window_start + SIMD_WIDTH <= self.window_size {
                    let window_slice = &self.window[window_start..window_start + SIMD_WIDTH];
                    let data_slice = &data[length..length + SIMD_WIDTH];

                    // Load into SIMD registers
                    let window_vec = u8x16::new([
                        window_slice[0],
                        window_slice[1],
                        window_slice[2],
                        window_slice[3],
                        window_slice[4],
                        window_slice[5],
                        window_slice[6],
                        window_slice[7],
                        window_slice[8],
                        window_slice[9],
                        window_slice[10],
                        window_slice[11],
                        window_slice[12],
                        window_slice[13],
                        window_slice[14],
                        window_slice[15],
                    ]);
                    let data_vec = u8x16::new([
                        data_slice[0],
                        data_slice[1],
                        data_slice[2],
                        data_slice[3],
                        data_slice[4],
                        data_slice[5],
                        data_slice[6],
                        data_slice[7],
                        data_slice[8],
                        data_slice[9],
                        data_slice[10],
                        data_slice[11],
                        data_slice[12],
                        data_slice[13],
                        data_slice[14],
                        data_slice[15],
                    ]);

                    // Compare
                    if window_vec == data_vec {
                        length += SIMD_WIDTH;
                    } else {
                        // Find first mismatch
                        for i in 0..SIMD_WIDTH {
                            if window_slice[i] != data_slice[i] {
                                return length + i;
                            }
                        }
                        break;
                    }
                } else {
                    // Wrap-around case, fall back to scalar
                    break;
                }
            }

            // Handle remaining bytes with scalar comparison
            while length < max_len {
                let window_idx = (dict_pos + length) % self.window_size;
                if self.window[window_idx] != data[length] {
                    break;
                }
                length += 1;
            }

            length
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_creation() {
        let dict = Dictionary::new(4096);
        assert_eq!(dict.window_size(), 4096);
        assert_eq!(dict.bytes_in_window(), 0);
    }

    #[test]
    fn test_add_byte() {
        let mut dict = Dictionary::new(1024);
        dict.add_byte(b'A');
        assert_eq!(dict.bytes_in_window(), 1);

        for _ in 0..100 {
            dict.add_byte(b'B');
        }
        assert_eq!(dict.bytes_in_window(), 101);
    }

    #[test]
    fn test_find_match_simple() {
        let mut dict = Dictionary::new(1024);

        // Add pattern "ABC" to dictionary
        dict.add_byte(b'A');
        dict.add_byte(b'B');
        dict.add_byte(b'C');

        // Try to find "ABC" again
        let data = b"ABCD";
        let match_result = dict.find_longest_match(data, 3, 10, 16);

        assert!(match_result.length >= 3);
        assert!(match_result.distance > 0);
    }

    #[test]
    fn test_no_match() {
        let mut dict = Dictionary::new(1024);

        // Add some data
        dict.add_byte(b'A');
        dict.add_byte(b'B');
        dict.add_byte(b'C');

        // Try to find completely different pattern
        let data = b"XYZ";
        let match_result = dict.find_longest_match(data, 3, 10, 16);

        assert_eq!(match_result.length, 0);
    }

    #[test]
    fn test_repetitive_data() {
        let mut dict = Dictionary::new(1024);

        // Add repetitive pattern
        for _ in 0..10 {
            dict.add_byte(b'A');
        }

        // Should find long match
        let data = b"AAAAAAAAAA";
        let match_result = dict.find_longest_match(data, 3, 100, 32);

        assert!(match_result.length >= 3);
        assert!(match_result.is_valid());
    }

    #[test]
    fn test_window_wraparound() {
        let mut dict = Dictionary::new(16);

        // Fill window and then some
        for i in 0..32 {
            dict.add_byte((i % 256) as u8);
        }

        assert_eq!(dict.bytes_in_window(), 16); // Should be capped at window size
    }

    #[test]
    fn test_clear() {
        let mut dict = Dictionary::new(1024);

        dict.add_byte(b'A');
        dict.add_byte(b'B');
        dict.add_byte(b'C');

        dict.clear();

        assert_eq!(dict.bytes_in_window(), 0);
    }

    #[test]
    fn test_hash_calculation() {
        let dict = Dictionary::new(1024);

        let pattern1 = b"ABC";
        let pattern2 = b"ABC";
        let pattern3 = b"XYZ";

        let hash1 = dict.calculate_hash(pattern1);
        let hash2 = dict.calculate_hash(pattern2);
        let hash3 = dict.calculate_hash(pattern3);

        // Same patterns should have same hash
        assert_eq!(hash1, hash2);

        // Different patterns should (usually) have different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_match_length_calculation() {
        let mut dict = Dictionary::new(1024);

        // Add "ABCDEF" to dictionary
        for &byte in b"ABCDEF" {
            dict.add_byte(byte);
        }

        // Find match length for "ABCXYZ"
        let data = b"ABCXYZ";
        let length = dict.find_match_length(0, data, 10);

        assert_eq!(length, 3); // Should match "ABC"
    }

    #[test]
    fn test_distance_calculation() {
        let mut dict = Dictionary::new(100);

        // Add some bytes
        for i in 0..10 {
            dict.add_byte(i);
        }

        // Distance should be calculated correctly
        let dist = dict.calculate_distance(0);
        assert_eq!(dist, 10);
    }
}
