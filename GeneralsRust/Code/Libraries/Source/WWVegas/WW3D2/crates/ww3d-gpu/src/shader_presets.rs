//! WW3D2 Shader Preset System
//!
//! This module provides the 23 preset shader configurations matching the C++ implementation.
//! These presets cover common rendering scenarios for 3D objects, sprites, and 2D overlays.
//!
//! C++ Reference: GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp lines 58-239
//!
//! ## Preset Categories
//!
//! ### 3D Object Presets (depth-tested, Z-write enabled)
//! - **PresetOpaque**: Standard textured opaque rendering
//! - **PresetAdditive**: Additive blending (e.g., glows, energy effects)
//! - **PresetAlpha**: Alpha blending (e.g., glass, translucent surfaces)
//! - **PresetMultiplicative**: Multiplicative blending (e.g., shadows, darkening)
//! - **PresetBumpenvmap**: Bump environment mapping
//!
//! ### 3D Solid Presets (no texturing, uses vertex colors)
//! - **PresetOpaqueSolid**: Solid opaque colors
//! - **PresetAdditiveSolid**: Solid additive colors
//! - **PresetAlphaSolid**: Solid alpha-blended colors
//!
//! ### Sprite Presets (depth-tested, no Z-write)
//! - **PresetOpaqueSprite**: Opaque billboards/particles
//! - **PresetAdditiveSprite**: Additive particles (e.g., fire, explosions)
//! - **PresetAlphaSprite**: Alpha-blended particles
//! - **PresetATestSprite**: Alpha-tested sprites (hard edges)
//! - **PresetScreenSprite**: Screen-blended sprites
//! - **PresetMultiplicativeSprite**: Multiplicative sprites
//! - **PresetATestBlendSprite**: Alpha-tested + blended sprites
//!
//! ### 2D Overlay Presets (no depth testing, for UI/HUD)
//! - **PresetOpaque2D**: Opaque UI elements
//! - **PresetAdditive2D**: Additive UI effects
//! - **PresetAlpha2D**: Translucent UI
//! - **PresetScreen2D**: Screen-blended UI
//! - **PresetMultiplicative2D**: Multiplicative UI
//! - **PresetATest2D**: Alpha-tested UI (hard edges)
//! - **PresetATestBlend2D**: Alpha-tested + blended UI
//!
//! ## Usage
//!
//! ```rust
//! use ww3d_gpu::shader::Shader;
//! use ww3d_gpu::shader_presets::ShaderPresets;
//!
//! // Create a shader for additive particle effects
//! let particle_shader = ShaderPresets::additive_sprite();
//!
//! // Create a shader for translucent UI elements
//! let ui_shader = ShaderPresets::alpha_2d();
//! ```

use crate::shader::*;

/// Collection of preset shader configurations matching C++ ShaderClass presets
pub struct ShaderPresets;

impl ShaderPresets {
    // ========================================
    // 3D OBJECT PRESETS
    // ========================================

