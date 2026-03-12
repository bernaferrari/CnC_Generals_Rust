//! Shared texture decoding helpers.
//!
//! This module translates image files (DDS, TGA, PNG/JPG/...) into a unified
//! `TextureData` structure that can be consumed by the higher level texture
//! loaders.  The intent is to keep all disk-format logic in one place so the
//! CPU-only loader and the WGPU-backed loader stay in sync.

use crate::core::error::{Error, Result};
use crate::core::ww3dformat::FormatDecision;
use crate::core::WW3DFormat;
use crate::rendering::texture_system::dds_loader::{load_dds_file, DdsTextureType};
use crate::rendering::texture_system::tga_loader::load_tga_file;
use bcdec_rs::{bc1, bc2, bc3};
use std::path::Path;

/// Texture dimensionality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureDataKind {
    Texture2D,
    CubeMap,
    Volume,
}

/// Per-mip metadata describing how to slice the raw data buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureMipLevel {
    pub offset: usize,
    pub size: usize,
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub slice_stride: usize,
}

/// Decoded texture payload.
#[derive(Clone, Debug)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_levels: u32,
    pub format: WW3DFormat,
    pub kind: TextureDataKind,
    pub data: Vec<u8>,
    pub mip_layout: Vec<TextureMipLevel>,
    pub format_decision: Option<FormatDecision>,
}

impl TextureData {
    /// Convenience accessor for mip metadata.
    pub fn mip_levels(&self) -> &[TextureMipLevel] {
        &self.mip_layout
    }

    /// Determine whether the decoded payload uses block compression.
    pub fn is_compressed(&self) -> bool {
        self.format.is_block_compressed()
    }

    /// Size of the backing buffer in bytes.
    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    pub fn drop_mip_levels(mut self, levels: u32) -> Self {
        if levels == 0 || self.mip_layout.len() <= 1 {
            return self;
        }

        let available = self.mip_layout.len() as u32;
        let drop = levels.min(available.saturating_sub(1));
        if drop == 0 {
            return self;
        }

        let start_index = drop as usize;
        if start_index >= self.mip_layout.len() {
            return self;
        }

        let start_offset = self.mip_layout[start_index].offset;
        let mut adjusted_layout = Vec::with_capacity(self.mip_layout.len() - start_index);
        for level in self.mip_layout[start_index..].iter().cloned() {
            let mut level = level;
            level.offset -= start_offset;
            adjusted_layout.push(level);
        }

        self.data = self.data[start_offset..].to_vec();
        self.mip_layout = adjusted_layout;
        self.mip_levels = self.mip_layout.len() as u32;
        self.width = (self.width >> drop).max(1);
        self.height = (self.height >> drop).max(1);
        if matches!(self.kind, TextureDataKind::Volume) {
            self.depth = (self.depth >> drop).max(1);
        }
        self
    }

    pub fn ensure_mip_levels(&mut self, target_levels: u32) -> Result<()> {
        if target_levels <= 1 || target_levels <= self.mip_levels {
            return Ok(());
        }

        if self.kind != TextureDataKind::Texture2D {
            return Err(Error::InvalidData(
                "Automatic mipmap generation is only supported for 2D textures".to_string(),
            ));
        }

        match self.format {
            WW3DFormat::A8R8G8B8 | WW3DFormat::R8G8B8A8 | WW3DFormat::X8R8G8B8 => {}
            other => {
                return Err(Error::InvalidData(format!(
                    "Cannot generate mipmaps for format {:?}",
                    other
                )));
            }
        }

        let max_levels = max_mip_levels(self.width, self.height, self.depth);
        let target = target_levels.min(max_levels);

        while self.mip_levels < target {
            let prev_index = (self.mip_levels - 1) as usize;
            let (prev_offset, prev_size, prev_width, prev_height) = {
                let info = &self.mip_layout[prev_index];
                (info.offset, info.size, info.width, info.height)
            };
            let previous = self.data[prev_offset..prev_offset + prev_size].to_vec();
            let (next_width, next_height, downsampled) =
                downsample_rgba(&previous, prev_width, prev_height);
            let offset = self.data.len();
            self.data.extend_from_slice(&downsampled);
            self.mip_layout.push(TextureMipLevel {
                offset,
                size: downsampled.len(),
                width: next_width,
                height: next_height,
                depth_or_layers: 1,
                slice_stride: downsampled.len(),
            });
            self.mip_levels += 1;
        }

        Ok(())
    }

