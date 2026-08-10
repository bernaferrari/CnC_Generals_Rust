//! Host objects `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod ai_authority;
mod support_states;
mod object_ai_combat;
mod weapon_upgrades;
mod resources_income;
mod crates_radar_power;
mod create_destroy_die;
mod object_queries;
mod host_ops_writeback;
mod ready_completions;
mod destroy_list_bounty;
mod spawn_templates;
