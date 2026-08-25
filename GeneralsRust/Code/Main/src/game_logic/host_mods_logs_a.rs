//! Host log modules (A).
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

#[path = "host_a10_strike_drop_log.rs"]
pub mod host_a10_strike_drop_log;

#[path = "host_ai_decision_log.rs"]
pub mod host_ai_decision_log;

#[path = "host_ai_mood_log.rs"]
pub mod host_ai_mood_log;

#[path = "host_ai_request_log.rs"]
pub mod host_ai_request_log;

#[path = "host_anthrax_bomb_drop_log.rs"]
pub mod host_anthrax_bomb_drop_log;

#[path = "host_artillery_barrage_drop_log.rs"]
pub mod host_artillery_barrage_drop_log;

#[path = "host_body_damage_log.rs"]
pub mod host_body_damage_log;

#[path = "host_bounce_land_log.rs"]
pub mod host_bounce_land_log;

#[path = "host_carpet_bomb_drop_log.rs"]
pub mod host_carpet_bomb_drop_log;

#[path = "host_cluster_mines_drop_log.rs"]
pub mod host_cluster_mines_drop_log;

#[path = "host_combat_attack_log.rs"]
pub mod host_combat_attack_log;

#[path = "host_daisy_cutter_drop_log.rs"]
pub mod host_daisy_cutter_drop_log;

#[path = "host_damage_log.rs"]
pub mod host_damage_log;
pub use host_damage_log::{HostDamageEvent, drain as drain_host_damage_events};

#[path = "host_attacked_by_log.rs"]
pub mod host_attacked_by_log;

#[path = "host_voice_fear_log.rs"]
pub mod host_voice_fear_log;

#[path = "host_death_type_log.rs"]
pub mod host_death_type_log;

#[path = "host_ai_attitude_log.rs"]
pub mod host_ai_attitude_log;

#[path = "host_ai_attitude_ready_log.rs"]
pub mod host_ai_attitude_ready_log;

#[path = "host_ai_mood_ready_log.rs"]
pub mod host_ai_mood_ready_log;

#[path = "host_ai_request_ready_log.rs"]
pub mod host_ai_request_ready_log;

#[path = "host_ai_state_log.rs"]
pub mod host_ai_state_log;

#[path = "host_ai_state_ready_log.rs"]
pub mod host_ai_state_ready_log;

#[path = "host_attack_log.rs"]
pub mod host_attack_log;

#[path = "host_attack_target_ready_log.rs"]
pub mod host_attack_target_ready_log;

#[path = "host_body_damage_ready_log.rs"]
pub mod host_body_damage_ready_log;

#[path = "host_bounce_land_ready_log.rs"]
pub mod host_bounce_land_ready_log;

#[path = "host_building_type_log.rs"]
pub mod host_building_type_log;

#[path = "host_building_type_ready_log.rs"]
pub mod host_building_type_ready_log;

#[path = "host_combat_attack_ready_log.rs"]
pub mod host_combat_attack_ready_log;

#[path = "host_combat_status_ready_log.rs"]
pub mod host_combat_status_ready_log;

#[path = "host_command_set_log.rs"]
pub mod host_command_set_log;

#[path = "host_command_set_ready_log.rs"]
pub mod host_command_set_ready_log;

#[path = "host_construction_complete_clear_ready_log.rs"]
pub mod host_construction_complete_clear_ready_log;

#[path = "host_construction_log.rs"]
pub mod host_construction_log;

#[path = "host_construction_progress_log.rs"]
pub mod host_construction_progress_log;

#[path = "host_construction_ready_log.rs"]
pub mod host_construction_ready_log;

#[path = "host_contain_capacity_log.rs"]
pub mod host_contain_capacity_log;

#[path = "host_contain_log.rs"]
pub mod host_contain_log;

#[path = "host_contain_ready_log.rs"]
pub mod host_contain_ready_log;

#[path = "host_continuous_fire_log.rs"]
pub mod host_continuous_fire_log;

#[path = "host_continuous_fire_ready_log.rs"]
pub mod host_continuous_fire_ready_log;

#[path = "host_crush_vision_log.rs"]
pub mod host_crush_vision_log;

#[path = "host_crush_vision_ready_log.rs"]
pub mod host_crush_vision_ready_log;

#[path = "host_death_type_ready_log.rs"]
pub mod host_death_type_ready_log;

#[path = "host_demo_mine_cheer_log.rs"]
pub mod host_demo_mine_cheer_log;

#[path = "host_demo_mine_cheer_ready_log.rs"]
pub mod host_demo_mine_cheer_ready_log;

#[path = "host_destroy_log.rs"]
pub mod host_destroy_log;

#[path = "host_destroy_ready_log.rs"]
pub mod host_destroy_ready_log;

#[path = "host_detector_log.rs"]
pub mod host_detector_log;

#[path = "host_detector_ready_log.rs"]
pub mod host_detector_ready_log;

#[path = "host_disable_timers_log.rs"]
pub mod host_disable_timers_log;

#[path = "host_disable_timers_ready_log.rs"]
pub mod host_disable_timers_ready_log;

#[path = "host_disguise_log.rs"]
pub mod host_disguise_log;

#[path = "host_disguise_ready_log.rs"]
pub mod host_disguise_ready_log;

#[path = "host_economy_log.rs"]
pub mod host_economy_log;
pub use host_economy_log::{HostEconomyEvent, drain as drain_host_economy_events};

#[path = "host_actively_constructing_log.rs"]
pub mod host_actively_constructing_log;

#[path = "host_angry_mob_member_follow_log.rs"]
pub mod host_angry_mob_member_follow_log;

#[path = "host_angry_mob_projectile_log.rs"]
pub mod host_angry_mob_projectile_log;

#[path = "host_aurora_bomb_projectile_log.rs"]
pub mod host_aurora_bomb_projectile_log;

#[path = "host_auto_deposit_log.rs"]
pub mod host_auto_deposit_log;

#[path = "host_battlemaster_horde_log.rs"]
pub mod host_battlemaster_horde_log;

#[path = "host_cannon_shell_projectile_log.rs"]
pub mod host_cannon_shell_projectile_log;

#[path = "host_china_infantry_horde_log.rs"]
pub mod host_china_infantry_horde_log;

#[path = "host_dozer_bored_log.rs"]
pub mod host_dozer_bored_log;
