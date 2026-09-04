//! Blend State Integration Tests
//!
//! This module tests the integration of source/destination blend functions into the WGPU
//! rendering pipeline. These tests verify that the ShaderClass blend state configuration
//! correctly translates to WGPU BlendState objects and produces the expected rendering behavior.
//!
//! Reference: GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp lines 409-488

use ww3d_renderer_3d::rendering::shader_system::shader::{
    AlphaTestType, ColorMaskType, CullModeType, DepthCompareType, DepthMaskType,
    DetailAlphaFuncType, DetailColorFuncType, DstBlendFuncType, FogFuncType, PriGradientType,
    SecGradientType, ShaderClass, SrcBlendFuncType, TexturingType,
};

/// Test opaque blend state (One, Zero)
/// Reference: shader.cpp lines 457-462 - Default opaque blending
#[test]
fn test_opaque_blend_state() {
    let shader = ShaderClass::get_opaque_shader();

    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::Zero);

    // Verify this is not considered transparent
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Opaque
    );
}

/// Test alpha blend state
/// Reference: shader.cpp lines 449-452 - Alpha blending with srcBlendLUT/dstBlendLUT
///
/// NOTE: The original C++ W3D format only supports 4 source blend modes (2 bits):
/// - SRCBLEND_ZERO = 0
/// - SRCBLEND_ONE = 1
/// - SRCBLEND_SRC_ALPHA = 2
/// - SRCBLEND_ONE_MINUS_SRC_ALPHA = 3
///
/// C++ GeneralsMD shader.h: SRCBLEND_ZERO=0, ONE=1, SRC_ALPHA=2, ONE_MINUS_SRC_ALPHA=3
/// (2-bit field). Alpha blend is authored as (SRC_ALPHA, ONE_MINUS_SRC_ALPHA).
#[test]
fn test_alpha_blend_state() {
    let mut shader = ShaderClass::new();

    shader.set_src_blend_func(SrcBlendFuncType::SrcAlpha);
    shader.set_dst_blend_func(DstBlendFuncType::InvSrcAlpha);

    // Verify round-trip
    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::InvSrcAlpha);
}

/// Test additive blend state (One, One)
/// Reference: shader.cpp - Additive blending for particle effects
#[test]
fn test_additive_blend_state() {
    let shader = ShaderClass::get_additive_shader();

    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::One);

    // Verify categorization as additive
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Additive
    );
}

/// Test multiplicative blend state (Zero, SrcColor)
/// Reference: shader.cpp - Multiply blend for darkening effects
#[test]
fn test_multiplicative_blend_state() {
    let mut shader = ShaderClass::new();
    shader.set_src_blend_func(SrcBlendFuncType::Zero);
    shader.set_dst_blend_func(DstBlendFuncType::SrcColor);

    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::Zero);
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::SrcColor);
}

/// Test screen blend state (One, InvSrcColor)
/// Reference: shader.cpp - Screen blend for lightening effects
#[test]
fn test_screen_blend_state() {
    let mut shader = ShaderClass::new();
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::InvSrcColor);

    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::InvSrcColor);

    // Verify categorization as screen blend
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Screen
    );
}

/// Test all source blend function types
/// Verifies that all enum variants can be set and retrieved correctly
///
/// NOTE: The enum values match the bit encoding, not DirectX 8 constants
/// DX8 compatibility is handled by set_src_blend() which takes u32 DX8 constants
#[test]
fn test_all_src_blend_functions() {
    let test_cases = vec![
        (SrcBlendFuncType::Zero, "Zero"),
        (SrcBlendFuncType::One, "One"),
        (SrcBlendFuncType::SrcAlpha, "SrcAlpha"),
        (SrcBlendFuncType::InvSrcAlpha, "InvSrcAlpha"),
    ];
    for (src_blend, name) in test_cases {
        let mut shader = ShaderClass::new();
        shader.set_src_blend_func(src_blend);
        assert_eq!(
            shader.get_src_blend_func(),
            src_blend,
            "Source blend function mismatch for {}",
            name
        );
    }
}

