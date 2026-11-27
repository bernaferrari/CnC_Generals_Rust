//! Command input, dispatch, and replay in the receiving application.

// Restricted re-exports so impl submodules can `use super::*;`
// without dumping the parent crate surface through `pub use`.
pub(in crate::command_system) use crate::game_logic::{
    AIState, BuildingType, CapturePowerKind, DockKind, GameLogic, KindOf, Object, ObjectId, Team,
};
pub(in crate::command_system) use glam::{Vec2, Vec3};
pub(in crate::command_system) use serde::{Deserialize, Serialize};
pub(in crate::command_system) use std::collections::{HashMap, VecDeque};
pub(in crate::command_system) use std::f32::consts::TAU;
pub(in crate::command_system) use std::sync::{Arc, Mutex, OnceLock};
pub(in crate::command_system) use std::time::{Duration, Instant, SystemTime};

mod helpers;
pub use helpers::*;
pub use generals_sim::command::{
    CommandEvaluateType, CommandResult, CommandType, DropTarget, GameCommand, GuardTarget,
    ModifierKeys, PowerTarget, SpecialPowerType, WeaponSlot, WeaponTarget,
    special_power_type_from_template_name,
};
use generals_sim::command::special_power_is_location_target_only;
#[cfg(test)]
use generals_sim::command::special_power_is_no_target;
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
