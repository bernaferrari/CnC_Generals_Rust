//! CPU mip chains for 8-bit RGBA colour textures (renderer plan R7).
//!
//! C++ textures carry full mip chains (DDS chains, or D3DX-generated ones
//! for TGA). The chain here is a 2x2 box filter on the stored (gamma-space)
//! bytes, the D3DX default, down to 1x1.

/// Mip count of a full chain for `width` x `height`.
pub fn full_mip_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

/// Next mip level of a tightly packed RGBA8 image (box filter; odd edges
/// clamp).
pub fn rgba8_half(width: u32, height: u32, data: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = ((width / 2).max(1), (height / 2).max(1));
    let mut out = vec![0u8; (w * h * 4) as usize];
    let at = |x: u32, y: u32, c: usize| {
        let (x, y) = (x.min(width - 1), y.min(height - 1));
        u32::from(data[((y * width + x) * 4) as usize + c])
    };
    for y in 0..h {
        for x in 0..w {
            for c in 0..4 {
                let sum = at(2 * x, 2 * y, c)
                    + at(2 * x + 1, 2 * y, c)
                    + at(2 * x, 2 * y + 1, c)
                    + at(2 * x + 1, 2 * y + 1, c);
                out[((y * w + x) * 4) as usize + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    (w, h, out)
}

/// Create a sampled RGBA8-class texture with a full mip chain and upload
/// every level.
pub fn create_rgba8_texture_with_mips(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    data: &[u8],
) -> wgpu::Texture {
    let mip_level_count = full_mip_count(width, height);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let (mut w, mut h) = (width, height);
    let mut level = data[..(width * height * 4) as usize].to_vec();
    for mip_level in 0..mip_level_count {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        if mip_level + 1 < mip_level_count {
            (w, h, level) = rgba8_half(w, h, &level);
        }
    }
    texture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_reaches_one_texel_and_box_filters() {
        assert_eq!(full_mip_count(256, 64), 9);
        assert_eq!(full_mip_count(1, 1), 1);
        let data = [0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255];
        let (w, h, half) = rgba8_half(2, 2, &data);
        assert_eq!((w, h), (1, 1));
        assert_eq!(half, vec![128, 128, 128, 255]);
    }
}