    pub fn max_possible_mip_levels(&self) -> u32 {
        max_mip_levels(self.width, self.height, self.depth)
    }

    pub fn truncate_to_mip_count(self, target_levels: u32) -> Self {
        if target_levels == 0 || target_levels >= self.mip_levels {
            return self;
        }
        let drop = self.mip_levels - target_levels;
        self.drop_mip_levels(drop)
    }

    /// Convert the decoded payload so it matches the selected GPU format.
    pub fn convert_to_format(mut self, decision: &FormatDecision) -> Result<Self> {
        let mut current_format = self.format;
        let target_format = decision.format;
        let mut data = std::mem::take(&mut self.data);
        let mut layout = std::mem::take(&mut self.mip_layout);
        self.format_decision = Some(decision.clone());

        if decision.requires_decompression && current_format.is_block_compressed() {
            let (decoded, decoded_layout) =
                decompress_texture_levels(current_format, &layout, &data)?;
            data = decoded;
            layout = decoded_layout;
            current_format = WW3DFormat::A8R8G8B8;
        }

        if target_format == current_format {
            self.data = data;
            self.mip_layout = layout;
            self.mip_levels = self.mip_layout.len() as u32;
            self.format = target_format;
            self.format_decision = Some(decision.clone());
            return Ok(self);
        }

        if target_format.is_block_compressed() {
            return Err(Error::InvalidData(format!(
                "Cannot convert texture data into block-compressed format {target_format:?}"
            )));
        }

        let (converted_data, converted_layout) =
            convert_texture_format(current_format, target_format, &layout, &data)?;
        self.data = converted_data;
        self.mip_layout = converted_layout;
        self.mip_levels = self.mip_layout.len() as u32;
        self.format = target_format;
        Ok(self)
    }
}

/// Decode a texture file from disk into CPU memory.
pub fn decode_texture_file<P: AsRef<Path>>(path: P) -> Result<TextureData> {
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "dds" => decode_dds(path),
        "tga" => decode_tga(path),
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp" => decode_standard_image(path),
        _ => decode_standard_image(path),
    }
}

fn decode_dds(path: &Path) -> Result<TextureData> {
    let dds = load_dds_file(path)?;

    let format = WW3DFormat::from_wgpu_format(dds.format).unwrap_or(WW3DFormat::A8R8G8B8);
    let kind = match dds.texture_type {
        DdsTextureType::Texture2D => TextureDataKind::Texture2D,
        DdsTextureType::CubeMap => TextureDataKind::CubeMap,
        DdsTextureType::Volume => TextureDataKind::Volume,
    };

    let mut mip_layout = Vec::with_capacity(dds.mip_levels as usize);
    let mut width = dds.width.max(1);
    let mut height = dds.height.max(1);

    for (level, &offset_u32) in dds.level_offsets.iter().enumerate() {
        if level >= dds.mip_levels as usize {
            break;
        }
        let offset = offset_u32 as usize;
        let size = dds.level_sizes.get(level).copied().unwrap_or(0) as usize;
        let layer_count = match kind {
            TextureDataKind::CubeMap => 6,
            TextureDataKind::Volume => (dds.depth >> level).max(1),
            TextureDataKind::Texture2D => 1,
        };
        let slice_stride = if layer_count > 0 {
            size / layer_count as usize
        } else {
            size
        };

        mip_layout.push(TextureMipLevel {
            offset,
            size,
            width,
            height,
            depth_or_layers: layer_count,
            slice_stride,
        });

        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }

    Ok(TextureData {
        width: dds.width,
        height: dds.height,
        depth: match kind {
            TextureDataKind::Volume => dds.depth.max(1),
            _ => 1,
        },
        mip_levels: dds.mip_levels.max(1),
        format,
        kind,
        data: dds.data,
        mip_layout,
        format_decision: None,
    })
}