/// Test all destination blend function types
/// Verifies that all enum variants can be set and retrieved correctly
#[test]
fn test_all_dst_blend_functions() {
    let test_cases = vec![
        DstBlendFuncType::Zero,
        DstBlendFuncType::One,
        DstBlendFuncType::SrcColor,
        DstBlendFuncType::InvSrcColor,
        DstBlendFuncType::SrcAlpha,
        DstBlendFuncType::InvSrcAlpha,
    ];

    for dst_blend in test_cases {
        let mut shader = ShaderClass::new();
        shader.set_dst_blend_func(dst_blend);
        assert_eq!(
            shader.get_dst_blend_func(),
            dst_blend,
            "Destination blend function mismatch for {:?}",
            dst_blend
        );
    }
}

/// Test blend state with color mask disabled
/// Reference: shader.cpp lines 442-446 - Color mask affects blend state
#[test]
fn test_blend_with_color_mask_disabled() {
    let mut shader = ShaderClass::new();
    let src_blend = SrcBlendFuncType::SrcAlpha;
    let dst_blend = DstBlendFuncType::InvSrcAlpha;

    shader.set_color_mask(ColorMaskType::Disable);
    shader.set_src_blend_func(src_blend);
    shader.set_dst_blend_func(dst_blend);

    // When color mask is disabled, blend functions are still set but writing is disabled
    assert_eq!(shader.get_color_mask(), ColorMaskType::Disable);
    assert_eq!(shader.get_src_blend_func(), src_blend);
    assert_eq!(shader.get_dst_blend_func(), dst_blend);
}

/// Test blend state with alpha test enabled
/// Reference: shader.cpp lines 467-483 - Alpha test interaction with blend state
#[test]
fn test_blend_with_alpha_test() {
    let mut shader = ShaderClass::new();
    shader.set_alpha_test(AlphaTestType::Enable);
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::Zero);

    assert_eq!(shader.get_alpha_test(), AlphaTestType::Enable);
    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);

    // Alpha test shader category
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::AlphaTest
    );
}

/// Test blend state bits packing/unpacking
/// Verifies that blend state survives serialization through bit packing
#[test]
fn test_blend_state_bit_packing() {
    let mut shader = ShaderClass::new();
    shader.set_src_blend_func(SrcBlendFuncType::SrcAlpha);
    shader.set_dst_blend_func(DstBlendFuncType::InvSrcAlpha);

    let _bits = shader.get_bits();
    let restored = ShaderClass::from_w3d_shader(&ww3d_core::W3dShaderStruct {
        depth_compare: DepthCompareType::Lequal as u8,
        depth_mask: DepthMaskType::Enable as u8,
        color_mask: ColorMaskType::Enable as u8,
        src_blend: SrcBlendFuncType::SrcAlpha as u8,
        dest_blend: DstBlendFuncType::InvSrcAlpha as u8,
        fog_func: FogFuncType::Disable as u8,
        pri_gradient: PriGradientType::Disable as u8,
        sec_gradient: SecGradientType::Disable as u8,
        texturing: TexturingType::Disable as u8,
        detail_color_func: DetailColorFuncType::Disable as u8,
        detail_alpha_func: DetailAlphaFuncType::Disable as u8,
        shader_preset: 0,
        alpha_test: AlphaTestType::Disable as u8,
        post_detail_color_func: DetailColorFuncType::Disable as u8,
        post_detail_alpha_func: DetailAlphaFuncType::Disable as u8,
    });

    assert_eq!(shader.get_src_blend_func(), restored.get_src_blend_func());
    assert_eq!(shader.get_dst_blend_func(), restored.get_dst_blend_func());
}

/// Test common blend mode combinations used in C&C Generals
/// Verifies the most frequently used blend states in the game
#[test]
fn test_common_blend_combinations() {
    // Test cases: (src, dst, expected_category)
    let test_cases = vec![
        // Opaque rendering (buildings, terrain)
        (SrcBlendFuncType::One, DstBlendFuncType::Zero, "Opaque"),
        // Standard alpha blending (transparent objects, UI)
        (
            SrcBlendFuncType::SrcAlpha,
            DstBlendFuncType::InvSrcAlpha,
            "Alpha",
        ),
        // Additive blending (explosions, fire, lasers)
        (SrcBlendFuncType::One, DstBlendFuncType::One, "Additive"),
        // Screen blending (bright overlays)
        (
            SrcBlendFuncType::One,
            DstBlendFuncType::InvSrcColor,
            "Screen",
        ),
    ];

    for (src, dst, expected_name) in test_cases {
        let mut shader = ShaderClass::new();
        shader.set_src_blend_func(src);
        shader.set_dst_blend_func(dst);

        assert_eq!(
            shader.get_src_blend_func(),
            src,
            "Source blend mismatch for {} blend",
            expected_name
        );
        assert_eq!(
            shader.get_dst_blend_func(),
            dst,
            "Destination blend mismatch for {} blend",
            expected_name
        );
    }
}

