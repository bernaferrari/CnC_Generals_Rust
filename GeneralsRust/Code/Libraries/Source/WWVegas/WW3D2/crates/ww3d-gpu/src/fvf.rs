//! Flexible Vertex Format (FVF) System
//!
//! This module provides DirectX 8 FVF compatibility by translating FVF codes
//! into wgpu VertexBufferLayout structures. It supports all common vertex formats
//! used in the original C++ codebase.

use bytemuck::{Pod, Zeroable};
use std::mem;
use wgpu::{VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};

/// FVF format codes (matching DX8 FVF constants)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum FvfFormat {
    /// Position only (XYZ)
    XYZ = 0x002,
    /// Position + Normal (XYZN)
    XYZN = 0x012,
    /// Position + Normal + 1 UV (XYZNUV1)
    XYZNUV1 = 0x112,
    /// Position + Normal + 2 UVs (XYZNUV2)
    XYZNUV2 = 0x212,
    /// Position + Normal + Diffuse + 1 UV (XYZNDUV1)
    XYZNDUV1 = 0x1112,
    /// Position + Normal + Diffuse + 2 UVs (XYZNDUV2)
    XYZNDUV2 = 0x2112,
    /// Position + Diffuse + 1 UV (XYZDUV1)
    XYZDUV1 = 0x1102,
    /// Position + Diffuse + 2 UVs (XYZDUV2)
    XYZDUV2 = 0x2102,
    /// Position + 1 UV (XYZUV1)
    XYZUV1 = 0x1002,
    /// Position + 2 UVs (XYZUV2)
    XYZUV2 = 0x2002,
    /// Position + Normal + Diffuse + 4 UVs with tangent/binormal (TG3)
    XYZNDUV1TG3 = 0x4112,
    /// Position + Normal + 3 UVs for displacement mapping
    XYZNUV2DMAP = 0x3112,
    /// Position + Normal + Diffuse for cube mapping
    XYZNDCUBEMAP = 0x0113,
}

impl FvfFormat {
    /// Get vertex stride (size in bytes) for this format
    pub fn stride(&self) -> u64 {
        match self {
            FvfFormat::XYZ => mem::size_of::<VertexXYZ>() as u64,
            FvfFormat::XYZN => mem::size_of::<VertexXYZN>() as u64,
            FvfFormat::XYZNUV1 => mem::size_of::<VertexXYZNUV1>() as u64,
            FvfFormat::XYZNUV2 => mem::size_of::<VertexXYZNUV2>() as u64,
            FvfFormat::XYZNDUV1 => mem::size_of::<VertexXYZNDUV1>() as u64,
            FvfFormat::XYZNDUV2 => mem::size_of::<VertexXYZNDUV2>() as u64,
            FvfFormat::XYZDUV1 => mem::size_of::<VertexXYZDUV1>() as u64,
            FvfFormat::XYZDUV2 => mem::size_of::<VertexXYZDUV2>() as u64,
            FvfFormat::XYZUV1 => mem::size_of::<VertexXYZUV1>() as u64,
            FvfFormat::XYZUV2 => mem::size_of::<VertexXYZUV2>() as u64,
            FvfFormat::XYZNDUV1TG3 => mem::size_of::<VertexXYZNDUV1TG3>() as u64,
            FvfFormat::XYZNUV2DMAP => mem::size_of::<VertexXYZNUV2DMAP>() as u64,
            FvfFormat::XYZNDCUBEMAP => mem::size_of::<VertexXYZNDCUBEMAP>() as u64,
        }
    }

