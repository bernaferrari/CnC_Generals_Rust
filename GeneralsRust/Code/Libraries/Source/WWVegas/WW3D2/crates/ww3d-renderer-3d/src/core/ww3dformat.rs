//! WW3D texture format helpers.
//!
//! The original renderer exposes a large enumeration of texture
//! encodings via `ww3dformat.h`.  The Rust renderer needs the same
//! information so that asset loading, caches, and WGPU translation all
//! agree on the semantics of a texture.  This module provides the
//! unified definition together with a thin `FormatManager` that mirrors
//! the behaviour of the old DirectX8 capability queries.

use crate::config;
use std::collections::HashSet;
use std::fmt;

/// Legacy WW3D surface formats (abridged from `ww3dformat.h`).
#[cfg_attr(
    feature = "serde_support",
    derive(serde::Serialize, serde::Deserialize)
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WW3DFormat {
    Unknown,
    R8G8B8,
    A8R8G8B8,
    X8R8G8B8,
    R8G8B8A8,
    R5G6B5,
    X1R5G5B5,
    A1R5G5B5,
    A4R4G4B4,
    R3G3B2,
    A8,
    A8R3G3B2,
    X4R4G4B4,
    A8P8,
    P8,
    L8,
    A8L8,
    A4L4,
    U8V8,
    L6V5U5,
    X8L8V8U8,
    DXT1,
    DXT2,
    DXT3,
    DXT4,
    DXT5,
    D16,
    D24S8,
    D32,
    D16Lockable,
}

impl WW3DFormat {
    /// Returns `true` if the format carries an alpha channel.
    pub fn has_alpha(self) -> bool {
        matches!(
            self,
            WW3DFormat::A8R8G8B8
                | WW3DFormat::R8G8B8A8
                | WW3DFormat::A1R5G5B5
                | WW3DFormat::A4R4G4B4
                | WW3DFormat::A8
                | WW3DFormat::A8R3G3B2
                | WW3DFormat::A8P8
                | WW3DFormat::A8L8
                | WW3DFormat::A4L4
                | WW3DFormat::DXT2
                | WW3DFormat::DXT3
                | WW3DFormat::DXT4
                | WW3DFormat::DXT5
        )
    }

    /// Returns the number of alpha bits contained in the format.
    pub fn alpha_bits(self) -> u32 {
        match self {
            WW3DFormat::A8R8G8B8
            | WW3DFormat::R8G8B8A8
            | WW3DFormat::A8
            | WW3DFormat::A8R3G3B2
            | WW3DFormat::A8P8
            | WW3DFormat::A8L8 => 8,
            WW3DFormat::A4R4G4B4
            | WW3DFormat::A4L4
            | WW3DFormat::DXT3
            | WW3DFormat::DXT4
            | WW3DFormat::DXT5 => 4,
            WW3DFormat::A1R5G5B5 | WW3DFormat::DXT2 => 1,
            _ => 0,
        }
    }

    /// Returns `true` for block-compressed (DXT) formats.
    pub fn is_block_compressed(self) -> bool {
        matches!(
            self,
            WW3DFormat::DXT1
                | WW3DFormat::DXT2
                | WW3DFormat::DXT3
                | WW3DFormat::DXT4
                | WW3DFormat::DXT5
        )
    }

