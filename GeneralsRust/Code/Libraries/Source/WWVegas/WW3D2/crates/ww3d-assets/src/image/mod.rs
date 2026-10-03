//! CPU image decoding used by retail asset loaders.
//!
//! Decoders own pixel/block bytes and mip metadata. They resolve no files or
//! game instances implicitly and expose no device, upload or renderer types.
//! Archive lookup and GPU capability/format decisions belong to their callers.

pub mod dds;
pub mod tga;

pub use dds::{
    DdsCompression, DdsData, DdsTextureType, decode_dxt1, decode_dxt3, decode_dxt5, load_dds_file,
    load_dds_from_memory,
};
pub use tga::{TgaData, load_tga_file, load_tga_from_memory};

/// Pixel/block encoding, including the source's linear or sRGB interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    Bc1RgbaUnorm,
    Bc1RgbaUnormSrgb,
    Bc2RgbaUnorm,
    Bc2RgbaUnormSrgb,
    Bc3RgbaUnorm,
    Bc3RgbaUnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    R8Unorm,
    Rgba16Float,
    Rgba16Unorm,
    Rgba32Float,
    Rgba8Unorm,
    Rgba8UnormSrgb,
}

/// CPU decoding errors preserve the previous loader's diagnostics.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ImageDecodeError {
    #[error("generic error: {0}")]
    Generic(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
}

impl From<std::io::Error> for ImageDecodeError {
    fn from(error: std::io::Error) -> Self {
        Self::Generic(error.to_string())
    }
}

pub type ImageDecodeResult<T> = Result<T, ImageDecodeError>;

#[cfg(test)]
mod tests;
