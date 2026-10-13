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

#[path = "host_fire_intent_log.rs"]
pub mod host_fire_intent_log;

#[path = "host_weapon_discharge_log.rs"]
pub mod host_weapon_discharge_log;

#[path = "host_fire_sound_loop_log.rs"]
pub mod host_fire_sound_loop_log;

#[path = "host_fire_spawn_log.rs"]
pub mod host_fire_spawn_log;

#[path = "host_heal_log.rs"]
pub mod host_heal_log;

#[path = "host_eva_log.rs"]
pub mod host_eva_log;

#[path = "movement_events.rs"]
pub(crate) mod movement_events;

#[path = "host_move_ambient_audio.rs"]
pub mod host_move_ambient_audio;

#[path = "host_owner_log.rs"]
pub mod host_owner_log;

#[path = "host_field_object_expire_log.rs"]
pub mod host_field_object_expire_log;

#[path = "host_fire_spread_log.rs"]
pub mod host_fire_spread_log;

#[path = "host_hacker_income_log.rs"]
pub mod host_hacker_income_log;