fn decode_tga(path: &Path) -> Result<TextureData> {
    let tga = load_tga_file(path)?;

    let format = WW3DFormat::from_wgpu_format(tga.format).unwrap_or(WW3DFormat::A8R8G8B8);
    let size = tga.data.len();

    Ok(TextureData {
        width: tga.width,
        height: tga.height,
        depth: 1,
        mip_levels: 1,
        format,
        kind: TextureDataKind::Texture2D,
        data: tga.data,
        mip_layout: vec![TextureMipLevel {
            offset: 0,
            size,
            width: tga.width,
            height: tga.height,
            depth_or_layers: 1,
            slice_stride: size,
        }],
        format_decision: None,
    })
}

fn decode_standard_image(path: &Path) -> Result<TextureData> {
    let image = image::open(path).map_err(|err| {
        Error::InvalidData(format!(
            "Failed to load image '{}' : {}",
            path.display(),
            err
        ))
    })?;

    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let data = rgba.into_raw();
    let size = data.len();

    Ok(TextureData {
        width,
        height,
        depth: 1,
        mip_levels: 1,
        format: WW3DFormat::A8R8G8B8,
        kind: TextureDataKind::Texture2D,
        data,
        mip_layout: vec![TextureMipLevel {
            offset: 0,
            size,
            width,
            height,
            depth_or_layers: 1,
            slice_stride: size,
        }],
        format_decision: None,
    })
}

fn decompress_texture_levels(
    format: WW3DFormat,
    layout: &[TextureMipLevel],
    data: &[u8],
) -> Result<(Vec<u8>, Vec<TextureMipLevel>)> {
    if !format.is_block_compressed() {
        return Err(Error::InvalidData(format!(
            "Format {format:?} is not block-compressed"
        )));
    }

    let mut output = Vec::new();
    let mut new_layout = Vec::with_capacity(layout.len());

    for level in layout {
        let offset = output.len();
        let layers = level.depth_or_layers.max(1) as usize;
        for layer in 0..layers {
            let slice_begin = level.offset + layer * level.slice_stride;
            let slice_end = slice_begin + level.slice_stride;
            if slice_end > data.len() {
                return Err(Error::InvalidData(
                    "Compressed texture slice exceeds buffer".to_string(),
                ));
            }
            let slice = &data[slice_begin..slice_end];
            let decoded = decompress_block_slice(format, slice, level.width, level.height)?;
            output.extend_from_slice(&decoded);
        }
        let size = output.len() - offset;
        let slice_stride = size / layers;
        new_layout.push(TextureMipLevel {
            offset,
            size,
            width: level.width,
            height: level.height,
            depth_or_layers: level.depth_or_layers,
            slice_stride,
        });
    }

    Ok((output, new_layout))
}

