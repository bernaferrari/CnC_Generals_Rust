//! Mipmap Generation System
//!
//! This module provides mipmap generation functionality for textures,
//! including box filtering, bilinear filtering, and automatic level calculation.

use crate::core::error::{Error, RendererResult};
use std::f32::consts::PI;
use std::sync::Arc;
use wgpu::{
    Device, Extent3d, Origin3d, Queue, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

/// Mipmap filtering method
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MipmapFilter {
    /// Simple box filter (average of 2x2 pixels)
    Box,
    /// Bilinear interpolation
    Bilinear,
    /// Lanczos resampling (higher quality)
    Lanczos,
}

/// Mipmap generation configuration
#[derive(Debug, Clone)]
pub struct MipmapConfig {
    pub filter: MipmapFilter,
    pub max_levels: Option<u32>,
    pub min_size: u32,
    pub gamma_correct: bool,
    pub alpha_premultiply: bool,
}

impl Default for MipmapConfig {
    fn default() -> Self {
        Self {
            filter: MipmapFilter::Box,
            max_levels: None,
            min_size: 1,
            gamma_correct: true,
            alpha_premultiply: false,
        }
    }
}

/// Mipmap level data
#[derive(Debug, Clone)]
pub struct MipmapLevel {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl MipmapLevel {
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height * 4) as usize; // RGBA
        Self {
            width,
            height,
            data: vec![0; size],
        }
    }

    pub fn with_data(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self {
            width,
            height,
            data,
        }
    }
}

/// Mipmap chain generator
pub struct MipmapGenerator {
    config: MipmapConfig,
}

impl MipmapGenerator {
    /// Create new mipmap generator
    pub fn new(config: MipmapConfig) -> Self {
        Self { config }
    }

    /// Create mipmap generator with default settings
    pub fn new_default() -> Self {
        Self {
            config: MipmapConfig::default(),
        }
    }

    /// Calculate the number of mipmap levels for given dimensions
    pub fn calculate_mip_levels(width: u32, height: u32, min_size: u32) -> u32 {
        let max_dimension = width.max(height);
        if max_dimension <= min_size {
            return 1;
        }

        let levels = (max_dimension as f32).log2().floor() as u32 + 1;

        // Ensure we don't go below min_size
        let mut actual_levels = 1;
        let mut test_width = width;
        let mut test_height = height;

        while test_width > min_size || test_height > min_size {
            test_width = (test_width / 2).max(1);
            test_height = (test_height / 2).max(1);
            actual_levels += 1;
        }

        levels.min(actual_levels)
    }

    /// Generate complete mipmap chain from base image
    pub fn generate_mipmaps(
        &self,
        base_data: &[u8],
        width: u32,
        height: u32,
    ) -> RendererResult<Vec<MipmapLevel>> {
        if base_data.len() != (width * height * 4) as usize {
            return Err(Error::InvalidParameter(
                "Base data size mismatch".to_string(),
            ));
        }

        let num_levels = if let Some(max_levels) = self.config.max_levels {
            max_levels.min(Self::calculate_mip_levels(
                width,
                height,
                self.config.min_size,
            ))
        } else {
            Self::calculate_mip_levels(width, height, self.config.min_size)
        };

        let mut levels = Vec::with_capacity(num_levels as usize);

        // Level 0 is the original image
        levels.push(MipmapLevel::with_data(width, height, base_data.to_vec()));

        let mut current_width = width;
        let mut current_height = height;
        let mut current_data = base_data.to_vec();

        // Generate subsequent levels
        for _ in 1..num_levels {
            let next_width = (current_width / 2).max(1);
            let next_height = (current_height / 2).max(1);

            let next_data = self.downsample_image(
                &current_data,
                current_width,
                current_height,
                next_width,
                next_height,
            )?;

            levels.push(MipmapLevel::with_data(
                next_width,
                next_height,
                next_data.clone(),
            ));

            current_width = next_width;
            current_height = next_height;
            current_data = next_data;

            // Stop if we've reached minimum size
            if current_width <= self.config.min_size && current_height <= self.config.min_size {
                break;
            }
        }

        Ok(levels)
    }

