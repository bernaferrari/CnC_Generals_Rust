//! Host-world save/load and map-runtime behavior split by ownership.
//!
//! The child modules retain the original contiguous implementation order and
//! keep the historical game_logic::world_save module/API seam intact.
#![allow(unused_imports, non_snake_case)]

use super::*;

#[path = "world_save/world_load.rs"]
mod world_load;
#[path = "world_save/world_paths.rs"]
mod world_paths;
#[path = "world_save/world_players.rs"]
mod world_players;
#[path = "world_save/world_runtime.rs"]
mod world_runtime;
#[path = "world_save/world_subsystems.rs"]
mod world_subsystems;
#[cfg(test)]
#[path = "world_save/world_tests.rs"]
mod world_tests;

// Helpers shared by the ownership modules remain private to this host-world
// module (or crate-visible where the original facade was crate-visible).
#[cfg(feature = "game_client")]
pub(super) use world_load::apply_cpp_heightmap_xy_and_border;
pub(crate) use world_load::encode_authored_bridge_visual;
pub(super) use world_load::{
    apply_logic_player_list_relationships, clear_live_campaign_victorious_for_new_game,
    landmark_bridge_half_sizes, leftover_bridge_info_for_object, leftover_bridge_template_name,
    leftover_template_is_landmark_bridge, load_multiplayer_scripts_scb,
};
