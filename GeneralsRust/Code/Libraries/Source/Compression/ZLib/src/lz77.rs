//! LZ77 sliding window compression algorithm
//!
//! This module implements the LZ77 compression algorithm used in DEFLATE:
//! - Sliding window dictionary (up to 32KB)
//! - Hash chain matching
//! - Lazy matching for better compression
//! - Optimized match finding with configurable search depth

use crate::{CompressionLevel, Result, ZlibError};

/// Maximum window size (32KB for DEFLATE)
pub const WINDOW_SIZE: usize = 32768;

/// Maximum match length
pub const MAX_MATCH: usize = 258;

/// Minimum match length
pub const MIN_MATCH: usize = 3;

/// Hash table size (must be power of 2)
const HASH_SIZE: usize = 65536;

/// Hash shift amount
const HASH_SHIFT: u32 = 5;

/// Hash mask
const HASH_MASK: u32 = (HASH_SIZE - 1) as u32;

/// LZ77 match result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub length: u16,
    pub distance: u16,
}

impl Match {
    pub fn new(length: u16, distance: u16) -> Self {
        Self { length, distance }
    }

    pub fn is_better_than(&self, other: &Match) -> bool {
        self.length > other.length
    }
}

/// LZ77 literal or match
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LZ77Token {
    Literal(u8),
    Match { length: u16, distance: u16 },
}

/// LZ77 compressor with sliding window
pub struct LZ77Compressor {
    level: CompressionLevel,
    window: Vec<u8>,
    hash_table: Vec<u16>,
    prev: Vec<u16>,
    position: usize,
}

impl LZ77Compressor {
    /// Create new LZ77 compressor
    pub fn new(level: CompressionLevel) -> Self {
        Self {
            level,
            window: Vec::with_capacity(WINDOW_SIZE * 2),
            hash_table: vec![0; HASH_SIZE],
            prev: vec![0; WINDOW_SIZE],
            position: 0,
        }
    }

    /// Compress data into LZ77 tokens
    pub fn compress(&mut self, data: &[u8]) -> Result<Vec<LZ77Token>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        self.reset();
        let mut tokens = Vec::new();

        // Copy data to window
        self.window.extend_from_slice(data);

        let mut pos = 0;
        let data_len = data.len();

        while pos < data_len {
            if pos + MIN_MATCH > data_len {
                // Not enough data for a match
                tokens.push(LZ77Token::Literal(data[pos]));
                pos += 1;
                continue;
            }

            // Find best match
            let best_match = self.find_match(pos, data_len);

            if let Some(m) = best_match {
                if m.length >= MIN_MATCH as u16 {
                    // Use lazy matching if enabled
                    if self.level.lazy_match() && pos + 1 < data_len {
                        let next_match = self.find_match(pos + 1, data_len);

                        if let Some(next) = next_match {
                            if next.length > m.length {
                                // Next match is better, emit literal now
                                tokens.push(LZ77Token::Literal(data[pos]));
                                self.insert_hash(pos);
                                pos += 1;
                                continue;
                            }
                        }
                    }

                    // Emit match
                    tokens.push(LZ77Token::Match {
                        length: m.length,
                        distance: m.distance,
                    });

                    // Insert all positions in the match
                    for i in 0..m.length as usize {
                        if pos + i < data_len {
                            self.insert_hash(pos + i);
                        }
                    }

                    pos += m.length as usize;
                    continue;
                }
            }

            // No good match found, emit literal
            tokens.push(LZ77Token::Literal(data[pos]));
            self.insert_hash(pos);
            pos += 1;
        }

