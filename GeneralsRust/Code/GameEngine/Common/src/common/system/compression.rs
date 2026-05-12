////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Compression System Implementation
//!
//! Provides data compression and decompression functionality for the game engine.
//! Supports various compression algorithms for file storage and network transmission.
//!
//! Rust conversion: 2025

use once_cell::sync::OnceCell;
use std::io::{self, Read};

use generals_compression::compression_manager::{
    CompressionManager as LegacyCompressionManager, CompressionType as LegacyCompressionType,
};

/// Compression types supported by the engine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionType {
    None,
    Zlib,
    LZ4,
    RefPack,
}

/// Compression level settings
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CompressionLevel {
    None = 0,
    Fast = 1,
    Default = 6,
    Best = 9,
}

const COMPRESSED_MAGIC: [u8; 4] = *b"CMP\0";

#[derive(Debug, Clone, Copy)]
struct CompressedHeader {
    compression_type: CompressionType,
    original_size: u32,
}

impl CompressedHeader {
    fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        if data[0..4] != COMPRESSED_MAGIC {
            return None;
        }
        let compression_type = match data[4] {
            0 => CompressionType::None,
            1 => CompressionType::Zlib,
            2 => CompressionType::LZ4,
            3 => CompressionType::RefPack,
            _ => return None,
        };
        let original_size = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);
        Some(Self {
            compression_type,
            original_size,
        })
    }
}

pub fn get_preferred_compression() -> CompressionType {
    CompressionType::RefPack
}

pub fn is_data_compressed(data: &[u8]) -> bool {
    LegacyCompressionManager::is_data_compressed(data) || CompressedHeader::decode(data).is_some()
}

pub fn get_uncompressed_size(data: &[u8]) -> Option<usize> {
    if LegacyCompressionManager::is_data_compressed(data) {
        return Some(LegacyCompressionManager::get_uncompressed_size(data) as usize);
    }

    CompressedHeader::decode(data).map(|header| header.original_size as usize)
}

pub fn compress_data(
    data: &[u8],
    compression_type: CompressionType,
    level: CompressionLevel,
) -> Result<Vec<u8>, io::Error> {
    if compression_type == CompressionType::None {
        return Ok(data.to_vec());
    }

    let legacy_type = legacy_compression_type(compression_type, level)?;
    let max_size =
        LegacyCompressionManager::get_max_compressed_size(data.len() as i32, legacy_type);
    if max_size <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsupported compression type",
        ));
    }

    let mut out = vec![0u8; max_size as usize];
    let compressed_size = LegacyCompressionManager::compress_data(legacy_type, data, &mut out);
    if compressed_size <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "legacy compression failed",
        ));
    }
    out.truncate(compressed_size as usize);
    Ok(out)
}

pub fn decompress_data(data: &[u8]) -> Result<Vec<u8>, io::Error> {
    if LegacyCompressionManager::is_data_compressed(data) {
        let size = LegacyCompressionManager::get_uncompressed_size(data);
        if size < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid legacy uncompressed size",
            ));
        }
        let mut out = vec![0u8; size as usize];
        let decompressed_size = LegacyCompressionManager::decompress_data(data, &mut out);
        if decompressed_size != size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy decompression failed",
            ));
        }
        return Ok(out);
    }

    let header = CompressedHeader::decode(data).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "data does not contain compression header",
        )
    })?;
    let payload = &data[9..];
    let mut fallback = None;
    let engine = get_compression_engine().unwrap_or_else(|| {
        fallback = Some(CompressionEngine::new());
        fallback.as_ref().unwrap()
    });
    engine.decompress(
        payload,
        header.compression_type,
        Some(header.original_size as usize),
    )
}

fn legacy_compression_type(
    compression_type: CompressionType,
    level: CompressionLevel,
) -> Result<LegacyCompressionType, io::Error> {
    match compression_type {
        CompressionType::None => Ok(LegacyCompressionType::None),
        CompressionType::RefPack => Ok(LegacyCompressionType::RefPack),
        CompressionType::Zlib => Ok(match level {
            CompressionLevel::None => LegacyCompressionType::ZLib1,
            CompressionLevel::Fast => LegacyCompressionType::ZLib1,
            CompressionLevel::Default => LegacyCompressionType::ZLib5,
            CompressionLevel::Best => LegacyCompressionType::ZLib9,
        }),
        CompressionType::LZ4 => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "LZ4 is not a Generals C++ compression type",
        )),
    }
}

