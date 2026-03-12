//! Complete Stealth Detection System
//!
//! This module implements the full C&C Generals stealth mechanics including:
//! - Stealth state tracking and conditions
//! - Detection range-based scanning
//! - Per-player visibility system
//! - Stealth breaking conditions
//! - Visual effects integration
//! - Upgrade system integration

pub mod detector;
pub mod integration;
pub mod state;
pub mod upgrade;
pub mod visibility;

#[cfg(test)]
mod tests;

pub use detector::{
    StealthDetectorController, StealthDetectorUpdate, StealthDetectorUpdateModuleData,
};
pub use integration::{
    StealthEvent, StealthEventListener, StealthIntegration, StealthShaderParams,
    StealthVisualEffects,
};
pub use state::{DetectionState, StealthStateManager, VisibilityState};
pub use upgrade::{StealthUpgrade, StealthUpgradeModuleData, StealthUpgradeType};
pub use visibility::{PerPlayerVisibility, PlayerVisibility, VisibilityManager};

use crate::common::*;
use std::sync::Arc;

/// Stealth detection range multipliers based on unit types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectionLevel {
    /// No detection capability
    None,
    /// Basic detection (100m range)
    Basic,
    /// Advanced detection (200m range)
    Advanced,
    /// Superior detection (300m range)
    Superior,
    /// Total detection (infinite range)
    Total,
}

impl DetectionLevel {
    pub fn get_range(&self) -> f32 {
        match self {
            DetectionLevel::None => 0.0,
            DetectionLevel::Basic => 100.0,
            DetectionLevel::Advanced => 200.0,
            DetectionLevel::Superior => 300.0,
            DetectionLevel::Total => f32::MAX,
        }
    }
}

/// Stealth difficulty levels (how hard to detect)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealthDifficulty {
    /// Easy to detect (basic stealth)
    Easy = 0,
    /// Normal difficulty
    Normal = 1,
    /// Hard to detect (advanced stealth)
    Hard = 2,
    /// Very hard to detect (elite stealth)
    VeryHard = 3,
}

impl StealthDifficulty {
    /// Get detection range modifier (0.0 = undetectable, 1.0 = full range)
    pub fn get_detection_modifier(&self) -> f32 {
        match self {
            StealthDifficulty::Easy => 1.0,
            StealthDifficulty::Normal => 0.75,
            StealthDifficulty::Hard => 0.5,
            StealthDifficulty::VeryHard => 0.25,
        }
    }
}

/// Conditions that can break stealth
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StealthBreakConditions {
    pub while_moving: bool,
    pub while_attacking: bool,
    pub while_damaged: bool,
    pub requires_power: bool,
    pub requires_not_garrisoned: bool,
}

impl Default for StealthBreakConditions {
    fn default() -> Self {
        Self {
            while_moving: false,
            while_attacking: true,
            while_damaged: false,
            requires_power: false,
            requires_not_garrisoned: false,
        }
    }
}

/// Central stealth system coordinator
pub struct StealthSystem {
    visibility_manager: Arc<VisibilityManager>,
}

impl StealthSystem {
    pub fn new() -> Self {
        Self {
            visibility_manager: Arc::new(VisibilityManager::new()),
        }
    }

    pub fn get_visibility_manager(&self) -> Arc<VisibilityManager> {
        self.visibility_manager.clone()
    }
}

impl Default for StealthSystem {
    fn default() -> Self {
        Self::new()
    }
}

// Tests live in `tests.rs` to keep this module focused.
