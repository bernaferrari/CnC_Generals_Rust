/***********************************************************************************************
 ***              C O N F I D E N T I A L  ---  W E S T W O O D  S T U D I O S               ***
 ***********************************************************************************************
 *                                                                                             *
 *                 Project Name : WW3D                                                         *
 *                                                                                             *
 *                     $Archive:: /Commando/Code/ww3d2/texture.h                              $*
 *                                                                                             *
 *                  $Org Author:: Jani_p                                                      $*
 *                                                                                             *
 *                       Author : Kenny Mitchell                                               *
 *                                                                                             *
 *                     $Modtime:: 08/05/02 1:27p                                              $*
 *                                                                                             *
 *                    $Revision:: 46                                                          $*
 *                                                                                             *
 * 05/16/02 KM Base texture class to abstract major texture types, e.g. 3d, z, cube, etc.
 * 06/27/02 KM Texture class abstraction																			*
 * 08/05/02 KM Texture class redesign (revisited)
 *---------------------------------------------------------------------------------------------*
 * Functions:                                                                                  *
 * - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - */

use crate::core::error::{Error, Result};
use crate::core::ww3dformat::FormatDecision;
use crate::core::WW3DFormat;
use crate::material_system::TextureStageSettings;
use crate::rendering::texture_decode::{decode_texture_file, TextureData, TextureDataKind};
use crate::rendering::texture_metrics;
use crate::rendering::texture_quality;
use crate::texture_system::{SurfaceClass, TextureFormat};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureUsagePolicy {
    pub allow_compression: bool,
    pub allow_reduction: bool,
    pub requested_mip_levels: Option<u32>,
}

impl TextureUsagePolicy {
    pub fn new(
        allow_compression: bool,
        allow_reduction: bool,
        requested_mip_levels: Option<u32>,
    ) -> Self {
        Self {
            allow_compression,
            allow_reduction,
            requested_mip_levels,
        }
    }
}

impl Default for TextureUsagePolicy {
    fn default() -> Self {
        Self::new(true, true, None)
    }
}

use std::borrow::Cow;
use std::sync::Arc;
use wgpu::{
    Device, Extent3d, Origin3d, Queue, TexelCopyBufferLayout, TexelCopyTextureInfo,
    Texture as WgpuTexture, TextureDescriptor, TextureDimension, TextureUsages,
    TextureViewDescriptor,
};

/// Mip count type with special constants
pub struct MipCountType;

impl MipCountType {
    pub const AUTO_GENERATE: u32 = 0xFFFFFFFF;
}
pub type TextureBase = TextureBaseClass; // Alias for compatibility

// Placeholder for reference counting - will be replaced with Arc
pub struct RefCountClass;

impl RefCountClass {
    pub fn add_ref(&self) {}
    pub fn release_ref(&self) {}
}

/// CPU-side metadata for each mip level stored in `system_memory`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemMipLevel {
    pub offset: usize,
    pub size: usize,
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub slice_stride: usize,
}

fn align_to(value: usize, alignment: usize) -> usize {
    ((value + alignment - 1) / alignment) * alignment
}

fn wgpu_bytes_per_pixel(format: wgpu::TextureFormat) -> Option<usize> {
    use wgpu::TextureFormat::*;
    match format {
        Rgba8Unorm | Rgba8UnormSrgb | Rgba8Snorm | Rgba8Uint | Rgba8Sint | Bgra8Unorm
        | Bgra8UnormSrgb => Some(4),
        Rg8Unorm | Rg8Snorm | Rg8Uint | Rg8Sint => Some(2),
        R8Unorm | R8Snorm | R8Uint | R8Sint => Some(1),
        Rgba16Float | Rgba16Uint | Rgba16Sint => Some(8),
        Rg16Float | Rg16Uint | Rg16Sint => Some(4),
        R16Float | R16Uint | R16Sint => Some(2),
        _ => None,
    }
}

fn align_rows(data: &[u8], row_bytes: usize, aligned_row_bytes: usize, rows: usize) -> Vec<u8> {
    let mut owned = vec![0u8; aligned_row_bytes * rows];
    for row in 0..rows {
        let src = row * row_bytes;
        let dst = row * aligned_row_bytes;
        owned[dst..dst + row_bytes].copy_from_slice(&data[src..src + row_bytes]);
    }
    owned
}

fn prepare_uncompressed_slice<'a>(
    ww3d_format: WW3DFormat,
    wgpu_format: wgpu::TextureFormat,
    raw: &'a [u8],
    width: u32,
    height: u32,
) -> Result<Cow<'a, [u8]>> {
    let wgpu_bpp = wgpu_bytes_per_pixel(wgpu_format).ok_or_else(|| {
        Error::InvalidData(format!("Unsupported WGPU texture format {:?}", wgpu_format))
    })?;
    let ww_bpp = ww3d_format.bytes_per_pixel().max(1) as usize;
    if ww_bpp == wgpu_bpp {
        return Ok(Cow::Borrowed(raw));
    }

    let expected = (width.max(1) as usize) * (height.max(1) as usize) * ww_bpp;
    if raw.len() != expected {
        return Err(Error::InvalidData(format!(
            "Texture slice size mismatch: expected {} bytes, found {}",
            expected,
            raw.len()
        )));
    }

    let converted = match (ww3d_format, wgpu_bpp) {
        (WW3DFormat::R5G6B5, 4) => convert_r5g6b5_to_rgba(raw),
        (WW3DFormat::A4R4G4B4, 4) => convert_a4r4g4b4_to_rgba(raw),
        (WW3DFormat::A1R5G5B5, 4) => convert_a1r5g5b5_to_rgba(raw),
        _ => {
            return Err(Error::InvalidData(format!(
                "Unsupported conversion from {:?} to {:?}",
                ww3d_format, wgpu_format
            )));
        }
    };

    Ok(Cow::Owned(converted))
}

