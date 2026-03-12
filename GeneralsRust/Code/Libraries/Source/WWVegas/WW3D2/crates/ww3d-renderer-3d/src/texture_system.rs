//! Texture system - comprehensive texture management and loading
//!
//! This module provides complete texture loading support including:
//! - DDS file format parsing and loading (DXT1, DXT3, DXT5 compression)
//! - Standard image formats (PNG, JPG, etc.)
//! - Mipmap generation and management
//! - Cube map and volume texture support
//! - Texture compression and optimization

use crate::core::{Error, Result};
use crate::material_system::TextureStageSettings;
use binrw::{BinRead, BinWrite};
use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use ww3d_core::W3dTextureStruct;

mod texture_surface;

pub use texture_surface::{
    SurfaceClass, SurfaceDescription, SurfaceLock, SurfaceLockMut, SurfaceRect,
};

/// Texture class - represents a texture resource
#[derive(Debug)]
pub struct TextureClass {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Vec<u8>,
    pub gpu_texture: Option<wgpu::Texture>,
    pub stage_settings: TextureStageSettings,
}

// Manual Clone implementation to handle wgpu::Texture
impl Clone for TextureClass {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            width: self.width,
            height: self.height,
            format: self.format,
            data: self.data.clone(),
            gpu_texture: None, // Don't clone the GPU texture, recreate it when needed
            stage_settings: self.stage_settings,
        }
    }
}

impl Default for TextureClass {
    fn default() -> Self {
        let mut texture = TextureClass::new("default_texture", 1, 1);
        texture
            .replace_pixels(vec![0, 0, 0, 255])
            .expect("default texture storage");
        texture
    }
}

