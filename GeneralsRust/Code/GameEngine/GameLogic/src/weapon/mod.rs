//! Weapon System
//!
#![allow(ambiguous_glob_reexports)]
#![allow(unused_variables, unused_mut)]
//! This module provides the core weapon system functionality for Command & Conquer Generals Zero Hour,
//! converted from the original C++ implementation to idiomatic Rust.
//!
//! The weapon system includes:
//! - Weapon templates defining weapon properties
//! - Weapon instances with state and ammunition
//! - Damage calculation and bonuses
//! - Projectile management
//! - Target validation and range checking

// Existing child modules (unchanged).
pub mod bezier; // Bezier curve system for projectile flight paths
pub mod damage_system;
mod projectile_launch_cast;
pub mod weapon;
pub mod weapon_set;
pub mod weapon_store;
mod weapon_template;

// Leftover god-file split (canonical types live here, re-exported below).
mod audio_event;
mod crc_snapshot;
mod helpers;
mod masks_enums;
mod store;
mod template;
mod weapon_instance;
mod weapon_instance_combat;

// Phase 12 consolidation: leftover WeaponTemplate and Weapon in this directory
// are the canonical definitions used throughout gamelogic.
// weapon.rs and weapon_template.rs contain supplementary logic that extends
// these types.

// Export damage constants from the canonical damage module
pub use crate::damage::HUGE_DAMAGE_AMOUNT;
pub use damage_system::*;
pub use weapon_set::*;
pub use weapon_store::*;

pub use audio_event::*;
pub use helpers::{ObjectId, INVALID_OBJECT_ID, NO_MAX_SHOTS_LIMIT};
pub use masks_enums::*;
pub use store::{
    initialize_weapon_store, with_weapon_store, with_weapon_store_mut, WeaponDelayedDamageInfo,
    WeaponStore,
};
pub use template::WeaponTemplate;
pub use weapon_instance::Weapon;

pub(crate) use helpers::{
    ammo_count_for_clip_size, dual_world_registry_unavailable, map_common_bonus_flags,
    map_weapon_slot_to_common, weapon_slot_from_u32, weapon_slot_to_u32, weapon_status_from_u32,
    weapon_status_to_u32, EFFECTIVELY_UNLIMITED_CLIP_AMMO,
};

#[cfg(test)]
mod tests;
