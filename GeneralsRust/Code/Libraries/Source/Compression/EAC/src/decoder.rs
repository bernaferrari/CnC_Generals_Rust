//! High-performance EAC decoder with SIMD optimizations
//!
//! This module provides unified decoding interface for all EAC compression algorithms
//! with support for parallel processing and streaming decompression.

use crate::{CompressionType, EacError, EacHeader, Result};
use rayon::prelude::*;

/// Configuration for decoder behavior
#[derive(Debug, Clone)]
pub struct DecoderConfig {
    /// Enable parallel processing for multi-part compressed data
    pub parallel: bool,
    /// Maximum memory usage for decompression buffers
    pub memory_limit: usize,
    /// Enable SIMD optimizations
    pub use_simd: bool,
    /// Verify decompressed data integrity
    pub verify_integrity: bool,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            parallel: true,
            memory_limit: 512 * 1024 * 1024, // 512MB
            use_simd: cfg!(feature = "simd"),
            verify_integrity: true,
        }
    }
}

/// High-performance EAC decoder
pub struct Decoder {
    config: DecoderConfig,
    // Reusable buffers for performance
    #[allow(dead_code)] // Pre-allocated for future decoder optimization
    work_buffer: Vec<u8>,
    #[allow(dead_code)] // Pre-allocated for future decoder verification
    verify_buffer: Vec<u8>,
}

impl Decoder {
    /// Create new decoder with default configuration
    pub fn new() -> Self {
        Self::with_config(DecoderConfig::default())
    }

    /// Create decoder with custom configuration
    pub fn with_config(config: DecoderConfig) -> Self {
        Self {
            config,
            work_buffer: Vec::new(),
            verify_buffer: Vec::new(),
        }
    }

    /// Decode EAC compressed data
    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < EacHeader::SIZE {
            return Err(EacError::BufferTooSmall {
                needed: EacHeader::SIZE,
                available: data.len(),
            });
        }

        // Parse header
        let header = EacHeader::from_bytes(data)?;
        let compressed_data = &data[EacHeader::SIZE..];

        log::debug!(
            "Decoding {} compressed data, expected size: {}",
            header.compression_type.name(),
            header.uncompressed_size
        );