impl TextureClass {
    /// Create a new texture
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        Self::with_format(name, width, height, TextureFormat::Rgba8Unorm)
    }

    /// Create a texture with an explicit pixel format.
    pub fn with_format(name: &str, width: u32, height: u32, format: TextureFormat) -> Self {
        Self {
            name: name.to_string(),
            width,
            height,
            format,
            data: vec![0; Self::storage_size(width, height, format)],
            gpu_texture: None,
            stage_settings: TextureStageSettings::default(),
        }
    }

    /// Create a placeholder texture from W3D metadata, deferring actual image
    /// loading until the texture manager resolves the resource.
    pub fn from_w3d_descriptor(descriptor: &W3dTextureStruct) -> Self {
        let raw_name = &descriptor.name;
        let length = raw_name
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(raw_name.len());
        let decoded = String::from_utf8_lossy(&raw_name[..length])
            .trim()
            .to_string();
        let name = if decoded.is_empty() {
            "unnamed_w3d_texture".to_string()
        } else {
            decoded
        };

        let mut texture = TextureClass::new(&name, 1, 1);
        texture.format = TextureFormat::Rgba8UnormSrgb;
        texture.stage_settings = TextureStageSettings::from_descriptor(descriptor);
        texture
    }

    /// Get texture name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Raw pixel buffer accessor.
    pub fn raw_pixels(&self) -> &[u8] {
        &self.data
    }

    /// Replace texture storage ensuring the size matches the declared dimensions.
    pub fn replace_pixels(&mut self, data: Vec<u8>) -> Result<()> {
        let expected = Self::storage_size(self.width, self.height, self.format);
        if data.len() < expected {
            return Err(Error::InvalidData(format!(
                "Texture pixel count mismatch: expected at least {expected} bytes, found {}",
                data.len()
            )));
        }
        self.data = data;
        Ok(())
    }

    /// Produce a CPU surface clone of this texture.
    pub fn to_surface(&self) -> Result<SurfaceClass> {
        let base_size = Self::storage_size(self.width, self.height, self.format);
        if self.data.len() < base_size {
            return Err(Error::InvalidData(format!(
                "Texture only stores {} bytes, but {} are required for the base surface",
                self.data.len(),
                base_size
            )));
        }
        SurfaceClass::from_bytes(
            self.width,
            self.height,
            self.format,
            &self.data[..base_size],
        )
    }

    /// Create a texture from an existing surface snapshot.
    pub fn from_surface(name: &str, surface: &SurfaceClass) -> Result<Self> {
        let desc = surface.description();
        let mut texture = TextureClass::with_format(name, desc.width, desc.height, desc.format);
        let bytes = surface.lock().pixels().to_vec();
        texture.replace_pixels(bytes)?;
        Ok(texture)
    }

    /// Load texture from file with automatic format detection
    pub fn load_from_file(path: &str) -> Result<Self> {
        let path_obj = Path::new(path);

        // Check file extension to determine loader
        if let Some(extension) = path_obj.extension() {
            match extension.to_str() {
                Some("dds") | Some("DDS") => {
                    return DDSLoader::load_dds_file(path);
                }
                _ => {}
            }
        }

        let image = image::open(path_obj)
            .map_err(|e| Error::Generic(format!("Failed to load texture '{}': {}", path, e)))?;
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();

        let mut texture = Self::with_format(path, width, height, TextureFormat::Rgba8UnormSrgb);
        texture.replace_pixels(rgba.into_raw())?;
        Ok(texture)
    }

    /// Create WGPU texture from loaded data
    pub fn create_wgpu_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<()> {
        if self.data.is_empty() {
            return Err(Error::Generic("No texture data available".to_string()));
        }

        let mut upload_data: Cow<'_, [u8]> = Cow::Borrowed(&self.data);
        let upload_format = match self.format {
            TextureFormat::R5G6B5 => {
                upload_data = Cow::Owned(convert_r5g6b5_to_rgba8(&self.data));
                TextureFormat::Rgba8Unorm
            }
            TextureFormat::A1R5G5B5 => {
                upload_data = Cow::Owned(convert_a1r5g5b5_to_rgba8(&self.data));
                TextureFormat::Rgba8Unorm
            }
            TextureFormat::A4R4G4B4 => {
                upload_data = Cow::Owned(convert_a4r4g4b4_to_rgba8(&self.data));
                TextureFormat::Rgba8Unorm
            }
            other => other,
        };

        let wgpu_format = match upload_format {
            TextureFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            TextureFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
            TextureFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
            TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Bc1RgbaUnorm => wgpu::TextureFormat::Bc1RgbaUnorm,
            TextureFormat::Bc2RgbaUnorm => wgpu::TextureFormat::Bc2RgbaUnorm,
            TextureFormat::Bc3RgbaUnorm => wgpu::TextureFormat::Bc3RgbaUnorm,
            TextureFormat::Bc4RUnorm => wgpu::TextureFormat::Bc4RUnorm,
            TextureFormat::Bc5RgUnorm => wgpu::TextureFormat::Bc5RgUnorm,
            TextureFormat::Bc6hRgbUfloat => wgpu::TextureFormat::Bc6hRgbUfloat,
            TextureFormat::Bc7RgbaUnorm => wgpu::TextureFormat::Bc7RgbaUnorm,
            TextureFormat::R5G6B5 | TextureFormat::A1R5G5B5 | TextureFormat::A4R4G4B4 => {
                return Err(Error::Generic(
                    "16-bit texture format upload attempted without conversion".to_string(),
                ));
            }
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&self.name),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            upload_data.as_ref(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(Self::calculate_bytes_per_row(self.width, upload_format)),
                rows_per_image: Some(Self::rows_per_image(self.height, upload_format)),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.gpu_texture = Some(texture);
        Ok(())
    }

    /// Access the inferred sampler settings for this texture.
    pub fn stage_settings(&self) -> TextureStageSettings {
        self.stage_settings
    }

    /// Override the sampler settings for this texture.
    pub fn set_stage_settings(&mut self, settings: TextureStageSettings) {
        self.stage_settings = settings;
    }

    /// Get WGPU texture view for binding
    pub fn get_texture_view(&self) -> Option<wgpu::TextureView> {
        self.gpu_texture
            .as_ref()
            .map(|texture| texture.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    /// Get sort level for texture batching
    pub fn get_sort_level(&self) -> u32 {
        // Sort by texture size and format for better batching
        // Smaller textures first, then by format
        let size_score = (self.width * self.height) / 1000;
        let format_score = match self.format {
            TextureFormat::Bc1RgbaUnorm => 0, // DXT1 - most common
            TextureFormat::Bc3RgbaUnorm => 1, // DXT3/DXT5
            _ => 2,                           // Other formats
        };
        size_score + format_score * 1000000
    }

    /// Calculate bytes per row for texture data layout
    fn calculate_bytes_per_row(width: u32, format: TextureFormat) -> u32 {
        match format {
            // Compressed formats
            TextureFormat::Bc1RgbaUnorm | TextureFormat::Bc4RUnorm => {
                // 4x4 blocks, 8 bytes per block
                ((width + 3) / 4) * 8
            }
            TextureFormat::Bc2RgbaUnorm
            | TextureFormat::Bc3RgbaUnorm
            | TextureFormat::Bc5RgUnorm => {
                // 4x4 blocks, 16 bytes per block
                ((width + 3) / 4) * 16
            }
            TextureFormat::Bc6hRgbUfloat | TextureFormat::Bc7RgbaUnorm => {
                // 4x4 blocks, 16 bytes per block
                ((width + 3) / 4) * 16
            }
            // Uncompressed formats
            TextureFormat::Rgba8Unorm
            | TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Bgra8Unorm
            | TextureFormat::Bgra8UnormSrgb => width * 4,
            TextureFormat::R5G6B5 | TextureFormat::A1R5G5B5 | TextureFormat::A4R4G4B4 => width * 2,
        }
    }

    fn rows_per_image(height: u32, format: TextureFormat) -> u32 {
        match format {
            TextureFormat::Rgba8Unorm
            | TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Bgra8Unorm
            | TextureFormat::Bgra8UnormSrgb
            | TextureFormat::R5G6B5
            | TextureFormat::A1R5G5B5
            | TextureFormat::A4R4G4B4 => height.max(1),
            TextureFormat::Bc1RgbaUnorm
            | TextureFormat::Bc2RgbaUnorm
            | TextureFormat::Bc3RgbaUnorm
            | TextureFormat::Bc4RUnorm
            | TextureFormat::Bc5RgUnorm
            | TextureFormat::Bc6hRgbUfloat
            | TextureFormat::Bc7RgbaUnorm => ((height + 3) / 4).max(1),
        }
    }

    fn storage_size(width: u32, height: u32, format: TextureFormat) -> usize {
        (Self::calculate_bytes_per_row(width, format) * Self::rows_per_image(height, format))
            as usize
    }
}

/// Texture formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    R5G6B5,
    A1R5G5B5,
    A4R4G4B4,
    // DDS compressed formats
    Bc1RgbaUnorm,  // DXT1
    Bc2RgbaUnorm,  // DXT3
    Bc3RgbaUnorm,  // DXT5
    Bc4RUnorm,     // ATI1
    Bc5RgUnorm,    // ATI2
    Bc6hRgbUfloat, // BPTC HDR
    Bc7RgbaUnorm,  // BPTC LDR
}

impl TextureFormat {
    pub fn from_ww3d(format: crate::core::WW3DFormat) -> Option<Self> {
        use crate::core::WW3DFormat as F;
        Some(match format {
            F::A8R8G8B8 | F::R8G8B8A8 | F::X8R8G8B8 => TextureFormat::Rgba8Unorm,
            F::R8G8B8 => TextureFormat::Rgba8Unorm,
            F::A8P8 | F::A8 | F::A8L8 | F::A4L4 | F::L8 => TextureFormat::Rgba8Unorm,
            F::R5G6B5 => TextureFormat::R5G6B5,
            F::A1R5G5B5 | F::X1R5G5B5 => TextureFormat::A1R5G5B5,
            F::A4R4G4B4 | F::X4R4G4B4 => TextureFormat::A4R4G4B4,
            F::DXT1 => TextureFormat::Bc1RgbaUnorm,
            F::DXT2 | F::DXT3 => TextureFormat::Bc2RgbaUnorm,
            F::DXT4 | F::DXT5 => TextureFormat::Bc3RgbaUnorm,
            _ => return None,
        })
    }
}

fn convert_r5g6b5_to_rgba8(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((data.len() / 2) * 4);
    for chunk in data.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let r = ((value >> 11) & 0x1F) as u8;
        let g = ((value >> 5) & 0x3F) as u8;
        let b = (value & 0x1F) as u8;
        output.extend_from_slice(&[
            (r << 3) | (r >> 2),
            (g << 2) | (g >> 4),
            (b << 3) | (b >> 2),
            255,
        ]);
    }
    output
}