    /// Convert FVF format to wgpu VertexBufferLayout
    pub fn to_vertex_buffer_layout(&self) -> VertexBufferLayout<'static> {
        match self {
            FvfFormat::XYZ => VertexXYZ::layout(),
            FvfFormat::XYZN => VertexXYZN::layout(),
            FvfFormat::XYZNUV1 => VertexXYZNUV1::layout(),
            FvfFormat::XYZNUV2 => VertexXYZNUV2::layout(),
            FvfFormat::XYZNDUV1 => VertexXYZNDUV1::layout(),
            FvfFormat::XYZNDUV2 => VertexXYZNDUV2::layout(),
            FvfFormat::XYZDUV1 => VertexXYZDUV1::layout(),
            FvfFormat::XYZDUV2 => VertexXYZDUV2::layout(),
            FvfFormat::XYZUV1 => VertexXYZUV1::layout(),
            FvfFormat::XYZUV2 => VertexXYZUV2::layout(),
            FvfFormat::XYZNDUV1TG3 => VertexXYZNDUV1TG3::layout(),
            FvfFormat::XYZNUV2DMAP => VertexXYZNUV2DMAP::layout(),
            FvfFormat::XYZNDCUBEMAP => VertexXYZNDCUBEMAP::layout(),
        }
    }

    /// Get format name for debugging
    pub fn name(&self) -> &'static str {
        match self {
            FvfFormat::XYZ => "XYZ",
            FvfFormat::XYZN => "XYZN",
            FvfFormat::XYZNUV1 => "XYZNUV1",
            FvfFormat::XYZNUV2 => "XYZNUV2",
            FvfFormat::XYZNDUV1 => "XYZNDUV1",
            FvfFormat::XYZNDUV2 => "XYZNDUV2",
            FvfFormat::XYZDUV1 => "XYZDUV1",
            FvfFormat::XYZDUV2 => "XYZDUV2",
            FvfFormat::XYZUV1 => "XYZUV1",
            FvfFormat::XYZUV2 => "XYZUV2",
            FvfFormat::XYZNDUV1TG3 => "XYZNDUV1TG3",
            FvfFormat::XYZNUV2DMAP => "XYZNUV2DMAP",
            FvfFormat::XYZNDCUBEMAP => "XYZNDCUBEMAP",
        }
    }
}

/// FVF information class - provides offset information for vertex components
#[derive(Debug, Clone)]
pub struct FvfInfo {
    pub format: FvfFormat,
    pub stride: u64,
    pub location_offset: u32,
    pub normal_offset: Option<u32>,
    pub texcoord_offsets: Vec<u32>,
    pub diffuse_offset: Option<u32>,
    pub specular_offset: Option<u32>,
}

impl FvfInfo {
    /// Create FvfInfo from format
    pub fn new(format: FvfFormat) -> Self {
        let (normal_offset, texcoord_offsets, diffuse_offset, specular_offset) = match format {
            FvfFormat::XYZ => (None, vec![], None, None),
            FvfFormat::XYZN => (Some(12), vec![], None, None),
            FvfFormat::XYZNUV1 => (Some(12), vec![24], None, None),
            FvfFormat::XYZNUV2 => (Some(12), vec![24, 32], None, None),
            FvfFormat::XYZNDUV1 => (Some(12), vec![28], Some(24), None),
            FvfFormat::XYZNDUV2 => (Some(12), vec![28, 36], Some(24), None),
            FvfFormat::XYZDUV1 => (None, vec![16], Some(12), None),
            FvfFormat::XYZDUV2 => (None, vec![16, 24], Some(12), None),
            FvfFormat::XYZUV1 => (None, vec![12], None, None),
            FvfFormat::XYZUV2 => (None, vec![12, 20], None, None),
            FvfFormat::XYZNDUV1TG3 => (Some(12), vec![28, 36, 48, 60], Some(24), None),
            FvfFormat::XYZNUV2DMAP => (Some(12), vec![24, 40, 48], None, None),
            FvfFormat::XYZNDCUBEMAP => (Some(12), vec![], Some(24), None),
        };

        Self {
            format,
            stride: format.stride(),
            location_offset: 0,
            normal_offset,
            texcoord_offsets,
            diffuse_offset,
            specular_offset,
        }
    }

    /// Get texture coordinate offset by index
    pub fn get_tex_offset(&self, index: usize) -> Option<u32> {
        self.texcoord_offsets.get(index).copied()
    }
}

// Vertex structure definitions matching C++ FVF formats

/// Position only (XYZ)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZ {
    pub position: [f32; 3],
}

impl VertexXYZ {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: VertexFormat::Float32x3,
        }];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal (XYZN)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZN {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl VertexXYZN {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal + 1 UV (XYZNUV1)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZNUV1 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub texcoord0: [f32; 2],
}

impl VertexXYZNUV1 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal + 2 UVs (XYZNUV2)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZNUV2 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub texcoord0: [f32; 2],
    pub texcoord1: [f32; 2],
}

impl VertexXYZNUV2 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Float32x2,
            },
            VertexAttribute {
                offset: 32,
                shader_location: 3,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal + Diffuse + 1 UV (XYZNDUV1) - Most common format
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZNDUV1 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub texcoord0: [f32; 2],
}

impl VertexXYZNDUV1 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Unorm8x4,
            },
            VertexAttribute {
                offset: 28,
                shader_location: 3,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal + Diffuse + 2 UVs (XYZNDUV2) - Dynamic buffer format
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZNDUV2 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub texcoord0: [f32; 2],
    pub texcoord1: [f32; 2],
}

