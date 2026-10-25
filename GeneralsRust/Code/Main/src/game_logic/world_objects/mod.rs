//! Host objects `impl GameLogic` split.
//! Submodules are grandchildren of `game_logic.rs`; `pub(in super::super)`
//! matches the previous `pub(super)` visibility on the flat file.
#![allow(unused_imports, non_snake_case)]

mod ai_authority;
mod bridge_body_callbacks;
mod containment_death;
mod crates_radar_power;
mod create_destroy_die;
mod damage_application;
mod destroy_list_bounty;
mod direct_destroy;
mod energy_influence;
mod field_lifetime;
pub(crate) mod fog_ghosts;
mod host_ops_writeback;
mod internal_move;
#[cfg(test)]
mod native_lifecycle_owner_tests;
mod object_ai_combat;
mod object_queries;
mod object_upgrade_callbacks;
mod overlord_addon_damage;
mod physical_retirement;
pub(crate) mod radar_live;
mod radar_provider_owner;
mod ready_completions;
pub(in crate::game_logic) use ready_completions::allocate_live_object_id;
mod resources_income;
mod selection_owner;
mod spawn_templates;
mod support_states;
mod team_transfer;
mod unit_ai_runtime;
mod weapon_upgrades;

#[cfg(test)]
mod unit_ai_runtime_tests;

#[cfg(test)]
mod residual_repulsor_owner_tests;

#[cfg(test)]
mod bridge_body_callback_tests;

#[cfg(test)]
mod fog_ghost_tests;

#[cfg(test)]
mod pilot_locomotor_owner_tests;

#[cfg(test)]
mod field_lifetime_owner_tests;

#[cfg(test)]
mod team_removal_owner_tests;

#[cfg(test)]
mod team_transfer_owner_tests;

#[cfg(test)]
mod produced_mine_order_tests;

#[cfg(all(test, feature = "game_client"))]
mod armor_fx_owner_tests;

#[cfg(all(test, feature = "game_client"))]
mod authored_die_owner_tests;
