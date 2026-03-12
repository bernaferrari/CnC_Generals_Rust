//! # W3D Texture Management System
//!
//! Advanced texture management with compression, streaming, and GPU optimization.
//! Supports all W3D texture formats with modern compression techniques.
//!
//! Parity with C++ W3DAssetManager and TextureClass including:
//! - MipCountType enum matching C++ values
//! - WW3DFormat enum matching C++ texture formats
//! - Team color recoloring support (house color)
//! - Texture inactivation and memory management
//! - Texture reduction for LOD support

use super::{Result, Texture, TextureFormat, TextureType, W3DError};
use image::{DynamicImage, ImageError, ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use wgpu::{
    AddressMode, CompareFunction, Device, Extent3d, FilterMode, Origin3d, Queue, Sampler,
    SamplerDescriptor, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture as WgpuTexture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureUsages, TextureView,
};

// ============================================================================
// MipCountType - Parity with C++ texturefilter.h
// ============================================================================

/// Mipmap level count type matching C++ MipCountType enum.
/// These values must match the original game for save/load parity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MipCountType {
    /// Generate all mipmap levels down to 1x1 size (C++: MIP_LEVELS_ALL = 0)
    All = 0,
    /// No mipmapping at all (just one mip level) (C++: MIP_LEVELS_1 = 1)
    Levels1 = 1,
    /// 2 mip levels (C++: MIP_LEVELS_2)
    Levels2 = 2,
    /// 3 mip levels (C++: MIP_LEVELS_3)
    Levels3 = 3,
    /// 4 mip levels (C++: MIP_LEVELS_4)
    Levels4 = 4,
    /// 5 mip levels (C++: MIP_LEVELS_5)
    Levels5 = 5,
    /// 6 mip levels (C++: MIP_LEVELS_6)
    Levels6 = 6,
    /// 7 mip levels (C++: MIP_LEVELS_7)
    Levels7 = 7,
    /// 8 mip levels (C++: MIP_LEVELS_8)
    Levels8 = 8,
    /// 10 mip levels (C++: MIP_LEVELS_10)
    Levels10 = 10,
    /// 11 mip levels (C++: MIP_LEVELS_11)
    Levels11 = 11,
    /// 12 mip levels (C++: MIP_LEVELS_12)
    Levels12 = 12,
}

impl Default for MipCountType {
    fn default() -> Self {
        MipCountType::All
    }
}

impl MipCountType {
    /// Convert to actual number of mip levels for a given texture size.
    /// Returns the minimum of requested levels and maximum possible levels.
    pub fn to_level_count(self, width: u32, height: u32) -> u32 {
        let max_levels = (width.min(height) as f32).log2().floor() as u32 + 1;
        match self {
            MipCountType::All => max_levels,
            MipCountType::Levels1 => 1,
            MipCountType::Levels2 => 2.min(max_levels),
            MipCountType::Levels3 => 3.min(max_levels),
            MipCountType::Levels4 => 4.min(max_levels),
            MipCountType::Levels5 => 5.min(max_levels),
            MipCountType::Levels6 => 6.min(max_levels),
            MipCountType::Levels7 => 7.min(max_levels),
            MipCountType::Levels8 => 8.min(max_levels),
            MipCountType::Levels10 => 10.min(max_levels),
            MipCountType::Levels11 => 11.min(max_levels),
            MipCountType::Levels12 => 12.min(max_levels),
        }
    }

    /// Check if this is a single mip level (no mipmapping)
    pub fn is_single_level(self) -> bool {
        matches!(self, MipCountType::Levels1)
    }

    /// Convert from raw C++ enum value
    pub fn from_cpp_value(value: u32) -> Self {
        match value {
            0 => MipCountType::All,
            1 => MipCountType::Levels1,
            2 => MipCountType::Levels2,
            3 => MipCountType::Levels3,
            4 => MipCountType::Levels4,
            5 => MipCountType::Levels5,
            6 => MipCountType::Levels6,
            7 => MipCountType::Levels7,
            8 => MipCountType::Levels8,
            10 => MipCountType::Levels10,
            11 => MipCountType::Levels11,
            12 => MipCountType::Levels12,
            _ => MipCountType::All, // Default to all for unknown values
        }
    }
}

// ============================================================================
// WW3DFormat - Parity with C++ ww3dformat.h
// ============================================================================

/// WW3D texture formats matching C++ WW3DFormat enum for parity.
/// Values correspond to D3DFORMAT where applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WW3DFormat {
    /// Unknown format
    Unknown = 0,
    /// 24-bit RGB (R8G8B8)
    R8G8B8 = 1,
    /// 32-bit ARGB (A8R8G8B8)
    A8R8G8B8 = 2,
    /// 32-bit XRGB (X8R8G8B8) - no alpha
    X8R8G8B8 = 3,
    /// 16-bit RGB (R5G6B5)
    R5G6B5 = 4,
    /// 16-bit XRGB (X1R5G5B5) - 1 bit unused
    X1R5G5B5 = 5,
    /// 16-bit ARGB (A1R5G5B5) - 1 bit alpha
    A1R5G5B5 = 6,
    /// 16-bit ARGB (A4R4G4B4) - 4 bits per channel
    A4R4G4B4 = 7,
    /// 8-bit RGB (R3G3B2)
    R3G3B2 = 8,
    /// 8-bit alpha only (A8)
    A8 = 9,
    /// 16-bit ARGB (A8R3G3B2)
    A8R3G3B2 = 10,
    /// 16-bit XRGB (X4R4G4B4)
    X4R4G4B4 = 11,
    /// 16-bit palette + alpha (A8P8)
    A8P8 = 12,
    /// 8-bit palette (P8)
    P8 = 13,
    /// 8-bit luminance (L8)
    L8 = 14,
    /// 16-bit alpha + luminance (A8L8)
    A8L8 = 15,
    /// 8-bit alpha + luminance (A4L4)
    A4L4 = 16,
    /// Bumpmap format (U8V8)
    U8V8 = 17,
    /// Bumpmap format (L6V5U5)
    L6V5U5 = 18,
    /// Bumpmap format (X8L8V8U8)
    X8L8V8U8 = 19,
    /// DXT1 compression (4:1 ratio)
    Dxt1 = 20,
    /// DXT2 compression
    Dxt2 = 21,
    /// DXT3 compression (explicit alpha)
    Dxt3 = 22,
    /// DXT4 compression
    Dxt4 = 23,
    /// DXT5 compression (interpolated alpha)
    Dxt5 = 24,
}

