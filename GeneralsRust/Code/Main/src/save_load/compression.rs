use crate::save_load::{SaveLoadError, SaveLoadResult};

/// Compression magic number to identify compressed files
const COMPRESSION_MAGIC: &[u8; 4] = b"GZLZ";

/// Compression level for save files (1-12, higher = better compression)
const DEFAULT_COMPRESSION_LEVEL: u32 = 6;

/// Maximum uncompressed size we'll handle (512MB)
const MAX_UNCOMPRESSED_SIZE: usize = 512 * 1024 * 1024;

/// Check if data is compressed by looking for magic header
pub fn is_compressed(data: &[u8]) -> SaveLoadResult<bool> {
    Ok(data.len() >= 4 && &data[0..4] == COMPRESSION_MAGIC)
}

/// Compress data using LZ4
pub fn compress(data: &[u8]) -> SaveLoadResult<Vec<u8>> {
    compress_with_level(data, DEFAULT_COMPRESSION_LEVEL)
}

/// Compress data with specific compression level
pub fn compress_with_level(data: &[u8], _level: u32) -> SaveLoadResult<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    // Use LZ4 compression
    let compressed = lz4_flex::compress_prepend_size(data);

    // Create header with magic + original size
    let mut result = Vec::with_capacity(compressed.len() + 8);
    result.extend_from_slice(COMPRESSION_MAGIC);
    result.extend_from_slice(&(data.len() as u32).to_le_bytes());
    result.extend_from_slice(&compressed);

    // Verify compression was worthwhile (at least 10% reduction)
    if result.len() > data.len() * 9 / 10 {
        log::debug!("Compression not beneficial, storing uncompressed");
        // Return original data without compression magic
        return Ok(data.to_vec());
    }

    log::debug!(
        "Compressed {} bytes to {} bytes ({:.1}% reduction)",
        data.len(),
        result.len(),
        (1.0 - result.len() as f64 / data.len() as f64) * 100.0
    );

    Ok(result)
}

/// Decompress data
pub fn decompress(data: &[u8]) -> SaveLoadResult<Vec<u8>> {
    if !is_compressed(data)? {
        // Data is not compressed, return as-is
        return Ok(data.to_vec());
    }

    if data.len() < 8 {
        return Err(SaveLoadError::Corrupted(
            "Invalid compressed data header".to_string(),
        ));
    }

    // Read original size from header
    let original_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    // Safety check
    if original_size > MAX_UNCOMPRESSED_SIZE {
        return Err(SaveLoadError::Corrupted(format!(
            "Uncompressed size too large: {}",
            original_size
        )));
    }

    // Decompress the data (skip 8 byte header)
    let compressed_data = &data[8..];

    match lz4_flex::decompress_size_prepended(compressed_data) {
        Ok(decompressed) => {
            if decompressed.len() != original_size {
                return Err(SaveLoadError::Corrupted(format!(
                    "Decompressed size mismatch: expected {}, got {}",
                    original_size,
                    decompressed.len()
                )));
            }

            log::debug!(
                "Decompressed {} bytes to {} bytes",
                data.len(),
                decompressed.len()
            );

            Ok(decompressed)
        }
        Err(e) => Err(SaveLoadError::Compression(format!(
            "LZ4 decompression failed: {}",
            e
        ))),
    }
}

/// Estimate compression ratio for given data
pub fn estimate_compression_ratio(data: &[u8]) -> f32 {
    if data.is_empty() {
        return 0.9;
    }

    // Quick entropy estimation
    let mut byte_counts = [0u32; 256];
    let sample_len = std::cmp::min(data.len(), 4096);
    for &byte in data.iter().take(sample_len) {
        byte_counts[byte as usize] += 1;
    }

    let sample_size = sample_len as f32;
    let mut entropy = 0.0;

    for &count in &byte_counts {
        if count > 0 {
            let probability = count as f32 / sample_size;
            entropy -= probability * probability.log2();
        }
    }

    // Higher entropy = less compressible
    // Scale entropy (0-8) to compression ratio (0.3-0.9)
    let ratio = 0.3 + (entropy / 8.0) * 0.6;
    ratio.clamp(0.3, 0.9)
}

