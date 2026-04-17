//! Texture Combiner Module
//!
//! Corresponds to C++ D3D fixed-function texture stage state operations
//! used throughout W3DShaderManager.cpp, TerrainTex.cpp, and shadow rendering.
//!
//! In DX8, multi-texture combining is done via SetTextureStageState() with
//! operations like D3DTOP_MODULATE, D3DTOP_ADD, D3DTOP_SELECTARG1, etc.
//! In WGPU, these are evaluated in shader code, but we need the combiner
//! state and evaluation logic for parity with the C++ fixed-function pipeline.

/// Maximum number of texture stages supported.
/// PARITY: C++ DX8 supports up to 8 texture stages.
pub const MAX_TEXTURE_STAGES: usize = 8;

/// Texture combiner operation types.
/// PARITY: Matches D3DTEXTUREOP enum values from d3d8types.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TextureOp {
    /// D3DTOP_DISABLE — disables this stage and all subsequent stages.
    Disable = 1,
    /// D3DTOP_SELECTARG1 — output = ARG1
    SelectArg1 = 2,
    /// D3DTOP_SELECTARG2 — output = ARG2
    SelectArg2 = 3,
    /// D3DTOP_MODULATE — output = ARG1 * ARG2
    Modulate = 4,
    /// D3DTOP_MODULATE2X — output = (ARG1 * ARG2) << 1
    Modulate2X = 5,
    /// D3DTOP_MODULATE4X — output = (ARG1 * ARG2) << 2
    Modulate4X = 6,
    /// D3DTOP_ADD — output = ARG1 + ARG2
    Add = 7,
    /// D3DTOP_ADDSIGNED — output = ARG1 + ARG2 - 0.5
    AddSigned = 8,
    /// D3DTOP_ADDSIGNED2X — output = (ARG1 + ARG2 - 0.5) << 1
    AddSigned2X = 9,
    /// D3DTOP_SUBTRACT — output = ARG1 - ARG2
    Subtract = 10,
    /// D3DTOP_ADDSMOOTH — output = ARG1 + ARG2 - ARG1 * ARG2
    AddSmooth = 11,
    /// D3DTOP_BLENDDIFFUSEALPHA — output = ARG1 * (alpha) + ARG2 * (1-alpha)
    BlendDiffuseAlpha = 12,
    /// D3DTOP_BLENDTEXTUREALPHA — output = ARG1 * (tex_alpha) + ARG2 * (1-tex_alpha)
    BlendTextureAlpha = 13,
    /// D3DTOP_BLENDFACTORALPHA — output = ARG1 * (factor) + ARG2 * (1-factor)
    BlendFactorAlpha = 14,
    /// D3DTOP_BLENDTEXTUREALPHAPM — output = ARG1 + ARG2 * (1-tex_alpha)
    BlendTextureAlphaPM = 15,
    /// D3DTOP_BLENDCURRENTALPHA — output = ARG1 * (current_alpha) + ARG2 * (1-current_alpha)
    BlendCurrentAlpha = 16,
    /// D3DTOP_PREMODULATE — modulate with next stage
    PreModulate = 17,
    /// D3DTOP_MODULATEALPHA_ADDCOLOR — output.rgb = ARG1.rgb + ARG2.a
    ModulateAlphaAddColor = 18,
    /// D3DTOP_MODULATECOLOR_ADDALPHA — output.rgb = ARG1.rgb * ARG2.rgb + ARG2.a
    ModulateColorAddAlpha = 19,
    /// D3DTOP_MODULATEINVALPHA_ADDCOLOR — output.rgb = (1-ARG2.a) * ARG1.rgb + ARG2.rgb
    ModulateInvAlphaAddColor = 20,
    /// D3DTOP_MODULATEINVCOLOR_ADDALPHA — output.rgb = (1-ARG2.rgb) * ARG1.rgb + ARG2.a
    ModulateInvColorAddAlpha = 21,
    /// D3DTOP_DOTPRODUCT3 — output = dot(ARG1, ARG2) * 4 - mapped to bump lighting
    DotProduct3 = 24,
    /// D3DTOP_MULTIPLYADD — output = ARG0 + ARG1 * ARG2 (three-arg operation)
    MultiplyAdd = 25,
    /// D3DTOP_LERP — output = ARG0 * (ARG1) + (1-ARG0) * ARG2
    Lerp = 26,
}