fn convert_r5g6b5_to_rgba(raw: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((raw.len() / 2) * 4);
    for chunk in raw.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let r = ((value >> 11) & 0x1F) as u8;
        let g = ((value >> 5) & 0x3F) as u8;
        let b = (value & 0x1F) as u8;
        output.extend_from_slice(&[scale_5_to_8(r), scale_6_to_8(g), scale_5_to_8(b), 255]);
    }
    output
}

fn convert_a4r4g4b4_to_rgba(raw: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((raw.len() / 2) * 4);
    for chunk in raw.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let a = ((value >> 12) & 0xF) as u8;
        let r = ((value >> 8) & 0xF) as u8;
        let g = ((value >> 4) & 0xF) as u8;
        let b = (value & 0xF) as u8;
        output.extend_from_slice(&[
            scale_4_to_8(r),
            scale_4_to_8(g),
            scale_4_to_8(b),
            scale_4_to_8(a),
        ]);
    }
    output
}

fn convert_a1r5g5b5_to_rgba(raw: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity((raw.len() / 2) * 4);
    for chunk in raw.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        let a = ((value >> 15) & 0x1) as u8;
        let r = ((value >> 10) & 0x1F) as u8;
        let g = ((value >> 5) & 0x1F) as u8;
        let b = (value & 0x1F) as u8;
        output.extend_from_slice(&[
            scale_5_to_8(r),
            scale_5_to_8(g),
            scale_5_to_8(b),
            scale_1_to_8(a),
        ]);
    }
    output
}

fn scale_1_to_8(value: u8) -> u8 {
    if value == 0 {
        0
    } else {
        255
    }
}

fn scale_4_to_8(value: u8) -> u8 {
    (value << 4) | value
}

fn scale_5_to_8(value: u8) -> u8 {
    ((value as u16 * 255 + 15) / 31) as u8
}

fn scale_6_to_8(value: u8) -> u8 {
    ((value as u16 * 255 + 31) / 63) as u8
}

/// Pool type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoolType {
    Default = 0,
    Managed,
    SystemMem,
}

/// Texture asset type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TexAssetType {
    Regular,
    Cubemap,
    Volume,
}

/// Base texture class
#[derive(Debug)]
pub struct TextureBaseClass {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mip_level_count: u32,
    pub pool: PoolType,
    pub asset_type: TexAssetType,
    pub name: String,
    pub full_path: String,
    pub ww3d_format: WW3DFormat,
    pub format: wgpu::TextureFormat,
    pub system_memory: Vec<u8>,
    pub system_mip_levels: Vec<SystemMipLevel>,
    pub format_history: Option<FormatDecision>,
    pub allow_compression: bool,
    pub allow_reduction: bool,
    pub requested_mip_levels: Option<u32>,
    pub stage_settings: TextureStageSettings,

    // WGPU texture handle
    pub wgpu_texture: Option<Arc<WgpuTexture>>,
    pub texture_view: Option<wgpu::TextureView>,
    pub sampler: Option<wgpu::Sampler>,
}

impl Clone for TextureBaseClass {
    fn clone(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            depth: self.depth,
            mip_level_count: self.mip_level_count,
            pool: self.pool,
            asset_type: self.asset_type,
            name: self.name.clone(),
            full_path: self.full_path.clone(),
            ww3d_format: self.ww3d_format,
            format: self.format,
            system_memory: self.system_memory.clone(),
            system_mip_levels: self.system_mip_levels.clone(),
            format_history: self.format_history.clone(),
            allow_compression: self.allow_compression,
            allow_reduction: self.allow_reduction,
            requested_mip_levels: self.requested_mip_levels,
            stage_settings: self.stage_settings,
            // Arc can be cloned, but we set texture_view and sampler to None
            // since they don't implement Clone and need to be recreated
            wgpu_texture: self.wgpu_texture.clone(),
            texture_view: None, // TextureView doesn't implement Clone
            sampler: None,      // Sampler doesn't implement Clone
        }
    }
}

impl PartialEq for TextureBaseClass {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width &&
        self.height == other.height &&
        self.depth == other.depth &&
        self.mip_level_count == other.mip_level_count &&
        self.pool == other.pool &&
        self.asset_type == other.asset_type &&
        self.name == other.name &&
        self.full_path == other.full_path &&
        self.ww3d_format == other.ww3d_format &&
        self.format == other.format &&
        self.system_memory == other.system_memory &&
        self.system_mip_levels == other.system_mip_levels &&
        self.format_history == other.format_history &&
        self.allow_compression == other.allow_compression &&
        self.allow_reduction == other.allow_reduction &&
        self.requested_mip_levels == other.requested_mip_levels &&
        self.stage_settings == other.stage_settings &&
        // For Arc comparison, use pointer equality
        match (&self.wgpu_texture, &other.wgpu_texture) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
        // Skip texture_view and sampler comparison as they don't implement PartialEq
        // and are typically recreated when needed
    }
}

