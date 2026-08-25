//! Renderer-owned contract for the C++ `W3DShroudMaterialPass`.
//!
//! The Main presentation layer owns the frozen R8 snapshot and its texture
//! lifetime.  The mesh renderer consumes this contract through its dedicated
//! rigid/skinned WGPU pass, while exact per-draw eligibility remains a frozen
//! presentation decision rather than a renderer-side simulation query.

use super::shader_system::shader::{
    ColorMaskType, CullModeType, DepthCompareType, DepthMaskType, DetailAlphaFuncType,
    DetailColorFuncType, DstBlendFuncType, FogFuncType, PriGradientType, SecGradientType,
    ShaderClass, SrcBlendFuncType, TexturingType,
};
use std::sync::Arc;

/// Frozen world-ground projection parameters paired with one presentation
/// shroud texture.  Rust render space uses X/Z for the ground plane, matching
/// Main's explicit adapter from the C++ X/Y shroud plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectedShroudProjection {
    /// `uv = world_xz * scale + offset`.
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
    /// Quantized C++ `GlobalData::m_shroudColor`, normalized for WGSL.
    pub shroud_color: [f32; 3],
    /// Identity of the complete frozen snapshot, including projection/tint.
    pub content_fingerprint: u64,
}

impl ProjectedShroudProjection {
    /// Build the exact `ShroudTextureShader::set` ground projection:
    /// `((world - origin) + cell_size) / (cell_size * texture_extent)`.
    pub fn from_cpp_grid(
        draw_origin_xy: [f32; 2],
        cell_size_xy: [f32; 2],
        texture_extent: (u32, u32),
        shroud_color_rgb: [u8; 3],
        content_fingerprint: u64,
    ) -> Option<Self> {
        let [cell_x, cell_y] = cell_size_xy;
        if texture_extent.0 == 0
            || texture_extent.1 == 0
            || !draw_origin_xy.into_iter().all(f32::is_finite)
            || !cell_size_xy
                .into_iter()
                .all(|value| value.is_finite() && value > 0.0)
        {
            return None;
        }
        let denominators = [
            cell_x * texture_extent.0 as f32,
            cell_y * texture_extent.1 as f32,
        ];
        if !denominators
            .into_iter()
            .all(|value| value.is_finite() && value > 0.0)
        {
            return None;
        }
        let uv_scale = [1.0 / denominators[0], 1.0 / denominators[1]];
        let uv_offset = [
            (-draw_origin_xy[0] + cell_x) * uv_scale[0],
            (-draw_origin_xy[1] + cell_y) * uv_scale[1],
        ];
        Some(Self {
            uv_scale,
            uv_offset,
            shroud_color: shroud_color_rgb.map(|channel| channel as f32 / 255.0),
            content_fingerprint,
        })
    }

    #[inline]
    pub fn uv_for_world_xz(self, world_x: f32, world_z: f32) -> [f32; 2] {
        [
            world_x * self.uv_scale[0] + self.uv_offset[0],
            world_z * self.uv_scale[1] + self.uv_offset[1],
        ]
    }
}

/// Renderer-owned handle to one immutable, presentation-uploaded R8 shroud.
///
/// The resource contains no simulation handle. Main may replace it only at a
/// frozen presentation-frame boundary; the mesh renderer retains the Arcs for
/// the duration of the WGPU frame.
#[derive(Clone)]
pub struct FrozenProjectedShroudTexture {
    texture_view: Arc<wgpu::TextureView>,
    sampler: Arc<wgpu::Sampler>,
    projection: ProjectedShroudProjection,
}

impl FrozenProjectedShroudTexture {
    pub fn new(
        texture_view: Arc<wgpu::TextureView>,
        sampler: Arc<wgpu::Sampler>,
        projection: ProjectedShroudProjection,
    ) -> Self {
        Self {
            texture_view,
            sampler,
            projection,
        }
    }

    #[inline]
    pub fn texture_view(&self) -> &Arc<wgpu::TextureView> {
        &self.texture_view
    }

    #[inline]
    pub fn sampler(&self) -> &Arc<wgpu::Sampler> {
        &self.sampler
    }

    #[inline]
    pub fn projection(&self) -> ProjectedShroudProjection {
        self.projection
    }
}

