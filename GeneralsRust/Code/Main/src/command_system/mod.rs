//! Wave 958: host_object dual-read seal (tests + residual).

// Restricted re-exports so impl submodules can `use super::*;`
// without dumping the parent crate surface through `pub use`.
pub(in crate::command_system) use crate::game_logic::{
    AIState, BuildingType, CapturePowerKind, DockKind, GameLogic, KindOf, Object, ObjectId, Team,
};
pub(in crate::command_system) use glam::{Vec2, Vec3};
pub(in crate::command_system) use serde::{Deserialize, Serialize};
pub(in crate::command_system) use std::collections::{HashMap, VecDeque};
pub(in crate::command_system) use std::f32::consts::TAU;
pub(in crate::command_system) use std::sync::{Mutex, OnceLock};
pub(in crate::command_system) use std::time::{Duration, Instant, SystemTime};

mod helpers;
pub use helpers::*;
mod types;
pub use types::*;
mod special_power;
pub use special_power::*;
mod command;
pub use command::*;
mod presentation;
pub use presentation::*;
mod system;
pub use system::*;
mod system_impl;
pub use system_impl::*;
mod commandable;
pub use commandable::*;
mod button_map;
pub use button_map::*;
mod record_tap;
pub use record_tap::*;

#[cfg(test)]
mod tests;

/// Concatenated live command_system sources for residual `include_str` scans.
pub const COMMAND_SYSTEM_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("button_map.rs"),
    include_str!("command.rs"),
    include_str!("commandable.rs"),
    include_str!("helpers.rs"),
    include_str!("presentation.rs"),
    include_str!("special_power.rs"),
    include_str!("system.rs"),
    include_str!("system_impl.rs"),
    include_str!("types.rs"),
    include_str!("record_tap.rs"),
);
