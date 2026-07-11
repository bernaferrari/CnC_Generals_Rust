/// Texture system for WW3D
///
/// This module implements texture loading, management, and rendering.
use crate::errors::W3DResult;
use crate::w3d_format::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Texture format types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    /// 8-bit grayscale
    R8,
    /// 16-bit grayscale
    R16,
    /// 8-bit red-green
    RG8,
    /// 16-bit red-green
    RG16,
    /// 24-bit RGB
    RGB8,
    /// 32-bit RGBA
    RGBA8,
    /// 16-bit RGB (5-6-5)
    RGB565,
    /// 16-bit RGBA (5-5-5-1)
    RGBA5551,
    /// 16-bit RGBA (4-4-4-4)
    RGBA4444,
    /// DXT1 compressed
    DXT1,
    /// DXT3 compressed
    DXT3,
    /// DXT5 compressed
    DXT5,
    /// BC4 compressed (1 channel)
    BC4,
    /// BC5 compressed (2 channels)
    BC5,
    /// BC6H compressed (HDR)
    BC6H,
    /// BC7 compressed
    BC7,
}

impl TextureFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            TextureFormat::R8 => 1,
            TextureFormat::R16
            | TextureFormat::RG8
            | TextureFormat::RGB565
            | TextureFormat::RGBA5551
            | TextureFormat::RGBA4444 => 2,
            TextureFormat::RGB8 => 3,
            TextureFormat::RGBA8 | TextureFormat::RG16 => 4,
            _ => 0, // Compressed formats don't have a fixed BPP
        }
    }

    pub fn is_compressed(&self) -> bool {
        matches!(
            self,
            TextureFormat::DXT1
                | TextureFormat::DXT3
                | TextureFormat::DXT5
                | TextureFormat::BC4
                | TextureFormat::BC5
                | TextureFormat::BC6H
                | TextureFormat::BC7
        )
    }

    pub fn has_alpha(&self) -> bool {
        matches!(
            self,
            TextureFormat::RGBA8
                | TextureFormat::RGBA5551
                | TextureFormat::RGBA4444
                | TextureFormat::DXT3
                | TextureFormat::DXT5
                | TextureFormat::BC7
        )
    }
}

/// Texture dimensions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDimensions {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

impl TextureDimensions {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
        }
    }

    pub fn new_3d(width: u32, height: u32, depth: u32) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    pub fn is_power_of_two(&self) -> bool {
        self.width.is_power_of_two()
            && self.height.is_power_of_two()
            && self.depth.is_power_of_two()
    }

    pub fn mip_levels(&self) -> u32 {
        let max_dim = self.width.max(self.height).max(self.depth);
        if max_dim == 0 {
            return 1;
        }
        (32 - max_dim.leading_zeros()).max(1)
    }
}

/// Texture data holder
#[derive(Debug, Clone)]
pub struct TextureData {
    pub dimensions: TextureDimensions,
    pub format: TextureFormat,
    pub mip_levels: Vec<Vec<u8>>,
}

impl TextureData {
    pub fn new(dimensions: TextureDimensions, format: TextureFormat) -> Self {
        Self {
            dimensions,
            format,
            mip_levels: Vec::new(),
        }
    }

    pub fn with_data(dimensions: TextureDimensions, format: TextureFormat, data: Vec<u8>) -> Self {
        Self {
            dimensions,
            format,
            mip_levels: vec![data],
        }
    }

    pub fn add_mip_level(&mut self, data: Vec<u8>) {
        self.mip_levels.push(data);
    }

    pub fn get_mip_level(&self, level: usize) -> Option<&[u8]> {
        self.mip_levels.get(level).map(|v| v.as_slice())
    }

    pub fn mip_count(&self) -> usize {
        self.mip_levels.len()
    }

    pub fn base_data(&self) -> Option<&[u8]> {
        self.get_mip_level(0)
    }