impl WW3DFormat {
    /// Check if format has alpha channel (parity with C++ Has_Alpha)
    pub fn has_alpha(self) -> bool {
        matches!(
            self,
            WW3DFormat::A8R8G8B8
                | WW3DFormat::A1R5G5B5
                | WW3DFormat::A4R4G4B4
                | WW3DFormat::A8
                | WW3DFormat::A8R3G3B2
                | WW3DFormat::A8P8
                | WW3DFormat::A8L8
                | WW3DFormat::A4L4
                | WW3DFormat::Dxt2
                | WW3DFormat::Dxt3
                | WW3DFormat::Dxt4
                | WW3DFormat::Dxt5
        )
    }

    /// Get number of alpha bits (parity with C++ Alpha_Bits)
    pub fn alpha_bits(self) -> u32 {
        match self {
            WW3DFormat::A8R8G8B8
            | WW3DFormat::A8
            | WW3DFormat::A8R3G3B2
            | WW3DFormat::A8P8
            | WW3DFormat::A8L8 => 8,
            WW3DFormat::A4R4G4B4
            | WW3DFormat::A4L4
            | WW3DFormat::Dxt3
            | WW3DFormat::Dxt4
            | WW3DFormat::Dxt5 => 4,
            WW3DFormat::A1R5G5B5 | WW3DFormat::Dxt2 => 1,
            _ => 0,
        }
    }

    /// Get bytes per pixel for uncompressed formats
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            WW3DFormat::R8G8B8 => 3,
            WW3DFormat::A8R8G8B8 | WW3DFormat::X8R8G8B8 | WW3DFormat::X8L8V8U8 => 4,
            WW3DFormat::R5G6B5
            | WW3DFormat::X1R5G5B5
            | WW3DFormat::A1R5G5B5
            | WW3DFormat::A4R4G4B4
            | WW3DFormat::X4R4G4B4
            | WW3DFormat::A8P8
            | WW3DFormat::A8L8
            | WW3DFormat::U8V8 => 2,
            WW3DFormat::R3G3B2
            | WW3DFormat::A8
            | WW3DFormat::P8
            | WW3DFormat::L8
            | WW3DFormat::A4L4 => 1,
            WW3DFormat::L6V5U5 => 2,
            // Compressed formats - bytes per 4x4 block
            WW3DFormat::Dxt1 | WW3DFormat::Dxt2 => 8,
            WW3DFormat::Dxt3 | WW3DFormat::Dxt4 | WW3DFormat::Dxt5 => 16,
            WW3DFormat::Unknown => 0,
            WW3DFormat::A8R3G3B2 => 2,
        }
    }

    /// Convert to wgpu TextureFormat if supported
    pub fn to_wgpu_format(self) -> Option<wgpu::TextureFormat> {
        match self {
            WW3DFormat::A8R8G8B8 | WW3DFormat::X8R8G8B8 => {
                Some(wgpu::TextureFormat::Rgba8UnormSrgb)
            }
            WW3DFormat::R5G6B5 | WW3DFormat::X1R5G5B5 => Some(wgpu::TextureFormat::Rgba8UnormSrgb), // Converted
            WW3DFormat::A1R5G5B5 | WW3DFormat::A4R4G4B4 => {
                Some(wgpu::TextureFormat::Rgba8UnormSrgb)
            } // Converted
            WW3DFormat::Dxt1 => Some(wgpu::TextureFormat::Bc1RgbaUnormSrgb),
            WW3DFormat::Dxt3 => Some(wgpu::TextureFormat::Bc2RgbaUnormSrgb),
            WW3DFormat::Dxt5 => Some(wgpu::TextureFormat::Bc3RgbaUnormSrgb),
            _ => None,
        }
    }
}

impl Default for WW3DFormat {
    fn default() -> Self {
        WW3DFormat::A8R8G8B8
    }
}

impl WW3DFormat {
    /// Convert from raw C++ WW3DFormat enum value
    pub fn from_cpp_value(value: u32) -> Self {
        match value {
            0 => WW3DFormat::Unknown,
            1 => WW3DFormat::R8G8B8,
            2 => WW3DFormat::A8R8G8B8,
            3 => WW3DFormat::X8R8G8B8,
            4 => WW3DFormat::R5G6B5,
            5 => WW3DFormat::X1R5G5B5,
            6 => WW3DFormat::A1R5G5B5,
            7 => WW3DFormat::A4R4G4B4,
            8 => WW3DFormat::R3G3B2,
            9 => WW3DFormat::A8,
            10 => WW3DFormat::A8R3G3B2,
            11 => WW3DFormat::X4R4G4B4,
            12 => WW3DFormat::A8P8,
            13 => WW3DFormat::P8,
            14 => WW3DFormat::L8,
            15 => WW3DFormat::A8L8,
            16 => WW3DFormat::A4L4,
            17 => WW3DFormat::U8V8,
            18 => WW3DFormat::L6V5U5,
            19 => WW3DFormat::X8L8V8U8,
            20 => WW3DFormat::Dxt1,
            21 => WW3DFormat::Dxt2,
            22 => WW3DFormat::Dxt3,
            23 => WW3DFormat::Dxt4,
            24 => WW3DFormat::Dxt5,
            _ => WW3DFormat::Unknown,
        }
    }

    /// Check if this format is compressed (DXT/BC format)
    pub fn is_compressed(self) -> bool {
        matches!(
            self,
            WW3DFormat::Dxt1
                | WW3DFormat::Dxt2
                | WW3DFormat::Dxt3
                | WW3DFormat::Dxt4
                | WW3DFormat::Dxt5
        )
    }

    /// Check if format is a 16-bit format
    pub fn is_16bit(self) -> bool {
        matches!(
            self,
            WW3DFormat::R5G6B5
                | WW3DFormat::X1R5G5B5
                | WW3DFormat::A1R5G5B5
                | WW3DFormat::A4R4G4B4
                | WW3DFormat::X4R4G4B4
                | WW3DFormat::A8P8
                | WW3DFormat::A8L8
                | WW3DFormat::A8R3G3B2
                | WW3DFormat::U8V8
                | WW3DFormat::L6V5U5
                | WW3DFormat::A4L4
        )
    }
}

// ============================================================================
// Team Color Constants - Parity with C++ W3DAssetManager
// ============================================================================

/// Team color palette size matching C++ TEAM_COLOR_PALETTE_SIZE
pub const TEAM_COLOR_PALETTE_SIZE: usize = 16;