    /// Bytes per texel (or bytes per 4×4 block for compressed formats).
    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            WW3DFormat::Unknown => 0,
            WW3DFormat::R8G8B8 => 3,
            WW3DFormat::A8R8G8B8 | WW3DFormat::X8R8G8B8 | WW3DFormat::R8G8B8A8 => 4,
            WW3DFormat::R5G6B5
            | WW3DFormat::X1R5G5B5
            | WW3DFormat::A1R5G5B5
            | WW3DFormat::A4R4G4B4
            | WW3DFormat::X4R4G4B4
            | WW3DFormat::A8R3G3B2
            | WW3DFormat::A8L8
            | WW3DFormat::U8V8
            | WW3DFormat::L6V5U5 => 2,
            WW3DFormat::R3G3B2
            | WW3DFormat::A8
            | WW3DFormat::A8P8
            | WW3DFormat::P8
            | WW3DFormat::L8
            | WW3DFormat::A4L4 => 1,
            WW3DFormat::X8L8V8U8 => 4,
            WW3DFormat::DXT1 => 8,
            WW3DFormat::DXT2 | WW3DFormat::DXT3 | WW3DFormat::DXT4 | WW3DFormat::DXT5 => 16,
            // Depth formats default to 0 here; GPU-side calculation should be used.
            WW3DFormat::D16 | WW3DFormat::D24S8 | WW3DFormat::D32 | WW3DFormat::D16Lockable => 0,
        }
    }

    /// Convert to a `wgpu::TextureFormat` if supported.
    pub fn to_wgpu_format(self) -> Option<wgpu::TextureFormat> {
        use wgpu::TextureFormat as Tf;
        Some(match self {
            WW3DFormat::Unknown => return None,
            WW3DFormat::R8G8B8 => Tf::Rgba8Unorm,
            WW3DFormat::A8R8G8B8 | WW3DFormat::R8G8B8A8 => Tf::Rgba8UnormSrgb,
            WW3DFormat::X8R8G8B8 => Tf::Rgba8Unorm,
            WW3DFormat::R5G6B5
            | WW3DFormat::X1R5G5B5
            | WW3DFormat::A1R5G5B5
            | WW3DFormat::A4R4G4B4
            | WW3DFormat::R3G3B2
            | WW3DFormat::A8R3G3B2
            | WW3DFormat::X4R4G4B4 => Tf::Rgba8Unorm,
            WW3DFormat::A8 | WW3DFormat::A8P8 | WW3DFormat::P8 | WW3DFormat::L8 => Tf::R8Unorm,
            WW3DFormat::A8L8
            | WW3DFormat::A4L4
            | WW3DFormat::U8V8
            | WW3DFormat::L6V5U5
            | WW3DFormat::X8L8V8U8 => Tf::Rg8Unorm,
            WW3DFormat::DXT1 => Tf::Bc1RgbaUnorm,
            WW3DFormat::DXT2 | WW3DFormat::DXT3 => Tf::Bc2RgbaUnorm,
            WW3DFormat::DXT4 | WW3DFormat::DXT5 => Tf::Bc3RgbaUnorm,
            WW3DFormat::D16 | WW3DFormat::D16Lockable => Tf::Depth16Unorm,
            WW3DFormat::D24S8 => Tf::Depth24PlusStencil8,
            WW3DFormat::D32 => Tf::Depth32Float,
        })
    }

    /// Convert from a `wgpu::TextureFormat` when the mapping exists.
    pub fn from_wgpu_format(format: wgpu::TextureFormat) -> Option<Self> {
        use wgpu::TextureFormat as Tf;
        match format {
            Tf::Rgba8Unorm | Tf::Rgba8UnormSrgb => Some(WW3DFormat::A8R8G8B8),
            Tf::Bgra8Unorm | Tf::Bgra8UnormSrgb => Some(WW3DFormat::A8R8G8B8),
            Tf::Rgba8Snorm | Tf::Rgba8Uint | Tf::Rgba8Sint => Some(WW3DFormat::A8R8G8B8),
            Tf::Rg8Unorm | Tf::Rg8Snorm | Tf::Rg8Uint | Tf::Rg8Sint => Some(WW3DFormat::A8L8),
            Tf::R8Unorm | Tf::R8Snorm | Tf::R8Uint | Tf::R8Sint => Some(WW3DFormat::L8),
            Tf::Bc1RgbaUnorm | Tf::Bc1RgbaUnormSrgb => Some(WW3DFormat::DXT1),
            Tf::Bc2RgbaUnorm | Tf::Bc2RgbaUnormSrgb => Some(WW3DFormat::DXT3),
            Tf::Bc3RgbaUnorm | Tf::Bc3RgbaUnormSrgb => Some(WW3DFormat::DXT5),
            Tf::Depth16Unorm => Some(WW3DFormat::D16),
            Tf::Depth24PlusStencil8 => Some(WW3DFormat::D24S8),
            Tf::Depth32Float => Some(WW3DFormat::D32),
            _ => None,
        }
    }
}