fn convert_a1r5g5b5_to_rgba8(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((data.len() / 2) * 4);
    for chunk in data.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let r = ((value >> 10) & 0x1F) as u8;
        let g = ((value >> 5) & 0x1F) as u8;
        let b = (value & 0x1F) as u8;
        let a = if (value & 0x8000) != 0 { 255 } else { 0 };
        output.extend_from_slice(&[
            (r << 3) | (r >> 2),
            (g << 3) | (g >> 2),
            (b << 3) | (b >> 2),
            a,
        ]);
    }
    output
}

fn convert_a4r4g4b4_to_rgba8(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((data.len() / 2) * 4);
    for chunk in data.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let r = ((value >> 8) & 0x0F) as u8;
        let g = ((value >> 4) & 0x0F) as u8;
        let b = (value & 0x0F) as u8;
        let a = ((value >> 12) & 0x0F) as u8;
        output.extend_from_slice(&[(r << 4) | r, (g << 4) | g, (b << 4) | b, (a << 4) | a]);
    }
    output
}

/// DDS file header structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct DDSHeader {
    pub magic: u32,                   // 'DDS '
    pub size: u32,                    // Size of structure (124)
    pub flags: u32,                   // DDS flags
    pub height: u32,                  // Height of texture
    pub width: u32,                   // Width of texture
    pub pitch_or_linear_size: u32,    // Pitch or linear size
    pub depth: u32,                   // Depth of volume texture
    pub mip_map_count: u32,           // Number of mip levels
    pub reserved1: [u32; 11],         // Reserved
    pub pixel_format: DDSPixelFormat, // Pixel format description
    pub caps: u32,                    // Texture capabilities
    pub caps2: u32,                   // Additional capabilities
    pub caps3: u32,                   // More capabilities
    pub caps4: u32,                   // Yet more capabilities
    pub reserved2: u32,               // Reserved
}