impl VertexXYZNDUV2 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Unorm8x4,
            },
            VertexAttribute {
                offset: 28,
                shader_location: 3,
                format: VertexFormat::Float32x2,
            },
            VertexAttribute {
                offset: 36,
                shader_location: 4,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Diffuse + 1 UV (XYZDUV1)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZDUV1 {
    pub position: [f32; 3],
    pub diffuse: u32,
    pub texcoord0: [f32; 2],
}

impl VertexXYZDUV1 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Unorm8x4,
            },
            VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Diffuse + 2 UVs (XYZDUV2)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZDUV2 {
    pub position: [f32; 3],
    pub diffuse: u32,
    pub texcoord0: [f32; 2],
    pub texcoord1: [f32; 2],
}

impl VertexXYZDUV2 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Unorm8x4,
            },
            VertexAttribute {
                offset: 16,
                shader_location: 2,
                format: VertexFormat::Float32x2,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 3,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + 1 UV (XYZUV1)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZUV1 {
    pub position: [f32; 3],
    pub texcoord0: [f32; 2],
}

impl VertexXYZUV1 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + 2 UVs (XYZUV2)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZUV2 {
    pub position: [f32; 3],
    pub texcoord0: [f32; 2],
    pub texcoord1: [f32; 2],
}

impl VertexXYZUV2 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x2,
            },
            VertexAttribute {
                offset: 20,
                shader_location: 2,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal + Diffuse + 4 UVs with tangent/binormal (XYZNDUV1TG3)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZNDUV1TG3 {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub texcoord0: [f32; 2],
    pub tangent: [f32; 3],
    pub binormal: [f32; 3],
    pub tangent_cross_binormal: [f32; 3],
}

impl VertexXYZNDUV1TG3 {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Unorm8x4,
            },
            VertexAttribute {
                offset: 28,
                shader_location: 3,
                format: VertexFormat::Float32x2,
            },
            VertexAttribute {
                offset: 36,
                shader_location: 4,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 48,
                shader_location: 5,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 60,
                shader_location: 6,
                format: VertexFormat::Float32x3,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal + 3 UVs for displacement mapping (XYZNUV2DMAP)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZNUV2DMAP {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub texcoord0: f32,
    pub texcoord1: [f32; 4],
    pub texcoord2: [f32; 2],
}

impl VertexXYZNUV2DMAP {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Float32,
            },
            VertexAttribute {
                offset: 28,
                shader_location: 3,
                format: VertexFormat::Float32x4,
            },
            VertexAttribute {
                offset: 44,
                shader_location: 4,
                format: VertexFormat::Float32x2,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

/// Position + Normal + Diffuse for cube mapping (XYZNDCUBEMAP)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct VertexXYZNDCUBEMAP {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
}

impl VertexXYZNDCUBEMAP {
    pub fn layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: &[VertexAttribute] = &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: VertexFormat::Float32x3,
            },
            VertexAttribute {
                offset: 24,
                shader_location: 2,
                format: VertexFormat::Unorm8x4,
            },
        ];

        VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_sizes() {
        assert_eq!(mem::size_of::<VertexXYZ>(), 12);
        assert_eq!(mem::size_of::<VertexXYZN>(), 24);
        assert_eq!(mem::size_of::<VertexXYZNUV1>(), 32);
        assert_eq!(mem::size_of::<VertexXYZNUV2>(), 40);
        assert_eq!(mem::size_of::<VertexXYZNDUV1>(), 36);
        assert_eq!(mem::size_of::<VertexXYZNDUV2>(), 44);
    }

    #[test]
    fn test_fvf_stride() {
        assert_eq!(FvfFormat::XYZ.stride(), 12);
        assert_eq!(FvfFormat::XYZNDUV2.stride(), 44);
    }

    #[test]
    fn test_fvf_info() {
        let info = FvfInfo::new(FvfFormat::XYZNDUV2);
        assert_eq!(info.location_offset, 0);
        assert_eq!(info.normal_offset, Some(12));
        assert_eq!(info.diffuse_offset, Some(24));
        assert_eq!(info.get_tex_offset(0), Some(28));
        assert_eq!(info.get_tex_offset(1), Some(36));
    }

    #[test]
    fn test_vertex_layout_generation() {
        let layout = FvfFormat::XYZNDUV2.to_vertex_buffer_layout();
        assert_eq!(layout.array_stride, 44);
        assert_eq!(layout.attributes.len(), 5);
    }
}