impl fmt::Display for WW3DFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Texture format manager roughly mirroring the behaviour of the
/// original DirectX8 implementation.  It understands adapter
/// capabilities, compression support, and engine configuration flags.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatDecision {
    pub source_format: WW3DFormat,
    pub preferred_format: WW3DFormat,
    pub format: WW3DFormat,
    pub requires_decompression: bool,
}

#[derive(Clone, Debug)]
pub struct FormatManager {
    compression_supported: bool,
    prefer_16_bit: bool,
    force_srgb: bool,
    max_color_bits: u8,
    supported_formats: HashSet<WW3DFormat>,
}

impl FormatManager {
    pub fn from_device(device: &wgpu::Device) -> Self {
        let features = device.features();
        let cfg = config::get();
        let compression_supported = features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let supported_formats = Self::probe_supported_formats(device);
        Self {
            compression_supported,
            prefer_16_bit: cfg.prefer_16bit_textures,
            force_srgb: cfg.force_srgb_textures,
            max_color_bits: if cfg.prefer_16bit_textures { 16 } else { 32 },
            supported_formats,
        }
    }

    pub fn default_cpu() -> Self {
        let cfg = config::get();
        let mut supported_formats = HashSet::new();
        supported_formats.insert(WW3DFormat::A8R8G8B8);
        supported_formats.insert(WW3DFormat::R8G8B8A8);
        supported_formats.insert(WW3DFormat::X8R8G8B8);
        supported_formats.insert(WW3DFormat::R8G8B8);
        supported_formats.insert(WW3DFormat::A4R4G4B4);
        supported_formats.insert(WW3DFormat::A1R5G5B5);
        supported_formats.insert(WW3DFormat::R5G6B5);
        supported_formats.insert(WW3DFormat::A8);
        supported_formats.insert(WW3DFormat::L8);
        supported_formats.insert(WW3DFormat::DXT1);
        supported_formats.insert(WW3DFormat::DXT2);
        supported_formats.insert(WW3DFormat::DXT3);
        supported_formats.insert(WW3DFormat::DXT4);
        supported_formats.insert(WW3DFormat::DXT5);
        Self {
            compression_supported: true,
            prefer_16_bit: cfg.prefer_16bit_textures,
            force_srgb: cfg.force_srgb_textures,
            max_color_bits: if cfg.prefer_16bit_textures { 16 } else { 32 },
            supported_formats,
        }
    }

    pub fn with_max_color_bits(mut self, bits: u8) -> Self {
        self.max_color_bits = bits.clamp(8, 32);
        self
    }