fn decompress_block_slice(
    format: WW3DFormat,
    slice: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>> {
    let block_size = match format {
        WW3DFormat::DXT1 => 8,
        WW3DFormat::DXT2 | WW3DFormat::DXT3 | WW3DFormat::DXT4 | WW3DFormat::DXT5 => 16,
        _ => {
            return Err(Error::InvalidData(format!(
                "Unsupported block format {format:?}"
            )));
        }
    };

    let blocks_x = ((width + 3) / 4) as usize;
    let blocks_y = ((height + 3) / 4) as usize;
    let expected = blocks_x * blocks_y * block_size;
    if slice.len() < expected {
        return Err(Error::InvalidData(
            "Compressed texture slice truncated".to_string(),
        ));
    }

    let mut output = vec![0u8; (width * height * 4) as usize];
    let mut block_rgba = [0u8; 4 * 4 * 4];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_index = by * blocks_x + bx;
            let block = &slice[block_index * block_size..block_index * block_size + block_size];
            match format {
                WW3DFormat::DXT1 => bc1(block, &mut block_rgba, 4 * 4),
                WW3DFormat::DXT2 | WW3DFormat::DXT3 => bc2(block, &mut block_rgba, 4 * 4),
                WW3DFormat::DXT4 | WW3DFormat::DXT5 => bc3(block, &mut block_rgba, 4 * 4),
                _ => unreachable!(),
            }

            for py in 0..4 {
                let dst_y = by * 4 + py;
                if dst_y as u32 >= height {
                    continue;
                }
                for px in 0..4 {
                    let dst_x = bx * 4 + px;
                    if dst_x as u32 >= width {
                        continue;
                    }
                    let dst_idx = ((dst_y * width as usize) + dst_x) * 4;
                    let src_idx = (py * 4 + px) * 4;
                    output[dst_idx..dst_idx + 4].copy_from_slice(&block_rgba[src_idx..src_idx + 4]);
                }
            }
        }
    }

    Ok(output)
}

fn convert_texture_format(
    from: WW3DFormat,
    to: WW3DFormat,
    layout: &[TextureMipLevel],
    data: &[u8],
) -> Result<(Vec<u8>, Vec<TextureMipLevel>)> {
    match to {
        WW3DFormat::A8R8G8B8 | WW3DFormat::R8G8B8A8 => convert_to_rgba8(layout, data, from, false),
        WW3DFormat::X8R8G8B8 => convert_to_rgba8(layout, data, from, true),
        WW3DFormat::R5G6B5 => convert_to_rgb565(layout, data, from),
        WW3DFormat::A4R4G4B4 => convert_to_argb4444(layout, data, from),
        WW3DFormat::A1R5G5B5 => convert_to_argb1555(layout, data, from),
        _ => Err(Error::InvalidData(format!(
            "Unsupported format conversion {from:?} -> {to:?}"
        ))),
    }
}

fn convert_to_rgba8(
    layout: &[TextureMipLevel],
    data: &[u8],
    from: WW3DFormat,
    force_opaque: bool,
) -> Result<(Vec<u8>, Vec<TextureMipLevel>)> {
    let mut output = Vec::new();
    let mut new_layout = Vec::with_capacity(layout.len());

    for level in layout {
        let offset = output.len();
        let layers = level.depth_or_layers.max(1) as usize;
        for layer in 0..layers {
            let slice_begin = level.offset + layer * level.slice_stride;
            let slice_end = slice_begin + level.slice_stride;
            if slice_end > data.len() {
                return Err(Error::InvalidData(
                    "Texture slice exceeds buffer".to_string(),
                ));
            }
            let slice = &data[slice_begin..slice_end];
            if slice.len() % 4 != 0 {
                return Err(Error::InvalidData(
                    "Unexpected pixel stride while converting to RGBA".to_string(),
                ));
            }
            let mut converted = Vec::with_capacity(slice.len());
            for pixel in slice.chunks_exact(4) {
                let (r, g, b, mut a) = read_rgba(from, pixel);
                if force_opaque {
                    a = 255;
                }
                converted.extend_from_slice(&[r, g, b, a]);
            }
            output.extend_from_slice(&converted);
        }
        let size = output.len() - offset;
        let slice_stride = size / layers;
        new_layout.push(TextureMipLevel {
            offset,
            size,
            width: level.width,
            height: level.height,
            depth_or_layers: level.depth_or_layers,
            slice_stride,
        });
    }

    Ok((output, new_layout))
}