/// DDS pixel format structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct DDSPixelFormat {
    pub size: u32,          // Size of structure (32)
    pub flags: u32,         // Pixel format flags
    pub four_cc: [u8; 4],   // Four character code
    pub rgb_bit_count: u32, // Number of bits per pixel
    pub r_bit_mask: u32,    // Red bit mask
    pub g_bit_mask: u32,    // Green bit mask
    pub b_bit_mask: u32,    // Blue bit mask
    pub a_bit_mask: u32,    // Alpha bit mask
}

/// DDS texture types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DDSType {
    Texture2D,
    CubeMap,
    VolumeTexture,
}

/// DDS loader for compressed and uncompressed textures
pub struct DDSLoader;

impl DDSLoader {
    /// Load DDS file from path
    pub fn load_dds_file<P: AsRef<Path>>(path: P) -> Result<TextureClass> {
        let path = path.as_ref();
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        Self::load_dds_from_memory(&buffer, path.to_string_lossy().as_ref())
    }

    /// Load DDS from memory buffer
    pub fn load_dds_from_memory(data: &[u8], name: &str) -> Result<TextureClass> {
        if data.len() < 4 || &data[0..4] != b"DDS " {
            return Err(Error::Generic("Not a valid DDS file".to_string()));
        }

        let mut cursor = std::io::Cursor::new(&data[4..]);
        let header: DDSHeader = DDSHeader::read_le(&mut cursor)
            .map_err(|e| Error::Generic(format!("Failed to read DDS header: {}", e)))?;
        let texture_data = &data[4 + std::mem::size_of::<DDSHeader>()..];

        let format = Self::determine_texture_format(&header.pixel_format)?;
        let texture_type = Self::determine_texture_type(header.caps, header.caps2)?;

        let mut texture = TextureClass::new(name, header.width, header.height);
        texture.format = format;
        texture.data = texture_data.to_vec();

        // Set additional properties based on texture type
        match texture_type {
            DDSType::CubeMap => {
                // Cube maps have 6 faces
                texture.data = Self::process_cube_map(texture_data, &header)?;
            }
            DDSType::VolumeTexture => {
                // Volume textures have depth
                // Additional processing needed
            }
            DDSType::Texture2D => {
                // Standard 2D texture
            }
        }

        Ok(texture)
    }