    pub fn generate_mipmaps(&mut self) {
        if self.mip_levels.is_empty() || self.format.is_compressed() {
            return;
        }

        let max_mips = self.dimensions.mip_levels() as usize;
        if self.mip_levels.len() >= max_mips {
            return;
        }

        // Generate mipmaps using box filtering
        let mut current_width = self.dimensions.width;
        let mut current_height = self.dimensions.height;

        while self.mip_levels.len() < max_mips && current_width > 1 && current_height > 1 {
            let prev_level = self.mip_levels.last().unwrap();
            let next_width = (current_width / 2).max(1);
            let next_height = (current_height / 2).max(1);

            let next_data = self.downsample_mip(
                prev_level,
                current_width,
                current_height,
                next_width,
                next_height,
            );

            self.mip_levels.push(next_data);

            current_width = next_width;
            current_height = next_height;
        }
    }

    fn downsample_mip(
        &self,
        src: &[u8],
        src_width: u32,
        src_height: u32,
        dst_width: u32,
        dst_height: u32,
    ) -> Vec<u8> {
        let bpp = self.format.bytes_per_pixel();
        let mut dst = vec![0u8; (dst_width * dst_height) as usize * bpp];

        for y in 0..dst_height {
            for x in 0..dst_width {
                let src_x = x * 2;
                let src_y = y * 2;

                // Box filter: average 4 pixels
                for c in 0..bpp {
                    let mut sum = 0u32;
                    let mut count = 0u32;

                    for dy in 0..2 {
                        for dx in 0..2 {
                            let sx = src_x + dx;
                            let sy = src_y + dy;

                            if sx < src_width && sy < src_height {
                                let src_idx = ((sy * src_width + sx) as usize * bpp) + c;
                                sum += src[src_idx] as u32;
                                count += 1;
                            }
                        }
                    }

                    let dst_idx = ((y * dst_width + x) as usize * bpp) + c;
                    dst[dst_idx] = (sum / count) as u8;
                }
            }
        }

        dst
    }
}

/// Texture animation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureAnimationType {
    Loop,
    PingPong,
    Once,
    Manual,
}

/// Texture animation info
#[derive(Debug, Clone)]
pub struct TextureAnimation {
    pub animation_type: TextureAnimationType,
    pub frame_count: u32,
    pub frame_rate: f32,
    pub current_frame: f32,
}

impl TextureAnimation {
    pub fn new(animation_type: TextureAnimationType, frame_count: u32, frame_rate: f32) -> Self {
        Self {
            animation_type,
            frame_count,
            frame_rate,
            current_frame: 0.0,
        }
    }

    pub fn update(&mut self, delta_time: f32) {
        self.current_frame += self.frame_rate * delta_time;

        match self.animation_type {
            TextureAnimationType::Loop => {
                if self.current_frame >= self.frame_count as f32 {
                    self.current_frame = 0.0;
                }
            }
            TextureAnimationType::PingPong => {
                let cycle = self.frame_count as f32 * 2.0;
                self.current_frame %= cycle;
                if self.current_frame >= self.frame_count as f32 {
                    self.current_frame = cycle - self.current_frame;
                }
            }
            TextureAnimationType::Once => {
                if self.current_frame >= self.frame_count as f32 {
                    self.current_frame = (self.frame_count - 1) as f32;
                }
            }
            TextureAnimationType::Manual => {
                // Do nothing, frame is set manually
            }
        }
    }

    pub fn get_frame_index(&self) -> u32 {
        self.current_frame.floor() as u32
    }
}

/// Texture class
#[derive(Debug, Clone)]
pub struct Texture {
    name: String,
    file_path: PathBuf,
    data: Option<TextureData>,
    animation: Option<TextureAnimation>,
    attributes: u32,
}

impl Texture {
    pub fn new(name: String) -> Self {
        Self {
            name,
            file_path: PathBuf::new(),
            data: None,
            animation: None,
            attributes: 0,
        }
    }

    pub fn from_w3d(w3d_tex: &W3dTextureStruct) -> Self {
        let name = String::from_utf8_lossy(&w3d_tex.name)
            .trim_end_matches('\0')
            .to_string();
        Self {
            name,
            file_path: PathBuf::new(),
            data: None,
            animation: None,
            attributes: 0,
        }
    }