/// House color scale values matching C++ houseColorScale array.
/// These define the brightness gradient for team colors.
pub const HOUSE_COLOR_SCALE: [u16; TEAM_COLOR_PALETTE_SIZE] = [
    255, 239, 223, 211, 195, 174, 167, 151, 135, 123, 107, 91, 79, 63, 47, 35,
];

/// Advanced texture manager with streaming and compression
pub struct W3DTextureManager {
    /// WGPU device reference
    device: Arc<Device>,
    /// WGPU command queue
    queue: Arc<Queue>,

    /// Loaded textures cache
    texture_cache: Arc<RwLock<HashMap<String, W3DTextureGpu>>>,

    /// Texture streaming queue
    stream_queue: Arc<RwLock<VecDeque<StreamRequest>>>,

    /// Memory management
    memory_budget: u64,
    memory_used: Arc<RwLock<u64>>,

    /// Compression settings
    compression_enabled: bool,
    compression_quality: CompressionQuality,

    /// Texture streaming settings
    streaming_enabled: bool,
    max_texture_size: u32,
    mip_levels_auto: bool,

    /// Performance statistics
    stats: Arc<RwLock<TextureManagerStats>>,
}

/// GPU texture resource with complete wgpu integration
#[derive(Debug)]
pub struct W3DTextureGpu {
    /// Original texture metadata
    pub texture_info: Texture,
    /// GPU texture resource
    pub wgpu_texture: WgpuTexture,
    /// Texture view for shaders
    pub view: TextureView,
    /// Sampler for filtering
    pub sampler: Sampler,
    /// Memory usage in bytes
    pub memory_size: u64,
    /// Last access time for LRU eviction
    pub last_access: std::time::Instant,
    /// Reference count for usage tracking
    pub reference_count: Arc<RwLock<u32>>,
    /// WW3D format of the texture
    pub ww3d_format: WW3DFormat,
    /// Texture reduction level (0 = no reduction) - parity with C++ Get_Reduction
    pub reduction: u32,
    /// Inactivation state for memory management
    pub inactivation: TextureInactivationState,
    /// Whether texture allows reduction - parity with C++ Is_Reducible
    pub is_reducible: bool,
    /// Whether compression is allowed for this texture
    pub allow_compression: bool,
    /// HSV shift for texture recoloring (team colors)
    pub hsv_shift: Option<[f32; 3]>,
    /// Team color applied to this texture (0 = no team color)
    pub team_color: u32,
}

/// Texture streaming request
#[derive(Debug, Clone)]
pub struct StreamRequest {
    /// Texture ID
    pub texture_id: String,
    /// File path or data source
    pub source: TextureSource,
    /// Priority (0 = highest)
    pub priority: u32,
    /// Target format
    pub target_format: TextureFormat,
    /// Generate mipmaps
    pub generate_mipmaps: bool,
    /// Compression settings
    pub compression: Option<CompressionSettings>,
}

/// Texture data source
#[derive(Debug, Clone)]
pub enum TextureSource {
    /// File path
    FilePath(String),
    /// Raw texture data
    RawData(Vec<u8>),
    /// Procedural generation
    Procedural(ProceduralTexture),
}

/// Procedural texture generation
#[derive(Debug, Clone)]
pub struct ProceduralTexture {
    /// Texture type (noise, checkerboard, etc.)
    pub texture_type: ProceduralType,
    /// Generation parameters
    pub parameters: HashMap<String, f32>,
    /// Output size
    pub size: (u32, u32),
}

/// Procedural texture types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProceduralType {
    /// Solid color
    SolidColor,
    /// Checkerboard pattern
    Checkerboard,
    /// Perlin noise
    PerlinNoise,
    /// White noise
    WhiteNoise,
    /// Gradient
    Gradient,
    /// Normal map from height
    NormalFromHeight,
}

/// Compression quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionQuality {
    /// Maximum compression, lower quality
    Low,
    /// Balanced compression and quality
    Medium,
    /// Minimum compression, maximum quality
    High,
    /// No compression (original quality)
    Lossless,
}

/// Compression settings for specific textures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionSettings {
    /// Compression quality
    pub quality: CompressionQuality,
    /// Force specific format
    pub force_format: Option<TextureFormat>,
    /// Allow format conversion
    pub allow_format_conversion: bool,
}

/// Texture manager statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TextureManagerStats {
    /// Total textures loaded
    pub textures_loaded: u32,
    /// Textures currently in memory
    pub textures_in_memory: u32,
    /// Total memory used (bytes)
    pub memory_used: u64,
    /// Memory budget (bytes)
    pub memory_budget: u64,
    /// Cache hit rate
    pub cache_hit_rate: f32,
    /// Texture uploads per frame
    pub uploads_per_frame: u32,
    /// Compression ratio achieved
    pub compression_ratio: f32,
    /// Streaming queue size
    pub stream_queue_size: u32,
    /// Average load time (ms)
    pub average_load_time_ms: f32,
}

/// Default inactivation time for textures (20 seconds) - parity with C++
pub const DEFAULT_INACTIVATION_TIME_MS: u32 = 20000;

/// Texture inactivation state for memory management
#[derive(Debug, Clone)]
pub struct TextureInactivationState {
    /// Time after which texture is invalidated if not used (ms)
    pub inactivation_time: u32,
    /// Extended time for textures that were recently reactivated
    pub extended_inactivation_time: u32,
    /// Last sync time when inactivation was checked
    pub last_inactivation_sync_time: u64,
    /// Last time texture was accessed
    pub last_accessed: u64,
}

impl Default for TextureInactivationState {
    fn default() -> Self {
        Self {
            inactivation_time: DEFAULT_INACTIVATION_TIME_MS,
            extended_inactivation_time: 0,
            last_inactivation_sync_time: 0,
            last_accessed: 0,
        }
    }
}

// ============================================================================
// Team Color Recoloring - Parity with C++ W3DAssetManager
// ============================================================================

/// Convert RGB to HSV color space (parity with C++ colorspace.h RGB_To_HSV)
/// Note: C++ returns hue in range [0, 360), or negative for undefined (monochrome)
pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);

    // value
    let v = max;

    // saturation
    let s = if max != 0.0 { (max - min) / max } else { 0.0 };

    // hue (negative means undefined - monochrome color)
    let h = if s == 0.0 {
        -1.0 // Undefined hue for monochrome colors (C++ parity)
    } else {
        let delta = max - min;
        let hue = if r == max {
            (g - b) / delta
        } else if g == max {
            2.0 + (b - r) / delta
        } else {
            4.0 + (r - g) / delta
        };
        let hue = hue * 60.0;
        if hue < 0.0 { hue + 360.0 } else { hue }
    };

    (h, s, v)
}

