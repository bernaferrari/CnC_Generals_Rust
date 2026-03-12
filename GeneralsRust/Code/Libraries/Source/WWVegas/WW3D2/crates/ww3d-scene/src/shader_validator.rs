//! Comprehensive Shader Validation System
//!
//! This module validates shader state combinations against GPU capabilities,
//! matching the C++ behavior from shader.cpp:
//! - Blend mode compatibility (shader.cpp:438-463)
//! - Fog compatibility (shader.cpp:280-327)
//! - Texture operation capabilities (shader.cpp:590-652)
//! - Hardware capability checking
//!
//! C++ References:
//! - shader.cpp:280-327 (Enable_Fog, fog compatibility with blending)
//! - shader.cpp:438-463 (Apply, blend mode setup)
//! - shader.cpp:590-652 (Texture operation capability checks)

use std::collections::HashMap;

// ============================================================================
// ENUMERATIONS AND TYPES
// ============================================================================

/// Blend function types - matches C++ ShaderClass enums
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SrcBlendFunc {
    Zero,
    One,
    SrcAlpha,
    InvSrcAlpha,
    DestColor,
}

/// Destination blend function types - matches C++ ShaderClass enums
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DstBlendFunc {
    Zero,
    One,
    SrcColor,
    InvSrcColor,
    SrcAlpha,
    InvSrcAlpha,
}

/// Fog function types - matches C++ ShaderClass FOG_* enums
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogFunc {
    Disable,
    Enable,
    ScaleFragment,
    White,
}

/// Gradient/texture operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientFunc {
    Disable,
    Modulate,
    Add,
    BumpEnvMap,
    BumpEnvMapLuminance,
    Modulate2X,
}

/// Detail color operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailColorFunc {
    Disable,
    Detail,
    Scale,
    InvScale,
    Add,
    Sub,
    SubR,
    Blend,
    DetailBlend,
    AddSigned,
    AddSigned2X,
    Scale2X,
    ModAlphaAddColor,
}

/// GPU capability flags - represents hardware texture operations
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    pub supports_add: bool,
    pub supports_modulate: bool,
    pub supports_modulate2x: bool,
    pub supports_select_arg1: bool,
    pub supports_bumpenvmap: bool,
    pub supports_bumpenvmap_luminance: bool,
    pub supports_addsmooth: bool,
    pub supports_subtract: bool,
    pub supports_blend_texture_alpha: bool,
    pub supports_blend_current_alpha: bool,
    pub supports_add_signed: bool,
    pub supports_add_signed2x: bool,
    pub supports_mod_alpha_add_color: bool,
    pub supports_fog: bool,
}

impl Default for GpuCapabilities {
    fn default() -> Self {
        // Modern GPUs support all operations
        Self {
            supports_add: true,
            supports_modulate: true,
            supports_modulate2x: true,
            supports_select_arg1: true,
            supports_bumpenvmap: true,
            supports_bumpenvmap_luminance: true,
            supports_addsmooth: true,
            supports_subtract: true,
            supports_blend_texture_alpha: true,
            supports_blend_current_alpha: true,
            supports_add_signed: true,
            supports_add_signed2x: true,
            supports_mod_alpha_add_color: true,
            supports_fog: true,
        }
    }
}

/// Severity level for validation messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValidationSeverity {
    Info,
    Warning,
    Error,
}