    /// Downsample an image to the next mipmap level
    fn downsample_image(
        &self,
        src_data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> RendererResult<Vec<u8>> {
        match self.config.filter {
            MipmapFilter::Box => {
                self.downsample_box(src_data, src_width, src_height, dst_width, dst_height)
            }
            MipmapFilter::Bilinear => {
                self.downsample_bilinear(src_data, src_width, src_height, dst_width, dst_height)
            }
            MipmapFilter::Lanczos => {
                self.downsample_lanczos(src_data, src_width, src_height, dst_width, dst_height)
            }
        }
    }

    /// Box filter downsampling (simple 2x2 average)
    fn downsample_box(
        &self,
        src_data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> RendererResult<Vec<u8>> {
        let mut dst_data = vec![0u8; (dst_width * dst_height * 4) as usize];

        let x_ratio = src_width as f32 / dst_width as f32;
        let y_ratio = src_height as f32 / dst_height as f32;

        for dst_y in 0..dst_height {
            for dst_x in 0..dst_width {
                let src_x = (dst_x as f32 * x_ratio) as u32;
                let src_y = (dst_y as f32 * y_ratio) as u32;

                // Sample 2x2 block (or what's available)
                let mut r_sum = 0u32;
                let mut g_sum = 0u32;
                let mut b_sum = 0u32;
                let mut a_sum = 0u32;
                let mut sample_count = 0u32;

                for dy in 0..2 {
                    for dx in 0..2 {
                        let sample_x = (src_x + dx).min(src_width - 1);
                        let sample_y = (src_y + dy).min(src_height - 1);
                        let src_idx = ((sample_y * src_width + sample_x) * 4) as usize;

                        if src_idx + 3 < src_data.len() {
                            r_sum += src_data[src_idx] as u32;
                            g_sum += src_data[src_idx + 1] as u32;
                            b_sum += src_data[src_idx + 2] as u32;
                            a_sum += src_data[src_idx + 3] as u32;
                            sample_count += 1;
                        }
                    }
                }

                if sample_count > 0 {
                    let dst_idx = ((dst_y * dst_width + dst_x) * 4) as usize;
                    dst_data[dst_idx] = (r_sum / sample_count) as u8;
                    dst_data[dst_idx + 1] = (g_sum / sample_count) as u8;
                    dst_data[dst_idx + 2] = (b_sum / sample_count) as u8;
                    dst_data[dst_idx + 3] = (a_sum / sample_count) as u8;
                }
            }
        }

        Ok(dst_data)
    }

    /// Bilinear filter downsampling
    fn downsample_bilinear(
        &self,
        src_data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> RendererResult<Vec<u8>> {
        let mut dst_data = vec![0u8; (dst_width * dst_height * 4) as usize];

        let x_ratio = (src_width - 1) as f32 / dst_width as f32;
        let y_ratio = (src_height - 1) as f32 / dst_height as f32;

        for dst_y in 0..dst_height {
            for dst_x in 0..dst_width {
                let src_x = dst_x as f32 * x_ratio;
                let src_y = dst_y as f32 * y_ratio;

                let x0 = src_x.floor() as u32;
                let y0 = src_y.floor() as u32;
                let x1 = (x0 + 1).min(src_width - 1);
                let y1 = (y0 + 1).min(src_height - 1);

                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;

                // Bilinear interpolation
                for component in 0..4 {
                    let p00 = src_data[((y0 * src_width + x0) * 4 + component) as usize] as f32;
                    let p01 = src_data[((y0 * src_width + x1) * 4 + component) as usize] as f32;
                    let p10 = src_data[((y1 * src_width + x0) * 4 + component) as usize] as f32;
                    let p11 = src_data[((y1 * src_width + x1) * 4 + component) as usize] as f32;

                    let interpolated = p00 * (1.0 - fx) * (1.0 - fy)
                        + p01 * fx * (1.0 - fy)
                        + p10 * (1.0 - fx) * fy
                        + p11 * fx * fy;

                    let dst_idx = ((dst_y * dst_width + dst_x) * 4 + component) as usize;
                    dst_data[dst_idx] = interpolated.round().clamp(0.0, 255.0) as u8;
                }
            }
        }

        Ok(dst_data)
    }

    fn sinc(x: f32) -> f32 {
        if x.abs() < 1e-6 {
            1.0
        } else {
            let pix = PI * x;
            pix.sin() / pix
        }
    }

    fn lanczos_kernel(x: f32, a: f32) -> f32 {
        let distance = x.abs();
        if distance >= a {
            0.0
        } else {
            Self::sinc(x) * Self::sinc(x / a)
        }
    }

    /// Lanczos filter downsampling using a separable Lanczos-3 kernel.
    fn downsample_lanczos(
        &self,
        src_data: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> RendererResult<Vec<u8>> {
        let mut dst_data = vec![0u8; (dst_width * dst_height * 4) as usize];
        let a = 3.0f32;

        let scale_x = src_width as f32 / dst_width as f32;
        let scale_y = src_height as f32 / dst_height as f32;
        let filter_scale_x = scale_x.max(1.0);
        let filter_scale_y = scale_y.max(1.0);
        let radius_x = a * filter_scale_x;
        let radius_y = a * filter_scale_y;

        for dst_y in 0..dst_height {
            for dst_x in 0..dst_width {
                let src_x = (dst_x as f32 + 0.5) * scale_x - 0.5;
                let src_y = (dst_y as f32 + 0.5) * scale_y - 0.5;

                let sample_min_x = (src_x - radius_x).floor() as i32;
                let sample_max_x = (src_x + radius_x).ceil() as i32;
                let sample_min_y = (src_y - radius_y).floor() as i32;
                let sample_max_y = (src_y + radius_y).ceil() as i32;

                let mut accum = [0.0f32; 4];
                let mut weight_sum = 0.0f32;

                for sy in sample_min_y..=sample_max_y {
                    let clamped_sy = sy.clamp(0, src_height as i32 - 1) as u32;
                    let wy = Self::lanczos_kernel((src_y - sy as f32) / filter_scale_y, a);
                    if wy == 0.0 {
                        continue;
                    }

                    for sx in sample_min_x..=sample_max_x {
                        let clamped_sx = sx.clamp(0, src_width as i32 - 1) as u32;
                        let wx = Self::lanczos_kernel((src_x - sx as f32) / filter_scale_x, a);
                        if wx == 0.0 {
                            continue;
                        }

                        let weight = wx * wy;
                        let src_idx = ((clamped_sy * src_width + clamped_sx) * 4) as usize;
                        for component in 0..4 {
                            accum[component] += src_data[src_idx + component] as f32 * weight;
                        }
                        weight_sum += weight;
                    }
                }

                let dst_idx = ((dst_y * dst_width + dst_x) * 4) as usize;
                if weight_sum.abs() > 1e-6 {
                    for component in 0..4 {
                        let value = (accum[component] / weight_sum).round().clamp(0.0, 255.0);
                        dst_data[dst_idx + component] = value as u8;
                    }
                } else {
                    // Fallback to nearest sample in degenerate cases.
                    let nearest_x = src_x.round().clamp(0.0, src_width as f32 - 1.0) as u32;
                    let nearest_y = src_y.round().clamp(0.0, src_height as f32 - 1.0) as u32;
                    let src_idx = ((nearest_y * src_width + nearest_x) * 4) as usize;
                    dst_data[dst_idx..dst_idx + 4].copy_from_slice(&src_data[src_idx..src_idx + 4]);
                }
            }
        }

        Ok(dst_data)
    }

    /// Generate mipmaps for WGPU texture
    pub fn generate_wgpu_mipmaps(
        &self,
        device: &Device,
        queue: &Queue,
        base_data: &[u8],
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> RendererResult<Arc<Texture>> {
        // Only support uncompressed formats for mipmap generation
        if !self.is_uncompressed_format(format) {
            return Err(Error::InvalidParameter(
                "Cannot generate mipmaps for compressed formats".to_string(),
            ));
        }

        let mip_levels = Self::calculate_mip_levels(width, height, self.config.min_size);
        let mipmaps = self.generate_mipmaps(base_data, width, height)?;

        // Create texture with all mip levels
        let texture_desc = TextureDescriptor {
            label: Some("Generated Mipmap Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = device.create_texture(&texture_desc);

        // Upload all mipmap levels
        for (level, mipmap) in mipmaps.iter().enumerate() {
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &mipmap.data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mipmap.width * 4),
                    rows_per_image: Some(mipmap.height),
                },
                Extent3d {
                    width: mipmap.width,
                    height: mipmap.height,
                    depth_or_array_layers: 1,
                },
            );
        }

        Ok(Arc::new(texture))
    }

    /// Check if format supports mipmap generation
    fn is_uncompressed_format(&self, format: TextureFormat) -> bool {
        matches!(
            format,
            TextureFormat::Rgba8Unorm
                | TextureFormat::Rgba8UnormSrgb
                | TextureFormat::Rgba8Snorm
                | TextureFormat::Rgba8Uint
                | TextureFormat::Rgba8Sint
                | TextureFormat::Rgb10a2Unorm
                | TextureFormat::Rg11b10Ufloat
                | TextureFormat::Rg32Float
                | TextureFormat::Rg32Uint
                | TextureFormat::Rg32Sint
                | TextureFormat::R32Float
                | TextureFormat::R32Uint
                | TextureFormat::R32Sint
        )
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_mip_level_calculation() {
        assert_eq!(MipmapGenerator::calculate_mip_levels(256, 256, 1), 9); // 256->128->64->32->16->8->4->2->1
        assert_eq!(MipmapGenerator::calculate_mip_levels(512, 512, 1), 10);
        assert_eq!(MipmapGenerator::calculate_mip_levels(1024, 512, 1), 11);
        assert_eq!(MipmapGenerator::calculate_mip_levels(64, 64, 4), 5); // Stop at 4x4
        assert_eq!(MipmapGenerator::calculate_mip_levels(2, 2, 1), 2); // 2x2->1x1
    }

    #[test]
    fn test_box_filter_downsampling() {
        let generator = MipmapGenerator::new_default();

        // Simple 2x2 -> 1x1 test
        let src_data = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
            0, 0, 255, 255, // Blue
            255, 255, 0, 255, // Yellow
        ];

        let result = generator.downsample_box(&src_data, 2, 2, 1, 1).unwrap();

        // Result should be average of all four pixels
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], 127); // Red: (255+0+0+255)/4 = 127.5 ≈ 127
        assert_eq!(result[1], 127); // Green: (0+255+0+255)/4 = 127.5 ≈ 127
        assert_eq!(result[2], 63); // Blue: (0+0+255+0)/4 = 63.75 ≈ 63
        assert_eq!(result[3], 255); // Alpha: (255+255+255+255)/4 = 255
    }

    #[test]
    fn test_mipmap_chain_generation() {
        let generator = MipmapGenerator::new_default();

        // 4x4 red image
        let base_data = [255u8, 0, 0, 255].repeat(16); // 4x4 red pixels

        let mipmaps = generator.generate_mipmaps(&base_data, 4, 4).unwrap();

        // Should generate: 4x4, 2x2, 1x1
        assert_eq!(mipmaps.len(), 3);
        assert_eq!(mipmaps[0].width, 4);
        assert_eq!(mipmaps[0].height, 4);
        assert_eq!(mipmaps[1].width, 2);
        assert_eq!(mipmaps[1].height, 2);
        assert_eq!(mipmaps[2].width, 1);
        assert_eq!(mipmaps[2].height, 1);
    }

    #[test]
    fn test_lanczos_downsampling_generates_valid_pixels() {
        let generator = MipmapGenerator::new(MipmapConfig {
            filter: MipmapFilter::Lanczos,
            ..MipmapConfig::default()
        });

        let src_data = vec![
            0, 0, 0, 255, 32, 0, 0, 255, 64, 0, 0, 255, 96, 0, 0, 255, 0, 32, 0, 255, 32, 32, 0,
            255, 64, 32, 0, 255, 96, 32, 0, 255, 0, 64, 0, 255, 32, 64, 0, 255, 64, 64, 0, 255, 96,
            64, 0, 255, 0, 96, 0, 255, 32, 96, 0, 255, 64, 96, 0, 255, 96, 96, 0, 255,
        ];

        let result = generator.downsample_lanczos(&src_data, 4, 4, 2, 2).unwrap();
        assert_eq!(result.len(), 2 * 2 * 4);
        assert!(result.iter().any(|&v| v != 0));
        assert_eq!(result[3], 255);
        assert_eq!(result[7], 255);
        assert_eq!(result[11], 255);
        assert_eq!(result[15], 255);
    }
}
