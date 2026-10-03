//! DDS (DirectDraw Surface) file format support
//!
//! This module provides support for loading DDS texture files, including
//! compressed formats like DXT1, DXT3, and DXT5.

use super::{ImageDecodeError as Error, ImageDecodeResult, ImageFormat};
use bytemuck::{Pod, Zeroable};
use std::io::{Cursor, Read};
use std::path::Path;

/// DDS file magic number
const DDS_MAGIC: u32 = 0x20534444; // "DDS "

/// DDS pixel format flags
#[allow(dead_code)] // C++ parity
mod ddpf {
    pub const ALPHAPIXELS: u32 = 0x1;
    pub const ALPHA: u32 = 0x2;
    pub const FOURCC: u32 = 0x4;
    pub const RGB: u32 = 0x40;
    pub const YUV: u32 = 0x200;
    pub const LUMINANCE: u32 = 0x20000;
}

/// DDS surface flags
#[allow(dead_code)] // C++ parity
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
#[allow(dead_code)] // C++ parity
mod ddscaps {
    pub const COMPLEX: u32 = 0x8;
    pub const MIPMAP: u32 = 0x400000;
    pub const TEXTURE: u32 = 0x1000;
}

/// DDS capabilities 2 flags
#[allow(dead_code)] // C++ parity
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

/// DX10 extended header FourCC
const DDS_FOURCC_DX10: u32 = 0x30315844; // "DX10"

/// DX10 resource dimension constants
#[allow(dead_code)] // C++ parity
mod d3d10_resource_dimension {
    pub const UNKNOWN: u32 = 0;
    pub const BUFFER: u32 = 1;
    pub const TEXTURE1D: u32 = 2;
    pub const TEXTURE2D: u32 = 3;
    pub const TEXTURE3D: u32 = 4;
}

/// DX10 misc resource flags
#[allow(dead_code)] // C++ parity
mod d3d10_resource_misc {
    pub const TEXTURECUBE: u32 = 0x4;
}

/// DXGI format constants (used by DX10 extended header)
#[allow(dead_code)] // C++ parity
mod dxgi_format {
    pub const UNKNOWN: u32 = 0;
    pub const R32G32B32A32_FLOAT: u32 = 2;
    pub const R16G16B16A16_UNORM: u32 = 10;
    pub const R16G16B16A16_FLOAT: u32 = 12;
    pub const R8G8B8A8_UNORM: u32 = 28;
    pub const R8G8B8A8_UNORM_SRGB: u32 = 29;
    pub const R8_UNORM: u32 = 61;
    pub const R8G8_UNORM: u32 = 67;
    pub const BC1_UNORM: u32 = 70;
    pub const BC1_UNORM_SRGB: u32 = 71;
    pub const BC2_UNORM: u32 = 73;
    pub const BC2_UNORM_SRGB: u32 = 74;
    pub const BC3_UNORM: u32 = 76;
    pub const BC3_UNORM_SRGB: u32 = 77;
    pub const B8G8R8A8_UNORM: u32 = 87;
    pub const B8G8R8A8_UNORM_SRGB: u32 = 88;
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

/// DX10 extended header — present when pixel format FourCC is "DX10".
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DdsHeaderDxt10 {
    pub dxgi_format: u32,
    pub resource_dimension: u32,
    pub misc_flag: u32,
    pub array_size: u32,
    pub misc_flags2: u32,
}

/// DDS texture type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DdsTextureType {
    Texture2D,
    CubeMap,
    Volume,
}

/// DDS compression format for DXT family block-compressed textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DdsCompression {
    Dxt1,
    Dxt3,
    Dxt5,
}

impl DdsCompression {
    pub fn format(self) -> ImageFormat {
        match self {
            DdsCompression::Dxt1 => ImageFormat::Bc1RgbaUnormSrgb,
            DdsCompression::Dxt3 => ImageFormat::Bc2RgbaUnormSrgb,
            DdsCompression::Dxt5 => ImageFormat::Bc3RgbaUnormSrgb,
        }
    }

    pub fn block_size_bytes(self) -> u32 {
        match self {
            DdsCompression::Dxt1 => 8,
            DdsCompression::Dxt3 | DdsCompression::Dxt5 => 16,
        }
    }

    pub fn expected_payload_size(self, width: u32, height: u32) -> usize {
        let blocks_x = width.div_ceil(4);
        let blocks_y = height.div_ceil(4);
        (blocks_x * blocks_y * self.block_size_bytes()) as usize
    }
}

/// DDS compressed data
pub struct DdsData {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub format: ImageFormat,
    pub texture_type: DdsTextureType,
    pub data: Vec<u8>,
    pub level_offsets: Vec<u32>,
    pub level_sizes: Vec<u32>,
    pub compression: Option<DdsCompression>,
}