impl TextureBaseClass {
    pub fn init_texture_resources(&mut self, texture: WgpuTexture) {
        let arc = Arc::new(texture);
        let view_desc = match self.asset_type {
            TexAssetType::Cubemap => TextureViewDescriptor {
                label: Some("WW3D Texture View"),
                dimension: Some(wgpu::TextureViewDimension::Cube),
                base_array_layer: 0,
                array_layer_count: Some(6),
                ..Default::default()
            },
            TexAssetType::Volume => TextureViewDescriptor {
                label: Some("WW3D Texture View"),
                dimension: Some(wgpu::TextureViewDimension::D3),
                ..Default::default()
            },
            TexAssetType::Regular => TextureViewDescriptor {
                label: Some("WW3D Texture View"),
                ..Default::default()
            },
        };

        self.texture_view = Some(arc.create_view(&view_desc));
        self.wgpu_texture = Some(arc);
        self.sampler = None;
    }
}

impl TextureBaseClass {
    /// Create new texture base
    pub fn new(
        width: u32,
        height: u32,
        mip_level_count: u32,
        pool: PoolType,
        asset_type: TexAssetType,
    ) -> Self {
        Self {
            width,
            height,
            depth: 1,
            mip_level_count,
            pool,
            asset_type,
            name: String::new(),
            full_path: String::new(),
            ww3d_format: WW3DFormat::Unknown,
            format: wgpu::TextureFormat::Rgba8Unorm,
            system_memory: Vec::new(),
            system_mip_levels: Vec::new(),
            format_history: None,
            allow_compression: true,
            allow_reduction: true,
            requested_mip_levels: None,
            stage_settings: TextureStageSettings::default(),
            wgpu_texture: None,
            texture_view: None,
            sampler: None,
        }
    }

    /// Port of WW3D Get_Valid_Texture_Format semantics to WGPU with adapter fallbacks
    /// - Honors sRGB
    /// - Falls back when BC compression is unsupported
    /// - Downgrades to 16-bit friendly formats if device bitdepth hints request it
    /// - Falls back to uncompressed sRGB when BC compression is unavailable on the adapter
    /// Port of WW3D Get_Valid_Texture_Format semantics to WGPU with adapter fallbacks
    /// - Honors sRGB and precision preferences
    /// - Downgrades gracefully when BC compression is unavailable on the adapter
    pub fn get_valid_texture_format(
        desired: wgpu::TextureFormat,
        srgb: bool,
    ) -> wgpu::TextureFormat {
        use wgpu::TextureFormat as Tf;

        let srgb = srgb || crate::config::get().force_srgb_textures;
        let bc_supported =
            crate::rendering::wgpu_renderer::wgpu_wrapper::WgpuWrapper::adapter_supports_bc();
        let prefer_16bit =
            crate::rendering::wgpu_renderer::wgpu_wrapper::WgpuWrapper::prefer_16bit_textures();

        let srgb_fallback = if srgb {
            Tf::Rgba8UnormSrgb
        } else {
            Tf::Rgba8Unorm
        };
        let precision_fallback = if prefer_16bit {
            Tf::Rgba16Float
        } else {
            srgb_fallback
        };

        match desired {
            Tf::Rgba8Unorm | Tf::Rgba8UnormSrgb => srgb_fallback,
            Tf::Bgra8Unorm | Tf::Bgra8UnormSrgb => {
                if srgb {
                    Tf::Bgra8UnormSrgb
                } else {
                    Tf::Bgra8Unorm
                }
            }
            Tf::Rgba16Float
            | Tf::Rg16Float
            | Tf::R16Float
            | Tf::Rgba16Unorm
            | Tf::Rgba16Snorm
            | Tf::Rg16Unorm
            | Tf::Rg16Snorm
            | Tf::R16Unorm
            | Tf::R16Snorm => {
                if prefer_16bit {
                    desired
                } else {
                    srgb_fallback
                }
            }
            Tf::Rgba16Uint
            | Tf::Rgba16Sint
            | Tf::Rg16Uint
            | Tf::Rg16Sint
            | Tf::R16Uint
            | Tf::R16Sint => {
                if prefer_16bit {
                    desired
                } else {
                    precision_fallback
                }
            }
            Tf::Bc1RgbaUnorm
            | Tf::Bc1RgbaUnormSrgb
            | Tf::Bc2RgbaUnorm
            | Tf::Bc2RgbaUnormSrgb
            | Tf::Bc3RgbaUnorm
            | Tf::Bc3RgbaUnormSrgb
            | Tf::Bc7RgbaUnorm
            | Tf::Bc7RgbaUnormSrgb => {
                if bc_supported {
                    desired
                } else {
                    srgb_fallback
                }
            }
            Tf::Depth32Float
            | Tf::Depth32FloatStencil8
            | Tf::Depth24Plus
            | Tf::Depth24PlusStencil8
            | Tf::Stencil8
            | Tf::R8Unorm
            | Tf::R8Snorm
            | Tf::R8Uint
            | Tf::R8Sint
            | Tf::Rg8Unorm
            | Tf::Rg8Snorm
            | Tf::Rg8Uint
            | Tf::Rg8Sint
            | Tf::Rgba8Snorm
            | Tf::Rgba8Uint
            | Tf::Rgba8Sint => desired,
            other => {
                let _ = other;
                precision_fallback
            }
        }
    }

