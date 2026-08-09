//! Host combat `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod registries;
mod special_power_flights;
mod strategy_center;
mod base_defense_lasers;
mod infantry_weapons;
mod ocl_and_scud;
mod vehicle_shells;
mod air_and_mig;
mod tanks_and_upgrades;
mod streams_and_rpg;
mod missile_defenders;
mod drones_and_garrison;
mod heroes_and_plans;
mod gps_and_fields;