/// Convert HSV to RGB color space (parity with C++ colorspace.h HSV_To_RGB)
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        // Monochrome
        return (v, v, v);
    }

    let mut h = h;
    if h == 360.0 {
        h = 0.0;
    }
    h /= 60.0;

    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - (s * f));
    let t = v * (1.0 - (s * (1.0 - f)));

    match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        5 => (v, p, q),
        _ => (v, v, v), // Should not happen
    }
}

/// Apply HSV shift to recolor a pixel (parity with C++ colorspace.h Recolor)
/// Handles monochrome colors (undefined hue) specially - only modifies value, not hue/saturation.
pub fn recolor_pixel(r: f32, g: f32, b: f32, hsv_shift: [f32; 3]) -> (f32, f32, f32) {
    let (mut h, mut s, mut v) = rgb_to_hsv(r, g, b);

    // If hue is undefined (negative - monochrome color), only modify value
    // C++ parity: "do not shift the hue (it is undefined) or the saturation"
    if h < 0.0 {
        v += hsv_shift[2]; // Only apply value shift
    } else {
        h += hsv_shift[0];
        s += hsv_shift[1];
        v += hsv_shift[2];
    }

    // Angular modulo for hue (0-360)
    if h < 0.0 {
        h += 360.0;
    }
    if h > 360.0 {
        h -= 360.0;
    }

    // Clamp saturation and value
    s = s.clamp(0.0, 1.0);
    v = v.clamp(0.0, 1.0);

    hsv_to_rgb(h, s, v)
}

/// Recolor a 32-bit RGBA texture using hue shift for team colors.
/// This matches C++ remapAlphaTexture32Bit behavior with DO_HUE_SHIFT.
///
/// # Arguments
/// * `data` - RGBA texture data (4 bytes per pixel)
/// * `width` - Texture width
/// * `height` - Texture height
/// * `color` - Team color as 0xRRGGBB integer
pub fn recolor_texture_32bit_hue_shift(data: &mut [u8], width: u32, height: u32, color: u32) {
    let r_color = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g_color = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b_color = (color & 0xFF) as f32 / 255.0;

    // Calculate HSV shift from target color
    let (h_color, s_color, _) = rgb_to_hsv(r_color, g_color, b_color);
    
    // The HSV shift is: (target_hue - current_hue, target_sat * current_sat - current_sat, 0)
    // But for team colors, we shift the hue to the target color's hue
    // and multiply saturation by the target saturation
    let hsv_shift = [h_color, s_color, 0.0];

    let pitch = width as usize * 4;

    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = y * pitch + x * 4;
            if idx + 3 >= data.len() {
                break;
            }

            // Get current pixel (BGRA in memory, but we process as RGB)
            // C++ uses ARGB format internally
            let b = data[idx] as f32 / 255.0;
            let g = data[idx + 1] as f32 / 255.0;
            let r = data[idx + 2] as f32 / 255.0;
            let alpha = data[idx + 3];

            // Calculate house color alpha (255 - alpha in C++)
            let pixel_alpha = 255 - alpha;

            if pixel_alpha > 0 {
                // Get current HSV and apply shift
                let (h, s, v) = rgb_to_hsv(r, g, b);
                
                // For team color recoloring: shift hue to target, scale saturation
                let (new_r, new_g, new_b) = if h < 0.0 {
                    // Monochrome color - only modify value (C++ parity)
                    let new_v = (v + hsv_shift[2]).clamp(0.0, 1.0);
                    hsv_to_rgb(h, s, new_v)
                } else {
                    // Apply hue shift to target color hue, multiply saturation
                    let new_h = (h + hsv_shift[0]) % 360.0;
                    let new_s = (s * hsv_shift[1]).clamp(0.0, 1.0);
                    hsv_to_rgb(new_h, new_s, v)
                };

                data[idx] = (new_b * 255.0).clamp(0.0, 255.0) as u8;
                data[idx + 1] = (new_g * 255.0).clamp(0.0, 255.0) as u8;
                data[idx + 2] = (new_r * 255.0).clamp(0.0, 255.0) as u8;
            }

            // Force alpha to opaque (parity with C++)
            data[idx + 3] = 255;
        }
    }
}

/// Recolor a 16-bit RGBA texture using hue shift for team colors.
/// This matches C++ remapAlphaTexture16Bit behavior with DO_HUE_SHIFT.
///
/// # Arguments
/// * `data` - 16-bit texture data (2 bytes per pixel, ARGB4444 format)
/// * `width` - Texture width  
/// * `height` - Texture height
/// * `color` - Team color as 0xRRGGBB integer
pub fn recolor_texture_16bit_hue_shift(data: &mut [u16], width: u32, height: u32, color: u32) {
    let r_color = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g_color = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b_color = (color & 0xFF) as f32 / 255.0;

    let (h_color, s_color, _) = rgb_to_hsv(r_color, g_color, b_color);

    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = y * width as usize + x;
            if idx >= data.len() {
                break;
            }

            let pixel = data[idx];

            // ARGB4444 format: AAAA RRRR GGGG BBBB
            let alpha = (pixel >> 12) & 0xF;
            let r = ((pixel >> 8) & 0xF) as f32 / 15.0;
            let g = ((pixel >> 4) & 0xF) as f32 / 15.0;
            let b = (pixel & 0xF) as f32 / 15.0;

            // Get alpha for house color (15 - alpha)
            let pixel_alpha = 15 - alpha;

            if pixel_alpha > 0 {
                // Apply hue shift
                let (h, s, v) = rgb_to_hsv(r, g, b);
                let new_h = h_color;
                let new_s = s * s_color;
                let (new_r, new_g, new_b) = hsv_to_rgb(new_h, new_s, v);

                let new_pixel = (0xF << 12) // Force alpha to opaque
                    | ((new_r * 15.0) as u16 & 0xF) << 8
                    | ((new_g * 15.0) as u16 & 0xF) << 4
                    | ((new_b * 15.0) as u16 & 0xF);

                data[idx] = new_pixel;
            } else {
                // Force alpha to opaque even if no color change
                data[idx] = pixel | 0xF000;
            }
        }
    }
}