    /// Has alpha for desired WW3D format
    pub fn has_alpha(desired: wgpu::TextureFormat) -> bool {
        use wgpu::TextureFormat as Tf;
        matches!(
            desired,
            Tf::Rgba8Unorm
                | Tf::Rgba8UnormSrgb
                | Tf::Bgra8Unorm
                | Tf::Bgra8UnormSrgb
                | Tf::Rgba16Float
                | Tf::Rgba16Unorm
                | Tf::Rgba16Snorm
                | Tf::Bc2RgbaUnorm
                | Tf::Bc2RgbaUnormSrgb
                | Tf::Bc3RgbaUnorm
                | Tf::Bc3RgbaUnormSrgb
                | Tf::Bc7RgbaUnorm
                | Tf::Bc7RgbaUnormSrgb
        )
    }

    /// Bytes per pixel approximation for uncompressed formats (compressed formats return bytes per 4x4 block)
    pub fn bytes_per_pixel(desired: wgpu::TextureFormat) -> u32 {
        use wgpu::TextureFormat as Tf;
        match desired {
            Tf::Rgba8Unorm
            | Tf::Rgba8UnormSrgb
            | Tf::Bgra8Unorm
            | Tf::Bgra8UnormSrgb
            | Tf::Rgba8Snorm
            | Tf::Rgba8Uint
            | Tf::Rgba8Sint => 4,
            Tf::Rg8Unorm | Tf::Rg8Snorm | Tf::Rg8Uint | Tf::Rg8Sint => 2,
            Tf::R8Unorm | Tf::R8Snorm | Tf::R8Uint | Tf::R8Sint | Tf::Stencil8 => 1,
            Tf::Rgba16Float
            | Tf::Rgba16Unorm
            | Tf::Rgba16Snorm
            | Tf::Rgba16Uint
            | Tf::Rgba16Sint => 8,
            Tf::Rg16Float | Tf::Rg16Unorm | Tf::Rg16Snorm | Tf::Rg16Uint | Tf::Rg16Sint => 4,
            Tf::R16Float | Tf::R16Unorm | Tf::R16Snorm | Tf::R16Uint | Tf::R16Sint => 2,
            Tf::Bc1RgbaUnorm | Tf::Bc1RgbaUnormSrgb => 8,
            Tf::Bc2RgbaUnorm
            | Tf::Bc2RgbaUnormSrgb
            | Tf::Bc3RgbaUnorm
            | Tf::Bc3RgbaUnormSrgb
            | Tf::Bc7RgbaUnorm
            | Tf::Bc7RgbaUnormSrgb => 16,
            Tf::Depth32Float | Tf::Depth32FloatStencil8 => 4,
            Tf::Depth24Plus | Tf::Depth24PlusStencil8 => 4,
            _ => 4,
        }
    }

    /// Get texture width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get texture height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get texture depth (for volume textures)
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Get number of mip levels
    pub fn mip_level_count(&self) -> u32 {
        self.mip_level_count
    }

    /// Get texture name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set texture name
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Get full path
    pub fn full_path(&self) -> &str {
        &self.full_path
    }

    /// Set full path
    pub fn set_full_path(&mut self, path: &str) {
        self.full_path = path.to_string();
    }

    /// Update the WW3D and GPU texture formats.
    pub fn set_format(&mut self, format: WW3DFormat) {
        self.ww3d_format = format;
        if let Some(mapped) = format.to_wgpu_format() {
            self.format = mapped;
        }
    }

    /// Update the texture dimensions and mip information.
    pub fn set_dimensions(&mut self, width: u32, height: u32, depth: u32, mip_levels: u32) {
        self.width = width;
        self.height = height;
        self.depth = depth.max(1);
        self.mip_level_count = mip_levels.max(1);
    }

    /// Replace the resident system-memory copy of the texture.
    pub fn set_system_memory(&mut self, bytes: Vec<u8>) {
        self.system_memory = bytes;
    }

    /// Update CPU-side mip metadata.
    pub fn set_system_mip_levels(&mut self, levels: Vec<SystemMipLevel>) {
        self.system_mip_levels = levels;
    }

    /// Borrow the system-memory backing store, if any.
    pub fn system_memory(&self) -> &[u8] {
        &self.system_memory
    }

    /// Optional record of how the texture format was resolved.
    pub fn format_history(&self) -> Option<&FormatDecision> {
        self.format_history.as_ref()
    }

    pub fn set_usage_policy(&mut self, policy: TextureUsagePolicy) {
        self.allow_compression = policy.allow_compression;
        self.allow_reduction = policy.allow_reduction;
        self.requested_mip_levels = policy.requested_mip_levels;
    }

    pub fn usage_policy(&self) -> TextureUsagePolicy {
        TextureUsagePolicy::new(
            self.allow_compression,
            self.allow_reduction,
            self.requested_mip_levels,
        )
    }