        Ok(tokens)
    }

    /// Find best match at current position
    fn find_match(&self, pos: usize, data_len: usize) -> Option<Match> {
        if pos + MIN_MATCH > data_len {
            return None;
        }

        let hash = self.hash_at(pos);
        let mut chain_pos = self.hash_table[hash as usize] as usize;

        let max_dist = WINDOW_SIZE;
        let max_len = std::cmp::min(MAX_MATCH, data_len - pos);
        let search_depth = self.level.search_depth();

        let mut best_match = None;
        let mut best_len = MIN_MATCH - 1;
        let mut depth = 0;

        while chain_pos > 0 && depth < search_depth {
            let distance = pos - chain_pos;

            if distance > max_dist {
                break;
            }

            // Check if we can get a longer match
            if self.window.get(chain_pos + best_len) == self.window.get(pos + best_len) {
                let len = self.match_length(pos, chain_pos, max_len);

                if len > best_len {
                    best_len = len;
                    best_match = Some(Match::new(len as u16, distance as u16));

                    if len >= self.level.good_length() || len == max_len {
                        break;
                    }
                }
            }

            chain_pos = self.prev[chain_pos % WINDOW_SIZE] as usize;
            depth += 1;
        }

        best_match
    }

    /// Calculate match length at two positions
    fn match_length(&self, pos1: usize, pos2: usize, max_len: usize) -> usize {
        let mut len = 0;

        while len < max_len {
            if self.window.get(pos1 + len) != self.window.get(pos2 + len) {
                break;
            }
            len += 1;
        }

        len
    }

    /// Compute hash for 3 bytes at position
    fn hash_at(&self, pos: usize) -> u32 {
        if pos + 2 >= self.window.len() {
            return 0;
        }

        let mut hash = self.window[pos] as u32;
        hash = ((hash << HASH_SHIFT) ^ self.window[pos + 1] as u32) & HASH_MASK;
        hash = ((hash << HASH_SHIFT) ^ self.window[pos + 2] as u32) & HASH_MASK;
        hash
    }

    /// Insert position into hash chain
    fn insert_hash(&mut self, pos: usize) {
        let hash = self.hash_at(pos);
        let window_pos = pos % WINDOW_SIZE;

        self.prev[window_pos] = self.hash_table[hash as usize];
        self.hash_table[hash as usize] = pos as u16;
    }

    /// Reset compressor state
    fn reset(&mut self) {
        self.window.clear();
        self.hash_table.fill(0);
        self.prev.fill(0);
        self.position = 0;
    }
}

/// LZ77 decompressor
pub struct LZ77Decompressor {
    window: Vec<u8>,
    position: usize,
}

impl LZ77Decompressor {
    /// Create new LZ77 decompressor
    pub fn new() -> Self {
        Self {
            window: Vec::new(),
            position: 0,
        }
    }

    /// Decompress LZ77 tokens
    pub fn decompress(&mut self, tokens: &[LZ77Token]) -> Result<Vec<u8>> {
        self.reset();

        for token in tokens {
            match token {
                LZ77Token::Literal(byte) => {
                    self.window.push(*byte);
                    self.position += 1;
                }
                LZ77Token::Match { length, distance } => {
                    if *distance as usize > self.position {
                        return Err(ZlibError::InvalidDeflateStream(format!(
                            "Invalid distance {} at position {}",
                            distance, self.position
                        )));
                    }

                    let start = self.position - *distance as usize;

                    // Handle overlapping matches (distance < length)
                    for _ in 0..*length {
                        let byte =
                            self.window[start + (self.window.len() - start) % (*distance as usize)];
                        self.window.push(byte);
                        self.position += 1;
                    }
                }
            }
        }

        Ok(self.window.clone())
    }

    /// Reset decompressor state
    fn reset(&mut self) {
        self.window.clear();
        self.position = 0;
    }
}

impl Default for LZ77Decompressor {
    fn default() -> Self {
        Self::new()
    }
}

/// Fast match finder using hash chains
pub struct MatchFinder {
    hash_table: Vec<u16>,
    prev: Vec<u16>,
    window_size: usize,
}

impl MatchFinder {
    /// Create new match finder
    pub fn new(window_size: usize) -> Self {
        Self {
            hash_table: vec![0; HASH_SIZE],
            prev: vec![0; window_size],
            window_size,
        }
    }

    /// Find matches at position
    pub fn find_matches(&mut self, data: &[u8], pos: usize, max_matches: usize) -> Vec<Match> {
        let mut matches = Vec::new();

        if pos + MIN_MATCH > data.len() {
            return matches;
        }

        let hash = Self::hash(data, pos);
        let mut chain_pos = self.hash_table[hash as usize] as usize;

        let max_dist = self.window_size;
        let max_len = std::cmp::min(MAX_MATCH, data.len() - pos);

        while chain_pos > 0 && matches.len() < max_matches {
            let distance = pos - chain_pos;

            if distance > max_dist {
                break;
            }

            let len = Self::match_length(data, pos, chain_pos, max_len);

            if len >= MIN_MATCH {
                matches.push(Match::new(len as u16, distance as u16));
            }

            chain_pos = self.prev[chain_pos % self.window_size] as usize;
        }

        matches.sort_by(|a, b| b.length.cmp(&a.length));
        matches
    }