fn convert_to_rgb565(
    layout: &[TextureMipLevel],
    data: &[u8],
    from: WW3DFormat,
) -> Result<(Vec<u8>, Vec<TextureMipLevel>)> {
    let mut output = Vec::new();
    let mut new_layout = Vec::with_capacity(layout.len());

    for level in layout {
        let offset = output.len();
        let layers = level.depth_or_layers.max(1) as usize;
        for layer in 0..layers {
            let slice_begin = level.offset + layer * level.slice_stride;
            let slice_end = slice_begin + level.slice_stride;
            if slice_end > data.len() {
                return Err(Error::InvalidData(
                    "Texture slice exceeds buffer".to_string(),
                ));
            }
            let slice = &data[slice_begin..slice_end];
            if slice.len() % 4 != 0 {
                return Err(Error::InvalidData(
                    "Unexpected pixel stride for RGB565 conversion".to_string(),
                ));
            }
            let mut converted = Vec::with_capacity(slice.len() / 2);
            for pixel in slice.chunks_exact(4) {
                let (r, g, b, _) = read_rgba(from, pixel);
                let value = ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | ((b as u16) >> 3);
                converted.extend_from_slice(&value.to_le_bytes());
            }
            output.extend_from_slice(&converted);
        }
        let size = output.len() - offset;
        let slice_stride = size / layers;
        new_layout.push(TextureMipLevel {
            offset,
            size,
            width: level.width,
            height: level.height,
            depth_or_layers: level.depth_or_layers,
            slice_stride,
        });
    }

    Ok((output, new_layout))
}

fn convert_to_argb1555(
    layout: &[TextureMipLevel],
    data: &[u8],
    from: WW3DFormat,
) -> Result<(Vec<u8>, Vec<TextureMipLevel>)> {
    let mut output = Vec::new();
    let mut new_layout = Vec::with_capacity(layout.len());

    for level in layout {
        let offset = output.len();
        let layers = level.depth_or_layers.max(1) as usize;
        for layer in 0..layers {
            let slice_begin = level.offset + layer * level.slice_stride;
            let slice_end = slice_begin + level.slice_stride;
            if slice_end > data.len() {
                return Err(Error::InvalidData(
                    "Texture slice exceeds buffer".to_string(),
                ));
            }
            let slice = &data[slice_begin..slice_end];
            if slice.len() % 4 != 0 {
                return Err(Error::InvalidData(
                    "Unexpected pixel stride for ARGB1555 conversion".to_string(),
                ));
            }
            let mut converted = Vec::with_capacity(slice.len() / 2);
            for pixel in slice.chunks_exact(4) {
                let (r, g, b, a) = read_rgba(from, pixel);
                let a1 = if a > 127 { 1u16 } else { 0u16 };
                let r5 = ((r as u16) >> 3) & 0x1F;
                let g5 = ((g as u16) >> 3) & 0x1F;
                let b5 = ((b as u16) >> 3) & 0x1F;
                let value = (a1 << 15) | (r5 << 10) | (g5 << 5) | b5;
                converted.extend_from_slice(&value.to_le_bytes());
            }
            output.extend_from_slice(&converted);
        }
        let size = output.len() - offset;
        let slice_stride = size / layers;
        new_layout.push(TextureMipLevel {
            offset,
            size,
            width: level.width,
            height: level.height,
            depth_or_layers: level.depth_or_layers,
            slice_stride,
        });
    }

    Ok((output, new_layout))
}