    pub fn has_gpu_texture(&self) -> bool {
        self.wgpu_texture.is_some()
    }

    pub fn apply_texture_data(&mut self, data: &TextureData) {
        self.set_dimensions(data.width, data.height, data.depth, data.mip_levels);
        self.set_format(data.format);
        self.set_system_memory(data.data.clone());
        let mips: Vec<SystemMipLevel> = data
            .mip_levels()
            .iter()
            .map(|level| SystemMipLevel {
                offset: level.offset,
                size: level.size,
                width: level.width,
                height: level.height,
                depth_or_layers: level.depth_or_layers,
                slice_stride: level.slice_stride,
            })
            .collect();
        self.set_system_mip_levels(mips);
        self.asset_type = match data.kind {
            TextureDataKind::Texture2D => TexAssetType::Regular,
            TextureDataKind::CubeMap => TexAssetType::Cubemap,
            TextureDataKind::Volume => TexAssetType::Volume,
        };
        if matches!(self.asset_type, TexAssetType::Cubemap) {
            self.depth = 1;
        }
        self.format_history = data.format_decision.clone();
        self.set_usage_policy(TextureUsagePolicy::default());
        if let Some(decision) = &self.format_history {
            let identifier = if !self.name.is_empty() {
                self.name.clone()
            } else if !self.full_path.is_empty() {
                self.full_path.clone()
            } else {
                "<unnamed texture>".to_string()
            };
            texture_metrics::record_decision(identifier, decision, self.mip_level_count);
        }
    }