/// Compression result structure
#[derive(Debug)]
pub struct CompressionResult {
    pub compressed_data: Vec<u8>,
    pub original_size: usize,
    pub compressed_size: usize,
    pub compression_ratio: f32,
}

/// Compression interface trait
pub trait CompressionInterface {
    /// Compress data using the specified algorithm and level
    fn compress(
        &self,
        data: &[u8],
        compression_type: CompressionType,
        level: CompressionLevel,
    ) -> Result<CompressionResult, io::Error>;

    /// Decompress data
    fn decompress(
        &self,
        compressed_data: &[u8],
        compression_type: CompressionType,
        expected_size: Option<usize>,
    ) -> Result<Vec<u8>, io::Error>;

    /// Get the maximum compressed size for input data
    fn get_max_compressed_size(
        &self,
        input_size: usize,
        compression_type: CompressionType,
    ) -> usize;

    /// Check if compression type is supported
    fn is_compression_supported(&self, compression_type: CompressionType) -> bool;
}

/// Main compression engine implementation
pub struct CompressionEngine {
    // Internal state if needed
}

impl Default for CompressionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CompressionEngine {
    /// Create a new compression engine
    pub fn new() -> Self {
        Self {}
    }

    /// Decompress zlib data
    fn decompress_zlib(&self, compressed_data: &[u8]) -> Result<Vec<u8>, io::Error> {
        use flate2::read::ZlibDecoder;

        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut result = Vec::new();
        decoder.read_to_end(&mut result)?;
        Ok(result)
    }

    /// Compress data using LZ4 (mock implementation)
    fn compress_lz4(&self, data: &[u8], _level: CompressionLevel) -> Result<Vec<u8>, io::Error> {
        // Mock LZ4 compression - in real implementation would use lz4 crate
        // For now, just return the original data with a header
        let mut result = Vec::with_capacity(data.len() + 8);
        result.extend_from_slice(&(data.len() as u32).to_le_bytes()); // Original size
        result.extend_from_slice(b"LZ4\0"); // Magic
        result.extend_from_slice(data); // Mock: just copy data
        Ok(result)
    }

    /// Decompress LZ4 data (mock implementation)
    fn decompress_lz4(&self, compressed_data: &[u8]) -> Result<Vec<u8>, io::Error> {
        if compressed_data.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid LZ4 data",
            ));
        }

        let _original_size = u32::from_le_bytes([
            compressed_data[0],
            compressed_data[1],
            compressed_data[2],
            compressed_data[3],
        ]) as usize;

        if &compressed_data[4..8] != b"LZ4\0" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid LZ4 magic",
            ));
        }

        // Mock: just return the data after header
        Ok(compressed_data[8..].to_vec())
    }

    /// RefPack decompression (mock implementation)
    fn decompress_refpack(&self, compressed_data: &[u8]) -> Result<Vec<u8>, io::Error> {
        if compressed_data.len() < 12 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid RefPack data",
            ));
        }

        if &compressed_data[0..8] != b"RefPack\0" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid RefPack magic",
            ));
        }

        let _original_size = u32::from_le_bytes([
            compressed_data[8],
            compressed_data[9],
            compressed_data[10],
            compressed_data[11],
        ]) as usize;

        // Mock: just return the data after header
        Ok(compressed_data[12..].to_vec())
    }
}

impl CompressionInterface for CompressionEngine {
    fn compress(
        &self,
        data: &[u8],
        compression_type: CompressionType,
        level: CompressionLevel,
    ) -> Result<CompressionResult, io::Error> {
        let compressed_data = match compression_type {
            CompressionType::None => data.to_vec(),
            CompressionType::Zlib | CompressionType::RefPack => {
                compress_data(data, compression_type, level)?
            }
            CompressionType::LZ4 => self.compress_lz4(data, level)?,
        };

        let original_size = data.len();
        let compressed_size = compressed_data.len();
        let compression_ratio = if original_size > 0 {
            compressed_size as f32 / original_size as f32
        } else {
            1.0
        };

        Ok(CompressionResult {
            compressed_data,
            original_size,
            compressed_size,
            compression_ratio,
        })
    }