    pub fn with_file_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.file_path = path.as_ref().to_path_buf();
        self
    }

    pub fn with_data(mut self, data: TextureData) -> Self {
        self.data = Some(data);
        self
    }

    pub fn with_animation(mut self, animation: TextureAnimation) -> Self {
        self.animation = Some(animation);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn data(&self) -> Option<&TextureData> {
        self.data.as_ref()
    }

    pub fn data_mut(&mut self) -> Option<&mut TextureData> {
        self.data.as_mut()
    }

    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }

    pub fn animation(&self) -> Option<&TextureAnimation> {
        self.animation.as_ref()
    }

    pub fn animation_mut(&mut self) -> Option<&mut TextureAnimation> {
        self.animation.as_mut()
    }

    pub fn update(&mut self, delta_time: f32) {
        if let Some(ref mut anim) = self.animation {
            anim.update(delta_time);
        }
    }

    pub fn set_attributes(&mut self, attributes: u32) {
        self.attributes = attributes;
    }

    pub fn attributes(&self) -> u32 {
        self.attributes
    }
}

/// Reference-counted texture
pub type TextureRef = Arc<Texture>;

/// Texture loader trait
pub trait TextureLoader: Send + Sync {
    /// Load a texture from a file
    fn load_from_file(&self, path: &Path) -> W3DResult<Texture>;

    /// Load a texture from memory
    fn load_from_memory(&self, name: String, data: &[u8]) -> W3DResult<Texture>;

    /// Get supported file extensions
    fn supported_extensions(&self) -> &[&str];
}

/// Texture manager for caching and managing textures
#[derive(Debug)]
pub struct TextureManager {
    textures: Vec<Arc<Texture>>,
    texture_map: std::collections::HashMap<String, usize>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: Vec::new(),
            texture_map: std::collections::HashMap::new(),
        }
    }

    pub fn add_texture(&mut self, texture: Texture) -> Arc<Texture> {
        let name = texture.name().to_string();
        let texture_ref = Arc::new(texture);

        let index = self.textures.len();
        self.textures.push(Arc::clone(&texture_ref));
        self.texture_map.insert(name, index);

        texture_ref
    }

    pub fn get_texture(&self, name: &str) -> Option<Arc<Texture>> {
        self.texture_map
            .get(name)
            .and_then(|&index| self.textures.get(index))
            .map(Arc::clone)
    }

    pub fn get_texture_by_index(&self, index: usize) -> Option<Arc<Texture>> {
        self.textures.get(index).map(Arc::clone)
    }

    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    pub fn clear(&mut self) {
        self.textures.clear();
        self.texture_map.clear();
    }

    pub fn remove_texture(&mut self, name: &str) -> Option<Arc<Texture>> {
        if let Some(&index) = self.texture_map.get(name) {
            self.texture_map.remove(name);
            // Note: We don't remove from the vector to preserve indices
            // The slot will be reused if a new texture is added
            Some(Arc::clone(&self.textures[index]))
        } else {
            None
        }
    }
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a solid color texture
pub fn create_solid_color_texture(name: String, color: [u8; 4], size: u32) -> Texture {
    let dimensions = TextureDimensions::new(size, size);
    let pixel_count = (size * size) as usize;
    let mut data = Vec::with_capacity(pixel_count * 4);

    for _ in 0..pixel_count {
        data.extend_from_slice(&color);
    }

    let texture_data = TextureData::with_data(dimensions, TextureFormat::RGBA8, data);

    Texture::new(name).with_data(texture_data)
}