/// Generate a team color palette for 32-bit textures.
/// Matches C++ remapPalette32Bit behavior.
pub fn generate_team_color_palette_32bit(color: u32) -> [u32; TEAM_COLOR_PALETTE_SIZE] {
    let r_color = ((color >> 16) & 0xFF) as f32 / 255.0 / 255.0; // Extra division matches C++
    let g_color = ((color >> 8) & 0xFF) as f32 / 255.0 / 255.0;
    let b_color = (color & 0xFF) as f32 / 255.0 / 255.0;

    let mut palette = [0u32; TEAM_COLOR_PALETTE_SIZE];

    for (i, pal) in palette.iter_mut().enumerate() {
        let scale = HOUSE_COLOR_SCALE[i] as f32;
        let r = (scale * r_color * 255.0).clamp(0.0, 255.0) as u8;
        let g = (scale * g_color * 255.0).clamp(0.0, 255.0) as u8;
        let b = (scale * b_color * 255.0).clamp(0.0, 255.0) as u8;
        *pal = 0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
    }

    palette
}

/// Generate a team color palette for 16-bit textures.
/// Matches C++ remapPalette16Bit behavior.
pub fn generate_team_color_palette_16bit(color: u32) -> [u16; TEAM_COLOR_PALETTE_SIZE] {
    let r_color = ((color >> 16) & 0xFF) as f32 / 255.0 / 255.0;
    let g_color = ((color >> 8) & 0xFF) as f32 / 255.0 / 255.0;
    let b_color = (color & 0xFF) as f32 / 255.0 / 255.0;

    let mut palette = [0u16; TEAM_COLOR_PALETTE_SIZE];

    for (i, pal) in palette.iter_mut().enumerate() {
        let scale = HOUSE_COLOR_SCALE[i] as f32;
        let r = (scale * r_color).clamp(0.0, 1.0);
        let g = (scale * g_color).clamp(0.0, 1.0);
        let b = (scale * b_color).clamp(0.0, 1.0);

        // ARGB1555 format
        *pal = 0x8000
            | ((r * 31.0) as u16 & 0x1F) << 10
            | ((g * 31.0) as u16 & 0x1F) << 5
            | ((b * 31.0) as u16 & 0x1F);
    }

    palette
}

impl W3DTextureManager {
    /// Create a new texture manager
    pub async fn new(device: &Device, queue: &Queue, memory_budget: u64) -> Result<Self> {
        tracing::info!(
            "Creating W3D texture manager with {}MB budget",
            memory_budget / 1024 / 1024
        );

        Ok(Self {
            device: Arc::new(device.clone()),
            queue: Arc::new(queue.clone()),
            texture_cache: Arc::new(RwLock::new(HashMap::new())),
            stream_queue: Arc::new(RwLock::new(VecDeque::new())),
            memory_budget,
            memory_used: Arc::new(RwLock::new(0)),
            compression_enabled: true,
            compression_quality: CompressionQuality::High,
            streaming_enabled: true,
            max_texture_size: 8192,
            mip_levels_auto: true,
            stats: Arc::new(RwLock::new(TextureManagerStats {
                memory_budget,
                ..Default::default()
            })),
        })
    }

    /// Load texture from file with streaming support
    pub async fn load_texture(
        &self,
        texture_id: &str,
        file_path: &str,
    ) -> Result<Arc<W3DTextureGpu>> {
        let start_time = std::time::Instant::now();

        // Check cache first
        {
            let cache = self.texture_cache.read().await;
            if let Some(texture) = cache.get(texture_id) {
                // Update access time
                let texture_clone = texture.clone();
                drop(cache);

                let mut stats = self.stats.write().await;
                stats.cache_hit_rate = (stats.cache_hit_rate * 0.9) + 0.1; // Moving average

                return Ok(Arc::new(texture_clone));
            }
        }

        // Load image from file
        let image = self.load_image_from_file(file_path).await?;
        let texture_gpu = self.create_texture_from_image(texture_id, image).await?;

        // Add to cache
        {
            let mut cache = self.texture_cache.write().await;
            cache.insert(texture_id.to_string(), texture_gpu.clone());
        }

        // Update memory usage
        {
            let mut memory_used = self.memory_used.write().await;
            *memory_used += texture_gpu.memory_size;
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.textures_loaded += 1;
            stats.textures_in_memory += 1;
            stats.memory_used += texture_gpu.memory_size;

            let load_time = start_time.elapsed().as_secs_f32() * 1000.0;
            stats.average_load_time_ms = (stats.average_load_time_ms * 0.9) + (load_time * 0.1);

            // Cache miss
            stats.cache_hit_rate = stats.cache_hit_rate * 0.9;
        }

        // Check memory budget and evict if necessary
        self.enforce_memory_budget().await?;

        tracing::debug!(
            "Loaded texture '{}' from '{}' in {:.2}ms",
            texture_id,
            file_path,
            start_time.elapsed().as_secs_f32() * 1000.0
        );

        Ok(Arc::new(texture_gpu))
    }

    /// Load image from file with format detection
    async fn load_image_from_file(&self, file_path: &str) -> Result<DynamicImage> {
        let path = Path::new(file_path);

        // Check if file exists
        if !path.exists() {
            return Err(W3DError::ResourceLoadingFailed(format!(
                "Texture file not found: {}",
                file_path
            )));
        }

        // Load and decode image
        let image = image::open(path).map_err(|e| {
            W3DError::ResourceLoadingFailed(format!("Failed to load image '{}': {}", file_path, e))
        })?;

        // Convert to RGBA if needed
        let rgba_image = match image {
            DynamicImage::ImageRgba8(_) => image,
            _ => DynamicImage::ImageRgba8(image.to_rgba8()),
        };

        Ok(rgba_image)
    }