/// A single validation message
#[derive(Debug, Clone)]
pub struct ValidationMessage {
    pub severity: ValidationSeverity,
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

impl ValidationMessage {
    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Info,
            code: code.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            code: code.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            code: code.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Validation result containing all messages and fallback recommendations
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub messages: Vec<ValidationMessage>,
    pub is_valid: bool,
    pub fallback_suggestion: Option<ShaderFallback>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            is_valid: true,
            fallback_suggestion: None,
        }
    }

    pub fn add_message(&mut self, msg: ValidationMessage) {
        if msg.severity == ValidationSeverity::Error {
            self.is_valid = false;
        }
        self.messages.push(msg);
    }

    pub fn with_fallback(mut self, fallback: ShaderFallback) -> Self {
        self.fallback_suggestion = Some(fallback);
        self
    }

    /// Count messages by severity
    pub fn count_by_severity(&self) -> HashMap<ValidationSeverity, usize> {
        let mut counts: HashMap<ValidationSeverity, usize> = HashMap::new();
        for msg in &self.messages {
            *counts.entry(msg.severity).or_insert(0) += 1;
        }
        counts
    }

    /// Get all messages of a specific severity
    pub fn get_messages_by_severity(
        &self,
        severity: ValidationSeverity,
    ) -> Vec<&ValidationMessage> {
        self.messages
            .iter()
            .filter(|msg| msg.severity == severity)
            .collect()
    }

    /// Print all messages for debugging
    pub fn print_messages(&self) {
        for msg in &self.messages {
            let severity_str = match msg.severity {
                ValidationSeverity::Info => "INFO",
                ValidationSeverity::Warning => "WARN",
                ValidationSeverity::Error => "ERROR",
            };
            eprintln!("[{}] {}: {}", severity_str, msg.code, msg.message);
            if let Some(suggestion) = &msg.suggestion {
                eprintln!("  Suggestion: {}", suggestion);
            }
        }
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Fallback shader configuration when validation fails
#[derive(Debug, Clone)]
pub struct ShaderFallback {
    pub src_blend: SrcBlendFunc,
    pub dst_blend: DstBlendFunc,
    pub fog_func: FogFunc,
    pub gradient: GradientFunc,
    pub reason: String,
}

impl ShaderFallback {
    /// Create a safe fallback for opaque rendering
    pub fn opaque() -> Self {
        Self {
            src_blend: SrcBlendFunc::One,
            dst_blend: DstBlendFunc::Zero,
            fog_func: FogFunc::Enable,
            gradient: GradientFunc::Modulate,
            reason: "Fallback to opaque rendering".to_string(),
        }
    }

    /// Create a safe fallback for alpha blending
    pub fn alpha_blend() -> Self {
        Self {
            src_blend: SrcBlendFunc::SrcAlpha,
            dst_blend: DstBlendFunc::InvSrcAlpha,
            fog_func: FogFunc::Enable,
            gradient: GradientFunc::Modulate,
            reason: "Fallback to alpha blending".to_string(),
        }
    }

    /// Create a safe fallback for additive blending
    pub fn additive() -> Self {
        Self {
            src_blend: SrcBlendFunc::One,
            dst_blend: DstBlendFunc::One,
            fog_func: FogFunc::ScaleFragment,
            gradient: GradientFunc::Modulate,
            reason: "Fallback to additive blending".to_string(),
        }
    }
}

// ============================================================================
// SHADER VALIDATOR
// ============================================================================

/// Comprehensive shader validator
///
/// Validates shader state combinations and provides helpful error messages
/// with automatic fallback suggestions.
pub struct ShaderValidator {
    gpu_capabilities: GpuCapabilities,
}

impl ShaderValidator {
    /// Create a new shader validator with default GPU capabilities
    pub fn new() -> Self {
        Self {
            gpu_capabilities: GpuCapabilities::default(),
        }
    }

    /// Create a shader validator with custom GPU capabilities
    pub fn with_capabilities(capabilities: GpuCapabilities) -> Self {
        Self {
            gpu_capabilities: capabilities,
        }
    }

    /// Get current GPU capabilities
    pub fn get_capabilities(&self) -> &GpuCapabilities {
        &self.gpu_capabilities
    }

    // ========================================================================
    // BLEND MODE VALIDATION
    // ========================================================================

    /// Validate blend mode compatibility
    /// C++ Reference: shader.cpp:438-463
    pub fn validate_blend_mode(
        &self,
        src_blend: SrcBlendFunc,
        dst_blend: DstBlendFunc,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Check for unsupported blend combinations
        match (src_blend, dst_blend) {
            // Valid combinations that use alpha
            (SrcBlendFunc::SrcAlpha, DstBlendFunc::InvSrcAlpha) => {
                result.add_message(ValidationMessage::info(
                    "BLEND_001",
                    "Alpha blend: standard alpha blending mode (SRC_ALPHA, INV_SRC_ALPHA)",
                ));
            }
            // Valid combinations for additive
            (SrcBlendFunc::One, DstBlendFunc::One) => {
                result.add_message(ValidationMessage::info(
                    "BLEND_002",
                    "Additive blend: additive blending (ONE, ONE)",
                ));
            }
            // Valid combinations for multiplicative
            (SrcBlendFunc::Zero, DstBlendFunc::SrcColor) => {
                result.add_message(ValidationMessage::info(
                    "BLEND_003",
                    "Multiplicative blend: (ZERO, SRC_COLOR)",
                ));
            }
            // Valid combinations for opaque
            (SrcBlendFunc::One, DstBlendFunc::Zero) => {
                result.add_message(ValidationMessage::info(
                    "BLEND_004",
                    "Opaque rendering: (ONE, ZERO)",
                ));
            }
            // Valid screen blend
            (SrcBlendFunc::One, DstBlendFunc::InvSrcColor) => {
                result.add_message(ValidationMessage::info(
                    "BLEND_005",
                    "Screen blend: (ONE, ONE_MINUS_SRC_COLOR)",
                ));
            }
            // Warn about less common but valid combinations
            (SrcBlendFunc::SrcAlpha, DstBlendFunc::SrcAlpha) => {
                result.add_message(
                    ValidationMessage::warning(
                        "BLEND_WARN_001",
                        "Unusual blend: (SRC_ALPHA, SRC_ALPHA) - may produce unexpected results",
                    )
                    .with_suggestion(
                        "Consider using (SRC_ALPHA, INV_SRC_ALPHA) for standard alpha blending",
                    ),
                );
            }
            // Invalid/problematic combinations
            (SrcBlendFunc::Zero, DstBlendFunc::Zero) => {
                result.add_message(
                    ValidationMessage::error(
                        "BLEND_ERR_001",
                        "Invalid blend mode: (ZERO, ZERO) produces no output",
                    )
                    .with_suggestion(
                        "Use (ONE, ZERO) for opaque or (SRC_ALPHA, INV_SRC_ALPHA) for blended",
                    ),
                );
            }
            (SrcBlendFunc::Zero, DstBlendFunc::One) => {
                result.add_message(
                    ValidationMessage::warning(
                        "BLEND_WARN_002",
                        "Blend mode (ZERO, ONE) ignores source completely",
                    )
                    .with_suggestion(
                        "This is rarely intended; verify this is the desired behavior",
                    ),
                );
            }
            _ => {
                result.add_message(
                    ValidationMessage::warning(
                        "BLEND_WARN_003",
                        format!(
                            "Uncommon blend combination: ({:?}, {:?})",
                            src_blend, dst_blend
                        ),
                    )
                    .with_suggestion("Ensure this blend mode produces the intended visual effect"),
                );
            }
        }

        result
    }

    // ========================================================================
    // FOG COMPATIBILITY VALIDATION
    // ========================================================================

    /// Validate fog compatibility with blend mode
    /// C++ Reference: shader.cpp:280-327 (Enable_Fog)
    pub fn validate_fog_compatibility(
        &self,
        src_blend: SrcBlendFunc,
        dst_blend: DstBlendFunc,
        requested_fog: FogFunc,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();

        if requested_fog == FogFunc::Disable {
            result.add_message(ValidationMessage::info("FOG_001", "Fog disabled"));
            return result;
        }

        // Determine what fog mode is compatible with the blend mode
        let compatible_fog = self.get_compatible_fog_mode(src_blend, dst_blend);

        match compatible_fog {
            Some(fog_mode) => {
                if fog_mode != requested_fog {
                    result.add_message(ValidationMessage::warning(
                        "FOG_WARN_001",
                        format!(
                            "Requested fog mode ({:?}) is incompatible with blend mode ({:?}, {:?})",
                            requested_fog, src_blend, dst_blend
                        ),
                    )
                    .with_suggestion(format!(
                        "Use fog mode {:?} instead for this blend combination",
                        fog_mode
                    )));
                    result.fallback_suggestion = Some(ShaderFallback {
                        src_blend,
                        dst_blend,
                        fog_func: fog_mode,
                        gradient: GradientFunc::Modulate,
                        reason: format!("Fog adjusted from {:?} to {:?}", requested_fog, fog_mode),
                    });
                    // Warning issued but still valid - can use the corrected fog mode
                } else {
                    result.add_message(ValidationMessage::info(
                        "FOG_002",
                        format!("Fog mode {:?} is compatible with blend mode", fog_mode),
                    ));
                }
            }
            None => {
                result.add_message(ValidationMessage::error(
                    "FOG_ERR_001",
                    format!(
                        "Cannot enable fog with blend mode ({:?}, {:?})",
                        src_blend, dst_blend
                    ),
                )
                .with_suggestion("Use opaque (ONE, ZERO), alpha (SRC_ALPHA, INV_SRC_ALPHA), or additive (ONE, ONE) blending with fog"));
                result.fallback_suggestion = Some(ShaderFallback {
                    src_blend,
                    dst_blend,
                    fog_func: FogFunc::Disable,
                    gradient: GradientFunc::Modulate,
                    reason: "Fog disabled due to incompatible blend mode".to_string(),
                });
            }
        }

        result
    }

    /// Determine the appropriate fog mode for a blend combination
    /// C++ Reference: shader.cpp:283-326
    fn get_compatible_fog_mode(
        &self,
        src_blend: SrcBlendFunc,
        dst_blend: DstBlendFunc,
    ) -> Option<FogFunc> {
        match (src_blend, dst_blend) {
            // Opaque rendering
            (SrcBlendFunc::One, DstBlendFunc::Zero) => Some(FogFunc::Enable),

            // Additive blending
            (SrcBlendFunc::One, DstBlendFunc::One) => Some(FogFunc::ScaleFragment),

            // Screen blending
            (SrcBlendFunc::One, DstBlendFunc::InvSrcColor) => Some(FogFunc::ScaleFragment),

            // Alpha blending
            (SrcBlendFunc::SrcAlpha, DstBlendFunc::InvSrcAlpha) => Some(FogFunc::Enable),

            // Inverse alpha blending
            (SrcBlendFunc::InvSrcAlpha, DstBlendFunc::SrcAlpha) => Some(FogFunc::Enable),

            // Multiplicative blending
            (SrcBlendFunc::Zero, DstBlendFunc::SrcColor) => Some(FogFunc::White),

            // Unsupported combinations
            _ => None,
        }
    }

    // ========================================================================
    // TEXTURE OPERATION VALIDATION
    // ========================================================================

    /// Validate texture operation support
    /// C++ Reference: shader.cpp:590-652
    pub fn validate_texture_operation(&self, gradient: GradientFunc) -> ValidationResult {
        let mut result = ValidationResult::new();

        match gradient {
            GradientFunc::Disable => {
                result.add_message(ValidationMessage::info(
                    "TEX_001",
                    "Texture operations disabled",
                ));
            }
            GradientFunc::Modulate => {
                result.add_message(ValidationMessage::info(
                    "TEX_002",
                    "Texture modulation: universally supported",
                ));
            }
            GradientFunc::Add => {
                if !self.gpu_capabilities.supports_add {
                    result.add_message(
                        ValidationMessage::error(
                            "TEX_ERR_001",
                            "ADD texture operation not supported by GPU",
                        )
                        .with_suggestion("Use MODULATE instead or update GPU drivers"),
                    );
                } else {
                    result.add_message(ValidationMessage::info(
                        "TEX_003",
                        "ADD texture operation supported",
                    ));
                }
            }
            GradientFunc::Modulate2X => {
                if !self.gpu_capabilities.supports_modulate2x {
                    result.add_message(
                        ValidationMessage::warning(
                            "TEX_WARN_001",
                            "MODULATE2X not supported; will fallback to MODULATE",
                        )
                        .with_suggestion("Consider using standard MODULATE for compatibility"),
                    );
                } else {
                    result.add_message(ValidationMessage::info(
                        "TEX_004",
                        "MODULATE2X texture operation supported",
                    ));
                }
            }
            GradientFunc::BumpEnvMap => {
                if !self.gpu_capabilities.supports_bumpenvmap {
                    result.add_message(
                        ValidationMessage::error("TEX_ERR_002", "BUMPENVMAP not supported by GPU")
                            .with_suggestion(
                                "Use MODULATE instead or use a shader-based bump mapping",
                            ),
                    );
                } else {
                    result.add_message(ValidationMessage::info(
                        "TEX_005",
                        "BUMPENVMAP texture operation supported",
                    ));
                }
            }
            GradientFunc::BumpEnvMapLuminance => {
                if !self.gpu_capabilities.supports_bumpenvmap_luminance {
                    result.add_message(
                        ValidationMessage::error(
                            "TEX_ERR_003",
                            "BUMPENVMAP_LUMINANCE not supported by GPU",
                        )
                        .with_suggestion("Use BUMPENVMAP or MODULATE instead"),
                    );
                } else {
                    result.add_message(ValidationMessage::info(
                        "TEX_006",
                        "BUMPENVMAP_LUMINANCE texture operation supported",
                    ));
                }
            }
        }

        result
    }

    /// Validate detail color operation support
    pub fn validate_detail_color_operation(
        &self,
        detail_color: DetailColorFunc,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();

        match detail_color {
            DetailColorFunc::Disable => {
                result.add_message(ValidationMessage::info(
                    "DETAIL_001",
                    "Detail color operations disabled",
                ));
            }
            DetailColorFunc::Detail | DetailColorFunc::Scale => {
                if !self.gpu_capabilities.supports_select_arg1
                    && !self.gpu_capabilities.supports_modulate
                {
                    result.add_message(ValidationMessage::error(
                        "DETAIL_ERR_001",
                        "Required texture operation (SELECTARG1 or MODULATE) not supported",
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_002",
                        "Detail color operation supported",
                    ));
                }
            }
            DetailColorFunc::Add => {
                if !self.gpu_capabilities.supports_add {
                    result.add_message(ValidationMessage::warning(
                        "DETAIL_WARN_001",
                        "ADD operation for detail not supported; will be skipped",
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_003",
                        "Detail ADD operation supported",
                    ));
                }
            }
            DetailColorFunc::Sub | DetailColorFunc::SubR => {
                if !self.gpu_capabilities.supports_subtract {
                    result.add_message(ValidationMessage::warning(
                        "DETAIL_WARN_002",
                        "SUBTRACT operation for detail not supported; will be skipped",
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_004",
                        "Detail SUBTRACT operation supported",
                    ));
                }
            }
            DetailColorFunc::Blend | DetailColorFunc::DetailBlend => {
                let op_name = if detail_color == DetailColorFunc::Blend {
                    "BLEND_TEXTURE_ALPHA"
                } else {
                    "BLEND_CURRENT_ALPHA"
                };
                if !self.gpu_capabilities.supports_blend_texture_alpha {
                    result.add_message(ValidationMessage::warning(
                        "DETAIL_WARN_003",
                        format!("{} not supported; will be skipped", op_name),
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_005",
                        format!("{} operation supported", op_name),
                    ));
                }
            }
            DetailColorFunc::AddSigned | DetailColorFunc::AddSigned2X => {
                let supports = if detail_color == DetailColorFunc::AddSigned {
                    self.gpu_capabilities.supports_add_signed
                } else {
                    self.gpu_capabilities.supports_add_signed2x
                };
                if !supports {
                    result.add_message(ValidationMessage::warning(
                        "DETAIL_WARN_004",
                        format!("{:?} not supported; will fallback to ADD", detail_color),
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_006",
                        format!("{:?} operation supported", detail_color),
                    ));
                }
            }
            DetailColorFunc::Scale2X => {
                if !self.gpu_capabilities.supports_modulate2x {
                    result.add_message(ValidationMessage::warning(
                        "DETAIL_WARN_005",
                        "MODULATE2X for detail not supported; will fallback to MODULATE",
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_007",
                        "Detail MODULATE2X supported",
                    ));
                }
            }
            DetailColorFunc::ModAlphaAddColor => {
                if !self.gpu_capabilities.supports_mod_alpha_add_color {
                    result.add_message(ValidationMessage::warning(
                        "DETAIL_WARN_006",
                        "MODULATEALPHA_ADDCOLOR not supported; will fallback to ADD",
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_008",
                        "MODULATEALPHA_ADDCOLOR operation supported",
                    ));
                }
            }
            DetailColorFunc::InvScale => {
                if !self.gpu_capabilities.supports_addsmooth {
                    result.add_message(ValidationMessage::warning(
                        "DETAIL_WARN_007",
                        "ADDSMOOTH for detail not supported; will be skipped",
                    ));
                } else {
                    result.add_message(ValidationMessage::info(
                        "DETAIL_009",
                        "Detail ADDSMOOTH supported",
                    ));
                }
            }
        }

        result
    }

    // ========================================================================
    // COMPREHENSIVE SHADER STATE VALIDATION
    // ========================================================================

    /// Comprehensive validation of complete shader state
    pub fn validate_shader_state(
        &self,
        src_blend: SrcBlendFunc,
        dst_blend: DstBlendFunc,
        fog_func: FogFunc,
        gradient: GradientFunc,
        detail_color: DetailColorFunc,
        texturing_enabled: bool,
    ) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Validate blend mode
        let blend_result = self.validate_blend_mode(src_blend, dst_blend);
        for msg in blend_result.messages {
            result.add_message(msg);
        }

        // Validate fog compatibility
        let fog_result = self.validate_fog_compatibility(src_blend, dst_blend, fog_func);
        for msg in fog_result.messages {
            result.add_message(msg);
        }
        if fog_result.fallback_suggestion.is_some() {
            result.fallback_suggestion = fog_result.fallback_suggestion;
        }

        // Validate texture operations only if texturing is enabled
        if texturing_enabled {
            let tex_result = self.validate_texture_operation(gradient);
            for msg in tex_result.messages {
                result.add_message(msg);
            }

            let detail_result = self.validate_detail_color_operation(detail_color);
            for msg in detail_result.messages {
                result.add_message(msg);
            }
        }

        result
    }

    // ========================================================================
    // PRESET SHADER VALIDATION
    // ========================================================================

    /// Validate a preset shader configuration
    pub fn validate_preset(&self, preset_name: &str) -> ValidationResult {
        let (src_blend, dst_blend, fog_func, gradient, detail_color, texturing) = match preset_name
        {
            "opaque" => (
                SrcBlendFunc::One,
                DstBlendFunc::Zero,
                FogFunc::Enable,
                GradientFunc::Modulate,
                DetailColorFunc::Disable,
                true,
            ),
            "additive" => (
                SrcBlendFunc::One,
                DstBlendFunc::One,
                FogFunc::ScaleFragment,
                GradientFunc::Modulate,
                DetailColorFunc::Disable,
                true,
            ),
            "alpha" => (
                SrcBlendFunc::SrcAlpha,
                DstBlendFunc::InvSrcAlpha,
                FogFunc::Enable,
                GradientFunc::Modulate,
                DetailColorFunc::Disable,
                true,
            ),
            "multiply" => (
                SrcBlendFunc::Zero,
                DstBlendFunc::SrcColor,
                FogFunc::White,
                GradientFunc::Modulate,
                DetailColorFunc::Disable,
                true,
            ),
            "screen" => (
                SrcBlendFunc::One,
                DstBlendFunc::InvSrcColor,
                FogFunc::ScaleFragment,
                GradientFunc::Modulate,
                DetailColorFunc::Disable,
                true,
            ),
            "bumpenvmap" => (
                SrcBlendFunc::One,
                DstBlendFunc::One,
                FogFunc::Disable,
                GradientFunc::BumpEnvMap,
                DetailColorFunc::Add,
                true,
            ),
            _ => {
                let mut result = ValidationResult::new();
                result.add_message(ValidationMessage::error(
                    "PRESET_ERR_001",
                    format!("Unknown preset shader: {}", preset_name),
                ));
                return result;
            }
        };

        self.validate_shader_state(
            src_blend,
            dst_blend,
            fog_func,
            gradient,
            detail_color,
            texturing,
        )
    }
}