impl DdsData {
    /// Create new DDS data structure
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            depth: 1,
            mip_levels: 1,
            format: ImageFormat::Rgba8UnormSrgb,
            texture_type: DdsTextureType::Texture2D,
            data: Vec::new(),
            level_offsets: Vec::new(),
            level_sizes: Vec::new(),
            compression: None,
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
fn calculate_dxtc_surface_size(width: u32, height: u32, format: ImageFormat) -> u32 {
    let block_size = match format {
        ImageFormat::Bc1RgbaUnorm | ImageFormat::Bc1RgbaUnormSrgb => 8,
        ImageFormat::Bc2RgbaUnorm
        | ImageFormat::Bc2RgbaUnormSrgb
        | ImageFormat::Bc3RgbaUnorm
        | ImageFormat::Bc3RgbaUnormSrgb => 16,
        _ => return width * height * 4, // Fallback for uncompressed formats
    };

    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    blocks_x * blocks_y * block_size
}

/// Convert DirectX FourCC to image encoding
fn fourcc_to_format(four_cc: u32) -> Option<ImageFormat> {
    match four_cc {
        0x31545844 => Some(ImageFormat::Bc1RgbaUnormSrgb), // "DXT1"
        0x32545844 => Some(ImageFormat::Bc2RgbaUnormSrgb), // "DXT2" (DXT3 + premultiplied alpha)
        0x33545844 => Some(ImageFormat::Bc2RgbaUnormSrgb), // "DXT3"
        0x34545844 => Some(ImageFormat::Bc3RgbaUnormSrgb), // "DXT4" (DXT5 + premultiplied alpha)
        0x35545844 => Some(ImageFormat::Bc3RgbaUnormSrgb), // "DXT5"
        _ => None,
    }
}

fn dxgi_format_to_image(dxgi: u32) -> Option<ImageFormat> {
    match dxgi {
        dxgi_format::BC1_UNORM => Some(ImageFormat::Bc1RgbaUnorm),
        dxgi_format::BC1_UNORM_SRGB => Some(ImageFormat::Bc1RgbaUnormSrgb),
        dxgi_format::BC2_UNORM => Some(ImageFormat::Bc2RgbaUnorm),
        dxgi_format::BC2_UNORM_SRGB => Some(ImageFormat::Bc2RgbaUnormSrgb),
        dxgi_format::BC3_UNORM => Some(ImageFormat::Bc3RgbaUnorm),
        dxgi_format::BC3_UNORM_SRGB => Some(ImageFormat::Bc3RgbaUnormSrgb),
        dxgi_format::R8G8B8A8_UNORM => Some(ImageFormat::Rgba8Unorm),
        dxgi_format::R8G8B8A8_UNORM_SRGB => Some(ImageFormat::Rgba8UnormSrgb),
        dxgi_format::B8G8R8A8_UNORM => Some(ImageFormat::Bgra8Unorm),
        dxgi_format::B8G8R8A8_UNORM_SRGB => Some(ImageFormat::Bgra8UnormSrgb),
        dxgi_format::R8_UNORM => Some(ImageFormat::R8Unorm),
        dxgi_format::R16G16B16A16_UNORM => Some(ImageFormat::Rgba16Unorm),
        dxgi_format::R16G16B16A16_FLOAT => Some(ImageFormat::Rgba16Float),
        dxgi_format::R32G32B32A32_FLOAT => Some(ImageFormat::Rgba32Float),
        _ => None,
    }
}

fn dxgi_to_compression(dxgi: u32) -> Option<DdsCompression> {
    match dxgi {
        dxgi_format::BC1_UNORM | dxgi_format::BC1_UNORM_SRGB => Some(DdsCompression::Dxt1),
        dxgi_format::BC2_UNORM | dxgi_format::BC2_UNORM_SRGB => Some(DdsCompression::Dxt3),
        dxgi_format::BC3_UNORM | dxgi_format::BC3_UNORM_SRGB => Some(DdsCompression::Dxt5),
        _ => None,
    }
}

/// Load DDS file from path
pub fn load_dds_file<P: AsRef<Path>>(path: P) -> ImageDecodeResult<DdsData> {
    let file_data = std::fs::read(path)?;
    load_dds_from_memory(&file_data)
}

/// Load DDS file from memory buffer
pub fn load_dds_from_memory(data: &[u8]) -> ImageDecodeResult<DdsData> {
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
    let is_compressed = header.pixel_format.flags & ddpf::FOURCC != 0;
    let is_rgb = header.pixel_format.flags & ddpf::RGB != 0;
    let is_dx10 = is_compressed && header.pixel_format.four_cc == DDS_FOURCC_DX10;

    if !is_compressed && !is_rgb {
        return Err(Error::InvalidData(format!(
            "Unsupported DDS pixel format flags: 0x{:x}",
            header.pixel_format.flags
        )));
    }

    let dx10_header: Option<DdsHeaderDxt10>;
    let data_start_offset: usize;

    let (format, compression) = if is_dx10 {
        let dx10_size = std::mem::size_of::<DdsHeaderDxt10>();
        let dx10_offset = 4 + header_size;
        if data.len() < dx10_offset + dx10_size {
            return Err(Error::InvalidData("DDS DX10 header truncated".to_string()));
        }
        let dx10_hdr: DdsHeaderDxt10 =
            *bytemuck::from_bytes(&data[dx10_offset..dx10_offset + dx10_size]);
        dx10_header = Some(dx10_hdr);
        data_start_offset = dx10_offset + dx10_size;
        let fmt = dxgi_format_to_image(dx10_hdr.dxgi_format).ok_or_else(|| {
            Error::InvalidData(format!("Unsupported DXGI format: {}", dx10_hdr.dxgi_format))
        })?;
        let comp = dxgi_to_compression(dx10_hdr.dxgi_format);
        (fmt, comp)
    } else if is_compressed {
        dx10_header = None;
        data_start_offset = 4 + header_size;
        let fmt = fourcc_to_format(header.pixel_format.four_cc).ok_or_else(|| {
            Error::InvalidData(format!(
                "Unsupported FourCC: 0x{:x}",
                header.pixel_format.four_cc
            ))
        })?;
        let comp = match fmt {
            ImageFormat::Bc1RgbaUnorm | ImageFormat::Bc1RgbaUnormSrgb => Some(DdsCompression::Dxt1),
            ImageFormat::Bc2RgbaUnorm | ImageFormat::Bc2RgbaUnormSrgb => Some(DdsCompression::Dxt3),
            ImageFormat::Bc3RgbaUnorm | ImageFormat::Bc3RgbaUnormSrgb => Some(DdsCompression::Dxt5),
            _ => None,
        };
        (fmt, comp)
    } else {
        dx10_header = None;
        data_start_offset = 4 + header_size;
        (ImageFormat::Rgba8UnormSrgb, None)
    };

    // For RGB uncompressed DDS, convert level 0 to RGBA and return as single-level texture
    if !is_compressed {
        let rgb_bit_count = header.pixel_format.rgb_bit_count;
        let a_bit_mask = header.pixel_format.a_bit_mask;
        let width = header.width;
        let height = header.height;
        let pitch = if header.flags & ddsd::LINEARSIZE != 0 {
            header.pitch_or_linear_size as usize / height as usize
        } else {
            header.pitch_or_linear_size as usize
        };
        let row_bytes = (rgb_bit_count as usize).div_ceil(8);
        let raw_row_size = width as usize * row_bytes;

        if data_start_offset >= data.len() {
            return Err(Error::InvalidData("DDS file truncated".to_string()));
        }

        let image_data = &data[data_start_offset..];
        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

        match rgb_bit_count {
            32 => {
                for y in 0..height {
                    let row_start = y as usize * pitch;
                    if row_start + raw_row_size > image_data.len() {
                        return Err(Error::InvalidData("DDS RGB32 data truncated".to_string()));
                    }
                    let row = &image_data[row_start..row_start + raw_row_size];
                    for chunk in row.chunks_exact(4) {
                        rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
                    }
                }
            }
            24 => {
                for y in 0..height {
                    let row_start = y as usize * pitch;
                    if row_start + raw_row_size > image_data.len() {
                        return Err(Error::InvalidData("DDS RGB24 data truncated".to_string()));
                    }
                    let row = &image_data[row_start..row_start + raw_row_size];
                    for chunk in row.chunks_exact(3) {
                        rgba_data.extend_from_slice(&[chunk[2], chunk[1], chunk[0], 255]);
                    }
                }
            }
            16 => {
                for y in 0..height {
                    let row_start = y as usize * pitch;
                    if row_start + raw_row_size > image_data.len() {
                        return Err(Error::InvalidData("DDS RGB16 data truncated".to_string()));
                    }
                    let row = &image_data[row_start..row_start + raw_row_size];
                    for chunk in row.chunks_exact(2) {
                        let pixel = u16::from_le_bytes([chunk[0], chunk[1]]);
                        if a_bit_mask != 0 {
                            let r = ((pixel >> 10) & 0x1F) as u8;
                            let g = ((pixel >> 5) & 0x1F) as u8;
                            let b = (pixel & 0x1F) as u8;
                            let a = if pixel & 0x8000 != 0 { 255 } else { 0 };
                            rgba_data.extend_from_slice(&[
                                (r * 255) / 31,
                                (g * 255) / 31,
                                (b * 255) / 31,
                                a,
                            ]);
                        } else {
                            let r = ((pixel >> 11) & 0x1F) as u8;
                            let g = ((pixel >> 5) & 0x3F) as u8;
                            let b = (pixel & 0x1F) as u8;
                            rgba_data.extend_from_slice(&[
                                (r << 3) | (r >> 2),
                                (g << 2) | (g >> 4),
                                (b << 3) | (b >> 2),
                                255,
                            ]);
                        }
                    }
                }
            }
            _ => {
                return Err(Error::InvalidData(format!(
                    "Unsupported DDS RGB bit count: {}",
                    rgb_bit_count
                )));
            }
        }

        let data_size = rgba_data.len();
        return Ok(DdsData {
            width,
            height,
            depth: 1,
            mip_levels: 1,
            format,
            texture_type: DdsTextureType::Texture2D,
            data: rgba_data,
            level_offsets: vec![0],
            level_sizes: vec![data_size as u32],
            compression: None,
        });
    }

    let texture_type = if is_dx10 {
        if let Some(ref dx10) = dx10_header {
            if dx10.misc_flag & d3d10_resource_misc::TEXTURECUBE != 0 {
                DdsTextureType::CubeMap
            } else {
                match dx10.resource_dimension {
                    d3d10_resource_dimension::TEXTURE3D => DdsTextureType::Volume,
                    _ => DdsTextureType::Texture2D,
                }
            }
        } else {
            DdsTextureType::Texture2D
        }
    } else if header.caps.caps2 & ddscaps2::CUBEMAP != 0 {
        DdsTextureType::CubeMap
    } else if header.caps.caps2 & ddscaps2::VOLUME != 0 {
        DdsTextureType::Volume
    } else {
        DdsTextureType::Texture2D
    };

    let width = header.width;
    let height = header.height;
    let depth = if texture_type == DdsTextureType::Volume {
        header.depth.max(1)
    } else {
        1
    };
    let mut mip_levels = if header.flags & ddsd::MIPMAPCOUNT != 0 {
        header.mip_map_count.max(1)
    } else {
        1
    };

    if mip_levels > 2 {
        mip_levels -= 2;
    } else {
        mip_levels = 1;
    }

    let mut level_offsets = Vec::with_capacity(mip_levels as usize);
    let mut level_sizes = Vec::with_capacity(mip_levels as usize);
    let mut data_offset = 0usize;

    for level in 0..mip_levels {
        let level_width = (width >> level).max(1);
        let level_height = (height >> level).max(1);

        let mut level_size = calculate_dxtc_surface_size(level_width, level_height, format);

        if texture_type == DdsTextureType::Volume {
            level_size *= depth;
        }

        if texture_type == DdsTextureType::CubeMap {
            level_size *= 6;
        }

        level_offsets.push(data_offset as u32);
        level_sizes.push(level_size);
        data_offset += level_size as usize;
    }

    if data.len() < data_start_offset + data_offset {
        return Err(Error::InvalidData("DDS file truncated".to_string()));
    }

    let texture_data = data[data_start_offset..data_start_offset + data_offset].to_vec();

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
        compression,
    })
}

