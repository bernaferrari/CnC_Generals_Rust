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

#[path = "host_damage_log.rs"]
pub mod host_damage_log;
pub use host_damage_log::HostDamageEvent;

#[path = "host_voice_fear_log.rs"]
pub mod host_voice_fear_log;

#[path = "host_attack_log.rs"]
pub mod host_attack_log;

#[path = "host_construction_log.rs"]
pub mod host_construction_log;

#[path = "host_destroy_log.rs"]
pub mod host_destroy_log;

#[path = "host_disable_timers_log.rs"]
pub mod host_disable_timers_log;

#[path = "host_economy_log.rs"]
pub mod host_economy_log;
pub use host_economy_log::HostEconomyEvent;

#[path = "host_auto_deposit_log.rs"]
pub mod host_auto_deposit_log;
