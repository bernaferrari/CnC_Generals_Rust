// Shader System
// Ported from shader.h

use crate::w3d_file::*;

#[derive(Debug, Clone, Copy)]
pub struct Shader {
    pub depth_compare: u8,
    pub depth_mask: u8,
    pub dest_blend: u8,
    pub pri_gradient: u8,
    pub sec_gradient: u8,
    pub src_blend: u8,
    pub texturing: u8,
    pub detail_color_func: u8,
    pub detail_alpha_func: u8,
    pub alpha_test: u8,
}

impl Default for Shader {
    fn default() -> Self {
        Self {
            depth_compare: W3DShaderDepthCompare::PassLEqual as u8,
            depth_mask: W3DShaderDepthMask::WriteEnable as u8,
            dest_blend: W3DShaderDestBlendFunc::Zero as u8,
            pri_gradient: W3DShaderPriGradient::Modulate as u8,
            sec_gradient: 0,
            src_blend: W3DShaderSrcBlendFunc::One as u8,
            texturing: 0,
            detail_color_func: 0,
            detail_alpha_func: 0,
            alpha_test: 0,
        }
    }
}