mod block_decode;
pub use block_decode::{decode_dxt1, decode_dxt3, decode_dxt5};

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_dxtc_size_calculation() {
        // DXT1 is 8 bytes per 4x4 block
        assert_eq!(
            calculate_dxtc_surface_size(4, 4, ImageFormat::Bc1RgbaUnormSrgb),
            8
        );
        assert_eq!(
            calculate_dxtc_surface_size(8, 8, ImageFormat::Bc1RgbaUnormSrgb),
            32
        );

        // DXT5 is 16 bytes per 4x4 block
        assert_eq!(
            calculate_dxtc_surface_size(4, 4, ImageFormat::Bc3RgbaUnormSrgb),
            16
        );
        assert_eq!(
            calculate_dxtc_surface_size(8, 8, ImageFormat::Bc3RgbaUnormSrgb),
            64
        );
    }

    #[test]
    fn test_fourcc_conversion() {
        assert_eq!(
            fourcc_to_format(0x31545844),
            Some(ImageFormat::Bc1RgbaUnormSrgb)
        ); // DXT1
        assert_eq!(
            fourcc_to_format(0x33545844),
            Some(ImageFormat::Bc2RgbaUnormSrgb)
        ); // DXT3
        assert_eq!(
            fourcc_to_format(0x35545844),
            Some(ImageFormat::Bc3RgbaUnormSrgb)
        ); // DXT5
        assert_eq!(fourcc_to_format(0x12345678), None); // Invalid
    }
}