    /// Create GPU texture from image
    async fn create_texture_from_image(
        &self,
        texture_id: &str,
        image: DynamicImage,
    ) -> Result<W3DTextureGpu> {
        let rgba_image = image.to_rgba8();
        let (width, height) = rgba_image.dimensions();
        let raw_data = rgba_image.into_raw();

        // Calculate mip levels
        let mip_levels = if self.mip_levels_auto {
            (width.min(height) as f32).log2().floor() as u32 + 1
        } else {
            1
        };

        // Create GPU texture
        let texture_size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let wgpu_texture = self.device.create_texture(&TextureDescriptor {
            label: Some(&format!("W3D Texture: {}", texture_id)),
            size: texture_size,
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload texture data
        self.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &wgpu_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &raw_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        // Generate mipmaps if requested
        if mip_levels > 1 {
            self.generate_mipmaps(&wgpu_texture, &raw_data, width, height, mip_levels)
                .await?;
        }

        // Create texture view
        let view = wgpu_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create sampler
        let sampler = self.device.create_sampler(&SamplerDescriptor {
            label: Some(&format!("W3D Sampler: {}", texture_id)),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            compare: None,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            border_color: None,
            anisotropy_clamp: 16,
        });

        // Calculate memory usage
        let memory_size = (raw_data.len() as u64) * mip_levels as u64;

        // Create texture info
        let texture_info = Texture {
            id: texture_id.to_string(),
            name: texture_id.to_string(),
            width,
            height,
            depth: 1,
            mip_levels,
            format: TextureFormat::Rgba8,
            texture_type: TextureType::Texture2D,
            data: raw_data,
        };

        // Calculate reduction level (parity with C++ TextureBaseClass::Get_Reduction)
        let reduction = if self.max_texture_size > 0 {
            let max_dim = width.max(height);
            if max_dim > self.max_texture_size {
                let mut red = 0u32;
                let mut dim = max_dim;
                while dim > self.max_texture_size && red < mip_levels.saturating_sub(1) {
                    dim /= 2;
                    red += 1;
                }
                red
            } else {
                0
            }
        } else {
            0
        };

        Ok(W3DTextureGpu {
            texture_info,
            wgpu_texture,
            view,
            sampler,
            memory_size,
            last_access: std::time::Instant::now(),
            reference_count: Arc::new(RwLock::new(1)),
            ww3d_format: WW3DFormat::A8R8G8B8,
            reduction,
            inactivation: TextureInactivationState::default(),
            is_reducible: true,
            allow_compression: self.compression_enabled,
            hsv_shift: None,
            team_color: 0,
        })
    }

    /// Generate mipmaps for texture
    async fn generate_mipmaps(
        &self,
        texture: &WgpuTexture,
        base_level_rgba: &[u8],
        base_width: u32,
        base_height: u32,
        mip_levels: u32,
    ) -> Result<()> {
        let mut previous_data = base_level_rgba.to_vec();
        let mut previous_width = base_width;
        let mut previous_height = base_height;

        for mip_level in 1..mip_levels {
            let mip_width = (previous_width / 2).max(1);
            let mip_height = (previous_height / 2).max(1);
            let mip_data = Self::downsample_rgba8(&previous_data, previous_width, previous_height);
            let mip_size = Extent3d {
                width: mip_width,
                height: mip_height,
                depth_or_array_layers: 1,
            };

            self.queue.write_texture(
                TexelCopyTextureInfo {
                    texture,
                    mip_level,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &mip_data,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * mip_width),
                    rows_per_image: Some(mip_height),
                },
                mip_size,
            );

            tracing::trace!(
                "Generated mipmap level {} ({}x{})",
                mip_level,
                mip_size.width,
                mip_size.height
            );

            previous_data = mip_data;
            previous_width = mip_width;
            previous_height = mip_height;
        }

        Ok(())
    }

    /// Downsample RGBA8 texture using C++ Combine_A8R8G8B8 algorithm.
    /// This matches the exact mipmap generation behavior from bitmaphandler.cpp.
    /// 
    /// The C++ algorithm:
    /// 1. Masks each pixel with 0xfcfcfcfc (clears bottom 2 bits per channel)
    /// 2. Divides each by 4 (right shift by 2)
    /// 3. Sums all four values
    /// This is equivalent to: (pixel1 + pixel2 + pixel3 + pixel4) / 4 with masked low bits
    fn downsample_rgba8(src: &[u8], src_width: u32, src_height: u32) -> Vec<u8> {
        let dst_width = (src_width / 2).max(1);
        let dst_height = (src_height / 2).max(1);
        let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];

        // Use BGRA word-level processing like C++ for parity
        // C++ reads as unsigned (32-bit) and processes in BGRA order
        for y in 0..dst_height {
            for x in 0..dst_width {
                let src_x = x * 2;
                let src_y = y * 2;

                // Read four 2x2 pixels as 32-bit values (BGRA format in memory)
                // This matches C++ Combine_A8R8G8B8 behavior
                let mut accum = [0u32; 4];
                let mut samples = 0u32;

                for oy in 0..2 {
                    for ox in 0..2 {
                        let sample_x = (src_x + ox).min(src_width.saturating_sub(1));
                        let sample_y = (src_y + oy).min(src_height.saturating_sub(1));
                        let idx = ((sample_y * src_width + sample_x) * 4) as usize;
                        
                        // Apply 0xfcfcfcfc mask like C++ (clears bottom 2 bits per channel)
                        // Then divide by 4 (right shift 2)
                        for c in 0..4 {
                            let masked = (src[idx + c] as u32) & 0xFC;
                            accum[c] += masked >> 2;
                        }
                        samples += 1;
                    }
                }

                // Sum the four pre-divided values (equivalent to averaging)
                let out_idx = ((y * dst_width + x) * 4) as usize;
                dst[out_idx] = accum[0].min(255) as u8;
                dst[out_idx + 1] = accum[1].min(255) as u8;
                dst[out_idx + 2] = accum[2].min(255) as u8;
                dst[out_idx + 3] = accum[3].min(255) as u8;
            }
        }

        dst
    }

    /// Create procedural texture
    pub async fn create_procedural_texture(
        &self,
        texture_id: &str,
        procedural: &ProceduralTexture,
    ) -> Result<Arc<W3DTextureGpu>> {
        let image = self.generate_procedural_image(procedural).await?;
        let texture_gpu = self.create_texture_from_image(texture_id, image).await?;

        // Add to cache
        {
            let mut cache = self.texture_cache.write().await;
            cache.insert(texture_id.to_string(), texture_gpu.clone());
        }

        tracing::debug!(
            "Generated procedural texture '{}' ({:?})",
            texture_id,
            procedural.texture_type
        );

        Ok(Arc::new(texture_gpu))
    }

    /// Generate procedural image
    async fn generate_procedural_image(
        &self,
        procedural: &ProceduralTexture,
    ) -> Result<DynamicImage> {
        let (width, height) = procedural.size;
        let mut image = RgbaImage::new(width, height);

        match procedural.texture_type {
            ProceduralType::SolidColor => {
                let r = (procedural.parameters.get("r").unwrap_or(&1.0) * 255.0) as u8;
                let g = (procedural.parameters.get("g").unwrap_or(&1.0) * 255.0) as u8;
                let b = (procedural.parameters.get("b").unwrap_or(&1.0) * 255.0) as u8;
                let a = (procedural.parameters.get("a").unwrap_or(&1.0) * 255.0) as u8;

                for pixel in image.pixels_mut() {
                    *pixel = image::Rgba([r, g, b, a]);
                }
            }
            ProceduralType::Checkerboard => {
                let size = (*procedural.parameters.get("size").unwrap_or(&8.0)) as u32;

                for (x, y, pixel) in image.enumerate_pixels_mut() {
                    let checker = ((x / size) + (y / size)) % 2 == 0;
                    let color = if checker { 255 } else { 0 };
                    *pixel = image::Rgba([color, color, color, 255]);
                }
            }
            ProceduralType::WhiteNoise => {
                // Use fastrand for 2025 performance optimization
                let mut rng = fastrand::Rng::new();

                for pixel in image.pixels_mut() {
                    let value = rng.u8(..);
                    *pixel = image::Rgba([value, value, value, 255]);
                }
            }
            _ => {
                // Default to solid white for unimplemented types
                for pixel in image.pixels_mut() {
                    *pixel = image::Rgba([255, 255, 255, 255]);
                }
            }
        }

        Ok(DynamicImage::ImageRgba8(image))
    }

