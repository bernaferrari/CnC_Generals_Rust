//! Upgrade System - Complete Implementation
//!
//! This module implements the full upgrade system from C++ Command & Conquer Generals Zero Hour.
//! Matches C++ files: Common/Upgrade.h/.cpp, GameLogic/Module/UpgradeModule.h/.cpp
//!
//! Original C++ Authors: Colin Day, Graham Smallwood, March 2002
//! Rust conversion: 2025

pub mod center;
pub mod effects;
pub mod instance;
pub mod mask;
pub mod modules;
pub mod player_upgrade_manager;
pub mod prerequisites;
pub mod production_integration;
pub mod template;

// Re-export main types
pub use center::UpgradeCenter;
pub use effects::{
    UpgradeEffect, UpgradeEffectApplicator, UpgradeEffectRegistry, UpgradeEffectType,
};
pub use instance::{Upgrade, UpgradeStatus};
pub use mask::{upgrade_mask_for_name, UpgradeMask};
pub use modules::*;
pub use player_upgrade_manager::PlayerUpgradeManager;
pub use prerequisites::{
    Prerequisite, PrerequisiteChecker, PrerequisiteType, TechTree, UpgradePrerequisites,
};
pub use production_integration::{
    UpgradeProductionIntegration, UpgradeProductionItem, UpgradeProductionQueue,
};
pub use template::{UpgradeTemplate, UpgradeType};

// Also export legacy upgrade functions for backwards compatibility
pub use crate::upgrade_legacy::upgrade_mask_for_ascii;

use crate::common::*;

/// Maximum number of upgrades in the system
/// Matches C++ UPGRADE_MAX_COUNT from Upgrade.h
pub const UPGRADE_MAX_COUNT: usize = 128;

/// Error types for upgrade system
#[derive(Debug, Clone, thiserror::Error)]
pub enum UpgradeError {
    #[error("Upgrade not found: {0}")]
    NotFound(String),

    #[error("Upgrade already exists: {0}")]
    AlreadyExists(String),

    #[error("Cannot afford upgrade: {0}")]
    CannotAfford(String),

    #[error("Prerequisites not met: {0}")]
    PrerequisitesNotMet(String),

    #[error("Conflicting upgrade: {0}")]
    ConflictingUpgrade(String),

    #[error("Invalid upgrade type")]
    InvalidType,

    #[error("Maximum upgrades reached")]
    MaxUpgradesReached,
}

pub type UpgradeResult<T> = Result<T, UpgradeError>;