    /// Upload the CPU copy to the GPU if needed.
    pub fn ensure_gpu_texture(&mut self, device: &Device, queue: &Queue) -> Result<()> {
        if self.wgpu_texture.is_some() {
            return Ok(());
        }

        if self.system_memory.is_empty() {
            return Err(Error::InvalidData(
                "Texture has no system-memory backing to upload".to_string(),
            ));
        }

        if self.system_mip_levels.is_empty() {
            return Err(Error::InvalidData(
                "Texture mip layout is missing; cannot upload".to_string(),
            ));
        }

        let (dimension, depth_or_layers) = match self.asset_type {
            TexAssetType::Regular => (TextureDimension::D2, 1),
            TexAssetType::Cubemap => (TextureDimension::D2, 6),
            TexAssetType::Volume => (TextureDimension::D3, self.depth.max(1)),
        };

        let descriptor = TextureDescriptor {
            label: Some(&self.name),
            size: Extent3d {
                width: self.width.max(1),
                height: self.height.max(1),
                depth_or_array_layers: depth_or_layers,
            },
            mip_level_count: self.mip_level_count,
            sample_count: 1,
            dimension,
            format: self.format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = device.create_texture(&descriptor);
        let is_compressed = self.ww3d_format.is_block_compressed();

        for (level_index, mip) in self.system_mip_levels.iter().enumerate() {
            let layers = mip.depth_or_layers.max(1) as usize;
            let slice_stride = if mip.slice_stride == 0 {
                mip.size
            } else {
                mip.slice_stride
            };

            for layer in 0..layers {
                let slice_begin = mip.offset + layer * slice_stride;
                let slice_end = slice_begin + slice_stride;
                if slice_end > self.system_memory.len() {
                    return Err(Error::InvalidData(format!(
                        "Mip level {} exceeds system-memory buffer",
                        level_index
                    )));
                }

                let raw = &self.system_memory[slice_begin..slice_end];
                let mut prepared: Cow<'_, [u8]> = if is_compressed {
                    Cow::Borrowed(raw)
                } else {
                    prepare_uncompressed_slice(
                        self.ww3d_format,
                        self.format,
                        raw,
                        mip.width,
                        mip.height,
                    )?
                };

                let (bytes_per_row, rows_per_image) = if is_compressed {
                    (None, None)
                } else {
                    let wgpu_bpp = wgpu_bytes_per_pixel(self.format).ok_or_else(|| {
                        Error::InvalidData(format!(
                            "Unsupported WGPU texture format {:?}",
                            self.format
                        ))
                    })?;
                    let row_bytes = (mip.width.max(1) as usize) * wgpu_bpp;
                    let expected_len = row_bytes * mip.height as usize;
                    if prepared.as_ref().len() != expected_len {
                        return Err(Error::InvalidData(format!(
                            "Mip level {} has unexpected byte count ({}) for upload",
                            level_index,
                            prepared.as_ref().len()
                        )));
                    }
                    let aligned = align_to(row_bytes, 256);
                    if aligned != row_bytes {
                        prepared = Cow::Owned(align_rows(
                            prepared.as_ref(),
                            row_bytes,
                            aligned,
                            mip.height as usize,
                        ));
                    }
                    (Some(aligned as u32), Some(mip.height))
                };

                let origin = match self.asset_type {
                    TexAssetType::Regular => Origin3d::ZERO,
                    TexAssetType::Cubemap | TexAssetType::Volume => Origin3d {
                        x: 0,
                        y: 0,
                        z: layer as u32,
                    },
                };

                queue.write_texture(
                    TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: level_index as u32,
                        origin,
                        aspect: wgpu::TextureAspect::All,
                    },
                    prepared.as_ref(),
                    TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row,
                        rows_per_image,
                    },
                    Extent3d {
                        width: mip.width,
                        height: mip.height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }

        self.init_texture_resources(texture);
        Ok(())
    }

    /// Replace the current texture contents with the provided slice.
    pub fn copy_from_slice(
        &mut self,
        width: u32,
        height: u32,
        mip_levels: u32,
        format: WW3DFormat,
        data: &[u8],
    ) -> bool {
        self.set_dimensions(width, height, 1, mip_levels.max(1));
        self.set_format(format);
        self.set_system_memory(data.to_vec());
        let level = SystemMipLevel {
            offset: 0,
            size: data.len(),
            width,
            height,
            depth_or_layers: 1,
            slice_stride: data.len(),
        };
        self.set_system_mip_levels(vec![level]);
        self.format_history = None;
        self.set_usage_policy(TextureUsagePolicy::default());
        true
    }

    /// Get pool type
    pub fn pool(&self) -> PoolType {
        self.pool
    }

    /// Get asset type
    pub fn asset_type(&self) -> TexAssetType {
        self.asset_type
    }

    /// Apply texture to current shader
    pub fn apply(&self, stage: usize) {
        // Bind texture to shader stage
        // In a full implementation, this would bind the texture to the specified shader stage
        // using the WGPU bind group system
        let _ = stage; // Use parameter to avoid warning
    }

    /// Peek at underlying texture handle
    pub fn peek_texture(&self) -> Option<&Arc<WgpuTexture>> {
        self.wgpu_texture.as_ref()
    }

    /// Is texture loaded?
    pub fn is_loaded(&self) -> bool {
        self.wgpu_texture.is_some()
    }

    /// Load texture from file
    pub fn load(&mut self, filename: &str) -> bool {
        self.set_name(filename);
        self.set_full_path(filename);

        match decode_texture_file(filename) {
            Ok(data) => {
                self.apply_texture_data(&data);
                true
            }
            Err(err) => {
                log::warn!("Failed to load texture '{}': {}", filename, err);
                false
            }
        }
    }

    /// Get texture priority
    pub fn get_priority(&self) -> f32 {
        // Implement priority system
        // In a full implementation, this would return a priority value for texture management
        0.0 // Default priority
    }

    /// Set texture priority
    pub fn set_priority(&mut self, priority: f32) {
        // Implement priority system
        // In a full implementation, this would set priority for texture management
        let _ = priority; // Use parameter to avoid warning
    }

    /// Get texture reduction factor
    pub fn get_reduction_factor(&self) -> f32 {
        let reduction = texture_quality::compute_effective_reduction(
            self.width.max(1),
            self.height.max(1),
            self.mip_level_count.max(1),
        );
        if reduction == 0 {
            1.0
        } else {
            let factor = 1u32 << reduction;
            1.0 / factor as f32
        }
    }

    /// Set texture reduction factor
    pub fn set_reduction_factor(&mut self, factor: f32) {
        // Implement reduction factor
        // In a full implementation, this would set the texture size reduction factor
        let _ = factor; // Use parameter to avoid warning
    }

    /// Get texture filter mode
    pub fn get_texture_filter(&self) -> TextureFilterMode {
        self.stage_settings.filter
    }

    /// Set texture filter mode
    pub fn set_texture_filter(&mut self, filter: TextureFilterMode) {
        self.stage_settings.filter = filter;
    }

    /// Get U address mode
    pub fn get_u_address_mode(&self) -> TextureAddressMode {
        self.stage_settings.address_u
    }

    /// Set U address mode
    pub fn set_u_address_mode(&mut self, mode: TextureAddressMode) {
        self.stage_settings.address_u = mode;
    }

    /// Get V address mode
    pub fn get_v_address_mode(&self) -> TextureAddressMode {
        self.stage_settings.address_v
    }

    /// Set V address mode
    pub fn set_v_address_mode(&mut self, mode: TextureAddressMode) {
        self.stage_settings.address_v = mode;
    }

    /// Get anisotropy level
    pub fn get_anisotropy_level(&self) -> u32 {
        self.stage_settings.anisotropy.into()
    }

    /// Set anisotropy level
    pub fn set_anisotropy_level(&mut self, level: u32) {
        let clamped = level.clamp(1, 16) as u16;
        self.stage_settings.anisotropy = clamped;
    }

    /// Access the current sampler stage settings recorded on this texture.
    pub fn stage_settings(&self) -> TextureStageSettings {
        self.stage_settings
    }

    /// Override the sampler stage settings recorded on this texture.
    pub fn set_stage_settings(&mut self, settings: TextureStageSettings) {
        self.stage_settings = settings;
    }

    /// Compress the texture
    pub fn compress(&self) -> Result<()> {
        // Placeholder implementation for texture compression
        // In a full implementation, this would compress the texture data using DXTn format
        Ok(())
    }

    /// Generate mipmaps for the texture
    pub fn generate_mipmaps(&self) -> Result<()> {
        // Placeholder implementation for mipmap generation
        // In a full implementation, this would generate mipmap levels for the texture
        Ok(())
    }

    /// Get memory usage of the texture in bytes (per top-level mip)
    pub fn get_memory_usage(&self) -> u64 {
        if !self.system_memory.is_empty() {
            return self.system_memory.len() as u64;
        }

        use wgpu::TextureFormat as Tf;

        let w = self.width.max(1) as u64;
        let h = self.height.max(1) as u64;

        match self.format {
            Tf::Bc1RgbaUnorm | Tf::Bc1RgbaUnormSrgb => {
                let blocks_w = (w + 3) / 4;
                let blocks_h = (h + 3) / 4;
                blocks_w * blocks_h * 8
            }
            Tf::Bc2RgbaUnorm
            | Tf::Bc2RgbaUnormSrgb
            | Tf::Bc3RgbaUnorm
            | Tf::Bc3RgbaUnormSrgb
            | Tf::Bc7RgbaUnorm
            | Tf::Bc7RgbaUnormSrgb => {
                let blocks_w = (w + 3) / 4;
                let blocks_h = (h + 3) / 4;
                blocks_w * blocks_h * 16
            }
            _ => w * h * Self::bytes_per_pixel(self.format) as u64,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.wgpu_texture.is_some() || !self.system_memory.is_empty()
    }
}

/// Texture filter mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFilterMode {
    Point,
    Linear,
    Anisotropic,
    Nearest, // Added for compatibility
}

/// Texture address mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureAddressMode {
    Wrap,
    Clamp,
    Mirror,
    Border,
    Repeat, // Added for compatibility (same as Wrap)
}

