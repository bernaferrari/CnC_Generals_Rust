use super::{Error, ImageDecodeResult};

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

pub fn decode_dxt1(data: &[u8], width: u32, height: u32) -> ImageDecodeResult<Vec<u8>> {
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

pub fn decode_dxt3(data: &[u8], width: u32, height: u32) -> ImageDecodeResult<Vec<u8>> {
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

pub fn decode_dxt5(data: &[u8], width: u32, height: u32) -> ImageDecodeResult<Vec<u8>> {
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