fn convert_to_argb4444(
    layout: &[TextureMipLevel],
    data: &[u8],
    from: WW3DFormat,
) -> Result<(Vec<u8>, Vec<TextureMipLevel>)> {
    let mut output = Vec::new();
    let mut new_layout = Vec::with_capacity(layout.len());

    for level in layout {
        let offset = output.len();
        let layers = level.depth_or_layers.max(1) as usize;
        for layer in 0..layers {
            let slice_begin = level.offset + layer * level.slice_stride;
            let slice_end = slice_begin + level.slice_stride;
            if slice_end > data.len() {
                return Err(Error::InvalidData(
                    "Texture slice exceeds buffer".to_string(),
                ));
            }
            let slice = &data[slice_begin..slice_end];
            if slice.len() % 4 != 0 {
                return Err(Error::InvalidData(
                    "Unexpected pixel stride for ARGB4444 conversion".to_string(),
                ));
            }
            let mut converted = Vec::with_capacity(slice.len() / 2);
            for pixel in slice.chunks_exact(4) {
                let (r, g, b, a) = read_rgba(from, pixel);
                let value = ((a as u16 >> 4) << 12)
                    | ((r as u16 >> 4) << 8)
                    | ((g as u16 >> 4) << 4)
                    | (b as u16 >> 4);
                converted.extend_from_slice(&value.to_le_bytes());
            }
            output.extend_from_slice(&converted);
        }
        let size = output.len() - offset;
        let slice_stride = size / layers;
        new_layout.push(TextureMipLevel {
            offset,
            size,
            width: level.width,
            height: level.height,
            depth_or_layers: level.depth_or_layers,
            slice_stride,
        });
    }

    Ok((output, new_layout))
}

fn read_rgba(from: WW3DFormat, pixel: &[u8]) -> (u8, u8, u8, u8) {
    match from {
        WW3DFormat::A8R8G8B8 | WW3DFormat::R8G8B8A8 => (pixel[0], pixel[1], pixel[2], pixel[3]),
        WW3DFormat::X8R8G8B8 => (pixel[0], pixel[1], pixel[2], 255),
        _ => (
            pixel[0],
            pixel[1],
            pixel[2],
            pixel.get(3).copied().unwrap_or(255),
        ),
    }
}

fn max_mip_levels(width: u32, height: u32, depth: u32) -> u32 {
    let mut levels = 1;
    let mut w = width.max(1);
    let mut h = height.max(1);
    let mut d = depth.max(1);

    while w > 1 || h > 1 || d > 1 {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        d = (d / 2).max(1);
        levels += 1;
    }

    levels
}

