//! Host GameLogic unit tests (child of `game_logic.rs` via `#[path]`).
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

mod helpers;
use helpers::*;

mod network_and_scripts;
mod pilots_and_movement;
mod projectiles_air;
mod superweapons_and_plans;
mod strategy_and_stealth;
mod combat_particles_and_economy;
mod production_and_mobs;
mod ocl_and_bombs;
mod base_defenses;
mod shells_and_missiles;
mod unit_residuals;
mod vehicles_and_lasers;
mod parachute_and_rebuild;
mod science_and_upgrades;
mod crates_and_salvage;
mod scatter_and_chain;
