/*!
 * Packing algorithms and utilities for ImagePacker
 */

use serde::{Deserialize, Serialize};

/// Compression settings for different texture formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionSettings {
    /// Quality for lossy formats (0-100)
    pub quality: u8,
    /// Whether to use DXT compression for DDS
    pub use_dxt: bool,
    /// Mipmap generation
    pub generate_mipmaps: bool,
    /// Compression level for lossless formats
    pub compression_level: u8,
}

impl Default for CompressionSettings {
    fn default() -> Self {
        Self {
            quality: 90,
            use_dxt: true,
            generate_mipmaps: false,
            compression_level: 6,
        }
    }
}

/// Available packing algorithms
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PackingAlgorithm {
    /// Skyline bottom-left algorithm
    Skyline,
    /// Guillotine bin packing
    Guillotine,
    /// MaxRects algorithm
    MaxRects,
}

impl Default for PackingAlgorithm {
    fn default() -> Self {
        PackingAlgorithm::Skyline
    }
}

/// Sprite arrangement optimization
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortOrder {
    /// Sort by area (largest first)
    ByArea,
    /// Sort by width (largest first)
    ByWidth,
    /// Sort by height (largest first)
    ByHeight,
    /// Sort by perimeter (largest first)
    ByPerimeter,
    /// No sorting
    None,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::ByArea
    }
}