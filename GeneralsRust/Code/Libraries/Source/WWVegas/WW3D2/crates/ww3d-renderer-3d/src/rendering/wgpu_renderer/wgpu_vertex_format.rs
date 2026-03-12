//! WGPU Vertex Format Definitions
//!
//! This module provides WGPU-compatible vertex format definitions that mirror
//! the DirectX8 FVF (Flexible Vertex Format) system from dx8fvf.h

use crate::core::error::{Error, Result};
use bytemuck::{Pod, Zeroable};
use wgpu::{VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

/// Vertex format flags (equivalent to DirectX8 FVF flags)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VertexFormatFlags {
    /// Position only (X, Y, Z)
    Xyz = 0x002,
    /// Position with normal (X, Y, Z, NX, NY, NZ)
    Xyzn = 0x042,
    /// Position, normal, 1 texture coordinate
    XyznUv1 = 0x242,
    /// Position, normal, 2 texture coordinates
    XyznUv2 = 0x442,
    /// Position, normal, diffuse color, 1 texture coordinate
    XyznDUv1 = 0x342,
    /// Position, normal, diffuse color, 2 texture coordinates
    XyznDUv2 = 0x542,
    /// Position, diffuse color, 1 texture coordinate
    XyzDUv1 = 0x142,
    /// Position, diffuse color, 2 texture coordinates
    XyzDUv2 = 0x1342,
    /// Position, 1 texture coordinate
    XyzUv1 = 0x1142,
    /// Position, 2 texture coordinates
    XyzUv2 = 0x1242,
    /// Position, normal, diffuse, tangent space (4 texture coordinates)
    XyznDUv1Tg3 = 0x3742,
    /// Position, normal, displacement mapping (3 texture coordinates)
    XyznUv2Dmap = 0x3462,
    /// Position, normal, diffuse, cube mapping
    XyznDCubemap = 0x2342,
}

