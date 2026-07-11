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
    pub fn to_wgpu_format(self) -> wgpu::TextureFormat {
        match self {
            DdsCompression::Dxt1 => wgpu::TextureFormat::Bc1RgbaUnormSrgb,
            DdsCompression::Dxt3 => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
            DdsCompression::Dxt5 => wgpu::TextureFormat::Bc3RgbaUnormSrgb,
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
    pub format: TextureFormat,
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
            format: TextureFormat::Rgba8UnormSrgb,
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
fn calculate_dxtc_surface_size(width: u32, height: u32, format: TextureFormat) -> u32 {
    let block_size = match format {
        TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => 8,
        TextureFormat::Bc2RgbaUnorm
        | TextureFormat::Bc2RgbaUnormSrgb
        | TextureFormat::Bc3RgbaUnorm
        | TextureFormat::Bc3RgbaUnormSrgb => 16,
        _ => return width * height * 4, // Fallback for uncompressed formats
    };

    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    blocks_x * blocks_y * block_size
}

/// Convert DirectX FourCC to WGPU texture format
fn fourcc_to_format(four_cc: u32) -> Option<TextureFormat> {
    match four_cc {
        0x31545844 => Some(TextureFormat::Bc1RgbaUnormSrgb), // "DXT1"
        0x32545844 => Some(TextureFormat::Bc2RgbaUnormSrgb), // "DXT2" (DXT3 + premultiplied alpha)
        0x33545844 => Some(TextureFormat::Bc2RgbaUnormSrgb), // "DXT3"
        0x34545844 => Some(TextureFormat::Bc3RgbaUnormSrgb), // "DXT4" (DXT5 + premultiplied alpha)
        0x35545844 => Some(TextureFormat::Bc3RgbaUnormSrgb), // "DXT5"
        _ => None,
    }
}

fn dxgi_format_to_wgpu(dxgi: u32) -> Option<TextureFormat> {
    match dxgi {
        dxgi_format::BC1_UNORM => Some(TextureFormat::Bc1RgbaUnorm),
        dxgi_format::BC1_UNORM_SRGB => Some(TextureFormat::Bc1RgbaUnormSrgb),
        dxgi_format::BC2_UNORM => Some(TextureFormat::Bc2RgbaUnorm),
        dxgi_format::BC2_UNORM_SRGB => Some(TextureFormat::Bc2RgbaUnormSrgb),
        dxgi_format::BC3_UNORM => Some(TextureFormat::Bc3RgbaUnorm),
        dxgi_format::BC3_UNORM_SRGB => Some(TextureFormat::Bc3RgbaUnormSrgb),
        dxgi_format::R8G8B8A8_UNORM => Some(TextureFormat::Rgba8Unorm),
        dxgi_format::R8G8B8A8_UNORM_SRGB => Some(TextureFormat::Rgba8UnormSrgb),
        dxgi_format::B8G8R8A8_UNORM => Some(TextureFormat::Bgra8Unorm),
        dxgi_format::B8G8R8A8_UNORM_SRGB => Some(TextureFormat::Bgra8UnormSrgb),
        dxgi_format::R8_UNORM => Some(TextureFormat::R8Unorm),
        dxgi_format::R16G16B16A16_UNORM => Some(TextureFormat::Rgba16Unorm),
        dxgi_format::R16G16B16A16_FLOAT => Some(TextureFormat::Rgba16Float),
        dxgi_format::R32G32B32A32_FLOAT => Some(TextureFormat::Rgba32Float),
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
        let fmt = dxgi_format_to_wgpu(dx10_hdr.dxgi_format).ok_or_else(|| {
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
            TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc1RgbaUnormSrgb => {
                Some(DdsCompression::Dxt1)
            }
            TextureFormat::Bc2RgbaUnorm | TextureFormat::Bc2RgbaUnormSrgb => {
                Some(DdsCompression::Dxt3)
            }
            TextureFormat::Bc3RgbaUnorm | TextureFormat::Bc3RgbaUnormSrgb => {
                Some(DdsCompression::Dxt5)
            }
            _ => None,
        };
        (fmt, comp)
    } else {
        dx10_header = None;
        data_start_offset = 4 + header_size;
        (TextureFormat::Rgba8UnormSrgb, None)
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

// ---------------------------------------------------------------------------
// DXT software decompression — C++ parity with ddsfile.cpp Get_4x4_Block
// ---------------------------------------------------------------------------

fn rgb565_to_rgb(color: u16) -> [u8; 3] {
    let r = ((color >> 11) & 0x1F) as u8;
    let g = ((color >> 5) & 0x3F) as u8;
    let b = (color & 0x1F) as u8;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

fn decode_dxt1_colors(c0: u16, c1: u16, allow_1bit_alpha: bool) -> [[u8; 4]; 4] {
    let color0 = rgb565_to_rgb(c0);
    let color1 = rgb565_to_rgb(c1);
    let c0_u16 = [color0[0] as u16, color0[1] as u16, color0[2] as u16];
    let c1_u16 = [color1[0] as u16, color1[1] as u16, color1[2] as u16];

    if !allow_1bit_alpha || c0 > c1 {
        [
            [color0[0], color0[1], color0[2], 255],
            [color1[0], color1[1], color1[2], 255],
            [
                ((2 * c0_u16[0] + c1_u16[0]) / 3) as u8,
                ((2 * c0_u16[1] + c1_u16[1]) / 3) as u8,
                ((2 * c0_u16[2] + c1_u16[2]) / 3) as u8,
                255,
            ],
            [
                ((c0_u16[0] + 2 * c1_u16[0]) / 3) as u8,
                ((c0_u16[1] + 2 * c1_u16[1]) / 3) as u8,
                ((c0_u16[2] + 2 * c1_u16[2]) / 3) as u8,
                255,
            ],
        ]
    } else {
        [
            [color0[0], color0[1], color0[2], 255],
            [color1[0], color1[1], color1[2], 255],
            [
                ((c0_u16[0] + c1_u16[0]) / 2) as u8,
                ((c0_u16[1] + c1_u16[1]) / 2) as u8,
                ((c0_u16[2] + c1_u16[2]) / 2) as u8,
                255,
            ],
            [0, 0, 0, 0],
        ]
    }
}

fn decode_dxt5_alpha_palette(alpha0: u8, alpha1: u8) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[0] = alpha0;
    out[1] = alpha1;
    if alpha0 > alpha1 {
        out[2] = ((6 * alpha0 as u16 + alpha1 as u16) / 7) as u8;
        out[3] = ((5 * alpha0 as u16 + 2 * alpha1 as u16) / 7) as u8;
        out[4] = ((4 * alpha0 as u16 + 3 * alpha1 as u16) / 7) as u8;
        out[5] = ((3 * alpha0 as u16 + 4 * alpha1 as u16) / 7) as u8;
        out[6] = ((2 * alpha0 as u16 + 5 * alpha1 as u16) / 7) as u8;
        out[7] = ((alpha0 as u16 + 6 * alpha1 as u16) / 7) as u8;
    } else {
        out[2] = ((4 * alpha0 as u16 + alpha1 as u16) / 5) as u8;
        out[3] = ((3 * alpha0 as u16 + 2 * alpha1 as u16) / 5) as u8;
        out[4] = ((2 * alpha0 as u16 + 3 * alpha1 as u16) / 5) as u8;
        out[5] = ((alpha0 as u16 + 4 * alpha1 as u16) / 5) as u8;
        out[6] = 0;
        out[7] = 255;
    }
    out
}

pub fn decode_dxt1(data: &[u8], width: u32, height: u32) -> RendererResult<Vec<u8>> {
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let expected_size = (blocks_x * blocks_y * 8) as usize;
    if data.len() < expected_size {
        return Err(Error::InvalidData("DXT1 data truncated".to_string()));
    }

    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_offset = ((by * blocks_x + bx) * 8) as usize;
            let c0 = u16::from_le_bytes([data[block_offset], data[block_offset + 1]]);
            let c1 = u16::from_le_bytes([data[block_offset + 2], data[block_offset + 3]]);
            let bitmap = u32::from_le_bytes([
                data[block_offset + 4],
                data[block_offset + 5],
                data[block_offset + 6],
                data[block_offset + 7],
            ]);

            let colors = decode_dxt1_colors(c0, c1, true);

            for py in 0..4 {
                for px in 0..4 {
                    let x = bx * 4 + px;
                    let y = by * 4 + py;
                    if x < width && y < height {
                        let bit_index = (py * 4 + px) * 2;
                        let color_index = ((bitmap >> bit_index) & 3) as usize;
                        let color = colors[color_index];
                        let pixel_index = ((y * width + x) * 4) as usize;
                        rgba_data[pixel_index..pixel_index + 4].copy_from_slice(&color);
                    }
                }
            }
        }
    }

    Ok(rgba_data)
}

pub fn decode_dxt3(data: &[u8], width: u32, height: u32) -> RendererResult<Vec<u8>> {
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let expected_size = (blocks_x * blocks_y * 16) as usize;
    if data.len() < expected_size {
        return Err(Error::InvalidData("DXT3 data truncated".to_string()));
    }

    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_offset = ((by * blocks_x + bx) * 16) as usize;

            let alpha_bits = u64::from_le_bytes([
                data[block_offset],
                data[block_offset + 1],
                data[block_offset + 2],
                data[block_offset + 3],
                data[block_offset + 4],
                data[block_offset + 5],
                data[block_offset + 6],
                data[block_offset + 7],
            ]);

            let color_offset = block_offset + 8;
            let c0 = u16::from_le_bytes([data[color_offset], data[color_offset + 1]]);
            let c1 = u16::from_le_bytes([data[color_offset + 2], data[color_offset + 3]]);
            let bitmap = u32::from_le_bytes([
                data[color_offset + 4],
                data[color_offset + 5],
                data[color_offset + 6],
                data[color_offset + 7],
            ]);
            let colors = decode_dxt1_colors(c0, c1, false);

            for py in 0..4 {
                for px in 0..4 {
                    let x = bx * 4 + px;
                    let y = by * 4 + py;
                    if x >= width || y >= height {
                        continue;
                    }

                    let pixel = py * 4 + px;
                    let color_index = ((bitmap >> (pixel * 2)) & 3) as usize;
                    let mut color = colors[color_index];
                    let alpha4 = ((alpha_bits >> (pixel * 4)) & 0xF) as u8;
                    color[3] = alpha4 * 17;

                    let dst = ((y * width + x) * 4) as usize;
                    rgba_data[dst..dst + 4].copy_from_slice(&color);
                }
            }
        }
    }

    Ok(rgba_data)
}

pub fn decode_dxt5(data: &[u8], width: u32, height: u32) -> RendererResult<Vec<u8>> {
    let blocks_x = width.div_ceil(4);
    let blocks_y = height.div_ceil(4);
    let expected_size = (blocks_x * blocks_y * 16) as usize;
    if data.len() < expected_size {
        return Err(Error::InvalidData("DXT5 data truncated".to_string()));
    }

    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_offset = ((by * blocks_x + bx) * 16) as usize;

            let alpha0 = data[block_offset];
            let alpha1 = data[block_offset + 1];

            let mut alpha_index_bits = 0u64;
            for i in 0..6usize {
                alpha_index_bits |= (data[block_offset + 2 + i] as u64) << (8 * i);
            }

            let alpha_palette = decode_dxt5_alpha_palette(alpha0, alpha1);

            let color_offset = block_offset + 8;
            let c0 = u16::from_le_bytes([data[color_offset], data[color_offset + 1]]);
            let c1 = u16::from_le_bytes([data[color_offset + 2], data[color_offset + 3]]);
            let bitmap = u32::from_le_bytes([
                data[color_offset + 4],
                data[color_offset + 5],
                data[color_offset + 6],
                data[color_offset + 7],
            ]);
            let colors = decode_dxt1_colors(c0, c1, false);

            for py in 0..4 {
                for px in 0..4 {
                    let x = bx * 4 + px;
                    let y = by * 4 + py;
                    if x >= width || y >= height {
                        continue;
                    }

                    let pixel = py * 4 + px;
                    let color_index = ((bitmap >> (pixel * 2)) & 3) as usize;
                    let alpha_index = ((alpha_index_bits >> (pixel * 3)) & 0x7) as usize;
                    let mut color = colors[color_index];
                    color[3] = alpha_palette[alpha_index];

                    let dst = ((y * width + x) * 4) as usize;
                    rgba_data[dst..dst + 4].copy_from_slice(&color);
                }
            }
        }
    }

    Ok(rgba_data)
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