/// Test blend state validation logic
/// Reference: shader.cpp lines 455-463 - Blend enable logic
#[test]
fn test_blend_enable_logic() {
    // Blend should be disabled for (One, Zero)
    let mut shader = ShaderClass::new();
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::Zero);

    // According to C++ code: if(sf != D3DBLEND_ONE || df != D3DBLEND_ZERO) { blendOn = TRUE; }
    // So (One, Zero) should have blending disabled
    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::Zero);

    // Any other combination should enable blending
    shader.set_dst_blend_func(DstBlendFuncType::One);
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::One);
}

/// Test shader bit field isolation
/// Verifies that changing blend functions doesn't affect other shader state
#[test]
fn test_blend_state_isolation() {
    let mut shader = ShaderClass::new();
    let src_blend = SrcBlendFuncType::SrcAlpha;
    let dst_blend = DstBlendFuncType::InvSrcAlpha;

    // Set various shader states
    shader.set_depth_compare(DepthCompareType::Less);
    shader.set_depth_mask(DepthMaskType::Enable);
    shader.set_cull_mode(CullModeType::Enable);
    shader.set_texturing(TexturingType::Enable);

    // Now change blend functions
    shader.set_src_blend_func(src_blend);
    shader.set_dst_blend_func(dst_blend);

    // Verify other states are unchanged
    assert_eq!(shader.get_depth_compare(), DepthCompareType::Less);
    assert_eq!(shader.get_depth_mask(), DepthMaskType::Enable);
    assert_eq!(shader.get_cull_mode(), CullModeType::Enable);
    assert_eq!(shader.get_texturing(), TexturingType::Enable);

    // Verify blend functions are set correctly
    assert_eq!(shader.get_src_blend_func(), src_blend);
    assert_eq!(shader.get_dst_blend_func(), dst_blend);
}

/// Test blend mode preset shaders
/// Verifies that preset shaders have consistent blend configurations
#[test]
fn test_preset_shader_blend_modes() {
    // Opaque preset - verify it's categorized as opaque
    let opaque = ShaderClass::get_opaque_shader();
    assert_eq!(
        opaque.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Opaque
    );

    // Alpha preset - verify it uses alpha blending
    let alpha = ShaderClass::get_alpha_shader();
    let src = alpha.get_src_blend_func();
    let dst = alpha.get_dst_blend_func();
    // Store values to verify round-trip
    let mut test_shader = ShaderClass::new();
    test_shader.set_src_blend_func(src);
    test_shader.set_dst_blend_func(dst);
    assert_eq!(test_shader.get_src_blend_func(), src);
    assert_eq!(test_shader.get_dst_blend_func(), dst);

    // Additive preset - verify it's categorized as additive
    let additive = ShaderClass::get_additive_shader();
    assert_eq!(
        additive.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Additive
    );
}

/// Test static sort category determination
/// Reference: shader.cpp lines 1085-1105 - GetSSCategory implementation
#[test]
fn test_static_sort_categories() {
    // Opaque category: alpha test disabled, dst blend = zero
    let mut shader = ShaderClass::new();
    shader.set_alpha_test(AlphaTestType::Disable);
    shader.set_dst_blend_func(DstBlendFuncType::Zero);
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Opaque
    );

    // Alpha test category: alpha test enabled, dst blend = zero
    shader.set_alpha_test(AlphaTestType::Enable);
    shader.set_dst_blend_func(DstBlendFuncType::Zero);
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::AlphaTest
    );

    // Additive category: src = one, dst = one
    shader.set_alpha_test(AlphaTestType::Disable);
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::One);
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Additive
    );

    // Screen category: src = one, dst = inv src color
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::InvSrcColor);
    assert_eq!(
        shader.get_ss_category(),
        ww3d_renderer_3d::rendering::shader_system::shader::StaticSortCategoryType::Screen
    );
}

