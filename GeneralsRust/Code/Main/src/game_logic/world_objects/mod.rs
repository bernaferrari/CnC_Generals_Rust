//! Host objects `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod ai_authority;
mod bridge_body_callbacks;
mod crates_radar_power;
mod create_destroy_die;
mod damage_application;
mod destroy_list_bounty;
mod direct_destroy;
mod host_ops_writeback;
mod internal_move;
mod object_ai_combat;
mod object_queries;
mod overlord_addon_damage;
mod radar_live;
mod ready_completions;
mod resources_income;
mod spawn_templates;
mod support_states;
mod unit_ai_runtime;
mod weapon_upgrades;

#[cfg(test)]
mod unit_ai_runtime_tests;

#[cfg(test)]
mod residual_repulsor_owner_tests;

#[cfg(test)]
mod bridge_body_callback_tests;
