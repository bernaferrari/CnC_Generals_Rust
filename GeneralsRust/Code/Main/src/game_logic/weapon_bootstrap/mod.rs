//! Host WeaponStore bootstrap for template → weapon binding.
//!
//! # Why the GameLogic WeaponStore is often empty
//!
//! 1. `gamelogic::initialize_weapon_store()` only constructs an empty store.
//! 2. Full Weapon.ini population happens when AssetManager loads BIG archives
//!    (`assets::ini_template_loader::load_weapon_templates`). Headless unit tests
//!    and many host probes never open archives, so the store stays empty and
//!    `ThingTemplate::resolve_primary_weapon` falls back to `Weapon::default()`.
//! 3. Engine startup also parses Weapon.ini into Common's separate
//!    `game_engine::common::ini::ini_weapon` store (INI block table). That is
//!    **not** the GameLogic store that `ThingTemplate::weapon_from_store` reads.
//!
//! This module is the reliable host-side fill path:
//! - Prefer loading extracted / shipped `Data/INI/Weapon.ini` when present on disk
//! - Always seed a small set of golden-unit weapons if still missing
//!
//! Fail-closed: seeding known host weapons is not full Weapon.ini parity.
//! Secondary slots (`Weapon = SECONDARY Name`) are seeded for known units only;
//! full WeaponSet upgrade matrices are deferred.
//!
//! # Secondary combat residual (host `update_combat`)
//!
//! Binding alone is not enough: fire must consider `Object.secondary_weapon`.
//! Fail-closed host rules (not full AutoChoose / PreferredAgainst):
//! - Prefer secondary vs structures when secondary damage ≥ primary (or primary cannot fire).
//! - Otherwise primary first; secondary when primary is reloading / OOR (alternate fire).
//! - Player `active_weapon_slot == 1` forces secondary preference when ready + in range.
//! - Ground force-fire still uses primary only.

// Restricted re-exports so impl submodules can `use super::*;`
// without dumping the parent crate surface through `pub use`.
pub(in crate::game_logic::weapon_bootstrap) use gamelogic::weapon::{
    WeaponAntiMask, WeaponTemplate, with_weapon_store, with_weapon_store_mut,
};
use glam::Vec3;
pub(in crate::game_logic::weapon_bootstrap) use std::path::{Path, PathBuf};
pub(in crate::game_logic::weapon_bootstrap) use std::sync::atomic::{AtomicBool, Ordering};

mod names;
pub use names::*;
mod honesty;
pub use honesty::*;
mod pitch_range;
pub use pitch_range::*;
mod reload_aim;
pub use reload_aim::*;
mod historic_speed;
pub use historic_speed::*;
mod scatter;
pub use scatter::*;
mod collide;
pub use collide::*;
mod shock_damage;
pub use shock_damage::*;
mod fx;
pub use fx::*;
mod projectile_sound;
pub use projectile_sound::*;
mod projectile_flight;
mod projectile_lifecycle;
pub use projectile_flight::*;
pub use projectile_lifecycle::*;
mod damage_kinds;
pub use damage_kinds::*;
mod estimate;
pub use estimate::*;
mod unit_map;
pub use unit_map::*;
mod store;
pub use store::*;
mod extra_bonus;
pub use extra_bonus::*;

#[cfg(test)]
mod tests;

/// Concatenated live weapon_bootstrap sources for residual `include_str` scans.
pub const WEAPON_BOOTSTRAP_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("collide.rs"),
    include_str!("damage_kinds.rs"),
    include_str!("estimate.rs"),
    include_str!("extra_bonus.rs"),
    include_str!("fx.rs"),
    include_str!("historic_speed.rs"),
    include_str!("honesty.rs"),
    include_str!("names.rs"),
    include_str!("pitch_range.rs"),
    include_str!("projectile_flight.rs"),
    include_str!("projectile_lifecycle.rs"),
    include_str!("projectile_sound.rs"),
    include_str!("reload_aim.rs"),
    include_str!("scatter.rs"),
    include_str!("shock_damage.rs"),
    include_str!("store.rs"),
    include_str!("unit_map.rs"),
);
