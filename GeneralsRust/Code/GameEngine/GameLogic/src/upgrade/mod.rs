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

/// Complete an upgrade for a player by name.
/// Matches C++ `Player::completeUpgrade(UpgradeTemplate*)`.
pub fn complete_upgrade(player_id: u32, upgrade_name: &str) -> UpgradeResult<()> {
    let template = center::with_upgrade_center(|center| center.find_upgrade(upgrade_name))
        .ok_or_else(|| UpgradeError::NotFound(upgrade_name.to_string()))?;

    let player_arc = {
        let Ok(list) = crate::player::ThePlayerList().read() else {
            return Err(UpgradeError::NotFound(format!(
                "Player {} list unavailable",
                player_id
            )));
        };
        list.get_player(player_id as i32).cloned()
    };
    let Some(player_arc) = player_arc else {
        return Err(UpgradeError::NotFound(format!(
            "Player {} not found",
            player_id
        )));
    };

    {
        let Ok(mut player_guard) = player_arc.write() else {
            return Err(UpgradeError::NotFound(format!(
                "Player {} lock failed",
                player_id
            )));
        };

        // Directly apply upgrade effects to player's objects.
        // We can't call upgrade_mgr.grant_upgrade() because it needs &mut Player
        // while the manager itself is borrowed from Player.
        let objects: Vec<_> = player_guard.get_all_objects().to_vec();
        let affects_existing = template.affects_existing_objects();
        let upgrade_mask = template.get_mask();
        let upgrade_key = template.get_name_key();

        if let Some(upgrade_mgr) = player_guard.get_upgrade_manager_mut() {
            upgrade_mgr.add_completed_upgrade(upgrade_key, upgrade_mask);
        }

        drop(player_guard);

        if affects_existing {
            for object_id in objects {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                    object_id,
                    |object_guard| {
                        if object_guard.is_destroyed()
                            || object_guard.get_controlling_player_id() != Some(player_id)
                        {
                            return;
                        }
                        object_guard.give_upgrade(template.as_ref());
                    },
                );
            }
        }
    }

    if let Ok(mut engine_guard) = crate::scripting::engine::get_script_engine().write() {
        if let Some(engine) = engine_guard.as_mut() {
            engine.notify_of_completed_upgrade(
                player_id as usize,
                upgrade_name,
                crate::common::INVALID_ID,
            );
        }
    }

    Ok(())
}
