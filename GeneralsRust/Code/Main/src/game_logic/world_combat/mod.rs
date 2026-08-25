//! Host combat `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod air_and_mig;
mod base_defense_lasers;
mod drones_and_garrison;
mod gps_and_fields;
mod heroes_and_plans;
mod infantry_weapons;
mod missile_defenders;
mod ocl_and_scud;
mod registries;
mod special_power_flights;
mod strategy_center;
mod streams_and_rpg;
mod tanks_and_upgrades;
mod temporary_weapon_fire;
mod temporary_weapon_force;
mod temporary_weapon_status;
mod vehicle_shells;
mod weapon_barrel_topology;
mod weapon_discharge;
pub(crate) mod weapon_visual_capture;
mod weapon_visual_freeze;

#[cfg(test)]
mod temporary_weapon_fire_tests;
#[cfg(test)]
mod weapon_visual_dispatch_tests;