impl Default for ShaderValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_mode_opaque() {
        let validator = ShaderValidator::new();
        let result = validator.validate_blend_mode(SrcBlendFunc::One, DstBlendFunc::Zero);
        assert!(result.is_valid);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].code, "BLEND_004");
    }

    #[test]
    fn test_blend_mode_alpha() {
        let validator = ShaderValidator::new();
        let result =
            validator.validate_blend_mode(SrcBlendFunc::SrcAlpha, DstBlendFunc::InvSrcAlpha);
        assert!(result.is_valid);
        assert_eq!(result.messages[0].code, "BLEND_001");
    }

    #[test]
    fn test_blend_mode_additive() {
        let validator = ShaderValidator::new();
        let result = validator.validate_blend_mode(SrcBlendFunc::One, DstBlendFunc::One);
        assert!(result.is_valid);
        assert_eq!(result.messages[0].code, "BLEND_002");
    }

    #[test]
    fn test_blend_mode_invalid_zero_zero() {
        let validator = ShaderValidator::new();
        let result = validator.validate_blend_mode(SrcBlendFunc::Zero, DstBlendFunc::Zero);
        assert!(!result.is_valid);
        assert_eq!(result.messages[0].code, "BLEND_ERR_001");
        assert!(result.messages[0].suggestion.is_some());
    }

    #[test]
    fn test_fog_opaque_compatible() {
        let validator = ShaderValidator::new();
        let result = validator.validate_fog_compatibility(
            SrcBlendFunc::One,
            DstBlendFunc::Zero,
            FogFunc::Enable,
        );
        assert!(result.is_valid);
    }

    #[test]
    fn test_fog_additive_requires_scale_fragment() {
        let validator = ShaderValidator::new();
        let result = validator.validate_fog_compatibility(
            SrcBlendFunc::One,
            DstBlendFunc::One,
            FogFunc::Enable,
        );
        // Valid but with warning - there's a compatible fog mode
        assert!(result.is_valid);
        assert!(result.fallback_suggestion.is_some());
        assert_eq!(
            result.fallback_suggestion.as_ref().unwrap().fog_func,
            FogFunc::ScaleFragment
        );
    }

    #[test]
    fn test_fog_incompatible_blend() {
        let validator = ShaderValidator::new();
        let result = validator.validate_fog_compatibility(
            SrcBlendFunc::Zero,
            DstBlendFunc::Zero,
            FogFunc::Enable,
        );
        assert!(!result.is_valid);
    }

    #[test]
    fn test_texture_operation_modulate() {
        let validator = ShaderValidator::new();
        let result = validator.validate_texture_operation(GradientFunc::Modulate);
        assert!(result.is_valid);
    }

    #[test]
    fn test_texture_operation_unsupported() {
        let mut caps = GpuCapabilities::default();
        caps.supports_bumpenvmap = false;
        let validator = ShaderValidator::with_capabilities(caps);
        let result = validator.validate_texture_operation(GradientFunc::BumpEnvMap);
        assert!(!result.is_valid);
        assert_eq!(result.messages[0].code, "TEX_ERR_002");
    }

    #[test]
    fn test_detail_color_operation() {
        let validator = ShaderValidator::new();
        let result = validator.validate_detail_color_operation(DetailColorFunc::Add);
        assert!(result.is_valid);
    }

    #[test]
    fn test_complete_shader_state_opaque() {
        let validator = ShaderValidator::new();
        let result = validator.validate_shader_state(
            SrcBlendFunc::One,
            DstBlendFunc::Zero,
            FogFunc::Enable,
            GradientFunc::Modulate,
            DetailColorFunc::Disable,
            true,
        );
        assert!(result.is_valid);
    }

    #[test]
    fn test_complete_shader_state_invalid_fog() {
        let validator = ShaderValidator::new();
        let result = validator.validate_shader_state(
            SrcBlendFunc::Zero,
            DstBlendFunc::Zero,
            FogFunc::Enable,
            GradientFunc::Modulate,
            DetailColorFunc::Disable,
            true,
        );
        assert!(!result.is_valid);
    }

    #[test]
    fn test_preset_opaque() {
        let validator = ShaderValidator::new();
        let result = validator.validate_preset("opaque");
        assert!(result.is_valid);
    }

    #[test]
    fn test_preset_additive() {
        let validator = ShaderValidator::new();
        let result = validator.validate_preset("additive");
        assert!(result.is_valid);
    }

    #[test]
    fn test_preset_alpha() {
        let validator = ShaderValidator::new();
        let result = validator.validate_preset("alpha");
        assert!(result.is_valid);
    }

    #[test]
    fn test_preset_multiply() {
        let validator = ShaderValidator::new();
        let result = validator.validate_preset("multiply");
        assert!(result.is_valid);
    }

    #[test]
    fn test_preset_invalid() {
        let validator = ShaderValidator::new();
        let result = validator.validate_preset("invalid_preset");
        assert!(!result.is_valid);
        assert_eq!(result.messages[0].code, "PRESET_ERR_001");
    }

    #[test]
    fn test_validation_message_with_suggestion() {
        let msg =
            ValidationMessage::error("TEST_001", "Test error").with_suggestion("Try this instead");
        assert_eq!(msg.severity, ValidationSeverity::Error);
        assert!(msg.suggestion.is_some());
        assert_eq!(msg.suggestion.as_ref().unwrap(), "Try this instead");
    }

    #[test]
    fn test_validation_result_severity_counting() {
        let mut result = ValidationResult::new();
        result.add_message(ValidationMessage::info("I001", "Info"));
        result.add_message(ValidationMessage::warning("W001", "Warning"));
        result.add_message(ValidationMessage::error("E001", "Error"));

        let counts = result.count_by_severity();
        assert_eq!(counts.get(&ValidationSeverity::Info).copied(), Some(1));
        assert_eq!(counts.get(&ValidationSeverity::Warning).copied(), Some(1));
        assert_eq!(counts.get(&ValidationSeverity::Error).copied(), Some(1));
    }

    #[test]
    fn test_fallback_suggestions() {
        let opaque_fb = ShaderFallback::opaque();
        assert_eq!(opaque_fb.src_blend, SrcBlendFunc::One);
        assert_eq!(opaque_fb.dst_blend, DstBlendFunc::Zero);

        let alpha_fb = ShaderFallback::alpha_blend();
        assert_eq!(alpha_fb.src_blend, SrcBlendFunc::SrcAlpha);
        assert_eq!(alpha_fb.dst_blend, DstBlendFunc::InvSrcAlpha);

        let additive_fb = ShaderFallback::additive();
        assert_eq!(additive_fb.src_blend, SrcBlendFunc::One);
        assert_eq!(additive_fb.dst_blend, DstBlendFunc::One);
    }

    #[test]
    fn test_fog_compatible_modes() {
        let validator = ShaderValidator::new();

        // Opaque should use FOG_ENABLE
        let result = validator.validate_fog_compatibility(
            SrcBlendFunc::One,
            DstBlendFunc::Zero,
            FogFunc::Enable,
        );
        assert!(result.is_valid);

        // Additive should use FOG_SCALE_FRAGMENT
        let result = validator.validate_fog_compatibility(
            SrcBlendFunc::One,
            DstBlendFunc::One,
            FogFunc::Enable,
        );
        // Valid but with fallback suggestion
        assert!(result.is_valid);
        assert_eq!(
            result.fallback_suggestion.as_ref().unwrap().fog_func,
            FogFunc::ScaleFragment
        );

        // Multiplicative should use FOG_WHITE
        let result = validator.validate_fog_compatibility(
            SrcBlendFunc::Zero,
            DstBlendFunc::SrcColor,
            FogFunc::Enable,
        );
        // Valid but with fallback suggestion
        assert!(result.is_valid);
        assert_eq!(
            result.fallback_suggestion.as_ref().unwrap().fog_func,
            FogFunc::White
        );
    }

    #[test]
    fn test_disabled_gpu_capability() {
        let mut caps = GpuCapabilities::default();
        caps.supports_add = false;
        let validator = ShaderValidator::with_capabilities(caps);

        let result = validator.validate_texture_operation(GradientFunc::Add);
        assert!(!result.is_valid);
        assert_eq!(result.messages[0].code, "TEX_ERR_001");
    }

    #[test]
    fn test_multiple_texture_ops() {
        let validator = ShaderValidator::new();
        let result = validator.validate_shader_state(
            SrcBlendFunc::One,
            DstBlendFunc::One,
            FogFunc::ScaleFragment,
            GradientFunc::Add,
            DetailColorFunc::Blend,
            true,
        );
        assert!(result.is_valid);
        assert!(result.messages.len() > 2); // Multiple operations validated
    }
}
