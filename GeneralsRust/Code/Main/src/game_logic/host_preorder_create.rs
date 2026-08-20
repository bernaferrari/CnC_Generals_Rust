//! Host PreorderCreate residual.
//!
//! C++: `PreorderCreate::onBuildComplete` — if controlling player preordered the
//! game, set MODELCONDITION_PREORDER; otherwise clear it.
//!
//! Retail: Command Centers and select structures carry PreorderCreate so
//! preorder-bonus cosmetics (flag/model) show on completion.
//!
//! Residual playability slice:
//! - Player `did_preorder` flag (skirmish/shell residual)
//! - MODELCONDITION_PREORDER bit **95** (ALLOW_SURRENDER-off layout)
//! - Applied on structure construction complete for PreorderCreate templates
//!
//! Fail-closed: not full ControlBar preorder UI / multiplayer preorder_mask matrix.

use serde::{Deserialize, Serialize};

/// C++ ModelConditionFlagType::PREORDER bit index.
pub const MC_BIT_PREORDER: u32 = 95;

/// Honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostPreorderCreateRegistry {
    pub applied_set: u32,
    pub applied_clear: u32,
}

impl HostPreorderCreateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_set(&mut self) {
        self.applied_set = self.applied_set.saturating_add(1);
    }
    pub fn record_clear(&mut self) {
        self.applied_clear = self.applied_clear.saturating_add(1);
    }
    pub fn honesty_ok(&self) -> bool {
        self.applied_set > 0 || self.applied_clear > 0
    }
}

/// C++ PreorderCreate is INI-authored; do not infer from template names.
pub fn is_preorder_create_module(has_module: bool) -> bool {
    has_module
}

/// Apply PREORDER model condition residual.
pub fn apply_preorder_model_bit(bits: u128, did_preorder: bool) -> u128 {
    let mask = 1u128 << MC_BIT_PREORDER;
    if did_preorder {
        bits | mask
    } else {
        bits & !mask
    }
}

pub fn has_preorder_model_bit(bits: u128) -> bool {
    bits & (1u128 << MC_BIT_PREORDER) != 0
}

pub fn honesty_preorder_create_residual_ok() -> bool {
    MC_BIT_PREORDER == 95
        && is_preorder_create_module(true)
        && !is_preorder_create_module(false)
        && has_preorder_model_bit(apply_preorder_model_bit(0, true))
        && !has_preorder_model_bit(apply_preorder_model_bit(1u128 << MC_BIT_PREORDER, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preorder_bit_and_templates() {
        assert!(honesty_preorder_create_residual_ok());
        let mut b = 0u128;
        b = apply_preorder_model_bit(b, true);
        assert!(has_preorder_model_bit(b));
        b = apply_preorder_model_bit(b, false);
        assert!(!has_preorder_model_bit(b));
    }
}
