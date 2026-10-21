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

#[path = "host_production_log.rs"]
pub mod host_production_log;

#[cfg(test)]
#[path = "host_production_progress_log.rs"]
pub mod host_production_progress_log;

#[path = "host_production_spawn_ready_log.rs"]
pub mod host_production_spawn_ready_log;

#[path = "host_radar_log.rs"]
pub mod host_radar_log;

#[path = "host_spawn_log.rs"]
pub mod host_spawn_log;

#[path = "host_status_log.rs"]
pub mod host_status_log;
