//! DDS (DirectDraw Surface) file format support
//!
//! This module provides support for loading DDS texture files, including
//! compressed formats like DXT1, DXT3, and DXT5.

use crate::core::error::{Error, RendererResult};
use bytemuck::{Pod, Zeroable};
use std::io::{Cursor, Read};
use std::path::Path;
use wgpu::TextureFormat;

/// DDS file magic number
const DDS_MAGIC: u32 = 0x20534444; // "DDS "

/// DDS pixel format flags
#[allow(dead_code)]
mod ddpf {
    pub const ALPHAPIXELS: u32 = 0x1;
    pub const ALPHA: u32 = 0x2;
    pub const FOURCC: u32 = 0x4;
    pub const RGB: u32 = 0x40;
    pub const YUV: u32 = 0x200;
    pub const LUMINANCE: u32 = 0x20000;
}

/// DDS surface flags
#[allow(dead_code)]
mod ddsd {
    pub const CAPS: u32 = 0x1;
    pub const HEIGHT: u32 = 0x2;
    pub const WIDTH: u32 = 0x4;
    pub const PITCH: u32 = 0x8;
    pub const PIXELFORMAT: u32 = 0x1000;
    pub const MIPMAPCOUNT: u32 = 0x20000;
    pub const LINEARSIZE: u32 = 0x80000;
    pub const DEPTH: u32 = 0x800000;
}

/// DDS capabilities flags
#[allow(dead_code)]
mod ddscaps {
    pub const COMPLEX: u32 = 0x8;
    pub const MIPMAP: u32 = 0x400000;
    pub const TEXTURE: u32 = 0x1000;
}

/// DDS capabilities 2 flags
#[allow(dead_code)]
mod ddscaps2 {
    pub const CUBEMAP: u32 = 0x200;
    pub const CUBEMAP_POSITIVEX: u32 = 0x400;
    pub const CUBEMAP_NEGATIVEX: u32 = 0x800;
    pub const CUBEMAP_POSITIVEY: u32 = 0x1000;
    pub const CUBEMAP_NEGATIVEY: u32 = 0x2000;
    pub const CUBEMAP_POSITIVEZ: u32 = 0x4000;
    pub const CUBEMAP_NEGATIVEZ: u32 = 0x8000;
    pub const VOLUME: u32 = 0x200000;
}

/// DDS pixel format structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DdsPixelFormat {
    pub size: u32,
    pub flags: u32,
    pub four_cc: u32,
    pub rgb_bit_count: u32,
    pub r_bit_mask: u32,
    pub g_bit_mask: u32,
    pub b_bit_mask: u32,
    pub a_bit_mask: u32,
}

/// DDS surface capabilities
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DdsCaps {
    pub caps1: u32,
    pub caps2: u32,
    pub caps3: u32,
    pub caps4: u32,
}

/// DDS surface descriptor
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DdsHeader {
    pub size: u32,
    pub flags: u32,
    pub height: u32,
    pub width: u32,
    pub pitch_or_linear_size: u32,
    pub depth: u32,
    pub mip_map_count: u32,
    pub reserved1: [u32; 11],
    pub pixel_format: DdsPixelFormat,
    pub caps: DdsCaps,
    pub reserved2: u32,
}

/// DDS texture type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DdsTextureType {
    Texture2D,
    CubeMap,
    Volume,
}

/// DDS compressed data
pub struct DdsData {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub format: TextureFormat,
    pub texture_type: DdsTextureType,
    pub data: Vec<u8>,
    pub level_offsets: Vec<u32>,
    pub level_sizes: Vec<u32>,
}

impl DdsData {
    /// Create new DDS data structure
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            depth: 1,
            mip_levels: 1,
            format: TextureFormat::Rgba8UnormSrgb,
            texture_type: DdsTextureType::Texture2D,
            data: Vec::new(),
            level_offsets: Vec::new(),
            level_sizes: Vec::new(),
        }
    }

    /// Get data for a specific mip level
    pub fn get_level_data(&self, level: u32) -> Option<&[u8]> {
        if level >= self.mip_levels {
            return None;
        }

        let offset = self.level_offsets[level as usize] as usize;
        let size = self.level_sizes[level as usize] as usize;

        if offset + size <= self.data.len() {
            Some(&self.data[offset..offset + size])
        } else {
            None
        }
    }
}

/// Calculate compressed surface size for DXTC formats
fn calculate_dxtc_surface_size(width: u32, height: u32, format: TextureFormat) -> u32 {
    let block_size = match format {
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => 8,
        TextureFormat::Bc2RgbaUnorm
        | TextureFormat::Bc2RgbaUnormSrgb
        | TextureFormat::Bc3RgbaUnorm
        | TextureFormat::Bc3RgbaUnormSrgb => 16,
        _ => return width * height * 4, // Fallback for uncompressed formats
    };

    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    blocks_x * blocks_y * block_size
}

/// Convert DirectX FourCC to WGPU texture format
fn fourcc_to_format(four_cc: u32) -> Option<TextureFormat> {
    match four_cc {
        0x31545844 => Some(TextureFormat::Bc1RgbaUnormSrgb), // "DXT1"
        0x33545844 => Some(TextureFormat::Bc2RgbaUnormSrgb), // "DXT3"
        0x35545844 => Some(TextureFormat::Bc3RgbaUnormSrgb), // "DXT5"
        _ => None,
    }
}