fn downsample_rgba(data: &[u8], width: u32, height: u32) -> (u32, u32, Vec<u8>) {
    let new_width = (width / 2).max(1);
    let new_height = (height / 2).max(1);
    let mut output = vec![0u8; (new_width * new_height * 4) as usize];

    for y in 0..new_height {
        for x in 0..new_width {
            let mut accum = [0u32; 4];
            let mut samples = 0u32;
            for dy in 0..2 {
                let src_y = y * 2 + dy;
                if src_y >= height {
                    continue;
                }
                for dx in 0..2 {
                    let src_x = x * 2 + dx;
                    if src_x >= width {
                        continue;
                    }
                    let idx = ((src_y * width + src_x) as usize) * 4;
                    accum[0] += data[idx] as u32;
                    accum[1] += data[idx + 1] as u32;
                    accum[2] += data[idx + 2] as u32;
                    accum[3] += data[idx + 3] as u32;
                    samples += 1;
                }
            }

            let samples = samples.max(1);
            let dst_idx = ((y * new_width + x) as usize) * 4;
            output[dst_idx] = (accum[0] / samples) as u8;
            output[dst_idx + 1] = (accum[1] / samples) as u8;
            output[dst_idx + 2] = (accum[2] / samples) as u8;
            output[dst_idx + 3] = (accum[3] / samples) as u8;
        }
    }

    (new_width, new_height, output)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    fn make_simple_texture() -> TextureData {
        let mut mip_layout = Vec::new();
        // Level 0: 4x4 RGBA
        mip_layout.push(TextureMipLevel {
            offset: 0,
            size: 16 * 4,
            width: 4,
            height: 4,
            depth_or_layers: 1,
            slice_stride: 16 * 4,
        });
        // Level 1: 2x2 RGBA, arranged sequentially after level 0
        mip_layout.push(TextureMipLevel {
            offset: 16 * 4,
            size: 4 * 4,
            width: 2,
            height: 2,
            depth_or_layers: 1,
            slice_stride: 4 * 4,
        });
        let mut data = vec![0u8; 16 * 4 + 4 * 4];
        for (idx, chunk) in data.chunks_exact_mut(4).enumerate() {
            chunk.copy_from_slice(&[idx as u8, 0, 0, 255]);
        }
        TextureData {
            width: 4,
            height: 4,
            depth: 1,
            mip_levels: 2,
            format: WW3DFormat::A8R8G8B8,
            kind: TextureDataKind::Texture2D,
            data,
            mip_layout,
            format_decision: None,
        }
    }

    #[test]
    fn drops_mip_levels() {
        let texture = make_simple_texture();
        let reduced = texture.drop_mip_levels(1);
        assert_eq!(reduced.width, 2);
        assert_eq!(reduced.height, 2);
        assert_eq!(reduced.mip_levels, 1);
        assert_eq!(reduced.data.len(), 4 * 4);
        assert_eq!(reduced.mip_layout[0].offset, 0);
    }

    #[test]
    fn ensure_mip_generation_adds_levels() {
        let mut texture = make_simple_texture();
        texture.ensure_mip_levels(3).unwrap();
        assert_eq!(texture.mip_levels, 3);
        let last = texture.mip_layout.last().unwrap();
        assert_eq!(last.width, 1);
        assert_eq!(last.height, 1);
    }

    #[test]
    fn truncate_to_requested_mip_count() {
        let texture = make_simple_texture();
        let truncated = texture.truncate_to_mip_count(1);
        assert_eq!(truncated.mip_levels, 1);
        assert_eq!(truncated.mip_layout.len(), 1);
    }

    #[test]
    fn drop_mip_levels_preserves_cube_map_layout() {
        let face0_size = 4 * 4 * 4;
        let face1_size = 2 * 2 * 4;
        let mut data = Vec::new();
        for layer in 0..6 {
            data.extend(std::iter::repeat(layer as u8).take(face0_size));
        }
        let level0_size = data.len();
        let level1_offset = data.len();
        for layer in 0..6 {
            data.extend(std::iter::repeat(100 + layer as u8).take(face1_size));
        }
        let level1_size = data.len() - level1_offset;

        let mip_layout = vec![
            TextureMipLevel {
                offset: 0,
                size: level0_size,
                width: 4,
                height: 4,
                depth_or_layers: 6,
                slice_stride: face0_size,
            },
            TextureMipLevel {
                offset: level1_offset,
                size: level1_size,
                width: 2,
                height: 2,
                depth_or_layers: 6,
                slice_stride: face1_size,
            },
        ];

        let cube = TextureData {
            width: 4,
            height: 4,
            depth: 1,
            mip_levels: 2,
            format: WW3DFormat::A8R8G8B8,
            kind: TextureDataKind::CubeMap,
            data,
            mip_layout,
            format_decision: None,
        };

        let reduced = cube.drop_mip_levels(1);
        assert_eq!(reduced.width, 2);
        assert_eq!(reduced.height, 2);
        assert_eq!(reduced.mip_levels, 1);
        assert_eq!(reduced.mip_layout.len(), 1);
        assert_eq!(reduced.mip_layout[0].depth_or_layers, 6);
        assert_eq!(reduced.mip_layout[0].slice_stride, face1_size);
        assert_eq!(reduced.data.len(), level1_size);
    }

    #[test]
    fn converts_to_rgb565() {
        let texture = make_simple_texture();
        let decision = FormatDecision {
            source_format: WW3DFormat::A8R8G8B8,
            preferred_format: WW3DFormat::A8R8G8B8,
            format: WW3DFormat::R5G6B5,
            requires_decompression: false,
        };
        let converted = texture
            .convert_to_format(&decision)
            .expect("conversion succeeds");
        assert_eq!(converted.format, WW3DFormat::R5G6B5);
        assert_eq!(converted.data.len(), (16 + 4) * 2);
    }
}
