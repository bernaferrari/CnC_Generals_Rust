//! Always-on residual helpers still wired into the default build.
//!
//! `#[path]` keeps the `.rs` files in this directory. Parent types and a
//! few sibling modules are imported so existing `use super::ObjectId`
//! (and similar) paths in those files keep resolving.

#![allow(unused_imports)]

use super::ObjectId;
use super::Team;
use super::VeterancyLevel;
use super::Weapon;
use super::combat;

#[path = "host_ai_path_combat_residual_wave105.rs"]
pub mod host_ai_path_combat_residual_wave105;

#[path = "host_armor_residual.rs"]
pub mod host_armor_residual;

#[path = "host_dock_contain_exit_heal_residual.rs"]
pub mod host_dock_contain_exit_heal_residual;

#[path = "host_enum_table_residual.rs"]
pub mod host_enum_table_residual;

#[path = "host_faction_skirmish_residual.rs"]
pub mod host_faction_skirmish_residual;

#[path = "host_fx_ocl_particle_audio_residual_wave107.rs"]
pub mod host_fx_ocl_particle_audio_residual_wave107;

#[path = "host_gamedata_lobby_residual.rs"]
pub mod host_gamedata_lobby_residual;

#[path = "host_live_cmd_filter_env_presentation_only_residual_wave217.rs"]
pub mod host_live_cmd_filter_env_presentation_only_residual_wave217;

#[path = "host_live_gameworld_shadow_overlay_residual_wave172.rs"]
pub mod host_live_gameworld_shadow_overlay_residual_wave172;

#[path = "host_live_mouse_input_presentation_only_residual_wave236.rs"]
pub mod host_live_mouse_input_presentation_only_residual_wave236;

#[path = "host_live_presentation_env_seed_gameworld_residual_wave466.rs"]
pub mod host_live_presentation_env_seed_gameworld_residual_wave466;

#[path = "host_live_presentation_seed_residual_wave171.rs"]
pub mod host_live_presentation_seed_residual_wave171;

#[path = "host_live_ui_helpers_presentation_only_residual_wave215.rs"]
pub mod host_live_ui_helpers_presentation_only_residual_wave215;

#[path = "host_main_menu_wnd_materialise_residual_wave162.rs"]
pub mod host_main_menu_wnd_materialise_residual_wave162;

#[path = "host_new_game_stream_drain_residual_wave167.rs"]
pub mod host_new_game_stream_drain_residual_wave167;

#[path = "host_partition_collision_physics_residual.rs"]
pub mod host_partition_collision_physics_residual;

#[path = "host_production_buildable_command_residual.rs"]
pub mod host_production_buildable_command_residual;

#[path = "host_radar_stealth_vision_residual.rs"]
pub mod host_radar_stealth_vision_residual;

#[path = "host_rank_ui_residual.rs"]
pub mod host_rank_ui_residual;

#[path = "host_residual_acquire.rs"]
pub mod host_residual_acquire;

#[path = "host_rng_residual.rs"]
pub mod host_rng_residual;

#[path = "host_shell_stack_push_residual_wave163.rs"]
pub mod host_shell_stack_push_residual_wave163;

#[path = "host_sp_science_upgrade_player_team_residual_wave109.rs"]
pub mod host_sp_science_upgrade_player_team_residual_wave109;

#[path = "host_special_power_enum_residual.rs"]
pub mod host_special_power_enum_residual;

#[path = "host_start_game_loading_residual_wave169.rs"]
pub mod host_start_game_loading_residual_wave169;

#[path = "host_structure_economy_residual.rs"]
pub mod host_structure_economy_residual;

#[path = "host_terrain_bridge_water_road_residual_wave108.rs"]
pub mod host_terrain_bridge_water_road_residual_wave108;

#[path = "host_terrain_env_boundary_residual_wave159.rs"]
pub mod host_terrain_env_boundary_residual_wave159;

#[path = "host_ui_presentation_residual.rs"]
pub mod host_ui_presentation_residual;

#[path = "host_upgrade_module_residuals.rs"]
pub(super) mod host_upgrade_module_residuals;

#[path = "host_w3d_main_menu_init_residual_wave168.rs"]
pub mod host_w3d_main_menu_init_residual_wave168;

#[cfg(any(test, feature = "host-residuals"))]
#[path = "host_wave_inflation.rs"]
pub mod host_wave_inflation;
#[cfg(any(test, feature = "host-residuals"))]
pub use host_wave_inflation::{
    residual_pack_cannot_set_playable_claim, self_table_honesty_is_inflation,
};