    /// Determine texture format from DDS pixel format
    fn determine_texture_format(pixel_format: &DDSPixelFormat) -> Result<TextureFormat> {
        // Check for compressed formats first (four character codes)
        if pixel_format.flags & 0x4 != 0 {
            // DDPF_FOURCC
            let four_cc = &pixel_format.four_cc;
            match four_cc {
                b"DXT1" => return Ok(TextureFormat::Bc1RgbaUnorm),
                b"DXT3" => return Ok(TextureFormat::Bc2RgbaUnorm),
                b"DXT5" => return Ok(TextureFormat::Bc3RgbaUnorm),
                b"ATI1" => return Ok(TextureFormat::Bc4RUnorm),
                b"ATI2" => return Ok(TextureFormat::Bc5RgUnorm),
                _ => {}
            }
        }

        // Check for uncompressed formats
        if pixel_format.flags & 0x40 != 0 {
            // DDPF_RGBA
            if pixel_format.rgb_bit_count == 32 {
                return Ok(TextureFormat::Rgba8Unorm);
            }
        }

        if pixel_format.flags & 0x20 != 0 {
            // DDPF_RGB
            if pixel_format.rgb_bit_count == 32 {
                return Ok(TextureFormat::Bgra8Unorm);
            }
        }

        Err(Error::Generic("Unsupported DDS pixel format".to_string()))
    }

    /// Determine texture type from DDS caps
    fn determine_texture_type(caps: u32, caps2: u32) -> Result<DDSType> {
        const DDS_CAPS_COMPLEX: u32 = 0x8;
        const DDS_CAPS2_CUBEMAP: u32 = 0x200;
        const DDS_CAPS2_VOLUME: u32 = 0x200000;

        if caps & DDS_CAPS_COMPLEX != 0 {
            if caps2 & DDS_CAPS2_CUBEMAP != 0 {
                return Ok(DDSType::CubeMap);
            }
            if caps2 & DDS_CAPS2_VOLUME != 0 {
                return Ok(DDSType::VolumeTexture);
            }
        }

        Ok(DDSType::Texture2D)
    }

    /// Process cube map data (6 faces)
    fn process_cube_map(_data: &[u8], _header: &DDSHeader) -> Result<Vec<u8>> {
        // Cube map processing - faces are stored sequentially
        // For now, just return the data as-is
        // In a full implementation, you might want to reorder or validate the faces
        Ok(_data.to_vec())
    }

    /// Calculate compressed texture size
    pub fn calculate_compressed_size(width: u32, height: u32, format: TextureFormat) -> u32 {
        let block_size = match format {
            TextureFormat::Bc1RgbaUnorm => 8,  // DXT1: 64 bits per 4x4 block
            TextureFormat::Bc2RgbaUnorm => 16, // DXT3: 128 bits per 4x4 block
            TextureFormat::Bc3RgbaUnorm => 16, // DXT5: 128 bits per 4x4 block
            TextureFormat::Bc4RUnorm => 8,     // ATI1: 64 bits per 4x4 block
            TextureFormat::Bc5RgUnorm => 16,   // ATI2: 128 bits per 4x4 block
            _ => return width * height * 4,    // Uncompressed fallback
        };

        let blocks_wide = (width + 3) / 4;
        let blocks_high = (height + 3) / 4;

        blocks_wide * blocks_high * block_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn load_from_file_decodes_png_with_real_dimensions() {
        let path =
            std::env::temp_dir().join(format!("ww3d_texture_system_{}.png", std::process::id()));
        let image = ImageBuffer::<Rgba<u8>, _>::from_fn(4, 2, |_x, _y| Rgba([11, 22, 33, 255]));
        image.save(&path).expect("write png");

        let texture =
            TextureClass::load_from_file(path.to_string_lossy().as_ref()).expect("load png");
        assert_eq!(texture.width, 4);
        assert_eq!(texture.height, 2);
        assert_eq!(texture.format, TextureFormat::Rgba8UnormSrgb);
        assert_eq!(texture.data.len(), 4 * 2 * 4);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_from_file_missing_path_returns_error() {
        let missing = format!(
            "/tmp/ww3d_texture_system_missing_{}_{}.png",
            std::process::id(),
            1_234_567u64
        );
        assert!(TextureClass::load_from_file(&missing).is_err());
    }
}