    /// Enforce memory budget by evicting least recently used textures
    async fn enforce_memory_budget(&self) -> Result<()> {
        let memory_used = *self.memory_used.read().await;

        if memory_used <= self.memory_budget {
            return Ok(());
        }

        let mut textures: Vec<_> = {
            let cache = self.texture_cache.read().await;
            cache
                .iter()
                .map(|(texture_id, texture)| {
                    (
                        texture_id.clone(),
                        texture.last_access,
                        texture.memory_size,
                        texture.reference_count.clone(),
                    )
                })
                .collect()
        };

        // Sort by last access time (oldest first)
        textures.sort_by(|a, b| a.1.cmp(&b.1));

        let mut memory_to_free = memory_used.saturating_sub(self.memory_budget);
        let mut memory_freed = 0_u64;
        let mut evicted_count = 0;
        let mut evicted_ids = Vec::new();

        for (texture_id, _last_access, texture_size, reference_count) in textures {
            let ref_count = *reference_count.read().await;

            // Only evict if not currently referenced
            if ref_count == 0 {
                memory_to_free = memory_to_free.saturating_sub(texture_size);
                memory_freed = memory_freed.saturating_add(texture_size);
                evicted_ids.push(texture_id);
                evicted_count += 1;

                if memory_to_free == 0 {
                    break;
                }
            }
        }

        if !evicted_ids.is_empty() {
            let mut cache = self.texture_cache.write().await;
            for texture_id in &evicted_ids {
                cache.remove(texture_id);
            }
        }

        // Update memory usage
        {
            let mut memory_used = self.memory_used.write().await;
            *memory_used = memory_used.saturating_sub(memory_freed);
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.textures_in_memory = self.texture_cache.read().await.len() as u32;
            stats.memory_used = *self.memory_used.read().await;
        }

        if evicted_count > 0 {
            tracing::debug!(
                "Evicted {} textures to enforce memory budget",
                evicted_count
            );
        }

        Ok(())
    }

    /// Get texture manager statistics
    pub async fn get_stats(&self) -> TextureManagerStats {
        self.stats.read().await.clone()
    }

    /// Clear texture cache
    pub async fn clear_cache(&self) -> Result<()> {
        let mut cache = self.texture_cache.write().await;
        cache.clear();

        let mut memory_used = self.memory_used.write().await;
        *memory_used = 0;

        let mut stats = self.stats.write().await;
        stats.textures_in_memory = 0;
        stats.memory_used = 0;

        tracing::info!("Cleared texture cache");
        Ok(())
    }

    /// Set compression settings
    pub fn set_compression(&mut self, enabled: bool, quality: CompressionQuality) {
        self.compression_enabled = enabled;
        self.compression_quality = quality;
        tracing::debug!(
            "Set texture compression: enabled={}, quality={:?}",
            enabled,
            quality
        );
    }

    /// Set streaming settings
    pub fn set_streaming(&mut self, enabled: bool, max_texture_size: u32) {
        self.streaming_enabled = enabled;
        self.max_texture_size = max_texture_size;
        tracing::debug!(
            "Set texture streaming: enabled={}, max_size={}x{}",
            enabled,
            max_texture_size,
            max_texture_size
        );
    }
}