        // Check if this is multi-part compressed data
        if compressed_data.len() >= 4 && &compressed_data[0..4] == b"MULT" {
            self.decode_multipart(compressed_data, &header)
        } else {
            self.decode_single_part(compressed_data, &header)
        }
    }

    /// Decode single-part compressed data
    fn decode_single_part(&mut self, data: &[u8], header: &EacHeader) -> Result<Vec<u8>> {
        let expected_size = header.uncompressed_size as usize;

        // Check memory limit
        if expected_size > self.config.memory_limit {
            return Err(EacError::DecompressionFailed(format!(
                "Decompressed size {} exceeds memory limit {}",
                expected_size, self.config.memory_limit
            )));
        }

        let result = match header.compression_type {
            CompressionType::None => {
                if data.len() != expected_size {
                    return Err(EacError::DecompressionFailed(format!(
                        "Uncompressed size mismatch: expected {}, got {}",
                        expected_size,
                        data.len()
                    )));
                }
                data.to_vec()
            }
            CompressionType::RefPack => self.decode_refpack(data, expected_size)?,
            CompressionType::BTree => self.decode_btree(data, expected_size)?,
            CompressionType::Huffman => self.decode_huffman(data, expected_size)?,
        };

        // Verify integrity if enabled
        if self.config.verify_integrity && result.len() != expected_size {
            return Err(EacError::DecompressionFailed(format!(
                "Size verification failed: expected {}, got {}",
                expected_size,
                result.len()
            )));
        }

        Ok(result)
    }

    /// Decode multi-part compressed data (parallel)
    fn decode_multipart(&mut self, data: &[u8], header: &EacHeader) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(EacError::DecompressionFailed(
                "Multi-part header too short".to_string(),
            ));
        }

        // Parse multi-part header
        let chunk_count = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let chunk_size = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

        log::debug!("Decoding {} chunks of size {}", chunk_count, chunk_size);

        // Parse chunk metadata
        let mut chunks = Vec::with_capacity(chunk_count);
        let mut offset = 12;

        for _ in 0..chunk_count {
            if offset + 4 > data.len() {
                return Err(EacError::DecompressionFailed(
                    "Unexpected end of chunk metadata".to_string(),
                ));
            }

            let chunk_compressed_size = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + chunk_compressed_size > data.len() {
                return Err(EacError::DecompressionFailed(
                    "Chunk data extends beyond buffer".to_string(),
                ));
            }

            chunks.push(&data[offset..offset + chunk_compressed_size]);
            offset += chunk_compressed_size;
        }

        // Decode chunks in parallel if enabled
        let decompressed_chunks: Result<Vec<_>> = if self.config.parallel && chunk_count > 1 {
            chunks
                .par_iter()
                .map(|chunk_data| {
                    let mut decoder = Decoder::with_config(self.config.clone());
                    decoder.decode(chunk_data)
                })
                .collect()
        } else {
            chunks
                .iter()
                .map(|chunk_data| self.decode(chunk_data))
                .collect()
        };

        let decompressed_chunks = decompressed_chunks?;

        // Combine chunks
        let mut result = Vec::with_capacity(header.uncompressed_size as usize);
        for chunk in decompressed_chunks {
            result.extend_from_slice(&chunk);
        }

        // Verify total size
        if self.config.verify_integrity && result.len() != header.uncompressed_size as usize {
            return Err(EacError::DecompressionFailed(format!(
                "Multi-part size verification failed: expected {}, got {}",
                header.uncompressed_size,
                result.len()
            )));
        }

        Ok(result)
    }

    /// Decode RefPack compressed data
    fn decode_refpack(&mut self, data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        crate::refpack::decode(data, expected_size)
    }

    /// Decode BTree compressed data
    fn decode_btree(&mut self, data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        crate::btree::decode(data, expected_size)
    }

    /// Decode Huffman compressed data
    fn decode_huffman(&mut self, data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
        crate::huffman::decode(data, expected_size)
    }

    /// Get information about compressed data without decompressing
    pub fn probe(&self, data: &[u8]) -> Result<CompressionInfo> {
        if data.len() < EacHeader::SIZE {
            return Err(EacError::BufferTooSmall {
                needed: EacHeader::SIZE,
                available: data.len(),
            });
        }

        let header = EacHeader::from_bytes(data)?;
        let compressed_data = &data[EacHeader::SIZE..];

        let is_multipart = compressed_data.len() >= 4 && &compressed_data[0..4] == b"MULT";

        let (chunk_count, chunk_size) = if is_multipart && compressed_data.len() >= 12 {
            let chunk_count = u32::from_le_bytes([
                compressed_data[4],
                compressed_data[5],
                compressed_data[6],
                compressed_data[7],
            ]) as usize;
            let chunk_size = u32::from_le_bytes([
                compressed_data[8],
                compressed_data[9],
                compressed_data[10],
                compressed_data[11],
            ]) as usize;
            (chunk_count, chunk_size)
        } else {
            (1, compressed_data.len())
        };

        Ok(CompressionInfo {
            compression_type: header.compression_type,
            uncompressed_size: header.uncompressed_size as usize,
            compressed_size: data.len(),
            is_multipart,
            chunk_count,
            chunk_size,
            compression_ratio: data.len() as f64 / header.uncompressed_size as f64,
        })
    }

    /// Validate compressed data format without decompressing
    pub fn validate(&self, data: &[u8]) -> Result<bool> {
        let info = self.probe(data)?;

        // Basic format validation
        if info.uncompressed_size == 0 && info.compression_type != CompressionType::None {
            return Ok(false);
        }

        if info.compressed_size < EacHeader::SIZE {
            return Ok(false);
        }

        // More detailed validation could be added here
        Ok(true)
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about compressed data
#[derive(Debug, Clone)]
pub struct CompressionInfo {
    pub compression_type: CompressionType,
    pub uncompressed_size: usize,
    pub compressed_size: usize,
    pub is_multipart: bool,
    pub chunk_count: usize,
    pub chunk_size: usize,
    pub compression_ratio: f64,
}

impl CompressionInfo {
    pub fn compression_percentage(&self) -> f64 {
        self.compression_ratio * 100.0
    }

    pub fn space_saving_percentage(&self) -> f64 {
        (1.0 - self.compression_ratio) * 100.0
    }

    pub fn is_compressed(&self) -> bool {
        self.compression_type != CompressionType::None
    }
}

/// Streaming decoder for large files
pub struct StreamingDecoder {
    decoder: Decoder,
    buffer: Vec<u8>,
    expected_total_size: Option<usize>,
    decoded_size: usize,
}

impl StreamingDecoder {
    pub fn new(config: DecoderConfig) -> Self {
        Self {
            decoder: Decoder::with_config(config),
            buffer: Vec::new(),
            expected_total_size: None,
            decoded_size: 0,
        }
    }

    /// Add compressed data to streaming decoder
    pub fn write(&mut self, data: &[u8]) -> Result<Vec<u8>> {
        self.buffer.extend_from_slice(data);

        // Try to decode if we have enough data
        if self.buffer.len() >= EacHeader::SIZE {
            // Try to decode a complete chunk
            if let Ok(decoded) = self.decoder.decode(&self.buffer) {
                self.buffer.clear();
                self.decoded_size += decoded.len();
                Ok(decoded)
            } else {
                Ok(Vec::new()) // Need more data
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Finish streaming and return any remaining decoded data
    pub fn finish(&mut self) -> Result<Vec<u8>> {
        if !self.buffer.is_empty() {
            let decoded = self.decoder.decode(&self.buffer)?;
            self.buffer.clear();
            self.decoded_size += decoded.len();
            Ok(decoded)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get current decoding progress (if total size is known)
    pub fn progress(&self) -> Option<f64> {
        self.expected_total_size.map(|total| {
            if total > 0 {
                self.decoded_size as f64 / total as f64
            } else {
                1.0
            }
        })
    }
}

/// Batch decoder for processing multiple compressed files
pub struct BatchDecoder {
    decoder: Decoder,
}

impl BatchDecoder {
    pub fn new(config: DecoderConfig) -> Self {
        Self {
            decoder: Decoder::with_config(config),
        }
    }

    /// Decode multiple compressed data in parallel
    pub fn decode_batch(&mut self, data_list: &[&[u8]]) -> Vec<Result<Vec<u8>>> {
        data_list
            .par_iter()
            .map(|data| {
                let mut decoder = Decoder::with_config(self.decoder.config.clone());
                decoder.decode(data)
            })
            .collect()
    }

    /// Get information about multiple compressed data
    pub fn probe_batch(&self, data_list: &[&[u8]]) -> Vec<Result<CompressionInfo>> {
        data_list
            .par_iter()
            .map(|data| self.decoder.probe(data))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compress, CompressionType};

    #[test]
    fn test_decoder_config_default() {
        let config = DecoderConfig::default();
        assert!(config.parallel);
        assert_eq!(config.memory_limit, 512 * 1024 * 1024);
        assert!(config.verify_integrity);
    }

    #[test]
    fn test_decode_uncompressed() {
        let mut decoder = Decoder::new();

        // Create uncompressed data
        let original = b"Hello, World!";
        let compressed = compress(original, CompressionType::None).unwrap();
        let decompressed = decoder.decode(&compressed).unwrap();

        assert_eq!(original, &decompressed[..]);
    }

    #[test]
    fn test_probe_compressed_data() {
        let decoder = Decoder::new();

        let original = b"This is test data for compression analysis.";
        let compressed = compress(original, CompressionType::RefPack).unwrap();
        let info = decoder.probe(&compressed).unwrap();

        assert_eq!(info.compression_type, CompressionType::RefPack);
        assert_eq!(info.uncompressed_size, original.len());
        assert!(info.is_compressed());
        assert!(!info.is_multipart);
    }

    #[test]
    fn test_validate_compressed_data() {
        let decoder = Decoder::new();

        // Valid compressed data
        let original = b"Valid test data";
        let compressed = compress(original, CompressionType::RefPack).unwrap();
        assert!(decoder.validate(&compressed).unwrap());

        // Invalid data (too short)
        let invalid_data = b"ABC";
        assert!(!decoder.validate(invalid_data).unwrap());
    }

    #[test]
    fn test_streaming_decoder() {
        let config = DecoderConfig::default();
        let mut streaming = StreamingDecoder::new(config);

        // Create test data
        let original = b"This is streaming test data that will be processed in chunks.";
        let compressed = compress(original, CompressionType::RefPack).unwrap();

        // Process in parts
        let part1 = &compressed[..compressed.len() / 2];
        let part2 = &compressed[compressed.len() / 2..];

        let result1 = streaming.write(part1).unwrap();
        let result2 = streaming.write(part2).unwrap();
        let final_result = streaming.finish().unwrap();

        // Combine results
        let mut total_result = Vec::new();
        total_result.extend_from_slice(&result1);
        total_result.extend_from_slice(&result2);
        total_result.extend_from_slice(&final_result);

        assert_eq!(original, &total_result[..]);
    }

    #[test]
    fn test_batch_decoder() {
        let config = DecoderConfig::default();
        let mut batch = BatchDecoder::new(config);

        // Create multiple compressed data
        let data1 = compress(b"First test data", CompressionType::RefPack).unwrap();
        let data2 = compress(b"Second test data", CompressionType::BTree).unwrap();
        let data3 = compress(b"Third test data", CompressionType::Huffman).unwrap();

        let batch_data = [data1.as_slice(), data2.as_slice(), data3.as_slice()];
        let results = batch.decode_batch(&batch_data);

        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_ok());

        assert_eq!(results[0].as_ref().unwrap(), b"First test data");
        assert_eq!(results[1].as_ref().unwrap(), b"Second test data");
        assert_eq!(results[2].as_ref().unwrap(), b"Third test data");
    }

    #[test]
    fn test_compression_info() {
        let info = CompressionInfo {
            compression_type: CompressionType::RefPack,
            uncompressed_size: 1000,
            compressed_size: 600,
            is_multipart: false,
            chunk_count: 1,
            chunk_size: 600,
            compression_ratio: 0.6,
        };

        assert_eq!(info.compression_percentage(), 60.0);
        assert_eq!(info.space_saving_percentage(), 40.0);
        assert!(info.is_compressed());
    }

    #[test]
    fn test_memory_limit() {
        let config = DecoderConfig {
            memory_limit: 100, // Very small limit
            ..Default::default()
        };

        let mut decoder = Decoder::with_config(config);

        // Try to decode data that would exceed memory limit
        let large_data = vec![0u8; 1000];
        let compressed = compress(&large_data, CompressionType::None).unwrap();

        let result = decoder.decode(&compressed);
        assert!(result.is_err());

        if let Err(EacError::DecompressionFailed(msg)) = result {
            assert!(msg.contains("exceeds memory limit"));
        }
    }
}
