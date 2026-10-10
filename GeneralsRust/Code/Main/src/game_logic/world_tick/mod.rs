//! Host tick `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod ai;
mod ai_phase;
mod airfield;
#[cfg(test)]
pub(in crate::game_logic) use airfield::heli_motion_tests;
mod attack;
mod collide_dispatch;
pub(in super::super) use collide_dispatch::host_object_footprint;
mod collide_modules;

mod combat;
mod combat_fire_fx;
mod crates;
mod disabled_expiry;
mod flight_terrain;
mod logic_crc;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod projectile_terrain_owner_tests;
pub(crate) use flight_terrain::FlightTerrainView;
mod mood;
mod movement;
mod movement_support;
mod physics;
mod presence;
mod production;
mod shock;
mod sleepy;
mod step;
mod water_owner;
pub(in super::super) use sleepy::{HostSleepyHeap, HostSleepyKind};
mod teams;