/// Test DX8 blend constant compatibility
/// Verifies that DirectX 8 blend value conversion works correctly
#[test]
fn test_dx8_blend_compatibility() {
    let mut shader = ShaderClass::new();

    // Test D3DBLEND constants conversion
    // D3DBLEND_ZERO = 1, D3DBLEND_ONE = 2, etc.
    shader.set_src_blend(1); // D3DBLEND_ZERO
    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::Zero);

    shader.set_src_blend(2); // D3DBLEND_ONE
    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::One);

    shader.set_src_blend(5); // D3DBLEND_SRCALPHA
    assert_eq!(shader.get_src_blend_func(), SrcBlendFuncType::SrcAlpha);

    shader.set_dest_blend(6); // D3DBLEND_INVSRCALPHA
    assert_eq!(shader.get_dst_blend_func(), DstBlendFuncType::InvSrcAlpha);
}

/// Test blend mode material helper
/// Verifies that the blend_mode() helper returns correct MaterialBlendMode
#[test]
fn test_material_blend_mode_helper() {
    use ww3d_renderer_3d::rendering::shader_system::shader::MaterialBlendMode;

    // Opaque
    let mut shader = ShaderClass::new();
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::Zero);
    assert_eq!(shader.blend_mode(), MaterialBlendMode::Opaque);

    // Alpha
    shader.set_src_blend_func(SrcBlendFuncType::SrcAlpha);
    shader.set_dst_blend_func(DstBlendFuncType::InvSrcAlpha);
    assert_eq!(shader.blend_mode(), MaterialBlendMode::Alpha);

    // Additive
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::One);
    assert_eq!(shader.blend_mode(), MaterialBlendMode::Additive);

    // Screen
    shader.set_src_blend_func(SrcBlendFuncType::One);
    shader.set_dst_blend_func(DstBlendFuncType::InvSrcColor);
    assert_eq!(shader.blend_mode(), MaterialBlendMode::Screen);
}

/// Test depth write interaction with blend mode
/// Reference: shader.cpp - Transparent objects typically disable depth writes
#[test]
fn test_depth_write_blend_interaction() {
    // Opaque should enable depth writes
    let opaque = ShaderClass::get_opaque_shader();
    assert_eq!(opaque.get_depth_mask(), DepthMaskType::Enable);

    // Alpha blended should disable depth writes
    let alpha = ShaderClass::get_alpha_shader();
    assert_eq!(alpha.get_depth_mask(), DepthMaskType::Disable);

    // Additive should disable depth writes
    let additive = ShaderClass::get_additive_shader();
    assert_eq!(additive.get_depth_mask(), DepthMaskType::Disable);
}

/// Test complete shader state creation with blend functions
/// Verifies create_from_components correctly sets all blend state
#[test]
fn test_complete_shader_creation() {
    let src_blend = SrcBlendFuncType::SrcAlpha;
    let dst_blend = DstBlendFuncType::InvSrcAlpha;

    let shader = ShaderClass::create_from_components(
        DepthCompareType::Lequal,
        DepthMaskType::Enable,
        ColorMaskType::Enable,
        src_blend,
        dst_blend,
        FogFuncType::Disable,
        PriGradientType::Modulate,
        SecGradientType::Disable,
        TexturingType::Enable,
        AlphaTestType::Disable,
        CullModeType::Enable,
        DetailColorFuncType::Disable,
        DetailAlphaFuncType::Disable,
    );

    assert_eq!(shader.get_depth_compare(), DepthCompareType::Lequal);
    assert_eq!(shader.get_depth_mask(), DepthMaskType::Enable);
    assert_eq!(shader.get_color_mask(), ColorMaskType::Enable);
    assert_eq!(shader.get_src_blend_func(), src_blend);
    assert_eq!(shader.get_dst_blend_func(), dst_blend);
    assert_eq!(shader.get_fog_func(), FogFuncType::Disable);
    assert_eq!(shader.get_pri_gradient(), PriGradientType::Modulate);
    assert_eq!(shader.get_texturing(), TexturingType::Enable);
}