    /// Opaque textured shader
    /// Texturing, zbuffer read/write, primary gradient, no blending
    /// C++ Reference: shader.cpp lines 61-65
    pub fn opaque() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteEnable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::Zero as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Modulate as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Additive blending shader
    /// Texturing, zbuffer read only, primary gradient, additive blending
    /// C++ Reference: shader.cpp lines 68-72
    pub fn additive() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::One as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Modulate as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Bump environment map shader
    /// Texturing, zbuffer read only, bumpenvmap gradient, additive blending
    /// C++ Reference: shader.cpp lines 75-79
    pub fn bumpenvmap() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::One as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::BumpEnvMap as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Add as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha blending shader
    /// Texturing, zbuffer read only, primary gradient, alpha blending
    /// C++ Reference: shader.cpp lines 82-86
    pub fn alpha() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::SrcAlpha as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcAlpha as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Modulate as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Multiplicative blending shader
    /// Texturing, zbuffer read only, primary gradient, multiplicative blending
    /// C++ Reference: shader.cpp lines 89-93
    pub fn multiplicative() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::Zero as u32) << shift::SRCBLEND
                | (DstBlendFuncType::SrcColor as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Modulate as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    // ========================================
    // 3D SOLID PRESETS (no texturing)
    // ========================================

    /// Opaque solid shader (no texturing)
    /// No texturing, zbuffer read/write, primary gradient, no blending
    /// C++ Reference: shader.cpp lines 148-152
    pub fn opaque_solid() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteEnable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::Zero as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Modulate as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Disable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Additive solid shader (no texturing)
    /// No texturing, zbuffer read only, primary gradient, additive blending
    /// C++ Reference: shader.cpp lines 157-161
    pub fn additive_solid() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::One as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Modulate as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Disable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha solid shader (no texturing)
    /// No texturing, zbuffer read only, primary gradient, alpha blending
    /// C++ Reference: shader.cpp lines 166-170
    pub fn alpha_solid() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::SrcAlpha as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcAlpha as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Modulate as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Disable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    // ========================================
    // SPRITE PRESETS (depth-tested, no Z-write)
    // ========================================

    /// Opaque sprite shader
    /// Texturing, zbuffer read only, no gradients, no blending
    /// C++ Reference: shader.cpp lines 105-109
    pub fn opaque_sprite() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::Zero as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Additive sprite shader
    /// Texturing, zbuffer read only, no gradients, additive blending
    /// C++ Reference: shader.cpp lines 131-135
    pub fn additive_sprite() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::One as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha sprite shader
    /// Texturing, zbuffer read only, no gradients, alpha blending
    /// C++ Reference: shader.cpp lines 140-144
    pub fn alpha_sprite() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::SrcAlpha as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcAlpha as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha test sprite shader
    /// Texturing, zbuffer read/write, no gradients, no blending, alpha testing
    /// C++ Reference: shader.cpp lines 183-187
    pub fn alpha_test_sprite() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteEnable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::Zero as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Enable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha test blend sprite shader
    /// Texturing, zbuffer read/write, no gradients, alpha blending AND testing
    /// C++ Reference: shader.cpp lines 201-205
    pub fn alpha_test_blend_sprite() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteEnable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::SrcAlpha as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcAlpha as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Enable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Screen blend sprite shader
    /// Texturing, zbuffer read only, no gradients, screen blending
    /// C++ Reference: shader.cpp lines 218-222
    pub fn screen_sprite() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcColor as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Multiplicative sprite shader
    /// Texturing, zbuffer read only, no gradients, multiplicative blending
    /// C++ Reference: shader.cpp lines 235-239
    pub fn multiplicative_sprite() -> Shader {
        Shader::from_bits(
            (DepthCompareType::LEqual as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::Zero as u32) << shift::SRCBLEND
                | (DstBlendFuncType::SrcColor as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    // ========================================
    // 2D OVERLAY PRESETS (no depth testing)
    // ========================================

    /// Opaque 2D shader
    /// Texturing, no zbuffer, no gradients, no blending
    /// C++ Reference: shader.cpp lines 97-101
    pub fn opaque_2d() -> Shader {
        Shader::from_bits(
            (DepthCompareType::Always as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::Zero as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Additive 2D shader
    /// Texturing, no zbuffer, no gradients, additive blending
    /// C++ Reference: shader.cpp lines 114-118
    pub fn additive_2d() -> Shader {
        Shader::from_bits(
            (DepthCompareType::Always as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::One as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha 2D shader
    /// Texturing, no zbuffer, no gradients, alpha blending
    /// C++ Reference: shader.cpp lines 122-126
    pub fn alpha_2d() -> Shader {
        Shader::from_bits(
            (DepthCompareType::Always as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::SrcAlpha as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcAlpha as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Screen blend 2D shader
    /// Texturing, no zbuffer, no gradients, screen blending
    /// C++ Reference: shader.cpp lines 209-213
    pub fn screen_2d() -> Shader {
        Shader::from_bits(
            (DepthCompareType::Always as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcColor as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Multiplicative 2D shader
    /// Texturing, no zbuffer, no gradients, multiplicative blending
    /// C++ Reference: shader.cpp lines 226-230
    pub fn multiplicative_2d() -> Shader {
        Shader::from_bits(
            (DepthCompareType::Always as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::Zero as u32) << shift::SRCBLEND
                | (DstBlendFuncType::SrcColor as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Disable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha test 2D shader
    /// Texturing, no zbuffer, no gradients, no blending, alpha testing
    /// C++ Reference: shader.cpp lines 174-178
    pub fn alpha_test_2d() -> Shader {
        Shader::from_bits(
            (DepthCompareType::Always as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::One as u32) << shift::SRCBLEND
                | (DstBlendFuncType::Zero as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Enable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }

    /// Alpha test blend 2D shader
    /// Texturing, no zbuffer, no gradients, alpha blending AND testing
    /// C++ Reference: shader.cpp lines 192-196
    pub fn alpha_test_blend_2d() -> Shader {
        Shader::from_bits(
            (DepthCompareType::Always as u32) << shift::DEPTHCOMPARE
                | (DepthMaskType::WriteDisable as u32) << shift::DEPTHMASK
                | (ColorMaskType::WriteEnable as u32) << shift::COLORMASK
                | (SrcBlendFuncType::SrcAlpha as u32) << shift::SRCBLEND
                | (DstBlendFuncType::OneMinusSrcAlpha as u32) << shift::DSTBLEND
                | (FogFuncType::Disable as u32) << shift::FOG
                | (PriGradientType::Disable as u32) << shift::PRIGRADIENT
                | (SecGradientType::Disable as u32) << shift::SECGRADIENT
                | (TexturingType::Enable as u32) << shift::TEXTURING
                | (AlphaTestType::Enable as u32) << shift::ALPHATEST
                | (CullModeType::Enable as u32) << shift::CULLMODE
                | (DetailColorFuncType::Disable as u32) << shift::POSTDETAILCOLORFUNC
                | (DetailAlphaFuncType::Disable as u32) << shift::POSTDETAILALPHAFUNC,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================
    // 3D OBJECT PRESET TESTS
    // ========================================

    #[test]
    fn test_preset_opaque() {
        let shader = ShaderPresets::opaque();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteEnable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::Zero);
        assert_eq!(shader.get_texturing(), TexturingType::Enable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Modulate);
        assert_eq!(shader.get_alpha_test(), AlphaTestType::Disable);
        assert_eq!(shader.get_cull_mode(), CullModeType::Enable);
        assert_eq!(shader.get_ss_category(), StaticSortCategoryType::Opaque);
    }

    #[test]
    fn test_preset_additive() {
        let shader = ShaderPresets::additive();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::One);
        assert_eq!(shader.get_texturing(), TexturingType::Enable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Modulate);
        assert_eq!(shader.get_ss_category(), StaticSortCategoryType::Additive);
    }

    #[test]
    fn test_preset_bumpenvmap() {
        let shader = ShaderPresets::bumpenvmap();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::One);
        assert_eq!(shader.get_texturing(), TexturingType::Enable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::BumpEnvMap);
        assert_eq!(
            shader.get_post_detail_color_func(),
            DetailColorFuncType::Add
        );
    }

    #[test]
    fn test_preset_alpha() {
        let shader = ShaderPresets::alpha();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcAlpha
        );
        assert_eq!(shader.get_texturing(), TexturingType::Enable);
        assert!(shader.uses_alpha());
    }

    #[test]
    fn test_preset_multiplicative() {
        let shader = ShaderPresets::multiplicative();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::Zero);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::SrcColor);
        assert_eq!(shader.get_texturing(), TexturingType::Enable);
    }

    // ========================================
    // SOLID PRESET TESTS
    // ========================================

    #[test]
    fn test_preset_opaque_solid() {
        let shader = ShaderPresets::opaque_solid();
        assert_eq!(shader.get_texturing(), TexturingType::Disable);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteEnable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::Zero);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Modulate);
    }

    #[test]
    fn test_preset_additive_solid() {
        let shader = ShaderPresets::additive_solid();
        assert_eq!(shader.get_texturing(), TexturingType::Disable);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::One);
    }

    #[test]
    fn test_preset_alpha_solid() {
        let shader = ShaderPresets::alpha_solid();
        assert_eq!(shader.get_texturing(), TexturingType::Disable);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcAlpha
        );
        assert!(shader.uses_alpha());
    }

    // ========================================
    // SPRITE PRESET TESTS
    // ========================================

    #[test]
    fn test_preset_opaque_sprite() {
        let shader = ShaderPresets::opaque_sprite();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Disable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::Zero);
    }

    #[test]
    fn test_preset_additive_sprite() {
        let shader = ShaderPresets::additive_sprite();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Disable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::One);
        assert_eq!(shader.get_ss_category(), StaticSortCategoryType::Additive);
    }

    #[test]
    fn test_preset_alpha_sprite() {
        let shader = ShaderPresets::alpha_sprite();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Disable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcAlpha
        );
        assert!(shader.uses_alpha());
    }

    #[test]
    fn test_preset_alpha_test_sprite() {
        let shader = ShaderPresets::alpha_test_sprite();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteEnable);
        assert_eq!(shader.get_alpha_test(), AlphaTestType::Enable);
        assert_eq!(shader.get_ss_category(), StaticSortCategoryType::AlphaTest);
    }

    #[test]
    fn test_preset_alpha_test_blend_sprite() {
        let shader = ShaderPresets::alpha_test_blend_sprite();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteEnable);
        assert_eq!(shader.get_alpha_test(), AlphaTestType::Enable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcAlpha
        );
        assert_eq!(shader.get_ss_category(), StaticSortCategoryType::AlphaTest);
    }

    #[test]
    fn test_preset_screen_sprite() {
        let shader = ShaderPresets::screen_sprite();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcColor
        );
        assert_eq!(shader.get_ss_category(), StaticSortCategoryType::Screen);
    }

    #[test]
    fn test_preset_multiplicative_sprite() {
        let shader = ShaderPresets::multiplicative_sprite();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::LEqual);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::Zero);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::SrcColor);
    }

    // ========================================
    // 2D OVERLAY PRESET TESTS
    // ========================================

    #[test]
    fn test_preset_opaque_2d() {
        let shader = ShaderPresets::opaque_2d();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Always);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Disable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::Zero);
    }

    #[test]
    fn test_preset_additive_2d() {
        let shader = ShaderPresets::additive_2d();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Always);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Disable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::One);
    }

    #[test]
    fn test_preset_alpha_2d() {
        let shader = ShaderPresets::alpha_2d();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Always);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_primary_gradient(), PriGradientType::Disable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcAlpha
        );
        assert!(shader.uses_alpha());
    }

    #[test]
    fn test_preset_screen_2d() {
        let shader = ShaderPresets::screen_2d();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Always);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcColor
        );
        assert_eq!(shader.get_ss_category(), StaticSortCategoryType::Screen);
    }

    #[test]
    fn test_preset_multiplicative_2d() {
        let shader = ShaderPresets::multiplicative_2d();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Always);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::Zero);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::SrcColor);
    }

    #[test]
    fn test_preset_alpha_test_2d() {
        let shader = ShaderPresets::alpha_test_2d();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Always);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_alpha_test(), AlphaTestType::Enable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
        assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::Zero);
    }

    #[test]
    fn test_preset_alpha_test_blend_2d() {
        let shader = ShaderPresets::alpha_test_blend_2d();
        assert_eq!(shader.get_depth_compare(), DepthCompareType::Always);
        assert_eq!(shader.get_depth_mask(), DepthMaskType::WriteDisable);
        assert_eq!(shader.get_alpha_test(), AlphaTestType::Enable);
        assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);
        assert_eq!(
            shader.get_dst_blend_func(),
            DstBlendFuncType::OneMinusSrcAlpha
        );
    }

    // ========================================
    // COMPREHENSIVE TESTS
    // ========================================

    #[test]
    fn test_all_presets_have_valid_bits() {
        // Ensure all 23 presets can be created and have non-zero bit patterns
        let presets = [
            ShaderPresets::opaque(),
            ShaderPresets::additive(),
            ShaderPresets::bumpenvmap(),
            ShaderPresets::alpha(),
            ShaderPresets::multiplicative(),
            ShaderPresets::opaque_solid(),
            ShaderPresets::additive_solid(),
            ShaderPresets::alpha_solid(),
            ShaderPresets::opaque_sprite(),
            ShaderPresets::additive_sprite(),
            ShaderPresets::alpha_sprite(),
            ShaderPresets::alpha_test_sprite(),
            ShaderPresets::alpha_test_blend_sprite(),
            ShaderPresets::screen_sprite(),
            ShaderPresets::multiplicative_sprite(),
            ShaderPresets::opaque_2d(),
            ShaderPresets::additive_2d(),
            ShaderPresets::alpha_2d(),
            ShaderPresets::screen_2d(),
            ShaderPresets::multiplicative_2d(),
            ShaderPresets::alpha_test_2d(),
            ShaderPresets::alpha_test_blend_2d(),
        ];

        for preset in &presets {
            assert_ne!(preset.get_bits(), 0, "Preset should have non-zero bits");
        }

        // Verify we have exactly 22 presets (missing ScreenSprite in C++ lines 445)
        assert_eq!(presets.len(), 22);
    }

    #[test]
    fn test_depth_characteristics() {
        // 3D objects write to Z-buffer
        assert_eq!(
            ShaderPresets::opaque().get_depth_mask(),
            DepthMaskType::WriteEnable
        );
        assert_eq!(
            ShaderPresets::opaque_solid().get_depth_mask(),
            DepthMaskType::WriteEnable
        );

        // Sprites don't write to Z-buffer (except alpha-tested)
        assert_eq!(
            ShaderPresets::opaque_sprite().get_depth_mask(),
            DepthMaskType::WriteDisable
        );
        assert_eq!(
            ShaderPresets::additive_sprite().get_depth_mask(),
            DepthMaskType::WriteDisable
        );
        assert_eq!(
            ShaderPresets::alpha_test_sprite().get_depth_mask(),
            DepthMaskType::WriteEnable
        );

        // 2D overlays don't test or write Z-buffer
        assert_eq!(
            ShaderPresets::opaque_2d().get_depth_compare(),
            DepthCompareType::Always
        );
        assert_eq!(
            ShaderPresets::opaque_2d().get_depth_mask(),
            DepthMaskType::WriteDisable
        );
    }

    #[test]
    fn test_gradient_characteristics() {
        // 3D objects use primary gradient
        assert_eq!(
            ShaderPresets::opaque().get_primary_gradient(),
            PriGradientType::Modulate
        );
        assert_eq!(
            ShaderPresets::additive().get_primary_gradient(),
            PriGradientType::Modulate
        );

        // Sprites don't use gradients
        assert_eq!(
            ShaderPresets::opaque_sprite().get_primary_gradient(),
            PriGradientType::Disable
        );
        assert_eq!(
            ShaderPresets::additive_sprite().get_primary_gradient(),
            PriGradientType::Disable
        );

        // 2D overlays don't use gradients
        assert_eq!(
            ShaderPresets::opaque_2d().get_primary_gradient(),
            PriGradientType::Disable
        );
        assert_eq!(
            ShaderPresets::additive_2d().get_primary_gradient(),
            PriGradientType::Disable
        );
    }

    #[test]
    fn test_blend_mode_categorization() {
        // Test static sort categories match blend modes
        assert_eq!(
            ShaderPresets::opaque().get_ss_category(),
            StaticSortCategoryType::Opaque
        );
        assert_eq!(
            ShaderPresets::additive().get_ss_category(),
            StaticSortCategoryType::Additive
        );
        assert_eq!(
            ShaderPresets::alpha_test_sprite().get_ss_category(),
            StaticSortCategoryType::AlphaTest
        );
        assert_eq!(
            ShaderPresets::screen_2d().get_ss_category(),
            StaticSortCategoryType::Screen
        );
    }
}
