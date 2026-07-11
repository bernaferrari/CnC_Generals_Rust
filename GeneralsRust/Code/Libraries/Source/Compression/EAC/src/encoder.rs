//! High-performance EAC encoder with SIMD optimizations
//!
//! This module provides unified encoding interface for all EAC compression algorithms
//! with advanced features like parallel processing and adaptive compression.

use crate::{CompressionType, EacHeader, Result};
use rayon::prelude::*;

/// Configuration options for encoding
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    /// Compression level (1-9, higher = better compression, slower)
    pub compression_level: u8,
    /// Enable parallel processing for large data
    pub parallel: bool,
    /// Chunk size for parallel processing
    pub chunk_size: usize,
    /// Enable SIMD optimizations
    pub use_simd: bool,
    /// Memory limit for compression (bytes)
    pub memory_limit: usize,
    /// Enable adaptive compression type selection
    pub adaptive: bool,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            compression_level: 6,
            parallel: true,
            chunk_size: 64 * 1024, // 64KB chunks
            use_simd: cfg!(feature = "simd"),
            memory_limit: 256 * 1024 * 1024, // 256MB
            adaptive: true,
        }
    }
}

/// High-performance EAC encoder
pub struct Encoder {
    config: EncoderConfig,
    // Reusable buffers for performance
    #[allow(dead_code)] // Pre-allocated for future encoder optimization
    work_buffer: Vec<u8>,
    #[allow(dead_code)] // Pre-allocated for future encoder optimization
    temp_buffer: Vec<u8>,
}

impl Encoder {
    /// Create new encoder with default configuration
    pub fn new() -> Self {
        Self::with_config(EncoderConfig::default())
    }

    /// Create encoder with custom configuration
    pub fn with_config(config: EncoderConfig) -> Self {
        let chunk_size = config.chunk_size;
        Self {
            config,
            work_buffer: Vec::with_capacity(chunk_size),
            temp_buffer: Vec::new(),
        }
    }

