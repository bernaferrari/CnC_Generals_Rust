/*!
 * Format-specific handling for different image formats
 */

use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat};
use std::path::Path;

/// Supported output formats matching C++ version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    PNG,
    TGA,
    DDS,
    JPG,
    BMP,
}

impl OutputFormat {
    pub fn from_string(s: &str) -> Result<Self> {
        match s.to_uppercase().as_str() {
            "PNG" => Ok(OutputFormat::PNG),
            "TGA" => Ok(OutputFormat::TGA),
            "DDS" => Ok(OutputFormat::DDS),
            "JPG" | "JPEG" => Ok(OutputFormat::JPG),
            "BMP" => Ok(OutputFormat::BMP),
            _ => Err(anyhow::anyhow!("Unsupported format: {}", s)),
        }
    }

    pub fn to_extension(&self) -> &'static str {
        match self {
            OutputFormat::PNG => "png",
            OutputFormat::TGA => "tga",
            OutputFormat::DDS => "dds",
            OutputFormat::JPG => "jpg",
            OutputFormat::BMP => "bmp",
        }
    }

    pub fn to_image_format(&self) -> Option<ImageFormat> {
        match self {
            OutputFormat::PNG => Some(ImageFormat::Png),
            OutputFormat::TGA => Some(ImageFormat::Tga),
            OutputFormat::JPG => Some(ImageFormat::Jpeg),
            OutputFormat::BMP => Some(ImageFormat::Bmp),
            OutputFormat::DDS => None, // DDS requires special handling
        }
    }

    pub fn supports_transparency(&self) -> bool {
        matches!(self, OutputFormat::PNG | OutputFormat::TGA | OutputFormat::DDS)
    }
}

/// Image format utilities
pub struct FormatHandler;

impl FormatHandler {
    /// Save image in specified format with proper settings
    pub fn save_image(
        image: &DynamicImage,
        path: &Path,
        format: OutputFormat,
        quality: Option<u8>,
    ) -> Result<()> {
        match format {
            OutputFormat::DDS => {
                // DDS format requires special handling
                Self::save_dds(image, path)?;
            }
            _ => {
                if let Some(image_format) = format.to_image_format() {
                    match format {
                        OutputFormat::JPG => {
                            // Convert to RGB if saving as JPEG (no alpha)
                            let rgb_image = image.to_rgb8();
                            rgb_image.save_with_format(path, image_format)
                                .with_context(|| format!("Failed to save as {:?}", format))?;
                        }
                        _ => {
                            image.save_with_format(path, image_format)
                                .with_context(|| format!("Failed to save as {:?}", format))?;
                        }
                    }
                } else {
                    return Err(anyhow::anyhow!("Unsupported format for saving: {:?}", format));
                }
            }
        }

        Ok(())
    }

    /// Save image in DDS format (placeholder - would need proper DDS implementation)
    fn save_dds(_image: &DynamicImage, path: &Path) -> Result<()> {
        // For now, save as PNG with a warning
        // In a complete implementation, this would use a DDS library
        log::warn!("DDS format not fully implemented, saving as PNG instead");
        
        let png_path = path.with_extension("png");
        _image.save_with_format(&png_path, ImageFormat::Png)
            .context("Failed to save DDS as PNG fallback")?;
        
        Ok(())
    }

    /// Get recommended format for specific use cases
    pub fn recommend_format(has_transparency: bool, target_size: u32) -> OutputFormat {
        if has_transparency {
            if target_size > 512 {
                OutputFormat::DDS // Better compression for large transparent textures
            } else {
                OutputFormat::PNG // Good quality for smaller transparent textures
            }
        } else {
            if target_size > 1024 {
                OutputFormat::JPG // Better compression for large opaque textures
            } else {
                OutputFormat::PNG // Good quality for smaller textures
            }
        }
    }

    /// Optimize image for specific format
    pub fn optimize_for_format(
        image: &mut DynamicImage,
        format: OutputFormat,
    ) -> Result<()> {
        match format {
            OutputFormat::JPG => {
                // Convert to RGB (remove alpha channel)
                *image = DynamicImage::ImageRgb8(image.to_rgb8());
            }
            OutputFormat::DDS => {
                // Ensure dimensions are power of 2 for DDS
                let (width, height) = (image.width(), image.height());
                if !width.is_power_of_two() || !height.is_power_of_two() {
                    let new_width = width.next_power_of_two();
                    let new_height = height.next_power_of_two();
                    log::info!("Resizing for DDS: {}x{} -> {}x{}", width, height, new_width, new_height);
                    *image = image.resize_exact(
                        new_width, 
                        new_height, 
                        image::imageops::FilterType::Lanczos3
                    );
                }
            }
            _ => {
                // No specific optimization needed
            }
        }
        
        Ok(())
    }

    /// Validate format compatibility
    pub fn validate_format_settings(
        format: OutputFormat,
        has_transparency: bool,
    ) -> Result<()> {
        if !format.supports_transparency() && has_transparency {
            log::warn!(
                "Format {:?} does not support transparency, alpha channel will be lost",
                format
            );
        }
        
        Ok(())
    }
}