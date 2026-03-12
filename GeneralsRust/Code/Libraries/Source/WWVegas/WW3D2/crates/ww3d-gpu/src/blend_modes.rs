//! Blend Modes
//!
//! This module maps DX8 blend modes to wgpu blend states, supporting all 13+
//! blend modes used in the original C++ codebase.

use wgpu::{BlendComponent, BlendFactor, BlendOperation, BlendState};

/// Blend modes matching the original WW3D shader system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum WW3DBlendMode {
    /// Opaque (no blending)
    Opaque = 0,
    /// Standard alpha blending (Src * SrcAlpha + Dst * (1 - SrcAlpha))
    AlphaBlend = 1,
    /// Additive blending (Src + Dst)
    Additive = 2,
    /// Multiply blending (Src * Dst)
    Multiply = 3,
    /// Multiply by 2 (Src * Dst * 2)
    Multiply2X = 4,
    /// Screen blending (1 - (1 - Src) * (1 - Dst))
    Screen = 5,
    /// Alpha test only (no blending, discard fragments below threshold)
    AlphaTest = 6,
    /// Pre-multiplied alpha (Src + Dst * (1 - SrcAlpha))
    PreMultipliedAlpha = 7,
    /// Additive with alpha (Src * SrcAlpha + Dst)
    AdditiveAlpha = 8,
    /// Min blend (min(Src, Dst))
    Min = 9,
    /// Max blend (max(Src, Dst))
    Max = 10,
    /// Reverse subtract (Dst - Src)
    ReverseSubtract = 11,
    /// Subtract (Src - Dst)
    Subtract = 12,
    /// Modulate alpha add color
    ModulateAlphaAddColor = 13,
}

impl WW3DBlendMode {
    /// Convert to wgpu BlendState
    pub fn to_blend_state(&self) -> Option<BlendState> {
        match self {
            WW3DBlendMode::Opaque => None, // No blending

            WW3DBlendMode::AlphaBlend => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            }),

            WW3DBlendMode::Additive => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),

            WW3DBlendMode::Multiply => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::Src,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::Src,
                    operation: BlendOperation::Add,
                },
            }),

            WW3DBlendMode::Multiply2X => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::Src,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::Src,
                    operation: BlendOperation::Add,
                },
            }),

            WW3DBlendMode::Screen => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrc,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            }),

            WW3DBlendMode::AlphaTest => None, // Handled by fragment shader discard

            WW3DBlendMode::PreMultipliedAlpha => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            }),

            WW3DBlendMode::AdditiveAlpha => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),

            WW3DBlendMode::Min => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Min,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Min,
                },
            }),

            WW3DBlendMode::Max => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Max,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Max,
                },
            }),

            WW3DBlendMode::ReverseSubtract => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::ReverseSubtract,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::ReverseSubtract,
                },
            }),

            WW3DBlendMode::Subtract => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Subtract,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Subtract,
                },
            }),

            WW3DBlendMode::ModulateAlphaAddColor => Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::Src,
                    operation: BlendOperation::Add,
                },
            }),
        }
    }

    /// Get blend mode name
    pub fn name(&self) -> &'static str {
        match self {
            WW3DBlendMode::Opaque => "Opaque",
            WW3DBlendMode::AlphaBlend => "AlphaBlend",
            WW3DBlendMode::Additive => "Additive",
            WW3DBlendMode::Multiply => "Multiply",
            WW3DBlendMode::Multiply2X => "Multiply2X",
            WW3DBlendMode::Screen => "Screen",
            WW3DBlendMode::AlphaTest => "AlphaTest",
            WW3DBlendMode::PreMultipliedAlpha => "PreMultipliedAlpha",
            WW3DBlendMode::AdditiveAlpha => "AdditiveAlpha",
            WW3DBlendMode::Min => "Min",
            WW3DBlendMode::Max => "Max",
            WW3DBlendMode::ReverseSubtract => "ReverseSubtract",
            WW3DBlendMode::Subtract => "Subtract",
            WW3DBlendMode::ModulateAlphaAddColor => "ModulateAlphaAddColor",
        }
    }

    /// Check if blend mode requires sorting
    pub fn requires_sorting(&self) -> bool {
        match self {
            WW3DBlendMode::Opaque | WW3DBlendMode::AlphaTest => false,
            _ => true, // All blend modes benefit from back-to-front sorting
        }
    }

    /// Check if blend mode requires alpha testing
    pub fn requires_alpha_test(&self) -> bool {
        matches!(self, WW3DBlendMode::AlphaTest)
    }
}

/// Preset blend states for common use cases
pub mod presets {
    use super::*;

    /// No blending (opaque)
    pub const OPAQUE: Option<BlendState> = None;

    /// Standard alpha blending
    pub const ALPHA_BLEND: BlendState = BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    };

    /// Additive blending
    pub const ADDITIVE: BlendState = BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        },
    };

    /// Pre-multiplied alpha
    pub const PREMULTIPLIED_ALPHA: BlendState = BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blend_mode_names() {
        assert_eq!(WW3DBlendMode::Opaque.name(), "Opaque");
        assert_eq!(WW3DBlendMode::AlphaBlend.name(), "AlphaBlend");
        assert_eq!(WW3DBlendMode::Additive.name(), "Additive");
    }

    #[test]
    fn test_blend_mode_sorting() {
        assert!(!WW3DBlendMode::Opaque.requires_sorting());
        assert!(WW3DBlendMode::AlphaBlend.requires_sorting());
        assert!(WW3DBlendMode::Additive.requires_sorting());
    }

    #[test]
    fn test_blend_mode_alpha_test() {
        assert!(!WW3DBlendMode::Opaque.requires_alpha_test());
        assert!(WW3DBlendMode::AlphaTest.requires_alpha_test());
    }

    #[test]
    fn test_opaque_blend_state() {
        let state = WW3DBlendMode::Opaque.to_blend_state();
        assert!(state.is_none());
    }

    #[test]
    fn test_alpha_blend_state() {
        let state = WW3DBlendMode::AlphaBlend.to_blend_state();
        assert!(state.is_some());
        let blend = state.unwrap();
        assert_eq!(blend.color.src_factor, BlendFactor::SrcAlpha);
        assert_eq!(blend.color.dst_factor, BlendFactor::OneMinusSrcAlpha);
    }

    #[test]
    fn test_all_blend_modes_valid() {
        // Ensure all blend modes can be converted without panicking
        for mode in [
            WW3DBlendMode::Opaque,
            WW3DBlendMode::AlphaBlend,
            WW3DBlendMode::Additive,
            WW3DBlendMode::Multiply,
            WW3DBlendMode::Multiply2X,
            WW3DBlendMode::Screen,
            WW3DBlendMode::AlphaTest,
            WW3DBlendMode::PreMultipliedAlpha,
            WW3DBlendMode::AdditiveAlpha,
            WW3DBlendMode::Min,
            WW3DBlendMode::Max,
            WW3DBlendMode::ReverseSubtract,
            WW3DBlendMode::Subtract,
            WW3DBlendMode::ModulateAlphaAddColor,
        ] {
            let _state = mode.to_blend_state();
            let _name = mode.name();
            let _sorting = mode.requires_sorting();
            let _alpha_test = mode.requires_alpha_test();
        }
    }
}
