//! Host log modules (B).
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

use super::host_structure_topple;

#[path = "host_emp_pulse_drop_log.rs"]
pub mod host_emp_pulse_drop_log;

#[path = "host_fire_intent_log.rs"]
pub mod host_fire_intent_log;

#[path = "host_weapon_discharge_log.rs"]
pub mod host_weapon_discharge_log;

#[path = "host_fire_sound_loop_log.rs"]
pub mod host_fire_sound_loop_log;

#[path = "host_fire_spawn_log.rs"]
pub mod host_fire_spawn_log;

#[path = "host_guard_log.rs"]
pub mod host_guard_log;

#[path = "host_heal_log.rs"]
pub mod host_heal_log;

#[path = "host_hijacker_log.rs"]
pub mod host_hijacker_log;

#[path = "host_hive_log.rs"]
pub mod host_hive_log;

#[path = "host_leaflet_b52_drop_log.rs"]
pub mod host_leaflet_b52_drop_log;

#[path = "host_locomotor_log.rs"]
pub mod host_locomotor_log;

#[path = "host_max_health_log.rs"]
pub mod host_max_health_log;

#[path = "host_paradrop_cargo_drop_log.rs"]
pub mod host_paradrop_cargo_drop_log;

#[path = "host_physics_motive_log.rs"]
pub mod host_physics_motive_log;

#[path = "host_economy_ready_log.rs"]
pub mod host_economy_ready_log;

#[path = "host_entity_power_log.rs"]
pub mod host_entity_power_log;

#[path = "host_entity_power_ready_log.rs"]
pub mod host_entity_power_ready_log;

#[path = "host_eva_log.rs"]
pub mod host_eva_log;

#[path = "host_experience_log.rs"]
pub mod host_experience_log;

#[path = "host_faerie_fire_log.rs"]
pub mod host_faerie_fire_log;

#[path = "host_faerie_fire_ready_log.rs"]
pub mod host_faerie_fire_ready_log;

#[path = "host_fire_intent_ready_log.rs"]
pub mod host_fire_intent_ready_log;

#[path = "host_formation_log.rs"]
pub mod host_formation_log;

#[path = "host_fow_log.rs"]
pub mod host_fow_log;

#[path = "host_ground_height_log.rs"]
pub mod host_ground_height_log;

#[path = "host_ground_height_ready_log.rs"]
pub mod host_ground_height_ready_log;

#[path = "host_guard_ready_log.rs"]
pub mod host_guard_ready_log;

#[path = "host_hijacker_ready_log.rs"]
pub mod host_hijacker_ready_log;

#[path = "host_hive_ready_log.rs"]
pub mod host_hive_ready_log;

#[path = "host_identity_log.rs"]
pub mod host_identity_log;

#[path = "host_identity_ready_log.rs"]
pub mod host_identity_ready_log;

#[path = "host_kind_of_log.rs"]
pub mod host_kind_of_log;

#[path = "host_locomotor_ready_log.rs"]
pub mod host_locomotor_ready_log;

#[path = "host_model_condition_log.rs"]
pub mod host_model_condition_log;

#[path = "host_model_condition_ready_log.rs"]
pub mod host_model_condition_ready_log;

#[path = "host_model_mesh_log.rs"]
pub mod host_model_mesh_log;

#[path = "host_move_log.rs"]
pub mod host_move_log;

#[path = "host_move_ambient_audio.rs"]
pub mod host_move_ambient_audio;

#[path = "host_move_target_ready_log.rs"]
pub mod host_move_target_ready_log;

#[path = "host_movement_log.rs"]
pub mod host_movement_log;

#[path = "host_movement_ready_log.rs"]
pub mod host_movement_ready_log;

#[path = "host_overcharge_log.rs"]
pub mod host_overcharge_log;

#[path = "host_overcharge_ready_log.rs"]
pub mod host_overcharge_ready_log;

#[path = "host_overlord_log.rs"]
pub mod host_overlord_log;

#[path = "host_overlord_ready_log.rs"]
pub mod host_overlord_ready_log;

#[path = "host_owner_log.rs"]
pub mod host_owner_log;

#[path = "host_owner_ready_log.rs"]
pub mod host_owner_ready_log;

#[path = "host_physics_motive_ready_log.rs"]
pub mod host_physics_motive_ready_log;

#[path = "host_player_cooldown_log.rs"]
pub mod host_player_cooldown_log;

#[path = "host_player_meta_log.rs"]
pub mod host_player_meta_log;

#[path = "host_player_progress_log.rs"]
pub mod host_player_progress_log;

#[path = "host_field_object_expire_log.rs"]
pub mod host_field_object_expire_log;

#[path = "host_fire_spread_log.rs"]
pub mod host_fire_spread_log;

#[path = "host_flashbang_comanche_helix_projectile_log.rs"]
pub mod host_flashbang_comanche_helix_projectile_log;

#[path = "host_fwwd_continuous_log.rs"]
pub mod host_fwwd_continuous_log;

#[path = "host_fwwd_reaction_log.rs"]
pub mod host_fwwd_reaction_log;

#[path = "host_hacker_income_log.rs"]
pub mod host_hacker_income_log;

#[path = "host_height_die_kill_log.rs"]
pub mod host_height_die_kill_log;

#[path = "host_heli_slow_death_kill_log.rs"]
pub mod host_heli_slow_death_kill_log;

#[path = "host_inferno_shell_projectile_log.rs"]
pub mod host_inferno_shell_projectile_log;

#[path = "host_jet_slow_death_kill_log.rs"]
pub mod host_jet_slow_death_kill_log;

#[path = "host_lifetime_expire_log.rs"]
pub mod host_lifetime_expire_log;

#[path = "host_player_radar_log.rs"]
pub mod host_player_radar_log;

#[path = "host_poison_dot_log.rs"]
pub mod host_poison_dot_log;