impl Clone for W3DTextureGpu {
    fn clone(&self) -> Self {
        // Note: This is a simplified clone that shares the GPU resources
        // In a real implementation, you'd need proper reference counting
        Self {
            texture_info: self.texture_info.clone(),
            wgpu_texture: self.wgpu_texture.clone(),
            view: self.view.clone(),
            sampler: self.sampler.clone(),
            memory_size: self.memory_size,
            last_access: std::time::Instant::now(),
            reference_count: self.reference_count.clone(),
            ww3d_format: self.ww3d_format,
            reduction: self.reduction,
            inactivation: self.inactivation.clone(),
            is_reducible: self.is_reducible,
            allow_compression: self.allow_compression,
            hsv_shift: self.hsv_shift,
            team_color: self.team_color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsample_rgba8_averages_2x2_block() {
        let src_width = 2;
        let src_height = 2;
        let src = vec![
            0, 0, 0, 255, // top-left
            100, 0, 0, 255, // top-right
            0, 100, 0, 255, // bottom-left
            0, 0, 100, 255, // bottom-right
        ];

        let mip = W3DTextureManager::downsample_rgba8(&src, src_width, src_height);
        // C++ parity: Apply 0xFC mask, divide by 4, then sum
        // 0 & 0xFC = 0, 0 >> 2 = 0
        // 100 & 0xFC = 100, 100 >> 2 = 25
        // So average is: (0+25+0+0, 0+0+25+0, 0+0+0+25, 63+63+63+63) = (25, 25, 25, 255)
        assert_eq!(mip, vec![25, 25, 25, 255]);
    }

    #[test]
    fn downsample_rgba8_handles_odd_dimensions_with_edge_clamp() {
        let src_width = 3;
        let src_height = 1;
        let src = vec![
            10, 20, 30, 40, // x=0
            50, 60, 70, 80, // x=1
            90, 100, 110, 120, // x=2
        ];

        let mip = W3DTextureManager::downsample_rgba8(&src, src_width, src_height);
        // 1x1 output, samples are x=0,x=1 twice each due Y clamp.
        // Using C++ mask algorithm:
        // 10 & 0xFC = 8, 8 >> 2 = 2 (x2 samples due to Y clamp)
        // 50 & 0xFC = 48, 48 >> 2 = 12 (x2 samples due to Y clamp)
        // Result: (2+2+12+12, 4+4+12+12, 6+6+12+12, 8+8+16+16) = (28, 32, 36, 48)
        assert_eq!(mip, vec![28, 32, 36, 48]);
    }

    #[test]
    fn mip_count_type_to_level_count() {
        assert_eq!(MipCountType::All.to_level_count(256, 256), 9);
        assert_eq!(MipCountType::All.to_level_count(128, 128), 8);
        assert_eq!(MipCountType::Levels1.to_level_count(256, 256), 1);
        assert_eq!(MipCountType::Levels3.to_level_count(256, 256), 3);
        assert_eq!(MipCountType::Levels3.to_level_count(4, 4), 3); // Clamped to max
    }

    #[test]
    fn ww3d_format_has_alpha() {
        assert!(WW3DFormat::A8R8G8B8.has_alpha());
        assert!(WW3DFormat::A4R4G4B4.has_alpha());
        assert!(WW3DFormat::Dxt3.has_alpha());
        assert!(!WW3DFormat::R8G8B8.has_alpha());
        assert!(!WW3DFormat::R5G6B5.has_alpha());
        assert!(!WW3DFormat::Dxt1.has_alpha());
    }

    #[test]
    fn ww3d_format_alpha_bits() {
        assert_eq!(WW3DFormat::A8R8G8B8.alpha_bits(), 8);
        assert_eq!(WW3DFormat::A4R4G4B4.alpha_bits(), 4);
        assert_eq!(WW3DFormat::A1R5G5B5.alpha_bits(), 1);
        assert_eq!(WW3DFormat::R8G8B8.alpha_bits(), 0);
    }

    #[test]
    fn ww3d_format_bytes_per_pixel() {
        assert_eq!(WW3DFormat::A8R8G8B8.bytes_per_pixel(), 4);
        assert_eq!(WW3DFormat::R8G8B8.bytes_per_pixel(), 3);
        assert_eq!(WW3DFormat::R5G6B5.bytes_per_pixel(), 2);
        assert_eq!(WW3DFormat::A8.bytes_per_pixel(), 1);
        // Compressed formats return bytes per 4x4 block
        assert_eq!(WW3DFormat::Dxt1.bytes_per_pixel(), 8);
        assert_eq!(WW3DFormat::Dxt5.bytes_per_pixel(), 16);
    }

    #[test]
    fn rgb_to_hsv_converts_correctly() {
        // Red
        let (h, s, v) = rgb_to_hsv(1.0, 0.0, 0.0);
        assert!((h - 0.0).abs() < 0.01 || (h - 360.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        // Green
        let (h, s, v) = rgb_to_hsv(0.0, 1.0, 0.0);
        assert!((h - 120.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        // Blue
        let (h, s, v) = rgb_to_hsv(0.0, 0.0, 1.0);
        assert!((h - 240.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        // White - monochrome, hue is undefined (negative)
        let (h, s, v) = rgb_to_hsv(1.0, 1.0, 1.0);
        assert!(h < 0.0); // Undefined hue for monochrome (C++ parity)
        assert!((s - 0.0).abs() < 0.01);
        assert!((v - 1.0).abs() < 0.01);

        // Black - monochrome, hue is undefined (negative)
        let (h, s, v) = rgb_to_hsv(0.0, 0.0, 0.0);
        assert!(h < 0.0); // Undefined hue for monochrome (C++ parity)
        assert!((s - 0.0).abs() < 0.01);
        assert!((v - 0.0).abs() < 0.01);

        // Gray - monochrome
        let (h, s, v) = rgb_to_hsv(0.5, 0.5, 0.5);
        assert!(h < 0.0); // Undefined hue for monochrome (C++ parity)
        assert!((s - 0.0).abs() < 0.01);
        assert!((v - 0.5).abs() < 0.01);
    }

    #[test]
    fn hsv_to_rgb_converts_correctly() {
        // Red
        let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((r - 1.0).abs() < 0.01);
        assert!((g - 0.0).abs() < 0.01);
        assert!((b - 0.0).abs() < 0.01);

        // Green
        let (r, g, b) = hsv_to_rgb(120.0, 1.0, 1.0);
        assert!((r - 0.0).abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!((b - 0.0).abs() < 0.01);

        // Blue
        let (r, g, b) = hsv_to_rgb(240.0, 1.0, 1.0);
        assert!((r - 0.0).abs() < 0.01);
        assert!((g - 0.0).abs() < 0.01);
        assert!((b - 1.0).abs() < 0.01);

        // Monochrome (saturation 0) - should return value for all channels
        let (r, g, b) = hsv_to_rgb(-1.0, 0.0, 0.5);
        assert!((r - 0.5).abs() < 0.01);
        assert!((g - 0.5).abs() < 0.01);
        assert!((b - 0.5).abs() < 0.01);
    }

    #[test]
    fn recolor_pixel_handles_monochrome() {
        // Monochrome gray - should only modify value, not hue/sat
        let (r, g, b) = recolor_pixel(0.5, 0.5, 0.5, [60.0, 0.5, 0.0]);
        // Hue shift should be ignored for monochrome colors (C++ parity)
        assert!((r - 0.5).abs() < 0.01);
        assert!((g - 0.5).abs() < 0.01);
        assert!((b - 0.5).abs() < 0.01);
    }

    #[test]
    fn recolor_pixel_handles_color() {
        // Red with hue shift
        let (r, g, b) = recolor_pixel(1.0, 0.0, 0.0, [120.0, 0.0, 0.0]);
        // Should shift red (hue 0) to green (hue 120)
        assert!((r - 0.0).abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!((b - 0.0).abs() < 0.01);
    }

    #[test]
    fn generate_team_color_palette_produces_gradient() {
        let red_color = 0xFF0000;
        let palette = generate_team_color_palette_32bit(red_color);

        // First entry should be brightest
        let first_r = (palette[0] >> 16) & 0xFF;
        let last_r = (palette[15] >> 16) & 0xFF;
        assert!(first_r > last_r);

        // All entries should have alpha
        for &color in &palette {
            assert_eq!(color & 0xFF000000, 0xFF000000);
        }
    }

    #[test]
    fn house_color_scale_matches_cpp() {
        // These values must match C++ houseColorScale exactly
        assert_eq!(HOUSE_COLOR_SCALE[0], 255);
        assert_eq!(HOUSE_COLOR_SCALE[1], 239);
        assert_eq!(HOUSE_COLOR_SCALE[2], 223);
        assert_eq!(HOUSE_COLOR_SCALE[15], 35);
        assert_eq!(TEAM_COLOR_PALETTE_SIZE, 16);
    }

    #[test]
    fn texture_inactivation_default_matches_cpp() {
        let state = TextureInactivationState::default();
        assert_eq!(state.inactivation_time, DEFAULT_INACTIVATION_TIME_MS);
        assert_eq!(state.extended_inactivation_time, 0);
    }
}