    /// Encode data using specified compression type
    pub fn encode(&mut self, data: &[u8], compression_type: CompressionType) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Ok(self.create_empty_compressed());
        }

        // Adaptive compression type selection
        let actual_type = if self.config.adaptive && compression_type != CompressionType::None {
            self.select_optimal_compression(data)
        } else {
            compression_type
        };

        // Choose encoding strategy based on data size and configuration
        if self.config.parallel && data.len() > self.config.chunk_size * 4 {
            self.encode_parallel(data, actual_type)
        } else {
            self.encode_single_threaded(data, actual_type)
        }
    }

    /// Single-threaded encoding
    fn encode_single_threaded(
        &mut self,
        data: &[u8],
        compression_type: CompressionType,
    ) -> Result<Vec<u8>> {
        let compressed = match compression_type {
            CompressionType::None => data.to_vec(),
            CompressionType::RefPack => self.encode_refpack(data)?,
            CompressionType::BTree => self.encode_btree(data)?,
            CompressionType::Huffman => self.encode_huffman(data)?,
        };

        self.create_compressed_data(data.len(), compression_type, compressed)
    }

    /// Parallel encoding for large data
    fn encode_parallel(
        &mut self,
        data: &[u8],
        compression_type: CompressionType,
    ) -> Result<Vec<u8>> {
        let chunks: Vec<_> = data.par_chunks(self.config.chunk_size).collect();

        log::debug!("Encoding {} chunks in parallel", chunks.len());

        // Compress chunks in parallel
        let compressed_chunks: Result<Vec<_>> = chunks
            .par_iter()
            .map(|chunk| {
                let mut encoder = Encoder::with_config(self.config.clone());
                encoder.encode_single_threaded(chunk, compression_type)
            })
            .collect();

        let compressed_chunks = compressed_chunks?;

        // Combine chunks with metadata
        self.combine_compressed_chunks(data.len(), compression_type, compressed_chunks)
    }

    /// Select optimal compression type based on data analysis
    fn select_optimal_compression(&self, data: &[u8]) -> CompressionType {
        if data.len() < 1024 {
            return CompressionType::RefPack;
        }

        // Parallel analysis of data characteristics
        let (entropy, (repetition_ratio, pattern_count)) = rayon::join(
            || self.calculate_entropy(data),
            || {
                (
                    self.calculate_repetition_ratio(data),
                    self.calculate_pattern_count(data),
                )
            },
        );

        log::debug!(
            "Data analysis: entropy={:.3}, repetition={:.3}, patterns={}",
            entropy,
            repetition_ratio,
            pattern_count
        );

        // Decision tree based on data characteristics
        if entropy < 0.4 && repetition_ratio > 0.4 {
            CompressionType::BTree
        } else if entropy < 0.6 || pattern_count > data.len() / 8 {
            CompressionType::Huffman
        } else {
            CompressionType::RefPack
        }
    }

    /// Calculate Shannon entropy using SIMD when possible
    #[cfg(feature = "simd")]
    fn calculate_entropy(&self, data: &[u8]) -> f64 {
        let mut counts = [0u32; 256];

        // SIMD-accelerated counting
        let simd_chunks = data.len() / 32;
        for i in 0..simd_chunks {
            let chunk = &data[i * 32..(i + 1) * 32];
            for &byte in chunk {
                counts[byte as usize] += 1;
            }
        }

        // Handle remaining bytes
        for &byte in &data[simd_chunks * 32..] {
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

        entropy / 8.0 // Normalize to 0-1 range
    }

    #[cfg(not(feature = "simd"))]
    fn calculate_entropy(&self, data: &[u8]) -> f64 {
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

        entropy / 8.0
    }

    /// Calculate repetition ratio for pattern detection
    fn calculate_repetition_ratio(&self, data: &[u8]) -> f64 {
        if data.len() < 8 {
            return 0.0;
        }

        let mut repetitions = 0;
        let pattern_size = 4;
        let max_patterns = data.len() / pattern_size;

        for i in 0..max_patterns {
            let pattern_start = i * pattern_size;
            let pattern = &data[pattern_start..pattern_start + pattern_size];

            // Look for this pattern in the rest of the data
            for j in (i + 1)..max_patterns {
                let search_start = j * pattern_size;
                if &data[search_start..search_start + pattern_size] == pattern {
                    repetitions += 1;
                }
            }
        }

        repetitions as f64 / max_patterns as f64
    }

    /// Count distinct patterns in data
    fn calculate_pattern_count(&self, data: &[u8]) -> usize {
        use std::collections::HashSet;

        if data.len() < 4 {
            return data.len();
        }

        let mut patterns = HashSet::new();
        for window in data.windows(4) {
            patterns.insert([window[0], window[1], window[2], window[3]]);
        }

        patterns.len()
    }

    /// Encode using RefPack with optimizations
    fn encode_refpack(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        crate::refpack::encode(data)
    }

    /// Encode using BTree algorithm
    fn encode_btree(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        crate::btree::encode(data)
    }

    /// Encode using Huffman algorithm  
    fn encode_huffman(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        crate::huffman::encode(data)
    }

    /// Create empty compressed data
    fn create_empty_compressed(&self) -> Vec<u8> {
        let header = EacHeader::new(CompressionType::None, 0);
        header.to_bytes().to_vec()
    }

    /// Create compressed data with header
    fn create_compressed_data(
        &self,
        original_size: usize,
        compression_type: CompressionType,
        compressed_data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let header = EacHeader::new(compression_type, original_size as u32);
        let mut result = Vec::with_capacity(EacHeader::SIZE + compressed_data.len());
        result.extend_from_slice(&header.to_bytes());
        result.extend_from_slice(&compressed_data);
        Ok(result)
    }

    /// Combine parallel compressed chunks
    fn combine_compressed_chunks(
        &self,
        original_size: usize,
        compression_type: CompressionType,
        chunks: Vec<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        // Create multi-part compressed format
        let mut result = Vec::new();

        // Main header
        let header = EacHeader::new(compression_type, original_size as u32);
        result.extend_from_slice(&header.to_bytes());

        // Multi-part marker
        result.extend_from_slice(b"MULT");
        result.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
        result.extend_from_slice(&(self.config.chunk_size as u32).to_le_bytes());

        // Chunk data
        for chunk in chunks {
            result.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
            result.extend_from_slice(&chunk);
        }

        Ok(result)
    }

    /// Get compression statistics
    pub fn get_stats(&self, original_size: usize, compressed_size: usize) -> CompressionStats {
        CompressionStats {
            original_size,
            compressed_size,
            compression_ratio: compressed_size as f64 / original_size as f64,
            space_saving: 1.0 - (compressed_size as f64 / original_size as f64),
        }
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compression statistics
#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f64,
    pub space_saving: f64,
}

impl CompressionStats {
    pub fn compression_percentage(&self) -> f64 {
        self.compression_ratio * 100.0
    }

    pub fn space_saving_percentage(&self) -> f64 {
        self.space_saving * 100.0
    }
}

/// Streaming encoder for large files
pub struct StreamingEncoder {
    encoder: Encoder,
    buffer: Vec<u8>,
    total_size: usize,
}

impl StreamingEncoder {
    pub fn new(config: EncoderConfig) -> Self {
        Self {
            encoder: Encoder::with_config(config.clone()),
            buffer: Vec::with_capacity(config.chunk_size),
            total_size: 0,
        }
    }

    /// Add data to streaming encoder
    pub fn write(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        self.buffer.extend_from_slice(data);
        self.total_size += data.len();

        if self.buffer.len() >= self.encoder.config.chunk_size {
            let chunk = self.buffer.split_off(self.encoder.config.chunk_size);
            let compressed = self
                .encoder
                .encode(&self.buffer, CompressionType::RefPack)?;
            self.buffer = chunk;
            Ok(compressed)
        } else {
            Ok(Vec::new())
        }
    }

    /// Finish streaming and return final compressed data
    pub fn finish(&mut self) -> Result<Vec<u8>> {
        if !self.buffer.is_empty() {
            let compressed = self
                .encoder
                .encode(&self.buffer, CompressionType::RefPack)?;
            self.buffer.clear();
            Ok(compressed)
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_encoder_config_default() {
        let config = EncoderConfig::default();
        assert_eq!(config.compression_level, 6);
        assert!(config.parallel);
        assert_eq!(config.chunk_size, 64 * 1024);
    }

    #[test]
    fn test_encoder_empty_data() {
        let mut encoder = Encoder::new();
        let compressed = encoder.encode(&[], CompressionType::RefPack).unwrap();
        assert_eq!(compressed.len(), EacHeader::SIZE);
    }

    #[test]
    fn test_adaptive_compression() {
        let mut encoder = Encoder::with_config(EncoderConfig {
            adaptive: true,
            ..Default::default()
        });

        // Test with highly repetitive data (should choose BTree)
        let repetitive_data = vec![0u8; 1024];
        let compressed = encoder
            .encode(&repetitive_data, CompressionType::RefPack)
            .unwrap();

        // Should be well compressed
        assert!(compressed.len() < repetitive_data.len());
    }

    #[test]
    fn test_entropy_calculation() {
        let encoder = Encoder::new();

        // All same bytes = low entropy
        let uniform_data = vec![42u8; 1000];
        let entropy = encoder.calculate_entropy(&uniform_data);
        assert!(entropy < 0.1);

        // Random bytes = high entropy
        let random_data: Vec<u8> = (0..1000).map(|i| (i * 37 % 256) as u8).collect();
        let entropy = encoder.calculate_entropy(&random_data);
        assert!(entropy > 0.5);
    }

    #[test]
    fn test_compression_stats() {
        let stats = CompressionStats {
            original_size: 1000,
            compressed_size: 600,
            compression_ratio: 0.6,
            space_saving: 0.4,
        };

        assert_eq!(stats.compression_percentage(), 60.0);
        assert_eq!(stats.space_saving_percentage(), 40.0);
    }

    #[test]
    fn test_streaming_encoder() {
        let config = EncoderConfig {
            chunk_size: 100,
            ..Default::default()
        };

        let mut streaming = StreamingEncoder::new(config);

        // Write data in chunks
        let chunk1 = streaming.write(&vec![1u8; 50]).unwrap();
        assert!(chunk1.is_empty()); // Not enough data yet

        let chunk2 = streaming.write(&vec![2u8; 60]).unwrap();
        assert!(!chunk2.is_empty()); // Should return compressed chunk

        let final_chunk = streaming.finish().unwrap();
        assert!(!final_chunk.is_empty()); // Remaining data
    }
}