    pub fn decide(
        &self,
        source_format: WW3DFormat,
        preferred_override: Option<WW3DFormat>,
        allow_compression: bool,
    ) -> FormatDecision {
        let mut preferred = preferred_override.unwrap_or(source_format);
        if preferred == WW3DFormat::Unknown {
            preferred = source_format;
        }

        let mut format = preferred;
        let mut requires_decompression = false;

        if format.is_block_compressed() {
            if allow_compression && self.compression_supported {
                if !self.supports(format) {
                    if matches!(format, WW3DFormat::DXT1)
                        && !self.supports(WW3DFormat::DXT1)
                        && self.supports(WW3DFormat::DXT2)
                    {
                        format = WW3DFormat::DXT2;
                    } else {
                        format = if matches!(format, WW3DFormat::DXT1) {
                            WW3DFormat::X8R8G8B8
                        } else {
                            WW3DFormat::A8R8G8B8
                        };
                        requires_decompression = true;
                    }
                }
            } else {
                requires_decompression = true;
                format = if matches!(format, WW3DFormat::DXT1) {
                    WW3DFormat::X8R8G8B8
                } else {
                    WW3DFormat::A8R8G8B8
                };
            }
        }

        if matches!(format, WW3DFormat::R8G8B8) {
            format = WW3DFormat::X8R8G8B8;
        } else if matches!(format, WW3DFormat::R8G8B8A8) {
            format = WW3DFormat::A8R8G8B8;
        }

        if (self.prefer_16_bit || self.max_color_bits <= 16) && !format.is_block_compressed() {
            if let Some(candidate) = self.promote_to_16_bit(format) {
                if self.supports(candidate) {
                    format = candidate;
                }
            }
        }

        if !self.supports(format) {
            if let Some(candidate) = self
                .fallback_chain(format, preferred, source_format)
                .into_iter()
                .find(|candidate| self.supports(*candidate))
            {
                if source_format.is_block_compressed() && !candidate.is_block_compressed() {
                    requires_decompression = true;
                }
                format = candidate;
            } else if self.supports(WW3DFormat::A8R8G8B8) {
                if source_format.is_block_compressed() {
                    requires_decompression = true;
                }
                format = WW3DFormat::A8R8G8B8;
            } else if let Some(candidate) = self.supported_formats.iter().copied().next() {
                format = candidate;
            } else {
                format = WW3DFormat::A8R8G8B8;
            }
        }

        if self.force_srgb && !format.is_block_compressed() {
            if let Some(candidate) = match format {
                WW3DFormat::X8R8G8B8 | WW3DFormat::R5G6B5 => Some(WW3DFormat::A8R8G8B8),
                _ => None,
            } {
                if self.supports(candidate) {
                    format = candidate;
                }
            }
        }

        if source_format.is_block_compressed() && !format.is_block_compressed() {
            requires_decompression = true;
        }

        FormatDecision {
            source_format,
            preferred_format: preferred,
            format,
            requires_decompression,
        }
    }

    pub fn compression_supported(&self) -> bool {
        self.compression_supported
    }

    pub fn force_srgb(&self) -> bool {
        self.force_srgb
    }

    fn supports(&self, format: WW3DFormat) -> bool {
        self.supported_formats.contains(&format)
    }

    fn promote_to_16_bit(&self, format: WW3DFormat) -> Option<WW3DFormat> {
        match format {
            WW3DFormat::A8R8G8B8 | WW3DFormat::R8G8B8A8 => Some(WW3DFormat::A4R4G4B4),
            WW3DFormat::X8R8G8B8 | WW3DFormat::R8G8B8 => Some(WW3DFormat::R5G6B5),
            _ => None,
        }
    }

    fn fallback_chain(
        &self,
        initial: WW3DFormat,
        preferred: WW3DFormat,
        source: WW3DFormat,
    ) -> Vec<WW3DFormat> {
        use std::collections::HashSet as StdHashSet;

        let mut order = Vec::new();
        let mut seen = StdHashSet::new();

        let mut push_unique = |fmt: WW3DFormat| {
            if fmt == WW3DFormat::Unknown {
                return;
            }
            if seen.insert(fmt) {
                order.push(fmt);
            }
        };

        push_unique(initial);
        push_unique(preferred);
        push_unique(source);

        let prefer_alpha = preferred.has_alpha() || source.has_alpha();
        let compressed_source = source.is_block_compressed();

        if prefer_alpha {
            push_unique(WW3DFormat::A8R8G8B8);
            push_unique(WW3DFormat::A4R4G4B4);
            push_unique(WW3DFormat::A1R5G5B5);
            push_unique(WW3DFormat::X8R8G8B8);
            push_unique(WW3DFormat::R5G6B5);
        } else {
            push_unique(WW3DFormat::X8R8G8B8);
            push_unique(WW3DFormat::R5G6B5);
            push_unique(WW3DFormat::A8R8G8B8);
            push_unique(WW3DFormat::A4R4G4B4);
        }

        if compressed_source {
            push_unique(WW3DFormat::X8R8G8B8);
            push_unique(WW3DFormat::A8R8G8B8);
        }

        push_unique(WW3DFormat::L8);
        push_unique(WW3DFormat::A8);

        order
    }

