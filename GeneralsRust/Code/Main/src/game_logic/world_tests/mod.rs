//! Host GameLogic unit tests (child of `game_logic.rs` via `#[path]`).
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

mod helpers;
use helpers::*;

mod base_defenses;
mod combat_particles_and_economy;
mod crates_and_salvage;
mod network_and_scripts;
mod ocl_and_bombs;
mod parachute_and_rebuild;
mod phase3_produce;
mod pilots_and_movement;
mod production_and_mobs;
mod projectiles_air;
mod scatter_and_chain;
mod science_and_upgrades;
mod shells_and_missiles;
mod strategy_and_stealth;
mod superweapons_and_plans;
mod unit_residuals;
mod vehicles_and_lasers;