    /// Insert position into hash chain
    pub fn insert(&mut self, data: &[u8], pos: usize) {
        let hash = Self::hash(data, pos);
        let window_pos = pos % self.window_size;

        self.prev[window_pos] = self.hash_table[hash as usize];
        self.hash_table[hash as usize] = pos as u16;
    }

    /// Compute hash at position
    fn hash(data: &[u8], pos: usize) -> u32 {
        if pos + 2 >= data.len() {
            return 0;
        }

        let mut hash = data[pos] as u32;
        hash = ((hash << HASH_SHIFT) ^ data[pos + 1] as u32) & HASH_MASK;
        hash = ((hash << HASH_SHIFT) ^ data[pos + 2] as u32) & HASH_MASK;
        hash
    }

    /// Calculate match length
    fn match_length(data: &[u8], pos1: usize, pos2: usize, max_len: usize) -> usize {
        let mut len = 0;

        while len < max_len && pos1 + len < data.len() && pos2 + len < data.len() {
            if data[pos1 + len] != data[pos2 + len] {
                break;
            }
            len += 1;
        }

        len
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.hash_table.fill(0);
        self.prev.fill(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lz77_literal_only() {
        let mut compressor = LZ77Compressor::new(CompressionLevel::Fast);
        let data = b"abc";
        let tokens = compressor.compress(data).unwrap();

        // Short data should be literals
        assert!(tokens.len() == 3);
        assert!(matches!(tokens[0], LZ77Token::Literal(b'a')));
    }

    #[test]
    fn test_lz77_simple_match() {
        let mut compressor = LZ77Compressor::new(CompressionLevel::Default);
        let data = b"abcabcabc";
        let tokens = compressor.compress(data).unwrap();

        // Should find repeated "abc"
        let has_match = tokens.iter().any(|t| matches!(t, LZ77Token::Match { .. }));
        assert!(has_match);
    }

    #[test]
    fn test_lz77_round_trip() {
        let mut compressor = LZ77Compressor::new(CompressionLevel::Default);
        let mut decompressor = LZ77Decompressor::new();

        let data = b"The quick brown fox jumps over the lazy dog. The quick brown fox!";
        let tokens = compressor.compress(data).unwrap();
        let decompressed = decompressor.decompress(&tokens).unwrap();

        assert_eq!(data, &decompressed[..]);
    }

    #[test]
    fn test_lz77_repeated_pattern() {
        let mut compressor = LZ77Compressor::new(CompressionLevel::Best);
        let data = b"aaaaaaaaaa";
        let tokens = compressor.compress(data).unwrap();

        // Should compress repeated 'a's efficiently
        assert!(tokens.len() < 10);
    }

    #[test]
    fn test_match_finder() {
        let mut finder = MatchFinder::new(WINDOW_SIZE);
        let data = b"abcdefabcdef";

        // Insert first occurrence of "abc"
        for i in 0..6 {
            finder.insert(data, i);
        }

        // Now look for matches at position 6 (second "abc")
        let matches = finder.find_matches(data, 6, 10);
        // Should find match with "abc" at position 0
        assert!(!matches.is_empty(), "Should find at least one match");
    }

    #[test]
    fn test_hash_consistency() {
        let data = b"hello world";
        let hash1 = MatchFinder::hash(data, 0);
        let hash2 = MatchFinder::hash(data, 0);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_overlapping_match() {
        let mut decompressor = LZ77Decompressor::new();

        // Pattern "aaa" can be encoded as literal 'a' + match(length=2, distance=1)
        let tokens = vec![
            LZ77Token::Literal(b'a'),
            LZ77Token::Match {
                length: 5,
                distance: 1,
            },
        ];

        let result = decompressor.decompress(&tokens).unwrap();
        assert_eq!(&result, b"aaaaaa");
    }

    #[test]
    fn test_compression_levels() {
        let data = b"This is a test. This is only a test. This is a test of compression.";

        for level in [
            CompressionLevel::Fast,
            CompressionLevel::Default,
            CompressionLevel::Best,
        ] {
            let mut compressor = LZ77Compressor::new(level);
            let tokens = compressor.compress(data).unwrap();
            assert!(!tokens.is_empty());
        }
    }
}