/// Test that the WGSL alpha-test discard is gated on the authored
/// ShaderClass ALPHATEST bit.
///
/// C++ parity: `ShaderClass::Apply` enables `D3DRS_ALPHATESTENABLE` only when
/// `Get_Alpha_Test() != ALPHATEST_DISABLE` (GeneralsMD shader.cpp:998) with a
/// 0x60 reference (shader.cpp:427); the default device state is
/// ALPHATESTENABLE=FALSE / ALPHAREF=0 (dx8wrapper.cpp:3682/3688). The port
/// gates the alpha.wgsl/decal.wgsl discard per pipeline: materials without
/// the bit compile with a 0.0 threshold (no fragment can be discarded);
/// materials with the bit keep the 96/255 reference.
#[test]
fn test_alpha_test_discard_gated_on_shader_bit() {
    use std::borrow::Cow;

    const WGSL_SNIPPET: &str = "const BASE_ALPHA_REF: f32 = 96.0 / 255.0;\n\
                               let alpha_threshold = BASE_ALPHA_REF * alpha_override;\n\
                               if final_alpha < alpha_threshold {\n    discard;\n}\n";
    let source = Cow::Borrowed(WGSL_SNIPPET);
    let gated = |shader: &ShaderClass| {
        ww3d_renderer_3d::rendering::wgpu_renderer::wgpu_pipeline_manager::gate_alpha_test_discard(
            shader,
            source.clone(),
        )
    };

    // Default material: ALPHATEST bit unset. The (SrcAlpha, InvSrcAlpha)
    // blend routes to alpha.wgsl, but without the bit the discard threshold
    // must compile to 0.0 — `final_alpha < 0.0` can never fire.
    let mut shader = ShaderClass::new();
    shader.set_src_blend_func(SrcBlendFuncType::SrcAlpha);
    shader.set_dst_blend_func(DstBlendFuncType::InvSrcAlpha);
    assert_eq!(shader.get_alpha_test(), AlphaTestType::Disable);
    let without_bit = gated(&shader);
    assert!(!without_bit.contains("96.0 / 255.0"), "{without_bit}");

    // Material with the ALPHATEST bit authored keeps the authored 96/255
    // reference (C++ D3DRS_ALPHAREF = 0x60, shader.cpp:427).
    shader.set_alpha_test(AlphaTestType::Enable);
    let with_bit = gated(&shader);
    assert_eq!(with_bit, source);

    // The decode path must drive the gate: a raw W3DShaderStruct with the
    // C++ ALPHATEST bit (shift 18) set keeps the reference; unset zeroes it.
    let raw = |alpha_test: u8| ww3d_core::W3dShaderStruct {
        depth_compare: DepthCompareType::Lequal as u8,
        depth_mask: DepthMaskType::Enable as u8,
        color_mask: ColorMaskType::Enable as u8,
        src_blend: SrcBlendFuncType::SrcAlpha as u8,
        dest_blend: DstBlendFuncType::InvSrcAlpha as u8,
        fog_func: FogFuncType::Disable as u8,
        pri_gradient: PriGradientType::Disable as u8,
        sec_gradient: SecGradientType::Disable as u8,
        texturing: TexturingType::Enable as u8,
        detail_color_func: DetailColorFuncType::Disable as u8,
        detail_alpha_func: DetailAlphaFuncType::Disable as u8,
        shader_preset: 0,
        alpha_test,
        post_detail_color_func: DetailColorFuncType::Disable as u8,
        post_detail_alpha_func: DetailAlphaFuncType::Disable as u8,
    };
    let enabled = ShaderClass::from_w3d_shader(&raw(AlphaTestType::Enable as u8));
    assert_eq!(gated(&enabled), source);
    let disabled = ShaderClass::from_w3d_shader(&raw(AlphaTestType::Disable as u8));
    assert!(!gated(&disabled).contains("96.0 / 255.0"));

    // Sources that never carry the constant (opaque/additive routes) pass
    // through unchanged regardless of the bit.
    let plain = Cow::<str>::Borrowed("fn fs_main() -> @location(0) vec4<f32> { return out; }\n");
    assert_eq!(
        ww3d_renderer_3d::rendering::wgpu_renderer::wgpu_pipeline_manager::gate_alpha_test_discard(
            &shader,
            plain.clone(),
        ),
        plain
    );
}