    fn probe_supported_formats(device: &wgpu::Device) -> HashSet<WW3DFormat> {
        let mut supported = HashSet::new();
        let features = device.features();

        let uncompressed = [
            WW3DFormat::A8R8G8B8,
            WW3DFormat::R8G8B8A8,
            WW3DFormat::X8R8G8B8,
            WW3DFormat::R8G8B8,
            WW3DFormat::A4R4G4B4,
            WW3DFormat::A1R5G5B5,
            WW3DFormat::R5G6B5,
            WW3DFormat::A8,
            WW3DFormat::L8,
        ];
        supported.extend(uncompressed);

        if features.contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            supported.extend([
                WW3DFormat::DXT1,
                WW3DFormat::DXT2,
                WW3DFormat::DXT3,
                WW3DFormat::DXT4,
                WW3DFormat::DXT5,
            ]);
        }

        supported
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    fn make_manager(
        compression_supported: bool,
        prefer_16_bit: bool,
        force_srgb: bool,
        max_color_bits: u8,
        supported: &[WW3DFormat],
    ) -> FormatManager {
        let mut set = HashSet::new();
        set.extend(supported.iter().copied());
        FormatManager {
            compression_supported,
            prefer_16_bit,
            force_srgb,
            max_color_bits,
            supported_formats: set,
        }
    }

    #[test]
    fn chooses_uncompressed_when_bc_disallowed() {
        let manager = make_manager(
            false,
            false,
            false,
            32,
            &[
                WW3DFormat::A8R8G8B8,
                WW3DFormat::X8R8G8B8,
                WW3DFormat::A4R4G4B4,
                WW3DFormat::R5G6B5,
            ],
        );

        let decision = manager.decide(WW3DFormat::DXT1, None, false);
        assert_eq!(decision.format, WW3DFormat::X8R8G8B8);
        assert!(decision.requires_decompression);
    }

    #[test]
    fn prefers_16_bit_when_available() {
        let manager = make_manager(
            true,
            true,
            false,
            16,
            &[
                WW3DFormat::A4R4G4B4,
                WW3DFormat::R5G6B5,
                WW3DFormat::A8R8G8B8,
                WW3DFormat::DXT1,
            ],
        );

        let decision = manager.decide(WW3DFormat::A8R8G8B8, None, true);
        assert_eq!(decision.format, WW3DFormat::A4R4G4B4);
        assert!(!decision.requires_decompression);
    }

    #[test]
    fn falls_back_to_supported_format() {
        let manager = make_manager(
            true,
            false,
            false,
            32,
            &[
                WW3DFormat::A8R8G8B8,
                WW3DFormat::A4R4G4B4,
                WW3DFormat::R5G6B5,
            ],
        );

        let decision = manager.decide(WW3DFormat::DXT5, None, true);
        assert_eq!(decision.format, WW3DFormat::A8R8G8B8);
        assert!(decision.requires_decompression);
    }

    #[test]
    fn upgrades_dxt1_to_dxt2_when_only_dxt2_supported() {
        let manager = make_manager(
            true,
            false,
            false,
            32,
            &[WW3DFormat::A8R8G8B8, WW3DFormat::DXT2],
        );

        let decision = manager.decide(WW3DFormat::DXT1, None, true);
        assert_eq!(decision.format, WW3DFormat::DXT2);
        assert!(!decision.requires_decompression);
    }

    #[test]
    fn forces_srgb_when_requested() {
        let manager = make_manager(
            true,
            false,
            true,
            32,
            &[
                WW3DFormat::A8R8G8B8,
                WW3DFormat::X8R8G8B8,
                WW3DFormat::R5G6B5,
            ],
        );

        let decision = manager.decide(WW3DFormat::X8R8G8B8, None, true);
        assert_eq!(decision.format, WW3DFormat::A8R8G8B8);
        assert!(!decision.requires_decompression);
    }
}