/// Regular 2D texture class
#[derive(Debug)]
pub struct TextureClass {
    base: TextureBaseClass,
}

impl TextureClass {
    /// Create new 2D texture
    pub fn new(width: u32, height: u32, mip_level_count: u32, pool: PoolType) -> Self {
        Self {
            base: TextureBaseClass::new(
                width,
                height,
                mip_level_count as u32,
                pool,
                TexAssetType::Regular,
            ),
        }
    }

    /// Get base texture
    pub fn base(&self) -> &TextureBaseClass {
        &self.base
    }

    /// Get mutable base texture
    pub fn base_mut(&mut self) -> &mut TextureBaseClass {
        &mut self.base
    }

    /// Copy texture data
    pub fn copy(
        &mut self,
        width: u32,
        height: u32,
        mip_levels: u32,
        format: WW3DFormat,
        data: &[u8],
    ) -> bool {
        self.base
            .copy_from_slice(width, height, mip_levels, format, data)
    }

    /// Lock texture for reading/writing
    pub fn lock(&mut self, level: u32) -> Option<TextureLockData> {
        let surface = self.get_surface_level(level)?;
        let lock = surface.lock();
        Some(TextureLockData {
            pitch: lock.pitch() as u32,
            data: lock.pixels().to_vec(),
        })
    }

    /// Unlock texture
    pub fn unlock(&mut self, level: u32) -> bool {
        self.base.system_mip_levels.get(level as usize).is_some()
    }

    /// Get surface level
    pub fn get_surface_level(&self, level: u32) -> Option<SurfaceClass> {
        let mip = self.base.system_mip_levels.get(level as usize)?;
        let texture_format = TextureFormat::from_ww3d(self.base.ww3d_format)?;
        let slice_stride = if mip.slice_stride == 0 {
            mip.size
        } else {
            mip.slice_stride
        };
        let slice_begin = mip.offset;
        let slice_end = slice_begin.checked_add(slice_stride)?;
        if slice_end > self.base.system_memory.len() {
            return None;
        }
        SurfaceClass::from_bytes(
            mip.width.max(1),
            mip.height.max(1),
            texture_format,
            &self.base.system_memory[slice_begin..slice_end],
        )
        .ok()
    }

    /// Get texture width
    pub fn get_width(&self) -> u32 {
        self.base.width()
    }

    /// Get texture height  
    pub fn get_height(&self) -> u32 {
        self.base.height()
    }

    pub fn is_initialized(&self) -> bool {
        self.base.is_initialized()
    }

    /// Get texture name
    pub fn get_name(&self) -> &str {
        self.base.name()
    }

    /// Check if texture is a lightmap
    pub fn is_lightmap(&self) -> bool {
        let name = self.base.name.to_ascii_lowercase();
        name.contains("lightmap") || name.contains("_lm")
    }

    /// Check if texture is procedural
    pub fn is_procedural(&self) -> bool {
        let name = self.base.name.to_ascii_lowercase();
        name.starts_with("proc_") || name.contains("procedural")
    }

    /// Get memory usage
    pub fn get_memory_usage(&self) -> usize {
        self.base.get_memory_usage() as usize
    }
}

/// Texture lock data structure
pub struct TextureLockData {
    pub pitch: u32,
    pub data: Vec<u8>,
}

/// Cube texture class
pub struct CubeTextureClass {
    base: TextureBaseClass,
}

impl CubeTextureClass {
    /// Create new cube texture
    pub fn new(width: u32, height: u32, mip_level_count: u32, pool: PoolType) -> Self {
        Self {
            base: TextureBaseClass::new(
                width,
                height,
                mip_level_count as u32,
                pool,
                TexAssetType::Cubemap,
            ),
        }
    }

    /// Get base texture
    pub fn base(&self) -> &TextureBaseClass {
        &self.base
    }

    /// Get mutable base texture
    pub fn base_mut(&mut self) -> &mut TextureBaseClass {
        &mut self.base
    }