/// Fixed state selected by C++ `W3DShroudMaterialPass`.
///
/// The source pass is an additional material pass over geometry that already
/// populated the depth buffer. Its sampled shroud level is multiplied into
/// the destination color (`Zero / SrcColor`) and it must not write depth.
/// Eligibility is intentionally not represented here: that belongs to the
/// presentation-owned final `ObjectShroudStatus`/Drawable binding decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProjectedShroudMaterialPassContract {
    pub depth_compare: DepthCompareType,
    pub depth_write_enabled: bool,
    pub source_blend: SrcBlendFuncType,
    pub destination_blend: DstBlendFuncType,
    pub color_write_enabled: bool,
    pub texturing_enabled: bool,
}

impl ProjectedShroudMaterialPassContract {
    /// Exact state required by the retail W3D projected shroud pass.
    pub const CXX: Self = Self {
        depth_compare: DepthCompareType::Equal,
        depth_write_enabled: false,
        source_blend: SrcBlendFuncType::Zero,
        destination_blend: DstBlendFuncType::SrcColor,
        color_write_enabled: true,
        texturing_enabled: true,
    };

    /// Build the legacy shader bitfield for this pass.
    ///
    /// This is a state-only constructor. It does not bind a shroud texture,
    /// project world coordinates, or schedule a draw call.
    pub fn shader(self) -> ShaderClass {
        let bits = crate::rendering::shader_system::shader::shade_const(
            self.depth_compare as u32,
            if self.depth_write_enabled {
                DepthMaskType::Enable as u32
            } else {
                DepthMaskType::Disable as u32
            },
            if self.color_write_enabled {
                ColorMaskType::Enable as u32
            } else {
                ColorMaskType::Disable as u32
            },
            self.source_blend as u32,
            self.destination_blend as u32,
            FogFuncType::Disable as u32,
            PriGradientType::Disable as u32,
            SecGradientType::Disable as u32,
            if self.texturing_enabled {
                TexturingType::Enable as u32
            } else {
                TexturingType::Disable as u32
            },
            0,
            CullModeType::Enable as u32,
            DetailColorFuncType::Disable as u32,
            DetailAlphaFuncType::Disable as u32,
        );
        let mut shader = ShaderClass::new();
        shader.set_bits(bits);
        shader
    }
}

impl Default for ProjectedShroudMaterialPassContract {
    fn default() -> Self {
        Self::CXX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cxx_projected_shroud_contract_keeps_depth_and_multiplicative_state() {
        let contract = ProjectedShroudMaterialPassContract::CXX;
        assert_eq!(contract.depth_compare, DepthCompareType::Equal);
        assert!(!contract.depth_write_enabled);
        assert_eq!(contract.source_blend, SrcBlendFuncType::Zero);
        assert_eq!(contract.destination_blend, DstBlendFuncType::SrcColor);
        assert!(contract.color_write_enabled);
        assert!(contract.texturing_enabled);
    }

    #[test]
    fn cxx_projected_shroud_contract_encodes_the_expected_shader_bits() {
        let shader = ProjectedShroudMaterialPassContract::CXX.shader();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Equal);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::Disable);
        assert_eq!(shader.get_color_mask(), ColorMaskType::Enable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::Zero);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::SrcColor);
        assert_eq!(shader.get_texturing(), TexturingType::Enable);
    }

    #[test]
    fn projection_matches_cxx_nonzero_origin_and_rust_xz_adapter() {
        let projection = ProjectedShroudProjection::from_cpp_grid(
            [100.0, -50.0],
            [10.0, 20.0],
            (8, 4),
            [255, 127, 0],
            77,
        )
        .expect("valid projection");

        assert_eq!(projection.uv_for_world_xz(100.0, -50.0), [0.125, 0.25]);
        assert_eq!(projection.uv_for_world_xz(180.0, 30.0), [1.125, 1.25]);
        assert_eq!(projection.shroud_color, [1.0, 127.0 / 255.0, 0.0]);
        assert_eq!(projection.content_fingerprint, 77);
    }

    #[test]
    fn projection_rejects_malformed_frozen_geometry() {
        assert!(
            ProjectedShroudProjection::from_cpp_grid([0.0, 0.0], [0.0, 10.0], (8, 8), [255; 3], 1,)
                .is_none()
        );
        assert!(
            ProjectedShroudProjection::from_cpp_grid(
                [f32::NAN, 0.0],
                [10.0, 10.0],
                (8, 8),
                [255; 3],
                1,
            )
            .is_none()
        );
    }
}