    fn decompress(
        &self,
        compressed_data: &[u8],
        compression_type: CompressionType,
        _expected_size: Option<usize>,
    ) -> Result<Vec<u8>, io::Error> {
        match compression_type {
            CompressionType::None => Ok(compressed_data.to_vec()),
            CompressionType::Zlib | CompressionType::RefPack
                if LegacyCompressionManager::is_data_compressed(compressed_data)
                    || CompressedHeader::decode(compressed_data).is_some() =>
            {
                decompress_data(compressed_data)
            }
            CompressionType::Zlib => self.decompress_zlib(compressed_data),
            CompressionType::LZ4 => self.decompress_lz4(compressed_data),
            CompressionType::RefPack => self.decompress_refpack(compressed_data),
        }
    }

    fn get_max_compressed_size(
        &self,
        input_size: usize,
        compression_type: CompressionType,
    ) -> usize {
        match compression_type {
            CompressionType::None => input_size,
            CompressionType::Zlib => input_size + (input_size / 1000) + 12, // zlib overhead
            CompressionType::LZ4 => input_size + (input_size / 255) + 16,   // LZ4 overhead
            CompressionType::RefPack => input_size + 32,                    // RefPack overhead
        }
    }

    fn is_compression_supported(&self, compression_type: CompressionType) -> bool {
        matches!(
            compression_type,
            CompressionType::None
                | CompressionType::Zlib
                | CompressionType::LZ4
                | CompressionType::RefPack
        )
    }
}

/// Global compression engine instance
static COMPRESSION_ENGINE: OnceCell<CompressionEngine> = OnceCell::new();

/// Initialize the global compression engine
pub fn init_compression_engine() {
    let _ = COMPRESSION_ENGINE.set(CompressionEngine::new());
}

/// Get reference to the global compression engine
pub fn get_compression_engine() -> Option<&'static CompressionEngine> {
    COMPRESSION_ENGINE.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_engine_creation() {
        let engine = CompressionEngine::new();
        assert!(engine.is_compression_supported(CompressionType::Zlib));
        assert!(engine.is_compression_supported(CompressionType::None));
    }

    #[test]
    fn test_no_compression() {
        let engine = CompressionEngine::new();
        let data = b"Hello, World!";

        let result = engine
            .compress(data, CompressionType::None, CompressionLevel::Default)
            .unwrap();
        assert_eq!(result.compressed_data, data);
        assert_eq!(result.compression_ratio, 1.0);

        let decompressed = engine
            .decompress(&result.compressed_data, CompressionType::None, None)
            .unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_zlib_compression() {
        let engine = CompressionEngine::new();
        let data = b"Hello, World! This is a test string for compression.";

        let result = engine
            .compress(data, CompressionType::Zlib, CompressionLevel::Default)
            .unwrap();
        assert_eq!(result.original_size, data.len());

        let decompressed = engine
            .decompress(&result.compressed_data, CompressionType::Zlib, None)
            .unwrap();
        assert_eq!(decompressed, data);
        assert_eq!(&result.compressed_data[0..4], b"ZL5\0");
    }

    #[test]
    fn test_lz4_mock_compression() {
        let engine = CompressionEngine::new();
        let data = b"Hello, World!";

        let result = engine
            .compress(data, CompressionType::LZ4, CompressionLevel::Fast)
            .unwrap();
        let decompressed = engine
            .decompress(&result.compressed_data, CompressionType::LZ4, None)
            .unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_refpack_mock_compression() {
        let engine = CompressionEngine::new();
        let data = b"Hello, World!";

        let result = engine
            .compress(data, CompressionType::RefPack, CompressionLevel::Default)
            .unwrap();
        let decompressed = engine
            .decompress(&result.compressed_data, CompressionType::RefPack, None)
            .unwrap();
        assert_eq!(decompressed, data);
        assert_eq!(&result.compressed_data[0..4], b"EAR\0");
    }

    #[test]
    fn top_level_compression_uses_cpp_headers_and_size() {
        let data = b"Generals compressed chunk payload".repeat(4);
        let compressed = compress_data(&data, CompressionType::Zlib, CompressionLevel::Best)
            .expect("compress cpp zlib");

        assert_eq!(&compressed[0..4], b"ZL9\0");
        assert!(is_data_compressed(&compressed));
        assert_eq!(get_uncompressed_size(&compressed), Some(data.len()));
        assert_eq!(
            decompress_data(&compressed).expect("decompress cpp zlib"),
            data
        );
    }

    #[test]
    fn test_max_compressed_size() {
        let engine = CompressionEngine::new();

        assert_eq!(
            engine.get_max_compressed_size(1000, CompressionType::None),
            1000
        );
        assert!(engine.get_max_compressed_size(1000, CompressionType::Zlib) > 1000);
    }
}
