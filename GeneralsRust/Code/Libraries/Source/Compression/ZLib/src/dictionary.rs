//! Custom compression dictionaries
//!
//! This module provides support for custom compression dictionaries:
//! - Pre-trained dictionaries for specialized data
//! - Dictionary training from sample data
//! - Integration with DEFLATE compression

#![cfg(feature = "custom_dictionary")]

use crate::{Result, ZlibError};

/// Compression dictionary
pub struct Dictionary {
    data: Vec<u8>,
    id: u32,
}

impl Dictionary {
    /// Create new dictionary from data
    pub fn new(data: Vec<u8>) -> Self {
        let id = Self::compute_id(&data);
        Self { data, id }
    }

    /// Train dictionary from sample data
    pub fn train(samples: &[&[u8]], max_size: usize) -> Result<Self> {
        if samples.is_empty() {
            return Err(ZlibError::CompressionFailed(
                "No samples provided for training".to_string(),
            ));
        }

        // Simple dictionary training: concatenate samples up to max_size
        let mut dict_data = Vec::new();

        for sample in samples {
            if dict_data.len() + sample.len() > max_size {
                break;
            }
            dict_data.extend_from_slice(sample);
        }

        Ok(Self::new(dict_data))
    }

    /// Get dictionary ID (Adler-32 checksum)
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Get dictionary data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Compute dictionary ID
    fn compute_id(data: &[u8]) -> u32 {
        const MOD_ADLER: u32 = 65521;
        let mut a = 1u32;
        let mut b = 0u32;

        for &byte in data {
            a = (a + byte as u32) % MOD_ADLER;
            b = (b + a) % MOD_ADLER;
        }

        (b << 16) | a
    }
}

/// Dictionary-based compressor
pub struct DictionaryCompressor {
    dictionary: Dictionary,
}

impl DictionaryCompressor {
    /// Create compressor with dictionary
    pub fn new(dictionary: Dictionary) -> Self {
        Self { dictionary }
    }

    /// Compress with dictionary
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        // For now, fallback to standard compression
        // A full implementation would pre-load the dictionary into the LZ77 window
        crate::compress(data, crate::CompressionLevel::Default)
    }

    /// Get dictionary
    pub fn dictionary(&self) -> &Dictionary {
        &self.dictionary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dictionary_creation() {
        let data = b"Sample dictionary data".to_vec();
        let dict = Dictionary::new(data);

        assert_eq!(dict.data().len(), 22);
        assert_ne!(dict.id(), 0);
    }

    #[test]
    fn test_dictionary_training() {
        let samples = vec![b"sample1" as &[u8], b"sample2", b"sample3"];
        let dict = Dictionary::train(&samples, 1024).unwrap();

        assert!(!dict.data().is_empty());
    }

    #[test]
    fn test_dictionary_id_deterministic() {
        let data = b"Test data".to_vec();
        let dict1 = Dictionary::new(data.clone());
        let dict2 = Dictionary::new(data);

        assert_eq!(dict1.id(), dict2.id());
    }
}
