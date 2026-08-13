//! Renderer-owned contract for the C++ `W3DShroudMaterialPass`.
//!
//! This module deliberately describes the pass state without pretending that
//! a projected texture is already bound to the active WGPU material pipeline.
//! The Main presentation layer owns the frozen R8 snapshot and its texture
//! lifetime; a later integration must supply that texture and the exact
//! per-draw eligibility before invoking this contract.

use super::shader_system::shader::{
    ColorMaskType, CullModeType, DepthCompareType, DepthMaskType, DetailAlphaFuncType,
    DetailColorFuncType, DstBlendFuncType, FogFuncType, PriGradientType, SecGradientType,
    ShaderClass, SrcBlendFuncType, TexturingType,
};

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
}