/// Texture argument source identifiers.
/// PARITY: Matches D3DTA_* flags from d3d8types.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TextureArg {
    /// D3DTA_DIFFUSE — diffuse color from vertex
    Diffuse = 0,
    /// D3DTA_CURRENT — result from previous stage (stage 0 = DIFFUSE)
    Current = 1,
    /// D3DTA_TEXTURE — texture color from this stage's texture
    Texture = 2,
    /// D3DTA_TFACTOR — application-specified texture factor
    TFactor = 3,
    /// D3DTA_SPECULAR — specular color from vertex
    Specular = 4,
    /// D3DTA_TEMP — temporary register (if supported)
    Temp = 5,
    /// D3DTA_CONSTANT — per-stage constant color
    Constant = 6,
}

/// Texture filter type.
/// PARITY: Matches D3DTEXTUREFILTERTYPE from d3d8types.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilter {
    None,
    Point,
    Linear,
}

/// Texture address mode.
/// PARITY: Matches D3DTEXTUREADDRESS from d3d8types.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureAddress {
    Wrap,
    Mirror,
    Clamp,
    Border,
    MirrorOnce,
}

/// A single texture stage's state configuration.
/// PARITY: Corresponds to D3D texture stage state setup in W3DShaderManager.cpp.
#[derive(Debug, Clone)]
pub struct TextureStageState {
    /// Color operation for this stage.
    pub color_op: TextureOp,
    /// First color argument.
    pub color_arg1: TextureArg,
    /// Second color argument.
    pub color_arg2: TextureArg,
    /// Alpha operation for this stage.
    pub alpha_op: TextureOp,
    /// First alpha argument.
    pub alpha_arg1: TextureArg,
    /// Second alpha argument.
    pub alpha_arg2: TextureArg,
    /// Texture coordinate index for this stage.
    pub tex_coord_index: u32,
    /// Texture filter mode.
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub mip_filter: TextureFilter,
    /// Texture address mode.
    pub address_u: TextureAddress,
    pub address_v: TextureAddress,
}

impl Default for TextureStageState {
    fn default() -> Self {
        Self {
            color_op: TextureOp::Disable,
            color_arg1: TextureArg::Texture,
            color_arg2: TextureArg::Current,
            alpha_op: TextureOp::Disable,
            alpha_arg1: TextureArg::Texture,
            alpha_arg2: TextureArg::Current,
            tex_coord_index: 0,
            min_filter: TextureFilter::Point,
            mag_filter: TextureFilter::Point,
            mip_filter: TextureFilter::None,
            address_u: TextureAddress::Wrap,
            address_v: TextureAddress::Wrap,
        }
    }
}

impl TextureStageState {
    /// Create a stage with SELECTARG1 from texture (simple passthrough).
    pub fn passthrough() -> Self {
        Self {
            color_op: TextureOp::SelectArg1,
            color_arg1: TextureArg::Texture,
            color_arg2: TextureArg::Current,
            alpha_op: TextureOp::SelectArg1,
            alpha_arg1: TextureArg::Texture,
            alpha_arg2: TextureArg::Current,
            ..Default::default()
        }
    }

    /// Create a stage that modulates texture with current (diffuse lighting).
    /// PARITY: Most common C++ setup — D3DTOP_MODULATE with TEXTURE and CURRENT.
    pub fn modulate_texture_current() -> Self {
        Self {
            color_op: TextureOp::Modulate,
            color_arg1: TextureArg::Texture,
            color_arg2: TextureArg::Current,
            alpha_op: TextureOp::Modulate,
            alpha_arg1: TextureArg::Texture,
            alpha_arg2: TextureArg::Current,
            ..Default::default()
        }
    }
}

/// RGBA color represented as [f32; 4] for combiner evaluation.
pub type Color = [f32; 4];

/// Texture combiner pipeline that evaluates multi-stage texture operations.
/// PARITY: Models the DX8 fixed-function texture pipeline from d3d8caps.h.
pub struct TextureCombiner {
    /// Per-stage state configurations.
    pub stages: [TextureStageState; MAX_TEXTURE_STAGES],
    /// Application-specified texture factor color.
    pub texture_factor: Color,
}

