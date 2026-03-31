//! TGA (Targa) file format support
//!
//! This module provides support for loading TGA texture files, including
//! uncompressed and RLE compressed formats.

use crate::core::error::{Error, RendererResult};
use bytemuck::{Pod, Zeroable};
use std::io::Cursor;
use std::path::Path;
use wgpu::TextureFormat;

/// TGA image type constants
#[allow(dead_code)] // C++ parity
mod tga_type {
    pub const NO_IMAGE: u8 = 0;
    pub const UNCOMPRESSED_COLOR_MAPPED: u8 = 1;
    pub const UNCOMPRESSED_TRUE_COLOR: u8 = 2;
    pub const UNCOMPRESSED_BLACK_WHITE: u8 = 3;
    pub const RLE_COLOR_MAPPED: u8 = 9;
    pub const RLE_TRUE_COLOR: u8 = 10;
    pub const RLE_BLACK_WHITE: u8 = 11;
}

/// TGA header structure
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TgaHeader {
    pub id_length: u8,
    pub color_map_type: u8,
    pub image_type: u8,
    pub color_map_spec: [u8; 5],
    pub x_origin: u16,
    pub y_origin: u16,
    pub width: u16,
    pub height: u16,
    pub bits_per_pixel: u8,
    pub image_descriptor: u8,
}

/// TGA image data
pub struct TgaData {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Vec<u8>,
}

impl TgaData {
    /// Create new TGA data structure
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            format: TextureFormat::Rgba8UnormSrgb,
            data: Vec::new(),
        }
    }
}

/// Load TGA file from path
pub fn load_tga_file<P: AsRef<Path>>(path: P) -> RendererResult<TgaData> {
    let file_data = std::fs::read(path)?;
    load_tga_from_memory(&file_data)
}

/// Load TGA file from memory buffer
pub fn load_tga_from_memory(data: &[u8]) -> RendererResult<TgaData> {
    if data.len() < std::mem::size_of::<TgaHeader>() {
        return Err(Error::InvalidData("TGA file too small".to_string()));
    }

    let mut cursor = Cursor::new(data);
    let header: TgaHeader = *bytemuck::from_bytes(&data[0..std::mem::size_of::<TgaHeader>()]);

    // Skip ID field if present
    cursor.set_position(std::mem::size_of::<TgaHeader>() as u64 + header.id_length as u64);

    // Skip color map if present
    if header.color_map_type != 0 {
        let color_map_length =
            u16::from_le_bytes([header.color_map_spec[2], header.color_map_spec[3]]);
        let color_map_entry_size = header.color_map_spec[4];
        let color_map_size = (color_map_length as u32 * color_map_entry_size as u32 + 7) / 8;
        cursor.set_position(cursor.position() + color_map_size as u64);
    }

    let width = header.width as u32;
    let height = header.height as u32;

    // Determine pixel format
    let (format, bytes_per_pixel) = match header.bits_per_pixel {
        8 => (TextureFormat::R8Unorm, 1),
        16 => (TextureFormat::Rgba8UnormSrgb, 2), // Will expand to RGBA
        24 => (TextureFormat::Rgba8UnormSrgb, 3), // Will expand to RGBA
        32 => (TextureFormat::Rgba8UnormSrgb, 4),
        _ => {
            return Err(Error::InvalidData(format!(
                "Unsupported TGA bit depth: {}",
                header.bits_per_pixel
            )))
        }
    };

    // Load pixel data based on image type
    let pixel_data = match header.image_type {
        tga_type::UNCOMPRESSED_TRUE_COLOR | tga_type::UNCOMPRESSED_BLACK_WHITE => {
            load_uncompressed_tga(&mut cursor, width, height, bytes_per_pixel)?
        }
        tga_type::RLE_TRUE_COLOR | tga_type::RLE_BLACK_WHITE => {
            load_rle_tga(&mut cursor, width, height, bytes_per_pixel)?
        }
        _ => {
            return Err(Error::InvalidData(format!(
                "Unsupported TGA image type: {}",
                header.image_type
            )))
        }
    };

    // Convert pixel data to RGBA format if needed
    let rgba_data = convert_to_rgba(&pixel_data, bytes_per_pixel, width, height)?;

    // Check if image needs to be flipped vertically
    let final_data = if header.image_descriptor & 0x20 == 0 {
        // Image is stored bottom-to-top, flip it
        flip_image_vertically(&rgba_data, width, height)
    } else {
        rgba_data
    };

    Ok(TgaData {
        width,
        height,
        format,
        data: final_data,
    })
}

/// Load uncompressed TGA pixel data
fn load_uncompressed_tga(
    cursor: &mut Cursor<&[u8]>,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
) -> RendererResult<Vec<u8>> {
    let total_pixels = width * height;
    let data_size = total_pixels * bytes_per_pixel;
    let current_pos = cursor.position() as usize;
    let data = cursor.get_ref();

    if current_pos + data_size as usize > data.len() {
        return Err(Error::InvalidData("TGA file truncated".to_string()));
    }

    Ok(data[current_pos..current_pos + data_size as usize].to_vec())
}