/// Load DDS file from path
pub fn load_dds_file<P: AsRef<Path>>(path: P) -> RendererResult<DdsData> {
    let file_data = std::fs::read(path)?;
    load_dds_from_memory(&file_data)
}

/// Load DDS file from memory buffer
pub fn load_dds_from_memory(data: &[u8]) -> RendererResult<DdsData> {
    let mut cursor = Cursor::new(data);

    // Read and verify magic number
    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    let magic_num = u32::from_le_bytes(magic);

    if magic_num != DDS_MAGIC {
        return Err(Error::InvalidData("Invalid DDS magic number".to_string()));
    }

    // Read DDS header
    let header_size = std::mem::size_of::<DdsHeader>();
    if data.len() < 4 + header_size {
        return Err(Error::InvalidData("DDS file too small".to_string()));
    }

    let header_bytes = &data[4..4 + header_size];
    let header: DdsHeader = *bytemuck::from_bytes(header_bytes);

    // Validate header
    if header.size != header_size as u32 {
        return Err(Error::InvalidData("Invalid DDS header size".to_string()));
    }

    // Determine texture format
    let format = if header.pixel_format.flags & ddpf::FOURCC != 0 {
        fourcc_to_format(header.pixel_format.four_cc).ok_or_else(|| {
            Error::InvalidData(format!(
                "Unsupported FourCC: {:x}",
                header.pixel_format.four_cc
            ))
        })?
    } else {
        // For now, only support compressed formats
        return Err(Error::InvalidData(
            "Only compressed DDS formats are supported".to_string(),
        ));
    };

    // Determine texture type
    let texture_type = if header.caps.caps2 & ddscaps2::CUBEMAP != 0 {
        DdsTextureType::CubeMap
    } else if header.caps.caps2 & ddscaps2::VOLUME != 0 {
        DdsTextureType::Volume
    } else {
        DdsTextureType::Texture2D
    };

    let width = header.width;
    let height = header.height;
    let depth = if texture_type == DdsTextureType::Volume {
        header.depth
    } else {
        1
    };
    let mut mip_levels = if header.flags & ddsd::MIPMAPCOUNT != 0 {
        header.mip_map_count.max(1)
    } else {
        1
    };

    // Apply reduction factors (similar to C++ implementation)
    // This simulates the texture quality reduction logic from the original
    if mip_levels > 2 {
        mip_levels -= 2;
    } else {
        mip_levels = 1;
    }

    // Calculate mip level offsets and sizes
    let mut level_offsets = Vec::with_capacity(mip_levels as usize);
    let mut level_sizes = Vec::with_capacity(mip_levels as usize);
    let mut data_offset = 4 + header_size;

    for level in 0..mip_levels {
        let level_width = (width >> level).max(1);
        let level_height = (height >> level).max(1);

        let mut level_size = calculate_dxtc_surface_size(level_width, level_height, format);

        // For volume textures, multiply by depth
        if texture_type == DdsTextureType::Volume {
            level_size *= depth;
        }

        // For cube maps, multiply by 6 faces
        if texture_type == DdsTextureType::CubeMap {
            level_size *= 6;
        }

        level_offsets.push(data_offset as u32);
        level_sizes.push(level_size);
        data_offset += level_size as usize;
    }

    // Verify we have enough data
    if data.len() < data_offset {
        return Err(Error::InvalidData("DDS file truncated".to_string()));
    }

    // Extract texture data
    let texture_data = data[4 + header_size..data_offset].to_vec();

    Ok(DdsData {
        width,
        height,
        depth,
        mip_levels,
        format,
        texture_type,
        data: texture_data,
        level_offsets,
        level_sizes,
    })
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_dxtc_size_calculation() {
        // DXT1 is 8 bytes per 4x4 block
        assert_eq!(
            calculate_dxtc_surface_size(4, 4, TextureFormat::Bc1RgbaUnormSrgb),
            8
        );
        assert_eq!(
            calculate_dxtc_surface_size(8, 8, TextureFormat::Bc1RgbaUnormSrgb),
            32
        );

        // DXT5 is 16 bytes per 4x4 block
        assert_eq!(
            calculate_dxtc_surface_size(4, 4, TextureFormat::Bc3RgbaUnormSrgb),
            16
        );
        assert_eq!(
            calculate_dxtc_surface_size(8, 8, TextureFormat::Bc3RgbaUnormSrgb),
            64
        );
    }

    #[test]
    fn test_fourcc_conversion() {
        assert_eq!(
            fourcc_to_format(0x31545844),
            Some(TextureFormat::Bc1RgbaUnormSrgb)
        ); // DXT1
        assert_eq!(
            fourcc_to_format(0x33545844),
            Some(TextureFormat::Bc2RgbaUnormSrgb)
        ); // DXT3
        assert_eq!(
            fourcc_to_format(0x35545844),
            Some(TextureFormat::Bc3RgbaUnormSrgb)
        ); // DXT5
        assert_eq!(fourcc_to_format(0x12345678), None); // Invalid
    }
}