/// Create a checkerboard texture for testing
pub fn create_checkerboard_texture(name: String, size: u32, checker_size: u32) -> Texture {
    let dimensions = TextureDimensions::new(size, size);
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let checker_x = (x / checker_size) % 2;
            let checker_y = (y / checker_size) % 2;
            let is_white = (checker_x + checker_y).is_multiple_of(2);

            let color = if is_white {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            };

            data.extend_from_slice(&color);
        }
    }

    let texture_data = TextureData::with_data(dimensions, TextureFormat::RGBA8, data);

    Texture::new(name).with_data(texture_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_format() {
        assert_eq!(TextureFormat::RGBA8.bytes_per_pixel(), 4);
        assert_eq!(TextureFormat::RGB8.bytes_per_pixel(), 3);
        assert!(TextureFormat::RGBA8.has_alpha());
        assert!(!TextureFormat::RGB8.has_alpha());
        assert!(TextureFormat::DXT1.is_compressed());
        assert!(!TextureFormat::RGBA8.is_compressed());
    }

    #[test]
    fn test_texture_dimensions() {
        let dims = TextureDimensions::new(256, 256);
        assert!(dims.is_power_of_two());
        assert_eq!(dims.mip_levels(), 9); // 256 -> 128 -> 64 -> 32 -> 16 -> 8 -> 4 -> 2 -> 1

        let dims2 = TextureDimensions::new(100, 100);
        assert!(!dims2.is_power_of_two());
    }

    #[test]
    fn test_texture_data() {
        let dims = TextureDimensions::new(4, 4);
        let data = vec![0u8; 64]; // 4x4 RGBA
        let mut tex_data = TextureData::with_data(dims, TextureFormat::RGBA8, data);

        assert_eq!(tex_data.mip_count(), 1);
        assert!(tex_data.base_data().is_some());

        tex_data.add_mip_level(vec![0u8; 16]); // 2x2
        assert_eq!(tex_data.mip_count(), 2);
    }

    #[test]
    fn test_texture_animation() {
        let mut anim = TextureAnimation::new(TextureAnimationType::Loop, 10, 10.0);

        assert_eq!(anim.get_frame_index(), 0);

        anim.update(0.1); // Advance 1 frame
        assert_eq!(anim.get_frame_index(), 1);

        anim.update(0.9); // Advance to end
        assert_eq!(anim.get_frame_index(), 0); // Should loop
    }

    #[test]
    fn test_texture_creation() {
        let texture = Texture::new("test_texture".to_string()).with_file_path("textures/test.dds");

        assert_eq!(texture.name(), "test_texture");
        assert_eq!(texture.file_path(), Path::new("textures/test.dds"));
        assert!(!texture.is_loaded());
    }

    #[test]
    fn test_texture_manager() {
        let mut manager = TextureManager::new();

        let tex1 = Texture::new("texture1".to_string());
        let tex2 = Texture::new("texture2".to_string());

        manager.add_texture(tex1);
        manager.add_texture(tex2);

        assert_eq!(manager.texture_count(), 2);
        assert!(manager.get_texture("texture1").is_some());
        assert!(manager.get_texture("texture2").is_some());
        assert!(manager.get_texture("texture3").is_none());
    }

    #[test]
    fn test_solid_color_texture() {
        let texture = create_solid_color_texture("red".to_string(), [255, 0, 0, 255], 8);

        assert!(texture.is_loaded());
        let data = texture.data().unwrap();
        assert_eq!(data.dimensions.width, 8);
        assert_eq!(data.dimensions.height, 8);

        let base = data.base_data().unwrap();
        assert_eq!(base[0], 255); // R
        assert_eq!(base[1], 0); // G
        assert_eq!(base[2], 0); // B
        assert_eq!(base[3], 255); // A
    }

    #[test]
    fn test_checkerboard_texture() {
        let texture = create_checkerboard_texture("checker".to_string(), 8, 4);

        assert!(texture.is_loaded());
        let data = texture.data().unwrap();
        assert_eq!(data.dimensions.width, 8);
        assert_eq!(data.dimensions.height, 8);
    }

    #[test]
    fn test_mipmap_generation() {
        let dims = TextureDimensions::new(4, 4);
        let data = vec![255u8; 64]; // 4x4 RGBA, all white
        let mut tex_data = TextureData::with_data(dims, TextureFormat::RGBA8, data);

        tex_data.generate_mipmaps();

        // Should have 3 mip levels: 4x4, 2x2, 1x1
        assert_eq!(tex_data.mip_count(), 3);
    }
}