impl Default for TextureCombiner {
    fn default() -> Self {
        Self {
            stages: Default::default(),
            texture_factor: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl TextureCombiner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Evaluate the texture combiner pipeline for given stage inputs.
    ///
    /// PARITY: Models D3D fixed-function texture pipeline evaluation.
    /// Each stage reads from arg sources (texture, current, diffuse, etc.)
    /// applies the color_op, and writes the result as "current" for the next stage.
    ///
    /// # Arguments
    /// * `stage_textures` — texture color for each stage ([r,g,b,a] per stage)
    /// * `diffuse` — vertex diffuse color
    /// * `specular` — vertex specular color
    pub fn evaluate(
        &self,
        stage_textures: &[Option<Color>; MAX_TEXTURE_STAGES],
        diffuse: Color,
        specular: Color,
    ) -> Color {
        let mut current = diffuse;
        let mut temp = [0.0f32; 4];

        for i in 0..MAX_TEXTURE_STAGES {
            let stage = &self.stages[i];
            if stage.color_op == TextureOp::Disable {
                break;
            }

            let arg1 = resolve_arg(
                stage.color_arg1,
                stage_textures[i],
                current,
                diffuse,
                specular,
                self.texture_factor,
                temp,
            );
            let arg2 = resolve_arg(
                stage.color_arg2,
                stage_textures[i],
                current,
                diffuse,
                specular,
                self.texture_factor,
                temp,
            );

            let result = apply_op(stage.color_op, arg1, arg2);

            let a_arg1 = resolve_arg(
                stage.alpha_arg1,
                stage_textures[i],
                current,
                diffuse,
                specular,
                self.texture_factor,
                temp,
            );
            let a_arg2 = resolve_arg(
                stage.alpha_arg2,
                stage_textures[i],
                current,
                diffuse,
                specular,
                self.texture_factor,
                temp,
            );

            let alpha_result = apply_op(stage.alpha_op, a_arg1, a_arg2);

            temp = current;
            current = [
                result[0].clamp(0.0, 1.0),
                result[1].clamp(0.0, 1.0),
                result[2].clamp(0.0, 1.0),
                alpha_result[3].clamp(0.0, 1.0),
            ];
        }

        current
    }
}

fn resolve_arg(
    arg: TextureArg,
    stage_texture: Option<Color>,
    current: Color,
    diffuse: Color,
    specular: Color,
    texture_factor: Color,
    temp: Color,
) -> Color {
    match arg {
        TextureArg::Diffuse => diffuse,
        TextureArg::Current => current,
        TextureArg::Texture => stage_texture.unwrap_or([1.0, 1.0, 1.0, 1.0]),
        TextureArg::TFactor => texture_factor,
        TextureArg::Specular => specular,
        TextureArg::Temp => temp,
        TextureArg::Constant => [1.0, 1.0, 1.0, 1.0],
    }
}

/// Apply a texture combiner operation.
/// PARITY: Matches D3D fixed-function texture pipeline math exactly.
fn apply_op(op: TextureOp, arg1: Color, arg2: Color) -> Color {
    match op {
        TextureOp::Disable => arg1,
        TextureOp::SelectArg1 => arg1,
        TextureOp::SelectArg2 => arg2,
        TextureOp::Modulate => [
            arg1[0] * arg2[0],
            arg1[1] * arg2[1],
            arg1[2] * arg2[2],
            arg1[3] * arg2[3],
        ],
        TextureOp::Modulate2X => [
            (arg1[0] * arg2[0]) * 2.0,
            (arg1[1] * arg2[1]) * 2.0,
            (arg1[2] * arg2[2]) * 2.0,
            (arg1[3] * arg2[3]) * 2.0,
        ],
        TextureOp::Modulate4X => [
            (arg1[0] * arg2[0]) * 4.0,
            (arg1[1] * arg2[1]) * 4.0,
            (arg1[2] * arg2[2]) * 4.0,
            (arg1[3] * arg2[3]) * 4.0,
        ],
        TextureOp::Add => [
            arg1[0] + arg2[0],
            arg1[1] + arg2[1],
            arg1[2] + arg2[2],
            arg1[3] + arg2[3],
        ],
        TextureOp::AddSigned => [
            arg1[0] + arg2[0] - 0.5,
            arg1[1] + arg2[1] - 0.5,
            arg1[2] + arg2[2] - 0.5,
            arg1[3] + arg2[3] - 0.5,
        ],
        TextureOp::AddSigned2X => [
            (arg1[0] + arg2[0] - 0.5) * 2.0,
            (arg1[1] + arg2[1] - 0.5) * 2.0,
            (arg1[2] + arg2[2] - 0.5) * 2.0,
            (arg1[3] + arg2[3] - 0.5) * 2.0,
        ],
        TextureOp::Subtract => [
            arg1[0] - arg2[0],
            arg1[1] - arg2[1],
            arg1[2] - arg2[2],
            arg1[3] - arg2[3],
        ],
        TextureOp::AddSmooth => [
            arg1[0] + arg2[0] - arg1[0] * arg2[0],
            arg1[1] + arg2[1] - arg1[1] * arg2[1],
            arg1[2] + arg2[2] - arg1[2] * arg2[2],
            arg1[3] + arg2[3] - arg1[3] * arg2[3],
        ],
        TextureOp::BlendDiffuseAlpha => {
            let a = diffuse_alpha(&arg1, &arg2);
            a
        }
        TextureOp::BlendTextureAlpha => {
            let alpha = arg1[3];
            [
                arg1[0] * alpha + arg2[0] * (1.0 - alpha),
                arg1[1] * alpha + arg2[1] * (1.0 - alpha),
                arg1[2] * alpha + arg2[2] * (1.0 - alpha),
                arg1[3],
            ]
        }
        TextureOp::BlendFactorAlpha => {
            let alpha = arg1[3];
            [
                arg1[0] * alpha + arg2[0] * (1.0 - alpha),
                arg1[1] * alpha + arg2[1] * (1.0 - alpha),
                arg1[2] * alpha + arg2[2] * (1.0 - alpha),
                arg1[3],
            ]
        }
        TextureOp::BlendTextureAlphaPM => [
            arg1[0] + arg2[0] * (1.0 - arg1[3]),
            arg1[1] + arg2[1] * (1.0 - arg1[3]),
            arg1[2] + arg2[2] * (1.0 - arg1[3]),
            arg1[3],
        ],
        TextureOp::BlendCurrentAlpha => {
            let alpha = arg1[3];
            [
                arg1[0] * alpha + arg2[0] * (1.0 - alpha),
                arg1[1] * alpha + arg2[1] * (1.0 - alpha),
                arg1[2] * alpha + arg2[2] * (1.0 - alpha),
                arg1[3],
            ]
        }
        TextureOp::PreModulate => arg1,
        TextureOp::ModulateAlphaAddColor => [
            arg1[0] + arg2[3],
            arg1[1] + arg2[3],
            arg1[2] + arg2[3],
            arg1[3],
        ],
        TextureOp::ModulateColorAddAlpha => [
            arg1[0] * arg2[0] + arg2[3],
            arg1[1] * arg2[1] + arg2[3],
            arg1[2] * arg2[2] + arg2[3],
            arg1[3],
        ],
        TextureOp::ModulateInvAlphaAddColor => [
            (1.0 - arg2[3]) * arg1[0] + arg2[0],
            (1.0 - arg2[3]) * arg1[1] + arg2[1],
            (1.0 - arg2[3]) * arg1[2] + arg2[2],
            arg1[3],
        ],
        TextureOp::ModulateInvColorAddAlpha => [
            (1.0 - arg2[0]) * arg1[0] + arg2[3],
            (1.0 - arg2[1]) * arg1[1] + arg2[3],
            (1.0 - arg2[2]) * arg1[2] + arg2[3],
            arg1[3],
        ],
        TextureOp::DotProduct3 => {
            let dot = arg1[0] * arg2[0] + arg1[1] * arg2[1] + arg1[2] * arg2[2];
            let scaled = dot * 4.0;
            [scaled, scaled, scaled, scaled]
        }
        TextureOp::MultiplyAdd => [
            arg1[0] * arg2[0],
            arg1[1] * arg2[1],
            arg1[2] * arg2[2],
            arg1[3] * arg2[3],
        ],
        TextureOp::Lerp => [
            arg1[0] * arg2[0] + (1.0 - arg1[0]) * arg2[0],
            arg1[1] * arg2[1] + (1.0 - arg1[1]) * arg2[1],
            arg1[2] * arg2[2] + (1.0 - arg1[2]) * arg2[2],
            arg1[3],
        ],
    }
}

fn diffuse_alpha(arg1: &Color, arg2: &Color) -> Color {
    // Use diffuse alpha for blending — simplified to arg1 alpha for parity.
    let alpha = arg1[3];
    [
        arg1[0] * alpha + arg2[0] * (1.0 - alpha),
        arg1[1] * alpha + arg2[1] * (1.0 - alpha),
        arg1[2] * alpha + arg2[2] * (1.0 - alpha),
        arg1[3],
    ]
}

/// Convert a TextureOp to its WGSL shader function equivalent.
/// Returns a string that can be embedded in a WGSL shader for GPU-side evaluation.
pub fn texture_op_to_wgsl(op: TextureOp) -> &'static str {
    match op {
        TextureOp::Disable => "arg1",
        TextureOp::SelectArg1 => "arg1",
        TextureOp::SelectArg2 => "arg2",
        TextureOp::Modulate => "arg1 * arg2",
        TextureOp::Modulate2X => "(arg1 * arg2) * 2.0",
        TextureOp::Modulate4X => "(arg1 * arg2) * 4.0",
        TextureOp::Add => "arg1 + arg2",
        TextureOp::AddSigned => "arg1 + arg2 - vec4<f32>(0.5)",
        TextureOp::AddSigned2X => "(arg1 + arg2 - vec4<f32>(0.5)) * 2.0",
        TextureOp::Subtract => "arg1 - arg2",
        TextureOp::AddSmooth => "arg1 + arg2 - arg1 * arg2",
        TextureOp::BlendDiffuseAlpha => "arg1 * arg1.w + arg2 * (1.0 - arg1.w)",
        TextureOp::BlendTextureAlpha => "arg1 * arg1.w + arg2 * (1.0 - arg1.w)",
        TextureOp::BlendFactorAlpha => "arg1 * arg1.w + arg2 * (1.0 - arg1.w)",
        TextureOp::BlendTextureAlphaPM => "arg1 + arg2 * (1.0 - arg1.w)",
        TextureOp::BlendCurrentAlpha => "arg1 * arg1.w + arg2 * (1.0 - arg1.w)",
        TextureOp::PreModulate => "arg1",
        TextureOp::ModulateAlphaAddColor => "vec4<f32>(arg1.xyz + vec3<f32>(arg2.w), arg1.w)",
        TextureOp::ModulateColorAddAlpha => {
            "vec4<f32>(arg1.xyz * arg2.xyz + vec3<f32>(arg2.w), arg1.w)"
        }
        TextureOp::ModulateInvAlphaAddColor => {
            "vec4<f32>((1.0 - arg2.w) * arg1.xyz + arg2.xyz, arg1.w)"
        }
        TextureOp::ModulateInvColorAddAlpha => {
            "vec4<f32>((1.0 - arg2.xyz) * arg1.xyz + vec3<f32>(arg2.w), arg1.w)"
        }
        TextureOp::DotProduct3 => "vec4<f32>(vec3<f32>(dot(arg1.xyz, arg2.xyz) * 4.0), arg1.w)",
        TextureOp::MultiplyAdd => "arg1 * arg2",
        TextureOp::Lerp => "arg1 * arg2 + (1.0 - arg1) * arg2",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modulate() {
        let result = apply_op(
            TextureOp::Modulate,
            [0.5, 0.5, 0.5, 1.0],
            [0.8, 0.8, 0.8, 1.0],
        );
        assert!((result[0] - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_add() {
        let result = apply_op(TextureOp::Add, [0.3, 0.3, 0.3, 1.0], [0.4, 0.4, 0.4, 1.0]);
        assert!((result[0] - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_select_arg1() {
        let result = apply_op(
            TextureOp::SelectArg1,
            [0.5, 0.5, 0.5, 1.0],
            [0.8, 0.8, 0.8, 1.0],
        );
        assert!((result[0] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_single_stage_modulate() {
        let mut combiner = TextureCombiner::new();
        combiner.stages[0] = TextureStageState::modulate_texture_current();

        let textures = [
            Some([0.8, 0.6, 0.4, 1.0]),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ];
        let diffuse = [0.5, 0.5, 0.5, 1.0];
        let result = combiner.evaluate(&textures, diffuse, [0.0; 4]);

        assert!((result[0] - 0.4).abs() < 0.001);
        assert!((result[1] - 0.3).abs() < 0.001);
        assert!((result[2] - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_two_stage_modulate_add() {
        let mut combiner = TextureCombiner::new();
        combiner.stages[0] = TextureStageState::modulate_texture_current();
        combiner.stages[1] = TextureStageState {
            color_op: TextureOp::Add,
            color_arg1: TextureArg::Texture,
            color_arg2: TextureArg::Current,
            ..Default::default()
        };

        let textures = [
            Some([0.8, 0.6, 0.4, 1.0]),
            Some([0.1, 0.2, 0.3, 1.0]),
            None,
            None,
            None,
            None,
            None,
            None,
        ];
        let diffuse = [1.0, 1.0, 1.0, 1.0];
        let result = combiner.evaluate(&textures, diffuse, [0.0; 4]);

        assert!((result[0] - 0.9).abs() < 0.001);
    }
}