/// Load RLE compressed TGA pixel data
fn load_rle_tga(
    cursor: &mut Cursor<&[u8]>,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
) -> RendererResult<Vec<u8>> {
    let total_pixels = width * height;
    let mut pixel_data = Vec::with_capacity((total_pixels * bytes_per_pixel) as usize);
    let data = cursor.get_ref();
    let mut pos = cursor.position() as usize;

    let mut pixels_read = 0;
    while pixels_read < total_pixels && pos < data.len() {
        let packet_header = data[pos];
        pos += 1;

        let packet_size = (packet_header & 0x7F) as u32 + 1;

        if packet_header & 0x80 != 0 {
            // RLE packet - repeat the next pixel
            if pos + bytes_per_pixel as usize > data.len() {
                return Err(Error::InvalidData("TGA RLE packet truncated".to_string()));
            }

            let pixel = &data[pos..pos + bytes_per_pixel as usize];
            pos += bytes_per_pixel as usize;

            for _ in 0..packet_size {
                pixel_data.extend_from_slice(pixel);
                pixels_read += 1;
                if pixels_read >= total_pixels {
                    break;
                }
            }
        } else {
            // Raw packet - copy pixels directly
            let raw_size = packet_size * bytes_per_pixel;
            if pos + raw_size as usize > data.len() {
                return Err(Error::InvalidData("TGA raw packet truncated".to_string()));
            }

            pixel_data.extend_from_slice(&data[pos..pos + raw_size as usize]);
            pos += raw_size as usize;
            pixels_read += packet_size;
        }
    }

    cursor.set_position(pos as u64);

    if pixels_read != total_pixels {
        return Err(Error::InvalidData("TGA pixel count mismatch".to_string()));
    }

    Ok(pixel_data)
}

/// Convert pixel data to RGBA format
fn convert_to_rgba(
    data: &[u8],
    bytes_per_pixel: u32,
    width: u32,
    height: u32,
) -> RendererResult<Vec<u8>> {
    let total_pixels = (width * height) as usize;
    let mut rgba_data = Vec::with_capacity(total_pixels * 4);

    match bytes_per_pixel {
        1 => {
            // Grayscale to RGBA
            for &gray in data {
                rgba_data.extend_from_slice(&[gray, gray, gray, 255]);
            }
        }
        2 => {
            // 16-bit to RGBA (assuming A1R5G5B5 format)
            for chunk in data.chunks_exact(2) {
                let pixel = u16::from_le_bytes([chunk[0], chunk[1]]);
                let r = ((pixel >> 10) & 0x1F) as u8;
                let g = ((pixel >> 5) & 0x1F) as u8;
                let b = (pixel & 0x1F) as u8;
                let a = if pixel & 0x8000 != 0 { 255 } else { 0 };

                // Scale 5-bit values to 8-bit
                rgba_data.extend_from_slice(&[(r * 255) / 31, (g * 255) / 31, (b * 255) / 31, a]);
            }
        }
        3 => {
            // BGR to RGBA
            for chunk in data.chunks_exact(3) {
                rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]);
            }
        }
        4 => {
            // BGRA to RGBA
            for chunk in data.chunks_exact(4) {
                rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
            }
        }
        _ => return Err(Error::InvalidData("Invalid bytes per pixel".to_string())),
    }

    Ok(rgba_data)
}

/// Flip image data vertically
fn flip_image_vertically(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let bytes_per_row = (width * 4) as usize;
    let mut flipped = Vec::with_capacity(data.len());

    for y in (0..height).rev() {
        let row_start = (y as usize) * bytes_per_row;
        let row_end = row_start + bytes_per_row;
        flipped.extend_from_slice(&data[row_start..row_end]);
    }

    flipped
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_rgba_conversion() {
        // Test 24-bit BGR to RGBA conversion
        let bgr_data = vec![255, 0, 0, 0, 255, 0, 0, 0, 255]; // Blue, Green, Red pixels
        let rgba_data = convert_to_rgba(&bgr_data, 3, 3, 1).unwrap();

        // Should be Red, Green, Blue in RGBA format
        assert_eq!(
            rgba_data,
            vec![
                0, 0, 255, 255, // Red pixel
                0, 255, 0, 255, // Green pixel
                255, 0, 0, 255, // Blue pixel
            ]
        );
    }

    #[test]
    fn test_image_flip() {
        // 2x2 image data
        let data = vec![
            255, 0, 0, 255, // Red pixel (top-left)
            0, 255, 0, 255, // Green pixel (top-right)
            0, 0, 255, 255, // Blue pixel (bottom-left)
            255, 255, 0, 255, // Yellow pixel (bottom-right)
        ];

        let flipped = flip_image_vertically(&data, 2, 2);

        // Bottom row should become top row
        assert_eq!(
            flipped,
            vec![
                0, 0, 255, 255, // Blue pixel (now top-left)
                255, 255, 0, 255, // Yellow pixel (now top-right)
                255, 0, 0, 255, // Red pixel (now bottom-left)
                0, 255, 0, 255, // Green pixel (now bottom-right)
            ]
        );
    }
}
