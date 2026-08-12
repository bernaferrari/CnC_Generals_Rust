//! Host objects `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod ai_authority;
mod crates_radar_power;
mod create_destroy_die;
mod destroy_list_bounty;
mod host_ops_writeback;
mod object_ai_combat;
mod object_queries;
mod ready_completions;
mod resources_income;
mod spawn_templates;
mod support_states;
mod weapon_upgrades;