/// Standard vertex structures (equivalent to DirectX8 vertex format structs)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyz {
    pub position: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyznUv1 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord1: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyznUv2 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord1: [f32; 2],
    pub tex_coord2: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyzn {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyznDUv1 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub tex_coord1: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyznDUv2 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub tex_coord1: [f32; 2],
    pub tex_coord2: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyzDUv1 {
    pub position: [f32; 3],
    pub diffuse: u32,
    pub tex_coord1: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyzDUv2 {
    pub position: [f32; 3],
    pub diffuse: u32,
    pub tex_coord1: [f32; 2],
    pub tex_coord2: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyzUv1 {
    pub position: [f32; 3],
    pub tex_coord1: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyzUv2 {
    pub position: [f32; 3],
    pub tex_coord1: [f32; 2],
    pub tex_coord2: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyznDUv1Tg3 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub tex_coord1: [f32; 2],    // UV1
    pub tangent: [f32; 3],       // Sx, Sy, Sz
    pub binormal: [f32; 3],      // Tx, Ty, Tz
    pub tangent_cross: [f32; 3], // SxTx, SxTy, SxTz
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyznUv2Dmap {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord1: f32,      // Single coordinate for displacement
    pub tangent1: [f32; 4],   // T1x, T1y, T1z, T1w
    pub tex_coord2: [f32; 2], // UV2
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexXyznDCubemap {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    // Texture coordinates are typically generated for cube mapping
}

/// Vertex format information class (equivalent to FVFInfoClass)
#[derive(Debug, Clone)]
pub struct VertexFormatInfo {
    /// The vertex format flags
    pub format_flags: VertexFormatFlags,
    /// Size of each vertex in bytes
    pub vertex_size: u32,
    /// Offset to position in vertex
    pub position_offset: u32,
    /// Offset to normal in vertex (if present)
    pub normal_offset: u32,
    /// Offset to diffuse color in vertex (if present)
    pub diffuse_offset: u32,
    /// Offsets to texture coordinates
    pub tex_coord_offsets: Vec<u32>,
    /// WGPU vertex buffer layout
    pub buffer_layout: VertexBufferLayout<'static>,
}

impl VertexFormatInfo {
    /// Create vertex format info for a given format
    pub fn new(format: VertexFormatFlags) -> Result<Self> {
        let (vertex_size, position_offset, normal_offset, diffuse_offset, tex_coord_offsets) =
            Self::calculate_offsets(format);

        let buffer_layout = Self::create_buffer_layout(format, vertex_size)?;

        Ok(Self {
            format_flags: format,
            vertex_size,
            position_offset,
            normal_offset,
            diffuse_offset,
            tex_coord_offsets,
            buffer_layout,
        })
    }

    /// Calculate field offsets for a vertex format
    fn calculate_offsets(format: VertexFormatFlags) -> (u32, u32, u32, u32, Vec<u32>) {
        let mut offset = 0u32;
        let mut normal_offset = 0;
        let mut diffuse_offset = 0;
        let mut tex_coord_offsets = Vec::new();

        // Position is always first
        let position_offset = offset;
        offset += 12; // 3 * f32

        // Check for normal
        if Self::has_normal(format) {
            normal_offset = offset;
            offset += 12; // 3 * f32
        }

        // Check for diffuse color
        if Self::has_diffuse(format) {
            diffuse_offset = offset;
            offset += 4; // u32
        }

        // Check for texture coordinates
        let tex_coord_count = Self::get_tex_coord_count(format);
        for i in 0..tex_coord_count {
            tex_coord_offsets.push(offset);
            let coord_size = Self::get_tex_coord_size(format, i);
            offset += coord_size;
        }

        (
            offset,
            position_offset,
            normal_offset,
            diffuse_offset,
            tex_coord_offsets,
        )
    }

    /// Create WGPU vertex buffer layout for the format
    fn create_buffer_layout(
        format: VertexFormatFlags,
        stride: u32,
    ) -> Result<VertexBufferLayout<'static>> {
        let mut attributes = Vec::new();
        let mut location = 0u32;

        // Position attribute (always present)
        attributes.push(VertexAttribute {
            offset: 0,
            shader_location: location,
            format: VertexFormat::Float32x3,
        });
        location += 1;

        let mut current_offset = 12u64; // After position

        // Normal attribute
        if Self::has_normal(format) {
            attributes.push(VertexAttribute {
                offset: current_offset,
                shader_location: location,
                format: VertexFormat::Float32x3,
            });
            location += 1;
            current_offset += 12;
        }

        // Diffuse color attribute
        if Self::has_diffuse(format) {
            attributes.push(VertexAttribute {
                offset: current_offset,
                shader_location: location,
                format: VertexFormat::Uint32,
            });
            location += 1;
            current_offset += 4;
        }

        // Texture coordinate attributes
        let tex_coord_count = Self::get_tex_coord_count(format);
        for i in 0..tex_coord_count {
            let coord_size = Self::get_tex_coord_size(format, i) as u64;
            let format = match coord_size {
                4 => VertexFormat::Float32,    // 1 component
                8 => VertexFormat::Float32x2,  // 2 components
                12 => VertexFormat::Float32x3, // 3 components
                16 => VertexFormat::Float32x4, // 4 components
                _ => {
                    return Err(Error::InvalidVertexFormat(format!(
                        "Unsupported vertex element size: {}",
                        coord_size
                    )))
                }
            };

            attributes.push(VertexAttribute {
                offset: current_offset,
                shader_location: location,
                format,
            });
            location += 1;
            current_offset += coord_size;
        }

        Ok(VertexBufferLayout {
            array_stride: stride as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: attributes.leak(), // Convert to static lifetime
        })
    }

    /// Check if format has normal
    fn has_normal(format: VertexFormatFlags) -> bool {
        matches!(
            format,
            VertexFormatFlags::Xyzn
                | VertexFormatFlags::XyznUv1
                | VertexFormatFlags::XyznUv2
                | VertexFormatFlags::XyznDUv1
                | VertexFormatFlags::XyznDUv2
                | VertexFormatFlags::XyznDUv1Tg3
                | VertexFormatFlags::XyznUv2Dmap
                | VertexFormatFlags::XyznDCubemap
        )
    }

    /// Check if format has diffuse color
    fn has_diffuse(format: VertexFormatFlags) -> bool {
        matches!(
            format,
            VertexFormatFlags::XyznDUv1
                | VertexFormatFlags::XyznDUv2
                | VertexFormatFlags::XyzDUv1
                | VertexFormatFlags::XyzDUv2
                | VertexFormatFlags::XyznDUv1Tg3
                | VertexFormatFlags::XyznDCubemap
        )
    }

    /// Get texture coordinate count
    fn get_tex_coord_count(format: VertexFormatFlags) -> usize {
        match format {
            VertexFormatFlags::XyzUv1 | VertexFormatFlags::XyzDUv1 => 1,
            VertexFormatFlags::XyzUv2
            | VertexFormatFlags::XyzDUv2
            | VertexFormatFlags::XyznUv1
            | VertexFormatFlags::XyznDUv1 => 1,
            VertexFormatFlags::XyznUv2 | VertexFormatFlags::XyznDUv2 => 2,
            VertexFormatFlags::XyznUv2Dmap => 3,
            VertexFormatFlags::XyznDUv1Tg3 => 4,
            _ => 0,
        }
    }

    /// Get texture coordinate size for specific index
    fn get_tex_coord_size(format: VertexFormatFlags, index: usize) -> u32 {
        match format {
            VertexFormatFlags::XyznDUv1Tg3 => match index {
                0 => 8,  // UV1 - 2 floats
                1 => 12, // Tangent - 3 floats
                2 => 12, // Binormal - 3 floats
                3 => 12, // Tangent cross - 3 floats
                _ => 0,
            },
            VertexFormatFlags::XyznUv2Dmap => match index {
                0 => 4,  // Single coord - 1 float
                1 => 16, // Tangent1 - 4 floats
                2 => 8,  // UV2 - 2 floats
                _ => 0,
            },
            _ => 8, // Default 2 floats (UV)
        }
    }

    /// Get position offset
    pub fn position_offset(&self) -> u32 {
        self.position_offset
    }

    /// Get normal offset
    pub fn normal_offset(&self) -> u32 {
        self.normal_offset
    }

    /// Get diffuse offset
    pub fn diffuse_offset(&self) -> u32 {
        self.diffuse_offset
    }

    /// Get specular offset
    /// Get texture coordinate offset
    pub fn tex_coord_offset(&self, index: usize) -> Option<u32> {
        self.tex_coord_offsets.get(index).copied()
    }

    /// Get vertex size
    pub fn vertex_size(&self) -> u32 {
        self.vertex_size
    }

    /// Get WGPU buffer layout
    pub fn buffer_layout(&self) -> &VertexBufferLayout<'static> {
        &self.buffer_layout
    }
}

/// Vertex format utilities
pub struct VertexFormatUtils;

impl VertexFormatUtils {
    /// Convert DirectX8 FVF to our vertex format flags
    pub fn d3d_fvf_to_vertex_format(fvf: u32) -> Result<VertexFormatFlags> {
        match fvf {
            0x002 => Ok(VertexFormatFlags::Xyz),
            0x042 => Ok(VertexFormatFlags::Xyzn),
            0x242 => Ok(VertexFormatFlags::XyznUv1),
            0x442 => Ok(VertexFormatFlags::XyznUv2),
            0x342 => Ok(VertexFormatFlags::XyznDUv1),
            0x542 => Ok(VertexFormatFlags::XyznDUv2),
            0x142 => Ok(VertexFormatFlags::XyzDUv1),
            0x1342 => Ok(VertexFormatFlags::XyzDUv2),
            0x1142 => Ok(VertexFormatFlags::XyzUv1),
            _ => Err(Error::UnsupportedVertexFormat(format!(
                "Unsupported vertex element type: {:#x}",
                fvf
            ))),
        }
    }

    /// Convert our vertex format to DirectX8 FVF (for compatibility)
    pub fn vertex_format_to_d3d_fvf(format: VertexFormatFlags) -> u32 {
        match format {
            VertexFormatFlags::Xyz => 0x002,
            VertexFormatFlags::Xyzn => 0x042,
            VertexFormatFlags::XyznUv1 => 0x242,
            VertexFormatFlags::XyznUv2 => 0x442,
            VertexFormatFlags::XyznDUv1 => 0x342,
            VertexFormatFlags::XyznDUv2 => 0x542,
            VertexFormatFlags::XyzDUv1 => 0x142,
            VertexFormatFlags::XyzDUv2 => 0x1342,
            VertexFormatFlags::XyzUv1 => 0x1142,
            VertexFormatFlags::XyzUv2 => 0x1242,
            VertexFormatFlags::XyznDUv1Tg3 => 0x3742,
            VertexFormatFlags::XyznUv2Dmap => 0x3462,
            VertexFormatFlags::XyznDCubemap => 0x2342,
        }
    }

    /// Calculate vertex size for a given format
    pub fn calculate_vertex_size(format: VertexFormatFlags) -> u32 {
        match format {
            VertexFormatFlags::Xyz => 12,
            VertexFormatFlags::Xyzn => 24,
            VertexFormatFlags::XyznUv1 => 32,
            VertexFormatFlags::XyznUv2 => 40,
            VertexFormatFlags::XyznDUv1 => 36,
            VertexFormatFlags::XyznDUv2 => 44,
            VertexFormatFlags::XyzDUv1 => 24,
            VertexFormatFlags::XyzDUv2 => 32,
            VertexFormatFlags::XyzUv1 => 20,
            VertexFormatFlags::XyzUv2 => 28,
            VertexFormatFlags::XyznDUv1Tg3 => 64,
            VertexFormatFlags::XyznUv2Dmap => 48,
            VertexFormatFlags::XyznDCubemap => 28,
        }
    }

    /// Get format name for debugging
    pub fn format_name(format: VertexFormatFlags) -> &'static str {
        match format {
            VertexFormatFlags::Xyz => "XYZ",
            VertexFormatFlags::Xyzn => "XYZN",
            VertexFormatFlags::XyznUv1 => "XYZNUV1",
            VertexFormatFlags::XyznUv2 => "XYZNUV2",
            VertexFormatFlags::XyznDUv1 => "XYZNDUV1",
            VertexFormatFlags::XyznDUv2 => "XYZNDUV2",
            VertexFormatFlags::XyzDUv1 => "XYZDUV1",
            VertexFormatFlags::XyzDUv2 => "XYZDUV2",
            VertexFormatFlags::XyzUv1 => "XYZUV1",
            VertexFormatFlags::XyzUv2 => "XYZUV2",
            VertexFormatFlags::XyznDUv1Tg3 => "XYZDUV1TG3",
            VertexFormatFlags::XyznUv2Dmap => "XYZNUV2DMAP",
            VertexFormatFlags::XyznDCubemap => "XYZNDCUBEMAP",
        }
    }
}