/// Compress data in chunks for large files
pub fn compress_chunked(data: &[u8], chunk_size: usize) -> SaveLoadResult<Vec<u8>> {
    if data.len() <= chunk_size {
        return compress(data);
    }

    let mut result = Vec::new();

    // Write chunked compression header
    result.extend_from_slice(b"GZCH"); // Chunked magic
    result.extend_from_slice(&(data.len() as u64).to_le_bytes());
    result.extend_from_slice(&(chunk_size as u32).to_le_bytes());

    let mut remaining = data;
    while !remaining.is_empty() {
        let chunk_len = std::cmp::min(remaining.len(), chunk_size);
        let chunk = &remaining[..chunk_len];

        let compressed_chunk = lz4_flex::compress_prepend_size(chunk);

        // Write chunk size and data
        result.extend_from_slice(&(compressed_chunk.len() as u32).to_le_bytes());
        result.extend_from_slice(&compressed_chunk);

        remaining = &remaining[chunk_len..];
    }

    Ok(result)
}

/// Decompress chunked data
pub fn decompress_chunked(data: &[u8]) -> SaveLoadResult<Vec<u8>> {
    if data.len() < 16 || &data[0..4] != b"GZCH" {
        return Err(SaveLoadError::Corrupted(
            "Invalid chunked data header".to_string(),
        ));
    }

    let original_size = u64::from_le_bytes([
        data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11],
    ]) as usize;

    let _chunk_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;

    // Safety check
    if original_size > MAX_UNCOMPRESSED_SIZE {
        return Err(SaveLoadError::Corrupted(format!(
            "Uncompressed size too large: {}",
            original_size
        )));
    }

    let mut result = Vec::with_capacity(original_size);
    let mut pos = 16;

    while pos < data.len() && result.len() < original_size {
        if pos + 4 > data.len() {
            return Err(SaveLoadError::Corrupted(
                "Truncated chunk header".to_string(),
            ));
        }

        let compressed_chunk_size =
            u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if pos + compressed_chunk_size > data.len() {
            return Err(SaveLoadError::Corrupted("Truncated chunk data".to_string()));
        }

        let compressed_chunk = &data[pos..pos + compressed_chunk_size];

        match lz4_flex::decompress_size_prepended(compressed_chunk) {
            Ok(chunk) => {
                result.extend_from_slice(&chunk);
                pos += compressed_chunk_size;
            }
            Err(e) => {
                return Err(SaveLoadError::Compression(format!(
                    "Failed to decompress chunk: {}",
                    e
                )));
            }
        }
    }

    if result.len() != original_size {
        return Err(SaveLoadError::Corrupted(format!(
            "Decompressed size mismatch: expected {}, got {}",
            original_size,
            result.len()
        )));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_small() {
        let original = b"Hello, World!";
        let compressed = compress(original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original.to_vec(), decompressed);
    }

    #[test]
    fn test_compress_decompress_large() {
        let original = vec![42u8; 10000];
        let compressed = compress(&original).unwrap();
        let decompressed = decompress(&compressed).unwrap();
        assert_eq!(original, decompressed);
        assert!(compressed.len() < original.len()); // Should compress well
    }

    #[test]
    fn test_uncompressed_data() {
        let original = b"Small data";
        let decompressed = decompress(original).unwrap();
        assert_eq!(original.to_vec(), decompressed);
    }

    #[test]
    fn test_compression_detection() {
        let original = vec![1, 2, 3, 4, 5];
        assert!(!is_compressed(&original).unwrap());

        let compressed = compress(&original).unwrap();
        if compressed.starts_with(COMPRESSION_MAGIC) {
            assert!(is_compressed(&compressed).unwrap());
        }
    }

    #[test]
    fn test_chunked_compression() {
        let original = vec![42u8; 50000];
        let compressed = compress_chunked(&original, 8192).unwrap();
        let decompressed = decompress_chunked(&compressed).unwrap();
        assert_eq!(original, decompressed);
    }

    #[test]
    fn test_entropy_estimation() {
        let random_data = (0..1000).map(|i| (i * 17) as u8).collect::<Vec<_>>();
        let uniform_data = vec![42u8; 1000];

        let random_ratio = estimate_compression_ratio(&random_data);
        let uniform_ratio = estimate_compression_ratio(&uniform_data);

        assert!(random_ratio > uniform_ratio);
        assert!((0.3..=0.9).contains(&random_ratio));
        assert!((0.3..=0.9).contains(&uniform_ratio));
    }
}