    /// Get face surface
    pub fn get_face_surface(&self, face: CubeFace, level: u32) -> Option<SurfaceClass> {
        let face_index = match face {
            CubeFace::PositiveX => 0,
            CubeFace::NegativeX => 1,
            CubeFace::PositiveY => 2,
            CubeFace::NegativeY => 3,
            CubeFace::PositiveZ => 4,
            CubeFace::NegativeZ => 5,
        };

        let mip = self.base.system_mip_levels.get(level as usize)?;
        let texture_format = TextureFormat::from_ww3d(self.base.ww3d_format)?;
        if mip.depth_or_layers == 0 || face_index as u32 >= mip.depth_or_layers {
            return None;
        }

        let slice_stride = if mip.slice_stride == 0 {
            mip.size
        } else {
            mip.slice_stride
        };

        let slice_begin = mip.offset + slice_stride * face_index as usize;
        let slice_end = slice_begin.checked_add(slice_stride)?;
        if slice_end > self.base.system_memory.len() {
            return None;
        }

        SurfaceClass::from_bytes(
            mip.width.max(1),
            mip.height.max(1),
            texture_format,
            &self.base.system_memory[slice_begin..slice_end],
        )
        .ok()
    }
}

/// Cube face enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CubeFace {
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

/// Volume texture class
pub struct VolumeTextureClass {
    base: TextureBaseClass,
}

impl VolumeTextureClass {
    /// Create new volume texture
    pub fn new(width: u32, height: u32, depth: u32, mip_level_count: u32, pool: PoolType) -> Self {
        let mut base = TextureBaseClass::new(
            width,
            height,
            mip_level_count as u32,
            pool,
            TexAssetType::Volume,
        );
        base.depth = depth;
        Self { base }
    }

    /// Get base texture
    pub fn base(&self) -> &TextureBaseClass {
        &self.base
    }

    /// Get mutable base texture
    pub fn base_mut(&mut self) -> &mut TextureBaseClass {
        &mut self.base
    }

    /// Get volume level
    pub fn get_volume_level(&self, level: u32) -> Option<SurfaceClass> {
        let mip = self.base.system_mip_levels.get(level as usize)?;
        let texture_format = TextureFormat::from_ww3d(self.base.ww3d_format)?;
        let slice_stride = if mip.slice_stride == 0 {
            mip.size
        } else {
            mip.slice_stride
        };
        let slice_begin = mip.offset;
        let slice_end = slice_begin.checked_add(slice_stride)?;
        if slice_end > self.base.system_memory.len() {
            return None;
        }
        SurfaceClass::from_bytes(
            mip.width.max(1),
            mip.height.max(1),
            texture_format,
            &self.base.system_memory[slice_begin..slice_end],
        )
        .ok()
    }
}

// Implement Deref for easy access to base functionality
impl std::ops::Deref for TextureClass {
    type Target = TextureBaseClass;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for TextureClass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl std::ops::Deref for CubeTextureClass {
    type Target = TextureBaseClass;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for CubeTextureClass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl std::ops::Deref for VolumeTextureClass {
    type Target = TextureBaseClass;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl std::ops::DerefMut for VolumeTextureClass {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    #[test]
    fn texture_load_decodes_png_into_system_memory() {
        let temp_path =
            std::env::temp_dir().join(format!("ww3d_texture_base_load_{}.png", std::process::id()));
        let image = ImageBuffer::<Rgba<u8>, _>::from_fn(2, 2, |_x, _y| Rgba([5, 10, 15, 255]));
        image.save(&temp_path).expect("write test texture");

        let mut base = TextureBaseClass::new(1, 1, 1, PoolType::Managed, TexAssetType::Regular);
        assert!(base.load(temp_path.to_string_lossy().as_ref()));
        assert_eq!(base.width(), 2);
        assert_eq!(base.height(), 2);
        assert!(!base.system_memory().is_empty());

        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn texture_lock_exposes_surface_bytes() {
        let mut texture = TextureClass::new(2, 2, 1, PoolType::Managed);
        let data = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        assert!(texture.copy(2, 2, 1, WW3DFormat::A8R8G8B8, &data));

        let locked = texture.lock(0).expect("lock should succeed");
        assert_eq!(locked.pitch, 8);
        assert_eq!(locked.data, data);
        assert!(texture.unlock(0));
        assert!(!texture.unlock(4));
    }

    #[test]
    fn volume_level_returns_first_slice_surface() {
        let mut volume = VolumeTextureClass::new(2, 2, 2, 1, PoolType::Managed);
        volume.base_mut().set_format(WW3DFormat::A8R8G8B8);
        volume
            .base_mut()
            .set_system_mip_levels(vec![SystemMipLevel {
                offset: 0,
                size: 32,
                width: 2,
                height: 2,
                depth_or_layers: 2,
                slice_stride: 16,
            }]);
        let mut bytes = vec![20u8; 32];
        bytes[..16].fill(10);
        volume.base_mut().set_system_memory(bytes);

        let surface = volume
            .get_volume_level(0)
            .expect("volume level surface should be available");
        let lock = surface.lock();
        assert_eq!(lock.pixels().len(), 16);
        assert!(lock.pixels().iter().all(|value| *value == 10));
    }
}
