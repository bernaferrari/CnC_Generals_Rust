//! Host log modules (C).
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

#[path = "host_production_door_log.rs"]
pub mod host_production_door_log;

#[path = "host_projectile_log.rs"]
pub mod host_projectile_log;

#[path = "host_rebuild_producer_log.rs"]
pub mod host_rebuild_producer_log;

#[path = "host_shock_stun_log.rs"]
pub mod host_shock_stun_log;

#[path = "host_sole_healing_log.rs"]
pub mod host_sole_healing_log;

#[path = "host_stealth_delay_log.rs"]
pub mod host_stealth_delay_log;

#[path = "host_production_door_ready_log.rs"]
pub mod host_production_door_ready_log;

#[path = "host_production_log.rs"]
pub mod host_production_log;

#[path = "host_production_progress_log.rs"]
pub mod host_production_progress_log;

#[path = "host_production_ready_log.rs"]
pub mod host_production_ready_log;

#[path = "host_production_spawn_ready_log.rs"]
pub mod host_production_spawn_ready_log;

#[path = "host_projectiles_ready_log.rs"]
pub mod host_projectiles_ready_log;

#[path = "host_radar_extend_log.rs"]
pub mod host_radar_extend_log;

#[path = "host_radar_extend_ready_log.rs"]
pub mod host_radar_extend_ready_log;

#[path = "host_radar_log.rs"]
pub mod host_radar_log;

#[path = "host_rally_log.rs"]
pub mod host_rally_log;

#[path = "host_rebuild_ready_log.rs"]
pub mod host_rebuild_ready_log;

#[path = "host_repulsor_log.rs"]
pub mod host_repulsor_log;

#[path = "host_repulsor_ready_log.rs"]
pub mod host_repulsor_ready_log;

#[path = "host_selection_radius_log.rs"]
pub mod host_selection_radius_log;

#[path = "host_selection_radius_ready_log.rs"]
pub mod host_selection_radius_ready_log;

#[path = "host_sell_ready_log.rs"]
pub mod host_sell_ready_log;

#[path = "host_shock_stun_ready_log.rs"]
pub mod host_shock_stun_ready_log;

#[path = "host_sole_healing_ready_log.rs"]
pub mod host_sole_healing_ready_log;

#[path = "host_spawn_log.rs"]
pub mod host_spawn_log;

#[path = "host_special_power_log.rs"]
pub mod host_special_power_log;

#[path = "host_special_power_ready_log.rs"]
pub mod host_special_power_ready_log;

#[path = "host_status_log.rs"]
pub mod host_status_log;

#[path = "host_stealth_delay_ready_log.rs"]
pub mod host_stealth_delay_ready_log;

#[path = "host_stealth_flags_log.rs"]
pub mod host_stealth_flags_log;

#[path = "host_stealth_flags_ready_log.rs"]
pub mod host_stealth_flags_ready_log;

#[path = "host_stored_supplies_log.rs"]
pub mod host_stored_supplies_log;

#[path = "host_stored_supplies_ready_log.rs"]
pub mod host_stored_supplies_ready_log;

#[path = "host_target_location_log.rs"]
pub mod host_target_location_log;

#[path = "host_target_location_ready_log.rs"]
pub mod host_target_location_ready_log;

#[path = "host_transform_ready_log.rs"]
pub mod host_transform_ready_log;

#[path = "host_turret_log.rs"]
pub mod host_turret_log;

#[path = "host_turret_ready_log.rs"]
pub mod host_turret_ready_log;

#[path = "host_upgrade_ready_log.rs"]
pub mod host_upgrade_ready_log;

#[path = "host_veterancy_log.rs"]
pub mod host_veterancy_log;

#[path = "host_veterancy_ready_log.rs"]
pub mod host_veterancy_ready_log;

#[path = "host_vision_camo_log.rs"]
pub mod host_vision_camo_log;

#[path = "host_vision_camo_ready_log.rs"]
pub mod host_vision_camo_ready_log;

#[path = "host_weapon_bonus_log.rs"]
pub mod host_weapon_bonus_log;

#[path = "host_weapon_bonus_ready_log.rs"]
pub mod host_weapon_bonus_ready_log;

#[path = "host_weapon_set_log.rs"]
pub mod host_weapon_set_log;

#[path = "host_weapon_set_ready_log.rs"]
pub mod host_weapon_set_ready_log;

#[path = "host_weapon_slot_log.rs"]
pub mod host_weapon_slot_log;

#[path = "host_weapon_slot_ready_log.rs"]
pub mod host_weapon_slot_ready_log;

#[path = "host_weapon_stats_log.rs"]
pub mod host_weapon_stats_log;

#[path = "host_weapon_stats_ready_log.rs"]
pub mod host_weapon_stats_ready_log;

#[path = "host_power_plant_rods_log.rs"]
pub mod host_power_plant_rods_log;

#[path = "host_scorpion_missile_projectile_log.rs"]
pub mod host_scorpion_missile_projectile_log;

#[path = "host_slow_death_kill_log.rs"]
pub mod host_slow_death_kill_log;

#[path = "host_spy_satellite_ping_log.rs"]
pub mod host_spy_satellite_ping_log;

#[path = "host_sticky_booby_attach_log.rs"]
pub mod host_sticky_booby_attach_log;

#[path = "host_stinger_hive_log.rs"]
pub mod host_stinger_hive_log;

#[path = "host_structure_collapse_kill_log.rs"]
pub mod host_structure_collapse_kill_log;

#[path = "host_structure_topple_crush_log.rs"]
pub mod host_structure_topple_crush_log;

#[path = "host_structure_topple_kill_log.rs"]
pub mod host_structure_topple_kill_log;

#[path = "host_topple_kill_log.rs"]
pub mod host_topple_kill_log;

#[path = "host_toxin_stream_projectile_log.rs"]
pub mod host_toxin_stream_projectile_log;
