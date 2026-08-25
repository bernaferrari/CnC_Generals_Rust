//! Weapon System
//!
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
mod weapon;
pub mod weapon_set;
mod weapon_store;
mod weapon_template;

// Leftover god-file split (canonical types live here, re-exported below).
mod audio_event;
mod crc_snapshot;
mod helpers;
mod masks_enums;
mod store;
mod template;
mod weapon_approach;
mod weapon_bonus;
mod weapon_instance;
mod weapon_instance_combat;
mod weapon_range;

// Phase 12 consolidation: leftover `template` / `weapon_instance` / `store`
// are the single public Weapon / WeaponTemplate / WeaponStore stack.
// Leftover `weapon.rs`, `weapon_template.rs`, and `weapon_store.rs` stay as
// private modules so their working tests keep compiling; they are not a
// second public type stack (C++ Weapon.cpp has one definition of each).

// Export damage constants from the canonical damage module
pub use crate::damage::HUGE_DAMAGE_AMOUNT;
pub use damage_system::*;
pub use weapon_set::*;

pub use audio_event::*;
pub use helpers::{INVALID_OBJECT_ID, NO_MAX_SHOTS_LIMIT, ObjectId};
pub use masks_enums::*;
pub use store::{
    WeaponDelayedDamageInfo, WeaponDelayedDamageSnapshotResidual, WeaponStore,
    honesty_weapon_store_delayed_damage_residual_ok, initialize_weapon_store,
    shutdown_weapon_store, with_weapon_store, with_weapon_store_mut,
};
pub use template::WeaponTemplate;
pub use weapon_instance::Weapon;

pub(crate) use helpers::{
    EFFECTIVELY_UNLIMITED_CLIP_AMMO, ammo_count_for_clip_size, dual_world_registry_unavailable,
    map_common_bonus_flags, map_weapon_slot_to_common, weapon_slot_from_u32, weapon_slot_to_u32,
    weapon_status_from_u32, weapon_status_to_u32,
};

#[cfg(test)]
mod tests;
